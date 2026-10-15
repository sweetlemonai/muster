# Gates: the proof that replaces your check

2026-10-14 · status: accepted (decision 0014) · feature mu-129, tasks mu-130 to mu-141.

The owner, 2026-10-14: "what I need here is a set of gates to ensure the quality of the products:
code review, QA, visual QA (very important + taking videos and screenshots) and compare to the
source or presentation for the user and such. This is the part I am the bottleneck now, need to
remove myself from the flow."

**Visual QA is the owner's proposal:** `.muster/proposal/Proposal Visual QA and Walkthrough QA in
Muster.md` (Oct 14, 2026). It is the design for G4 below and its build plan is the order of
mu-133 to mu-140. This page adds only what the proposal leaves to the rest of Muster: the other
gates, the records, when a task is accepted without the owner, and defaults for its open questions.
Where this page and the proposal differ, the proposal wins.

This is the **Gate** part of `docs/PIPELINE.md` (decision 0006). Muster stores the results,
refuses moves without them and shows them. Reusable pieces go in the personal layer.

## The ladder

A task leaves Testing only when every gate it needs is green **at the branch's tip**. Each gate
writes one record. Muster checks the record's shape and its `head`, and stores it (mu-131).

| # | Gate | Proves | Who | Needed when | Today |
|---|---|---|---|---|---|
| G1 | Build | format, lint, types, tests, build are green | the builder, then the merge | always | built: `bin/gate`, tree-stamped |
| G2 | Code review | no verified defect is left | Kia with lens subagents (`/deep-review`) | always | built: `muster review` (0005); started by hand until the dispatcher (mu-119) |
| G3 | Criteria | each criterion is **met**, with evidence that would fail if it broke | Kia, through `ac-verifier` | always | not built (mu-132) |
| G4 | Visual QA | every touched screen state matches its reference or its approved baseline; screens with neither have been walked through | Kia (reference QA) and the owner (walkthrough), per the proposal | the task touches UI (`qa:` set by the shaper) | not built (mu-133 to mu-140) |

## G3, criteria

Braindump's `ac-verifier`, promoted to the personal layer, gives each criterion a verdict: **met**,
**partial**, **unmet** or **unproven** (looks done, but nothing would catch it breaking). Only
*met* passes. Muster ticks the criteria from its record, so nobody ticks them by hand.

A criterion that only the owner can check is a sign the criterion is badly written. At shaping it
becomes something provable. If it really is a judgment about the screen, it becomes a step in a
walkthrough. 0028's owner-only criteria stay for old tasks.

## G4, visual QA (the proposal, in short)

- **Reference QA:** Kia captures each touched screen state, then compares it three ways. Against
  the approved baseline it uses a pixel diff with masks. Against a reference (a mockup, or a
  reference app at the same route) it uses a vision judgment. Rule checks cover contrast,
  truncation, overflow, dark mode, tap targets and the project's design rules. Each flagged screen
  gets one composite image (reference · actual · diff). Video is recorded only for motion and
  flows. Only the differences reach the owner.
- **Walkthrough QA:** used when a screen has no reference, or when the owner wants to look. Kia
  writes the script from the criteria. The app runs in a visible browser with a step overlay, in
  Auto, Guided or Mixed mode. The owner gives a verdict on each check (pass, fail, comment), and
  can pick an element or drag a box to point at something. Each check saves a screenshot, the
  selector and the logs. Passed checks become baselines and their steps become scripts. Each run is
  kept as `qa/run-N`.
- **They feed each other:** each walkthrough leaves baselines, scripts and rules behind. Two
  identical passes make a check automatic, and a repeated comment becomes a proposed rule. A fully
  automatic walkthrough becomes a Playwright test in the repo's gate.
- **The screen map**, one per project, lists every screen and state with how to reach it, its
  reference and its baseline. It grows from walkthroughs and tasks.
- **Definition of Done for UI tasks:** every touched screen state can be reached by a script and
  has an approved screenshot.
- **Platforms:** web first, which covers Muster, since its frontend runs in Playwright with the
  Tauri IPC mocked. iOS comes second and Mac last (mu-130 checks the drivers).

The walkthrough overlay is injected into the app under test, in its own browser. It is not part of
Muster's UI, so Muster's no-overlay rule (DESIGN.md) doesn't apply to it. Muster's own screens show
walkthrough runs and evidence in place, as always.

**Headless for agents:** everything an agent runs (captures, reference QA, Auto replays) runs in
headless Playwright WebKit with the IPC mocked, and never clicks, types or raises a window on the
owner's screen. A visible browser opens only for a walkthrough the owner starts, because the owner
is the one driving it. Native-only checks are screenshots of a window that's already open, with
no input.

**Fixtures:** captures and walkthroughs run on synthetic fixture data (a fixture queue for Muster),
with `HOME` and the app's config in `$TMPDIR`. Never the owner's own data, because screenshots get
shown and kept.

## Accepted by gates (mu-137)

A merged task goes to Done **without the owner** when all of these hold:

- G1 to G3 are green at the tip;
- for a UI task, every touched screen state has an approved baseline or a reference, and reference
  QA found only expected differences;
- no question and no vision conflict is open.

If not, it goes to Your check with the reason, and the card says what's needed. For example:
"3 screens changed: 2 expected, 1 not", or "2 screens have no baseline: Start walkthrough".

So the owner is needed only for screens nothing has approved yet (the first walkthrough of a new
screen) and for differences Kia can't judge. Both shrink with every walkthrough, as the proposal
intends. The owner can reopen any accepted task from its card. A reopen files a linked bug and is
recorded as an **escape** against the gate that missed it. Two escapes of one kind propose a
lesson, which needs the owner's yes (as in 0005). A project can set `accept = "owner"` to keep
today's flow.

## The proposal's open questions: defaults until the owner answers

| Question | Default | Why |
|---|---|---|
| Where baselines live | in the project's repo, `qa/baselines/<screen>/<state>-<viewport>-<theme>.png`, beside the screen map (`qa/screens.toml`) and the run scripts | they become the repo's Playwright tests (stage 5), and most queues are local and not in git; the screen map prunes them |
| Lovable at work: preview URLs and login | **the owner's call (asked):** Muster lives under `~/playground` and work material doesn't come here. The reference app is a generic "reference URL" source; pointing it at Lovable happens from the work side | the boundary in `~/playground/CLAUDE.md` |
| Identical passes before a check is automatic | 2 | the proposal's lower bound; any later diff still flags |
| UI tasks default to `qa: walkthrough` until a screen has a baseline | yes | there's nothing to compare against before then |
| Overlay: injected script or browser extension | injected script | simplest, no extension to install; revisit for pages the owner opens by hand |

## What's left out

- Hosted visual testing (Percy, Chromatic, Applitools). Screenshots would leave the Mac.
- A separate visual QA role. Kia does it, as the proposal says, with Dariush, Azadeh and Kaveh
  taking the findings.
