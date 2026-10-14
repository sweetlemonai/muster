# 0008 — Agent teams: where Muster uses them, and who a teammate is

Date: 2026-10-14 · Status: accepted (the owner, 2026-10-14). Proposed by Gholi from the mu-40 spike.

## What the spike found

Agent teams are already on for every session on this Mac (`CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`
in the environment), so the spike used the teams that were running instead of a new paid run:
this session's (team `session-56779e18`: worker-0033, two reviewers, qa-0033, worker-0060) and
muster-lead's (`session-531266d4`). Evidence for each answer is in mu-40's log.

1. **Session id.** A teammate has its own transcript (`<lead session>/subagents/agent-<id>.jsonl`)
   but carries the lead's `sessionId`, and its shell sees the lead's `CLAUDE_CODE_SESSION_ID`. Proof:
   mu-49 was built by muster-lead's teammate and tested by muster-lead under another label, both
   stamped `session 531266d4`. Muster keys stamps, check-ins, the answer mailbox, cost (0025, 0030)
   and the tester rule (0047) on the session id alone, so **it can't tell teammates apart**. That is
   why mu-47's criterion failed in QA.
2. **Folder.** Teammates start in the lead's cwd (`cwd` in every transcript). They work in a Muster
   worktree only by using absolute paths, as told; nothing gives a teammate its own worktree
   (the Agent tool's `isolation: "worktree"` makes a subagent, not a teammate).
3. **muster CLI.** Teammates run `muster` through Bash with their own `--as` label (mu-49's log), but
   the session part of the stamp is the lead's, and an answer to a teammate's question goes to the
   lead's mailbox: the hook delivers it at the next tool call of whichever agent in that session
   fires first. In this setup teammates also can't commit (guard hook) and the lead posts for them.
4. **Task tools.** Not on (`CLAUDE_CODE_ENABLE_TODO_TOOLS` unset; no TaskCreate/TaskList in the
   lead's tools), so teams coordinate by messages only and `TaskCreated`/`TaskCompleted` never fire.
5. **Hooks.** Hook input carries `agent_id` (and `agent_type`) for a teammate's tool calls; that is
   the handle Muster lacks in (1) and (3). `TeammateIdle` exit 2 keeps a teammate working (no matcher;
   fires for every teammate), so a hook could send a teammate back while its Muster task is still in
   Building with no gate line. `TaskCompleted` needs the Task tools (4).
6. **Cost.** From the 0033 transcripts: the worker made 720 calls and read 299M cached tokens (2.5M
   written, 18k output); each reviewer 6–9M cached reads; the lead 43M. Every teammate ran on the
   lead's model with its 1M context (`model: opus[1m]` in the team config), and the cached re-reads of
   a long context are most of the cost. A 3-agent review plus QA cost about a twentieth of the build.

## Decision

- **Who a teammate is:** Muster's stamp becomes `session + agent`: `muster` reads the agent id when
  given one (`--agent`, or a hook-provided value), the answer hook routes mail by `session/agent`
  using the hook input's `agent_id`, and the tester rule (0047) compares the pair, so a teammate of
  the builder's lead counts as a different tester only if its agent id differs and it didn't build.
  Until then, QA for a task runs in a separate session (not a teammate of the builder's lead).
- **Where teams are used:** the planning room (mu-41: product, research, UI, tech, skeptic roles
  arguing in rounds) and review lenses in Testing (0027). Not parallel building: that stays one
  worktree per task (0023), each built by one worker.
- **Cost rules:** spawn workers and reviewers on a named smaller model unless the task needs the
  lead's (reviews on Sonnet, mechanical fixes on Haiku); keep briefs short so contexts stay short;
  one combined review-and-QA agent per task unless it's security-related.
- **Hooks Muster installs:** later, with the agent id (above): a `TeammateIdle` hook that keeps a
  teammate working while its claimed task is in Building without a gate line. No Task-tool hooks
  until the Task tools are on.

## Options considered

- *Teams for parallel building:* teammates share the lead's folder and session id, and same-file
  edits overwrite each other; worktrees per task with one worker each already work. Turned down.
- *Turn teams off and use plain subagents:* loses teammate messaging, which the coordinator uses to
  steer workers mid-task. Turned down; the identity fix above is cheaper.
- *A separate Claude session per role (agent view, `claude --bg`):* real separate session ids, but
  heavier to start and steer. Kept for QA until the agent id lands.

## Revisit when

Teams leave experimental, the Task tools are on by default, or teammates get their own session id
or worktree.
