---
covers: app/src/AccountMeter.tsx, app/src/accountsData.ts, app/src-tauri/src/profiles.rs, app/src-tauri/src/usage.rs, app/src-tauri/src/accounts_cmds.rs, app/src-tauri/src/bin/muster-statusline.rs, bin/statusline-usage
---
# Accounts and their usage

- The header shows each account's 5-hour and weekly use as two bars, the reserve marked, "resets 14:20" or "stale"; read, never guessed; clicking opens Settings › Accounts. (DESIGN.md "The account meter")
- The header wraps instead of clipping: the meter, mode and live move to a second line together. (mu-113)
