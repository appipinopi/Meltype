#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/linux/build/Meltype-flash-linux"
target="${CARGO_TARGET_DIR:-$root/target}"
if [[ "$(uname -s)" != Linux ]]; then
    echo 'Run this script on Linux.' >&2
    exit 1
fi
cargo build --manifest-path "$root/Cargo.toml" --locked --release --workspace --target-dir "$target"
mkdir -p "$out"
cp "$target/release/meltype-ibus" "$target/release/meltype-flash" "$target/release/meltype-rs" "$out/"
cp "$root/LICENSE" "$root/THIRD-PARTY-NOTICES.md" "$root/rust/README.md" "$root/rust/MIGRATION.md" "$root/rust/ibus/install.sh" "$root/linux/icon.png" "$out/"
files=(meltype-ibus meltype-flash meltype-rs LICENSE THIRD-PARTY-NOTICES.md README.md MIGRATION.md install.sh icon.png)
if [[ -x "$root/native/mozc/bin/linux/meltype_mozc_helper" ]]; then
    mkdir -p "$out/mozc"
    cp "$root/native/mozc/bin/linux/meltype_mozc_helper" "$root/native/mozc/bin/linux/MOZC-LICENSE.txt" "$root/native/mozc/bin/linux/MOZC-CREDITS.html" "$out/mozc/"
    files+=(mozc/meltype_mozc_helper mozc/MOZC-LICENSE.txt mozc/MOZC-CREDITS.html)
else
    echo 'Mozc helper is absent; this bundle uses the limited bundled conversion dictionary.' >&2
fi
archive_files=()
for file in "${files[@]}"; do archive_files+=("Meltype-flash-linux/$file"); done
tar -czf "$root/linux/build/Meltype-flash-linux.tar.gz" -C "$root/linux/build" "${archive_files[@]}"
echo "Created: $out (not installed)"
echo "Archive: $root/linux/build/Meltype-flash-linux.tar.gz"
