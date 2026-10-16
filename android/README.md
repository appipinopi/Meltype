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
