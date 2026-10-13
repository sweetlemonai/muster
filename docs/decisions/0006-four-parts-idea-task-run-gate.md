# 0006 — Four parts: idea → task → run → gate, each owning one thing

Date: 2026-10-13 · Status: accepted (the owner: "yes to all" on `docs/PIPELINE.md`'s four
choices). PIPELINE.md is the picture; this records the choices.

## Decision

The pipeline has four parts, each owning one thing, talking only through commands and files the
owning part defines (none writes another's files):

| Part | Product | Owns |
|---|---|---|
| Idea | GenIdea (`~/playground/germ`) | the decisions: interview, ledger, probe, seal |
| Task | Muster | the work and its rules: tasks, stages, claims, questions, the owner's attention, cost |
| Run | Drover | the sessions: lanes, worktrees, accounts, steering |
| Gate | shared tools in the personal layer | the proof: gate line, review verdict, one verdict per criterion |

The owner's four choices:

1. **Planning lives in GenIdea.** Its idea board grows into the planning room (roles that argue,
   research, mockups, rounds with the owner), with roles as a Claude Code agent team (0040 settles
   the mechanics). Muster keeps one-pass shaping for bugs and small notes. Task 0041 moves to
   GenIdea's queue.
2. **A Drover lane running a Muster task shows Muster's stages and criteria;** Drover's own item
   steps stay for quick asks with no task (Drover 0023).
3. **One gate kit in the personal layer:** `/gate` as is, an `ac-verifier` agent (from
   braindump), a `/visual` skill (braindump's `visual.mjs` + choopan's `pixelDiff`). Drover runs
   them, Muster reads their results.
4. **Muster's stage names everywhere a task shows** (GenIdea, Drover).

## Options considered

Each choice's alternative is in PIPELINE.md "What the owner decides" (planning in Muster;
Drover's own steps; each repo its own gates; differing names). The owner took every pick.

## What changes

- Hand-offs as PIPELINE.md lists them: `muster import germ <folder>` on seal, assignments to
  Drover (Drover 0018), proof checked at Building and Testing.
- Tasks: 0041 moves to GenIdea; 0009 (orchestration) and 0040 (agent teams spike) are no longer
  waiting on this; new tasks cover the import, the gate kit and Drover's lane view.
