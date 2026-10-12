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
| Commands `/muster`, `/mu` | `docs/COMMANDS.md` | Designed, not built |
| Desktop app | `app/` (Tauri 2 + React 19, decision 0001) | Built and running: board, waiting for you with answering, task pages, files, activity, problems |

## The app

`app/` is a Tauri 2 + React desktop app on Drover's stack (decision 0001), so it can move into
Drover later. Open it with `open app/src-tauri/target/release/bundle/macos/Muster.app` (build:
`cd app && pnpm install && pnpm release:build`; develop: `pnpm app`).

- **Queues:** every `~/playground/*/tasks` that looks like a Muster queue (today Drover's and
  Muster's), plus queues added by hand, with All or one at a time in the header.
- **Projects** (tab): every project with its folder, task count and where it came from (found
  in `~/playground`, or added). **New queue…** picks a project folder and creates its `tasks/`:
  status folders, README with the rules, `WAITING-FOR-OWNER.md`, the tools' stand-ins in `bin/`,
  and its own git with a first commit. It refuses inside a git repo, and refuses if `tasks/`
  already exists. **Add existing queue…** picks any queue folder. Rename, Hide/Show, and Remove
  (added ones only) never touch files. Saved in `~/Library/Application Support/ai.sweetlemon.muster/projects.json`.
  Folders are picked by the core's own dialog, so the window never hands the core a path.
- **Logo:** the owner's dog-face mark, recoloured mint to blue on a dark blue-black with thicker
  lines so it reads in the Dock and at 32 px (owner: "different color, slightly different, more
  visible"). The original and two other colourings (amber, bright violet) are in
  `muster/tasks/attachments/0006/`. It's the app icon and sits in the header.
- **Board:** Inbox / To do / Doing / Blocked / Done. Cards show the id, title, type, priority,
  "needs you", open questions, files, who's on it (flagged when quiet for 12 h), what a blocked
  task waits on, and when it last changed. Tasks waiting on the owner sort first. Done shows the
  latest 8.
- **Waiting for you:** escalated questions and owner-blocked tasks, each opening its task.
- **Task panel** (loads on first open): the task rendered as Markdown with a Raw switch,
  open questions with an answer box (⌘↩), the files as thumbnails that open large (← → between
  them, Esc to close), who's on it, what it waits on, and the commit.
- **Answering** writes file contract v1, so the asking agent gets it pushed by the answer hook.
  It's the only write the app makes.
- **Activity:** queue commits and changed task files, newest first. **Problems:** the same
  checks as `manifest.py --check`, done natively.
- **Live:** refreshes every 2 s while the window is visible, and never while it's hidden.
  On the real queues (21 tasks): 19 ms for the first read, 0.6 ms after that. Startup code is
  229 KB (72 KB gzipped). The task panel is a separate 102 KB chunk. The app is 5.2 MB.

## How the pieces talk

```
owner ──chat──▶ capture ──writes──▶ queue/todo/NNNN-*.md ◀──mv──  coder (claims)
                   ▲                         │  ## Questions  ◀── task ask (session recorded in mail/asks.json)
                   │ escalates (plain terms) │
owner ◀────────────┘ WAITING-FOR-OWNER.md    └─ task answer ─▶ mail/<session8>/NNNN-Qn.md ─▶ answer-hook ─▶ coder's context
```

- **Ids:** four digits, assigned by capture (`manifest.py` reports `next_id` and catches
  duplicates). A coder that finds something new drops a note in `inbox/`.
- **Locks:** `flock` on `mail/.lock` for every task-file write through the tools. Claiming is an
  atomic `mv`.
- **Time:** the tools stamp times. Agents' own timestamps aren't trusted (`DOGFOOD.md` 37).
- **Git:** each queue is its own git repo, outside every code repo. Capture commits named files
  (never `git add -A` on the queue while others write, `DOGFOOD.md` 31).

## Checked

- App: `pnpm check` green (typecheck, 7 UI tests, build, `cargo fmt`, clippy with warnings as
  errors, 15 Rust tests (queue reading, contract writing, projects: list, add, create, refuse
  inside a repo, settings round trip), plus `tests/interop.rs`: a question asked with `bin/task`, answered by
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

## Known limits

- **New session id after `/clear` or a handoff:** answers to the old session's questions wait in
  its old mailbox (`task inbox --session <old>`). Fix: carry open question ids in the handoff,
  and route by task claim too.
- **The hook only runs where it's installed:** Drover's four checkouts so far. New worktrees,
  and `link-personal-layer` rewriting `settings.local.json`, can drop it. Fix: put it in the
  personal layer (owner's call).
- **One capture session at a time** for ids, and capture's watcher lives in its chat session.
  The app now shows changes live, but it doesn't wake capture. Ids become collision-proof
  with task 0002 (`file` under the lock).
- **Files still move between status folders** (v2). The owner wants files that never move, with
  status only inside the file. The app already reads both layouts. The tools and the move
  itself are next (muster task 0005).
- **Rules not enforced by the tools yet:** claiming, one claim per session, blocking and done
  are still by hand (README rules). The planned `/mu` commands move them into the tool.

## What's left (in order)

1. **App, next round:** a look by the owner, then theme it like Drover (its `src/ui`), and add
   a menu-bar count of what's waiting, a notification when something new needs you, and
   moving tasks on the board for the owner (claims stay with agents).
2. **More of the commands** (`claim`, `next`, `block`, `done`, `found`, `triage`, `report`), and
   the `/muster` / `/mu` slash commands over them.
3. **Other projects:** one shared queue repo with a folder per project, or a queue next to each
   project (owner's call). The hook and rules go in through the personal layer. Pilot on one
   more project.
4. **Into Drover:** Drover task 0005 (task creator per project) and 0015 (waiting list) are the
   first pieces. The app's screens move into Drover when it's ready.
