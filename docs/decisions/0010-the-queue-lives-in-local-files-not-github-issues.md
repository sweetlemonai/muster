# 0010 — The queue lives in local files, not GitHub Issues

Date: 2026-10-14 · Status: accepted (the owner: "I like it on the record"). Records a choice
Muster made from the start (0003, 0005) but never wrote down.

The owner asked: "what we did for muster doing it in local files, couldn't we have done it in
github issues?"

## Decision

A task is a markdown file in the project's queue (`.muster/` in the repo, or the older
`<project>/tasks/` with its own git), read and changed only through `muster`. GitHub Issues is not
the store. If GitHub is wanted later, it gets a **mirror**: a one-way publish of chosen tasks, or
PR state read back in, never the source of truth.

## Why

- **Claims must be atomic.** Muster's core is "one agent takes a task, nobody else can": `muster
  claim`/`start` under the queue's file lock, with a lease, heartbeat and reclaim. An issue has no
  compare-and-set: two agents can assign themselves in the same second and both succeed, so a lock
  service would have to be built beside it anyway.
- **Speed and limits.** The app reads about 800 tasks in 20 queues every 2 s and agents call
  `muster` constantly. Local reads cost next to nothing; through the API each is a network call,
  and an authenticated user gets 5,000 REST calls an hour, which a busy night of agents passes.
- **Token-frugal agents** (product principle): an agent reads a short file, not JSON from `gh`,
  and nothing fails when the network does.
- **Local first and private** (baseline privacy): task notes, review findings and the owner's
  answers stay on the Mac. `sweetlemonai/muster` is public, so its issues would be public too.
- **Next to the code.** Worktrees, the merge gate, the review of record (0005) and Your check run
  in the repo; with issues every stage change is an API round trip plus webhooks to stay in step.
- **Plain files outlive the tool.** Four tools in this space shut down in 2026
  (`docs/research/2026-10-13-agent-task-tools.md`); a folder of markdown with git can't be taken
  away.

## Options considered

- *GitHub Issues (and Projects) as the store:* free sync across machines, a phone view, links to
  PRs and commits, notifications and collaborators for nothing. Turned down for the reasons above:
  no atomic claim, rate limits, public by default here, and the owner's local-first rule.
- *Linear or another hosted tracker:* the same costs plus a vendor and its telemetry; its agent
  delegate model is worth copying (research doc), not adopting.
- *Local files, mirrored to GitHub later:* chosen shape. The mirror is not built.

## What it costs

No sync between Macs and no phone view; the board, Your check and notifications had to be built
(mu-1, mu-52) instead of borrowed.

## Revisit when

- Muster is sold to teams who already live in GitHub, or the owner works from a second Mac or the
  phone: then build the mirror (publish chosen tasks, read PR status back), still with the files as
  the truth.
- PRs per task come back (0005's "until PRs return"): reviews are posted there, and the task links
  its PR.
