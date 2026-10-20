# SPDX-License-Identifier: GPL-3.0-or-later
# Settings/Core preview without registration. TSF input needs machine installation.
[CmdletBinding()]
param([switch]$Tray)
$ErrorActionPreference = 'Stop'
$executable = Join-Path $PSScriptRoot 'meltype-settings.exe'
if (-not (Test-Path -LiteralPath $executable)) { throw 'Run from an extracted package.' }
$previous = $env:MELTYPE_RUST_DATA_DIR
try {
    $env:MELTYPE_RUST_DATA_DIR = Join-Path $PSScriptRoot 'data'
    New-Item -ItemType Directory -Path $env:MELTYPE_RUST_DATA_DIR -Force | Out-Null
    $arguments = @()
    if ($Tray) { $arguments += '--tray' }
    & $executable @arguments
} finally {
    $env:MELTYPE_RUST_DATA_DIR = $previous
}
