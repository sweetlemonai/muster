# 0012 — Muster reads only the personal Claude accounts, named in one allowlist

Date: 2026-10-14 · Status: accepted (the owner, on mu-99: Muster under `~/playground` uses only
the personal accounts cc-mg and cc-hg; `~/.claude` and a bare `claude` are the company account
and are never read).

## Why

Muster reads Claude Code's account folders in four places: the session states
(`claude agents --json --all`, 0048), `muster list --json`, the transcripts behind a task's
sessions and costs (`transcripts.rs`), and the helper's account picker. Before mu-99 each of them
took every `~/.claude*` folder, and a stamp with no account part meant the default `~/.claude`.
That folder is the company account: its sessions, transcripts and settings are work material and
have no place on the personal side.

A rule about folders alone doesn't hold on the owner's Mac. `~/.claude-mg/projects` and
`~/.claude-hg/projects` are links to `~/.claude/projects`, so all three accounts share one
transcripts folder, with personal projects (`-Users-<name>-playground-…`) and company ones
(`-Users-<name>-code-…`) side by side.

## Decision

1. **One allowlist of labels.** `claude_accounts` in Settings (`claudeAccounts` in
   `settings.json`), default `["mg", "hg"]`. A label is 1 to 12 lowercase letters or digits and
   names `~/.claude-<label>`. `""`, `.claude` and anything else invalid can't be entered, and are
   dropped when the file loads (stderr gets the count, never the values). An empty list reads no
   account.
2. **One place resolves it.** `accounts::allowed` turns each label into its folder, canonicalized;
   it must be a real directory and not `~/.claude` or anything inside it (so a
   `~/.claude-x → ~/.claude` link is refused). Every reader takes its accounts from there:
   `agents.rs`, `cli.rs`, `transcripts.rs`, `helper.rs`. Each run of `claude` gets
   `CLAUDE_CONFIG_DIR` set; there is no unset branch. A stamp with no account, or one not on the
   list, reads nothing: the card's state is unknown and the heartbeats judge.
3. **Transcripts are also filtered by project.** In an allowed account's `projects` folder, only a
   folder whose name is the key of a path under `~/playground` is read: `-Users-<name>-playground`,
   or that followed by `-` (`accounts::playground_key`, `is_under`). Project folders that are links
   are skipped. This is what keeps company transcripts out while the folders stay linked.
   **And by the stamp's account** (the owner's call in review round 1): a session is read only if
   the stamp that names it carries an allowed account. A stamp with no account (bare `claude`,
   the company account) gives no Work tokens, no live dot and no transcript, even when its
   transcript sits under a `~/playground` key. Stamps from before 0030, which have no account
   part, are treated the same: privacy over old history, so older tasks lose their token counts.
4. **Never opened:** `~/.claude.json`, `~/.claude/settings.json`, and anything else in `~/.claude`.
   The one exception is `helper::find_claude` looking for the `claude` program at
   `~/.claude/local/claude`: that's where Claude Code installs, not account data.
5. **Resolved on every read**, not cached per settings change. It costs one `canonicalize` per
   label, and an account folder made after the app started is picked up without a restart.

## Options considered

- *A path rule only, "never open anything under `~/.claude`":* simple, but on this Mac every
  account's `projects` resolves into `~/.claude/projects`, so it would cut off every personal
  transcript too.
- *Follow the account's folder only (read `~/.claude-mg/projects`, never `~/.claude/projects`):*
  the link takes it straight back to the shared folder, company transcripts included.
- *Un-link the account folders, so each has its own `projects`:* the cleanest data, but it changes
  the owner's setup, which Muster doesn't do.
- *Chosen:* the label allowlist for accounts, plus the playground-key filter on project folders.

## Known limits

- **The key is lossy.** Claude Code names a project folder by turning every character but ASCII
  letters and digits into `-`, so `~/playground-x` (key `-Users-<name>-playground-x`) also passes
  the filter. Accepted: such a folder is on the personal side too.
- The filter trusts the folder name and the stamp. A transcript filed under a playground key is
  read whatever it holds, when an allowed account's stamp names its session.
- Tasks worked before 0030's stamps show no sessions or tokens in Work.

## Revisit when

- The account folders stop sharing one `projects` folder: the project filter could then go, and
  the account rule alone would do.
- Claude Code records the account or the working folder in a way Muster can check directly, so the
  key match isn't needed.
- Muster is used by someone whose accounts aren't named `~/.claude-<label>`.
