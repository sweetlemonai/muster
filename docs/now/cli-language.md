---
covers: app/src/i18n/locales/en/cli.json, app/src/i18n/locales/fa/cli.json, app/src-tauri/src/jalali.rs, app/src/i18n/fixtures/bidi-controls.json
---
# The command line's language

- `muster` prints in English unless `--lang fa` (any command) or `MUSTER_LANG=fa` says Persian; `--lang` takes en or fa only. (mu-152)
- `LANG` and `LC_*` are never read: agents inherit the owner's shell. Inside an agent session (`CLAUDE_CODE_SESSION_ID` set) `MUSTER_LANG` is ignored and only `--lang` counts; agents Muster starts never get `MUSTER_LANG` (`helper::STRIP_ENV`). (mu-152)
- Persian covers `help`, `help <command>`, list, show's footer, ready, inbox, check's own lines and the confirmations `cli.rs` prints after a write. Refusals and text built in other modules stay English (mu-153). (mu-152)
- Never translated: `--json`, task files, front matter, logs, `INDEX.md`, `tasks.json`, merge-log output, usage lines and examples, anything a script parses. Every read command's `--json` is the same bytes in both languages. (mu-152)
- In Persian, dates read Jalali (`23 مهر 1405 10:01`), but digits, keys, commands, paths, types, priorities and estimates stay Latin: people copy them into commands. (mu-152)
- A Persian line starts with a Persian word, so a terminal that does bidi picks right to left. No bidi control characters are printed: `cli.json` holds none (tested). (mu-152)
- Every bidi control in what a command prints, stdout and stderr, shows as `<U+202E>` in both languages, so a crafted title can't reorder a line. `--json` keeps the text raw. (mu-152)
- In VS Code's terminal (`TERM_PROGRAM=vscode`), a Persian run says once, on stderr, that VS Code can't show right-to-left text. Terminal and iTerm2 can. Findings: `docs/research/2026-10-15-terminal-bidi.md`. (mu-152)
- `help` in Persian comes from `cli.json`'s `help.*`. `en` mirrors `help.rs` (tested), and a help row without Persian fails a test. (mu-152)
