# 選択範囲の再変換と元版の比較

基準コミットは `45e14f86558aa6ea34f2c2ecd038ba95ca8c3c46`。元版の実際の `CaptureGate` と `CompositionController` に選択範囲を持つ試験ホストを接続し、28操作の結果を採取しています。Rustから期待値を生成しません。

```powershell
dotnet run --project tools/parity/reconversion/Reconversion.csproj -p:CoreProject=<元版Meltype.Core.csprojの絶対パス> -- tools/parity/reconversion/cases.json
cargo test -p meltype-core --test reconversion --locked
python rust/native/tests/reconversion_smoke.py <target>/release/meltype_native.dll
```

固定結果は `rust/core/tests/data/upstream-reconversion.json`。ローマ字文章、日本語と英語の混在、空白、数字、URL保護、直接入力中の明示操作、日本語の読み指定、確定・取消・原文候補の選択・選択対象の無効化を含みます。試験変換器は読みの保持／括弧付加の2種類です。元版の非公開 `ConvertRomajiText` から再変換計画の結果も採取し、Coreの単体テストで照合しています。開始表示、文節、選択位置、変換状態、最終文書を比較し、ASCII文章では候補一覧も比較します。ライブ変換・学習・予測は無効です。

元版は各語の日本語区間をまとめて単独変換に渡し、英語区間の前後に空白を入れます。保留中の末尾子音は通常入力の確定と異なる扱いです。変換結果が原文と同じ場合、通常の再変換へ進み、読める英語のローマ字候補を優先します。

ネイティブ変換コールバックはセッションのロック外で呼びます。プライベート入力・無効設定・入力中・無効セッションでは変換器を呼ばず、編集もしないことを検査します。返された `selectionExpect` と実選択範囲の照合はホストの責任です。選択変更ケースでは、Rust側の試験ホストが失効時に `meltype_reset` を呼び、古い選択への確定を防ぎます。この試験は実TSFホストによる自動検知や他アプリでの範囲照合を証明しません。

実OSの読み取得、全候補源・意味注釈、独自ローマ字表との全組合せ、画面レイアウト、実アプリへの配送は引き続き未検証です。単独変換を持つフロントエンドは既存の任意ABI `meltype_set_converter` の登録も必要です。
