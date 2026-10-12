# Muster: as built (2026-10-12, updated with the app)

What exists, how it fits together, how it was checked, and what's left. Newest state, not
history (history is in `DOGFOOD.md` and the queue's git log).

## In one paragraph

A **queue** is a folder of Markdown task files, one per task, where the subfolder is the
status (`inbox/ todo/ doing/ blocked/ done/`). A **capture** session (PM + tech lead) writes the
owner's words up as tasks, answers technical questions, and escalates product questions to the
owner in plain terms. **Coder** sessions claim a task by moving its file, work in their repo,
log, and ask questions with `task ask` without stopping. The answer is written into the task and
pushed into the asking session by a Claude Code hook. A generated **manifest** (`tasks.json`)
gives one view of everything and checks the queue for problems. The owner reads
`WAITING-FOR-OWNER.md` (and, in Drover, the waiting list on Home).

## Parts

| Part | Where | State |
|---|---|---|
| Protocol v2.1 | `docs/PROTOCOL.md`; each queue's `README.md` | In use on Drover |
| File contract v1 (for tools that write answers without `bin/task`) | `docs/PROTOCOL.md`, "File contract" | In use: Drover's waiting list (Drover task 0015, `327af59`) writes against it |
| `task` CLI: ask / answer / escalate / inbox | `bin/task` | Built, tested |
| Answer push | `bin/answer-hook.py` | Live in Drover's checkouts (`drover/main`, `agent-controls`, `cleanup`, `docs-wt`), verified end to end on 2026-10-12 |
| Manifest + check | `bin/manifest.py` → `tasks.json` | Built, tested. Capture rebuilds it on every commit |
| Owner's waiting list | `WAITING-FOR-OWNER.md` per queue (by hand); Drover's Home (task 0015) | By hand here. Built in Drover |
| Attachments | `attachments/<id>/` per queue | In use (screenshots, mockups) |
| `muster` command line | the app's binary, `bin/muster` shim | Built: new, list, show, move, set, note, attach, answer, check, projects, init (same rules as the app) |
| Commands `/muster`, `/mu` | `docs/COMMANDS.md` | Designed, not built |
| Desktop app | `app/` (Tauri 2 + React 19, decision 0001) | Built and running: board, new task, task management, waiting for you with answering, files, activity, problems, projects, the AI helper |

## The app

`app/` is a Tauri 2 + React desktop app on Drover's stack (decision 0001), so it can move into
Drover later. Open it with `open app/src-tauri/target/release/bundle/macos/Muster.app` (build:
`cd app && pnpm install && pnpm release:build`; develop: `pnpm app`).

- **Projects sidebar** (left): All and every queue (`~/playground/*/tasks` plus ones added by
  hand), each with its task count, how many need you (gold) and how many are in progress
  (teal), and **+ Project** at the bottom. The header keeps only the views, New task and
  Appearance, on one line.
- **Projects** (tab, or **+ Project** next to the project chips): every project with its folder, task count and where it came from (found
  in `~/playground`, or added). **New queue…** picks a project folder and creates its `tasks/`:
  status folders, README with the rules, `WAITING-FOR-OWNER.md`, the tools' stand-ins in `bin/`,
  and its own git with a first commit. It refuses inside a git repo, and refuses if `tasks/`
  already exists. **Add existing queue…** picks any queue folder. Rename, Hide/Show, and Remove
  (added ones only) never touch files. Saved in `~/Library/Application Support/ai.sweetlemon.muster/projects.json`.
  Folders are picked by the core's own dialog, so the window never hands the core a path.
- **Themes and accents** (Appearance, docked): 7 themes (Violet night, Violet day, Graphite,
  Midnight, Paper, Snow, High contrast), with Match the Mac picking one dark and one light, and 6
  accents (Violet, Teal, Rose, Gold, Green, Graphite). No blue, no orange. They apply live and are
  remembered. Default: Muster's violet, following the Mac. A test checks every theme and accent
  pair: text 7:1, secondary text 4.5:1, accent-coloured words 4.5:1 (they use the strong shade),
  button text 4.5:1, fills 3:1.
- **Look** (Muster task 0010): a violet palette (accent `#8B6CF0` dark and `#6D4AE0` light, with
  violet-tinted backgrounds and surfaces), chosen because orange belongs to Drover and Braindump
  and blue to 127. Board status colours: inbox gray, to do violet, doing teal, blocked rose,
  needs you gold, done green, with no orange or blue. Contrast is checked to WCAG AA. Dark
  buttons use dark text on violet. In the light theme, status colours are fills behind dark
  text, with darker variants for coloured words.
- **Icon:** a bot face with a visor eye and a check, drawn by capture as a vector
  (`muster/tasks/attachments/0010/`). In use: **B**, the lineup's style (violet bot on near-black
  `#16131F`, eye and check cut out), like Drover's orange crook on navy. **A** (light face on a
  `#2E1F66`→`#170F38` gradient) is ready if the owner prefers it. `dock-preview-drover-a-drover-b.png`
  shows both next to Drover's. The earlier dog-face mark and its recolourings are in
  `attachments/0006/`.
