# Windows・共通コアの C# / Rust 比較

開発の優先対象は Windows と共通コアです。他 OS の連携部分の移植は保留し、元版との動作一致と、この Windows 計測を先に確認します。

リポジトリのルートから実行します。Python 3.12 以降、.NET 10 SDK と NativeAOT 用の Visual C++ ビルド環境、Rust が必要です。

```powershell
$env:RUSTUP_HOME = Join-Path $env:USERPROFILE '.rustup'
$env:CARGO_TARGET_DIR = 'C:/opencode-rust/meltype-target-1.94'
$env:PYTHONUTF8 = '1'
python tools/benchmark-windows.py --output dist/windows-common-benchmark --samples 15 --warmups 3 --rounds 2
```

比較対象の C# は、Git の移植基準コミット `45e14f86558aa6ea34f2c2ecd038ba95ca8c3c46` から取り出します。通常は C# NativeAOT と Rust Release を比較します。`--jit` を付けた場合は結果に JIT と明記されます。`--baseline-ref` で基準を変更できますが、両版に埋め込む辞書の SHA-256 が違う場合は計測を中止します。

対象はメモリ内のキー処理と、各キーでの JSON 生成です。日本語・英日混在・英語・URL の 32 / 64 / 128 / 256 キー、短文 4 件、Backspace と F7 を含む編集 1 件の計 21 ケースを実行します。各ケースの最後に Enter で確定します。入力メソッドのインストール・OS 登録・ユーザー辞書の読み取りは行いません。

条件をそろえるため、両版とも同梱のスペル辞書を有効にします。OS スペルチェック・外部漢字変換エンジン・ライブ変換・予測・学習を無効にし、周辺文字の削除を許可します。セッションと辞書の生成、プロセス起動、結果照合は計測区間外です。**Windows の TSF、候補ウィンドウ描画、実際のアプリへの入力、起動速度、変換エンジンの速度、メモリ消費の比較は含みません。** この段階の結果をアプリ全体の高速化率として扱わないでください。

以前のホストはC#の同梱スペル辞書も無効にしていたため、その結果を公平な比較に使用しないでください。修正版はCore全ソース・辞書・ハーネスのハッシュをビルド前に記録し、ビルド中の変更を検出すると中止します。Rustの測定用実行ファイルは出力先へコピーして固定します。測定中にソースやバイナリが変わった場合は `accepted_performance_baseline=false` とし、速度比較の対象から除外します。他のビルド等の負荷は自動検出できないため、測定中は実行しないでください。

既定では各ケースを 3 回ウォームアップしてから 15 回測り、C#→Rust、Rust→C# の順で 2 周します。各試行の設定・学習状態は作り直します。計測中は別のビルドや重い処理を避けてください。

出力先の `report.json` に次を保存します。

- マシン、コンパイラ、コミット、作業ツリー、辞書・入力・計測ソースのハッシュ
- 全試行の所要時間・各キーの時間、ケースごとの中央値・キー時間の p95
- 確定文字と、キーごとの consumed / commits / 表示文字 / 変換状態の照合結果
- Rust / C# の時間比。小さいほど Rust が速く、1 より大きい場合は遅い

途中表示が異なるケースは `eligible_for_speed_claim=false` とし、同じ動作の高速化とは扱いません。候補一覧・注釈・ヒント等を含む完全な UI 一致はこのフラグの判定対象外です。生の応答も各 `*.jsonl` に残るため、別途確認できます。固定倍率の合否基準は設けず、遅い結果や不一致もそのまま報告します。

## 速度を測らず各キーの差を調べる

ビルド済みの同じホストを使って、未知語・助詞・小書きかな・大文字・保護範囲・削除後の再入力を61ケース照合できます。

```powershell
cargo +stable build -p meltype-core --example benchmark --release --locked
python tools/check-core-parity.py --csharp dist/windows-common-benchmark/csharp/Compare.exe --rust C:/opencode-rust/meltype-target-1.94/release/examples/benchmark.exe --output dist/core-parity
```

結果の `report.json` に確定文字の一致数、途中状態までの一致数、全不一致キー・元版の期待値・Rustの実際の値を保存します。`--cases` で任意のキー列JSONを指定できます。`--profile` は `dictionaries/romaji.txt` を配置した試験専用データディレクトリーを両版へ渡します。実際の利用者のプロファイルは指定しないでください。`--check` を加えると途中状態の不一致で終了コード1になります。移行途中の全61ケースはまだ一致していません。元版の各キーを固定した未知語9入力、音単位境界9入力、独自ローマ字表12入力は通常のRustテストでも検査します。

この照合用コマンドは速度の評価には使用しません。速度測定はソースと実行ファイルを固定し、他のビルド等が走っていない状態で行ってください。
