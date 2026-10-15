# The owner's flows through Muster: what they come to see and do, where it's slow or hidden (2026-10-15)

Task mu-106, a spike: no code changed. Walked by Pouya on 2026-10-15 against main at `7e1bf45`,
headless, on a synthetic copy of the queue's shape. Every number here was measured in that run
unless it says "code read". Proposals are mine and are labelled.

**Answer.** Three things cost the owner most, every day:

1. **The board hides the owner's own column.** Your check is the last column, and with the owner's
   widths it sits past the board's right edge (in ADHD partly at 1440 px and wholly at 1100 px; in Focus wholly at 1100 px),
   with no cue. In ADHD, open a task and the board is gone altogether. (R4, R5)
2. **Numbers without words, and words with two meanings.** "9 working" on All counts projects;
   "16 working" under it counts tasks. Project rows read "17 ⏸1 41". The Work section said
   "Tokens 4.4M of 800k (546%)", 4M of it cache reads. That's what "couldn't tell sessions were
   running" and "nonsense" came from. (R3, R6, R11)
3. **Too much stands between the owner and what needs them.** The ADHD filter bar is 9 to 10 rows
   above the board, the toolbar runs off a 1100 px window, and Waiting for you is one list of 275
   items, 37,790 px tall. (R1, R2, R9)

Each of the 19 findings is now a note on the open task that owns it (8) or a new backlog note
(11). See [Where each finding went](#where-each-finding-went).

## How it was captured

- **Harness:** the lead's headless QA harness (`/private/tmp/claude-501/qah`, Playwright 1.63
  WebKit, headless) on the frontend built from main (`vite build` into the harness folder). Tauri's
  IPC was mocked and answered from a fixture; anything that would write was answered with nothing.
  No window was raised on the owner's screen, and nothing was clicked or typed in the real app.
  Nothing was added to the repo, and no dependency.
- **Fixture:** a read-only snapshot of every queue (`muster projects`, `muster list`), then rewritten
  by `synth.py`, which keeps the shape and replaces every word. The shape is stages, counts, claims,
  check-ins, questions, criteria and times: 20 projects, 944 tasks, 275 needing the owner. Every
  title, project name, label, path, session id, check-in, question, criterion, comment, note and
  activity line is generated, and a guard checks that no real name or path is left. On top of
  that, staged cases: session states on the held cards (working, waiting for input, ended), two of
  the owner's own tasks, a pick-one question, and a two-session Work section. A second fixture has
  Persian titles and labels with the language set to fa.
