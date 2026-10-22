# SPDX-License-Identifier: GPL-3.0-or-later
# Read-only Inspect and malformed package tests. Never registers an IME.
$ErrorActionPreference = 'Stop'
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('meltype-package-test-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
$manifest = @{}
foreach ($name in @('meltype-flash.exe','meltype-rs.exe','tip/x64/MeltypeTip.dll','tip/x86/MeltypeTip.dll',
    'tip/x64/meltype-tip-worker.exe','tip/x86/meltype-tip-worker.exe','Manage-Windows.ps1','Start-Portable.ps1')) {
    $path = Join-Path $fixture $name
    New-Item -ItemType Directory -Path (Split-Path -Parent $path) -Force | Out-Null
    [IO.File]::WriteAllText($path, 'controlled fixture: '+$name)
    if ($name.EndsWith('MeltypeTip.dll')) {
        $identityBytes = [IO.File]::ReadAllBytes($path) + ([Guid]'C540BAF6-88B1-5B30-B524-2C8702B034FC').ToByteArray() + ([Guid]'6D01DE9C-8AB3-5CFC-A5C6-10893CDF8762').ToByteArray()
        [IO.File]::WriteAllBytes($path, [byte[]]$identityBytes)
    }
    $manifest[$name] = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
}
$manifestPath = Join-Path $fixture 'SHA256SUMS.json'
$manifest | ConvertTo-Json | Set-Content -LiteralPath $manifestPath -Encoding UTF8
& (Join-Path $PSScriptRoot 'Manage-Windows.ps1') -Action Inspect -Package $fixture
& (Join-Path $PSScriptRoot 'Manage-Windows.ps1') -Action Install -Package $fixture -ReplaceExisting -WhatIf
function Expect-Rejection([string]$Pattern) {
    $rejected = $false
    try { & (Join-Path $PSScriptRoot 'Manage-Windows.ps1') -Action Inspect -Package $fixture }
    catch {
        if ($_.Exception.Message -notlike $Pattern) { throw }
        $rejected = $true
    }
    if (-not $rejected) { throw "Expected rejection: $Pattern" }
}
[IO.File]::WriteAllText((Join-Path $fixture 'meltype-rs.exe'), 'tampered')
Expect-Rejection '*SHA256 mismatch*'
$manifest.Remove('meltype-rs.exe')
$manifest | ConvertTo-Json | Set-Content -LiteralPath $manifestPath -Encoding UTF8
Expect-Rejection '*Missing manifest entry*'
$manifest['meltype-rs.exe'] = (Get-FileHash -LiteralPath (Join-Path $fixture 'meltype-rs.exe') -Algorithm SHA256).Hash
$manifest['../outside.txt'] = 'bad'
$manifest | ConvertTo-Json | Set-Content -LiteralPath $manifestPath -Encoding UTF8
Expect-Rejection '*Path leaves root*'
Write-Host "PASS: valid hashes, tampering, missing required files, traversal. Fixture: $fixture"
