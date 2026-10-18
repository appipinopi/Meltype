#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
set -euo pipefail
[[ "$(uname -s)" == Darwin ]] || { echo 'macOS SDK required' >&2; exit 1; }
root="$(cd "$(dirname "$0")/../../.." && pwd)"
library="$1"
temporary="$(mktemp -d "${TMPDIR:-/tmp}/meltype-key-policy.XXXXXX")"
trap 'rm -rf "$temporary"' EXIT
app="$temporary/KeyPolicy.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Frameworks"
cp "$library" "$app/Contents/Frameworks/libMeltypeNative.dylib"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>CFBundleExecutable</key><string>KeyPolicy</string><key>CFBundleIdentifier</key><string>moe.meltype.tests.keypolicy</string><key>CFBundlePackageType</key><string>APPL</string><key>LSBackgroundOnly</key><true/><key>NSPrincipalClass</key><string>NSApplication</string><key>InputMethodServerControllerClass</key><string>MeltypePolicyController</string><key>InputMethodServerDelegateClass</key><string>MeltypePolicyController</string></dict></plist>
PLIST
plutil -insert InputMethodConnectionName -string "MeltypePolicy_$(basename "$temporary")" "$app/Contents/Info.plist"
swiftc -swift-version 5 -DMELTYPE_RUST_CORE -DMELTYPE_POLICY_TEST \
  "$root/mac/Sources/MeltypeIME/NativeKeyPolicy.swift" \
  "$root/mac/Sources/MeltypeIME/NativeCore.swift" \
  "$root/mac/Sources/MeltypeIME/NativeEditorPolicy.swift" \
  "$root/mac/Sources/MeltypeIME/KeyMapping.swift" \
  "$root/mac/Sources/MeltypeIME/LegacyKeyMapping.swift" \
  "$root/mac/test/key-policy/fixtures/Converter.swift" \
  "$root/mac/test/key-policy/main.swift" \
  -framework AppKit -framework Carbon -framework InputMethodKit -o "$app/Contents/MacOS/KeyPolicy"
mkdir -p "$temporary/data"
MELTYPE_RUST_DATA_DIR="$temporary/data" "$app/Contents/MacOS/KeyPolicy"
