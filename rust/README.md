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

Rust 1.88以上。WindowsはMSVC C++ビルドツール、LinuxはCリンカーと `libibus-1.0-dev`、macOSはXcode command line toolsが必要です。Cargo.lockを共有しています。

| crate | 内容 |
|---|---|
| meltype-core | ローマ字表222件、日英混在判定、保護範囲、JISかな・字種変換、候補・予測・意味・英訳・誤字提案、永続学習・設定・辞書・バックアップ、Mozcヘルパー通信 |
| meltype-native | C ABI、既存フロントエンド用JSON、TSF用のプロセス内接続、Android JNI |
| meltype-settings | Rust製の設定・辞書・定型文・学習削除・バックアップ・入力テスト画面 |
| meltype-cli | 全デスクトップ共通の変換コマンド、JSON Linesセッション操作 |
| meltype-ibus | Linux IBusのRust製エンジン実行ファイル。libibus/GLibへ直接接続し、Python/C++の接続コードを使用しない |

CLIは引数なしでstdinからJSONを1行ずつ受け取ります。`{"op":"key","vk":0,"ch":107}`、`{"op":"key","vk":32,"ch":32}`、`{"op":"commit"}`、`{"op":"select","index":0}`、`{"op":"reset"}`、`{"op":"direct","enabled":true}`。再変換は `{"op":"reconvert_selection","text":"橋","reading":"はし"}`（readingは省略可）、確定Undoは `{"op":"undo_commit","before":"キャレット直前の文字列"}`。結果は `consumed/commits/view` のJSONです。削除時は `expect`、再変換開始時は `selectionExpect` をホスト側で必ず照合してください。vkは既存実装と同じWindows仮想キー、chはUnicodeスカラー、modifiersはShift=1/Ctrl=2/Alt=4/Command=8です。

## OSとの接続

Linux IBusのRust版は `bash rust/build-linux.sh` で `linux/build/Meltype-Rust-linux/` と、実行権限を保持する配布用の `linux/build/Meltype-Rust-linux.tar.gz` に組み立てます。ビルドには `libibus-1.0-dev` が必要で、実行時はシステムのIBus/GLibを利用します。Rust側のエンジンとOS接続は同じ実行ファイルに含まれ、Python、GI、別のRust DLLは実行時に不要です。既存のPython版は上流実装の参照用として残っています。fcitx5は引き続き既存C++アダプターです。

入力ソース用アイコンは元版の `linux/icon.png` をそのまま同梱し、登録XMLにインストール先の絶対パスを記録します。日本語・空白・引用符を含む配置先でも、アイコンの内容と登録パスをインストール試験で照合します。

生成フォルダーで `sudo bash install.sh --system` を明示的に実行すると、`/usr/libexec/meltype-rust/ibus` にプログラムを置き、IBusが通常読み込む `/usr/share/ibus/component/meltype-rust.xml` を登録します。既存の上流版の登録やRustの設定・学習データは置き換えません。IBusの再起動や入力ソースの選択は行わず、サインインし直した後に「Meltype Rust」を選びます。`sudo bash /usr/libexec/meltype-rust/ibus/install.sh --system --uninstall` は専用マーカーと登録名を確認して配布ファイルを削除し、設定・学習と未知のファイルは保持します。`--prefix` は別の絶対パスへの配置用です。登録・削除は一時フォルダーだけで検証しています。

管理者権限を使わない `bash install.sh` はユーザーのデータフォルダーへ配置します。ただしIBusはこの登録先を通常は自動で読みません。この方式ではIBus起動前に `IBUS_COMPONENT_PATH` へ表示されたフォルダーとシステムの登録フォルダーを設定する必要があります。削除は `bash install.sh --uninstall` です。

