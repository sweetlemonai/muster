# 0009 — The work log merges through a muster merge driver

Date: 2026-10-14 · Status: accepted (the shaper decided it for task mu-67; the owner can overrule
it any time).

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
