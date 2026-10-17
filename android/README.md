# Meltype Rust — Android IME preview

Android 8.0 (API 26) 以降、arm64-v8a / x86_64。変換処理とセッションは Rust、InputMethodService のライフサイクルと入力欄・ソフトキーボード表示は Java です。ネットワーク権限は要求しません。元実装と同等の変換精度・機能を備えた完成版ではありません。

必要なもの: Rust 1.88以上、JDK 17、Gradle 8.13、Android SDK platform 35 / build-tools 35.0.0、NDK 27.2.12479018。

リポジトリのルートで:

```sh
rustup target add aarch64-linux-android x86_64-linux-android
cargo install cargo-ndk --version 4.1.2 --locked
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/27.2.12479018"
CARGO_NDK_PLATFORM=26 cargo ndk -t arm64-v8a -t x86_64 -o android/app/src/main/jniLibs build -p meltype-native --release --locked
gradle -p android assembleDebug lintDebug --no-daemon
adb install -r android/app/build/outputs/apk/debug/app-debug.apk
```

Windows では `export` の代わりに PowerShell の `$env:ANDROID_NDK_HOME = ...`、APIレベルには `$env:CARGO_NDK_PLATFORM = '26'` を使います。既存の `ANDROID_NDK_ROOT` / `ANDROID_NDK` があれば同じNDKのパスに揃えてください。

設定 → システム → キーボード → 画面キーボードで **Meltype Rust Preview** を有効にし、入力方式を切り替えます。QWERTY ローマ字、変換・候補タップ・確定・削除、英数切替、ハードウェアキーに対応する実装です。パスワード・数字入力は直接入力にし、学習禁止の入力欄も入力を保持しません。通常の入力では学習とユーザー辞書をアプリ専用ディレクトリに保存します。パスワード欄と学習禁止欄では保存しません。予測候補はタップで確定できます。

アプリのアイコン、キーボードの「設定」、Androidの入力方式設定から管理画面を開けます。入力設定・プロファイル・辞書の登録/削除/取込/書出・定型文・学習削除・バックアップの保存/検査/復元に対応しています。ファイルの選択はAndroidのドキュメント画面を使い、ストレージ全体へのアクセス権限は要求しません。バックアップの復元と学習の全消去には確認画面を表示します。

入力欄で文字列を選択して「再変換」を押すと、辞書・学習履歴から読みを取得して変換候補を表示します。読みが取得できない語は変更しません。「取消」または外付けキーボードのEscで取り消すと選択前の文字列に戻ります。

文節は左右キーで選択し、「縮める」「伸ばす」で境界を調整します。「戻す」は直前の確定を未確定へ戻します。キャレット直前の文字列が一致した場合だけ削除します。外付けキーボードではShift＋左右でも境界を調整できます。

漢字候補は同梱の補助辞書によるものです。Android版にはMozcのエンジン・辞書を同梱していません。フリック入力、配布署名、Play配布、実機検証は未対応です。CIのdebug APKは開発検証用です。

