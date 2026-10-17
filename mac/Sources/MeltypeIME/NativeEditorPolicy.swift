// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
import Foundation

struct NativeEditorPolicy {
    private typealias ContextFunction = @convention(c) (UInt64, UInt64, Int32, UnsafeMutablePointer<UInt64>?) -> Int32
    private typealias ReplacementFunction = @convention(c) (UInt64, UInt64, UInt64, Int64, UnsafePointer<UInt16>?, UInt32, UnsafeMutablePointer<UInt64>?) -> Int32
    private typealias EqualFunction = @convention(c) (UnsafePointer<UInt16>?, UInt32, UnsafePointer<UInt16>?, UInt32) -> Int32
    private let context: ContextFunction
    private let replacement: ReplacementFunction
    private let equal: EqualFunction
    init?(lookup: (String) -> UnsafeMutableRawPointer?) {
        guard let context = lookup("meltype_macos_context_range"),
              let replacement = lookup("meltype_macos_replacement_range"),
              let equal = lookup("meltype_macos_expected_equal") else { return nil }
        self.context = unsafeBitCast(context, to: ContextFunction.self)
        self.replacement = unsafeBitCast(replacement, to: ReplacementFunction.self)
        self.equal = unsafeBitCast(equal, to: EqualFunction.self)
    }
    func contextRanges(selection: NSRange, code: Bool) -> (NSRange, NSRange)? {
        guard let location = UInt64(exactly: selection.location), let length = UInt64(exactly: selection.length) else { return nil }
        var output = [UInt64](repeating: 0, count: 4)
        guard context(location, length, code ? 1 : 0, &output) != 0 else { return nil }
        return (NSRange(location: Int(output[0]), length: Int(output[1])), NSRange(location: Int(output[2]), length: Int(output[3])))
    }
    func replacementRange(marked: NSRange, selected: NSRange, remove: Int, expected: String?) -> NSRange? {
        guard let markedLocation = UInt64(exactly: marked.location), let markedLength = UInt64(exactly: marked.length),
              let selectedLocation = UInt64(exactly: selected.location), remove <= 0 || expected != nil else { return nil }
        let units = Array((expected ?? "").utf16)
        guard let length = UInt32(exactly: units.count) else { return nil }
        var output = [UInt64](repeating: 0, count: 2)
        let success = units.withUnsafeBufferPointer { replacement(markedLocation, markedLength, selectedLocation, Int64(remove), $0.baseAddress, length, &output) }
        guard success != 0 else { return nil }
        return NSRange(location: Int(output[0]), length: Int(output[1]))
    }
    func expectedEqual(_ actual: String, _ expected: String) -> Bool {
        let left = Array(actual.utf16), right = Array(expected.utf16)
        guard let leftCount = UInt32(exactly: left.count), let rightCount = UInt32(exactly: right.count) else { return false }
        return left.withUnsafeBufferPointer { a in right.withUnsafeBufferPointer { b in
            equal(a.baseAddress, leftCount, b.baseAddress, rightCount) != 0
        } }
    }
}