`meltype-ibus --version` は版を表示し、`--print-component` は実行ファイルの現在位置に対応するIBusのXMLを出力します。`--ibus` は登録されたIBusから起動される入口で、引数なしは検証用の動的コンポーネント登録です。どちらも通常のユーザー設定へ入力ソースを追加しません。Mozcヘルパーは `mozc/meltype_mozc_helper` または `MELTYPE_MOZC_HELPER` で指定します。ヘルパーを同梱できないビルドは内蔵補助辞書に限られ、元版の変換語彙とは同等ではありません。

`xvfb-run -a dbus-run-session -- /usr/bin/python3 rust/ibus/tests/integration.py target/release/meltype-ibus` は別のIBusデーモン・候補パネル・合成入力欄でRust実行ファイルを検証します。`MELTYPE_IBUS_TEST_INSTALLED=1` を付けると、一時フォルダーへの実インストール後にXMLからIBusが自動起動する経路で同じ10ケースを検証します。Python/GIはこのテスト用クライアントだけで使用します。フォーカスを外した場合はIBusが表示中の未確定文字を確定するため、Rustは同じ文字を再挿入せず、その表示を学習します。Enterによる確定では従来どおり末尾の `n` もかなに確定します。通常のデスクトップアプリ・Wayland/X11・サインインし直した後の動作は未検証です。

| OS | ビルド | 残っているOS固有コード・制限 |
|---|---|---|
| Windows x64/x86 | `./rust/build-windows.ps1 -WithTip` | C++ TSFシェル + Rust DLL（表示属性COM・言語バー・候補／意味ウィンドウはRust実装）。C#サーバー不要。出力は `native/tip/bin/rust-x64` と `rust-x86`。自動インストールしません。元と同じCOM登録IDなので、既存インストールと共存させる配布には未対応です。 |
| macOS | `bash rust/build-desktop.sh` | Swift InputMethodKitシェルとazooKey変換エンジンを再利用。出力 `mac/build/Meltype.app`。インストールしません。 |
| Linux | `bash rust/build-desktop.sh` | Rust IBus実行ファイル。出力 `linux/build/Meltype-Rust-linux`。漢字変換には別途C++ Mozcヘルパーが必要です。fcitx5は既存C++アダプターを別途ビルドします。 |
| Android | [android/README.md](../android/README.md) | Java InputMethodServiceとJNI。補助辞書による変換。 |

デスクトップの既存ビルド手順はそのままで、上記のRust用ビルドだけがRustエンジンを選びます。C APIは `rust/native/include/meltype.h` を参照。戻り値はmalloc由来で `meltype_free` で解放し、同一セッションの呼び出しを直列化してください。Macの変換コールバックはmalloc由来の文字列を返します。LinuxのMozcヘルパーには上流と同じC/US/RSプロトコルで接続し、応答のタイムアウトとサイズ制限を設けています。

WindowsのRustビルドは、元の `DisplayAttributes.cpp` をコンパイルせず、Rustの `ITfDisplayAttributeInfo` / `IEnumTfDisplayAttributeInfo` を使用します。入力中の点線・変換済みの細線・選択文節の太線、GUID、日本語説明とアプリの色を維持します。COM参照は `Release`、説明のBSTRは `SysFreeString` で解放します。通常の文字列用 `meltype_free` と混用しないでください。`rust/native/tests/tsf_com_smoke.py` はSDK互換のvtableを実際に呼び、Rust DLLに実装があることも照合します。CIは64bitと32bitのPythonプロセスで、TSFシェルのCOMプロバイダーからの表示属性取得まで確認します。

`LangBar.cpp` もRustビルドから除外し、入力モードボタン・`ITfSource` の通知購読・「あ／A」の描画・ツールチップ・クリックをRustで処理します。元と同じGUID・日本語表示・タスクバーの明暗に合わせた透明アイコンを使います。残るC++入力サービスにはモード取得と切替のコールバックだけを接続し、終了時に切り離します。コールバックは生成したTSFスレッドでだけ実行し、通知先が通知の途中で購読を解除する操作にも対応します。COM試験では両インターフェースの同一性、モード切替、再入する通知、終了後の切離し、アイコンの非空画素・透明度・リソース解放を確認します。現在の環境で描画した2種類のアイコンは元C++版と全画素が一致しています。

