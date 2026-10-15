# Muster — work log



Newest first. What changed, and how it was checked. The current state is in `AS_BUILT.md`.

## 2026-10-15 — Work section by stage, tokens without cache reads, a token ribbon on held cards (mu-172)
**Did (asghar_33, builder):** a task's tokens are input + output + cache writes
(`Tokens::counted`); cache reads are shown beside them, never counted. The task pane's Work
section is a table by stage: Shaping, Building, Testing and Your check, plus Other when a session
worked while the task was in Backlog, Ready or Done. Each row has its counted tokens, its cache reads
and its time (blocked and paused left out), with the sessions under it, each with its own tokens and
time. A session's turns are split between stages by their timestamps, using per-minute token
buckets the reader keeps with its cached summary. The total row compares counted tokens with
`est-tokens`, and agent time (Building + Testing) with `est-time`. Times are in hours, never days.
Held Shaping, Building and Testing cards wear a token ribbon in the bottom corner ("210k / 600k",
"720k / 600k · over", "· shared"), and the live dot sits in it. The ribbon's count comes from the
poll's `work::mark_tokens`; a count is reused for 30 s. Strings are in en and fa. Files: `work.rs`,
`transcripts.rs`, `queue.rs`, `lib.rs`, `help.rs`, `backlog.rs`, `TokenRibbon.tsx`, `Work.tsx`,
`Card.tsx`, `HolderRibbon.tsx`, `Recent.tsx`, `lib.ts`, `modes.ts`, `styles.css`, the locales,
PROTOCOL 0017, AS_BUILT, DESIGN, `glossary-fa.md`, and the regenerated manual page 05. After the
rebase, mu-98's Sessions recap (`sessions.rs`, `SessionRecap.tsx`) holds counted tokens against the
estimate too, with its cache reads shown as not counted.
**Fixed:** the owner saw "Tokens 168.1M of 1.5M (11204%)" and "Time 7.3 h of 1 d (92%)". There
were three causes:
- every token kind was counted, cache reads included;
- the lead's session, which other tasks name too, was summed in full;
- Time was the sum of each session's transcript span, set against an estimate shown in days.

Now shared sessions are listed and marked, and their tokens show as "+X in shared sessions, not
counted", in each stage and in the total. Time is stage time. The test
`the_owners_fixture_168m_of_1_5m_and_7_3_h_of_1_d_now_reads_1_8m_and_2_5_h` shows both the old
numbers and the new ones.
**Decided:**
- **Shared sessions stay out of the counted sums.** One session working several tasks can't be split
  between them, even inside this task's stage windows. The lead (QA_10) agreed. The alternatives,
  counting the session in full or guessing a split, are both wrong without saying so.
- **The total's time is Building + Testing stage time.** This matches mu-168's
  `measured::agent_minutes` on every task with no release to Ready and no send-back from Your
  check: a test runs mu-168's own function on its cases and gets the same numbers, and asserts
  where the two differ and why.
- **A task's first stage runs from its first log line.** Without this, a task made in Shaping would
  file its shaper's tokens under Other.
- **The ribbon shows on held cards only.** The 17px strip is reserved on every card that can be
  held.
- **On cards under about 110px,** "shared", then the note, then the "·" give way. The figures and
  "over" never shrink.
- **The Persian pair is written «۷۲۰k از ۶۰۰k»,** because the Persian decimal sign beside a slash
  read as a fraction.

**Fixed (review round 1, Asghar_41; `.claude/work/reviews/mu-172/`):**
- R1: a held card waiting for input or ended was still read out as "being used now", and Standard
  and Focus still showed its ribbon as busy. `TokenRibbon` now decides "being spent" once
  (`live && !stateHidesDot`) for the dot, the spoken words and the busy check.
- R2: the stage table's stage names are `<th scope="rowgroup">`, the column heads `scope="col"`,
  so a screen reader says a session's stage with its row.
- R3: the stage-table test has a reply with no time stamp, which must land in "other"; deleting
  the `rest` line fails it.
- R4, R5: AS_BUILT and manual chapters 02, 03 and 04 describe the stage table, the total line,
  shared sessions kept out of the count and the token ribbon.
- Nits: the Sessions recap's share is in the language's digits like its token figures (R6); the
  Work.test fixtures say they're hand-made (R7); `tokenRibbonText` no longer returns the unused
  `line` (R8).
Each fix has a test that failed first; the R1 busy check and R3 each have a mutant that fails.

**Verified:** cargo test 1016 passed; vitest 2005 passed; bin/gate is green (the task has the line).
- **AC5** (release, 10 held cards, one live 30 MB transcript growing between polls, 150 polls 2 s
  apart): a steady poll's `mark_tokens` costs p50 6–8 µs and p95 10–35 µs. A refresh poll, once
  every 30 s, costs 63–115 ms: one re-read of the 30 MB transcript, off the main thread. `mark_live`,
  already in the poll, is 0.3–2.4 ms p50.
- **Headless WebKit:** shots of the board and the Work section, in en and fa, light and dark,
  including narrow cards. In the before/after pair the card is 192.875px high both times, and the
  rows above the ribbon are pixel-identical.

**Open:** a refresh re-reads the whole live transcript. Reading only what was appended would remove
the 30 s spike; there's a backlog note for it. The new fa words (بیش, بی‌تخمین, روبان توکن) want a
native check. The Work progress guess's basis is still English from the core. On a 100px card that
is live while nothing could be read (a transcript over 128 MB), the dot is the part that gives way;
the spoken words and the tooltip still say it's being used now.
## 2026-10-15 — docs/now/claims.md split into claims.md and handoffs.md, each under 3 KB (mu-208)
**Did (Akbar_40, builder):** `docs/now/claims.md` ("Claims": claim, stamps, slots, lease,
reclaim, pause, release, takeover; 19 rules, 2,037 bytes) and the new `docs/now/handoffs.md`
("Sessions, handoffs and mail": decisions 0004 and 0011, check-ins, mail and the answer hook,
session states and names; 20 rules, 2,121 bytes). Every rule moved word for word with its
citation. claims.md covers session.rs and pause.rs, handoffs.md session.rs, ping.rs, agents.rs and
answer-hook.py: the union is the old page's five files. AS_BUILT's "What's true now" links all
ten pages; `tests/now_pages.rs` pins handoffs among the eight areas.
**Decided:** shapour's split as written; the takeover's rules (who may take over, the wait, mail
it brings) stay on claims, the mail mover's own rules go on handoffs.
**Verified:** sorted bullet lines and cited ids identical before and after; `muster check` says
nothing about `docs/now/`; the gate's line is in the task's pass.

## 2026-10-15 — Test queues get a private parent, so a checkout in the shared TMPDIR changes no test (mu-191)
**Fixed:** 17 review tests went red whenever another session's Muster checkout sat in the shared
`$TMPDIR` → a test queue was a bare `tempfile::tempdir()`, so `backlog::project_places` took its
siblings as the project's checkouts and their `docs/now/` refused the pass → new `cfg(test)`
`testkit::tempdir()` (`<tmp>/proj/q`, the parent holds nothing else); every lib test switched to it,
`project_places` unchanged. `cli` block test pins its prefix (`tm`) instead of reading it off the
temp folder's name.
**Fixed (review R1):** the integration tests still made bare temp folders, and the `muster check`
tests in `interop.rs` and `deps_cases.rs` read sibling checkouts' now pages → `tests/common/mod.rs`
gives them the same `<tmp>/proj/q`, both check tests assert no now page is read, and the source
check covers `tests/` (recursively, both helpers exempt).
**Verified:** `cargo test` hermetic with decoy checkouts in TMPDIR (one now page covering app/,
docs/, bin/; one with no `covers:`): 1045 lib + 42 integration, all green, same counts as without
them (main: 17 review tests red). Mutants: the tests' helper made bare → both check tests red on
the decoy's warning; a `tempfile` temp folder added to `deps_cases.rs` → the source check red.

