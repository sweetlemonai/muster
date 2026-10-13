# Muster: commands

**Name:** Muster (owner, 2026-10-12). It replaces the working names "tasky" and "the task queue".
chit (`~/playground/chit`) is the earlier CLI and spec it grows from. A single task can still be
called a ticket.

**Slash commands:** `/muster`, with `/mu` as the short form. Both are one router, like `/pr` in
`~/playground/.claude/commands/pr.md`. The first word picks the route, a task id (four digits)
shows that task, and anything else is captured as a backlog one-liner, never guessed as a command.
A route matches only its shape: `/mu next week we try dark mode` is captured, not a claim. Each
route runs one `muster` command, which holds the rules.

The router is `commands/muster.md`. `commands/mu.md` is an identical copy, because Claude Code has
no alias for a command: a one-line pointer would cost an extra read on every call and would need
the router's absolute path, which differs between this repo and the personal layer.
`bin/test_commands.py` keeps the two byte for byte equal and checks this page against the router
and against `muster help --json`.

**What it may run:** its `allowed-tools` name one prefix per routed command (`Bash(muster list:*)`,
`Bash(muster claim:*)`, …), plus Read, Glob and Grep, rather than `Bash(muster:*)`. A slash
command runs those without asking, so a mis-parsed line or text in a task file can't make it
`attach` a file from anywhere on the Mac, `init` a queue, or `set`, `move` or `stage` a task. Those
stay the owner's, with `! muster …`.

Options go right after the id (for `answer`, right after `Q<n>`), or right after the route word
when there's no id. Everything after them is text.

**Installed:** not yet. They go in the personal layer (`~/playground/.claude/commands/`), so every
repo gets them, once the owner says yes (asked in `WAITING-FOR-OWNER.md`).

Not built yet: anything marked (task NNNN). Until then the route says so and does nothing.

## For the owner

Answers in plain words: task titles with their ids, and for anything that needs a decision, what
it is, why it needs you, the options, the pick, and what happens with no answer.

| Command | What it does | Runs |
|---|---|---|
| `/mu` | What's waiting for you (questions escalated to you, tasks in Your check, tasks blocked on you), then the queue in one line | `muster list`, then `muster show <id>` for each task marked [needs you] |
| `/mu <id>` | One task in plain words: what it is, its stage, who's on it, open questions, criteria left, the last thing that happened | `muster show <id>` |
| `/mu waiting` | Only what's waiting for you | `muster list`, then `muster show <id>` for each task marked [needs you] |
| `/mu add <text>` | Capture a one-liner in the backlog, for shaping into a task | `muster add -- '<text>'` |
| `/mu answer <id> Q<n> <text>` | Answer a question, recorded as yours. It reaches the agent that asked | `muster answer <id> Q<n> --as owner -- '<text>'` |
| `/mu <anything else>` | Captured as a backlog one-liner, and it says so | `muster add -- '<all of it>'` |

## For agents doing the work

Prints the command it ran and muster's output, verbatim. `--as <your branch>` and the other
options pass through.

| Command | What it does | Runs |
|---|---|---|
| `/mu ask <id> <question>` | Ask without stopping. The answer is pushed back (answer hook) | `muster ask <id> -- '<question>'` |
| `/mu claim <id>` | Claim a Ready task | `muster claim <id>` |
| `/mu next` | Claim the top Ready task this agent may take | `muster next` |
| `/mu heartbeat <id>` | Keep a claim alive on long work | `muster heartbeat <id>` |
| `/mu note <id> <text>` | Add a line to the task's log | `muster note <id> -- '<text>'` |
| `/mu pass <id> …` | Hand on to the next stage (from, gate line, commit as needed) | `muster pass <id> …` |
| `/mu return <id> <why>` | Send back a stage, with the reason | `muster return <id> -- '<why>'` |
| `/mu block <id> <on what>` | Mark it blocked on a task id, owner or capture; it keeps its stage | `muster block <id> --on '<on what>'` |
| `/mu unblock <id>` | Clear the blocked flag | `muster unblock <id>` |

## For running the system

Prints the command and its output, verbatim.

| Command | What it does | Runs |
|---|---|---|
| `/mu list [--stage <s>]` | Tasks by stage, or one stage | `muster list [--stage <s>]` |
| `/mu check` | Problems in the queue: ids used twice, stale claims, broken waits | `muster check` |
| `/mu help [<command>]` | These routes, then muster's own help | `muster help [<command>]` |

## The `muster` command

`muster help` lists every command, the same list as the app's Help tab. The ones /mu doesn't
route run as `! muster …` in a session: new, move, stage, set, attach, escalate, inbox, reclaim,
projects, init, tick, untick (the owner's criteria, task 0028), edit (task 0020), and the backlog's
shape, file and drop (task 0015). Also coming: `depends:` (task 0019).

`bin/task ask|answer|escalate|inbox` is a stand-in that runs the same muster commands, kept for
scripts and agents that call it. `bin/answer-hook.py` delivers answers into a running session, and
`bin/manifest.py` writes the board data (`--check` checks it).
