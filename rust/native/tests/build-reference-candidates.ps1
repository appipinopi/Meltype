# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
param([string]$OutDir)
$ErrorActionPreference = 'Stop'
if (-not $OutDir) { $OutDir = Join-Path $PSScriptRoot '../../../dist/candidate-reference' }
$OutDir = [IO.Path]::GetFullPath($OutDir)
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) { throw 'MSVC tools are required for the test-only upstream renderer.' }
$vcvars = Join-Path $vs 'VC/Auxiliary/Build/vcvarsall.bat'
foreach ($arch in 'x64','x86') {
    $vcarch = if ($arch -eq 'x64') { 'x64' } else { 'x64_x86' }
    $source = Join-Path $PSScriptRoot 'reference_candidates.cpp'
    $cmd = "`"$vcvars`" $vcarch >nul && cl /nologo /utf-8 /std:c++17 /EHsc /DUNICODE /D_UNICODE /LD `"$source`" /Fo`"$OutDir/reference-$arch.obj`" /link /OUT:`"$OutDir/reference-$arch.dll`" /IMPLIB:`"$OutDir/reference-$arch.lib`" ole32.lib oleaut32.lib uuid.lib user32.lib gdi32.lib advapi32.lib d2d1.lib dwrite.lib dwmapi.lib"
    cmd /c $cmd
    if ($LASTEXITCODE -ne 0) { throw "Reference renderer failed: $arch" }
}