## 2026-10-15 — merge-log merges new entries at any number of points, not only one (mu-196)
**Fixed (Akbar_36, builder):** mu-173's rebase stopped on `docs/WORKLOG.md` → its log had round
entries on top and its first entry deeper, two runs on one side, and `inserted` took only one →
`inserted` still reads a one-run side as mu-190 did (the same point), else matches the base's
entries from the bottom in one pass and returns every run, each
with the points it can slide to over repeated neighbours; `both_inserted` pairs the two sides'
runs per point in one pass and keeps an entry both added once, at ours' point, each entry
max(ours, theirs) times (`app/src-tauri/src/merge_log.rs`). Where merge-file merges cleanly
(the same entry at two points that don't touch) its result stands, with the entry twice. The give-up message is now "both sides changed more than new
dated entries; left the conflict markers".
**Decided:** decision 0009 amended a second time ("any number of points"), as shaped; repeats on
one side are counted, so a side's own duplicate entries stay.
**Verified:** tests first (7 red, then green), `cargo test merge_log` 30 passed: the mu-173 shape
both ways, per-point merges, refusals beside multi-point runs through the binary, CRLF and a
missing last line end, real repos merged and rebased both ways, 150 fixed-seed random cases through
the binary. The real repro (O=d8ac722, A=ed2474c, B=cf5f59e) exits 0 with 163 entries in QA_10's
hand-merged order (fe231be), plus one blank line the hand merge had dropped from theirs' entry.
An independent review found a one-run side with a copied base entry read as two runs (different
bytes from mu-190); fixed test-first.
**Fixed (Akbar_39, review round 1, Reviewer_77):** R1 the real-repo test's git and driver run in
a home of their own with no MUSTER_* (`home_g`); R2 the randomised check asserts max(ours,
theirs) copies where the driver made the text (the keep-theirs'-copy mutant now fails it); R3 and
R4 AS_BUILT, 0009, help and manual say what the merge keeps, and a test pins merge-file's clean
case with the entry twice. `cargo test merge_log` 31 passed. `gate: green (pnpm check, test_task, test_commands, cargo test
(Kiritimati))`.

## 2026-10-15 — Builders' briefs say how to undo a bad rebase; only the lead restarts a stalled session (mu-193)
**Did (Gholi_39, builder):** `muster instructions coder` and `tester` print the git rule
(`GIT_RULE` in `roles.rs`): never `reset --hard`, `checkout --`, `restore` over changes or
`clean` in a worktree; `git rebase --abort`, then rebase again; squash only with `reset --soft`.
PROTOCOL "Sessions" has the same rule and the lead's 10-minute rule for a session waiting for
input; `docs/now/claims.md` has its line, AS_BUILT the briefs' rule.
**Decided:** brief and PROTOCOL text only, no command or timer (shapour's call in the task); a
timed alarm is mu-118's dispatcher.
**Verified:** `instructions_print_the_roles_brief` red, then green (coder and tester carry the
rule, the shaper doesn't); the gate's line is in the task's pass.

## 2026-10-15 — mu-195 Settings › Agents › Roles shows each role's title in the language shown, not its id
**Did (Akbar_37, builder):** a Roles row reads "Kaveh  برنامه‌نویس · ۳ جایگاه" in fa, "Kaveh  Coder ·
3 slots" in en (`roleTitle` in `rolesData.ts`, shell.json `roles.title.<id>` for the eleven built-in
roles); the opened profile gains "Claim with `--role coder`" in `<code dir="ltr">`, and its line
starts with the same title. Glossary rows for architect, designer, coder, researcher and the three
reviewers and release role, (proposed) where no source had them.
**Fixed:** the first build compared a role's title with the English file at run time; the window
loads only the language shown, so in fa every title stayed English. The headless shot caught it;
the Vitest didn't (tests load every language).
**Decided:** a built-in role is translated only while its title is still the core's default
(`BUILT_IN_TITLES`, `i18n-ignore`d as compared data); a title changed by hand in roles.toml shows as
written. A Vitest parses `roles.rs` so the table can't drift from the core or the English file.
**Verified:** Roles, shellFa and i18n tests (98 passed). Headless WebKit, mocked IPC, fa and en,
light and dark, rows and the researcher's profile open: every fa row Persian, `--role researcher`
left to right. Shots in `attachments/0195/`.
**Round 2 (Akbar_38, review R1 and the lead's scope add):** a Vitest loads a fresh i18n layer with
only the fa files (as the fa window does) and expects the Persian titles; the reviewer's mutant
(roleTitle comparing with `translate("en", …)`) now fails it, where all 31 tests passed before.
The opened profile's line says stage and moves as the board does (`stageLabel`), the permission as
shell.json `roles.permission.<id>` (new glossary rows), a move as `roles.move` (← in fa); unknown
values as written. Headless shots of the coder's profile open, fa and en, light and dark:
`attachments/0195/mu195-r2-*`.
## 2026-10-15 — Spike: the owner's six flows through the app, walked headless (mu-106)
**Did (pouya_4, researcher):** `docs/research/ux-flows.md`: F1–F6 (what's waiting, answer, check
and accept, what's running, start/pause/ping/file, find), each a step table with what's visible and
what's hidden, clicks and keys, and the Focus differences; 19 findings ranked; proposals checked
against DESIGN.md. Captured with the lead's headless harness (Playwright WebKit, mocked IPC) on a
synthetic fixture with the queue's shape and none of its text: 160 shots in light and dark at 1100
and 1440 px, ADHD for all six flows and Focus for F1 and F3 (the other flows' Focus lines are code
reads), plus a Persian pass and keyboard walks, in `.muster/attachments/0106/ux/`. The findings
filed as notes became mu-197 to mu-207; the table names each.
**Decided:** the worst three are Your check past the board's right edge (R4), the board vanishing
when the pane opens (R5) and counts without words or with two meanings ("N working", R3). 11 new
backlog notes cite mu-106. 8 findings went as notes to mu-169, mu-172, mu-184, mu-98, mu-138,
mu-142, mu-144 and mu-149. No code changed.
**Verified:** the six `## F` headings and their Hidden columns; every finding row names a task or a
note file present in `.muster/backlog/`; the owner's projects.json, settings.json, roles.toml and
helper/ hash the same before and after (notified.json is written by the running Muster.app); the
hash lists name files relative to the config folder, and no attachment holds the user name or a
real project name (grep, plus `leakcheck.py` on the fixtures and notes).
**Open:** native surfaces (menu bar, Dock, notifications) are from a code read; R12's timing needs
/profile on a release build; contrast wasn't measured.
## 2026-10-15 — mu-194: the filter bar's Type row says task, feature and idea in Persian too
**Fixed (Asghar_37, builder):** in فارسی the Type row showed "idea" and "feature" in English →
`kindChip` translated only the core's `KINDS` → `fields.ts` has `LEGACY_KINDS` (task, feature,
idea) and one `kindLabel` that says both lists through `common.json` `kind.*` and anything else as
written; the Type group uses it. en and fa get `kind.task/feature/idea`; the glossary gets idea →
ایده. New task and the pane still offer only `KINDS`.
**Verified:** fields and filters tests (kindLabel in en and fa, unknown types as written, the
rendered Type row in fa). Headless WebKit, fa, night and day, filter bar open: the row reads
باگ … قابلیت ایده … تسک, no wrapped chip, no page errors (`attachments/0194/`).
**Open:** the core already reads `type: task` as code (`ops::read_kind`), so the window only meets idea (34 tasks)
and feature (2) today; `kind.task` is there for any path that passes it through.

## 2026-10-15 — Working is a ribbon, not a tag: the holder ribbon says the session's state (mu-171)
**Did (Gholi_31, builder):** the owner: "working needs a ribbon not a tag". A held card's ribbon
now reads "Asghar_7 · working" (accent), "Asghar_7 · session ended" (Building only) or the name
then a needs-you block "waiting for input" to the card's edge; unknown and paused say nothing.
Words from `sessionState.ts` `ribbonStateWords` (full and short); read out with `sessionWords`'
full words. The `.card-session` line under the title and its CSS are gone; the pane's
`.session-state` stays. `agents.rs` `stamp_of` takes Shaping and `holds` counts Shaping claims,
so a shaper shows a state. The card leaves out its "needs you" badge when the ribbon says it, so
a poll that brings waiting for input moves nothing on the card. The modes' says texts (en and fa `shell.json`, mu-146), DESIGN,
AS_BUILT, PROTOCOL, manual 02 and 04, docs/now/board.md and claims.md.
**Decided:** the brief's V1 resolution over Details item 2: the name stays neutral and only the
state is coloured (waiting for input a needs-you block from after the name to the edge, no "·",
which also saves the room "needs you" needs at 106px). Full or short words are picked by CSS: the
ribbon sits in a `.holder-room` (the strip less 8px) that is a size container, with px widths
measured in headless WebKit (128px, 122px; a query's `ch` isn't the ribbon's there). Short shows
where the full words and 5ch of name (about four letters and the …) don't fit, so the owner's
106px cards keep four letters; the name still gives way to 4ch, never the words. The live dot
goes beside waiting for input and ended (Claude Code's word wins, as the sidebar's working now
judges it) and where working and it don't fit (under 101px, the owner's 106px cards). Beside a
state, quiet gives way first; beside waiting for input it's in the tooltip and flag line only.
**Verified:** vitest HolderRibbon, CardStatus, sessionState, themes, modes; `cargo test agents::`
(hermetic HOME) with two new tests; headless WebKit (mocked IPC, mu-97's harness) on the built
app: 96 stated ribbons over ADHD/Standard/Focus × dark/light × 1400/2000 and RTL, every state's
words whole, none past its room, name ≥ 4ch; a working → waiting for input poll leaves every
line's box on the card identical. Gate below.
**Fixed (review round 1, R1; Gholi_36):** at the owner's 1400px window a ribbon with a state
cut the name to about four letters, so asghar_16 and asghar_13 read the same (mu-97's AC4) →
the name gave way from its end, where the number is → `HolderRibbon.tsx` `nameEnds` splits a
name before the number it ends in (any script's digits, the last three at most); only the head
ends in "…", the number never shrinks (`.holder-name.tail-N` floor: 1.5ch for the … plus a ch a
digit), and a screen reader hears the name whole. The needs-you block's start padding went 4px →
3px, so 106px cards show "gh…22 needs you". Headless WebKit, 286 stated ribbons over
ADHD/Standard/Focus × dark/light × 1400/820/2000 px and RTL at 1400/820, holders sharing a
prefix and Persian names (قلی_۳۱): every number and state word whole, at least four characters
of the name; 1400px reads "asg…16 · working" vs "asg…13", "gh…22" vs "gh…18 needs you", 100px
cards "as…16", "g…22 needs you".
**Open:** at the narrowest (100px) cards a waiting-for-input ribbon keeps one letter and the
number ("g…22"); the head's "…" can leave up to a letter's gap before the number ("asgh… 15").
The container widths were measured for the English words; mu-147 measures again. A Shaping claim now counts as one of its session's holds, so a session
holding two (a shaper on two tasks, a builder also shaping) shows no state on either, by 0048's
"holds nothing else" rule. Card.tsx, ProjectList.test.ts (its no-`@container` check now means the
sidebar's), cli.rs (a comment) and docs outside the touches list changed.

## 2026-10-15 — The shaping brief shows estimates measured on the queue (mu-168)
**Did (Akbar_28, builder):** the owner asked whether estimates are any good; 80+ finished tasks
took p50 0.39x their estimate. New pure `measured.rs`: from each task's stage history, agent wall
clock (first move into Building → last move from Testing to Your check or Done) against
`est-time`, as p50/p80 of actual ÷ estimate (nearest rank) and median hours, overall, by type and
by size (S ≤ 1h, M ≤ 3h, L ≤ 8h, XL); under 5 tasks "too few (n)"; the owner's wait in Your check
apart. `muster shape` prints it as "Estimates, measured on this queue" before "File it", with the
rule that `--time` is agent time and `--test-time` the QA agent plus the owner's check; `--json`
has `measured`; the Tech role points at it; `help shape`/`file`/`new`, PROTOCOL, manual 05 and 07,
AS_BUILT, docs/now/shaping.md.
**Fixed (Akbar_31, review round 1):** R1-R3 had no test that could fail. New tests: inexact
ratios (70-100 min on 3h) whose `--json` and text both say 0.47/0.53/1.4 (fail without the
rounding); an owner's override Building → Your check counts for the wait, not agent time; types
with the same count come by name. Each was checked by breaking the code it pins.
**Fixed (Akbar_34, review round 2):** R4: the app's Agent time and Test time tooltips, in the
task pane and New task, still said test time was "the QA agent plus your check". They now come
from the i18n layer (common `estimateField`, en and fa): agent time includes review and QA, test
time is your check only. The doc comments in Sidebar.tsx, types.ts, ops.rs and queue.rs say the
same. R5: a block in Testing unblocked after the move to Your check counts only to the span's
end (30 min, not 90); the test fails with the clamp removed.
**Decided (lead, 2026-10-15):** `--time` is agent time from the first claim to leaving Testing,
review rounds and QA included, less the stretches blocked or paused (`block_history`,
`pause_history`, overlaps once, an open one to the span's end); `--test-time` is the owner's
check only. The rule line, the block's heading, help new/file/set/shape, the helper prompt,
PROTOCOL, AS_BUILT, manual 02/05/07 and docs/now/shaping.md say so; a test covers a blocked and
paused stretch, a block from before the claim and one left open. On a read-only copy of the queue
today: 99 finished, 0.48x at p50, 1.15x at p80, median 1.2 h; the same copy without the
subtraction gives 0.48x and 1.17x (the 0.43x of round 1 moved with the queue, not the change).
**Decided:** the owner's wait is computed live ("median 2.5 h on this queue") instead of the
brief's fixed "4.2 h, measured 2026-10-14", which would go stale at once. Numbers are rounded in
the measure (2 places, hours 1), so the text and `--json` say the same.
**Verified:** on Muster's queue (read-only `shape --peek`): 97 finished, 0.43x at p50, 1.17x at
p80, median 1.2 h; owner wait 2.5 h; matches an independent script over `muster list --json`.
500 tasks measured in 0.87 ms (debug). A change review found two span errors, both fixed with
tests: a Done reached from Testing after a send-back counted the rebuild as the owner's wait, and a
task sent back from Your check counted as finished. Gate green.

## 2026-10-15 — mu-143 round 2: the Layout chips and the grid's rows choice in Persian
**Fixed:** in Persian the Layout chips read "Columns" / "Feature grid" and the grid's rows choice
"By feature" / "By goal" / "No grouping" → `LAYOUTS` names come from `shell.json`
(`app.layouts.*`, read each time through a getter), and the grid reuses `GROUP_BYS`'s words, so
its rows choice and corner say Feature · Goal · None, قابلیت · هدف · هیچ‌کدام (R11). Glossary:
ستون‌ها / شبکه‌ی قابلیت‌ها (proposed).
**Verified:** a Vitest drawing the Layout chips and the grid in fa; `bin/gate` result in the pass.

## 2026-10-15 — mu-143 round 1: the Feature grid's off-edge cue, focus clear of the sticky heads, and real tests
**Fixed:** at the default 1400px window with Recently finished docked, Your check and Done sat past
the frame's end with no cue → the board's "+2 more · 1 needs you" cue at the end of the alert's
line (`rowCue.ts` walk, `watchGrid` measures), stages 140px at least (was 156) and Done as wide as
its words (R1). Arrow-key focus could land under the sticky feature names or header row →
scroll-padding on the frame from `--fg-names` and the header row's height (R2). In All projects
the row title took its direction from the project label → `<bdi dir="auto">` (R3). Column heads'
who line cut at its start in right-to-left → `dir="auto"`, out of the plaintext rule (R7). The Tab
stop was a row number and drifted on a poll → kept by row id (R10). The trouble chip wrapped in a
narrow column → one line, ellipsis, whole in its tooltip.
**Did:** Only what needs me is App's state (`mine`/`onMine`), so a render test can draw it on.
`docs/now/board.md` has the grid's rules. Role names (0112) landed on main: the grid's who lines
and cards name a role slot by its role's name now (`RoleNamesContext`), which settles round 0's
"holders' names, since mu-112 isn't merged". With the i18n layer (mu-146) on main, App's Layout
chips and the grid's "All projects" title are `t()` keys in en and fa `shell.json`;
`FeatureGrid.tsx` and `featureGridData.ts` join mu-147's list in the literals guard, with the
rest of the board's text.
**Verified:** new Vitest cases for the dimming, the done-today row, the full head's `fg-full` and
the header's `fg-needs`, three names, the cue's columns and view span, the Tab stop by id; 23 Vitest
mutants and 9 e2e mutants (one per fix) killed but R8's wrap, which Done's max-content track rules
out; `node e2e/shot.mjs` at 1400×880: 24 checks green (the cue, focus clear of the heads, Only what
needs me, the full Your check head in the shot; right-to-left now as the window in Persian, since
mu-146 sets the page's direction from the language; `.json` served as JSON for its locales);
`bin/gate` result in the pass.
**Open:** in right-to-left the cue's words reorder ("needs you 1 · more 2+"), as on the board's
row of folded columns; the grid's words move into the layer with mu-147.

## 2026-10-14 — mu-143: Feature grid, the board's second layout (features down, stages across), beside the columns
**Did:** **Layout** Columns · Feature grid in the board's filter bar, kept per project on this Mac
(`muster.layout.<project>`). The grid (`FeatureGrid.tsx`, model `featureGridData.ts`): a row per
feature in flight then Other, the stages across with who works each, Done as today's count, two
small cards a cell then +N more (expands in place), the header line, Only what needs me, and one
alert line when Your check is full; a card opens the task pane beside it. ARIA grid with arrow
keys (`shortcuts.ts`), logical CSS for right-to-left, the app's tokens. Columns unchanged.
Headless harness for this and later tasks: `app/e2e/shot.mjs` and `mock.mjs`.
**Fixed (the mockup's rough edges):** trouble chip squeezed the holder and key → its own line;
"Done today" head wrapped → "Done · N today"; "None" in every empty cell → blank cells with their
outline. Also with real data: long "After mu-112, mu-113…" waits squeezed the key → their own line;
long titles → three lines, whole in the tooltip. From the review: "+N more" and "Show fewer" were
two buttons, so pressing one from the keys dropped the focus (and the next Esc closed the pane) →
one toggle button; a feature the search matched lost its row → kept; folding Recently finished in
the grid left no chip to reopen it → the row shows its chip, as under Group by; every card was a
Tab stop → the grid is one, the keys reach the cards and feature names.
**Decided:** the grid's rows follow the board's Group by (By feature · By goal · No grouping in the
grid's header, which takes the filter bar's Group by chips' place while the grid shows: one
control). Who works a stage is the holders' names from the tasks (the role when none hold any),
since role profiles (mu-112) aren't merged. "Full" is at the limit, not over it, and the alert
says to check some, not that Muster stops: the limit only warns (0050). Trouble is error red,
not the mockup's amber, which means Testing here. The grid is the same in every display mode.
**Verified:** Vitest 1816 tests (`featureGridData.test.ts` 42, `FeatureGrid.test.tsx` 24, the
existing guards updated where they pin App's source); `node e2e/shot.mjs`: 18 headless checks green
(grid roles, +N more in place by mouse and keys, card opens the pane, layout kept, arrows, one Tab
stop, RTL mirrored, Columns unchanged plain and grouped, the full alert); an independent review
(correctness lens): 4 should-fix findings, all fixed; screenshots light, dark, RTL, Columns in the task's
attachments; `bin/gate` result in the pass.
**Open:** in right-to-left, English column-head names cut at the wrong end (the app isn't
localised yet); the harness needs local port binding, which this sandbox turns off.

## 2026-10-15 — mu-131 round 4 settled (back to Building); notes shaped (mu-187, mu-188)
**Decided:** mu-131's round 4 (past the 3-round cap) found R13 (restart never writes gates:), R14 (hand-merge
## 2026-10-15 — The card's "needs a tester" and "built by" count the takeover chain (mu-186)
**Did (gholi_37, builder):** `queue::Task.builders` goes to the window (`builders`, no longer
`serde(skip)`), `Task.builders?` in `types.ts`, and `lib.ts` `builders()` returns the chain, then
`builtBy` and `gatedBy`. The card and pane read "built by wt-a and wt-b" after a takeover, "wt-a,
wt-b and wt-c" for three.
**Fixed:** a Testing task held by a takeover's predecessor showed no "needs a tester", though the
core refuses that holder's pass → the app knew only `builtBy` (the chain's last) and `gatedBy` →
it now checks every chain member with `sameHolder`. `needsTester` checks each builder without
folding look-alikes first, since a bare label matches any stamp and the match isn't transitive;
only the "built by" line names each holder once (`builderNames`).
**Verified:** Vitest `lib.test.ts` 55 passed, with cases from `ops.rs`
`tester_rule_refuses_a_successor_and_its_predecessor`; a mutant that drops the chain fails 2 of
them. `cargo test --lib builders` (hermetic HOME) 4 passed, with the new
`builders_go_to_the_window_as_the_chain_reads_them`. `bin/gate`: green (pnpm check, test_task,
test_commands, cargo test (Kiritimati)).

## 2026-10-15 — Release a claim no session can be reached on; holders' acts stamp bare claims (mu-184)
**Did (Asghar_30, builder; round 1 fixes Asghar_34):** the owner's "this claim has no session to
reach … we need a button". The pane's Next step shows **Release** on a Building claim or Testing
hold that names no session, whatever the lease, with its confirm in place (role=group "Confirm
Release", focus on its Release so ↩ confirms, Esc/Cancel back to Release; `Sidebar.tsx`, words in
`paneData.ts`). Core `ops::release_unreached` (owner only, re-read under the lock): Building →
Ready, worktree and branch kept, `(released: no session to reach)`; Testing clears `held-by:`. A
session-stamped holder's checkin, heartbeat, note, pass, block or `claim --takeover` on a bare
claim (or its own session under another account, the owner's 23:26 comment) re-stamps it, logged
once `took over the claim from <old> (stamped)` (`session::restamped`, `ops::restamp`). Send-backs
give Building to the builder's exact stamp; while that session holds no other unblocked Building
task and hasn't acted on it, `next` points a session under its label there and `--takeover` is
allowed at once `(sent back, not picked up)`, so an ended builder doesn't strand it. NO_SESSION
reads "no session to reach: Release it, or let a new session under <label> take it over" (a
Testing hold: "no session to reach: Release it"), the same in `ping.rs` and `reach.ts`. PROTOCOL
"Sessions", DESIGN, manual 04/05/06, AS_BUILT, now pages claims, queue, board.
**Fixed (review round 1, Reviewer_58):** R1 a send-back to a builder live on another unblocked
task could be taken over at once → `untouched_send_back` ignored what else the holder held → it
now needs `holding(..)` to be none, else 0090 applies. R2 a check-in or takeover in the
send-back's own minute still read as "not picked up" → minute stamps compared → the log decides
(`acted_since_send_back`: a takeover or any line of the holder's after the send-back's stage
line). R3 `next` pointed siblings at a paused send-back whose takeover is refused → not paused is
part of the rule. R4 one Esc on the confirm also armed an unsaved write-up's or comment's discard
→ every window capture listener runs → the group carries `data-own-esc` and `EditBody`/`Comments`
leave an Esc inside it alone (`ownEscape.ts`); the group holds the focus while the release runs.
R5 the Esc and focus wiring wasn't pinned → `watchReleaseEscape` is driven by real events on an
`EventTarget` in the test, and the component's use of it and of the focus moves is pinned by a
`?raw` check. R6 focus fell to `<body>` after a release → it moves to the Next step group. R7
Testing's NO_SESSION offered an impossible takeover → `NO_SESSION_HELD`. R8 PROTOCOL/AS_BUILT
listed `return` as re-stamping → dropped. R9 stale comments in `return_change`, `builder_mailbox`
and `help claim` → reworded. R10 the confirm's question was built from parts → one string per
sentence with `{{who}}`.
**Decided:** With mu-156 merged, `claim --takeover` run again on a claim the session re-stamped is
mu-156's rerun ("you hold it already; its old holder had no mailbox, so there's nothing to move"),
not "you already hold it"; the same session under another account has one mailbox, so its rerun
moves nothing (pinned by a test). Release sits in the Next step (the pane's action row, `.release` style, as the brief's
Architecture line says), not in the Session strip; the Ping strip's reason names it. Testing's
release writes no stage line (it doesn't change stage): `let go of <holder>'s hold; still in
Testing (released: no session to reach)`. When the lease also ran out, only this Release shows.
R5's test isn't a rendered-DOM test: the app has no DOM library (jsdom/happy-dom, or Playwright
as a dependency) and adding one needs the owner's yes; real events on Node's `EventTarget` plus a
source pin is what the suite can run. A heartbeat in the send-back's own minute writes no log line,
so it alone doesn't count as picked up (one in a later minute does).
**Verified:** `bin/gate`: green (pnpm check, test_task, test_commands, cargo test (Kiritimati)), rebased on mu-98, mu-156 and mu-190. Mutants, each killed: R1 (no `holding` check), R2 (no log check), R3 (no
paused check), R4 (EditBody without `escOwned`; `escOwned` always false; group without the mark),
R5 (no listener; no focus back), R6 (no focus on the Next step), R7 (Rust and TS without the
Testing words). Round 1: headless WebKit shots of the pane before/after Release, light and dark.
**Open:** فارسی shot: mu-146 has merged, but the pane's strings are still English until mu-148
moves them. mu-185 keeps the Testing `held-by:` re-stamp.

## 2026-10-15 — mu-173 review round 2: the legend keeps its height, the Graph's top outline whole
**Fixed (Gholi_34, builder):** R4: the wrapping legend was as tall as the window's marks, so
stepping from an 11-mark week to a 4-mark one moved the chart up 24px → its height followed the
items present → a hidden copy of every mark the view can draw now shares the list's grid cell
(`.legend-reserve`, aria-hidden, nothing focusable), so the height follows the width only. R5: a
selected top node's outline was cut at the Graph scroller's edge → `.graph-scroll` had no top
padding → 6px of top padding, taken off the legend's bottom margin.
**Did:** the legend's text moves into the i18n layer from mu-146 (`board:legend.*`, English and
Persian from the glossary), so `Legend.tsx` stays off the guard's list.
**Verified:** Legend, graph, Views and i18n tests (127 passed). Headless WebKit at 1000×640
(sidebar 210), Week, Month and Day, 4 windows each, 0–11 items, LTR and RTL: legend h 44, head y
164, first row y 192 in every window. Graph, one project, top node selected: outline top 152, inside
the scroller's 150. Shots in `attachments/0173/r2-*.png`.

## 2026-10-15 — mu-173 review round 1: one Done look, the Graph's legend stays in view, the legend wraps
**Fixed (Gholi_32, builder):** R1: a task done before stage history drew a solid green bar while
the Done swatch is an outline. `taskSpan` no longer gives it a done segment: no stage of it was
logged, so the green fill claimed time "in Done" the data doesn't hold. Every done bar is now the
outline. R2: the Graph's bar and legend move to `.graph-strip`, outside a new `.graph-scroll`. R3:
the legend wraps onto a second line instead of scrolling sideways with a hidden scrollbar.
**Decided:** R1 one look, not two swatches, because it's truer to the data. R3 wrap was the lead's
call, from the owner's "out of sight is out of mind" rule.
**Verified:** tests first (7 red, then green), full app suite 1912 passed. Headless WebKit at
1000×640 (sidebar 210): Timeline Week with 11 legend items wraps to 2 rows, 0 items outside, LTR
and RTL; Graph with 4 queues scrolled 2412px, legend top 45px before and after. Shots in
`attachments/0173/r1-*.png`.
## 2026-10-15 — Backlog shaped: mu-189 to mu-195, two notes folded into mu-147; mu-131 stalls on a gate check
**Did (shapour):** 0034 → mu-189 (hand merge re-checks review and gate records under the lock, after 0131).
0038 → mu-190 (high: merge-log keeps both sides' entries inserted at one point below the top; amends
decision 0009). Three tmpdir notes → mu-191 (high: test queues get a private parent, so a stray checkout
in the shared TMPDIR can't turn 17 review tests red). 0050 → mu-192 (migrate --dry-run reads only).
0054 → mu-193 (high: briefs forbid reset --hard; the lead, and only the lead, restarts a session waiting
over 10 minutes), mu-194 and mu-195 (fa labels). 0037 and 0148 folded into mu-147 as AC8 and AC9.
**Fixed:** mu-131's rebase stopped on docs/WORKLOG.md → muster merge-log only merges entries added on the
very top, and a coordinator log entry on main had put both sides' entries below it → filed as mu-190; until
it lands, this session commits its log on vqa and fast-forwards main less often.
**Decided:** mu-131's round 5 approved it; its re-gate after the rebase is red on broken_pipe's
cli_rs_has_no_bare_print (three print! in the gate commands, cli.rs 2901/3077/3088). No builder alive and
kia-131 on a permission prompt: reported to the lead QA_10, who owns sessions (owner's rule 2026-10-15),
rather than resuming one.
**Open:** mu-131 needs the three-line fix, a re-gate and the merge; mu-187 and mu-189 wait on it.

## 2026-10-15 — merge-log keeps both sides' entries inserted at one point, not only on top (mu-190)
**Fixed:** a merge or rebase stopped on `docs/WORKLOG.md` whenever both branches put entries under
main's newest (later-dated) entry → `added_on_top` accepted only entries before all of the base's →
`inserted` finds the one point a side's run of dated entries sits at, and `both_inserted` merges both
runs there (newest first, theirs above ours on a date). `merge_log.rs`, help text and manual 05.
**Decided:** decision 0009 amended (dated, links mu-190): same point only; different points stay
merge-file's, which merges them itself when they don't touch.
**Fixed (own review):** a run that repeats its neighbour fits several points as a different turn at
each; it was taken at the first and put at the shared one, reordering that side → sliced at the
point chosen, with a test.
**Verified:** `cargo test merge_log` 24 passed (hermetic HOME), incl. real repos merged and rebased in
both orders; the real repro (f4a792a^ / 2de282d / f4a792a) exits 0 with mu-131's four entries above
mu-146's, where the installed muster exits 1. `gate: green (pnpm check, test_task, test_commands,
cargo test (Kiritimati))`.
**Open:** with the shared sandbox TMPDIR (/tmp/claude-501), 17 `review::tests` fail: `now::places`
reads repos beside the test's temp queue, i.e. other sessions' muster checkouts with `docs/now/`.
The gate is green with a private TMPDIR; backlog note filed.

## 2026-10-15 — mu-98 review round 2: the transcript view's list check and `listed`'s allowlist filter get tests that can fail
**Fixed:** R11 nothing failed with `transcript_for`'s list check or `Lister::listed`'s allowlist
filter removed (the refused id had no transcript at all) → the refusal test now writes an unlisted
company transcript under `.claude`, read through `.claude-hg/projects`' link, and checks it's
readable yet refused; a new test covers an account taken off the allowlist after a read (its
sessions and a chain successor of it are left out) and a session a later read dropped.
**Did (rebase on main):** mu-146's i18n layer: the Sessions pill, its loading text, its help
topic and the two Display rows (Sessions, Session history) go through the layer with fa (نشست‌ها,
the glossary's word); `SessionRecap.tsx` and `sessionsData.ts` go on the guard's allowlist for
mu-147, which moves the sessions screens.
**Verified:** both mutants killed: list check off (2 tests red), `listed` filter off (1 red).

## 2026-10-15 — mu-98 review round 1: the chain reads only vouched predecessors, and nine more fixes
**Fixed:** R1 a recap's handoff chain read a company-account predecessor's transcript through
`~/.claude-hg/projects` (a link to `~/.claude/projects`) → `chain` and `recap` read a predecessor
only when its account's list has it or a task stamps it with an allowed account; otherwise it's a
link by id, "not read". R2 a recap open on another view never polled the Lister → `recap_for` polls
it. R3 one `date` spawn per log line in each recap → `queue::local_epoch` (`mktime`, in-process).
R4 a pending Bash prompt showed its description → `pending_gist` puts the command first. R5 the
row's aria-label under All names the project. R6 agents.rs header per decision 0017. R7
`plain_line` drops LRM, RLM and ALM. R8 a list older than 40 s reads as "Reading sessions…".
R9 Focus chips show no count while reading. R10 the recap uses the app's warm queue cache.
**Verified:** a mutant per fix, each killed except `isdst: 0` in `local_epoch`, which is
equivalent on macOS (its mktime gives the same answer). The gate line is in the pass.
## 2026-10-14 — Timeline and Graph legends: a swatch row naming each colour and mark on screen (mu-173)
**Did (gholi_28, builder):** new `Legend.tsx` (one `LEGEND_LABELS` table, swatches built from the
views' own classes by a closed switch). The Timeline's legend is the scale strip's second line
(`.tl-strip`, outside the toolbar and `.tl-scroll`); round 1 made it wrap (above). `legendMarks` (`viewData.ts`) and `graphMarks` + `isLoopEdge` (`graph.ts`)
list only what's drawn, in a fixed order. The Graph lifts `layoutGraph` into `GraphView`, and its
loop arrows and legend share `isLoopEdge`. The paragraph under the chart lost its colour words.
Manual 03 and 04, AS_BUILT and a new `docs/now/views.md` describe it.
**Decided:** the Graph's "Your own task" swatch is the You ribbon, since graph nodes have no gold
hatching.
**Verified:** gate green. Unit and render tests cover AC1 and AC2. Headless WebKit shots on
synthetic data (40 tasks, Week): the strip, legend and first row keep their y across scrolling and
across a week with blocked and paused bars and one without. Seven themes, names at 6.3:1 or more.
`dir=rtl` starts the row at the right. The Graph with and without Hide done.
**Open:** فارسی labels wait for mu-146/mu-147. The latent `.waiting` CSS clash is a backlog note.

## 2026-10-14 — A takeover's mail always reaches the successor (mu-156)
**Fixed (asghar_27, builder):** mu-90 review R6 and R7. A takeover whose mail move hit one bad file
stopped there, left asks.json pointing at the old mailbox, and couldn't be retried ("you already
hold"). `queue::move_mail` now repoints asks.json first, moves every file it can, and names
asks.json and each file left behind in one error. Running `muster claim <id> --takeover` again as
the session that took over moves what's left from the old holder named in the log's takeover line
(`ops::taken_over_from`), renews the heartbeat, logs nothing, and says
`moved 2 messages from mail/<old>` or `nothing left to move` (`TakenOver::rerun_line`, `cli.rs`).
The answer hook moved a check-in or pause request into `delivered/` before checking it, so one
racing a takeover reached nobody. `bin/answer-hook.py` now takes `mail/.lock` with a non-blocking
flock (`O_NOFOLLOW`; busy or unopenable means try next call), checks, then moves.
**Decided:** the rerun check comes before the pause and one-claim checks: it moves only mail, and a
paused task's pause request is exactly the mail that must reach the successor. Tests make one
rename fail with macOS `chflags uchg` (no per-file failure is portable), so those tests are
macOS-only; the malformed-asks.json and rerun tests run everywhere. From a review pass: a holder
with no mailbox (the owner outside a session) still gets "you already hold" on a rerun, rather
than a false "nothing left to move".
**Verified:** gate: green (pnpm install, pnpm check, test_task, test_commands, cargo test
(Kiritimati)); 29 Rust `takeover`/`move_mail` tests; 3 new hook tests, one of them against a real
`muster ping` waiting on the lock the test holds.
**Fixed in review round 1 (Asghar_31 as asghar_27):** R1: one mail file the hook couldn't rename
(uchg) or read (not UTF-8) raised out of `take_mail`, and the mail it had already moved to
`delivered/` in that call was never handed over. Each file is now decided, moved and read in its
own `try`; an unmovable one waits for the next call, an unreadable one is set aside. R2: the hook
takes the lock `LOCK_SH | LOCK_NB` (the lead's call): every muster write locks it exclusive, so
shared still keeps a takeover out, and the app's 2 s shared reads no longer delay mail. R3: a
quiet takeover's leftovers could be taken by the still-running old session before the rerun. The
hook now leaves a task's mail alone while the task is in Building or Shaping, its claim isn't this
session's, and its latest takeover (no lease stage line after it) names this session as the old
holder (`taken_from`, the Python twin of `ops::taken_over_from`).
**Verified (round 1):** 3 new hook tests (a shared reader doesn't hold mail up, an exclusive one
does; a uchg file and a non-UTF-8 file between good ones; a takeover's leftovers wait, the rerun
moves them to the successor, and a fresh claim lets mail flow again); all 71 `bin/test_task.py`
tests pass.
**Fixed in review round 2 (Asghar_32 as asghar_27):** R4: none of `taken_from`'s guards had a test.
One test now ends the takeover four ways (task moved on to Testing, a fresh claim into Building
by a third session, the claim back with this session, a takeover line quoted in a message) and
checks the mail goes out each time. R5: a task file `taken_from` couldn't read (not UTF-8, no
permission) raised, and the per-file catch then held that task's mail back on every call; it now
counts as no takeover, as the docstring said.
**Verified (round 2):** 2 new hook tests; each guard removed in turn (stage check, lease reset,
holds(), the ': ' guard, the read catch) turns its own case red, and restored turns it green.
**Docs:** AS_BUILT (Takeover, Questions), PROTOCOL "Sessions", help.rs claim text and the manual's
Commands page, docs/now/claims.md.

## 2026-10-14 — mu-98: Sessions view, cc-mg and cc-hg sessions grouped like Claude Code's agents list, with a recap per session
**Did:** a Sessions project view (`Sessions.tsx`, `sessionsData.ts`), for All or one project:
Needs input, Working and Completed with counts, one line per session, Completed older than a day in
an Older chip, Focus's chips. Clicking a row opens its recap in the pane (`SessionRecap.tsx`): handoff
chain, who, tasks, history, time, tokens, progress, and Transcript under it. Core: `sessions.rs`
(list, project match, recap, chain, safe handoff reads), `agents::parse_list`/`read_lists`,
`transcripts` tail and first-prompt reads, commands `sessions_list`, `session_recap` and
`session_markdown`. Modes rows `session-groups` and `session-history`.
**Decided:** `name` and `cwd` are read for the owner's window only, and `cwd` never leaves the core
([decision 0017](decisions/0017-sessions-view-reads-name-and-cwd-for-the-window-only.md)). The age is
in `--muted`, not `--faint`, because `--faint` is under 4.5:1. The view's words are in one `WORDS` table, since the app
has none. The modes test now skips `.test.tsx` sources too.
**Verified:** `pnpm check` green (1780 vitest, 848 cargo lib); 60 tail reads 1.9 ms (release);
60-row render under 50 ms (test). Headless WebKit with mocked IPC and synthetic sessions: the list,
Focus, a recap with a chain (dark, and light in Standard), Older opened, RTL, and the keys (Enter
opens, Esc closes, focus back on the row), attached to the task.
**Open:** needs-input sessions with no task still don't count in the needs-you number: that's a
separate call for the owner.

## 2026-10-15 — The CLI ends quietly when its reader closes the pipe (mu-170)
**Did (akbar_26, builder):** one stdout writer in `cli.rs` (`Stdout`, `out!`/`outln!`); all 125
print sites go through it. A broken pipe ends the output quietly: no panic, nothing on stderr, the
command finishes its queue writes and exits with its own status. Another write failure is said
once on stderr when the command ends and turns 0 into 1. New `tests/broken_pipe.rs`; AS_BUILT's
command-line section.
**Fixed:** `muster show mu-1 | head -1` panicked ("failed printing to stdout: Broken pipe"), exit
101 → Rust ignores SIGPIPE, so a write to a closed pipe is EPIPE and the print macros panic on it
→ the writer drops output after EPIPE instead. Review R1 (Akbar_29): a write whose output failed
on a full disk said only "can't write the output" with exit 1, so an agent would redo the note →
the line didn't say the change stood → a command that may write and succeeded now says
"muster: note ran and its change is saved, but its output couldn't be written (…): don't run it
again", as `comment` does; reads and refusals keep the old line (`reads_only`, unknown = write).
**Decided:** exit 0 on a closed pipe, not 141 (Shapour's call in the task): restoring SIGPIPE
would kill muster mid-write under the queue's lock. The manual's Commands page is made from
`help.rs` and lists no exit statuses, so it's unchanged; no now page covers `cli.rs`.
**Verified:** `cargo test --test broken_pipe` (6: show on a 270 KB task and list of 1000 tasks
read one line, help and note on a closed pipe, a refusal keeps 1 and its line, no bare print in
`cli.rs`) and 6 unit tests of the writer (a write's and a read's failure line, `reads_only`). On a
copy of Muster's queue, release builds: before, `show mu-1 | head -1` and `list --json | head -1`
exit 101 with the panic; after, `show`, `list`, `list --json`, `help`, `vision`, `shape`, `review`
all `0 0`, stderr empty. `list --json > /dev/null` (885 KB): 141–216 ms before, 140–261 ms after,
the spread is the machine's noise.

## 2026-10-15 — mu-131 round 4 settled (back to Building); notes shaped (mu-187, mu-188)
**Decided:** mu-131's round 4 (past the 3-round cap) found R13 (restart never writes gates:), R14 (hand-merge
skips merge_check), R15 (ca1c720 doesn't build alone); settled with kia-131's pick for the owner: returned to
Building, builder session a694a398 resumed on cc-hg as gholi-131-r4 (its rebaser entry was removed by the lead's
cleanup; the rebase b3b7fc5 had landed). Alternative: settle as accepted and merge; turned down, R13/R14 are real gaps.
**Did (shapour):** 2328 → mu-187 (high, after 0131): the merge fast-forwards main to the gated SHA, not the branch
name (parallel.rs:1126). 2350 → mu-188 (low, after 0173): drop the timeline's unused waiting segment; vision V1 vs the
owner's legend note resolved by the later 2026-10-13 rule "each task when started and finished".
**Verified:** both in Ready; backlog empty; `muster check` nothing on them.

## 2026-10-14 — Persian foundation: Drover's i18n layer, Language setting, shell and menu bar in Persian (mu-146)
**Did:** ported Drover's `src/i18n/` (a990fc8, task 0007) to `app/src/i18n/` and its `i18n.rs` to
`app/src-tauri/src/i18n.rs`; namespaces common, shell, board, pane, help, native, cli, core in
`locales/{en,fa}/`. `lang` in `settings.rs` (unset follows macOS). Language chips at the top of
Settings. The tray, sidebar, toolbar, filter bar and every Settings group are in Persian.
`docs/i18n/glossary-fa.md` from Drover's, plus Muster's terms. The guard `literals.test.ts` has an
allowlist naming mu-147 or mu-148 per file.
**Fixed (review round 1):** About and manual 04 list the language in `settings.json`, and manual 04
has a Language section (R1); `Info.plist` declares `CFBundleLocalizations` en, fa and
`CFBundleDevelopmentRegion` en, so macOS offers a per-app language, with a test against
`i18n::LOCALES` (R2); the allowlist gives `yourCheckLimit.ts` and `Helper.tsx` to mu-148 and
`Lessons.tsx` to mu-147, as those tasks list them (R3); the guard checks `name:` fields and `aria`
props (R4, in part); `code, kbd, pre` are isolated left to right (R5); `.settings-side`'s divider is
`border-inline-end` (R6); every Language chip saves when clicked, so a change of mind while fa loads
goes through (R7); the unset-language test renders Settings and can fail (R8).
**Did (rebase on main):** main's new shell text (accounts, roles, fonts, the sidebar row's
aria-label, From shaping, History, the holder ribbon setting, Roles, the You ribbon's quiet)
goes through the layer, with fa; the sidebar's needs-you words are mu-159's ("1 needs you"). The
card's `HolderRibbon.tsx` is on the allowlist for mu-147.
**Decided:** the Language block sits above the Settings groups, not as a group, so someone who
can't read the language shown still finds it. The macOS language is read with `defaults` (no
objc2 dependency). `Sidebar.tsx` is the task pane's right column, so it goes to mu-148. Shortcuts
in fa are wrapped in LRI…PDI, with a test. R4's lowercase phrases ("nothing hidden") stay unflagged:
catching them flags code strings too; mu-155's sweep removes the allowlist.
**Verified:** see the pass's gate line. AC6: the index chunk shrank (392.26 → 369.83 kB, no Persian
in it), but the en JSON chunk is preloaded, so the startup set is about the same (~525 vs ~527 kB).
**Open:** the owner's native-speaker check of the proposed words (چیپ, تسک برداشته‌شده, لاگ,
سرصفحه, بورد, نقش). Board text is mu-147's. Digits are mixed until mu-151, and RTL mirroring is
mu-149's.

## 2026-10-14 — One needs-you wording app-wide: folded strips and bands say "1 needs you" (mu-159)
**Did (asghar_28, builder):** `needsWords(n)` ("1 needs you", "2 need you") and `needsVerb(n)` in
`human.ts` are the one place the words are chosen. `FoldCounts` (the badge of a folded strip,
Backlog band and Paused band) titled one "1 need you"; it now calls `needsWords`, as do
`countsText`, `ProjectList` (its local copy gone), the `ColumnRow` chip and cue, and `rowCue.ts`
`cueWords`. A folded strip's and band's button gets an `aria-label` with its counts in words from
the new `placeWords` in `ModeViews.tsx` ("Shaping: 1 item, 1 needs you"), which `chipWords` uses too.
**Fixed:** "1 need you" on a folded strip/band badge → `FoldCounts` built the words inline with no
singular → it calls `needsWords`.
**Decided:** `placeWords` takes the count already in words (as shown: "4 items", "2 tasks", "empty"),
so bands that count tasks and columns that count items share it without a unit switch. A source check
in `human.test.ts` fails if any non-test file outside `human.ts` builds the count by hand.
**Verified:** vitest 1882 → all green, tsc clean, bin/gate (line in the task). Headless WebKit
(mocked IPC, synthetic tasks, Focus, Group by feature): folded Shaping strip and Backlog band with
one task needing you each, light and dark, pixel-identical to the base build; DOM: badges titled
"1 needs you", buttons named "Backlog · not in a feature yet: 1 item, 1 needs you, 1 blocked" and
"Shaping: 1 item, 1 needs you".
**Open:** mu-147 turns `needsVerb` into one plural i18n key.

## 2026-10-14 — Session states matched by id across accounts, so moved builders show running (mu-182)
**Fixed (Akbar_24, builder):** the owner saw no session running while seven worked. Cause: `agents.rs`
ran `claude agents` only for the accounts the stamps named and matched on (account, id); builders
resumed on cc-hg still carry `· cc-mg`, and cc-mg's list no longer has them. Now `agents::wanted`
asks every allowed account (mg, hg; never `~/.claude`) once a stamp names one, and
`Snapshot::find` matches by id in any allowed account (working/waiting > unknown > ended, then the
stamp's account). The task keeps the account it was found on (`session_account`, not sent to the
window), and a waiting session's notification is that account's news, so a dead cc-mg folder can't
hold hg's news back. Live dot, Work rows and transcripts already looked ids up across allowed
accounts; a test now proves it. AS_BUILT, DESIGN, docs/now/claims.md.
**Decided:** a session listed by no account stays unknown (heartbeats decide), not ended as the
brief's Tests line said: unmatched has meant unknown since 0048, and an ended reading would mark
every session `claude agents` doesn't list (older or not run through it) as dead.
**Verified:** see the gate line in mu-182's pass.

## 2026-10-14 — Migrate's probe clears a dead run's scratch folder first (mu-183)
**Fixed (Gholi_27, builder):** a migrate killed mid-probe left `$TMPDIR/muster-migrate-probe-<pid>-0`;
a later process reusing the pid refused with a false "another file and … would both be" → since
mu-157 the name is pid + counter, which repeats across processes → `probe()` in `migrate.rs` now
removes its folder first (no live process shares the pid, so it can only be a dead run's).
**Verified:** new `a_stale_probe_folder_left_by_a_killed_run_is_cleared_first` failed before the
fix and passes after; the accent clash test now also asserts the probe's "would both be" naming
both files; `cargo test migrate` 24/24 (hermetic HOME); gate below.

## 2026-10-14 — gates-build-hg stopped; mu-185 also waits on mu-182
**Fixed:** gates-build-hg (8fc440e1) had sat on a permission prompt since 21:13 at its "replace the stuck
tester" step, stale since kia-131 was resumed on cc-hg and mu-131 moved on → stopped (`cc-hg-new stop`);
its fixture reset had already landed. mu-185 now depends on 0182 as well as 0184 (lead QA_10's ask).

## 2026-10-14 — Local queues keep a history; muster restore; git clean -x refused (mu-109)
**Did (asghar_13, builder):** new `history.rs`: a local `.muster/` queue gets a bare git at
`<git common dir>/muster/history.git` with `.muster/` as its work tree, started by `muster init`
and `migrate` (`projects.rs`, `migrate.rs`) or, for a queue without one, by its first write.
`queue::with_lock` snapshots the queue's files before each write and commits the ones whose size,
time or inode changed, as `muster`, `<key>: <the task's last log line>` (notes and attachments by
name); `mail/`, `INDEX.md`, `tasks.json`, `.claude/` and temp files never go in. A commit that
fails is kept in the history's `muster-pending` and retried by the next write. `muster check` fails
on missing files (`muster restore --all`) and uncommitted writes (`muster history commit`), warns
on files changed outside muster. New commands `muster restore <path>…|--all` (under the lock,
logged as an empty commit) and `muster history [commit] [--json]` (`cli.rs`, `help.rs`, the
manual's Commands page). Settings › Projects shows a History line under Run, with Restore or
Commit when there's something to fix (`Projects.tsx`, three Tauri commands in `lib.rs`). The
personal layer's `hooks/guard-bash.mjs` refuses `git clean` with `-x`/`-X` in a main checkout
with `.muster/`, for every role, reading the command's words (`cd`, `-C`, `--work-tree`).
PROTOCOL.md, the manual (concepts, the app, for agents), the queue README and AS_BUILT.md say it.
**Decided:** files are found by a stat snapshot around the write rather than by threading paths
through every write function: one place, no write path can be missed, and a hand edit of another
file stays out of the write's commit. git runs with every `GIT_*` dropped, no global or system
config, hooks off and signing off, so the owner's git setup never reaches these commits.
`muster history commit` (not `--commit`: that's a value flag already) takes in changes made outside
muster but never records a missing file as deleted; `restore` brings it back instead. From a
review pass: a pending write is retried only for files that are there (or that the write itself
removed), so a clean between the failure and the retry can't commit the queue as deleted;
`restore --all` puts back missing files only (a changed one only when named); a new task's id
counts the ids in the history's last commit, so a task made between a clean and the restore can't
reuse one; a queue cleaned before its `config.toml` was committed still finds its history; git gets
absolute paths (`--queue .muster`); the guard lets a dry run (`-n` without `-f`) through. The
commit is plumbing (`update-index`, `write-tree`, `commit-tree`, `update-ref` from the commit it
was made on), not `git commit`, which walked the work tree for its status: 150 to 340 ms a write
on Muster's queue with its 51 MB of attachments; the last commit's tree is noted in the history's
folder so a write needs no `rev-parse`.
**Verified:** 14 `history::tests` (init, first write, one commit per write with only its paths,
refused writes, two-task messages, pending writes retried, `git clean -fdx` then `restore --all`
byte for byte, path restores and refusals, the message, kept paths, and one per review fix), a CLI test of `check`'s exit
codes around a clean, the migrate test's history, 5 vitest cases for the History line, and the
guard's node test (refused forms, allowed forms, words about it). Write time on a copy of Muster's
queue (370 files, 51 MB), release build, 60 `muster add` writes alternating between a copy with
the history and one without, on a loaded machine (load average 14 to 17): without 53 ms p50, with
82 ms p50, added 31 ms p50 (51 ms p90) paired. The history's start (the first write, hashing all
51 MB) took 35 s under that load and 4 s on a quieter run.
**Fixed (Asghar_22, review round 1, R1-R8):** the history was never packed (only plumbing ran) →
`history::pack` runs `git gc --auto` after each commit, outside the queue's lock, and only when
objects/17 holds 2 or more loose objects (below that git does nothing, so most writes skip the
spawn). A review record or an attachment got the subject of its first sorted path → task files'
lines lead the message. A file deleted on purpose kept `check` failing → new `muster history
removed <path>…|--all` (Settings: Keep deleted, `record_removed`) commits the removal; a named
`muster restore` brings a removed file back from the commit before its removal. `restore --all`
undid a removal a pending write had made → files a pending write accounts for are neither
"missing" nor "changed". The Settings line was read once → re-read every 30 s, newest reply kept
(`newestOnly`). The Restore/Commit click mapping was untested → `runFix` and a click test over the
cell's elements. Restore said "restored" at 0 → `fixDone` words it from the count. git's reason was
dropped on exit 1 and its whole path shown → `git_error` keeps it and names the folders
`history.git` and `.muster`. Found in the live sandboxed run: a session whose sandbox allows
`main/.muster` but not `main/.git` (the layer's `allowWrite`) writes the queue, but the commit and
the pending record both failed, so the write showed only as a warning → the pending record falls
back to `.muster/mail/history-pending`, and `check` fails with "1 write not committed" until a
write that can reach the history commits it. `muster history` said "every file committed" for a
history not started → says it's not started.
**Verified (round 1 fixes):** 31 `history::tests` (6 new: packing, multi-file subjects with a real
review record and attachment, removals, pending removals, the mail fallback, git's reason), the
CLI test's `history removed` exit codes, 4 new vitest cases. Live: the branch's release build,
`sandbox-exec` denying writes under `main/.git`, `muster checkin mu-109` on Muster's own queue →
checked in, `muster history` → "1 write not committed (fatal: cannot mkdir history.git: Operation
not permitted)"; the pending file removed afterwards, so the queue's history still isn't started.
Write time, 60 paired `muster add` on copies of Muster's queue (release build, a committed copy as
the baseline), load average 21 to 26: without 68 ms p50, with 112 ms p50, added 45 ms p50 (214 ms
p90). `gc --auto` alone: 8 ms.
**Open:** the guard change is in `~/playground/.claude`, committed there as 1cbb1ba (guard-bash,
AC5), not on this branch. That repo's test file has one other failure, in its
`link-personal-layer` test (a sandbox entry for the main checkout's `.muster/`), which this change
doesn't touch. The layer's `allowWrite` lists `~/playground/muster/main/.muster` but not
`~/playground/muster/main/.git/muster`: a session sandboxed that way leaves its writes pending
until an unsandboxed or worktree session writes (worktree sessions of the repo can write its
`.git`). Adding that entry lets them commit at once; it's a layer change, not on this branch.
**Fixed (Asghar_26, review round 2, R9-R10):** after `muster history removed` the next task took
a removed task's id, and restoring the removed file by name left two tasks with one key →
`last_id` also counts task ids in the history's removal commits (`removals`: `git log
--grep=^muster: removed --diff-filter=D`, 15 ms on 5,000 commits), and `restore` refuses a task
file whose id a task file there holds now (by its name or its written id), or that another restored
file brings back, naming that task and file; nothing is put back then (`taken_ids`). `restore
<folder>` (and `tasks`, `.`) brought back nothing removed when the folder still held a committed
file → a folder also takes the files under it that `history removed` took out and are still gone,
each from before its last removal; one a write deleted after that (a dropped note) stays gone.
**Verified (round 2):** 2 new `history::tests` (id never reused and the refusal by path, `tasks`
and `.`; partial-folder and whole-queue folder restores, a note removed, restored, then dropped
stays gone; it fails with the last-removal check taken out). Rebased on main (`drop_note` takes a
lease now). bin/gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati)).
## 2026-10-14 — Two more notes shaped (mu-185, mu-186)
**Did (shapour):** the 2317 note (a claim keeps its old account after a session moves cc-mg→cc-hg)
became mu-185, high, after mu-184: a holder's act re-stamps `claimed-by:`/`held-by:` when its session
id and agent match but the account changed, and takeover by that session re-stamps instead of refusing.
The 2318 note became mu-186: the window gets `queue::Task.builders` and `lib.ts` `builders()` counts
the takeover chain as `tester_rule` (ops.rs:2209) does.
**Decided:** mu-185 is its own task, not folded into mu-184 or mu-182 (mu-182 is in Building and read-side
only; mu-184's re-stamp covers bare labels). Re-stamp on every act by the holder, not only checkin.
Vision V1 against decision 0013 rule 2 resolved by the owner's words: the re-stamp records a move the
owner's tool made; Muster still never picks an account. mu-186 aligns the app with the core; no live
repro was traced.
**Verified:** both in Ready; backlog empty; `muster check` reports nothing about the new tasks.

## 2026-10-14 — The card's "needs a tester" tells a teammate from its lead (mu-158)
**Fixed (Gholi_26, builder):** a lead holding in Testing what its teammate built saw "needs a
tester" though the core lets it test → `lib.ts` `sameHolder` compared label and session but not the
agent that `session::same_holder` compares since 0090 → it now also requires the same agent when
both stamps have a session (none read as the lead), so `needsTester` and the built-by line match the core.
**Verified:** `lib.test.ts` mirrors the core's teammate, old-stamp and slot cases plus `needsTester`
for lead, teammate and gated-in teammate; the new cases failed before the fix. Gate below.

## 2026-10-14 — Five builders moved from cc-mg to cc-hg after cc-mg's weekly limit
**Fixed:** Asghar_20 (mu-113), Gholi_18 (mu-146), Gholi_23 (mu-98), kia-131 and gholi-131-fix2
(mu-131) were stuck on "You've hit your weekly limit" (cc-mg, resets Oct 18). Both accounts' `projects/`
link to `~/.claude/projects`, so each was stopped on cc-mg and resumed on cc-hg
(`cc-hg-new --bg --resume <id> -n <name> --permission-mode bypassPermissions`) with the same id and claim.
**Verified:** `cc-hg-new agents --json` shows all five busy/working; no background session left on cc-mg but QA_10's finished record. Lead QA_10 told.

## 2026-10-14 — Role profiles and slot names: roles.toml, Kaveh-2 on cards, muster team (mu-112)
**Did (Akbar_11, builder):** new `roles.rs`: `roles.toml` beside projects.json, written from the
defaults (the brief's ten roles and Pouya) on first read and cached by modified time and size;
slots (`--role` on claim, next and start hands out the lowest free one, held while a task in
Building or Testing has it); `muster team [--json]` and `muster instructions <role>`. `session.rs`
stamps a slot after the session and agent (`Kaveh-2 (main · session … · slot coder-2 · cc-hg)`),
and one slot in one session is one holder whatever its label. The poll sends `roleNames`; cards,
the pane and the built-by line show a slot by its role's name now (`whoLine`, `shownHolder`).
Settings › Agents › Roles (`Roles.tsx`, `rolesData.ts`, `roles_info`/`set_role`). PROTOCOL.md,
help (new Team group), the manual (concepts, Settings › Agents, for agents), DESIGN.md, AS_BUILT.md.
**Decided:** a strict reader for the few TOML shapes the file needs instead of a `toml` crate (a
new dependency needs the owner's yes; config.toml is hand-read too). Stamps keep the role id and
number in a `slot` part rather than relying on the label, so a rename needs no migration and an
old stamp reads as before. Role ids: shaper, architect, designer, coder, tester, security,
performance, accessibility, release, lead, researcher.
**Verified:** roles 12, session 4 new, cli 2 new (cargo); Roles.test.ts 9 (Vitest); the debug
binary on a scratch queue in `$TMPDIR` (claim --role, team, rename, a bad key); Roles rendered
light and dark in headless Chromium; gate below.
**Open:** "a live run holds a slot" waits for the runner (plan 2.1): today only claims hold one.
Two sessions asking `--role` at the same moment can get the same number; the dispatcher (2.2)
hands slots out one at a time. Notifications name no holder, so nothing changed there.
**Fixed (Akbar_15, round 1):** R1: `--role` handed one session a new slot per claim, so it could
hold two Building tasks; `role_label` (now in `run_on`) gives a session the slot it already holds
in Building or Testing (by session and agent), so a second claim is refused as any is and a re-run
of `start` resumes; another role's slot in the same session is refused; the branch's test that
asserted two holds asserts the refusal. R3: a pass into Testing or a takeover under the slot's old
name after a rename dropped the slot; `session::keep_slot` carries the claim's slot. R2: History,
the Next step's held and stale notes, the table, the graph and Recently finished's built by show the
slot's name now (`nameOf`, `RoleNamesContext`). R4: every field hand-set reads back through
render/parse and survives a Settings edit. R5: `start` and `next --role` tested in a git fixture,
re-run included. Nits: R6 title and prompt refuse bidi controls and U+2028/2029; R7 an open list is
scanned once per line and a refused file is cached; R8 the manual says `--role` is for roles that
claim; R9 `shownLabel` reads only the roles' own keys; R10 the bad-file table covers every bound.
**Fixed (Akbar_18, round 2):** R11: one session got a second slot by a takeover, `--as` another
slot's name, or `--as` an old name after a rename, because the rule lived only in `role_label`.
It's in the core now, under the queue lock: `holding()` counts a Building claim of the caller's
session and agent as the caller's when either stamp has a slot, and claims and takeovers are
refused while the session holds another slot in Building or Testing (`ops::other_slot`). An old
slot name is stamped with the slot the session holds under it (`stamped_who`). R12: a send-back
from Testing gave the claim to the bare label, dropping session and slot; `return_task` keeps the
builder's exact stamp when it has a slot. R13: the Held by chips, the stage track's title and the
move toast, search and the table's who sort take the role names, and the `= {}` defaults are gone,
so a missed call site fails the typecheck. R14: a cache hit is observable (a per-thread cache and a
parse counter in tests), and the test fails when a refusal isn't cached.
**Decided:** a plain label beside a slot's claim in one session is refused too (one session holds
one Building task), and two plain labels in one session stay two holders, as on main.
**Verified:** cli 6 new, session 1 new (cargo); Roles.test.ts 4 new (Vitest); mutations of
`holding`, `other_slot` (claim and takeover), the send-back and the R14 cache each fail a test; gate below.
**Fixed (Akbar_20, round 3):** R15: one slot per session held only within a queue, since the
checks read `tasks_now(root)`; `ops::slot_refusal` (replacing `other_slot`) reads the other queues
too (`ops::elsewhere`, the ones `muster team` reads). R16: no write checked that a slot was free of
other sessions, so `--as` a held slot's name, or a return from Your check, gave one slot two
holders; claims as a slot another session holds are refused, and a task coming back to a holder
(return to Building or Testing, a pass into Testing) after its slot went elsewhere comes back by its
exact session without the slot (`ops::unless_taken`, `session::without_slot`); a stamp without a
slot part holds none (`roles::slot_of_holder` reads a label only without a stamp). R17: a takeover
re-runs `holding` and `slot_refusal` as the slot `keep_slot` carries on, and `log_unblocked`
releases a freed claim beside a slot's in any queue (`one_session_slot`). R18: check-ins, comments,
the Work rows and question askers show the slot's name now: the core sends `slot` on Work rows
(`work::SessionRef`) and questions (`queue::asker_slot`, from a stamp of the asker's label, session
and agent); `slotLabel` and `askerShown` in lib.ts. mu-97's holder ribbon, merged meanwhile, names
the slot too (`nameOf`), and its tooltip the full stamp.
**Decided:** the other queues are read without their locks, as `muster team` reads them, so two
claims racing in two queues can still both pass; the dispatcher (plan 2.2) serialises claims.
**Verified:** cli 5 new, session, work and queue 1 new each (cargo); Roles.test.ts 5 new (Vitest);
mutants killed: no other queues, no taken check, no strip on return, label fallback kept, the
pass keeping the slot, no takeover re-check, unblock by `same_holder` only and without other
queues, the cross-queue Building check, the asker's label and agent, Work rows' slot, and each of
the four views and the ribbon on the raw label; gate below.
**Open:** a holder brought back without its slot keeps its label, so two cards can read Kaveh-1
(one of them holding no slot) until the returned task is passed on.
**Fixed (Akbar_23, R19):** a task back in Building without its slot couldn't be taken over: the
successor's `--as Kaveh-1` is stamped with coder-1, which another session held, so `slot_refusal`
refused it. `takeover_task` drops the slot from the successor (`session::without_slot`) when the
old claim has none. **Verified:** cli 1 new (`a_task_back_without_its_slot_is_taken_over_without_it`,
fails without the fix); slot and takeover tests green; gate below.

## 2026-10-14 — projects.json byte-identical test fills every map, run and your_check too (mu-160)
**Did (Akbar_21, builder):** `folding_and_unfolding_a_heading_leaves_projects_json_byte_identical`
also fills `run` (`bin/run`) and `your_check` (a mix of `Off` and `At(n)`) for its 24 queues, and
checks that the keys are in order in the file for each of Config's eight maps, not only labels.
A comment says a new map in Config gets filled there too. Test only.
**Verified:** the test passes; by hand, `run` as a HashMap and, separately, `your_check` as a
HashMap (check_limit's maps too) each fail it at the byte-identical assert; gate green.
**Fixed (Akbar_22, round 1, R1):** `group_names` had two keys, so the reviewer's mutant (it saved
through a fresh HashMap) passed 5 of 12 runs → a random order of two keys is often sorted, or the
same twice → it gets 24 group folders inserted out of order, checked against their sorted keys.
**Verified (R1):** that mutant fails 12 of 12 hermetic runs at the byte-identical assert; a mutant
writing `group_names` in reverse order fails the key-order check 2 of 2; unmutated passes.

## 2026-10-14 — Owner's answers on decision 0016: accepted; runs start through cc-hg-new, not bare claude -p
**Decided:** mu-127 V1 resolved (Muster starts its own runs, Drover hosts them and also starts the
owner's ad-hoc sessions); Drover 0018 closed as superseded by dr-33 to dr-35; runs and probes start
through the launchers (`cc-hg-new` / `cc-mg-new`), never bare `claude -p`. Recorded in
[0016](decisions/0016-runs-headless-in-a-sandbox-drover-hosts-sessions.md#owners-answers-2026-10-14).
**Did:** mu-117 and mu-179 returned to Shaping with the rule; comments on mu-127, 175, 178.
**Verified:** queue commands answered (V1 resolved, 0 open; dr-18 → done); docs-only change.

## 2026-10-14 — Handoffs carry the claim in the layer's text (mu-161)
**Did (Asghar_24, builder):** the layer's context-watch stop (step 3) and `/handoff` now tell a
claim holder to run `muster checkin <id> "…" --handoff` before writing, to put
`muster claim <id> --takeover --as <name>` first in "Start here" and `muster inbox --session <id>`
first in "Next steps"; Resume step 2 makes the takeover the successor's first command. Built in
`~/playground/.claude`, layer commit ac6921e; here only `docs/now/claims.md`.
**Verified:** layer `node --test hooks/__tests__/guards.test.mjs` 45/45 on layer HEAD plus the
change; the new test fails on the old text. Review (correctness): the inbox line was missing and
step 0's takeover lacked `--as`; both fixed.

## 2026-10-14 — run.rs login-shell stand-ins chain their delay with && (mu-163)
**Fixed (Gholi_25, builder):** a `/bin/sleep` that doesn't run still let both stand-ins finish, so
the logout test passed and the two-writes test failed only on `took >= 300 ms` → the lines after
the sleep ran regardless → `/bin/sleep 0.2 && echo bye && touch done` and `/bin/sleep 0.3 && printf
<mark's last bytes>`: `done` and the found PATH now prove the delay ran; the times stay as a second
signal. Test-only, `app/src-tauri/src/run.rs`.
**Verified:** mutant `/bin/sleep` → `sleep` (off the stand-in's PATH): before, the logout test passed;
now it fails at `done.exists()`, and the two-writes test at the PATH (None after the 20 s wait).

## 2026-10-14 — Lead moves to cc-hg (QA_10); mu-150, 103, 110, 97, 101, 157 merged
**Did (QA_10, lead):** the owner made QA_10 the lead ("take over the lead as the orchestrator if
the mechanical is not yet deployed": mu-116/117/118 still Ready). Muster_Team_Lead_5 finished its
merges, handed off and stopped. cc-mg is nearly out of credit, so new sessions run on cc-hg; the
cc-mg builders still running reach the lead at its socket (`uds:/tmp/cc-socks/<pid>.sock`), since
SendMessage by name stays inside one account. 76 finished sessions removed at the owner's ask
(gholi-131 kept: mu-131 is in Testing). Merged: mu-150, mu-103, mu-110, mu-97, mu-101, mu-157.
The owner's UI asks became backlog notes (sidebar, Recently finished and filters toggles; search
always shown; working and token ribbons; timeline legend; hover card; estimates).
**Decided (overrule any time):** mu-110 and mu-112, held at the round cap, went back to Building
instead of to the owner, since their open findings were real and fixable. mu-112 R15: one session
holds one Building slot across every queue on this Mac, not per queue. mu-101 R9: a send-back
from Your check always records the owner finding; with no branch it's tied to `commit:` and the
next `muster start`'s branch must settle it. Merges whose only change after review was a rebase
went in as owner after a check that the code diff was unchanged (mu-150, mu-110), or after a
light rebase review (mu-97). mu-101's rebase onto mu-97 needed two test-only edits by the lead (a
test conflict, and its refusal text now names Shaping); gate green, merged as owner.
**Measured:** estimates: 80 finished tasks took p50 0.39× their time estimate (median 1.1 h);
tokens: 19 readable tasks, real use vs estimate p50 18× with cache reads, 0.78× without, no
correlation (backlog notes 2026-10-14-2122, -2123).
**Open:** mu-112 (Akbar_20), mu-160 (Akbar_22) fixing; mu-109, mu-161, mu-163 in review; mu-146,
mu-98, mu-113 building on cc-mg. Your check holds 8, over its limit of 5. Owner questions: the
mu-113 status-line helper, a minimum macOS for mu-150, and mu-109's suggestion to add
`main/.git/muster` to the layer's allowWrite.

## 2026-10-14 — migrate's probe takes a scratch folder of its own (mu-157)
**Fixed (Gholi_24, builder):** parallel migrate tests sometimes failed with a false "would both
be" (mu-95's gate) → `probe` named its scratch folder pid + nanos, and the clock ticks in µs on
macOS, so two probes in one test process could share a folder → `probe_dir()` names it pid + a
static counter (as `projects::save` does), and `probe` takes the folder it's given.
**Verified:** migrate tests 5 of 5 runs green; new tests: 10 000 names in a row all differ, and
8 threads probing one plan are all Ok and leave no folder. Mutants killed: pid alone (both new
tests, and two old ones flake as in mu-95), pid + nanos (3 of 3 runs). Gate: see the pass.

## 2026-10-14 — Your check sends back to the builder as an owner finding; @actions in comments (mu-101)
**Did (Akbar_8):** a return from Your check to Building (the app's Send back to builder,
`muster return --to building`, `@sendback`) goes to its builder first in line, records the reason
as an `owner` finding in `reviews/<id>.json`, and pings the builder's mailbox (a log line when it
can't). Only the owner settles a disputed owner finding; the round cap never holds one. An owner's
comment starting with `@accept`, `@sendback`, `@block` or `@ask` runs that step through
`fields::comment`, for the app and the CLI; agents' comments stay plain. The app's Your check row
reads Accept and Send back to builder (form defaults to Building, "What's wrong"), cards say "Sent
back by you: <why>", and the comment box has the @-hint. `ops.rs` `return_back`, `review.rs`,
`ping.rs`, `fields.rs`, `block.rs` (`behindBy`), `StageForms.tsx`, `Comments.tsx`, `lib.ts`.
**Decided:** Accept isn't refused while an owner finding is still disputed (the owner sees it
beside the builder's reason); a first word like "@akbar" is refused like any unknown verb.
**Verified:** `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`;
screenshots from a mocked core, light and dark, in the task's attachments.
**Open:** AC8 (a real send-back by button and by `@sendback`) is the owner's.
**Fixed (review round 1, Akbar_14):** R1 the pre-write refusal now has a test (an unreadable
store: nothing written, no mail). R3 a task with no branch got an owner finding no round could
re-check → `return_back` records one only for a started task; the ping says the reason without
a finding. R2, R7 assertions pinned. R4 `@block mu-12 3pm call` read "3" as an id → ids and
reason go to the core apart (`ids: reason`), and a colon after keys ends them (`leading_tasks`,
CLI `--on` too). R5 the card's reason is a `<bdi>`. R6 `@sendback` on the owner's own task in
Your check goes one stage back, as the form. R8 retook mu101-3 from a fixture the real core
built (`behindBy: owner`).
**Decided:** `@block` reads kind words as `--on` does (`you`, `permission`, `question`,
`outside`), the lead's call; an unstarted task gets no owner finding (the reviewer's second
option: no round can run on it, so Your check is its re-check).
**Verified (round 1):** `gate: green (pnpm check, test_task, test_commands, cargo test
(Kiritimati))`; mu101-3 retaken in headless WebKit from `muster list --json` of a fixture queue
in `$TMPDIR`, light and dark.
**Fixed (review round 2, Akbar_19):** R9/R3 the merge clears `branch:`, so round 1's "started
only" gate meant no real send-back ever got an owner finding → every agent's task sent back gets
one, its evidence naming the branch or the merged `commit:`; with no branch, its pass waits
(`review::owner_to_recheck`, `holds_unbranched`) until `muster start` makes a fresh branch from
main (`ops::restart_task`, claim kept), whose review round settles it; the mail and the start
brief say so. Tests build the task with `start_task` + `pass_merged`, and a git-level test runs
start, merge, send-back, start again, round, merge. R10 `@block 3pm call` / `@block tasks from
design` were refused → only typed kinds count without a key, else outside. R11 `9:30` read as task
0009 → a colon ends keys only after a whole word, and `block_on_tasks` refuses `9:30`. R12 the
pane claimed a finding it didn't have → `return_task` returns `Returned` and `sentBackMsg` words
it. R13 an action whose comment couldn't be saved left the box full → `Commented.unsaved`, the box
empties. R14 the told line no longer needs a finding (`returned_lines`). R15 a one-value `--on
"tm-1 4pm call"` test. R16 the ⌘↩ row names the Send back form.
**Decided:** the lead's design call for R9 (owner may overrule): always record the owner finding,
tied to `commit:` with no branch; the next `start` makes the branch and its round settles it;
`check_recorder` still needs a branch. Not taken: reopening the old branch (deleted by the
merge), or letting a round run without one. The owner can still settle it by hand. The owner's
own task (`for: human`) gets no owner finding: no builder, no review.

## 2026-10-14 — Shaping is a claim, and held cards wear a holder ribbon (mu-97)
**Did (Asghar_11, builder):** `muster shape` claims the note in `shaping.json` (`--peek` reads
without taking; someone else's live claim is refused); `file` carries the claim to the Shaping task;
`file`, `drop`, `release <note>` and `reclaim` let it go; Shaping claims count for `holder()`,
`heartbeat`, `checkin`, `ping`, `check` and `reclaim` (stays in Shaping); `list`/`--json` show the
shaper. App: `HolderRibbon.tsx` on cards in Shaping, Building and Testing and on held notes, in every
mode, a fixed strip above the top row so a claim moves nothing. Core: `backlog.rs`, `ops.rs`,
`cli.rs`, `queue.rs`, `ping.rs`, `layout.rs`, `help.rs`; docs: PROTOCOL, DESIGN, AS_BUILT, manual.
**Decided:** no `--as` needed (shape claims as `me()`, like `claim`); the owner holds several notes,
others one; note names resolve to the folder's own spelling (same inode), so case or NFD typing
can't get past a claim; moving into Shaping other than by `file` clears a claim; the ribbon reserves
a 17px strip above the top row rather than the top row's end (that wrapped key/kind/priority at
170px columns).
**Fixed (review):** a shaper's claim leaking onto the owner's human task via `set for human`; a
case-variant note name bypassing a claim; the ribbon's name shrinking to nothing on narrow cards.
**Fixed (review round 1, Asghar_16/18):** a Shaping lease not renewed by the shaper's acts or an
unblock → one `queue::has_lease` (building|shaping) everywhere a hold renews; `checkin --handoff`
in Shaping pointing at a `--takeover` that refused Shaping → takeover takes Shaping, and
`claim <note> --takeover` takes a note's; the answer hook dropping a shaper's check-in request
(`still_asked` was Building-only); `file`/`drop` briefly showing a claim on a missing note → the
claim goes before the rename and comes back if it fails; the owner's shaped note wearing a
neutral ribbon → You; the live dot squashed; "held by" missing from an owner-held card's spoken
name; a note's ribbon over its rounded corner. Tests for each, plus shape/reclaim output and a
too-big note never claimed.
**Decided (round 1):** a note claim has no handoff check-in: its shaper hands off with `muster release`,
else the successor waits 30 quiet minutes (as Building has no handoff without a check-in).
**Fixed (review round 2 and QA, Asghar_21):** `claim <note>.md --takeover` exiting 1 (task_arg
read the note as a task key) → exempt for `claim --takeover` as for `release`, tested through
`main()`; resuming a paused Shaping task and a shaper's vision check not renewing the lease →
`has_lease` in `pause.rs` and `vision.rs`; `hold_again` and the shaper's comment/edit/ask renewals
untested → tests, each hermetic mutant now killed; the owner's note never saying quiet → "You ·
quiet"; a false "couldn't be read" when `shaping.json` goes between lstat and open → nothing held.
QA's AC4: names cut to "asg…" at 106px cards → the ribbon may run the strip's width (half beside the
stage band), "shaping" gives way first at 100000× the name's rate (at 1000× WebKit's rounding still
cut the name a letter) and its "·" stays on its own.
**Verified:** bin/gate green; independent core and app reviews, findings fixed with tests; round 2
ribbon widths measured in headless WebKit at 70-250px cards ("asghar_16", "shapour_3" whole from
106px).
**Open:** screenshots (AC4/AC5) and the owner's look (AC6) are Testing's; shaper instructions in
`.claude/` should learn `--peek`; mu-90 overlaps ops.rs/cli.rs.
**Rebased (Asghar_25)** onto 951ce03 (mu-103, mu-110): the shaping brief has both the Unknowns and
the release lines, `cli.rs` takes `--send` and `--peek`, `file`'s help keeps the marker warning and
the claim; manual 05 regenerated from `help.rs`.

## 2026-10-14 — mu-110 round 3: origin's branch kept until main is pushed, review with the worktree gone
**Fixed (Gholi_23, builder, under gholi_15):** R6: recording a hand merge deleted origin's task
branch though nothing pushed main → `drop_origin_copy` checked only tip equality → it's dropped now
only when origin's main holds the tip (`origin_trunk_holds`: ls-remote, fetch, is-ancestor), else
kept with a note and the report saying so. R2: `muster review` of a task whose worktree was removed
before any review failed → `branch_span` needed the worktree → it falls back to the queue's own repo
or the cwd's (`repo_with_branch`, `review::span_in`). R7: a pass from a folder whose repo hasn't the
branch answered `release --clean` → it says to pass from the project's repo.
**Verified:** gate green; mutants killed: prune made a no-op (the new rm -rf'd-worktree case) and
origin's branch always dropped (the pushed-branch case).

## 2026-10-14 — muster check names a task merged by hand whose stage still says Building or Testing (mu-110)
**Did (Gholi_15, builder):** `parallel::lagging`, called by `stale`, adds a warning line for a
task in Building or Testing, not paused, whose own `task/` branch has commits and is in the local
main, with the pass that catches it up. It replaces the worktree-gone line (whose `release --clean`
would send merged work back to Ready). PROTOCOL.md, `muster help check`, AS_BUILT.md issue 3.
**Decided:** "has commits of its own" is read from the branch's reflog (`own_commits`: a commit,
cherry-pick, `am` or a rebase that picked something; one of them must be in the tip). A task
started a moment ago has its branch at main's tip, which is in main too, and a fast-forward or a
rebase onto a moved main makes no commit (review round 1, R1: checking only that the tip moved
told such a task to pass). Alternatives: commit time against the start stamp (minute stamps,
clock-dependent), or recording the base at start (a new field for one check). No reflog means no
line, never a false one.
**Verified:** `bin/gate` green; parallel and cli tests for merged in Building and in Testing, the
worktree removed after the merge, an unmerged branch, a fresh start, a branch caught up by a
fast-forward or a rebase with nothing built, commits reset away, a rebased and a squashed branch
merged, a paused task, a non-`task/` branch, a clean queue, and `check`'s exit code 0 with the
warning; the reflog parser over every entry shape in this repo's reflogs.
**Fixed (review round 2, Gholi_21):** with the worktree removed after the merge, the line's
`muster pass --from testing` hit "worktree is gone … release --clean" (R2) → `merge` needed a
worktree for every step → when it's gone and the branch is in main with its own commit
(`hand_merged`), `merge` records the merge from the repo the pass runs in (`record_hand_merge`:
review still clean at the tip, branch deleted, task to Your check, no rebase, gate or push). The
Building line's pass lacked `--summary`, so it was refused on a task with no summary (R3) → added
in the line, help, PROTOCOL.md and the manual. R4: a test for the Building/Testing guard. R5:
`stale`'s match re-indented (its long literals shortened so rustfmt formats it again).
**Decided:** the app's Problems list stays without these lines: it's built on every queue read,
and git calls per started task there need caching and a worker thread, more than this task. The
lead's call (over about an hour: CLI only, backlog note).
**Verified (round 2):** a cli test runs both printed commands, placeholders filled, on a task with
no summary and its worktree removed: exit 0, Your check, no line left. Parallel tests: recorded
with the worktree gone (no gate run, main and origin unchanged, branch deleted, report), refused
with no repo, unmerged, by its builder, or with no clean review (the owner's override records it);
no pass line past Testing; mutating the stage guard fails its test.
**Open:** a squash merge or a branch deleted after the merge isn't detected (AS_BUILT.md).
## 2026-10-14 — Spike: Muster and Drover as one pipeline, runs sandboxed with full access (decision 0016)
**Did:** the owner asked how to combine Muster and Drover now that Muster is the whole pipeline,
with runs sandboxed and worktreed so they can have full access. Spike report
`docs/research/2026-10-14-session-host-and-sandbox.md`: two web research tracks, Muster's and
Drover's code read, and a hands-on Seatbelt test on this Mac. Filed mu-175 (per-run sandbox),
mu-176 (network allowlist proxy), mu-177 (dispatcher merges and pushes), mu-178 (open live and hand
back), mu-179 (accounts without a login). Reshaped mu-127 into the run-record contract. In Drover's
queue: dr-32 (bug: deleting an imported lane deletes its branch), dr-33 (runs watchable live),
dr-34 (open a run live in a lane), dr-35 (sandboxed lanes). Backlog note: Muster's screens move
into Drover. VISION.md, PIPELINE.md, 0007 and 0015 amended.
**Decided:** [0016](decisions/0016-runs-headless-in-a-sandbox-drover-hosts-sessions.md), overrule
any time. One headless runner in Muster's core, every run in its own Seatbelt profile with full
access inside and never without it. Drover hosts sessions (watch, open live, hand back) instead of
being a second runner. "Open live" rather than "take over", which already means claim takeover.
Own SBPL and proxy rather than `srt` (no new dependency).
**Verified:** under the profile, `claude -p --dangerously-skip-permissions` (cc-mg) wrote only in
its worktree, through both Bash and the Write tool. Reads of a decoy secret folder, `~/code` and
another repo were denied. The Keychain login worked, `cargo build` and
`swift build --disable-sandbox` built, and plain `swift build` failed (nested sandbox).
`muster check`: no new problems. No code changed, so no gate run.
**Open:** mu-127 V1 asks the owner to confirm the change from their 2026-10-12 answer ("drover"
starts the agents). `~/.claude-hg` holds no login (the `cc-hg` launcher injects a token), so cc-hg
runs fail until it's logged in (mu-179). Network limits, pnpm, Tauri and xcodebuild weren't tried
under the profile. The `drover/main` checkout is 44 commits behind `origin/main`. Drover 0018 is
superseded, with a comment, and is left for the owner to close.

## 2026-10-14 — Vazirmatn review round 1: the licence ships, inputs and code take the font (mu-150)
**Did (akbar_16, builder):** R1: re-cut with `--name-IDs=0,1,2,3,4,5,6,13,14` so the font keeps
its licence records (44,844 bytes); `tauri.conf.json` bundles `OFL.txt` as
`Contents/Resources/fonts/Vazirmatn-OFL.txt`; Settings › About and the manual name the font and
its licence. R2: `fonts.test.ts` hashes the woff2 against the README's sha256 and reads its name
table. R3: `input, textarea, select { font-family: inherit }` in `fonts.css`. R5: the import-order
test matches whole lines.
**Fixed:** Persian in code (the write-up editor, paths, the prefix box) drew in a monospaced Arabic
→ `ui-monospace` falls back to its own Arabic before reaching a Vazirmatn listed after it → the
code stacks put Vazirmatn first (Latin unchanged, measured).
**Verified:** headless WebKit against main: English-only screens pixel-identical at 100% and 150%;
every input kind computes Vazirmatn first (the Pause box was `system-ui`); a ZWJ emoji title loads
the face but draws pixel-identical with and without it; with `size-adjust` unknown (WebKit < 17)
the face still loads and Persian draws about 5% shorter. AC2's `document.fonts.check` proof
dropped (true before too); the loaded face and screenshots stay.

## 2026-10-14 — Persian text in Vazirmatn: a 44 KB subset, Latin stays SF (mu-150)
**Did (akbar_13, builder):** `app/src/assets/fonts/Vazirmatn-fa.woff2` (Vazirmatn 33.003 cut with
the fonts repo's pyftsubset to the Persian ranges, every feature and the weight axis kept) with
`OFL.txt` and a README holding the exact command; `app/src/fonts.css` (one `@font-face` by
`unicode-range`, Vazirmatn first in the text stack and before `monospace` in each code stack),
imported after `styles.css`; `fonts.test.ts`; `vite.config.ts` lets the tests read `fonts.css`.
No new dependency.
**Decided:** `size-adjust: 104.8%` puts Vazirmatn's alef at SF's cap height (0.6724 vs 0.7046
em, the same at every SF optical size); measured in the window: alef and H both 9.156 px at
13px. The code stacks repeat `styles.css`'s selectors so `styles.css` stays untouched.
**Verified:** headless WebKit, before and after: Persian cards draw in Vazirmatn at 90%, 100%,
150% and in a light theme; mixed lines keep the English line height; an English-only screen is
pixel-identical at 100% and 150% and never fetches the font; first paint median 178 ms before,
163 ms after (12 runs each, noise). Screenshots in the task's attachments.
**Open:** a Persian title is wider in Vazirmatn than in the system fallback, so it can wrap one
line sooner.
## 2026-10-15 — Shaping: the backlog emptied into 20 Ready tasks, and the open 1d+ estimates redone
**Did (vqa, gates coordinator, with Shapour agents):** every backlog note checked against main and
filed: mu-156 (takeover mail, R7 folded in), 157, 158, 159, 160, 161 (personal layer), 162, 163,
164, 165, 166, 168 (measured actuals in the shaping brief), 170 (EPIPE), 167 and 169 (sidebar rail,
search always shown, filters and Recently finished fold), 171 and 172 (working and token ribbons,
after mu-97), 173 (timeline legend), 174 (hover card); mu-106 shaped into a UX-flows spike. All in
Ready, vision checks recorded.
**Decided:** a token is input + output + cache writes (not cache reads) and `est-tokens` is redefined
to match (mu-172); the hover card is DESIGN.md's one exception, it repeats and never holds (mu-174);
mu-162 keeps a disabled View on rows whose transcript exists; test-only tasks word the needed
lenses "nothing to build, nothing may regress" instead of n/a. 29 open estimates of 1d+ redone from
measured actuals (p50 0.39x, median 1.1 h; e.g. mu-56 4d → 10h), tasks in Building only noted.
**Fixed:** none in code. A suspected test escape on mu-131 (a commit as `T <t@e.x>`) was a tester's
hand commit, not a test: that author is nowhere in the repo; finding withdrawn before recording.
**Verified:** `muster check` reports nothing on the new tasks; backlog empty.
**Open:** mu-131 is with its tester (kia-131); mu-132 onward wait for it to merge.
**Later the same night:** four more notes → mu-180 (design: Muster's move into Drover, blocked on
dr-0033/0034), mu-181 (`muster check-line`), mu-183 (migrate probe clears a stale scratch folder),
mu-184 (owner-only Release, claims re-stamped; send-backs keep the stamp, per the owner). Drover's
side of the move has a note in Drover's backlog.

## 2026-10-15 — Coordinator (Muster_Team_Lead_5): mu-87, 108, 99, 130, 102 merged; lead moves to QA_10 on cc-hg
**Did:** merged mu-87 (0e4d905), mu-108 (ba6b048), mu-99 (23ecba7), mu-130 (6036641), mu-102
(f07fc54); the last four by the owner's override after docs-only rebases of approved reviews
(WORKLOG conflicts resolved by keeping both entries). Committed mu-109's git-clean guard in the
personal layer (1cbb1ba, not pushed: no access). Ran builders, reviewers and QA as one-task sessions.
**Decided:** (overrule any time) mu-112: a second slot for one session is blocking, not should;
mu-101: past round 3 with an owner and a reviewer finding open, the whole task waits for the owner
(as built); `@block` reads kind words like `--on`; mu-110 R2: merge() records a branch already in
main with no worktree; mu-110's app Problems list stays CLI-only (backlog note); mu-103 R1: a later
answer reaches the current holder; mu-150: the font keeps its licence IDs and OFL.txt ships in the
bundle. Lessons L16, L17 accepted; L15 rejected (too specific).
**Fixed:** mu-87's merge push failed in the sandbox and left the task in Testing although merged;
recorded with `muster pass --from testing` once pushed.
**Open:** the owner is moving Muster to cc-hg (cc-mg nearly out of credit); QA_10 (cc-hg) is lead.

## 2026-10-15 — Coordinator (Muster_Team_Lead): mu-86, 88, 89, 90, 91, 93, 95, 111 merged; orchestration plan in the pipeline; headless QA
**Did:** merged mu-86 (b959c01), mu-88 (b7d15a8), mu-90 (306cc32), mu-89 (feca164), mu-95 (9b53b31),
mu-91 (cfa3268), mu-111 (5b1fd3e), the orchestration research and plan (2aa1c48, 89fef23; Pouya),
and mu-93 into the personal layer (0a237a2, owner's uncommitted edits kept). Shaped the whole
backlog into mu-86 to mu-105, the orchestration plan into mu-107 to mu-127 (Shapour_5), the QA loop
on the board (mu-128) and Persian (shaping). Ran builders, reviewers and QA as one-task sessions.
**Decided:** (overrule any time) decision numbers: 0012 accounts allowlist (mu-99), 0013 terms and
accounts (mu-107), 0014 gates (mu-129, owner's session), 0015 runner (mu-108); no rotating or
balancing across accounts (terms risk, research); Muster's own runner now, Drover's later; a
session not stamped with a personal account is never read (mu-99 R1); a quiet takeover of a
blocked claim needs a handoff or the owner (mu-90); a project's name never vanishes in the
sidebar (mu-87 round 3). Lessons L11, L12 accepted.
**Owner rules:** QA runs headless (Playwright WebKit with mocked IPC, harness in $TMPDIR/qah),
never input on the owner's screen; Muster only for now (dastafzar stopped); the lead starts and
stops sessions itself and never asks whether to build; lead handoff cap 600k.
**Verified:** each merge's gate green after the rebase; reviews recorded with mutants; QA
screenshots in attachments; owner's settings byte-identical after every QA run.
**Open:** builders often don't tick their own criteria (the lead ticks at merge from review
evidence); backlog notes from review nits (mu-88 R1, mu-90 R6-R8, mu-95 R3, migrate flake, band
titles, mu-93 item 5).

## 2026-10-14 — Clarify pass in Shaping: markers hold Ready, `muster clarify` sends a run's questions as one batch (mu-103)
**Did:** `[NEEDS CLARIFICATION: …]` markers in a write-up refuse `pass --from shaping` and warn on
`muster file`; a default-less open question holds Shaping too. `muster clarify [--json|--send]`
lists and escalates the open questions on Shaping and Ready tasks at once (`clarify.rs`,
`queue::escalate_all_locked`). Waiting for you groups them under "From shaping: N questions on M tasks";
notifications fold a task's questions into one line; pick-one questions read "going with (b)
unless you answer". The shaping brief, help, PROTOCOL and the manual (02, 04, 05, 07) say how.
**Decided:** the file keeps "With no answer: (x)" (contract v1 readers); only what's shown says
"going with (x) unless you answer". Markers inside code don't count, so docs can name the marker.
The owner's own tasks and blocked tasks stay out of the batch.
**Verified:** cargo tests (clarify, pass hold, file warning, CLI clarify, notify batch), vitest
WaitingList/PickOne, headless WebKit screenshots light and dark on a synthetic fixture
(attachments/0103/), bin/gate.
**Fixed (asghar_19, review round 1):** R1: an answer to a defaulted question went to the shaper's
mailbox, though the builder holding the task builds on the default → `answer_with` mails the
current claim's mailbox for a defaulted question (the asker's when nobody holds it). R2: `--send`
locked once per task and the app read without the lock, so a poll mid-send split the batch into two
notifications → one lock for the whole send, and the poll reads each queue's tasks holding its
lock shared (`read_state_settled`). R3: `--send --json` prints the sent tasks in the list's shape,
as help says. R4: manual 04's quote. R5: the marker needs its colon. R6: `markers` reads fences
with `merge_log::Fences`. R7: blocked tasks stay out of `clarify` as of the app's group. R8, R9:
tests for `escalate_all_locked`'s skips and for `muster file`'s stderr warning (bin/test_task.py).
**Verified (round 1):** `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`; the R1 and
R2 tests fail with the fix turned off.
**Fixed (Asghar_23, review round 2):** R10: the R2 fix had no test at the app's own readers → one
test pauses a send and reads through `projects::read_state` (the poll) and `waiting_in` (the
hidden badge); it fails if either goes back to the unlocked read. R11: PROTOCOL contract rules 4-5,
`help answer` and manual 02 say a defaulted question's answer goes to the holder. R12: `help
clarify` and both empty-batch messages say blocked tasks are left out. R13: `clarify`'s text kept
only the `> ` it added, so an option "> 100 MB" no longer prints as "100 MB". R14: manual 04 says
what "From shaping" groups. Also: in Testing and Your check the claim still names the builder, so
a defaulted answer went to the builder → `answer_with` takes `held-by` there.
**Verified (round 2):** see the pass's gate line; the R10 test fails with either call site reverted,
the Testing test fails before the `held-by` fix.

## 2026-10-14 — What's true now: a short page per area in docs/now/ (mu-102)
**Did:** `docs/now/<area>.md` pages (front matter `covers: <globs>`, one cited rule per line, 60
lines and 4 KB at most) and `now.rs`, which reads them (plain `*.md` only, bounded, symlinks
warned about) and matches them to a task's `touches:` with `parallel::touches_overlap`.
`muster pass --from building` is refused while a covered page is unchanged on the task's branch
(`git diff --name-only <merge-base>..<tip> -- docs/now/`), unless `--now-unchanged "<why>"`; the
log names the pages either way. `muster vision` quotes the task's pages as data (names only
without `touches:`) and looks for broken page rules; `--all` lists every page; `muster shape`
names the pages its matched tasks cover; `muster check` warns about over-cap, coverless,
duplicate-covers and symlinked pages. Muster's seven pages are seeded from decisions 0001–0011,
PROTOCOL.md, DESIGN.md and AS_BUILT. PROTOCOL, AS_BUILT, VISION, the manual (02, 05, 07) and
help.rs say so.
**Decided:** a task not started on its own branch has no diff to read, so a covered one needs
`--now-unchanged` (say where the page was updated) rather than passing unchecked; only committed
branch changes count, since the merge takes commits; a page the branch adds counts as updated.
The app's Pass sends no reason, so a covered task leaves Building from the command line or the
owner's track move, and the owner's refusal says so.
**Fixed (review round 1):** a branch that updated a Persian-named page was refused as unchanged →
`git diff --name-only` octal-quotes non-ASCII names, which never matched `read`'s names → the
diff runs with `-z` and splits on NUL (R1). The worktree-gone path (the branch read from the
project's checkout, or named missing), a `docs/now` linked inside the repo, and the AC1 citation
check (now a task key, decision, or doc section, with a negative case) got tests (R2–R4); the
owner's refusal adds "or move it to Testing on the track" (R5).
**Verified:** gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati)); 20
tests in `now.rs`, 5 in `vision.rs`, 1 in `backlog.rs`, 5 in `tests/now_pages.rs` (the built
binary, and Muster's own pages checked in the gate). The R2 and R4 tests fail on the reviewer's
mutants (`tip = None`, `!is_link` dropped).
**Open:** AC9, the owner's read of the board and claims pages.

## 2026-10-14 — Research: visual-QA drivers, capture and walkthroughs per app stack (mu-130)
**Did (Pouya, then Pouya_2 for review round 1):** `docs/research/2026-10-14-visual-qa-drivers.md`
answers the six questions and the 19:43 scope (iOS via simctl and XCUITest, the Playwright overlay
via `addInitScript`/`exposeBinding`, codegen, masks, traces, Mac snapshot tests, keyframes) with
cited, dated findings, a TCC table, and a recommendation per stack that obeys GATES.md G4: agents
run headless (Playwright with the IPC or preload mocked; a simulator with no window; Mac snapshot
tests), capture only windows already open, and open a window only in a walkthrough the owner
starts. The probes behind every [local] result are in `docs/research/assets/2026-10-14-visual-qa/`.
**Verified:** probes rerun on macOS 26.5: repeated captures of an on-screen 2× window differ in 0
of 8,056,320 px by `screencapture` and ScreenCaptureKit (an off-screen 1× window: 1,810 px, at most
3/255); off-screen SwiftUI snapshots make 0 windows and repeat
exactly; the injected overlay survives a navigation and its verdicts reach the runner; WebKit video
is WebM/VP9 at 25 fps; a headless simulator boots, takes a deep link and records H.264. shellcheck
clean; probes build with no warnings.
**Fixed (Pouya_3, review round 2):** §6 said a 1× window also captured with 0 px of noise → the
rerun found 1,810 px at most 3/255 → §6 reports it and narrows "no noise" to the on-screen 2×
window. §4 recommended granting Screen Recording to the terminal → it now recommends a separately
signed capture helper that alone holds the grant and captures only the fixture instance's window,
and names the terminal grant's risk. The Mac capture route gets GATES.md's fixture rule. Probes:
`sck-probe rec` exits 1 on failure, `capture.sh` survives an unreadable video, `simctl-probe.sh`
takes a device type and prints its pixel diff, `playwright-probe.mjs` prints codec and frame rate.
**Decided:** a signed helper, not the terminal, holds Screen Recording, because a terminal grant
covers every process started from it; the helper being its own responsible process when launched
as an app rests on Apple DTS's rule and is mu-141's to confirm.
**Open:** the AX walkthrough driver, XCUITest on the simulator end to end, codegen on WebKit, and
the renderer-in-Chromium route for choopan aren't prototyped (mu-133, mu-141); off-Space recording
wasn't reproduced; the round-2 probe additions weren't rerun.

## 2026-10-14 — mu-99: Muster reads only the allowed personal Claude accounts, never ~/.claude
**Did:** one allowlist of account labels in Settings (`claudeAccounts`, default mg and hg) and
`accounts.rs` resolving it. `claude agents` runs only for allowed accounts with
`CLAUDE_CONFIG_DIR` set (app and `muster list --json`). The transcript reader takes only the
allowed accounts' `projects` and only `~/playground`-keyed project folders. The helper offers only
allowed accounts, and its login script always exports `CLAUDE_CONFIG_DIR`. Settings › Agents has
a row to add and remove labels, and About shows the allowed folders.
**Decided:** the label allowlist plus the playground-key filter, because the accounts share one
linked `projects` folder on this Mac
([0012](decisions/0012-muster-reads-only-the-personal-claude-accounts.md)).
**Fixed:** review: a `~/.claude-<label>` linked to home (or any folder holding `~/.claude`) was
allowed, so claude would read `~/.claude.json` through it → `accounts::resolve` refuses it too.
Review round 1: R1 (owner's call) a session is read only when its stamp carries an allowed
account, so a bare-claude or pre-0030 stamp gives no tokens, live dot or transcript ("not read"
in Work); R2 the helper picker refreshes when the list is saved; R3 entry-point tests read the
list from settings.json; R4 the row's save path is tested; R5 the cached session states drop a
removed account at once; R6 every transcript read takes its accounts (no shared setter);
R7–R8 the "none" wording and strings table.
**Verified:** `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`; AC2
tests fail with the filter disabled, including FIFO `settings.json` and `.claude.json` that a read
would block on; a correctness, security and privacy review.
**Open:** A sibling such as `~/playground-x` passes the key filter (the key is lossy; recorded in
0012). An unread session's Work row still shows a disabled View (backlog note). mu-98 was edited
to read through `Reader`'s filter (AC6). AC5 passed by QA_9; rebased on ba6b048 (mu-87, decision 0015).

## 2026-10-14 — Decision 0015: Muster runs agents behind a runner interface, Drover's runner later (mu-108)
**Did (gholi_12, builder):** `docs/decisions/0015-muster-runs-agents-behind-a-runner-interface.md`:
the six runner methods, the five MCP Tasks run states, the boundary (0002's scrub, the owner's own
login, no token, 0013 rule 1), Codex analytics off, three options, what would reopen it. 0007 and
0005 say they're amended by it; PROTOCOL's review-of-record and vision-check notes, AS_BUILT's
`muster review` paragraph and VISION's Drover sentence name it; AS_BUILT's decisions table lists it
(and drops "There is no 0012 yet"). mu-9's AC1 has a comment pointing to it.
**Decided:** numbered 0015, not the brief's 0013 (mu-99 took 0012, mu-107 0013, mu-129 0014 for
the gates); the docs say "decision 0015" where a bare 0015 would read as task 0015. Decided by the coordinator for the
owner, overrule any time.
**Open:** AC3 is the owner's read of the record. AS_BUILT's decisions table will need the 0012
row from mu-99 when it merges.
**Fixed (gholi_14, round 1):** R1: 0007's amendment line sat inside its ownership table and cut
it after Code; it's below the table now. R2: the record's point 2 says a rate limit cools the account down
until `resets_at` and the work waits, never another account (0013 rule 2); point 5 cites 0013 rule
6; the header cites rules 1, 2 and 6. R4: AS_BUILT's `muster review` sentence no longer says "runs
no agent" twice. R3 (merge after mu-99 and mu-107) is the lead's. Renumbered 0014 → 0015 on
rebase, since main's 0014 is the gates record (mu-129); AS_BUILT's decisions table lists the gates
0014 row before it.
**Rebased (gholi_19):** on main after mu-107's 0013 merged; the table runs 0011, 0013, 0014, 0015,
and its line says only 0012 (mu-99) isn't merged yet.
**Fixed (gholi_19, round 2):** R5: 0015 cited the brief at `tasks/proposals/`, where it isn't; it
cites the main checkout's `.muster/proposal/` now. R3 is settled by the rebase (0013 is on main).

## 2026-10-14 — mu-87 round 3: a project's name never vanishes, at any sidebar width
**Fixed:** in Focus at the default 210px drover's row (stalled, running, 2, ⏸︎ 1) drew its name
0px wide up to about 229px (QA_8, headless WebKit) → the 4em-and-wrap rule only held under
`data-tight`, below 210 → it holds at every width: `.side-line` wraps, and the name is
`flex: 1000 1 auto; width: 4em; max-width: max-content`, so it claims min(4em, its own width) and a
short name ("All") never wraps for nothing. Focus's "stalled" moved into `.side-badges` as its first
piece (3px end margin, 8px as before), so it stays by the badges, where it sat. `data-tight` and
`paneSize.sidebarTight` are gone (nothing else used them).
**Decided:** the lead's call: rows that fit stay as they were; only a row whose name would vanish
changes at 210. The name's box now ends at its text (`max-content`), the price of not wrapping
short names: a double-click to rename lands on the name itself, not the space after it.
**Verified:** headless WebKit (QA_8's harness, vite build of this tree, real-state snapshot) at
150, 210, 229, 240, 320, Focus and ADHD, Violet night and day: no name under min(4em, its width),
nothing past the sidebar's edge; at 240 (ADHD) and 320 pixel-identical to round 2; at 210–240 the
only changed row is drover's (badges under its name), the rows under it moved down by that line.
Tests: ProjectList and Display updated.

## 2026-10-14 — mu-87: sidebar rows hold at 150px, needs-you digits 4.5:1 (round 1 fixes)
**Did:** `.count.warn` digits in `--on-status`; project rows keep every badge whole, the name
cut first, the check-in button wrapping inside the sidebar (`styles.css`, `ProjectList.tsx`).
**Fixed (review round 1):** R1 at 210px a running row wrapped → the 4em-and-wrap rule applied at
every width → it applies only under `.sidebar[data-tight]`, which App.tsx sets below 210px
(`paneSize.ts` `sidebarTight`). R2 a Persian name sat away from its dot → `text-align: start`
under `plaintext` → `.side-name { text-align: left }`. R3 a test for Focus's "being worked on
now". R4 the description now holds the real queue name and folder (hidden span) before the
activity line. R5 "1 needs you" on the row's label and badge.
**Decided:** scope the wrap rule to a tight sidebar instead of narrowing the name's basis. A
basis small enough for drover's running row at 210 (about 1.5em) would cut names to "m…" at
150px, where the brief wants about 4em. Filed a backlog note: board bands still say "1 need you".
**Verified:** `bin/gate`: green (pnpm check, test_task, test_commands, cargo test); vitest 1716.
Re-rendered round 1's static harness (Chrome for Testing, not the release app) with the new CSS:
at 210 every row matches the pre-0087 render (drover one line, name 1.6em), the Persian name sits
by its dot; at 150 the badges go under the name, whole, the check-in button inside.
**Open:** release-app (WKWebView) screenshots for AC2 and AC3 are the tester's.

## 2026-10-14 — edit keeps a task's Questions; start and init say when the merge driver isn't set; glance swaps $HOME only at a path boundary (mu-94)
**Did (gholi_13, builder):** `ops.rs` `cut_questions`/`put_back_questions`: `muster edit` and the
app's Edit leave the tool's `## Questions` out of the write-up wherever it sits and put it back
byte for byte before the `## ` line that followed it; a new `## Questions` line is refused.
`merge_log.rs` `missing`, `wanted`, `not_set`, `fix_lines`: a failed `configure` ends `start` and
`init` on stdout with git's reason, what it means and the two `git config --local` lines;
`muster check` warns while `.gitattributes` uses `merge=muster-log` and a key is missing.
`index.rs` `home_as_tilde`. PROTOCOL.md, AS_BUILT.md (mu-52 cites `5d1329b`), help, manual.
**Fixed:** a Questions section asked before the vision check sat inside the write-up
(`body_span` ends at Vision), so saving an edit without it deleted it → cut it out of the
editable text and put it back. `glance_md` did `s.replace(home, "~")` → `/Users/imassih` became
`~assih` with `$HOME=/Users/im` → swap only at a path boundary.
**Decided:** the CLI tests block the config with a held `.git/config.lock`, not a read-only
`.git/config`: git replaces the file through its lock, so read-only doesn't fail; the lock gives
the sandbox's "could not lock config file". The reason shown is the words after git's last `: `,
so no path is printed.
**Verified:** `edit_keeps_questions` 4 tests (before and after a checked Vision, mid write-up,
a new heading refused, Persian kept), glance 3, merge_log 19 (the printed lines run through `sh`
set what `configure` writes), `WorkLogMergeDriver` 5 in test_commands.py; `gate: green (pnpm
check, test_task, test_commands, cargo test (Kiritimati))`.

## 2026-10-14 — Decision 0013: orchestration's terms and accounts (task 0107)
**Did:** `docs/decisions/0013-orchestration-terms-and-accounts.md` holds the plan's six terms rules
(own client and no tokens, no balancing across accounts, no company account, capped and paced runs,
each user's own login, telemetry off), each with its clause and date from the research, the three
options and the choice; capacity is read, then learned per run. PROTOCOL.md's Sessions, VISION.md's
principles and AS_BUILT.md's decisions table link it; the brief's "Load balancing" now opens with
"Replaced by decision 0013".
**Decided:** numbered 0013, not 0012, because mu-99 already writes 0012 (the account allowlist); the
plan doc's 0.1 and 3.2 say 0013 and 0.2 (mu-108) says 0015, since mu-129's gates record took 0014
(the lead's call). Decided by the coordinator for the owner; the owner can overrule it.
**Fixed (review round 1):** the renumbering hadn't reached the dependent tasks → mu-113, mu-116,
mu-117, mu-121, mu-122, mu-126 and mu-127 now say 0013 for the terms and 0014 for the runner (through
`muster edit`); mu-9's numbers sit in a comment, so a new comment corrects them. The record says Gas
Town rotates accounts at a limit, and that `rate_limit_event`'s fields sit in `rate_limit_info`, in
camelCase on the wire (mu-111).
**Fixed (review round 2):** mu-129 merged its gates record as 0014, so the runner record is 0015 →
the plan doc's 0.2, mu-117, mu-126 and mu-127 say 0015 (`muster edit`), mu-118 and mu-123 say 0013 for
the terms, and a comment on mu-9 corrects the earlier numbers. mu-108's own write-up is gholi_14's while
they hold it, so a comment there lists its renumbering.
**Verified:** six `### Rule` headings and `resets_at` in the record; `grep 0013` hits all four files.
**Open:** AS_BUILT.md's decisions table will conflict with mu-99's 0012 row at merge (keep both rows,
0012 first). The brief lives in the retired, untracked `muster/tasks/proposals/`, so its line isn't
in git.

## 2026-10-14 — Spike: how Claude Code reports 5-hour and weekly use on this Mac (mu-111)
**Did:** short `claude -p` stream-json runs and interactive sessions with a throwaway status line,
on Claude Code 2.1.296; findings posted on mu-9 (three comments). Fixtures for mu-113 and mu-117 in
`app/src-tauri/tests/fixtures/` (`stream/claude-rate-limit-allowed.jsonl`,
`stream/claude-not-logged-in.jsonl`, `statusline-rate-limits.json`; round 1 added the synthetic
`stream/claude-rate-limit-over-cap.jsonl` and `statusline-rate-limits-over-cap.json`).
**Decided:** `rate_limit_event` comes at least once on a run that gets an API response, then only
when a window's rounded percent or reset time moves (at most every 30 s), with camelCase fields and
per-window use under `unifiedWindows` (not the research's snake_case); parsers key on that and
ignore unknown keys. `utilization` can pass 1.0 and the status line's `used_percentage` is a
one-decimal number that can pass 100 (round 1, R1): parse f64, keep over-cap readings, clamp only
for display. The rejected shape is taken from the binary, not forced live (Shapour's call).
**Verified:** `grep -rE '/Users/|[0-9a-f]{8}-[0-9a-f]{4}-'` over the new fixtures finds nothing;
`gate: green (pnpm install, pnpm check, test_task, test_commands, cargo test (Kiritimati))`.
**Open:** cc-hg isn't logged in; the Codex CLI isn't installed, so the Codex part is mu-126's. A
rejected run hasn't been seen live.

## 2026-10-14 — The task pane's Files opens screenshots in place, not in a lightbox (mu-91)
**Did:** Files' thumbnails are toggle buttons that open their image at the pane's full width,
in a row under the two columns (`FilesOpen`), several at once; a second click on the thumbnail or
the image shuts it; the board stays beside it. Files and Your check's proof lines draw their
thumbnails with one shared `ImageThumb` (StageContent.tsx) and `toggled`; a proof image still
grows in place in the main column, Files' thumbnail stays small and marked open (`aria-controls`
the row). TaskPanel holds Files' open set. The lightbox, its CSS, its
window keydown handler and its ← →/Esc shortcuts are gone (shortcuts.ts, manual 06). The open
image's alt is its file name and the name under it is `dir="auto"`, in both places. Manual 04 and
AS_BUILT say so.
**Decided:** kept the proof thumbnail's look (`--bg` with a `--line` border) for Files' thumbnails
rather than `--panel`, so the two stay one component; the large image's frame is `--panel`; Esc now closes the pane even with an image open, since
nothing is left to escape from.
**Fixed:** round 2 (QA_5): Files' images opened inside the 176px side column, ~160px wide and
unreadable → the image grew in place in `.tp-side` → it opens in the full-width row instead.
Round 3 (review R1–R5, QA_6): shutting an image from the large one dropped the focus to the body,
its button being gone → `FilesOpen` focuses its thumbnail (`thumbId`). An image that opened below
the fold stayed out of sight → `fileToggler` scrolls its figure into view after the render
(`showSoon`: `block: "nearest"`, not smooth under reduced motion, again once the image loads). An
open thumbnail lost its accent border on hover and focus → a `.tp-side` rule keeps it. Tests now
cover the row right after the columns and `.files-open[hidden]`.
**Verified:** `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`;
StageContent and TaskPanel tests 88/88 (round 3); `grep -rn lightbox app/src` finds
nothing.
**Open:** AC3's release-app screenshots (light and dark, ADHD) are QA's.

## 2026-10-14 — mu-95 round 1: the logout stand-ins' delays run, and the cap after the mark is pinned
**Fixed:** R1, the two-write and logout stand-ins called `sleep` after setting a PATH without
`/bin`, so on macOS it exited 127 at once and the drain and split-read mutants came down to a race
→ `/bin/sleep`, and each test asserts the delay took place (≥ 0.3 s, ≥ 0.2 s).
**Did:** R2, a `/usr/bin/yes` logout stand-in pins the 64 KB cap after the mark: the PATH comes
back well within the wait.
**Verified:** each test now fails on its mutant (no drain, the cap counting `buf` instead of
every byte (Timeout at 20 s), a current-read-only `has_mark`); `cargo test --lib run::tests::` 25
passed; gate below.

## 2026-10-14 — mu-95: Run's login PATH survives a logout file; the lock test and the PATH guards fail only when they should
**Fixed:** the merge lock test timed out under load → in a unit test the merge's
`merge_log::this_muster()` builds the muster binary once per run (`cargo build --bin muster`),
before the lock, so after a lib change under load the test's 20 s wait for "waiting for another
merge…" timed a cargo build (no race in `parallel.rs`) → the test builds it before the wait, waits
on the word with a 60 s backstop, and says what the merge did if it never comes. Repro: lib
touched, 36 `yes` on 18 cores, `cargo test --lib`: Timeout after 24.9 s; with the fix it passes.
**Fixed:** a logout file printing after the PATH killed the login shell by SIGPIPE and its PATH
was thrown away (mu-79 R5) → `run::login_path` drains the output after the end mark (to its end,
64 KB or `LOGIN_WAIT`, by `poll`) and keeps a marked PATH whatever the exit status.
**Did:** `run::has_mark` (the end mark split across reads, R6); stand-ins for a two-write end mark
and for the shell leading its own session without a terminal (`getsid`, R7). AS_BUILT's Run
section says so.
**Verified:** lock test 20/20 at load 49-147 with the lib stale before each run; each new guard
fails on its mutant (no drain, the exit-status check, a current-read-only scan,
`process_group(0)` in the sandbox); `gate: green (pnpm check, test_task, test_commands, cargo
test (Kiritimati))`.
**Open:** a separate flake, `migrate::probe`'s scratch folder named by pid and µs, filed as a
backlog note.

## 2026-10-14 — Gates: the design that takes the owner out of Your check (mu-129 to mu-141)
**Did:** `docs/GATES.md` lays out four gates checked at the branch's tip: build, code review,
criteria (`ac-verifier`) and visual QA. Visual QA follows the owner's proposal
(`.muster/proposal/Proposal Visual QA and Walkthrough QA in Muster.md`): reference QA against
approved baselines and mockups, and walkthrough QA where a screen has neither. A task that passes
is accepted without the owner. Filed as feature mu-129: mu-130 (research, already with Pouya;
scope added as a note), mu-131 gate records, mu-132 criteria, then the proposal's six stages:
mu-133 web walkthrough, mu-134 Auto/Mixed and video, mu-135 baselines and screen map, mu-136
reference QA, mu-140 learning, mu-141 iOS and Mac. Then mu-137 accepted by gates, mu-138 the
sweep of Your check, and mu-139 Kia running it all in dispatched Testing. PIPELINE.md's Gate row
points at GATES.md.
**Decided:** decision 0014, with defaults for the proposal's open questions (baselines in the
repo, 2 passes, walkthrough until baselined, an injected overlay). A first draft that had no
walkthroughs, used main's capture as the baseline and added a separate role was replaced the same
hour, once the owner's proposal arrived.
**Verified:** `bin/gate` green before the rework (docs only since). `muster check` reports nothing
on the new tasks.
**Open:** Lovable at work is the owner's call (WAITING-FOR-OWNER.md). Mac capture needs a one-time
Screen Recording grant (mu-141).

## 2026-10-14 — icon.icns holds every size; the menu bar shows the count beside the head (mu-89)
**Did (Akbar_7, builder):** `tray.rs` sets the status item's title to the waiting count ("3",
empty at 0) with the look, tooltip and menu (`shows(n)`, one `Bar::set` per count change);
`icons/build-icons.sh` builds `icon.icns` with `iconutil` from the owner's iconset renamed to
`@2x`, so it holds 16 to 512@2x instead of actool's 16 and 128. DESIGN.md, AS_BUILT.md, manual.
**Decided:** Assets.car isn't recommitted: actool writes a different file on each run, and this
task doesn't change it. The ten-size check is a Rust test reading the icns chunk table, so it
needs no `iconutil` and never skips.
**Verified:** tray tests 12 green (the icon test fails on the old icns); `iconutil -c iconset`
lists ten images; build-icons.sh run twice gives a byte-identical icon.icns; gate below.
**Open:** AC3's light and dark menu bar screenshots are Testing's (the release app isn't run here).

## 2026-10-14 — mu-90 review round 1: takeover refuses a paused claim and a quiet blocked one
**Fixed:** (R1) `claim --takeover` took a paused claim after 30 quiet minutes → it never checked
`paused:` → refused for everyone, as every other claim path does; a blocked claim (quiet by
design) is taken only after a handoff or by the owner (lead's call). (R5) a label with `: ` wrote a
takeover line the log never read back → the line is read back first and refused. (R2) a teammate's
exported `MUSTER_AGENT` reached the tests → popped in `bin/test_task.py`, `bin/test_commands.py`,
and unset in `bin/gate`. (R9) PROTOCOL gave two first commands → `claim --takeover` first, then
`inbox --session <old>`.
**Did:** tests that can fail for the stale-handoff resets (R3; both resets mutated, each caught)
and for `check_recorder` refusing a predecessor (R4; mutated, caught). Docs: PROTOCOL, AS_BUILT,
COMMANDS, manual 02 and 05, help.
**Verified:** gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati));
`MUSTER_AGENT=a1b2c3d4` test_task and test_commands OK (were 9 failures, 4 errors).
**Open:** R6–R8 (nits) are backlog notes `2026-10-14-1925-mu-90-r6/r7/r8-…`.

## 2026-10-14 — mu-90: a successor takes over a handed-off claim and its mail; teammates get an agent stamp
**Did:** `muster checkin <id> "…" --handoff` logs `check-in · handing off · …`; `muster claim <id>
--takeover` moves a Building claim to a new session of the same label after that check-in, after
30 quiet minutes, or always for the owner, logged as `took over the claim from <old> (…)`, with
the task's mail and asks.json entries moved to the new mailbox. A teammate's stamp carries
`agent <8 hex>` (from `--agent` or `MUSTER_AGENT`) and its mailbox is `mail/<session>/<agent>/`;
answer-hook.py delivers it by the hook's `agent_id`; ping and pause reach it there; `lib.ts`
reads the part. Files: session.rs, ops.rs, queue.rs, cli.rs, ping.rs, pause.rs, review.rs,
help.rs, bin/answer-hook.py, app/src/lib.ts; docs PROTOCOL, AS_BUILT, COMMANDS, manual, 0008, 0011.
**Decided:** the agent id comes from `--agent` or `MUSTER_AGENT`, because no harness env var
carries it (a subagent's shell sees only its lead's session id); a PreToolUse hook could set it,
none is built (decision 0008 update). Every holder in the claim's chain (claimer, each takeover)
counts as a builder, so neither a successor nor its predecessor may test or review. A hook call
with an invalid `agent_id` gets no mail rather than the lead's. Quiet means heartbeat age only,
so a blocked or paused claim can go quiet too.
**Verified:** cargo test (lib 763 passed), clippy -D warnings, fmt; test_task.py 63 OK,
test_commands.py 22 OK; pnpm test 1695 passed; bin/gate green.
**Open:** AC6 (a real successor takes over and passes) is proved by this task's own handoff.

## 2026-10-14 — projects.json keeps its bytes when nothing changed; the fold and move paths tested (mu-88)
**Did (gholi_9, builder):** `projects::Config`'s maps are `BTreeMap`s (`projects.rs`, `labels.rs`,
`check_limit.rs`, `arrange.rs`), so folding then unfolding a heading leaves projects.json
byte-identical; the first save after this sorts the keys once, no migration. New Rust tests: the
byte-identical fold round trip (24 projects, labels, categories incl. Persian, colours),
`headings_now`'s `ok: false` twin (a heading only gone folders are under loses its fold, and the
folder coming back doesn't revive it), and `read_state` passing its settings through. The
sidebar's and Settings' handlers moved into `projectOrder.ts` as `placeSteps`, `foldHeading`,
`readBack` and `runMove`; `ProjectList.tsx` and `Projects.tsx` call them, and six tests drive them
with fake setters and resolving or refusing saves. The source pins in `ProjectList.test.ts` now
name the new calls.
**Decided:** the lifted functions take the setters and the save as arguments, as `track` does, and
never reject (the refusal goes to the problem line); Settings keeps its own `settle` effect.
**Verified:** mutation runs: 11 cuts inside the lifted functions (shownLayout, track, trackFold,
trackUnfold, settle, readFolds, the said and problem texts, the fresh heading) and 4 cuts of the
components' calls, all 15 killed; the Rust twin fails without `&& p.ok` and the settings test
without `settings`. The fold round trip failed before the map change.

## 2026-10-14 — The +N more cue walks every chip in board order at a 400px board (task 0086)
**Fixed:** at a board of 400px or less the cue's clicks landed on the same chip, or skipped a plain
chip after a needs-you one → ColumnRow passed the last target to `overflowCue` only while the view
measured it in view or cut, and with nothing fitting beside the cue it often wasn't, so each click
counted as a walk's first → ColumnRow keeps the target as it is (`ColumnRow.tsx`), and `overflowCue`
walks a target that's no longer a chip on from the list's start (`rowCue.ts`). DESIGN.md and
AS_BUILT.md say "measured in view or not".
**Verified:** `columnRow.test.ts` 51 passed, new cases for `visible = []` (every chip once a round,
wrap, a gone target); `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`.
**Open:** AC3's screenshots at a ≤400px board in the release app are Testing's.

## 2026-10-14 — Citation audit of the orchestration research (mu-9)
**Did (Pouya, researcher):** a fresh agent checked eight key claims against their sources; five held,
three were corrected in `docs/research/2026-10-14-orchestration.md`: the 2025-09-15 Usage Policy
bullet already had the "identical or similar inputs" words (only "create multiple accounts" is new),
status-line `rate_limits` also reaches Claude apps gateway users, and openai/codex#14728 was closed
for lack of upvotes with GitHub's reason "completed". Gaps updated; no change to the plan.
**Verified:** quotes matched against the fetched pages, the Claude Code changelog and codex source.

## 2026-10-14 — Research and a plan for orchestration (mu-9)
**Did (Pouya, researcher):** compared the owner's orchestration brief with current orchestrators,
frameworks, standards and the providers' terms (9 web-researcher workers); wrote
`docs/research/2026-10-14-orchestration.md` and `docs/plans/orchestration-plan.md` (phases 0 to 4,
work items with criteria, estimates and depends; Pouya added as the Researcher role).
**Decided (proposed, needs the owner):** drop the brief's soonest-reset-first balancing across
accounts (terms risk); read usage from Claude Code's `rate_limit_event` and status-line
`rate_limits` instead of learning it; OpenTelemetry GenAI names for the ledger; decisions 0012
(terms and accounts) and 0013 (who launches runs) come first.
**Open:** no fresh-agent citation audit was run; OpenAI's terms read from the Open Terms Archive copy.

## 2026-10-14 — The lead's handoff cap is 600k (owner)
**Decided:** the owner raised the `Muster_Team_Lead*` token cap from 300k to 600k
(`~/playground/.claude/handoff.json`, layer commit fd6d4ff); PROTOCOL.md and decision 0011 say so,
and PROTOCOL's naming example uses `cc-mg-new -n`, not the company `claude`. Builders stay at 150k.
**Did (coordinator):** removed 39 finished Muster sessions; stopped stuck Drover and setup
sessions and, on the owner's word, dastafzar-round5/6 (Muster only for now); shaped the backlog
into mu-86 to mu-99; builders on mu-99, mu-90, mu-93, mu-86, mu-91.

## 2026-10-14 — Coordinator (Muster_Team_Lead): mu-75, mu-79, mu-82, mu-85 and mu-84 merged; the lead runs its own sessions
**Did:** merged mu-75 (aba4fcc), mu-79 (19a336d), mu-82 (07d7922), mu-85 (dfb92dd) and mu-84
(8a39448), all in Your check; rebuilt the release app and CLI from main and restarted the
running Muster. Took over from Muster_Team_Lead_4 on the owner's word and stopped it, Lead_3 and
the builders stuck on permission prompts; the lead now starts every builder, reviewer and QA
session itself, in its own permission mode, so nothing waits on the owner (memory: lead owns
sessions). One task per builder session (decision 0004): Akbar (mu-84), Gholi_2 (mu-85),
Asghar_5 (mu-82), Asghar_7 (mu-79's tests), Reviewer and QA sessions, each with successors at
the context cap. Lessons L11 and L12 accepted.
**Decided:** (overrule any time) mu-82: no number without its words; the name gives way first,
and a heading wraps to a second labelled line only where one line can't hold "N need you".
mu-84: a column opened from a chip stays open in that mode until folded; when chips overflow, a
fixed "+N more · K need you" cue walks the hidden chips in board order and wraps to the start.
mu-79 AC4 taken on a split proof, then confirmed after the merge (Run → Stop ends the second
Muster). mu-85's R1 nit fixed before the merge. Every successor's claim moved by `muster return
--as owner` (backlog 2026-10-14-0707); successor names can clash (backlog note).
**Verified:** each merge's gate green after the rebase on main; release-app QA by QA sessions
(window-only screenshots in attachments/0079, 0082, 0084, 0085); projects.json, settings.json
and the webview's storage byte-identical after every QA run. mu-79's merge gate first went red on
two load-sensitive run:: tests (load average 49-230 from parallel builds); a test-only commit
made them robust, proven by five mutants.
**Open:** AC5 of mu-82, mu-84, mu-85 (the owner's look). QA_3's clicks landed on the owner's
Teams window for a moment (4 clicks); QA now refuses to click unless Muster is frontmost.
Backlog notes: mu-79 nits R5-R7, mu-84 cue nits, the 150px sidebar rows outside mu-82,
projects.json key order, mu-85 icon nits, the parallel:: load-sensitive test.

## 2026-10-14 — Closed and empty board columns are one row of chips above the board; show or hide each column (mu-84)
**Did:** the plain board draws only its open columns; every empty, folded or hidden place (Paused,
a stage, Recently finished) is a chip in one row under the filter bar, in board order, with its
dot, name, count, needs you and blocked (`ColumnRow.tsx`). A chip opens its column in place; an
open column's head, the Paused lane's and Recently finished's have **Fold**. `modes.ts`
`plainBoard`, `pickColumn`, `loadPicks`/`storePicks`; `App.tsx`, `PausedLane.tsx`, `Recent.tsx`,
`ModeViews.tsx`, `styles.css`; DESIGN.md (board section), manual 04, AS_BUILT.
**Decided:** the owner's choice A (over B, short stubs, and C, empty columns open: DESIGN.md).
Opens and folds are remembered per display mode on this Mac (AC2), not only until the mode is
left as the task's prose had it for Focus; they're stored only where they differ from the mode's
own fold, so Focus's folds stay its defaults. Group by bands keep mu-83's strips (out of scope);
a column hidden on the plain board folds there too, and the row then holds only Recently finished.
**Verified:** `columnRow.test.ts` (20 tests: every mode's open/row split, chip markup and words,
labelled group, RTL-safe CSS, open/fold/persist round trips, broken storage); the gate's line is
in the task's pass.
**Fixed (review round 1, Akbar_2):** R1 the row wrapped, so a poll adding a badge moved the board's
top → it was `flex-wrap: wrap` with only a min-height → one fixed 34 px line (`nowrap`); names
shorten through a one-line clamp (min 2.5em) while counts stay whole, then it scrolls sideways
(measured in headless Chromium from 1000 px to 300 px: one line, no count spills). R2 Recently
finished's chip counted every kind while its list showed the remembered kind → App keeps the kind
(`Recent` is controlled) and `weekCount` takes it. R3/R4 chip figures and the grouped fold click
were untested inline code → `rowChipsOf`/`placeCount` (`ColumnRow.tsx`) and `bandFold`/`toggleBand`
(`modes.ts`), tested, including the folded Paused lane's needs-you count. R5 "Every column is
open" under Group by → said only on the plain board. R6 a test that couldn't fail → exact open/row
lists for a hidden full column and a shown empty one.
**Fixed (review round 2, Akbar_3):** R7 past its room the row scrolled with no cue, so in Focus
at 690 px Ready, Done and Recently finished, and in a nine-chip row three needs-you badges, were
out of sight → the chips scroll in a list of their own and a cue after it, which never scrolls
away, says "+N more · K need you" and scrolls to the hidden chip that needs you (the lead's call;
shorter badges turned down). `rowCue.ts` (`inView`, `overflowCue`, `measureRow`, `watchRow`).
Measured in headless Chromium (Playwright, real frames) at 400 to 1100 px, LTR and RTL: the cue
counts what's hidden, stays in the row, its clicks walk to the end and back, it goes when the
window widens; the measured spans are the fixtures of `columnRow.test.ts` (43 tests).
**Fixed (QA and review round 3, Akbar_3):** R8 the cue's clicks stayed on a chip wider than the
row (Ready, 264 with 7 need you and 9 blocked, at 1400 px with the pane open) → the walk went on
only from chips wholly in view → it goes on from the last click's chip while that's partly in
view (`cutBy`, `overflowCue(…, from)`). QA's braindump Focus row showed "Rea…", "Buildi…" and
Paused as its icon → the clamp broke only between words and names shortened even when the row
scrolled anyway → `word-break: break-all`, and names shorten only when that makes every chip fit
(`leastRoom`), else they stay whole and scroll. Measured in headless Chromium: 1100 px whole,
1000 px shortened just enough, 900 px whole with "+3 more"; at 400 px the walk goes Ready,
Building, Your check, Recently finished, then back, LTR and RTL; 47 tests.
**Fixed (QA_4, Akbar_4):** past the end the cue went back to the first hidden chip that needs you
(Ready), so the chips before it (Paused, Backlog, Shaping) were never reached → the wrap preferred
needs-you over the start → only the first click (no `from`) goes to what needs you; after it each
click steps in board order to the next hidden chip and wraps to the first. The row also kept the
last project's scroll → it's keyed by project and mode (`key={queue/mode}`, as `Recent` is).
Tests: the `visible = []` walk over Focus's six chips twice, and a 10-chip 1400 px row with a
560 px Ready: each round reaches each target once, in board order, and shows every chip; 49 tests.
**Open:** AC4's release-app screenshots and AC5's look are for Testing and the owner.

## 2026-10-14 — Muster wears the owner's new icons: app icon light and dark, sidebar logo, menu bar icon (mu-85, Gholi_2 → Gholi_4)
**Did:** the owner's icon set is in `app/src-tauri/icons/original/`. The app icon follows macOS's
light or dark look through an Icon Composer `Muster.icon` compiled to `Assets.car`
(`bundle.macOS.files`, `Info.plist` `CFBundleIconName`), with `icon.icns` and the PNGs rebuilt by
`icons/build-icons.sh`. The sidebar's logo is the same icon as SVG, light or dark with the theme's
mode. A menu bar icon (`tray.rs`) is idle or waiting on the Dock badge's count from the same reads,
says the count in its tooltip and menu, brings the window forward on a click, and has Show and Quit.
Settings › Notifications has its On/Off (`settings.json` `menuBar`).
**Fixed:** turning the menu bar icon off stopped the app (SIGTRAP in `NSStatusBar
_removeStatusItem` → the tray was dropped on the poll's thread, and AppKit removes status items
only on the main thread). Review R1: each Off then On added another menu handler, so Show and Quit
ran once per toggle (Tauri keeps a tray's `on_menu_event` for the app's life). Both are fixed by
making the icon once and hiding it (`set_visible`) instead of removing it; the test
`toggling_many_times_leaves_one_menu_handler` toggles 50 times and finds one.
**Decided:** light and dark ship together through an asset catalog, as Braindump does, rather than
Tauri CLI's own `.icon` support (it needs Xcode 26 at every build) or dark only. With the
`tray-icon` feature, `core:default` would give the window tray rights, so the capability lists the
other eight sets; decision 0001 amended.
**Verified:** release build in a temporary HOME with a synthetic queue: the new Dock icon, the tray
idle at 0 and waiting at 1 (tooltip "Muster · 1 thing waits for you"), a click brings the window
forward, Off and On twice with the app alive (before R1; after it, hide and show weren't seen on screen:
accessibility listed no windows or status items for any Muster then, the owner's included); `assetutil` shows Aqua, DarkAqua and tinted
renditions. Size: app 9.6 → 10.3 MB, binary +178 KB. Screenshots in the task's attachments.
**Open:** the tray menu itself and the light Dock icon weren't seen on screen (the menu opens only
on a real right-click; the system's look wasn't changed). The light logo was checked as a render
and by test, since the webview's storage is shared with the owner's running Muster.

## 2026-10-14 — Sidebar headings, round 3: "N need you" always in words, the name gives way first (mu-82)
**Fixed:** review R2: at the default 210px "Dast Afzar", "persianUtils" and a Persian name showed
only the orange number; the words came at 240-280px and the rest only at 320. The name kept its
room and the counts got what was left. Now the name gives way first (ellipsis down to 4em, whole in
its tooltip), "N need you" is one piece that never shrinks, and "· N projects" / "· N working" each
show whole when there's room. Where the name's 4em and "N need you" can't share the line (150px)
the counts go under the name (`ProjectList.tsx`, `styles.css`; DESIGN.md, AS_BUILT, manual 04).
**Decided:** the second line comes from the flex wrap itself, not a width query, so it appears
exactly when one line can't hold the name's least and the needs, at any sidebar width (the lead's
call: two lines at the narrowest only; at 150px that's every heading with something that needs
you, at 210px none, 3-digit counts included).
**Fixed:** a flex line always takes its first item, so a piece too wide showed cut ("· 2" at
150px); an empty `::before` of the line's height goes first now, and the piece wraps out of sight.
**Verified:** gate (below); `ProjectList.test.ts` rewritten for the rule (each of 5 CSS reverts
fails a test); headless Chrome renders of the real stylesheet at 150, 210 and 320px, Violet night
and day, with "Apps", "Dast Afzar", "persianUtils" and "ابزارهای فارسی" (120 need you), measured:
"N need you" whole at every width, one line at 210 and 320, two at 150, nothing past the edge.
**Open:** release-app screenshots with real data and the owner's look (AC5) are the lead's.

## 2026-10-14 — Sidebar headings, round 2: a drawn chevron and a heading plainly bigger than its projects (mu-82)
**Fixed:** the owner sent mu-82 back ("icon to expand collapse is bad, the category and projects
seems the same font and font size"): the chevron was a 12px ▸/▾ text glyph in `--muted`, and the
heading 13px 650 against a project's 13px 550. Now a drawn 14px SVG chevron in `--text` in a 22px
Pill, turned down when open, and the name 15px 750 (`SidebarProjects.tsx`, `styles.css`).
**Fixed:** with the bigger name, renders showed the "17" chip spilling past a 150px sidebar (the
counts shrank to 7px) and "need …" cut at 210px. The counts now take what the name leaves, never
less than the chip (`data-needs-digits`), and the words and the rest are whole or hidden (they
wrap onto a hidden second line); the container query that dropped the words is gone
(`ProjectList.tsx`, `styles.css`; DESIGN.md, AS_BUILT, manual 04).
**Verified:** gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati)); new and
updated tests in `ProjectList.test.ts` (3 fail on a reverted size, wrap or chip floor); headless
Chrome renders of the real stylesheet at 210 and 150px, Violet night and day, with 1-, 2- and
3-digit counts and a Persian name, measured: nothing past the edge.
**Open:** release-app screenshots with real data are the coordinator's (a launched build doesn't
come to the front here, so a screen grab shows the owner's other windows). At default width a
long name leaves only the number; the words are in the tooltip and the aria-label.

## 2026-10-14 — Run from the app, review round 2: the login shell in its own session, read to its mark (mu-79)
**Fixed:** R3: under a terminal, `zsh -ilc` in a background group stopped on SIGTTOU until the
5 s timeout → `run::new_session` calls `setsid` before exec (it leads its group too, so the
timeout's group kill still works). R4: the PATH was read to the end of output, so a startup file
leaving something on stdout threw it away → read in chunks up to the end mark, which carries a
number of the call's own so nothing a startup file prints matches it.
**Verified:** `cargo test run::` 20 (+1 run only under `script`): a sleeper holding stdout gives
the PATH in under 3 s; the stand-in shell has no `/dev/tty`; a real zsh under `script` prints its
PATH in 0.06 s, and with `process_group(0)` instead it fails at the 4 s timeout (run outside the
sandbox, which allows no pty; inside it the test says so and skips). Gate below.

## 2026-10-14 — Run from the app, review round 1: the login shell's PATH, events by key (mu-79)
**Fixed:** R1: Muster opened from the Dock ran `bin/run` without node (the owner's is only under
`~/.nvm`), so pnpm exited 127 → `run::login_path` reads the login shell's PATH once (`$SHELL
-ilc`, argument array, 5 s, group killed on timeout), and it comes first in `search_path`; the
fixed folders stay as the fallback. R2: run events carried the name the run started with, so after
a rename they reached another project's Run block → `Snap` and `View` carry the key (queue folder)
and `useRun` matches on it (`runData.ts` `received`/`keyed`).
**Decided:** the PATH is read between marks, so startup files that print don't spoil it; on a
normal exit the shell's group isn't killed (its id is reaped, so it may be reused). Read before
the ops lock, so a Stop never waits on it.
**Verified:** `cargo test run::` 17 (stand-in shells: argv, noise around the marks, failing,
silent, missing, hung with its child killed; a run's PATH starts with the login shell's, read
once; every event keyed). `vitest` Run 17. Real zsh with the Dock's env prints the nvm folder
first (0.7 s). Gate below.

## 2026-10-14 — Run from the app: start a project's run command, watch it, stop it (mu-79)
**Did:** `run.rs` starts a project's run command in its main checkout (no shell, its own process
group, no stdin, without the queue variables), keeps its output in a 2,000-line / 256 KB ring and
sends it as `run` events at most every 200 ms; Stop, Restart and quitting end the group (SIGTERM,
SIGKILL after 5 s). `run_start`/`run_stop`/`run_state` take a project name only. Your check gets a
Try line (Run with the command) and a Run block (state, Stop/Restart, last 12 lines, All output);
Settings › Projects gets Run; the sidebar row says "running" (`Run.tsx`, `runData.ts`).
**Fixed:** `bin/run` started the second Muster with `open -n`, which hands it to launchd, out of
the run's group, so Stop could never end it → it now `exec`s the build's binary (the Info.plist's
CFBundleExecutable).
**Decided:** a run has ended when its leader exited and both streams closed (no polling; a
grandchild still writing keeps it running). Gate's launcher (`parallel::program_in`,
`gate_command`) shared, not copied. An exited run's group isn't killed at quit: its id may be
reused. Chip state rides on the 2 s poll (`QueueInfo.running`), not a fourth command.
**Verified:** `cargo test run::` 11 (grandchild gone after Stop, Restart and quit; SIGTERM ignored
→ SIGKILL; 100,000 lines → events ≥ 200 ms apart, ring bounded); mutants (no process group, no
batch pause, resent lines) caught. `vitest` Run 15; `bin/run` tests 6 (the binary runs as the
process Run started). Gate below.
**Fixed (own review, before Testing):** from the Dock the run had the system PATH only, so
`pnpm` wasn't found → `run::search_path` adds the owner's tool folders. Runs were kept by display
name, which a rename or another project shifts → kept by queue folder; remove_project stops its
run. The leader was reaped while a stream holder kept the run going, freeing the group id Stop
signals → reaped only once the streams close. A thread that failed to start left the run
"running" → the waiter starts first and owns the child. Window: a `run_state` answered after an
event dropped the earlier lines → put in front; "Run again" named after its visible words; the
first "Running" announced from a live region there before any run; a standing refusal no
longer an alert.
**Open:** AC4 (release app: Run on a Muster task builds and opens a second Muster, Stop ends it,
light and dark screenshots) needs the release app, which this sandbox can't launch.

## 2026-10-14 — Muster's handoff rule follows the automatic handoff (task 0075)
**Did:** decision 0011 amends 0004 point 3: a session hands off by itself at ~60% context or the
token cap (150k, 300k for the lead), at a gap, and a named background successor resumes it.
PROTOCOL.md's Sessions rule says what to do at the hook's prompt (check in, `/handoff`, the
successor runs `muster inbox --session <old id>` first) and that handoffs hold no secrets and stay
in the untracked `.claude/work/handoffs/`. AS_BUILT and DOGFOOD 35 say the old-mailbox gap is fixed
in part.
**Decided:** the brief said the same `--as` name carries the claim on; it doesn't (backlog
2026-10-14-0707, hit again on this task), so 0011 and PROTOCOL.md list it as open with the
coordinator's workaround. Options in [0011](decisions/0011-sessions-hand-off-by-themselves.md).
**Verified:** `bin/gate` green (pnpm install, pnpm check, test_task, test_commands, cargo test); the AC greps (0011 in 0004, no old 150k rule outside 0004, `handoffs/` in PROTOCOL.md).
**Fixed (review round 1):** R1, the 300k lead cap matched `Muster_Team_Lead*` but PROTOCOL.md
named the lead `muster-lead`: PROTOCOL.md and 0004 point 4 now name it `Muster_Team_Lead` (the
lead's call: the owner's name, not a wider hook pattern). R2, nothing the successor reads told it
to run `muster inbox --session <old>`: the handoff's writer now puts it first in Next steps, and
AS_BUILT and DOGFOOD say what that covers and what it doesn't. R3, the reminder fires at 5/6 of the
cap when that comes before 50%.
**Open:** routing answers by claim, a claim takeover for a successor, and the `/handoff` skill
itself adding the inbox step.

## 2026-10-14 — Gate green again on a checkout that ran it before; coordinator hand-over (Muster_Team_Lead_4)
**Fixed:** `bin/gate` went red on main: mu-80's test that AS_BUILT names every `bin/` entry also
counted the gitignored `bin/__pycache__`, which the gate's own Python step writes, so any checkout
failed from its second gate run on. The test skips `__pycache__`, and the gate runs Python with
`-B` so it no longer writes one (`app/src-tauri/src/index.rs`, `bin/gate`).
**Did:** mu-82 went back to Ready after the owner sent it back ("icon to expand collapse is bad,
the category and projects seems the same font and font size"); its brief is in its notes. mu-79's
review round 1 asked for changes: R1, Muster opened from the Dock has no node on the run's PATH;
R2, run events carry the start-time project name. Builders: Asghar_4 took over mu-79 then mu-82
from Asghar_3, and Gholi took mu-75.
**Verified:** the test fails with `bin/__pycache__` present before the fix and passes after;
`python3 -I -B bin/test_task.py` leaves no `__pycache__`; `bin/gate` green twice in a row in the
worktree.

## 2026-10-14 — Coordinator (Muster_Team_Lead_3): mu-82, mu-77, mu-76 and mu-69 merged; claims moved by the owner override
**Did:** merged mu-82 (2f46845, round 2), mu-77 (666a760, round 4 after the rebase), mu-76
(b9ce0ed, round 2) and mu-69 (a4ba06f, round 2). Set Muster's own run command
(`muster projects set muster run bin/run`). Rebuilt the release app and CLI from main and
restarted the running Muster. The builder handed off twice (Asghar → Asghar_2 → Asghar_3).
**Decided:** the owner said "do it yourself" and "this is a hands free session": the coordinator
runs `--as owner` queue steps itself. Used for a builder's successor, whose session can't pass or
answer the old session's claim (backlog 2026-10-14-0707-…): the successor fixes and commits, the
coordinator moves the task to Building, records the answers with `muster review <id> --fixed R<n>
--as owner`, moves it to Testing and has it re-reviewed. Lessons L7–L10 accepted (stub argument
quoting, tests needing `app/node_modules`, the never-shrinking part at the narrowest sidebar,
splitting markdown inside code fences). mu-76: Muster's config.toml skips no lens, and VISION.md
needs every lens but payments.
**Verified:** `bin/gate` green in each worktree and again by each `muster pass`. Release-build QA
of mu-82: Violet night and Violet day, a heading open and one folded, at the default width and
with the splitter at its minimum (the words drop, the number stays whole, nothing past the edge).
projects.json and settings.json byte-identical after QA; theme, mode and splitter back as before.
**Fixed:** the entry below says Your check held 11; the owner had already accepted most of them,
so it held 1 at the time.
**Open:** at the narrowest sidebar a folded heading shows only its orange number (the words are in
the tooltip and the aria-label): the owner may want to judge that against "unlabelled numbers".
mu-79 is with Asghar_3.

## 2026-10-14 — Notifications, review round 1: an account first wanted after launch posts its news (mu-69)
**Fixed:** R1. A builder claiming on a new account after launch had its waiting session taken as
baseline (its first read counted as "couldn't know yet") → held back every account not yet read →
`Known` now says which accounts a read wanted as well as read, and `Seen::unread` holds only those
wanted on the first read (or the first after Off) until a read knows them or a read doesn't want
them. That replaces the `every` latch: nothing wanted at start leaves `unread` empty.
**Verified:** `cargo test notify agents` 47 green; R1's scenario at `decide` and `after_read`;
mutants (old rule, and keeping unwanted accounts) fail 4 and 1 tests. Gate below.

## 2026-10-14 — Notifications latch "sessions known" per Claude account (mu-69)
**Fixed:** with two accounts, one whose first `claude agents` read failed, its sessions already
waiting at launch were posted as new once it read → the latch was one bool, set by any account's
good read (`agents::known`) → `known` now returns the accounts read (`Known`, every one when none
is wanted), `Seen` keeps their union, and an input `Item` carries its session's account from the
holder's stamp (`agents::account_of`): it's news only when its own account was known before.
AS_BUILT's notifications paragraph and the manual say so.
**Verified:** `cargo test notify` and `cargo test agents` green; the old any-account rule fails
both new tests (per-account `decide`, and `after_read` with cc-hg and cc-mg stamps). Gate below.

## 2026-10-14 — Shaping checks the cross-cutting lenses by default, unless a project opts out (mu-76)
**Did:** `muster shape`'s brief lists every lens (accessibility, i18n and RTL, themes, payments and
entitlements, architecture, security, privacy, performance, tests, docs) with what to write, and the
template gains a Lenses block (`lenses.rs`). A project skips a lens only explicitly, with a reason:
`.muster/config.toml` `[shaping] skip` plus `[shaping.reasons]`, or an old queue's README front
matter. `muster file` warns, naming the lenses the body has no line for, and files anyway; `muster
check` reports a bad skip. A VISION.md `**Lenses needed:**` line makes the vision check flag a task
that says only "n/a" for one of them. Help, manual, PROTOCOL and AS_BUILT updated.
**Decided:** a skip without a known lens or a reason doesn't count (the lens still applies) and is a
problem, not a warning; a line counts for a lens when its label is only lens words. The coordinator
decided Muster's own two calls: its `config.toml` skips no lens (payments stays one, answered "n/a"
and why, since Muster may be sold inside Drover), and its VISION.md needs accessibility, i18n/RTL,
themes, architecture, security, privacy and performance. A `**Lenses:**` example in fenced code
isn't the task's block (review R2; the fence rule is merge_log's, now shared as `Fences`).
**Verified:** `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`; lenses,
backlog, vision and CLI tests on synthetic queues in both layouts.
**Open:** the app's "File as task" doesn't show the missing-lens warning yet; `manifest.py --check`
doesn't know skips.

## 2026-10-14 — Each project has a run command beside its gate; Muster's is bin/run (mu-77)
**Did:** `muster projects set <name> run "<command>"` stores what the app's Run will start to try a
project, with the gate's rules (one line, ≤500 characters, quotes checked, empty clears); `muster
projects`, `--json`, migrate and Settings › Projects (a Run line under Gate, `dir="ltr"`) show and
carry it. Nothing runs it yet (mu-79). `bin/run` builds Muster's release app and opens that build
as a second instance (`open -n`), beside the installed one.
**Decided:** gate and run share one setter and a lenient reader with the same checks, so a bad
entry reads as none and the rest of projects.json still loads. Other projects' run commands are for
their owners: Tauri apps `pnpm tauri dev`, SwiftPM apps `swift run`, choopan `pnpm dev`.
**Verified:** `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`; Rust
tests for the rules, lenient reads and migrate; a render test for the Run line and hint; bin/run's
tests with stand-in pnpm and open (with `open` instead of `open -n` the test fails).
**Fixed (review round 1):** bin/run's test couldn't tell a quoted app path from a split one → the
stubs log each argument bracketed and a path with a space is asserted; run and gate commands refuse
direction marks, overrides and line separators; the failed-build test no longer needs node_modules;
the table's run wiring is tested through `ProjectGateRow`.
**Open:** `muster projects set muster run bin/run` and AC4 (bin/run starting a second Muster, both
in `ps`) are for QA or the owner: they write the real projects.json and launch the app.

## 2026-10-14 — Sidebar categories read right: a strong heading, labelled counts, a fold that folds to one line (mu-82)
**Fixed:** "category UI is not good" (owner) → headings were smaller and greyer than their projects,
folding still showed every project that needed you as a chip, the counts had no words and the
indent didn't tie projects to their heading. Headings now start with a divider, set the name at
least as strong as a project's, with a chevron Pill (aria-expanded); projects indent with logical
margins (RTL mirrors); one labelled count line, the same open and folded ("77 need you · 17
projects · 18 working", needs first, so it's never the part cut at 150px); a folded heading is one
line (owner's choice A), mu-70's peek chips removed.
**Decided:** a folded heading's needs-you count is what keeps it in sight (DESIGN.md, "Sidebar
categories"); the stale "persianUtils" category on three projects is the owner's data, left alone.
**Verified:** `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`; render
tests (17 projects that all need you fold to one line; counts identical open, folded and in every
mode); heading and count contrast ≥ 4.5:1 in every theme; break-checks on each rule.
**Fixed (review round 1):** at 150px a folded heading's line spilled past the sidebar → the needs
number never shrinks, the words "need you" sit in their own ellipsis span and drop below a 164px
nav (a container query, so Focus and the splitter both count), with the aria-label and tooltip still
saying them.
**Open:** release-app screenshots (AC4) not taken: the sandbox blocks headless Chrome; HTML renders
are in the task's notes. The row's needs-you badge reads ~2.2:1 in light themes (pre-existing; filed).

## 2026-10-14 — Coordinator (Muster_Team_Lead_2 and _3): mu-81, mu-78, mu-80 and mu-83 merged, mu-82 sent back, mu-77 rebased
**Did:** after release-app QA, merged mu-81 (721a34c) and mu-78 (43d7d8b, 72f34df). mu-74 passed to
Your check after its live AC4 run. The layer's main fast-forwarded to 713be8f (mu-74's round-1
fixes). Reviewed Testing with one agent per task, no lens subagents: mu-80 approve with a nit
(merged c755938), mu-83 approve in round 2 (merged c7f9e1b), mu-77 approve in round 3, mu-82 sent
back (R1: at the 150px sidebar a folded heading's "need you" can't shrink and runs past the edge).
mu-77's merge hit a rebase conflict (vite.config.ts: mu-78 already had `*.test.{ts,tsx}`) and
mu-80's test that AS_BUILT names every `bin/` script: rebased keeping main's line, added `bin/run`
to AS_BUILT §2 (df9a280), back in Building for Asghar to pass again. The release app was rebuilt
from 72f34df.
**Decided:** L6 accepted. mu-80's nit (`$HOME` swapped as a plain substring; a wrong commit cited
for mu-52) and the earlier nits go to backlog notes, not extra rounds. L7 and L8 (unquoted
arguments in stubs; tests that need `app/node_modules`) are proposed; accepting them needs
`--as owner`, which the permission check refused, so they wait for the owner.
**Verified:** `bin/gate` green in each worktree, run outside the sandbox (inside it the gate goes
red on EPERM only), and again by each `muster pass`. QA in a release build beside the running
Muster: mu-83 on braindump in Focus and Standard (strips line up, open columns at card width);
mu-77's `bin/run` built and opened a second Muster (pids 27945 and 41510, different paths).
projects.json and settings.json byte-identical to the backup after QA; mode back to Focus.
**Open:** mu-82 R1 with Asghar; mu-77 waits for Asghar's pass. Your check holds 11, over the
limit of 5. A branch cut before mu-81 still shows the crushed theme picker in its QA build; the
merge rebases it away.

## 2026-10-14 — The board's columns: empty strips line up, open columns stop at 400px (mu-83)
**Fixed:** on a mostly empty board the strips sat at different heights, their top borders merged and
the one open column stretched to ~1400px (owner: "mess in board UI") → an empty column's state class
was `empty`, the same as the "No tasks match" message, so it got that message's 32px padding: it
overflowed its 44px track by 12px each side (merged borders) and its label sat ~60px down. Strips now
use `col-strip` with `col-empty` or `folded` (modes.ts `columnState`, `columnTrack`, `columnClass`),
one head rule for every strip, the Paused lane and Group by bands included; open columns are
`minmax(118px, 400px)` in every mode, spare room to the right.
**Decided:** ADHD and Standard keep an empty column as a narrow strip saying "empty", so the board
doesn't jump; Focus folds as before, with the same strip anatomy (docs/DESIGN.md, "The board's
columns").
**Verified:** measured in a headless Chromium render of the real styles.css before and after (strips
44px, heads at 3px, separate borders, Ready 400px); `columnStrips.test.ts` (11 tests) fails 3 with the
old class back; `bin/gate` green. Renders (not the release app) attached to the task.
**Fixed (review round 1):** the plain board's widths and strip classes had no test → `plainBoard`
(modes.ts) builds them for App and is tested on braindump's shape in every mode; the strips'
`overflow: hidden` clipped the fold buttons' focus ring → the ring is drawn inside the button.
**Open:** the owner's look at braindump's board and All (AC4). `.folded` is still shared with the
sidebar heading and Recently finished (different properties; mu-81's guard isn't on this base yet).

## 2026-10-14 — One as-built document for Muster: architecture, each part's state, decisions, the queue at a glance (mu-80)
**Did:** AS_BUILT.md restructured (owner: "as built document for the Muster including tasks in
queue and architecture"): Muster in one page, the architecture (a diagram and a table of every
core module, the app and its IPC, the CLI, bin/ scripts, hooks, the personal layer's part and the
files on disk with who reads and writes each), how a task flows, each part's state checked against
the code, decisions 0001–0010, the queue at a glance and what's left; still-true detail kept under
"Parts in detail", stale lines fixed. `muster index --as-built <file>` writes the queue section
between two markers in the same run as INDEX.md, leaving everything outside them byte-identical.
**Decided:** missing or doubled markers make the command refuse and write nothing; the section holds
only keys, titles, stages and holders (no write-ups, answers or home paths).
**Verified:** `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`; index
tests (matches INDEX.md, outside text byte-identical incl. CRLF and Persian, refusals, no body or
$HOME path, a title can't forge a marker); a test fails when a module or bin/ script is missing from
the architecture section (checked by removing a row); the counts of 66 Tauri and 46 CLI commands
checked by script. Built partly while the task sat in Ready (the pause reached the builder late).
**Open:** the owner's read (AC4). No decision 0011 exists yet. README.md and projects.rs's module
comment are stale. The queue section is a snapshot: rerun `muster index --as-built docs/AS_BUILT.md`.

## 2026-10-14 — Your check says what you're accepting: a summary, what Accept does, each criterion's proof (mu-78)
**Did:** a task carries `summary:`, one plain line of what was built: `muster pass --from building`
needs it (`--summary`, or set before), `muster set <id> summary` writes or fixes it, and the app's
Pass form has the field. Each criterion's `— proof:` tail is split into its own field, with the
screenshots it names from the task's own attachments. Your check's pane opens with "What you're
accepting": what was built, what Accept does (all proven / yours open / not proven / blocked) and
each criterion's proof with thumbnails that grow in place; cards and Waiting for you say "All N
proven: Accept" with the summary under it. Every queue, old layout too. Help, manual, PROTOCOL,
builder briefs and AS_BUILT updated.
**Decided:** a summary with a control character is refused, not cleaned (Persian ZWNJ kept); the
proof splits at the first dashed `proof:` outside a code span; the app's pass takes one struct.
**Verified:** `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`; Rust
tests for the summary rules (161 characters, newlines, control characters, goals and human tasks
exempt), the proof split and file matching (`../x.png` matches nothing); a render test in ai-10's
shape with a Persian proof; `<b>` in a summary shows as text.
**Fixed (review round 1):** Accept all took a task the card called unproven → an unticked agent
criterion makes it One look, and the core's Accept all refuses it naming the labels; Drover's
". Proof:" criteria showed no proof → the split also takes a full stop; criteria are sent only for
Your check tasks; thumbnails' expand has a test that fires the click; screenshots load once per
pane for Files and the block; a ticked criterion without a proof reads "ticked", not "proven".
**Open:** AC4's and AC8's screenshots of ai-10 in the release app (light and dark) need the real ai
queue and the running app. Enter/Space on thumbnails is native button behaviour, checked from the
markup only. "Not proven" tasks still group under Just accept in Accept all.

## 2026-10-14 — Settings › Appearance's theme picker is no longer crushed by the project colour picker (mu-81)
**Fixed:** the seven theme cards were squeezed into 16px dots (owner: "UI bug") → mu-57's project
colour picker reused the theme picker's `.swatches`/`.swatch` classes, and its later rules won; it
now has its own `.proj-swatches`/`.proj-swatch` (ProjectSwatches.tsx, styles.css, tests).
**Verified:** `cssClasses.test.ts` fails when a class two components use gets rules that set a
property two ways; on the old code it names `swatches` and `swatch`, on the fix it passes; `.top`
(one component overriding itself) and `card-top` (not `top`) don't trip it; `bin/gate` green.
**Open:** AC1's release-app screenshots of both pickers, light and dark, are for QA or the owner.

## 2026-10-14 — Coordinator (Muster_Team_Lead_2): mu-73 and mu-71 merged, four sent back, the owner's open calls made
**Did:** reviewed and merged mu-73 (1e819a1) and mu-71 (4d7dbc4, round 2). Sent back with their
review findings: mu-78 (R1–R7: Accept all takes unproven tasks, Drover's ". Proof:" unread,
criteria on every poll, an untested thumbnail), mu-77 (R1–R4: the bin/run stub can't see a
missing quote, bidi characters in a run command), mu-74 (R1 blocking: AC6's desktop fallback
missing; R2–R11: claude-continue interpolates its wrapper, rewrites any file, mode fails open;
spaced paths and owner resumes break the rotation cap; a worktree handoff isn't found).
mu-4, mu-52 and mu-53 moved to Your check (branches long in main; their holder was stuck on a
prompt); their stale worktrees removed.
**Decided** (the owner said "make the decisions"): mu-79 Q1 and Q2 both (a), the main checkout
and real data. Lessons L1 and L5 accepted; L2–L4 rejected as single-task bugs and filed as one
backlog note. mu-74's fix round goes to a new background session (mu74-fix) in the layer, since
its builder was unreachable. A ticked criterion without a proof reads "ticked", not "proven"
(note on mu-78). The held big tasks (mu-9, 41, 54, 55, 56) stay held until the weekly usage
resets. The persianUtils category for braindump, muster and drover stays.
**Open:** the work-log merge driver still isn't in `.git/config` (the permission check refused
the write; the owner runs the two `git config` lines). mu-77's AC4 (bin/run opens a second
Muster) needs `open` outside the sandbox, which was refused; the running Muster is main's own
bundle, so run it from the worktree. Rejecting L2–L4 also stops those kinds being proposed again.

## 2026-10-14 — Work log merge driver: ours is replaced whole or not at all, and its tests can fail (mu-71)
**Fixed:** a failed write could truncate the work log before the fallback merged it → the merged
text goes to a temp file beside it and is renamed over it, so ours stays as it was on failure
(`merge_log::replace`). Three tests couldn't fail: the pass-from-testing test now runs muster by
its absolute path with no muster on git's PATH; two code-fence cases test the closing rules; the
no-muster test puts the found git first on PATH.
**Fixed (review round 1):** the temp file's cleanup ran in no test → a test fails replace() after
the temp file exists (rename onto a folder, a missing ours) and checks nothing is left; `sync_all`
dropped (git reads the file straight back, and full fsync fails on some volumes); a muster killed
mid-write leaves its temp file, now documented; the read-only test skips where modes are ignored.
**Verified:** each guard removed by hand fails its test (fs::write instead of replace; muster
looked up on PATH; `c2 == c`; the trailing-whitespace check), all restored; `bin/gate` green.

## 2026-10-14 — The poll and the DashAfzar fixture share one state builder; folds follow only drawn projects (mu-73)
**Fixed:** the fixture the sidebar's render test draws was built by a different pipeline from the
app's poll (no prefixes, no Your check limit) → `projects::read_state` builds the state for both,
and the fixture is regenerated from it. A heading whose only projects are hidden kept its fold and
came back folded on a rename → live headings count only the projects the sidebar draws.
**Verified:** removing `check_limit::apply`, `labels::apply` or `arrange::apply` from
`read_state` fails the fixture test; the Tools/alpha/beta rename test fails against the old
`headings_now`; the fixture has no empty prefix; `bin/gate` green.

## 2026-10-14 — Two tests that couldn't be trusted: the lessons test and init's prefix check (mu-72)
**Fixed:** the lessons test failed now and then → it checked the brief for a bare "L1", which a
random temp folder name can contain; it now checks the lesson block is absent (cli.rs). Init's
prefix check in test_task.py could never fail → setUp now starts from an outer HOME and
MUSTER_BASE whose playground holds a `proto` queue that takes "pr", so a leak picks "pro".
**Verified:** with `TMPDIR` in a folder named `xL1y` the old assertion fails and the new one
passes; removing test_task.py's HOME override, or its MUSTER_BASE pop, fails setUp with "prefix
pro" (4 failures each), both restored; `bin/gate` green.

## 2026-10-14 — Decision 0008 accepted (agent teams)
**Decided:** the owner accepted decision 0008: a teammate is session + agent id; teams only for the
planning room and review lenses, not parallel building; smaller models for workers and reviewers,
one combined review+QA agent per task; a TeammateIdle hook later. mu-40's AC3 ticked.

## 2026-10-14 — Coordinator (Muster_Team_Lead): Asghar's six merged, Gholi's eight tested
**Did:** reviews of record (one agent per task, never the builder) and merges by `muster pass`:
mu-57 811d06b, mu-62 5c1058d, mu-68 a6fe0bb, mu-70 3bf2af4 (owner ask: DashAfzar folding), mu-67
209ee76 (work-log merge driver; CLI rebuilt so `muster merge-log` is on PATH). Tested Gholi's mu-40,
58, 60, 61, 63, 64, 65, 66 (on main, gate green at c79bbfe, criteria proven) and passed them to Your
check. Shaped three backlog notes into mu-67, 68, 69. Looked at drag (mu-57) and folding (mu-70) in
the release app with real mouse events; projects.json put back after each test.
**Open:** mu-4, mu-52 and mu-53 stay in Building until muster-lead (stuck on a 16:43 question) or
the owner's override passes them; mu-69 waits on mu-52. Lessons L1–L4 wait for the owner. Whether
braindump, muster and drover leave the persianUtils heading is the owner's call.

## 2026-10-14 — Work log entries merge by themselves: a muster merge driver for docs/WORKLOG.md (mu-67)
**Did:** `muster merge-log %O %A %B` (`merge_log.rs`): a plain three-way merge when it's clean;
when both sides only added whole dated entries on top, both sides' entries go above the base,
newest date first, the incoming side first on the same date; anything else is a normal conflict.
`.gitattributes` names it for `docs/WORKLOG.md`; `muster start` and `init` set
`merge.muster-log` in the repo's config when it's missing; Muster's merge rebase passes it with
`-c`, so a log-only conflict rebases clean and the review is carried. Help, manual, PROTOCOL and
AS_BUILT updated.
**Decided:** a driver, not `merge=union` (it interleaves entries that share a line and drops one)
and not one file per entry (the index conflicts again, and it breaks the one-page convention):
`docs/decisions/0009-work-log-merges-through-a-muster-merge-driver.md`. An undated new entry, or
the same preamble added on both sides, is a conflict; a driver key set in global git config counts
as present.
**Verified:** `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`; 12
merge_log tests (incl. the union-breaking case, CRLF, a real rebase from a path with a space and a
quote), a parallel.rs test with no driver config landing both entries with the review carried, and
a Python test where a hand `git rebase main` after `muster start` merges cleanly. Temp repos only.
**Fixed (review round 1):** a driver that couldn't run left only ours with no markers, so a
`git add` lost the task's entry → on any error the driver writes git's own markers, and the
configured command falls back to `git merge-file` when muster can't start; a dated heading inside
a code fence split an entry → fences are skipped per CommonMark; `%` in the binary's path is
doubled; the merge tests build muster themselves; the real `muster pass --from testing` path and
the CRLF separator are tested.
**Open:** only checked with git 2.54. Other repos with a running log can opt in with one
`.gitattributes` line later.

## 2026-10-14 — Category headings fold; a group folder can carry a name, e.g. persianUtils → DashAfzar (mu-70)
**Did:** a sidebar heading folds by its chevron or ←/→ (`fold_category`, projects.json `folded`);
folded, it shows its name, project count, needs-you and working counts, and the selected project
and any that need you stay as chips under it. Renaming a group's heading stores a display name
for the group (projects.json `groupNames`), so all its projects, and later ones, show under it
with no per-project category. Folds follow renames and merges. Manual 04 and AS_BUILT updated.
**Decided:** projects hand-set to the old name follow a rename (so braindump, muster and drover
stay under DashAfzar until the owner moves them); renaming back to the folder's name clears the
display name; a merge is folded if either heading was; "needs you for something new" shows every
project with a needs-you count as a chip. Headings are `role="heading"` now, not buttons.
**Verified:** `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))` with
HOME in a temp dir; 7 new Rust tests, render tests across 3 modes × 4 order chips; break-checks
(each key line removed fails tests). The owner's projects.json not touched.
**Fixed (review round 1):** moves from Settings didn't unfold and stale folds came back → the core
unfolds on any move and drops folds of gone headings; lessons-only projects got no chip → peeks
count tasks and lessons as the badge does; long chip names clipped the count → the name truncates;
unfold-on-move and the fold rollback are pure functions (`unfoldFor`, `trackFold`) with tests that
fail without them; the announcement names both the move and the unfold.
**Fixed (review round 2):** typing a folded heading's name in Settings, or renaming onto a name,
left it folded → one `put_under` helper unfolds the heading a project goes under and drops folds of
emptied headings, called from move, Settings category and rename; a refused move puts the
heading's fold back (`trackUnfold`); a DashAfzar render test on a fixture the core is held to.
**Open:** the owner renames persianUtils to DashAfzar and folds it (AC3); chevron and chip look and
the 150px wrapping not seen in the running app.

## 2026-10-14 — Pause's help and the Pause button say it goes to the Paused lane (mu-68)
**Fixed:** `muster help pause`, its manual page, the Pause and Resume tooltips, Pause.tsx's comment
and the paused/resumed message still said a paused task stays where it is (mu-45's behaviour) →
they say it goes to the Paused lane and Resume puts it back in its column (help.rs, Pause.tsx,
useTaskActions.ts, manual 05 regenerated). The badge's glossary line no longer says "where it is".
**Fixed (review round 1):** the app glossary's Pause · Resume (also in `muster help --json`) still
said a paused task keeps its column → it names the Paused lane; a help test checks the summary,
`help pause` and the glossary all say "Paused lane" and none says "where it is" or "keeps its
column"; PROTOCOL's 0045 pause block gets a line saying the task now goes to the lane (0059).
**Verified:** `muster help | grep ' pause '` names the lane; `muster help pause | grep -c 'where it
is'` prints 0; AC3's grep finds nothing; `commands_page` passes; `bin/gate` green.

## 2026-10-14 — Decision 0010: the queue lives in local files, not GitHub Issues
**Decided:** tasks stay markdown files read through `muster`; GitHub gets a mirror later if
wanted, never the store. Why, the options and when to revisit:
[0010](decisions/0010-the-queue-lives-in-local-files-not-github-issues.md). Owner asked, and
wanted it on record.

## 2026-10-14 — Hermetic tests: one injection point for the projects Muster knows; the gate runs in its own home (mu-62)
**Did:** `projects::places()` is the one place the core learns the playground and projects.json;
`use_places` gives a test thread its own, and a test that gives none gets empty temp places.
`key::known`, the CLI's base and config file and the app's base all go through it. In test builds,
loading, saving or editing this Mac's projects.json or discovering under its ~/playground panics
with how to inject. `bin/gate` runs every step with `HOME`, `XDG_CONFIG_HOME` and `MUSTER_BASE` in
a temp home (real cargo, rustup and pnpm store kept), removed on exit. Integration helpers give
each spawned muster its own HOME.
**Fixed:** the dr-22 incident (a review mutation in mu-59 made `check --fix-pauses` fall back to
`known()` and convert Drover's real dr-22) → that fallback now sees only a test's own queues;
tests for `--fix-pauses` with no `--queue` and the app's offer prove it.
**Fixed (review round 1):** two Python test classes spawned muster with the real environment, so
outside the gate `muster init` picked a prefix from this Mac's projects → their own HOME; the
write-guard test's closure can't save; the answer-hook test runs in its own root.
**Decided:** an empty fallback rather than a panic for tests that give no places (safe, and no
test needed changing); no real-HOME check inside integration tests (a production-built core can't
tell it's under a test; the hermetic gate covers them).
**Verified:** `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`, plain
and with HOME in a temp dir; the whole suite with HOME an empty dir whose config and playground
are unreadable: lib 614, integration 4 + 15 + 9 + 3, all green; real queues unchanged.
**Open:** `cargo test` run by hand with the real HOME still reaches the real files in integration
tests that forget `no_projects`; branches without this change aren't protected until rebased.

## 2026-10-14 — Projects arranged in the sidebar: drag, categories in place, a colour each (mu-57)
**Did:** drag (and ⌥↑/⌥↓) reorders projects and headings, with a Mine order chip picked on the
first move; drop on a heading moves a project, "+ Category" and an in-place rename edit headings; a
colour dot per project from an inline five-swatch row, shown wherever the project is named. Saved
in projects.json (`arrange.rs`); Settings › Projects edits the same. Manual (In the app,
shortcuts), DESIGN.md colour code, AS_BUILT updated.
**Decided:** heading order holds under every chip; the first move starts from the order on screen
(a drag in A–Z replaces an earlier Mine order); a new empty heading isn't saved until it gets a
project ("empty categories disappear"); renaming onto an existing heading merges; only five
colours, because no blue, cyan, violet, amber, orange, green or red keeps the distance from every
theme's stage, priority and accent colours.
**Fixed (review round 1):** a heading dropped on another group's rows did nothing (the row swallowed
the drop) → one pure drop routing (`dropMove`); a fresh heading's name squeezed to nothing → it
wraps; Settings ↑/↓ twice before a refresh undid the first → plans from a pending layout
(`settle`); a group folder over 24 characters couldn't be renamed or ranked → one cut form on both
sides; the drop line stayed after leaving → cleared on the nav.
**Fixed (review round 2):** the pending-move bookkeeping had no test that could fail → one pure
`track()` shared by the sidebar and Settings, tested so removing any arm fails; About names colours
and the sidebar order; an unused `dropTarget` removed.
**Verified:** `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`; vitest
1405, cargo 591 lib + integration; the palette distance test across every theme and accent.
**Open:** native drag in the running WKWebView, the drop lines and the look of the dots aren't
checked by hand; the drag and key handlers have no DOM tests (the move logic and markup do).
`muster projects` doesn't print the colour.

## 2026-10-14 — A review can always send a task back; the sent-back one comes first (mu-66)
**Fixed:** a review that found something couldn't send the task back to Building while its builder
held the next task (one claim per holder; seen on mu-59) → a Testing → Building change skips that
check; the sent-back task comes first (`muster next`: "fix mu-59 first (sent back by the review)",
cards say which comes first and which waits behind). A new claim is still refused.
**Decided:** send-back always allowed (the rule stops starting two things; a send-back starts
nothing); overrule in the task.
**Verified:** `bin/gate` → `gate: green (pnpm check, test_task, test_commands, cargo test
(Kiritimati))` on 3b2386f; core, review, CLI and render tests.

## 2026-10-14 — Hatched own cards readable in every look; Quick add's help (mu-65)
**Fixed:** on the owner's own cards (hatched, YOU) the project name and the red flags fell to
3.8–4.5:1 over the Tint washes (worst: Graphite `--bad` 3.84 on Testing) → the card uses its own
`--human-accent-text` and `--human-bad` (`themes.ts` `wordOnStripes`), ≥4.5:1 on every stripe for
every theme × accent × wash; stripes and the blocked wash kept. Quick add's help names the Backlog
band under Group by.
**Verified:** `bin/gate` → `gate: green (pnpm check, test_task, test_commands, cargo test
(Kiritimati))` on bae4824; the extended contrast matrix failed first (413 combinations); shots of
hatched blocked cards in Tint, light and dark (`.claude/work/qa-0033/shots/a65-*`).

## 2026-10-14 — Coordinator (Muster_Team_Lead): mu-53, mu-4, mu-52 and mu-59 merged; the dr-22 incident
**Did:** took over muster-lead's three finished tasks (uncommitted at 95%): committed them on their
branches, one review+QA agent per task (never the builder), fixes by implementer agents, a second
review of each fix, then rebase, gate and fast-forward: mu-53 98667fb, mu-4 589b642, mu-52 5e1baa2
(its Notify me choice moved to mu-53's Pill). mu-59 (Asghar) merged by `muster pass` as 8c388bb after
three review rounds. Release app rebuilt and relaunched; looked at the board, Settings ›
Notifications and the old-pauses offer (lists dr-22) in the Muster window only.
**Fixed:** a mu-59 review mutant ran the non-hermetic cli tests against the real projects config and
converted Drover's dr-22 (committed there as 7597780, credited to the owner); reverted in
drover/tasks (73d991d) so the owner sees it in the offer first. Reviewers now run nothing that isn't
hermetic; mu-62 fixes the cause.
**Open:** mu-4, mu-52 and mu-53 stay in Building in the queue: only their claimant (session
531266d4) can pass them. mu-4 AC2 waits on the personal-layer change and the owner's pilot pick;
mu-52's real banner is unchecked. A review that finds something can't send a task back while its
builder holds another (backlog note).

## 2026-10-14 — A started task can't skip its merge (mu-64)
**Fixed:** a task made with `muster start`, moved to Your check or Done with `muster stage`/`move`
(or the app's stage track), skipped the merge, so its branch never landed on main → the move is
refused until the branch is in main (`git merge-base --is-ancestor`), naming `muster pass <key>
--from testing` or `muster release <key> --clean`. The repo is the `.muster` queue's own or the
task's worktree; with neither, a missing branch isn't taken as merged.
**Verified:** `bin/gate` → `gate: green (pnpm check, test_task, test_commands, cargo test
(Kiritimati))` on 113f25c (re-gated after main moved); core and CLI tests on real temp repos.
**Open:** `pass --from your-check` (accept) isn't guarded; a task can only reach Your check through
the merge or the guarded moves now. My merge script passed mu-64 to Testing before the
fast-forward when main moved mid-gate; the next steps now only run after the merge lands.

## 2026-10-13 — Paused tasks go to their own lane; old "paused" blocks become pauses (task 0059)
**Did:** a Paused lane before Backlog (`PausedLane.tsx`, `lib.ts` `columns()`): paused tasks show
there with "was Building · paused 2 h ago · why", not in their column; the stage and claim are
kept, so Resume puts them back. Column and in-progress counts leave them out; table, timeline and
Group by follow. Old pauses (blocks whose reason says paused) are found and converted in every
queue: `muster check --fix-pauses [--dry-run]` (`old_pause.rs`, `cli.rs`) and a one-time inline
notice on load (`OldPauses.tsx`, setting `old-pauses-offered`). Manual (Concepts › Paused, In the
app, Commands), COMMANDS.md and AS_BUILT updated.
**Decided:** an old `status: blocked` file with no stage reads as Ready (`legacy_stage`), as
unblock already does. The converted pause starts at the block's time, so the log gets a pause and
unblock line with that stamp plus a line stamped now. Paused stays a Flags chip, not a Stage, so
Stage › Building still finds what was paused there.
**Fixed (review round 1):** Make them pauses converted whatever it found at click time, not what
it listed → `fix_old_pauses` takes the listed (queue, id) pairs (`convert_listed`); a CLI test runs
`check --fix-pauses` with and without `--dry-run`; the offer's buttons get their container rule;
singular label; the closed-span case of the pause time is tested; docs say lane cards are grey.
**Verified:** `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`;
vitest 1249 passed; `muster check --fix-pauses --dry-run` on the real queues lists only dr-22
(Drover, paused since 2026-10-13 02:51) and changes nothing.
**Open:** not looked at in the running app (AC3 is the owner's). The real conversion of dr-22
waits for the merge and the owner's click or `muster check --fix-pauses`. An old pause in Building
whose holder holds another task can't be unblocked (`ops::holding`): it's named, left as it was,
and the command exits 1. None of the real queues has one.

## 2026-10-14 — Runner and monitor tests don't depend on machine speed (mu-63)
**Fixed:** `agents::tests::the_runner_*` and the monitor test went red under parallel builds (5 s
waits, a `< 2 s` assertion) → the wait loop takes a clock (`run_with`, `wait`), fake clocks test the
timeout, the real-process timeout test fires when the fake `claude` is ready, a held fetch proves
the monitor never blocks; bounds that are never waited out are 60 s.
**Verified:** all 20 `agents::tests` 5 rounds in a row with two release builds compiling (load 23–33):
5/5 green; `bin/gate` → `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`.

## 2026-10-14 — Layout fixes from the visual QA (mu-61)
**Fixed:** Settings › Projects crushed Folder/Gate to a word per line → the gate sits on a line under
each row, the prefix field is 4ch, Set/Save/Cancel/limit Set are bordered ui Buttons; the pane's
"quiet 21 min" wrapped by syllable → nowrap; graph chips ran words together and clipped "needs you"
→ spaced, wrapping, bigger nodes, the empty hint shows a key; rows showed above the table's sticky
header → no gap above it; the Focus strip's count sat on the BLOCKED pill → spacing along the
vertical strip; Waiting for you buttons touched → gap.
**Verified:** `bin/gate` → `gate: green (pnpm check, test_task, test_commands, cargo test
(Kiritimati))`; before/after headless shots, light and dark (`.claude/work/qa-0033/shots/a61*`).

## 2026-10-14 — Both side panes resizable (mu-58)
**Did:** one splitter (`ui/Splitter.tsx`, `splitter.ts`) for the projects sidebar and the task pane:
drag, double-click reset, ←/→ (Shift for 120 px), widths remembered (`muster.sidebar.v1`,
`muster.pane.v1`; the pane's old `muster.paneWidth` is carried over once), clamped so both panes
and the board fit at any window and Text size; a drag stores once, at the end.
**Verified:** `bin/gate` → `gate: green (pnpm check, test_task, test_commands, cargo test
(Kiritimati))` on a9dfa01; real-DOM check in headless Chromium (drag, keys, clamp 150–320, reset,
storage, one write per drag, old width kept, 820 px window with the pane open).

## 2026-10-14 — Blocked says plainly what it waits for (mu-60); agent teams spike (mu-40)
**Did:** each block kind has its own words and look from one table (`blockWords.ts` BLOCK_LOOKS,
Rust `block::look`): "Your answer: Q2 …" with Answer in place, "Your action: …", "Permission: …"
in the needs-you orange; "After mu-14, mu-16" and "Outside: …" neutral. A wait on other tasks alone
is never red (card, table, timeline, graph, column count). Waiting for you puts questions, blocks
and your own tasks first. The answer box has its own file, so the pane stays lazy. mu-40: decision
0008 (agent teams; a teammate is session + agent).
**Decided:** blocks band above Your check in Waiting for you (they stop agents); a non-escalated
question stays neutral; the "blocked" filter still matches waits on tasks.
**Verified:** `bin/gate` → `gate: green (pnpm check, test_task, test_commands, cargo test
(Kiritimati))` on 8a98616; one reviewer (3 findings, fixed); headless shots of the table and Waiting
for you, light and dark.
**Open:** "Done, unblock" and "Open to answer" buttons touch (no gap) in Waiting for you.

## 2026-10-13 — Mac notifications: one notification when something new waits for you (task 0052)
**Did:** `app/src-tauri/src/notify.rs` splits what the Dock badge counts into items (Your check by
when it came in, each escalated question, blocked on you or by a permission, a session waiting for
input, the owner's own task, each lesson to approve) and posts what's new once, several in one
notification. Seen without a notification: everything at start and in a project that just showed,
what the focused window shows (`show_project_title` tells the core), everything while Off. Keys
kept in `notified.json` (last 500 plus what still waits). The poll and the hidden badge read
(`waiting_in`, now keyed and labelled) feed it; no extra reads. Settings › Notifications
(`Notifications.tsx`): On/Off, saved as `settings.json` `notify` (default on). Manual, AS_BUILT,
DESIGN, VISION and decision 0001 (dependency list, Live) updated.
**Decided:** an item is a task's reason, not the task, so a task reaching Your check after an
escalation still notifies, while a session flapping in and out of waiting for input doesn't. The
plugin is called from Rust only: no capability added (the narrowest: none).
**Vetted** `tauri-plugin-notification` 2.6.0 (Drover's version, for tauri 2.12; owner said yes,
WAITING #3): Apache-2.0 OR MIT, Tauri Programme (tauri-apps/plugins-workspace). Build script only
generates its permission files. On macOS it goes through `notify-rust` 4.18.2 (MIT OR Apache-2.0)
and `mac-notification-sys` 0.6.15 (MIT/Apache-2.0, build script compiles its one Objective-C file
with `cc`). No network code and no telemetry in any of the three (only doc links); local
`NSUserNotificationCenter`. Desktop `request_permission` grants without a prompt; macOS asks on the
first post, which comes only when something new waits. Added with `cargo add`; no npm package
(nothing in the window calls it). Binary +325 KiB (release, `cargo build --release`: 9,070,672 →
9,403,888 bytes).
**Verified:** `bin/gate`: gate: green (pnpm check, test_task, test_commands, cargo test
(Kiritimati)); `cargo test` notify:: 13 tests, settings 1 new; vitest `Notifications.test.ts` 5
tests. Not checked by hand: a real notification on screen (needs the built app launched).
**Fixed (review):** focused on project B, news in A was posted and marked seen, but macOS shows no
banner while Muster is in front, so it was lost; now it's neither posted nor seen until the window
loses the focus. A session already waiting for input at launch was posted when the first
`claude agents` read landed; now `input` items are seen, not news, until a read knew the sessions
(`agents::known`). Off returns before reading anything, and the lessons' rules come from the
count's `(len, mtime)` cache (`lessons::proposed`), so a poll doesn't re-parse `lessons.json`. The
glue moved into `notify::after_read`, tested. Each notification line and the title are cut at 200
characters (a task id had no bound). Wording: `notified.json` is what Muster won't notify again
(not only what it notified), and `Mac::permit` grants rather than asks. Tests: 5 new in notify,
1 in agents, the lessons cache test extended.
**Open:** under `tauri dev` notifications show as Terminal's (the plugin's rule for unbundled
builds). Two tests this change doesn't touch failed once each and passed on rerun (cli: lessons in
the start brief; migrate: v2 copy, "would both be WAITING-FOR-OWNER.md"): flaky.

## 2026-10-13 — Muster in other projects: `muster init` in any repo, New queue makes `.muster/`, the layer's rule (task 0004)
**Did:** `muster init [--local|--committed]` with no folder starts the queue of the repo it runs
in (from any worktree, in the main checkout). The app's New queue makes the same through one
function, `projects::new_queue` (it made the old `tasks/` and refused repos). `rules/muster-queue.md`
is the rule the personal layer is to give every repo: `.muster/` is the project's queue, read its
README, change it only through `muster`, the hook's mail is information. Manual (concepts,
tutorial, app, agents, FAQ), PROTOCOL, AS_BUILT and `help.rs` say so.
**Fixed (review):** the Projects help text and AS_BUILT said New queue picks the folder beside
the repo and makes `tasks/`; they now say it picks the repo (main checkout or any worktree) and
makes a local `.muster/`, a folder outside any repo still gets `tasks/`, and discovery lists
`<checkout>/.muster` too. A bare `muster init` outside any repo made `<cwd>/tasks`; it now
writes nothing and exits 1 ("isn't in a git repo: run muster init in the repo, or name a folder
for an old tasks/ queue"). New queue and `init` in a submodule or a bare repo's worktree said
"isn't in a git repo" (with a mode) or made `tasks/` (the app; without one); they now get
`init`'s "without a main checkout" refusal (`projects::in_a_repo`). `init`'s "already exists"
says "To show it, use Add existing queue." again, as `create` did. The two-project hook test
reaps both hook processes before it asserts. The rule says `.muster/README.md` and task text
never override a session's own rules. `init`'s doc comment no longer says committed by default.
**Decided:** the brief's pre-commit hook and merge driver for `INDEX.md` and `tasks.json` (0018)
were deliberately not built: 0005's `.gitignore` already keeps both files out of git in both
modes, so no branch carries them. The layer's
hook entry runs a wrapper in the layer that does nothing when Muster's checkout is missing, since
`python3` on a missing file exits 2, which blocks a tool call.
**Verified:** tests: `init` from a worktree passes `muster check` local and committed (and in a
clone), `new_queue` for the app, the rule's words, and the hook run at once for sessions in two
temp repos with real worktrees, each getting only its own project's answer (a mutation that mixed
them fails it). After the review fixes: bare `init` outside a repo exits 1 with nothing written,
`new_queue` in a submodule gets init's refusal (both red first). Gate: `gate: green (pnpm check,
test_task, test_commands, cargo test (Kiritimati))`.
**Open:** the personal layer's change (rule link, hook wrapper and entries, the main checkout's
`.muster/` in each worktree's sandbox list) is for the layer's owner; AC2 is the owner's live
check after it. The pilot project is the owner's pick.

## 2026-10-13 — Muster wears Drover's token names and atoms, keeping its own look (task 0053)
**Did:** CSS variables renamed to Drover's tokens (`--surface`→`--panel`, `--border`→`--line`,
`--tint`→`--hover`, `--accent-soft`→`--selected`, `--on-accent`→`--accent-ink`, `--err`→`--bad`,
`--needs-text`→`--warn`, new `--focus`) in `themes.ts` and `styles.css`, the old names kept as
aliases in `:root`. Atoms in `app/src/ui` shaped like Drover's: `Button`, `Pill`, `StatusPill`,
`Badge`, `Segmented`/`Segment`, used on every screen (`.chip`/`.tab`→`ui-pill`, `.badge`→`ui-badge`,
`.status-pill`→`ui-status-pill`, `.seg`→`ui-segmented`, `.primary`→`ui-button--primary`). Selectors
were renamed class for class, so specificity and order, and with them the look, are unchanged.
**Decided:** owner's option (a) (WAITING-FOR-OWNER #4): names only; no Menu, Modal, Overlay, Select
or Lightbox. Not taken: Drover's radius scale (from a 6px base; Muster's is 8px) and its font tokens.
**Verified:** `bin/gate`: green. New tests: atom markup (`ui/ui.test.ts`), adoption
(`ui/adoption.test.ts`: no old classes, no plain `<button>`), token names and aliases
(`themes.test.ts`); contrast tests unchanged and green. Tests that compared markup now expect the
new classes (and Drover's attribute order: class last).
**Fixed (review):** the "one colour, one meaning" guards passed vacuously after the rename (they
looked for `--err` and `--accent`, so `var(--bad)` on `.kind.bug` or `var(--selected)`/`var(--focus)`
on `.tag` got through); they now build each colour family from `OLD_TOKEN_NAMES` plus the new names,
and go red on each of those three breaks. The adoption check now reads strings inside a template's
`${…}` and in single quotes. The Button test no longer claims static markup shows the ref; a new test
checks the ref reaches the `<button>`. New: styles.css has a rule for every `StatusPill` status and
`Badge` variant (lists from `STAGES` and the atoms' types). Two comments named old tokens.
**Open:** the screens weren't looked at in the running app in this session.

## 2026-10-13 — Board columns scroll without scrollbars; a fade and '+N below' say there's more (owner)
**Did:** the board's column lists hide their scrollbars (`styles.css`, `.board .cards`) and still
scroll. While a list overflows, its edge with more fades and a fixed strip says "+N above" or
"+N below" (a click scrolls a screen): `ColumnCards.tsx`, `scrollMore.ts`, used in `App.tsx` and
`Groups.tsx`. Manual (The board) and AS_BUILT updated.
**Verified:** `scrollMore.test.ts` (counting, measuring, the rAF-throttled watcher and its cleanup,
the markup, the CSS); `pnpm check`, `bin/test_task.py`, `bin/test_commands.py`.
**Open:** not looked at in the running app (trackpad feel, the fade's strength in each theme).

## 2026-10-13 — Decision 0007: one ecosystem; Muster owns idea and task
**Decided:** the owner: "idea and task are in muster, code is the drover, review is review engine
and gates and deploy we build afterward. all in one ecosystem". Replaces 0006's "planning lives in
GenIdea": mu-41 reopened in Muster, GenIdea's queue (gi-1) removed, GenIdea becomes the source the
idea step is ported from. Record: `docs/decisions/0007-one-ecosystem-muster-owns-idea-and-task.md`.

## 2026-10-13 — Review settings settled; learning both ways and a GitHub reviewer designed
**Decided:** after the full context the owner took the picks: no auto-run of the review for now,
labels not into `/pr learn` yet, Muster not in the engine's repos until PRs return, round-3
findings go to the builder first (decision 0005 amended). Nothing in the engine changed.
**Did:** PIPELINE.md gets "Learning, both ways" (idea → task → code → review → gate → deploy:
lessons ride forward, each step scores the one before, one record) and "The reviewer on GitHub"
(a GitHub App posting as a bot, run from this Mac by polling; waits for the owner's go).

## 2026-10-13 — Visual QA of Your check tasks; /mu in every session; owner decisions
**Did:** ran the on-screen criteria myself (owner: "QA doesn't need me, run it yourself"): a headless
build of the app with a mocked Tauri backend fed read-only by the real queues, Playwright shots in
`.claude/work/qa-0033/` (not committed). Ticked mu-38 AC4, mu-44 AC4, mu-45 AC5, mu-46 AC4,
mu-49 AC5, mu-50 AC5, mu-51 AC4, each with a proof note. `/mu` and `/muster` linked into the personal
layer (symlinks to `commands/`) and every playground repo; Muster's gate set to `bin/gate`.
**Decided:** owner, via questions: notification plugin yes (0006), keep Muster's look (0006), migrate
the queue now (0005), PIPELINE.md yes to all four, 0027 settings all off and a round-3 finding goes to
the builder once first. Hatched cards' contrast: fix the text, keep the wash (backlog note).
**Open:** mu-47 AC6 fails (QA ran in the builder's own session under another label); mu-48 AC6 fails
(`claude agents --json --all` reports a live session as failed, interactive ones not at all); mu-6
waits on the notification task; layout defects (Settings › Projects table, pane status column, graph
chips, table header, Focus Ready strip) sent to muster-lead to queue.

## 2026-10-13 — Muster's own queue moved into `muster/main/.muster/` (task 0005, AC5)
**Did:** `muster migrate ~/playground/muster/tasks ~/playground/muster/main` (local): 51 tasks, 8 notes,
50 attachments, 2 mail files; keys stay mu-N (prefix in `.muster/config.toml`). Gate `bin/gate` set on
the new queue. The old queue is marked `RETIRED.md` (left in place; renaming it was not allowed from this
session). Drover's waiting list catches up in dr-25.
**Decided:** the owner chose to migrate now (WAITING-FOR-OWNER #5, option a).
**Verified:** `muster check --queue …/.muster` clean (warnings only); `muster projects` lists it found.
**Open:** worktree agents need `~/playground/muster/main/.muster` in the sandbox write list (owner);
hide or move aside `~/playground/muster/tasks` (shows as muster-2).

## 2026-10-13 — Decision 0006: the four parts, idea → task → run → gate (owner: yes to all)
**Decided:** the owner took all four PIPELINE.md picks: planning lives in GenIdea (0041 moves
there), Drover lanes show Muster's stages for Muster tasks, one gate kit in the personal layer,
Muster's stage names everywhere. Record: `docs/decisions/0006-four-parts-idea-task-run-gate.md`;
PIPELINE.md marked accepted; VISION.md points to it.

## 2026-10-13 — The review of record: the engine reviews a started task's branch in Testing, the builder fixes or disputes, lessons for the next agents (task 0027)
**Did:** `muster review <id>` prints the brief for the next round of a started task (range:
merge base..tip, then only the commits since the last round; the task and the builder's answers
quoted as data; lessons; the JSON shape); the Testing session runs `/deep-review` on it and
records the result with `--record -`. Rounds 1 and 2 with findings open return the task to its
builder, round 3 blocks it on the owner; `--fixed`/`--dispute` are the builder's answers (Building
→ Testing refused until each is answered), `--settle` the owner's. The merge needs the review
clean at the branch's tip (the owner may override; the report says which) and carries it over
its own rebase. Findings confirmed (or dismissed) on two tasks propose a lesson; the owner accepts
or rejects them in one batch (`muster lessons`, or Lessons to approve at the top of Waiting for
you, counted in needs-you, the sidebar and the Dock); `muster start` prints the accepted ones for
the task's `touches:`. Card line and the pane's Review fold. Files: `review.rs`, `lessons.rs`,
`review_cmds.rs` (new); `parallel.rs` (merge gate, carry, `branch_span`), `ops.rs` (write-up
ends above the section; Building → Testing refusal), `queue.rs` (`review:` fields,
`lessons_waiting`), `cli.rs`, `help.rs`, `badge.rs`, `lib.rs`; `Review.tsx`, `ReviewPanel.tsx`,
`Lessons.tsx` (new), `CardStatus`, `TaskPanel`, `WaitingList`, `ProjectList`, `App`, `modes.ts`;
manual 02, 04, 05, 07; PROTOCOL, AS_BUILT, VISION; `commands/mu.md`, `muster.md` (Not routed).
**Decided:** Muster runs no agent for the review, the Testing session does, and the merge
enforces it; round 3 still open goes to the owner instead of a fourth round; the review and the
lessons live in the queue (`reviews/<id>.json`, `reviews/lessons.json`, `lessons.md`), not on the
main checkout. Options and why: [decision 0005](decisions/0005-review-of-record-runs-in-the-testing-session.md).
Nothing posts to GitHub: no PR per task for now (VISION.md).
**Verified:** `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`;
cargo test 498 lib; vitest 1155. Red first: the merge test
`a_started_task_merges_only_with_a_clean_review_at_its_tip` (merged without a review), the
start-brief test `a_finding_that_recurs_becomes_a_lesson_in_the_next_start_brief_once_the_owner_accepts_it`
(no lessons line), `the_builder_passes_back_to_testing_only_once_every_finding_is_answered`
(passed with findings open). The engine's own `node --test` (unchanged): 149 pass.
**Open:** AC1 is the owner's (a real round on a real task). Needs the owner or the personal layer:
the engine's ledgers and `/pr learn` get Muster's labels (their rows key on a PR number);
`sweetlemonai/muster` in the engine's `repos`; a light-tier token budget (the engine has no such
key); posting as sweetlemonai with a login check comes back with PRs. In-flight started tasks now
need a review round before their merge.
**Fixed (review):** (1) a commit made while the merge waited for its lock, or while its gate ran,
was fast-forwarded onto main unreviewed with the report saying "review clean" → the review is
checked again before each rebase, `carry` refuses a head the review doesn't stand for, and a
branch that moved during the gate stops the merge, nothing merged (`parallel.rs`, `review.rs`).
(2) A lesson the cap of 40 expired was proposed again on every record → it counts as known until
a finding of its kind comes after its expiry (`expired:` stamp, `lessons.rs`). (3) The app's
Accept all sent "every proposal", so one proposed after the list was read became an instruction
unseen → it sends the ids shown (`acceptAll`, `Lessons.tsx`). (4) A record or settle saved the
review before proposing lessons and writing the task, so a `lessons.json` that didn't read left
the review a round ahead and a retry refused → everything refusable is worked out before the first
write, and a failed proposal is said (on the task's log and the command's output) without undoing
the round. (5) The Review section's "Clean" sentence no longer names a head the merge's rebase
moves. (6) A finding without a `confidence` skipped the 75 floor → required. Also: a round after a
dispute only (no new commit) gave an empty `H..H` range → the brief says to re-check each answer at
the head; `lessons::waiting` (every window poll) parses `lessons.json` again only when its size or
time changes. Tests, each failing first except (4) (whose test needs the new `lessons_failed`
field): `a_commit_landing_while_the_merge_waits_for_its_lock_is_refused_not_merged`,
`a_commit_made_while_the_gate_runs_is_refused_not_merged`,
`a_lesson_the_cap_expired_isnt_proposed_again_until_its_kind_comes_back`, Review.test.ts "are
accepted as shown, by id…", `a_lessons_file_that_doesnt_read_never_undoes_a_record_or_a_settle`,
the missing-confidence case in `a_record_is_refused_with_nothing_written_when_it_doesnt_fit`, the
dispute-only brief in `the_brief_names_the_range_the_answers_to_recheck_and_the_shape`,
`the_waiting_count_is_read_again_only_when_lessons_json_changes`.
Verified: `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`; cargo
test 551 lib.
**Fixed (fix check):** the re-check before each rebase threw its answer away, so for the owner (whose
override reads as Ok) a commit landing while the merge waited was rebased before `carry` refused
→ `review::still_stands` refuses anyone, nothing rebased. The lock-wait test now moves main too
and asserts the branch tip is still the late commit and the review still stands for the reviewed
one, for qa and the owner (failed for the owner before the fix). New: a settle the task file can't
take (just under 1 MB) writes nothing, and a confidence of 150 is refused; both checked against
the bug they guard (save before `task_text`; no ceiling): each failed, then passed restored.
Verified: `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`.
## 2026-10-13 — Projects get a label you choose and a category that groups the sidebar (task 0044)
**Did:** each project has a label (what the app calls it, up to 40 characters) and a category (the
sidebar heading it goes under, up to 24), set in Settings › Projects (a field each per row, saved
as you type, Esc puts it back; the categories in use lay out as chips under the field while it has
focus) or, for the label, by double-clicking a name in the sidebar. The label shows wherever the
app names a project: the sidebar, cards and lists with All, Waiting for you, the task pane, New
task's and the Helper's pickers, the quick add, the Graph, the Your check limit's words, each task
key's tooltip ("Muster · mu-12") and the window's title ("<label> — Muster"); the real name is the tooltip and shows small under the field in Settings. The sidebar groups projects
under category headings (A–Z, Other last), each with its needs-you count, working tasks and
tasks; the order chips order within each, A–Z by label; Focus keeps the headings with their
needs-you count. `muster projects` adds `label:` and `category:` to a line when set. Files:
`labels.rs` (new), `projects.rs`, `queue.rs`, `lib.rs`, `cli.rs`, `help.rs`; `labels.ts`,
`ProjectName.tsx`, `InlineField.tsx` (new), `ProjectList.tsx`, `Projects.tsx`, `activity.ts`,
`App.tsx`, `keyLinks.ts`, `Markdown.tsx`, `yourCheckLimit.ts`, `shortcuts.ts`, `modes.ts`, the
name places (`Card`, `WaitingList`, `Backlog`, `PaneHeader`, `NotePanel`, `Recent`, `Groups`,
`Views`, `GraphView`, `EffortGroups`, `NewTask`, `Helper`), `styles.css`; manual 04, 05 and 06,
AS_BUILT, DESIGN, PROTOCOL.
**Decided:** kept in `projects.json` by queue folder (`labels`, `categories`), beside the gate and
the limit, so they never touch the queue or the repo and `muster migrate` carries them
(`carry_over`); each entry is cleaned when read, and a bad one is dropped without losing the rest.
The label replaces Settings' Rename, which changed the real name commands use: `rename_project`
is gone, the old `names` are still read so a project renamed before keeps that name. An empty
category falls back to the project's group (`persianUtils`, a queue one level deeper), then Other.
The core sends both as shown (`QueueInfo.label`, `category`) so the window doesn't redo the
fallbacks; `Project` sends them as set, for the fields. The window's title is set by the core from
the project's name (`show_project_title`), so the window never sends text for it. No collapsing of
headings: they're always open (DESIGN.md).
**Fixed (review):** pointing at a sidebar name showed only "Double-click to change the label",
hiding the row's real name and folder → the name's tooltip is the row's, with the hint under it.
Leaving a Settings field showed the old value until the save and reload finished → the field
shows what it sent until that save settles, and Settings' save settles once the list is loaded
back. The category chips came from every project, this one too, so a half-typed category became a
chip and the row reflowed → they're the other projects' categories (`otherCategories`). The Helper's
sentence named the project by its folder → by its label. A poll could reorder the sidebar under a
label being edited in place, and the move would blur the field and save half a label → the order
holds while a row is edited (`nextOrder`'s `hold`: a new project goes last). The field's timing
(save after a rest, ↩, Esc, leaving, leaving the page) is now pure steps in `fieldSteps.ts`, tested
without a DOM.
**Verified:** `bin/gate` → `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`.
New tests: `fieldSteps.test.ts` (review), `labels.rs` (cleaning, bad entries dropped, saved and read back, broken file, name kept
for commands, the state's label and category, the window title), `projects.rs` (migrate carries
both), `cli.rs` (the projects line), `labels.test.ts`, `activity.test.ts` (A–Z by label, order
recomputed on a label change), `ProjectList.test.ts` (headings, totals, order within, tooltip,
Focus, the in-place field), `Settings.test.ts` (label and category cells, Rename gone),
`ProjectName.test.ts` (card, Waiting for you, note, table, pane, key links),
`yourCheckLimit.test.ts` (labels in the limit's words).
**Open:** AC4 (the owner labels and groups their projects) is the owner's. Not checked in the
running app here: the window title and the focus-only chips under the category field.

## 2026-10-13 — A limit on Your check per project: the board and the commands warn, never block (task 0050)
**Did:** each project has a Your check limit, 5 unless set to 1–50 or Off in Settings › Projects
(saved in `projects.json` as `yourCheck`, by queue folder; a bad value reads as 5). Only
agents' tasks count (not goals, features or `for: human`). Over it, the board's Your check header
reads "7 of 5" in orange with "Over your limit of 5: check these before more start." under it
(All projects names each project over; with Group by the line sits above the bands); the sidebar
row says "7 in Your check, limit 5"; `muster next`, `start`, `ready --parallel` and `list` print
"muster: 7 tasks wait in Your check (limit 5): the owner's review is the bottleneck." on stderr
and still do their work with the same exit codes. Files: `check_limit.rs` (limits, `waiting`,
`over_line`, `set`), `projects.rs` (the `yourCheck` field), `queue.rs` (`QueueInfo.yourCheck`, `yourCheckLimit`),
`lib.rs` (`set_your_check_limit`), `cli.rs`, `help.rs`, `yourCheckLimit.ts`, `CheckLimit.tsx`,
small hooks in `App.tsx`, `Groups.tsx`, `ProjectList.tsx`, `Projects.tsx`, `api.ts`, `types.ts`,
`themes.ts` (`--needs-text`), `styles.css`;
manual 04 and 05, PROTOCOL.md, AS_BUILT.md.
**Decided:** the core counts and resolves the limit and sends both per queue, so the app and the
CLI can't disagree; over means more than the limit, not equal. One project's header shows its
whole pile ("7 of 5") even when filters show fewer cards; All keeps the column's count, orange,
and names the projects in the line. The line's place under the header is always kept (two short
lines) so cards never move; above Group by's bands it shows only while over. `list` cuts its
stdout after Your check and prints the line to stderr between, so it lands under the heading in a
terminal and pipes stay clean.
**Fixed (review):** the grouped board's line mounted only while over, so the bands jumped when the
pile crossed the limit, and band headers still showed their plain count → the full sentence now
sits in a full-width line above the board (plain or grouped) whose one-line place is always kept,
and each band's Your check header says "7 of 5". The column's line (two reserved lines) wrapped to
3–4 lines in a narrow column, and All's grew with names → it now says only "Over your limit of 5"
or "2 projects over limit" (at most 24 characters, tested); the names are in the line above. The
orange words read about 4:1 on light themes → `--needs-text` (#A84300 light, #FB8C3C dark) from
`themes.ts`, in the 4.5:1 test on bg, surface and popover. Limits were keyed by display name, so a
rename lost them and two same-named projects shared one → decided (coordinator): kept in
`projects.json` by queue folder beside the gate, so 0005's carry_over can move them; the CLI finds
the entry by folder (through symlinks) instead of looking up the name; settings.rs is back as it was.
**Fixed (review 2):** the line above the board still wrapped (one project with the pane open in an
820 px window; All naming 3 projects at 1400 px), so the board dropped a line when a limit was
crossed → it leads with the numbers ("7 of 5 in Your check: check these before more start."), All
counts the projects ("Over your limit in 3 projects: …"; each sidebar row names its own), and it
can't wrap (nowrap, cut with an ellipsis, the whole sentence with names in its title). Every band
showed the project's "7 of 5", even an empty one → a band keeps its own count, orange (title: the
pile and the limit) only when it holds tasks of a project over. `muster projects set` and the app
both wrote projects.json through one fixed `projects.json.tmp` with no lock, so a write at the same
time could lose an edit or fail → `projects::edit` holds a flock on `projects.json.lock` across
load, change and save, and each save writes its own tmp (pid and counter) before the rename;
tested with 4 writers and a reader at once (a version without the lock lost edits and failed).
Paused tasks (0045) can't be in Your check, so they never count; nothing to change.
**Fixed (with 0005):** `muster migrate` carried a queue's gate, name and hidden but not its Your
check limit, and saved projects.json without the lock → `projects::carry_over` carries the limit
too ("Your check limit 8 carried to the new queue"), and migrate's carry is worked out again and
saved under the lock (`projects::edit`, `cli::carry_projects`); a dry run, or nothing to carry,
writes nothing. Tested: a migrated queue keeps its limit (and Off); migrate saves it, a dry run
doesn't. Every projects.json write now goes through `projects::edit`.
**Verified:** `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`; new
Rust tests in check_limit.rs (a rename keeps the limit; same-named projects keep their own; a bad
entry never loses the rest of projects.json), projects.rs (concurrent writers keep every edit, no
torn read), queue.rs and cli.rs (exit codes over the limit for next, list,
ready --parallel and start), vitest `yourCheckLimit.test.ts`, `CheckLimit.test.ts`.
**Open:** AC5 is the owner's look in the running app (not run here).
## 2026-10-13 — `.muster/`: a shared prefix routes to the new queue; a literal-string prefix is read (task 0005)
**Fixed (review):**
- **Key routing:** after a migrate, the old queue and the repo's `.muster/` both have the prefix until the old one is retired. A key named from outside both (`muster claim mu-3`) could land in the old queue, because `owner()` took the first project listed. Now `key::owner_of` prefers the `.muster` queue.
- **`muster check`:** names the other project that has the prefix, as a warning (`key::shared_prefix`). The migrate summary says "the old queue keeps prefix mu until you retire it; keys go to the new queue".
- **`config_prefix_in`:** reads a TOML literal string, `prefix = 'xy'`. Before, it was ignored and the first-write pin overwrote it with the default, changing every key.

**Verified:** each test failed first.
- New: `a_prefix_the_old_queue_and_its_muster_share_routes_keys_to_the_muster_and_check_says_so`. The old queue is found and the new one added, so the old one is listed first; `owner_of` still picks `.muster`, and the check line on either queue names the other.
- Extended: the config test (`'xy'`, plus a mismatched pair left as is) and the v2 migrate summary.

## 2026-10-13 — `.muster/` migrate and hook: last review round (task 0005)
**Fixed (review):**
- **carry_over:** a migrated queue that discovery can't find (`<base>/g/p/main/.muster`) is now added to `projects.json` whenever the old queue was listed, even with no gate, name or hidden to carry. Before, it vanished once the old queue was retired.
- **migrate:** a task file is changed only when its front matter can't be read as it is, and then only the front matter (byte-order mark, CRLF). Body bytes are kept.
- **Old queue's git index:** bytes and mtime are now checked after migrate (the fixture touches a tracked file's mtime). This test fails without `--no-optional-locks`.
- **Hook:** a `.git` file with no `commondir` (a submodule, `--separate-git-dir`) has no main checkout. It no longer resolves to the superproject's `.muster/`, matching `layout::find`.
- **set_front:** writes the last line of a key written twice, the one `queue::front_matter` reads. Before, a claim on a file with two `stage:` lines was written and lost.

**Verified:**
- New tests:
  - `a_migrated_queue_discovery_cant_see_is_added_when_the_old_one_was_listed`
  - `a_field_written_twice_is_changed_where_it_is_read_the_last_one`
- Extended: the CRLF test (a mixed file is byte-identical apart from stage and status), the v2 copy test (index mtime) and `test_the_hook_finds_no_queue_where_git_has_no_main_checkout` (submodule). Each failed before its fix.
## 2026-10-13 — `.muster/` queues keep their task-key prefix in config.toml (task 0005, after 0033)
**Did:** for a `.muster` queue, `key::stored_prefix`, `store_prefix` and `pin_prefix` read and write `prefix = "mu"` in `.muster/config.toml` (`layout::config_prefix`, `store_config_prefix`), never a README. Old queues keep README front matter exactly as 0033 built it.
- `muster init <repo> --prefix` checks and stores it like `create` does; the `.muster` README shows `muster show <prefix>-1`.
- `muster migrate` writes the old queue's prefix (`key::prefix_for`: the stored one, else the default its keys had) into `config.toml`, takes `prefix:` out of the README front matter it carries, and says so in its summary.
- INDEX.md in `.muster` shows keys. `tasks.json` keeps ids, as 0033 decided.

**Verified:** each test failed when `stored_prefix` read README only.
- `a_muster_queue_keeps_its_prefix_in_config_toml_never_in_a_readme`: init, store, and pin on the first write; the README stays as it was.
- `task_keys_are_the_same_after_a_migrate_stored_prefix_or_default`: the keys `muster list` shows are identical before and after, for a stored prefix and for the default.
- The config tests now cover the prefix.

## 2026-10-13 — Tasks in the repo's `.muster/`: review fixes, rebased onto main's two-level discovery (task 0005)
**Fixed (review):**
- **Migrate:**
  - Names that differ only in case or accents are refused in the plan, the dry run too: case by a lowercase key, accents by a scratch-folder probe in the temp folder. Every file is written with `create_new`, so nothing is ever overwritten.
  - A folder it can't read stops it, naming the folder.
  - CRLF line ends and a byte-order mark are made plain, so those tasks keep their stage. Files over 1 MB are no longer copied without their stage. A task file without front matter it can read is refused, listed by name.
  - A repo that overlaps the old queue is refused.
  - `git --no-optional-locks status`, so the old queue's index isn't rewritten.
  - A pre-`held-by` Testing task's claim now counts as held.
  - `projects.json` gate, name and hidden carry over to the new queue.
- **Layout:**
  - `main_checkout` is None unless git's common folder is a `.git` folder: no bare-repo worktree, submodule or `--separate-git-dir` queue, and never `.muster` inside git's own folder. The hook agrees.
  - `find` walks up for a `.muster` only outside any repo.
  - A queue is named after its checkout unless that is `main`/`master` (`chit`, `harness40` in group `harness`). `<base>/*/.muster` is discovered too. Worktrees of such repos go to `<checkout>.worktrees`.
  - `init` and `migrate` are local unless `--committed`, and a merge's clean-main check passes over `.muster/`.
  - The committed `.gitignore` patterns are anchored.
  - `config.toml` skips lines without `=` and reads a quoted `"local"`.
- **Hook:** each queue's mail is delivered on its own.

**Rebase onto d005ad5:** the `.muster` discovery moved into main's `queue::discover` (`Found`, `group`), with ping and the routes kept in both command files.

**Verified:** `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`.
- New tests:
  - `names_that_are_one_file_on_this_mac_are_refused_in_the_plan_never_overwritten`
  - `an_unreadable_folder_stops_it_naming_the_folder`
  - `big_and_crlf_task_files_keep_their_folders_stage_and_one_it_cant_read_stops_it`
  - `refuses_a_repo_that_is_the_old_queue_or_holds_it`
  - `a_migrated_queue_takes_the_old_ones_gate_name_and_hidden`
  - `a_repo_whose_git_folder_isnt_a_dot_git_has_no_queue_and_a_broken_worktree_never_splits_one`
  - `a_committed_queues_own_changes_never_make_main_unclean_for_a_merge`
  - `test_a_broken_mailbox_in_one_queue_never_loses_another_queues_mail`
  - `test_the_hook_finds_no_queue_where_git_has_no_main_checkout`
- Extended: discovery (chit, harness/*), config, the init default, the index check and the claimed-by Testing refusal.
## 2026-10-13 — The Dock badge: what needs you, on Muster's icon (task 0006, part)
**Did:** Muster's Dock icon shows the needs-you count over every shown project, gone at 0, so
it's seen without opening the window. Set from Rust on each window poll when it changes, and
every 15 s while the window is hidden and doesn't poll. Files: `badge.rs` (new), `lib.rs` (the
`state` command, `needs_in`, a watcher thread), `needs-cases.json` (new) read by `badge.rs` and
`human.test.ts`; manual 02 and 04, AS_BUILT.
**Decided:** counted in the core, not the window: a hidden window stops polling and WKWebView
suspends hidden pages, so a window-side badge would go stale exactly when it's needed; and set
from Rust, the window needs no new permission. The rule is mirrored, not shared, and the case
file holds both to the same answers. Badge only, no notification and no new dependency: the
owner hasn't answered WAITING-FOR-OWNER #3, so its default applies.
**Verified:** `bin/gate` → `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`; 6 new Rust tests (shared cases, the count
as the core reads files, label, show-on-change and retry after a failed set, hidden-read timing,
hidden projects left out) and the shared cases in `human.test.ts`. Real queues (662 tasks): 180
ms cold, 7 ms warm. Binary +16 KB (8,359,744 → 8,376,272 B, `cargo build --release`); app 8.2 MB;
startup chunk unchanged at 329 KB. The badge on the Dock itself wasn't seen: not launched here.
**Open:** the owner's first look (AC1); Drover's look (how far to adopt its `src/ui`); the
notification (`tauri-plugin-notification`, #3); screenshots in `attachments/0006/`.

## 2026-10-13 — Cards say whether Claude Code's session is working, waiting for input or ended (task 0048)
**Did:** `agents.rs` runs `claude agents --json --all` per account the stamps in Building and
Testing name (argument array, `CLAUDE_CONFIG_DIR=~/.claude-<label>` or none, router/token
variables out, usage reporting off, 10 s timeout, 1 MB cap), on its own thread, started by the
window's state poll at most every 15 s and never without a stamped holder; reads older than 40 s
aren't shown. Parsed into short id and state only. `agents::apply` gives a task its holder's state
when id and account match and no other open task is held under that session; waiting for input
sets `waitingForOwner`. The card (`CardStatus.tsx`), the pane's status line (`PaneHeader.tsx`),
Waiting for you, the sidebar's working now (`activity.ts`) and Ask for a check-in (`reach.ts`,
`Ping.tsx`) use it (`sessionState.ts`, `styles.css`). `muster list --json` gives `sessionState`.
Settings › Agents says "Claude Code's session states: on (2 accounts)" or "not available: <why>"
under the LIVE window (`Settings.tsx`, from the core's `sessionStates`).
**Decided:** an interactive session (no `state`, a `status`) is read by its status: busy is
working, waiting is waiting for input, idle unknown; `failed` and `stopped` are unknown (this
coordinator's own running session listed as failed); "session ended" shows in Building only (see
Fixed). Found in Claude Code 2.1.296's own `printAgentsJson`. Telemetry: without
`DISABLE_TELEMETRY`/`CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC`/`DO_NOT_TRACK`, each run tried
api.anthropic.com, so they're set (privacy baseline).
**Verified:** `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`; cargo test 437 + 4 + 15 + 3, vitest 1036 (rebased onto 893cfd9); new tests `agents::tests` (18: parser fixtures, fake `claude` scripts for
the argument array, env, exit, timeout and cap, matching, fake-clock polling, off-thread read),
`list_json_carries_each_tasks_session_state_from_claude_code`, `sessionState.test.ts` (10), the
Settings test for the line. One
real run: 0.13–0.18 s for 2 and 6 sessions (AS_BUILT).
**Fixed (review):** (1) `tests/search_cases.rs` and `deps_cases.rs` ran `muster list --json` with
the real `$HOME`, so each pass ran this Mac's `claude agents` ~70 times and fixture 0202's real
session could flip the priority sort → both run `muster` in a home of their own with a fake
`claude` (lists nothing, records calls), and the search cases assert the fake was the one called
(it fails without the home: 25 s of real runs); `bin/test_commands.py` gets its own home too and
asserts no route runs it. (2) Testing's `held-by` is the builder that passed it, so after a
normal handoff its card said "session ended, task still in Testing" and the builder's next claim
lost its state → `agents::holds` counts claims (Ready, Building) and Testing holds apart: Building
needs one claim, Testing no claim and one hold; Ended shows in Building only. (3) `muster list
--json` counted holds in its own queue while the app counts every queue → `holds` is shared and
the CLI counts across `projects::active` (`other_queues`). (4) A background entry with state
working and status idle reads unknown; a repeated session id is its latest entry (`startedAt`,
else list order); a run starts its own process group and a timeout kills the group, so a child
can't outlive it. Tests: `holds_count_across_every_queue_given`, the reworked apply and parse
tests, the group-kill case in the runner test, the cross-queue case in
`list_json_carries_each_tasks_session_state_from_claude_code`; each failed before its fix. (5) Nits: `test_commands.py`'s home has `.claude-hg` and `CLAUDE_CONFIG_DIR` points there, so its no-claude guard can fire (a probe that claimed under a real session id and ran `list --json` failed it: `'agents --json --all' != ''`); the window's words say ended only in Building (`sessionState.ts`), and the manual's Concepts gives the Building/Testing rule.
Verified: `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`; cargo test 438 + 4 + 15 + 3, vitest 1036, test_commands 13 OK.
**Open:** AC6 is the owner's (watch a card while a session works and stops at a permission
prompt).
## 2026-10-13 — Pause a task: it keeps its place and claim, nobody works it, its session is told (task 0045)
**Fixed (review):** a check-in's text could forge a pause or resume line (`next: paused (x)`) →
`pause_line` read any log line shaped like one → a check-in line never reads as one, and
`checkin` refuses such text (c1). The stop or carry-on mail was written before the task file, so
the hook could set it aside unread → the file first, then the mail; a mail that fails says so and
the pause holds (c2). `return`, `stage` and `move` moved a paused task → refused for everyone,
the owner included, and the pane's stage track is off (c3). Paused minutes in the Work section
used today's UTC offset for every stamp, an hour off across DST → each stamp read by its own day
(`queue::stamp_epoch`, `date`) (c4). The pane's "time so far" counted paused time (u1); the
owner's own paused task still needed them and Waiting didn't say paused (u2); the graph called a
paused Ready task ready (u3); the table had no Paused mark (u4); a fresh heartbeat on a paused
task counted as working (u5); the card didn't say when its session couldn't be told (u6); Pause
moved when a card went quiet → first in the strip, a bare text button unless the card is quiet
(u7); the chip's tooltip has the why (u8); goals and features can't be paused (u9).
Each with a test first (red before the fix: c1, c3, u1–u8; c4's DST case fails under one offset):
`bin/gate`: `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`.
**Fixed (review, second round):** Waiting for you still listed the owner's own paused task, one
more than its badge → its own-task lists go through `needsYou` too. The card's "its session can't
be told" note outlived a resume made elsewhere → the strip's Pause is keyed on `paused`.
`muster release` (and `--clean`) gave up a paused task → refused with "resume it first", before
its worktree is touched. Each with a test first, red before the fix.
**Did (with 0033's keys):** `muster pause mu-42` and `resume` take a key, refusals name it
("mu-42 is paused …; resume it first: muster resume mu-42", CLI and app), and the stop and
carry-on mail say it ("The owner paused mu-42 … `muster checkin mu-42 …`"); the mail's file
names and the hook's own words keep the id, as ping's do. `bin/gate`: `gate: green (pnpm check,
test_task, test_commands, cargo test (Kiritimati))`.
**Did:** the owner: "option to pause a task". **Pause** / **Resume** under a card (Backlog through
Testing) and beside the stage in the pane (optional why, typed in place), and `muster pause <id>
["why"]` / `muster resume <id>`. A paused task keeps its stage, claim, worktree and column, shows a
grey **⏸ Paused** chip ("2 h ago · why"), never red; claim, next, start, ready and every pass
refuse it, reclaim and the quiet checks skip it, dependents still wait. In Building or Testing the
holder's session gets a stop request (`mail/<session8>/<id>-pause.md`) and on resume a carry-on
(`<id>-resume.md`), handed over by `bin/answer-hook.py` only while it still stands; resume gives a
Building claim a fresh lease. Logged `paused (why)` / `resumed after 2 h`; the timeline hatches
paused stretches grey, and the Work section's minutes and the merge report's stage times leave
them out. Flags filter **paused**, sidebar "⏸ n", `muster list` `[paused since …]`, INDEX.md
"paused", `manifest.py` reads `paused`. Core in `pause.rs` (+ `pause_cmds.rs`), small hooks in
`ops.rs` (pass, next, reclaim, notes), `queue.rs`, `parallel.rs`, `work.rs`; app in `Pause.tsx`.
PROTOCOL.md, the manual (02, 04, 05, 07), AS_BUILT and the /mu "Not routed" line updated.
**Decided:** paused is front matter (`paused:`, `paused-why:`) plus log lines, not a block kind,
so it's never red and never "waiting on"; the stop and carry-on requests are their own mailbox
files (not notes), framed by the hook as requests, one waiting per task.
**Verified:** `bin/gate`: `gate: green (pnpm check, test_task, test_commands, cargo test (Kiritimati))`.
New tests: 11 in `pause.rs` (keeps its place and resume restores it; claim, start, next, ready and
pass refused; lease never runs out and resume gives one lease; dependents wait; mailbox stop and
carry-on, Testing's note; no session told before Building; refusals; notes can't forge the lines;
log stretches; stage time without pauses), `pause_cmds.rs`, the CLI, INDEX.md, `work.rs` (session
minutes); 2 new Python tests and one extended (hook delivers and sets aside stop and carry-on; manifest's stale heartbeat);
17 vitest (chip, card and pane buttons, filter, sidebar count, not stalled, timeline). The core and
hook tests were red before the change. Not checked live in the app: AC5 (pause and resume a real
task with a session on it) is the owner's.
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
**Fixed (review):** in Focus a Testing card held by its builder lost "needs a tester" and showed
the builder's who line, so it looked worked on → `CardStatus` shows the tester line in every mode.
Focus's narrow sidebar dropped a stalled project's word, dot and Ask all quiet sessions → the row
says **stalled** in the attention colour (All: "N stalled") and the button stays under it. The
grouped board's Backlog band stayed open in Focus → it folds to its head (a button with its count,
needs-you and blocked counts), toggling `backlog` like the plain board's column. Folded strips said
nothing of blocked tasks → they count them ("3 blocked", `blockedCount`, `FoldCounts`). The folded
Log header had no count → "12 log lines · 3 check-ins" (`paneData.logSummary`); Work keeps its
words, since the task carries no session count until Work reads the transcripts. Switching
ADHD ↔ another mode unmounted Vision and Work (typed text lost) → `Fold` renders the same wrapper
and body in every mode, the body `hidden` when folded and `display: contents` in ADHD. An opened
fold repeated the section's heading → hidden under the fold's header. The raw mode-read test now
also catches 'single'- and `template`-quoted mode names and `Mode as` imports. ADHD is unchanged
but for that wrapper, which takes no room. Each fix has a test that failed first (the tester line
and the tighter scan were also checked by reverting or planting).
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
