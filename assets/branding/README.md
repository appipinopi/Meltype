# Meltype-flash artwork

`meltype-flash-logo.png`（横長）と`meltype-flash-mark.png`（正方形）は、
2026-10-22に提供された透過PNGをそのまま保存した原本です。

- `readme-banner.svg`: 原本ロゴを白い背景に載せたREADME用バナー。
  SVG内にPNGを埋め込み、GitHubの明暗テーマに依存せず表示します。
- `meltype-flash.ico`: 原本マークの16/20/24/32/48/64/128/256pxを含むWindows用ICO。
- `meltype-flash-32.png`, `meltype-flash-256.png`: ICOと同じマークのPNG形式。
- `meltype-flash.tiff`: macOSのRust版バンドル用。

通知領域では同じマークを白地に表示し、小さな青／灰色の印で有効／無効を示します。
元のC#版の画像資産は参照用として保持し、Flash用のビルドからこちらを参照します。
