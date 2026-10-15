# Orchestration plan (mu-9)

2026-10-14 · by Pouya (Researcher) · status: proposed, for Shapour to turn into tasks under mu-9.
Sources: the owner's brief (`tasks/proposals/Muster orchestration design brief.md`), the research in
`docs/research/2026-10-14-orchestration.md` (cited below as [R§n]), decisions 0002, 0004, 0007, 0008,
0011.

## What changes from the brief

| Brief | Plan | Why |
|---|---|---|
| Dispatcher is code; LLM lead only for judgment; state in the queue; decision log; dry run | **Kept** | Vendor guidance and OpenAI's Symphony do the same [R§4][R§5] |
| Capacity is learned from limit hits | **Read, then learned per run.** Claude Code reports each account's 5-hour and weekly use and reset time (`rate_limit_event`, status-line `rate_limits`); Codex's app server does too. The ledger learns what a run costs as a share of the window | [R§2][R§3] |
| Load balancing: spend the account that resets soonest, across several accounts | **Dropped.** One account per dispatcher by default. A second account the owner already holds can be pinned by hand to a project; no automatic rotation, ever. More capacity comes from a bigger plan, a Team plan or an API key | Terms risk [R§1] |
| Calibration: one actual ÷ estimate ratio per role, type and size | **Distributions**: keep the ratios, plan with p50 and fit-check with p80 | Up to 30× spread on the same task [R§6] |
| Spikes 1–3 (read usage, limit-hit shape, sessions sharing a window) | **Mostly answered by docs.** One short spike confirms them on a real run | [R§2] |
| Kia's reviewers as subagents | **Kept, and required:** agent teams don't start under `claude -p` | [R§2] |
| Stuck run: "past a role's limit" | **Concrete numbers**: no stream event for 300 s → ping; 600 s → stop and reclaim. Backoff `min(10 s × 2^(n−1), 5 min)` | Symphony's defaults [R§4] |
| Runner adapters; Drover later | **Kept, with a decision first**: decision 0007 says Drover runs agents. Muster's own `claude -p` runner is the first adapter and Drover's replaces it | 0007, mu-9 AC1 |
| Ledger fields | **OpenTelemetry GenAI names**, stored locally, version pinned, nothing exported | [R§7] |
| Run states | **MCP Tasks vocabulary**: `working`, `input_required`, `completed`, `failed`, `cancelled` | [R§7] |
| Roles | **Kept, plus Pouya (Researcher)** | Owner, via the lead, 2026-10-14 |

## Terms rules the build must keep (decision 0013, item 0.1)

1. Muster starts the owner's own, unmodified `claude` and `codex`, signed in by the owner. It never
   reads, stores or passes on login tokens, and never calls a provider's API with a subscription
   token (0002 already does this) [R§1].
2. No automatic choice among several accounts of one provider to get past limits. A limit hit means
   cooldown on that account until `resets_at`, not a switch [R§1].
3. The company account is never used (mu-99, decision 0012), and Muster never reads it.
4. Every run is capped (turns and budget) and paced (owner's reserve, weekly pacing), so use stays
   close to one person's.
5. Sellable shape: each user brings their own login or API key; Muster has no pooling feature to
   switch on.
6. Telemetry stays off: no `CLAUDE_CODE_ENABLE_TELEMETRY` export; the Codex adapter turns off the app
   server's analytics, which 0.162.0 switched on by default [R§3].

## Roles

The brief's table stands (Shapour, Arash, Dariush, Kaveh, Kia, Sohrab, Parviz, Azadeh, Mehran,
Leila), with slots numbered per role (Kaveh-1, Kaveh-2). One role is added:

