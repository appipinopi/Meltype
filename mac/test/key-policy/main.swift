// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
import AppKit
import Carbon.HIToolbox

precondition(NativeCore.shared.keyPolicy != nil, "real NativeCore did not resolve Rust key ABI")
precondition(NativeKeyPolicy(lookup: { _ in nil }) == nil, "missing ABI accepted")
func event(_ code: UInt16, _ text: String) -> NSEvent {
    guard let value = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0,
        windowNumber: 0, context: nil, characters: text, charactersIgnoringModifiers: text, isARepeat: false, keyCode: code) else {
        fatalError("could not create controlled key event")
    }
    return value
}
var count = 0
func compare(_ code: UInt16, _ text: String) {
    let value = event(code, text)
    let expected = LegacyKeyMapping.virtualKey(for: value)
    let actual = KeyMapping.virtualKey(for: value)
    precondition(actual == expected, "key mapping mismatch: code=\(code), text=\(text.debugDescription), actual=\(String(describing: actual)), expected=\(String(describing: expected))")
    count += 1
}
let texts = ["", "a", "Z", "0", "9", ",", ".", "-", "/", "[", "]", ";", " ", "\n", "\t", "\u{1f}", "かな", "😀", "İ", "Σ", "𐐀", "e\u{301}", "👩‍💻", "Ａ", "\u{200d}", "\u{0}"]
for code in UInt16.min...UInt16.max {
    autoreleasepool { compare(code, "q") }
}
for code in UInt16(0)...UInt16(255) {
    for text in texts { autoreleasepool { compare(code, text) } }
}
var modifiersCount = 0
for bits in UInt(0)..<512 {
    for deviceFlags in [UInt(0), UInt(0xffff)] {
        let flags = NSEvent.ModifierFlags(rawValue: (bits << 16) | deviceFlags)
        var expected: Int32 = 0
        if flags.contains(.shift) { expected |= 1 }
        if flags.contains(.control) { expected |= 2 }
        if flags.contains(.option) { expected |= 4 }
        if flags.contains(.command) { expected |= 8 }
        precondition(KeyMapping.modifiers(for: flags) == expected, "SDK modifier mismatch")
        modifiersCount += 1
    }
}
print("macOS Rust key policy: \(count) SDK/legacy events, \(modifiersCount) modifier states, Unicode normalization, special-key priority and missing-ABI rejection passed")
