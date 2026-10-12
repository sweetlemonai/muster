# Muster

A file-based task system for coding agents. One session (capture) writes the owner's ideas and
bugs up as tasks. Other sessions pick them up, ask questions without stopping, and get the
answers pushed into their session. The owner sees only what really needs them.

It grows out of chit (`~/playground/chit`) and its design (`~/playground/izzy/docs/agent-task-system.md`),
dogfooded on Drover since 2026-10-11. The plan is for Muster to become part of Drover. Until
then it's its own repo, tools and (soon) app.

- **Protocol** (what every session follows): [`docs/PROTOCOL.md`](docs/PROTOCOL.md)
- **As built** (what exists today, and what's left): [`docs/AS_BUILT.md`](docs/AS_BUILT.md)
- **Commands** (planned `/muster`, `/mu`): [`docs/COMMANDS.md`](docs/COMMANDS.md)
- **Adopted from chit, and next:** [`docs/ADOPTION.md`](docs/ADOPTION.md)
- **Dogfood notes** (what went wrong, and what it taught): [`docs/DOGFOOD.md`](docs/DOGFOOD.md)

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
| Drover | `/Users/imassih/playground/drover/tasks` (own git; `bin/` there holds stand-ins that forward here) |
