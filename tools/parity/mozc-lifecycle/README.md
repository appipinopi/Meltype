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

`filter-cases.json`の20ケースでは、元版Controllerの文節・文脈を制御し、実際の`Learn`から非同期の`ILearningConverter.Learn`へ渡った文節列を採取します。日本語の連続区間、英語、入力した英字そのもの、翻訳、日時・計算記号、読みの英数字、学習無効、プライベート欄、10 UTF-16単位の文脈切出しと文末境界を含みます。

```powershell
dotnet run --project tools/parity/mozc-lifecycle/MozcLifecycle.csproj -- --filter tools/parity/mozc-lifecycle/filter-cases.json > filter-reference.jsonl
python rust/native/tests/mozc_learning_reference.py filter-reference.jsonl
cargo test -p meltype-core --lib original_controller_learning_runs_and_context_match --locked
```

元版の結果とソース／ホストのSHA-256は`rust/core/tests/data/upstream-engine-learning.json`へ固定しています。これは選別・文脈の比較です。全キー操作から文節ごとのRaw／Translationsを組み立てる処理の一致を証明する試験ではありません。

## Windows worker

Coreの確定処理から通知を取り出し、ネイティブのセッションロックを解放してからworkerへ渡します。学習時に候補・変換キャッシュを破棄し、MozcのIPCだけをバックグラウンドへ移します。次のMozc問合せはその処理と直列化し、System／WinRTのCOMオブジェクトは専用STAから移動しません。終了時にも保留中の学習を回収し、正常なヘルパーへ`Q`を送ります。

`windows_worker_converter_smoke.py`は実際のフレーム通信で、学習通知・プライベート／無効設定・通常欄での再開、別候補確定後の先頭候補変更、キャッシュ再照会、750msかかる学習を確定応答で待たないこと、終了保存を確認します。学習後の候補変更は試験ヘルパーによる制御結果で、実Mozcの辞書品質・永続学習の証明ではありません。

残る確認は、文節ごとのRaw／Translations保持、全キー・編集・独自表・全候補源の組合せ、TSFで選択失効や書込み失敗が起きた後の学習との整合、各OSの学習コールバック、実Mozcの配布と保存データを再起動して使う実動検証です。限定された比較だけで全Core移行完了とは扱いません。

## ローカル検証（2026-10-19）

ワークスペース175テスト、元版Controllerの20ケース、native単体のx86 41テストとRust 1.99の全体／TSF-server Clippyが成功。正常／保存拒否／終了無応答の元版との全通信比較はx64／x86のRust実行ファイルで成功。学習を止める設定、別候補の学習後の再問合せと750msの遅い学習、終了保存・学習中のSystem設定への切替・学習通信中にヘルパーが終了したときの復帰は両ABIの最終Release workerで確認済み。既存ABIと選択再変換の回帰も成功。

結果はローカルの`dist/mozc-learning-*.log`へ保存。元版通信の期待値をRustから作ることはなく、同じ試験ヘルパーを元版とRustから独立に起動して比較する。今回の変更の速度ベンチマークはまだ行っていない。
