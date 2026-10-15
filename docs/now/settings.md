---
covers: app/src/Settings, app/src/settingsData.ts, app/src-tauri/src/settings.rs, app/src-tauri/src/projects.rs
---
# Settings

- Settings is a view in the header's row, never a window. (mu-36)
- Its groups: Appearance, Display, Projects, Agents, Accounts, Notifications, About. (mu-113)
- Language (English · فارسی) sits above the groups, whatever group is open; unset, it follows macOS's language for Muster. (mu-146)
- Under it, always shown: Digits (۱۲۳ · 123) and Calendar (Jalali · Gregorian), each following the language until picked. (mu-151)
- Picking the language's own digits or calendar saves it empty, so it follows the language again. (mu-151)
- A pick switches the window and the menu bar at once and is saved as `lang` in `settings.json`; agents and task files stay English. (mu-146)
- The shell's text (sidebar, toolbar, filter bar, Settings) comes from the translation files; the guard fails on new English outside them. (mu-146)
- The filter bar's Type row says every type in the language shown, the older task, feature and idea too; only the core's types are offered, and an unknown one shows as written. (mu-194)
- Accounts edits each account's reserve, concurrency, roles, models, plan and paused; the company `~/.claude` can't be added; no automatic balancing across accounts. (decision 0013)
- Settings are per Mac, in `settings.json`, `projects.json` and `accounts.json`, with readings in `usage/`. (AS_BUILT "Settings")
- Paths under the home folder show as `~/…`. (mu-36)
- Agents sets the claim lease (12 h; 1 h to 7 d) and the LIVE window (5 min; 1 min to 1 h). (mu-36)
- A missing or bad setting reads as its default, field by field. (mu-36)
- Projects shows each gate and run command read-only; `muster projects set` sets them. (mu-77)
- Projects shows each local queue's history under its gate, with Restore, Keep deleted or Commit when something's off. (mu-109)
- A prefix is set in Projects and refused when bad or another project's. (mu-33)
- The Your check limit is set per project in Projects: 5, 1 to 50, or Off. (mu-50)
- New queue makes a local `.muster/` in the repo's main checkout. (mu-4)
- The window never hands the core a path; folders come from the core's dialog. (AS_BUILT "Projects")
- Display modes: ADHD (the default), Standard, Focus; ⌘. toggles Focus. (mu-38)
- 7 themes and 6 accents, no blue or orange, contrast tested. (AS_BUILT "Themes and accents")
- Settings › Agents › Roles edits `roles.toml` beside projects.json; a refused file says its line and key. (mu-112)
- A Roles row shows the role's title in the language shown, not its id; the open profile shows `--role <id>`. (mu-195)
