---
covers: app/src/i18n/locales/en/pane.json, app/src/i18n/locales/fa/pane.json, app/src/i18n/locales/en/help.json, app/src/i18n/locales/fa/help.json, app/src/Markdown.tsx, app/src/blockDir.ts, app/src/shortcuts.ts, app/src/shortcutsPage.ts, app/src/manual.ts, app/src/Help.tsx
---
# The task pane's and Help's words, in every language

- The task pane, its forms, comments, review, vision and the Helper read every word from `pane.json`; Help's chrome, page titles and the Shortcuts page from `help.json`. (mu-148)
- `pane.json` loads with the pane's code and `help.json` with Help's (`withText`): neither is in the first screen's load, and neither shows English first. (mu-148)
- Words a startup screen shares stay where that screen loads them: the answer box and pick-one chips in `common.json` (Waiting for you shows them), the card's review and vision badges in `board.json`, the Problems list's vision line in `shell.json`. (mu-148)
- Task content shows as written: the write-up, comments, criteria, review and vision sections, the log and front-matter values; log lines stay English, they're the protocol. (mu-148)
- Every Markdown block (paragraph, heading, list item, quote, table cell) gets `dir="auto"` from the renderer, so a Persian paragraph reads right to left inside an English body and the other way round; code blocks are `dir="ltr"`. Content can never set `dir`. (mu-148)
- The manual's pages render the same way, so an English page reads left to right in the fa window until mu-154 translates it. (mu-148)
- The Shortcuts page is made from `shortcuts.ts` and `help.json` in the language shown; the English page is `docs/manual/06-shortcuts.md`, byte for byte. Keys stay as they are, isolated right to left. (mu-148)
- A long Persian label on a Next step button or a value chip truncates with the words in its `title`; it never wraps. (mu-148)
- Every guard-listed file has moved: `literals.test.ts`'s list is empty, and any English written into a screen fails the gate. (mu-148)
- Every Persian word follows `docs/i18n/glossary-fa.md`; mu-148's review is `docs/i18n/reviews/mu-148-fa-review.md`. (mu-148)
