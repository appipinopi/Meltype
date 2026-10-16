#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
# Explicit installation. Never restart IBus or select an input source.
set -euo pipefail
source_dir="$(cd "$(dirname "$0")" && pwd)"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
case "$data" in /*) ;; *) echo 'XDG_DATA_HOME must be absolute.' >&2; exit 1 ;; esac
system=false
prefix=/usr
uninstall=false
while [[ $# -gt 0 ]]; do
    case "$1" in
        --system) system=true; shift ;;
        --prefix) [[ $# -ge 2 ]] || exit 1; prefix="$2"; shift 2 ;;
        --uninstall) uninstall=true; shift ;;
        *) echo 'Usage: bash install.sh [--system [--prefix /absolute/path]] [--uninstall]' >&2; exit 1 ;;
    esac
done
if $system; then
    case "$prefix" in /*) ;; *) echo 'Prefix must be absolute.' >&2; exit 1 ;; esac
    target="$prefix/libexec/meltype-rust/ibus"
    components="$prefix/share/ibus/component"
else
    target="$data/MeltypeRust/ibus"
    components="$data/ibus/component"
fi
component="$components/meltype-rust.xml"
if [[ -L "$target" || -L "$target/mozc" || -L "$component" ]]; then
    echo 'Refusing to replace a symlink at the installation target.' >&2
    exit 1
fi
if $uninstall; then
    if [[ ! -f "$target/.meltype-rust-ibus" ]]; then
        echo 'No owned Rust IBus installation found.' >&2
        exit 1
    fi
    if [[ -e "$component" ]] && ! grep -q '<name>org.freedesktop.IBus.MeltypeRust</name>' "$component"; then
        echo 'Component file belongs to another application; keeping it.' >&2
        exit 1
    fi
    rm -f -- "$component"
    for file in meltype-ibus meltype-settings meltype-rs LICENSE THIRD-PARTY-NOTICES.md README.md MIGRATION.md install.sh .meltype-rust-ibus; do
        rm -f -- "$target/$file"
    done
    for file in meltype_mozc_helper MOZC-LICENSE.txt MOZC-CREDITS.html; do
        rm -f -- "$target/mozc/$file"
    done
    rmdir -- "$target/mozc" 2>/dev/null || true
    rmdir -- "$target" 2>/dev/null || true
    echo 'Removed the Rust IBus program and component. Settings and learning data remain.'
    exit 0
fi
for file in meltype-ibus meltype-settings meltype-rs; do
    [[ -f "$source_dir/$file" ]] || { echo "Missing $file" >&2; exit 1; }
done
mkdir -p "$target" "$components"
for file in meltype-ibus meltype-settings meltype-rs LICENSE THIRD-PARTY-NOTICES.md README.md MIGRATION.md install.sh; do
    if [[ -f "$source_dir/$file" ]]; then cp -- "$source_dir/$file" "$target/$file"; fi
done
chmod 755 "$target/meltype-ibus" "$target/meltype-settings" "$target/meltype-rs"
if [[ -d "$source_dir/mozc" ]]; then
    mkdir -p "$target/mozc"
    for file in meltype_mozc_helper MOZC-LICENSE.txt MOZC-CREDITS.html; do
        if [[ -f "$source_dir/mozc/$file" ]]; then cp -- "$source_dir/mozc/$file" "$target/mozc/$file"; fi
    done
    if [[ -f "$target/mozc/meltype_mozc_helper" ]]; then chmod 755 "$target/mozc/meltype_mozc_helper"; fi
fi
printf 'Meltype Rust IBus\n' > "$target/.meltype-rust-ibus"
# Generate paths after copying: current_exe must refer to the installed binary.
temporary="$(mktemp "$components/.meltype-rust.XXXXXX")"
trap 'rm -f -- "$temporary"' EXIT
"$target/meltype-ibus" --print-component > "$temporary"
mv -- "$temporary" "$component"
if $system; then
    echo 'Installed. Sign out/in, then select Meltype Rust in IBus input sources.'
    printf 'Remove with: sudo bash %q --system --prefix %q --uninstall\n' "$target/install.sh" "$prefix"
else
    echo 'User files installed. IBus does not discover user components by default.'
    echo 'Configure IBUS_COMPONENT_PATH to include the following folder and the system component folders before starting IBus:'
    echo "$components"
    printf 'Remove with: bash %q --uninstall\n' "$target/install.sh"
fi
