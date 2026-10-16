#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cargo build --manifest-path "$root/Cargo.toml" --locked --release --workspace --target-dir "$root/target"
case "$(uname -s)" in
  Darwin)
    MELTYPE_NATIVE_LIBRARY="$root/target/release/libmeltype_native.dylib" \
      MELTYPE_VERSION=0.1.0 bash "$root/mac/build.sh" --no-install
    ;;
  Linux)
    MELTYPE_NATIVE_LIBRARY="$root/target/release/libmeltype_native.so" bash "$root/linux/build.sh"
    ;;
  *) echo 'Run rust/build-windows.ps1 on Windows.' >&2; exit 1 ;;
esac
