# Idea → Task → Run → Gate → Done: the four parts

**Status: accepted (2026-10-13).** The owner said yes to all four choices: decision
`docs/decisions/0006-four-parts-idea-task-run-gate.md`. **Then changed who owns idea (decision
0007): idea and task are both Muster's; GenIdea is ported into Muster, not a separate part.** Read
"GenIdea" below as "Muster's idea step (ported from GenIdea)".

The owner, 2026-10-13: "in general we need agent orchestration, or think of idea -> task ->
execution -> gates -> done. Execution is Drover, task is Muster, idea is the third part and gates
(QA, visual QA) is the fourth."

## The parts

Each part owns one thing. Parts talk only through commands and files the owning part defines,
the rule GenIdea's plan already had ("germ talks to chit only through chit's command line").
None writes another part's files.

| Part | Product | Owns | Takes in | Hands on | Today |
|---|---|---|---|---|---|
| **Idea** | GenIdea (`~/playground/germ`) | the *decisions*: what we're building and why | a one-liner | a **sealed ledger**: every decision made, checked by a fresh agent (the probe) that finds nothing left to invent; `handoff.md` | built: interview, ledger, probe, seal, reopen; idea board (directions → pieces → workspaces) experimental. No hand-off out yet (it was planned to chit) |
| **Task** | Muster | the *work and its rules*: tasks, criteria, stages, claims, questions, the owner's attention, cost | a sealed idea, or a one-liner for small things | tasks in Ready, assignments; state through `muster list --json` | built: stages, claims with leases, check-ins, questions pushed to sessions, vision check, cost, board, manual |
| **Run** | Drover | the *sessions*: lanes, worktrees, accounts, models, steering | an assignment (Drover 0018) | an agent working the task, which claims, checks in and passes with `muster` | built: lanes and worktrees, gates as commands, Agents view (Needs you / Working / Waiting / Idle), steering, plan pieces. Not built: reading Muster's assignments (0018), Muster's stages (0023) |
| **Gate** | shared tools in the personal layer | the *proof*: that the work does what the task said | a branch and the task's criteria | a gate result line with its stamp; a review verdict; one verdict per criterion with its evidence (test, screenshot, diff) | built separately: `/gate` (result line + tree-stamped `last.json`), `/deep-review`, the `/pr` engine (no runs yet), braindump's `ac-verifier` and `visual.mjs`, choopan's `pixelDiff`, MobileBuildMCP screenshots. Not joined up. **Joined up by `docs/GATES.md` (decision 0014, feature mu-129)** |

**Done** isn't a part: it's the owner accepting in Muster's Your check, and the result flowing
back (the idea shows "built", its decisions marked with the tasks that built them).

## The hand-offs (the arrows)

