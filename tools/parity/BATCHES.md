# 有限コーパスの分割比較と再開

全68設定の有限値域の直積は約2.07×10²⁴構成で、全件を実行したとは扱わない。OS辞書にも任意の入力・文脈・版・学習状態がある。ここでの完了は、指定した有限コーパスの全入力をC#と比較できたことを指す。

## 設定の入力例を増やす

```powershell
python tools/parity/settings-candidates/exhaustive.py dist/settings-expanded --offset 128 --limit 512 --scenes all
python tools/parity/batch_compare.py settings --cases dist/settings-expanded/cases.json --csharp tools/parity/settings-candidates/bin/Debug/net10.0/Meltype.Core.Tests.dll --dll target/release/meltype_native.dll --x86-dll target/i686-pc-windows-msvc/release/meltype_native.dll --x86-python dist/python-3.12.10-win32/python.exe --output dist/settings-expanded/compare --batch-size 320
```

`--scenes all` は既存の20入力例を各構成に適用する。既定の `basic` は従来どおり2入力例。全入力履歴・全候補源を列挙するオプションではない。上記はoffset 128から640までの512構成、10,240シナリオ。UIやOS起動設定の実動はCore入力結果だけでは確認できない。

## 同梱読みを全件含める

```powershell
python tools/parity/windows-converter/corpus.py dist/os-all --all
python tools/parity/batch_compare.py windows --cases dist/os-all/cases.json --csharp tools/parity/windows-converter/bin/Debug/net10.0-windows/WindowsConverter.dll --dll target/release/meltype_native.dll --x86-dll target/i686-pc-windows-msvc/release/meltype_native.dll --x86-python dist/python-3.12.10-win32/python.exe --output dist/os-all/compare --batch-size 128
```

現データでは20,181読み＋境界26入力＝20,207要求。C# x64参照を採取してから、Rust各ABIの変換・文節・全候補と順序を照合する。利用者の文書へ入力したりIMEを登録したりせず、変換APIを問い合わせる。利用可能件数とWinRT期限切れの規定内再試行を別々に記録する。OSの版や学習状態が変わるので、このPCでの成功を全OS・全状態の成功とは扱わない。

OSの生応答は学習内容を含み得るため `dist/` 内に保存する。公開するのは集計・ハッシュと同梱語由来の範囲情報に留める。既知の期限境界の差を切り分ける際は他のビルド・負荷を止め、失敗入力の参照から別実行として採り直す。先行の失敗を上書き・消去しない。

`--reuse-sta` を比較コマンドに追加すると、C#ホストの `--reuse` とRust診断APIの `requests` 配列を使い、バッチ内で変換器を保持する。実workerと同じSTA所有期間の比較であり、1要求ごとに初期化する既定モードとは区別する。Rustの単一要求APIは維持し、診断配列は最大256要求。64入力の試行では両モードの結果が一致したが、所要時間の改善は確認できていない。

## 重複のない分割と集計

```powershell
python tools/parity/partition.py dist/settings-expanded/cases.json dist/settings-shards --shards 4 --group-size 20
python tools/parity/partition.py dist/os-all/cases.json dist/os-shards --shards 4
```

各 `cases-0.json` ～ `cases-3.json` をbatch_compareへ渡し、出力を同じ分割フォルダー内の `compare-0` ～ `compare-3` にする。設定は20入力例を同じ分割に保持する。独立したプロセスとして実行できるが、実OSの並行照会は期限切れにも影響するため、シャード数と初回の不一致を記録し、失敗入力の単独再照合を別に残す。

```powershell
python tools/parity/summarize_shards.py dist/settings-expanded/cases.json dist/settings-shards dist/settings-expanded/summary.json
python tools/parity/summarize_shards.py dist/os-all/cases.json dist/os-shards dist/os-all/summary.json
```

集計は全元入力との内容一致、重複・欠落、分割間の同じDLL・ホスト・ツール・所有期間、証跡ハッシュと個別結果を確認する。途中でも集計できるが、未比較入力があればpassedはfalse。公開用の集計に候補の生応答は含めない。

## 成功判定と中断

- `summary.json` の `checked` と `matched` を分け、要求数・一致・不一致・実IME利用件数・期限切れ再試行をABI別に数える。`passed` は全件確認・不一致なし・比較プロセス成功の場合だけtrue。
- 各バッチの生出力・比較ログを保持する。不一致があっても後続バッチを調べる。クラッシュ等で比較結果を作れなかったバッチは確認済み件数に加えない。
- 同じコマンドで再開する。完了バッチは不一致も含めて保持し、成功するまで自動的に再実行しない。未完了バッチには別attemptディレクトリーを作る。
- 再開前に入力・DLL・C#依存DLL・比較ツール・Windows版と保存証跡のハッシュを確認する。変化があれば拒否し、新しい出力フォルダーで実行する。OS辞書や学習内容を固定する仕組みではないので、長期間をまたぐ比較はその旨も記録する。
- `corpus.py` のcoverageは生成範囲、`exhaustive.py` のnext_offsetは列挙位置。いずれも照合成功の証明ではない。必ず比較のsummaryと一緒に読む。

検証ツール自体の回帰試験は `python -m unittest discover -s tools/parity -p test_batch_compare.py -v`。
