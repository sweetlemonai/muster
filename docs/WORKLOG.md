# Muster — work log



Newest first. What changed, and how it was checked. The current state is in `AS_BUILT.md`.

## 2026-10-13 — Decision 0004: one session per task, thin coordinator, handoff at ~150k
**Decided:** the owner said yes after seeing today's numbers (almost every token is context
re-read per turn; a 450k coordinator pays 450k per turn). Record:
`docs/decisions/0004-one-session-per-task-thin-coordinator.md`. 0009 and 0023 note it.

## 2026-10-13 — The four parts: idea → task → run → gate (proposal)
**Did:** the owner: "idea -> task -> execution -> gates -> done; execution is Drover, task is
Muster, idea is the third part and gates (QA, visual QA) the fourth". Mapped what exists (GenIdea
in `germ`, Drover, the gate tools in the personal layer, braindump and choopan) and wrote
`docs/PIPELINE.md`: what each part owns, the hand-offs between them, where each research finding
goes, and four choices for the owner, each with a pick.
**Open:** the owner's yes on PIPELINE.md's four choices, then decision records and tasks.

## 2026-10-13 — How Muster compares: OpenSpec, Spec Kit, Beads, Linear, agent boards
**Did:** the owner asked what OpenSpec and similar tools do. Deep research (7 web researchers, a
citation audit) over spec-driven tools, agent task files, trackers with agents, hosted agents
and parallel-worktree boards: `docs/research/2026-10-13-agent-task-tools.md`. Muster's own ground:
stage moves that need proof, answers pushed into the asking session, the vision check, cost per
task from local transcripts, no telemetry. Worth adopting: the tester isn't the builder, reading
Claude Code's agent states, stop and pick-one questions, "what's true now" pages, a clarify pass
in Shaping, a work-in-progress limit. Each went to the backlog as a note, not built.
**Fixed:** merging 0035/0037 onto 0022: the sidebar counted "needs you" without the owner's own
tasks (0037 moved the row out of App.tsx) → it uses 0022's `needsYouCount`; the commands page
was regenerated for 0022's commands.
**Verified:** 12 of 14 audited claims as written, one corrected (Factory: "in order", not
"serially"); merge gate: `pnpm check` exit 0 (vitest 712, cargo test 313 + 13), both Python
suites OK.

## 2026-10-13 — Projects sidebar: which are being worked on now (task 0037)
**Did:** each project row says "N working" (a live session, or a check-in or heartbeat within
30 minutes on a task someone holds), "stalled" (work in Building, nothing for 30 minutes), and
when it last moved ("12 min ago", "yesterday"); order chips Working / A–Z / Needs you,
remembered; the All row sums projects ("3 working · 1 stalled"). `activity.ts` (pure, tested),
`ProjectList.tsx`. Built by an implementer agent that claimed, checked in and passed it itself.
**Fixed (review):** rows swapped places twice a minute when two projects' activity fell in the
same minute (the order now holds unless the order chosen, the projects, the working set or the
counts change); a task handed back to Ready or blocked still read as working for 30 minutes;
"yesterday" meant 24–48 hours, now the previous calendar day.
**Verified:** `pnpm check` green.

## 2026-10-13 — Text size for the whole window
**Did:** the owner: "option to zoom in the help page and all pages". ⌘+ ⌘− ⌘0 and a Text size
row in Appearance (90–150%), remembered, for everything in the window including the manual.
CSS zoom on the page root (no window permission). `zoom.ts`, the shortcut table, the manual.
**Verified:** `pnpm check` green.

