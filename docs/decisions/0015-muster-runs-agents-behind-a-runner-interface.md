# 0015 — Muster runs agents behind a runner interface: its own `claude -p` runner now, Drover's later

Date: 2026-10-14 · Status: accepted, decided by the coordinator for the owner, 2026-10-14,
overrule any time (Muster_Team_Lead on mu-108, from item 0.2 of `docs/plans/orchestration-plan.md`).
Amends 0007 (Drover still ends up running agents; Muster runs them until Drover's runner exists)
and 0005 (Muster still runs no agent for the review). Builds on 0002 (the owner's own `claude`,
scrubbed) and 0013 rules 1, 2 and 6 (never a login token, never another account past a
limit, telemetry off).

## Why

The owner's orchestration brief (the main checkout's
`.muster/proposal/Muster orchestration design brief.md`, 2026-10-14) has Muster launch
`claude -p` itself in build step 5 ("Claude Code runner, one role (Kaveh), one slot"), and
in step 11 "Drover becomes one more adapter later (decision 0007),
taking over launching agents". Decision 0007 and mu-9's AC1 (the owner, 2026-10-12) say Drover runs
the agents and Muster plans and assigns. Drover's runner (Drover 0018) isn't built, so the two
can't both hold today. The plan asks for this decision before the runner (mu-117) is built.

## Decision

1. **One runner interface in Muster's core.** The dispatcher talks only to it, so adding a
   provider or Drover never changes the dispatcher. Its six methods (the brief's "Runner
   adapters"; built in mu-117):
   - `start`(role profile, task, worktree, account) → a run handle.
   - `status`(run) → the run's state (below), plus live usage when the provider reports it.
   - `stop`(run) → ends the run; it ends `cancelled`.
   - `usage`(run) → the ledger record's fields (tokens in, cached, out; cost; turns; model).
   - `limit`(run) → whether the run ended on a limit, and the limit's `resets_at`.
   - `capabilities` → what the provider supports (hooks for answer push, ping and pause; budget
     and turn caps; streaming usage). A provider without hooks gets only work that needs no
     mid-run questions, or its runs check `muster inbox` between steps.
2. **Five run states,** the vocabulary of MCP's Tasks extension (research §7), so Muster doesn't
   invent its own:
   - `working`: running.
   - `input_required`: waiting on an answer or a permission (a denied tool is named).
   - `completed`: ended with a result. Terminal.
   - `failed`: crashed, exited without a result, or hit a turn, budget or rate limit (`limit`
     says which). Terminal. A rate limit cools that account down until its `resets_at` and the
     work waits; it's never requeued on another account (0013 rule 2).
   - `cancelled`: stopped by `stop` or a signal. Terminal.
3. **Muster's own runner now.** Muster ships a local Claude Code runner (`claude -p
   --output-format stream-json`, mu-117) that implements the interface. It starts the owner's own,
   unmodified `claude`, signed in by the owner, with 0002's environment scrub (`STRIP_ENV` in
   `helper.rs`: `ANTHROPIC_BASE_URL`, `ANTHROPIC_CUSTOM_HEADERS`, `ANTHROPIC_API_KEY`,
   `ANTHROPIC_AUTH_TOKEN`, `CLAUDE_CODE_OAUTH_TOKEN` and an inherited `CLAUDE_CONFIG_DIR` removed),
   then `CLAUDE_CONFIG_DIR` set for the run's account (only the allowlisted personal accounts,
   0012). It never reads, stores or passes a token of its own (0013 rule 1).
4. *Amended by [0016](0016-runs-headless-in-a-sandbox-drover-hosts-sessions.md) (2026-10-14): Drover builds no runner; it hosts the runs (watch, open live, hand back), and this runner stays the only one.* **Drover's runner later.** Drover's runner (Drover 0018, Muster's side in mu-127) implements
   the same interface and replaces Muster's; Muster's stays as the fallback. mu-117's fake-runner
   suite is the contract Drover's runner must pass.
5. **The Codex adapter turns analytics off.** Codex 0.162.0 enabled analytics by default for
   daemon-launched app servers (research §3). The Codex adapter (mu-126) starts the app server with
   analytics off, per the owner's no-telemetry rule (0013 rule 6). No runner exports telemetry
   (never `CLAUDE_CODE_ENABLE_TELEMETRY`): the ledger stays on the Mac.
6. **What Muster still doesn't run.** The review of record (0005) and the vision check stay with
   a session the owner or the coordinator starts: `muster review` and `muster vision` print a brief
   and run no agent. Only the dispatcher's runs go through the runner.

## Options considered

- *Drover only: wait for Drover 0018.* Keeps 0007 as written and builds one runner, not two. But
  orchestration waits on another project's roadmap, with no date. Rejected.
- *Muster only, forever.* One runner, no interface to keep stable. But it splits the ecosystem
  0007 set up (code runs in Drover) and leaves Drover's sessions, lanes and worktrees unused for
  the dispatcher's runs. Rejected.
- *Muster's runner now behind an interface, Drover's later.* Orchestration starts now; the cost is
  an interface to keep stable and a second implementation to write when Drover's lands, with
  mu-117's tests as its contract. Chosen.

## What would make it worth revisiting

- Drover's runner landing (Drover 0018): then Drover's becomes the default and this record says
  so.
- A runner need the six methods or five states can't express (a provider that pauses and resumes
  mid-run, or one run serving several tasks).

## What changes

- 0007 and 0005 each get a line saying they're amended by this record.
- `docs/PROTOCOL.md`'s review-of-record and vision-check notes say "Muster runs no agent for the
  review or the vision check; the dispatcher's runs are decision 0015's". `docs/AS_BUILT.md` says
  the same for `muster review`, and lists this record.
- `docs/VISION.md`: Drover runs agents, "Muster's own runner until then, decision 0015".
- mu-9's AC1 gets a comment pointing here.