`CandidateWindow.cpp` もRustビルドから除外しました。Direct2D／DirectWriteの候補・注釈・入力キー・訂正提案・予測表示、9件のページ、選択色、画面内配置、1.5秒後の意味欄をRustで処理します。ウィンドウは入力欄のフォーカスを奪いません。文字の寸法・フォント・余白・テーマ色・色付き絵文字の描画方式を元版から移し、現在の環境では6種類の表示と意味欄の寸法・クライアント画素が元C++版と一致します。`tsf_candidate_smoke.py` は実際の非アクティブHWNDを生成し、x64/x86でページ番号の対応・予測クリック・タイマー取消・画面端の配置・複数UIスレッド・クリック中の破棄を検証します。比較用の `reference_candidates.cpp` は元ソースを直接コンパイルするテスト専用DLLで、製品には同梱しません。キー処理・編集セッション・COMサーバー登録などのTSF接続にはまだC++が残り、実際のタスクバー・各アプリ・複数モニター上での動作は未検証です。

WindowsのクラスファクトリーもRustへ移しました。Rustビルドは元のC++ `ClassFactory` を除外し、`DllGetClassObject` からRustの `IClassFactory` を返します。クラスとインターフェースの照合、非集約の生成、参照管理、`LockServer` をRustで処理し、未移植の入力サービスの生成だけをC++コールバックへ渡します。ファクトリーが存在する間と明示的なサーバーロックの間は、コールバックを持つTSF DLLを使用中として保持します。

明示的な `DllRegisterServer` / `DllUnregisterServer` もRustの登録処理へ接続しました。元と同じHKLMのCOM登録先、日本語プロファイルID・言語ID、5種類のカテゴリ、Apartment設定、シェルDLLのアイコンを使います。登録が失敗した場合のカテゴリ・プロファイル・COMの後片付けと、削除を続行する動作を代替バックエンドで検証します。実際のユーザー環境への登録・解除・有効化は行っておらず、その動作は未検証です。

編集セッションの `ITfEditSession` オブジェクト・`RequestEditSession` 呼出・即時結果と非同期結果の扱いもRustへ移しました。非同期要求ではOSが保持するCOM参照が残る間、C++側の入力編集コールバックとDLLを保持します。実行・取消・即時拒否の各経路でコールバックを一度だけ解放し、捕捉したCOM参照などを破棄してからDLLの使用中参照を解放します。`tsf_edit_session_smoke.py` は制御したITfContextを使い、x64/x86で6経路の実COM呼出を確認します。`build-edit-session-host.ps1` は実際のTSFシェルのオブジェクトファイルをテストプログラムへリンクし、捕捉したC++ラムダの寿命・DLL参照も確認します。テストは実際の入力欄やIME登録を操作しません。キー処理・選択範囲と未確定文字の編集そのもの・COMサーバーの薄いDLL入口は引き続きC++です。

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
- 元のScoreEngineのスコア寄与・閾値・かな判定・編集距離加点を移植。抽出した判定128例と早期判定のタイミングが一致します。設定画面の入力テストと `detection_evaluate` 管理APIで理由を確認できます。
- Windows TSFの英数状態検知。選択範囲が空で、入力欄が期待文字列を照合して削除できる場合だけ、日本語への切替を行います。保留キーを隠す方式とは異なり、直前の英字を確認して置き換えます。
- 読めない子音の周辺を直すローマ字ミスタイプ訂正。隣接キー・欠落・余計なキー・入替を比較し、候補が競合した場合や保護された入力は変更しません。
- JISかなの濁点・半濁点キーの削除、かな入力でのF6〜F8、Undo時のキー履歴復元、Romaji/Kana/Both。
- JISかな入力、F8の半角カタカナ、F9/F10の大小文字切替、Ctrl+U/I/O/Pの字種切替・Ctrl+Hの削除、Shift+Spaceの全角空白、句読点、英単語の前後の空白。
- 日付・時刻の候補と日時文節の結合、数字に続く計算記号の候補、通常候補の後ろに絵文字を並べる処理。日時や計算記号は通常の変換として学習しません。
- 次の語に応じた直前の曖昧な語の訂正。最大4語まで保持し、削除能力と期待文字列を確認します。キャレット移動、手動選択、学習済みの言語選択を尊重します。共通の日本語を英字にする学習は2回目から有効です。
- バックアップの作成・検査・復元、学習の全消去・期間指定削除。上流のデータファイルを指定したデータフォルダーから取り込む機能。

