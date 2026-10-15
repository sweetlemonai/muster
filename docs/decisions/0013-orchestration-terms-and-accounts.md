# 0013 — Orchestration's terms: each account pinned to projects by hand, no balancing across accounts, usage read not guessed, each user brings their own login

Date: 2026-10-14 · Status: accepted,
decided by the coordinator for the owner, 2026-10-14, overrule any time (Muster_Team_Lead on mu-107, from item 0.1 of `docs/plans/orchestration-plan.md`;
the owner's standing rule is that Muster stays sellable and carries no terms risk). Replaces the
"Load balancing" section of the owner's orchestration brief
(`tasks/proposals/Muster orchestration design brief.md`, 2026-10-14). Builds on 0002 (the owner's
own `claude`) and 0012 (only the personal accounts are read).

## Why

The brief asks Muster to run a team across "one or more Claude accounts and other providers", and
to spend "the account whose window resets soonest while it still has budget left". The research
(`docs/research/2026-10-14-orchestration.md` §1, read 2026-10-14) found no clause that bans
holding two accounts, but several that aim at exactly what that rule is for: getting past one
account's limits. A product sold to others can't pool accounts at all. So the dispatcher and the
account profiles (mu-113, mu-116, mu-118, mu-121, mu-122) need fixed rules before they're built.
These six are those rules; every later mu-9 task is built and vision-checked against them.

The bracketed numbers are the research's source list ([1] Anthropic Consumer Terms, [2] Claude
Code docs "Legal and compliance", [3] Agent SDK overview, [4] Anthropic Usage Policy, [9]
TechCrunch on the July 2025 weekly limits, [16] Claude Code background sessions, [22] Codex
authentication, [29] Codex changelog, [31] OpenAI Terms of Use).

## Decision

### Rule 1: Muster runs the owner's own client and never touches a login token

1. Muster starts the owner's own, unmodified `claude` and `codex`, signed in by the owner; it never
   reads, stores or passes on login tokens and never calls a provider's API with a subscription
   token, as 0002 already does for the helper.

It rests on:
- "OAuth authentication is intended exclusively for purchasers of Claude Free, Pro, Max, Team, and
  Enterprise subscription plans and is designed to support ordinary use of Claude Code and other
  native Anthropic applications." (Legal and compliance, undated, read 2026-10-14 [2])
- "developers may not collect, store, or intermediate Claude.ai credentials or session tokens"
  (Legal and compliance, undated, read 2026-10-14 [2]; the same in the Agent SDK overview, read
  2026-10-14 [3])
- ChatGPT sign-in tokens "are intended for trusted scripts, schedulers, and private CI runners", and
  a ChatGPT `auth.json` must not be shared "across concurrent jobs or multiple machines" (Codex
  authentication, undated, read 2026-10-14 [22]). One Mac, the owner's own login, fits that.

### Rule 2: no automatic choice among accounts to get past limits

2. Muster never picks automatically among several accounts of one provider to get past limits: a
   limit hit puts that account in cooldown until its `resets_at`, and the work waits; it never
   moves to another account.

It rests on:
- "bypassing any of our systems or protective measures" is forbidden (Consumer Terms §3, effective
  2025-10-08 [1]). Usage limits are a protective measure, and rotating accounts past one is the
  plainest way to read as bypassing it.
- "Coordinate malicious activity across multiple accounts, or create multiple accounts, to avoid
  detection, circumvent product guardrails, …" (Usage Policy, effective 2026-11-12, published
  2026-10-08 [4]; "or create multiple accounts" is new against the 2025-09-15 version in force
  today).
- "circumventing any rate limits or restrictions" is forbidden (OpenAI Terms of Use, Europe,
  updated 2026-01-16, read from the Open Terms Archive copy [31]).

### Rule 3: the company account is never used and never read

3. The company account (`~/.claude`, a bare `claude`) is never used to run work and never read;
   Muster uses only the personal accounts on the allowlist of decision 0012 (mu-99).

It rests on the owner's rule, not a provider clause: "Muster under `~/playground` uses only the
personal accounts cc-mg and cc-hg; `~/.claude` and a bare `claude` are the company account and are
never read" (the owner on mu-99, 2026-10-14, recorded in decision 0012). The terms point the same
way: "You may not share your Account login information, Anthropic API key, or Account credentials
with anyone else." (Consumer Terms, effective 2025-10-08 [1]) A work account isn't the owner's to
lend to a personal tool.

### Rule 4: every run is capped and paced, so use stays one person's

