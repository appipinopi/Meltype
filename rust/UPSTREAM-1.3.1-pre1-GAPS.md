# Meltype 1.2.1〜1.3.1-pre.1のRust版未対応・未確認一覧

確認日: 2026-10-20。対象は元版 [v1.3.1-pre.1](https://github.com/yksr-melt/Meltype/releases/tag/v1.3.1-pre.1)、コミット `ecaf80f6208b7cb75a480fcee2b5296465ff5dfe`。Rustの従来の実API比較基準は `45e14f86558aa6ea34f2c2ecd038ba95ca8c3c46` のまま。今回の一覧作成は最新版の取り込み・完全互換の確認ではない。

1.3.0には1.2.2-pre.1/2までの変更が含まれるため、[1.3.0の更新内容](https://github.com/yksr-melt/Meltype/releases/tag/v1.3.0)、1.3.1-pre.1の更新内容、両基準間のCore・UI・OS連携のソース差分を参照した。追加設定の定義とRust既定値・実装参照の照合は [inventory.json](../tools/parity/pre1/inventory.json)。未知のJSON項目が保存されることと、その機能が働くことは区別する。

## 対象の5リリースと版ごとの不足

5つのリリースノートとタグのソースを個別に確認した。比較基準45e14f8は2026-10-14の開発コミットで、版番号は1.2.1だが、1.2.2-pre.1の公開（10月13日）より後にある。版番号だけで対応済みとは判断しない。

| 対象版 | 主な未対応・一部対応・要実動確認 |
|---|---|
| [1.2.1](https://github.com/yksr-melt/Meltype/releases/tag/v1.2.1) | Meltype IME導入の初回案内・利用不可時のキーボード方式への復帰、Windowsキーボード方式の製品接続、専用の誤判定報告画面、更新前の変換差の案内、winget/正式インストーラー連携。モード表示の色は保存されるが元版相当の専用色選択UIはない。最近の学習削除はRustに1時間の操作があり、元版の「今日」も含む同じ選択UIはない。LINE/Affinity/遠隔入力等のアプリ別動作は実アプリ確認が必要。定型文・Tab変換・F8・Ctrl+H・日時/記号候補等は既存の部分検証があり、未実装とは数えない |
| [1.2.2-pre.1](https://github.com/yksr-melt/Meltype/releases/tag/v1.2.2-pre.1) | IME利用不可時のモード項目の無効化・設定からのIME切替、色選択UI、報告/更新の修正一式、Mac/Linuxの同等な導入処理。予測一覧・英字読み・文節伸縮・設定の競合保存・バックアップなどは部分実装と回帰試験があるが、元版と同じ画面/実操作は未受入。37件修正のまとめを丸ごと対応済みとはしない |
| [1.2.2-pre.2](https://github.com/yksr-melt/Meltype/releases/tag/v1.2.2-pre.2) | 下表の追加設定27項目が未対応。数字/記号の学習、入力候補のクリック、ローマ字・Enter/Esc・Shift/フォーカス・破損JSONなどの最新動作も再比較が必要 |
| [1.3.0](https://github.com/yksr-melt/Meltype/releases/tag/v1.3.0) | FullWidthSpaceByDefaultが追加。AutoSwitch/Bothの廃止と移行、数字/元号/単位/季節候補、予測Delete削除、辞書の追従、キーボードだけで動くポータブル配布、Linux ARM64/apt・Mac Homebrew、最新のTSF/IBus/fcitx5・安全性修正の実動確認が残る。1.2.2-pre.1/2の内容を含む |
| [1.3.1-pre.1](https://github.com/yksr-melt/Meltype/releases/tag/v1.3.1-pre.1) | Zenzai関連の3設定とサーバー/モデル連携・Mozc文節への割当、Mozc-UT任意ビルド、学習保存/予測探索の新しい最適化、文頭スラッシュ修正、macOSアイコン・rpm/dnf・Homebrew再ビルドの配布修正。辞書メモリはRustが独自のFST方式なので、元版のPackedLines移植の有無とは別に評価する |

上表は製品機能・配布・安全性の対応状況。サイト・寄付者一覧・bot等の運営変更はRust IMEの未対応機能には数えない。全OS実動の未確認を、機能が必ず欠けているという意味では使わない。

## 未対応の追加設定（31項目）

以下のキーは元版のSettingsに追加されているが、現在のRust既定値・本番Rustソースに対応キーがなく、設定画面にもない。各キーで機能を切り替えられないことを確認した一覧であり、似た既存動作がすべて欠けているという意味ではない。

| 設定キー | 元版の項目名 | 最初に含まれる対象版 |
|---|---|---|
| `ShiftSpaceHalfWidth` | Shift + Space で半角スペース | v1.2.2-pre.2 |
| `EnterCommitsAsShown` | Enter は見えているとおりに確定 | v1.2.2-pre.2 |
| `AlphanumericKeyStartsAsTyped` | F9 / F10 の 1 回目は打ったまま | v1.2.2-pre.2 |
| `FullWidthParentheses` | カッコを全角で入力 | v1.2.2-pre.2 |
| `FullWidthSlash` | / を全角のスラッシュ／にする | v1.2.2-pre.2 |
| `SquareBrackets` | [ ] を角括弧［ ］にする | v1.2.2-pre.2 |
| `EscStepsBack` | Esc で一段ずつ戻す | v1.2.2-pre.2 |
| `EscRevertsSelectedClause` | 変換中の Esc は選んだ文節だけをかなに戻す | v1.2.2-pre.2 |
| `FunctionKeysAffectSelectedClause` | 変換中の F6〜F10 は選んだ文節だけに効かせる | v1.2.2-pre.2 |
| `CompatibleShortcuts` | Ctrl + K / L で文節の長さを変える | v1.2.2-pre.2 |
| `PartialCommit` | Shift + Enter で選んだ文節までを確定 | v1.2.2-pre.2 |
| `MacControlKeys` | Control キーを macOS 標準の日本語入力と同じにする | v1.2.2-pre.2 |
| `CursorIntoBrackets` | カッコを打ったら中にカーソルを移動 | v1.2.2-pre.2 |
| `ControlKeyBindings` | Control キーの割り当て | v1.2.2-pre.2 |
| `CalculatorCandidates` | 計算式を変換すると結果を候補に出す | v1.2.2-pre.2 |
| `LogKeyIntervals` | 打鍵の間隔を判定の記録に残す (実験) | v1.2.2-pre.2 |
| `LaughterAfterKana` | かな + w を笑い (ねw・さw) にする | v1.2.2-pre.2 |
| `ContextualLearning` | 英語 / 日本語の学習を前の文脈ごとに分ける | v1.2.2-pre.2 |
| `FullWidthSpaceByDefault` | スペースを全角で入力 (Shift + Space で半角) | v1.2.2-pre.2 |
| `ShowCandidatesWhileTyping` | 入力中も変換候補をいくつか表示 | v1.2.2-pre.2 |
| `ToggleKeyboardShortcuts` | 日本語 / 英数を切り替えるキーを追加 | v1.2.2-pre.2 |
| `GitHubIssueCandidates` | #番号で GitHub の Issue・PR のタイトルを候補に出す | v1.2.2-pre.2 |
| `GitHubRepository` | GitHub のリポジトリ | v1.2.2-pre.2 |
| `ClipboardCandidates` | クリップボードの文字を変換候補に出す | v1.2.2-pre.2 |
| `PauseHotkey` | Meltype の一時停止 / 再開のホットキー | v1.2.2-pre.2 |
| `UseZenzai` | Zenzai を使う (Windows・実験) | v1.3.1-pre.1 |
| `ZenzaiServerPath` | Zenzai: llama-server の場所 | v1.3.1-pre.1 |
| `ZenzaiModelPath` | Zenzai: モデル (GGUF) の場所 | v1.3.1-pre.1 |
| `EnglishNamePredictions` | 予測変換に英字の社名・サービス名 | v1.3.0 |
| `EnglishFirstInput` | 英字を主に入力 (変換キー / ↓ で日本語に) | v1.2.2-pre.2 |
| `ConsonantAbbreviations` | 子音の略入力を候補に出す | v1.2.2-pre.2 |

`PauseHotkey` は既定の一時停止ホットキーと競合時の代替キーがRustにもあるが、任意の組合せを設定する部分が未対応。`ShowCandidatesWhileTyping` は既存の予測候補とは別の設定で、元版が追加した入力途中の変換候補表示との一致は未確認。既存のF6〜F10・Ctrl操作の一部があっても、上表の追加設定と同等とは扱わない。

## 設定以外の実装・データの不足

| 対象 | 現状と必要な作業 | 元版の根拠 |
|---|---|---|
| Zenzai一式 | llama-server/zenz起動・停止、応答期限、通常エンジンへの復帰、Mozc文節への割当が未実装 | [ZenzaiConverter.cs](https://github.com/yksr-melt/Meltype/blob/v1.3.1-pre.1/src/Meltype.Core/Composition/ZenzaiConverter.cs)、[PR #849](https://github.com/yksr-melt/Meltype/pull/849)、[PR #941](https://github.com/yksr-melt/Meltype/pull/941)、[PR #889](https://github.com/yksr-melt/Meltype/pull/889) |
| 数字→漢数字・大字・元号 | RustにNumberCandidates相当の生成器がない。既存の数字の半角/全角候補とは別 | [NumberCandidates.cs](https://github.com/yksr-melt/Meltype/blob/v1.3.1-pre.1/src/Meltype.Core/Composition/NumberCandidates.cs) |
| 元号→西暦、尺貫法→メートル法、月に応じた時候の挨拶 | Rustのtime_candidatesは日時候補を実装済みだが、EraToYear/TypedUnitCandidates/SeasonalGreetings相当はない | [CompositionController.cs](https://github.com/yksr-melt/Meltype/blob/v1.3.1-pre.1/src/Meltype.Core/Composition/CompositionController.cs) |
| 計算式の計算、GitHubタイトル候補、クリップボード候補 | 上表の設定・候補源とも未対応。既存math_candidatesは「かける」等を記号へ置換する処理で、式の評価ではない | 同CoreのCalculator.cs/GitHubIssueTitles.cs/CompositionController.cs |
| 予測のDeleteによる学習削除 | Rustの候補キー処理に元版相当のDelete削除分岐がない。管理画面からの学習削除とは別 | 同CoreのCompositionController.cs/Predictor.cs |
| 辞書の更新 | candidates・english-readable・english・japanese・phrases・propernounsの6ファイルが最新版と異なる。改行をLFに正規化して比較し、CRLF差を更新差として数えていない | inventory.jsonのchanged_dictionaries、元版dictionaries/ |
| Mozc-UTの任意ビルド | MELTYPE_UT_DICTIONARYを使う統合・ビルド・通知ファイルの仕組みが未取り込み。RustのMozc接続があることとは別 | [PR #962](https://github.com/yksr-melt/Meltype/pull/962) |
| 学習保存と予測探索の新しい最適化 | 元版のDeferredSaverや予測の新索引と同じ変更は未取り込み。RustのStoreは整合性のため再読込・ロック・保存を行い、予測はphrasesを走査する | [PR #647](https://github.com/yksr-melt/Meltype/pull/647)、Rust storage.rs/linguistics.rs |
| 元版の辞書メモリ最適化 | 元版のPackedLines/PackedWordSetと同じ実装ではない。Rustは独自のFST＋値データを実装済みで、機能欠落とは数えない。メモリ比較は測定条件ごとに必要 | [PR #947](https://github.com/yksr-melt/Meltype/pull/947)、rust/core/build.rs/system_data.rs |

## 最新版との差を再確認する必要がある動作

以下は元版で変更・修正されているが、Rustで対応済みと断定できる最新版の実API・実アプリ証跡がない。未確認と未実装を区別する。

- 動作モードAutoSwitchと入力方式Bothの廃止、および旧設定からの移行。Rustにはまだ両選択肢と旧移行規則がある。
- 文頭・コードのコメント/文字列での `/`、4〜6連続スラッシュ、末尾の大文字N、Enter/Escの最新挙動、ローマ字の追加綴り。旧基準の回帰成功では最新版の一致を保証しない。
- 最新の日英判定・誤変換修正（suityoku、exe、shatsu、IME、dlsite、funyaosu等）。辞書更新だけでなく判定ロジックも比較が必要。
- 予測候補のクリック、1文字の読み・英字読み、数字/記号の選択学習、文節伸縮後のエンジン候補。既存の部分実装を最新版の受入成功とはしない。
- Windowsの複数キーボードのShift、初回打鍵、private-window/パスワード判定、Win+Xメニュー、フォーカス移動、全角スペース、異なるDPIの候補位置、設定変更時のIME切替。
- 最新の設定移行、プロファイル書出し、破損JSONの退避、phrasesバックアップ、クリップボード復元、パイプ所有者・管理者プロセスなどのセキュリティ修正。現在のRust方式で同じ危険を防げるか個別に検証する。
- Windowsのキーボードだけで動くポータブル版、setup.exe、zipインストール・更新・削除の一式。今回のRust ZIPはファイル検査済みの開発用プレビューで、正式インストーラーではない。
- macOS入力ソースのアイコン・重複登録・モード表示・設定メニュー・辞書取り込み、Linux IBus/fcitx5の最新の確定/フォーカス処理、ARM64配布・apt/rpm/dnf・Homebrewの配布経路。Rust版の対象機能をOSごとに確認する必要がある。

## 今回の小さなUI対応と検証方針

判定テストの見出し、アプリ別設定の「有効」/プロセス名の例、説明欄と判定テストの上下順、4行固定の説明と全文ツールチップ、設定末尾の余白を修正した。管理ページのナビゲーションを上部へ置き、プロファイル欄を広く使えるようにした。元版の独立ウィンドウとRustの統合ページの差は残り、全画面のレイアウト一致は未達。

[完了条件から除外した総当たり](MIGRATION.md)は再開しない。Windowsの実用上の代表操作・境界値・設定間の相互作用を比較し、不一致を修正して回帰テストに残す。今回の実施範囲はUIの小修正・配布物点検・性能再測定と、この未対応一覧の作成。
