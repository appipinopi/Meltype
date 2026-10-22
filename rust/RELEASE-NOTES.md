# Meltype-flash v0.1.0 Pre2

2026年10月22日のプレリリースです。改名前のPreview 1から、製品名・ロゴと専用IME登録を整え、入力と設定画面の修正をまとめました。

## Pre2の変更

- Windowsの設定画面で、透明な右クリック領域がスイッチ・ボタン・入力欄のクリックを遮る不具合を修正。
- 長文・高速入力中に変換ワーカーが時間切れになる問題を修正。入力中の候補取得と学習処理がキー入力を長時間止めないように改善。
- 登録なしのポータブル入力を終了するとき、UI Automationの応答待ちで終了が止まる可能性がある処理を修正。
- Meltype-flash専用のロゴ・アイコン、COM登録ID、データ保存先、起動・配布経路を採用。参照元のC#版と区別。
- Linuxの`code`、Macの`com.microsoft.VSCode`など、OSごとのアプリ名・アプリIDで入力モードを設定。ネイティブ名の個別設定を優先。
- ChromeOSの設定にサイト別の通常入力・コード入力を追加。`vscode.com`・`vscode.dev`・`github.dev`を初期候補にし、タブ参照を許可した場合に自動切替。切替で未確定文字を消さないように対応。
- 上流の変更履歴を取り込み、Rust版の製品名・登録・配布を維持する検査を追加。

## 配布と導入

共通Rustエンジンを使うWindows・Linux・macOS・Android・ChromeOS向けプレビューです。元C#版と全機能・変換精度が同等の完成版ではありません。実機・実アプリでの受入確認が残っています。

| 環境 | 配布ファイル | 導入 |
|---|---|---|
| Windows 10/11 x64 | Meltype-flash-Windows-x64.zip | 全体を展開し、README-Windows.mdを参照。管理者PowerShellでManage-Windows.ps1を実行。32bitアプリ用TIP/workerも同梱 |
| Linux x64（Ubuntu 24.04でビルド） | Meltype-flash-Linux-x64.tar.gz | 展開後、sudo bash install.sh --system。システムのIBus/GLibと設定画面のGUI実行環境が必要 |
| macOS 13以降・Apple Silicon | Meltype-flash-macOS-arm64.zip | ZIP全体を展開し、Install Meltype.commandを実行 |
| macOS 13以降・Intel | Meltype-flash-macOS-x64.zip | ZIP全体を展開し、Install Meltype.commandを実行 |
| Android 8.0以降・arm64/x86_64 | Meltype-flash-Android-debug.apk | 開発検証用APKをインストール後、システムのキーボード設定でMeltype-flash Previewを有効化 |
| ChromeOS | Meltype-flash-ChromeOS.zip | 展開後、chrome://extensionsで「パッケージ化されていない拡張機能を読み込む」。入力方法にMeltype-flashを追加 |

Windowsは元C#版と異なる専用COM登録IDを使います。配布ZIPを展開するだけではIME登録は行いません。導入済みのFlash版を更新する場合は、同梱の`Manage-Windows.ps1 -Action Update`を管理者PowerShellで実行してください。旧バージョンのファイルと個人データは保持します。設定画面にはOpenGL 2.0以上が必要です。CIではソフトウェアOpenGLで実トレイの終了試験を行い、実Windowsでも設定クリックと更新を確認しています。

LinuxはRust製IBus版です。Mozcヘルパーを同梱せず、変換は共通Coreの補助辞書に限られます。fcitx5用パッケージは含みません。IBusがアプリ名を渡さない入力経路（gnome-shellや一部のQt・XIMなど）ではアプリ別の自動切替を行いません。

macOSはSwift InputMethodKitとazooKeyを利用します。署名はアドホック署名で、Developer ID署名・Apple公証は行っていません。初回起動はmacOSの許可操作が必要になる場合があります。

Androidはエミュレーターで検証するdebug APKです。Play Store配布・正式な配布署名・物理端末での確認は未対応です。このAPKの署名鍵はCI実行ごとに変わるため、別のCI生成版からの上書き更新は保証できません。アンインストールするとアプリ内データを失うので、必要なデータは先にバックアップしてください。Mozcは同梱しません。

ChromeOSはManifest V3とchrome.input.imeを使用する試験版です。ChromeOS製品版・管理ポリシーによって導入できない場合があります。Windows/macOS/Linux上のChrome用IMEではありません。Chrome Web Store公開は行っていません。サイト別切替は初期状態では無効です。設定の「サイトごとの自動切替」を有効にし、タブ参照を許可して保存してください。周辺テキストを渡さないエディターではコメント・文字列内の日本語入力を自動判別できないため、そのサイトを通常入力へ変更してください。ChromeOS実機でのPre2の入力検証は未実施です。

RustとAndroidの内部バージョンは`0.1.0-pre.2`です。ChromeOSは既存版からの番号の後退を避け、拡張機能の内部バージョン`0.1.1`を保持し、表示名を`0.1.0 Pre2`にしています。

WindowsのC#最新版との比較検査には、`nihongoGitHub`の分割差が残っています。Rustは「にほんご」と「GitHub」に分け、比較対象のC#版は全体を英字として扱います。この比較検査は引き続き有効で、Pre2の共通Core・各OSの動作検査とは別に追跡します。

各ファイルのSHA-256はSHA256SUMS.txtを参照してください。WindowsとChromeOSのZIPには内部ファイルのハッシュも含みます。ソースと各環境の詳細は同じリリースタグ内のrust/README.md、android/README.md、chromeos/README.mdを参照してください。
