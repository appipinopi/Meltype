# SPDX-License-Identifier: GPL-3.0-or-later
# Only query and invalid arguments are sent to the real executable. Never changes startup.
param([Parameter(Mandatory=$true)][string]$Binary)
$ErrorActionPreference = 'Stop'
$exe = (Resolve-Path -LiteralPath $Binary).Path
$root = Join-Path ([IO.Path]::GetTempPath()) ('meltype startup 日本語-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$previousDataDirectory = $env:MELTYPE_RUST_DATA_DIR
function Run-Probe([string]$Operation, [string]$Report, [int]$Expected) {
    $child = Start-Process -FilePath $exe -ArgumentList @('--startup-action', $Operation, '--startup-result', ('"'+$Report+'"')) -WindowStyle Hidden -PassThru -RedirectStandardError (Join-Path $root ($Operation+'.log'))
    try {
        if (-not $child.WaitForExit(30000)) { $child.Kill(); throw 'Headless command timed out or launched GUI' }
        if ($child.ExitCode -ne $Expected) { throw "Unexpected exit code: $($child.ExitCode)" }
    } finally { $child.Dispose() }
}
try {
    $env:MELTYPE_RUST_DATA_DIR = Join-Path $root 'unused-data'
    $report = Join-Path $root 'query.json'
    Run-Probe 'query' $report 0
    $result = Get-Content -LiteralPath $report -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($result.protocol -ne 1 -or $result.ok -ne $true -or $result.changed -ne $false -or $result.enabled -isnot [bool]) {
        throw 'Invalid startup query result'
    }
    $invalid = Join-Path $root 'invalid.json'
    Run-Probe 'invalid' $invalid 2
    if (Test-Path -LiteralPath $invalid) { throw 'Rejected command wrote a result' }
    if (Test-Path -LiteralPath $env:MELTYPE_RUST_DATA_DIR) { throw 'Command initialized user data' }
    Write-Host "PASS: actual headless read-only query, invalid command, no user-data initialization. Results: $root"
} finally { $env:MELTYPE_RUST_DATA_DIR = $previousDataDirectory }
