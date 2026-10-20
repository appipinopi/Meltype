# Rust Windows版・配布前確認用

Windows 10/11 x64向けの開発用プレビューです。x86のTIPとworkerは32bitアプリ用です。正式インストーラー、更新、削除の動作確認は未完了です。このZIPにインストーラーは含めていません。

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

このZIPを展開しただけではIMEは登録されません。元版とRust版のTIPは同じCOM登録IDを使い、既存のIME登録に影響するため、普段使いの環境での上書き登録は配布前確認に含めていません。元版の `Register-Tip.ps1` はRust workerなどの依存ファイルをまとめて配置するインストーラーではありません。

検査の成功は、必要ファイル・PEのx64/x86・TSF公開関数・静的DLL依存・ZIP内ハッシュの確認です。クリーンなWindowsへの導入、OpenGL描画環境、動的ロードされる全コンポーネント、実アプリ入力・更新・削除の受入成功を意味しません。

## 再作成・再点検

リポジトリで `python rust/package-windows.py --help` を参照し、ビルド済みの設定/CLI/TIP/workerとVisual Studioの `VC/Redist/MSVC/<version>` を指定してください。既存フォルダーとZIPは上書きしません。

```powershell
python tools/check-rust-windows-package.py <展開フォルダー> --report <検査結果.json>
```
