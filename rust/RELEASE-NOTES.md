# Meltype-flash v0.1.0 Pre6（準備中）

2026年10月24日の開発プレビュー準備です。公開タグは `flash-v0.1.0-preview.6` を予定しています。全OSの配布物はまだ揃っておらず、公開・配布は行っていません。

## Pre6の変更

- 共通Core: 読み取り専用の辞書を共有し、辞書複製時の追加ヒープ要求を4,189,182バイトから0バイトへ削減しました。初期辞書・OSのRSS・アプリ全体のメモリー削減率はこの測定に含みません。
- 共通Core: 報告された英語・ローマ字・ドットファイル等の判定、1文字読みのユーザー辞書、選択文節へのF6〜F10を修正しました。日付・曜日・アプリ名・バージョンなどの候補も補いました。
- 設定: z記号変換の無効化、無変換でのカタカナ切替、テンキー直接確定、日本語と数字の自動空白を追加しました。いずれも既定オフです。テンキーの識別接続はWindows向けです。
- 入力: 候補のPage Up／Down移動を追加し、Windows TSFのCtrl+Enterを入力中の確定処理へ届けます。
- Linux: IBus・fcitxの設定済み切替キーと、ターミナルで日本語へ切り替える操作を修正しました。
- 管理: 設定用JSON Schemaの生成・書き出しと、Pre6以降のAltStore PAL配布に向けた検査スクリプトを追加しました。Appleの署名・公証・PAL公開は未完了です。

全242件の上流Issueの完了ではありません。[対応一覧](../docs/ISSUE-AUDIT.md)に48件分の限定的な修正・確認・部分対応を記録しています。[追加設定の使い方](../docs/ISSUE-OPTIONS.md)も参照してください。

### Pre6の確認範囲

版番号更新前の修正コードで共通Core 210件、Windows native単体55件、TSFキー検査18件、Windows／Linuxの切替API各7件、隔離IBus 23件が成功しました。IBusは初回に既存ケース1件が失敗し、再検査で再現しませんでした。原因特定とは扱いません。

Pre6へ版番号を更新した後、CLIのコンパイル検査、追加操作・設定Schemaの13件、PAL検査スクリプトの9件が成功しました。全OSのPre6配布ビルドは未実施です。

| OS | 修正コードのビルド | 実機・報告アプリ |
| --- | --- | --- |
| Windows | Core・TSF DLL・設定アプリ・CLI成功。インストーラー未実施 | 未実施 |
| macOS | 未実施 | 未実施 |
| Linux | WSLのIBus・native・fcitx5アドオン成功。設定UI・配布パッケージ未実施 | 通常デスクトップ未実施。隔離IBus検査は上記参照 |
| Android | 未実施 | 未実施 |
| ChromeOS | 未実施 | 未実施 |
| iOS | 未実施 | 未実施 |

Rust・Androidの内部版番号は `0.1.0-pre.6`、AndroidのversionCodeは6です。iOSのビルド番号は既存の10より大きい11とし、マーケティング版番号は `0.1.0` を維持します。ChromeOSは内部版番号 `0.1.5`、表示名 `0.1.0 Pre6` です。

以下はPre5以前の履歴です。過去の検証成功をPre6の検証結果として扱いません。

## Meltype-flash v0.1.0 Pre5

2026年10月23日の開発プレビューです。

## Pre5の変更

- 共通Core: 候補リスト初期化時の余剰確保を削減しました。同じ辞書の要求中ヒープを約9.4%削減し、品質評価の全出力一致を確認しています。アプリ全体のRAMの削減率ではありません。
- 共通Core: 学習・登録語を含む予測処理の不要な複製と全辞書走査を減らし、候補順を保つ回帰テストを追加しました。
- ChromeOS・共通Core: 英字と日本語が混ざる入力の文節を保持し、`chatgptwotsukau` → `chatgptを使う`、大文字を含む`ChatGPT`、Ctrl+Enterでの確定を修正しました。
- Android: 変換・入力・フリック・設定画面・背景処理をRustへ集約しました。OS接続用クラスはRustからビルド時に生成します。背景色・テーマ・好きな画像を選べます。
- ベンチマーク: Windows・macOS・LinuxでCoreとNative C ABIを比較できます。前版を自動選択し、CoreはRust2者比較5枚とC#を含む3者比較5枚を生成します。表示範囲を自動調整し、極端な外れ値は表へ残します。
- CI: 主な4入口と管理・Web・スポンサー処理の計7本へ整理しました。

### 確認範囲と残る不一致

Pre5準備前の検証では、Windows・macOS・Linux・Android・ChromeOS・iOSのビルドが成功しました。Androidエミュレーター、iOSシミュレーターと各OSの物理端末IME入力は区別しており、物理端末での入力確認は全OS未実施です。

