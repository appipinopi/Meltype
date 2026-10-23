// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
import UIKit
@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?
    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?) -> Bool {
        let window = UIWindow(frame: UIScreen.main.bounds)
        let controller = SetupViewController()
        window.rootViewController = UINavigationController(rootViewController: controller)
        window.makeKeyAndVisible()
        self.window = window
        return true
    }
}

final class SetupViewController: UIViewController {
    private let field = UITextView()
    private let extensionStatus = UILabel()
    private let settings = UIButton(type: .system)

    // Check the installed bundle: signing tools can remove PlugIns from a valid IPA.
    static func hasKeyboardExtension(in app: Bundle = .main) -> Bool {
        guard let plugins = app.builtInPlugInsURL,
              let urls = try? FileManager.default.contentsOfDirectory(at: plugins, includingPropertiesForKeys: nil) else { return false }
        return urls.filter { $0.pathExtension == "appex" }.contains { url in
            guard let bundle = Bundle(url: url),
                  let info = bundle.object(forInfoDictionaryKey: "NSExtension") as? [String: Any] else { return false }
            return info["NSExtensionPointIdentifier"] as? String == "com.apple.keyboard-service"
                && FileManager.default.isExecutableFile(atPath: bundle.executableURL?.path ?? "")
        }
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Meltype-flash"
        view.backgroundColor = .systemGroupedBackground
        let scroll = UIScrollView()
        scroll.keyboardDismissMode = .interactive
        scroll.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(scroll)
        let stack = UIStackView()
        stack.axis = .vertical; stack.spacing = 16
        stack.translatesAutoresizingMaskIntoConstraints = false
        scroll.addSubview(stack)
        NSLayoutConstraint.activate([
            scroll.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor),
            scroll.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            scroll.bottomAnchor.constraint(equalTo: view.keyboardLayoutGuide.topAnchor),
            stack.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 24),
            stack.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -24),
            stack.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 24),
            stack.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -24),
            stack.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -48)
        ])
        let logo = UIImageView(image: UIImage(named: "BrandMark"))
        logo.contentMode = .scaleAspectFit
        logo.heightAnchor.constraint(equalToConstant: 88).isActive = true
        logo.isAccessibilityElement = true; logo.accessibilityLabel = "Meltype-flash ロゴ"
        stack.addArrangedSubview(logo)
        stack.addArrangedSubview(label("日本語も英語も、そのまま入力。", style: .title2))
        extensionStatus.numberOfLines = 0
        extensionStatus.font = .preferredFont(forTextStyle: .body)
        extensionStatus.adjustsFontForContentSizeCategory = true
        extensionStatus.accessibilityIdentifier = "keyboard-extension-status"
        stack.addArrangedSubview(extensionStatus)
        stack.addArrangedSubview(label("1　キーボードを追加", style: .headline))
        stack.addArrangedSubview(label("設定 → 一般 → キーボード → キーボード → 新しいキーボードを追加 → Meltype-flash\n\n下のボタンはアプリの設定を開きます。「キーボード」があれば、そこからも追加できます。", style: .body))
        configure(settings, title: "設定を開く", symbol: "gearshape.fill") { [weak self] in
            guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
            UIApplication.shared.open(url, options: [:]) { opened in
                guard !opened else { return }
                DispatchQueue.main.async {
                    let alert = UIAlertController(title: "設定を開けませんでした", message: "ホーム画面の「設定」から、一般 → キーボードへ進んでください。", preferredStyle: .alert)
                    alert.addAction(UIAlertAction(title: "閉じる", style: .default))
                    self?.present(alert, animated: true)
                }
            }
        }
        settings.accessibilityIdentifier = "open-settings"
        stack.addArrangedSubview(settings)
        stack.addArrangedSubview(label("2　入力を試す", style: .headline))
        stack.addArrangedSubview(label("入力欄をタップして、🌐を長押しし、Meltype-flashを選びます。未確定文字はキーボード内に表示され、確定すると入力欄に入ります。", style: .body))
        field.font = .preferredFont(forTextStyle: .body)
        field.adjustsFontForContentSizeCategory = true
        field.backgroundColor = .secondarySystemGroupedBackground
        field.layer.cornerRadius = 12
        field.textContainerInset = UIEdgeInsets(top: 12, left: 12, bottom: 12, right: 12)
        field.heightAnchor.constraint(equalToConstant: 140).isActive = true
        field.accessibilityIdentifier = "input-test"; field.accessibilityLabel = "キーボードの入力テスト"
        stack.addArrangedSubview(field)
        let appearance = UIButton(type: .system)
        configure(appearance, title: "背景・テーマ", symbol: "paintpalette") { [weak self] in
            self?.navigationController?.pushViewController(AppearanceViewController(), animated: true)
        }
        stack.addArrangedSubview(appearance)
        stack.addArrangedSubview(label("フルアクセスは不要です。入力内容の通信・履歴保存は行いません。", style: .footnote))
        navigationItem.rightBarButtonItem = UIBarButtonItem(title: "完了", style: .done, target: self, action: #selector(endEditing))
        refreshExtensionStatus()
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        refreshExtensionStatus()
    }

    private func refreshExtensionStatus() {
        let available = Self.hasKeyboardExtension()
        extensionStatus.text = available
            ? "✓ キーボード拡張を同梱しています。設定から追加して使えます。"
            : "キーボード拡張が見つかりません。この状態では設定から追加できません。\n\niPASideなどの導入ツールが拡張を削除する場合があります。キーボード拡張（.appex）を残し、本体と拡張の両方を署名できる方法で再導入してください。"
        extensionStatus.textColor = available ? .label : .systemRed
        settings.isEnabled = available
    }

    private func label(_ text: String, style: UIFont.TextStyle) -> UILabel {
        let label = UILabel()
        label.text = text; label.numberOfLines = 0
        label.font = .preferredFont(forTextStyle: style)
        label.adjustsFontForContentSizeCategory = true
        return label
    }

    private func configure(_ button: UIButton, title: String, symbol: String, action: @escaping () -> Void) {
        var config = UIButton.Configuration.filled()
        config.title = title; config.image = UIImage(systemName: symbol); config.imagePadding = 10
        config.baseBackgroundColor = UIColor(red: 1, green: 0.86, blue: 0.15, alpha: 1)
        config.baseForegroundColor = .black; config.cornerStyle = .medium
        config.contentInsets = NSDirectionalEdgeInsets(top: 14, leading: 16, bottom: 14, trailing: 16)
        button.configuration = config
        button.heightAnchor.constraint(greaterThanOrEqualToConstant: 50).isActive = true
        button.addAction(UIAction { _ in action() }, for: .touchUpInside)
    }

    @objc private func endEditing() { view.endEditing(true) }
}
