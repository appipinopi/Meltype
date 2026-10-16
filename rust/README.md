# Meltype Rust 移植プレビュー

Fork: https://github.com/appipinopi/Meltype · 元: https://github.com/yksr-melt/Meltype

**全機能をRustに書き換えた完成版ではありません。** 元の実装を残したまま、OS共通のRustエンジンと4OSから接続する入口を追加しています。元のC#の機能・精度と同等とは保証できません。元のREADMEのダウンロードリンクは上流版です。

## ビルド・確認

```sh
cargo test --workspace --locked
cargo run -p meltype-cli -- --convert kyouhagoogledekensaku
# きょうはgoogleでけんさく
cargo build --workspace --release --locked
```

Rust 1.88以上。WindowsはMSVC C++ビルドツール、LinuxはCリンカー、macOSはXcode command line toolsが必要です。Cargo.lockを共有しています。

| crate | 内容 |
|---|---|
| meltype-core | ローマ字表222件、日英混在判定、保護範囲、JISかな・字種変換、候補・予測・意味・英訳・誤字提案、永続学習・設定・辞書・バックアップ、Mozcヘルパー通信 |
| meltype-native | C ABI、既存フロントエンド用JSON、TSF用のプロセス内接続、Android JNI |
| meltype-settings | Rust製の設定・辞書・定型文・学習削除・バックアップ・入力テスト画面 |
| meltype-cli | 全デスクトップ共通の変換コマンド、JSON Linesセッション操作 |

CLIは引数なしでstdinからJSONを1行ずつ受け取ります。`{"op":"key","vk":0,"ch":107}`、`{"op":"key","vk":32,"ch":32}`、`{"op":"commit"}`、`{"op":"select","index":0}`、`{"op":"reset"}`、`{"op":"direct","enabled":true}`。再変換は `{"op":"reconvert_selection","text":"橋","reading":"はし"}`（readingは省略可）、確定Undoは `{"op":"undo_commit","before":"キャレット直前の文字列"}`。結果は `consumed/commits/view` のJSONです。削除時は `expect`、再変換開始時は `selectionExpect` をホスト側で必ず照合してください。vkは既存実装と同じWindows仮想キー、chはUnicodeスカラー、modifiersはShift=1/Ctrl=2/Alt=4/Command=8です。

## OSとの接続

| OS | ビルド | 残っているOS固有コード・制限 |
|---|---|---|
| Windows x64/x86 | `./rust/build-windows.ps1 -WithTip` | C++ TSFシェル + Rust DLL。C#サーバー不要。出力は `native/tip/bin/rust-x64` と `rust-x86`。自動インストールしません。元と同じCOM登録IDなので、既存インストールと共存させる配布には未対応です。 |
| macOS | `bash rust/build-desktop.sh` | Swift InputMethodKitシェルとazooKey変換エンジンを再利用。出力 `mac/build/Meltype.app`。インストールしません。 |
| Linux | Mozcヘルパーを準備後 `bash rust/build-desktop.sh` | Python IBus / C++ fcitx5シェル、C++ Mozcヘルパーを再利用。出力 `linux/build/Meltype-linux`。 |
| Android | [android/README.md](../android/README.md) | Java InputMethodServiceとJNI。補助辞書による変換。 |

デスクトップの既存ビルド手順はそのままで、上記のRust用ビルドだけがRustエンジンを選びます。C APIは `native/include/meltype.h` を参照。戻り値はmalloc由来で `meltype_free` で解放し、同一セッションの呼び出しを直列化してください。Macの変換コールバックはmalloc由来の文字列を返します。LinuxのMozcヘルパーには上流と同じC/US/RSプロトコルで接続し、応答のタイムアウトとサイズ制限を設けています。

## 追加済みの共通機能

