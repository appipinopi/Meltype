// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
import Foundation

struct NativeEditorPolicy {
    private typealias ContextFunction = @convention(c) (UInt64, UInt64, Int32, UnsafeMutablePointer<UInt64>?) -> Int32
    private typealias ReplacementFunction = @convention(c) (UInt64, UInt64, UInt64, Int64, UnsafePointer<UInt16>?, UInt32, UnsafeMutablePointer<UInt64>?) -> Int32
    private typealias EqualFunction = @convention(c) (UnsafePointer<UInt16>?, UInt32, UnsafePointer<UInt16>?, UInt32) -> Int32
    private typealias ApplyFunction = @convention(c) (UnsafePointer<UInt8>?, UInt32, UnsafeMutableRawPointer?, NativeEditHost) -> Int32
    private typealias CompositionFunction = @convention(c) (UnsafePointer<UInt8>?, UInt32, UnsafeMutableRawPointer?, NativeCompositionHost) -> Int32
    private typealias CandidatesFunction = @convention(c) (UnsafePointer<UInt8>?, UInt32, UnsafeMutableRawPointer?, NativeCandidatesHost) -> Int32
    private typealias MeaningUpdate = @convention(c) (UnsafeMutableRawPointer?, Int64, UnsafePointer<UInt16>?, UInt32, Int32) -> Int32
    private typealias MeaningMatches = @convention(c) (UnsafeMutableRawPointer?, Int64, UnsafePointer<UInt16>?, UInt32) -> Int32
    private typealias MeaningDelay = @convention(c) () -> UInt32
    private typealias MeaningSchedule = @convention(c) (UnsafeMutableRawPointer?, NativeMeaningCallback) -> Int32
    private typealias WindowFunction = @convention(c) (UnsafeMutableRawPointer?, UInt32, Int64, UnsafePointer<UInt16>?, UInt32) -> Int32
    private typealias ServerCreate = @convention(c) (UnsafePointer<UInt16>?, UInt32, UnsafePointer<UInt16>?, UInt32) -> UnsafeMutableRawPointer?
    private typealias CandidatesCreate = @convention(c) (UnsafeMutableRawPointer?, UInt64) -> UnsafeMutableRawPointer?
    private let serverCreate: ServerCreate
    private let candidatesCreate: CandidatesCreate
    private let windowFunction: WindowFunction
    private let meaningSchedule: MeaningSchedule
    private let meaningDelay: MeaningDelay
    var meaningDelayMilliseconds: Int { Int(meaningDelay()) }
    private let meaningUpdate: MeaningUpdate
    private let meaningMatches: MeaningMatches
    private let candidatesFunction: CandidatesFunction
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
              let composition = lookup("meltype_macos_composition_utf8"),
              let candidates = lookup("meltype_macos_candidates_utf8"),
              let meaningUpdate = lookup("meltype_macos_meaning_update"),
              let meaningMatches = lookup("meltype_macos_meaning_matches"),
              let meaningDelay = lookup("meltype_macos_meaning_delay_ms"),
              let meaningSchedule = lookup("meltype_macos_meaning_schedule"),
              let windowFunction = lookup("meltype_macos_candidate_window"),
              let serverCreate = lookup("meltype_macos_server_create"),
              let candidatesCreate = lookup("meltype_macos_candidates_create") else { return nil }
        self.serverCreate = unsafeBitCast(serverCreate, to: ServerCreate.self)
        self.candidatesCreate = unsafeBitCast(candidatesCreate, to: CandidatesCreate.self)
        self.windowFunction = unsafeBitCast(windowFunction, to: WindowFunction.self)
        self.meaningSchedule = unsafeBitCast(meaningSchedule, to: MeaningSchedule.self)
        self.meaningDelay = unsafeBitCast(meaningDelay, to: MeaningDelay.self)
        self.meaningUpdate = unsafeBitCast(meaningUpdate, to: MeaningUpdate.self)
        self.meaningMatches = unsafeBitCast(meaningMatches, to: MeaningMatches.self)
        self.candidatesFunction = unsafeBitCast(candidates, to: CandidatesFunction.self)
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
    @discardableResult
    func candidates(_ view: CompositionView, previous: [String], visible: Bool, windowIndex: Int,
                    operation: @escaping (UInt32, Int) -> Int32) -> Bool {
        struct Update: Encodable { let candidates: [String]; let selectedIndex: Int; let previous: [String]; let visible: Bool; let windowIndex: Int }
        let update = Update(candidates: view.candidates, selectedIndex: view.selectedIndex, previous: previous, visible: visible, windowIndex: windowIndex)
        guard let data = try? JSONEncoder().encode(update), let count = UInt32(exactly: data.count) else { return false }
        let sink = NativeCandidatesSink(operation)
        return withExtendedLifetime(sink) {
            data.withUnsafeBytes { candidatesFunction($0.bindMemory(to: UInt8.self).baseAddress, count, Unmanaged.passUnretained(sink).toOpaque(), nativeCandidatesHost) != 0 }
        }
    }
    func createServer(name: String, bundleIdentifier: String) -> AnyObject? {
        let nameUnits=Array(name.utf16), bundleUnits=Array(bundleIdentifier.utf16)
        guard let nameCount=UInt32(exactly:nameUnits.count), let bundleCount=UInt32(exactly:bundleUnits.count) else { return nil }
        let pointer=nameUnits.withUnsafeBufferPointer { name in
            bundleUnits.withUnsafeBufferPointer { bundle in serverCreate(name.baseAddress,nameCount,bundle.baseAddress,bundleCount) }
        }
        guard let pointer else { return nil }
        return Unmanaged<AnyObject>.fromOpaque(pointer).takeRetainedValue()
    }
    func createCandidates(server: AnyObject, panelType: UInt64) -> AnyObject? {
        let pointer=withExtendedLifetime(server) { candidatesCreate(Unmanaged.passUnretained(server).toOpaque(),panelType) }
        guard let pointer else { return nil }
        return Unmanaged<AnyObject>.fromOpaque(pointer).takeRetainedValue()
    }
    func candidateWindow(_ window: AnyObject, operation: UInt32, argument: Int = 0, text: String? = nil) -> Int32 {
        let units = Array((text ?? "").utf16)
        guard let count = UInt32(exactly: units.count) else { return 0 }
        return withExtendedLifetime(window) {
            units.withUnsafeBufferPointer { windowFunction(Unmanaged.passUnretained(window).toOpaque(), operation, Int64(argument), $0.baseAddress, count) }
        }
    }
    @discardableResult
    func scheduleMeaning(_ action: @escaping () -> Void) -> Bool {
        let context = Unmanaged.passRetained(NativeMeaningSink(action)).toOpaque()
        if meaningSchedule(context, nativeMeaningCallback) != 0 { return true }
        Unmanaged<NativeMeaningSink>.fromOpaque(context).release()
        return false
    }
    func reserveMeaning(_ handle: UnsafeMutableRawPointer?, index: Int, text: String?) -> Bool {
        let units = Array((text ?? "").utf16)
        guard let count = UInt32(exactly: units.count) else { return false }
        return units.withUnsafeBufferPointer { meaningUpdate(handle, Int64(index), $0.baseAddress, count, text == nil ? 0 : 1) != 0 }
    }
    func matchesMeaning(_ handle: UnsafeMutableRawPointer?, index: Int, text: String) -> Bool {
        let units = Array(text.utf16)
        guard let count = UInt32(exactly: units.count) else { return false }
        return units.withUnsafeBufferPointer { meaningMatches(handle, Int64(index), $0.baseAddress, count) != 0 }
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

private typealias NativeCandidatesHost = @convention(c) (UnsafeMutableRawPointer?, UInt32, Int64) -> Int32
private final class NativeCandidatesSink {
    let operation: (UInt32, Int) -> Int32
    init(_ operation: @escaping (UInt32, Int) -> Int32) { self.operation = operation }
}
private let nativeCandidatesHost: NativeCandidatesHost = { context, operation, argument in
    guard let context, let value = Int(exactly: argument) else { return -1 }
    return Unmanaged<NativeCandidatesSink>.fromOpaque(context).takeUnretainedValue().operation(operation, value)
}

private typealias NativeMeaningCallback = @convention(c) (UnsafeMutableRawPointer?) -> Void
private final class NativeMeaningSink {
    let action: () -> Void
    init(_ action: @escaping () -> Void) { self.action = action }
}
private let nativeMeaningCallback: NativeMeaningCallback = { context in
    guard let context else { return }
    let sink = Unmanaged<NativeMeaningSink>.fromOpaque(context).takeRetainedValue()
    sink.action()
}