API仕様: [InputMethodService](https://developer.android.com/reference/android/inputmethodservice/InputMethodService)、[Rust Android targets](https://doc.rust-lang.org/rustc/platform-support/android.html)。

入力欄への応答反映はRustからJNIでInputConnectionを直接呼び出します。選択文字と削除前の文字をUTF-16単位で照合し、未確定文字の置換、確定、バッチ編集の開始・終了をRustで処理します。削除対象と選択範囲が競合する場合や、応答の表示データが不正な場合は書き込みません。編集が途中まで成功した後の失敗や、書込み中の例外ではキーを重ねて送らないようにし、コア状態をリセットします。Androidのバッチは通知をまとめるためのもので、書込みを巻き戻す仕組みではありません。

`cargo build -p meltype-native --features android-jni-test --locked` と `python rust/native/tests/android_editor_smoke.py target/debug` は、JDK 17以降のJVMで実際のJNIを検証します。専用の入力欄を使い、絵文字・単独サロゲートの照合、選択、順次確定、拒否・例外・部分編集、未確定文字、実コアのローマ字変換と確定を確認します。Android端末への登録・操作を行う試験ではありません。サービスのライフサイクル、キーイベント、ソフトキーボードと管理画面には引き続きJavaが残っています。

入力欄の開始時の判定と外付けキーの変換もRustへ移しました。InputTypeのクラス／バリエーションと学習禁止フラグから、直接入力・非公開入力を決め、前の入力状態と削除権限を解除します。KeyEventの公開キー値・メタ状態はRustで従来の仮想キー／修飾ビットへ変換し、修飾キー単体はコアへ送らず通します。JVM試験は旧Javaマッピングを比較用に残し、実コアに未確定文字がある状態で840ケースの応答一致、秘密欄から通常欄への切替と修飾キーを確認します。キーイベントの受取り自体、入力欄の周辺文字取得、サービスと画面はJavaに残っています。

入力欄の前後4000 UTF-16単位の取得、削除対応の確認、表示中の未確定文字を文脈から取り除く処理、Undo時の周辺文字取得もRust／JNIへ移しました。非公開入力欄と修飾キー単体では取得APIを呼びません。取得が例外になった場合は削除を無効にし、空の文脈として扱います。ソフトキーの直接入力への代替もRustで、Unicode文字の確定、コードポイント単位の削除、左右キーのdown/up、EditorInfoのEnterアクション／改行を呼びます。

ContextHostの実JVM試験は前後文字の受渡し、未確定文字の除外、秘密欄の無取得、取得例外、実コアの入力→確定→Undo、絵文字の入力／削除とEnter動作を確認します。左右キーのJNIコンストラクター／引数確認にはテスト専用のSDKと同じ形のKeyEventクラスを使います。Android端末上の実KeyEvent配信を検証したものではありません。InputMethodServiceのコールバック、ソフトキーボード・候補ボタンと管理画面のJava、端末での実動確認は残ります。

入力終了時の最終確定とリセット、キャレット／選択範囲の変更時の未確定状態解除、再変換する選択文字の取得と検証もRust／JNIへ移しました。同じセッションの終了中に同期通知が再入した場合は二重に終了処理を行いません。入力終了の表示結果はJavaの描画関数へ返し、確定処理を重ねません。非公開欄・変換中・空／不正なUTF-16選択では再変換しません。

LifecycleHostのJVM試験は、実コアの最終確定、繰り返し終了、キャレット位置を保つ通知、範囲選択の解除、終了通知の再入、書込拒否・入力欄なし・終了例外、読みのある／ない選択の再変換を確認します。InputMethodServiceのOSコールバックと画面描画はJavaが受け持ち、端末上でのフォーカス移動や各アプリとの動作確認は未完了です。

Androidサービスが保持していた英数・非公開・Shift・未確定状態と表示文字を、セッションごとのRust所有領域へ移しました。Javaは表示時に状態を参照し、同じ値を独立に保持しません。入力欄の開始・明示リセット・選択変更で世代を更新し、古い編集／終了の結果は新しい入力欄の状態を上書きしません。世代変更後の追加書込みは止め、開始済みのバッチは終えます。すでに完了した編集のキーは、入力欄が切り替わった後も重ねて送り直しません。

「あ/A」の確定からモード切替までをRust内で処理し、確定中に別の欄へ切り替わった場合は新しい欄のモードを変えません。選択通知後と入力ビューの再生成時はRustの現在の表示を使います。StateHostのJVM試験は独立した複数セッション、1回だけのShift、モード／非公開状態、未確定文字、書込み中の欄切替・失敗・破棄と、新しい入力状態の保持を確認します。ソフトキーボード・候補・管理画面のJava描画とOSコールバック、Android実機での確認は残ります。