設定アプリは `cargo run -p meltype-settings` で起動します。データは上流版と分離して `MeltypeRust` に保存します。Androidはアプリ専用のfilesディレクトリに保存し、バックアップは無効にしています。`MELTYPE_RUST_DATA_DIR` でデスクトップの保存先を指定できます。日本語フォントが見つからない場合は `MELTYPE_FONT` でフォントファイルを指定してください。

デスクトップの設定変更・プロファイルの編集は「OK」または×で保存し、「キャンセル」で破棄します。開いている間に他のプロセスが変えた項目は、画面で変更していなければ保持します。プロファイルの変更も名前ごとにマージします。判定テストは保存前の設定を使い、テスト入力の学習はディスクへ保存しません。辞書・定型文などの管理操作は各画面の登録・削除操作で保存します。プロファイルの入出力は元版と同じ `meltype-profile` 形式で、OS標準のファイル選択を使います。

ユーザー辞書はローマ字の読みをかなに直してプレビューし、内蔵辞書・ユーザー辞書の候補を選んで登録できます。辞書・定型文・バックアップの取込と書出もファイル選択から行います。定型文は元版と同じ「名前 → 文」のJSON形式を使い、同じ名前は取り込んだ文に置き換えます。UTF-8/UTF-16に対応し、不正な文書では既存データを変更しません。学習画面は日英・変換・英訳・文の一覧と個別の「忘れる」操作を表示します。バックアップを開くと内容を表示し、復元すると画面の設定も読み直します。

ローマ字入力の「慎重」は、ローマ字として読める英単語を英文の続きや前後の英語から判断し、`google` のようにローマ字として読めない既知語は英字のまま表示します。「積極的」は片側の英語も根拠にします。「手動」は英字の提案を表示し、Tabで選ぶまで自動では切り替えません。大文字で始めた語は明示的な英字指定として扱います。手動では学習済みの英字指定や確定後の自動訂正でかな表示を置き換えません。

`SigilWordsDirect` が有効なローマ字入力では、先頭か空白直後の `/command`・`$skill`・`@src/app.ts` を変換ボックスへ溜めず、各キーをアプリへ渡して補完を利用できます。空白・改行の後は通常入力に戻ります。ホストが空白キーを送らない場合も、次に渡されるキャレット前の文字から名前の終了を判断します。Escで補完を閉じた後は名前を続けられ、Tab・キャレット移動・ショートカットの後は位置の情報を破棄します。名前の本文は保持せず、境界と文字数だけを追跡します。

Windowsでは `meltype-settings --tray` でTSF用の管理トレイを起動できます。同じデータフォルダーでのトレイの二重起動を防ぎ、有効／一時停止、判定の強さ、プロファイル、設定・辞書・定型文・学習・バックアップ・判定テスト、データフォルダーを開く操作に対応します。ダブルクリックは有効／一時停止です。設定をOK・×で閉じると保存してトレイに戻り、キャンセルでは編集を破棄します。「終了」は設定を保存し、共有設定を無効にしてトレイを終了します。この管理トレイはIMEの登録や切替を行いません。