WindowsのMicrosoft IMEとC#参照の比較テストでは`plain:にほんご`の変換結果が一致せず、後続のWindows統合検証は未完了です。この不一致を修正済みとはしていません。取り込み前にも別の`nihongoGitHub`分節不一致がありました。共通品質評価は216/216、周辺削除不可の入力先は215/216です。

以下はPre4からの変更履歴です。

## Pre4の変更

- Windows: TSFへ全角スペース設定を伝え、通常SpaceとShift+Spaceを設定どおり処理します。
- Windows: Brave等のChromium系ブラウザーで、通常欄にアプリ独自のPassword属性だけが付く場合を処理します。Windows側の秘密欄指定と読み取り失敗時の入力拒否は維持します。
- Windows: メニュー操作中のキーをアプリへ渡し、変換中のCtrl+K/Lを文節長変更の処理へ届けます。
- Windows: Keyboard方式で遠隔操作ツール等の注入入力設定を反映し、Chrome Remote Desktop・Parsecの稼働判定を追加しました。自己送信したキーは再処理しません。
- 共通Core: zh/zj/zk/zl等の矢印・記号入力、gameoverの英語保持、Fキーで表記を選んだ後の新しい入力を修正しました。
- macOS: 辞書選択パネルの有効化と前面表示を追加しました。実機での操作確認は未実施です。
- ChromeOS: 固定したMozc辞書からオフライン変換用データを生成し、拡張機能へ同梱します。
- Linux: 設定画面の初回案内にWindows専用の導入手順が表示される問題を修正しました。WSLg上のGTK入力欄と実IBusエンジンで、日本語の入力・確定を確認しました。

全145件の実機での解消を保証するものではありません。行頭スラッシュの新仕様、TSFの左右Alt単独切替などに差分が残っています。Rustの対象試験274件とWindows側の追加試験327件が成功しました。

## Pre3までの変更

2026年10月22日のプレリリースです。Pre2以降に確認したWindows・Linux・macOS・Androidと共通Coreの不具合を修正しました。

- Windows: ポータブル入力の修飾キー・長文・フォーカス切替・アプリ別設定を修正。変換キーを処理し、メールアドレス等の入力欄では英数字を直接入力します。
- Windows: トレイ、終了処理、報告・修正履歴ダイアログを改善。テキスト入力アプリの設定を実際の対象判定へ接続しました。
- Linux: フォーカス移動時の二重確定、機密欄と学習抑止の区別、周辺テキスト削除の検証、候補選択、カーソル移動、Shift+Tabを修正。IBusの設定画面起動とインストール失敗時の復元を改善しました。
- macOS: マーク中の文字列を置き換える際、既存文字列と照合してからマークを解除し、重複挿入を防止します。
- Android: フリック入力と実際の編集処理を接続し、操作・ライフサイクルの回帰検査を追加しました。
- 共通Core: 登録名「ちな→千奈」により「ちなみに」が「千奈みに」となる分割を修正。OpenRouter・TokenRouterを英単語辞書へ追加しました。

上流の高優先度Issueはクローズ済みを含む42件を照合し、現在のRust版に確認できた不具合を修正しました。各Issueに挙がる実アプリ・実機条件すべてでの解消を保証するものではありません。ベンチマークは同じコンパイラー・21入力ケースでPre2と比較し、結果と測定条件をリリース本文に掲載します。測定資料はダウンロード用に添付しません。

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

ChromeOSはManifest V3とchrome.input.imeを使用する試験版です。ChromeOS製品版・管理ポリシーによって導入できない場合があります。Windows/macOS/Linux上のChrome用IMEではありません。Chrome Web Store公開は行っていません。サイト別切替は初期状態では無効です。設定の「サイトごとの自動切替」を有効にし、タブ参照を許可して保存してください。周辺テキストを渡さないエディターではコメント・文字列内の日本語入力を自動判別できないため、そのサイトを通常入力へ変更してください。ChromeOS実機でのPre5の入力検証は未実施です。

RustとAndroidの内部バージョンは`0.1.0-pre.5`です。AndroidのversionCodeとiOSのビルド番号は5です。ChromeOSは既存版から更新できる拡張機能の内部バージョン`0.1.4`を使い、表示名を`0.1.0 Pre5`にしています。

WindowsのC#最新版との比較検査には、`nihongoGitHub`の分割差が残っています。Rustは「にほんご」と「GitHub」に分け、比較対象のC#版は全体を英字として扱います。この比較検査は引き続き有効で、Pre3の共通Core・各OSの動作検査とは別に追跡します。

各ファイルのSHA-256はSHA256SUMS.txtを参照してください。WindowsとChromeOSのZIPには内部ファイルのハッシュも含みます。ソースと各環境の詳細は同じリリースタグ内のrust/README.md、android/README.md、chromeos/README.mdを参照してください。