1. **Idea → Task:** on seal, GenIdea runs `muster import germ <idea folder>`. Muster owns the
   format (as chit's plan did). The idea becomes a feature with the ledger and hand-off attached;
   shaping splits it into tasks, and each task names the decisions it builds (`D-012`). GenIdea
   asks how it's going with `muster show <feature> --json`.
2. **Reopen → cost of changing course:** a reopened and resealed idea re-imports; Muster's vision
   check marks the tasks built on superseded decisions and shows what changing course costs.
3. **Task → Run:** Muster writes an assignment; Drover starts the lane, the agent claims with
   `muster claim`. Drover shows Muster's stage and criteria for that lane instead of its own item
   steps.
4. **Run → Gate → Task:** leaving Building needs the gate line *and* a gate stamp whose tree matches
   the branch (proof checked, not just pasted). Leaving Testing needs the per-criterion table from a
   session that isn't the builder, and the review verdict (0027 wraps `/pr`).
5. **Task → Owner:** Your check says what to try, or "nothing to try: Accept".

## The research, taken in without losing the idea

What stays Muster's own: stage moves need proof, the owner's attention is protected, one picture
per project, local with no telemetry, plain files with git
(`docs/research/2026-10-13-agent-task-tools.md`). Each finding goes to the part it belongs to:

| Finding (source tool) | Goes to | How |
|---|---|---|
| The tester isn't the builder (GitHub, Factory) | Gate + Task | Testing → Your check refused when the ticking session built it |
| "What's true now" outlives the task (OpenSpec) | **Idea** | the sealed ledger *is* the living spec; no new spec format in Muster |
| A clarify pass before planning (Spec Kit, Factory) | **Idea** | GenIdea's interview is that pass; Muster's one-pass shaping stays for small things |
| Read the agent tool's own session states (Claude Code agent view) | Run | Drover already derives Needs you / Working; Muster reads it from Drover, or `claude agents --json` without Drover |
| Stop and pick-one questions (Linear) | Run + Task | stop and steer are Drover's; questions with options are Muster's mailbox |
| A work-in-progress limit (Willison, Osmani) | Task | warn when too much waits in Your check |
| Agent teams (Claude Code) | Idea and Gate | roles that argue (planning, review lenses), not parallel building (one session per worktree, Run) |
| An MCP server, imports from other tools | Task, later | for selling |

## What the owner decides

1. **Where planning lives.** GenIdea's idea board already has directions, pieces, research and a
   devil's advocate, so the planning room (0041) overlaps it. *Pick: planning lives in GenIdea;
   the board grows into the room with roles as an agent team; Muster keeps one-pass shaping for
   bugs and small notes, and 0041 moves to GenIdea.* Without an answer: 0041 stays parked behind
   the teams spike (0040).
2. **Drover's item steps vs Muster's stages.** *Pick: a lane running a Muster task shows Muster's
   stages and criteria; Drover's own items stay for quick asks with no task.* Without an answer:
   Drover 0023 does only the waiting list, as written.
3. **Where gates live.** *Pick: one gate contract in the personal layer: `/gate` (as is), an
   `ac-verifier` agent promoted from braindump, and a `/visual` skill from braindump's `visual.mjs`
   and choopan's `pixelDiff`; Drover runs them, Muster reads their results.* Without an answer:
   each repo keeps its own.
4. **One set of words** across the four (GenIdea's idea states, Muster's stages, Drover's steps).
   *Pick: Muster's stage names everywhere a task shows.*

## Learning, both ways (proposed 2026-10-13)

The owner: "so the reviewer can be part of the whole ecosystem, integrated learning, 2 way
learning also: idea -> tasks -> code -> review -> gates -> deploy". Read as six steps over the
four parts: **idea** (GenIdea) → **task** (Muster) → **code** (Drover's sessions) → **review**
and **gate** (the shared proof tools) → **deploy** (a release: build, sign, publish, per app).

- **Forward, what was learned rides along:** every step's output carries the lessons that apply
  to it. A recurring review finding becomes a lesson (owner's yes, decision 0005); lessons for a
  task's area go into the builder's brief (`muster start`); lessons about *decisions* ("we keep
  forgetting offline behaviour") go back to GenIdea, so its interview asks about them up front.
- **Backward, each step scores the one before:**
  - review → code: findings the builder fixed (real) or won a dispute on (false alarm);
  - gate → review: a gate failure after a clean review is something the review missed;
  - deploy → everything: a bug found after release is traced to the step that should have
    caught it (idea, task criteria, review lens, gate), and each gets the miss;
  - task → idea: a task that needed a decision the sealed idea didn't make is a gap in the
    interview (GenIdea's probe gets it).
- **One record:** every outcome is a row with the idea id, task key, branch, lens, step and
  verdict, written by the step that saw it, read by all. The engine's ledger
  (`~/playground/.claude/reviews/`) and Muster's reviews and lessons are its first two sources;
  the importer the review builder designed (`import-muster.mjs`, rows with `task:` instead of a PR
  number) is the first bridge. Owner's rule kept: nothing becomes an instruction to agents
  without the owner's yes, in batches.
- **Not now:** labels into `/pr learn` and Muster in the engine's repos wait (decision 0005,
  confirmed); this section is the target they grow into.

## The reviewer on GitHub (proposed 2026-10-13)

The owner: "added to github with an app to be a reviewer like coderabbit". The engine already
serves every repo on this Mac (`/pr`, `/deep-review`); what's missing is its own identity on PRs.
Shape: a **GitHub App under sweetlemonai** (e.g. "sweetlemon-review"), private key in the
Keychain, posting as `sweetlemon-review[bot]`; **no server and no third party**: this Mac polls
the listed repos for new or updated PRs and reviews them locally (reviews happen while the Mac is
on); it never approves or merges, reviews only listed repos, never uses iMassih. Creating the app
is outward-facing: it waits for the owner's go.

## Notes

- GenIdea's researcher runs `claude --tools "WebSearch,WebFetch" --dangerously-skip-permissions`
  (germ `docs/specs/researcher-agent.md`). The tools are limited to search and fetch, but the flag
  skips every permission prompt; worth checking when the planning room is built there.
- GenIdea's spec docs sit outside its repo (`germ/.stuff/docs/`), and its STATUS.md points at the
  wrong path.