## 2026-10-13 — Help is a manual: concepts, tutorials, commands, shortcuts (task 0035)
**Did:** the Help tab is a manual with a docked sidebar (every page and section, search across
pages, ◀ ▶): Start here, Concepts, Tutorials, In the app, Commands (generated from `help.rs`),
Shortcuts (generated from one table, `shortcuts.ts`, that the key handlers read), For agents,
FAQ. Pages are Markdown in `docs/manual/`, so they read on GitHub too. A "?" beside each view's
toolbar and each pane's title opens its section. Tests fail when a command or shortcut is
missing, or a link between pages is broken. Built by an implementer agent that claimed, noted
and passed the task itself.
**Open:** screenshots (the sandbox can't run the app); the owner reads Start here and a tutorial.

## 2026-10-13 — Ready is cyan, well apart from Building
**Fixed:** the owner: "ready and building colors are too close" (blue next to violet, distance 47
though past the general check). Ready is now cyan (88 from violet Building, 60+ from every
accent's Building), Shaping blue. A test keeps Ready and Building at least 60 apart.
**Verified:** `pnpm check` green.

## 2026-10-13 — The Table remembers its sort
**Did:** the owner: "tables should save what I sorted it last". The Table's sort column and
direction are saved (local storage) and restored on start; anything unreadable falls back to
Stage ascending. `Views.tsx`, `viewData.ts` (`parseTableSort`, tested).
**Verified:** `pnpm check` green.

## 2026-10-13 — Timeline shows work: from when a task started to when it finished (0031 follow-up)
**Fixed:** the owner: "it should be hour based, each task when started and finished". Bars
started at capture (mostly a date, so midnight) and ran to now, so waiting drowned the work.
Now a bar starts when the task was claimed into Building and ends when it reached Your check or
Done, one segment per stage; the wait for the owner's check is a thin dotted tail; work not
started isn't drawn but counted ("14 tasks haven't started yet"). Old done tasks with only a
logged capture and done time draw between the two.
**Found:** the times themselves were off for today's tasks: the main session passed its
builders' tasks on in batches after merging, so "finished" was the queue update, not the work
(0031 shows 00:04 for work done at 23:34). From now on builders pass their own tasks and check in.

## 2026-10-13 — Timeline: Day is the last 24 hours; done tasks end at their logged time (0031 follow-up)
**Fixed:** the owner's screenshot just after midnight: the Day scale showed the calendar day, so
it was nearly empty, and done tasks from before stage history (0011–0013) ran into the next day.
Cause: for a task with only dates, the end was `max(capture, done day) + 1 day`, which with the
exact capture time became capture + 24 h. Now the core reads the done time from the log
(`done_at`: the last timed `→ done` line), the fallback ends with the done day, and Day shows
the 24 hours from 20 h before the current hour to 4 h after it (◀ ▶ move a day; "Now" comes
back), with clock hours on the axis and the weekday where the day changes.
**Verified:** `pnpm check` green (vitest 345, cargo 192 + interop 12), `test_task.py` OK.


## 2026-10-13 — Who worked on a task and what it cost: sessions, tokens, time, live (task 0025)
**Did:** the task pane has a Work section: every session that held or logged on the task, with
stage, start, end (or live), minutes and tokens, read from the transcripts on this Mac
(`~/.claude*/projects`, linked account folders counted once, usage once per reply, subagents
included); totals against the estimates ("40 min of 2 h (33%)"); a labelled progress guess; and
View, which shows a session's transcript as escaped Markdown in the pane. Cards get a live dot
while a session's transcript changed in the last 5 minutes. `transcripts.rs` (with
`actuals()`, shared with 0017), `work.rs`, `Work.tsx`, two Tauri commands (the transcript one
only for a session the task names). Built by an implementer agent; review fixes by the main
session.
**Decided:** a session counts in full on every task that names it (no time-splitting yet); the
pane says so on the row ("also on 3 other tasks") and on the totals.
**Fixed (review):** shared sessions weren't marked, so a controller session's whole cost read as
each task's; a live row inherited the header's right-aligned style.
**Rebased onto main (0016, 0019, 0020, 0030, 0031, 0034):** a session is found in the claim, in
0030's `held-by` and in the log; "holds it" follows 0030's holder (the claim, or `held-by` in
Testing and Your check). Cards show 0030's who line and 0025's live dot (0030 left no dot of its
own). The Work section sits after the check-ins.
**Verified:** after the rebase, `pnpm check` green (vitest 357 in 16 files, cargo 217 + interop
12, clippy and fmt clean), `test_task.py` 36 OK, `test_commands.py` 10 OK; transcript paths
can't be reached from the window (8-hex id, named by the task, symlinks excluded); nothing
leaves the Mac.

## 2026-10-13 — One colour, one meaning: a fixed colour code
**Fixed:** the owner: "tags for priority and stage the color are mixing together". High priority
used the "needs you" orange (= Your check), every tag the accent (= Building), a bug's type red
(= Blocked), and normal priority violet (= Building, in colour-by-priority). Now stage colours
only say stages; high priority is its own magenta chip, normal a light slate, low grey; tags and
types are neutral outlined labels. The code is a table in `docs/DESIGN.md`; tests keep priority
colours away from every stage and check the CSS uses them.
**Verified:** `pnpm check` green.

## 2026-10-13 — Search, filter and sort every task (task 0032)
**Did:** a search box at the start of the filter bar (⌘F, Esc clears): id (`19` finds 0019),
title, tags, holder and the task's text, accent- and case-insensitive, words in any order, matches
highlighted on cards, rows and backlog notes. Filter chips by stage, type, priority, tags, holder,
flags (blocked, needs you, open questions) and dates, showing only values the project has, with
an active line (✕ each, Clear all, "N hidden by filters"). Board sort chips; saved views per
project. `muster list --search/--stage/--type/--tag/--holder/--blocked/--sort` with the same
rules (a shared fixture runs both). Built by an implementer agent; review fixes by a second.
**Fixed (review):** Esc in the search box threw away a New task draft; ⌘F also took Ctrl+F (a
text-field key on macOS); every task's search text rode on every 2 s poll (now fetched only while
searching, refreshed when a task changes); Unicode cases the CLI/app fixture didn't cover; Esc in
an empty box closed the task pane; backlog notes weren't highlighted.
**Decided:** a word of digits finds ids only; accents fold by one table shared by Rust and TS (no
new dependency for full normalisation).
**Verified:** after the rebase onto main (0016 levels, 0017 fields, 0018 index, 0019 graph, 0020
edit, 0021 header, 0030 holders, 0031 timeline, 0034 colours): `pnpm check` green (vitest 554,
cargo 244 + interop 13 + search cases 3), `test_task.py` 38 OK, `test_commands.py` 10 OK.

