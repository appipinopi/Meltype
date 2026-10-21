# Windows・共通コアの C# / Rust 比較

開発の優先対象は Windows と共通コアです。他 OS の連携部分の移植は保留し、元版との動作一致と、この Windows 計測を先に確認します。

リポジトリのルートから実行します。Python 3.12 以降、.NET 10 SDK と NativeAOT 用の Visual C++ ビルド環境、Rust が必要です。

```powershell
$env:RUSTUP_HOME = Join-Path $env:USERPROFILE '.rustup'
$env:CARGO_TARGET_DIR = 'C:/opencode-rust/meltype-target-1.94'
$env:PYTHONUTF8 = '1'
python tools/benchmark-windows.py --output dist/windows-common-benchmark --samples 15 --warmups 3 --rounds 2
```

比較対象の C# は、Git のv1.3.0コミット `6310c44b728d5ad88284853f9ef37ea224993b18` から取り出します。通常は C# NativeAOT と Rust Release を比較します。`--jit` を付けた場合は結果に JIT と明記されます。`--baseline-ref` で基準を変更できますが、両版に埋め込む辞書の SHA-256 が違う場合は計測を中止します。

対象はメモリ内のキー処理と、各キーでの JSON 生成です。日本語・英日混在・英語・URL の 32 / 64 / 128 / 256 キー、短文 4 件、Backspace と F7 を含む編集 1 件の計 21 ケースを実行します。各ケースの最後に Enter で確定します。入力メソッドのインストール・OS 登録・ユーザー辞書の読み取りは行いません。

条件をそろえるため、両版とも同梱のスペル辞書を有効にします。OS スペルチェック・外部漢字変換エンジン・ライブ変換・予測・学習を無効にし、周辺文字の削除を許可します。キー処理の速度では、セッションと辞書の生成、プロセス起動、結果照合は計測区間外です。CPU・RAMは別の区間で測定します（下記参照）。**Windows の TSF、候補ウィンドウ描画、実際のアプリへの入力、起動速度、変換エンジンの速度は比較に含みません。** この段階の結果をアプリ全体の高速化率として扱わないでください。

以前のホストはC#の同梱スペル辞書も無効にしていたため、その結果を公平な比較に使用しないでください。修正版はCore全ソース・辞書・ハーネスのハッシュをビルド前に記録し、ビルド中の変更を検出すると中止します。Rustの測定用実行ファイルは出力先へコピーして固定します。測定中にソースやバイナリが変わった場合は `accepted_performance_baseline=false` とし、速度比較の対象から除外します。他のビルド等の負荷は自動検出できないため、測定中は実行しないでください。

既定では各ケースを 3 回ウォームアップしてから 15 回測り、C#→Rust、Rust→C# の順で 2 周します。各試行の設定・学習状態は作り直します。計測中は別のビルドや重い処理を避けてください。

出力先の `report.json` に次を保存します。

- マシン、コンパイラ、コミット、作業ツリー、辞書・入力・計測ソースのハッシュ
- 全試行の所要時間・各キーの時間、ケースごとの中央値・キー時間の p95
- 確定文字と、キーごとの consumed / commits / 表示文字 / 変換状態の照合結果
- Rust / C# の時間比。小さいほど Rust が速く、1 より大きい場合は遅い

途中表示が異なるケースは `eligible_for_speed_claim=false` とし、同じ動作の高速化とは扱いません。候補一覧・注釈・ヒント等を含む完全な UI 一致はこのフラグの判定対象外です。生の応答も各 `*.jsonl` に残るため、別途確認できます。固定倍率の合否基準は設けず、遅い結果や不一致もそのまま報告します。

## CPU・RAMの測定

`report.json` の `process_resources` と途中保存の `resources.json` に、各巡のCPU時間・使用率とRAM使用量を記録します。Windows APIで約50ms間隔で観測し、CPU使用率はカーネル＋ユーザーCPU時間を経過時間で割ります。1コア基準と、さらに論理CPU数で割ったPC全体基準を併記します。

