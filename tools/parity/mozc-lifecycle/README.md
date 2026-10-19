# Mozcの学習通知・保存・終了の比較

元版の基準は`45e14f86558aa6ea34f2c2ecd038ba95ca8c3c46`です。元の`MozcConverter.cs`と`CompositionController.cs`は変更せず、元プロジェクトを参照したC#ホストから実行します。

## 通信・保存

`cases.json`の13操作（空の文節列1件、学習要求12件）を、元版とRustの`Mozc`へ同じ順で渡します。タブ／改行／USの置換、文節間のRS、文脈、学習拒否`NG`、成功5回ごとの`S`を全通信ログで比較します。保存を拒否するヘルパーと、終了通知`Q`に応答しないヘルパーも別プロセスで比較します。正常時は`Q`でヘルパー自身の最終保存へ進み、応答しなければ元版と同じ1秒の猶予後に終了させます。失敗した通信・不正な応答では強制終了し、正常保存の経路へ戻しません。

```powershell
rustc --edition=2021 -O rust/native/tests/fixtures/windows_mozc.rs -o helper.exe
dotnet build tools/parity/mozc-lifecycle/MozcLifecycle.csproj
cargo build -p meltype-core --example mozc_lifecycle --locked
python rust/native/tests/mozc_lifecycle_smoke.py helper.exe tools/parity/mozc-lifecycle/bin/Debug/net10.0/MozcLifecycle.dll target/debug/examples/mozc_lifecycle.exe
```

終了を無視する試験では、元版の`Dispose`はKill直後に戻るため、C#比較ホストは共有読取りでログを開きます。製品の終了待機や保存処理は変更しません。試験ヘルパーは専用一時フォルダー内の記録だけを更新し、実Mozcの学習プロファイルには触れません。

## 学習対象の選別

`filter-cases.json`の22ケースでは、元版Controllerの文節・文脈を制御し、実際の`Learn`から非同期の`ILearningConverter.Learn`へ渡った文節列を採取します。日本語の連続区間、英語、入力した英字そのもの、翻訳、日時・計算記号、読みの英数字、学習無効、プライベート欄、10 UTF-16単位の文脈切出しと文末境界を含みます。

```powershell
dotnet run --project tools/parity/mozc-lifecycle/MozcLifecycle.csproj -- --filter tools/parity/mozc-lifecycle/filter-cases.json > filter-reference.jsonl
python rust/native/tests/mozc_learning_reference.py filter-reference.jsonl
cargo test -p meltype-core --lib original_controller_learning_runs_and_context_match --locked
```

追加した2ケースは、隣の文節のRawや翻訳候補を現在の文節の情報として扱わないことを確認します。元版の結果とソース／ホストのSHA-256は`rust/core/tests/data/upstream-engine-learning.json`へ固定しています。これは選別・文脈の比較です。Rawの生成は下記の別の照合でも確認し、全キー操作・全設定・全候補源の一致とは区別します。

## Windows worker

Coreの確定処理から通知を取り出し、ネイティブのセッションロックを解放してからworkerへ渡します。学習時に候補・変換キャッシュを破棄し、MozcのIPCだけをバックグラウンドへ移します。次のMozc問合せはその処理と直列化し、System／WinRTのCOMオブジェクトは専用STAから移動しません。終了時にも保留中の学習を回収し、正常なヘルパーへ`Q`を送ります。

`windows_worker_converter_smoke.py`は実際のフレーム通信で、学習通知・プライベート／無効設定・通常欄での再開、別候補確定後の先頭候補変更、キャッシュ再照会、750msかかる学習を確定応答で待たないこと、終了保存を確認します。学習後の候補変更は試験ヘルパーによる制御結果で、実Mozcの辞書品質・永続学習の証明ではありません。

残る確認は、Raw／Translations生成の全条件との照合、全キー・編集・独自表・全候補源の組合せ、TSFで選択失効や書込み失敗が起きた後の学習との整合、各OSの学習コールバック、実Mozcの配布と保存データを再起動して使う実動検証です。限定された比較だけで全Core移行完了とは扱いません。

## ローカル検証（2026-10-19）

ワークスペース175テスト、元版Controllerの20ケース、native単体のx86 41テストとRust 1.99の全体／TSF-server Clippyが成功。正常／保存拒否／終了無応答の元版との全通信比較はx64／x86のRust実行ファイルで成功。学習を止める設定、別候補の学習後の再問合せと750msの遅い学習、終了保存・学習中のSystem設定への切替・学習通信中にヘルパーが終了したときの復帰は両ABIの最終Release workerで確認済み。既存ABIと選択再変換の回帰も成功。

