# Muster — work log

Newest first. What changed, and how it was checked. The current state is in `AS_BUILT.md`.

## 2026-10-12 — Tag editor shows only the task's own tags
**Did:** the tag editor (task panel and New task) shows only the tags on the task, each with ×
to remove. The suggested tags complete as you type in "+ tag" (a native `datalist`, minus the
ones already on), and the box disables at the core's 8-tag cap. `app/src/Tags.tsx`, tests in
`app/src/Tags.test.ts`. Owner's note in task 0006.
**Verified:** `pnpm check` green: vitest 108 passed (7 new), cargo test 68 + 3 passed, clippy and
build clean.

## 2026-10-12 — Design rules for the whole app; tasks 0019–0024 and 0026 filed
**Decided:** the owner has ADHD, and out of sight is out of mind. Every screen shows its options
(no menus, popovers, short-choice dropdowns, or tabs for things used together), but keeps them out
of the way through weight, size and fixed placement, not by hiding them. Written as
`docs/DESIGN.md` and linked from the README.
**Did:** filed 0019 (dependency graph view and a `depends:` field), 0020 (edit a task's body,
comments in the panel), 0021 (task panel header redesign) and 0022 (Human board and AI board per
project), and 0023 (parallel sessions: a worktree per task, then merge, clean up, report and
take the next; split out of 0009), and 0024 (a blocked task says why: by tasks, by you, by a
permission, by a question, or by something outside), and 0026 (card looks to pick in Appearance). Added the owner's tags note (show only selected tags) and the design rule to 0006.
Brought 0021 and 0022 in line with the rule. Queue commits `eff688b`, `ce98eab`, `4fc7efa`,
`8d57a2c`, `5e5aebc`, `0774e29`, `7c8191d`, `af57e1c`.

## 2026-10-12 — Decided: `.muster/` in the repo, backlog, stages with roles, levels, points
**Decided:** the owner moved tasks into the repo's `.muster/` (committed or local per project),
picked the stage names (Backlog, Shaping, Ready, Building, Testing, Your check, Done) and the
levels (Goal, Feature, Task). Blocked is a flag on any stage, claims live in the shared git
folder, and actuals are stamped by the tool. Recorded in decision 0003.
**Did:** filed Muster tasks 0014 (stages), 0015 (backlog and shaping), 0016 (levels), 0017
(type, points, links, comments) and 0018 (`INDEX.md`). Rescoped 0002, 0004 and 0005. The build
order is 0014, 0002, 0005, then the rest. Queue commit `7d5d372`. Updated `AS_BUILT.md`.
**Verified:** `muster check` and `manifest.py --check` are clean.
**Open:** whether the sandbox lets a worktree write to the main checkout's git folder (0005
checks first). Drover's waiting-list reader has to follow `.muster/` (tell Drover's capture).
`PROTOCOL.md` is still Drover's v2 text and gets rewritten with 0005.

## 2026-10-12 — Handed over to its own session
**Did:** Muster gets its own session (builder + capture for `muster/tasks`). Drover's capture
session keeps `drover/tasks`. Handoff: `.claude/work/handoffs/2026-10-12-1421-muster-takeover.md`.
**Open:** 0002 (claims in the tool, lease, reclaim), then 0005 (files never move), 0004 (other
projects, layer sandbox + hook), 0003 (/muster, /mu), 0006 (app next round), 0009 (orchestration).

## 2026-10-12 — The app, the CLI, the helper, and the look
**Did:** desktop app (board, table, timeline, waiting list, activity, problems, projects sidebar,
docked task pane, New task, tags, estimates, themes and accents, Help), the `muster` CLI from the
same binary, the AI helper (owner's own `claude`, restricted), violet palette and the bot icon,
date-named inbox notes no longer read as tasks.
**Decided:** decisions 0001 (Drover's stack) and 0002 (helper restricted to `muster`); queues next
to each project; one Rust implementation of the rules.
**Verified:** `pnpm check` green (37 Rust, 74 UI tests, 2 interop tests with the Python tools);
headless WebKit screenshots in `muster/tasks/attachments/0010`, `0013`, `0014`; helper run end to
end on a throwaway project (filed via `muster new`, writes outside it refused).
**Fixed:** `muster list --tag` didn't filter (edit never applied) → applied, tested; ids 2027/2028
from a dated inbox note → `is_task_name` and `next_id` fixed, tested.

## 2026-10-11 — From a hand-made queue to Muster
**Did:** the queue protocol (v2), `bin/task` ask/answer/inbox, the answer hook (verified live),
`manifest.py`, the file contract v1; moved out of Drover's queue into this repo.
**Decided:** grow chit's ideas here instead of switching to chit (owner); name Muster (owner).
