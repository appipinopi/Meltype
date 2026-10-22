# Sync fork と Meltype-flash

Flashの製品ブランチは `rust`。GitHubの既定ブランチも `rust` を使用する。
`main` は上流を取り込んで比較するために残し、過去のフォーク履歴は削除しない。

## GitHubのSync forkを使う場合

1. ブランチ選択を `main` にする。
2. Sync fork → Update branchで上流を取り込む。
3. 変更を確認し、Flashに必要な修正だけを `rust` 向けのPull Requestへ反映する。

競合時のDiscard commitsや、`--force` による上書きを製品ブランチで使用しない。
通常の同期で競合が出た場合はPull Requestで解決する。

CLIで参照ブランチだけを同期する場合:

```powershell
gh repo sync appipinopi/Meltype --source yksr-melt/Meltype --branch main
```

このコマンドに `--force` は付けない。`rust` への取り込みやインストールは実行しない。

## 戻りを防ぐ仕組み

- `rust` はPull Requestと `Flash identity` 検査を通して更新する。
- 製品ブランチの強制push・削除を禁止する。
- 検査ではFlashのREADME、起動EXE名、専用IME登録ID、保存先、配布入口を確認する。
- 上流のC#ビルド／配布／.NET更新／winget処理は元リポジトリに限定する。
- Flashのリリースは手動実行のみ。同期や通常pushから勝手に配布しない。
- 参照用C#コードはGitHubの言語集計から除外し、比較用ソースとして保持する。

上流のコミットでワークフローや表示が変わっても、製品検査で検出してから取り込む。
権限を持つ管理者が保護を無効化・迂回した場合まで、この仕組みで防げるとはしない。

Sync forkはGitHub上のソースの同期であり、PCにインストール済みのC#版をRust版へ変更する操作ではない。
Flashの実機導入は別の作業として行う。
