# 0005 — The review of record runs in the Testing session; Muster records it and gates the merge

Date: 2026-10-13 · Status: accepted (the owner settled one review, on this Mac, part of Testing,
no PR per task for now, posting only as sweetlemonai; the rest is the builder's call for task
0027, overrule any time).

## Decision

- **Muster runs no agent for the review**, as with the vision check (0029). *Amended by
  [0015](0015-muster-runs-agents-behind-a-runner-interface.md) (2026-10-14):* Muster runs no
  agent for the review or the vision check; the dispatcher's runs are 0015's. `muster review <id>`
  prints a brief; the Testing session (never the builder, 0047) runs the owner's engine on the
  branch (`/deep-review <range>`: its lenses, verifiers and tier router) and hands the result back
  as JSON (`muster review <id> --record -`). Muster checks it (strict shape, `head` is the
  branch's tip, every answered finding answered), stores it and moves the task on.
- **Round 3 still open goes to the owner,** blocked on you, instead of a fourth round. Rounds 1
  and 2 return the task to its builder. A finding first raised in round 3 goes to the builder once
  (fix or dispute) before it reaches the owner (owner, 2026-10-13). Nits never hold anything.
- **The merge needs a clean review at the branch's tip.** A commit after the review needs a new
  round, of just that commit; the merge's own rebase carries the review to the replayed commits
  (the same commits, the gate runs after). The owner may merge without one, and the report says so.
- **The review lives in the queue,** `reviews/<id>.json` (the truth, written only by muster), with
  the task's `## Review` section rendered from it. Lessons: `reviews/lessons.json`, rendered as
  `lessons.md`, all in the queue's folder as `layout` says: `.muster/reviews/` and
  `.muster/lessons.md` in a repo's queue (kept out of git in a local one, like the rest), copied by
  `muster migrate`. Both move into the live state once task 0005's store exists.
- **Lessons need the owner's yes**, in one batch, since a reviewer wrote them and they become
  instructions; a kind must recur on two tasks to be proposed, and a rejected kind isn't proposed
  again.

## Confirmed by the owner (2026-10-13)

After the full context: **no auto-run for now** (the Testing session runs the review; starting it
automatically comes with 0009's dispatcher); **labels don't feed `/pr learn` yet** and Muster isn't
in the engine's repos until PRs return; round-3 findings go to the builder first. The learning
across the whole chain is designed in `docs/PIPELINE.md` ("Learning, both ways").

## Options considered

- *Muster launches the review itself* (`claude -p` on `pass` from Building): automatic, but a
  headless run of a multi-agent skill from inside a CLI call, with tokens spent and no session to
  see or stop it, and a second way of running agents beside Drover (0009). The tester session
  already exists (0047), so the review is its first step and the merge enforces it.
- *A GitHub PR and the `/pr` engine* (the task's first plan): the owner settled no PR per task for
  now, so the branch is reviewed locally. `compose-review.mjs` and `threads.mjs` come back with PRs.
- *Lessons in the repo's `docs/LESSONS.md` until 0005:* writing them on the main checkout leaves
  it dirty, and the merge refuses a dirty main. The queue folder is where task 0005 puts `.muster/`.
- *Findings parsed back from the task's Markdown* (as Vision does): findings carry more fields
  and a history, so a JSON file is the truth and the section is its rendering.
- *A fourth round, then the owner:* one more round of tokens on a disagreement two rounds didn't
  settle.

## Revisit when

PRs per task come back (post the review as sweetlemonai through `compose-review.mjs`, with a
login check before every post), 0009's dispatcher can start the review session itself, or the
engine's ledgers get a place for a task without a PR number (then the labels feed `/pr learn`).
