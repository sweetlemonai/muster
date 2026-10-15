# Spike: Muster and Drover as one pipeline, with runs sandboxed and given full access

Date: 2026-10-14 · Asked by the owner: "muster is turning from a task manager to a whole coding
pipeline, what we are missing now is the agent/session management … how to combine the two
apps?" and "if we can sandbox them and worktree them we can give them dangerously full access".
The owner's part is ideas and QA. Now and then they look into a session or answer it.

Result: decision [0016](../decisions/0016-runs-headless-in-a-sandbox-drover-hosts-sessions.md).
Tasks: mu-175 to mu-179 (Muster), mu-127 reshaped, dr-0032 to dr-0035 (Drover).

## The answer in short

1. **One runner, in Muster's core.** It runs headless `claude -p --output-format stream-json` in
   the task's worktree (mu-117). Headless gives typed end states, usage, turns, cost and the
   `rate_limit_event`. A PTY would have to read all of that off the screen. Drover reads only the
   status line and transcripts today, and can't tell that a run ended on a limit.
2. **Drover is where the owner sees and steps into sessions.** It lists every run, shows its
   transcript live, and **opens a run live**: Muster stops the headless run (the claim stays),
   and Drover opens `claude --resume <session>` in a lane on the same worktree, inside the same
   sandbox, with the same permission mode. When the owner closes it, the run goes back to the
   dispatcher, which carries on headless from the same conversation.
3. **Every run is in its own Seatbelt sandbox, and inside it the agent has full access**
   (`--dangerously-skip-permissions`). The sandbox wraps the whole `claude` process. Claude
   Code's own sandbox covers only Bash, so it isn't enough.
4. **One app later.** Muster's screens move into Drover (decision 0001's plan). Muster's core,
   runner included, then runs inside Drover's process.
5. Orca, iTerm and Warp drop out of the pipeline. Nothing starts a pipeline session by hand.

## What was tested on this Mac (macOS 26.5, Claude Code installed, 2026-10-14)

All runs were outside the session's own sandbox, in `/private/tmp/claude-501/sbx`, on a throwaway
git repo. The profile was `(allow default)` with `(deny file-write*)`, then writes allowed only to:
the worktree, `CLAUDE_CONFIG_DIR`, `/private/tmp/claude-501`, `^/private/tmp/claude-[0-9a-f]+-cwd$`,
`/private/var/folders`, `~/.cache`, `~/.cargo`, `~/Library/Caches`, the ttys and `/dev/null`.
Reads were denied for a decoy secret folder, `~/code` and `~/playground/drover`.

| Probe | Result |
|---|---|
| `claude -p --dangerously-skip-permissions` (cc-mg, Haiku) under the profile | Ran. The login came from the Keychain without extra rules (`allow default` keeps mach-lookup) |
| Bash `echo ok > inside.txt` in the worktree | Written |
| Bash write outside the worktree | `operation not permitted` |
| **Write tool** outside the worktree | `EPERM`. The profile covers Claude's own file tools, not just Bash |
| Read of the decoy secret folder, `ls ~/code`, `ls ~/playground/drover` | All `Operation not permitted` |
| Bash without `/private/tmp/claude-<uid>` writable | Bash tool can't start (`EPERM: mkdir …`) |
| Bash without `/private/tmp/claude-<hex>-cwd` writable | Commands run, but each ends with exit 1 from the cwd marker |
| `swift build` under the profile | Fails: SwiftPM sandboxes its manifest, and `sandbox_apply: Operation not permitted` |
| `swift build --disable-sandbox` under the profile | Builds and runs (7 s) |
| `cargo build` under the profile (`~/.cargo` writable) | Builds and runs |
| `sandbox-exec` inside our `(allow default)` profile | Works. Inside Claude Code's own Bash sandbox it fails (`sandbox_apply: Operation not permitted`) |
| `touch ~/.npmrc-probe` | Denied. Home stays read-only |
| `claude -p` with only `CLAUDE_CONFIG_DIR=~/.claude-hg`, no sandbox | **Not logged in.** The `cc-hg` launcher passes a login token from outside; the folder holds no login. `~/.claude-mg` has one and works |

Not tested: network limits (needs a proxy, below), `xcodebuild`, pnpm and Tauri bundle builds,
`claude --resume` of a sandboxed run in a PTY, git commits that write into the main checkout's
`.git` from a linked worktree.

## Evidence from the web (2026-10-14)

Sandboxing (sources in the agent reports; primary unless marked):

- Claude Code's docs: "Always run `--dangerously-skip-permissions` sessions inside a container, a
  VM, or the sandbox runtime, so that file tools, MCP servers, and hooks are also inside the
  boundary." And: "The sandbox covers shell commands only. Claude's file tools, MCP servers, and
  hooks run outside it." (code.claude.com/docs/en/sandbox-environments, /sandboxing)
- Anthropic's sandbox runtime (`srt`, Apache-2.0, v0.0.79 of 2026-10-07, "Beta Research
  Preview") wraps any process tree with `sandbox-exec` on macOS. Writes are denied by default,
  and the network goes through a localhost proxy with a domain allowlist. Its macOS profile
  always allows `com.apple.SecurityServer` and `com.apple.securityd.xpc`, which is how the Keychain
  login keeps working. It needs Node 22.12.
- `sandbox-exec` is deprecated but works on macOS 26. Codex, Bazel and Chromium use Seatbelt.
  Sandboxes don't nest (Bazel's docs). SwiftPM needs `--disable-sandbox` inside one, and
  `xcodebuild` needs `OTHER_SWIFT_FLAGS=-disable-sandbox` (community report, SwiftPM's own
  issue closed fixed).
