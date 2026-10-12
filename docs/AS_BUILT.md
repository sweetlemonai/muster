# Muster: as built (2026-10-12)

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
| Desktop app (the UI) | not yet | Planned: Tauri 2 + React/TS, same stack as Drover so it can move into Drover later |

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
  Fix: the app (or Drover) watches. Ids become collision-proof.
- **Rules not enforced by the tools yet:** claiming, one claim per session, blocking and done
  are still by hand (README rules). The planned `/mu` commands move them into the tool.

## What's left (in order)

1. **The desktop app**, to see what's happening: board by status, waiting for the owner, open
   questions, who's on what with heartbeats, recent activity, live. Tauri 2 + React/TS.
2. **More of the commands** (`claim`, `next`, `block`, `done`, `found`, `triage`, `report`), and
   the `/muster` / `/mu` slash commands over them.
3. **Other projects:** one shared queue repo with a folder per project, or a queue next to each
   project (owner's call). The hook and rules go in through the personal layer. Pilot on one
   more project.
4. **Into Drover:** Drover task 0005 (task creator per project) and 0015 (waiting list) are the
   first pieces. The app's screens move into Drover when it's ready.
