# Muster — the one picture

What Muster is, how a task moves end to end, and which task builds which piece. Every task spec
points here; a task that contradicts this page either changes this page (with a decision record)
or changes itself. Decisions: `docs/decisions/`. Screens: `docs/DESIGN.md`. Rules for sessions:
`docs/PROTOCOL.md`.

## What it is

The owner's task system for AI agents: the owner says what they want in a line, agents shape it,
build it in parallel, review and test it, and the owner only checks results and answers what only
they can answer. It's a Tauri app plus the `muster` command (one binary, one set of rules), and
it moves into Drover later (Drover runs agents, Muster decides who does what, 0009 / Drover 0018).

Principles: no babysitting (ask the owner only what's theirs, in batches); everything visible but
out of the way (ADHD, `DESIGN.md`); local first, nothing sent anywhere but the model; tokens are
budgeted; only `muster` writes state, agents never edit status, claims or times by hand.

## Where things live (decision 0003)

- `.muster/` in each repo: `config.toml`, `backlog/`, `tasks/NNNN-slug.md` (never moved),
  `attachments/`, `INDEX.md`, `lessons.md` (0027). Committed by default, or local.
- Live state (stage, claim, lease, blocked, questions, comments, lock) in the repo's shared git
  folder, one copy for every worktree. The write-up (spec, criteria) on the claimant's branch.
- Today's `<project>/tasks` queues with status folders are the old layout until 0005 migrates them.

## The spine: one task, start to finish

| Stage | Who (role) | What happens | Built by |
|---|---|---|---|
| Backlog | anyone | a one-liner from the app, CLI, a session, a file | 0015 |
| Shaping | product + tech + UI agents | criteria, type, parent (0016), estimates (0017), `depends:` (0019), `touches:` (0023), `for:` (0022) | 0015 |
| Ready | — | claimable when its `depends:` are done; `muster ready --parallel` picks a safe set | 0019, 0023 |
| Building | code agent (or the owner, for `for: human`) | `muster start`: claim (0002) + branch + worktree (0023); lessons for its `touches:` in the brief (0027); gate | 0002, 0023, 0027 |
| Testing | review engine + QA agent | `pass` opens the PR; the engine reviews it, the builder fixes or disputes (up to 3 rounds); QA verifies each criterion; then merge, clean up the worktree | 0023, 0027 |
| Your check | the owner | try it on main; tick the owner-only criteria (0028); Accept | 0014, 0028 |
| Done | — | actuals stamped (0017, 0025) | 0017 |

**Blocked** is a flag on any stage with a kind (0024): by tasks (derived from `depends:`, never
typed twice), on you, by a permission, by a question, by something outside.

**What needs the owner** is one list everywhere (Human board 0022, the Dock badge 0006, counts):
Your check, escalated questions, blocked on you or by a permission, human tasks, lessons to approve.

## The app (one place per job)

Board (AI and Human, both on screen, 0022), Table, Timeline, Graph (0019) are views of the same
tasks. The task panel is docked beside them (0021 header, 0020 edit and comments, 0025 sessions
and cost, 0024 why blocked). Appearance holds themes, accents and card looks (0026).

## Open questions (owner's to settle; the spine above shows capture's pick)

- Parallel sessions: workers pull their own tasks now, dispatcher (0009) later; merge through
  GitHub's main; merge after Testing; 3 at once.
- Merging after Testing means Done comes after the merge, so decision 0003's "Done rides in the
  PR" changes to: stage and actuals are stamped into the file at merge; Done is a tool commit on
  main that touches only `.muster/`.
- Human tasks use the same stages with the owner in Building (skipping Testing and Your check),
  rather than a separate To do / Doing / Done.
- The PR engine (0027) is the review of record; builders run the gate, not a second full review.
- Review: who posts (owner's account or a bot), where it runs, own stage or part of Testing (0027).
