# Muster: what's adopted, being tried, and next

**Name:** Muster (owner, 2026-10-12), with slash commands `/muster` and `/mu`. See `COMMANDS.md`.

The owner's plan (2026-10-11): don't switch to chit as it is (half done). Take chit's ideas into
this queue one at a time, test them on real Drover work, record what happens (`DOGFOOD.md`), and
build the system on the side as we go. Later it becomes part of Drover (task 0005, task
creator per project).

Sources: chit `~/playground/chit/SPEC.md` (its implementation is a reference for the rules),
design rev 5 `~/playground/izzy/docs/agent-task-system.md`, the first draft
`~/playground/@download/tasky/`.

## Status of each idea

| # | Idea (chit source) | Here | Since | Result so far |
|---|---|---|---|---|
| 1 | One file per task, folder = status, atomic `mv` claim (§2, §6.1) | adopted | v1 | Works. Moving breaks held paths: see 2 |
| 2 | Find a task by id, never by path (CLI `find`) | adopted as a rule (`ls tasks/*/00NN-*`) | v2 | — |
| 3 | Roles with their own moves: PM, coder, human (§5) | adopted: owner, capture (PM + tech lead), coder | v2 | — |
| 4 | Questions go to the tech lead, never technical ones to the human (§6.4) | adopted: capture answers or escalates | v1 | Works when answers are seen; see 9 |
| 5 | Blocked with `waiting_on`, claim released (§6.4) | adopted (`blocked/`) | v2 | — |
| 6 | One claim per session, heartbeat, reclaim after 12 h (§6.2) | adopted as rules, capture reclaims | v2 | — |
| 7 | Inbox for raw reports, PM triages (§10a) | adopted (`inbox/`, date-time names) | v2 | — |
| 8 | Only the PM assigns ids (instead of chit's random ids) | adopted | v2 | Simpler while one capture session exists |
| 9 | Non-blocking ask with an answer channel any role may append to | **new, not in chit**; adopted | v1–v2 | Answers were missed twice (0007, 0008): needs a push |
| 10 | Acceptance criteria with `verify:` kinds (§7) | adopted for new tasks | v2 | — |
| 11 | Git history for the tasks repo, CLI commits (§12) | adopted: capture commits snapshots | v2 | — |
| 12 | Answer push: the asker is told when an answer lands | **live**: `bin/task` + `bin/answer-hook.py`, hook installed in drover/main, agent-controls, cleanup, docs-wt (2026-10-12, owner's yes) | v2.1 | Checked against the Claude Code docs: `additionalContext` on PostToolUse / UserPromptSubmit / SessionStart; `CLAUDE_CODE_SESSION_ID` is documented. Limit: the id changes on `/clear` and on a handoff, so answers to older questions wait in the old mailbox (`task inbox --session <old id>`) |
| 13 | A CLI that enforces the rules (`chit`) | later: rules are by hand for now | — | Rules by hand rely on agents reading the README |
| 14 | Leases held by a runner process (`chit run`) | later | — | — |
| 15 | Plan checkpoint, reviewer role, proof kinds (§5.3, §7.2) | later | — | — |
| 18 | A manifest of all tasks (`chit list --json`), generated, plus a queue check | adopted (owner asked): `bin/manifest.py` → `tasks.json` | v2 | Generated, never edited, so it avoids Taskmaster's single-shared-file race. The check catches id clashes (30) and stale claims |
| 17 | Attachments per task, `attachments/<id>/` (§2, `--attach`) | adopted (owner asked) | v2 | Screenshots for 0003 and 0008 saved. Chat paste files are temporary, so they're saved at capture |
| 16 | Dependencies and `touches` overlap checks (§6.3) | partly: `depends on` in Details | v2 | — |

## Next to try

- **Answer push (12).** The cheapest version: whoever answers also writes a one-line note to
  `inbox/answers/<task>-<q>.md`, and coders check it at every step. The real version: Drover
  types the answer into the asker's session between turns (Drover already does this for item
  questions, `mcp.rs` `ask_question`).
- **A tiny CLI (13)** for the risky moves only: `claim`, `ask`, `answer`, `block`, `done`. It
  takes chit's code where it fits, so rules don't depend on agents reading the README.
- **Watcher outside the capture session** (`DOGFOOD.md` 10): Drover's monitor watches the
  queue.

## Ideas for later (owner, not for now)

- **The task session as dispatcher** (owner, 2026-10-11: "it would be a good idea if you or the
  task session is in charge of which agent works on what and how they communicate with each
  other and inject stuff… just as an idea, not for this session"). Today coders pick their own
  tasks, and messages go through files. The idea is that the capture/task session runs the
  floor:
  - **assigns** work: which agent (and account, model, worktree) takes which task, instead of
    agents self-picking. It balances load and keeps overlapping files apart (chit's `touches`).
  - **routes communication:** agents talk to each other through it (questions, answers,
    hand-offs, "I changed X, you depend on it"), and it delivers each message to the right
    session at the right time.
  - **injects:** it puts context, answers, rule changes and new priorities into running
    sessions (between turns), which fixes the answer push (12) and rule changes not reaching
    running sessions (`DOGFOOD.md` 24).
  - In Drover: the mechanics already exist in pieces. Drover starts agents (pins, routing,
    decision 0007), types messages into a session between turns (`term_send`, item questions),
    injects context through hooks (0011), and plans parallel pieces (decision 0009). The task
    session would be the brain that drives them. Belongs with 0005 and 0011.

## How it becomes Drover

Task 0005 (task creator per project) designs it into Drover: the capture session as a standing
project session, the queue as Drover items or a local tracker option, questions on Drover's
tracked questions list, answers pushed into the asking agent. 0005 reads this file and
`DOGFOOD.md` first.
