# 0004 — One session per task, a thin coordinator that hands off early, sessions named by role

Date: 2026-10-13 · Status: accepted (the owner: "yes, do it"). Applies to how agents work
Muster's queue now, and to what 0009 (orchestration) and 0023 (parallel sessions) build.

## Why

Measured on 2026-10-13 from the two sessions' transcripts (local, `~/.claude-hg/projects/…`):

| | Turns | Tokens processed | Of which re-read from cache |
|---|---|---|---|
| coordinator main-29 | 754 | 347M | 345M |
| its 40 subagents | 1,599 | 214M | 210M |
| coordinator main-f2 | 387 | 175M | 174M |
| its 63 subagents | 3,587 | 679M | 664M |

Almost every token is the conversation re-read on each turn; output was under 1M in all. A
turn costs about the context's size, so a long-lived coordinator at ~450k pays ~450k for a
one-line "merged, pushed", and a fresh builder at ~40k pays a tenth of that per turn.

## Decision

1. **One session per task.** A task is built in its own fresh session (a subagent, a background
   session or a Drover lane), in its own worktree. That session claims it, checks in and passes
   it itself (stage times stay real), then ends. It never picks up a second task.
2. **The coordinator stays thin.** It reads the queue (`muster list --json`) and the builders'
   short reports, merges, and talks to other coordinators. It doesn't read source files or long
   outputs itself; subagents do and report the conclusion.
3. **Hand off early, not when full:** at about **150k** context the coordinator writes its state
   (into the queue: notes, check-ins; plus a short handoff note) and a fresh coordinator takes
   over. Builders that pass ~150k on one task hand off the same way (a check-in with what's done
   and next, then a fresh session resumes the claim).
4. **Sessions are named by role** (`claude -n` / `/rename`): `muster-lead`, `muster-build-0042`,
   so listings, messages and claim stamps say what each is.
5. **Several coordinators are the exception.** One coordinator with N one-task sessions is the
   default; a second coordinator only when the owner starts one, and then they split the queue by
   message and merge only into a clean main (as main-29 and main-f2 did).

## Options considered

- *One long session working the queue task by task:* simplest, but its context grows with every
  task, so each later turn costs more, and stage times blur.
- *Several long coordinators each taking tasks one by one:* works (2026-10-13), but doubles the
  coordinator overhead and needs messages to avoid collisions on main.
- *Chosen:* thin coordinator + one fresh session per task, handoff at ~150k.

## What changes

- 0009 builds the dispatcher this way (Drover runs the one-task sessions); 0023's worker loop is
  one task per session; PROTOCOL.md gets the handoff rule; the Work section (0025) shows a task's
  tokens so the effect is visible.
