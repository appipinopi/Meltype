// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
import XCTest
import UIKit

final class Document: NSObject, UITextDocumentProxy {
    var text = ""
    var documentContextBeforeInput: String? { text }
    var documentContextAfterInput: String? { "" }
    var selectedText: String? { nil }
    var documentInputMode: UITextInputMode? { nil }
    let documentIdentifier = UUID()
    var hasText: Bool { !text.isEmpty }
    func insertText(_ text: String) { self.text += text }
    func deleteBackward() { if !text.isEmpty { text.removeLast() } }
    func adjustTextPosition(byCharacterOffset offset: Int) { }
    func setMarkedText(_ markedText: String, selectedRange: NSRange) { }
    func unmarkText() { }
}
final class TestKeyboard: KeyboardViewController {
    let document = Document()
    override var textDocumentProxy: UITextDocumentProxy { document }
}
final class KeyboardTests: XCTestCase {
    @MainActor
    func testAppearanceImagePreparationAndRemovalPreserveTheme() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = KeyboardAppearanceStore(directory: directory)
        try store.setTheme(.mint)
        let format = UIGraphicsImageRendererFormat(); format.scale = 1
        let source = UIGraphicsImageRenderer(size: CGSize(width: 2400, height: 1600), format: format).image { context in
            UIColor.blue.setFill(); context.fill(CGRect(x: 0, y: 0, width: 2400, height: 1600))
        }
        let file = directory.appendingPathComponent("source.png")
        try XCTUnwrap(source.pngData()).write(to: file)
        let prepared = try KeyboardAppearanceStore.prepareImage(from: file)
        let image = try XCTUnwrap(UIImage(data: prepared))
        XCTAssertLessThanOrEqual(max(image.size.width, image.size.height), 1280)
        try store.installImage(prepared)
        let settings = store.load()
        XCTAssertEqual(settings.theme, .mint)
        XCTAssertNotNil(store.image(for: settings))
        try store.setTheme(.dark)
        XCTAssertEqual(store.load().imageName, settings.imageName)
        try store.removeImage()
        XCTAssertEqual(store.load().theme, .dark)
        XCTAssertNil(store.load().imageName)
        XCTAssertFalse(FileManager.default.fileExists(atPath: directory.appendingPathComponent(try XCTUnwrap(settings.imageName)).path))
    }
    @MainActor
    func testInvalidAppearanceAndImageCannotEscapeTheSharedDirectory() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = KeyboardAppearanceStore(directory: directory)
        try store.save(KeyboardAppearance(theme: .pink, imageName: "../outside.jpg"))
        XCTAssertNil(store.load().imageName)
        XCTAssertEqual(store.load().theme, .pink)
        let file = directory.appendingPathComponent("invalid.png")
        try Data([1, 2, 3]).write(to: file)
        XCTAssertThrowsError(try KeyboardAppearanceStore.prepareImage(from: file))
        XCTAssertEqual(store.load().theme, .pink)
        try Data("{".utf8).write(to: directory.appendingPathComponent("appearance.json"))
        XCTAssertEqual(store.load(), KeyboardAppearance())
    }
    @MainActor
    private func tap(_ title: String, in view: UIView) {
        func buttons(_ parent: UIView) -> [UIButton] {
            (parent as? UIButton).map { [$0] } ?? parent.subviews.flatMap { buttons($0) }
        }
        let key = buttons(view).first { $0.title(for: .normal) == title }
        XCTAssertNotNil(key, title)
        key?.sendActions(for: .touchUpInside)
    }
    @MainActor
    func testActualButtonsComposeAndCommitThroughProxy() {
        let keyboard = TestKeyboard()
        keyboard.loadViewIfNeeded()
        for character in "nihongo" { tap(String(character), in: keyboard.view) }
        XCTAssertEqual(keyboard.document.text, "")
        let preedit = keyboard.view.subviews.flatMap { $0.subviews }.first { $0.accessibilityIdentifier == "preedit" } as? UILabel
        XCTAssertEqual(preedit?.text, "にほんご")
        tap("確定 ↵", in: keyboard.view)
        XCTAssertEqual(keyboard.document.text, "にほんご")
        tap("⌫", in: keyboard.view)
        XCTAssertEqual(keyboard.document.text, "にほん")
    }
    @MainActor
    func testFocusChangeDropsPreeditAndShiftTypesUppercase() {
        let keyboard = TestKeyboard()
        keyboard.loadViewIfNeeded()
        tap("n", in: keyboard.view)
        keyboard.textWillChange(nil)
        tap("確定 ↵", in: keyboard.view)
        XCTAssertEqual(keyboard.document.text, "\n")
        tap("⇧", in: keyboard.view)
        tap("A", in: keyboard.view)
        tap("確定 ↵", in: keyboard.view)
        XCTAssertTrue(keyboard.document.text.hasSuffix("A"))
    }
}
