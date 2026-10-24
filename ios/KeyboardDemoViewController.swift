// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
import UIKit

// This demo uses the same controller and engine as the system keyboard.
private final class DemoDocument: NSObject, UITextDocumentProxy {
    weak var field: UITextView?
    var text = "" { didSet { field?.text = text } }
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
private final class DemoKeyboard: KeyboardViewController {
    let document = DemoDocument()
    override var textDocumentProxy: UITextDocumentProxy { document }
    override var initialFlickLayout: Bool? { true }
    override var allowsSystemKeyboardSwitch: Bool { false }
}
final class KeyboardDemoViewController: UIViewController {
    private let keyboard = DemoKeyboard()
    override func viewDidLoad() {
        super.viewDidLoad(); title = "フリックを試す"
        let column = AppStyle.scrollColumn(in: self)
        if let scroll = column.superview as? UIScrollView {
            scroll.canCancelContentTouches = false; scroll.delaysContentTouches = false
        }
        column.addArrangedSubview(AppStyle.label("タップで「あ」、左へ「い」。", .title2))
        column.addArrangedSubview(AppStyle.label("かなキーを上下左右へフリック。゛小゜で濁点・半濁点・小文字を切り替えます。", .subheadline, color: .secondaryLabel))
        let field = UITextView(); field.font = .preferredFont(forTextStyle: .body)
        field.adjustsFontForContentSizeCategory = true; field.isEditable = false
        field.backgroundColor = .secondarySystemGroupedBackground; field.layer.cornerRadius = 14
        field.textContainerInset = UIEdgeInsets(top: 12, left: 12, bottom: 12, right: 12)
        field.heightAnchor.constraint(equalToConstant: 86).isActive = true
        field.accessibilityIdentifier = "flick-demo-output"; field.accessibilityLabel = "確定した文字"
        keyboard.document.field = field
        column.addArrangedSubview(field)
        addChild(keyboard)
        column.addArrangedSubview(keyboard.view); keyboard.didMove(toParent: self)
        column.addArrangedSubview(AppStyle.label("アプリ内のお試し入力です。ほかのアプリで使うには、設定からキーボードを追加してください。", .footnote, color: .secondaryLabel))
    }
}
