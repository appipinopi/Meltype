# Meltype-flash 0.1.0 Preview 1

共通Rustエンジンを使うWindows・Linux・macOS・Android・ChromeOS向けプレビューです。元C#版と全機能・変換精度が同等の完成版ではありません。実機・実アプリでの受入確認が残っています。

| 環境 | 配布ファイル | 導入 |
|---|---|---|
| Windows 10/11 x64 | Meltype-flash-Windows-x64.zip | 全体を展開し、README-Windows.mdを参照。管理者PowerShellでManage-Windows.ps1を実行。32bitアプリ用TIP/workerも同梱 |
| Linux x64（Ubuntu 24.04でビルド） | Meltype-flash-Linux-x64.tar.gz | 展開後、sudo bash install.sh --system。システムのIBus/GLibと設定画面のGUI実行環境が必要 |
| macOS 13以降・Apple Silicon | Meltype-flash-macOS-arm64.zip | ZIP全体を展開し、Install Meltype.commandを実行 |
| macOS 13以降・Intel | Meltype-flash-macOS-x64.zip | ZIP全体を展開し、Install Meltype.commandを実行 |
| Android 8.0以降・arm64/x86_64 | Meltype-flash-Android-debug.apk | 開発検証用APKをインストール後、システムのキーボード設定でMeltype-flash Previewを有効化 |
| ChromeOS | Meltype-flash-ChromeOS.zip | 展開後、chrome://extensionsで「パッケージ化されていない拡張機能を読み込む」。入力方法にMeltype-flashを追加 |

Windowsは元版とCOM登録IDが共通です。元版を置き換える際は同梱説明のReplaceExisting手順を確認してください。配布ZIPを展開するだけではIME登録は行いません。OpenGL 2.0以上が必要です。GitHubのWindows検証環境ではOpenGL不足によるトレイ起動試験の失敗が残っており、この公開は実トレイの受入完了を意味しません。

LinuxはRust製IBus版です。Mozcヘルパーを同梱せず、変換は共通Coreの補助辞書に限られます。fcitx5用パッケージは含みません。

macOSはSwift InputMethodKitとazooKeyを利用します。署名はアドホック署名で、Developer ID署名・Apple公証は行っていません。初回起動はmacOSの許可操作が必要になる場合があります。

Androidはエミュレーターで検証するdebug APKです。Play Store配布・正式な配布署名・物理端末での確認は未対応です。このAPKの署名鍵はCI実行ごとに変わるため、別のCI生成版からの上書き更新は保証できません。アンインストールするとアプリ内データを失うので、必要なデータは先にバックアップしてください。Mozcは同梱しません。

ChromeOSはManifest V3とchrome.input.imeを使用する試験版です。ChromeOS製品版・管理ポリシーによって導入できない場合があります。Windows/macOS/Linux上のChrome用IMEではありません。Chrome Web Store公開は行っていません。

各ファイルのSHA-256はSHA256SUMS.txtを参照してください。WindowsとChromeOSのZIPには内部ファイルのハッシュも含みます。ソースと各環境の詳細は同じリリースタグ内のrust/README.md、android/README.md、chromeos/README.mdを参照してください。
