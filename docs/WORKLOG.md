# Muster — work log



Newest first. What changed, and how it was checked. The current state is in `AS_BUILT.md`.

## 2026-10-13 — Display modes: ADHD, Standard and Focus, in Settings and on ⌘. (task 0038)
**Did:** Settings › Display picks ADHD (default, today's screens), Standard or Focus: three chips
and a table of every element by mode, current column lit; applied at once, kept in
`localStorage`. The header ends with the mode as a chip that opens that page; ⌘. (Ctrl+. too)
toggles Focus and back. `modes.ts` is the one table of what differs (filter bar, Who and Group by,
card lines, board columns, task pane sections, sidebar); components read it through `useLook`
(`FilterBar`, `Card`/`CardStatus`, `Groups`, `ProjectList`, `TaskPanel` via `ModeViews.tsx`'s
`Fold`, `ColumnHead`, `ChoiceChips`, `ModeChip`; App for the board and sidebar). Standard: filters
behind **Filters (n)**, Who and Group by as their value, brief cards (the rest on the selected
card), Vision, Work and Log folded. Focus: Building, Testing and Your check open, other columns
strips with counts and needs-you counts, title-and-status cards, a Search and filters ⌘F button
for the bar, a narrow sidebar, and 0051's Recently finished rail folded to a strip with its count
(a click opens it in place, Fold puts it back; ADHD and Standard keep it open as 0051 made it).
0051's Done line ("done 4 min ago") has its own row, the same in all three: it's the Done card's
status line. DESIGN.md is per mode (shared rules first); the manual has
Settings › Display and ⌘.; AS_BUILT updated. Mockups in the queue's `attachments/0038/`.
**Decided:** fold in place (design A of three, `attachments/0038/mockups.md`): one layout, a mode
only says how much shows, every fold a labelled chip, strip or header one click or key away.
Standard's secondary lines show on the selected card, not on hover (hover would push the cards
below). Focus keeps the filter bar out while anything hides tasks. Posted on the task as "decided
for you, overrule any time".
**Verified:** `bin/gate`: `gate: green (pnpm check, test_task, test_commands, cargo test
(Kiritimati))`; 1004 app tests, new `modes.test.ts` (15: the table, ⌘. and kept mode, and the
AC2 scan, shown to fail on a planted `useLook("nope")` and `mode === "focus"`) and
`Display.test.ts` (23: Settings, header chip, ⌘., each element per mode, needs you in every mode).
**Open:** the owner tries all three on the real queue (AC4); ⌘. not yet pressed in the built app.
## 2026-10-13 — Task keys with a project prefix: mu-12 (task 0033)
**Did:** every place that shows a task shows its key (`mu-12`, no leading zeros): cards, rows,
graph, timeline, task pane, Recently finished, INDEX.md, `muster list`/`show`/`check`, refusals
and new log lines; files, `depends:`, `tasks.json` and manifest.py keep `0012`. Every command and
the app's search take `mu-12`, `MU-12`, `12` or `0012`; another project's key finds its queue
(`muster show br-7` from Muster), a conflicting `--queue` is refused. Prefix per project in the
queue's README front matter (`prefix: mu`), set with `muster projects set <name> prefix`,
`muster init --prefix` or Settings › Projects (inline, refusal shown under the field); 2–4
lowercase letters, unique. Keys link in the write-up, Log, comments and questions. Branches are
`task/mu-12-slug`. Files: `key.rs`, `taskKey.ts`, `keyLinks.ts`, cli/ops/parallel/projects/index.
**Decided:** default prefix = first 2 letters of the project, 3 on a clash, and a queue stores its
default on its first write so keys never drift (a rename or a new project can't change them); a
branch belongs to its task by number whatever the prefix; error text is keyed at the CLI/app edge
(only ids of tasks in that queue, plus the typed one). Prefix moves to `.muster/config.toml` in 0005.
**Fixed (review):** a prefix change refused merge/release of started tasks; `block --on "mu-2, mu-3"`
blocked on one; a new prefix left INDEX.md stale; New task's Parent took only `0030`.
**Verified:** `bin/gate` → `gate: green (pnpm check, test_task, test_commands, cargo test
(Kiritimati))` on ec8db79; two reviewers (correctness+Rust, TS+tests), 7 findings, all fixed test-first.
**Open:** keys in the Vision section don't link; the helper's own errors aren't keyed; the first
write to each real queue adds `prefix:` front matter to its README (a change in the queue's git).

## 2026-10-13 — Recently finished: what got merged and accepted, newest first, "4 min ago" (task 0051)
**Did:** the owner: "looking at a project I need to see list of recent tasks done, by date: like
4 min ago". **Recently finished** is docked at the board's right edge (plain or grouped): merged
(the latest Testing → Your check stage line) and accepted (into Done, `doneAt`) rows, newest
first, grouped Today · Yesterday · dates with counts, when in words ("4 min ago", "2 h ago",
"yesterday 18:20", then the date); chips All · Merged · Accepted, remembered; the last 7 days,
**Show older** adds a week; each row opens its task and says who built it and its time and
tokens (0025's totals). With All, rows carry their project. The Done column sorts newest finished
first under the default sort (app `lib.ts` and the CLI's `search.rs`, the shared order), and Done
cards say "done 4 min ago". Files: `recentData.ts`, `Recent.tsx`, `CardStatus.tsx`, `App.tsx`
(board area only), `lib.ts`, `search.rs`, `help.rs`, `styles.css`; manual 03, 04, 05 and
AS_BUILT updated.
**Decided:** rows only for what's still finished (a task sent back drops out); a merge is only a
Testing → Your check line (a hand move isn't one); the owner's own task's Done is an accepted row;
the list follows the picked project, not the filter bar; the Done order changes only under the
priority sort, so the other Sort chips still order Done their way.
**Verified:** `bin/gate` → `gate: green (pnpm check, test_task, test_commands, cargo test
(Kiritimati))`; vitest 1017 tests on main f038f0b (new: `recentData.test.ts`, `Recent.test.ts`, `paneSize.test.ts`, Card's Done line,
all with a fake clock, also run under Kiritimati, Los Angeles and Lord Howe time); cargo
`priority_puts_done_newest_finished_first`.
**Fixed (review):** the rail was a fixed 272 px, so with the pane open the board lost most of its
width (398 px on a 1440 px window, 0 with the pane at its widest, the rail's right edge
clipped) → the rail gives way (about a quarter of the row, 200 to 272 px) and on the board the
pane's widest leaves the rail 200 px and the board its old 210 px (`paneSize.ts` `clampPane`,
pinned in `paneSize.test.ts` with the CSS numbers). No fold to a strip: the clamp keeps every
row in view, and a strip would hide them.
**Fixed (review):** windows of 820–1040 px with the pane open still squeezed the board (10 px at
820, 130 at 960) because the pane's floor of 420 px won and the rail kept 200 → the pane's widest
is now the window less the sidebar and the board's 210 px in every view (no 420 floor, so 400 at
820), and when the board's row can't hold the board and the rail side by side the rail moves under
the board, full width (`railBeside`, `.board-row.stacked`), never hidden. The pane no longer
changes width between the board and the other views. Widths count in the page's pixels under ⌘+
zoom (`layoutWidth`). Pinned in `paneSize.test.ts`: the board's room at 820, 900, 960, 1000 and
1040–2560 with the pane at its narrowest, default and widest.
**Fixed (review):** under ⌘+ zoom the page can be narrower than the sidebar, the board's 210 px and
the pane's 360 px (780): at 150% a 1000 px window left the board 97 px, and at 820 × 150% the
content collapsed and the pane ran 23 px past the window → the pane gives way first: it fits the
window down to 240 px (`PANE_FLOOR`, `.pane` min-width 240 px; you still drag it no narrower than
360), rounded down so the board keeps its full room, and never past the window. The board keeps
210 px while the page is at least 660 px (820 × 115% → pane 293; 1000 × 150% → pane 246);
below that it gets less (820 × 130% → 181 px, 820 × 150% → 97 px) and nothing runs off-screen.
The earlier claim "210 px from 820 px" holds at 100% and 115% only. Pinned in `paneSize.test.ts`.
**Open:** AC4 is the owner's look in the real app; the rail's look wasn't checked in a running
window or screenshot by the builder.
## 2026-10-13 — Pick-one questions: agents ask with options, the owner answers with one click (task 0049)
**Did:** `muster ask <id> "<q>" --option "<text>"… [--pick b] [--no-answer a]` writes 2 to 6
options under the question as plain indented lines (`- (a) … (my pick)`, `With no answer: (a)`),
so `bin/task`, Drover and `manifest.py` see ordinary text; they're read back from those lines
(`choices.rs`), a question written that way by hand included. `muster answer <id> Qn --pick b
["<more>"]` writes `(b) <the option's text> — <more>`, logs `answered Qn` and mails it to the
asker like any answer (`queue::answer_pick`, sharing `answer_with` with `answer`). The task pane's
open questions and the Waiting for you entry show the options as chips (`PickOne.tsx`): every
option, the pick marked "pick", "with no answer: a"; one click answers (the pane adds the box's
words), and the box still answers in words. `muster escalate` prints the options for
`WAITING-FOR-OWNER.md`. PROTOCOL.md, `muster help ask|answer|escalate`, the manual (02, 04, 05,
07), `/muster` and `/mu` (act with `--pick`), and AS_BUILT say so.
**Decided:** options live in the question's text, not a new section or field, so file contract
v1 holds without a version bump; the core gives the app the question without its option lines
(`choices.question`). Refused with nothing written: fewer than 2 or more than 6 options, an option
over 200 characters, empty, a duplicate or ending in "(my pick)", a pick or fallback that isn't an
option, `--pick` without options, a pick on a question without options. `bin/task` passes no
options: it still asks in words and answers any question in words.
**Verified:** `bin/gate`: gate: green (pnpm check, test_task, test_commands, cargo test
(Kiritimati)). New tests: `choices::tests` (5), `ask_with_options_writes_them_as_indented_lines_and_reads_them_back`,
`answer_pick_writes_the_options_text_logs_it_and_mails_the_asker`, `a_bad_pick_or_bad_options_write_nothing`,
`ask_with_options_and_answer_with_pick_on_the_command_line`, `options_text_lists_each_option_with_the_pick_and_the_fallback`,
interop `a_question_with_options_is_read_and_answered_by_the_python_tools`, Python
`test_a_question_with_options_is_plain_text_to_bin_task_and_muster_answers_it_by_pick` and
`test_a_question_with_options_is_answered_by_its_pick`, and `PickOne.test.ts` (6).
**Fixed (review):** a second click on a chip (the chips came back on after a pick until the next
poll, and the pane and the Waiting list each had their own) added a second answer, logged it twice
and mailed the asker again → `answer_with` refuses a ticked question ("Q1 is already answered"),
for a pick and an answer in words alike, writing nothing and sending no mail, and the chips stay
off once a pick is answered (`chipsOff`; the click goes through `sendPick`, now tested without a
DOM). `/mu ask … --option 'A window' --pick b Where?` put the flags into the question → the
router's pass-through list has `--option` (repeatable, single-quoted) and `--no-answer`, and its
ask row says so. New tests: `an_answered_question_refuses_a_second_answer_and_writes_nothing`
(fails without the check), `sendPick` and `chipsOff` in `PickOne.test.ts`, and
`test_the_ask_route_passes_options_through_before_the_question` (fails on the old router).
**Open:** AC5 is the owner's: answer a real question with options by clicking a chip and see the
asking session receive the chosen text.

## 2026-10-13 — The tester isn't the builder: Testing → Your check refused for the builder (task 0047)
**Did:** `muster pass` out of Testing is refused when the passer built the task (claimed it last,
or passed it into Testing with the gate line), matched as one holder (`session::same_holder`):
nothing is written, and the refusal names the builder and suggests a QA label. The merge of a
started task checks it first, so a builder's `pass --from testing` stops before any rebase, gate or
merge; `pass_merged` keeps it too. A builder's `muster stage <id> your-check` (or `done`) is
refused the same way, and so is `muster move <id> done`. The owner always passes, logged `(owner's override: no separate tester)`; a
tester under another label in a builder's session (subagents share their coordinator's session
id) passes, logged `(tested under another label in the builder's session)`. Tasks carry
`builtBy`, `gatedBy` and `testedBy`, and the app counts builders as the core does (the claimer and
the last gated passer). The Testing card says "needs a tester" on a quiet line of its own while a
builder holds it; the Your-check card says "built by … · tested by …", wrapped rather than cut;
the pane's status line says the same from Testing on.
Core in `ops.rs` (`tester_rule`, `built_it`), `queue.rs` (the two fields); app in `lib.ts`,
`CardStatus.tsx`, `PaneHeader.tsx`. PROTOCOL.md (a note, the stages note, the parallel loop,
Sessions), the manual (Concepts, For agents, and Commands from `muster help pass` and `stage`)
and AS_BUILT updated.
**Decided:** a builder is whoever claimed the task last *or* passed it into Testing with the gate
line, so a new session that rebuilt it under the builder's label after a return can't test it
either; an override into Testing (`(set)`) doesn't make anyone a builder. The owner's override is
logged whenever the owner moves a built agent's task past Testing, not only when the builder held
Testing last: if the owner moves it, no other session tested it.
**Verified:** `bin/gate`: `gate: green (pnpm check, test_task, test_commands, cargo test
(Kiritimati))`; cargo test 402 + 4 + 14 + 3 (9 new: the builder refused stamped, unstamped and
after a return and re-pass, nothing written; another label passes and is logged; another label in
the builder's session logged so; the owner's pass and `stage` logged as the override; the builder's
`stage` refused; the builder's merge refused before the rebase, gate or merge with branch, worktree
and main untouched; `pass_merged` refuses it too), all 9 failing with the rule taken out;
vitest 944 (`sameHolder`, `needsTester`, `builtTested`, the cards' and pane's lines);
`test_commands.py` now checks the builder's pass from Testing exits 1 and a QA one 0.
**Fixed (review):** "needs a tester" and "built by … · tested by …" were cut off by the card's
one-line ellipsis in a narrow column → "needs a tester" is its own quiet line and `.card-built` /
`.card-tester` wrap (a CSS test pins it); the app knew only the claimer, so a task gated into
Testing by a session that never claimed it showed no "needs a tester" → `gatedBy` is serialized
and `needsTester`/`builtTested` count both builders like `tester_rule`; `muster move <id> done`
skipped the rule → `move_task` runs it and logs the override like `stage`; the Concepts page's
LIVE sentence is back with the holder paragraph, and the docs say where "built by · tested by"
shows (the Your-check card, the pane from Testing on). Re-verified: `gate: green (pnpm check,
test_task, test_commands, cargo test (Kiritimati))`, cargo test 404, vitest 945.
**Open:** AC6 (the next agent task reaching Your check shows both in its log) is the owner's to
check. Coordinators need to start a QA subagent with its own `--as` for each task in Testing.

## 2026-10-13 — Muster finds queues one level deeper, and About says which it found and which you added (owner)
**Fixed:** Settings › About said Muster finds every `~/playground/*/tasks` queue on its own and
listed four persianUtils queues as "found", while twelve more `~/playground/persianUtils/<x>/tasks`
queues weren't shown at all (owner: "how? it doesn't???") → `queue::discover` looked one level
down only, the four came from `added` in projects.json, and About listed every active queue under
"found" without asking where each came from → `discover` now looks at `<base>/*/tasks` and
`<base>/*/*/tasks` and no further, keeps only folders that pass `projects::looks_like_queue`, skips
`.*`, `_*`, `node_modules`, `worktrees`, `*.worktrees`, `target` and `dist`, follows a symlink only
while it stays inside the base, and finds a queue reached by two paths once. A nested queue is
named by its own folder (`colors`, "-2" on a clash) and `Project` carries its `group`
(`persianUtils`) for task 0044. A queue both found and added shows once, as found. About lists
"Found on their own (~/playground/<project>/tasks and one level deeper)", then "Added by you" (or
"none"), each path through `HomePath` (`~/`, the real path as tooltip and on copy).
`muster projects` marks each line found or added. Projects' intro, the empty board, help.rs, the
manual (02, 03, 04, 05) and AS_BUILT say the same. The `dump`/`probe` examples read
`projects::active`.
**Verified:** `pnpm check` green after the rebase onto a58e10b (tsc, vitest 959, vite build,
cargo fmt, clippy -D warnings, cargo test 398 + 4 + 14 + 3); `python3 -I bin/test_task.py` 48 OK;
`python3 -I bin/test_commands.py` 11 OK. New tests: `discovers_queues_one_level_deeper_and_no_further`,
`discovery_skips_hidden_archived_and_build_folders`, `discovery_stays_inside_the_base_through_symlinks`,
`a_queue_one_level_deeper_is_named_by_its_own_folder_and_keeps_its_group`,
`a_queue_both_found_and_added_shows_once_as_found`, `projects_says_which_queues_were_found_and_which_were_added`,
and About's three. `muster projects` on this Mac: 19 queues, all found (braindump, drover, muster
and 16 persianUtils), `harness/c3-docs/tasks` left out.

## 2026-10-13 — Appearance only in Settings, Help only in the header; paths shown with ~ (owner)
**Did:** the owner: "remove appearance from top header since it is now under the settings", and
why Help was both in Settings and outside it. The header's one-click **Appearance** is gone
(`AppearanceShortcut.tsx` deleted), so Settings › Appearance is the only place; no key opened it,
so no shortcut changed. Settings' **Help** group (links into the manual) is gone too: the
header's Help tab, which every ? opens, is the manual's one place (`settingsData.ts`,
`Settings.tsx`, the unused `settings` help topic and `.settings-links` CSS). Settings shows paths
under the home folder as `~/…` (About, the Agents settings file, Projects' folders), with the
real path in the tooltip and on the clipboard when copied: `homePath` and `copiedPath` in
`lib.ts`, `HomePath.tsx`, and `home` in the core's `settings_info`. Manual (04) and AS_BUILT
updated.
**Verified:** `pnpm check` green (tsc, vitest 949 tests, vite build, cargo fmt, clippy -D
warnings, cargo test 391 + 4 + 14 + 3); new tests: the header has no Appearance button and
Settings still has the group; no Help group, the Help tab still opens the manual; `homePath`
under home, home itself, home with and without a trailing slash, a sibling like
`/Users/someone2` and a path outside home left as is, empty or `/` home; copying all or part of
a shown path; About shows no user name and keeps the full paths as tooltips.
`python3 -I bin/test_task.py` 48 OK, `python3 -I bin/test_commands.py` 11 OK.

## 2026-10-13 — Group by Feature or Goal keeps the backlog notes and the quick add in view (task 0046)
**Fixed:** with Group by on, the backlog notes and the **Add to the backlog…** field disappeared →
they were drawn only in the plain board's Backlog column, and the bands hold tasks, so notes (no
parent) fell out → the grouped board starts with a **Backlog** band ("Backlog · not in a feature
yet", or goal): the quick add, the notes newest first, then the Backlog tasks under no feature or
goal, which leave No feature. Drawn even when empty. The filter bar's hidden count now counts the
notes the search hides on the grouped board too (`hiddenCount` in `Backlog.tsx`). `Groups.tsx`,
`App.tsx` (board only), `styles.css`; the manual (04) and AS_BUILT updated.
**Verified:** `bin/gate`: `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`; vitest
946 tests, 8 new (7 Backlog band renders by feature and by goal, empty, after an add, search marks,
no-match; 1 hidden count), red before the fix. Not checked live in the app: AC4 is the owner's.

## 2026-10-13 — Ping a working session from the app: ask for a check-in, or send a message (task 0039)
**Did:** the owner, on two cards saying "no check-in for 36 min": "how to ping them back from the
app". **Ask for a check-in** under a quiet card, in the task pane's new **Session** strip and, for
a stalled project, **Ask all quiet sessions for a check-in** in the sidebar; **Message the session
working on this** in the pane; `muster ping <id> ["<message>"]` on the command line. Each writes to
the holder's session mailbox (`mail/<session8>/<id>-checkin.md`, `<id>-note-<nnnn>.md`) and logs it;
`bin/answer-hook.py` hands it over after that session's next tool call, check-in requests first
with the hook's own words to answer with `muster checkin`, messages quoted as the owner's note
(500 characters, control characters out). The card says "asked 2 min ago" until a check-in comes.
A holder without a session stamp shows the button disabled with why. Core in `ping.rs` (+
`ping_cmds.rs`), app in `Ping.tsx` and `reach.ts`; PROTOCOL.md, the manual (04, 05, 07) and
AS_BUILT updated.
**Verified:** `pnpm check` green (tsc, vitest 744 tests, vite build, cargo fmt, clippy -D
warnings, cargo test 343 + 13 + 3); `python3 -I bin/test_task.py` 44 OK (4 new: the hook delivers
a check-in request and a quoted message, both with answers, and refuses a claim without a stamp);
`python3 -I bin/test_commands.py` 11 OK.
**Fixed (review):** U+2028/U+2029 in a message could break out of its quote → they become line
breaks in the core and the hook, each part quoted; note-10 was delivered before note-9 → message
files are numbered `0001`…; the pane's maxLength counted UTF-16 units and cut pastes → a live
"N/500" in characters, Send off past it; a check-in request waiting while the task left Building
asked for a `muster checkin` that fails → the hook sets it aside unless the task is still in
Building and claimed by that session.
**Open:** the optional auto-nudge after 45 quiet minutes (off by default) isn't built; it can now
go in Settings → Agents (0036). AC1's live
check in one real session is the owner's.

## 2026-10-13 — Settings: one place for appearance, projects, agents, help and about (task 0036)
**Did:** **Settings** is a view in the header's row: the groups (Appearance, Projects, Agents,
Help, About) docked on the left, the open group's settings on the right (`Settings.tsx`,
`settingsData.ts`, whose `SETTINGS_GROUPS` is where 0038's display modes and 0006's notifications
go). The header's **Appearance** opens it at Appearance in one click and **+ Project** at
Projects; the Appearance pane and the Projects tab are gone, their contents moved in. **Agents**
has the helper's account and its model (Sonnet by default, or Haiku or Opus, in `helper.json`),
the claim lease (12 h, 1 h to 7 d) and the LIVE window (5 min, 1 min to 1 h). Those two are
per machine in `settings.json` beside `projects.json` (`settings.rs`): `muster reclaim` with no
`--after` and `muster check` read it, and the app reads it every poll for Release, the card's
"quiet" and dot, and the pane's LIVE. Missing, broken or out-of-bounds values are the defaults.
**About** shows the version, the settings folder and every folder the app reads. The manual's In
the app page has a Settings section; other pages point there; the Commands page was regenerated.
**Fixed (review round 1):** "1.25h" saved as 72 min → spans went through estimates' one-decimal
cleaning → `settings::span_minutes` reads the raw text, and `reclaim --after` uses it too. Agents
jumped when the accounts loaded → Claims come first. The model chip lit only after a refetch →
it lights when the core saves it. Esc's shortcut text still named Appearance. The CLI unit tests
read this Mac's settings.json → the CLI reads it under the caller's `$HOME` (none in tests). The
queue README template said 12 h → "longer than the lease"; `manifest.py`'s STALE_HOURS stays the
default 12 h, and its comment says so.
**Fitted to main** (rebased onto 0022's rework, 0023, 0042, 0043): the cards' lease and LIVE
window reach 0043's Card and Your turn lines through the same context; Your check's **Open
Appearance** and **Open Projects** open Settings at that group, and a criterion naming Settings
gets **Open Settings**; Settings › Projects shows each project's 0023 gate command, read-only
(the command line stays the one place to set it).
**Decided:** bounds and a settable helper model went to the owner as 0036 Q1 with pick (a); built
as (a) while it waits.
**Verified:** `pnpm check` exit 0 (vitest 748, cargo test 341 + 14 + 3, clippy and fmt clean,
build), `python3 -I bin/test_task.py` OK (40), `python3 -I bin/test_commands.py` OK (11). A test
with a 1-hour lease in `settings.json` makes `muster check` and `muster reclaim` release a claim
quiet for 2 hours that the default 12 h leaves alone.
**Open:** the owner's answer to 0036 Q1; the Settings screen not yet looked at in the running app.

## 2026-10-13 — Your check says what to do: next step, effort groups, Accept all (task 0043)
**Did:** the owner, on 21 tasks in Your check: "not clear what I need to do about it". Each Your
check card now says **Your turn** (the pane too, instead of "on it: you") and its next step, the
first open owner criterion as an instruction ("Try: group the board by Feature and check a goal's
progress") or "Nothing to try: Accept". The Your check column (each band's too) and Waiting for
you group by effort: Just accept, One look, Needs a session (an open criterion mentions a session,
terminal or command; `— effort: look|session` overrides). **Accept all** in Just accept names the
tasks in place, then a second click sends them to a new core command, `accept_all` (lib.rs), which
passes each from Your check as Accept does, logged, and reports what it refused. The pane's
checklist lists open owner criteria first with Tick and a button per view they name. Logic in
`yourCheck.ts`; `EffortGroups.tsx`, `WaitingList.tsx` (moved out of App.tsx); manual's In the app
(new Your check section), Concepts, AS_BUILT, shortcuts page (Esc cancels the confirm).
**Fixed (review round 1):** Accept all could take a goal or feature, or a task with a question
escalated to the owner, which a single Accept never offers → `justAccept` and the Waiting list take
tasks only, an open escalated question or a block on the owner is owner work (One look; the next
step says "Answer Q2: …" or "Unblock: …"), and `accept_all_in` refuses both too. The confirm takes
the focus and its Esc is a window capture listener (never closes the pane). "Your turn" is a filled
chip (orange text was ~2.4:1 on light themes). The result line is kept by place outside the column,
so it outlives an emptied column and goes on the next change; refusals drop the id and `muster` hints.
**Verified:** `pnpm check` exit 0, both Python suites OK (counts in the 0043 pass). The core test
`accept_all_accepts_exactly_the_just_accept_tasks_each_logged` fails when the from-Your-check
check is dropped (a Testing task gets moved). AC5 is the owner's.

## 2026-10-13 — A dependency is met once it's merged: Your check counts, not only Done (task 0042)
**Did:** the owner settled that work merges after Testing and waits in Your check so the next
tasks build on it, but the core still held dependents until Done (0005 couldn't be claimed with
0002 and 0014 merged). Now an agent's task in Your check or Done meets a `depends:`; a human task
still needs Done. One rule in `block.rs` (`met_at`, `holds`, `open_ids`) feeds `claim`/`next`
and `pass` into Building (review: a Pass on the Ready card slipped past it), `muster start` and
`ready --parallel` (0023),
the derived "Blocked by tasks" (card, table, timeline, pane, `muster list`, `check`, INDEX.md's
"waits on"), `set depends`, `block --on <ids>` and the old-block check; `manifest.py` mirrors it
and the Graph's "ready" and blocked badge read the core's verdict (`readyToStart`). Reaching Your
check logs `unblocked: 0002 in Your check` on the tasks it frees (Done after it writes nothing
more). A return from Your check blocks unstarted dependents again and notes started ones
(`0002 went back to Building: <why>`) without moving them; they aren't held by it again. Shared
fixture `app/src/deps-cases.json` holds the CLI (`tests/deps_cases.rs`), the app
(`deps-cases.test.ts`) and `manifest.py` (`test_task.py`) to the same answers. Help text, the
commands page, PROTOCOL.md, VISION.md's spine, AS_BUILT.md, COMMANDS.md and the manual (Concepts,
Tutorials, FAQ) say so.
**Verified:** `pnpm check` green (vitest 849, cargo test 368 + 4 + 13 + 3, clippy, fmt, build);
`python3 -I bin/test_task.py` (41) and `bin/test_commands.py` (11) OK. Mutating `met_at` back to
Done-only fails 3 of the 4 fixture tests.
**Open:** a task under way whose dependency went back can't be blocked on that task again
(refused, saying so).

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