- **Views:** the owner's own window settings (`owner-ls.json`: sidebar 320 px, pane 718 px, the
  columns they opened), in ADHD mode, plus Focus (the owner's saved mode) for F1 and F3. The other flows' Focus lines are "code read". Light (violet-day)
  and dark (violet-night), at 1100×760 and 1440×900.
- **Output:** 160 screenshots (32 of them Focus), accessibility snapshots, and the measurements in `notes.jsonl`, all
  in `.muster/attachments/0106/ux/` with the scripts that made them (`synth.py`, `leakcheck.py`, `run.sh`, `steps/`; Focus runs are `MODE=focus run.sh F1`).
  Links below are relative to this file and resolve in the main checkout. File names read
  `<flow>-<step>-<mode>-<theme>-<width>.png`.
- **Settings untouched:** `projects.json`, `settings.json`, `roles.toml` and `helper/` hash the same
  before and after (`settings-before.sha`, `settings-after.sha`). `notified.json` changed in
  between: the owner's running Muster.app (pid 95761) writes it. The harness has no core to write it.
- **Not walked:** the menu bar, the Dock badge and notifications are native, so they were read in
  `tray.rs` and `notify.rs` ("code read"), not shot. Light and dark were checked by eye. No contrast
  was measured, and none of the findings rests on colour alone.

## F1 What's waiting for me, across every project

ADHD, 1440 px, All. [Open, light](../../.muster/attachments/0106/ux/F1-1-open-adhd-light-1440.png) ·
[open, dark](../../.muster/attachments/0106/ux/F1-1-open-adhd-dark-1440.png) ·
[open 1100](../../.muster/attachments/0106/ux/F1-1-open-adhd-dark-1100.png) ·
[Waiting for you, light](../../.muster/attachments/0106/ux/F1-2-waiting-adhd-light-1440.png) ·
[dark](../../.muster/attachments/0106/ux/F1-2-waiting-adhd-dark-1440.png) ·
[Focus 1440](../../.muster/attachments/0106/ux/F1-1-open-focus-dark-1440.png) ·
[Focus 1100, light](../../.muster/attachments/0106/ux/F1-1-open-focus-light-1100.png) ·
[Focus, Waiting for you](../../.muster/attachments/0106/ux/F1-2-waiting-focus-dark-1440.png)

| Step | Screen | Action | Visible | Hidden |
|---|---|---|---|---|
| 1 | Board, All | open the app | "Waiting for you 275" in the toolbar; sidebar "275" on All, "12 need you" and "263 need you" on the headings; "Over your limit in 17 projects" | Your check past the board's right edge (scrollWidth 1042 > 851 at 1440; Building, Testing and Your check at 1100); at 1100 Help, Settings, the mode chip and "live" are past the toolbar's edge; the board starts at y≈361 under 10 rows of filters |
| 2 | Waiting for you | 1 click on the toolbar pill (no key; 59 Alt+Tab presses) | From shaping (1 question), "Questions, blocks and your own tasks 16", the first 4 items | Just accept 124 and One look 134, below a 37,790 px list; the group heads scroll away with it |
| 3 | Menu bar (code read) | glance | the head with its antennas marked, and "275" beside it | which items: the menu is only Show Muster, the count, Quit |
| 4 | Notification (code read) | click | Muster comes forward | the item: nothing opens it, the window stays on its last view |

**Total:** 1 click to the list; no key reaches it. Scrolling to Just accept is a few screens; to
the end, about 42.
**Focus:** the sidebar drops the working, total and time figures and keeps names, needs-you counts
and the "N in Your check, limit 5" lines; the filter bar folds to "Search and filters ⌘F"; the board
starts at y=135 (361 in ADHD); cards are a title and one status line. The board still opens on All.
With the Backlog, Shaping and Ready columns the owner opened in Focus, every column fits at 1440
(scrollWidth 851 = clientWidth), but at 1100 Testing is cut and Your check is wholly past the edge
(786 > 580; R4). Waiting for you is the same 275-item list.
**Keyboard:** with macOS Keyboard navigation off (the default), Tab stops at 6 places only: the two
headings, the splitter, Search, Quick add and the page. WebKit skips buttons. With Alt+Tab,
Waiting for you is the 59th stop, after two stops per project (R14). In Focus, Tab has 5 stops
(no Search field) and Waiting for you is the 54th Alt+Tab stop. The sidebar's accessibility
labels are complete ("harbor: 17 in progress, 1 paused, 41 tasks").

## F2 Answer a question and unblock a task blocked on me

[Waiting, question first](../../.muster/attachments/0106/ux/F2-1-waiting-question-adhd-light-1440.png) ·
[pane beside the list, light](../../.muster/attachments/0106/ux/F2-2-pane-question-adhd-light-1440.png) ·
[dark 1100](../../.muster/attachments/0106/ux/F2-2-pane-question-adhd-dark-1100.png) ·
[blocked on you, pane](../../.muster/attachments/0106/ux/F2-3-pane-blocked-on-you-adhd-dark-1440.png)

| Step | Screen | Action | Visible | Hidden |
|---|---|---|---|---|
| 1 | Waiting for you | 1 click | the pick-one question with its three options, the asker's pick, "going with (a) unless you answer" | nothing on the first screen |
| 2a | Waiting for you | 1 click on an option | answered in place | — |
| 2b | Pane beside the list | "Open to answer" (1 click); the pane takes 0.73–0.91 s the first time | the question with its options and a text box at y≈569 of the 846 px pane; Answer; ⌘↩ sends | at 1100 the list beside it narrows to one column |
| 3 | Waiting for you | "Done, unblock" (1 click) on a block on the owner | "Your action: (a signed build)" | — |
| 3' | Board → pane | card (1), then Unblock in Next step (1) | the block's words on the card and in the pane | the card is one of 308 in Ready on All |

**Total:** pick-one, 2 clicks. Words: 2 clicks, typing and ⌘↩. Unblock: 2 clicks.
This flow works. What's wrong in it: both buttons on a block item are primary, and a block that
asks nothing still says "Open to answer" (R9).
**Focus:** the same (code read).

## F3 Check and accept a task, or send it back

[Board, ember](../../.muster/attachments/0106/ux/F3-1-board-your-check-adhd-light-1440.png) ·
[pane, light](../../.muster/attachments/0106/ux/F3-2-pane-top-adhd-light-1440.png) ·
[pane, dark](../../.muster/attachments/0106/ux/F3-2-pane-top-adhd-dark-1440.png) ·
[pane at 1100](../../.muster/attachments/0106/ux/F3-2-pane-top-adhd-dark-1100.png) ·
[Just accept](../../.muster/attachments/0106/ux/F3-4-waiting-just-accept-adhd-light-1440.png) ·
[Focus pane, light](../../.muster/attachments/0106/ux/F3-2-pane-top-focus-light-1440.png) ·
[Focus pane 1100, scrolled](../../.muster/attachments/0106/ux/F3-3-pane-scrolled-focus-dark-1100.png)

| Step | Screen | Action | Visible | Hidden |
|---|---|---|---|---|
| 1 | Sidebar | pick the project (1 click) | its board, Your check open with Just accept / One look groups | on All, Your check is past the edge (R4) |
| 2 | Pane | the card (1 click; 0.83 s the first time) | What you're accepting (Built, Accept, Try), Proof per criterion, Your checklist, Next step: Accept, Send back to builder, Block | the board: at 1440 it stacks under Recently finished and gets ~0 px; at 1100 the content column is the filter bar wrapped a word a line (R5) |
| 3 | Pane | Tick the owner's criterion (1), Accept (1) | Accept is muted until that criterion is ticked | — |
| 3' | Pane | Send back to builder (1), the reason, ⌘↩ | | |
| 3" | Waiting for you › Just accept | Accept all (1) and its confirm (1) | 124 tasks with nothing to try | One look's 134 are below it |

**Total:** 4 clicks per task from the board; 2 clicks for all of Just accept.
The pane's header spends three lines on session stamps ("Building: Shaper_67 (main · session
dd876827 · cc-hg) · Testing: …") before what was built (R16).
**Focus:** the pane folds Vision, Review, Work and the Log to a header each; the steps, the click
count and where Accept and Send back sit (y=254 and 287 in the pane at 1440) are the same. The board
stays in view beside the pane: at 1440 its columns keep about 450 px of height above Recently
finished; at 1100 it narrows to a column and a half, so Your check is off the right edge there. So
R5 is ADHD's: Focus's folded filter bar is what leaves the board its room.
**fa:** [board](../../.muster/attachments/0106/ux/Ffa-F1-open-fa-adhd-light-1440.png) ·
[pane](../../.muster/attachments/0106/ux/Ffa-F3-pane-fa-adhd-light-1440.png): the layout mirrors
whole. Two sentences read in the wrong order (R19). The rest is untranslated text, which is
mu-147's and mu-148's.

