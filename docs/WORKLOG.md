# Muster — work log

Newest first. What changed, and how it was checked. The current state is in `AS_BUILT.md`.

## 2026-10-13 — Parallel workers: slash commands (0003), editing (0020), goals and features (0016)
**Did:** three tasks built at once by workers on their own branches (task/0003, task/0020, task/0016),
each reviewed and fixed on its branch, then merged into main one by one, with the gate after each merge:
- 0003: `/mu` and `/muster`, allowed only the commands they route.
- 0020 part 1: edit a task's write-up in the app and with `muster edit`.
- 0016: goals, features and tasks with roll-ups and grouping.
**Fixed (from reviews):**
- `/mu`'s blanket `Bash(muster:*)` would have let it run `muster attach` on any file.
- Unsaved edits could be lost on Esc or by switching cards, and `muster edit` stamped the open time
  and deleted the copy when the editor failed.
- A held task could become a feature, a goal showed Done with an empty feature, and grouping hid a
  matching parent.
**Verified:** after the last merge (0016), `pnpm check` exit 0 (276 UI, 160 Rust, 12 interop tests),
`python3 -I bin/test_task.py` 33 OK, `bin/test_commands.py` 10 OK, also under
`TZ=Pacific/Kiritimati`.
**Open:**
- With Group by on, backlog notes and quick add don't show. Filed as a backlog note.
- 0020's comments wait for 0017.
- `/mu` needs the owner's yes to go into the personal layer.

## 2026-10-13 — Backlog one-liners, shaped into tasks (task 0015)
**Did:**
- Any non-task `.md` in a queue's `inbox/` is a backlog note.
- `muster add` (and a quick-add field at the top of the Backlog column) writes one.
- `muster shape` prints a brief for the shaping agents: the note quoted as data, the roles, the docs
  to read, related tasks and how to file it.
- `muster file` makes a numbered task in Shaping and moves the note into its attachments.
- `muster drop` keeps the note in `inbox/dropped/` with the reason.
- `muster list` shows notes, and `list --json` is now `{tasks, backlog}`.
- The note pane has File as task, Drop and Copy shaping brief.
- Main files: `app/src-tauri/src/backlog.rs` (new), `cli.rs`, `ops.rs`, `app/src/{Backlog,NotePanel}.tsx`,
  `bin/manifest.py`.
