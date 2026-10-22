// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
import UIKit
@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?
    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?) -> Bool {
        let window = UIWindow(frame: UIScreen.main.bounds)
        let controller = UIViewController()
        controller.view.backgroundColor = .systemBackground
        let stack = UIStackView()
        stack.axis = .vertical
        stack.spacing = 24
        stack.translatesAutoresizingMaskIntoConstraints = false
        controller.view.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: controller.view.safeAreaLayoutGuide.leadingAnchor, constant: 24),
            stack.trailingAnchor.constraint(equalTo: controller.view.safeAreaLayoutGuide.trailingAnchor, constant: -24),
            stack.centerYAnchor.constraint(equalTo: controller.view.centerYAnchor)
        ])
        let label = UILabel()
        label.numberOfLines = 0
        label.text = "Meltype-flash iOS プレビュー\n\n設定 → 一般 → キーボード → キーボード → 新しいキーボードを追加 → Meltype-flash\n\n入力欄の🌐で切り替えます。未確定文字はキーボード上部に表示され、確定時に入力欄へ送ります。\n\nフルアクセスは不要です。通信・入力履歴の保存は行いません。"
        stack.addArrangedSubview(label)
        let field = UITextView()
        field.font = .systemFont(ofSize: 20)
        field.layer.borderWidth = 1
        field.layer.borderColor = UIColor.separator.cgColor
        field.heightAnchor.constraint(equalToConstant: 140).isActive = true
        field.accessibilityIdentifier = "input-test"
        stack.addArrangedSubview(field)
        window.rootViewController = controller
        window.makeKeyAndVisible()
        self.window = window
        return true
    }
}
