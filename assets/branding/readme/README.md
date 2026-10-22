# Meltype-flash — README共通画像

以前の「M＋稲妻」ロゴを原本のまま使い、Windows・Linux・Mac・Android・ChromeOS・iOSのREADMEを揃えています。
黒・白・黄色を共通にし、6環境すべてのバナー・OSカード・本文見出しに雷を使います。ChromeOSのカードはリポジトリにあるChromeOS専用版を案内します。

- `logo.svg`: 共通の横長ロゴ。
- `<環境>/hero.svg`: 「Meltype-flash for Windows」など、正式な製品名と環境名入りの6環境のバナー。
- `cards/os-<環境>.svg`: リンクカード。`-active`付きは現在のページ用。
- `<環境>/headings/*.svg`: 「内容」「動く環境」「インストール」など、各READMEの雷付き見出し。日本語を含め文字をパス化しています。
- `preview.html`: バナー・見出し・カードを並べた確認用ページ。

SVGは白背景を持ち、GitHubの明暗テーマで同じ配色になります。PNG原本をSVG内に埋め込み、外部フォント・外部画像・JavaScriptには依存しません。アプリのアイコンは変更していません。

## 再生成

リポジトリのルートから、fonttoolsとNoto Sans JPを用意して実行します。

```sh
python tools/make-flash-readme-art.py --font /path/to/NotoSansJP.ttf --write-readmes
```

既定のフォントパスは `C:/Windows/Fonts/NotoSansJP-VF.ttf` です。
別の保存場所では `--font` または `MELTYPE_README_FONT` を指定します。
READMEの見出し文言や画像のaltを変更してから再生成すると、画像にも反映します。
生成済みのSVGを直接編集せず、配置・配色は生成スクリプトで変更してください。

文字は [Noto Sans JP](https://github.com/google/fonts/tree/main/ofl/notosansjp)（SIL Open Font License 1.1）を使用しています。ライセンスは `OFL-NotoSansJP.txt` に保存しています。
