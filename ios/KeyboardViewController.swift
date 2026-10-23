// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
import UIKit

struct EngineReply: Decodable {
    struct Edit: Decodable { let text: String; let deleteBefore: Int }
    struct View: Decodable { let text: String; let candidates: [String] }
    let consumed: Bool
    let commits: [Edit]
    let view: View?
}

class KeyboardViewController: UIInputViewController {
    private var session: UInt64 = 0
    private let preedit = UILabel()
    private let candidates = UIStackView()
    private var composing = false
    private var applying = false
    private var upper = false
    private var letters = [UIButton]()
    private var globe: UIButton?
    private let appearanceStore = KeyboardAppearanceStore()
    private let backdrop = KeyboardBackdrop()
    private var appearance = KeyboardAppearance()
    private var appearanceImage: UIImage?
    private var loadedImageName: String?

    override func viewDidLoad() {
        super.viewDidLoad()
        session = meltype_ios_create()
        backdrop.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(backdrop)
        NSLayoutConstraint.activate([
            backdrop.leadingAnchor.constraint(equalTo: view.leadingAnchor), backdrop.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            backdrop.topAnchor.constraint(equalTo: view.topAnchor), backdrop.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])
        let column = UIStackView()
        column.axis = .vertical
        column.spacing = 7
        column.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(column)
        NSLayoutConstraint.activate([
            column.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 5),
            column.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -5),
            column.topAnchor.constraint(equalTo: view.topAnchor, constant: 7),
            column.bottomAnchor.constraint(equalTo: view.bottomAnchor, constant: -7)
        ])
        preedit.text = "Meltype-flash"
        preedit.font = .preferredFont(forTextStyle: .body)
        preedit.accessibilityIdentifier = "preedit"
        preedit.heightAnchor.constraint(equalToConstant: 28).isActive = true
        column.addArrangedSubview(preedit)
        let scroll = UIScrollView()
        scroll.heightAnchor.constraint(equalToConstant: 36).isActive = true
        candidates.axis = .horizontal
        candidates.spacing = 6
        candidates.translatesAutoresizingMaskIntoConstraints = false
        scroll.addSubview(candidates)
        NSLayoutConstraint.activate([
            candidates.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor),
            candidates.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor),
            candidates.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor),
            candidates.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor),
            candidates.heightAnchor.constraint(equalTo: scroll.frameLayoutGuide.heightAnchor)
        ])
        column.addArrangedSubview(scroll)
        for row in ["qwertyuiop", "asdfghjkl", "zxcvbnm", "1234567890", "-@/.,?!"] {
            let line = makeRow()
            for character in row {
                let text = String(character)
                let key = button(text) { [weak self] in
                    guard let self = self else { return }
                    self.type(self.upper ? text.uppercased() : text)
                }
                if character.isLetter { letters.append(key) }
                line.addArrangedSubview(key)
            }
            column.addArrangedSubview(line)
        }
        let actions = makeRow()
        let switchKey = button("🌐") { }
        switchKey.addTarget(self, action: #selector(handleInputModeList(from:with:)), for: .allTouchEvents)
        switchKey.accessibilityLabel = "次のキーボード"
        globe = switchKey
        actions.addArrangedSubview(switchKey)
        actions.addArrangedSubview(button("⇧") { [weak self] in
            guard let self = self else { return }
            self.upper.toggle()
            for key in self.letters {
                let title = key.title(for: .normal) ?? ""
                key.setTitle(self.upper ? title.uppercased() : title.lowercased(), for: .normal)
            }
        })
        actions.addArrangedSubview(button("空白・変換") { [weak self] in self?.type(" ") })
        actions.addArrangedSubview(button("⌫") { [weak self] in
            guard let self = self else { return }
            if self.composing { self.apply(meltype_ios_key(self.session, 8, 0, 0)) }
            else { self.textDocumentProxy.deleteBackward() }
        })
        actions.addArrangedSubview(button("確定 ↵") { [weak self] in
            guard let self = self else { return }
            if self.composing { self.apply(meltype_ios_commit(self.session)) }
            else { self.textDocumentProxy.insertText("\n") }
        })
        column.addArrangedSubview(actions)
        reloadAppearance()
    }
    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        reloadAppearance()
    }
    private func reloadAppearance() {
        appearance = appearanceStore.load()
        if loadedImageName != appearance.imageName {
            appearanceImage = appearanceStore.image(for: appearance)
            loadedImageName = appearance.imageName
        }
        applyAppearance()
    }
    private func applyAppearance() {
        backdrop.apply(appearance.theme, image: appearanceImage)
        view.backgroundColor = appearance.theme.background
        preedit.textColor = appearance.theme.text
        preedit.backgroundColor = appearance.theme.key
        func style(_ parent: UIView) {
            if let key = parent as? UIButton { key.backgroundColor = appearance.theme.key; key.setTitleColor(appearance.theme.text, for: .normal) }
            parent.subviews.forEach(style)
        }
        style(view)
    }
    override func traitCollectionDidChange(_ previousTraitCollection: UITraitCollection?) {
        super.traitCollectionDidChange(previousTraitCollection)
        if isViewLoaded { applyAppearance() }
    }
    override func viewWillLayoutSubviews() {
        super.viewWillLayoutSubviews()
        globe?.isHidden = !needsInputModeSwitchKey
    }
    override func textWillChange(_ textInput: UITextInput?) {
        super.textWillChange(textInput)
        if !applying && session != 0 { apply(meltype_ios_reset(session)) }
    }
    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        if session != 0 { apply(meltype_ios_reset(session)) }
    }
    deinit { if session != 0 { meltype_ios_destroy(session) } }

    private func makeRow() -> UIStackView {
        let row = UIStackView()
        row.distribution = .fillEqually
        row.spacing = 5
        row.heightAnchor.constraint(equalToConstant: 38).isActive = true
        return row
    }
    private func button(_ title: String, action: @escaping () -> Void) -> UIButton {
        let key = UIButton(type: .system)
        key.setTitle(title, for: .normal)
        key.titleLabel?.font = .systemFont(ofSize: 16)
        key.backgroundColor = appearance.theme.key
        key.setTitleColor(appearance.theme.text, for: .normal)
        key.layer.cornerRadius = 5
        key.addAction(UIAction { _ in action() }, for: .touchUpInside)
        return key
    }
    private func type(_ text: String) {
        guard let scalar = text.unicodeScalars.first else { return }
        let vk = text == " " ? 32 : Int32(text.uppercased().unicodeScalars.first!.value)
        if !apply(meltype_ios_key(session, vk, Int32(scalar.value), upper ? 1 : 0)) {
            textDocumentProxy.insertText(text)
        }
    }
    @discardableResult
    private func apply(_ pointer: UnsafeMutablePointer<CChar>?) -> Bool {
        guard let pointer = pointer else { return false }
        defer { meltype_ios_free(pointer) }
        guard let reply = try? JSONDecoder().decode(EngineReply.self, from: Data(String(cString: pointer).utf8)) else { return false }
        applying = true
        defer { applying = false }
        // The Rust host disables backward corrections. Never delete host text here.
        guard reply.commits.allSatisfy({ $0.deleteBefore == 0 }) else {
            if let reset = meltype_ios_reset(session) { meltype_ios_free(reset) }
            composing = false
            preedit.text = ""
            return true
        }
        for edit in reply.commits { textDocumentProxy.insertText(edit.text) }
        composing = !(reply.view?.text.isEmpty ?? true)
        preedit.text = reply.view?.text ?? ""
        for child in candidates.arrangedSubviews { candidates.removeArrangedSubview(child); child.removeFromSuperview() }
        for (index, text) in (reply.view?.candidates ?? []).prefix(20).enumerated() {
            candidates.addArrangedSubview(button("  \(text)  ") { [weak self] in
                guard let self = self else { return }
                self.apply(meltype_ios_select(self.session, UInt32(index)))
                self.apply(meltype_ios_commit(self.session))
            })
        }
        return reply.consumed
    }
}
