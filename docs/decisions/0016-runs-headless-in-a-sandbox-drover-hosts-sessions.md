# 0016 — Runs are headless and sandboxed with full access inside; Drover is where the owner watches and takes them over

Date: 2026-10-14 · Status: accepted, decided by the session for the owner, 2026-10-14, overrule
any time (the owner asked for the spike and its tasks: "do a deep spike about the whole thing and
create tasks in the pipeline"). Amends 0015 (point 4: Drover doesn't get a second runner) and
0007 (the Code row: Drover hosts the sessions; the runs start from Muster's core until the
screens move into Drover). Builds on 0002 (the owner's own `claude`, scrubbed), 0013 (terms) and 0001
(Muster's screens move into Drover later). Evidence: `docs/research/2026-10-14-session-host-and-sandbox.md`.

## Why

Muster is now the whole pipeline: idea, task, build, test, merge. The owner does ideas and QA,
and only now and then looks into a session or answers one. Today sessions run by hand in Orca,
iTerm and Warp. The owner wants runs in worktrees and sandboxes, so the agents can have full
access. Decision 0015 left two runners to build: Muster's `claude -p` runner, and Drover's
replacing it later. Drover task 0018 had Drover start PTY agents from assignment files. The spike
found that a PTY runner would have to read state, usage and rate limits off the screen. Headless
runs report them typed, and a headless run can be reopened live with `claude --resume`.

## Decision

1. **One runner, in Muster's core.** Runs are headless `claude -p --output-format stream-json`
   in the task's worktree (mu-117). Drover doesn't build a second runner. When Muster's screens
   move into Drover (0001), the core moves with them, and the runs then start from Drover's process.
2. **Every run is wrapped in its own Seatbelt profile** (`sandbox-exec -f <profile> claude …`),
   generated per run by Muster's core (mu-175):
   - Writes are allowed only to the worktree, what a commit on the task's own branch needs in the
     main checkout's `.git`, the queue's `.muster/`, the account's `CLAUDE_CONFIG_DIR`, Claude's
     temp files (`/private/tmp/claude-<uid>`, `/private/tmp/claude-<hex>-cwd`), the user temp and
     cache folders, and the toolchain caches (`~/.cargo`, `~/.rustup`, `~/.npm`, pnpm, SwiftPM,
     DerivedData, `~/.cache`).
   - Reads are denied for secrets (`~/.ssh`, `~/.gnupg`, `~/.aws`, the other accounts' config
     folders and the company account's `~/.claude`), `~/code`, and every other project under
     `~/playground`.
   - The network goes only to a local allowlist proxy (mu-176).
   - The run passes `--settings` that turn Claude Code's own sandbox off (it can't nest inside
     Seatbelt) and tells builds to use `swift build --disable-sandbox` and
     `OTHER_SWIFT_FLAGS=-disable-sandbox`.
3. **Full access inside the sandbox.** A run in the profile starts with
   `--dangerously-skip-permissions`, or with its role's narrower permission profile (mu-112) when
   that is read-only. **No run gets full access without the wrap.** If the wrap can't be applied,
   the start is refused (fail closed). The personal layer's guard hooks still run as a second line.
4. **Runs don't push.** The sandbox denies the SSH key, so `git push` fails inside it. The
   dispatcher merges and pushes outside the run after Testing passes (mu-177).
5. **Drover is the session host** (Drover tasks dr-0032 to dr-0035):
   - it lists every run with its state and shows its transcript live, read-only;
   - **Open live** asks Muster to stop the headless run (the claim stays, the run ends
     `cancelled` with the reason `opened live`), then opens `claude --resume <session>` in a lane on
     the task's own worktree, inside the same profile, with the same permission mode;
   - closing that terminal **hands it back**: the dispatcher resumes the run headless from the
     same conversation (mu-178).
   Without Drover, `muster run open` opens the same sandboxed resume in Terminal. The two apps talk
   through a run record (a versioned JSON per run, in `PROTOCOL.md`) and Drover's
   `drover session open` command (mu-127).
6. **Orca, iTerm and Warp leave the pipeline.** Pipeline sessions start only from the dispatcher.
   The owner's own ad-hoc sessions can still run in Drover lanes, sandboxed by choice (dr-0035).

## Options considered

- *Drover runs PTY agents for assignments (Drover 0018).* The owner sees a real terminal at once.
  But states, usage, cost and rate limits would have to come from screen reading and transcripts,
  which Drover can't do for limits today. Two runners would need to pass the same suite, and the
  build is about 5-6 days in Drover. Rejected, but its intent (sessions live in Drover) is kept
  by opening runs live in Drover.
- *`claude --bg` sessions and `claude attach`.* These are Claude Code's own. But they can't run
  with `-p`, their states are coarse, they report no usage, and they depend on feature flags.
  Rejected.
- *Claude Code's own sandbox plus bypass mode.* It's one setting. But it covers Bash only, and the
  docs say it isn't enough for unattended runs. Rejected.
- *Anthropic's sandbox runtime (`srt`) as the wrapper.* It's documented, and it has a network proxy.
  But it's a new Node dependency (Node 22.12 at run time) and a beta. Our profile is a page of
  SBPL, tested above. Rejected for now, revisit in mu-176 if our proxy falls short.
- *Containers or macOS VMs.* Containers run Linux and can't build macOS apps. VMs are capped at
  two per host and heavy per task. Rejected.
- *Chosen:* headless runs in a per-run Seatbelt profile with full access inside, and Drover as the
  place to watch runs and open them live.

## What would make it worth revisiting

- Apple removes `sandbox-exec`, or a Claude Code update breaks it under Seatbelt.
- Claude Code lets `-p` runs be attached to live, or `--bg` sessions report usage and limits.
- A tool the projects need can't run in the profile even with the documented flags.
- The owner wants every pipeline session as a live terminal from the start.

## What changes

- 0015 point 4 and 0007's Code row get a line saying they're amended by this record.
- `docs/VISION.md` and `docs/PIPELINE.md` say where runs start and where they're watched.
- mu-127 becomes Muster's side of the Drover contract (the run record, opening live and handback).
  Drover task 0018 is superseded by dr-0032 to dr-0035.

## Owner's answers, 2026-10-14

- **Accepted** (mu-127 V1): "Muster is going to be part of Drover later on, but yes it starts its
  own session there, in drover we can start random sessions too." Muster's runner starts its runs;
  Drover hosts them, and the owner also starts ad-hoc sessions in Drover outside Muster. Drover
  task 0018 is closed as superseded.
- **Runs start through the owner's launcher, not bare `claude -p`:** "do not do claude -p do
  cc-hg or cc-hg-new". Point 1's command runs through `cc-hg-new` / `cc-mg-new` (the zsh
  functions that hand Claude the account's login), not `claude` with only `CLAUDE_CONFIG_DIR`.
  Muster still never reads or stores a token (0013 rule 1): the launcher does. mu-117 and mu-179
  went back to Shaping to reshape the start command, the token scrub and the account check;
  mu-175 and mu-178 have the rule as a comment.