## F4 See what's running now

[Board, Building](../../.muster/attachments/0106/ux/F4-1-board-building-adhd-light-1440.png) ·
[pane, light](../../.muster/attachments/0106/ux/F4-2-pane-top-adhd-light-1440.png) ·
[pane, dark](../../.muster/attachments/0106/ux/F4-2-pane-top-adhd-dark-1440.png) ·
[Activity](../../.muster/attachments/0106/ux/F4-4-activity-adhd-dark-1440.png) ·
[Timeline](../../.muster/attachments/0106/ux/F4-5-timeline-adhd-dark-1440.png)

| Step | Screen | Action | Visible | Hidden |
|---|---|---|---|---|
| 1 | Sidebar | glance | All "9 working", Apps "16 working", harbor "8 working" | that 9 counts projects and 16 and 8 count tasks: only the tooltips say it (R3) |
| 2 | Board, project | pick it (1 click) | "BUILDING 7"; the first card's ribbon and "working" / "waiting for input" lines (11 cards say a session state) | 6 of 7 Building cards ("+6 below"); no "4 working · 1 waiting · 1 ended" anywhere |
| 3 | Pane | the card (1 click) | "LIVE on it: Builder_43 … started 1 min ago", "working", the stage track, Ask for a check-in, the message box | the Work section: below the description, Vision and Review |
| 4 | Pane › Work | scroll | "Time 4.6 h of 4 h (115%)", "Tokens 4.4M of 800k (546%)", 4M of it cache reads (R6) | what the task itself cost |
| 5 | Activity / Timeline | 1 click each | log lines, each with the full session stamp; Timeline rows by key | what's running: Activity mixes it with every change; the Timeline's first screen is Ready and Done (R8, R17) |