## 2026-10-13 — Pane header D2 (0021), fields and comments (0017), INDEX.md (0018), vision check (0029)
**Did:** four tasks built in parallel on their own branches, each reviewed with one fix round, then
merged into main one at a time (each merge resolved against the other session's 0019, 0030 and 0031):
- 0021: your D2 header. Title and status first, the stage track, the stage's content beside a
  sidebar with the next step.
- 0017: the new type list, test-time estimates, https-only links, and a comment thread under the
  write-up.
- 0018: `INDEX.md`, regenerated after every write. A change made outside muster is only a refresh
  warning.
- 0029: `muster vision`, a brief for an agent to check a task against VISION.md, decisions and
  DESIGN.md. It records conflicts, holds Shaping → Ready while any is open, and turns a change of
  direction into one owner question with the cost of pivoting.
**Fixed (from reviews):**
- Time spent showed three times too long (agent-day vs clock-day).
- An undo after a track move could leave Building unheld or override an agent's move. Fixed
  with `stage --from`.
- The drop zone swallowed text drops.
- Answers from Drover would have failed `check` on `INDEX.md`.
- Agent text in a vision question could route the owner's answer to a mailbox.
- Repeated checks piled up owner questions.
**Verified:** after the last merge, `pnpm check` exit 0 (437 UI, 258 Rust, 13 interop tests),
`python3 -I bin/test_task.py` 38 OK, `bin/test_commands.py` 10 OK, also under `TZ=Pacific/Kiritimati`.
The release app was rebuilt.
**Open:**
- Not seen in a real window: the D2 layout in light and dark, links opening, and the comment
  box's Esc guard.
- Your check reads "you owner" in the status line (wording).

## 2026-10-12 — Who's on it, since when, how far: session stamps, held-by, check-ins (task 0030)
**Did:** `muster` stamps who acts with the session (`wt-a (wt-0030 · session 7b5e8856 · cc-hg)`:
checkout folder name, short session id, account label; `session.rs`). A holder is its label plus
its session. Testing and Your check get a `held-by`. `muster checkin <id> "<text>" [--pct N]` is
the holder's heartbeat with words. Every card from Ready to Your check says who's on it and
since when, its latest check-in, and "no check-in for 45 min" past 30 minutes; the pane lists
holders per stage and every check-in. `list`/`show` print the same. PROTOCOL asks for a check-in
at each milestone and every 30 minutes. Built by an implementer agent; review fixes by the main
session. The live dot is 0025's.
**Decided:** with no `--as`, the actor is the queue folder's git user name plus the session
(owner, 2026-10-12: "git username and session id is fine"), never `$USER`; in a session with no
git name it's `agent`, never `owner`. A task sent back to Building returns to the builder's
label (so a new session under it carries on), unless another session under that label holds
work, then to the exact session.
**Fixed (review):** a checkout, session name or account containing the login name is dropped
(not only an exact match); a blank `--as` is refused even in a session; a return no longer locks
the task to a session that may have ended.
**Merged with main:** rebased on 0015/0016/0019/0020/0031/0034: holders are compared by label
and session in 0020's edit permission and heartbeat, 0019's `next`, and 0015's add/file/drop
actors; a write-up line that reads as a check-in is refused, as a stage line is; `checkin` is
listed as not routed by `/mu` (it's an agent command).
**Verified:** after the rebase, `pnpm check` green (vitest 343, cargo 191 + interop 12),
`test_task.py` 36 OK, `test_commands.py` 10 OK.

## 2026-10-12 — Dependency graph: `depends:` and a Graph tab (task 0019)
**Did:** tasks have a `depends:` field (ids in the same queue), set with `muster set <id> depends
0014,0016`, which refuses unknown ids, itself and a loop. `muster check`, the Problems tab and
`manifest.py --check` flag a bad id, a missing one, itself and loops (Tarjan, iterative). The app
has a Graph tab beside Board, Table and Timeline: a layered left-to-right layout in our own code
(longest-path ranks, barycentre sweeps), arrows from a task to what waits on it, ready-to-start
marked, done dimmed with "Hide done", unlinked tasks in a strip below; selecting a task lights
its whole up- and downstream. `queue.rs`, `ops.rs`, `help.rs`, `graph.ts`, `GraphView.tsx`,
`App.tsx`, `styles.css`, `bin/manifest.py`. Muster's queue migrated (0002–0005, 0015–0018,
0026, 0028); preview in `tasks/attachments/0019/`.
**Verified:** `pnpm check` green: vitest 120 (12 new), cargo 77 + interop 5, clippy, build;
`test_task.py` 23 OK; layout of 100 tasks under 50 ms (test); `muster check` clean on the queue.
**Fixed (review):** `set depends` now checks and writes under one lock (two opposite sets at
once could have made a loop); `claim` refuses and `next` skips a task whose dependencies aren't
done; done tasks dim; a filtered-out dependency keeps its task in the graph with "waits on 0002
(not shown)", and every node says what it waits on and how many wait on it, in words; unlinked
chips show their state; manifest.py splits and reads ids as the core does.
**Open:** `ready --parallel` (0023) will read `depends:` too.
## 2026-10-12 — Timeline scales: a day in hours, a week, a month (task 0031)
**Did:** the Timeline has Day · Week · Month chips (saved), ◀ ▶ and Today. Bars come from the
stage history's stamps: waiting from capture (to the minute when the log knows it, new
`captured_at`), then one segment per stage in its colour, blocked stretches hatched (new
`block_history`, from the log's block and unblock lines), and a now line. Week and Month snap to
quarter days and days. `viewData.ts` (pure, tested), `Views.tsx`, `queue.rs`, `styles.css`,
`AS_BUILT.md`. Built by an implementer agent; capture time, blocked spans and review fixes by the
main session. Preview: `tasks/attachments/0031/`.
**Decided:** time is wall-clock minutes (local stamps read as UTC), so days are always 24 h; a
span across a clock change is off by that hour.
**Fixed (review):** blocked spans (spec AC2) were missing; the controls' tests only checked
labels (now they press the buttons); a timing test could fail a busy gate (best of 5); ▶ moved
when the scale changed; a first log line that's a claim was taken for the capture time.
**Verified:** `pnpm check` green: vitest 268, cargo 115 + interop 8; mutation (◀/▶ swapped) caught.

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

## 2026-10-12 — Memorable stage colours; Building uses the accent (task 0034)
**Decided:** the owner asked for colours people know, "not our made up rules": Ready blue,
Building the accent, Testing amber, Your check (and "needs you") orange, Done green, Blocked red,
Backlog grey, Shaping light cyan. The old "stage colours are never orange or blue" rule is gone;
accents keep theirs (that one tells the apps apart). An accent that would pass for another stage
(rose, gold, green, graphite, teal) leaves Building violet, and Appearance says why.
**Did:** `themes.ts` (STAGE_COLOURS, `colourDistance`, `buildingColour`), `styles.css` defaults,
`Appearance.tsx` note, `AS_BUILT.md`. Preview: `tasks/attachments/0034/`.
**Verified:** `pnpm check` green (vitest 233); every stage pair at colour distance ≥ 30; dark words
on every stage colour AA; edges 3:1 (0026's tests).
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
