# Orchestration (mu-9): the brief against what exists, and what the terms allow (2026-10-14)

Research for the owner's orchestration brief (`tasks/proposals/Muster orchestration design
brief.md`, 2026-10-14). The plan built from it is `docs/plans/orchestration-plan.md`. Earlier
research on agent task trackers (Beads, Spec Kit, OpenSpec, Task Master, Conductor) is in
`2026-10-13-agent-task-tools.md` [78] and isn't repeated here.

**Answer:** The brief's core holds up. A dispatcher written as code, one fresh session per task,
state in the queue, per-run caps, a decision log and a dry run are what the vendors' own guidance
and the one comparable open spec (OpenAI's Symphony) recommend [32][46][50][52]. Three parts need
changing:

- **Capacity doesn't have to be learned.** Claude Code now reports each account's 5-hour and
  weekly use and reset time to programs, both in headless output and to status-line scripts
  [13][14][21]. Codex reports it through its app server [25].
- **Load balancing across several personal Claude accounts is a terms risk.** Anthropic says Pro
  and Max limits "assume ordinary, individual usage" [2], forbids "bypassing any of our systems or
  protective measures" [1], and its Usage Policy from 2026-11-12 forbids services "that route
  requests through consumer subscriptions" [4]. No clause bans holding two accounts. But a
  rule that spends whichever account resets soonest exists to get past per-account limits, and
  that's the use these clauses are aimed at. A product sold to others can't do it at all [2][3].
- **Calibration needs ranges, not one ratio.** Runs of the same coding task vary up to 30× in
  tokens, and models underestimate their own use [72].

**Confidence:** high on the mechanics (Anthropic's and OpenAI's own docs and source, read
2026-10-14). Medium on the terms: the Anthropic clauses are primary and quoted, but no clause
speaks to one person's several accounts directly, so the risk reading is mine. OpenAI's Terms of
Use couldn't be fetched (403), so its clauses are quoted from the Open Terms Archive copy.

## Findings

### 1. The terms: several accounts, unattended use, selling Muster

**Anthropic (Claude).** Quoted from the pages as read on 2026-10-14:

| Clause | Source, date | What it means for the brief |
|---|---|---|
| "You may not share your Account login information, Anthropic API key, or Account credentials with anyone else." | Consumer Terms, effective 2025-10-08 [1] | A sold Muster can't run a user's work on anyone else's account |
| "To access the Services through automated or non-human means, whether through a bot, script, or otherwise … Except when you are accessing our Services via an Anthropic API Key or where we otherwise explicitly permit it" | Consumer Terms §3, 2025-10-08 [1] | Scripted use of a subscription is allowed only where Anthropic permits it. Claude Code's headless mode and background sessions are documented, supported features of Anthropic's own client [10][16], which is the permission the owner's own runs rely on |
| "bypassing any of our systems or protective measures" | Consumer Terms §3, 2025-10-08 [1] | Usage limits are a protective measure. Rotating accounts to keep working past one account's limit is the clearest way to read as bypassing them |
| "Advertised usage limits for Pro and Max plans assume ordinary, individual usage of Claude Code and the Agent SDK." | Claude Code docs, Legal and compliance (undated, read 2026-10-14) [2] | A dispatcher running slots around the clock is not "ordinary". It's allowed, but Anthropic sizes and polices limits against it |
| "OAuth authentication is intended exclusively for purchasers of Claude Free, Pro, Max, Team, and Enterprise subscription plans and is designed to support ordinary use of Claude Code and other native Anthropic applications." | Legal and compliance [2] | Muster must drive the owner's own `claude` binary, never call the API with a subscription token itself (decision 0002 already does this) |
| "Anthropic does not permit third-party developers to offer Claude.ai login into their own applications, or to route requests through Free, Pro, or Max plan credentials on behalf of their users. Moreover, developers may not collect, store, or intermediate Claude.ai credentials or session tokens" | Legal and compliance [2]; the same in the Agent SDK overview [3] | A sold Muster can't read, store or pass on login tokens. It may start the user's own Claude Code, signed in by the user |
| "Customers may not pay for, resell, or intermediate Claude usage on their end users' behalf. Each end user must authenticate with their own Anthropic API key, Claude subscription plan credentials, or 3P inference provider credential" | Legal and compliance [2] | The sellable shape: each user brings their own login, Muster never pools accounts |
| "Resell, proxy, or otherwise provide access to Claude through unauthorized means, including services that route requests through consumer subscriptions or misrepresent the product or client being used" | Usage Policy, effective **2026-11-12** (published 2026-10-08, not yet in force; not in the 2025-09-15 version) [4][4a] | Aimed at services for others, but "route requests through consumer subscriptions" also describes an account pool |
| "Coordinate malicious activity across multiple accounts, or create multiple accounts, to avoid detection, circumvent product guardrails, or generate identical or similar inputs that otherwise violate our Usage Policy" | Usage Policy, effective **2026-11-12** [4]. The 2025-09-15 version in force today reads "Coordinate malicious activity across multiple accounts to avoid detection or circumvent product guardrails or generating identical or similar inputs that otherwise violate our Usage Policy" [4a]; the new words are "or create multiple accounts" | From 2026-11-12, creating accounts to "circumvent product guardrails" is named outright. Usage limits are a guardrail on any plain reading |
| "We only allow a maximum of three Claude accounts to be verified with the same phone number." | Support, 2026-08-17 [6] | The only numeric rule on several accounts. Holding two or three is not itself banned |
| "Both Pro and Max plans have a five-hour session limit and a weekly limit." "These limits are shared across Claude and Claude Code" | Support, read 2026-10-14 [5][71] | The owner's chat and interactive sessions draw on the same pool as dispatched runs, so the reserve matters |

Enforcement is real. On 2026-04-04 Anthropic stopped subscription limits from covering third-party
harnesses such as OpenClaw; Claude Code's lead called it making "clear and explicit" what "isn't
something we support" [8]. The July 2025 weekly limits were introduced against use "continuously in
the background, 24/7", account sharing and reselling [9]. Background sessions consume quota "the same
as interactive sessions, so running ten agents in parallel uses quota roughly ten times as fast" [16].

Gas Town, the closest open-source orchestrator, ships a `quota_dog` that rotates Claude accounts
when one hits its limit (changelog 1.2.0, 2026-05-27) [34]. That is the pattern to avoid.

**OpenAI (Codex).** Codex's docs: "Use API key authentication for programmatic Codex CLI
workflows, such as CI/CD jobs", and ChatGPT sign-in tokens "are intended for trusted scripts,
schedulers, and private CI runners" [22]. Running `codex exec` on the owner's own Mac with the owner's
ChatGPT login fits that. The Terms of Use (Europe version, updated 2026-01-16, read from the Open
Terms Archive copy because openai.com returned 403) say: "You may not share your account credentials or
make your account available to anyone else", forbid "circumventing any rate limits or restrictions",
and forbid "Automatically or programmatically extracting data or Output" [31]. Codex's CI guide adds
that a ChatGPT `auth.json` must not be shared "across concurrent jobs or multiple machines" [22]. OpenAI's 2026-04-28
safety post says it may ban "the same user's other accounts" after a bannable offence [31]. No OpenAI
text on holding several accounts was found.

**My reading (not a lawyer's):**

| Brief item | Risk | Why |
|---|---|---|
| Dispatcher starting the owner's own `claude -p` on the owner's own account | Low | Documented feature of the official client [10]; the owner's own login; caps and pacing keep it near "ordinary" use |
| Running unattended overnight | Low to medium | Supported by `--bg` and `-p` [10][16], but limits were tightened for 24/7 use [9]. Weekly pacing and the reserve keep it moderate |
| Several personal Claude accounts with soonest-reset-first balancing | **High: drop it** | It exists to get past per-account limits ("bypassing … protective measures" [1]; "ordinary, individual usage" [2]; account pooling, as in Gas Town [34]). From 2026-11-12 the Usage Policy names creating "multiple accounts, to … circumvent product guardrails" [4] |
| The same for Codex across several ChatGPT accounts | **High: drop it** | "circumventing any rate limits or restrictions" [31] |
| The owner using a second account they already hold, chosen by hand for a project | Low to medium | No clause bans it. Keep it a manual choice in the owner's settings, never automatic rotation |
| Selling Muster with account pooling | **Not allowed** | Third parties may not route through subscription credentials or intermediate usage [2][3] |
| Selling Muster that starts each user's own `claude` or `codex`, signed in by them | Low | Each end user authenticates themselves [2]; Conductor sells this shape and says it "does not bill or resell model usage" [38] |
| More Claude capacity | Clean routes | A higher plan, a Team plan, or an API key (Commercial Terms, which allow powering products [7]) |

### 2. What Claude Code gives a dispatcher (version 2.1.296 on this Mac; docs at 2.1.300)

- **Headless runs.** `claude -p --output-format stream-json --verbose` streams events and ends with
  a `result` message: `subtype` (`success`, `error_max_turns`, `error_max_budget_usd`,
  `error_during_execution`), `duration_ms`, `num_turns`, `total_cost_usd`, `usage` (main loop only),
  `modelUsage` (including subagents), `permission_denials` [10][12]. Costs are client-side
  estimates, not billing data [20].
- **Caps.** `--max-turns` and `--max-budget-usd` (print mode only; an estimate that can overshoot;
  subagent spend counts from v2.1.217) [11]. `--permission-prompts none` (v2.1.259) denies prompts
  nobody can answer instead of hanging [11]. `--session-id`, `--resume`, `--model`,
  `--append-system-prompt`, `--allowedTools` and `--permission-mode` are all available [11].
- **Usage limits, readable.** Stream-json and the SDK emit a `rate_limit_event` whose status is
  `allowed`, `allowed_warning` or `rejected`, with `resets_at`, `rate_limit_type` (`five_hour`,
  `seven_day`, `seven_day_opus`, `seven_day_sonnet`, `overage`) and `utilization` (0.0–1.0), all on the
  event's nested `rate_limit_info`. It is
  "emitted when rate limit status changes" [13], and v2.1.295 improved its warnings [21]. Status-line
  scripts receive `rate_limits.five_hour` and `.seven_day` with `used_percentage` and `resets_at`, for
  Pro and Max (or behind a Claude apps gateway with a spend limit), after the first response; each
  window may be absent on its own [14]. A `StopFailure` hook fires with a `rate_limit` matcher
  when a turn ends on one [15]. Retries show as `system/api_retry` events with a `rate_limit`
  category [10].
- **Don't use the hidden endpoint.** Some menu-bar meters call an undocumented
  `api.anthropic.com/api/oauth/usage` [69]. It rate-limits hard, and an issue asking about it was
  closed with no Anthropic reply [70]. It also means handling OAuth tokens, which [2] forbids for a
  product.
- **Background sessions** (`claude --bg`) are supervised, isolate edits in a worktree, are listed by
  `claude agents --json` (states `working`, `blocked`, `done`, `failed`, `stopped`), and can't be
  combined with `-p` [16]. Muster's `agents.rs` already reads that list.
- **Agent teams don't start in `-p`.** "In non-interactive mode with the `-p` flag, including Agent
  SDK sessions, Claude doesn't spawn teammates" [17]. A headless Kia gets her reviewers as subagents
  (which support `isolation: worktree` and `maxTurns` [18]), not as teammates. That fits the brief's
  "subagent of Kia's session", and changes decision 0008's "review lenses as a team" for runs the
  dispatcher starts.
- **Telemetry.** Claude Code's OpenTelemetry export has `claude_code.token.usage`,
  `claude_code.cost.usage` and an `api_request` event, and "doesn't set `gen_ai.usage.*`
  attributes" [19]. It's off unless `CLAUDE_CODE_ENABLE_TELEMETRY=1`. The ledger doesn't need it: the
  result message and `transcripts.rs` hold the same numbers locally.

### 3. What Codex gives a dispatcher (0.162.1, 2026-10-09)

- `codex exec --json` streams JSONL (`thread.started`, `turn.completed` with token usage, `item.*`,
  `turn.failed`, `error`) [23][28]. Usage has input, cached input, cache write, output and
  reasoning tokens, and no rate-limit field [28]. A request to add rate limits to exec output was
  closed on 2026-05-16 without being built ("This feature hasn't received any upvotes, so closing";
  GitHub records the reason as "completed") [26].
- Rate limits are readable through the app server's `account/rateLimits/read` and
  `account/rateLimits/updated`, with `usedPercent`, `windowDurationMins` (300 for the 5-hour window)
  and `resetsAt` [25][27]. The app server is "experimental and isn't supported for production
  workloads" [80]. `codex mcp-server` was removed in 0.154.0 [80].
- `--full-auto` is gone; exec runs read-only by default, with `--sandbox workspace-write` or
  `--approve-for-me` to write [23]. Hooks (`PreToolUse`, `Stop`, `PermissionRequest` …) exist [30].
- Plan limits: Plus and Business get estimates per 5-hour period, "weekly limits may also apply",
  Pro has no 5-hour limit, and credits extend use [24]. No weekly numbers are published.
- **Privacy flag:** 0.162.0's changelog says "Enable analytics by default for daemon-launched app
  servers" [29]. Any adapter that starts the app server has to turn that off, per the owner's
  no-telemetry rule.

### 4. How other orchestrators do it

| | Who assigns | Isolation | Failure handling | Cost and limits | Humans |
|---|---|---|---|---|---|
| **OpenAI Symphony** (SPEC draft v1) | A deterministic poll tick; global concurrency 10 by default, per-state limits [32] | Per-issue workspace [32] | Retry with `min(10000 × 2^(attempt−1), max_backoff)` ms, capped at 5 min; kill a worker silent for 300 s and retry [32] | Tracks the latest rate-limit payload and token totals [32] | "A run MUST NOT stall indefinitely waiting for user input"; ends at a handoff state such as Human Review [32] |
| **Gas Town** | An LLM Mayor plus a config-driven Scheduler that caps dispatch "to prevent API rate limit exhaustion" [33] | Worktrees; a Bors-style Refinery merges, polecats never push to main [33] | Witness/Deacon/Daemon watchdogs; a circuit breaker caps respawns (0.12.0) [34] | Rotates accounts on limits (`quota_dog`, 1.2.0) [34] | Escalations by severity up to a human Overseer [33] |
| **OpenHands** | The app or SDK caller | Docker or remote workspace | Stuck detector (same error 3+ times) [35] | No server-enforced budget yet (open, 2026-10-07) [36] | Confirmation policy pauses in `WAITING_FOR_CONFIRMATION` [35] |
| **Copilot cloud agent** | A human assigns an issue | Ephemeral environment, one branch and PR per task; 59-minute cap [41] | — | One premium request per session × model rate [43] | Another person must approve; Actions don't run on its pushes until approved [42] |
| **Devin** | An LLM coordinator spawns child sessions [44] | A VM each [44] | Sleeps after 30 idle minutes [45] | ACU limit per child [44] | A child needing input wakes the coordinator [44] |
| **Cursor cloud agents** | Humans or integrations [40] | Isolated VMs [40] | — | API pricing, with a spend limit set up front [40] | — |
| **Conductor, claude-squad** | The human [37][39] | Worktrees ("not a security boundary") [37] | — | User's own account or key; Conductor "does not bill or resell model usage" [38] | The human drives |

Only Symphony matches the brief's "dispatcher is code" end to end. Its stall timeout, backoff formula,
concurrency caps and no-blocking rule are concrete numbers worth starting from. Gas Town shows the
merge queue and circuit breaker are worth having, and the account rotation isn't. Every hosted
product puts a human approval before merge, which Muster already has in Your check.

### 5. Design guidance from frameworks and research

- Anthropic: "Workflows are systems where LLMs and tools are orchestrated through predefined code
  paths", and agents need "stopping conditions (such as a maximum number of iterations)" [46].
  Microsoft: "If you can write a function to handle the task, do that instead of using an AI agent"
  (2026-07-29) [50]. OpenAI's Agents SDK: "orchestrating via code makes tasks more deterministic and
  predictable, in terms of speed, cost and performance" [52]. All three back principle 1.
- Multi-agent costs about 15× the tokens of chat, and coding has fewer parallel subtasks than
  research [47]. Cognition: "Actions carry implicit decisions, and conflicting decisions carry bad
  results" [48]. The MAST study of 1,600+ traces sorts failures into system design, inter-agent
  misalignment and task verification [49]. Both back one session per task and an independent tester.
- Anthropic's research system pairs agents with "deterministic safeguards like retry logic and
  regular checkpoints" [47]. LangGraph's resume re-runs a node from its start, so side effects before
  a pause must be idempotent [54]; the same holds for a relaunched run (worktree and branch kept,
  claim re-taken, nothing done twice). CrewAI defaults `max_iter` 20 and `max_retry_limit` 2 [53].

### 6. Capacity, metering and calibration

- Third-party meters group local transcripts into 5-hour blocks from the first message (ccusage
  [67]) or learn a limit from the 90th percentile of 8 days of use (Claude-Code-Usage-Monitor [68]).
  Both predate the official `rate_limits` field [14], which gives the real figure instead.
- Anthropic doesn't say when a 5-hour window starts [71]; the weekly reset is a fixed time per
  account (support summary, not fetched verbatim) [5]. Use `resets_at` from the account itself.
- Total tokens vary up to 30× between runs of the same SWE-bench task, frontier models predict their
  own use with correlation up to 0.39 and underestimate it, and expert difficulty ratings match cost
  only weakly [72]. METR's time horizons measure human-equivalent task length, not agent tokens [73].
- Evidence-based scheduling keeps each estimator's history of estimate ÷ actual and runs a Monte
  Carlo over it to give a probability of finishing by each date [74]. The same works per role, type
  and size, giving a p50 and p80 rather than one ratio.
- Retry: Anthropic's SDK backs off exponentially, twice by default, honouring `retry-after`; spend-cap
  429s have none and shouldn't be retried [75]. Temporal fails an attempt whose heartbeat goes quiet
  past its timeout [76]. Beads' 5-minute lease turned out to say nothing about liveness once nothing
  refreshed it; measured claim-to-merge was p50 117 min, p90 333 min [77]. A process the dispatcher
  started is better watched directly (exit, last event time) than through a lease.

### 7. Standards worth adopting

- **AGENTS.md**, now stewarded by the Agentic AI Foundation under the Linux Foundation [55][56], is
  what Codex, Cursor, Copilot and others read [55]. Claude Code reads `CLAUDE.md`. A Codex adapter
  needs the repo's instructions in `AGENTS.md`, or a `CLAUDE.md` that imports it.
- **MCP 2026-07-28** [57]. Its Tasks extension uses `working`, `input_required`, `completed`, `failed`,
  `cancelled` (the last three terminal) [58][59]. That's a tested vocabulary for a run's state.
  Muster doesn't need to speak MCP to use it.
- **OpenTelemetry GenAI conventions**, now in their own repo and all at Status Development [60]. The
  attribute names fit the ledger: `gen_ai.operation.name` (`invoke_agent`), `gen_ai.provider.name`,
  `gen_ai.agent.name`, `gen_ai.agent.id`, `gen_ai.conversation.id`, `gen_ai.request.model`,
  `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`,
  `gen_ai.usage.cache_read.input_tokens`, `gen_ai.usage.cache_write.input_tokens` [60][61]. Metric
  names changed recently (`gen_ai.client.token.usage` is gone) [62], so pin a version and store the
  names locally. Nothing gets exported.
- **ACP** (Zed and JetBrains, Apache-2.0, v1 stable, v2 draft 2026-07-20) drives Claude Code and
  Codex through adapters over JSON-RPC on stdio [63][64]. Worth watching for runner adapters; not
  needed for the first one.
- **A2A 1.0** (in AAIF since 2026-08-17) is for networked agents calling each other [65][66]. Over
  the top for one Mac and one owner.

## Trade-offs, and what would change the answer

- If Anthropic published a rule allowing one person's several accounts for one workload, or if the
  owner moves to a Team plan, account choice across seats becomes a clean feature again.
- If `rate_limit_event` turns out to carry `utilization` only on status changes (the docs say it's
  emitted on change [13]), the ledger still needs the status-line snapshot or before/after readings
  to cost each run. The first spike in the plan settles this on a real run.
- If Drover's runner (decision 0007) is close, Muster's own `claude -p` runner is a stop-gap and
  should stay thin.
- If the Codex app server leaves experimental, it becomes the better Codex adapter (approvals,
  steering, rate limits in one place) than `codex exec`.

## Gaps and caveats

- No clause found that speaks directly to one person spreading a workload over several of their
  own accounts. The risk rating above is an inference from [1][2][4][34].
- The Usage Policy quoted [4] takes effect 2026-11-12. The version in force today [4a] lacks the
  resale bullet and the "create multiple accounts" words; both are new.
- OpenAI's Terms of Use and help centre returned 403. Its clauses are from the Open Terms Archive
  copy of the Europe terms (updated 2026-01-16) [31]; the US page wasn't read, nor the help article
  on Codex with ChatGPT plans. "No multiple-accounts sentence" in OpenAI's terms is a tool-reported
  absence.
- A fresh agent audited eight key claims on 2026-10-14 against the sources ([1], [2], [4], [4a],
  [13], [14], [17], [26], [28], [72]): five held as written, three were corrected (the old Usage
  Policy bullet, who gets status-line `rate_limits`, how #14728 was closed). Other claims rest on
  one worker each.
- Codex doc pages carry no dates; dates come from its changelog, releases and issues.
- The status-line `rate_limits` field's first version (2.1.80) is from the Claude Code changelog;
  the docs page names no version.
- Gas Town's account rotation has no docs beyond its changelog lines [34].
- Several pages were read through a summarising fetch, so a quote may be trimmed.
- No prompt-injection attempts were found in fetched pages.

## Sources

[1] Consumer Terms of Service — Anthropic, effective 2025-10-08 — https://www.anthropic.com/legal/consumer-terms (primary)
[2] Legal and compliance — Anthropic, Claude Code docs, undated, read 2026-10-14 — https://code.claude.com/docs/en/legal-and-compliance (primary)
[3] Agent SDK overview — Anthropic, read 2026-10-14 — https://code.claude.com/docs/en/agent-sdk/overview (primary)
[4] Usage Policy — Anthropic, effective 2026-11-12 — https://www.anthropic.com/legal/aup (primary)
[4a] Usage Policy (effective 2025-09-15, archived) — Anthropic — https://www.anthropic.com/legal/archive/22742366-2ef0-4c7a-a833-6523f10d3944 (primary)
[5] Use Claude Code with your Pro or Max plan; What is the Max plan? — Anthropic Support, read 2026-10-14 — https://support.claude.com/en/articles/11145838-use-claude-code-with-your-pro-or-max-plan ; https://support.claude.com/en/articles/11049741-what-is-the-max-plan (primary)
[6] Verify your phone number — Anthropic Support, 2026-08-17 — https://support.claude.com/en/articles/8287232-verify-your-phone-number (primary)
[7] Commercial Terms of Service — Anthropic, effective 2025-06-17 — https://www.anthropic.com/legal/commercial-terms (primary; older than 12 months)
[8] Anthropic says Claude Code subscribers will need to pay extra for OpenClaw usage — TechCrunch, 2026-04-04 — https://techcrunch.com/2026/04/04/anthropic-says-claude-code-subscribers-will-need-to-pay-extra-for-openclaw-support/ (secondary)
[9] Anthropic unveils new rate limits to curb Claude Code power users — TechCrunch, 2025-07-28 — https://techcrunch.com/2025/07/28/anthropic-unveils-new-rate-limits-to-curb-claude-code-power-users/ (secondary)
[10] Run Claude Code programmatically — Anthropic, read 2026-10-14 — https://code.claude.com/docs/en/headless (primary)
[11] CLI reference — Anthropic, read 2026-10-14 — https://code.claude.com/docs/en/cli-reference (primary)
[12] Agent SDK reference, TypeScript — Anthropic, read 2026-10-14 — https://code.claude.com/docs/en/agent-sdk/typescript (primary)
[13] Agent SDK reference, Python (RateLimitEvent) — Anthropic, read 2026-10-14 — https://code.claude.com/docs/en/agent-sdk/python (primary)
[14] Customize your status line — Anthropic, read 2026-10-14 — https://code.claude.com/docs/en/statusline (primary)
[15] Hooks reference — Anthropic, read 2026-10-14 — https://code.claude.com/docs/en/hooks (primary)
[16] Agent view and background sessions — Anthropic, read 2026-10-14 — https://code.claude.com/docs/en/agent-view (primary)
[17] Orchestrate teams of Claude Code sessions — Anthropic, read 2026-10-14 — https://code.claude.com/docs/en/agent-teams (primary)
[18] Create custom subagents — Anthropic, read 2026-10-14 — https://code.claude.com/docs/en/sub-agents (primary)
[19] Monitoring (OpenTelemetry) — Anthropic, read 2026-10-14 — https://code.claude.com/docs/en/monitoring-usage (primary)
[20] Track cost and usage — Anthropic, read 2026-10-14 — https://code.claude.com/docs/en/agent-sdk/cost-tracking (primary)
[21] Claude Code CHANGELOG (2.1.295–2.1.300) — Anthropic, 2026-10 — https://raw.githubusercontent.com/anthropics/claude-code/main/CHANGELOG.md (primary)
[22] Codex authentication — OpenAI, undated, read 2026-10-14 — https://learn.chatgpt.com/docs/auth ; https://learn.chatgpt.com/docs/auth/ci-cd-auth (primary)
[23] Codex non-interactive mode — OpenAI, undated — https://learn.chatgpt.com/docs/non-interactive-mode (primary)
[24] Codex pricing — OpenAI, undated — https://learn.chatgpt.com/docs/pricing (primary)
[25] Codex App Server — OpenAI, undated — https://learn.chatgpt.com/docs/app-server (primary)
[26] feat(exec): emit rate_limits in exec mode JSONL output (#14728) — openai/codex, 2026-03-15, closed 2026-05-16 — https://github.com/openai/codex/issues/14728 (primary)
[27] Expose rate-limit reset times … as status_line tokens (#24080) — openai/codex, 2026-05-22 — https://github.com/openai/codex/issues/24080 (secondary)
[28] codex-rs/exec/src/exec_events.rs — openai/codex main, 2026-10 — https://raw.githubusercontent.com/openai/codex/main/codex-rs/exec/src/exec_events.rs (primary)
[29] ChatGPT & Codex changelog (0.162.0) — OpenAI, 2026-10-07/08 — https://developers.openai.com/codex/changelog (primary)
[30] Codex hooks — OpenAI, undated — https://learn.chatgpt.com/docs/hooks (primary)
[31] Terms of Use (Europe, updated 2026-01-16) — OpenAI, via Open Terms Archive — https://raw.githubusercontent.com/OpenTermsArchive/genai-contrib-versions/main/ChatGPT/Terms%20of%20Service.md ; Our commitment to community safety — OpenAI, 2026-04-28 — https://openai.com/index/our-commitment-to-community-safety/ (primary; see the note in Gaps on how the text was obtained)
[32] Symphony SPEC.md (draft v1) — OpenAI, undated — https://github.com/openai/symphony/blob/main/SPEC.md (primary)
[33] Gas Town README — gastownhall/gastown, undated — https://github.com/steveyegge/gastown (primary)
[34] Gas Town CHANGELOG (0.2.3 2026-01-09; 0.12.0 2026-03-11; 1.2.0 2026-05-27) — https://github.com/gastownhall/gastown/blob/main/CHANGELOG.md (primary)
[35] Stuck Detector; security (confirmation policy) — OpenHands docs — https://docs.openhands.dev/sdk/guides/agent-stuck-detector (primary)
[36] Settable, server-enforced per-conversation cost budget (#5561) — OpenHands, 2026-10-07 — https://github.com/OpenHands/software-agent-sdk/issues/5561 (primary)
[37] Isolated workspaces — Conductor — https://www.conductor.build/docs/concepts/workspaces-and-branches (primary)
[38] Configure model providers — Conductor — https://www.conductor.build/docs/guides/providers (primary)
[39] claude-squad README — smtg-ai — https://github.com/smtg-ai/claude-squad (primary)
[40] Cloud Agents — Cursor — https://cursor.com/docs/cloud-agent (primary)
[41] About Copilot cloud agent — GitHub Docs — https://docs.github.com/en/copilot/concepts/agents/cloud-agent/about-cloud-agent (primary)
[42] Review Copilot PRs — GitHub Docs — https://docs.github.com/en/copilot/how-tos/use-copilot-agents/cloud-agent/review-copilot-prs (primary)
[43] Copilot requests — GitHub Docs — https://docs.github.com/en/copilot/concepts/billing/copilot-requests (primary)
[44] Advanced capabilities — Devin docs — https://docs.devin.ai/work-with-devin/advanced-capabilities (primary)
[45] Usage — Devin docs — https://docs.devin.ai/admin/billing/usage (primary)
[46] Building effective agents — Anthropic, 2024-12-19 — https://www.anthropic.com/engineering/building-effective-agents (primary; older than 12 months)
[47] How we built our multi-agent research system — Anthropic, 2025-06-13 — https://www.anthropic.com/engineering/multi-agent-research-system (primary; older than 12 months)
[48] Don't Build Multi-Agents — Cognition, 2025-06-12 — https://cognition.com/blog/dont-build-multi-agents (primary; older than 12 months)
[49] Why Do Multi-Agent LLM Systems Fail? — arXiv 2503.13657, v1 2025-03-17, v3 2025-10-26 — https://arxiv.org/abs/2503.13657 (primary)
[50] Microsoft Agent Framework overview — Microsoft Learn, 2026-07-29 — https://learn.microsoft.com/en-us/agent-framework/overview/ (primary)
[51] Workflows: Checkpoints — Microsoft Learn, 2026-09-16 — https://learn.microsoft.com/en-us/agent-framework/workflows/checkpoints (primary)
[52] Orchestrating multiple agents — OpenAI Agents SDK — https://openai.github.io/openai-agents-python/multi_agent/ (primary)
[53] Running agents — OpenAI Agents SDK; Agents — CrewAI — https://openai.github.io/openai-agents-python/running_agents/ ; https://docs.crewai.com/en/concepts/agents (primary)
[54] Interrupts — LangGraph — https://docs.langchain.com/oss/python/langgraph/interrupts (primary)
[55] AGENTS.md — https://agents.md (primary)
[56] Linux Foundation announces the Agentic AI Foundation — 2025-12-09 — https://www.linuxfoundation.org/press/linux-foundation-announces-the-formation-of-the-agentic-ai-foundation (primary)
[57] MCP versioning — 2026-07-28 — https://modelcontextprotocol.io/specification/versioning (primary)
[58] MCP 2026-07-28 key changes — https://modelcontextprotocol.io/specification/2026-07-28/changelog (primary)
[59] MCP Tasks extension — https://modelcontextprotocol.io/extensions/tasks/overview (primary)
[60] GenAI agent spans — OpenTelemetry, Status Development — https://raw.githubusercontent.com/open-telemetry/semantic-conventions-genai/main/docs/gen-ai/gen-ai-agent-spans.md (primary)
[61] GenAI spans — OpenTelemetry — https://raw.githubusercontent.com/open-telemetry/semantic-conventions-genai/main/docs/gen-ai/gen-ai-spans.md (primary)
[62] GenAI token metrics — OpenTelemetry — https://raw.githubusercontent.com/open-telemetry/semantic-conventions-genai/main/docs/gen-ai/gen-ai-token-metrics.md (primary)
[63] ACP updates — Agent Client Protocol, 2026-07-20 — https://agentclientprotocol.com/updates (primary)
[64] ACP governance; agents — https://agentclientprotocol.com/community/governance ; https://agentclientprotocol.com/overview/agents (primary)
[65] A2A specification 1.0 — https://a2a-protocol.org/latest/specification/ (primary)
[66] A2A joins AAIF — 2026-08-17 — https://aaif.io/blog/a2a-joins-aaif (primary)
[67] ccusage blocks reports — https://ccusage.com/guide/blocks-reports (primary)
[68] Claude-Code-Usage-Monitor README — https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor (primary)
[69] CodexBar docs/claude.md — steipete — https://raw.githubusercontent.com/steipete/CodexBar/main/docs/claude.md (primary)
[70] /api/oauth/usage endpoint aggressively rate limits (#31637) — anthropics/claude-code, 2026-03-06, closed 2026-06-01 — https://github.com/anthropics/claude-code/issues/31637 (primary)
[71] How do usage and length limits work? — Anthropic Support, read 2026-10-14 — https://support.claude.com/en/articles/11647753-how-do-usage-and-length-limits-work (primary)
[72] How Do AI Agents Spend Your Money? — Bai et al., arXiv 2604.22750, 2026-04 — https://arxiv.org/abs/2604.22750 (primary)
[73] Time Horizon 1.1 — METR, 2026-01-29 — https://metr.org/blog/2026-1-29-time-horizon-1-1/ (primary)
[74] Evidence Based Scheduling — Joel Spolsky, 2007-10-26 — https://www.joelonsoftware.com/2007/10/26/evidence-based-scheduling/ (primary; old, method only)
[75] API errors — Anthropic — https://platform.claude.com/docs/en/api/errors (primary)
[76] Detecting Activity failures — Temporal — https://docs.temporal.io/encyclopedia/detecting-activity-failures (primary)
[77] fix: widen DefaultLeaseTTL from 5min to 4h (#5470) — gastownhall/beads, 2026-08-09 — https://github.com/gastownhall/beads/pull/5470 (primary)
[78] What OpenSpec and similar tools do, and where Muster stands — Muster, 2026-10-13 — docs/research/2026-10-13-agent-task-tools.md (internal)
[79] Higher usage limits and a SpaceX compute deal — Anthropic, 2026-05-06 — https://www.anthropic.com/news/higher-limits-spacex (primary)
[80] Codex MCP server removal — OpenAI, undated — https://learn.chatgpt.com/docs/mcp-server (primary)
