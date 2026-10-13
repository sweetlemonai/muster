# Muster

A file-based task system for coding agents. One session (capture) writes the owner's ideas and
bugs up as tasks. Other sessions pick them up, ask questions without stopping, and get the
answers pushed into their session. The owner sees only what really needs them.

It grows out of chit (`~/playground/chit`) and its design (`~/playground/izzy/docs/agent-task-system.md`),
dogfooded on Drover since 2026-10-11. The plan is for Muster to become part of Drover. Until
then it's its own repo, tools and (soon) app.

- **The one picture** (what Muster is, how a task flows, which task builds what; every task points here): [`docs/VISION.md`](docs/VISION.md)
- **Protocol** (what every session follows): [`docs/PROTOCOL.md`](docs/PROTOCOL.md)
- **As built** (what exists today, and what's left): [`docs/AS_BUILT.md`](docs/AS_BUILT.md)
- **Commands** (planned `/muster`, `/mu`): [`docs/COMMANDS.md`](docs/COMMANDS.md)
- **Adopted from chit, and next:** [`docs/ADOPTION.md`](docs/ADOPTION.md)
- **Design rules** (every screen of the app; read before any UI work): [`docs/DESIGN.md`](docs/DESIGN.md)
- **Dogfood notes** (what went wrong, and what it taught): [`docs/DOGFOOD.md`](docs/DOGFOOD.md)

## The app

`app/`: a desktop app to watch every queue live: board, what's waiting for you (answer
in place), task pages with their files, activity, problems. Build with `cd app && pnpm install &&
pnpm release:build`, then open `app/src-tauri/target/release/bundle/macos/Muster.app`. Decision:
[`docs/decisions/0001-app-on-drovers-stack.md`](docs/decisions/0001-app-on-drovers-stack.md).

## Layout

`~/playground/muster/main` is this repo. `~/playground/muster/tasks` is Muster's own queue.

## Command line

`bin/muster`: the same rules as the app, from a terminal or for agents (`muster help`). See
[`docs/AS_BUILT.md`](docs/AS_BUILT.md), "The command line".

## Tools (`bin/`)

Python 3 standard library only. No dependencies. Every tool reads the queue folder from
`MUSTER_ROOT`.

| Tool | Does |
|---|---|
| `task` | `ask`, `answer`, `escalate`, `inbox`: questions in task files, answers to the asker's mailbox |
| `answer-hook.py` | Claude Code hook (PostToolUse, UserPromptSubmit, SessionStart): hands waiting answers to the session that asked |
| `manifest.py` | Builds `tasks.json` (every task, counts, what's waiting for the owner) and `--check` finds queue problems |
| `test_task.py` | Tests: `python3 -I bin/test_task.py` |

## Queues using it

| Project | Queue folder |
|---|---|
| Drover | `/Users/imassih/playground/drover/tasks` |
| Muster itself | `/Users/imassih/playground/muster/tasks` |

Each queue sits next to its project's repo (`<project>/tasks`) with its own git history. Its
`bin/` holds stand-ins that run these tools with `MUSTER_ROOT` set to that queue. Muster's own
work is in `muster/tasks`: the desktop app (0001) is next.
