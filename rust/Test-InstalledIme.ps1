# SPDX-License-Identifier: GPL-3.0-or-later
# Exercise a real machine registration, then restore the original IME.
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$Package,
    [Parameter(Mandatory=$true)][string]$ReportDirectory,
    [switch]$ExerciseRegistration,
    [switch]$ProbeOnly
)
$ErrorActionPreference = 'Stop'
$clsid = '{417D801B-A9BD-4C26-BD16-356A825A6998}'
$profile = '{21F643F4-72D5-4946-BF70-0A126AB52D05}'
$windows = [Environment]::GetFolderPath('Windows')
if ($ProbeOnly) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class InstalledImeProbe {
    [DllImport("ole32.dll")] static extern int CoInitializeEx(IntPtr p, uint flags);
    [DllImport("ole32.dll")] static extern void CoUninitialize();
    [DllImport("ole32.dll")] static extern int CoCreateInstance(ref Guid clsid, IntPtr outer, uint context, ref Guid iid, out IntPtr value);
    public static void Run() {
        int init = CoInitializeEx(IntPtr.Zero, 2);
        Marshal.ThrowExceptionForHR(init);
        try {
            Guid clsid = new Guid("417D801B-A9BD-4C26-BD16-356A825A6998");
            Guid iid = new Guid("aa80e7f7-2021-11d2-93e0-0060b067b86e");
            IntPtr value;
            Marshal.ThrowExceptionForHR(CoCreateInstance(ref clsid, IntPtr.Zero, 1, ref iid, out value));
            if (value == IntPtr.Zero) throw new Exception("Null ITfTextInputProcessor");
            Marshal.Release(value);
        } finally { CoUninitialize(); }
    }
}
'@
    [InstalledImeProbe]::Run()
    Write-Host "PASS: registered ITfTextInputProcessor COM activation ($([IntPtr]::Size*8)-bit)"
    return
}
$packageRoot = (Resolve-Path -LiteralPath $Package).Path
& (Join-Path $packageRoot 'Manage-Windows.ps1') -Action Inspect -Package $packageRoot
if (-not $?) { throw 'Invalid package' }
function Server([Microsoft.Win32.RegistryView]$View) {
    $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::LocalMachine,$View)
    try {
        $key = $base.OpenSubKey("SOFTWARE\Classes\CLSID\$clsid\InprocServer32")
        if ($null -eq $key) { return $null }
        try { return [string]$key.GetValue('') } finally { $key.Dispose() }
    } finally { $base.Dispose() }
}
$old = @{ x64 = (Server ([Microsoft.Win32.RegistryView]::Registry64)); x86 = (Server ([Microsoft.Win32.RegistryView]::Registry32)) }
if (-not $ExerciseRegistration) {
    $old | ConvertTo-Json
    Write-Host 'Read-only inspection. -ExerciseRegistration temporarily installs Rust and restores these original registrations.'
    return
}
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    $shell = Join-Path $windows 'System32/WindowsPowerShell/v1.0/powershell.exe'
    $arguments = @('-NoProfile','-ExecutionPolicy','Bypass','-File',('"'+$PSCommandPath+'"'),'-Package',('"'+$packageRoot+'"'),'-ReportDirectory',('"'+[IO.Path]::GetFullPath($ReportDirectory)+'"'),'-ExerciseRegistration')
    $child = Start-Process -FilePath $shell -Verb RunAs -ArgumentList $arguments -WindowStyle Hidden -Wait -PassThru
    try { if ($child.ExitCode -ne 0) { throw "Elevated registration test failed: $($child.ExitCode). See $ReportDirectory" } } finally { $child.Dispose() }
    return
}
if (-not [Environment]::Is64BitProcess) { throw 'Run the registration exercise in 64-bit PowerShell.' }
foreach ($arch in 'x64','x86') {
    if (-not $old[$arch] -or -not (Test-Path -LiteralPath $old[$arch] -PathType Leaf)) { throw "Original $arch IME DLL required for restoration." }
}
$machine = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::LocalMachine,[Microsoft.Win32.RegistryView]::Registry64)
try {
    $key = $machine.OpenSubKey('SOFTWARE\Microsoft\Windows\CurrentVersion')
    try { $installRoot = Join-Path ([string]$key.GetValue('ProgramFilesDir')) 'MeltypeRust' } finally { $key.Dispose() }
} finally { $machine.Dispose() }
if (Test-Path -LiteralPath (Join-Path $installRoot 'installed.json')) {
    $prior = Get-Content -LiteralPath (Join-Path $installRoot 'installed.json') -Raw | ConvertFrom-Json
    $versions = Join-Path $installRoot 'versions'
    if ($prior.unregistered -ne $true -or @((Get-ChildItem -LiteralPath $versions -File -Recurse -Force -ErrorAction SilentlyContinue)).Count -ne 0) { throw 'Existing managed Rust installation: exercise refused to preserve it.' }
    foreach ($arch in 'x64','x86') { if ($old[$arch].StartsWith($installRoot+'\',[StringComparison]::OrdinalIgnoreCase)) { throw 'Rust IME still registered; exercise refused.' } }
}
New-Item -ItemType Directory -Path $ReportDirectory -ErrorAction Stop | Out-Null
$report = (Resolve-Path -LiteralPath $ReportDirectory).Path
Start-Transcript -LiteralPath (Join-Path $report 'registration.log') | Out-Null
$manager = Join-Path $packageRoot 'Manage-Windows.ps1'
$restored = $false; $attempted = $false; $passed = $false
$backups = @()
try {
    $snapshot = @{}
    foreach ($arch in 'x64','x86') { $snapshot[$arch] = @{ path=$old[$arch]; sha256=(Get-FileHash -LiteralPath $old[$arch]).Hash } }
    $snapshot | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $report 'original.json') -Encoding UTF8
    # Export the exact shared COM/TIP keys, including original profile flags.
    foreach ($bits in 64,32) {
        foreach ($entry in @(@{name='com';path="HKLM\SOFTWARE\Classes\CLSID\$clsid"},@{name='tip';path="HKLM\SOFTWARE\Microsoft\CTF\TIP\$clsid"})) {
            $file = Join-Path $report "$($entry.name)-$bits.reg"
            & "$windows/System32/reg.exe" export $entry.path $file /y "/reg:$bits" | Out-Null
            if ($LASTEXITCODE -ne 0 -and $entry.name -eq 'com') { throw 'Original COM registry export failed' }
            if ($LASTEXITCODE -eq 0) { $backups += @{file=$file;bits=$bits} }
        }
    }
    $attempted = $true
    & $manager -Action Install -Package $packageRoot -ReplaceExisting -SkipCurrentUserStartup
    $state = Get-Content -LiteralPath (Join-Path $installRoot 'installed.json') -Raw | ConvertFrom-Json
    foreach ($entry in @(@{arch='x64';view=[Microsoft.Win32.RegistryView]::Registry64;folder='System32'},@{arch='x86';view=[Microsoft.Win32.RegistryView]::Registry32;folder='SysWOW64'})) {
        if ((Server $entry.view) -ne [string]$state.($entry.arch)) { throw "Registered DLL mismatch: $($entry.arch)" }
        $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::LocalMachine,$entry.view)
        try {
            $key = $base.OpenSubKey("SOFTWARE\Microsoft\CTF\TIP\$clsid\LanguageProfile\0x00000411\$profile")
            if ($null -eq $key) { throw "Missing real TSF language profile: $($entry.arch)" }
            $key.Dispose()
        } finally { $base.Dispose() }
        $shell = Join-Path $windows "$($entry.folder)/WindowsPowerShell/v1.0/powershell.exe"
        & $shell -NoProfile -ExecutionPolicy Bypass -File $PSCommandPath -Package $packageRoot -ReportDirectory $report -ProbeOnly
        if ($LASTEXITCODE -ne 0) { throw "Registered COM activation failed: $($entry.arch)" }
    }
    $passed = $true
} finally {
    try {
        if ($attempted) {
            # Remove the test profile before re-registering the original server.
            if (Test-Path -LiteralPath (Join-Path $installRoot 'installed.json')) {
                try { & $manager -Action Uninstall -SkipCurrentUserStartup } catch { Write-Warning ('Test uninstall failed; restoring original servers regardless: '+$_.Exception.Message) }
            }
            foreach ($entry in @(@{arch='x64';folder='System32'},@{arch='x86';folder='SysWOW64'})) {
                $child = Start-Process -FilePath "$windows/$($entry.folder)/regsvr32.exe" -ArgumentList @('/s',('"'+$old[$entry.arch]+'"')) -WindowStyle Hidden -Wait -PassThru
                try { if ($child.ExitCode -ne 0) { throw "Original IME restore failed: $($entry.arch)" } } finally { $child.Dispose() }
            }
            foreach ($backup in $backups) {
                & "$windows/System32/reg.exe" import $backup.file "/reg:$($backup.bits)" | Out-Null
                if ($LASTEXITCODE -ne 0) { throw 'Original registry restore failed' }
            }
            if ((Server ([Microsoft.Win32.RegistryView]::Registry64)) -ne $old.x64 -or (Server ([Microsoft.Win32.RegistryView]::Registry32)) -ne $old.x86) { throw 'Restored DLL paths differ from snapshot' }
        }
        $restored = $true
    } finally {
        @{registeredComAndProfilePassed=$passed;originalRestored=$restored;externalApplicationInputTested=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $report 'result.json') -Encoding UTF8
        Stop-Transcript | Out-Null
    }
}
Write-Host "PASS: temporary x64/x86 registration, TSF profile, COM activation, original restoration. Report: $report"
