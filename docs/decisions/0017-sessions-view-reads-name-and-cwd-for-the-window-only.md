# 0017 — The Sessions view reads a session's name and cwd, for the owner's window only

Date: 2026-10-14 · Status: accepted (task mu-98; vision conflict V1, resolved by the shaper, and
the owner's ask for a Sessions view like Claude Code's agents list).

## Why

0048 reads `claude agents --json --all` for the cards' session states and keeps only each
session's short id and state. On purpose, it drops `name` and `cwd`: they can carry user names
(the privacy baseline). The Sessions view (mu-98) can't work that way. Its rows are named like the
agents list ("Asghar_6"), and it groups sessions by project, which only the session's working folder
tells.

## Decision

1. **Two readers, two shapes.** The cards keep `agents::Entry` (id and state) and 0048's rule
   unchanged. The view reads a second shape, `agents::ListEntry` (id, name, cwd, start, state word
   and `waitingFor`), which only `sessions.rs` uses. It goes through the same `Fetch`, argument
   array, `QUIET_ENV`, 10 s timeout and 1 MB cap. It reads only the accounts `accounts::allowed`
   returns (decision 0012), each with its own `CLAUDE_CONFIG_DIR`.
2. **`cwd` never leaves the core.** `sessions.rs` matches it against the projects Muster knows and
   their worktree folders, by whole path components, deepest first. Only the project's name crosses
   to the window, or "" (Other). The recap shows the checkout's folder name, never its path.
3. **Nothing is written.** Names, messages, recaps and first prompts live in memory: in the
   lister's latest list and in the transcript reader's bounded caches, which drop an entry when its
   transcript goes. Nothing goes to queue files, task logs, INDEX, the app's disk storage or logs.
   No network call is made.
4. **Only while the view asks.** `sessions::Lister` starts a read on its own thread when the open
   view polls and the last read is 15 s old, never two at once. While the view is closed nobody
   polls and nothing runs beyond 0048's own reads.
5. **Untrusted text stays text.** Names and messages are made one line (control characters and
   bidi overrides stripped, cut by characters) and are drawn as plain text with `dir="auto"`,
   never as Markdown or HTML. A handoff path found in a first prompt is read only if it's a regular
   file, not a link, directly in a `.claude/work/handoffs/` folder under `~/playground`, and at
   most 256 KB. Only front matter that closes is parsed.

## Options considered

- *Widen `Entry` for both readers:* one parse, but the cards would carry `name` and `cwd`
  through the monitor's state for nothing, and 0048's rule would quietly lapse.
- *Show ids only, no project grouping:* keeps 0048's rule everywhere, but it isn't what the owner
  asked for. Their screenshot is names, grouped, per project and all in one.
- *Send `cwd` to the window and match there:* less Rust, but paths with the user's name would
  cross the IPC boundary and sit in the webview's memory.
- *Chosen:* a second shape for the view only, with `cwd` turned into a project name in the core.

## Revisit when

- Drover 0018 replaces the `Fetch` with its own runner's list. The shape stays; the source changes.
- Claude Code adds a project field to `claude agents`, so `cwd` isn't needed.
- The owner wants needs-input sessions with no task to count in the needs-you number: that's a
  separate call, left out of mu-98.