- **Layout: nothing covers the board.** A task, New task and Appearance open in one pane docked
  beside the content, with the board still visible and live beside it (the owner: "need to have
  everything in front of my eyes"). Drag the edge to resize it; the width is remembered. Esc
  closes it.
- **Columns always fit:** an empty column becomes a slim strip with its name written vertically,
  so all five stay in view, even with a task open beside the board.
- **Filters:** above the board, by priority and by tag, with a "24 of 98 tasks" count.
- **Three views of a project:** **Board** (the kanban below), **Table** (every task as a row:
  project, id, title, type, priority, status, who's on it or what it waits on, captured, changed,
  tags; click a header to sort) and **Timeline** (each task as a bar from the day it was captured
  to the day it was done, or to today while open; the darker part starts the day it was claimed;
  a line marks today). All three share the priority and tag filters and open tasks in the docked
  pane.
- **Help** (tab): every `muster` command with usage, options and examples (click to copy), how to
  use Muster inside a Claude Code session (you, agents, capture), and the app's keys. It's the
  same list `muster help` prints (`help.rs`, with a test that fails if a command isn't documented).
- **Board:** Inbox / To do / Doing / Blocked / Done. Cards show the id, title, type, priority (on every card: HIGH filled gold, normal outlined, low faint), tags,
  "needs you", open questions, files, who's on it (flagged when quiet for 12 h), what a blocked
  task waits on, and when it last changed. Tasks waiting on the owner sort first. Done shows the
  latest 8.
- **Waiting for you:** escalated questions and owner-blocked tasks, each opening its task.
- **Task panel** (loads on first open): the task rendered as Markdown with a Raw switch,
  open questions with an answer box (⌘↩), the files as thumbnails that open large (← → between
  them, Esc to close), who's on it, what it waits on, and the commit.
- **New task** (`+ New task`, ⌘N, in the docked pane): project, type, priority, tags, a one-line title, a Markdown
  description, and files by paste, drop or picking. The id comes from the core under the queue's
  lock. The task lands in To do and opens.
- **Tags:** a `tags:` list in the front matter. Suggested: spike, chore, tech-debt, code, docs,
  design, research, test, perf, security, and any other word works. They're cleaned the same way
  everywhere (lowercase, hyphens, 24 characters, 8 at most). On cards, in the filter, in New task
  and on each task. `muster new --tags`, `muster set <id> tags`, `muster list --tag`.
- **Estimates:** `est-time` (45m, 2h, 1.5d; a day is 8 agent hours) and `est-tokens` (150k, 1.2M)
  on each task, cleaned the same way everywhere. On cards (`~2h · 500k`), in the table (sortable),
  in New task and on each task. The filter bar totals what's shown and still open ("≈ 10 d · 20.1M
  tokens left, 11 of 13 open tasks estimated"). The helper estimates every task it files with a size
  guide (S 30m/150k, M 2h/500k, L 1d/2M, XL: split). `muster new --time --tokens`, `muster set <id>
  time|tokens`. Actual time and tokens per task can come later from Drover (Drover task 0018).
- **Managing a task** (top of its panel): status (inbox, to do, blocked with what it waits on,
  done; moving a claimed task releases the claim; the app never claims), priority, type, rename,
  a note to the log, and files by paste, drop or **Attach…**. Every change is logged with the
  time and "owner".
- **Answering** writes file contract v1, so the asking agent gets it pushed by the answer hook.
- **What the app writes**, all through the core's rules, under the queue's lock, logged with the
  time and "owner": new tasks (and their ids), status moves, priority, type and title, log notes,
  attachments, answers to questions, and new queues (`New queue…`). Its own settings go in
  `projects.json`, and the helper's transcripts in its settings folder. **The app never claims a
  task.** Claims belong to the agents doing the work.
- **Activity:** queue commits and changed task files, newest first. **Problems:** the same
  checks as `manifest.py --check`, done natively.
- **Live:** refreshes every 2 s while the window is visible, and never while it's hidden.
  On the real queues (21 tasks): 19 ms for the first read, 0.6 ms after that. Startup code is
  229 KB (72 KB gzipped). The task panel is a separate 102 KB chunk. The app is 5.2 MB.

## The AI helper

The **Helper** tab: a chat per project that does what the capture session does. You write ideas,
bugs and tasks (and paste screenshots). It checks the queue for duplicates, reads the project to
understand the code, files one task per point with your words, details and acceptance criteria,
and asks in plain words when something's unclear, with its own pick. You see its replies and a
folded "steps" line, never file edits.

- It runs your own `claude` with the account you pick once (decision 0002), in restricted mode:
  only `muster` and read-only tools, file reading inside the project folder, and anything else
  refused. Router and token variables are stripped. Model: Sonnet.
- **Accounts:** the picker shows each `~/.claude*` folder's login (from `claude auth status`, no model
  call), with **Change account** any time. An account that only works through a shell command
  with a token (like `cc-hg`) isn't logged in on its own: the helper says so in plain words and
  offers another account or **Log in…** (Terminal on that account's own `claude auth login`).
- Conversations resume per project. **New conversation** starts fresh, and **Stop** ends a turn.
  The transcript lives in the app's settings folder (`helper/<project>.jsonl`).
- Checked end to end on a throwaway project: a two-item message filed both items (a high-priority
  bug and an idea) through `muster new`, for $0.05–0.07 a turn. Writes, deletes and reads outside the
  project were refused. The queue check was clean afterwards.

## The command line

The app's binary is also the `muster` command (`bin/muster` runs it). It uses the same Rust code as
the app (`ops.rs`), so the app and the command line can't disagree. It picks the queue from
`--queue <name|folder>`, then `$MUSTER_ROOT`, then the nearest queue from the current folder (a
queue, or a `tasks/` beside or above it).

`muster` is linked into `~/.local/bin`, so it works in any terminal and in any Claude Code
session as `! muster …`. `muster help` lists everything, and `muster help <command>` shows usage,
options and examples (`--json` for scripts).

```
muster new "Board flickers on resize" --type bug --priority high --body "Steps: …"
muster list [--status todo] [--json]      muster show 0007 [--json]
muster move 0007 blocked --on owner       muster set 0007 priority low
muster note 0007 "Checked on the Mac"     muster attach 0007 shot.png
muster answer 0007 Q2 "Use 5173"          muster check      muster projects      muster init ~/playground/x
```

`ask` and `inbox`, and the answer hook, are still the Python `bin/task` for now. They move to
`muster` next (task 0002), then the Python stand-ins point at `muster`.

## How the pieces talk

```
owner ──chat──▶ capture ──writes──▶ queue/todo/NNNN-*.md ◀──mv──  coder (claims)
                   ▲                         │  ## Questions  ◀── task ask (session recorded in mail/asks.json)
                   │ escalates (plain terms) │
owner ◀────────────┘ WAITING-FOR-OWNER.md    └─ task answer ─▶ mail/<session8>/NNNN-Qn.md ─▶ answer-hook ─▶ coder's context
```

- **Ids:** four digits, taken in order under the queue's lock by whoever creates the task: `muster
  new`, the app's New task, or the helper (through `muster new`). Taking them under the lock is
  what prevents two tasks getting the same id. The Python `bin/task` doesn't create tasks yet
  (task 0002 has it allocate the same way, or hand over to `muster`). `manifest.py` reports
  `next_id` and catches duplicates left from before. A coder that finds something new drops a
  note in `inbox/`.
- **Locks:** `flock` on `mail/.lock` for every task-file write through the tools (`muster`, the
  app, `bin/task`). Claiming is still an atomic `mv` from `todo/` to `doing/`, done by hand.
- **Time:** the tools stamp times. Agents' own timestamps aren't trusted (`DOGFOOD.md` 37).
- **Git:** each queue is its own git repo, outside every code repo. Capture commits named files
  (never `git add -A` on the queue while others write, `DOGFOOD.md` 31).

## Checked

- App: `pnpm check` green (typecheck, 7 UI tests, build, `cargo fmt`, clippy with warnings as
  errors, 26 Rust tests (queue reading, contract writing; projects: list, add, create, refuse
  inside a repo, settings round trip; task operations: create with ids and template, refusals,
  flat layout, moves and claim release, fields and notes, attachments that never overwrite or
  escape, base64; the command line), plus a second interop test (tasks the app makes pass
  `manifest.py --check` and work with `bin/task`), plus `tests/interop.rs`: a question asked with `bin/task`, answered by
  the app, is read back by `bin/task inbox`, refused as already answered, and passes
  `manifest.py --check`). The real queues were read by `examples/probe.rs`. The release app was
  built and launched. Its screens haven't been looked at by a person yet.

- `bin/test_task.py`: 15 tests (ask/answer/deliver, numbering, refusal of an already answered
  question, earlier questions listed, old unnumbered questions, sessions from env and text,
  refusals, control characters and tabs stripped, uppercase session ids, escalate, hook given bad
  input, the inbox command, atomic manifest).
- Live: a self-test question asked and answered through `task` appeared in the asking session's
  context after its next tool call (2026-10-12).
- Drover's own implementation of the file contract found three contract bugs within an hour, all
  fixed (`DOGFOOD.md` 36).
- Hook cost: about 10 ms per tool call with an empty mailbox.

## Decided: a queue next to each project

Each project's queue is `<project>/tasks`, beside its repo, with its own git history. There's no
central queue repo. **New queue…** and `muster init` create it there. The app finds every
`~/playground/*/tasks` (plus queues added by hand), and its **All** view gives the view across
projects that was the reason for a central repo. Chosen by the build and confirmed in review
(2026-10-12). It matches how every project under `~/playground` already lays out its repo.

## Known limits

- **A new queue needs its folder in the layer's sandbox list** (`~/playground/.claude/layer-settings.json`,
  `allowWrite`), then `link-personal-layer` re-run in that project's checkouts. Otherwise its agents
  can read tasks but not claim, log or ask (`DOGFOOD.md` 41). `muster init` and New queue don't do it
  yet (task 0004).

- **New session id after `/clear` or a handoff:** answers to the old session's questions wait in
  its old mailbox (`task inbox --session <old>`). Fix: carry open question ids in the handoff,
  and route by task claim too.
- **The hook only runs where it's installed:** Drover's four checkouts so far. New worktrees,
  and `link-personal-layer` rewriting `settings.local.json`, can drop it. Fix: put it in the
  personal layer (owner's call).
- **Capture's watcher lives in its chat session.** The app shows changes live, but nothing wakes
  a capture session when an agent asks something.
- **Files still move between status folders** (v2). The owner wants files that never move, with
  status only inside the file (task 0005). The app already reads both layouts. **0005 must ship
  with `muster claim` (task 0002), not before.** Today a claim is safe because the `mv` is atomic:
  of two coders moving the same file, only one succeeds. Once files stop moving, that guarantee
  is gone, and two agents editing the front matter at once could both believe they won. So
  `muster claim` reads the status and writes the claim under the queue's lock, and agents never
  edit the status themselves.
- **Rules not enforced by the tools yet:** claiming, one claim per session, heartbeats, blocking
  and done are still by hand (README rules), so agents type their own times and they can be
  wrong (`DOGFOOD.md` 40). Task 0002 moves them into `muster`.
- **No crash recovery yet.** A card flags a claim whose file hasn't changed in 12 hours, but
  nothing releases it. A crashed coder's task stays claimed until someone notices. Task 0002
  includes the lease (`muster heartbeat`, stamped by the tool) and reclaim (`muster reclaim`
  releases claims past their lease, with a log line, and the app offers it on stale cards).

## What's left (in order)

1. **App, next round:** a look by the owner, then theme it like Drover (its `src/ui`), and add
   a menu-bar count of what's waiting, a notification when something new needs you, and
   moving tasks on the board for the owner (claims stay with agents).
2. **Claims in the tool, then files that never move:** task 0002 (`claim` and `next` under the
   lock, `heartbeat` and lease, `reclaim`, `block`, `done`, `found`, `triage`, and `ask`/`inbox`
   in `muster`), then task 0005 on top of it. Then the `/muster` / `/mu` slash commands (0003).
3. **Other projects:** the answer hook and the rules go in through the personal layer (task
   0004). Pilot on one more project.
4. **Orchestration:** Muster decides who does what, Drover starts and steers the agents (owner,
   2026-10-12; Muster 0009, Drover 0018).
5. **Into Drover:** Drover task 0005 (task creator per project) and 0015 (waiting list) are the
   first pieces. The app's screens move into Drover when it's ready.
