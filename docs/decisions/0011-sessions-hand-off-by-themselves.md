# 0011 — Sessions hand off by themselves at ~60% context or the token cap

Date: 2026-10-14 · Status: accepted (the owner, on mu-74: "Sessions hand off by themselves at
~60% context: finish the step, wait for a gap, write /handoff, a fresh session resumes it, no
human step", and "I really need it to make the whole process automated."). Amends 0004 point 3;
0004's points 1, 2, 4 and 5 stand.

## Why

0004 said "hand off at about 150k" and left the handoff to the session: notice the size, write a
note, have someone start a fresh session. In practice sessions noticed late or not at all, and the
switch needed the owner. mu-74 built the trigger and the switch into the personal layer
(`hooks/context-watch.mjs`, the `/handoff` skill, `bin/claude-continue`, `handoff.json`), so the
rule can say what the hook does.

A second gap came with it. A question asked with `muster ask` is routed to the asking session's
mailbox by session id, so a successor never saw the answers to its predecessor's questions
(DOGFOOD 35).

## Decision

1. **The trigger.** A session hands off when its context reaches **60%** of its window or its
   **token cap**, whichever comes first. The cap is 150k for every session and 600k for
   `Muster_Team_Lead*` (300k until the owner raised it, 2026-10-14) (`caps` in `~/playground/.claude/handoff.json`; other session-name patterns
   can get their own; the lead is named `Muster_Team_Lead`, then `_2`, `_3`…, per 0004 point 4).
   From 50%, or from 5/6 of the cap (125k for 150k) if that comes first, the hook reminds the
   session to finish its step.
2. **At a gap.** The hook acts at a Stop with nothing of the session's own running: no subagent,
   teammate, background shell or workflow. Until then it asks again at each stop.
3. **The handoff.** The session writes `/handoff` (every owner request with its status, decisions,
   failed approaches, changes, open questions). Its front matter carries `session:` (the old
   session id) and `generation:`. A builder holding a claim first runs `muster checkin` with
   what's done and next (`--handoff`, since task 0090), and the handoff names the claim and the
   open question ids. The
   handoff's first Next step is `muster inbox --session <old id>`.
4. **An automatic successor.** `claude-continue` starts a named background session with the same
   account and permission mode, which runs `/handoff resume`. No human step. It follows the
   handoff's Next steps, so its first command is `muster inbox --session <old id>` and answers that
   went to the old mailbox reach it. It keeps the old `--as` name.
5. **Off switches and fallbacks.** `PERSONAL_HANDOFF_AUTO=0` keeps the trigger but starts no
   successor; `PERSONAL_HANDOFF=0` turns the hook off. The hook asks for `/clear` instead of a
   successor in the desktop app (it can't open a background session without a click), when the
   permission mode is unknown, and after 5 automatic handoffs in a row with no owner prompt
   (`maxRotations`). `CLAUDE_HANDOFF_ACT_PCT`, `CLAUDE_HANDOFF_WARN_PCT`,
   `CLAUDE_HANDOFF_ACT_TOKENS` and `CLAUDE_HANDOFF_MAX_ROTATIONS` override the file for one session.
6. **What a handoff may hold.** No secrets or tokens: it names where a credential lives, never its
   value. The session id it carries isn't a secret. Handoffs can quote the owner, so they stay in
   the untracked `.claude/work/handoffs/` and are never committed or attached to a task.

## Options considered

- *Keep 0004's manual handoff at ~150k:* no new parts, but it relied on the session noticing and
  on the owner starting the next one, which is the babysitting the owner wants gone.
- *Let Claude Code's auto-compact do it:* no setup, but it summarizes from memory with nothing to
  check, drops owner requests and dead ends, and fires only when the context is nearly full, after
  the expensive turns.
- *A token cap only:* the same number means 75% of a 200k window and 15% of a 1M one, so it
  either fires too late on small windows or needs a number per model.
- *A percentage only:* on a 1M window 60% is 600k, and 0004's measurements say a turn costs about
  the context's size, so turns would cost four times what the 150k rule allowed.
- *Chosen:* 60% or the cap, whichever first, acting only at a gap, with an automatic successor.

## Cost

0004's numbers stand: a turn costs about the context's size (a coordinator at ~450k paid ~450k for
a one-line "merged, pushed"). With the cap, a builder's turn costs at most about 150k, a third of
that, and the lead's at most about 600k (the owner raised it from 300k on 2026-10-14, trading cost per turn for fewer lead handoffs).

## Built since (task 0090, 2026-10-14)

- **The claim carries over.** The handing-off session runs `muster checkin <id> "<what's done,
  what's next>" --handoff` (logged `check-in 40% · handing off · <text>`). The successor runs
  `muster claim <id> --takeover --as <the same name>`, which moves the claim to its session under
  the lock, renews the lease and logs `<new>: took over the claim from <old> (handed off)`. With
  no handoff after the latest claim or takeover, it's allowed once the claim has had no heartbeat
  for 30 minutes (`(quiet 42 min)`). The owner can always take over (`(owner's override)`).
  Another label is refused. The coordinator no longer needs owner overrides to move it.
- **A task's answers follow its claim.** On a takeover the old mailbox's mail for that task moves
  to the successor's, and `mail/asks.json` entries for its questions are repointed, so a later
  `muster answer` reaches the successor. Other tasks' mail stays in the old mailbox.
- **Builders stay builders.** Every holder in the chain (the claimer and each takeover's two
  sides) counts as the builder for the tester rule (0047) and the reviewer check.

## Still open

- **Nothing but the handoff tells the successor to read the old mailbox.** The `/handoff` skill,
  the hooks and `claude-continue` don't run `muster inbox`; a handoff written without that first
  Next step leaves the old answers unread (they're still in the task file). A takeover now brings
  the claimed task's mail along, so this is only about other tasks' answers, and the coordinator's
  own questions, which have no claim to follow.
