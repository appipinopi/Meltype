# Meltype Rust 移植プレビュー

Fork: https://github.com/appipinopi/Meltype · 元: https://github.com/yksr-melt/Meltype

**全機能をRustに書き換えた完成版ではありません。** 元の実装を残したまま、OS共通のRustエンジンと4OSから接続する入口を追加しています。元のC#の機能・精度と同等とは保証できません。元のREADMEのダウンロードリンクは上流版です。

## ビルド・確認

```sh
cargo test --workspace --locked
cargo run -p meltype-cli -- --convert kyouhagoogledekensaku
# きょうはgoogleでけんさく
cargo build --workspace --release --locked
```

Rust 1.85以上。WindowsはMSVC C++ビルドツール、LinuxはCリンカー、macOSはXcode command line toolsが必要です。Cargo.lockを共有しています。

| crate | 内容 |
|---|---|
| meltype-core | ローマ字表222件、日英混在の辞書分割、保護対象、かな・カタカナ変換、候補、入力セッション、セッション内の学習、ユーザー辞書API、Mozcヘルパー通信 |
| meltype-native | C ABI、既存フロントエンド用JSON、TSF用のプロセス内接続、Android JNI |
| meltype-cli | 全デスクトップ共通の変換コマンド、JSON Linesセッション操作 |

CLIは引数なしでstdinからJSONを1行ずつ受け取ります。`{"op":"key","vk":0,"ch":107}`、`{"op":"key","vk":32,"ch":32}`、`{"op":"commit"}`、`{"op":"select","index":0}`、`{"op":"reset"}`、`{"op":"direct","enabled":true}`。結果は `consumed/commits/view` のJSONです。vkは既存実装と同じWindows仮想キー、chはUnicodeスカラー、modifiersはShift=1/Ctrl=2/Alt=4/Command=8です。

## OSとの接続

| OS | ビルド | 残っているOS固有コード・制限 |
|---|---|---|
| Windows x64/x86 | `./rust/build-windows.ps1 -WithTip` | C++ TSFシェル + Rust DLL。C#サーバー不要。出力は `native/tip/bin/rust-x64` と `rust-x86`。自動インストールしません。元と同じCOM登録IDなので、既存インストールと共存させる配布には未対応です。 |
| macOS | `bash rust/build-desktop.sh` | Swift InputMethodKitシェルとazooKey変換エンジンを再利用。出力 `mac/build/Meltype.app`。インストールしません。 |
| Linux | Mozcヘルパーを準備後 `bash rust/build-desktop.sh` | Python IBus / C++ fcitx5シェル、C++ Mozcヘルパーを再利用。出力 `linux/build/Meltype-linux`。 |
| Android | [android/README.md](../android/README.md) | Java InputMethodServiceとJNI。補助辞書による変換。 |

デスクトップの既存ビルド手順はそのままで、上記のRust用ビルドだけがRustエンジンを選びます。C APIは `native/include/meltype.h` を参照。戻り値はmalloc由来で `meltype_free` で解放し、同一セッションの呼び出しを直列化してください。Macの変換コールバックはmalloc由来の文字列を返します。LinuxのMozcヘルパーには上流と同じC/US/RSプロトコルで接続し、応答のタイムアウトとサイズ制限を設けています。

## 移植が残っているもの

- 元の文脈スコア・高度な日英判定・確定後の訂正・文節別の候補操作・かな入力・半角カタカナ・予測変換
- 翻訳、意味注釈、誤字指摘、スニペット、アプリ別プロファイル、コードのコメント/文字列の判定
- 設定・学習・ユーザー辞書のディスク保存、設定画面、トレイ、バックアップ、更新機構、配布用インストーラー
- C++/Swift/Python/JavaによるOS連携と外部変換エンジンをRustに置き換える作業
- 各OSでの実際の入力、フォーカス移動、パスワード欄、IME切替、候補表示、インストール・削除の実機検証

F8は現時点ではカタカナ表示です。コード入力モード・privateモードは保守的に直接入力にします。Windows/Androidで漢字変換する際は補助辞書だけでは語彙・精度が足りません。WindowsでもC APIからMozcヘルパーを登録可能ですが、Windowsパッケージの自動初期化は未実装です。

Rust previewワークフローは3OSのRustテスト・ライブラリ/CLIビルド、Windows TSFシェルのビルド、Android JNIライブラリとdebug APKのビルド・Lintを実行する設定です。CIの成功は実際の入力動作や全機能互換の証明ではありません。

## ライセンス

元コード・ローマ字表の著作権表記とGPL-3.0-or-laterを維持します。辞書の出典と個別ライセンスは元の各辞書ヘッダー、LICENSE、THIRD-PARTY-NOTICES.mdを参照してください。
