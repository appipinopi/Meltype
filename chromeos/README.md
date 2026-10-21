# ChromeOS 試験版 — chrome.input.ime + Rust/WASM

2026-10-21時点の実装です。ChromeOSの実機を使った導入・入力試験は未実施です。Windows版・全機能の互換性を保証する版ではありません。

ソースコード: [appipinopi/Meltype](https://github.com/appipinopi/Meltype)。GPL-3.0-or-laterの全文、辞書等の第三者表示、解決済みRust依存ライブラリのライセンス本文をZIPへ同梱します。

## 使用するAPIと2023年の告知

対象は`chrome.input.ime`です。Googleは2023年7月にChromeOS 117での廃止予定を告知し、8月には最短119まで延期しました。[公式告知](https://chromeos.dev/en/posts/chrome-input-ime-deprecation)。廃止予定の告知と、実際に削除されたことは区別します。

現在の[公式API文書](https://developer.chrome.com/docs/extensions/reference/api/input/ime)は存在し、2026-10-21に確認した[Chromiumの機能定義](https://chromium.googlesource.com/chromium/src/+/e060ba3ace847abe456467721691a2b4821b22fa/chrome/common/extensions/api/_api_features.json)には`input`権限・ChromeOS・privileged extensionという条件が残ります。特定の製品版・管理端末で使える証明にはなりません。APIがなければ設定画面で理由を表示し、入力を横取りする代替処理は行いません。Manifest V3を使い、Manifest V2やChrome Appsには依存しません。

## 作成と導入

必要なものはRust stable、Python 3.9以上、Node.js 22以上、**wasm-bindgen-cli 0.2.129**です。バインディング生成ツールとCargoで固定した版を一致させます。

```powershell
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
python chromeos/build.py
```

既存の生成ツールは`--wasm-bindgen <実行ファイル>`、別の配布先は`--out <専用フォルダー>`で指定できます。`CARGO_TARGET_DIR`も尊重します。

`dist/chromeos-preview`と同名ZIPを生成します。ZIPを展開して、対応ChromeOSの`chrome://extensions`でデベロッパーモードを有効にし「パッケージ化されていない拡張機能を読み込む」からフォルダーを選びます。これは拡張機能の開発者向け設定で、OSのDeveloper Modeへ切り替える手順ではありません。管理端末では管理者の許可・ポリシーに従います。ChromeOSの「言語と入力 → 入力方法」でMeltype Rustを追加し、入力方法として選択します。メニューの表記や導入可否はOSと管理設定で異なります。

Chrome Web Storeへは公開していません。`manifest.json`の最低119はパッケージの下限であり、119以降の全バージョンで試験した意味ではありません。Windows/LinuxのChromeではIME APIを利用できません。

## 接続した機能

- 同じ`meltype-core::Session`をWASMへコンパイル。ローマ字・かな処理、判定、同梱FST辞書・補助候補、句読点・キー操作を共有する。
- キーの順序を保ち、非同期`onKeyEvent`には`keyEventHandled`を一度だけ返す。変換中の文字列、候補、選択位置、注釈をOSのIME APIへ送る。カーソル位置はUTF-16単位。
- Space、候補移動、クリック選択、Enter、Esc、F6〜F10などを共通Coreへ渡す。ChromeOSが先に処理するキーは取得できない。
- フォーカス・入力先種別の変更、無効化、resetで未確定状態を破棄する。非同期初期化中に入力先が変わった古いイベントは適用しない。パスワード、不明・数値・電話番号欄、他拡張機能からのイベントは処理しない。
- `chrome.storage.local`へ設定・学習のスナップショットを保存。`shouldDoLearning=false`やシークレットでは学習保存しない。設定JSONの編集、未知の設定の保持、AutoSwitch/Bothの移行、学習削除、JSON書き出しを用意する。
- API操作・保存の失敗を診断表示する。壊れた保存データを既定値で上書きしない。未確定文字列・周辺文は保存しない。

## 制約と残る確認

Coreのデスクトップ標準ビルドでは従来のSQLite・ファイルロック・原子的保存を維持します。WASMでは`filesystem` featureを外して、同じ学習モデルだけを共有します。ChromeOS版の保存形式はJSONであり、SQLiteを実装したとは扱いません。

OS辞書/Mozcの接続、OSのクリップボード・GitHubへの取得、アプリ別プロセス検出、Windowsの登録・トレイ・自動更新は接続しません。OS辞書に依存する候補の全同等性、全画面一致、ライブ変換・再変換のホスト統合も受入未完了です。設定項目を保存できても、その項目が必要とするOS連携まで動作する意味ではありません。

既存文書の削除・確定後訂正・Undoは`can_delete=false`で制限します。カーソル移動を伴う`CursorIntoBrackets=true`は明示的に拒否します。レイアウトはmanifestで1つの`us::eng`を宣言し、DOMの物理codeからOEM/JISキーを対応付けています。実JIS配列とかな入力・AltGr・拡張文字、媒体キー・修飾キー、候補ページ、複数ウィンドウ、スリープ復帰・Service Worker再起動はChromeOS実機で確認が必要です。Service Worker再起動後は確定済みの学習と設定を復元しますが、未確定文字列を復元しません。

ストレージ容量を超える場合は保存失敗を表示します。読み取り権限やサーバーへの権限は要求しません。拡張機能を削除すると保存データも失うため、必要なら設定画面からJSONを書き出してください。スナップショット復元画面と辞書編集は未対応です。

## 検証

`python chromeos/build.py`は実WASMで変換・候補・選択学習・再起動復元・学習禁止・設定移行・F6/F7を検査し、APIアダプターの順序・UTF-16位置・フォーカス競合・パスワード拒否・初期化/IME API失敗を模擬APIで検査します。APIモック成功をChromeOS実機成功とは扱いません。ZIPには同梱ファイルのSHA256とライセンスを含めます。現在の移行状況は[移行記録](../rust/MIGRATION.md)、全体構成は[ファイル解説](../FILE_GUIDE.md)を参照してください。
