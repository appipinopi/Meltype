#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
# Linux CI only: private AVD, fixed explicit serial, no IME registration.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
test -n "$sdk" || { echo 'Android SDK path is required' >&2; exit 1; }
adb="$sdk/platform-tools/adb"
emulator="$sdk/emulator/emulator"
avdmanager="$sdk/cmdline-tools/latest/bin/avdmanager"
serial=emulator-5580
artifacts="$root/dist/android-emulator"
mkdir -p "$artifacts"
"$adb" start-server
if "$adb" devices | awk '{print $1}' | grep -Fxq "$serial"; then
    echo "$serial is already in use; refusing to touch it" >&2; exit 1
fi
temp_parent="$(realpath "${TMPDIR:-/tmp}")"
private="$(mktemp -d "$temp_parent/meltype-avd.XXXXXXXX")"
emulator_pid=''
cleanup() {
    if [[ -n "$emulator_pid" ]]; then
        if kill -0 "$emulator_pid" 2>/dev/null; then
            timeout 20 "$adb" -s "$serial" logcat -d > "$artifacts/logcat.txt" 2>&1 || true
            kill "$emulator_pid" 2>/dev/null || true
        fi
        wait "$emulator_pid" 2>/dev/null || true
    fi
    # Verify the resolved target before recursive cleanup, including a replaced link.
    if [[ "$(realpath "$private")" == "$private" && "$private" == "$temp_parent"/meltype-avd.* ]]; then
        rm -rf -- "$private"
    fi
}
trap cleanup EXIT
export ANDROID_AVD_HOME="$private/avd"
export ANDROID_USER_HOME="$private/user"
export ANDROID_EMULATOR_HOME="$private/emulator"
mkdir -p "$ANDROID_AVD_HOME" "$ANDROID_USER_HOME" "$ANDROID_EMULATOR_HOME"
printf 'no\n' | "$avdmanager" create avd --name MeltypeRustTest --package 'system-images;android-35;google_apis;x86_64' --device pixel_2
"$emulator" -avd MeltypeRustTest -port 5580 -no-window -no-audio -no-boot-anim -no-snapshot -gpu swiftshader_indirect -camera-back none -camera-front none > "$artifacts/emulator.log" 2>&1 &
emulator_pid=$!
timeout 180 "$adb" -s "$serial" wait-for-device
ready=false
for _ in $(seq 1 120); do
    kill -0 "$emulator_pid" 2>/dev/null || { echo 'Test emulator exited' >&2; exit 1; }
    if [[ "$("$adb" -s "$serial" shell getprop sys.boot_completed | tr -d '\r')" == 1 ]]; then ready=true; break; fi
    sleep 2
done
"$ready" || { echo 'Test emulator did not finish booting' >&2; exit 1; }
"$adb" -s "$serial" shell input keyevent 82
"$adb" -s "$serial" install -r "$root/android/app/build/outputs/apk/debug/app-debug.apk"
"$adb" -s "$serial" install -r -t "$root/android/app/build/outputs/apk/androidTest/debug/app-debug-androidTest.apk"
python3 "$root/android/tests/instrument.py" --adb "$adb" --serial "$serial" --output "$artifacts"
