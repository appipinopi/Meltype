// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
import Foundation

struct NativeEditorPolicy {
    private typealias ContextFunction = @convention(c) (UInt64, UInt64, Int32, UnsafeMutablePointer<UInt64>?) -> Int32
    private typealias ReplacementFunction = @convention(c) (UInt64, UInt64, UInt64, Int64, UnsafePointer<UInt16>?, UInt32, UnsafeMutablePointer<UInt64>?) -> Int32
    private typealias EqualFunction = @convention(c) (UnsafePointer<UInt16>?, UInt32, UnsafePointer<UInt16>?, UInt32) -> Int32
    private typealias ApplyFunction = @convention(c) (UnsafePointer<UInt8>?, UInt32, UnsafeMutableRawPointer?, NativeEditHost) -> Int32
    private typealias CompositionFunction = @convention(c) (UnsafePointer<UInt8>?, UInt32, UnsafeMutableRawPointer?, NativeCompositionHost) -> Int32
    private let compositionFunction: CompositionFunction
    private let applyFunction: ApplyFunction
    private let context: ContextFunction
    private let replacement: ReplacementFunction
    private let equal: EqualFunction
    init?(lookup: (String) -> UnsafeMutableRawPointer?) {
        guard let context = lookup("meltype_macos_context_range"),
              let replacement = lookup("meltype_macos_replacement_range"),
              let equal = lookup("meltype_macos_expected_equal"),
              let apply = lookup("meltype_macos_apply_utf8"),
              let composition = lookup("meltype_macos_composition_utf8") else { return nil }
        self.compositionFunction = unsafeBitCast(composition, to: CompositionFunction.self)
        self.applyFunction = unsafeBitCast(apply, to: ApplyFunction.self)
        self.context = unsafeBitCast(context, to: ContextFunction.self)
        self.replacement = unsafeBitCast(replacement, to: ReplacementFunction.self)
        self.equal = unsafeBitCast(equal, to: EqualFunction.self)
    }
    @discardableResult
    func apply(_ result: SessionResult, marked: @escaping () -> NSRange, selected: @escaping () -> NSRange,
               substring: @escaping (NSRange) -> String?, insert: @escaping (String, NSRange) -> Void,
               render: @escaping () -> Void) -> Bool {
        guard let data = try? JSONEncoder().encode(result), let count = UInt32(exactly: data.count) else { return false }
        let sink = NativeEditSink(marked: marked, selected: selected, substring: substring, insert: insert, render: render)
        return withExtendedLifetime(sink) {
            data.withUnsafeBytes { bytes in
                applyFunction(bytes.bindMemory(to: UInt8.self).baseAddress, count, Unmanaged.passUnretained(sink).toOpaque(), nativeEditHost) != 0
            }
        }
    }
    @discardableResult
    func composition(_ view: CompositionView, mark: @escaping (Int, NSRange) -> Void, publish: @escaping (Int) -> Void) -> Bool {
        guard let data = try? JSONEncoder().encode(view), let count = UInt32(exactly: data.count) else { return false }
        let sink = NativeCompositionSink(mark: mark, publish: publish)
        return withExtendedLifetime(sink) {
            data.withUnsafeBytes { bytes in compositionFunction(bytes.bindMemory(to: UInt8.self).baseAddress, count, Unmanaged.passUnretained(sink).toOpaque(), nativeCompositionHost) != 0 }
        }
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

private typealias NativeEditHost = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt64, UInt64, UnsafePointer<UInt16>?, UInt32, UnsafeMutablePointer<UInt64>?) -> Int32
private final class NativeEditSink {
    let marked: () -> NSRange
    let selected: () -> NSRange
    let substring: (NSRange) -> String?
    let insert: (String, NSRange) -> Void
    let render: () -> Void
    var buffer: UnsafeMutablePointer<UInt16>?
    init(marked: @escaping () -> NSRange, selected: @escaping () -> NSRange, substring: @escaping (NSRange) -> String?, insert: @escaping (String, NSRange) -> Void, render: @escaping () -> Void) {
        self.marked = marked; self.selected = selected; self.substring = substring; self.insert = insert; self.render = render
    }
    deinit { buffer?.deallocate() }
}
private let nativeEditHost: NativeEditHost = { context, operation, location, length, text, count, output in
    guard let context else { return 0 }
    let sink = Unmanaged<NativeEditSink>.fromOpaque(context).takeUnretainedValue()
    switch operation {
    case 0, 1:
        guard let output else { return 0 }
        let range = operation == 0 ? sink.marked() : sink.selected()
        guard let at = UInt64(exactly: range.location), let size = UInt64(exactly: range.length) else { return 0 }
        output[0] = at; output[1] = size
    case 2:
        guard let output, let at = Int(exactly: location), let size = Int(exactly: length),
              let value = sink.substring(NSRange(location: at, length: size)) else { return 0 }
        let units = Array(value.utf16)
        guard UInt32(exactly: units.count) != nil else { return 0 }
        sink.buffer?.deallocate()
        let buffer = UnsafeMutablePointer<UInt16>.allocate(capacity: max(1, units.count))
        for (index, unit) in units.enumerated() { buffer[index] = unit }
        sink.buffer = buffer
        output[0] = UInt64(UInt(bitPattern: buffer)); output[1] = UInt64(units.count)
    case 3:
        guard let at = Int(exactly: location), let size = Int(exactly: length), let text else { return 0 }
        sink.insert(String(decoding: UnsafeBufferPointer(start: text, count: Int(count)), as: UTF16.self), NSRange(location: at, length: size))
    case 4: sink.render()
    default: return 0
    }
    return 1
}

private typealias NativeCompositionHost = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt64, UInt64, UInt32) -> Int32
private final class NativeCompositionSink {
    let mark: (Int, NSRange) -> Void
    let publish: (Int) -> Void
    init(mark: @escaping (Int, NSRange) -> Void, publish: @escaping (Int) -> Void) { self.mark = mark; self.publish = publish }
}
private let nativeCompositionHost: NativeCompositionHost = { context, operation, location, length, style in
    guard let context, let at = Int(exactly: location), let size = Int(exactly: length) else { return 0 }
    let sink = Unmanaged<NativeCompositionSink>.fromOpaque(context).takeUnretainedValue()
    switch operation {
    case 0:
        guard style <= 2 else { return 0 }
        sink.mark(Int(style), NSRange(location: at, length: size))
    case 1: sink.publish(at)
    default: return 0
    }
    return 1
}
