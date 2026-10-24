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
    override var initialFlickLayout: Bool? { false }
    override var textDocumentProxy: UITextDocumentProxy { document }
}
final class KeyboardTests: XCTestCase {
    @MainActor
    private func flickKey(_ center: String, keyboard: TestKeyboard) -> FlickKey {
        func find(_ view: UIView) -> FlickKey? {
            if let key = view as? FlickKey, key.kana[0] == center { return key }
            return view.subviews.compactMap(find).first
        }
        return find(keyboard.view)!
    }
    @MainActor
    func testFlickDirectionsCancelAndReturnToCenter() {
        var output: [String] = []
        let key = FlickKey(kana: FlickKana.groups[0]) { output.append($0) }
        for offset in [CGPoint.zero, CGPoint(x: -20, y: 0), CGPoint(x: 0, y: -20), CGPoint(x: 20, y: 0), CGPoint(x: 0, y: 20)] {
            key.updateSelection(offset: offset); key.sendActions(for: .touchUpOutside)
        }
        XCTAssertEqual(output, ["あ", "い", "う", "え", "お"])
        key.updateSelection(offset: CGPoint(x: 30, y: 0)); key.cancelTracking(with: nil)
        XCTAssertEqual(output.count, 5)
        key.sendActions(for: .touchUpInside)
        XCTAssertEqual(output.last, "あ")
        XCTAssertEqual(FlickKana.direction(CGPoint(x: 11, y: -11)), 0)
        XCTAssertEqual(FlickKana.direction(CGPoint(x: 5, y: -18)), 2)
        key.updateSelection(offset: CGPoint(x: 20, y: 0)); key.updateSelection(offset: .zero)
        key.sendActions(for: .touchUpInside); XCTAssertEqual(output.last, "あ")
    }
    @MainActor
    func testFlickComposeVariantsAndLayoutSwitchThroughProxy() {
        let keyboard = TestKeyboard(); keyboard.loadViewIfNeeded()
        tap("かなフリックに切替", in: keyboard.view)
        flickKey("は", keyboard: keyboard).sendActions(for: .touchUpInside)
        tap("゛小゜", in: keyboard.view); tap("゛小゜", in: keyboard.view)
        let tsu = flickKey("た", keyboard: keyboard)
        tsu.updateSelection(offset: CGPoint(x: 0, y: -24)); tsu.sendActions(for: .touchUpInside)
        tap("゛小゜", in: keyboard.view)
        let ya = flickKey("や", keyboard: keyboard)
        ya.updateSelection(offset: CGPoint(x: 0, y: -24)); ya.sendActions(for: .touchUpInside)
        tap("゛小゜", in: keyboard.view)
        tap("ABC・QWERTYに切替", in: keyboard.view)
        XCTAssertEqual(keyboard.document.text, "ぱっゅ")
        tap("a", in: keyboard.view); tap("確定 ↵", in: keyboard.view)
        XCTAssertTrue(keyboard.document.text.hasSuffix("あ"))
        let before = keyboard.document.text
        tap("かなフリックに切替", in: keyboard.view)
        tap("゛小゜", in: keyboard.view)
        XCTAssertEqual(keyboard.document.text, before)
        tap("⌫", in: keyboard.view)
        XCTAssertEqual(keyboard.document.text, String(before.dropLast()))
    }
    @MainActor
    func testFlickCandidateSelectionCommitsToHost() {
        let keyboard = TestKeyboard(); keyboard.loadViewIfNeeded()
        tap("かなフリックに切替", in: keyboard.view)
        let ni = flickKey("な", keyboard: keyboard)
        ni.updateSelection(offset: CGPoint(x: -24, y: 0)); ni.sendActions(for: .touchUpInside)
        let ho = flickKey("は", keyboard: keyboard)
        ho.updateSelection(offset: CGPoint(x: 0, y: 24)); ho.sendActions(for: .touchUpInside)
        let n = flickKey("わ", keyboard: keyboard)
        n.updateSelection(offset: CGPoint(x: 0, y: -24)); n.sendActions(for: .touchUpInside)
        flickKey("か", keyboard: keyboard).updateSelection(offset: CGPoint(x: 0, y: 24))
        flickKey("か", keyboard: keyboard).sendActions(for: .touchUpInside)
        tap("゛小゜", in: keyboard.view)
        tap("空白・変換", in: keyboard.view)
        func buttons(_ view: UIView) -> [UIButton] {
            (view as? UIButton).map { [$0] } ?? view.subviews.flatMap(buttons)
        }
        let candidate = buttons(keyboard.view).first { $0.title(for: .normal)?.contains("ニホンゴ") == true }
        XCTAssertNotNil(candidate)
        candidate?.sendActions(for: .touchUpInside)
        XCTAssertEqual(keyboard.document.text, "ニホンゴ")
    }
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
