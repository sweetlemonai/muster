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
`attach`, `answer`, `check`, `projects`, `init`, and the stage commands (task 0014): `pass`
(to the next stage. `--from <stage>` refuses if it has moved on. `--gate` is needed out of
Building, and `--commit` goes with the pass into Done), `return` (back, with
the reason), `block --on` / `unblock` (a flag, the task keeps its stage), and `stage` (the owner's
override). Claims and their lease (task 0002): `claim` (Ready → Building, one claim per holder),
`next` (the top Ready task you may take, by priority then id), `heartbeat` (the holder keeps a
claim alive), and `reclaim [--after 12h]` (claims quiet past their lease go back to Ready). Questions
(task 0002): `ask`, `escalate` and `inbox`, beside `answer`. Levels (task 0016): `new --level
goal|feature --parent <id> --due YYYY-MM-DD` and `set <id> level|parent|due <value>` (an empty
parent or due clears it), with the rules checked: a task's parent is a feature or a goal, a
feature's a goal, a goal has none, no loops, and only a goal has a due date. `claim`, `next` and
`pass` never take a goal or a feature into Building. `check` reports a missing or wrong-level
parent and a loop, and warns (without failing) about a goal past its due date with tasks open. The
slash commands will call it.

### Python tools (in `bin/`)

`task ask|answer|escalate|inbox` (a stand-in that runs the `muster` commands with the same
arguments), the answer hook (reads the mailbox itself, for speed), `manifest.py` (board data +
`--check`, which reads `level`, `parent` and `due` into `tasks.json` and checks them as `muster
check` does, warnings in their own list). Still to build: `found` and `triage` (task 0015),
`depends:` (task 0019).
