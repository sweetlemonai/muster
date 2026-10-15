---
covers: app/src/i18n/locales/en/board.json, app/src/i18n/locales/fa/board.json, app/src/i18n/locales/en/native.json, app/src/i18n/locales/fa/native.json, app/src-tauri/src/notify.rs, app/src-tauri/src/app_menu.rs, app/src/human.ts, app/src/YouRibbon.tsx
---
# The board's words, in every language

- The board, its views, the backlog and sessions read every word from `board.json`; words the sidebar shares come from `common.json`. (mu-147)
- `board.json` loads before the first render, with `common` and `shell`: no line shows blank or English first. (mu-147)
- The needs-you count has one key pair, `common.json` `needs.words` (and `needs.chip`), read only by `needsWords`/`needsVerb`; a second count phrase in any locale file fails `human.test.ts`. (mu-147)
- A screen-reader sentence is one key per case, never fragments glued in English order; the shown words are hidden from it. (mu-147)
- A count beside its sign stays together right to left: «+۲ دیگر», never "۲+". (mu-147)
- Titles, tags, holder and session names show as written, isolated with `dir="auto"` or `<bdi>`. (mu-147)
- The owner as holder is "you" everywhere: the card, the Table, the Graph and the Held by chip. (mu-147)
- A longer Persian word never wraps a chip or moves a control: it truncates with an ellipsis, the full text in `title`. (DESIGN.md "No layout jumps")
- Persian is never letter-spaced; one `:lang(fa)` rule covers every head and chip, as `docs/i18n/glossary-fa.md` says. (mu-147)
- Right to left the layout mirrors (card stripe, table heads); the Timeline and Graph keep their left-to-right drawing. (mu-147)
- Mac notifications and the app's menus follow the language in effect and change with it; "Muster" stays as it is. (mu-147)
- A notification's task title stays plain text, cut as before. (mu-147)
- Fixed English, on purpose: the core's own words (mu-153) and time stamps' Latin digits (mu-151). (mu-147)
- Every Persian word follows `docs/i18n/glossary-fa.md`; each area’s words get an independent review. (mu-146)