4. Every run has a turn cap and a budget cap, and the dispatcher paces runs against the owner's
   reserve and the weekly window, so an account's use stays close to one person's.

It rests on:
- "Advertised usage limits for Pro and Max plans assume ordinary, individual usage of Claude Code
  and the Agent SDK." (Legal and compliance, undated, read 2026-10-14 [2])
- Background sessions consume quota "the same as interactive sessions, so running ten agents in
  parallel uses quota roughly ten times as fast" (Agent view and background sessions, read
  2026-10-14 [16]).
- The July 2025 weekly limits were introduced against use "continuously in the background, 24/7"
  (TechCrunch, 2025-07-28 [9]).

### Rule 5: sellable shape, each user brings their own login or API key

5. A sold Muster has each user sign in with their own login or API key; Muster has no pooling
   feature, not even one that's off by default.

It rests on:
- "Customers may not pay for, resell, or intermediate Claude usage on their end users' behalf. Each
  end user must authenticate with their own Anthropic API key, Claude subscription plan
  credentials, or 3P inference provider credential" (Legal and compliance, undated, read 2026-10-14
  [2]).
- "Anthropic does not permit third-party developers to offer Claude.ai login into their own
  applications, or to route requests through Free, Pro, or Max plan credentials on behalf of their
  users." (Legal and compliance, undated, read 2026-10-14 [2])
- "You may not share your account credentials or make your account available to anyone else"
  (OpenAI Terms of Use, Europe, updated 2026-01-16 [31]).

### Rule 6: telemetry stays off

6. Muster never exports `CLAUDE_CODE_ENABLE_TELEMETRY`, and the Codex adapter turns off the app
   server's analytics.

It rests on the owner's no-telemetry rule (`baseline-privacy.md`: nothing sent to third parties)
and on Codex 0.162.0's changelog: "Enable analytics by default for daemon-launched app servers"
(ChatGPT & Codex changelog, 2026-10-07/08 [29]). Claude Code's OpenTelemetry export is off unless
`CLAUDE_CODE_ENABLE_TELEMETRY=1` (Monitoring, read 2026-10-14 [19]); the run ledger takes its
numbers from the local result message and transcripts instead.

### Capacity is read, then learned per run

The brief's "capacity is learned from limit hits" becomes **read, then learned per run**. Claude
Code reports each account's 5-hour and weekly use and its reset time: the stream's
`rate_limit_event`, whose nested `rate_limit_info` carries the status, the reset time and each
window's use (the research names them `resets_at` and `utilization`; mu-111's spike found them in
camelCase on the wire: `resetsAt`, `rateLimitType`, and per-window `utilization` under
`unifiedWindows`), and the status line's `rate_limits` (`used_percentage`, `resets_at`); Codex's
app server reports `usedPercent` and `resetsAt`. The dispatcher reads those, and the ledger learns
what a run costs as a share of the window. A limit hit is never a probe to learn from.

## Options considered

- *The brief's soonest-reset-first balancing across accounts:* spends budget before it expires at
  each reset and gets the most work out of several accounts. Rejected: it exists to get past
  per-account limits, which is what "bypassing … protective measures" [1], "ordinary, individual
  usage" [2] and, from 2026-11-12, "create multiple accounts, to … circumvent product guardrails"
  [4] aim at. Gas Town's `quota_dog` rotates Claude accounts when one hits its limit (1.2.0,
  2026-05-27): the same pooling, and the pattern to avoid. A sold Muster couldn't offer it at all
  [2][3].
- *One account only, no second account at all:* the simplest reading of the terms, nothing to
  explain in Settings. Rejected: the owner already holds two personal accounts and pins work to
  them by hand today; no clause bans holding them, and the research rates a second account chosen
  by hand for a project low to medium risk.
- *Chosen: each account pinned to projects by hand.* The owner may pin a project to a second
  account they hold (plan item 3.2, mu-122); the dispatcher uses each account only for its pinned
  projects, and a limit on one never moves work to another. More capacity comes from a bigger
  plan, a Team plan or an API key, which the Commercial Terms allow for powering products.

## What would make it worth revisiting

- A provider publishes a clause that allows, or plainly forbids, one person using several personal
  accounts. Today no clause speaks to it, so the risk reading is the research's, not a lawyer's.
- The Usage Policy of 2026-11-12 changes before or after it takes effect.
- Muster moves to API keys or a Team plan, where capacity is bought rather than rationed: rules 2
  and 4 then become a budget question.
- The owner overrules this record; it was decided for them and stands only until they say otherwise.
