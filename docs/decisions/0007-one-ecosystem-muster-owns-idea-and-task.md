# 0007 — One ecosystem: Muster owns idea and task, Drover the code, the review engine the review

Date: 2026-10-13 · Status: accepted (the owner). Replaces decision 0006's part assignment
("planning lives in GenIdea"); 0006's other three choices stand.

The owner: "we should own all: idea and task are in muster, code is the drover, review is review
engine and gates and deploy we build afterward. all in one ecosystem".

## Decision

| Step | Owned by | Notes |
|---|---|---|
| Idea | **Muster** | the planning room (mu-41) and, over time, GenIdea's interview, decision ledger, probe and idea board, ported into Muster |
| Task | **Muster** | as today |
| Code | **Drover** | sessions, lanes, worktrees, accounts (Drover runs agents, decision 0001 / Drover 0018) |
| Review | **the review engine** (`~/playground/.claude/reviews`) | in Testing (decision 0005); on GitHub as its own app (mu-55) |
| Gate | to build | the shared gate kit (0006 choice 3) |
| Deploy | to build | release: build, sign, publish, per app |

*Amended by [0015](0015-muster-runs-agents-behind-a-runner-interface.md) (2026-10-14):* Muster runs agents until Drover's runner exists.

- **One ecosystem, our own:** every step is a tool the owner owns; they talk through commands and
  files (0006), share one record of outcomes (mu-54), and use Muster's stage names (0006).
- **GenIdea** (`~/playground/germ`, built, 9 phases) becomes the source the idea step is ported
  from, not a separate home: its sealed-ledger model and probe gate are the idea step's core.
  Until the port, GenIdea still works on its own, and `muster import germ` can bring a sealed
  ledger in.

## Options considered

- *GenIdea as its own idea app* (0006): one more app and one more place, against the owner's
  "fewer places" principle; overruled by the owner.
- *Drover owning idea and task too* (the long-run plan says Muster moves into Drover): still the
  direction for the screens (VISION.md); the ownership of the steps' rules stays in Muster's core.

## What changes

- mu-41 (planning room) is reopened in Muster; GenIdea's queue (gi-1) is removed.
- New task: bring GenIdea's interview, ledger and probe into Muster's idea step.
- PIPELINE.md and VISION.md say who owns each step.
