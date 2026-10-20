# Meltype 1.3.0の参照比較

対象タグは `v1.3.0`、コミット `6310c44b728d5ad88284853f9ef37ea224993b18`。`python tools/parity/v130/regenerate.py` でタグのCore・辞書を一時フォルダーへ展開し、元ソースを変更せず.NET 10でビルドして応答を再生成する。既存のC#ソースや利用者の設定は変更しない。

`Reference.csproj` は元Coreのfriend assembly名を使って内部APIを呼ぶ比較用ツールであり、配布製品ではない。`SessionReference.cs` はWindowsのCompositionServiceと同じ設定デリゲートを接続し、OS辞書は制御したconverterへ置き換える。既定の共有セッションfactoryはPredictor/ShowTypedKeys等の接続がWindowsと異なるため使用しない。

- 472例: 数字、計算、元号、単位、季節、クリップボードの1行検査、キー設定、英字名、GitHubリポジトリ形式。
- 79入力列: 入力表示、候補一覧、候補クリック、確定、文節操作、設定ON/OFF、文脈付き学習の読込。実OS変換エンジンやネットワークは制御した応答。
- Rustの `upstream_v130_sessions` は、ヒント・選択番号・typed・committedの画面情報を比較から除く。全画面同等とは呼ばない。
- `known-gaps.json` は途中表示の保持境界の既知差分。通過群から分けて未対応として保持する。

検査: `cargo +stable test -p meltype-core --test upstream_v130_values --test upstream_v130_sessions --test v130_learning --locked`

全機能の残りは [UPGRADE-1.3.0.md](../../../rust/UPGRADE-1.3.0.md) を参照。
