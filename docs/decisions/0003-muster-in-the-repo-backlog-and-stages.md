# 0003 — Tasks live in the repo's `.muster/`, flow from a backlog through stages with roles

Date: 2026-10-12 · Status: accepted (owner chose location, stage names and levels; the rest is
capture's call, overrule any time). Replaces "Decided: a queue next to each project" in
`AS_BUILT.md`.

## Decision

- **Where:** one folder in the repo, `.muster/`:
  ```
  .muster/
    config.toml        per-project settings (committed or local, stage names, who works each stage)
    backlog/           raw notes, anything goes, no id
    tasks/             NNNN-slug.md, never moved or renamed
    attachments/NNNN/
    INDEX.md           generated master list, never edited by hand
  ```
  **Committed by default**, so a task's stage, ticked criteria and actuals land in the same
  commit and PR as the code. **`local = true`** in `config.toml` gitignores everything but
  `config.toml`, for queues that stay on this Mac. In that case the tool reads the main
  checkout's `.muster/` from any worktree.
- **Each piece of a task lives in one place.** Every worktree has its own copy of the task
  files, so two places holding the same fact would disagree and conflict on merge:

  | What | Lives in | Copies |
  |---|---|---|
  | What's happening now: stage, who holds it, lease, heartbeat, questions, answers, new comments, the lock | the repo's shared git folder, `$(git rev-parse --git-common-dir)/muster/` | one, seen by every worktree, never committed |
  | The write-up: spec, criteria, final actuals | `.muster/tasks/NNNN-slug.md` | one per branch, but only the branch that holds the claim edits it |
  | `INDEX.md`, `tasks.json` | `.muster/` | rebuilt by the tool, never merged |

  The tool reads a task as main's file, then the claimant's worktree copy if someone holds it
  (the claim names the worktree), then the live state on top. On Done it writes the stage,
  ticked criteria, comments and actuals into the claimant's file, so they reach main in that
  branch's PR. Backlog notes and new tasks take their id from the live state and are committed on
  main by the tool, touching only `.muster/` (never pushed). A local queue has one copy anyway:
  ignored files don't exist in worktrees.
- Worktrees from Claude Code, Drover lanes (`.claude/worktrees/<lane>`) and Orca
  (`<project>/worktrees/<repo>/<name>`) are all git worktrees of the same repo, so they share
  one live state.
- **Backlog → task:** anything in `backlog/` is a backlog item, a one-line `.md` included,
  however it got there (app, CLI, Claude Code, a file dropped by hand). Shaping turns it into a
  numbered task in `tasks/`. That's the only move a task ever makes.
- **Stages** (live state while in flight, written into the file at Done; only `muster` writes them):

  | Stage | Who | Meaning |
  |---|---|---|
  | Backlog | anyone | raw; lives in `backlog/` |
  | Shaping | agents: product + tech + UI | read code and docs, write criteria, type, parent, estimates |
  | Ready | — | shaped and estimated; claimable |
  | Building | agent: code | implement, gate |
  | Testing | agent: QA | verify each criterion |
  | Your check | the owner | try it by hand |
  | Done | — | shipped, actuals recorded |

  **Blocked is a flag** (`waiting_on`) on any stage, not a stage, so a task blocked in Testing
  stays in Testing.
- **Levels:** Goal → Feature → Task. A Goal is an outcome (it may have a date); a Feature groups
  tasks; Tasks are what agents build, and their criteria are the steps. A task names its parent.
- **A task has:** priority; type (code, bug, chore, tech-debt, spike, design, docs, research);
  tags; parent; points = estimates (agent time, test time, tokens) set at shaping, and actuals
  stamped by the tool (minutes from each stage's start and end, tokens summed from the
  transcripts of the sessions that held it); attachments; external links; questions; comments
  (`muster comment`, stamped with who and when); the log.

## Options considered

- **Location:** `.muster/` committed, or local, per project (chosen); always committed (no
  private queues); always gitignored (tasks never in a PR); `<project>/tasks` beside the repo with
  its own git (today's layout: tasks never in a PR, and the sandbox has to be widened for every
  new queue, `DOGFOOD.md` 41).
- **Folder names:** `.tasks/` for tasks plus `.muster/` for config (two dot-folders); one
  `.muster/` (chosen: one place to find, one thing to gitignore).
- **Claims in the committed files:** each worktree would have its own copy, so two worktrees
  could both claim a task, and claims would conflict on merge. Hence the shared git folder.
- **Stage names:** plain words (chosen); classic (Triaged, In progress, In QA, Human QA); Muster
  themed (Recruits … Dismissed).
- **Levels:** Goal → Feature → Task (chosen); Phase → Feature → Task; Milestone → Epic → Story →
  Task. Five levels is more than one person with agents needs.
- **Status folders:** they keep moving files and breaking held paths (`DOGFOOD.md` 1). Only the
  backlog → task move remains.

## Revisit when

The sandbox refuses writes to the common git folder from a worktree (fall back to a state folder
in the layer's allowWrite list), or a second person works the same queue from another machine
(claims would then need to travel, e.g. through a pushed ref).
