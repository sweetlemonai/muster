# 0014 — Gates replace your check: a task that passes every gate it needs is accepted without the owner

Date: 2026-10-14 · Status: accepted. The owner asked for it ("a set of gates … code review, QA,
visual QA (very important + taking videos and screenshots) and compare to the source or
presentation … need to remove myself from the flow"). Visual QA follows the owner's proposal
(`.muster/proposal/Proposal Visual QA and Walkthrough QA in Muster.md`). The rest are the session's
defaults; the owner can overrule any of them. Builds on 0005 (review of record) and 0006 (one gate
kit in the personal layer). The design is in `docs/GATES.md`; the work is feature mu-129.

## Decision

1. **Four gates, each a record checked at the branch's tip:** build (G1), code review (G2, 0005
   unchanged), criteria through `ac-verifier` (G3), and visual QA (G4) as the proposal describes
   it. Reference QA runs when a screen has a reference or an approved baseline. Walkthrough QA runs
   when it has neither, or when the owner wants to look. G1–G3 always run; G4 runs when the shaper
   sets `qa:` on the task. Nothing is run or recorded by the task's builder (0047).
2. **Accepted by gates is the default.** A merged task goes to Done without the owner when its
   gates are green and every touched screen state matches an approved baseline or a reference with
   only expected differences. The owner sees these in Recently finished and can reopen any of them.
   A project can set `accept = "owner"` to keep today's flow.
3. **The owner's part shrinks.** The owner walks through screens nothing has approved yet, and
   differences Kia can't judge. Each walkthrough leaves baselines, scripts and rules behind, so the
   next one asks less (the proposal's learning loop).
4. **No new owner-only criteria.** A check an agent can't prove is rewritten as a provable one, or
   becomes a walkthrough step. 0028 stays for old tasks.
5. **Fixtures only.** Captures and walkthroughs run on synthetic data in a hermetic `HOME`, never on
   the owner's own queue or files, because screenshots are kept and shown.
6. **Kia does visual QA.** There is no separate role. Dariush, Azadeh and Kaveh take the findings.
7. **A reopen is an escape.** It is recorded against the gate that should have caught it. Two
   escapes of one kind propose a lesson, which needs the owner's yes (as in 0005).

The proposal's open questions get defaults in `docs/GATES.md`: baselines in the project's repo,
2 identical passes before a check is automatic, `qa: walkthrough` until a screen has a baseline,
and an injected overlay script. Lovable at work is asked of the owner, because work material
doesn't come under `~/playground`.

## Options considered

- **Keep Your check and make it faster** (Accept all and effort groups, which mu-43 and mu-78
  built). The owner still looks at every task, and that is the bottleneck.
- **Auto-accept on the review and the gate line alone.** Nothing proves the screens, and most of
  what waits in Your check is UI.
- **Fully automatic visual QA from the start, with main's capture as the baseline.** This was the
  first draft of this page. It needs no owner at all, but nothing says the screen is *right*, only
  that it changed. The proposal's owner-approved baselines anchor it, and walkthroughs make the
  approval cheap.
- **Hosted visual testing** (Percy, Chromatic, Applitools). These upload screenshots to a third
  party, against the privacy baseline.
- **A vision model as the judge.** It is inconsistent. Vision only flags; baselines and rule checks
  decide (the proposal's mitigation).

## Revisit when

- Escapes cluster on one gate.
- The judge's false alarms cost more rounds than the owner's checks did.
- mu-130's research finds a stack that can't be driven without the owner's ongoing help.
- The owner answers the proposal's open questions differently from the defaults.
