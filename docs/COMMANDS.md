# Muster: commands

**Name:** Muster (owner, 2026-10-12). It replaces the working names "tasky" and "the task queue".
chit (`~/playground/chit`) is the earlier CLI and spec it grows from. A single task can still be
called a ticket.

**Slash commands:** `/muster`, with `/mu` as the short form. Both are one router, like `/pr` in
`~/playground/.claude/commands/pr.md`. The first word picks the route, a task (its key, `mu-8`,
or its number, `8` or `0008`) shows that task, and anything else is captured as a backlog
one-liner, never guessed as a command.
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

**Installed:** in the personal layer since 2026-10-13: `~/playground/.claude/commands/mu.md` and
`muster.md` are links to this repo's `commands/` (layer commit `663a4c7`), so every repo gets them.

Not built yet: anything marked (task NNNN). Until then the route says so and does nothing.

## For the owner

Answers in plain words: task titles with their ids, and for anything that needs a decision, what
it is, why it needs you, the options, the pick, and what happens with no answer.

| Command | What it does | Runs |
|---|---|---|
| `/mu` | What's waiting for you (questions escalated to you, tasks in Your check, tasks blocked on you or by a permission), then the queue in one line | `muster list`, then `muster show <id>` for each task marked [needs you] |
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
| `/mu block <id> <on what>` | Mark it blocked, and why: tasks of this project (mu-14,mu-16 or 0014,0016, added to its depends), you, permission, question Q2 or outside, then what it waits on (owner and capture still work); it keeps its stage | `muster block <id> --on '<on what>'` |
| `/mu unblock <id>` | Clear the blocked flag (a block on tasks clears by itself when they're done) | `muster unblock <id>` |

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
checkin (task 0030), ping (task 0039), projects, init, tick, untick (the owner's criteria, task 0028), edit (task
0020), the backlog's shape, file and drop (task 0015), link and comment (task 0017: a task's
external links, and a comment thread anyone may add to), index (task 0018), vision (a task
checked against the project's picture, task 0029), and ready, start and release (parallel
sessions, task 0023).

Parallel sessions (task 0023): `ready [--parallel [<n>]] [--json]` lists the tasks a session could
start now, or up to n (default 3) safe side by side: none depends on another, and their `touches:`
(folders and globs, `new`/`file --touches`, `set <id> touches`) don't overlap; a task without
touches never pairs. `start <id> [--at <path>|--here]` claims one with a branch `task/<id>-<slug>`
from the local main and a worktree at `<project>/worktrees/<repo>/<id>-<slug>`, both recorded in the
task. `pass <id> --from testing` of a started task is the merge: one at a time per repo, rebase on
the local main, the project's gate in the worktree (`projects set <name> gate "<command>"`, run as
words without a shell; this repo's is `bin/gate`; again if main moved while it ran, twice at
most), a fast-forward of the local main when its checkout is clean and on main (never a merge
commit, so main only gets a tree the gate saw), a plain push of main to origin, then the worktree
(never a runner's, `--here`) and branch removed, Your check with the merge commit and a report,
and the next task to start. A conflict or a red gate returns it to Building; nothing is deleted on
any failure before main has the merge, and nothing is ever force-pushed. `ready --parallel` keeps
clear of the tasks already in Building or Testing too.
`release <id> --clean [--force]` gives one up, listing what would be lost first. `/mu pass` runs
`muster pass` as typed, so it merges too.

Who's on it (task 0030): `checkin <id> "<what's done, what's next>" [--pct <0-100>]` is the holder
saying how far along it is, which renews the claim too. `ping <id> ["<message>"]` (task 0039) asks
the holder's session for a check-in, or sends it a message, through its mailbox. Inside a session every command stamps the
session next to `--as`; with no `--as` the actor is `$MUSTER_AS`, then the folder's git user.name,
never the login name.

Old pauses (task 0059): before `pause` (task 0045), a pause was a block whose reason said so
(`waiting_on: owner (paused)`). `check` warns about each one, and `check --fix-pauses` makes them
pauses in every queue Muster knows (`--queue` for one): it lists each (key, title, stage, since
when, the reason it keeps) before it changes anything, then gives each `paused:` the time of the
line that blocked it (else the latest line about the pause, else the last line) and `paused-why:`
what the reason says besides "owner" and "paused", and clears the block as `unblock` does, the
stage kept. `--dry-run` lists them and changes nothing. No session is told, a block that says
anything else is never touched, and running it again changes nothing.

The master list (task 0018): `INDEX.md` in the queue folder lists every goal with its features and
tasks, then the tasks under none by stage, the backlog notes, and what's done (collapsed), each
with its stage, the tasks it still waits on (`depends:` not merged yet, as "waits on 0014"),
its open vision conflicts (as "⚠ 2 vision", task 0029), priority, type, estimate (the text `muster list` shows: agent time, test time, tokens), how many
comments it has, and a link to its file. muster rewrites it after every
change it makes, under the queue's lock, and only when its text changed. It's generated, so never
edit it by hand. After a hand edit to a task file, `muster index` rebuilds it, and `muster index
--check` exits 1 when it isn't exactly what muster would write. `muster check` fails on a list
that shows something the files no longer say, and only warns ("INDEX.md: refresh due") when just
its "As of" and sources lines are behind, as after Drover writes an answer outside muster.
`manifest.py --check` reads only the sources line at the end, so a mismatch there is a warning;
a list without one is a problem. A missing list is fine, and `manifest.py` never writes it.

Levels (task 0016): `new --level goal|feature --parent <id> --due YYYY-MM-DD` and `set <id>
level|parent|due <value>` (an empty parent or due clears it), with the rules checked: a task's
parent is a feature or a goal, a feature's a goal, a goal has none, no loops, and only a goal has a
due date. `claim`, `next` and `pass` never take a goal or a feature into Building. `check` reports a
missing or wrong-level parent and a loop, and warns (without failing) about a goal past its due date
with tasks open.

Human tasks (task 0022): `new "<title>" --human` (the owner only: `--as owner`) makes the owner's
own task, `for: human`, and `set <id> for human|ai` switches who does one while nobody is on it.
`claim` refuses a human task for anyone but the owner and `next` passes it by; `pass` takes it To
do (Ready) → Doing (Building) → Done, with no gate line. An agent proposes one with `add`. `list`
marks human tasks `[you]` and INDEX.md says "you"; bare `/mu` and `/mu waiting` list the ones not
done after what waits on the owner, with the `/mu pass <id> --from ready --as owner` that starts
one.

`bin/task ask|answer|escalate|inbox` is a stand-in that runs the same muster commands, kept for
scripts and agents that call it. `bin/answer-hook.py` delivers answers into a running session, and
`bin/manifest.py` writes the board data (`--check` checks it, and reads `level`, `parent`, `due`
and `for` into `tasks.json` and checks them as `muster check` does, warnings in their own list).