結果はローカルの`dist/mozc-learning-*.log`へ保存。元版通信の期待値をRustから作ることはなく、同じ試験ヘルパーを元版とRustから独立に起動して比較する。今回の変更の速度ベンチマークはまだ行っていない。

## 文節ごとの元入力と翻訳情報

`Clause`はRawとTranslationsを個別に保持します。通常の文節生成では、保持している音単位と最終読みに対応する元入力を確認し、音単位の途中で切った場合や対応を証明できない場合はRawを付けません。かな入力・選択範囲からの再変換では通常入力のRawを流用せず、文節伸縮後は古いRaw／Translationsを消します。翻訳を選んだ文節はエンジン学習から除外し、永続履歴にも翻訳として記録します。元の英字を選んだ文節を通常の変換履歴へ登録せず、複数文節を別々に記録した後で文全体を再び通常変換として登録する重複も避けます。文全体の翻訳候補一覧で隣の文節まで除外しません。

```powershell
dotnet run --project tools/parity/mozc-lifecycle/MozcLifecycle.csproj -- --raw tools/parity/mozc-lifecycle/raw-cases.json > raw-reference.jsonl
python rust/native/tests/mozc_learning_reference.py filter-reference.jsonl raw-reference.jsonl
```

元の`CompositionText.RawForReading`を、Balanced／Manualの36入力・全346 UTF-16範囲について採取した値は`rust/core/tests/data/upstream-clause-raw.json`です。Rustの文節読みは有効なUTF-8文字列なので、サロゲートペアの途中を切る12範囲は構成できません。元期待値は削除せず保存し、Rustで構成できる334範囲をすべて照合します。拗音の途中、促音、保留n、英語の前後、大文字、句読点、絵文字を含みます。この試験は元版の音単位との対応を比較するもので、同じ入力の全てのUI表示が一致したことを意味しません。

文節のない変換の日英学習と全文節編集経路、全レベル・独自ローマ字表・誤字修正・JISかな・編集履歴・全候補源の組合せ、元入力／全角英字候補を追加する全経路と候補順、TSF適用失敗後の学習の整合と実Mozcの永続学習は継続作業です。

今回の文節情報の変更後は、ワークスペース178テストとRust 1.99の全体／TSF-server Clippyが成功。最終Releaseのx64／x86でABI、元版の文節伸縮20入力列、選択再変換28操作、実フレーム通信の学習ワーカー試験も再実行して成功しました。結果は`dist/clause-metadata-shipping-{x64,x86}-*.log`に保存しています。

## 文節選択による日英学習

通常の文節にChangedを保持し、候補選択API・候補送りで操作した文節だけを変更済みにします。境界伸縮と再変換では初期化します。変更済みの日本語文節でRawと同じ英字を選ぶと英語として、英語文節で日本語候補を選ぶと日本語として学習します。候補からの選択はF6／F10の明示切替とは区別して、既存の回数学習へ渡します。

```powershell
dotnet run --project tools/parity/mozc-lifecycle/MozcLifecycle.csproj -- --language tools/parity/mozc-lifecycle/language-cases.json > clause-language-reference.jsonl
python rust/native/tests/mozc_learning_reference.py filter-reference.jsonl raw-reference.jsonl clause-language-reference.jsonl
```

変更していない元ControllerとLanguageMemoryを一時メモリで実行し、通常と同じ組込みローマ字辞書を接続した16ケースを`upstream-clause-language.json`へ採取します。選択の有無、2回必要な短語・日本語、大小文字、元入力不在、英語から日本語、対象外文字、学習無効・プライベートを比較します。Rustは学習方向・回数・有効状態を照合し、実際の文節選択から確定までと伸縮時の状態破棄も別に検査します。全候補生成経路、文節を持たない単一変換、言語判定表示、実アプリ内での入力と永続保存の全組合せはこの比較だけでは証明しません。

この日英学習の追加後は全180テストとRust 1.99の全体／TSF-server Clippyが成功。最終Releaseのx64／x86でABI・文節伸縮20入力列・選択再変換28操作・学習ワーカーも成功しました。ローカル結果は`dist/clause-language-*.log`に保存しています。

## 通常変換履歴と翻訳履歴の条件

