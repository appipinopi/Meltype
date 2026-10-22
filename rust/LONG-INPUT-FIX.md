# 長文・連続入力で変換が中断する問題

2026-10-22、Windows の実変換器を使う分離プロファイルで再現した。
ライブ変換中に文節ごとの候補列挙と単独再変換を毎キー実行しており、
29キー目で859msかかった。TSFの要求期限は800msであり、期限を超えると
トランスポートがワーカーを終了し、保持していた未確定セッションが失われる。

入力中のライブ表示は主候補の文節変換だけを行い、候補展開と単独再変換は
明示的な変換時に行う。Windowsの補助候補取得は、文節ごとの500msではなく
1要求全体で100msの予算を共有する。診断用APIの500ms設定は維持する。
学習中のMozcワーカーは入力スレッドから待ち合わせず、完了だけを確認し、
未処理の学習を次の送信へ保持する。

## 検証

- 旧DLLで新しいABI回帰試験が候補列挙の検査に失敗し、修正DLLでは成功。
- 実MSIMEを使い、ローマ字350キーを待ち時間なしで送信。ライブ変換ON/OFFの
  両方で、途中の確定・セッション消失なしに変換、確定、次の入力まで成功。
  このPCでの最大要求時間はそれぞれ672ms、703ms（末尾の変換操作を含む）。
- native単体試験46件、clippy、ABI、Shift+Space 20ケース、文節伸縮20ケース、
  予測110ケース、実ワーカーの予測4ケース、TSFサーバー10件が成功。
- x64/x86実ワーカーでSystem/Mozc、750msの低速学習中の新規入力、
  学習完了の反映、秘密入力、エンジン障害時のフォールバックを検証。
- Windows配布物の9個のPEファイル、両アーキテクチャの専用IME ID、
  依存DLL、SHA-256、ZIP内容を検証。インストール後に32/64bitのCOM生成を確認。

```powershell
cargo test -p meltype-native --lib --locked
cargo clippy -p meltype-native --lib --locked -- -D warnings
python rust/native/tests/abi_smoke.py <meltype_native.dll>
python rust/native/tests/long_input_worker_smoke.py <meltype-tip-worker.exe>
```

これらは今回の入力列と実行環境の検証であり、任意の長さ・端末負荷での応答時間を
保証するものではない。インストール済みDLLは起動中のアプリに保持されるため、
既存アプリはMeltype-flashを選び直すか、そのアプリを再起動する。
