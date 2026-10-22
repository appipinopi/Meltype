# 配布ファイルの選び方

配布先は [Meltype-flash v0.1.0 Pre2](https://github.com/appipinopi/Meltype/releases/tag/flash-v0.1.0-preview.2) です。
本家のC#版と区別するため、Flashのファイル名を使用します。

| ファイル | 用途 |
| --- | --- |
| `Meltype-flash-0.1.0-pre.2-setup.exe` | Windows向け。ウィザードでインストールし、専用IMEを登録します。管理者確認が出ます。 |
| `Meltype-flash-Windows-portable.zip` | Windows向け。展開後 `Start-Portable.cmd` で起動します。IME登録なしのバッファ入力で、設定・学習は展開先の `data` に保存します。 |
| `Meltype-flash-Windows.zip` / `Meltype-flash-Windows-x64.zip` | Windows向け。PowerShellで導入・更新するための従来の一式。同じ内容です。 |
| `Meltype-flash-Linux-x64.zip` | Linux x64向け。展開後 `sudo bash install.sh --system` で導入します。 |
| `Meltype-flash-Linux-arm64.zip` | Linux ARM64向け。同じ手順で導入します。 |
| `meltype-flash_0.1.0~pre.2_amd64.deb` | Ubuntu 24.04以降のx64向け。`sudo apt install ./meltype-flash_0.1.0~pre.2_amd64.deb` で導入します。 |
| `meltype-flash_0.1.0~pre.2_arm64.deb` | Ubuntu 24.04以降のARM64向け。`sudo apt install ./meltype-flash_0.1.0~pre.2_arm64.deb` で導入します。 |
| `Meltype-flash-macOS-arm64.zip` | Apple Silicon Mac向け。 |
| `Meltype-flash-macOS-x64.zip` | Intel Mac向け。 |
| `Meltype-flash-Android-debug.apk` | Android試験版。デバッグ署名です。 |
| `Meltype-flash-ChromeOS.zip` | ChromeOS拡張の試験版。通常のWindows版ChromeではIMEとして動作しません。 |
| `Meltype-flash-iOS-Simulator-arm64.zip` | Apple Silicon MacのiOS Simulator向け。iPhone実機にはインストールできません。 |
| `Meltype-flash-iOS-unsigned.ipa` | 自分のApple IDで署名してiPhoneに入れるための実機用IPA。WindowsからのAltStore Classicの手順は [iOSの案内](../ios/README.md) を参照してください。 |
| `Meltype-flash-iOS-source.zip` | Xcodeで自分の署名を設定して実機ビルドするためのソース。[iOSの手順と制限](../ios/README.md)を参照してください。 |

ZIPをアプリにドラッグしたり、ファイルの拡張子を変えたりして他のOSで使うことはできません。
LinuxはIBusエンジンです。導入後はログインし直して入力ソースにMeltype-flashを追加してください。
Linux・iOSには補助辞書による変換の制限が残っています。
Windowsのポータブル入力の制限と操作は [Windowsの案内](WINDOWS-PACKAGE.md) を参照してください。

追加配布のワークフローは既存Pre2のエンジンを使用し、公開済みファイルの置換を拒否します。
ファイルのSHA256は添付の `SHA256SUMS.txt` とGitHubの各ファイルの表示で照合できます。
iOSのソースには追加実装のコミットが含まれ、対応するコミット番号をリリース説明に記載します。