**Fixed (from review):**
- The brief didn't mark the note as data, and the note could escape its fence.
- A `\` in a note's name made actions hit a different note.
- The related-task check matched on the note's own "added by" footer.
- add/file/drop weren't refused from another project's folder.
- Four test gaps.
- Copy didn't write the clipboard inside the click.
**Verified:** `pnpm check` exit 0 (243 UI, 134 Rust, 9 interop tests), `python3 -I bin/test_task.py` 30 OK,
also under `TZ=Pacific/Kiritimati`. Reviewed for correctness, security and tests, with one fix round re-reviewed.
**Open:** the shaping agents themselves run in Drover (0018), and a session shapes by hand until then.

## 2026-10-12 — The owner's criteria don't block Testing; the owner ticks them in Your check (task 0028)
**Fixed:** a criterion only the owner can verify ("— verify: owner") couldn't be ticked before
Your check, and Testing → Your check refused while any criterion was open, so such tasks were
stuck. Now `pass` from Testing skips the owner's (the stage line says how many wait), and Your
check → Done waits only on them. A criterion is the owner's when its last `verify:` starts with
"owner"; "command (test) and owner" still needs the agent's proof.
**Did:** `queue::criteria_items` (labels, wrapped lines, offsets; `criteria` derives from it);
`ops::tick_criterion` and `muster tick` / `untick` (owner's criteria only, under the lock,
logged); the task pane lists them in Your check with a tick each, and Accept waits for them;
`muster check` lists what waits on the owner's look. `PROTOCOL.md`, `COMMANDS.md`, help.
**Verified:** `pnpm check` green: vitest 114, cargo 82 + interop 5, clippy, build;
`test_task.py` OK. Reviewed (correctness, security); fixed: Done held on agents' criteria too,
stale `help pass`, `check` didn't list them.
**Open:** built in a worktree; merges into main once the session holding 0002 commits.
## 2026-10-12 — Card looks in Appearance (task 0026)
**Did:** Appearance has a Cards section: five looks (Stripe, the default, now 5px; Tint; Top bar,
a band naming the stage or priority; Dot; Outline), each with a live preview of a plain, a
needs-you and a blocked card, and "Colour shows: Stage | Priority". Saved with the theme, applied
as `data-cards` / `data-card-colour` on the page, CSS only. Blocked and needs you take over the
colour in every look. Edges are darkened or lightened per theme to 3:1 on the card (the old 3px
stripe was about 1.5:1 in Testing teal and Your-check gold on light themes). `themes.ts`,
`CardLook.tsx`, `Appearance.tsx`, `styles.css`, `App.tsx` (the card), `lib.ts` (`priorityName`,
so P0–P3 colour the way the board sorts them). Preview: `tasks/attachments/0026/`.
**Fixed (review):** `overflow: hidden` on the card would have squashed a full column instead of
scrolling (now `overflow: clip` and `flex: none`); hover coloured three sides in four looks;
the Tint wash put the project name and a bug's label under AA on dark themes (dark tint now 8%).
**Decided:** Stripe stays the default (continuity); the owner can pick another in Appearance.
**Verified:** `pnpm check` green: vitest 221 passed (82 new), cargo 72 + interop 5, clippy and
build clean. Two reviewers (correctness, tests); every finding fixed; mutations of `edgeColour`,
the tint, `apply()` and blocked-over-needs each fail the tests.
**Open:** the owner looks at the looks (AC1, AC2). Built in a worktree; merges into main once the
session holding 0002 commits its working tree.
## 2026-10-12 — Claims, lease and questions in the tool (task 0002)
**Did:**
- `muster claim` and `muster next` give one claim per holder (the owner is exempt), checked
  centrally for every way into Building. Each claim names the queue it was made in.
- `muster heartbeat` and the holder's notes, passes, questions and unblocks renew a 12 h lease.
  `muster reclaim` (and Release in the task pane) returns quiet claims to Ready, and `check`, the
  card and reclaim judge staleness the same way.
- A claim from a folder whose own queue differs from `MUSTER_ROOT` is refused unless `--queue`
  says so (the Drover 0022 mix-up).
- `muster ask`, `escalate` and `inbox` are in Rust. `bin/task` is a stand-in that runs them, and
  works on macOS's Python 3.9. The answer hook frames answers as information from the queue.
- Main files: `app/src-tauri/src/{ops,queue,cli,help,lib}.rs`, `bin/task`, `bin/answer-hook.py`,
  `app/src/{lib,TaskPanel}.ts(x)`.
**Fixed (from review):**
- Unblocking left an old heartbeat, so the claim was reclaimable at once.
- `bin/task` crashed on Python 3.9.
- Tests depended on the wall clock and would have failed from 02:06 tonight. They now take an
  explicit time, and were checked under three time zones.
- The end-to-end refusal and the exact 12 h boundary weren't tested.
- An early test wrote two questions into the real queue's 0001. Undone.
**Decided:** decision 0001's "stale" amended: by the tool's heartbeat, falling back to mtime.
**Verified:** `pnpm check` exit 0 (113 UI, 102 Rust, 8 interop tests), `python3 -I bin/test_task.py`
26 OK, and the Rust and Python suites also under `TZ=Pacific/Kiritimati`. Reviewed on four lenses
(correctness, concurrency, tests, security), with two fix rounds, each re-reviewed.
**Open:** nothing runs `muster reclaim` on a schedule yet. Claims still go in the task file until
0005 moves live state into `.git/muster/`.

## 2026-10-12 — One picture: tasks checked for conflicts, VISION.md, vision check queued
**Did:** read every open task (0002–0029) against decision 0003 and wrote `docs/VISION.md`: the
spine (each stage, who works it, which task builds it), where things live, and the open
questions. Linked from the README. Queue commit `620b4b3`.
**Fixed:** conflicts between tasks filed today. 0019 and 0024 kept two records of task-on-task
waits (blocked by tasks is now derived from `depends:`), and 0024 had a second CLI spelling
(`--by`, now the existing `--on`). 0023's `finish` skipped Testing (now: `pass` from Building
opens the PR, Testing is 0027's review plus QA, merge on `pass` from Testing, conflicts return to
the builder). `DESIGN.md` said every option shows, but the owner had said priority and type show
only their value (rule corrected). 0015's shaping now also sets `depends:`, `touches:` and `for:`.
**Open (owner's):** human tasks on the same stages vs their own columns (0022); Done after the
merge changes decision 0003's "Done rides in the PR"; one review of record (PR engine) vs a
second in-session review; the four parallel-session choices. Filed 0029 (vision check with a
cost-of-pivoting section) so this check runs on every new task.

## 2026-10-12 — Stages with roles (task 0014)
**Did:** tasks have a stage: Backlog, Shaping, Ready, Building, Testing, Your check, Done.
- `stage:` in the file is the truth; blocked is a flag that keeps the task in its stage; stage
  history comes from the tool's own log lines.
- New commands `muster pass` (with `--from`, `--gate`, `--commit`), `return`, `block`, `unblock` and
  `stage`. `list` groups by stage.
- The board has one column per stage. The task pane has the stage picker, Pass, Send back,
  Block/Unblock and Accept, running one action at a time.
- `status:` and the folder stay as a mirror until 0005.
- Main files: `app/src-tauri/src/{ops,queue,cli,help,lib,projects}.rs`, `app/src/{lib,TaskPanel,App,Views,viewData,themes}.ts(x)`,
  `bin/manifest.py`, `bin/task`.
**Fixed (from review):**
- A double-click on Pass could go Testing → Your check → Done. Fixed with an expected
  `from` stage and one action at a time.
- Hand moves weren't reported. `check` now reports a stage that disagrees with its status.
- Notes, titles, block reasons, a multi-line answer with its own `## Log`, and a `who` with a line
  break could each fake a stage line. Fixed with the exact stage-line check, readers that use the
  last `## Log`, indented question and answer lines (file contract rule 3), and `who` cleaned in
  Rust and Python (rule 2).
