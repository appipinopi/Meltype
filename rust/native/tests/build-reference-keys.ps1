# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
param([string]$OutDir)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
if (-not $OutDir) { $OutDir = Join-Path $root 'dist/key-reference' }
$OutDir = [IO.Path]::GetFullPath($OutDir)
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$baseline = '45e14f86558aa6ea34f2c2ecd038ba95ca8c3c46'
$object = "${baseline}:native/tip/TextService.cpp" | & git -C $root cat-file --batch-check
if ($LASTEXITCODE -ne 0) { throw 'Could not inspect the comparison object.' }
if ($object -match ' missing$') {
    # CI checkouts are shallow. Fetch the immutable comparison commit only.
    & git -C $root fetch --no-tags --depth=1 origin $baseline
    if ($LASTEXITCODE -ne 0) { throw 'Could not fetch the upstream comparison commit.' }
}
$original = (& git -C $root show "${baseline}:native/tip/TextService.cpp") -join "`n"
if ($LASTEXITCODE -ne 0) { throw 'Could not read the original key helpers.' }
$first = $original.IndexOf('bool IsModifier(')
$last = $original.IndexOf('std::string RandomId()', $first)
if ($first -lt 0 -or $last -le $first) { throw 'Original key-helper boundaries changed.' }
$helpers = $original.Substring($first, $last - $first)
$source = Join-Path $OutDir 'reference_keys.cpp'
$prefix = @"
// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
// Test only: verbatim key helper block from upstream $baseline.
#include <windows.h>
"@
$exports = @'
extern "C" __declspec(dllexport) unsigned int meltype_reference_key_flags(unsigned int key,unsigned long long message) {
    LPARAM param=static_cast<LPARAM>(static_cast<ULONG_PTR>(message));
    return (IsModifier(key)?1:0)|(IsHankakuZenkaku(key)?2:0)|(IsAltGraveToggle(key,param)?4:0)|(IsCompositionShortcut(key)?8:0);
}
extern "C" __declspec(dllexport) unsigned int meltype_reference_key_char(unsigned int key,unsigned long long message) {
    return CharOf(key,static_cast<LPARAM>(static_cast<ULONG_PTR>(message)));
}
extern "C" __declspec(dllexport) int meltype_reference_key_down(int key) {return KeyDown(key)?1:0;}
'@
[IO.File]::WriteAllText($source,$prefix+"`n"+$helpers+"`n"+$exports,[Text.UTF8Encoding]::new($false))
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) { throw 'MSVC tools are required for the test-only upstream key helpers.' }
$vcvars = Join-Path $vs 'VC/Auxiliary/Build/vcvarsall.bat'
foreach ($arch in 'x64','x86') {
    $vcarch = if ($arch -eq 'x64') { 'x64' } else { 'x64_x86' }
    $cmd = "`"$vcvars`" $vcarch >nul && cl /nologo /utf-8 /std:c++17 /EHsc /DUNICODE /D_UNICODE /LD `"$source`" /Fo`"$OutDir/reference-$arch.obj`" /link /OUT:`"$OutDir/reference-$arch.dll`" /IMPLIB:`"$OutDir/reference-$arch.lib`" user32.lib"
    cmd /c $cmd
    if ($LASTEXITCODE -ne 0) { throw "Reference key helpers failed: $arch" }
}
