# 画面の検証範囲

`capture_settings.py <visual-test付き実行ファイル> <出力フォルダー>` は利用中のデータから隔離した6プロファイルで、設定・辞書・定型文・学習・バックアップ・入力テストを描画します。設定の保存やSQLiteへの移行を行わなかったことも確認します。Linuxでは `xvfb-run -a` が必要です。

Windowsの記録は [evidence/windows-fst-sqlite](evidence/windows-fst-sqlite) にあります。これは元版と全画面が一致したという証拠ではありません。元版 `src/Meltype/UI/UserDictionaryForm.cs` は560×520、`SnippetsForm.cs` は620×580の独立ウィンドウですが、Rustは740×740の共通ウィンドウ内のページです。設定項目名・説明・列挙名の一致、画面レイアウトの一致、実ファイル選択/キーボード操作は別々に確認します。

Windows候補は `tsf_candidate_smoke.py` が元のC++描画器との寸法・ピクセル・操作を、`tsf_candidate_view_smoke.py` が実COM ABIで文節/候補の投影と位置を検査します。全設定や全OS上の全画面の一致をこれらだけで完了にしません。
