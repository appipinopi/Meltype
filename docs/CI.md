# CIの構成

Meltype-flashの検査は、次のGitHub Actionsワークフローで行います。

| ファイル | 役割 | 起動条件 |
| --- | --- | --- |
| `rust.yml` | Windows・Linux・macOSのRust検査、ABI・設定画面・参照実装との比較、Android検査の呼び出し | 関連ファイルのPR、手動 |
| `rust-android.yml` | JNI・APK・lint・実エミュレーターの検査 | Rust preview／リリースからの呼び出し、手動 |
| `fcitx-rust.yml` | Rust／従来版fcitx5アダプターのビルド、JSON・キー・状態・イベントABIの比較 | 関連ファイルのPR、Rust previewの手動実行からの呼び出し、単独の手動実行 |
| `macos-rust.yml` | Carbon・IMK・Swiftの統合検査とmacOSパッケージの検査 | 関連ファイルのPR、手動 |
| `chromeos.yml` | WASMとChromeOS拡張機能のビルド | 関連ファイルのPR、リリースからの呼び出し、手動 |
| `flash-identity.yml` | Flash固有の実行ファイル・登録ID・保存先・公開条件の保護 | rust／mainへのpush・PR、手動 |

Androidの手順は`rust-android.yml`だけで管理します。fcitx5の手順も`fcitx-rust.yml`だけで管理し、PRでは専用CI、Rust previewの手動実行では再利用呼び出しで検査します。

Rust previewは参照用C#コード、設定画面の検査スクリプト、CI補助スクリプト、ブランド画像の変更でも起動します。`docs/`内だけの変更では起動しません。PRの更新時は同じPRの古い検査を中止します。手動実行は進行中の実行を中止せず、Android・fcitx5は呼び出し元ごとに待機を分けます。

制限時間はRust desktopが90分、Androidが45分、fcitx5が30分、Flash identityが5分です。macOS SDKは既存の15分／30分を維持します。

## ベンチマークと画像

`rust.yml`のWindows desktopジョブで、前のRust公開版と現在版の共通Coreを比較します。Cargoのバージョンより前の公開タグを数値順に探します。現在はPre3なので`flash-v0.1.0-preview.2`が基準です。手動実行の`benchmark_baseline`でタグまたはコミットを指定することもできます。

両版を同じRustコンパイラでReleaseビルドします。現在版のRelease依存関係を利用し、前の版はCoreと測定ホストだけを追加ビルドします。プロセスを交互に測定する前に両バイナリを固定し、元のビルド先が上書きされても影響を受けないようにします。各版のコード・依存関係・同梱辞書の違いを含む比較です。コンパイラ・コミット・辞書・ソース・バイナリのハッシュも保存します。

日本語・混在・英語・URL・編集の計21ケースで、各ケースを1回ウォームアップして5回測定します。旧→新、新→旧の2巡で、各版・各ケース10試行です。外部変換・予測・学習・TSF・描画は対象外で、プロセス全体のCPU・RAMとキー処理の時間は区別します。

実行のSummaryに中央値・新/旧の時間比・比較対象の状態一致を表示します。`windows-common-benchmark`成果物から、次を取得できます（保存期間14日）。

- `charts/speed-32-64.png`、`speed-128-256.png`、`speed-short-edit.png`: 全21ケースの時間の分布
- `charts/cpu.png`、`charts/ram.png`: 各版2プロセスのCPU・メモリ観測値
- `report.json`、`*.jsonl`、`resources.json`、ビルドログ: 測定条件・ハッシュ・生データ
- `charts/summary.json`、CSV、統計JSON、`README.md`: 再描画用データと説明

箱ひげ図は外れ値もすべて残します。動作が違うケースには`TRACE MISMATCH`を付け、同等動作の速度比較から除外します。ソースやバイナリが測定中に変わった場合は画像生成を失敗させます。共有ランナーの速度変動を考慮し、固定倍率での合否判定は行いません。

画像にはmatplotlib 3.10.8とNumPy 2.4.2を使います。依存関係を測定前に導入し、測定中は同じジョブで他のビルドを動かしません。測定ステップは20分で打ち切り、後続テストが失敗した場合も測定済みデータを保存します。成果物には基準ソースやバイナリを含めません。

測定専用のジョブやOSマトリックスは増やしません。C#との比較はCIから外し、整形確認もLinuxの1回にまとめます。従来の手動C#比較は`tools/benchmark-windows.py`で引き続き実行できます。

## 手元での確認

変更したワークフローに`actionlint`を実行します。加えて、Flashの識別情報を次で確認できます。

```powershell
python tools/check-flash-identity.py
python -m unittest discover -s tools -p test_flash_identity.py -v
```

各OSの統合検査はGitHub Actionsで確認します。リリース公開は引き続き手動操作で、`rust-release.yml`と`rust-distributions.yml`が担当します。上流C#版のビルド・配布ワークフローにあるリポジトリ条件は維持します。

## 整理時点で確認した既存の失敗

2026年10月22日の[実行39777313015](https://github.com/appipinopi/Meltype-Flash/actions/runs/39777313015)には、Windowsの`nihongoGitHub`の参照実装との分節不一致と、Androidの確認ダイアログで「復元」ボタンを検出できない失敗があります。今回の整理では該当テストを維持しています。CI設定の検証成功と、各OSの統合検査成功は別に確認してください。