| Role | Name | Stage | Runs as |
|---|---|---|---|
| Researcher | **Pouya** ("seeker"; the lead's pick, the owner can overrule) | Shaping, and on demand for the planning room (mu-41) | its own short session that fans out `web-researcher` subagents through `/research` |

**Pouya's profile:**
- **Called when:** a backlog note or task is marked `needs: research` (a fact that may have changed
  since training: a library, an API, a price, a policy, a standard), Shapour asks, the planning room
  asks, or the owner asks. The dispatcher starts Pouya before Shapour shapes that item.
- **Inputs:** the question, the item it's for, VISION.md and the decisions it names.
- **Outputs:** `docs/research/YYYY-MM-DD-<slug>.md` (answer, confidence, cited and dated findings,
  gaps), committed on its own branch; a plan in `docs/plans/` when asked; a short report to whoever
  asked. Shapour turns a plan into tasks.
- **Never:** builds, files or moves tasks, or edits code.
- **Tools:** WebSearch and WebFetch through workers; Read, Grep and Glob; Write and Edit only under
  `docs/research/` and `docs/plans/`; git on its own branch.
- **Model:** the lead model for the lead, Sonnet for the workers.
- **Cap:** 150k context (0011); 8 workers at most per question; a per-run budget cap.

Each role's profile (brief, "A role profile") lives in one file of profiles, so renaming a role or
changing its models is one line.

## Phases and work items

Estimates use Muster's units (`est-time` is agent time; `est-tokens` is total processed). Each
item ends usable on its own and ships its part of the app.

### Phase 0: decide and fix the ground (before anything launches)

**0.1 Decision: orchestration's terms and account policy**
- What: a decision record (0013; 0012 went to mu-99's account allowlist) with the six rules above,
  quoting the clauses and their dates from the research.
- Why: the owner's rule is sellable and no terms risk; the brief's load balancing breaks it.
- Acceptance: the record exists with the owner's yes; the brief's "Load balancing" section is marked
  replaced; PROTOCOL.md links the rules.
- Touches: `docs/decisions/0013-*.md`, `docs/PROTOCOL.md`, `docs/VISION.md`.
- Estimate: 1h · 150k. Depends: none.

**0.2 Decision: who launches runs**
- What: amend 0007 and mu-9 AC1. Muster's core owns the runner interface and ships a local
  `claude -p` runner now; Drover's runner implements the same interface later and replaces it.
- Why: the brief's step 5 launches from Muster, but 0007 says Drover runs agents.
- Acceptance: the record has the owner's yes; mu-9's AC1 note points to it.
- Touches: `docs/decisions/0015-*.md`, mu-9.
- Estimate: 30m · 100k. Depends: none.

**0.3 Back up local queues** (brief, AS_BUILT issue 1)
- What: `muster init` makes `.muster/` its own git repo; the core commits the paths it wrote after
  each change, under the lock. The gate refuses `git clean -x`/`-X` in a main checkout.
- Why: one `git clean -fdx` deletes a whole queue, and a dispatcher multiplies how often agents run
  git.
- Acceptance: a test makes a queue, changes a task, runs `git clean -fdx` in the checkout, and
  restores the task from `.muster/`'s history; each write leaves one commit; `muster check` reports a
  queue with uncommitted changes.
- Touches: `ops.rs`, `queue.rs`, `layout.rs`, `cli.rs`, the layer's guard.
- Estimate: 1d · 2M. Depends: none.

**0.4 Stage-lag check** (AS_BUILT issue 3)
- What: `muster check` warns when a task's branch is in main but its stage is Building or Testing,
  and prints the pass command.
- Why: the dispatcher reads stages; a lagging stage blocks dependents and wastes slots.
- Acceptance: a fixture with a merged branch in Building gets the warning and the command; a clean
  queue gets none.
- Touches: `parallel.rs`, `queue.rs` (problems), `cli.rs`.
- Estimate: 2h · 400k. Depends: none.

**0.5 Claims carry over to a successor** (0011 "Still open")
- What: a claim records the run that holds it as well as the session; `muster takeover <task>
  --from <old session>` moves it to a successor under the same `--as` name, logged.
- Why: dispatched runs hand off at the cap (0011); today a successor can't check in or pass without
  an owner override.
- Acceptance: a test claims as session A, takes over as B, and B checks in and passes; C without
  `--from` is refused.
- Touches: `ops.rs`, `session.rs`, `cli.rs`, PROTOCOL.md.
- Estimate: 4h · 800k. Depends: none.

### Phase 1: see before acting (nothing launches)

**1.1 Spike: usage signals on a real run**
- What: run one short `claude -p --output-format stream-json --verbose` on each of the owner's
  accounts. Record when `rate_limit_event` appears and with which fields (does it carry `utilization`
  without a status change?), its time units, and what a rejected run's last events, exit code and
  `StopFailure` hook look like. Read the status-line `rate_limits` from the same account.
- Why: settles the brief's spikes 1–3 on this Mac's version (docs say the event comes "when rate
  limit status changes").
- Acceptance: a note in mu-9 with samples (no tokens or paths in it), and a parser test fixture.
- Touches: `app/src-tauri/tests/fixtures/`. Estimate: 2h · 300k. Depends: none.

**1.2 Role profiles and slot names** (brief step 1)
- What: a profiles file for every role, Pouya included (name, prompt or `muster instructions <role>`,
  tools and permission profile, models, turn and budget caps, allowed stage moves, providers);
  claims stamped `Kaveh-2 (checkout · session … · cc-hg)`; names on cards and in logs; Settings ›
  Agents › Roles.
- Acceptance: `muster team` lists roles and free slots; renaming a role in the file renames it
  everywhere; a stamp parses back to role, slot and session; the lowest free slot is handed out.
- Touches: new `roles.rs`, `session.rs`, `cli.rs`, `help.rs`, the app's Settings.
- Estimate: 1d · 2M. Depends: none.

**1.3 Account profiles and usage readings** (brief step 4, without balancing)
- What: account profiles in the app folder (provider, config folder, plan, roles and models it may
  serve, concurrency, reserve, paused). Readings come from runs' `rate_limit_event` and from a small
  status-line shim the layer can install that writes `{account, five_hour, seven_day, at}` to the app
  folder. Never the undocumented usage endpoint. The company account can't be added.
- Acceptance: `muster accounts` shows both windows' use, reset times and reserve for each account
  from real readings; a reading older than its window shows as stale; adding a `~/.claude` (company)
  folder is refused; the header meter shows two bars per account.
- Touches: new `accounts.rs`, `settings.rs`, `cli.rs`, the layer's status line, the app header.
- Estimate: 1d · 2M. Depends: 1.1.

**1.4 Run ledger** (brief step 3)
- What: one JSONL record per run in the app folder, with OpenTelemetry GenAI names (pinned version):
  `gen_ai.operation.name=invoke_agent`, `gen_ai.provider.name`, `gen_ai.agent.name` (Kaveh-2),
  `gen_ai.agent.id` (role), `gen_ai.conversation.id` (session), `gen_ai.request.model`,
  `gen_ai.usage.input_tokens`, `output_tokens`, `cache_read.input_tokens`,
  `cache_write.input_tokens`; plus Muster's own `muster.task`, `muster.account`, wall time, turns,
  context peak, window share used (before and after), outcome. Filled from the run's `result`
  message and `transcripts.rs`; backfilled from existing transcripts for tasks with stamps.
- Acceptance: a finished run writes exactly one record that matches the task pane's Work numbers; the
  backfill produces records for Done tasks; no field holds a path, prompt or token.
- Touches: new `ledger.rs`, `transcripts.rs`, `work.rs`. Estimate: 1d · 2M. Depends: 1.2.

**1.5 Calibration and forecast**
- What: per role, type and size, the history of actual ÷ estimate for tokens, time and window share;
  p50 and p80 by resampling (evidence-based scheduling). The forecast says how much of Ready fits
  the current window and the week at p80.
- Acceptance: with a seeded history the p50 and p80 match a hand-computed fixture; with fewer than 5
  runs the ratio falls back to the role's, then to 1.0, and says so.
- Touches: new `calibrate.rs`, the board header. Estimate: 6h · 1M. Depends: 1.4.

**1.6 `muster dispatch --dry-run` and the decision log** (brief step 2)
- What: the tick from the brief (candidates, drop what doesn't fit, order by critical path, priority,
  project fairness, age), with one account per run picked by the owner's pinning rules, never by
  rotation. Each decision and skip is logged with its reason; the task pane shows the latest one;
  the header switch has Off and Dry run.
- Acceptance: on fixture queues, the output matches expected launches and skips, including
  `depends:` not merged, touches overlap, too big for one context (to Shaping), paused, held, no
  headroom ("waits until 14:20"), and a sent-back task first. A run of a day beside the manual flow
  is recorded in mu-9.
- Touches: new `dispatch.rs`, `block.rs`, `parallel.rs`, `cli.rs`, the app's Decision log.
- Estimate: 1.5d · 3M. Depends: 1.2, 1.3, 1.5, 0.4.

### Phase 2: one runner, one slot

**2.1 The runner interface and the Claude Code runner** (brief step 5)
- What: `start`, `status`, `stop`, `usage`, `limit`, `capabilities`. The Claude runner starts
  `claude -p --output-format stream-json --verbose -n <slot> --session-id <uuid> --max-turns
  --max-budget-usd --permission-prompts none --model …` in the task's worktree with
  `CLAUDE_CONFIG_DIR` for the account and 0002's environment scrub; reads the stream; maps the end to
  `completed`, `failed`, `cancelled` or `input_required`; `permission_denials` become a "blocked by a
  permission" with the denied tool.
- Acceptance: tests with a fake `claude` on PATH cover success, max turns, max budget, a rejected
  rate limit, a crash (exit without result), SIGTERM (exit 143) and permission denials; one real
  short run on a scratch task writes a ledger record.
- Touches: new `runner.rs`, `runner_claude.rs`, `ledger.rs`. Estimate: 1.5d · 3M. Depends: 0.2,
  1.1, 1.4.

**2.2 Dispatcher On: Kaveh, one slot, with failure rules**
- What: the dispatcher launches Kaveh-1 on the top candidate; on crash, reclaims at once (branch and
  worktree kept); on a rejected limit, puts the account in cooldown until `resets_at` and returns the
  task to Ready with a note (not a failure); on 300 s without a stream event, pings; at 600 s, stops
  and reclaims; retries a failed start with backoff `min(10 s × 2^(n−1), 5 min)`; reruns a failed
  gate once; caps respawns per task at 3 in an hour. Ticks every 30 s and on queue change, run exit
  or limit event; runs inside the app, plus `muster dispatch --watch`.
- Acceptance: each rule has a test with the fake runner; one real task goes Ready → Testing with no
  owner step; quitting the app stops the run and leaves the claim reclaimable.
- Touches: `dispatch.rs`, `runner.rs`, `ops.rs` (reclaim), notifications.
- Estimate: 1.5d · 3M. Depends: 2.1, 0.3, 0.5.

**2.3 Kia with lens reviewers as subagents** (brief step 6)
- What: Kia's run gets subagent definitions for Sohrab, Parviz and Azadeh, chosen by the task's
  lenses (mu-76); findings are signed "Kia → Sohrab" in the review of record; Kia is never the
  builder (0047).
- Acceptance: a real task goes Testing → Your check; each finding names its reviewer; a backend-only
  task runs no Azadeh; the brief's spike 5 (subagents versus separate sessions) is recorded with cost.
- Touches: `review.rs`, role profiles, the subagent files. Estimate: 1d · 2M. Depends: 2.2.

**2.4 Team view** (the app)
- What: every role and slot with task, account, model, since when, state, tokens and time; Stop,
  ping, reclaim; open the task or transcript.
- Acceptance: matches `muster team`; the actions work on a fake run.
- Touches: app views. Estimate: 1d · 1.5M. Depends: 2.2.

### Phase 3: scale inside the terms

**3.1 Slots, repo limits, fairness and pacing** (brief step 7, without rotation)
- What: N slots per account (default 2), builders per repo, merges one at a time, priority then
  round-robin across projects, weekly pacing (what's left of the week spread over the days to reset,
  with burst room), the owner's reserve (default 20%) against `used_percentage`.
- Acceptance: fixtures show no repo over its limit, no project starved over 20 ticks, no launch that
  would cross the reserve at p80, and a 7-day simulation that doesn't run out before the reset.
- Touches: `dispatch.rs`, `accounts.rs`. Estimate: 1d · 2M. Depends: 2.2, 1.5.

**3.2 A second account by hand**
- What: the owner may pin projects to a second account they hold. The dispatcher uses each account
  only for its pinned projects. A limit on one never moves work to another.
- Acceptance: a test with two accounts at their limits shows no move; Settings says why there is no
  automatic balancing and links decision 0013.
- Touches: `accounts.rs`, Settings › Accounts. Estimate: 3h · 500k. Depends: 0.1, 3.1.

**3.3 Leila for escalations** (brief step 9)
- What: after 2 review rounds or 2 failed gates, or a technical question, a short Leila run reads
  VISION and decisions and answers, splits, returns with guidance, or escalates to Waiting for you
  with what she tried.
- Acceptance: fixtures for each path; only product questions reach the owner; Leila's run is capped.
- Touches: `dispatch.rs`, role profiles. Estimate: 1d · 2M. Depends: 2.2.

**3.4 Shaping roles and Pouya** (brief step 8)
- What: Shapour, Arash and Dariush shape backlog notes and split anything above one context at p80;
  Pouya runs first for items marked `needs: research`.
- Acceptance: a backlog note goes to Ready without the owner; a too-big task is split; a research
  item gets a cited doc before shaping.
- Touches: `backlog.rs`, role profiles. Estimate: 1d · 2M. Depends: 2.2, 1.2.

**3.5 Usage view**
- What: the ledger by account, role, project and day; estimate versus actual; calibration ratios.
- Acceptance: totals match the ledger file; filters work. Touches: app views.
- Estimate: 1d · 1.5M. Depends: 1.4, 1.5.

### Phase 4: more runners

**4.1 Codex runner, reviewer roles first** (brief step 10)
- What: `codex exec --json --sandbox workspace-write` for start, usage from `turn.completed`; rate
  limits from the app server's `account/rateLimits/read`, with its analytics turned off; the repo's
  instructions available as `AGENTS.md`. Serves Sohrab or a second reviewer, since a different model
  catches different mistakes.
- Acceptance: a fake-codex test suite like 2.1's; one real review run; a check that no analytics
  are on; capabilities say "no mid-run answers".
- Touches: new `runner_codex.rs`, `accounts.rs`. Estimate: 1.5d · 3M. Depends: 2.1, 1.1 (Codex part).

**4.2 Drover runner** (brief step 11)
- What: Drover implements the runner interface; Muster's own Claude runner becomes the fallback.
- Acceptance: the 2.1 test suite passes against Drover's runner. Estimate: Drover's task; 1d on
  Muster's side. Depends: 0.2, Drover 0018.

## Order and the critical path

0.1, 0.2 → 1.1 → 1.3 → 1.6 → 2.1 → 2.2 → 2.3 and 3.1. Phase 0's fixes (0.3–0.5) and 1.2, 1.4 run in
parallel with that path. About 20 agent-days and 40M tokens in all, before calibration.

## Open questions, with the default if nobody answers

| Question | Default |
|---|---|
| Owner's reserve per window | 20%, measured against the account's own `used_percentage` |
| Concurrent runs per account | 2 (the owner's interactive work draws on the same pool) |
| Fairness across projects | priority, then round-robin; weights later if a project starves |
| Where the dispatcher runs | inside the app plus `muster dispatch --watch`; a login item (launchd) only after a week of On without surprises |
| Names | keep the brief's set, add Pouya |
| Second provider | Codex, on the owner's ChatGPT login on this Mac (the docs allow trusted scripts on private machines), reviewer roles only |
| Several Claude accounts | one per dispatcher; a second only pinned by hand (3.2); no rotation. For more capacity: a bigger plan, a Team plan or an API key |
| Who launches runs | Muster's runner now, Drover's later (0.2) |
