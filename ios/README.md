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

## Windowsから自分のiPhoneへ入れる

`Meltype-flash-iOS-unsigned.ipa` は実機向けの未署名アプリとキーボード拡張を含みます。
ファイルをiPhoneで開くだけでは入りません。WindowsのAltServerとiPhoneのAltStore Classicを使い、
ご自身のApple IDで署名して導入します。Macは不要です。AltStore PALとは別の手順です。

1. [AltStore ClassicのWindows公式手順](https://faq.altstore.io/altstore-classic/how-to-install-altstore-windows)に従って、必要なApple製ソフトとAltServerをWindowsに導入します。
2. iPhoneをUSBでつなぎ、「このコンピュータを信頼」を許可して、AltServerからAltStore Classicを入れます。Apple IDの入力はAltServer/AltStoreの画面で行います。
3. 必要に応じてiPhoneの「設定 → 一般 → VPNとデバイス管理」で自分の開発元を信頼し、iOS 16以降では「設定 → プライバシーとセキュリティ → デベロッパモード」を有効にします。
4. iPhoneでPre2の `Meltype-flash-iOS-unsigned.ipa` をダウンロードし、AltStore Classicの「My Apps」の「＋」から選びます。**アプリ拡張の選択が出たらキーボード拡張を残してください。拡張を削除するとキーボードとして使えません。**
5. インストール後、下記の手順でキーボードを追加します。

無料アカウントのアプリは通常7日で期限が切れるため、AltServerが動くPCに接続してAltStoreで更新します。
アプリ本体とキーボード拡張にはそれぞれApp ID枠が必要です。
詳細は [AltStoreの更新・制限](https://faq.altstore.io/altstore-classic/your-altstore) と
[App IDの説明](https://faq.altstore.io/altstore-classic/app-ids) を参照してください。
この手順による実機での入力は未検証です。CIのビルド・Simulatorでの検証とは区別しています。

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