元ControllerのLearnから、ConversionHistoryとTranslationHistoryへ実際に渡された結果を25ケースで採取します。選び直していない文節、英語文節、1 UTF-16単位の読み、ひらがな・カタカナ・元入力と同じ候補、日時・計算記号は通常の優先候補へ登録しません。翻訳候補の所属は別の条件で扱い、選び直し状態や読みの長さで選択回数を捨てません。元と同じく読みの長さはUTF-16単位で数えます。

```powershell
dotnet run --project tools/parity/mozc-lifecycle/MozcLifecycle.csproj -- --history tools/parity/mozc-lifecycle/history-cases.json > clause-history-reference.jsonl
python rust/native/tests/mozc_learning_reference.py filter-reference.jsonl raw-reference.jsonl clause-language-reference.jsonl clause-history-reference.jsonl
```

期待値は変更していない元Controller／ConversionHistory／TranslationHistoryから取得し、`upstream-clause-history.json`へ固定します。翻訳の回数、1文字の翻訳、変更していない候補、英語文節、日時と計算記号、学習無効・プライベート、サロゲートペアの読みを含みます。Rustでは通常の候補を選ばず確定した場合と、同じ先頭候補を明示的に選んでから確定した場合も分けて検査します。短い翻訳とUTF-16の読みは一時プロファイルへの保存・読込みでも検査します。

通常入力のライブ変換から通常履歴を自動登録する処理を除き、文節のない変換でも選び直し状態を通常履歴の条件へ適用します。これは履歴の選別の照合であり、文節のない変換の日英学習、全候補源・全文節編集経路・保存制限・設定画面の一致や、TSF適用失敗時の学習整合まで証明するものではありません。

今回の履歴修正後は全183テストとRust 1.99の全体／TSF-server Clippyが成功。最終Releaseのx64／x86でABI・文節伸縮20入力列・選択再変換28操作・学習ワーカーも再実行して成功しました。結果は`dist/clause-history-workspace-verified.log`と`dist/clause-history-shipping-{x64,x86}-*.log`へ保存しています。先行試験では古いカタカナ学習前提の回帰が1件失敗し、そのログも保持しています。通常候補の選択学習を検査する試験へ修正し、カタカナを通常履歴へ登録しない元版の期待値は比較データ側で維持しています。

## 単一変換と go の入力中候補

`Choice.cs`は元の公開`MeltypeSession.HandleKey`／`SelectCandidate`を使用し、文字入力→Space（またはgoの入力中候補）→候補選択→Enterを実行します。元版の組込み辞書・LanguageMemoryを接続したManualの16操作について、確定文字列と日英学習の方向・回数・有効状態を`upstream-single-choice.json`へ採取します。Rust側も公開のキー／候補選択入口を通し、通常の単一変換が文節リストを持たない経路であることを明示的に検査します。候補一覧の全順序は比較対象にしていません。

```powershell
dotnet run --project tools/parity/mozc-lifecycle/MozcLifecycle.csproj -- --choice tools/parity/mozc-lifecycle/choice-cases.json > single-choice-reference.jsonl
python rust/native/tests/mozc_learning_reference.py filter-reference.jsonl raw-reference.jsonl clause-language-reference.jsonl clause-history-reference.jsonl single-choice-reference.jsonl
```

hashi・go・sushiの元入力への選び直しと2回待ち、goの入力中候補、学習無効とプライベート欄を比較します。全大文字は自動のSpace処理とは分け、F6のあとSpaceで候補を開く操作として固定しています。表示字種を変えても変換用の区間が英語であれば、英字候補の選択は日英の訂正として覚えません。同じ英語区間で日本語候補を選んだ場合は日本語として記録します。Fキーで表示を変えてから変換した場合は、変換候補の学習へ同時に別の明示Fキー学習を混ぜません。

Rustは文節のない変換に同じ日英学習の選別を適用し、goの入力中候補へもRawを渡します。比較対象の16操作では、入力どおりの英字などをエンジンへ学習通知しないことも検査します。全レベル・各文脈・途中編集・全大文字の自動Space操作・全候補源の分類一致、Fキー後の全エンジン通知、TSF適用失敗時の学習整合、実アプリ上の操作は継続確認です。

今回の単一変換修正後は全184テストとRust 1.99の全体／TSF-server Clippyが成功。最終Releaseのx64／x86でABI・文節伸縮20入力列・選択再変換28操作・学習ワーカーも成功しました。結果は`dist/single-choice-workspace-first.log`と`dist/single-choice-shipping-{x64,x86}-*.log`に保存しています。元版で全大文字のSpace時に候補が開かない先行観測や、F6後の誤った英語学習を検出した先行失敗も保存し、上記の固定した操作と区別します。
