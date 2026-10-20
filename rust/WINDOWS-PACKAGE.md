# Rust Windows版・配布前確認用

Windows 10/11 x64向けの開発用プレビューです。x86のTIPとworkerは32bitアプリ用です。登録・更新・登録解除用の `Manage-Windows.ps1` を同梱します。管理者権限が必要です。クリーン環境での導入・更新・削除の実動確認は未完了です。

## 内容

- `meltype-settings.exe`：設定画面。`--tray` で管理トレイを起動。
- `meltype-rs.exe`：共通CoreのCLI。
- `tip/x64` と `tip/x86`：各アーキテクチャの `MeltypeTip.dll` と `meltype-tip-worker.exe`。workerはDLLの隣に必要です。
- 各実行フォルダーのVisual C++ランタイムDLL：使用したVisual Studioの再配布可能CRTから同梱。
- `LICENSE`、`THIRD-PARTY-NOTICES.md`：本体と依存ライブラリのライセンス情報。
- `package-audit.json`、`SHA256SUMS.json`：静的依存関係・ファイル検査の結果とハッシュ。

システム辞書のFSTと値データは実行ファイルに組み込まれています。別のシステム辞書TXT/FSTや `sqlite3.dll` の配置は不要です。利用者の設定とユーザー辞書はRust版のデータフォルダーに作成され、このZIPには個人データを含めません。元版とは別の保存先です。

既定のデータに触れずに設定画面を確認する場合は、PowerShellで一時的な試験用フォルダーを指定します。

```powershell
$env:MELTYPE_RUST_DATA_DIR = Join-Path $env:TEMP 'Meltype-Rust-preview-check'
./meltype-settings.exe
```

## 制限

System変換にはWindowsの日本語言語コンポーネントが必要です。任意のMozcヘルパー・Mozc-UT・Zenzaiサーバーやモデルは同梱していません。Mozcが無い場合のSystemへのフォールバックは、実Mozcの動作確認とは別です。

このZIPを展開しただけではIMEは登録されません。元版とRust版のTIPは同じCOM登録IDを使い、既存のIME登録に影響するため、普段使いの環境での上書き登録は配布前確認に含めていません。元版の `Register-Tip.ps1` ではなく、同梱の管理スクリプトを使用してください。

検査の成功は、必要ファイル・PEのx64/x86・TSF公開関数・静的DLL依存・ZIP内ハッシュの確認です。クリーンなWindowsへの導入、OpenGL描画環境、動的ロードされる全コンポーネント、実アプリ入力・更新・削除の受入成功を意味しません。

## 再作成・再点検

リポジトリで `python rust/package-windows.py --help` を参照し、ビルド済みの設定/CLI/TIP/workerとVisual Studioの `VC/Redist/MSVC/<version>` を指定してください。既存フォルダーとZIPは上書きしません。

```powershell
python tools/check-rust-windows-package.py <展開フォルダー> --report <検査結果.json>
```

## 初回導入・更新・登録解除

設定画面には初回案内と「報告・更新」があり、トレイからも報告・配布一覧を開けます。
初回案内の完了は設定へ保存されます。ブラウザーに入力内容や学習データを自動送信しません。
更新案内はAutoUpdateがONなら起動時にバックグラウンドでRustフォークの公開配布を確認します。設定画面からも再確認できます。元C#版のZIPをRust版として案内しません。自動ダウンロード・置換は未接続です。

まず通常のPowerShellで検査し、導入時だけ管理者PowerShellを使用します。

```powershell
.\Manage-Windows.ps1 -Action Inspect
.\Manage-Windows.ps1 -Action Install -WhatIf
.\Manage-Windows.ps1 -Action Install
# 既存の元版Meltypeを置き換えるときだけ、バックアップ後に -ReplaceExisting を追加
.\Manage-Windows.ps1 -Action Update -Package 'C:\path\to\new-package'
.\Manage-Windows.ps1 -Action Uninstall
```

SHA256SUMS.jsonの全ファイルを検査し、Program Files/MeltypeRust/versions/<配布ハッシュ>へ配置します。
両アーキテクチャのDLLとworker・CRTをまとめて配置し、登録失敗時は以前のDLLを再登録します。
ハッシュは破損検知用です。配布元の認証や署名の代わりではありません。
元版との共通COM登録を置き換える場合は、元版のトレイを終了して起動設定を解除し、`-ReplaceExisting` を明示してください。
古いバージョンは保持するため、稼働中DLLの上書きを避けます。
更新後は旧トレイを終了し、新フォルダーからトレイを起動して起動設定も更新してください。

UninstallはIME登録を解除します。起動設定はトレイから解除して終了してください。
ロック中DLLと利用者データを勝手に消さないため、配布ファイルは残します。
アプリを終了・再起動した後にProgram Files/MeltypeRustを手動削除できます。
完全なアンインストーラー、起動設定の自動引継ぎ、実OSでのロールバック検証は残ります。

## ポータブル起動

```powershell
.\Start-Portable.ps1
# 管理トレイも起動する場合
.\Start-Portable.ps1 -Tray
```

設定画面と入力テストは、ZIP内のdataフォルダーを保存先として使えます。
環境変数は子プロセスだけへ引継ぎ、スクリプト終了時に元へ戻します。
これは設定/Coreのポータブル動作です。未登録の実アプリに入力するキーボード方式は未接続です。
別アプリ内のTSF workerはこの起動スクリプトの子ではないため、ポータブルdataの共有も未対応です。