- 上流の69設定項目を保持する設定形式、名前付きプロファイル、アプリ別設定の読み込み。未移植の設定は保存できますが、動作にはまだ反映されません。設定アプリは対応済み項目だけを標準で表示します。
- ディスクへの設定・学習・辞書・定型文の保存。複数プロセスの書き込みはロックと一時ファイルによる置換で保護します。データが壊れた場合は上書きせずエラーを返します。
- ユーザー辞書の登録・削除、UTF-8/UTF-16のMicrosoft IME/Google形式とApple XML plistの取り込み、UTF-16の書き出し。
- 予測候補とTab選択、翻訳候補、意味注釈、文脈辞書による候補選択、書き間違いの提案、定型文の展開。
- 外部変換器を接続したデスクトップでのライブ変換。かなが確定した日本語入力をプレビューし、Spaceで文節選択に進みます。英単語・保護範囲・未完のローマ字は変換器へ渡しません。
- TSFのコード行で日本語入力に切り替える操作、キャレット移動での解除、コメント・文字列の判定。
- 文節ごとの候補保持と選択、左右キーでの文節移動、Shift＋左右での境界調整。Mozc・macOSの文節を保持し、境界調整後は内蔵辞書で候補を再生成します。
- 選択範囲の再変換APIとAndroidの「再変換」操作。同梱辞書・ユーザー辞書・学習履歴から読みを推定し、取消時は元の文字列へ戻します。読みを取得できない語は変更しません。ホストは `selectionExpect` を現在の選択範囲と照合する必要があります。
- 直前の確定を未確定入力へ戻すAPI。削除可否・周辺文字列を確認し、削除時の期待文字列を返します。Androidに操作を接続済みです。
- JISかな入力、F8の半角カタカナ、F9/F10の大小文字切替、Ctrl+U/I/O/Pの字種切替・Ctrl+Hの削除、Shift+Spaceの全角空白、句読点、英単語の前後の空白。
- バックアップの作成・検査・復元、学習の全消去・期間指定削除。上流のデータファイルを指定したデータフォルダーから取り込む機能。

設定アプリは `cargo run -p meltype-settings` で起動します。データは上流版と分離して `MeltypeRust` に保存します。Androidはアプリ専用のfilesディレクトリに保存し、バックアップは無効にしています。`MELTYPE_RUST_DATA_DIR` でデスクトップの保存先を指定できます。日本語フォントが見つからない場合は `MELTYPE_FONT` でフォントファイルを指定してください。

CLIでは `meltype-rs --control '{"op":"settings_get"}'` の形式で管理操作を実行します。例:

```json
{"op":"settings_set","name":"PredictiveCandidates","value":false}
{"op":"profile_add","name":"仕事"}
{"op":"profile_switch","name":"仕事"}
{"op":"dictionary_set","reading":"めるたいぷ","word":"Meltype"}
{"op":"snippet_set","name":"sig","text":"署名"}
{"op":"backup_create","path":"/absolute/path/my.meltype-backup"}
{"op":"learning_forget_since","since":1792108800}
```

C ABIの `meltype_control`、Android JNIも同じ管理APIを使います。Androidにも設定・辞書・定型文・学習・プロファイル・バックアップ画面を追加しています。Androidのファイル取込・書出はシステムのドキュメント選択を使います。既に開いているデスクトップセッションには次の入力開始時に保存した値を読み直します。

## 移植が残っているもの

- 元の文脈スコア・高度な日英判定の完全な互換、確定後の訂正、外部変換器による境界調整後の再変換、デスクトップの選択範囲再変換への接続、未登録語の読み取得、日英混在・かな入力を含むライブ変換の完全互換
- JISかな入力の詳細な編集操作、ローマ字ミスタイプ訂正、Undoの全デスクトップ操作への接続、プロファイルの全設定を各OSへ反映する処理
- トレイ・モード表示・入力停止・起動設定・更新機構・配布用インストーラー
- C++/Swift/Python/JavaによるOS連携と外部変換エンジンをRustに置き換える作業
- 各OSでの実際の入力、フォーカス移動、パスワード欄、IME切替、候補表示、インストール・削除の実機検証

`cargo run -p meltype-core --example quality -- quality-report.json --check` は、元の品質テストから抽出した216件を変更せずに比較します。現在は **191/216一致** で、25件の不一致が残っています。`--check` はこの移植途中の基準からの退行を検出します。全機能互換を認定するチェックではありません。抽出元は `src/Meltype.Core.Tests/QualityTests.cs` です。

Windows/Androidで漢字変換する際は補助辞書だけでは語彙・精度が足りません。WindowsでもC APIからMozcヘルパーを登録可能ですが、Windowsパッケージの自動初期化は未実装です。

Rust previewワークフローは3OSのRustテスト・ライブラリ/CLIビルド、Windows TSFシェルのビルド、Android JNIライブラリとdebug APKのビルド・Lintを実行する設定です。CIの成功は実際の入力動作や全機能互換の証明ではありません。

## ライセンス

元コード・ローマ字表の著作権表記とGPL-3.0-or-laterを維持します。辞書の出典と個別ライセンスは元の各辞書ヘッダー、LICENSE、THIRD-PARTY-NOTICES.mdを参照してください。