常駐RAM（Working Set）は共有ページを含み、Private Bytesは専用のコミット済みメモリです。RAMの割合はOSから見える全物理RAMを分母とします。プロセス全体の値は起動・辞書／セッション生成・ウォームアップ・JSON出力も含み、キー処理のみの速度とは計測区間が異なります。実際の常駐IME全体やアイドル時の使用率を示しません。短時間のピークは観測間隔によって取りこぼす可能性があります。上記の速度測定の対象外という記述と区別してください。

## 速度を測らず各キーの差を調べる

ビルド済みの同じホストを使って、未知語・助詞・小書きかな・大文字・保護範囲・削除後の再入力を61ケース照合できます。

```powershell
cargo +stable build -p meltype-core --example benchmark --release --locked
python tools/check-core-parity.py --csharp dist/windows-common-benchmark/csharp/Compare.exe --rust C:/opencode-rust/meltype-target-1.94/release/examples/benchmark.exe --output dist/core-parity
```

結果の `report.json` に確定文字の一致数、途中状態までの一致数、全不一致キー・元版の期待値・Rustの実際の値を保存します。`--cases` で任意のキー列JSONを指定できます。`--profile` は `dictionaries/romaji.txt` を配置した試験専用データディレクトリーを両版へ渡します。実際の利用者のプロファイルは指定しないでください。`--check` を加えると途中状態の不一致で終了コード1になります。2026-10-18時点で既定61件と大文字・日本語接続の追加17件は全打鍵が一致しました。重複を除いた75入力を `rust/core/tests/data/upstream-balanced-keys.json` に固定し、独自ローマ字表12入力とともに通常のRustテストでも検査します。Balanced・周辺削除可・学習／ライブ変換／予測無効という限定条件の結果で、全設定・全Core機能の一致を示しません。

この照合用コマンドは速度の評価には使用しません。速度測定はソースと実行ファイルを固定し、他のビルド等が走っていない状態で行ってください。

`--cases` には通常の配列と、固定試験JSONの `cases` オブジェクトを指定できます。
`--scored` で両ホストの全入力にスコア区切りを適用します。各入力の `scored` が無ければ従来どおりOFFです。
スコア区切りONの87入力は[専用の再現手順](../../parity/scored/README.md)を参照してください。
長文21入力と境界追加42入力の63件は[長文・境界の再現手順](../../parity/long/README.md)を参照してください。元版の2,530打鍵を固定期待値とし、通常のRust試験とDLLのABI試験で照合します。
比較ホストは `-p:CoreProject=<基準コミットから取り出したCoreプロジェクトの絶対パス>` にも対応します。

## 現在の測定結果

**[2026-10-21の再測定・全5枚の箱ひげ図](results/20261021-boxplots/README.md)** が現在の共通Coreの結果です。Rust `ed6067e` / C# v1.3.0 NativeAOT、各入力30試行・各版10プロセス。21/21入力が一致し、全入力でRustの中央値が短い結果でした。中央値・四分位・全外れ値を表示し、CSVと再現用summary.jsonを公開しています。画像生成は `python tools/plot-windows-benchmark.py <report.jsonまたはsummary.json> --output <新規フォルダー>`（matplotlib/numpyが必要）。実IME全体の測定ではありません。

以下は過去の測定です。

[1.3.0取り込み途中のC# / Rust比較](results/v130-initial.md)は21/21入力の確定・比較対象の全打鍵状態が一致し、21/21でRustの中央値が短い結果です。C#の基準はv1.3.0です。

[旧Rustと198ecddを同じrustc 1.99.1で比較](results/rust-f3ede6a-vs-198ecdd.md)したところ、14/21で新しい版が速く、7/21で遅い結果でした。以前はrustcの版と試行数が異なっていたため、そのままコード変更の回帰とは断定できません。RAM増加と混在短文の悪化は残っています。

全機能・実アプリ・OS変換・候補描画の速度を示すものではありません。FST検索部分の改善は別に記録し、全IMEの倍率には使いません。

古い報告・生ログ・生成物・比較専用の一時ビルドを整理しました。[過去のGit履歴](https://github.com/appipinopi/Meltype/tree/96d575f/tools/performance/windows/results)で元の報告を参照できます。現在の生測定データと固定Rustバイナリ、必要なコンパイラ情報はdistの今回の比較先に残しています。
