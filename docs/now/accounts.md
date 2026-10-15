---
covers: app/src/AccountMeter.tsx, app/src/accountsData.ts, app/src-tauri/src/profiles.rs, app/src-tauri/src/usage.rs, app/src-tauri/src/accounts_cmds.rs, app/src-tauri/src/ledger.rs, app/src-tauri/src/bin/muster-statusline.rs, bin/statusline-usage
---
# Accounts and their usage

- The header shows each account's 5-hour and weekly use as two bars, the reserve marked, "resets 14:20" or "stale"; read, never guessed; clicking opens Settings › Accounts. (DESIGN.md "The account meter")
- The header wraps instead of clipping: the meter, mode and live move to a second line together, with Help and Settings before them ([toolbar](toolbar.md)). (mu-202)
- The meter keeps its width as readings change: each account's status reserves its widest words in the language shown ("resets 23 Oct 14:20 · 7 d stale"), as hidden copies never read out, so the wrap never follows a reading; adding, removing or pausing an account may change it. (mu-216)
- The run ledger keeps one record per run in `ledger/YYYY-MM.jsonl`, tokens and time equal to the Work rows, OpenTelemetry GenAI names (schema pinned); no title, prompt, transcript line, path or token; each run once; 13 months. (mu-114)
- `muster ledger backfill` covers Done tasks' read sessions and adds nothing on a second run; `--dry-run` writes nothing. (mu-114)
