# Muster: as built

As of 2026-10-14, main at `72f34df`. What exists, how it fits together, and what's left: the
newest state, not history. History is in `docs/WORKLOG.md`, `docs/DOGFOOD.md` and each task's
log. Section 6 is generated from the queue; everything else is written by hand and checked against
the code.

1. [Muster in one page](#1-muster-in-one-page)
2. [Architecture](#2-architecture)
3. [How a task flows](#3-how-a-task-flows)
4. [Each part's state](#4-each-parts-state)
5. [Decisions](#5-decisions)
6. [The queue at a glance](#6-the-queue-at-a-glance) (generated)
7. [Known limits and what's left](#7-known-limits-and-whats-left)

Then [Parts in detail](#parts-in-detail): every feature as built, for when the tables above
aren't enough.

## 1. Muster in one page

Muster is a task system for coding agents, run by one owner on one Mac. The owner says what they
want; agents write it up as tasks, shape them, build them in their own worktrees, test them and
merge them; the owner sees only what really needs them, and checks the result at the end.

**A queue** is a folder of Markdown task files, one per task, plus a backlog of one-line notes.
Muster's own queue, and any repo set up with `muster init`, is the repo's `.muster/` (decision
0003, task 0005): files sit in `tasks/` and never move, and `stage:` says where each task is.
Older queues are `<project>/tasks` beside the repo, with their own git, where the subfolder mirrors
the stage. Muster's own moved out of that layout on 2026-10-13; the other 19 projects' queues are
still in it. Drover reads and answers its queue directly (file contract v1, its waiting list).

**Who uses it:**

- **The owner**, in the Muster app (board, task pane, Waiting for you, the Dock badge, Mac
  notifications) and with `! muster …` in any session. They answer questions, tick what only they
  can verify, and accept or send back finished work.
- **Agent sessions** (Claude Code), through the `muster` command, the `/mu` and `/muster` slash
  commands and the answer hook. A capture or lead session writes the owner's words up as tasks.
  Shapers make them ready, builders claim and build them, a separate QA session tests and merges
  them (decision 0004: one session per task).
- **Other tools** that read or write the same files: Drover (answers through file contract v1),
  `bin/manifest.py` and `bin/task`.

**The stages** (decision 0003, task 0014): Backlog → Shaping → Ready → Building → Testing → Your
check → Done. Each has a role: anyone, product + tech + UI agents, claimable, the code agent, the
QA agent, the owner. Blocked and paused are flags on a stage, not stages. Goals and features group
tasks; their stage rolls up from their children. The owner's own tasks (`for: human`) go To do →
Doing → Done on the same board.

**One set of rules:** the app and the command line are the same Rust binary. Every change goes
through `ops.rs` and its neighbours, under the queue's one lock, with the time stamped by the
tool and the change logged in the task.

## 2. Architecture

### The picture

```
                                   owner
                 ┌───────────────────┴────────────────────┐
                 ▼                                        ▼
   Muster.app: React webview                  Claude Code sessions
   (board, pane, Waiting for you)             (capture, shapers, builders, QA)
                 │ invoke: 71 Tauri commands      │ muster …, /mu, bin/task
                 ▼                                ▼
   ┌───────────────── one Rust crate (app/src-tauri) ──────────────────┐
   │  lib.rs (the app)                          cli.rs (`muster`)      │
   │  queue · ops · layout · index · block · backlog · lenses · key …  │
   │  every write: flock on mail/.lock, atomic rename, a log line      │
   └──────┬──────────────────┬──────────────────┬──────────────────────┘
          ▼                  ▼                  ▼
   the queue folder     this Mac's app folder   git (muster start, the merge)
   .muster/ or          projects.json           worktrees, rebase, bin/gate,
   <project>/tasks      settings.json           fast-forward main, push
     tasks/ backlog/    notified.json, helper/
     mail/ reviews/ gates/
     INDEX.md
          │ mail/<session8>/… (a teammate's: mail/<session8>/<agent8>/…)
          ▼
   bin/answer-hook.py  (run by the layer's hooks/muster-answer)
          → the asking session's context, after its next tool call

   read only, allowed accounts only (0012): ~/.claude-<label>/projects/… transcripts of
   projects under ~/playground, `claude agents --json`, `claude auth status`; never ~/.claude
```

The same picture as a table, part by part:

| Part | Where | What it does |
|---|---|---|
| The core | `app/src-tauri/src/*.rs` | Every rule: reading queues, changing tasks, the lock, INDEX.md, keys, claims, merges, reviews. One crate, `muster_lib` |
| The app | `lib.rs` + `app/src/` (React 19, Vite, TypeScript) | The window: polls `state` every 2 s while visible, changes tasks only through the core's commands |
| The command line | `cli.rs`, `help.rs`; `bin/muster` | `muster <command>`: the same binary as the app (`main.rs` picks), 48 commands |
| Scripts and hooks | `bin/` | The answer hook, the gate, the manifest, the `task` stand-in, the Python tests |
| The personal layer | `~/playground/.claude` (not in this repo) | Links Muster's rule, hook and slash commands into every repo; the handoff and review engine |
| The queue | `.muster/` or `<project>/tasks` | Task files, backlog notes, attachments, mail, reviews, INDEX.md; a local queue's history in `.git/muster/history.git` |
| This Mac's settings | `~/Library/Application Support/ai.sweetlemon.muster/` | Which projects show and how, the lease and LIVE window, notifications, the helper |

### The core, module by module

Every top-level module in `app/src-tauri/src` (a test in `index.rs` fails when one is missing
here):

| Module | Does |
|---|---|
| `main.rs` | The entry point: with arguments it's `muster` (`cli::main`), with none it's the app (`run`) |
| `lib.rs` | The app: the Tauri builder, the 71 commands, the state poll, the hidden window's badge thread. Disk and git work runs in `spawn_blocking` |
| `cli.rs` | `muster` on the command line: options, which queue (`--queue`, `$MUSTER_ROOT`, the folder's), each command |
| `help.rs` | Every command's usage, options and examples; one source for `muster help`, the Help tab and `docs/manual/05-commands.md` |
| `queue.rs` | Reading queues (both layouts), the `Task` and `State` types, stages, the lock (`with_lock`, `hold_lock`), `write_atomic`, questions and answers (file contract v1) |
| `ops.rs` | Changing tasks: create, claim, pass, return, block, set fields, notes, attachments, reclaim. The rules both the app and the CLI use |
| `layout.rs` | Where a layout keeps what: the old `<project>/tasks` and the repo's `.muster/`, and finding a repo's queue from any worktree |
| `migrate.rs` | `muster migrate`: copy an old queue into a repo's `.muster/`, never moving the old one |
| `history.rs` | A local queue's history (mu-109): a bare git at `<git common dir>/muster/history.git` with `.muster/` as its work tree, started by `init` and `migrate` (or the first write); `with_lock` commits the files each write changed, `<key>: <log line>`, never `mail/`, `INDEX.md` or `tasks.json`; writes it couldn't commit, `check`'s lines, `muster restore`, `muster history` and `history removed`; `git gc --auto` after each commit, outside the lock |
| `index.rs` | INDEX.md, the generated master list, rewritten after every change; and the queue at a glance for this document (`--as-built`) |
| `backlog.rs` | Backlog notes: add, shape (the brief), file as a task, drop |
| `measured.rs` | The shaping brief's estimates, measured on this queue (mu-168): actual ÷ `est-time` at p50/p80 and median hours of finished tasks, overall, by type and size, from stage history; pure; mu-115 replaces it |
| `lenses.rs` | The shaping lenses (accessibility, i18n, themes, payments, architecture, security, privacy, performance, tests, docs): a project's skips with their reasons, read from `.muster/config.toml`'s `[shaping]` or an old queue's README front matter (written by hand, never by Muster) and their problems for `check`; a task's `**Lenses:**` lines; a VISION's `**Lenses needed:**` line. Read by `backlog.rs` (shape, file), `vision.rs` and `queue.rs` (problems) |
| `block.rs` | Why a task is blocked: five kinds, the sentence every screen shows, and `depends:` met once merged |
| `fields.rs` | A task's links and comments; the owner's comment @actions (`parse_action`, `comment`: @accept, @sendback, @block, @ask, run before the comment is saved) |
| `choices.rs` | Pick-one questions: 2 to 6 options, a pick, what happens with no answer |
| `clarify.rs` | The clarify pass in Shaping (mu-103): `[NEEDS CLARIFICATION: …]` markers that hold Ready, open questions without a default that hold it, and `muster clarify`, the batch of open questions on unstarted tasks sent to the owner at once |
| `key.rs` | Task keys (`mu-12`): the project's prefix, stored with the queue |
| `search.rs` | `muster list`'s search, filters and sort, held to the app's by a shared fixture |
| `vision.rs` | The vision check: the brief, the agent's result recorded, conflicts resolved |
| `now.rs` | What's true now (mu-102): the `docs/now/` pages read and matched to a task's `touches:`, the building pass's page check, `check`'s warnings |
| `review.rs` | The review of record: the brief, the result recorded, rounds, the merge check; owner findings from a send-back out of Your check (lens `owner`) |
| `gates.rs` | Gate records (G3 criteria, G4 visual QA): the record checked and stored at the branch's tip, which gates a task needs (`qa:`, `gates: on`), `muster gates`, the merge check and carry |
| `testing.rs` | What the review and the gate records share: the tool's sections (fenced code skipped), the head a record names, who may record in Testing |
| `verify.rs` | The criteria gate (G3, mu-132): `muster verify`'s brief for the `ac-verifier` agent, the ticks a criteria record writes (met ticked with its evidence, the rest unticked with their gaps logged), and the refusal of new owner-only criteria in `new`, `file` and `edit` (a hand edit of the task file isn't checked) |
| `lessons.rs` | Lessons from reviews: proposed from repeated findings, accepted or rejected by the owner |
| `pause.rs` | Pause and Resume: a task keeps its stage and claim, nobody works it |
| `old_pause.rs` | Turning pre-0045 "paused" blocks into pauses (`muster check --fix-pauses`) |
| `ping.rs` | Asking a working session for a check-in, or sending it a message, through its mailbox; the owner's `@ask` question and the send-back notice to the builder |
| `parallel.rs` | Parallel sessions: which Ready tasks are safe together, `muster start`'s worktree, the merge, `release`, what's left behind |
| `merge_log.rs` | Git's merge driver for `docs/WORKLOG.md` (`muster merge-log`), and setting it in a repo's config |
| `session.rs` | The holder stamp (`label (checkout · session 7b5e8856 · cc-hg)`, a teammate's with `· agent a1b2c3d4` after the session, a role slot's with `· slot coder-2` after those, 0112), its mailbox, and when two holders are the same (label or slot, session and agent) |
| `roles.rs` | The role profiles (0112): `roles.toml`'s strict reader (no TOML crate: strings, numbers and lists of strings; an unknown table or key, a wrong shape or a value off its bounds or allowlist refused with its line and key), the defaults (the brief's ten roles and Pouya), the file cached by modified time and size, `edit` for Settings, slots (`Slot`, `slot_of_label`, `held`, `lowest_free`), `muster team` and `muster instructions` (each role's brief: what it does and its rules from PROTOCOL; the coder's and the tester's carry the git rule, `GIT_RULE`: never `reset --hard`, `checkout --`, `restore` over changes or `clean` in a worktree, `git rebase --abort` and rebase again, squash only with `reset --soft`, mu-193). `--role` gives a session (and agent) the slot it already holds in Building or Testing, so one session is one slot; `session::keep_slot` carries a claim's slot to the Testing holder and a takeover's successor, even under its old name after a rename. `ops::slot_refusal` keeps one session to one slot across every queue (`ops::elsewhere` reads the others) and a slot to one session; `ops::unless_taken` brings a task back to a holder without the slot when another session holds it now (`session::without_slot`), and a takeover of a claim without a slot takes it over without one; a stamp without a slot part holds none |
| `work.rs` | Who worked on a task and what it cost, from the sessions its claim and log name; only sessions stamped with an allowed account are read (0012) |
| `transcripts.rs` | The one reader of Claude Code transcripts on this Mac, sums only: the allowed accounts' `projects` folders, and in them only project folders under `~/playground` (0012) |
| `accounts.rs` | The Claude accounts Muster reads (0012): the `claude_accounts` allowlist resolved to `~/.claude-<label>` folders (`allowed`), never `~/.claude` or inside it; the playground key that filters transcript folders (`playground_key`, `is_under`); the account a `CLAUDE_CONFIG_DIR` names (`from_config_dir`) |
| `profiles.rs` | The accounts Muster runs work on (0113), in `accounts.json`: provider, plan, roles, models, concurrency, reserve, paused, pinned projects; seeded from the allowlist; add refuses the company account and any off the allowlist; the views the header and `muster accounts` show (headroom, stale) |
| `usage.rs` | Each account's 5-hour and weekly use (0113), read never guessed (decision 0013): the stream's `rate_limit_event` (`parse_rate_limit_event`, for the runner) and the status line's `rate_limits` (`parse_statusline`), bounded; `usage/<label>.jsonl`, one line per change, 7 days at most |
| `agents.rs` | Claude Code's own session states (`claude agents --json`), allowed accounts only, off the main thread, at most every 15 s; and the Sessions view's wider list read (`ListEntry`, 0098) |
| `sessions.rs` | The Sessions view (0098): every session of the allowed accounts grouped as the agents view groups them, its project matched from `cwd` in the core (only the name goes out), and a session's recap (tasks, history, handoff chain, time, tokens, progress); polled only while the view is open |
| `projects.rs` | Which queues the app shows (discovery under `~/playground`, added, hidden), `projects.json` under its lock, new queues, the sidebar's state builder |
| `labels.rs` | A project's label and category |
| `arrange.rs` | The sidebar's order, headings, colours and folds |
| `check_limit.rs` | The limit on Your check per project |
| `settings.rs` | This Mac's settings: lease, LIVE window, notifications, the menu bar icon, the old-pauses offer, the Claude accounts Muster reads (`claude_accounts`), the language |
| `i18n.rs` | The core's languages (mu-146, from Drover's): the language in effect (the setting, else macOS's), and the menu bar's text from `native.json` with the same plural rule and digits as the window |
| `badge.rs` | The Dock badge's count, set from Rust |
| `tray.rs` | The menu bar icon: idle or waiting with the count beside it, its tooltip and menu, from the badge's count, in the language in effect |
| `notify.rs` | Mac notifications of what newly waits for the owner, and `notified.json` |
| `run.rs` | Run from the app: a project's run command in its main checkout, its output in a bounded ring, Stop and quitting end its process group |
| `helper.rs` | The AI helper: the owner's own `claude` on an allowed account, restricted to `muster` and reading |
| `helper_cmds.rs` | The app's commands for the helper |
| `accounts_cmds.rs` | The app's commands for the accounts (0113); the readings kept in memory and read again when their files change, which the 2 s poll notices and tells the window (`accounts-changed`) |
| `pause_cmds.rs` | The app's commands for pause, resume and old pauses |
| `ping_cmds.rs` | The app's commands for check-ins and messages |
| `review_cmds.rs` | The app's commands for the review brief and the lessons |
| `testkit.rs` | Tests only (`cfg(test)`): `testkit::tempdir()`, the one temp folder a lib test makes, at `<tmp>/proj/q` so its parent holds nothing else (mu-191) |

Integration tests are in `app/src-tauri/tests/` (`interop.rs`, `deps_cases.rs`, `search_cases.rs`,
`keys.rs`, `broken_pipe.rs`, `now_pages.rs`); their temp folders come from `tests/common/mod.rs`
(`common::tempdir()`, the same `<tmp>/proj/q` as `testkit`, mu-191); `examples/probe.rs` and `examples/dump.rs` time and print reads of the real queues.

### The app: webview and IPC

The window is React 19 in `app/src/`: about 190 files, each test beside what it tests. Screens
such as `App.tsx`, `TaskPanel.tsx`, `WaitingList.tsx`, `Settings.tsx`, `Helper.tsx`, `Help.tsx`
and `Views.tsx`; pure logic in `lib.ts`, `filters.ts`, `search.ts` and `modes.ts`; the atoms in
`app/src/ui/`; the shared fixtures (`*-cases.json`) that hold the core and the window to the same
answers. The task pane, the backlog note pane and the helper load on first open. Its one capability
(`capabilities/default.json`) lists `core:default`'s sets but the tray's (task 0085: with the
`tray-icon` feature on, `core:default` would include `core:tray:default`; a test in `tray.rs`
holds it): no file system, shell, dialog, notification, tray or network permission. Folders are picked by the core's own dialog, and the window names a queue by
its project name, never by a path.

**Fonts and their licences** (mu-150). Latin text is the system's SF; code is SF Mono. Persian text
is **Vazirmatn 33.003** (Saber Rastikerdar, SIL Open Font License 1.1, no Reserved Font Name),
bundled as `Vazirmatn-fa.woff2`: the variable font (weights 100–900) cut to the Arabic-script
ranges Persian uses plus ZWNJ, ZWJ, the bidi marks and « », 44,844 bytes, the plain cut (digits
are the app's choice, not the font's). The licence goes with it three ways: the font's name table
keeps the licence and its URL (name IDs 13 and 14); `tauri.conf.json` bundles
`app/src/assets/fonts/OFL.txt` as `Muster.app/Contents/Resources/fonts/Vazirmatn-OFL.txt`; and
Settings › About names the font, its licence and that file. It comes from the owner's font
collection at a recorded upstream commit, nothing is fetched at build or run time, and the CSP's
`font-src 'self'` covers it. `app/src/fonts.css`, imported after `styles.css` in `main.tsx`, holds
its one `@font-face` (`unicode-range`, so a screen without Persian never loads it; `size-adjust:
104.8%`, Vazirmatn's alef to SF's cap height) and puts "Vazirmatn" first in the text stack and in
every code stack (first there too: `ui-monospace` falls back to a monospaced Arabic of its own, so
a later Vazirmatn is never reached; Latin isn't in its range and stays SF Mono, only « » in code
take Vazirmatn's), by repeating `styles.css`'s selectors with only `font-family`. `input, textarea,
select { font-family: inherit }` there too, since WebKit gives form controls the system's control
font and some rules (the pane's Pause box) set only a size. WebKit before 17 (Safari 16, which
macOS 11 and older stop at) ignores `size-adjust` and keeps the face: Persian is still Vazirmatn,
about 5% shorter than the English beside it. An English title with a ZWJ emoji loads the face (U+200D
is in its range) but draws pixel for pixel the same. `fonts.test.ts` holds the face, the stacks
(every monospace rule in `styles.css` has its override), the 90 KB budget, the file's sha256 against
the README, the licence records in its name table and the bundled licence. How to cut it again,
byte for byte: `app/src/assets/fonts/README.md`. No other fonts are bundled.

The 84 commands it can call (`generate_handler!` in `lib.rs`), by what they do:

| Group | Commands |
|---|---|
| Reading | `state` (the 2 s poll), `search_index`, `help`, `task_body`, `image`, `projects`, `settings_info`, `locale_info`, `task_sessions`, `session_transcript`, `editable_body`, `shape_note`, `vision_brief` |
| Changing a task | `create_task`, `pass_task`, `accept_all`, `return_task`, `block_task`, `unblock_task`, `reclaim_task`, `release_unreached`, `set_stage`, `set_field`, `add_note`, `tick_criterion`, `edit_body`, `add_comment`, `attach_data`, `attach_pick`, `answer`, `answer_pick`, `resolve_vision`, `move_task` (registered; the window no longer calls it) |
| Backlog | `add_backlog_note`, `file_note`, `drop_note` |
| Projects and settings | `add_project`, `create_project`, `set_project_label`, `set_project_category`, `set_project_colour`, `arrange_projects`, `rename_category`, `fold_category`, `show_project_title`, `set_project_prefix`, `hide_project`, `remove_project`, `set_setting`, `set_your_check_limit`, `roles_info`, `set_role` |
| Sessions | `ask_checkin`, `message_session`, `pause_task`, `resume_task`, `old_pauses`, `fix_old_pauses`, `set_old_pauses_offered` |
| Review | `review_brief`, `lessons`, `decide_lessons` |
| Helper | `helper_info`, `helper_set_account`, `helper_set_model`, `helper_reset`, `helper_stop`, `helper_send`, `helper_login` |
| Links | `open_link` (a task's https links only) |
| Run | `run_start`, `run_stop`, `run_state` (a project's name only) |
| Accounts | `accounts_info`, `account_add`, `account_remove`, `account_save`; the event `accounts-changed` |

### The `muster` command line

The app's binary, run with arguments (`main.rs`). `bin/muster` finds the release build
(`app/src-tauri/target/release/muster`) and runs it; it's linked into `~/.local/bin`. 55 commands
in `help.rs`, grouped as the manual groups them (the table lists the main ones):

| Group | Commands |
|---|---|
| Tasks | `new`, `list`, `show`, `set`, `edit`, `note`, `comment`, `link`, `attach` |
| Stages | `pass`, `return`, `block`, `unblock`, `pause`, `resume`, `stage`, `tick`, `untick`, `move` |
| Claims | `claim`, `next`, `heartbeat`, `checkin`, `ping`, `reclaim`, `ready`, `start`, `release`, `merge-log` |
| Review | `review`, `verify`, `gate`, `gates`, `lessons` |
| Team | `team`, `instructions` |
| Questions | `ask`, `answer`, `escalate`, `clarify`, `inbox` |
| Backlog | `add`, `shape`, `file`, `drop`, `vision` |
| Projects and the queue | `projects`, `init`, `migrate`, `check`, `index`, `help` |
| Accounts | `accounts` |

### `bin/` and the hooks

Every script in `bin/` (the same test fails when one is missing here):

| Script | Does | Run by |
|---|---|---|
| `bin/muster` | Runs the release build of the app's binary as `muster` | everyone, through `~/.local/bin/muster` |
| `bin/task` | The old `task ask / answer / escalate / inbox`, now a stand-in that runs the same `muster` commands | scripts and agents that still call `task` |
| `bin/answer-hook.py` | Claude Code hook: hands a session the mail waiting in its mailbox (answers, check-in and pause requests, the owner's messages), framed as information. Reads the mailbox and `.git` files itself, about 10 ms a call | the layer's `hooks/muster-answer`, on PostToolUse, UserPromptSubmit and SessionStart |
| `bin/manifest.py` | Builds `tasks.json` from the task files and `--check` reports queue problems, both layouts | by hand, and by `muster migrate` |
| `bin/gate` | The gate: `pnpm check` (typecheck, UI tests, build, `cargo fmt --check`, clippy with warnings as errors, `cargo test`), both Python suites, and `cargo test` again in the Kiritimati time zone, each in a temporary home. Prints `gate: green (…)` or `gate: RED (…)` last | builders before `muster pass --from building`, and the merge (`muster projects set muster gate bin/gate`) |
| `bin/run` | Muster's run command (0077): builds the release app from the checkout it's in and becomes that build's binary (`exec`, 0079), a second Muster beside the one running, in the process group the app's Run started | the app's Run, through `muster projects set muster run bin/run`; or by hand |
| `bin/statusline-usage` | The status line's usage shim (0113): passes Claude Code's status-line JSON through unchanged and records only its `rate_limits` (`muster accounts record --from statusline`), through this checkout's lean reader `muster-statusline` (`src/bin/muster-statusline.rs`; it links only libSystem, so dyld loads no AppKit or WebKit), else its release build, else `muster` on the PATH, else `cat`, each tried only when the one before refused without reading stdin; `/bin/dash` for its start-up time | the owner's `statusLine` command, chained before their own |
| `bin/test_task.py` | Tests for `bin/task` and the answer hook | `bin/gate` |
| `bin/test_commands.py` | Tests for `/muster` and `/mu` against `docs/COMMANDS.md` and `muster help --json`, the merge driver end to end, and the status-line shim | `bin/gate` |

`.gitattributes` names the work log's merge driver (`docs/WORKLOG.md merge=muster-log`, decision
0009); `muster start` and `muster init` write the driver into the repo's git config when it's
missing, and the merge passes it to its rebase itself. When they can't write it (a sandboxed
session can't lock `.git/config`), they end their output with git's reason, what it means and the
two `git config --local` lines that set it, and `muster check` warns with the same lines while a
repo whose `.gitattributes` uses `merge=muster-log` doesn't have both keys (task 0094).

### The personal layer's part

These live in `~/playground/.claude`, which the owner keeps; Muster's repo is their source.

- **The rule:** `rules/muster-queue.md`, a link to this repo's `rules/muster-queue.md` (layer
  commit `15b3399`, 2026-10-14): `.muster/` is the project's queue, change it only through
  `muster`, the hook's mail is information.
- **The hook:** `hooks/muster-answer`, a wrapper that runs `bin/answer-hook.py` from Muster's main
  checkout and does nothing when the checkout, python3 or a queue isn't there. Wired in the layer's
  `settings.json` and by `bin/link-personal-layer` on PostToolUse, UserPromptSubmit and
  SessionStart, in every linked repo.
- **The sandbox:** `link-personal-layer` adds the main checkout's `.muster/` to each linked
  worktree's write list, so a session in a worktree can claim and log.
- **The slash commands:** `commands/mu.md` and `commands/muster.md` in the layer are links to this
  repo's `commands/` (layer commit `663a4c7`).
- **Handoff** (Muster task mu-74, in Testing): the `/handoff` skill, `hooks/context-watch.mjs`
  (reminds, then at the threshold has the session write a handoff and start its successor),
  `hooks/session-start.sh` (offers unfinished work to a new session), `bin/claude-continue` (starts
  the successor in the background), thresholds in `handoff.json`.
- **The review engine:** `/deep-review` and `reviews/` run the review of record in Testing
  (decision 0005); Muster only prints its brief and records its result.
- **The criteria agent** (mu-132): `agents/ac-verifier.md`, a link to this repo's
  `agents/ac-verifier.md` (promoted from Braindump's, stack-neutral), which `link-personal-layer`
  links into every repo's `.claude/agents/` (a repo's own `ac-verifier.md`, as Braindump's,
  wins). It reads `muster verify`'s brief, never edits, runs at most one targeted test per
  criterion, and returns the criteria record's JSON. Its fixture test is
  `agents/test_ac_verifier.py` (in `bin/gate`): the agent's definition, and a fixture task with a
  planted met, partial, unmet and unproven criterion whose expected verdicts a real run's JSON is
  checked against (`--make <dir>`, then `--check <dir> <json>`).
- **The work log reminder:** `hooks/worklog-check.mjs`, a Stop hook.

### Files on disk, and who reads and writes each

In the queue folder (`<repo>/.muster/`; an old queue has the same under `<project>/tasks`, with
status folders and `inbox/` for the backlog):

| File | What it is | Written by | Read by |
|---|---|---|---|
| `config.toml` | `local = true`, the key prefix (`prefix = "mu"`) and the lenses the project skips (`[shaping]` `skip = […]`, `[shaping.reasons]`); the only file a local queue commits | `muster init`, `migrate`, `muster projects set <name> prefix`; `[shaping]` by hand | the core (`layout.rs`, `key.rs`, `lenses.rs`) |
| `.gitignore` | A local queue ignores everything but itself and `config.toml` | `init`, `migrate` | git |
| `README.md` | The queue's short rules for agents; an old queue's front matter also holds its prefix and the lenses it skips (`shaping-skip:`, `shaping-reason-<lens>:`) | `init`, `migrate`; the skips by hand | agents; an old queue's front matter: `key.rs`, `lenses.rs` |
| `tasks/NNNN-slug.md` | One task: front matter, write-up, criteria, Questions, Vision, Review, Gates, Comments, Log | only `muster` and the app, under the lock; Drover writes answers (file contract v1) | the core, `manifest.py`, Drover, agents |
| `backlog/*.md`, `backlog/dropped/` | Backlog notes, and dropped ones with the reason | `muster add`, the app's quick add, anyone dropping a file in | the core |
| `shaping.json` | Who is shaping which backlog note: `{ "<note>": { "by", "at", "heartbeat" } }` (task 0097); live state, ignored by a committed queue's `.gitignore`; a missing file means nothing is held | `muster shape`, `file`, `drop`, `release`, `reclaim`, under the lock, atomically | the core (`backlog.rs`), `muster check` |
| `attachments/NNNN/` | A task's files (screenshots, mockups), and the note a task was filed from | `muster attach`, the app, `muster file` | the app, agents |
| `mail/.lock` | The queue's one lock (`flock`) | every write | every write |
| `mail/asks.json` | Which mailbox asked which question: `<session8>`, or `<session8>/<agent8>` for a teammate | `muster ask`; a takeover repoints the task's entries | `muster answer` |
| `mail/<session8>/` | A session's mailbox: answers, check-in, pause and resume requests, messages; `delivered/` once handed over. A teammate's is `<agent8>/` inside it, which the lead never takes | `muster answer`, `ping`, `pause`, `resume`, the app; a takeover moves the task's mail to the successor's | `bin/answer-hook.py` (a teammate's by the hook input's `agent_id`), `muster inbox` |
| `reviews/<id>.json` | The review of record's rounds and findings (the truth); an owner finding has lens `owner` | `muster review --record`, `--fixed`, `--dispute`, `--settle`; a return from Your check to Building adds the owner finding (a new store at round 0 when there's none) | the core, the task's `## Review` section |
| `gates/<id>/criteria.json`, `gates/<id>/visual.json` | A task's gate records (the truth): head, verdict, by, at, items | `muster verify --record`, `muster gate --record`; the merge's carry | the core, `muster gates`, the merge, the task's `## Gates` section |
| `reviews/lessons.json`, `lessons.md` | Lessons from reviews (truth, and rendered) | the core, under the lock; `muster lessons` | `muster start`'s brief, the review brief, the app |
| `INDEX.md` | The master list, generated | the core after every write; `muster index` | the owner, `muster check`, `manifest.py --check` |
| `tasks.json` | The manifest, generated | `manifest.py` only | scripts |
| `WAITING-FOR-OWNER.md` | What capture took to the owner | capture, by hand | the owner |
| `migrated-from.txt` | Where a migrated queue came from, and its last commit | `muster migrate` | the owner |
| `backlog/helper/` | Files pasted into the AI helper | the helper's `helper_send` | the helper's `claude` |

On this Mac, in `~/Library/Application Support/ai.sweetlemon.muster/`:

| File | What it is | Written by | Read by |
|---|---|---|---|
| `projects.json` (+ `projects.json.lock`) | Added and hidden projects, gates, names, labels, categories, colours, order, folds, Your check limits | the app and `muster projects set`, through `projects::edit` (flock, temp file, rename) | the app, `muster` (through `projects::places`, which tests replace) |
| `settings.json` | Claim lease, LIVE window, notifications on or off, the old-pauses offer, the Claude accounts Muster reads, the language (`lang`, only once picked) | the app's Settings | the app on every poll; `muster reclaim` and `muster check` |
| `roles.toml` | The role profiles (0112): each role's name, title, stage, prompt, tools, permission, models, caps, workers, slots, stage moves, providers, writes | `roles.rs` from its defaults on first read; Settings › Agents › Roles; the owner by hand | the app's poll (each role's name, for slot stamps), Roles, `muster team`, `instructions`, and `--role` on claim, next and start |
| `notified.json` | The keys of what's been notified (the last 500, plus every one still waiting) | `notify.rs` | `notify.rs` |
| `helper/helper.json`, `helper/<project>.jsonl` | The helper's account and model, and each project's conversation | the app | the app |

Elsewhere:

| What | Where | Who |
|---|---|---|
| The merge lock | `<repo>/.git/muster/merge.lock` | `muster pass --from testing` (one merge at a time per repo) |
| A started task's worktree and branch | `<project>/worktrees/<repo>/<key>-<slug>` (or `--at`), branch `task/<key>-<slug>` | `muster start`; removed by the merge or `muster release --clean` |
| Transcripts | `~/.claude-<label>/projects/<folder key>/<session>.jsonl`, allowed accounts, keys under `~/playground` and sessions stamped with an allowed account only (0012) | read only, by `transcripts.rs`; never sent anywhere, and the window gets counts, never paths |
| Session states | `claude agents --json --all`, per allowed account | run by `agents.rs`, read only |
| Handoffs | `~/playground/**/.claude/work/handoffs/*.md`, named by a session's first prompt (`/handoff resume <path>`) | read only, by `sessions.rs`: a regular file directly in that folder, at most 256 KB, front matter only (0098) |
| The project's picture | `<repo>/docs/VISION.md`, `docs/decisions/`, `docs/DESIGN.md` | read only: `vision.rs` (the brief) and `lenses.rs` (VISION's `**Lenses needed:**` line) |
| What's true now | `<repo>/docs/now/<area>.md`, committed with the code | written by the builder in the task's branch; read only by `now.rs` (the building pass, `check`), `vision.rs` and `backlog.rs` (the briefs) |

## 3. How a task flows

| Step | Stage after | Who | Commands |
|---|---|---|---|
| Capture | Backlog (a note) or Ready (a task) | the owner, capture, any agent | `muster add "<one line>"` (a backlog note); `muster new "<title>"` (a task); the app's quick add and New task; `/mu <anything>` |
| Shape | Shaping | product, tech and UI agents | `muster shape <note>` (the brief, with the lenses the project doesn't skip), `muster file <note> "<title>" --body -` (warns about a lens with no line under `**Lenses:**`, files anyway), `muster vision <id>` then `--record -` (a lens the VISION needs said "n/a" becomes a conflict) |
| Ready | Ready | the shaper | `muster pass <id> --from shaping` (needs acceptance criteria and no open vision conflict) |
| Start | Building | one builder session per task | `muster start <id>` (its own branch and worktree), or `muster claim <id>` / `muster next` |
| Build | Building | the builder | `muster heartbeat`, `muster checkin <id> "<done, next>"`, `muster ask <id> "<question>"` (the answer comes back through the hook) |
| Gate | Testing | the builder | `bin/gate`, then `muster pass <id> --from building --gate "<its last line>" --summary "<what was built>"` (a task needs its one-line summary, mu-78) |
| Review of record | Testing (or back to Building) | a session that didn't build it | `muster review <id>` (the brief), `/deep-review <range>`, `muster review <id> --record -`; open findings send it back to Building (rounds 1 and 2) or block it on the owner (round 3); the builder answers with `--fixed` or `--dispute` |
| Merge | Your check | the QA session | `muster tick <id> <AC>` for each criterion with its proof, then `muster pass <id> --from testing`: rebase on main, gate again, fast-forward main, push, remove the worktree and branch |
| Your check | Done | the owner | Accept in the app (or Accept all, or a comment starting `@accept`), or `muster pass <id> --from your-check --as owner` after ticking the owner's criteria; Send back to builder (or `@sendback <why>`, or `muster return <id> "<why>" --to building`) gives it back to its builder first in line, the reason an owner finding to fix or dispute (mu-101) |

Anywhere along it: `muster block <id> --on <what>` and `unblock`, `muster pause` and `resume`,
`muster return <id> "<why>"`. A task that waits on others (`depends:`) can't be started until each
is merged (Your check or Done).

## 4. Each part's state

Built means on main and covered by tests in `bin/gate`. A task's own stage, which can lag the code,
is in section 6.

| Part | State | Task |
|---|---|---|
| Queue files, stages, claims, leases, questions, file contract v1 | Built, tested | mu-2, mu-14 |
| `.muster/` in the repo: layout, local mode, one queue for every worktree, `muster init`, `muster migrate` | Built, tested. Muster's own queue moved 2026-10-13; the other 19 projects' haven't | mu-5 |
| A local queue's history: a commit per write, `muster restore`, `muster history`, `check`'s lines, Settings › Projects' History line, the layer's guard against `git clean -x` | Built, tested; on its branch, not on main. The guard is in the personal layer | mu-109 |
| Committed mode with live state in `.git/muster/` | Not built (committed mode today has one copy, like local) | 0005's next step (a backlog note) |
| Task keys (`mu-12`) | Built, tested | mu-33 |
| INDEX.md, and the queue at a glance in this document | Built, tested | mu-18, mu-80 |
| `tasks.json` (`manifest.py`) | Built, tested; rebuilt only when `manifest.py` runs, so Muster's is as old as its migration | mu-2 |
| Backlog and shaping, the vision check | Built, tested | mu-15, mu-29 |
| What's true now: `docs/now/` pages, the building pass's page rule, the pages in the vision and shaping briefs, `check`'s warnings; Muster's seven pages | Built, tested; on its branch, not on main | mu-102 |
| Shaping lenses: the brief's list, a project's skips with reasons, `file`'s warning, `check`'s problems, VISION's needed lenses | Built, tested; on its branch, not on main | mu-76 |
| Goals, features and tasks; depends met once merged | Built, tested | mu-16, mu-42 |
| Blocked says what it waits for (five kinds) | Built, tested | mu-24, mu-60 |
| Pause and Resume, the Paused lane, old pauses converted | Built, tested | mu-45, mu-59 |
| Parallel sessions: `ready --parallel`, `start`, the merge, `release --clean` | Built, tested | mu-23 |
| The tester isn't the builder; send-back comes first | Built, tested | mu-47, mu-66 |
| Review of record and lessons | Built, tested; the owner's first real round hasn't happened | mu-27 |
| Work log merge driver | Built, tested | mu-67, mu-71 |
| Pick-one questions | Built, tested | mu-49 |
| Ping a session (check-in, message) | Built, tested; the auto-nudge isn't built | mu-39 |
| Release a claim no session can be reached on; a holder's act stamps it; send-backs keep the session | Built, tested | mu-184 |
| A claim or Testing hold follows its session to another account on the holder's next act | Built, tested | mu-185 |
| Claim takeover after a handoff or 30 quiet minutes, mail following the claim; a teammate is its own holder (agent id) with its own mailbox | Built, tested; on its branch, not on main. No hook passes the agent id: `--agent` or `MUSTER_AGENT` | mu-90 |
| Claude Code's session states | Built, tested (a research-preview format) | mu-48 |
| Who worked on a task and what it cost | Built, tested; a session counts in full on every task it names | mu-25 |
| The app: board, Table, Timeline, Graph, task pane, Waiting for you, Recently finished | Built, tested | mu-1, mu-13, mu-19, mu-21, mu-31, mu-51 |
| Search, filters, sort, saved views | Built, tested | mu-32 |
| Sidebar: activity, labels and categories, drag to reorder, colours, folds, one state builder | Built, tested | mu-37, mu-44, mu-57, mu-70, mu-73 |
| Settings, display modes, themes and accents, Drover's token names and atoms; one CSS class per component | Built, tested | mu-36, mu-38, mu-12, mu-53, mu-81 |
| Your check: next step, effort groups, Accept all, the limit per project | Built, tested | mu-43, mu-50 |
| Your check's verdict: Send back to builder (first in line, an owner finding, the builder's session told) and the owner's comment @actions (@accept, @sendback, @block, @ask) | Built, tested; on its branch, not on main | mu-101 |
| Dock badge | Built, tested | mu-6 |
| Mac notifications | Built, tested, merged 2026-10-13 (`5d1329b`) | mu-52 |
| The owner's icons: app icon light and dark, the sidebar's logo, the menu bar icon | Built, tested; on its branch, not on main | mu-85 |
| The AI helper | Built; checked end to end once, by hand | mu-8 |
| The manual and the Help tab | Built, tested; no screenshots | mu-35 |
| `/muster` and `/mu` | Built, tested, linked into the personal layer | mu-3 |
| Other projects: `muster init` in a repo, the layer's rule and hook | Built: Muster's side tested, the layer's committed 2026-10-14; one live session in a new project is left (mu-4 AC2) | mu-4 |
| Hermetic tests (no test reads this Mac's projects) | Built | mu-62 |
| A run command per project, beside its gate | Built, tested, merged 2026-10-14 (`666a760`) | mu-77 |
| Run from the app: a project's run command, its output, Stop | Built, tested on its branch; the release-app check (AC4) is the owner's | mu-79 |
| Your check says what you're accepting: the builder's summary, what Accept does, each criterion's proof | Built, tested, merged 2026-10-14 (`72f34df`) | mu-78 |
| Automatic handoff at about 60% context | In the personal layer, in Testing | mu-74 |
| Orchestration (Muster decides, Drover runs the agents) | Not built | mu-9 |
| The planning room; ideas in Muster | Not built | mu-41, mu-56 |
| A GitHub reviewer app | Not built | mu-55 |

## 5. Decisions

In `docs/decisions/`, newest last.

| # | Decision |
|---|---|
| [0001](decisions/0001-app-on-drovers-stack.md) | The app is on Drover's stack (Tauri 2, React 19, Vite, TypeScript), in `app/`, with only crates Drover already uses, so it can move into Drover later |
| [0002](decisions/0002-helper-runs-the-owners-claude-restricted.md) | The AI helper runs the owner's own `claude` with their own login, restricted to `muster` and reading |
| [0003](decisions/0003-muster-in-the-repo-backlog-and-stages.md) | Tasks live in the repo's `.muster/` and flow from a backlog through seven stages with roles; goals, features and tasks |
| [0004](decisions/0004-one-session-per-task-thin-coordinator.md) | One session per task, a thin coordinator that hands off early, sessions named by role |
| [0005](decisions/0005-review-of-record-runs-in-the-testing-session.md) | The review of record runs in the Testing session with the owner's engine; Muster records it and gates the merge |
| [0006](decisions/0006-four-parts-idea-task-run-gate.md) | Four parts, idea → task → run → gate, each owning one thing and talking through commands and files |
| [0007](decisions/0007-one-ecosystem-muster-owns-idea-and-task.md) | One ecosystem: Muster owns idea and task, Drover the code, the review engine the review; gate and deploy to build |
| [0008](decisions/0008-agent-teams-where-muster-uses-them.md) | Agent teams for the planning room and review lenses, not for building; a teammate's stamp becomes session + agent id (not built yet) |
| [0009](decisions/0009-work-log-merges-through-a-muster-merge-driver.md) | The work log merges through `muster merge-log`, so new entries on top never stop a merge |
| [0010](decisions/0010-the-queue-lives-in-local-files-not-github-issues.md) | The queue lives in local files, not GitHub Issues |
| [0011](decisions/0011-sessions-hand-off-by-themselves.md) | Sessions hand off by themselves at \~60% context or the token cap, at a gap, to an automatic successor that reads the old mailbox; amends 0004 point 3 |
| [0012](decisions/0012-muster-reads-only-the-personal-claude-accounts.md) | Muster reads only the personal Claude accounts in one allowlist (default cc-mg, cc-hg), never `~/.claude`, and only transcripts of projects under `~/playground` |
| [0013](decisions/0013-orchestration-terms-and-accounts.md) | Orchestration's terms: Muster runs each user's own `claude` and `codex` and never touches a login token, never balances across accounts (a limit means cooldown until `resets_at`), pins each account to projects by hand, reads usage instead of learning it from limit hits, caps and paces every run, keeps telemetry off; replaces the brief's load balancing |
| [0014](decisions/0014-gates-replace-your-check.md) | Gates replace your check: a task that passes every gate it needs is accepted without the owner |
| [0015](decisions/0015-muster-runs-agents-behind-a-runner-interface.md) | Muster runs agents behind one runner interface (`start`, `status`, `stop`, `usage`, `limit`, `capabilities`; states `working`, `input_required`, `completed`, `failed`, `cancelled`): its own `claude -p` runner now, with 0002's scrub and no token, Drover's later; the Codex adapter turns analytics off; amends 0007 and 0005 |

## 6. The queue at a glance

Generated from Muster's queue by `muster index --as-built docs/AS_BUILT.md`, run in this repo
(the same run rebuilds the queue's INDEX.md). It holds only keys, titles, stages and who's on
each; never a task's write-up or answers. Don't edit between the two marker lines: the next run
replaces everything there, and nothing outside them. For the live list, `muster list` or the app.

<!-- muster:queue-at-a-glance start -->

As of 2026-10-14 06:30 (the newest change in a task's log). 83 tasks: 9 Ready, 1 Building, 4 Testing, 8 Your check, 61 Done. 12 backlog notes.

### Ready (9)

| Key | Title | On it |
| --- | --- | --- |
| mu-9 | Orchestration: send teams of agents to work on tasks, and run how they talk |  |
| mu-41 | Planning room: features planned by a team of roles (product, research, UI, tech, skeptic) in rounds with the owner, before any task is built |  |
| mu-54 | Learning both ways: one record of outcomes across idea, task, code, review, gate and deploy, lessons forward, misses backward |  |
| mu-55 | A GitHub reviewer like CodeRabbit: the engine posts as its own GitHub App, run from this Mac |  |
| mu-56 · Feature | Ideas in Muster: GenIdea's interview, decision ledger, probe and seal ported into Muster's idea step |  |
| mu-69 | Notifications: a session already waiting at launch isn't posted later when its Claude account's first read failed |  |
| mu-75 | Muster's handoff rule follows the automatic handoff: decision 0004 and PROTOCOL.md hand off at \~60% context by themselves, and the successor takes the old session's answers |  |
| mu-76 | Shaping checks the cross-cutting lenses by default: accessibility, i18n/RTL, themes, payments, architecture, security, privacy, performance, unless the project opts out |  |
| mu-79 | Run from the app: one click starts a project's run command in its main checkout, shows its progress and output in place, and stops it (and everything it started) on Stop or when Muster quits |  |

### Building (1)

| Key | Title | On it |
| --- | --- | --- |
| mu-80 | One as-built document for Muster: architecture, every part's state, the decisions, and the queue at a glance, kept current | asghar |

### Testing (4)

| Key | Title | On it |
| --- | --- | --- |
| mu-74 | Sessions hand off by themselves at \~60% context: finish the step, wait for their own work to land, write the handoff, start the successor, with no human step (personal layer) | Massihx |
| mu-77 | Each project has a run command beside its gate: muster projects set \<name\> run "\<command\>", shown in Settings › Projects | asghar |
| mu-82 | Sidebar categories look right: a clear heading, a fold that really folds to one line, labelled counts, projects indented under their heading | asghar |
| mu-83 | Board with mostly empty columns: one open column stretches cards to the full width, and the folded strips don't line up | asghar |

### Your check (8)

| Key | Title | On it |
| --- | --- | --- |
| mu-4 | Muster in other projects: \`.muster/\` in each repo, set up in one step | owner |
| mu-52 | Mac notifications: one macOS notification when something new waits for you (tauri-plugin-notification) | owner |
| mu-53 | Muster wears Drover's token names and the atoms that fit (Button, Pill, StatusPill, Badge, Segmented), keeping its own look | owner |
| mu-59 | Paused tasks go to their own place: a Paused lane beside Backlog, out of the working columns, back where they were on Resume | owner |
| mu-60 | Blocked says plainly what it waits for: your answer, your action, a permission, other tasks or something outside, each looking different | owner |
| mu-70 | Category headings fold: the persianUtils projects under one DashAfzar heading that collapses to its name and counts | owner |
| mu-78 | Your check says what you're accepting: the builder's one-line summary, what Accept does, and each criterion's proof with its screenshots, on the card and in the pane | owner |
| mu-81 | Settings › Appearance's theme picker is crushed: mu-57's project colour swatches reuse the global .swatches/.swatch classes | owner |

### Done (61)

| Key | Title | On it |
| --- | --- | --- |
| mu-1 | Muster desktop app: see what's happening across the queues, live |  |
| mu-2 | Move the rest of the rules into the tool: claim, next, block, done, found, triage |  |
| mu-3 | \`/muster\` and \`/mu\` slash commands, one router like \`/pr\` |  |
| mu-5 | Tasks live in the repo's \`.muster/\`, files never move, claims live in the shared git folder |  |
| mu-6 | Muster app, next round: the owner's first look, Drover's look, and telling you when you're needed |  |
| mu-7 | Create and manage tasks in the app and from the command line, with one set of rules |  |
| mu-8 | An AI helper in the app that does what the capture session does, without the file noise |  |
| mu-10 | Muster's palette: violet, plus the board's status colours and the icon |  |
| mu-11 | Tags on tasks: spike, chore, tech debt, code and your own |  |
| mu-12 | Themes and accents in the app: a few styles, each with its own backgrounds and colours |  |
| mu-13 | Task details beside the board, never over it |  |
| mu-14 | Stages with roles: Backlog, Shaping, Ready, Building, Testing, Your check, Done |  |
| mu-15 | Backlog and shaping: one-liners from anywhere, shaped into tasks by product, tech and UI agents |  |
| mu-16 | Goals, features and tasks: parent links and roll-ups |  |
| mu-17 | Task fields: type, points (estimates and actuals in minutes and tokens), links, comments |  |
| mu-18 | INDEX.md: one generated master list of every task, with links |  |
| mu-19 | Dependency flow view: every task and what it waits on, as a graph |  |
| mu-20 | Edit a task's description and criteria in the app, and add comments from the task panel |  |
| mu-21 | Task panel header redesign: everything visible, nothing in the way |  |
| mu-22 | Human board and AI board for each project: what you do, what the agents do |  |
| mu-23 | Parallel sessions: one worktree per task, then merge, clean up, report, move on |  |
| mu-24 | Blocked says why: by other tasks, by you, by a permission, by a question, or by something outside |  |
| mu-25 | Who worked on a task and what it cost: sessions, tokens, time, live, and the transcript |  |
| mu-26 | Card looks in Appearance: the side stripe and other styles to pick from |  |
| mu-27 | Agent PRs reviewed by the owner's own engine, fixed by the agent, and learned from |  |
| mu-28 | Criteria only the owner can verify don't block the pass into Your check |  |
| mu-29 | Vision check: every new or changed task checked against the one picture, with the cost of pivoting |  |
| mu-30 | Who's on it, since when, and how far: session name, start time and check-ins on the card |  |
| mu-31 | Timeline scales: a day in hours, a week, a month |  |
| mu-32 | Search, filter and sort every task in a project |  |
| mu-33 | Task keys with a project prefix: mu-12, br-7 |  |
| mu-34 | Memorable stage colours: Ready blue, Building the accent, Testing amber, Your check orange, Done green |  |
| mu-35 | Help as a real manual: concepts, tutorials, commands, shortcuts, in sections and pages |  |
| mu-36 | Settings: one place for appearance, projects, agents and help |  |
| mu-37 | Projects sidebar: which projects are being worked on right now, and which are quiet |  |
| mu-38 | Display modes in Settings: ADHD, Standard, Focus |  |
| mu-39 | Ping a working session from the app: ask for a check-in, or send it a message |  |
| mu-40 | Agent teams spike: what Muster can rely on (session ids, worktrees, muster CLI, hooks, cost), then a decision |  |
| mu-42 | Dependencies are met once merged: a task in Your check counts, not only Done |  |
| mu-43 | Your check says what to do: the next step on each card, grouped by effort, Accept all for the ones with nothing to try |  |
| mu-44 | Projects get a label you choose and a category that groups the sidebar |  |
| mu-45 | Pause a task: it keeps its place and claim, nobody works it, its session is told to stop, Resume carries on |  |
| mu-46 | Group by Feature or Goal hides the backlog notes and the quick add: show them in a Backlog band first |  |
| mu-47 | The tester isn't the builder: Testing → Your check is refused when the builder passes it |  |
| mu-48 | Read Claude Code's session states (claude agents --json): working, waiting for input, ended, falling back to heartbeats |  |
| mu-49 | Pick-one questions: agents ask with options, the owner answers with one click in the app or --pick in the CLI |  |
| mu-50 | A limit on Your check per project: the board warns when too much waits for you, and never blocks |  |
| mu-51 | Recently finished: what got merged and accepted in a project, newest first, '4 min ago' |  |
| mu-57 | Projects in the sidebar: drag to reorder, group by category, a colour each, all edited right there |  |
| mu-58 | Both side panes resizable: the projects sidebar on the left and the task pane on the right, widths remembered |  |
| mu-61 | Layout fixes from the visual QA: Settings › Projects table, pane status column, graph chips, table header, Focus strip, button gaps |  |
| mu-62 | Hermetic tests: no test reads the real projects.json or \~/playground; one injection point for the projects Muster knows |  |
| mu-63 | Runner tests don't depend on machine speed: no 5 s timeouts that fail under parallel builds |  |
| mu-64 | A started task can't skip its merge: stage or move into Your check or Done is refused until its branch is in main |  |
| mu-65 | Hatched own cards reach 4.5:1 in every look, and Quick add's help names the Backlog band |  |
| mu-66 | A review can always send a task back to its builder, even while the builder holds its next task; the sent-back one comes first |  |
| mu-67 | Work log entries merge by themselves: a muster merge driver for docs/WORKLOG.md keeps every entry whole, newest on top |  |
| mu-68 | muster help pause and the Pause button still say a paused task stays where it is; it moves to the Paused lane |  |
| mu-71 | Work log merge driver follow-ups: ours is replaced whole or not at all, and its tests can fail (mu-67 R7-R10) |  |
| mu-72 | Two tests that can't be trusted: the lessons test fails on a temp path holding L1, and init's prefix check can't catch a HOME leak (mu-62 R3) |  |
| mu-73 | Sidebar state follow-ups: the poll and the DashAfzar fixture share one state builder, and folds follow only the projects the sidebar draws (mu-70 R12, R13) |  |

### Backlog notes (12)

| Note | File |
| --- | --- |
| Clarify pass in Shaping: shaping agents mark unknowns and send them to the owner in one batch | 2026-10-13-0158-clarify-pass-in-shaping-shaping-agents-mark.md |
| Later, for selling: an MCP server for agents outside Claude Code, and import from Spec Kit tasks.md, OpenSpec changes and Beads | 2026-10-13-0158-later-for-selling-an-mcp-server-for-agents.md |
| What's true now: when a task is Done, its decisions update a short page per area that the vision check reads | 2026-10-13-0158-what-s-true-now-when-a-task-is-done-its.md |
| 0005 next step: committed .muster/ mode with live state (stage, claim, lease, questions, mail, lock) in .git/muster/, the task write-up edited only on the claimant's branch | 2026-10-13-0522-0005-next-step-committed-muster-mode-with-live.md |
| muster start/init must say plainly when it can't write the work-log merge driver to .git/config (a sandboxed session gets 'operation not permitted'); today it may stay silent (Asghar, mu-71) | 2026-10-14-0418-muster-start-init-must-say-plainly-when-it-can-t.md |
| The task pane's Files opens screenshots in a lightbox (Sidebar.tsx:251), which docs/DESIGN.md:39/:127 rule out: show them in place (found shaping mu-78) | 2026-10-14-0422-the-task-pane-s-files-opens-screenshots-in-a.md |
| muster edit drops a task's \#\# Questions section when it sits before \#\# Vision (seen on mu-79): edit must keep Questions, wherever it is | 2026-10-14-0425-muster-edit-drops-a-task-s-questions-section.md |
| mu-73 nits: test headings\_now's p.ok half (added project whose folder is gone keeps a fold), and that projects::read\_state passes settings through to State.settings | 2026-10-14-0513-mu-73-nits-test-headings-now-s-p-ok-half-added.md |
| Board: show or hide each column (owner), remembered per Mac; a hidden column stays a thin labelled strip with its counts, never gone (docs/DESIGN.md), like Focus's folds | 2026-10-14-0514-board-show-or-hide-each-column-owner-remembered.md |
| From rejected lessons L2-L4: test Settings ↑/↓'s pending move (0059 R5), the fold rollback and settle effect (0070 R4/R5), and undo the optimistic unfold on refusal (0057 R5) | 2026-10-14-0600-from-rejected-lessons-l2-l4-test-settings-s.md |
| mu-81 nit: cssClasses.test's shared-class guard only fails on clashing values; fail on any class with \>1 top-level rule used by \>1 component, and cover :hover/compound selectors | 2026-10-14-0608-mu-81-nit-cssclasses-test-s-shared-class-guard.md |
| Needs-you badge (.count.warn) reads \~2.2:1 in light themes, below 4.5:1: use --on-status, with a themes test (from mu-82) | 2026-10-14-0630-needs-you-badge-count-warn-reads-2-2-1-in-light.md |

<!-- muster:queue-at-a-glance end -->

## 7. Known limits and what's left

### Known limits

- **A worktree session writes into the main checkout's `.muster/`.** A linked worktree has it in its
  sandbox write list (the layer's linker); a worktree made some other way, or a sandbox that allows
  only its own folder and the main checkout's `.git`, can read tasks but not claim, log or ask
  (decision 0003, "Revisit when").
- **An old-layout queue needs its folder in the layer's sandbox list** (`layer-settings.json`,
  `allowWrite`) and `link-personal-layer` re-run in that project's checkouts; otherwise its agents
  can read tasks but not claim, log or ask (`DOGFOOD.md` 41).
- **A local queue's history keeps everything, for good** (mu-109): nothing is pruned, and
  attachments (up to 25 MB each) stay in it as long as the queue does. It is packed: `git gc
  --auto` runs after each commit, outside the queue's lock, and packs once there are about 6,700
  loose objects (about 1,700 writes), in the background. A file deleted on purpose is recorded
  with `muster history removed` (Settings: Keep deleted) and stays in the older commits (its id
  isn't given again, and `restore` refuses a task file whose id is taken); there is
  no way to erase it from the history short of removing `.git/muster/history.git`. A file changed outside
  muster (a hand edit, Drover's answers) goes in only with the next `muster history commit`, and
  `check` warns until then. The guard against `git clean -x` reads the command's words: a script
  that runs it, or git through another program, isn't seen; `muster restore --all` is the undo.
- **A committed queue has one copy too, for now:** every worktree writes the main checkout's
  `.muster/`, so every claim and pass rewrites tracked files there. That's why `init` and `migrate`
  write a local queue unless `--committed` is given.
- **Files still move between status folders in the old layout** until its queue is migrated.
- **`tasks.json` goes stale:** nothing rebuilds it after a change but `manifest.py` run by hand.
- **A task's stage can lag its code:** a lead that merges a task by hand has to pass it too.
  mu-52 and mu-53 were merged to main on 2026-10-13 and read Building until 2026-10-14. Closed
  by mu-110: `muster check` names a task in Building or Testing whose `task/` branch is in main
  holding a commit it made itself (`parallel::own_commits`, from its reflog; a fast-forward or
  rebase onto main with nothing built doesn't count), with the pass that catches it up
  (`parallel::lagging`, a warning). With its worktree gone, that pass from Testing records the
  merge main already has (`parallel::record_hand_merge`: no rebase, gate or push, but the review and the gate records
  it needs must stand at the tip, as a merge's (mu-131); origin's branch is kept until origin's
  main has the tip), and `muster review`, `muster gate` and `muster gates` read the branch from
  the project's repo (`parallel::branch_span`). `muster check`
  only, not the app's Problems list, which never shows the left-behind lines.
  Not covered: a squash merge (the branch tip never lands in main), a branch deleted after the
  merge, and a branch whose reflog is gone (nothing says where it started, so nothing is said).
- **New session id after `/clear` or a handoff:** answers to the old session's questions wait in its
  old mailbox. Covered: the handoff carries the old session id (its `session:` front matter), and
  PROTOCOL.md (decision 0011) has the writer put `muster inbox --session <old>` first in its Next
  steps, which the successor follows. Not covered: nothing in the `/handoff` skill, the hooks or
  `claude-continue` runs or checks that step, so a handoff without it leaves the answers unread
  (they're still in the task file). A takeover (task 0090) moves the claim and brings that task's
  mail and answers along; other tasks' answers, and a coordinator's own, still wait in the old
  mailbox.
- **A teammate is told apart only when it says who it is:** `--agent <id>` or `MUSTER_AGENT`. Its
  shell sees only the lead's session id, and no hook adds the agent id yet, so a teammate that
  leaves it out is stamped, and matched, as its lead.
- **The hook reaches a session only where the layer is linked**, and a ping or pause reaches a
  session only through the hook: a session without it sees a check-in request, message or stop
  only when it runs `muster inbox`, and a claim with no session stamp can't be reached at all.
- **Nothing wakes a capture session** when an agent asks something; the app and the notifications
  show it to the owner.
- **Reclaim runs only when someone runs it** (`muster reclaim`, or Release on a card); no
  scheduler, so a crashed session's claim waits until then. No auto-nudge of quiet sessions either.
- **A session counts in full on every task it names** (0025): splitting a transcript by the
  stretches it held each task isn't built.
- **Who may pause or resume isn't limited to the owner**, as with block and unblock.
- **Claude Code's session states are a research preview** (0048): the format may change, a
  coordinator's subagents share its session id, `failed`, `stopped` and an idle interactive session
  read as unknown, and only the allowed `~/.claude-<label>` accounts are read (0012). `agents::Fetch` is the one place
  the source would change when Drover supplies states.
- **A teammate of the builder's lead isn't a separate tester yet** (decision 0008): QA runs in a
  session of its own until stamps carry the agent id.

### What's left, in order

1. **The owner's checks:** the tasks in Your check (section 6), the review of record's first real
   round (mu-27 AC1), and one live session in a new project (mu-4 AC2).
2. **Finish what's in flight** (section 6 says where each stands): the owner-reported regressions
   first (mu-82, sidebar categories; mu-83, a board of mostly empty columns), then mu-77 (a run
   command per project) and mu-74's handoff in the layer.
3. **The rest of Ready,** high priority first: mu-79 (run from the app, after mu-77),
   mu-76 (shaping checks the cross-cutting lenses), mu-75 (the handoff rule in decision 0004 and
   PROTOCOL.md, after mu-74), then mu-69 (a notification bug, after mu-52).
4. **Committed `.muster/` with live state in `.git/muster/`** (0005's next step), and migrating the
   other projects' queues with `muster migrate`.
5. **Orchestration** (mu-9): Muster decides who does what, Drover starts and steers the agents
   (decisions 0006, 0007; Drover 0018). Then the planning room (mu-41), ideas in Muster (mu-56),
   one record of outcomes (mu-54) and a GitHub reviewer app (mu-55).
6. **Review of record, the parts outside Muster** (each the owner's call): the engine's ledgers and
   `/pr learn` get Muster's labels (they key rows on a PR number, so a task needs a place in their
   schema); `sweetlemonai/muster` in the engine's `repos` and a token budget for its light tier;
   posting to GitHub as sweetlemonai, with a login check before every post, comes back when PRs do.
   The review's and lessons' files move into the live state with 0005's store.
7. **Into Drover:** Drover's task creator (Drover 0005) and waiting list (Drover 0015) are the
   first pieces; the app's screens move into Drover when it's ready (decision 0001).

## Parts in detail

What each part does, as built, task by task. How-tos are in the manual (`docs/manual/`).

### The app

`app/` is a Tauri 2 + React desktop app on Drover's stack (decision 0001), so it can move into
Drover later. Open it with `open app/src-tauri/target/release/bundle/macos/Muster.app` (build:
`cd app && pnpm install && pnpm release:build`; develop: `pnpm app`).

- **Projects sidebar** (left): All and every queue (found at `~/playground/*/tasks` and one level deeper,
  `~/playground/*/*/tasks`, and the repos' `.muster/` at `~/playground/*/.muster` and
  `~/playground/*/*/.muster` in a main checkout, a project with both showing its `.muster/` under
  its name; plus ones added by hand), each with its task count, how many need you (orange) and how many are in progress
  (the Building colour), and **+ Project** at the bottom (Settings, at Projects). The header keeps only the views, New task and
  the display mode chip (0038), on one line. Under each name, quietly (0037, `activity.ts`): the Building-colour
  dot and "2 working" (tasks with a live session, or a check-in or heartbeat in the last 30
  minutes; never in Done, and in Your check only while live), or **stalled** in the attention
  colour (work in Building, not blocked and not the owner's, and nothing in the project for over
  30 minutes), and the last activity ("now", "12 min ago", "3 h ago", "yesterday", "5 days ago":
  the newest stage change, check-in, heartbeat, file change, queue commit or live transcript).
  "N working" counts tasks everywhere the sidebar says it, from one helper (mu-197, `human.ts`
  `workingWords`): a row's, a heading's ("· 16 working", the sum of its projects') and All's. The
  All row (`SumsLine`, `sumActivity`'s `working` and `projects`) says the tasks and, in words, the
  projects they're in, one sentence (`sidebar.workingIn`, `workingInWords`): "29 working in 9
  projects · 2 projects stalled" (stalled is a project's state, `stalledProjectsWords`). "N
  working" and the stalled count never give way; where the line doesn't fit, the when goes first,
  then "in 9 projects", each whole, and a piece gone gives its room back: the rest of the line
  (`.side-sum-rest`, one line tall, showing its last line) wraps "in 9 projects" out of sight
  where it and the stalled count don't both fit, and the when grows into what's left
  (`.side-sum-tail`). How many lines All takes follows the sidebar's width, never the counts
  (`.side-sum-box`, a container query): under 230px the rest always has a second line under "N
  working", stall or not ("in 9 projects · 2 projects stalled", or "in 9 projects" and the when),
  and from 230px it's one line, where "129 working · 12 projects stalled" fits in en and fa; the
  " · " before the stalled count is the tail's on one line and the end of "in 9 projects" on the
  second, so no line starts with it. In Focus All's badge is the short "2 stalled" ("2 projects
  stalled" in its tooltip and aria-label), and under 200px All's badges always take the line under
  the name (`.side-all-focus`), so a poll that brings a stall moves no row below. All of it stays in the tooltip and the row's description; a project's row keeps "8 working" whole the same way, its
  when giving way. In Focus, a row's aria-label says the line's working words behind the dot.
  Chips above the list order it: **Working**
  (default: working projects first, then by last activity), **A–Z**, **Needs you** (most waiting
  first, then as Working), remembered in `localStorage` (`muster.sidebarOrder`).
- **Labels and categories** (task 0044, `labels.rs`, `labels.ts`, `ProjectName.tsx`,
  `InlineField.tsx`): `projects.json` keeps `labels` and `categories` by queue folder, beside the
  gate and the limit; each entry is cleaned when read and set (control characters stripped,
  trimmed, at most 40 and 24 characters), and one that isn't text or is empty is left out without
  losing the rest of the file (`labels::read_labels`, `read_categories`). `Project` carries them
  as set (`label`, `category`, "" for none) and `QueueInfo` as shown (`labels::apply`): the label,
  else the name; the category, else the project's group (`persianUtils`), else "Other" ("other"
  in any case is Other). The queue's `name` stays what commands, `--queue`, task keys' links and
  the window's own bookkeeping use; `muster projects` adds `label:` and `category:` after the rest
  of a line when set. The window calls a project by its label everywhere it names one (the
  `Labels` context, read through its Consumer by `ProjectName` and `TaskKey`, so rows stay
  hook-free): the sidebar, cards and lists with All, Waiting for you, the task pane, New task's and
  the Helper's project pickers, the backlog's quick add, the Graph, the Your check limit's words,
  and each task key's tooltip ("Muster · mu-12", Markdown links too); the real name is the tooltip.
  The window's title is "<label> — Muster" for one project and "Muster" for All
  (`show_project_title`, by the project's name, `labels::window_title`). The sidebar groups its
  rows under category headings (A–Z, Other last; `byCategory`), each with its needs-you count,
  its working tasks and its tasks ("3 tasks in 2 projects"), always shown, nothing folded; the
  order chips order within each heading, and A–Z goes by the label. Double-clicking a row's name
  puts a field in its place (`SideLabelEdit`): ↩ or leaving it saves, Esc keeps it, the folder's
  name typed back is no label. Settings › Projects has a label field (the real name small under
  it) and a category field per row, saved as you type (`InlineField`, its timing in `fieldSteps.ts`:
  after 400 ms, at once on ↩ or blur; Esc puts back what it held and saves that; left, it shows
  what it sent until the save settles), the other projects' categories as chips under the field
  while it has focus. The sidebar's order holds while a label is edited in place. `set_project_label` and `set_project_category` replace
  `rename_project`: the old `names` are still read, so a project renamed before keeps that name in
  commands, but the window no longer sets them. `carry_over` (0005's migrate) carries both.
- **Arranged in the sidebar** (task 0057, `arrange.rs`, `projectOrder.ts`, `SidebarProjects.tsx`,
  `ProjectSwatches.tsx`, `projectColours.ts`): projects and category headings drag to reorder (a
  heading takes its projects), and ⌥↑/⌥↓ moves a focused row; the first move picks the fourth order
  chip, **Mine**, built from the order on screen plus that move. Heading order holds under every
  chip; the chips sort within a heading. Dropping a project on another heading moves it there (at
  the top), "+ Category" makes an empty heading kept only in the window until a project goes in,
  double-click or ↩ renames a heading (a name another heading has merges the two), and empty
  headings go. Each project can have a colour, a left-edge dot on its row: clicking it shows an
  inline row of five swatches (`.proj-swatch`; Lime, Orchid, Pink, Tan, Mint, a shade per theme; `PROJECT_COLOURS`,
  CIE76 ≥ 30 from every stage, blocked, needs-you, priority and gold colour in every theme and
  accent, and from each other). The dot shows wherever `ProjectName` names the project. Settings ›
  Projects has a colour cell and ↑/↓ place cells, in Mine order. `projects.json` keeps `colours`,
  `order` and `categoryOrder` under its lock (`set_project_colour`, `arrange_projects`,
  `rename_category`; unknown entries are dropped when read, a refused change writes nothing);
  `remove_project` clears them and `carry_over` carries them.
- **Headings fold, and a group can be named** (task 0070, `arrange.rs`, `projectOrder.ts`,
  `SidebarProjects.tsx`): a heading's chevron, or ← and → on a focused heading, folds it to its one
  line (below, 0082), in every display mode and under every chip; nothing of its projects shows
  under it (owner, 2026-10-14: mu-70's chips are gone), its needs-you count carries them. A project put
  under a folded heading unfolds it, in the core (`arrange::put_under`, under the lock with the write):
  a drag, ⌥↑/⌥↓, Settings' ↑/↓ and Settings' Category field alike. A heading a change leaves with no
  project the sidebar draws (shown: not hidden, still a queue; 0073) loses its fold, so one made
  again by that name, by a move or a rename, comes back open, and only a heading the sidebar draws
  can be folded. Headings are
  `role="heading"` (focusable, with `aria-keyshortcuts`); the chevron is a `Pill` with
  `aria-expanded`. `projects.json` keeps `folded` (headings, read leniently, `fold_category`;
  `QueueInfo.categoryFolded`) and `groupNames` (a group folder's display name). Renaming a group's
  heading (`rename_category`) sets the group's display name, so its projects, and any it gains,
  go under it with no category of their own; projects whose own category was the old name take the
  new one, as before; renaming it back to the folder's name clears it; a fold and a place move with
  a rename, and a merge is folded if either heading was. A migrated queue outside its group keeps
  the group's heading as its category (`carry_over`). The window's handlers are plain functions in
  `projectOrder.ts` that the components only call (mu-88), each tested with fake setters and a save
  that resolves or refuses: `placeSteps` (Settings' ↑/↓, planned from the move still pending),
  `foldHeading` (a refused fold put back, the refusal in the problem line), `readBack` (a read of
  the queues settles the pending move and the folds) and `runMove` (a refused move takes back its
  unfold).
- **Sidebar headings read as headings** (task 0082, `ProjectList.tsx` `CategoryHead`,
  `projectOrder.ts` `headingCounts`/`countsText`, styles.css): a 1px `--line` divider starts each
  group; the name is 15px, weight 750, in `--text` (a project's is 13px, 550; the owner saw 13px
  650 as the same font), `unicode-bidi: plaintext` for a Persian name; the chevron is a 22px
  `Pill` with `aria-expanded` holding a drawn 14px SVG chevron (`.side-cat-chevron`, 2px stroke in
  `--text`) that points where the text starts and turns 90° when open (no transition under reduced
  motion), mirrored with `:dir(rtl)`. One line of labelled counts, the same open, folded and in every mode: "77 need you ·
  17 projects · 18 working" (needs from tasks and lessons, as the row badge; 0 left out but the
  projects), the needs part in the row's `.count.warn` chip (its digits in `--on-status` everywhere
  since 0087). The chevron, the name and the counts wrap in the heading's own box (`.side-cat`,
  `flex-wrap: wrap`, `gap: 2px 6px`, inline margins 4px; a new heading's hint wraps there too).
  The name (`flex: 1000 1 4em`, `max-width: max-content`, ellipsis, its own `title`) grows to its
  width before the counts get any room and gives way first, down to 4em. The counts
  (`flex: 1 1 auto`, `width: max-content`) ask for "N need you" on one line: where the name's 4em
  and that don't fit beside the chevron (the 150px sidebar), the counts take a second line spanning
  the heading's full width, from under the chevron (mu-212). The needs piece is `flex: 0 1 auto`
  and its words (`.side-cat-needs-words`) may wrap, between words, the chip beside their first
  line, only where even that full line is too narrow (Persian at 150px with a 3-digit count, a
  larger text): never cut, never "…", never past the edge. "· N projects" and
  "· N working" are `.side-cat-part` pieces in `.side-cat-rest` (`contain: inline-size`, so it adds
  nothing to the counts' width; one-line height, `flex-wrap: wrap`, overflow hidden, and an empty
  `::before` of the line's height so even the first piece wraps out of sight rather than showing
  cut); each brings its own no-break space. The tasks in the tooltip, the counts in the heading's
  `aria-label`. Projects indent 14px with `margin-inline-start` in every mode. Folded is that one
  line alone.
- **A project's row holds at the narrowest sidebar** (task 0087, `ProjectList.tsx` `ProjectItem`,
  styles.css): the name (`.side-name`, ellipsis, `unicode-bidi: plaintext`, `text-align: left` so
  an RTL name starts by the dot, not at its box's right end) takes what the badges leave and gives
  way first; `.side-badges` is one `flex: none` group at the inline end
  (`margin-inline-start: auto`), each badge `flex: none` and `nowrap` (the paused "⏸︎ N" never
  splits); Focus's "stalled" is its first piece (8px from the badges, as it sat before 0087).
  `.side-line` wraps (`gap: 2px 8px`), at every sidebar width. The name is
  `flex: 1000 1 auto; width: 4em; max-width: max-content; min-width: 0`: when the line breaks it
  claims 4em, or its own width if that's less ("All", "ai"), then grows into what the badges
  leave, up to its own width. Where that and the badges don't fit, the whole group takes the line
  under the name, and a count growing a digit can move it there. A row that fits is one line,
  drawn as before 0087 (the name's box now ends at its text, so a double-click renames on the
  name itself). Round 2 scoped this to a sidebar
  under 210px (`data-tight`); headless WebKit showed drover's name 0px wide in Focus from 210 to
  about 229px, so it holds at every width now and only drover's row changes at 210 (Focus up to
  about 240, ADHD up to about 229). Wider than the whole row (the running chip and every count at
  150px), the badges wrap among themselves, each whole (`max-width: 100%`). The
  tooltip leads with a label in full when it isn't the project's name, then the real name and
  folder. The row's `aria-label` is the name and every count in words ("drover: 17 need you, 2 in
  progress, 1 paused, 48 tasks; running", "1 needs you" for one, 0 left out but the tasks; in
  Focus also "being worked on now" and "stalled"). Its `aria-describedby` is the tooltip's real
  name and folder (a `hidden` span, since a description doesn't fall back to the title), then
  `.side-more` (hidden when empty), which holds its activity line and Your check limit. The needs-you
  badge, `.count.warn`, draws its digits in `--on-status` everywhere (≥ 4.5:1 on `--st-needs` in
  every theme and accent; `--accent-ink` read about 2.2:1 in light themes). The stalled project's
  "Ask all quiet sessions for a check-in" button wraps its words inside the sidebar
  (`.side-ping button`: `white-space: normal`, `max-width: 100%`); the board's stay one line.
- **One builder for the sidebar's state** (task 0073, `projects::read_state`): the shown queues read
  with their tasks' keys (each project's prefix) and this Mac's settings, then each queue's Your
  check limit, label and heading, and colour, places and fold. The poll (`state` in lib.rs) calls
  it and adds the live dot, Claude Code's session states and notifications. So does the test that
  holds `app/src/dashafzar-queues.json`, the state the sidebar's render test draws (AC2 of 0070),
  to the core: dropping a step fails it. The hidden window's badge reader (`waiting_in`) reads
  on its own, for the Dock badge only.
- **Reaching a working session** (0039, `ping.rs`, `Ping.tsx`, `reach.ts`): **Ask for a
  check-in** under a quiet card ("no check-in for N min"; a strip joined to the card's bottom,
  since the card is a button), in the task pane's **Session** strip (Building and Testing, an
  agent's task the owner doesn't hold) beside **Message the session working on this** (one line,
  500 characters), and **Ask all quiet sessions for a check-in (n)** under a stalled project in
  the sidebar. Each goes through the core to the holder's session mailbox
  (`mail/<session8>/<id>-checkin.md`, `<id>-note-<nnnn>.md`; a teammate's `mail/<session8>/<agent8>/`) and is logged on the task; the answer
  hook hands it over after that session's next tool call, check-in requests first, messages
  quoted as the owner's note. The card shows "asked 2 min ago" in the check-in's place until a
  check-in of that Building stage comes (`checkinAsked`, from the log's ask line). A holder
  without a session stamp, or in Testing for a check-in, shows the button disabled with the
  reason. Not built: the optional auto-nudge after 45 quiet minutes, off by default; it can now
  go in Settings → Agents (0036).
- **Send-back comes first** (mu-66, `ops.rs` `change_locked`, `block.rs` `put_sent_back_first`): a
  Testing → Building change (a review round, `muster return`) skips the one-claim-per-holder check,
  so a review can always send a task back to its builder; the sent-back task comes first (`muster
  next` refuses with "fix mu-59 first", its card says so, the holder's other card says it waits
  behind it). Claiming another task stays refused while you hold one.
- **The owner's QA send-back** (mu-101, `ops.rs` `return_back`, `review.rs`, `ping::sent_back`):
  any return from Your check to Building (the app's Send back to builder, `muster return <id>
  "<why>" --to building`, `@sendback`, an agent relaying the owner) is sent back first in line, as
  from Testing: `sent_back` now covers Your check → Building too, and the `Task` carries
  `sentBackBy` (the return's label, stamp left off; "owner" for the owner) and `sentBackWhy` (its
  reason), both empty for a review's send-back; the holder's other task gets `behindBy` (the
  sender of the one it waits behind), so its card reads "waiting behind mu-82 (sent back by you)". `muster next` says "fix mu-82 first (sent back by
  you: <why>)", or "by <label>", and the card "Sent back by you: <why>". The reason becomes an
  owner finding: lens `owner`, blocking, open, numbered after the last R (no review yet: a new
  store at round 0), `history[0].by` the sender, for every agent's task (none for the owner's
  own). Its `evidence` ties it to the branch, or with no `branch:` (the merge, `pass_merged`,
  clears it) to `commit:` ("found on the merged commit abc1234; muster start makes a fresh branch
  from main, and its review round settles this first"). While such a task has no branch,
  `ops::current` loads `review::owner_to_recheck` (owner findings open, or answered and not
  re-checked) and `pass_change` refuses Building → Testing and Testing → on with
  `review::holds_unbranched`. `parallel::start` takes a Building task with no branch held by the
  caller (`ops::check_restart` before git, `ops::restart_task` under the lock after): a fresh
  `task/<key>-<slug>` branch and worktree from main, the claim and stage kept, heartbeat set,
  logged "started again in worktree … on branch …"; its brief names the owner findings first.
  That branch's round must answer them (`record` refuses a round that skips an answered
  finding), and `merge_check` waits for it. The builder's session gets a mailbox note with
  the reason quoted under `> `; the session is the claim's stamp, else `built_by`'s when its label
  matches. No stamp, or an unwritable mailbox, logs "couldn't tell the builder's session it was
  sent back (…); muster next shows it first", and the send-back stands. Dependents as any return
  from Your check. A plain `muster return` from Your check still goes to Testing.
- **Pause and Resume** (0045, `pause.rs`, `pause_cmds.rs`, `Pause.tsx`): **Pause** or **Resume**
  in the strip under a card (Backlog through Testing; the strip a quiet card's Ask for a check-in
  shares) and beside the stage in the pane's status line, with an optional why typed in place. A
  paused task keeps its stage and claim but leaves its column for the **Paused lane** (0059,
  `PausedLane.tsx`, `lib.ts` `columns()`): the board's first column, before Backlog, always shown
  with its count (a chip in the row above the board when empty or folded, mu-84), a Paused band first under
  Group by; its card says "⏸ was Building · paused 2 h ago · why", and Resume brings it back to its
  column on the next poll. The stage columns' counts and the sidebar's in-progress count leave
  paused tasks out; the sidebar's "⏸ 2" stays. Elsewhere it shows the grey **⏸ Paused** chip with "2 h ago
  · why" (never red, never a stage colour); its claim isn't quiet and its lease doesn't run (the
  card's flags, Release, the sidebar's stalled), Pass is off with why, the timeline hatches its
  paused stretches grey (`pauseHistory`), the Flags filter has **paused**, and the sidebar row
  counts them ("⏸ 2"); the table has a grey ⏸ Paused pill and a "was Building" pill (sorted first by
  Stage), the timeline bar says "Paused · was Building", the graph a paused badge (and never
  "ready"), Waiting for you shows the chip, the owner's own paused task doesn't need them, the
  stage track is off, and the pane's time so far leaves paused time out. In Building or Testing the holder's session is told through its mailbox
  (`<id>-pause.md`, `<id>-resume.md`); the pane says whether it was.
- **Old pauses** (0059, `old_pause.rs`, `OldPauses.tsx`): a block from before 0045 whose reason says
  paused ("owner (paused)") is found in every queue and turned into a pause with its reason and the
  time it was blocked (`muster check --fix-pauses`, below). Once, on load, the board shows an
  inline notice listing them with **Make them pauses** and **Not now**
  (`old_pauses`, `fix_old_pauses`, `set_old_pauses_offered`; the setting `old-pauses-offered`).
- **Claude Code's session states** (0048, `agents.rs`, `sessionState.ts`): the core runs
  `claude agents --json --all` (argument array, `CLAUDE_CONFIG_DIR` set for each account on the
  allowlist (0012), every one of them, each once per read, as soon as a stamp in Shaping, Building or Testing
  names one of them (0182); a stamp with no account or an unlisted one causes no run and reads
  nothing, so its card is unknown and the heartbeats judge, usage reporting off, in a process group of its own that a 10 s timeout stops whole, 1 MB cap) on a thread of its own,
  started by the window's state poll (or the Dock badge's own read while the window is hidden,
  0006) at most every 15 s and never while nothing has a stamped
  holder; the poll uses the latest result and never waits for a run. Parsed with serde into the
  session id and state only (`cwd` and `name` are never kept); an interactive session's `status`
  stands in for its missing `state`, working with an idle `status` is unknown, and a repeated id
  is its latest entry. `agents::holds` counts each session's claims and Testing holds across every
  queue (the app's poll, and `muster list --json` through `projects::active`), and
  `agents::apply_with` gives a task its holder's state when the session id matches in any allowed
  account, not only the one the stamp names (0182: cc-mg's builders resumed on cc-hg with the same
  ids and claims still say `· cc-mg`), and the session holds nothing else. Listed on two accounts,
  working or waiting beats unknown, which beats ended (`Snapshot::find`), then the stamp's account;
  the account it was found on is kept on the task (`session_account`, not sent to the window) and
  names it for the notifications. The live dot, the Work rows and the transcript already look the
  id up in every allowed account's folders (`transcripts::Reader`), so they follow it too. Holding
  nothing else means, in Testing, no claim and one Testing hold; a paused task (0045) gets none and isn't counted as a hold. A Shaping claim (0097) is a claim:
  `stamp_of` takes Shaping, Building and Testing, and `holds` counts a Shaping claim with the
  others (mu-171). On the card the state is on the holder ribbon after the name (mu-171, it was a
  line under it): `HolderRibbon.tsx` `CardHolderRibbon` adds "working" (accent), "waiting for
  input" (a needs-you block, `--st-needs` with `--on-status` words, to the card's edge; the task
  counts as needs you: the core sets `waitingForOwner`, and the card leaves out its "needs you"
  badge, `ribbonWaits`) or "session ended" (Building only), from `sessionState.ts`
  `ribbonStateWords(t, short)`, short "needs you" and "ended". Both forms are in the markup and
  the ribbon's room (`.holder-room`, the strip less 8px, a size container) picks one by
  `@container holder` widths measured in WebKit (128px, 122px; the live dot goes under 101px).
  The card's accessible name reads `sessionWords`' full words ("held by Asghar_7, waiting for
  input in Claude Code"). "no check-in for N min" stays on the card in muted text; the pane's status
  line says the same and "Waiting on you: input in Claude Code"; the sidebar's working now takes
  Claude Code's word over the transcript and heartbeat; Ask for a check-in is disabled with
  "waiting for input in Claude Code: answer it there first", and Ask all leaves those out and
  says so. Unknown, unmatched or unreadable: the card reads as before. Settings › Agents shows
  one line under the LIVE window from `sessionStates` ("Claude Code's session states: on (2
  accounts)" / "not available: <why>"). One real run (2.1.296, 2026-10-13,
  this Mac, 2 and 6 sessions listed): 0.13–0.18 s, 0.5 and 1.5 KB of output; the task's note had
  seen several seconds on one run, which the 10 s timeout and the background thread cover.
- **Sessions** (0098, `sessions.rs`, `Sessions.tsx`, `SessionRecap.tsx`, `sessionsData.ts`;
  decision 0017): a project view beside Board, Table, Timeline and Graph, for one project or All.
  - **The list.** `agents::read_lists` runs the same `claude agents --json --all` (same `Fetch`,
    argument array, `QUIET_ENV`, 10 s, 1 MB) for every allowed account, and `agents::parse_list`
    keeps id, name, cwd, `startedAt` (ms), the state word and `waitingFor` (`ListEntry`; the cards'
    `Entry` is unchanged). `ListState` groups them as the agents view does: `blocked`/`waiting` →
    Needs input; `working`, or an interactive `busy` → Working; `done`, `stopped`, `failed`,
    interactive idle, a background `working` whose process is idle, and unknown words →
    Completed, with the word kept and shown. `agents::plain_line` makes names and messages one
    line: control characters and every bidi control (`index::is_bidi_control`) stripped, cut by
    characters.
  - **Row message**, from the transcript's last 64 KB (`transcripts::read_tail`, cached by path,
    size and mtime, dropped when the transcript goes). Needs input: `approve <Tool>: <gist>` from
    the tail's pending tool call (its command, path, address, pattern or query; the model's
    description only when it has none of those), else the last text, else `waiting for <waitingFor>`. Other groups:
    the last text. The state word when there's nothing. 160 characters. The row's time is the
    transcript's mtime, else its start.
  - **Project.** `cwd` is matched in the core against each shown project's folders, by whole
    components, deepest first: a `.muster` queue in main/master uses the project folder (which
    holds `worktrees/`), a checkout that is its own project uses that checkout and
    `<checkout>.worktrees`, and an old `<project>/tasks` queue uses the project folder. Only the
    name goes to the window ("" = Other). One project's list keeps only its rows.
  - **Polling.** `sessions::Lister` starts a read on its own thread when the view's `sessions_list`
    poll (every 3 s while the window shows) finds the last read 15 s old, never two at once, and
    hands back the latest. While the view is closed nothing runs. Rows of accounts no longer
    allowed are dropped at poll time. A read that landed more than `agents::MAX_AGE_SECS` (40 s)
    before the poll (the view was closed meanwhile) isn't shown: "Reading sessions…" until the
    new one lands. The recap's own 15 s refresh polls the Lister too, so a recap left open on
    another view follows the session's state. 60 tail reads: 1.9 ms on a release build.
  - **The window.** A header count ("8 need input · 0 working · 37 completed"), then Needs input,
    Working and Completed, always in that order with their counts; an empty group says "none".
    Newest first. Completed rows older than 24 h fold into one "Older: N" chip that opens them in
    place. Each row is one button (`aria-label` "name, state, age, message"; under All "name, project,
    state, age, message", the project as the row shows it): name, the project's
    dot and name under All (Other for none), the message, the age at the inline end. Names and
    messages are plain text with `dir="auto"`. Needs input has the orange edge and `--warn` words.
    Arrows, Home and End move between rows, Enter opens the recap, Esc closes it and focus goes back
    to the row. "Reading sessions…" in place until the first read lands. The words are in
    `sessionsData.ts` `WORDS` (the app has no strings table of its own). Modes (`modes.ts`):
    `session-groups` (Focus shows Needs input and the other two as chips with counts, no count
    while the first read runs) and
    `session-history` (Standard and Focus fold the recap's History to its header).
  - **The recap** (`session_recap`), in the task pane's dock with the list still live beside it: a
    strip with the name, Transcript and Close; the chain line ("Asghar_4 → Asghar_5 → Asghar_6",
    this one bold); then Who (name, id, account, project and checkout folder name, state and age),
    Tasks it worked on (every task whose `session_ids` include it, with key, title, stage and
    "holds it", each a link to the task), History (its log lines in those tasks, by `work::log_who`:
    claim, check-in with %, pass, return, takeover, note, plus handoff-from and handoff-to, in time
    order), Time and Tokens (`transcripts::Reader::usages`, the session's and the chain's sums, and
    their counted tokens' share of `est-tokens` when it worked on one task, cache reads shown
    beside them and never counted, as in the Work section since mu-172; log stamps read in-process by
    `queue::local_epoch`, the C library's `mktime`, no `date` per line), and Progress (its latest check-in per
    task, else 0025's guess, labelled a guess). Transcript (`session_markdown`) shows 0025's folded
    Markdown under the recap, never in its place.
  - **The chain.** A session's first prompt (`transcripts::read_first_prompt`, 128 KB head) naming
    `/handoff resume <path>` (the slash command's `<command-args>`, or typed) links it to the
    handoff's `session:` (a uuid, a short id, or `34fa2f95 (QA_5)`); a resumed handoff is renamed
    `.done.md`, so that name is tried too. Forward, a listed session that resumed a handoff whose
    `session:` is this one; of two, the one the handoff names as `successor:`, else the earliest.
    At most 32 each way, each session once. The handoff is read only as a regular file (no link),
    at most 256 KB, canonically directly in a `.claude/work/handoffs/` folder under `~/playground`,
    and only front matter that closes. A predecessor's transcript (its first prompt, its time and
    tokens) is read only when an allowed account vouches for it: the latest list of an allowed
    account has it, or a task stamps it with one (`SessionId::readable`, as the Work section
    does). Otherwise it's a link by id, counted "not read on this Mac", and the walk back stops
    there: `~/.claude-hg/projects` links to the company account's folder on the owner's Mac, so
    a transcript found through an allowed folder proves nothing.
  - Nothing of it is written anywhere; `cwd` never crosses to the window; no network.
- **Projects** (in Settings, or **+ Project** under the project list): every project with its folder, task count and where it came from (found
  in `~/playground`, or added). Discovery (`queue::discover`) looks at `<project>/tasks` and one
  level deeper, `<group>/<project>/tasks`, and no further, and at a repo's `<checkout>/.muster`
  at the same two levels, in main checkouts only (see `.muster/` below); it passes over folders named `.*`, `_*`,
  `node_modules`, `worktrees`, `*.worktrees`, `target` and `dist`, follows a symlink only while it
  stays inside `~/playground`, and keeps only queues: a `.muster/` with its `config.toml`, or a
  folder with a `README.md` and `todo/` (or `tasks.json`). A nested queue is named by its own project folder and carries its group (the
  folder above it). A queue both found and added shows once, as found. `muster projects` marks
  each line found or added. **New queue…** picks the project's repo (its main checkout or any
  worktree of it) and makes a local `.muster/` in the main checkout, as `muster init` does
  (`projects::new_queue`, task 0004); it refuses if `.muster/` is there already ("To show it, use
  Add existing queue.") and in a repo without a main checkout. A folder outside any repo still
  gets the older `tasks/`: status folders, README with the rules, `WAITING-FOR-OWNER.md`, the
  tools' stand-ins in `bin/`, and its own git with a first commit, never over an existing
  `tasks/`. **Add existing queue…** picks any queue folder. The label and category fields
  (0044), Hide/Show, and Remove (added ones only) never touch files. Each project's **Prefix** (0033) shows with **Set** beside
  it: stored in its queue (a `.muster/`'s `config.toml`, an old queue's README.md), refused (why shows under the field) when it's bad or
  another project's; New queue stores the default. Saved in `~/Library/Application Support/ai.sweetlemon.muster/projects.json`.
  Folders are picked by the core's own dialog, so the window never hands the core a path.
- **Settings** (task 0036, `Settings.tsx`, `settingsData.ts`, `settings.rs`): a view in the header's
  row (never a window), its groups docked on the left (Appearance, Display, Projects, Agents,
  Notifications, About; `SETTINGS_GROUPS` in `settingsData.ts`), the open
  group's settings on the right. Appearance is only here and Help only in the header: on the
  owner's word (2026-10-13) the header's one-click Appearance button and Settings' Help group
  (links into the manual, which the Help tab and every ? already open) were removed; the old
  Appearance pane and Projects tab are gone, their contents moved here. Paths under the home
  folder (About, the Agents settings file, Projects' folders) show as `~/…` (`homePath` in
  `lib.ts`, `HomePath.tsx`; the core sends `home` in `settings_info`), the real path in the
  tooltip and on the clipboard when copied. **Agents** has the
  Claude accounts Muster reads (0012: "Claude accounts Muster reads: cc-mg · cc-hg" as chips with
  a remove ✕ and an inline add field, `set_setting` `claude-accounts`, a label refused with why;
  "Never ~/.claude, the default account." under it; an empty list says "none: Muster reads no
  Claude sessions"), the helper's account and model, the **claim lease** (default 12 h, 1 h to 7 d) and the **LIVE
  window** (default 5 min, 1 min to 1 h), typed as `30m`, `2h`, `1d` and checked by the core.
  Both are saved per machine in `settings.json` beside `projects.json`; `muster reclaim` (with
  no `--after`) and `muster check` read the same file, the app reads it on every poll (Release,
  the card's "quiet" and dot, the pane's LIVE), and a missing, broken or out-of-bounds value is
  the default, field by field. **Roles** (0112, `Roles.tsx`, `rolesData.ts`) lists each role
  of `roles.toml` with its name, title and slot count. The title is in the language shown
  (`roleTitle`, mu-195): a built-in role whose title is still the core's default reads as shell.json
  `roles.title.<id>`, and a title changed by hand, or an unknown role's, shows as written. The id
  isn't in the row; the open one has it as its claim hint (`--role coder`, left to right). Its
  line (`roleLine`) says the title, stage, permission and stage moves in the language shown too:
  the stages as the board names them (`stageLabel`), the permission as shell.json
  `roles.permission.<id>` (`PERMISSIONS`, pinned to roles.rs by a Vitest), a move as
  `roles.move` (→ in en, ← in fa); a stage or permission the window doesn't know shows as written.
  The open one shows its name, models, providers, slots and turn, budget and context caps as fields, each saved by `set_role` (`roles::edit`,
  which checks the whole team and leaves the file as it was on a refusal) and the queues read
  again so a rename shows at once. The poll sends `roleNames` (id → name) with the state, read
  from `roles.toml` only when its modified time or size changes, and cards (`whoLine`), the
  pane (`person`, `shownHolder`, History, the Next step's notes, check-ins and comments), the
  Work rows (`SessionRef::slot`, `slotLabel`), question askers (`Question::slot` from a stamp of
  the asker's label and session, `askerShown`), the holder ribbon (`HolderRibbon.tsx`), the table, the graph, Recently finished and the
  built-by line (`nameOf`) show a stamp's `slot coder-2` as the
  role's name now (`Kaveh-2`); a stamp without a slot part, or a refused file, shows the label
  as written. **Projects** also shows each project's gate command (0023),
  read-only, with the `muster projects set` line when none is set, and under it, in the same
  row and look, its run command (0077): `projects.json`'s `run`, by queue folder like `gate`, set
  with `muster projects set <name> run "<command>"` under the gate's rules (`split_words`, one line
  with no direction marks or overrides, at most 500 characters, "" clears it), listed by `muster projects` and `--json`, carried
  by `muster migrate`, and read leniently (`projects::read_commands`: a file without `run`, a
  `run` that isn't an object, or an entry `set_run` wouldn't keep reads as none, and the rest of
  the file loads). The command shows left to right (`dir="ltr"`), with Run beside it (0079,
  `Run.tsx` `RunControls`): Run, or while it runs a "running" chip with Stop and Restart. Muster's
  own is `bin/run`: it builds the release app (`pnpm --dir app release:build`) and becomes that
  build's binary in `app/src-tauri/target/release/bundle/macos/Muster.app` (or under an absolute
  `$CARGO_TARGET_DIR`), the `Info.plist`'s `CFBundleExecutable`, with `exec` (0079): it starts
  beside the installed Muster, in the run's own process group, so Stop ends it. `open -n` would
  hand it to launchd, where Stop can't reach it. Your check's view buttons
  (0043) open Settings for Appearance, Projects and Settings. **About** shows the version, the
  settings folder and its files, and every folder the app reads: the queues found on their own,
  then the ones added by you (or that there are none), the allowed Claude account folders and
  that only projects under `~/playground` are read (0012), with paths under the home folder as `~/`.
- **Display modes** (task 0038, `modes.ts`, `ModeViews.tsx`, `Display.tsx`): **ADHD** (the default:
  today's screens), **Standard** and **Focus**, picked in Settings › Display (three chips and a
  table of every element by mode, the current column lit), shown as a chip at the end of the
  header that opens that page, and toggled by ⌘. (Focus from any mode, back to the one before).
  Kept in `localStorage` (`muster.displayMode`, `muster.displayMode.before`). `modes.ts` is one
  table of the elements a mode could change (filter bar, Who and Group by, card lines, the Done
  line, board columns, Recently finished, task pane sections, sidebar), each with its behaviour,
  words and how to get back what it folds;
  components read it through `useLook` only, and `modes.test.ts` fails on a read of an element
  the table lacks, on a mode read around it, and on a row nothing reads. Standard: the filter
  groups and Sort behind **Filters (n)**, opened in place; Who and Group by as their value; brief
  cards (who, check-in, branch, tags, parent, estimate and files on the selected card); Vision,
  Review, Work, and the Log and check-ins folded to a header each. Focus: Building, Testing and Your check
  open, the other columns chips in the row above the plain board with counts, needs-you and blocked
  counts (a click opens one, remembered in Focus until it's folded again, mu-84), and strips in the
  grouped board's bands, whose Backlog band folds to its head the same way (a strip's or band's
  needs-you badge titled "1 needs you" / "2 need you" by `needsWords`, its button named with its
  counts in words by `placeWords`, "Backlog · not in a feature yet: 1 item, 1 needs you, 1
  blocked", mu-159); cards are the title, one status line and blocked, quiet, needs a tester or needs you; the filter bar is a **Search and filters ⌘F** button until ⌘F, and
  stays while a search, filter or Who hides tasks; the sidebar is 150 px of category headings with their
  needs-you count, names, a working dot,
  stalled (with its Ask all quiet sessions) and needs-you counts; Recently finished (0051) is the row's last chip with its name and count, which
  opens it in place (Fold puts it back), and reads no costs while folded; the Done card's "done 4 min ago" line
  shows in all three; the pane folds as Standard. Needs-you counts and badges show in every mode. A pane fold keeps its
  section mounted in every mode (ADHD's wrapper is `display: contents`), so typed text survives a
  fold or a mode switch; the folded Log's header counts its lines and check-ins.
- **Themes and accents** (Settings' Appearance): 7 themes (Violet night, Violet day, Graphite,
  Midnight, Paper, Snow, High contrast), with Match the Mac picking one dark and one light, and 6
  accents (Violet, Teal, Rose, Gold, Green, Graphite). No blue, no orange. They apply live and are
  remembered. Default: Muster's violet, following the Mac. A test checks every theme and accent
  pair: text 7:1, secondary text 4.5:1, accent-coloured words 4.5:1 (they use the strong shade),
  button text 4.5:1, fills 3:1.
- **One class name, one component** (task mu-81, `cssClasses.test.ts`): the project colour picker
  has its own `.proj-swatches`/`.proj-swatch` (it had reused the theme picker's `.swatches`, whose
  later rules turned the seven theme cards into 16 px dots). The test reads `styles.css` and every
  component, and fails when a class two components use gets rules that set one property two ways.
- **Drover's names** (task 0053): the CSS variables carry Drover's token names (`--panel`, `--line`,
  `--hover`, `--selected`, `--accent-ink`, `--focus`, `--bad`, `--warn`; table in DESIGN.md), with
  Muster's values; the old names stay as aliases in `styles.css`'s `:root`. Buttons, chips, stage
  pills, badges and the pane's value picker are atoms in `app/src/ui` shaped like Drover's (`Button`,
  `Pill`, `StatusPill`, `Badge`, `Segmented`/`Segment`, classes `ui-<name>`), drawn in Muster's own
  look. `ui/adoption.test.ts` fails on a screen using the old classes or a plain `<button>`.
- **Look** (Muster task 0010): a violet palette (accent `#8B6CF0` dark and `#6D4AE0` light, with
  violet-tinted backgrounds and surfaces), chosen because orange belongs to Drover and Braindump
  and blue to 127. Stage colours (task 0034, colours people know from boards): Backlog grey,
  Shaping blue, Ready cyan, Building the accent (violet when the accent would pass for
  another stage), Testing amber, Your check and "needs you" orange, Done green, Blocked red. No
  two stages are closer than a colour distance of 30. Contrast is checked to WCAG AA. Dark
  buttons use dark text on violet. In the light theme, status colours are fills behind dark
  text, with darker variants for coloured words.
- **Icon (task 0085, the owner's set of 2026-10-14):** the robot head with a check, violet on a
  near-black gradient (dark) or on lavender (light). The owner's files are kept byte for byte in
  `app/src-tauri/icons/original/` (named so because the worktree guard refuses commands with
  the word for it): the SVGs, `png/` 16–1024, `Muster.iconset/` (its 2x files end in `-2x`), the
  menu bar templates and `web/` (favicons, apple-touch and 192/512 icons, stored for a future site;
  the manual has no HTML). macOS 26 shows the light or dark app icon with the system's look:
  `icons/Muster.icon` (Icon Composer: the robot as a light and a dark layer on the two fills)
  compiled by `actool` into `icons/Assets.car` (Aqua, DarkAqua and tinted renditions), copied
  into the bundle by `bundle.macOS.files` and named by `CFBundleIconName` in `src-tauri/Info.plist`,
  as Braindump does. `icon.icns`, the flat icon for older macOS, is built by `iconutil` from a
  copy of `original/Muster.iconset/` renamed to `@2x`, so it holds all ten sizes, 16 to 512@2x
  (task 0089; actool's own fallback held only 16 and 128, so Finder upscaled it); rebuilt, it's
  byte for byte the same, and a tray.rs test reads its chunk table for the ten. The other PNGs and
  `icon.ico` come from `tauri icon` on the dark 1024 PNG (dark is the default theme).
  `icons/build-icons.sh` rebuilds them all (Xcode 26); the outputs are committed, so builds don't
  need Xcode 26. Icon Composer's `ictool` preview crashes on this file (an assertion in
  IconComposerKit), so the check is `xcrun assetutil --info icons/Assets.car` and the Dock.
  The sidebar's logo is the same icon as an SVG (`app/src/logo-{dark,light}.svg`, the owner's
  without its C2PA metadata), picked by the theme's mode (`data-mode` on the page, set by
  `apply()`). Release size with all of it, 2026-10-14: the app 10.3 MB (was 9.6 MB: Assets.car
  660 KB, icon.icns 100 KB smaller; +740 KB in all), the binary 10.11 MB (was 9.93 MB: +178 KB for the tray
  and the two images).
- **Icon before 0085:** a bot face with a visor eye and a check, drawn by capture as a vector
  (the queue's `attachments/0010/`). In use: **B**, the lineup's style (violet bot on near-black
  `#16131F`, eye and check cut out), like Drover's orange crook on navy. **A** (light face on a
  `#2E1F66`→`#170F38` gradient) is ready if the owner prefers it. `dock-preview-drover-a-drover-b.png`
  shows both next to Drover's. The earlier dog-face mark and its recolourings are in
  `attachments/0006/`.
- **Layout: nothing covers the board.** A task, a backlog note and New task open in one pane docked
  beside the content, with the board still visible and live beside it (the owner: "need to have
  everything in front of my eyes"). Drag the edge to resize it; the width is remembered. Esc
  closes it. The projects sidebar resizes the same way (0058): one splitter (`ui/Splitter.tsx`,
  rules in `splitter.ts`) on both, drag, double-click for the default, ← → (Shift for 120 px).
- **The row of folded columns** (task 0084, the owner's choice A; `ColumnRow.tsx`, `modes.ts`
  `plainBoard`): every place of the plain board that isn't open (the Paused lane, a stage, Recently
  finished) is a chip in one row under the filter bar, in board order: its stage dot, name and
  count, and "2 need you" and "1 blocked" when there are any. A place is in the row when it's
  empty (every mode), when the mode folds it (Focus: all but Building, Testing and Your check, and
  Recently finished) or when the owner folded it. The row is a labelled group of buttons, each
  read out with its counts in words, and keeps its one line of a fixed 34 px ("Every column is
  open" on the plain board, empty under Group by) so nothing moves: it never wraps
  (`flex-wrap: nowrap`; the one exception, set by width alone, is the board's edge cue's own line
  under 30em, mu-198 below); short of room the names shorten to an ellipsis (a one-line clamp that
  breaks anywhere, `word-break: break-all`, at least 2.5em) while counts and needs-you and blocked
  stay whole. That's only while it makes the chips fit: `measureRow` works out the least room they
  can take (`leastRoom`: their boxes less what each name could still give up), the same whether
  they're drawn whole or short, and past the row's room the list gets `col-row-scrolls`, whose
  chips keep their whole names and scroll sideways in a list of their own (`.col-row-chips`). Then the row ends with a cue that never
  scrolls away (`RowCue`, review R7): "+3 more · 1 needs you", read as "3 more columns past the
  edge, 1 needs you. Show them"; its first click scrolls to and focuses the first hidden chip that
  needs you (past the far edge first), else the next one past the edge. Each click after steps in
  board order to the next hidden chip past the one the last click went to, measured in view or not,
  so a chip wider than the list, never wholly in view, doesn't hold it (R8); with nothing past it,
  back to the start, the first hidden chip, so a round reaches every chip once (QA_4). Task 0086:
  ColumnRow first passed the last target only while the view measured it in view or cut, so at a
  board of 400px or less (nothing fits beside the cue, and a scroll's measure can miss it) each
  click counted as a walk's first and went back to what needs you, landing on the same chip or
  skipping the plain chip after a needs-you one; now it keeps the target as it is, and a target
  that's no longer a chip (its column opened) walks on from the list's start. The row is keyed by project and mode, so a new one starts scrolled to its start.
  `rowCue.ts` `watchRow` measures on a ResizeObserver (row, list, each chip) and on scroll, once a
  frame, never on a poll that changes no size; a chip is in view only with both edges inside
  the list's box, so it reads the same right to left, and the cue shows only when the chips need
  more than the whole row even shortened, so it doesn't keep itself there (`measureRow`, `overflowCue`). App builds the chips with `rowChipsOf` and counts each place with `placeCount` (Backlog
  with its notes, Recently finished its week of the kind its chips pick: App keeps that pick,
  `muster.recentKind`, so the chip, the list and whether the rail opens by itself agree); the
  grouped board's fold click is `toggleBand`. The board's grid holds only the open columns, at most 400 px each; an open column's head
  (the Paused lane's, Recently finished's too) has a quiet **Fold**. The owner's opens and folds
  are kept per display mode in `localStorage` (`muster.columns`, read with `loadPicks`, which
  drops unknown modes, places and values), only where they differ from what the mode does
  (`pickColumn`), so a mode switch resets nothing. Backlog stays open empty for its quick add, Your
  check while it has Accept all's result or its limit. With Group by, the bands keep 0083's strips
  (a column the owner hid is folded there too) and the row holds only Recently finished.
- **The board's own cue: columns past the edge** (task mu-198, the UX spike's R4; `rowCue.ts`
  `edgeCue`, `measureBoard`, `watchBoard`, `scrollDelta`; `ColumnRow.tsx` `BoardCue`,
  `edgeColumnsOf`). The plain board (`main.board`, App hands ColumnRow the element through a
  callback ref, `boardEl`, only while it shows: not in the Feature grid or under Group by) is a
  grid of its open columns, each `data-col` marked, that scrolls sideways. `watchBoard` measures
  it with the row's machinery (`watchScroll`: a ResizeObserver on the board and each column, a
  passive scroll listener, once a frame) and `measureBoard` says which columns are wholly inside
  the board's client box and which an edge cuts; content that fits (`scrollWidth` within a pixel
  of `clientWidth`) is all in view. `edgeCue` takes the open columns (`edgeColumnsOf`: the stage
  names, Paused from `board.json` `legend.paused`, and each one's `needsYouCount`, as its head
  counts it), hides the ones not wholly in view, names those that need you first and walks with
  `overflowCue` (generalised from chips to anything with an id and a needs count). The Pill after
  the row's cue (`col-cue col-edge`) reads "Past the edge: Your check · 21 need you, Done"
  (`board.json` `edge.lead` with `<list/>`, `edge.item` with the name and the `count warn` badge,
  `edge.sep`), its label "2 columns past the edge: Your check, 21 need you; Done. Show them"
  (`edge.label`, plural) and its tooltip the whole text. A click scrolls only the board
  (`scrollBy` by `scrollDelta`: the column's start to the view's inline start inside the board's
  padding, its right edge in right to left; `auto` with reduced motion), keeps the focus, and puts
  "Your check is in view" (`edge.live`) in the row's polite live region, there while the board
  is. The cue sits in a slot (`EdgeSlot`, `.col-edge-slot`) rendered while the board is, cue or
  no cue: `flex: 1 1 0` (the row's free room, never sized by the cue's words; the chips' list is
  `flex: 0 1 auto` and doesn't grow), at least 13em; `watchRow` takes that min-width and the row's
  gap from the chips' room (`slotReserve`, `measureRow`'s `reserved`) while the slot is on the
  chips' line, so the chips' answer never changes with the cue's words. The cue never cuts a name
  (review R1, R2): `watchEdge` (a ResizeObserver on the slot and two hidden, `aria-hidden` probes
  holding the whole words, one frame each) gives `edgeFit`: "line" when the one-line probe fits
  the slot, "wrap" (`col-edge-two`, 11px on 13px lines, padding-block 0: two lines fill the row's
  28px) when the wrapping probe takes two lines and no word sticks out, else "count" (`edge.count`,
  `edge.countNeeds`: "4 columns past the edge · 265 need you", on up to two lines, which 13em holds
  in en and fa). The probes hold every name whatever is drawn, so the form never feeds back on
  itself. The needs-you badge is `white-space: nowrap`. App wraps the row in `.col-row-box`, a
  size container (`colrow`): under 30em (the task pane open at 1100) the row wraps, 62px, and the
  slot takes a full line of its own under the chips and the row's cue. A wrapping row lines items
  up by their flex basis, so there the chips' list is `flex: 1 1 0`: it shares line 1 with the
  row's cue instead of taking it alone and pushing the board's cue to a third line over the column
  heads (review R6; 96 headless runs, en and fa, half with Building folded, polls changing every
  count: every row item inside the row). ColumnRow
  loads the `board` namespace itself (`useT`), so the first cue never draws empty. Persian in `fa/board.json`
  ("بیرون از لبه‌ی بورد"). Headless WebKit on the synthetic 944-task fixture
  (`attachments/0198/`): ADHD and Standard at 1440 "Past the edge: Your check · 258 need you,
  Done", one click scrolls 191 px and Your check is wholly in view; at 1100 one click (to Building,
  the first hidden that needs you) scrolls to the end, 462 px, with Your check in view too; Focus
  at 1100 one click, 206 px; Focus at 1440 fits, no cue.
- **Columns scroll without scrollbars** (owner, 2026-10-13: "hide the scrollbar, in the columns
  doesn't feel good"): `.board .cards` hides its scrollbar (`scrollbar-width: none` and
  `::-webkit-scrollbar`), plain and grouped board; other scroll areas keep theirs. `ColumnCards.tsx`
  draws every column's list: while it overflows, a strip above and below it (18 px, fixed, so
  nothing moves) holds "+N above" and "+N below" (muted text; a click scrolls one screen, smooth
  unless reduced motion), and the edge with more fades (a mask, so it takes the column's own
  background). `scrollMore.ts` counts the cards (task and note cards) wholly or partly out of
  sight (`moreAround`, a pure function); `watchMore` measures on a passive scroll listener and a
  ResizeObserver on the list and its children (a MutationObserver follows children), one
  measure per frame, all released on unmount. Grouped bands grow to fit, so their columns never
  scroll and show neither.
- **Search, filters and sort** (task 0032): one bar above Board, Table, Timeline and Graph; ⌘F
  stays in whichever of them is showing. Group by (0016) sits on the same bar.
  - **Search** (always there, ⌘F, Esc clears): words in any order, case and accents aside, in the
    id, title, tags, text and holder; the matches are marked on cards, backlog notes and rows. A
    word of digits only (0–9) is an id (19 finds 0019) and never matches text, so dates in logs
    don't swamp it; a key (mu-19, MU-19) finds that project's task (0033). The core builds each task's search text when it reads the file (title, tags,
    body without front matter, folded, each word once, digit-only words out, 4 KB at most),
    cached with the task, so the window filters in memory (1,000 tasks well under a frame,
    measured in a test). The text isn't in the 2 s `state` poll or `--json`: the window asks for
    it (`search_index`, one project or all) only while the search has words, and again only when
    a task in view is new or its `modified` changed. ⌘F only (Ctrl+F stays a text field's
    "forward a character"). Esc in the search box stays there: it clears the words, or leaves an
    empty box, and never closes the pane beside the board or throws away a New task draft.
  - **Filters** as chip groups: Stage, Type, Priority, Tags, Held by, Flags (blocked, needs you,
    open questions), Captured / Changed / Done (today, this week, this month). Held by is whoever
    holds the task in its stage (0030: the claim in Ready and Building, `held-by` in Testing and
    Your check, the owner there when it names nobody), shown as the stamp's label: `wt-a (wt-0030
    · session 7b5e8856 · cc-hg)` is `wt-a`, in the app and in `muster list`. Each group shows
    only the values the project has. Any chip within a group, every group together. The line
    under the chips reads what's active ("“flicker” · Building · high · #design"), each with ✕,
    then "36 hidden by filters" and Clear all; it keeps its place when nothing is active. Backlog
    notes follow the search and hide under any filter but a Stage one with Backlog.
  - **Sort:** the board's Sort chips order every column (priority, the default: needs you first,
    then high to low, Done newest finished first by `doneAt`, else `doneOn`, then latest changed,
    0051, in `muster list --sort priority` too; newest, oldest, changed, estimate, id), inside
    each Group by band too. The Table sorts by its columns; the Timeline and Graph keep their own
    order.
  - **Saved views:** + Save view keeps the search, filters and sort under a name, per project, in
    local storage, as a chip; ✕ deletes it.
  - **`muster list --search --stage --type --tag --holder --blocked --sort`** gives the same
    tasks (`search.rs`; values joined by commas mean any of them). The rules are written once in
    `search.rs` and `search.ts`, and `app/src/search-cases.json` holds both to the same answers
    (`tests/search_cases.rs` runs the real command on it, `search-cases.test.ts` the app's filter).
- **Three views of a project:** **Board** (the kanban below), **Table** (every task as a row:
  project, id, title, type, priority, status, who's on it or what it waits on, captured, changed,
  tags; click a header to sort) and **Timeline** (0031: scale chips Day · Week · Month, Week by
  default and remembered, with ◀ ▶ and a way back to the current period. Day is one day in hours,
  Week seven days in quarter days, Month the calendar month in days. Each task's bar is waiting
  (captured → first claim) in a quiet tint, then one segment per stage it held, in the stage
  colour, from the stage history's stamps; tasks with dates only fall back to whole days with a
  faded start. A now line runs across the chart and open tasks run to it. Bars are worked out in
  `viewData.ts` in wall-clock minutes, so a clock change doesn't bend them). All three, and the
  Graph (0019), share the search and filters and open tasks in the docked pane.
- **Legend on the Timeline and the Graph** (mu-173): one `Legend` component (`app/src/Legend.tsx`,
  a `role="group"` "What the colours mean" with a list of swatch + name). The Timeline's sits in
  `.tl-strip` as the second line under the scale toolbar (not inside it), outside `.tl-scroll`, so
  it never scrolls away. It wraps onto a second line when the window is too narrow for it (at
  least one 20px line, items never split), so no item is ever out of sight; nothing in it scrolls.
  A hidden copy of every item the view can draw (`TIMELINE_MARKS`, `GRAPH_MARKS`) shares the
  list's grid cell (`.legend-reserve`: `visibility: hidden`, `aria-hidden`, nothing focusable), so
  the row is as tall as the full set wraps to at this width and stepping between windows never
  moves the chart below it.
  `legendMarks(rows, now)` (`viewData.ts`) lists only what
  the placed bars show, in a fixed order: Backlog, Shaping, Ready, Building, Testing, Waiting for
  your check (the dotted tail), Done (the edge), Waiting, Start not certain (fuzzy), Blocked, Paused,
  Now. Every done bar is the Done outline with no done segment: a task done before stage history
  was logged draws its captured-to-done span with no segment, since no stage of it is known. The
  Graph's sits with the Hide done bar in `.graph-strip`, outside `.graph-scroll`, so it stays in
  view too (6px of the scroller's top padding keeps a selected or focused top node's outline whole), and follows Hide done: `graphMarks(graphs)` (`graph.ts`) names the stages of nodes
  and unlinked chips, Ready to start, Blocked, Your own task (the You ribbon) and A loop, which
  uses the same `isLoopEdge` (an arrow to a rank no later than its own) the drawing routes below
  the nodes. Each swatch is built from the view's own classes (`tl-bar`/`tl-seg s-<stage>`,
  `tl-block`, `tl-pause`, `tl-now`, `g-node`, `g-edge loop`) by a closed switch over mark ids, with
  legend-scoped CSS only for size and place, so a colour change can't drift from its legend. The
  names are the board namespace's `legend.*` keys in the i18n layer, English and Persian. The paragraph under the chart keeps
  only what a swatch can't say.
- **Help** (tab, task 0035): the manual in `docs/manual/`, eight pages (Start here, Concepts,
  Tutorials, In the app, Commands, Shortcuts, For agents, FAQ), bundled at build time and rendered
  with the same sanitised Markdown as task text. A docked sidebar lists every page and its
  sections, the current one lit, under a search box over all pages (an in-memory match); ◀ ▶ step
  between pages; links between pages and sections move within the tab. Commands is made from
  `help.rs` (`help::commands_page`, grouped: tasks, stages, claims, questions, backlog, levels,
  projects), and a Rust test keeps `docs/manual/05-commands.md` equal to it
  (`MUSTER_WRITE_MANUAL=1 cargo test commands_page` rewrites it). Shortcuts is made from one table,
  `app/src/shortcuts.ts`, which the key handlers read (`pressed`; the window's Esc, ⌘N and ⌘F
  and the search box's Esc in `keys.ts`; ⌘F takes ⌘ only), and a test fails when a handler
  acts on a key the table doesn't list for its file; `pnpm test -u` rewrites
  `docs/manual/06-shortcuts.md`. Another test fails on a link to a page, section or repo file that
  doesn't exist. A small **?** beside each pane's title and in each view's toolbar opens its
  section, with the pane left in place. Click a command line to copy it. The manual has no
  screenshots yet.
- **Stages** (task 0014, decision 0003): Backlog, Shaping (product, tech and UI agents), Ready,
  Building (code agent), Testing (QA agent), Your check (the owner), Done. `stage:` in the file is
  the truth, written only by `muster`. A stage change is logged as `<time> <who>: stage <from> → <to>`,
  and those lines are the stage history. Blocked is a flag (`waiting_on`, with its kind in
  `blocked-kind:`, task 0024) that keeps the task in its
  stage. `pass` moves it on, refusing when the task is blocked, when someone else holds it, or when it
  has moved since you looked (`--from`). It also refuses Building → Testing without the gate's line
  (or, for a task, without a summary, mu-78), and Testing → Your check with any criterion unticked. `return` sends it back with a reason and
  hands Building back to its last builder. In the old layout `status:` and the status folder are a
  mirror of the stage for older readers (Drover), and `check` reports a stage whose status doesn't
  match, which is what a hand move leaves. In `.muster/` (0005) no `status:` is written, and one
  left in a file isn't read: the status the window and `muster list --status` use is worked out
  from the stage.
- **The tester isn't the builder** (task 0047): `pass_change` (so `pass`, the merge's
  `check_merge_pass` before any rebase or gate, and `pass_merged`), `set_stage` and `move_task`
  (past Testing) refuse the builder of an agent's task, matched by `session::same_holder` against
  whoever last claimed it (`ready → building (claimed)`), every holder a takeover moved the claim
  from or to since (0090, `queue::builder_chain`; `builtBy` is the last; a `(stamped)` re-stamp
  only when its old holder is the same holder as one in the chain, by `same_holder`, mu-185), and whoever last passed
  it into Testing with a gate line. The reviewer check (`review.rs`) refuses the same chain. The refusal names the builder and suggests a QA label (`muster/qa-0042`). The
  owner is always let through, logged `(owner's override: no separate tester)`; another label in
  a builder's session and agent is let through, logged `(tested under another label in the builder's
  session)`. The task carries `builders` (the takeover chain), `builtBy`, `gatedBy` and `testedBy`
  (the last pass into Your check), and the app counts builders as the core does: `lib.ts`
  `builders()` checks each of the chain, `builtBy` and `gatedBy` (mu-186), so a Testing card held
  by a predecessor of a takeover says "needs a tester" too. A Testing card says "needs a tester" on
  a line of its own while a builder holds it; the Your-check card says "built by … · tested by …"
  (each builder once, "built by wt-a and wt-b" after a takeover), wrapped, never cut; the task
  pane's status line says the same from Testing on.
- **Depends, met once merged** (tasks 0019, 0024, 0042): a task's `depends:` hold it back until
  each is merged, which for an agent's task is Your check or Done (work merges into main after
  Testing passes) and for a human task only Done. One rule in `block.rs` (`met_at`, `holds`)
  feeds every reader: `claim` and `next` (`waiting_on_deps`), the derived "After 0014" block
  (`block::derive`) that the card, table, timeline, pane, `muster list`, INDEX.md's "waits on"
  and `check` show, the Graph's "ready" (`readyToStart` reads the core's verdict), and
  `manifest.py`, which mirrors it. A change that merges a task logs `unblocked: 0014 done; 0017 in
  Your check` on each task it frees (Done after Your check writes nothing more). Sent back from
  Your check (`return`, or a `stage` override), it blocks again the tasks that depend on it and
  haven't started; each one under way (held in Building, or in Testing or Your check) keeps its
  stage and claim, gets `0017 went back to Building: <why>` on its log, and isn't held by that
  task again. A shared fixture, `app/src/deps-cases.json`, holds the CLI
  (`tests/deps_cases.rs`), the app (`deps-cases.test.ts`) and `manifest.py` (`test_task.py`) to
  the same answers.
- **Backlog** (task 0015): any `.md` in the queue's `inbox/` that isn't a numbered task is a
  backlog note, a one-liner or a whole page, however it got there. Add one with `muster add` or the
  quick-add field at the top of the Backlog column. Open a note in the docked pane to:
  - **File as task:** `muster file` makes a numbered task in Shaping and moves the note into its
    attachments.
  - **Drop** it: `muster drop`, kept in `inbox/dropped/` with the reason.
  - **Copy shaping brief:** `muster shape` gives the note, quoted as data, with the roles and the docs
    to read, related tasks, the estimates measured on this queue, and the command to file it.
  - **Estimates, measured on this queue** (task mu-168, `measured.rs`, pure): the brief's block
    before "File it". Each task with an `est-time` whose stage history has a move into Building and
    a later move from Testing to Your check or Done counts; its agent time runs from the first
    move into Building to the last such move (send-backs, review rounds and QA included), less the
    minutes it was blocked or paused in between (`block_history`, `pause_history`, overlaps counted
    once, an open stretch running to the span's end). A task that went back after it
    (sent back from Your check, reopened) isn't finished until it leaves Testing again. Stamps read
    as naive local minutes (`ops::stamp_minutes`); a zero or backwards span, or one blocked throughout, is skipped. It gives
    actual ÷ estimate at p50 and p80 (nearest rank) and the median hours, overall, by type (most
    tasks first; no type reads "untyped") and by size (S ≤ 1h, M ≤ 3h, L ≤ 8h, XL), a cell under 5
    tasks "too few (n)"; and, apart, the owner's wait (last entry into Your check → the last move
    to Done, when that came from Your check; median). Ratios are rounded to 2 places and hours to
    1, so `--json`'s `measured` (`overall`, `byType`, `bySize`: `name`, `n`, `p50`, `p80`,
    `medianHours`, null under 5; `ownerWait`: `n`, `medianHours`) says what the text does. The rule
    line says `--time` is agent time (review rounds and QA included, blocked and paused left out)
    and `--test-time` the owner's check only. The Tech
    role points at the block. Interim: mu-115's calibration replaces it, and the module goes.
  - **Shaping is a claim** (task 0097): `muster shape <note>` also claims the note for the
    caller in `shaping.json` (label, since, heartbeat); running it again renews the heartbeat, and
    someone else's live claim is refused with the holder and since when. `--peek` reads without
    taking. One note per holder per queue (the owner may hold several). `file` carries the claim to
    the new Shaping task (`claimed-by:`, fresh heartbeat); `file`, `drop`, `release <note>` and
    `reclaim` after the lease let it go. The owner, or anyone once the lease ran out, may file or
    drop a held note, logged as taken over. A Shaping task's claim is released by
    `pass --from shaping` and by going back to Backlog; `reclaim` releases a quiet one and keeps
    it in Shaping; the shaper renews it with `heartbeat` and `checkin`, and, as in Building, with
    its own note, comment, ask, edit and vision check and with an unblock or a resume
    (`queue::has_lease`). After a
    `checkin --handoff`, `claim --takeover` moves a Shaping claim as it does a Building one;
    `muster claim <note> --takeover` takes a note's claim (the same label once it's been quiet
    30 minutes, or the owner). `list` shows
    "· shaping: <name>, <ago>" on the note's line, `list --json` gives `heldBy`, `heldSince`,
    `heartbeat`, and `check` reports a malformed, oversized or stale `shaping.json`.
  - **Holder ribbon** (task 0097, `HolderRibbon.tsx`): a card held in Shaping, Building or Testing
    wears its holder's short name in the top-right corner (top-left in RTL), where the You ribbon
    sits on the owner's own; never both. Neutral (`--panel`, `--line`, `--text`; `--warn` and
    "quiet" when the lease ran out), the LiveDot inside it when the session is working, the name
    cut with "…" only when it doesn't fit nearly the card's width (half beside the stage band),
    a name ending in a number cut before the number, which stays (`nameEnds`, mu-171: "as…16"),
    whole in the tooltip, and "held by …" in the card's accessible name. A held note in the
    Backlog says "shaping · <name>" ("shaping" gives way first, the "·" stays), or wears the You
    ribbon when the owner shapes it, with "· quiet" once the lease ran out. The space is reserved
    whether or not a claim is there, so a claim arriving moves nothing. It shows in every display
    mode (`modes.ts`).

  `muster list` shows notes too. They live in `.muster/backlog/` (0005), and in the old layout's
  `inbox/`.
- **Shaping lenses** (task 0076, `lenses.rs`): ten lenses every shaped task answers, one line each
  under `**Lenses:**` in its body (what it must do for the lens, or "n/a" and why): accessibility,
  i18n and RTL, themes, payments and entitlements, architecture, security, privacy, performance,
  tests, docs.
  - `muster shape` lists them with what each line covers, and its template has the block. Its
    `--json` has `lenses` (`id`, `name`, `write`) and `skipped` (`lens`, `name`, `why`).
  - A project skips a lens only with a reason. A `.muster` queue does it in `config.toml`
    (`[shaping]` `skip = ["payments"]`, `[shaping.reasons]` `payments = "<why>"`; a small reader
    for just these two tables, strings with their escapes, a list over several lines). An old
    queue does it in README.md's front matter (`shaping-skip: payments, themes`,
    `shaping-reason-<lens>: <why>`). A skip of an unknown lens, a skip with no reason, a reason for
    a lens it doesn't skip, an unknown `[shaping]` key or a malformed value are problems in the
    queue's state (`muster check`, Problems), and the lens still applies.
  - `muster file` returns `missingLenses` and warns on stderr, naming them, and files anyway. A
    line counts when its label (the words before the colon, parentheses aside) is only lens words
    and "and": a list item (sub-items don't count) or a sentence in the block's paragraph
    (`Security, privacy: n/a.`). The template's `<…>` doesn't count.
- **Goals, features and tasks** (task 0016): `level: goal | feature | task`, `parent:` and a goal's
  `due:` set with `muster new --level/--parent/--due` and `muster set`. A task sits under a feature
  or goal, a feature under a goal. Goals and features are never claimed or built: their stage comes
  from their children (Done only when every child is), and they roll up "n of m done" and the open
  estimate. **Group by** None · Feature · Goal (chips above the board and table) gives one band per
  parent, with the stage columns inside, after a Backlog band (0046) with the quick add, the backlog
  notes and the Backlog tasks under no parent. Cards show their parent. `check` reports missing parents,
  wrong levels and loops, and warns about late goals.
- **Editing** (task 0020, part 1): **Edit** beside the description turns it into a text box in place.
  ⌘S saves, Esc twice discards, and nothing typed is lost when you switch cards. `muster edit` does
  the same with `$VISUAL` or `$EDITOR`. A save is refused if the file changed since it was opened,
  or while someone else holds the task in Building (add a note instead). Comments come with 0017.
- **Slash commands** (task 0003): `commands/muster.md` and `commands/mu.md` route `/mu <word>` to
  one `muster` command each. They may run only the commands they route. Anything else is captured
  as a backlog one-liner. They're linked into the personal layer's `commands/` (layer commit
  `663a4c7`), so every linked repo has them.
- **The task pane** (task 0021, the owner's design D2):
  - The title (rename in place) and status line come first: stage, who's on it and since when,
    LIVE, blocked, and what waits on you.
  - Then the seven-stage track: click to move, with undo. It refuses if the task moved since.
  - Then two columns. On the left, what the stage needs: work so far, your checklist and the
    history, or the criteria. On the right, a sidebar with the one next step, Send back,
    Block/Unblock, priority and type as one chip each, estimates, tags, files and links.
  - Under the write-up come the vision check, the review of record and the comment thread. The
    note bar stays at the bottom.
- **Fields** (task 0017):
  - Types are code, bug, chore, tech-debt, spike, design, docs, research.
  - Estimates are agent time, test time and tokens.
  - `links:` hold https links, opened only if the task has them.
  - Comments live in a tool-written `## Comments` section (`muster comment`), and Edit never
    rewrites them.
  - The owner's comment acts when its first word is an @action (mu-101, `fields::comment`, shared
    by the app's `add_comment` and `muster comment --as owner`): `@accept` (Your check only),
    `@sendback <why>` (the Send back form's default: from Your check the QA send-back, elsewhere
    one stage back; the owner's own task one stage back, as the form), `@block <why|keys>`
    (as `--on`: keys block on those tasks, a colon after them setting the reason apart; a
    `block::KINDS` word first blocks on that kind; other words on outside) and `@ask <question>` (`ping::question` to the holder's session;
    refused with no reachable holder). First word only, case-insensitive after Unicode
    whitespace, one action. An unknown @word is refused naming the four and saves nothing; the
    action runs first, and its refusal saves nothing; on success the comment is saved with
    `\n→ <what it did>`. An agent's comment never acts. `add_comment` returns `{path, did}`
    (`fields::Commented`, `did` empty for a plain comment). The comment box has the one hint line.
- **INDEX.md** (task 0018): one generated list per queue, regenerated after every write. Goals,
  features and tasks by stage, each with its estimate, what it waits on, comments and open vision
  conflicts, then backlog notes and Done. `muster index` rebuilds it, and `check` flags a stale one.
- **Vision check** (task 0029):
  - `muster vision <id>` prints the brief.
  - `--record -` stores an agent's findings as a `## Vision` section with V-numbered conflicts, and
    the section isn't editable once the task is checked.
  - `--resolve Vn` settles one.
  - Shaping → Ready waits while a conflict is open.
  - A change of direction becomes one owner question with the cost of pivoting.
  - Lenses (0076): when `docs/VISION.md` has a `**Lenses needed:** <lens>, <lens>` line, a task
    that says "n/a" for one of them is listed in the brief (`lensFlags` in `--json`), and
    `--record` adds a conflict for each unless the JSON has one whose title says n/a and names the
    lens.
- **Review of record** (task 0027): the card says where it stands in Building and Testing
  (`review 1/3 · 2 to fix`, `review 2/3 · 1 to re-check`, `review clean · round 2`, `not reviewed
  yet` on a started task in Testing; quiet, bolder while findings wait for the builder,
  `Review.tsx`); the pane's **Review** fold (`ReviewPanel.tsx`) shows the status, **Copy review
  brief** (the core's brief, loaded on open, a started task only) and the section as sanitized
  Markdown. **Lessons to approve** leads Waiting for you (`Lessons.tsx`): each proposal in full as
  plain text, **Reject…** with why, **Accept all** (the core's `decide_lessons` as the owner);
  the queue's waiting lessons count in the tab's needs-you count, the sidebar's and the Dock's.
- **Board:** one column per stage, with its role under the name. Blocked cards stay in their stage
  with a badge that says plainly what it waits for (task 0024; each kind its words and look, task
  0060: "Your answer: Q2 …", "Your action: …", "Permission: …" in needs-you orange, "After mu-14",
  "Outside: …", "Question: Q2 …, waiting on capture" neutral, each with a red mark), and Your check
  cards count as "needs you". The task pane has a
  stage picker, Pass to <next> (with the gate line out of Building), Send back with a reason,
  Block/Unblock, and Accept / Send back to builder in Your check (its form defaults to Building and asks "What's wrong (the builder gets this)", mu-101). Only one action runs at a time. Cards show the key, title, type, priority (on every card: HIGH filled magenta, normal outlined, low faint), tags,
  "needs you", open questions, files, who's on it (flagged when quiet for 12 h), what a blocked
  task waits on, and when it last changed. Tasks waiting on the owner sort first. Done shows the
  latest 8.
- **Feature grid** (task mu-143, the owner's mockup): the board's second layout, beside the
  columns, which stay the default. **Layout** Columns · Feature grid in the filter bar, kept per
  project in the window's storage (`muster.layout.<project>`, `featureGridData.ts`
  `loadLayout`/`storeLayout`); the chips' names come from `shell.json` (`app.layouts.*`), and in
  Persian read ستون‌ها and شبکه‌ی قابلیت‌ها (glossary, proposed). Features (or goals, per the board's Group by, which its own Feature ·
  Goal · None control sets, with Group by's words) down the side with their progress, Backlog to Your
  check across with who works each (the holders' names, else the role; Your check "N of L",
  "full" at the limit) and Done as today's count. Two small cards a cell, then "+N more", which
  expands in place; a card's title, its trouble on its own line (sent back N times, quiet, needs a
  tester), why it waits, then one status (who with the live dot, needs you, paused) and its key.
  The header line, Only what needs me (dims), and one alert line when Your check is full. A second
  renderer over the board's data (`gridRows` over the filtered work, `groupItems`, the board's
  sort), no new state beyond the setting. `FeatureGrid.tsx` draws it as an ARIA grid (row and
  column headers, roving tab stop kept by row id, arrows, Home, End, ↩ and Esc from
  `shortcuts.ts`), with logical CSS so it mirrors in right-to-left. The frame's scroll-padding
  (`--fg-names`, `--fg-head-h`) keeps a focused cell clear of the sticky names and header row;
  columns past the edge get the board's cue (`rowCue.ts` `overflowCue`, `ColumnRow` `RowCue`) at
  the end of the alert's line, measured by `watchGrid` on resize and scroll (`columnChips`,
  `viewSpan`). Only what needs me is App's state for the window's life. Tests:
  `featureGridData.test.ts`, `FeatureGrid.test.tsx`; screenshots from `app/e2e/shot.mjs`.
- **Human tasks, on the one board** (task 0022; the owner, 2026-10-13: "human board is taking
  space from the ai board", so one board, human cards patterned): `for: human` makes a task the
  owner's own (`muster new --human --as owner`, the Human/AI chips in New task, the For chip in
  the task pane, `muster set <id> for human|ai`). It goes To do (Ready) → Doing (Building) → Done
  with the pane's Start and Done, no gate line, no Testing, no Your check, no lease. Agents never
  take one: `claim` refuses it and `next` passes it by; an agent proposes one with `muster add`,
  and only the owner makes one. Its card sits in the same stage columns as the agents' tasks
  (`lib.ts` `boardColumn`: To do in Ready, Doing in Building, Done in Done; never Backlog,
  Shaping, Testing or Your check), drawn so it can't be mistaken: a faint diagonal hatch in the
  human gold (`.card.human`, CSS only: stripes of `--st-human` mixed by `--human-hatch`, 14%,
  `themes.ts` `HUMAN_HATCH`, into the card's own background, so the Tint look's blocked or
  needs-you wash stays under them) plus the gold "You" ribbon (`#A08428`, never a stage colour),
  in every card look. `themes.test.ts` holds every card word on a stripe over the plain card to
  its contrast in every theme and accent (text 7:1; muted, project name and warnings 4.5:1), the
  title (7:1) and the muted, project-name and warning words (4.5:1) on a stripe over every wash the
  Tint look can give the card (each stage, priority, blocked, needs you; mu-65). The project name and
  the quiet and vision flags sit just under 4.5:1 there in their plain colours, so `.card.human`
  takes `--human-accent-text` and `--human-bad` (`themes.ts` `wordOnStripes`: the colour mixed toward
  the theme's text until every stripe reads), the stripes and wash untouched. The hatch
  visible (CIE76 8+ from the surface), and the gold a hue apart from blocked red and needs-you
  orange. The ribbon marks human tasks in Table, Timeline, Graph and the pane too; the pane's
  stage track shows a human task only To do · Doing · Done (`paneData.ts` `trackSteps`), and the
  Stage filter chips go by the board column. The **Who** chips (All · You · Agents, remembered)
  sit above the Board, Table and Timeline: You shows only the owner's tasks, Agents only the
  agents'; backlog notes show under every Who (nobody's yet).
  What the agents wait on the owner for has no lane of its own: Your check is a column, and
  escalated questions and blocks on you or by a permission are needs-you cards where they are.
  `needsYouCount` (`human.ts`: the owner's own tasks to do and doing, plus those) is the one
  count: sidebar, Waiting for you, the needs-you filter chip, and the Dock badge (0006, counted
  in the core by `badge.rs`, which `needs-cases.json` holds to the same answers). An
  agent's task that depends on an unfinished human task says "Blocked by tasks: 0041" (0024's
  sentence; derived, so the blocked chip, `muster list --blocked`, the graph and the activity
  feed count it as blocked too) and can't be claimed until it's done. `muster list` marks human
  tasks `[you]`, INDEX.md "you", and `manifest.py` reads `for`.
- **Recently finished** (task 0051, `recentData.ts`, `Recent.tsx`): docked at the board's right
  edge (plain or grouped), always in view, its own scroll; about a quarter of the board's row,
  200 to 272 px. When the row can't hold the board's 210 px and the rail's 200 px (a narrow
  window, or a wide pane), the rail goes under the board, full width, at most a third of the
  window high (`paneSize.ts` `railBeside`, the `stacked` row): moved, never hidden. The pane's
  widest leaves the content 210 px in every view, counted in the page's pixels under ⌘+ zoom
  (`clampPane`, `layoutWidth`), so it doesn't change width with the view. In a narrow page it
  gives way first, below the 360 px you can drag it to, down to 240 px (`PANE_FLOOR`, the `.pane`
  min-width): the board keeps 210 px while the page is at least 660 px wide (an 820 px window up
  to 115% zoom, 1000 px at 150%); narrower than that the board gets less (97 px at 820 × 150%),
  and the pane never runs past the window. What landed on main (the latest
  Testing → Your check stage line, **merged**) and what moved into Done (`doneAt`, **accepted**;
  the owner's own task's Done too), each a row of its own, for the project in view (every project
  with All, each row with its project), whatever the filter bar says. Only what's still finished:
  a task sent back has no row; goals and features have none. Newest first, grouped by day (Today,
  Yesterday, then "Sun 11 Oct"), each day with its count; when in words ("4 min ago", "2 h ago",
  "yesterday 18:20", then the date), the stamp in the tooltip. Chips All · Merged · Accepted,
  remembered (`muster.recentKind`). The last 7 days, **Show older · n more** adds a week. Each row
  opens its task in the pane, and its second line says who built it (the session that took it
  into Building) and its time and tokens (0025's totals, `task_sessions`, read one task at a time
  for the rows in view and again only when a file changes). Done cards say "done 4 min ago"
  (`CardStatus.tsx`).
- **Waiting for you:** what the agents wait on the owner for (escalated questions, Your check,
  blocked on you or by a permission, `queue::needs_owner`), then the owner's own tasks to do and
  doing, each opening its task (`human.ts` `waitingForYou`: the needs-you count's tasks). Your check comes first, by
  effort (`WaitingList.tsx`, task 0043).
- **Your check says what to do** (task 0043, `yourCheck.ts`, `EffortGroups.tsx`):
  - Each Your check card says **Your turn** (not "on it: owner"; the pane's status line too) and
    its next step: the first open owner criterion as an instruction ("Try: …", or "Check: …"
    when it isn't written as something the owner does), cut to 90 characters with the whole in
    its tooltip and the pane, "· n more", or, with nothing of the owner's open, "All 4 proven:
    Accept" (mu-78).
  - The next step is, in order: an open question escalated to the owner ("Answer Q2: …"), a
    block on them or by a permission ("Unblock: …"), the first open owner criterion, else
    "All N proven: Accept" when every criterion is ticked and each of the agents' has its proof
    ("All N ticked: Accept" when one has none written), "Not proven: AC3 · send it back, or
    check it yourself" when an agent's isn't ticked, and "Nothing to try: Accept" when the core
    sent no criteria.
  - Effort is derived from the open owner criteria: any mentioning a session, terminal or command
    (whole words) → Needs a session; any other open one, an open escalated question, a block on
    the owner, or an agent's criterion not ticked (mu-78) → One look; else Just accept. `— effort: look|session`
    on a criterion overrides it, and its words leave it out.
  - The board's Your check column (each band's too, with Group by) and Waiting for you group by
    effort, in that order, each with its count; an empty group keeps its heading.
  - **Accept all** in Just accept: the first click freezes the tasks shown (tasks only, not
    blocked) and names them in place, with the focus on Cancel; the second sends them to the
    core's `accept_all` (lib.rs, one call per project), which checks every id and the stamp first, then passes each from Your check as the
    owner, as Accept does (`ops::pass_task` with `from: your-check`, so the commit the merge
    recorded stays). The core's rules refuse an open owner criterion, a block or a task that moved
    on, and `accept_all_in` also refuses a goal or feature and an open escalated question (read
    just before each pass), with the reason, and the rest go on. Each accepted task's log gets its
    stage line. Esc cancels the confirm (a capture listener, so it never closes the pane), and
    the focus goes back to the button. The result line is kept by place (board, band, Waiting for
    you) outside the column, so it outlives an emptied column, and goes on the next change
    (`staleResult`). Refusals read without the leading id or `muster` hints.
  - The pane's checklist lists the open owner criteria first, each with its effort, **Tick** and
    a button per view it names (Board, Table, Timeline, Graph, Appearance in any case; Helper,
    Waiting for you, Activity, Problems, Projects, Help only capitalized). Ticked ones follow with
    **Untick**.
- **Your check says what you're accepting** (task mu-78):
  - **The summary.** `summary:` in the front matter: what was built, one plain line, at most 160
    characters, no line breaks or control characters (`ops::clean_summary`, which refuses rather
    than folds them; runs of spaces become one). `muster pass <id> --from building --summary
    "…"` writes it (logged as `summary set to …`, before the stage line), and a task (not a goal,
    a feature or the owner's own) is refused out of Building without one, unless it has one
    already. `--summary` anywhere else is refused. `muster set <id> summary "…"` writes or fixes
    it in any stage. The window's gate form asks for it beside the gate line (`GateFormView`,
    prefilled with the task's own), and `pass_task` takes it. Reading clamps a hand edit to 160
    characters without control characters (`queue::SUMMARY_MAX`). The state sends it as `summary`.
  - **Each criterion's proof.** `criteria_items` splits the first `— proof:` (em or en dash, or a
    hyphen, between spaces) or `. Proof:` (a full stop and a space, Drover's shape; the stop is
    left off), `proof:` in any case, outside a code span, off the text into `proof`, so a
    criterion that only mentions the tail keeps its words and a proof's own "verify: owner"
    can't make it the owner's. `parse_task` fills `proofFiles` with the task's attachments that
    proof names (`queue::proof_files`): `attachments/<id>/<file>` or a bare file name, matched
    exactly against `list_attachments`; `../`, another task's folder, an absolute path, a
    backslash or a subfolder name nothing. The state sends every criterion as `criteria` only for
    a task in Your check, its one reader (left out of the JSON elsewhere, so the 2 s poll stays
    lean); the owner's still come as `ownerCriteria` in every stage. The start brief prints each
    criterion with its proof.
  - **Accept all** (`accept_all_in` → `not_just_accept`) also refuses a task with an agent's
    criterion not ticked, naming them, as the window files it under One look.
  - **The card and Waiting for you:** the next step ("All 4 proven: Accept", above), then the
    summary on one line cut to the width with the whole in its tooltip (`.card-summary`,
    `.summary-line`), shown as text. No images on cards. Every queue, old layout too: it's read
    from the task file.
  - **The pane:** "What you're accepting" (`StageContent.tsx` `AcceptingBlock`, a labelled group)
    comes first in Your check, above Your checklist: **Built** (the summary in its own `dir="auto"`
    span, as text, or the title and `muster set <key> summary`), **Accept** (`yourCheck.ts`
    `acceptWords`: blocked, not proven, the owner's still open by name, or every criterion proven,
    or only ticked when one of the agents' has no proof written: Accept moves it to Done), and **Proof**: each criterion in one line, label first and its
    words and proof each `dir="auto"`, how it stands in words, and its proof's images as Files-size
    thumbnails under it. Each is a native button (file name as its label, `aria-expanded`;
    Enter and Space click it) that opens the image large in place, no overlay, and shrinks on a
    second click; other files are named. The images come from one map per pane (`TaskPanel.tsx`
    `usePaneImages`, through `api.image`, once per task and set of files), which Files reads too,
    so nothing is fetched twice. No motion, and
    `prefers-reduced-motion` turns any off. The merge report goes last, in small type.
- **Run from the app** (task mu-79, `run.rs`, `Run.tsx`, `runData.ts`):
  - **Starting.** `run_start`, `run_stop` and `run_state` take a project's name and nothing else:
    the core finds the project in `projects::list`, its `run` command, and its main checkout
    (`run::checkout`: the repo a `.muster/` queue is in; for an old `<project>/tasks`, the
    `<project>/main` or `master` git checkout beside it, else refused with why). `run::plan`
    splits the command again (`cli::split_words`, refused on an unclosed quote, never handed to a
    shell) and checks it as projects.json does (`projects::command_refused`). It starts as the
    gate does (`parallel::program_in`, `parallel::gate_command`: `bin/run` from the checkout, no
    stdin, without `MUSTER_ROOT` and `TASKS_ROOT`), both streams piped, leading a process group of
    its own. Its PATH (`run::search_path`, each folder once, absolute ones only) is the owner's
    login shell's first, then Muster's own, then `~/.cargo/bin`, `~/.local/bin`,
    `/opt/homebrew/bin`, `/usr/local/bin` and the system's: from the Dock, Muster has only the
    system's, and the owner's node is only under `~/.nvm`. The login shell's PATH is read once,
    on a thread `Runs::new` starts (a run that comes first waits for it, outside the lock Stop
    takes): `$SHELL -ilc` with a `printf` of `$PATH` between marks, as an argument array, no
    stdin, in a session of its own (`setsid` before exec: no controlling terminal, so an
    interactive zsh never stops on SIGTTOU when Muster was started from a terminal). The PATH is
    taken at its end mark (a number of the call's own in it; `run::has_mark` finds one split
    across reads), not at the end of output, which something its startup files leave going may
    hold; after the mark the output is still read and dropped, to its end, 64 KB in all or the
    5 s, so a logout file or EXIT trap that prints runs to its end instead of dying of SIGPIPE,
    and a PATH between the call's marks is kept whatever the shell's exit status
    (`run::login_path`). If it prints no PATH or takes over 5 s (then its group is killed), the
    fixed folders stand in. It runs as the owner would start it, on their real data (Q2). One run per project,
    kept by its queue folder, so a rename or another project's name can't take it; removing a
    project stops its run. Run while one goes restarts it. Runs live in `AppState.runs`, in memory
    only.
  - **Its output.** A reader thread per stream splits lines (`run::read_lines`: lenient UTF-8, a
    line past 4,096 bytes cut with "…") into a ring of the last 2,000 lines or 256 KB, whichever
    is smaller. A sender thread emits `run` events (`run::Snap`: the run's id, the project's name
    and key (its queue folder), state, times and the new lines with where they start) at most once every 200 ms, and a last one when it ends.
    A waiter thread reaps the leader only once both streams closed: until then an exited leader
    stays a zombie holding its id, so no other process group can take the id Stop signals. Nothing
    is logged or written to disk, and nothing polls the process: a run has ended when its leader exited and both streams closed, so a grandchild still
    writing keeps it running.
  - **Stopping.** Stop, Restart and quitting (`RunEvent::Exit` → `Runs::stop_all`, every run at
    once) send SIGTERM to the group, then SIGKILL (to the leader too) after 5 s, through
    `/bin/kill` with an argument array as `agents.rs` does. After SIGKILL it's Stopped within 2 s
    even if something that left the group still holds the streams. Hiding the window stops
    nothing: the app stays alive for the Dock badge.
  - **The window.** `useRun` listens for `run` events, then reads `run_state`, and takes the
    events of the project's key, which `run_state` gives (`runData.ts` `received`/`keyed`; until
    it answers, the last 50 wait), never by name: a run keeps the name it started with, which a
    rename can pass to another project. `runData.ts`
    `merged` puts the two together by the lines' places and the run's id (an older run's late
    event changes nothing, a `run_state` answered after an event puts its earlier lines in front,
    an ended run stays ended) within the same bounds. A polite live region, there before any run
    (`RunStatusLive`), says the state for screen readers. Your check's What
    you're accepting has a **Try** line (`RunLine`): Run (Restart while it runs) named for the
    project, with the command beside it as code, or "No run command: `muster projects set <name>
    run …`". Under the block, once the project has a run, the **Run** block (`RunBlock`): the
    state ("Running bin/run in muster/main", "Exited 0 after 2m 10s", "Stopped after 4s"), the running time ticking each
    second beside it, Stop and Restart (or Run again), and the last 12 lines in a labelled
    `role="log"` that isn't live; **All output** opens every line kept in place. Lines are text,
    each `unicode-bidi: plaintext`. While a run goes, the state's `running` puts a "running" chip
    on the project's sidebar row, in words, with no motion.
- **A limit on Your check per project** (task 0050, `check_limit.rs`, `yourCheckLimit.ts`,
  `CheckLimit.tsx`): `projects.json`'s `yourCheck` maps a queue folder (the project's key, as
  `gate`, `names` and `hidden` do) to 1–50 or `"off"`, so a rename keeps it and two projects shown
  under one name keep their own; a folder not in it, or with a bad value, has 5, and a bad value
  never stops the rest of the file loading (`check_limit::read_limits`). The core counts each
  queue's waiting tasks (`check_limit::waiting`: stage your-check, level task, not `for: human`)
  and sends it with the limit in the state (`QueueInfo.yourCheck`, `yourCheckLimit`, null for off;
  the `state` command fills the limit from projects.json with `check_limit::apply`). Over the limit
  (more than it, not equal), the Your check header reads "7 of 5" (the project's whole pile,
  whatever the filters show) in `--warn` (the orange on dark themes, #A84300 on light ones:
  4.5:1) on the plain board. A Group by band's Your check keeps its own count, orange (with the
  pile in its title) when it holds tasks of a project over its limit. Under the plain board's
  header a short line ("Over your limit of 5", or "2 projects over limit" in All; at most 24
  characters) has a reserved two-line place; above the board, plain or grouped, a full-width
  line with a reserved one-line place says "7 of 5 in Your check: check these before more
  start." or, in All, "Over your limit in 2 projects: …"; it never wraps (nowrap, cut with an
  ellipsis at the narrowest) and its title has it whole, with each project over in All. Over, the column stays open even when a filter
  empties it. The sidebar row of a project over says "7 in Your check, limit 5" under its activity
  line. Settings › Projects has a Your check column: "limit 5 · 7 waiting", an **Off** chip and a
  1–50 field with **Set** (`set_your_check_limit` by the project's key, checked by the core; the
  board refreshes at once); a hidden project shows "—". On the command line `next`, `start` and
  `ready --parallel` print `muster: 7 tasks wait in Your check (limit 5): the owner's review is the
  bottleneck.` on stderr before their work, and `list` prints it between its Your check and Done
  groups (stdout is cut there, `list_parts`); with `--json` it goes on stderr after the JSON. Exit
  codes don't change. The CLI finds the queue's entry by folder, through symlinks too.
  Every change to projects.json, the app's and `muster projects set`'s, goes through
  `projects::edit`: an exclusive flock on `projects.json.lock` held across load, change and
  save, and a temporary file per writer (process id and counter) renamed over it, so concurrent
  writers keep each other's edits and a reader never sees half a file. Its maps (`names`, `labels`,
  `categories`, `gate`, `run`, `yourCheck`, `colours`, `groupNames`) are `BTreeMap`s, saved in key
  order, so a save that changes nothing else (a fold and its unfold) keeps the file's bytes (mu-88).
- **Task panel** (loads on first open): the task rendered as Markdown with a Raw switch,
  open questions with an answer box (⌘↩), the files as thumbnails (the shared `ImageThumb` of
  Your check's proof) that open their image at the pane's full width in a row under the two
  columns (`FilesOpen`, `data-part="files-open"`), several at once, a second click on the
  thumbnail or the image shutting one; one that opens is scrolled into view, and shutting it
  from the image puts the focus on its thumbnail; the side column is too narrow to read one in
  (mu-91, no overlay), who's on it, what it waits on, and the commit.
- **The clarify pass in Shaping** (mu-103, `clarify.rs`, `queue::escalate_all_locked`,
  `WaitingList.tsx`): a `[NEEDS CLARIFICATION: …]` marker (with its colon) left in a write-up
  (above Comments and Log, outside code, fences read as `merge_log::Fences` reads them) refuses
  `pass --from shaping`, naming each; `muster file` warns about them on stderr (`markers`). An
  open question with no `--no-answer` default holds Shaping → Ready too; one with a default
  doesn't, and a later answer to it is mailed to whoever holds the task then (the claim's
  mailbox; the asker's when nobody holds it). `muster clarify [--json]` lists the open,
  unescalated questions on Shaping and Ready tasks that aren't blocked, grouped by task;
  `--send` escalates them under one hold of the queue's lock, one log line per task, and
  `--send --json` prints what it sent in the list's shape. The window's poll reads each queue's
  tasks holding its lock shared (`queue::read_state_settled`), so it sees a batch whole or not at
  all and posts one notification. Waiting for you
  puts those tasks first under "From shaping: N questions on M tasks"; a notification folds a
  task's questions into one line ("asks you 3 questions"). The shaping brief's "Unknowns:" says
  to mark, decide or ask, and send once at the end of a run.
- **Pick-one questions** (task 0049, `choices.rs`, `PickOne.tsx`): a question asked with options
  shows them as chips in the pane's open questions and in its Waiting for you entry, every option
  on screen, the asker's pick marked **pick** and "going with (a) unless you answer" after them. One click
  answers (`answer_pick`): the answer is `(b) <the option's text>`, plus the answer box's words
  after " — " when there are any; the box stays below for something else. Escalated ones keep
  the orange edge, as before.
- **Work** (task 0025, below the description and check-ins in the task panel): every Claude Code
  session that worked on the task, found by the `session <8 hex>` in its claim or `held-by` and in
  the who of its log lines (the older `label (session 7b5e8856)` and 0030's stamped
  `label (checkout · session 7b5e8856 · cc-hg)`). A table by stage (mu-172; columns Stage and
  session, Tokens, Time and View): a row for each stage with its counted tokens, cache reads and
  shared sessions apart, and its time; under it the sessions that spent there, each with its short
  id and label, "holds it" when it holds the task now (0030's holder: the claim, or `held-by` in
  Testing and Your check), "shared with N other tasks, not counted" when it's shared, and its
  tokens and time in that stage. Each stage is a row group headed by its name (`<th
  scope="rowgroup">`), so a screen reader says a session's stage with its row. A session no stage
  has tokens of (its transcript missing, too big, not stamped, or with no tokens) is listed under
  "No tokens by stage" with why. A live session says **live** after its label. Then one total line
  against the estimates ("Total: 1.8M of 1.5M tokens (120%) · 2.5 h of 8 h (31%)"), cache reads
  and shared sessions apart after it, the tokens by kind, and a progress guess from ticked
  criteria and the stage, labelled as a guess. **View** opens that session's transcript below the
  table as
  Markdown: one line per prompt, reply and tool call, tool
  output and thinking left out, the latest 1,500 entries, every punctuation mark escaped so nothing
  in it renders as a link, image or HTML. Only a session the task names can be viewed.
  - **What counts** (mu-172, `work::split`, `work::agent_minutes`): tokens are the own sessions'
    input, output and cache writes (`Tokens::counted`, `countedTokens`), against `est-tokens`;
    their cache reads are shown beside them and never counted (they grow with a session's length,
    not its work). A session another task of the same queue names too (the lead's, say) is kept
    out of every total and shown apart (`sharedTokens`, `sharedRead`): its transcript can't say
    which part was this task's, so neither counting it in full nor guessing a split is right.
    Time against `est-time` is agent time (`agentMinutes`): Building's stage time plus Testing's,
    blocked and paused stretches left out, each minute once (in Building or Testing it runs to
    now). For a task never released to Ready nor sent back from Your check that's mu-168's first
    claim to last leaving Testing; time back in Ready isn't work, and Your check is the owner's
    wait (`agent_minutes_match_mu_168_on_its_own_cases`). Shown in hours ("2.5 h of 8 h (31%)";
    a day of estimate is 8 h, never "1 d"). The rows' minutes and the breakdown are the
    own sessions' transcript spans. Before mu-172 the owner's task read "Tokens 168.1M of 1.5M
    (11204%)" and "Time 7.3 h of 1 d (92%)": every kind of every session, the lead's whole
    transcript included; it now reads "Total: 1.8M of 1.5M tokens (120%) · 2.5 h of 8 h (31%)"
    (`the_owners_fixture_…` in `work.rs`).
  - **By stage** (mu-172, `stages`, `work::stage_rows`): a row each for Shaping, Building, Testing
    and Your check, then "other", and under each the sessions that used tokens there. Each
    transcript keeps its tokens by minute (`transcripts::Buckets`: one entry per minute with any
    use, subagents merged in, held in the summary cache with the rest of the file's sums), and
    each minute goes to the stage the task was in then: a stage line opens a window of the stage
    it moved to until the next one, the last runs to now, and before the first the task was in
    that line's `from` stage since its first log line (a task made in Shaping has its Shaping
    row). Ready, Backlog, Done, before the task was made and replies with no time stamp go to
    "other", so the rows always add up to the totals. A row's time is the task's time in that
    stage, blocked and paused left out (Your check's is the owner's wait, never in the total); a
    session's is its first to its last minute there, paused left out. A shared session is listed
    under each stage it used tokens in, its tokens in the row's `sharedTokens`, never counted. When
    a stage line's stamp can't be read as a time, everything goes to "other" and agent time is
    unknown.
  - **The card's token ribbon** gets the same number (`work::mark_tokens`, `tokensUsed`,
    `tokensShared`): the state read counts it for an agent's task in Shaping, Building or Testing
    from the same split, so card and Work section agree to the token. A count is reused for 30 s
    while the task names the same sessions (a live transcript changes every poll), the memo holds
    only the cards marked by the latest poll, and it's taken out of the app's state while
    transcripts are read, so no lock is held over a read.
  - **Where the numbers come from** (`transcripts.rs`, the one reader 0017's actuals will use too:
    `transcripts::actuals`): `~/.claude-<label>/projects/<folder key>/<session id>.jsonl`, each allowed
    account (0012) once (`.claude-hg/projects` linking to `.claude/projects` isn't counted twice;
    `~/.claude` itself is never read), and only a folder key under `~/playground`
    (`-Users-<name>-playground` or that plus `-…`; linked project folders skipped), so company
    transcripts in the shared linked folder stay out. Plus the
    session's `subagents/*.jsonl` for tokens. Usage counts once per reply (Claude Code repeats it
    on each content block); minutes are first to last time stamp. Read on this Mac and sent nowhere;
    the window gets counts, ids and times, never a path. A session not found reads "not on this
    Mac"; one whose stamp names no allowed account isn't looked for and reads "not read: not
    stamped with a Claude account Muster reads" (`Found::NotRead`, 0099). Transcripts over 128 MB and lines over 8 MB are skipped, and summaries are cached by
    size and modified time (512 files).
  - **Live:** a transcript (or one of its subagents') changed in the last 5 minutes. The card shows
    a small dot while any of the task's sessions is live (in its token ribbon on a card that has
    one, mu-172; by its id on the others), worked out in the state read
    from file times only (20 cards × 50 sessions: about 0.5 ms). Done tasks aren't checked.
  - **Speed:** 50 transcripts of 1,000 lines each read in about 0.3 s cold in a debug build and
    0.5 ms warm. On the owner's Mac, five real sessions (up to 60 MB) read in 0.15 s cold in a
    release build, with no line skipped.
- **New task** (`+ New task`, ⌘N, in the docked pane): project, type, priority, tags, a one-line title, a Markdown
  description, and files by paste, drop or picking. The id comes from the core under the queue's
  lock. The task lands in To do and opens.
- **Tags:** a `tags:` list in the front matter. Suggested: spike, chore, tech-debt, code, docs,
  design, research, test, perf, security, and any other word works. They're cleaned the same way
  everywhere (lowercase, hyphens, 24 characters, 8 at most). On cards, in the filter, in New task
  and on each task. `muster new --tags`, `muster set <id> tags`, `muster list --tag`.
- **Estimates:** `est-time` (45m, 2h, 1.5d; a day is 8 agent hours) and `est-tokens` (150k, 1.2M;
  input, output and cache writes, not cache reads, since mu-172) on each task, cleaned the same
  way everywhere. On cards (`~2h · 500k`), in the table (sortable),
  in New task and on each task. The filter bar totals what's shown and still open ("≈ 10 d · 20.1M
  tokens left, 11 of 13 open tasks estimated"). The helper estimates every task it files with a size
  guide (S 30m/150k, M 2h/500k, L 1d/2M, XL: split). `muster new --time --tokens`, `muster set <id>
  time|tokens`. Actual time and tokens per task can come later from Drover (Drover task 0018).
- **Managing a task** happens in the pane (0021, above): the stage track, Pass, Send back, Block
  and Unblock, priority and type, rename in place, a note to the log, and files by paste, drop or
  **Attach…**. Every change is logged with the time and "owner". The app never claims.
- **Answering** writes file contract v1, so the asking agent gets it pushed by the answer hook.
- **What the app writes**, all through the core's rules, under the queue's lock, logged with the
  time and "owner": new tasks (and their ids), stage changes, fields, the write-up, comments, log
  notes, ticks, attachments, answers to questions, backlog notes, pauses, pings, the owner's
  decisions on lessons, and new queues (`New queue…`). Its own settings go in
  `projects.json` and `settings.json`, and the helper's transcripts in its settings folder. **The app never claims a
  task.** Claims belong to the agents doing the work.
- **Activity:** queue commits (an old queue's own git) and changed task files, newest first. **Problems:** the same
  checks as `manifest.py --check`, done natively.
- **Live:** refreshes every 2 s while the window is visible; the window never refreshes while
  hidden (the Dock badge reads on its own, below).
  **2026-10-12:** on the real queues (21 tasks), 19 ms for the first read, 0.6 ms after that. Startup code is
  229 KB (72 KB gzipped). The task panel is a separate 102 KB chunk. The app is 5.2 MB.
  **2026-10-13 (0006):** on the real queues (662 tasks), 180 ms cold and 7 ms warm (release
  build, `examples/probe.rs`). Startup code is 329 KB (106 KB gzipped), the task panel 42 KB.
  The app is 8.2 MB (`pnpm release:build`), the binary 8.4 MB.
- **Dock badge** (task 0006): Muster's Dock icon shows how many tasks need the owner in every
  shown project (the needs-you count; no badge at 0), so it's seen without opening the window.
  `badge.rs` counts in the core and sets it from Rust (`set_badge_count`), so the window has no
  permission for it. Each window poll sets it when the count
  changes; while the window is hidden and doesn't poll, a thread reads the queues every 15 s for
  the badge alone (7 ms each on the real queues), and applies Claude Code's session states as the
  poll does (0048), so a session waiting for input stays on the badge. That starts at most one
  `claude agents` run per allowed account per 15 s, on its own thread and never waited on, and only
  while some task in Building or Testing has a holder stamped with an allowed account. `needs_you` mirrors `human.ts`'s `needsYou`
  (the owner's own paused task left out, 0045);
  `app/src/needs-cases.json` holds both to the same answers. It adds 16 KB to the binary.
- **Menu bar icon** (task 0085): `tray.rs`, through Tauri's tray (Cargo feature `tray-icon`; its
  `tray-icon` and `muda` crates were already in the lockfile). The owner's template images
  (`icons/original/png/menubar-{idle,waiting}-template-2x.png`, black and alpha, read at compile
  time) so macOS tints them for the menu bar; the waiting one fills in the antennas' tips, so it
  isn't told apart by colour alone. While something waits, the status item's title shows the
  number right of the head ("3"; empty at 0) in the menu bar's font and tint (task 0089): `shows(n)`
  gives the look, title, tooltip and menu words together, set in one `Bar::set`, so the title
  changes only with the count, and Tauri keeps it while the icon is hidden. It's a Rust-formatted
  number, never a task's text. Idle at 0, waiting from 1, on `badge::dock_count`, the Dock
  badge's count: each window poll and each hidden read pass it on (`MenuBar::sync`), so it adds
  no reads, and the icon changes only when the count does. Tooltip "Muster · 3 things wait for
  you" ("Muster" before the first read); a click brings the window forward; its menu
  (right-click or Control-click) is Show Muster, the count in words (disabled) and Quit Muster,
  only built-in actions, never a task's text. Settings › Notifications turns it off
  (`settings.json` `menuBar`, on unless turned off, a bad value reads as on); `set_setting`
  applies the switch at once. The status item is removed on the main thread (AppKit stops the
  app otherwise, found in QA). The window has no tray permission.
- **Mac notifications** (task 0052): one notification when something new waits for the owner.
  `notify.rs` splits what the badge counts into items, one per reason a task waits (Your check,
  keyed by when it came in, so a second arrival is new; each escalated question; blocked on you
  or by a permission; its session waiting for input; the owner's own task) and one per lesson to
  approve, and keeps the keys it has seen. Each window poll and each hidden read (above) passes
  its state through it (`notify::after_read`), so there are no extra reads: the lessons' rules
  come from the same `(len, mtime)` cache as their count (`lessons::proposed`), and while
  Settings › Notifications is Off (`settings.json` `notify`, default on) nothing is read for them.
  New items are posted once, several in one notification (the first three named, the rest
  counted; each line at most 200 characters). Taken as seen without a notification: everything
  on the first read after start, after Off, and in a project the last read didn't have (no
  flood); a session waiting for input on an account Muster couldn't know yet (task 0069:
  `agents::known` says which accounts a read wanted (every allowed one it asks, 0182, and every one
  a stamp names) and which it read; a waiting session's item names the account it was found
  waiting on, which may not be its stamp's; `Seen::unread` holds those
  wanted on the first read, or the first after Off, until a read knows them or no longer wants
  them), so one waiting at launch isn't posted when its account's first good `claude agents` read
  lands, even when another account read fine earlier, while an account first wanted later posts
  its first read's news; and what the window shows while it has the focus (its project, or
  All; the window tells the core with `show_project_title`). News from another project is
  neither posted nor seen while the window has the focus (`mac-notification-sys` shows no banner
  while the app is frontmost) and is posted at the first read once it hasn't. The keys are kept
  in `notified.json` beside it: the last 500, plus every one still waiting, so nothing comes up
  twice. Permission: asked at the first thing to say, never at start
  (`tauri-plugin-notification` 2.6.0 on desktop grants it itself; macOS asks the owner on the
  first post; no usage string exists for notifications). Posted from Rust through the plugin, so
  the window has no notification permission; local only, the plugin
  has no network code. Under `tauri dev` they show as Terminal's (the plugin's rule for unbundled
  builds); the built app shows them as Muster's.

### Languages: the i18n layer (mu-146)

Muster speaks English and Persian through **Drover's translation layer, copied from Drover's
commit `a990fc8`** (Drover task 0007, decision 0021), with the same names and file shapes so the
move into Drover is a merge. Our own small layer, no library; the files are in i18next's JSON
format (nested keys, `{{name}}`, plural suffixes `_one`/`_other` from `Intl.PluralRules`), and the
Rust core reads the same JSON.

- **Files** (`app/src/i18n/`): `runtime.ts` (loading, lookup, plurals, `{{name}}` filling with
  numbers in the language's digits, FSI/PDI isolation of filled-in values right to left,
  `applyToDocument` setting `<html lang dir>`, the pseudo-locale), `react.tsx` (`useT`, `useLocale`,
  `<Trans>` for elements inside a sentence), `lib.ts` (`tn`, `tl`, `isolate`, `unmarked`,
  `sentences`), `format.ts` (`fmt`: dates, numbers, lists, relative times through cached `Intl`
  objects), `locales.ts` (`en`, `fa`, and `pseudo` while developing; `fa` is right to left),
  `types.ts` (keys typed from the English files: a typo fails `tsc`), `boot.ts` (the language at
  start), `prefetch.ts` (`withText`: a lazy screen's text loads with its code), `index.ts`. Tests:
  `runtime`, `react`, `lib`, `locales`, `bidi`, `literals`; `testSetup.ts` and `testReader.ts`
  read the files from disk with English loaded. Muster's differences from Drover's: no agent
  language (`agent.ts`, `agent.json`; see DESIGN.md › Languages), no `lib` namespace (`tl` reads
  `common`), and its tests render with react-dom's server renderer, as Muster has no jsdom.
- **Namespaces** (`app/src/i18n/locales/{en,fa}/<ns>.json`), each area's task editing only its
  own: `common` (shared words), `shell` (sidebar, toolbar, filter bar, Settings: mu-146), `board`
  (board, views, backlog, sessions: mu-147), `pane` and `help` (task pane, comments, review, the
  manual: mu-148), `native` (menu bar, notifications; Rust reads it), `cli` (mu-152), `core`
  (mu-153).
- **Lazy:** the files aren't in the JavaScript. `tools/vite-locales.mjs` (Drover's plugin) serves
  them at `/locales/<language>/<ns>.json` in development and copies them, minified and checked,
  into the build; the window fetches only the language it shows. Before the first render
  (`main.tsx`) it loads `common` and `shell` in the language saved in `localStorage`
  (`muster.locale`, a copy of the core's), sets `<html lang dir>`, then asks the core
  (`locale_info`) and follows it, and its `locale-changed` event after.
- **The setting:** `lang` in `settings.json` (`settings.rs`), written only once picked, so a file
  from before it loads unchanged; unset follows macOS's language for Muster (`i18n.rs`
  `system_languages`: the app's own choice in System Settings, else the Mac's, read once with
  `defaults`, as Muster has no objc2 to ask `NSLocale` as Drover does). Settings › Language, at the
  top of Settings whatever group is open, has a chip per language, `English · فارسی`, each in its
  own script with its `lang`; the chip of the language in effect is on, and a line says when it's
  macOS's. A pick goes through `set_setting lang`: the core saves it, switches its own text and
  rebuilds the menu bar's menu (`MenuBar::retext`), and the window switches at once, no restart.
- **The core's text** (`i18n.rs`): `include_str!` of each language's `native.json`, the same
  plural rule as `Intl.PluralRules`, Persian digits for counts; the menu bar's menu, tooltip and
  count are in it (`tray.rs`), its title keeps Western digits like the Dock badge.
- **The guard** (`literals.test.ts`): reads every source file with the TypeScript parser and
  fails on text written straight into a screen (JSX text, text props, text fields, sentences).
  Until every area has moved, the files still holding text are listed with the task that moves
  them (mu-147, mu-148); a file off the list fails, and so does a listed one with nothing left.
  The shell's files are off it. `Sidebar.tsx` is the task pane's right column, not the window's
  sidebar, so it's mu-148's. mu-155 removes the list.
- **The shell in the layer** (mu-146): the sidebar, the header, the filter bar, every Settings
  group, Waiting for you and Recently finished read their words from `shell.json`, and the words
  more than one area uses (stage names, priorities, types, time units, "Loading…", Save, Cancel)
  from `common.json`. The parts of other areas' files the shell shows moved with it (the Who and
  Group by chips, the card previews in Appearance, the helper's account picker, the sidebar's
  check-in button and Your check limit, a project's Run buttons, the "?" buttons' names), and those
  files stay listed for their own text. Data lists (`STAGES`, `SETTINGS_GROUPS`, `THEMES`,
  `MODES`, `GROUPS`, `SIDEBAR_ORDERS`…) keep their ids, which is what's stored and compared, and
  read their words through getters each time, so a language switch shows at once and nothing
  keeps English in a constant. What's stored stays as it is: a filter's value, a theme's id, the
  Other heading ("Other" in projects.json, سایر on screen), a span typed as `30m`. Dates in
  Recently finished say the weekday and month in the language's own words. `shellFa.test.ts`
  draws all of it in Persian and fails on any Latin word that isn't a product name, a key or the
  data shown. The header's account meter and Settings › Accounts (mu-113) read theirs from
  `shell.json`'s `meter` and `profiles`; a reset time's month is Recently finished's.
- **A type as the screen says it** (mu-194): `fields.ts` `kindLabel` says the core's types
  (`KINDS`) and the older ones the queues still hold (`LEGACY_KINDS`: task, feature, idea) through
  `common.json` `kind.*`, and any other value as written. The filter bar's Type row uses it.
  Older types are for display only: New task and the pane's type chip offer only `KINDS`, and the
  task files keep their `type:` as written.
- **How to add a string:** put it in `app/src/i18n/locales/en/<ns>.json` (nested keys; a count's
  forms as `key_one`/`key_other` with `{{count}}` in `_other`), and its Persian in
  `locales/fa/<ns>.json` with the same placeholders and every Persian plural form
  (`locales.test.ts` checks both); in a component, `const t = useT("<ns>")` and
  `t("group.key", { count, name })`; outside one, `tn("<ns>", "group.key")`. Values are filled
  in as text, never HTML. A label in a data list is a getter (`get label() { return tn(…) }`),
  never a string kept at load. A view a test calls as a plain function uses `tn`, not `useT`. A
  string the core shows goes in `native.json` and through `i18n::t`.

### The AI helper

The **Helper** tab: a chat per project that does what the capture session does. You write ideas,
bugs and tasks (and paste screenshots). It checks the queue for duplicates, reads the project to
understand the code, files one task per point with your words, details and acceptance criteria,
and asks in plain words when something's unclear, with its own pick. You see its replies and a
folded "steps" line, never file edits.

- It runs your own `claude` with the account you pick once (decision 0002), in restricted mode:
  only `muster` and read-only tools, file reading inside the project folder, and anything else
  refused. Router and token variables are stripped. Model: Sonnet, or Haiku or Opus picked in
  Settings (0036; saved in `helper/helper.json`, and anything else there reads as Sonnet).
- **Accounts:** the picker shows each allowed account's login (0012; from `claude auth status`, no
  model call), with **Change account** any time. A saved account that is `.claude` or no longer on
  the list is cleared, and the login script always exports `CLAUDE_CONFIG_DIR`. An account that only works through a shell command
  with a token (like `cc-hg`) isn't logged in on its own: the helper says so in plain words and
  offers another account or **Log in…** (Terminal on that account's own `claude auth login`).
- Conversations resume per project. **New conversation** starts fresh, and **Stop** ends a turn.
  The transcript lives in the app's settings folder (`helper/<project>.jsonl`). Pasted files are
  saved in the queue's backlog folder under `helper/`, and the message names them.
- Checked end to end on a throwaway project: a two-item message filed both items (a high-priority
  bug and an idea) through `muster new`, for $0.05–0.07 a turn. Writes, deletes and reads outside the
  project were refused. The queue check was clean afterwards.

### The command line

The app's binary is also the `muster` command (`bin/muster` runs it). It uses the same Rust code as
the app (`ops.rs`), so the app and the command line can't disagree. It picks the queue from
`--queue <name|folder>`, then `$MUSTER_ROOT`, then the current folder's: its repo's `.muster/`
(the main checkout's, found through git's common folder from any worktree, 0005), else the nearest
old queue (a queue, or a `tasks/` beside or above it); a task named by another project's key
(`br-7`) picks that project's queue instead.

`muster` is linked into `~/.local/bin`, so it works in any terminal and in any Claude Code
session as `! muster …`. `muster help` lists everything, and `muster help <command>` shows usage,
options and examples (`--json` for scripts).

```
muster new "Board flickers on resize" --type bug --priority high --body "Steps: …"
muster list [--status todo] [--json]      muster show 0007 [--json]
muster block 0007 --on you "pick a colour" muster set 0007 priority low
muster note 0007 "Checked on the Mac"     muster attach 0007 shot.png
muster answer 0007 Q2 "Use 5173"          muster check      muster projects      muster init ~/playground/x
```

**Output on a closed pipe** (task 0170): every line a command prints goes through one writer in
`cli.rs` (`out!`/`outln!`, never the std print macros, which panic on a write error;
`tests/broken_pipe.rs` keeps them out). When the reader closes the pipe early
(`muster show mu-1 | head -1`), the write's broken pipe ends the output quietly: no panic, nothing
on stderr, and the command still finishes its queue writes and exits with its own status (0, not
git's 141: restoring SIGPIPE would kill muster mid-write under the queue's lock). Any other write
failure (a full disk) is said once on stderr when the command has ended and turns a status of 0
into 1, so a script never takes cut-off output for all of it. The line depends on the command. A
command that may write to the queue (note, add, new, ask, comment, pass, and anything `reads_only`
in `cli.rs` doesn't know as a read) that succeeded says
`muster: note ran and its change is saved, but its output couldn't be written (…): don't run it again`,
as `comment` does when its action stood, so nobody repeats a write for the lost output. A read
(show, list, help, inbox --keep, …) or a refusal says `muster: can't write the output: …`. The
app (no arguments) doesn't use the writer.

**Claims** (task 0002): `muster claim <id>` or `muster next` takes a Ready task for you, one
per holder (`--as`), and says which queue it claimed in. A claim made in a queue that came from
`MUSTER_ROOT` but isn't the current folder's own queue is refused unless `--queue` says so. The
claim has a **lease**: `heartbeat`, and the holder's notes, passes and questions, renew it, and
`muster reclaim` (or **Release** on a stale card) returns a claim quiet past the lease to Ready:
12 h, or this Mac's lease from Settings (0036).
**Takeover** (task 0090, `ops.rs` `takeover_task`): `muster claim <id> --takeover --as <label>`
moves a Building claim under the same label from another session (or agent) to this one, under
the lock, and renews the lease. Allowed when the holder logged `muster checkin <id> "…" --handoff`
(`check-in 40% · handing off · …`) after the latest claim or takeover, or after 30 minutes with no
heartbeat (`TAKEOVER_QUIET`); the owner always. A paused claim is refused for everyone; a blocked
one (waiting, quiet by design) only after a handoff or for the owner. The log line is read back
before it's written, so a label with `: ` is refused. Logged `<new>: took over the claim from <old>
(handed off | quiet 42 min | owner's override)`. The task's top-level mail in the old mailbox moves
to the new one (`queue::move_mail`, notes renumbered) and its `asks.json` entries are repointed.
A refusal says who holds it, since when, and the two ways out. A note or check-in that would read
as a takeover or a handoff line is refused.
The mail move (mu-156) repoints `asks.json` first, then moves each file, carrying on past one it
can't move; the error names `asks.json` (malformed, or unwritable) and every file left behind,
and the claim stays moved. Running the takeover again finishes it: the holder whose claim came
from a takeover (the log's latest takeover line, with no claim into Building or Shaping after
it, names it as the one who took over: `ops::taken_over_from`) gets the rest moved from the old
holder that line names, never one given as an argument, and the heartbeat renewed, with no log
line: `mu-42: you hold it already; moved 2 messages from mail/1c6af0d9`, or `nothing left to
move`. Any other holder running it gets "you already hold".
**A claim with no session to reach** (task 0184): `session::restamped` gives a bare-label claim
(or the acting session's own stamp under another account) the acting holder's stamp, and
`ops::restamp` writes it with the takeover line `<who>: took over the claim from <old> (stamped)`
(`queue::STAMPED`, a known reason of `takeover_line`, so `free_text_line` refuses it as free
text). It runs under the lock in the holder's own writes: `heartbeat_task`, `checkin_task_with`,
`add_note` (`restamped_text`), every `change_locked` act of the holder that keeps the claim or
passes it to Testing (pass, block), and
`takeover_task`, which re-stamps instead of refusing "you already hold it" and moves no mail;
run again after that, it is mu-156's rerun, which finds no other mailbox to move from.
The owner's and an unstamped holder's acts never re-stamp, and in `change_locked` only a change
that keeps the claim or passes it to Testing does (not a reclaim or a freed task's release).
A Testing task's `held-by:` is re-stamped the same way (task 0185): `add_note` (`restamped_text`
with the field `held-by`) and every `change_locked` act that keeps it in Testing (block,
unblock), logged with the same `(stamped)` takeover line; `queue::builder_chain` adds a
`(stamped)` line's holders only when its old holder is in the chain already, so a tester's
re-stamp keeps it out and 0047 still lets it record and pass. A pass or return out of Testing
doesn't re-stamp the hold. Only the same session id and agent under
another account re-stamp; a stamp with no account (`~/.claude`) never replaces a labelled one.
`return_change` gives Building back to the builder's exact stamp (`unless_taken`), no longer its
bare label; while that session holds no other unblocked Building task (`holding`) and hasn't
acted on it (`untouched_send_back`: not paused, heartbeat still the stage's own minute, and
`acted_since_send_back` finds no takeover and no line of the holder's after the send-back's stage
line) `next_task` points a session under the same label at it and `takeover_task` allows the
takeover at once, `(sent back, not picked up)` (`queue::SENT_BACK`). The owner's **Release**
(`ops::release_unreached`, Tauri `release_unreached`, `api.releaseUnreached`, the pane's
`ReleaseUnreached` in `Sidebar.tsx`, words in `paneData.ts` `unreachedWords`): owner only, re-read
under the lock; Building → Ready with the claim cleared, worktree and branch kept, logged
`(released: no session to reach)` (`ops::UNREACHED`); Testing keeps its stage with `held-by:`
cleared and the same words in a line that isn't a stage line. Refused for a holder with a
session, the owner's own hold, a paused or unheld task, another stage, or one that moved on from
the stage the window showed. `ping::NO_SESSION` is a template with `{label}` (`no_session`),
word for word `reach.ts` `NO_SESSION` (a source check on each side); a hold in Testing, which no
takeover reaches, gets `NO_SESSION_HELD` ("no session to reach: Release it") in both. The
confirm's words are whole sentences with `{{who}}` (`whoIn` fills the label; the question is cut
at `{{who}}` only to put the name in its `<bdi>`). Its Esc runs through `watchReleaseEscape` (a
capture listener on the window while it's open); its group carries `data-own-esc` (`ownEscape.ts`
`escOwned`), so the unsaved write-up's and comment box's window Esc listeners leave an Esc pressed
in it alone, and holds the focus (`tabIndex -1`) while the release runs. A release that goes
through moves the focus to the Next step group (`tabIndex -1`), since Release goes with the claim.
**Questions:** `muster ask`, `escalate`, `inbox` and `answer` follow the file contract. `bin/task`
is a stand-in that runs them. The answer hook still reads the mailbox itself, for speed. With mail
waiting it takes the queue's lock shared without waiting (mu-156: `flock(LOCK_SH | LOCK_NB)` on
`mail/.lock`, opened with `O_NOFOLLOW`), decides whether each check-in, pause or resume request
still stands, and only then moves it to `delivered/`. Every muster write locks it exclusive, so a
write in progress (a takeover among them) or a lock it can't open leaves that queue's mail for the
next call, and a request racing a takeover is left for the mail move to carry to the successor,
never retired in between. Shared is enough: the app's settled reads hold it shared every 2 s and
don't delay the mail, and two hooks never hand a file over twice (the rename happens once; the
loser skips it). Mail for a task whose claim was taken over from this session (the task in
Building or Shaping, its latest takeover line naming this session as the old holder, no claim
after it) stays in the mailbox for the takeover's rerun to move; a task file that can't be read
(not UTF-8, no permission) or found counts as no takeover, so its mail goes out. Each file is decided, moved and
read on its own: one that can't be moved stays for the next call, one moved but not UTF-8 is set
aside in `delivered/`, and the mail beside them still goes out. An empty mailbox returns before
the lock.
**Pick-one questions** (task 0049): `muster ask <id> "<question>" --option "<text>"…
[--pick <a|1>] [--no-answer <a|1>]` writes 2 to 6 options (one line, 200 characters each) under
the question as plain indented lines (`- (a) … (my pick)`, `With no answer: (a)`), so file
contract v1 readers see text; `muster answer <id> Qn --pick b ["<more>"]` answers with the
option's text. Refusals (count, length, a pick or fallback that isn't an option, a question
without options) write nothing. `muster escalate` prints the options for `WAITING-FOR-OWNER.md`.
`bin/task` passes no options: it asks in words and answers any question in words.
**Ping** (task 0039): `muster ping <id>` asks the holder's session for a check-in (Building),
`muster ping <id> "<message>"` sends it a message (Building or Testing), the same rules as the
app's buttons (`ping.rs`): only the session in the holder's stamp, refused with why otherwise,
logged on the task.
**Pause** (task 0045): `muster pause <id> ["<why>"]` and `muster resume <id>`, the app's rules
(`pause.rs`): Backlog through Testing; while paused, `claim`, `next`, `start`, `ready` and every
pass refuse it and `reclaim` skips it; the holder's session in Building or Testing gets a stop
request, and on resume a carry-on (and in Building a fresh lease). `muster list` marks it
`[paused since …]`, INDEX.md "paused". `muster check` warns about old paused blocks (0059), and
`muster check --fix-pauses [--dry-run]` lists them in every queue Muster knows before changing
anything, then unblocks each (an old-layout file moves back to `todo/`, stage Ready) and pauses it
from the original time: a `paused (why)` and `unblocked` line with that stamp, then a line stamped
now saying it was converted. A dry run changes nothing; a task it can't unblock is named, left as
it was, and the command exits 1.
**Session states** (task 0048): `muster list --json` gives each task `sessionState` (`working`,
`needs-input`, `ended`, `unknown`), read from `claude agents --json` for that listing only, under
the caller's `$HOME`; no other command runs it, a write never does.
**The review of record** (task 0027, `review.rs`): `muster review <id>` prints the brief for the
next round of a started task (the range: merge base..tip the first time, then the last reviewed
commit..tip while the branch still has it; the task quoted as data, the builder's answers to
re-check, the owner's lessons, the JSON shape) and runs no agent (only the dispatcher's runs go
through a runner, decision 0015): a session that didn't build it runs `/deep-review` on the
range and records the result with `--record -` (strict JSON: `deny_unknown_fields`, every text
one line and capped, `head` must be the branch's tip, every
answered finding needs an answer, findings under confidence 75 refused). The truth is
`reviews/<id>.json` in the queue; the task gets a `## Review` section rendered from it (above
Comments and Log; `muster edit` leaves it out and refuses a new `## Review` line), `review: round
<n> · <open> open · <answered> answered` in the front matter (`Task::review_round`,
`review_open`, `review_answered`; a malformed one is a `check` problem) and a log line per change.
After a record, open findings (not nits) return it to Building (rounds 1 and 2, `ops::return_task`)
or block it on you (round 3 on, `ops::block_as`), each after the lock is let go. `--fixed` and
`--dispute` are the holder's in Building (keep the claim alive); `--settle` is the owner's.
Owner findings (mu-101) are answered with `--fixed` and `--dispute` like any other, and the next
round checks them; a round that accepts the dispute of an owner finding leaves it disputed
("dispute left for the owner"): it doesn't hold the round or the merge, and only the owner's
`--settle` closes it. The three-round cap applies only while a finding that isn't the owner's is
open. `--record` refuses a finding with lens `owner`: only a send-back raises one. A send-back of
a merged task (no branch) records it too, tied to `commit:`; its pass waits for `muster start`'s
fresh branch, whose round settles it (`owner_to_recheck`, `holds_unbranched`).
`parallel::merge` calls `review::merge_check` before its lock, rebase and gate (clean at the
tip, or the owner's override, said in the report) and `review::carry` after each rebase.
`review::Review::stands_at` (clean, and of the tip) is the one test the merge and `muster gates`
use for the review. The section helpers, `clean_head` and the tester check (`check_tester`:
Testing, not paused or blocked, not the builder chain, the owner always) live in `testing.rs`,
shared with the gate records. `testing::headings` skips `## ` lines inside a fenced block only
when the fence closes in the write-up: fences are read only above the tool's Comments (or its Log,
`queue::comments_span`/`log_heading`, found plainly) and never in the `## Questions` content
(`queue::questions_span`), whose answers, like comments, hold the tool's indented lines. A fence
still open there hides nothing, so a stray fence in a write-up can't hide the tool's sections and
get them copied on each record, whatever fence lines a comment or an answer adds below. A
`## Questions` that turns out quoted in a closed fence is the write-up's, and it's read again with
its lines as fences. A backtick opener whose info string holds a backtick isn't a fence
(CommonMark).
**Gate records** (mu-131, decision 0014, `gates.rs`): `muster gate <id> criteria|visual` prints
what to check; `--record -` takes `{head, verdict, items: [{id, verdict, expected?, text?,
evidence[], note?}], not_checked?: []}` (`deny_unknown_fields`, 1 MB, 200 items, 20 evidence paths
an item, texts one line and capped; `not_checked` criteria only, 20 lines of 200 characters, stored
as `notChecked` and rendered as `Not checked: a; b` under the criteria, never in the verdict). `head` must prefix the branch's tip (`gates::tip`, from `parallel::branch_span`);
the recorder passes `testing::check_tester`. Criteria ids are normalised on both sides
(`criterion_id`: `AC1`, `ac1`, `1` → `AC1`; a criterion without a label is `AC<n>`); items are
`met|partial|unmet|unproven`. Visual items are `pass|fail|needs-walkthrough` against a screen-state
id (`[a-z0-9-_./:@]`, 120 at most), and `expected: false` can't pass. The verdict is worked out
(criteria green when every non-owner criterion is met; visual red on any fail, needs-walkthrough on
any walkthrough) and a stated one that differs is refused. Evidence paths are checked by
`evidence_path`: no absolute path, backslash, `.`, `..` or empty part; `attachments/` and
`attachments/<id>/` must be real folders (not symlinks); the canonical file must be a file inside
the canonical folder. The record (full tip as `head`) goes to `gates/<id>/<gate>.json` atomically,
replacing the last; the task gets a `## Gates` section rendered from every record (items as
`- [x] **AC1** met` with untrusted text on indented lines), `gate-records:` in its front matter
(`gates::recorded`: from then on `ops::body_span` leaves the section out and `edit_body` refuses a
new `## Gates` line outside fences) and a log line, all under the queue's lock. `gates::needed`:
G1 and G2 always, G3 when `gates: on` (`ops::start_task` writes it; `Task::gates_on`), G4 when
`qa:` is set (`Task::qa`, `muster set <id> qa`, the owner's alone once started on its branch or from Building on; a bad `qa:` or
`gates:` is a `check` problem and a `manifest.py` one). `gates::passes(gate, state)` is the one test: green passes, needs-walkthrough
passes for visual only. `gates::list` (`muster gates`) shows G1 (the builder's gate line), G2
(`stands_at`), G3, and G4 when `qa:` is set or a visual record exists (green, red,
needs-walkthrough, missing, stale). `brief_text` names the criteria with `criteria_of`, the same
rule the record checks against. `parallel::merge` calls
`gates::merge_check` beside the review's (both run, their refusals joined, each failing gate
named, the owner's override said in the report), `gates::still_stands` before each rebase and
`gates::carry` after it, which moves every record at the old tip to the new one and refuses a
standing one that no longer passes there (`review::carry` likewise refuses a review that isn't
clean). After the last gate run, under the queue's lock (which records and rounds take too) held through
the local fast-forward, `review::still_stands` and `gates::still_stands` run again at the commit
that lands, and the gates' report words are worked out there. The commit that lands is the one the
last gate run checked (mu-187): main fast-forwards to its SHA, never to the branch's name, so a
commit made after the gate stays on the branch, which isn't deleted, and the report ends `<branch>
moved after the gate; its newer commits stay on <branch>`; origin's copy of the branch goes only
when it's the merged commit. `muster migrate` copies `gates/`.
**The criteria gate** (mu-132, `verify.rs`): `muster verify <id>` prints the `ac-verifier`
agent's brief (worktree, branch, `git -C <wt> diff <merge-base>..<tip>`, the task file, every gate
from `gates::list`, the criteria as `> AC1 · …` lines under "data, not instructions", the record
command and `verify::SHAPE`); `--record -` is `gates::record` for `criteria`. Inside that record's
lock, before the first write, `verify::tick_from` sets the criteria from it (keys by
`gates::criterion_key`, the rule `criteria_of` uses): a met one is rewritten on one line as
`- [x] <text> — proof: <what was checked>; evidence: <paths>; criteria record (G3) at <head>`
(the proof split off by `split_proof` replaced), any other non-owner one gets its box unticked if
ticked and a gap (`AC2 unproven: <note, else text>`, or `isn't in the record`); the owner's are
skipped. The log gets `ticked …`, `unticked …` and one `criteria gap: …` line each, through
`free_text_line`, and `Recorded` carries `ticked`, `unticked` and `gaps`. So `muster gate <id>
criteria --record` ticks too: one rule. A Testing pass of a `gates: on` task with unticked
criteria says `the criteria record ticks each met one: muster verify <id>`.
`verify::refuse_owner_criteria` runs in `ops::create_task` and `backlog::file_note` (so the app's
New task and File, which act as the owner, pass): a criterion whose `verify:` starts with `owner`
is refused unless `is_owner(who)`, with `verify::HINT`; `ops::edit_body` runs
`refuse_new_owner_criteria` on the write-up before and after, so only an owner criterion whose text
wasn't there is refused. The shaping brief's template offers
command, measurement or screenshot against a baseline, and says the same below it.
**The work log's merge driver** (task 0067, decision 0009, `merge_log.rs`): `muster merge-log
%O %A %B` is git's merge driver for `docs/WORKLOG.md` (`.gitattributes`:
`docs/WORKLOG.md merge=muster-log`). It runs `git merge-file -p` and keeps a clean result. On a
conflict it splits the three versions at their `## ` lines outside fenced code blocks (``` and
~~~, CommonMark's opening and closing rules, an unclosed one running to the end) into the
preamble, then whole entries (bytes as they are: CRLF, a missing last line end and non-UTF-8 text are kept). When each side is
the base's preamble and entries, unchanged and in order, with any number of runs of dated entries
(`## YYYY-MM-DD…`) inserted among them (on top, or under any base entry: tasks 0190 and 0196,
`inserted`, `both_inserted`), it writes the base with each side's runs at their points; where
both sides have a run at the same point they're merged there by date, newest first, theirs (`%B`) first
on a tie, each side's own order kept. An entry both added is kept once, at ours' point when the
sides put it at different points; that's only on this path: when merge-file merges the sides
cleanly (the same entry at two points that don't touch), its result stands and the entry is there
twice. The dedup counts copies per entry, so each entry is kept max(ours, theirs) times: where one
side repeats an entry the other added too, both copies stay, where task 0190 dropped every copy
theirs had and lost one. `inserted` first reads a side as one run as task 0190 did (entries the
same from the top and from the bottom), so a run goes to the point 0190 put it at; otherwise it matches the base's entries
from the bottom, each to the lowest side entry it can be (one pass,
so every run is as high as it goes), then lets each run down over the base entries below it that
it repeats, up to the next run: a run that repeats its neighbours fits several points, all giving
the side's file. `both_inserted` pairs the two sides' runs whose points overlap in one pass (each
side's runs are at disjoint ranges, in order) and puts a pair at their topmost shared point, any
other run at its own topmost. Anything else (an edited or removed base entry, a changed preamble,
an undated new entry) gets merge-file's conflict markers (labels `ours`, `base`, `theirs`) and
exit 1, with "both sides changed more than new dated entries; left the conflict markers" on
stderr. The result replaces `%A` whole or
not at all (`replace`, task 0071): written to a new `.<name>.muster-merge-<pid>` beside it with
`%A`'s mode, then renamed over it, with no fsync (git reads it straight back, and a full sync
can fail on network and external volumes). On a failure the new file is removed and `%A` is
untouched. A muster killed between making the new file and the rename leaves it in the worktree
root (`%A` untouched); nothing cleans that up.
When it can't merge (a missing file, over 64 MB, merge-file failing, a `%A` it can't replace)
it runs `git merge-file` in place on the untouched `%A`
(markers and 1, or its clean result and 0); only if that fails too is `%A` left as it was, with
exit 2. A driver that exits non-zero with `%A` unwritten would leave git a conflicted file
holding only ours, so the configured driver is
`muster merge-log %O %A %B; s=$?; [ $s -le 1 ] && exit $s; git merge-file -L ours -L base -L theirs %A %O %B`:
a muster that isn't on the PATH, was killed or exited 2 hands over to merge-file (tested with a
rebase under a PATH of the found git's folder, `/usr/bin` and `/bin`, and no muster).
`merge_log::configure` (called by `muster start` from where it runs, and by `muster init` for a
repo's `.muster/`) sets `merge.muster-log.name` and `merge.muster-log.driver`
(that command) with `git config --local` only for a key `git config --get` doesn't
find, and prints a line when it wrote one. A failure is a warning on stderr and stops nothing.
`parallel::merge` puts `merge_log::config_args(this_muster())` before its `rebase` (`-c` for both
keys, the same command with the binary's path, `%` doubled and shell-quoted), so the merge's rebase uses this binary whatever the config
says. A clash of only new dated entries, at any number of points, no longer sends a task back to Building. Its unit tests
build the muster binary they run as the driver once per test run (`cargo build --bin muster`
into the harness's own target and profile), so `cargo test --lib` works in a fresh target too.
**Lessons** (`lessons.rs`): `reviews/lessons.json` (truth) and `lessons.md` (rendered) in the
queue (`layout::reviews_dir`, `layout::lessons_file`: `.muster/reviews/` and `.muster/lessons.md`
in a repo's queue; `muster migrate` copies both, never moves them). `propose_locked` runs under the lock of each record and settle: a `kind` confirmed on 2+
tasks proposes one for builders, false-positive on 2+ one for reviewers, never one already
proposed, accepted or rejected. `muster lessons accept|reject` is the owner's (`--as owner`);
over 40 live, the least recently mattered expire; 60 days without a finding of its kind or an
acceptance, an accepted one reads as expired. `muster start` prints the live builders' lessons
whose areas overlap the task's `touches:` (`parallel::touches_overlap`; all of them without
touches). `QueueInfo::lessons_waiting` feeds the window's needs-you count, the sidebar, Waiting
for you's Lessons block and the Dock badge (`badge::dock_count`); `muster check` lists it.

### Keys, ids, locks, time and git

- **Keys** (task 0033, `key.rs` and `taskKey.ts`, held to `app/src/key-cases.json`): a task is
  shown and typed as `mu-12`, its project's prefix and its number; files, `depends:` and ids in
  JSON keep `0012`. The prefix is 2 to 4 lowercase letters stored with the queue (a `.muster/`'s
  `config.toml`, `prefix = "mu"`; an old queue's README.md front matter, `prefix: mu`; read only
  by `key::stored_prefix`), else the first 2 letters of the project's
  name (3 or 4 when another project Muster knows starts the same), stored by the first write
  under the queue's lock (`key::pin_prefix`) so it stays. Every command takes `mu-12`,
  `MU-12`, `12` or `0012`; another project's key runs in its queue (refused against a `--queue`
  that names another). Cards, rows, the timeline, the graph, the pane, the parent line, the
  activity feed, `muster list`/`show`, what commands print, branches (`task/mu-12-slug`) and new
  log lines, INDEX.md, `muster check` and the core's refusals (`key::name_tasks`, at the command
  line's and the window's edge) name tasks by key, and a key in a task's text, log, comments or
  questions links to it. `manifest.py` and `tasks.json`, which scripts read, keep ids.
- **Ids:** four digits, taken in order under the queue's lock by whoever creates the task: `muster
  new`, the app's New task, or the helper (through `muster new`). Taking them under the lock is
  what prevents two tasks getting the same id. `bin/task` doesn't create tasks: it only runs
  `muster`'s question commands. `manifest.py` reports `next_id` and catches duplicates left from
  before. A coder that finds something new runs `muster add`, a note in the backlog.
- **Locks:** `flock` on `mail/.lock` for every task-file write through the tools (`muster`, the
  app, and `bin/task` through `muster`), with each file written to a temporary file and renamed
  over the old one. A claim is `muster claim` or `next`, which read the stage and write the claim
  under that lock; in an old queue the file also moves from `todo/` to `doing/`.
- **Time:** the tools stamp times. Agents' own timestamps aren't trusted (`DOGFOOD.md` 37).
- **Git:** an old queue is its own git repo, beside the code repo, and capture commits named files
  in it (never `git add -A` while others write, `DOGFOOD.md` 31). A local `.muster/` isn't in git:
  only its `config.toml` and `.gitignore` are committed with the code.

### How it was checked

- `bin/gate`, on 2026-10-14 at this task's branch: `pnpm check` (typecheck; 1,556 UI tests in 69
  files; the build; `cargo fmt --check`; clippy with warnings as errors; 686 Rust unit tests and
  31 integration tests in `tests/`), `bin/test_task.py` (60 tests), `bin/test_commands.py` (16)
  and `cargo test` again in the Pacific/Kiritimati time zone, all green. Among the integration
  tests, `tests/interop.rs` asks a question with `bin/task`, answers it as the app does, reads it
  back with `bin/task inbox`, is refused when it's already answered, and passes
  `manifest.py --check`. The real queues are read by `examples/probe.rs`. The release app is built
  with `pnpm release:build`; its screens have had visual QA rounds (mu-61) and the owner's checks
  in Your check.
- Headless screens (mu-143): `app/e2e/shot.mjs` serves `app/dist` on 127.0.0.1, drives it in
  headless WebKit (Playwright from another checkout on this Mac, `$PLAYWRIGHT`; nothing added to
  Muster's dependencies) and answers Tauri's IPC from a read-only snapshot (`app/e2e/mock.mjs`:
  `muster list --json` made into the core's `state`; every command that would write is refused).
  It checks the Feature grid at the app's default 1400×880 window (its grid roles, +N more, a card
  opening the pane, the layout kept, the arrow keys and the focus clear of the sticky names, the
  off-edge cue, Only what needs me, the full Your check head, right-to-left as the window in
  Persian) and the columns, and takes the screenshots, light, dark and right-to-left. `cd app && pnpm build && node e2e/shot.mjs`; it never touches the owner's screen.
- Hermetic tests (task 0062): every read of the projects Muster knows (`key::known`,
  `cli::base`, `cli::config_file`, the app's `base`) starts at `projects::places()`, the folder
  queues are found under and projects.json. The app and `muster` get this Mac's
  (`$MUSTER_BASE` or `~/playground`, and `~/Library/Application Support/ai.sweetlemon.muster/projects.json`).
  A lib test gets empty ones in a temp folder of its own thread, unless it sets its own with
  `projects::use_places(Places { base, config_file })` for the rest of its thread. A thread the
  test starts gets its own empty ones. In a test build, `projects::load`, `save`, `edit` and
  `queue::discover` panic on this Mac's real projects.json or `~/playground` and name
  `projects::use_places`. So `check --fix-pauses` with no `--queue`, and the paused lane's
  `find_all` and `convert_listed` over the app's queues (0059), see only a test's own queues, and
  a change that sent them to this Mac's would fail the test before reading anything (a test run
  once converted Drover's real dr-22 that way). Integration tests build the core without `cfg(test)`, so they have no
  guard: the ones that call the core in-process call `use_places` (`no_projects` in
  `tests/interop.rs`), and every `muster` they run gets a temp `HOME` of its own. Checked by
  running the whole suite with `HOME` set to an empty folder whose projects.json folder and
  `playground` can't be read.
- Test queues have a private parent (mu-191). A queue's project is its parent folder and every
  repo beside it is a checkout (`backlog::project_places`), so a queue straight in the shared
  `$TMPDIR` took other sessions' checkouts there as its own, and their `docs/now/` pages refused
  the review tests' passes. Every lib test's temp folder comes from `testkit::tempdir()`, and
  every integration test's from `tests/common/mod.rs`'s `common::tempdir()`: both are
  `<tmp>/proj/q` with nothing else in `<tmp>/proj`; a test that wants checkouts beside its queue
  plants them in that parent. `testkit::tests::test_queues_have_a_private_parent` fails when a
  file under `src/` or `tests/` other than those two helpers names `tempfile`. The `muster check`
  tests in `interop.rs` and `deps_cases.rs` assert no now page is read. Checked by running
  `cargo test` with decoy checkouts (`.git/` and a now page covering `app/`, `docs/`, `bin/`; and
  one with no `covers:`) in `TMPDIR`.
- `bin/gate` runs every step in a home of its own (task 0062): `HOME`, `XDG_CONFIG_HOME` and
  `MUSTER_BASE` in a temporary folder it removes at the end, `MUSTER_ROOT` and `TASKS_ROOT` unset.
  `CARGO_HOME`, `RUSTUP_HOME`, `~/.cargo/bin` and pnpm's store and cache stay this Mac's, so
  `pnpm install --frozen-lockfile` in a fresh worktree still links from the real store.

- `bin/test_task.py`: 60 tests, among them ask/answer/deliver, numbering, refusal of an already
  answered question, earlier questions listed, old unnumbered questions, sessions from env and
  text, control characters and tabs stripped, uppercase session ids, escalate, the hook given bad
  input, the inbox command and an atomic manifest.
- Live: a self-test question asked and answered through `task` appeared in the asking session's
  context after its next tool call (2026-10-12).
- Drover's own implementation of the file contract found three contract bugs within an hour, all
  fixed (`DOGFOOD.md` 36).
- Hook cost: about 10 ms per tool call with an empty mailbox.

### `.muster/` in the repo (decision 0003, task 0005 step 1)

Built: the layout, local mode, one queue for every worktree, discovery and `muster migrate`.
Muster's own queue moved on 2026-10-13 (`migrated-from.txt`: from `~/playground/muster/tasks`,
which stays, hidden, as the project `muster-2`). The other 19 projects' queues are still
`<project>/tasks` until they're migrated, and both layouts keep working (`layout.rs` is the one
place that says where a layout keeps what).

- **Layout:** `.muster/` in the repo's main checkout: `config.toml` (`local = true|false`, true
  as `init` and `migrate` write it; `prefix = "mu"`, the task keys' prefix (0033), which
  `key::stored_prefix`/`store_prefix` read and write there for a `.muster` queue, and the first
  write pins when it's missing, never in a README (an old queue keeps it in its README's front
  matter); other keys, quoted or not, and other tools' multi-line values are left alone; `init
  --prefix` sets it, and `migrate` writes the old queue's (stored, or the default its keys had),
  so `muster list` shows the same keys before and after), `tasks/NNNN-slug.md`
  (flat, never moved, `stage:` the only truth, no `status:` written or read), `backlog/` and
  `backlog/dropped/`, `attachments/NNNN/`, `mail/` (asks, mailboxes, the lock), `INDEX.md`,
  `tasks.json`, `README.md`. Every write goes through the same lock (`mail/.lock`) and refreshes
  `INDEX.md`. `muster check` also reports a `config.toml` it can't read and a local queue whose
  `.gitignore` would let it into git.
- **Local mode (the default):** `.muster/.gitignore` ignores everything but itself and
  `config.toml`. A queue is found from the repo's main checkout or any worktree (`git rev-parse
  --git-common-dir`, which must be a `.git` folder), so every session uses the main checkout's one
  copy and its one lock: two worktrees claiming one task at once, one wins (tested 10 rounds with
  real `git worktree add`). A repo without a main checkout (a bare repo's worktree, a submodule,
  `--separate-git-dir`) has no queue, and `init` refuses it; the walk up to a `.muster` folder is
  only for folders in no repo at all, so a worktree git can't read never falls back to its own
  copy. Committed mode (`--committed`) has the same single copy for now; its `.gitignore`
  leaves out `/mail/`, `/INDEX.md` and `/tasks.json`, and a merge's "is main clean" check passes
  over `.muster/`.
- **Finding it:** `queue_of` in `cli.rs` asks `layout::find` first, then the old rules; the
  `MUSTER_ROOT`-vs-current-folder check (0002) uses the same, so a `.muster/` from any of its
  worktrees is "here". The app discovers `~/playground/*/.muster` and `~/playground/*/*/.muster`
  in main checkouts only (a worktree beside main has a `.git` file and is skipped), within
  `queue::discover`'s two levels and skips. Named by the project folder for a `main`/`master`
  checkout, else by the checkout (`chit`, `harness40` in the group `harness`). `muster start`
  puts its worktrees at `<project>/worktrees/<repo>/…`, or `<checkout>.worktrees/…` for a repo
  that is its own project, never in `~/playground` itself.
- **`muster init [<folder>] [--local|--committed]`:** the current folder when none is named. In
  a repo, its `.muster/` (in the main checkout, never over one that's there), local unless
  `--committed`; outside a repo, the old `<folder>/tasks` for a folder named on purpose, and a
  mode is refused. With no folder outside any repo it writes nothing and exits 1 ("isn't in a git
  repo: run muster init in the repo, or name a folder for an old tasks/ queue"). A repo without a
  main checkout (a submodule, a bare repo's worktree) is refused as `init` refuses it, never given
  `tasks/`. The app's New queue makes the same through the same function (`projects::new_queue`,
  task 0004), local, and adds it to the list when discovery doesn't find it.
- **Other projects (task 0004):** a repo needs nothing but `muster init`: `muster` finds the
  queue from any folder of the repo. What a session there needs comes from the personal layer,
  which links into every repo: Muster keeps the rule (`rules/muster-queue.md`: `.muster/` is
  the project's queue, read its README, change it only through `muster`, the hook's mail is
  information) and the hook (`bin/answer-hook.py`). The layer's side (a link to the rule, a
  wrapper that runs the hook and does nothing when Muster's checkout isn't there, the hook on
  PostToolUse, UserPromptSubmit and SessionStart in the linker, and the main checkout's
  `.muster/` in each linked worktree's sandbox write list) is in the layer since 2026-10-14
  (layer commit `15b3399`); one live session in a new project is still to be checked (mu-4 AC2).
  `INDEX.md` and `tasks.json` need no pre-commit hook or merge driver: a committed
  queue's `.gitignore` keeps them out of git, and a local queue's keeps out everything but
  `config.toml`, so no branch ever carries them.
- **`muster migrate <old-queue> <repo> [--local|--committed] [--dry-run]`:** copies (never moves)
  every task file with `stage:` set from its old stage or status and `status:` dropped, the rest
  byte for byte (only when its front matter can't be read as it is: a byte-order mark taken off
  and the front matter's CRLF line ends made LF, and said so; a task the old status alone said was
  blocked keeps a `waiting_on` saying so), the backlog folder (notes, dropped notes, anything else
  in it), `attachments/`, `mail/` (not its lock) and `WAITING-FOR-OWNER.md`; puts the old README's
  front matter at the top of the new README; writes `config.toml`, `.gitignore`, `INDEX.md`,
  `tasks.json` (by `manifest.py`) and `migrated-from.txt` (the old queue's folder, the time, its
  last commit, read with `git --no-optional-locks` so the old index is never rewritten); carries
  the old queue's gate, run command, name, hidden and Your check limit in `projects.json` to the new one
  (saved through `projects::edit`, under the file's lock), and adds the new one
  to the list when the old one was listed but discovery can't find the new (three levels down,
  outside `~/playground`). It holds the old
  queue's lock while it reads, builds the new queue in a folder beside it with every file made
  new (never over another) and renames it into place, so a failure leaves nothing. Refused while
  an agent holds a task in Building or Testing (listed; a Testing task from before `held-by` by
  its claim), when `.muster/` is there, when the repo and the old queue overlap, on a folder it
  can't read, on task files without front matter it can read (listed), and when two files would
  be one: the same path, or names that differ only in case (checked in the plan) or accents
  (checked by making the plan's names in a scratch folder of its own in the temp folder, cleared
  before and after, so the dry run says so too). Prints what it copied, notes (a stage that wasn't one, a file made plain) and what it
  didn't copy (symlinks, `bin/`, stray files). `--dry-run` prints the same plan and writes nothing
  in the repo or the old queue.
- **The tools:** `manifest.py` reads `.muster/` (`MUSTER_ROOT=<repo>/.muster`); `bin/task` runs
  muster, which finds the queue from the folder; the answer hook also reads the session
  folder's repo's `.muster/mail/` (no queue where git has no main checkout, as `layout.rs`), each
  queue's mail on its own, so one queue's problem never loses another's; the helper's brief says
  where the queue is.

The owner decided on 2026-10-12 to move each queue into the repo as `.muster/`, either committed
or local (local is what the tools write until the live state moves to `.git/muster/`). With that
came:
- A backlog of raw one-liners, which agents shape into tasks.
- Stages with roles: Backlog, Shaping, Ready, Building, Testing, Your check, Done. Blocked is a
  flag on a stage, not a stage of its own.
- Goals, features and tasks.
- Points: estimates and actuals, in minutes and tokens.
- Links, comments, and a generated `INDEX.md`.

Live claims in the repo's shared git folder (`.git/muster/`), for committed queues whose task
files have one copy per branch, are the next step of 0005. See
`docs/decisions/0003-muster-in-the-repo-backlog-and-stages.md`.

### What's true now: `docs/now/` (task 0102)

The rules in force per area live in short pages, one per area, in `docs/now/`; this document keeps
how things were built. The rules of the queue are in [queue](now/queue.md), claims, the lease and
the takeover in [claims](now/claims.md), sessions, handoffs, check-ins and mail in
[handoffs](now/handoffs.md), worktrees and the merge in [parallel](now/parallel.md), Testing and the
review in [testing](now/testing.md), shaping and the vision check in [shaping](now/shaping.md),
the board and the task pane in [board](now/board.md), the row of folded columns above the board
and its edge cue in [column row](now/column-row.md), the sidebar and Settings in
[sidebar](now/sidebar.md), the Sessions view and its recap in [sessions](now/sessions.md), the
Table, Timeline and Graph views in [views](now/views.md), and the account meter in
[accounts](now/accounts.md). The format and the pass rule are in PROTOCOL.md ("What's true now").

- **Reading** (`now.rs`): only plain `*.md` files directly in `docs/now/` (100 at most), each read
  up to 16 KB, control characters out. A symlinked page is passed over, and so is a `docs/now/`
  that leads out of its repo; `check` warns about both. `covers:` is read from the front matter
  with `queue::parse_touches`, and a page is a task's when `parallel::touches_overlap` says its
  `covers:` and the task's `touches:` overlap. The pages come from `backlog::project_places`, the
  folders the vision brief reads its picture from.
- **The building pass** (`ops::pass_task`, `now::pass_check`): before the queue's lock, for a task
  leaving Building, the pages and the diff come from its worktree while it exists (`HEAD`), else
  from the project's checkout against `refs/heads/<branch>`. One `git merge-base` and one
  `git diff --name-only -z --relative --no-renames <base> <tip> -- docs/now/`, as argument
  arrays; `-z` keeps git from quoting a non-ASCII name, so a page named in Persian matches the
  name `read` gives it. A page unchanged there needs `--now-unchanged` (cleaned to one line, 500
  characters), and the log line (`now pages updated: …` / `now pages unchanged: … (<why>)`) goes
  in with the stage change, under the lock. Without a branch, or when git fails, the pass says it
  can't see the diff and asks for `--now-unchanged`. The app's Pass sends no reason, so a covered
  task leaves Building from the command line or by the owner's move on the track (the rule
  doesn't apply to a track move); the owner's refusal says so.
- **The briefs:** `muster vision` lists the covered pages (`now` in the JSON: path, covers, text)
  and quotes their text line by line; a task without `touches:` gets every page's path, no text.
  `muster vision --all` lists every page's path once. `muster shape` lists the pages its matched
  open tasks cover, each with those tasks' ids, names only.
- **`muster check`:** a page over 60 lines or 4096 bytes, without `covers:`, with the same
  `covers:` as another (in any order), or a symlink is a warning line; the exit code ignores them.
- **Checked by:** unit tests in `now.rs` (parsing, matching, reading, warnings, the branch diff, the
  pass in a temp repo with worktrees: refused, page changed, page added, `--now-unchanged`, no
  branch, no `touches:`, no covering page, no `docs/now/`), `vision.rs` and `backlog.rs` (the
  briefs, a recorded conflict quoting a page), and `tests/now_pages.rs` (the built binary's `check`,
  `vision` and `pass`, and Muster's own pages: no warning, each rule cited, every `covers:` naming
  files that exist).
