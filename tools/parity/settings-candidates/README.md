# 設定の組合せと候補源の互換試験

元版 `45e14f86558aa6ea34f2c2ecd038ba95ca8c3c46` の `src/Meltype.Core` を変更せず、Windows CompositionService と同じ CompositionOptions を組み立てる。外部変換器は固定応答を返す。利用者の実データには触れない。

- 68設定の値域を定義し、全2項目間の値の組合せ12,595通りを42構成で網羅。生成時に網羅数を検査する。
- 翻訳、意味表示、予測、Tab変換、ライブ変換、SeparateKeyRoles、ContinueAfterConversion は128構成を全列挙。
- 4判定レベル、スコア区切り、ローマ字/JISかな、学習の32構成も検査。計648入力シナリオ。
- 追加候補源は外部エンジンの成功/空応答、一般語・外来語・ブランド・語句、ユーザー辞書、plist、文脈、絵文字、変換/翻訳/予測学習、独自書き間違い、複数文節、数式/時刻/全角数字、英語/日本語/固有名詞追加辞書を検査する。
- 比較は各打鍵のキー消費、確定編集、候補全文と順序、文節、選択位置、意味・注釈、予測、入力キー。非変換中の選択番号とhint文言は比較しない。
- アプリ別設定は別ホスト settings-apps で384通り。実行名の大文字小文字、拡張子、適用プロファイル、独自種類、有効/無効、ゲーム判定を含む。

```powershell
dotnet build tools/parity/settings-candidates/SettingsCandidates.csproj
python tools/parity/settings-candidates/matrix.py dist/candidates-matrix
python tools/parity/settings-candidates/check.py --csharp tools/parity/settings-candidates/bin/Debug/net10.0/Meltype.Core.Tests.dll --dll target/release/meltype_native.dll --cases dist/candidates-matrix/cases.json --output dist/candidates-check
python tools/parity/settings-candidates/sources.py dist/candidate-sources.json
python tools/parity/settings-candidates/check.py --csharp tools/parity/settings-candidates/bin/Debug/net10.0/Meltype.Core.Tests.dll --dll target/release/meltype_native.dll --cases dist/candidate-sources.json --output dist/sources-check
python tools/parity/settings-candidates/sources.py --dictionaries dist/candidate-dictionaries.json
python tools/parity/settings-candidates/check.py --csharp tools/parity/settings-candidates/bin/Debug/net10.0/Meltype.Core.Tests.dll --dll target/release/meltype_native.dll --cases dist/candidate-dictionaries.json --output dist/dictionaries-check
python tools/parity/settings-candidates/replay.py target/release/meltype_native.dll
cargo test -p meltype-core --test upstream_settings_apps
```

`check.py --reference-only` はC#のみを採取する。再照合の `--reference <採取ディレクトリー>` はケースとC# DLLのハッシュが一致する場合だけ使用できる。`freeze.py --output <固定先> <成功記録...>` はRust比較が全成功したC#出力のみを期待値として固定する。

全68項目の直積、OSスペルサービスの全応答、実System/Mozc辞書の全語彙、全UI操作や全OS実動の一致を証明するものではない。固定変換器でエンジン結果の扱いを検査し、実変換器/ワーカーは既存の別試験で確認する。設定UI・更新・OS起動設定などはCore入力トレースだけでは実動を確認できない。
