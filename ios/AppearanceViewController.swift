// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
import UIKit
import PhotosUI
import UniformTypeIdentifiers

final class AppearanceViewController: UIViewController, PHPickerViewControllerDelegate {
    private let store = KeyboardAppearanceStore()
    private let status = UILabel()
    private let backdrop = KeyboardBackdrop()
    private let preview = UIStackView()
    private var themes: [KeyboardTheme: UIButton] = [:]
    private let choose = UIButton(type: .system)
    private let remove = UIButton(type: .system)
    private var importing = false

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "背景・テーマ"; view.backgroundColor = .systemBackground
        let scroll = UIScrollView(); scroll.translatesAutoresizingMaskIntoConstraints = false; view.addSubview(scroll)
        let column = UIStackView(); column.axis = .vertical; column.spacing = 12; column.translatesAutoresizingMaskIntoConstraints = false
        scroll.addSubview(column)
        NSLayoutConstraint.activate([
            scroll.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor), scroll.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor),
            scroll.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor), scroll.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            column.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 20),
            column.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -20),
            column.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 20),
            column.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -20),
            column.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -40)
        ])
        let help = UILabel(); help.numberOfLines = 0
        help.text = "背景色と好きな画像を選べます。画像は端末内で縮小保存し、中央を基準に表示します。キーボードを開き直すと反映されます。"
        column.addArrangedSubview(help)
        for theme in KeyboardTheme.allCases {
            let button = UIButton(type: .system); button.contentHorizontalAlignment = .leading
            button.heightAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true
            button.addAction(UIAction { [weak self] _ in
                guard let self = self else { return }
                self.perform { try self.store.setTheme(theme) }
            }, for: .touchUpInside)
            column.addArrangedSubview(button); themes[theme] = button
        }
        choose.setTitle("好きな画像を選ぶ", for: .normal)
        choose.addAction(UIAction { [weak self] _ in
            guard let self = self else { return }
            var configuration = PHPickerConfiguration(); configuration.filter = .images; configuration.selectionLimit = 1
            let picker = PHPickerViewController(configuration: configuration); picker.delegate = self
            self.present(picker, animated: true)
        }, for: .touchUpInside)
        column.addArrangedSubview(choose)
        remove.setTitle("背景画像を外す（テーマは保持）", for: .normal)
        remove.addAction(UIAction { [weak self] _ in
            guard let self = self else { return }
            self.perform { try self.store.removeImage() }
        }, for: .touchUpInside)
        column.addArrangedSubview(remove)
        status.numberOfLines = 0; status.accessibilityIdentifier = "appearance-status"; column.addArrangedSubview(status)
        let panel = UIView(); panel.heightAnchor.constraint(equalToConstant: 160).isActive = true
        backdrop.translatesAutoresizingMaskIntoConstraints = false; panel.addSubview(backdrop)
        preview.axis = .horizontal; preview.distribution = .fillEqually; preview.spacing = 8
        preview.translatesAutoresizingMaskIntoConstraints = false; panel.addSubview(preview)
        NSLayoutConstraint.activate([
            backdrop.leadingAnchor.constraint(equalTo: panel.leadingAnchor), backdrop.trailingAnchor.constraint(equalTo: panel.trailingAnchor),
            backdrop.topAnchor.constraint(equalTo: panel.topAnchor), backdrop.bottomAnchor.constraint(equalTo: panel.bottomAnchor),
            preview.leadingAnchor.constraint(equalTo: panel.leadingAnchor, constant: 12), preview.trailingAnchor.constraint(equalTo: panel.trailingAnchor, constant: -12),
            preview.centerYAnchor.constraint(equalTo: panel.centerYAnchor), preview.heightAnchor.constraint(equalToConstant: 48)
        ])
        for label in ["あ", "か", "さ"] { let key = UIButton(type: .system); key.setTitle(label, for: .normal); key.layer.cornerRadius = 5; preview.addArrangedSubview(key) }
        column.addArrangedSubview(panel)
        refresh()
        if store.directory == nil {
            status.text = "背景設定を共有できません。本体アプリとキーボードのApp Groupsの署名設定を確認してください。"
            choose.isEnabled = false; remove.isEnabled = false; themes.values.forEach { $0.isEnabled = false }
        }
    }
    private func refresh() {
        let settings = store.load(); backdrop.apply(settings.theme, image: store.image(for: settings))
        for (theme, button) in themes {
            button.setTitle((settings.theme == theme ? "✓ " : "　 ") + theme.label, for: .normal)
            button.accessibilityValue = settings.theme == theme ? "選択中" : ""
        }
        for case let key as UIButton in preview.arrangedSubviews { key.backgroundColor = settings.theme.key; key.setTitleColor(settings.theme.text, for: .normal) }
    }
    private func perform(_ operation: () throws -> Void) {
        do { try operation(); refresh(); status.text = "保存しました。キーボードを開き直すと反映されます。" }
        catch { status.text = "保存できませんでした。署名設定や空き容量を確認してください。" }
    }
    func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
        picker.dismiss(animated: true)
        guard !importing, let provider = results.first?.itemProvider else { return }
        importing = true; choose.isEnabled = false; remove.isEnabled = false; status.text = "画像を準備中…"
        // The provider's temporary file is valid only inside its completion handler.
        provider.loadFileRepresentation(forTypeIdentifier: UTType.image.identifier) { [weak self] file, _ in
            let result = Result<Data, Error> {
                guard let file = file else { throw CocoaError(.fileReadCorruptFile) }
                return try KeyboardAppearanceStore.prepareImage(from: file)
            }
            DispatchQueue.main.async {
                guard let self = self else { return }
                self.importing = false; self.choose.isEnabled = true; self.remove.isEnabled = true
                switch result {
                case .success(let data): self.perform { try self.store.installImage(data) }
                case .failure: self.status.text = "画像を読み込めませんでした。別の画像を選んでください。"
                }
            }
        }
    }
    override func traitCollectionDidChange(_ previousTraitCollection: UITraitCollection?) {
        super.traitCollectionDidChange(previousTraitCollection)
        if isViewLoaded { refresh() }
    }
}
