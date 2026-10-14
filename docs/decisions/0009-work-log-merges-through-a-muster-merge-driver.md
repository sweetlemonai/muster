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
  drops a line or reorders the lines inside an entry.
- **Git config isn't committed, so muster writes it.** `muster start` and `muster init` set
  `merge.muster-log.name` and `merge.muster-log.driver` (`muster merge-log %O %A %B`) in the
  repo's own `.git/config`, which every worktree reads. They only set a key git doesn't have yet,
  and leave every other key alone. That way a hand `git rebase main` in a worktree resolves the
  log the same way.
- **Muster's own merge never depends on that config.** Its rebase runs as
  `git -c merge.muster-log.name=… -c merge.muster-log.driver='<this muster binary>' merge-log %O %A %B rebase main`.
  The path is quoted for the shell git runs the driver in.
- **If the driver can't run** (muster isn't on the PATH, or the config is missing in a hand merge),
  git treats the log as a conflict, or falls back to its own text merge. That's how it worked
  before this change, and it never gives a wrong merge.

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
