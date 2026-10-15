# Fonts bundled with the app

## Vazirmatn-fa.woff2

Persian text in the window (mu-150). Loaded by `../../fonts.css`, only when a Persian glyph is on
screen (`unicode-range`); Latin keeps the system font.

- **Font:** Vazirmatn 33.003 by Saber Rastikerdar (The Vazirmatn Project Authors), the variable
  `Vazirmatn[wght].ttf` (weights 100 to 900), upstream commit
  `6e553e33489a8f9dfaccc76860a2e3f3c1e66de7` of https://github.com/rastikerdar/vazirmatn, sha256
  `696249a2c74b39ffdef55de4df2809c5b639d3ff80d618d8160a095d2fd49dca`. Taken from the owner's font
  collection (`~/playground/persianUtils/fonts/main/fonts/vazirmatn/`), not downloaded. The plain
  cut, not the FD one: digits are the app's choice, never the font's.
- **Licence:** SIL Open Font License 1.1, in `OFL.txt` beside it. No Reserved Font Name, so a
  subset may keep the name. It may be bundled in a sold app. The licence goes with every copy in
  three places: the font's own name table keeps the licence and its URL (name IDs 13 and 14,
  `--name-IDs` below); `tauri.conf.json` bundles `OFL.txt` as
  `Muster.app/Contents/Resources/fonts/Vazirmatn-OFL.txt`; and Settings › About names the font and
  its licence.
- **What's in it:** Arabic (U+0600–06FF, Persian digits ۰–۹ and ٫ ٬ among them), ZWNJ, ZWJ and the
  bidi marks (U+200C–200F), the presentation forms (U+FB50–FDFF, U+FE70–FEFF), and « » (U+00AB,
  U+00BB). Every OpenType feature (joining, ligatures, marks, kerning) and the weight axis kept;
  no Latin. 44,844 bytes (budget 90 KB).
- **size-adjust 104.8%** in `fonts.css`: Vazirmatn's alef is 0.6724 em tall and SF's capital H
  0.7046 em (SF's at optical sizes 12 to 20, read with fontTools from
  `/System/Library/Fonts/SFNS.ttf` at weight 400), and 0.7046 / 0.6724 = 1.048.

To cut it again, from the fonts repo with its own fontTools (4.66.1, brotli 1.2.0, pinned in its
`uv.lock`), writing into this folder:

```sh
cd ~/playground/persianUtils/fonts/main
uv run pyftsubset 'fonts/vazirmatn/Vazirmatn[wght].ttf' --unicodes=U+0600-06FF,U+200C-200F,U+FB50-FDFF,U+FE70-FEFF,U+00AB,U+00BB '--layout-features=*' --name-IDs=0,1,2,3,4,5,6,13,14 --flavor=woff2 --output-file=<checkout>/app/src/assets/fonts/Vazirmatn-fa.woff2
```

The output is byte for byte the same each run, sha256
`f7541ac6832a6150b3fe4fb61c8bd3238adb5012268de9a3a3247133d5174a8c`; `fonts.test.ts` checks the
committed file against it. If the ranges change, change `unicode-range` in `fonts.css` to match
(`fonts.test.ts` holds the two together).
