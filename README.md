# Meltype-flash

![Meltype-flash](assets/branding/readme-banner.svg)

Meltypeを参照して開発している、Rust版の日本語入力です。製品の開発ブランチは **`rust`** です。

- [Windowsの起動・導入・制限](rust/WINDOWS-PACKAGE.md)
- [ビルド方法と移植状況](rust/README.md)
- [本家との共存・専用IME登録ID](rust/BRANDING.md)
- [上流の安全な取り込み](rust/UPSTREAM-SYNC.md)
- [画面の再現状況](rust/UI-V0.1.1.md)

## 起動

Rust 1.88以上と、WindowsではMSVC Build Toolsが必要です。

```powershell
cargo run -p meltype-settings
# トレイを起動する場合
cargo run -p meltype-settings -- --tray
```

起動するのは **meltype-flash.exe** です。IME本体を含むWindowsビルドは次のコマンドです。

```powershell
./rust/build-windows.ps1 -WithTip
```

## 配布

この変更でMeltype-flashのリリースは公開していません。既存の `rust-v0.1.0-preview.1` は改名前のRustプレビューです。
新しい配布は `Meltype-flash-*` という名前で、専用のFlashリリースワークフローから作成します。
参照元のC#版インストーラーをFlashのダウンロードとして案内しません。

## 上流との関係

参照元は [yksr-melt/Meltype](https://github.com/yksr-melt/Meltype)（GPL-3.0-or-later）です。
`main` は上流の取り込み確認用として履歴を保持し、製品ブランチ `rust` と分けます。
Sync forkは選択したブランチへ上流を取り込む操作です。Flashへの反映は専用検査を通すPull Requestで行います。

C#の `src/` は比較・検証用の上流参照コードです。GitHubの言語集計では参照コードとして除外しますが、ソースは削除しません。
Rustで未実装の機能や、各OSで未確認の動作は [移植状況](rust/README.md) に記載しています。