一時停止／再開のホットキーは元版と同じ順で `Ctrl+Alt+F12`、`Ctrl+Alt+F11`、`Ctrl+Shift+Alt+A`、`Ctrl+Alt+Pause` の登録を試み、使える組合せをメニューに表示します。終了時に登録を解除します。有効状態・プロファイル変更の通知は `ShowNotifications` に従います。「Windows の起動時に起動」は現在のユーザー用の専用タスクを登録し、タスク登録が拒否された場合はユーザーのRun登録へ切り替えます。解除では両方の専用登録を削除します。登録・解除はメニューで操作したときだけ行い、元版の起動登録は変更しません。

元版の全メニュー・その他の通知・更新・キャレット付近のモード表示、およびmacOS/Linuxの管理トレイは未移植です。自動起動の登録処理は代替コマンド／レジストリを使うテストで確認しており、サインインし直した際の実機動作は未検証です。

CLIでは `meltype-rs --control '{"op":"settings_get"}'` の形式で管理操作を実行します。例:

```json
{"op":"settings_set","name":"PredictiveCandidates","value":false}
{"op":"profile_add","name":"仕事"}
{"op":"profile_switch","name":"仕事"}
{"op":"dictionary_set","reading":"めるたいぷ","word":"Meltype"}
{"op":"snippet_set","name":"sig","text":"署名"}
{"op":"snippets_import","path":"/absolute/path/snippets.json"}
{"op":"snippets_export","path":"/absolute/path/snippets.json"}
{"op":"backup_create","path":"/absolute/path/my.meltype-backup"}
{"op":"learning_forget_since","since":1792108800}
```

C ABIの `meltype_control`、Android JNIも同じ管理APIを使います。Androidにも設定・辞書・定型文・学習・プロファイル・バックアップ画面を追加しています。Androidのファイル取込・書出はシステムのドキュメント選択を使います。既に開いているデスクトップセッションには次の入力開始時に保存した値を読み直します。

## 移植が残っているもの

- 元の文脈スコア・高度な日英判定の完全な互換、確定後の訂正、外部変換器による境界調整後の再変換、デスクトップの選択範囲再変換への接続、未登録語の読み取得、日英混在・かな入力を含むライブ変換の完全互換
- JISかな・Both入力とローマ字ミスタイプ訂正の全回帰ケースへの適合、Undoの全デスクトップ操作への接続、プロファイルの全設定を各OSへ反映する処理
- トレイの全メニュー・モード表示・入力停止・各OSの起動設定と実機検証・更新機構・配布用インストーラー
- 残っているC++/Swift/JavaによるOS連携と外部変換エンジンをRustに置き換える作業（IBusのPython接続はRust版を追加済み）
- 各OSでの実際の入力、フォーカス移動、パスワード欄、IME切替、候補表示、インストール・削除の実機検証

`cargo run -p meltype-core --example quality -- quality-report.json --check` は、元の品質テストから抽出した216件を変更せずに比較します。期待文字列を照合して削除できるホストでは **216/216一致**、削除できないホストでは **215/216一致** です。後者では `site PR` の確定後の空白訂正を行いません。比較ツールも削除前の期待文字列とUTF-16長を検査します。`--check` は両方の基準からの退行を検出します。これは抽出した品質ケースの一致であり、全機能互換を認定するチェックではありません。抽出元は `src/Meltype.Core.Tests/QualityTests.cs` です。

Windows/Androidで漢字変換する際は補助辞書だけでは語彙・精度が足りません。WindowsでもC APIからMozcヘルパーを登録可能ですが、Windowsパッケージの自動初期化は未実装です。

Rust previewワークフローは3OSのRustテスト・ライブラリ/CLIビルド、Windows TSFシェルのビルド、Android JNIライブラリとdebug APKのビルド・Lintを実行する設定です。CIの成功は実際の入力動作や全機能互換の証明ではありません。

## ライセンス

元コード・ローマ字表の著作権表記とGPL-3.0-or-laterを維持します。辞書の出典と個別ライセンスは元の各辞書ヘッダー、LICENSE、THIRD-PARTY-NOTICES.mdを参照してください。
