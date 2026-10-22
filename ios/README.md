# Meltype-flash iOS プレビュー

Rustの共通コアを使うiPhone・iPad用カスタムキーボードです。iOS 15以降。
QWERTY入力、英語・日本語の判定、補助辞書による変換、候補選択、空白、削除、確定を実装しています。
未確定文字はキーボード上部に表示し、確定した文字だけを入力欄へ送ります。
通信・フルアクセス・アプリ間の共有領域は使用しません。学習と設定の永続保存は無効です。

## ビルド

MacにXcode、Rust、XcodeGen (`brew install xcodegen`) を用意して `bash ios/build.sh` を実行します。
Apple Siliconのシミュレーター用ZIPと、未署名の実機用アプリを生成します。
シミュレーター用ZIPは実機にはインストールできません。

実機では `ios/MeltypeFlash.xcodeproj` をXcodeで開き、アプリとキーボード拡張の両方に
自分の開発チームと固有のBundle Identifierを設定し、署名を有効にしてビルドしてください。
TestFlight / App Store配布にはApple Developerの配布署名とプロビジョニングが必要です。
このリポジトリには署名鍵がないため、署名済みIPAやApp Store公開は行っていません。

インストール後、設定 → 一般 → キーボード → キーボード → 新しいキーボードを追加から
Meltype-flashを追加し、🌐で切り替えます。フルアクセスの許可は不要です。

## 制限と検証

初期プレビューです。フリック入力、デスクトップの外部変換エンジン、翻訳、アプリごとの設定は未対応です。
入力欄を移動したときは未確定文字を破棄します。入力欄の確定済み文字をさかのぼって修正しません。
パスワード欄などではiOSが標準キーボードへ切り替え、ホストアプリが外部キーボードを禁止する場合もあります。
Rust ABIの入力・確定・長文・セッション分離と、Xcodeでの実機用／シミュレーター用ビルドをCIで検証します。
実機での入力、メモリー使用量、各アプリでの動作とApp Store審査は別途確認が必要です。

Apple資料: [カスタムキーボード](https://developer.apple.com/documentation/uikit/creating-a-custom-keyboard)、
[配布署名](https://help.apple.com/xcode/mac/current/en.lproj/devcac6ab5b3.html)。