- A `claude` sandbox without Keychain IPC says "Not logged in" (a community PR). This matches what
  we'd expect from a deny-default profile. Ours is allow-default, so it's fine.
- Containers (Docker, Apple's `container`) run Linux and can't build macOS apps. macOS VMs (Tart,
  now FSL-licensed under OpenAI) are capped at two per host by Apple's licence. Neither fits.
- Dev containers don't stop exfiltration under bypass mode, including `~/.claude` credentials
  (Claude Code's devcontainer page). A network allowlist is the part that limits that.

Sessions:

- `claude --bg` and `claude agents` (agent view) run interactive sessions you can attach to, but
  `--bg` can't be combined with `-p` (agent-view docs). A `-p` run can't be attached to live.
- "You can still resume one by passing its session ID to `claude --resume <session-id>`" (sessions
  docs). A bypass mode isn't restored on resume; it has to be passed again at launch.
- `--input-format stream-json` keeps a `-p` run alive for follow-up messages (Agent SDK docs).
- `--permission-mode` accepts `auto`, `dontAsk` and `bypassPermissions`. `--permission-prompts none`
  denies anything that would prompt (CLI reference, headless docs).
- Other tools: Conductor, Superset, Orca and Jean run agents on the host with no OS sandbox.
  Sculptor (Docker), Cursor (microVM) and Codex (Seatbelt) contain them. vibe-kanban drives
  headless `-p` with stream-json both ways. Claude Squad drives tmux PTYs by reading the screen.
  Most kanban tools ship PostHog analytics.

## Code read (2026-10-14)

Muster (`app/src-tauri/src`):

- No `runner.rs`, `dispatch.rs` or `ledger.rs` yet. mu-117 (runner), mu-118 (dispatcher On) and
  mu-127 (Drover's runner) are Ready. `helper.rs` already runs `claude -p` with the 0002 scrub.
  `agents.rs` reads `claude agents --json`. `parallel.rs` `start` makes `task/<key>-<slug>`
  worktrees, and `merge` rebases, gates, fast-forwards main and **pushes** from the session that
  passes Testing.
- No task covers a per-run sandbox, full access, or opening a run live. mu-120's "Open session"
  goes to mu-98's recap, not a terminal.
- The personal layer gives every worktree session Claude Code's own sandbox
  (`sandbox.enabled: true`, `failIfUnavailable: true` in `settings.local.json`). Inside a Seatbelt
  wrap that sandbox can't start, so a run must pass `--settings` with it off. CLI settings outrank
  local settings.

Drover (read from `origin/main` a990fc8; the `drover/main` checkout is 44 commits behind):

- Every agent is interactive in a PTY (`pty.rs`, `$SHELL -l -i -c …`). Drover never runs `-p`.
  `--resume <id>` works in a lane whose folder matches the session's cwd (`lib.rs` `term_open`).
- No way in from outside: the binary has `mcp`, `guard` and `budget` subcommands and drop folders
  (`hook-events/`, `mcp-events/`) drained on the 2 s tick. No socket, URL scheme or single-instance plugin.
- `import_worktrees` adopts every worktree as `imported`. But `delete_lane` deletes the branch of an
  imported lane too, which would destroy a Muster `task/…` branch.
- Agents inherit Drover's own environment apart from the credential variables.
- `guard.rs` is a PreToolUse check that asks and calls itself "not a sandbox". A Seatbelt wrap
  slots into `pty.rs` `Terminals::spawn`.
- Drover task 0018 ("run agents for Muster's assignments") is blocked waiting for Muster's assignment format.

## Why headless runs opened live on demand, not live PTYs

| | Headless `-p`, Drover opens live on demand (chosen) | Live PTY in Drover from the start | `claude --bg` sessions |
|---|---|---|---|
| End states, usage, cost, limits | Typed, from the stream (mu-111's fixtures) | Read off the screen and transcripts; limits not detected | Agent view states only; no usage |
| The owner looks in | Drover shows the transcript live | Terminal | `claude attach` |
| The owner answers | Answer hook mid-run; open live for anything longer | Type into it | Attach |
| Two writers on one session | Never: the run stops before it opens live | n/a | n/a |
| Work to build | mu-117 (planned) plus opening live | Drover items 1-9 of the code read, about 5-6 days | Not combinable with `-p`; feature-flag dependent |

## Network

A Seatbelt profile can limit the network to one localhost port, but the allowlist itself needs a
proxy. Options: `srt` (a new Node dependency, Node 22.12 at run time, beta) or a small
proxy in Muster's core with the standard library only (HTTP CONNECT, exact host matches). Decided
in mu-176: our own. No new dependency, and it's the same proxy for both runners.

## Risks and open points

- `sandbox-exec` is deprecated. If Apple removes it, `srt` and Codex break too, and the fallback is
  macOS VMs (two per host).
- Allow-default profiles leave mach services, process exec and reads outside the deny list open.
  The deny list (secrets, `~/code`, other projects) has to be kept up to date. A deny-default
  profile is stricter but fragile across tools.
- A linked worktree commits into the main checkout's `.git` (objects, its `worktrees/<name>`, the
  branch's ref). Granting all of `.git` lets a run move `main`. mu-175 grants only what a commit
  on the task's branch needs, and tests it.
- `cc-hg` has no login of its own in `~/.claude-hg`. Its runs fail "Not logged in" until the
  owner logs that folder in once (mu-179).
