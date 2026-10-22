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
