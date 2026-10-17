# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$baseline='45e14f86558aa6ea34f2c2ecd038ba95ca8c3c46'
$object="${baseline}:native/tip/Json.cpp" | & git -C $root cat-file --batch-check
if ($LASTEXITCODE -ne 0) {throw 'Could not inspect JSON comparison object.'}
if ($object -match ' missing$') {
    & git -C $root fetch --no-tags --depth=1 origin $baseline
    if ($LASTEXITCODE -ne 0) {throw 'Could not fetch JSON comparison commit.'}
}
$original=(& git -C $root show "${baseline}:native/tip/Json.cpp") -join "`n"
if ($LASTEXITCODE -ne 0 -or !$original.Contains('class Parser {') -or !$original.Contains('std::string JsonString(')) {throw 'Original JSON comparison source unavailable.'}
# Use the current, unchanged value layout/UTF conversion declarations in one TU;
# parser/quoting implementations below are otherwise the immutable upstream code.
$original=[regex]::Replace($original,'(?m)^#include[^\r\n]*\r?\n','')
$dll=(& git -C $root show "${baseline}:native/tip/Dll.cpp") -join "`n"
if ($LASTEXITCODE -ne 0) {throw 'Original UTF conversion helpers unavailable.'}
$first=$dll.IndexOf('std::string ToUtf8(')
$last=$dll.IndexOf('void TipLog(', $first)
if ($first -lt 0 -or $last -le $first) {throw 'Original UTF conversion boundaries changed.'}
$helpers=$dll.Substring($first,$last-$first)
$original="namespace meltype {`n$helpers`n}`n$original"
$out=Join-Path $root 'dist/json-reference'
New-Item -ItemType Directory -Force -Path $out | Out-Null
[IO.File]::WriteAllText((Join-Path $out 'original.inc'),$original,[Text.UTF8Encoding]::new($false))
