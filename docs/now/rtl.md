---
covers: app/src/styles.css, app/src/rtl*, app/src/logicalCss*, app/src/ltrIslands*, app/src/splitter*, app/src/ui/Splitter.tsx
---
# Right to left in فارسی

- The layout mirrors: sidebar right, docked pane left, Backlog column at the right, Timeline from right to left, Graph arrows pointing left. (mu-149)
- Styles use logical sides only. A physical left or right in CSS or a TSX inline style fails `logicalCss.test.ts`. The exceptions carry a `physical:` comment: two inset accent shadows with `:dir(rtl)` twins, and hatch textures. (mu-149)
- Directional glyphs mirror from one block at the end of `styles.css`. A `→` or `◀` in markup is a `.dir-glyph` span. Key names don't mirror. (mu-149)
- Keys, commands, paths, branches, code and estimate inputs carry `.ltr` and read left to right inside Persian. (mu-149)
- The direction comes from `isRtl()` alone. No component reads `document.dir` or a computed `direction`. (mu-149)
- Arrows meaning next and previous follow the reading direction: in فارسی ← unfolds a heading and → folds it. Arrows on a splitter stay physical. A dragged splitter follows the pointer and stays within its bounds. (mu-149)
- Each toolbar count sits 6 px from its label in both languages, whichever count comes first. (mu-149)