**Total:** 2 clicks for one task's state, 2 more for one project's. No one place shows every
session at work across projects; that place is mu-98's Sessions view, now in Testing.
**Focus:** cards are a title and one status line (F1's Focus shots). That reads faster, but
"N working" means the same mixed things (code read).

## F5 Start, pause, ping or stop work, and file a new idea

[Quick add](../../.muster/attachments/0106/ux/F5-1-quick-add-adhd-light-1440.png) ·
[New task](../../.muster/attachments/0106/ux/F5-2-new-task-adhd-light-1440.png) ·
[held task's pane](../../.muster/attachments/0106/ux/F5-3-pane-held-controls-adhd-dark-1440.png) ·
[session ended](../../.muster/attachments/0106/ux/F5-4-pane-session-ended-adhd-dark-1440.png)

| Step | Screen | Action | Visible | Hidden |
|---|---|---|---|---|
| 1 | Board › Backlog | type in "Add to the backlog of …", ↩ | the field at the column's top | in Focus, a folded Backlog is a chip until opened (1 click; code read; the owner keeps it open) |
| 2 | Pane: New task | ⌘N (1 key; 23–48 ms) | project, title, description, ⌘↩ creates | — |
| 3 | Pane of a held task | the card (1), then Pause (1), why optional | Pause in the header, Send back, Block | — |
| 4 | Pane › Session | Ask for a check-in (1), or a message and Send | the strip under the header | — |
| 5 | Pane of a task whose session ended | the card (1) | "Ask for a check-in", Pause | anything that frees the claim (mu-184 adds Release) |
| — | Start, Stop | none in the app | | by design the app never claims; mu-118 (dispatcher) and mu-120 (Team view: Stop, Pause, Ping, Reclaim) add them |

**Total:** a note takes 1 field and ↩; a task takes ⌘N, typing and ⌘↩. Pause and Ping take
2 clicks each.
**Focus:** the same, except the quick add is behind the Backlog chip when Backlog is folded (code read).

## F6 Find a task or a project

[Search](../../.muster/attachments/0106/ux/F6-1-search-adhd-light-1100.png) ·
[a key](../../.muster/attachments/0106/ux/F6-2-search-key-done-task-adhd-light-1440.png) ·
[Table](../../.muster/attachments/0106/ux/F6-3-table-adhd-dark-1440.png) ·
[sidebar](../../.muster/attachments/0106/ux/F6-4-sidebar-scrolled-adhd-light-1440.png)

| Step | Screen | Action | Visible | Hidden |
|---|---|---|---|---|
| 1 | Board, All | ⌘F (1 key), type | "45 of 944 tasks", "916 hidden by filters · Clear all", matches marked on the cards | the board starts below 10 rows of filter chips (y≈470 at 1100) |
| 2 | Board | type a key ("qu-74") | the Done task's card alone | — |
| 3 | Sidebar | pick a project (1 click) | its board | at 900 px tall, about 8 of 20 projects are below the sidebar's fold |
| 4 | Filters | 1 click per chip | every group, every value | Held by lists all 30 holders |

**Total:** 1 key and typing. Search works, and it's fast.
**Focus:** no filter bar until ⌘F (F1's Focus shots), so the board has the room. That's what mu-169 asks for in every
mode.

## Ranked findings

Score = how often the owner meets it (1–3) × what it costs them (1–3) ÷ the effort to fix (1–3).
Ties go to the higher cost. A dash means the finding isn't scored on that column. The "rule" column
names the DESIGN.md rule each finding breaks.

| # | Finding | Flow | Screen | DESIGN.md rule | Score | Where |
|---|---|---|---|---|---|---|
| R1 | The ADHD filter bar is 9–10 rows (~250–300 px) above the board; Held by lists 30 names | all | Board, Table, Timeline | Edges, not the middle; lead with what matters | 3×2÷1 = 6 | mu-169 (note) |
| R2 | At 1100 px the toolbar runs off the window: Help, Settings, mode chip, live | F1 | Toolbar | Fixed places; everything visible | 2×3÷1 = 6 | mu-202 (filed from `2026-10-15-0207-toolbar-runs-past-the-window-at-1100-px-help.md`) |
| R3 | "N working" counts projects on All but tasks on headings and rows | F4 | Sidebar | One wording per count; words carry the meaning | 3×2÷1 = 6 | mu-197 (filed from `2026-10-15-0207-n-working-counts-projects-on-the-all-row-but.md`) |
| R4 | Your check is past the board's right edge with no cue (ADHD: 1440 partly, 1100 wholly; Focus: 1100 wholly) | F1, F3 | Board | What needs you is always in view; fold in place | 3×3÷2 = 4.5 | mu-198 (filed from `2026-10-15-0207-your-check-sits-past-the-board-s-right-edge-with.md`) |
| R5 | With the pane open in ADHD, the board disappears: Recently finished stacks over its height (1440), the filter bar wraps a word a line (1100) | F2–F5 | Board + pane | Side by side over tabs; no layout jumps | 3×3÷2 = 4.5 | mu-203 (filed from `2026-10-15-0207-with-the-task-pane-open-the-board-disappears.md`, after mu-169: its fold is mu-169's toggle applied by itself) |
| R6 | The Work section's "Tokens 4.4M of 800k (546%)" counts cache reads | F4 | Pane › Work | Show the value that means something | 2×2÷1 = 4 | mu-172 (note) |
| R7 | A task whose session ended offers Ask for a check-in, and nothing frees it | F5 | Pane | Every action visible | 2×2÷1 = 4 | mu-184 (note) |
| R8 | No one place answers "what's running": Activity is every change, each with its session stamp | F4 | Activity | Lead with what matters | 2×2÷1 = 4 | mu-98 (note) |
| R9 | Waiting for you is 275 items, 37,790 px long; two primary buttons per block item; "Open to answer" where nothing is asked | F1, F2 | Waiting for you | One primary action per place; group by purpose | 3×2÷2 = 3 | mu-199 (filed from `2026-10-15-0207-waiting-for-you-is-one-275-item-37-790-px-list.md`) |
| R10 | 258 of 275 needs-you items are Your check over its limit in 17 projects; orange on 17 sidebar rows | F1, F3 | Sidebar, Board | Attention colour only on what's waiting | 3×3÷3 = 3 | mu-138 (note) |
| R11 | Project rows show bare digits ("17 ⏸1 41") | F1, F6 | Sidebar | No number without its words (0082) | 3×1÷1 = 3 | mu-200 (filed from `2026-10-15-0207-sidebar-project-rows-show-bare-digits-17-1-41-in.md`) |
| R12 | First pane open 0.73–0.91 s headless, later ones 45–69 ms (for /profile) | F2–F5 | Pane | Performance baseline | 3×1÷1 = 3 | mu-206 (filed from `2026-10-15-0207-the-task-pane-s-first-open-takes-0-7-0-9-s.md`) |
| R13 | Menu bar and notifications never lead to the item (code read) | F1 | Menu bar, notification | What needs you is surfaced | 2×2÷2 = 2 | mu-205 (filed from `2026-10-15-0207-a-notification-or-the-menu-bar-never-leads-to.md`) |
| R14 | Tab reaches 6 stops by default; Waiting for you is Alt+Tab stop 59; no keys for views | all | Window | Accessibility lens | 2×2÷2 = 2 | mu-207 (filed from `2026-10-15-0207-keyboard-tab-reaches-6-stops-by-default-webkit.md`) |
| R15 | The app opens on the whole board for All, not on what needs the owner | F1 | Board | What needs you is always in view | 3×2÷3 = 2 | mu-142 (note) |
| R16 | The Your check pane's header is three lines of session stamps | F3 | Pane | Show the value, not every value | 2×1÷1 = 2 | mu-144 (note) |
| R17 | The Timeline sorts by key, so what's running is below the first screen | F4 | Timeline | Lead with what matters | 2×1÷1 = 2 | mu-204 (filed from `2026-10-15-0207-timeline-doesn-t-lead-with-what-s-running-rows.md`) |
| R18 | Recently finished lists a task twice (merged, accepted) | F1, F4 | Recently finished | Compact, not cramped | 2×1÷1 = 2 | mu-201 (filed from `2026-10-15-0207-recently-finished-lists-a-task-twice-under-all.md`) |
| R19 | fa: the over-limit line and "changed … ago" read in the wrong order | F1, F3 | Board, pane | Languages: no sentence built from parts | 1×2÷1 = 2 | mu-149 (note) |

What's already queued and not repeated here: the timeline legend (mu-173), the hover card
(mu-174), the toggles for the sidebar, Recently finished and filters (mu-167, mu-169), "search
always visible" (mu-169), working as a ribbon (mu-171), the token ribbon (mu-172), session states
across accounts (mu-182, done), and Release for an unreachable claim (mu-184). R1, R6 and R7 add
measurements to three of those.

## Proposals, checked against DESIGN.md's rules

Each proposal below is mine. The note it went to holds the detail.

| Finding | Proposal | Overlay or covering layer? | Control seen only on pointing? | Folds away what needs you? | Fixed places? |
|---|---|---|---|---|---|
| for R2 | the toolbar keeps its line: the quiet pills shorten, never leave the window | no | no | no | yes |
| for R3 | one helper for "N working", saying projects or sessions in words | no | no | no | yes |
| for R4 | an off-edge cue at the board's right ("+2 columns · 29 need you", click scrolls there), or Your check first while it has cards | no | no | no: it surfaces it | yes, the cue has a fixed place, as ColumnRow's does |
| for R5 | with the pane open and the board under its minimum, mu-169's toggles apply by themselves: Recently finished folds to its chip and filters to their button (mu-203, after mu-169) | no | no | no: chips keep their counts | yes |
| for R9 | a fixed jump line of each group's count at the list's top; one primary per item | no | no | no | yes |
| for R11 | the row's counts in words where there's room, else a one-line key | no | no | no | yes |
| for R13 | Show Muster opens Waiting for you; the menu lists the first 3 items; a notification's click opens the list | no: native menu items | no | no | yes |
| for R14 | ⌘1–⌘9 for the views, j/k through cards, a skip-to-board key | no | no | no | yes |
| for R17 | "Active now first" as the Timeline's default sort | no | no | no | yes |
| for R18 | one row per task with both stamps | no | no | no | yes |

**Rules check:** DESIGN.md allows no popover, no modal except to confirm something destructive,
and no hover-only control. None of these proposals adds one. R12 is a measurement for /profile, not
a design.

## Where each finding went

- **New backlog notes (11), each citing mu-106 and its finding:** R2, R3, R4, R5, R9, R11, R12, R13,
  R14, R17, R18. Shaping has since filed them as mu-197 to mu-207; the findings table names each
  task and the note it came from.
- **Notes on open tasks (8):** mu-169 (R1), mu-172 (R6), mu-184 (R7), mu-98 (R8), mu-138 (R10),
  mu-142 (R15), mu-144 (R16), mu-149 (R19).
- **The flow map can seed mu-135's screen map:** F1–F6 name the screens and how each is reached.

## Gaps

- Native surfaces weren't shot: the menu bar, the Dock badge and notifications. They come from a
  code read (R13).
- Timings come from headless WebKit with mocked IPC: they leave out the core's poll and disk
  reads. R12 is for /profile on a release build.
- Contrast wasn't measured. No finding depends on colour alone.
- The fixture keeps the queue's shape, not its text. Long real titles and Persian text in real
  tasks may wrap more than the generated ones do.
- mu-143 (feature grid), mu-98 (Sessions view), mu-171, mu-172, mu-173 and mu-184 are changing the
  same screens. This walk is of main before them.
