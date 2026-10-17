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

// Reference the previous InputController arithmetic and Swift String equality.
guard let editor = NativeCore.shared.editorPolicy else { fatalError("missing Rust editor policy") }
precondition(NativeEditorPolicy(lookup: { _ in nil }) == nil)
func legacyReplacement(_ marked: NSRange, _ selected: NSRange, _ remove: Int, _ expected: String?) -> NSRange? {
    if remove <= 0 { return NSRange(location: NSNotFound, length: 0) }
    let caret = marked.location != NSNotFound && marked.length > 0 ? marked.location : selected.location
    guard caret != NSNotFound, caret >= remove, let expected, (expected as NSString).length == remove else { return nil }
    return NSRange(location: caret - remove, length: remove)
}
var rangeCount = 0
for location in [0, 1, 19, 20, 21, 3999, 4000, 4001, Int.max - 40, NSNotFound] {
    for length in [0, 1, 20] {
        for code in [false, true] {
            let selection = NSRange(location: location, length: length)
            let result = editor.contextRanges(selection: selection, code: code)
            if location == NSNotFound { precondition(result == nil) }
            else {
                let start = max(0, location - (code ? 4000 : 20))
                precondition(result?.0 == NSRange(location: start, length: location - start))
                precondition(result?.1 == NSRange(location: location + length, length: 20))
            }
            rangeCount += 1
        }
    }
}
let expectedTexts: [String?] = [nil, "", "a", "😀", "e\u{301}", "é", "かな"]
for location in [0, 5, 99, NSNotFound] {
    for length in [0, 1, 3] {
        for selected in Array(0...11) + [NSNotFound] {
            for remove in -1...6 {
                for expected in expectedTexts {
                    let marked = NSRange(location: location, length: length)
                    let selection = NSRange(location: selected, length: 0)
                    precondition(editor.replacementRange(marked: marked, selected: selection, remove: remove, expected: expected) == legacyReplacement(marked, selection, remove, expected))
                    rangeCount += 1
                }
            }
        }
    }
}
precondition(editor.contextRanges(selection: NSRange(location: Int.max - 1, length: 2), code: false) == nil)
let comparisons = ["", "a", "a\u{0}b", "e\u{301}", "é", "Å", "A\u{30a}", "가", "\u{1100}\u{1161}", "ﬁ", "fi", "😀"]
var equalityCount = 0
for actual in comparisons {
    for expected in comparisons {
        precondition(editor.expectedEqual(actual, expected) == (actual == expected), "Swift canonical equality mismatch")
        equalityCount += 1
    }
}
print("macOS Rust editor policy: \(rangeCount) legacy NSRange cases, \(equalityCount) Swift canonical comparisons, UTF-16 lengths, marked/selected caret and overflow rejection passed")
