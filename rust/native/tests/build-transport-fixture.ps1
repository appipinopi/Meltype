# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$out=Join-Path $root 'dist/tip-transport-fixture'
New-Item -ItemType Directory -Force -Path $out | Out-Null
foreach ($arch in 'x64','x86') {
    $target=if ($arch -eq 'x64') {'x86_64-pc-windows-msvc'} else {'i686-pc-windows-msvc'}
    & rustc --edition 2021 --target $target -O (Join-Path $PSScriptRoot 'support/tip_transport_fixture.rs') -o (Join-Path $out "fixture-$arch.exe")
    if ($LASTEXITCODE -ne 0) {throw "Transport fixture build failed: $arch"}
}
