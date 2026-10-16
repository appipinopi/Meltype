#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/linux/build/Meltype-Rust-linux"
target="${CARGO_TARGET_DIR:-$root/target}"
if [[ "$(uname -s)" != Linux ]]; then
    echo 'Run this script on Linux.' >&2
    exit 1
fi
cargo build --manifest-path "$root/Cargo.toml" --locked --release --workspace --target-dir "$target"
mkdir -p "$out"
cp "$target/release/meltype-ibus" "$target/release/meltype-settings" "$target/release/meltype-rs" "$out/"
cp "$root/LICENSE" "$root/THIRD-PARTY-NOTICES.md" "$root/rust/README.md" "$root/rust/MIGRATION.md" "$root/rust/ibus/install.sh" "$out/"
if [[ -x "$root/native/mozc/bin/linux/meltype_mozc_helper" ]]; then
    mkdir -p "$out/mozc"
    cp "$root/native/mozc/bin/linux/meltype_mozc_helper" "$root/native/mozc/bin/linux/MOZC-LICENSE.txt" "$root/native/mozc/bin/linux/MOZC-CREDITS.html" "$out/mozc/"
else
    echo 'Mozc helper is absent; this bundle uses the limited bundled conversion dictionary.' >&2
fi
echo "Created: $out (not installed)"
