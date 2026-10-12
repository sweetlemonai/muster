# Muster: commands (draft, 2026-10-12)

**Name:** Muster (owner, 2026-10-12). It replaces the working names "tasky" and "the task queue".
chit (`~/playground/chit`) is the earlier CLI and spec it grows from. A single task can still be
called a ticket.

**Slash commands:** `/muster`, with `/mu` as a short alias. Both are one router, like `/pr` in
`~/playground/.claude/commands/pr.md`. The first word picks the action. A task number shows that
task. Anything else is captured as a new idea. Each command is a thin wrapper over the `muster`
script (today `bin/task`), which enforces the rules, so they don't live only in a prompt.

## For the owner

| Command | What it does |
|---|---|
| `/mu` | What's waiting for you, then a one-screen view of the queue |
| `/mu <any text>` | Capture an idea or bug: written up as a task, checked for duplicates, questions asked if unclear |
| `/mu 0008` | One task in plain words: status, who's on it, open questions, what's left |
| `/mu waiting` | Only what needs you, each with what it is, why, the options, the pick, and what happens with no answer |
| `/mu answer 0008 Q2 <text>` | Answer a question. It reaches the asking agent by itself |
| `/mu board` | Every task by status |
| `/mu report` | The as-built doc, with the manual test script and QA checklist (task 0017) |
| `/mu shot 0008` | Attach the image just pasted to a task |

## For agents doing the work

| Command | What it does |
|---|---|
| `/mu next` | Claim the top task this agent may take, and start |
| `/mu claim 0013` | Claim a specific task |
| `/mu ask 0013 <question>` | Ask without stopping. The answer is pushed back (answer hook) |
| `/mu block 0013 <on what>` | Park the task with what it waits on, and release the claim |
| `/mu done 0013 <commit>` | Close the task with its proof |
| `/mu found <text>` | Drop a new bug or idea in the inbox for capture |

## For running the system

| Command | What it does |
|---|---|
| `/mu capture` | Make this session the capture session (PM + tech lead): watch, answer, escalate |
| `/mu triage` | Turn inbox notes into tasks |
| `/mu check` | Find problems: duplicate ids, stale claims, broken `waiting_on` links |
| `/mu init <project>` | Start Muster in another project |
| `/mu dogfood <note>` | Log a rough edge in Muster itself (`DOGFOOD.md`) |

## Built today

The `muster` command (the app's binary, `bin/muster`): `new`, `list`, `show`, `move`, `set`, `note`,
`attach`, `answer`, `check`, `projects`, `init`. The slash commands will call it.

### Python tools (in `bin/`)

`task ask`, `task answer`, `task escalate`, `task inbox`, the answer hook, `manifest.py` (board
data + `--check`). Everything else above is still to build.
