# 0009 — The work log merges through a muster merge driver

Date: 2026-10-14 · Status: accepted (the shaper decided it for task mu-67; the owner can overrule
it any time). Amended 2026-10-15 four times: entries under an entry, not only on top (mu-190), then
at any number of points (mu-196), then entries changed or removed on one side (mu-209), then a
result that loses an untouched entry refused (mu-218).

## Why

Every task puts its entry at the top of `docs/WORKLOG.md`, so any two branches clash there. Muster's
merge (`muster pass <id> --from testing`) rebases the branch on main. Before this, the rebase
stopped on that clash and sent the task back to Building. The builder fixed it by hand, and the new
commit meant the review of record no longer stood at the branch's tip. That cost a whole review
round, for two log entries that never really clashed.

## Decision

- **`muster merge-log %O %A %B` is git's merge driver for the log.** `.gitattributes` names it:
  `docs/WORKLOG.md merge=muster-log`. It runs a plain three-way merge (`git merge-file`) first and
  uses the result when that's clean, so edits to older entries merge as they always did. When the
  merge conflicts only because each side is the base with whole `## ` entries put on top, the
  result is the base with both sides' new entries on top. Each entry stays whole, and they're
  ordered by the date in their heading, newest first. On the same date the incoming side goes on
  top (theirs, `%B`: the commit a rebase replays, the branch a merge brings in), since it lands
  last. In any other case it writes git's conflict markers and exits 1: the preamble changed, the
  same older entry changed both ways, a side removed entries, or a new entry has no date. It never
  drops a line or reorders the lines inside an entry. A `## ` line inside a fenced code block
  (``` or ~~~, CommonMark's rules) is part of its entry, never a new one.
- **Git config isn't committed, so muster writes it.** `muster start` and `muster init` set
  `merge.muster-log.name` and `merge.muster-log.driver` in the repo's own `.git/config`, which
  every worktree reads. They only set a key git doesn't have yet, and leave every other key alone.
  That way a hand `git rebase main` in a worktree resolves the log the same way. The driver they
  write is `muster merge-log %O %A %B; s=$?; [ $s -le 1 ] && exit $s; git merge-file -L ours -L base -L theirs %A %O %B`
  (git runs it through `sh`).
- **Muster's own merge never depends on that config.** Its rebase passes the same driver with
  `git -c`, with this muster binary's absolute path in place of `muster`. The path's `%` is
  doubled (git expands `%A` and the rest before the shell sees it), then it's quoted for the shell.
- **When muster can't merge, git's merge-file does, so both sides always show.** A driver that
  exits non-zero without writing `%A` makes git mark the log conflicted with only our side in it:
  no markers, the other side's entry gone, and `git add` would lose it. So muster never leaves it
  like that. When it can't merge (a file it can't read or over 64 MB, merge-file failing, `%A` it
  can't replace: the result goes to a new file beside it, renamed over it, so a failed write leaves
  `%A` as it was), `muster merge-log` runs `git merge-file` in place: its markers and exit 1, or its
  clean result. Only if that fails too does it leave `%A` alone and exit 2. The driver's shell
  part covers a muster that never ran (not on the PATH, as for a GUI git client or
  `PATH=/usr/bin:/bin`), was killed, or exited 2: it runs `git merge-file` itself. Muster's own
  merge also aborts the rebase on any conflict, so nothing half-merged lands. With no driver in
  config at all (a hand merge in a repo muster never started in), git does its own text merge,
  markers and all, as before this change.

## Amendment 2026-10-15: entries inserted at one point (task mu-190)

The newest-first order above makes "on top" too narrow. When a later-dated entry lands on main (a
coordinator's log commit) after two tasks started, each task rebases with its own entries under
that entry, not on top. The next merge or rebase of the two then clashes below the top, and the
driver gave up: nearly every task merge on 2026-10-14/15 stopped on `docs/WORKLOG.md` and was
resolved by hand. The why of this decision covers that case, so the rule widens from "on top" to
"at one point":

- A side is accepted when it is the base's preamble and entries, unchanged and in order, with one
  run of new dated whole entries inserted at a single point among them (point 0 is on top, as
  before).
- When both sides insert at the same point, the result is the base with both runs merged there as
  above: newest date first, theirs above ours on the same date, each side's order kept, an entry
  both added kept once. The base's entries never move or change.
- Everything else keeps git's conflict markers, as before: two runs on one side, an undated new
  entry, a changed preamble or older entry. Inserts at different points on each side are left to
  `git merge-file`, which runs first and merges them itself when they don't touch.
- The driver's give-up message now reads "both sides changed more than new entries at one point".

The shaper made this call for mu-190 (the owner can overrule it); only the same-point case was
added, as the backlog note asked.

## Amendment 2026-10-15: entries inserted at any number of points (task mu-196)

The rule above still stopped the common case. A task with review rounds has its first entry
rebased below main's newer entries, then puts each round's entry on top. Its log then has new
entries at two points, and mu-173's rebase onto main stopped on `docs/WORKLOG.md` with "both sides
changed more than new entries at one point" and was merged by hand. The why of this decision
covers it, so the rule widens again, from "at one point" to "at any number of points":

- A side is accepted when it is the base's preamble and entries, unchanged and in order, with any
  number of runs of new dated whole entries, each at a point among the base's entries.
- The result is the base with, at each point, one side's run as it is, or both sides' runs merged
  there as above (newest date first, theirs above ours on the same date, each side's order kept).
  An entry both sides added is kept once, at ours' point when the sides put it at different
  points, since ours is what the target branch already shows; a side that repeats it keeps its
  copies (each entry max(ours, theirs) times). That holds where the sides clash, the driver's
  own merge. `git merge-file` runs first, and when the two points don't touch it merges cleanly
  and its result stands: the entry is then there twice. The base's entries never move or change.
- A run that repeats the base entries beside it fits several points. It goes where mu-190 put
  it: at a point the other side's run can be at too, the topmost such, else its own topmost. The
  result is the same bytes on every run.
- Markers and exit 1 stay for everything else: an edited or removed base entry, a changed
  preamble, an undated new entry. Two runs on one side are no longer on that list.
- The give-up message now reads "both sides changed more than new dated entries; left the conflict
  markers", with no point count to go stale.

The shaper made this call for mu-196 (the owner can overrule it).

## Amendment 2026-10-15: entries changed or removed on one side (task mu-209)

The rule above still took a side only when the base's entries were untouched. Main then removed
two stray `||||||| base` lines from older entries (ad8dacb), and mu-131's rebase onto it stopped
on `docs/WORKLOG.md` (8fb0ffe onto fa2ed9d) and was resolved by hand (2730c13). So a fix to an old
entry on main stopped every open branch's next rebase. The why of this decision covers it: the
two sides never touched the same entry. The merge becomes entry-wise:

- Each side is read against the base. A base entry is kept (the same bytes), changed (a side
  entry with the same whole `## ` heading line, in its place) or removed. Every other side entry
  is new: dated, in a run at a point among the base's entries, merged per point as above. A run
  beside a removed entry is where the first removed entry was. An edited heading reads as the
  old entry removed and a new entry in its place, and stays there: the run shares that point
  only with the other side's run when that one starts there or above, whose entries go first on
  the old entry's date or before. So what the other side put under the old entry stays under the
  new one, and two headings edited one on each side keep their order (review round 1).
- Per base entry: kept by both stays; an entry changed or removed on one side only is as that
  side has it; changed the same way on both, or removed on both, is there once or gone once.
- Markers and exit 1 stay for the rest: one entry changed differently on each side, or changed on
  one and removed on the other (an edited heading included); a new entry with the heading of a
  base entry either side changed or removed (an entry moved, or a copy of the changed one: which
  is which can't be told); an entry both sides removed with different new entries where it was
  (an edited heading against a removal or another edit reads so, and so does a removal beside
  an insert on one side: they can't be told apart, so both stop); a changed preamble on either
  side, as the lead's brief asked (it's the log's own header); a run of the other side's between
  two base entries one side removed or renamed while it put new entries there (which of them was
  above that run can't be told); an undated new entry; and a
  result whose entries don't split back where they were put (an entry's unclosed code fence
  would take in the headings after it).
- Duplicate base entries match deterministically, the same bytes on every run. The shaping said
  "from the top"; as built, entries that are in the base and the side once each are matched first
  (the longest run of them in order, as patience diff does), then the rest between them from the
  bottom, as mu-196 already read a side. A side with only inserts is still read as mu-196 read it,
  so its bytes don't change. The real log has one entry twice, and removing either copy merges.
- A gate test (`app/src-tauri/tests/conflict_markers.rs`) fails on a conflict-marker line
  (`<<<<<<< `, `||||||| `, `>>>>>>> `, or exactly `=======`) outside fenced code in `docs/**/*.md`
  or a top-level `*.md`, so markers a hand merge leaves can't land unseen. A gate test and not a
  `muster check` warning, since warnings fail nothing.

The shaper made this call for mu-209 (the owner can overrule it).

## Amendment 2026-10-15: a result that loses an untouched entry is refused (task mu-218)

Every merge so far keeps the entries neither side touched by how it's built, and nothing checked
it. A reported loss in mu-151's rebase turned out to be a false alarm (its base predated mu-210),
but the lead asked for insurance: a later change to the entry-wise merge, or `git merge-file`
lining an entry up with its twin (every entry ends in the same Verified line, the union finding
below), must not drop history unseen.

- An *untouched entry* is one byte-identical in base, ours and theirs. A result must have it
  whole, as one of its own entries, as many times as it's untouched; the line end the merge adds
  to an entry that ended the file without one is allowed. A preamble the same in all three must
  be the result's preamble.
- How many times is the fewest copies in base and in each side, where a side's copies are less
  the base copies only the other side changed. Each base entry is matched to each side as the
  entry-wise reading matches it, whatever the preamble or new entries; one matched to other bytes
  is changed. Only that lowers the count,
  and a side can only have changed as many copies as it no longer holds whole. A removal never
  lowers it: an entry another one was moved around reads as removed though its bytes are still on
  that side, and round 2 of the review found clean losses through exactly that (seeds 2633, 4034
  and 5139). So when the base has an entry twice and one side removes copy 2 while the other edits
  copy 1, the count is 0 and the edit merges clean, as merge-file merges it. When both sides remove
  a different copy, each kept one, so a result with none is refused. A side's own changed copy is
  already missing from its count, so it isn't taken off again: when one side edits copy 2 and the
  other keeps both, copy 1 is untouched and a result without it is refused (review round 3's R4,
  where round 3's count took the edit off twice and needed none).
- The check runs on both clean results. The entry-wise merge's result that fails it is refused
  like any other it can't make: merge-file's markers, exit 1. A clean `git merge-file` result that
  fails it goes to the entry-wise merge, checked the same way; when that can't merge either, the
  result is all of ours, base and theirs, whole, between merge-file's `<<<<<<< ours`,
  `||||||| base`, `=======` and `>>>>>>> theirs` lines, exit 1, so nothing is lost and git stops.
  stderr says the merge was refused because it would lose an entry neither side changed, and
  that the markers wrap all three files whole.
- The unit is the entry, not the line: the Verified and blank lines repeat across entries, so a
  per-line count could miss a dropped or moved line.
- An entry a side changed or removed isn't untouched, so a one-sided change or removal, the same
  change on both sides, an entry both added and an entry moved up by a removal above it merge as
  before. A clean result that loses an untouched entry is refused, and so, on purpose, are some
  correct merges of an entry the log holds more than once: where no copy reads as changed, the
  copies are counted by content. That happens when one side removes a copy and the other edits
  one, but the matching pairs the editing side's whole copy with the copy it edited, so the edit
  reads as a removal (seeds 3585 and 4786 of round 2's 6000 randomised merges). A refusal leaves
  all three files whole, so nothing is lost; telling those apart from a real loss would need the
  matching to know which copy each side meant, and it can't. Of round 2's flipped seeds, all 11
  real losses are refused and 10 of the 12 duplicate refusals now merge clean, as main merges
  them. R4's fix changes none of that: 6000 randomised merges give the same 13 seeds refused (the
  11 losses, 3585, 4786) and main's exit and bytes for the rest. The driver's contract (0 clean,
  1 markers, 2 can't merge) is unchanged.

The shaper made this call for mu-218 (the owner can overrule it).

## Options considered

- *`merge=union`* (git's built-in "keep both sides' lines"). It corrupts entries. We checked with
  `git merge-file -p --union` on two entries added at the top on each side, both ending in the same
  `` **Verified:** `gate: green (pnpm check)` `` line. Git matched the shared line, so the result
  was A's heading and Did, then B's heading and Did, then a single Verified line. The entries
  interleave and a line is lost, with no warning. Union's order is also ours, then theirs, so in a
  rebase the task landing now goes below the one that landed before it. Turned down.
- *One file per entry* (`docs/worklog/<date>-<slug>.md` plus an index). Entries never conflict, but
  the index does unless it's generated, and the log stops being one page to read. It also breaks
  the playground-wide convention (one `docs/WORKLOG.md` or `docs/BUILDLOG.md`, newest first) and
  the Stop hook that looks for that file (`~/playground/.claude/hooks/worklog-check.mjs`), in every
  repo. Turned down.
- *A merge driver* keeps the single file and the convention. It has two weak spots: drivers live
  in git config, which isn't committed, and the driver might not be able to run. Both are handled
  above. Chosen.

## Revisit when

Another repo with a running log (choopan's `docs/BUILDLOG.md`) opts in with one `.gitattributes`
line, or entries start needing an order other than their date (several a day that must stay in
landing order across more than two branches).