- Also four test gaps, stale names on Done cards, and stale forms.
**Decided:** the old status and folder stay as a mirror of the stage (Drover's tools read them).
The owner can't Pass a task an agent holds; the stage picker overrides.
**Verified:** `pnpm check` exit 0 (108 UI, 72 Rust, 5 interop tests), `python3 -I bin/test_task.py` 22 OK.
Deep review across 10 lenses: 6 should-fix and 4 nits, all fixed over 4 rounds, each re-reviewed;
the final re-review was clean. 0014 itself was passed Building → Testing → Your check with the new
commands, and `muster check` is clean.
**Open:**
- Not seen in a real window: the sandbox can't launch WebKit, so the owner's check covers the look.
- 0028: criteria only the owner can verify shouldn't block the pass into Your check.
- Drover must adopt contract rule 3 (note in Drover's inbox).
- `bin/task` still writes its "asked/answered" log lines at the end of the file.

## 2026-10-12 — Tag editor shows only the task's own tags
**Did:** the tag editor (task panel and New task) shows only the tags on the task, each with ×
to remove. The suggested tags complete as you type in "+ tag" (a native `datalist`, minus the
ones already on), and the box disables at the core's 8-tag cap. `app/src/Tags.tsx`, tests in
`app/src/Tags.test.ts`. Owner's note in task 0006.
**Verified:** `pnpm check` green: vitest 108 passed (7 new), cargo test 68 + 3 passed, clippy and
build clean.

## 2026-10-12 — Design rules for the whole app; tasks 0019–0024, 0026 and 0027 filed
**Decided:** the owner has ADHD, and out of sight is out of mind. Every screen shows its options
(no menus, popovers, short-choice dropdowns, or tabs for things used together), but keeps them out
of the way through weight, size and fixed placement, not by hiding them. Written as
`docs/DESIGN.md` and linked from the README.
**Did:** filed 0019 (dependency graph view and a `depends:` field), 0020 (edit a task's body,
comments in the panel), 0021 (task panel header redesign) and 0022 (Human board and AI board per
project), and 0023 (parallel sessions: a worktree per task, then merge, clean up, report and
take the next; split out of 0009), and 0024 (a blocked task says why: by tasks, by you, by a
permission, by a question, or by something outside), 0026 (card looks to pick in Appearance), and 0027 (agent PRs reviewed by the owner's
review engine, fixed by the agent, lessons kept for the next agents). Added the owner's tags note (show only selected tags) and the design rule to 0006.
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
