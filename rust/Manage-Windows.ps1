# SPDX-License-Identifier: GPL-3.0-or-later
# Versioned installation; personal data and old versions are retained.
[CmdletBinding(SupportsShouldProcess=$true)]
param(
    [ValidateSet('Inspect','Install','Update','Uninstall')][string]$Action = 'Inspect',
    [string]$Package = $PSScriptRoot,
    [switch]$ReplaceExisting
)
$ErrorActionPreference = 'Stop'
$clsid = '{417D801B-A9BD-4C26-BD16-356A825A6998}'
function Get-Server([Microsoft.Win32.RegistryView]$View) {
    $hive = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::LocalMachine, $View)
    try {
        $key = $hive.OpenSubKey("SOFTWARE\Classes\CLSID\$clsid\InprocServer32")
        if ($null -eq $key) { return $null }
        try { return [string]$key.GetValue('') } finally { $key.Dispose() }
    } finally { $hive.Dispose() }
}
function Assert-Child([string]$Base, [string]$Path) {
    $full = [IO.Path]::GetFullPath($Path)
    $prefix = [IO.Path]::GetFullPath($Base).TrimEnd('\') + '\'
    if (-not $full.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) { throw "Path leaves root: $Path" }
    return $full
}
function Test-Package([string]$Root) {
    $manifest = Get-Content -LiteralPath (Join-Path $Root 'SHA256SUMS.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    foreach ($name in @('meltype-settings.exe','meltype-rs.exe','tip/x64/MeltypeTip.dll','tip/x86/MeltypeTip.dll',
        'tip/x64/meltype-tip-worker.exe','tip/x86/meltype-tip-worker.exe','Manage-Windows.ps1','Start-Portable.ps1')) {
        if ($manifest.PSObject.Properties.Name -notcontains $name) { throw "Missing manifest entry: $name" }
    }
    foreach ($item in $manifest.PSObject.Properties) {
        $path = Assert-Child $Root (Join-Path $Root $item.Name)
        $cursor = Get-Item -LiteralPath $path
        while ($cursor.FullName -ne $Root) {
            if ($cursor.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Reparse point: $path" }
            $cursor = Get-Item -LiteralPath (Split-Path -Parent $cursor.FullName)
            if ($null -eq $cursor) { throw "Invalid package parent: $path" }
        }
        if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $item.Value) { throw "SHA256 mismatch: $($item.Name)" }
    }
    return $manifest
}
$packageRoot = (Resolve-Path -LiteralPath $Package).Path.TrimEnd('\')
if ($Action -ne 'Uninstall') {
    $manifest = Test-Package $packageRoot
    Write-Host "Package hashes verified: $packageRoot"
    if ($Action -eq 'Inspect') { return }
}
$machine = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::LocalMachine, [Microsoft.Win32.RegistryView]::Registry64)
try {
    $key = $machine.OpenSubKey('SOFTWARE\Microsoft\Windows\CurrentVersion')
    try { $programFiles = [string]$key.GetValue('ProgramFilesDir') } finally { $key.Dispose() }
} finally { $machine.Dispose() }
$installRoot = [IO.Path]::GetFullPath((Join-Path $programFiles 'MeltypeRust'))
$statePath = Assert-Child $installRoot (Join-Path $installRoot 'installed.json')
$state = if (Test-Path -LiteralPath $statePath) { Get-Content -LiteralPath $statePath -Raw | ConvertFrom-Json } else { $null }
$old = @{ x64 = (Get-Server ([Microsoft.Win32.RegistryView]::Registry64)); x86 = (Get-Server ([Microsoft.Win32.RegistryView]::Registry32)) }
if ($Action -eq 'Update' -and $null -eq $state) { throw 'No managed installation. Use Install first.' }
if ($Action -ne 'Uninstall') {
    foreach ($arch in 'x64','x86') {
        if ($old[$arch] -and -not $old[$arch].StartsWith($installRoot+'\',[StringComparison]::OrdinalIgnoreCase) -and -not $ReplaceExisting) {
            throw "Another Meltype is registered: $($old[$arch]). Back up its data; use -ReplaceExisting to replace the shared registration."
        }
    }
}
if (-not $PSCmdlet.ShouldProcess($installRoot, "$Action Meltype Rust IME")) { return }
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Run in an administrator PowerShell.' }
$windows = [Environment]::GetFolderPath('Windows')
$reg64 = Join-Path $windows 'System32\regsvr32.exe'
if (-not [Environment]::Is64BitProcess) { $reg64 = Join-Path $windows 'Sysnative\regsvr32.exe' }
function Register-Server([string]$Arch,[string]$Dll,[switch]$Remove) {
    $exe = if ($Arch -eq 'x64') { $reg64 } else { Join-Path $windows 'SysWOW64\regsvr32.exe' }
    $arguments = @('/s')
    if ($Remove) { $arguments += '/u' }
    $arguments += '"'+$Dll+'"'
    $process = Start-Process -FilePath $exe -ArgumentList $arguments -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "regsvr32 $Arch failed: $($process.ExitCode)" }
}
if ($Action -eq 'Uninstall') {
    if ($null -eq $state) { throw 'No managed installation; nothing removed.' }
    foreach ($arch in 'x64','x86') {
        $dll = Assert-Child $installRoot ([string]$state.$arch)
        if ($old[$arch] -and $old[$arch] -ne $dll) { throw "Registration changed externally: $($old[$arch])" }
    }
    foreach ($arch in 'x64','x86') { Register-Server $arch ([string]$state.$arch) -Remove }
    Write-Host "IME unregistered. Close the tray and disable its startup setting. Files remain at $installRoot for removal after applications close/reboot. User data is retained."
    return
}
$stamp = (Get-FileHash -LiteralPath (Join-Path $packageRoot 'SHA256SUMS.json') -Algorithm SHA256).Hash.Substring(0,16)
$versionRoot = Assert-Child $installRoot (Join-Path $installRoot "versions\$stamp")
if (Test-Path -LiteralPath (Join-Path $versionRoot 'SHA256SUMS.json')) {
    # A live DLL need not be overwritten when reinstalling the identical package.
    $null = Test-Package $versionRoot
    if ((Get-FileHash -LiteralPath (Join-Path $versionRoot 'SHA256SUMS.json') -Algorithm SHA256).Hash -ne
        (Get-FileHash -LiteralPath (Join-Path $packageRoot 'SHA256SUMS.json') -Algorithm SHA256).Hash) {
        throw 'Existing version folder belongs to a different package.'
    }
} else {
    New-Item -ItemType Directory -Path $versionRoot -Force | Out-Null
    foreach ($item in $manifest.PSObject.Properties) {
        $destination = Assert-Child $versionRoot (Join-Path $versionRoot $item.Name)
        New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
        Copy-Item -LiteralPath (Join-Path $packageRoot $item.Name) -Destination $destination -Force
    }
    Copy-Item -LiteralPath (Join-Path $packageRoot 'SHA256SUMS.json') -Destination $versionRoot -Force
    $null = Test-Package $versionRoot
}
$attempted = @()
try {
    foreach ($arch in 'x64','x86') {
        $attempted += $arch
        Register-Server $arch (Join-Path $versionRoot "tip\$arch\MeltypeTip.dll")
    }
    $newState = @{ x64 = (Join-Path $versionRoot 'tip\x64\MeltypeTip.dll'); x86 = (Join-Path $versionRoot 'tip\x86\MeltypeTip.dll'); package = $versionRoot }
    $temporary = Join-Path $installRoot 'installed.json.tmp'
    $newState | ConvertTo-Json | Set-Content -LiteralPath $temporary -Encoding UTF8
    Move-Item -LiteralPath $temporary -Destination $statePath -Force
} catch {
    $failure = $_
    foreach ($arch in $attempted) {
        try {
            if ($old[$arch] -and (Test-Path -LiteralPath $old[$arch])) { Register-Server $arch $old[$arch] }
            else { Register-Server $arch (Join-Path $versionRoot "tip\$arch\MeltypeTip.dll") -Remove }
        } catch { Write-Warning "Rollback $arch failed: $_" }
    }
    throw $failure
}
Write-Host "Installed: $versionRoot. Select Meltype in Windows language settings; restart open applications."
Write-Host "Start tray: & '$versionRoot\meltype-settings.exe' --tray"
