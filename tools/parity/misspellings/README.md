# 元版の書き間違い候補との比較

変更していない元の`MisspellingDictionary.Find`を実行し、正しい形への置換全文か、候補なしをJSONLへ採取します。元版のUTF-16範囲と`CompositionText.ToKatakana`で置換を組み立て、Rustの探索結果と比較します。

```powershell
dotnet run --project tools/parity/misspellings/Misspellings.csproj -- tools/parity/misspellings/cases.json > misspelling-reference.jsonl
python rust/native/tests/misspelling_reference.py misspelling-reference.jsonl
cargo test -p meltype-core --test upstream_misspellings --locked
```

同梱辞書の全ての誤記・正記を単独、ASCIIの前後、絵文字の前後に置き、長いURL・ASCII・結合文字・空入力に、濁点除去・促音の挿入・長音の除去／挿入・結合文字の並び・誤記との隣接を加えた2182入力です。元版が正常に返した2045入力は候補の全文と候補なしが一致します。137入力では元版の曖昧探索がサロゲートペアの途中を切り、FormD正規化で`ArgumentException`になります。これらは期待値から削除せず、例外名も固定します。Rustでは全2182入力を実行してパニックしないことを検査し、元版の137例外を正常応答の一致件数へ加えません。

固定期待値は`rust/core/tests/data/upstream-misspellings.json`で、元ソース・辞書・ホストのSHA-256と基準コミットを含みます。Windows CIでも元版の596結果を再採取して比較します。先行の例外終了を含むログはローカル`dist/misspellings-original-*.jsonl`へ保持しています。

RustのASCII探索省略は、読み込み済み辞書の誤記と正規化した形にASCIIだけで一致し得るキーがない場合に限ります。辞書内容から条件を導き、ASCIIの誤記や正規化キーが含まれる場合は通常の探索を使います。日本語を含む読みの探索順・誤記優先・正記内の抑止条件は変更していません。

この比較は同梱の書き間違い辞書だけを対象とし、ユーザー辞書の追加・画面・全候補源・全設定や入力メソッド全体の一致を証明しません。性能は[Windows共通Coreの固定測定](../../performance/windows/README.md)で別に確認します。

重なる部分文字列の正規化を、各文字の正規化結果の連結で再利用します。残る文字の結合クラスが全て0の場合だけこの経路を使い、それ以外は部分文字列全体を正規化します。元の長い候補を優先する順序と、正しい形に当たったときに短い候補を止める条件を維持します。最初の596入力は元の期待値が不変なことも採取時に確認しました。
