# Commands

Every `muster` command, grouped by the job it does. This page is made from the command line's own help, so it always matches the `muster` you have: in a terminal, `muster help` lists the commands and `muster help <command>` shows one. In the app, click a command line to copy it.

## Tasks

Make a task, read it, and change what it says: its fields, its write-up, its log, its comments, its links and its files.

### muster new

Create a task in Ready. Its id is taken in order under the queue's lock, and it's logged as you. With --level it's a goal (an outcome, may have a due date) or a feature (groups tasks); agents build only tasks. With --human it's the owner's own to do (for: human): only the owner makes one, and agents never take it; an agent that finds something only the owner can do proposes it with muster add. An idea that isn't a task yet goes in the backlog instead (muster add).

```
muster new "<title>" [--type code|bug|chore|tech-debt|spike|design|docs|research] [--priority high|normal|low] [--tags a,b] [--time 2h] [--test-time 30m] [--tokens 400k] [--touches <folders,globs>] [--level goal|feature] [--parent <id>] [--due YYYY-MM-DD] [--human] [--body <text>|-]
```

| Option | What it does |
| --- | --- |
| `--type` | code (default), bug, chore, tech-debt, spike, design, docs or research; task still means code |
| `--priority` | high, normal (default) or low |
| `--tags` | comma-separated: spike, chore, tech-debt, code, docs, design, research, test, perf, security, or your own |
| `--time` | estimated agent time: 45m, 2h, 1.5d |
| `--test-time` | estimated test time, the QA agent plus your check: 30m, 1h |
| `--tokens` | estimated tokens: 150k, 1.2M |
| `--touches` | where in the repo it works, folders and globs, comma-separated (app/src/TaskPanel\*, docs/), for muster ready --parallel |
| `--level` | goal, feature or task (default) |
| `--parent` | the id it sits under: a task's is a feature or a goal, a feature's a goal; a goal has none |
| `--due` | a goal's date, YYYY-MM-DD |
| `--human` | the owner's own task, To do → Doing → Done, hatched on the board with the You ribbon (only the owner, --as owner) |
| `--body` | the description in Markdown; \\n is a line break; - reads it from stdin. It can't have a "## Comments" line (comments are the tool's) |

Examples:

```
muster new "Board flickers on resize" --type bug --priority high --tags code --time 1h --test-time 20m --tokens 300k
```

```
muster new "Buy the domain" --human --as owner
```

```
muster new "Export as CSV" --type code --body "What: …\n\nDone looks like: …"
```

```
pbpaste | muster new "From the clipboard" --body -
```

```
muster new "Ship 1.0" --level goal --due 2026-12-01
```

```
muster new "Stage track" --level feature --parent 0030
```

```
muster new "Pass button" --parent 0031 --time 2h
```

### muster list

Tasks grouped by stage (Backlog, Shaping, Ready, Building, Testing, Your check, Done), with type, priority, \[you\] on the owner's own tasks (for: human), what a blocked task waits on, estimate, tags, "needs you" and open questions. Backlog notes come first, under Backlog notes, by file name and title (not under a filter only tasks have). Search, filters and sort follow the app's rules, so the same query lists the same tasks. Several values in one option (ready,building) mean any of them; options together mean all of them.

```
muster list [--search "<words>"] [--stage <s,…>] [--type <t,…>] [--tag <t,…>] [--holder <who,…>] [--blocked] [--sort <order>] [--status <s>] [--json]
```

| Option | What it does |
| --- | --- |
| `--search` | words in any order, case and accents aside, each in the id (19 finds 0019), title, tags, text or holder; a word of digits only is an id |
| `--stage` | backlog, shaping, ready, building, testing, your-check or done |
| `--type` | code, bug, chore, tech-debt, spike, design, docs or research (task means code) |
| `--tag` | only tasks with this tag |
| `--holder` | who holds it in its stage, as the board shows it, without the session (drover/main, wt-a, owner) |
| `--blocked` | only blocked tasks, those waiting only on the tasks they depend on too |
| `--sort` | priority (needs you, then high to low; Done last), newest, oldest, changed, estimate or id (the default) |
| `--status` | the old status: inbox, todo, doing, blocked or done |
| `--json` | machine-readable: {"tasks": \[…\], "backlog": \[…\]}, for scripts and agents |

Examples:

```
muster list
```

```
muster list --stage ready --tag spike
```

```
muster list --search "board flicker" --stage ready,building --sort changed
```

```
muster list --queue drover --json
```

### muster show

One task in full: its file as Markdown, or its parsed fields with --json.

```
muster show <id> [--json]
```

| Option | What it does |
| --- | --- |
| `--json` | fields, questions, attachments, log tail |

Examples:

```
muster show 0007
```

```
muster show 0007 --json
```

### muster set

Change one field, logged with the time and who. type is one of code, bug, chore, tech-debt, spike, design, docs, research; time is agent time and test-time the QA agent plus your check. touches is where in the repo it works, folders and globs, comma-separated (app/src/TaskPanel\*, docs/), which muster ready --parallel keeps apart; empty clears it. level, parent and due keep the rules of muster new (a parent that exists at the right level, no loop, a due date only on a goal); a task in Building or Testing, or held, stays a task. An empty parent or due clears it. for is human or ai: who does it (a human task is the owner's own, hatched on the board with the You ribbon); only a task, and not while it's held, in Building, Testing or Your check; made human in Backlog or Shaping, it moves to Ready (To do).

```
muster set <id> <priority|type|title|tags|time|test-time|tokens|touches|level|parent|due|depends|for> <value>
```

Examples:

```
muster set 0007 priority high
```

```
muster set 0041 for human
```

```
muster set 0007 type tech-debt
```

```
muster set 0007 tags spike,tech-debt
```

```
muster set 0007 tags ""
```

```
muster set 0007 time 3h
```

```
muster set 0007 test-time 45m
```

```
muster set 0007 tokens 600k
```

```
muster set 0007 title "A clearer title"
```

```
muster set 0031 level feature
```

```
muster set 0032 parent 0031
```

```
muster set 0031 parent ""
```

```
muster set 0030 due 2026-12-01
```

```
muster set 0018 depends 0014,0016
```

```
muster set 0018 depends ""
```

```
muster set 0031 touches "app/src/TaskPanel*, docs/"
```

### muster edit

Edit a task's write-up: the description, To do and acceptance criteria between the front matter and the Log. Opens $VISUAL or $EDITOR (else vi; quotes work as in a shell, but no shell runs it) on a copy and saves it when the editor exits, logged as an edit. Unchanged text prints "no change" and writes nothing. The front matter and the Log stay the tool's: a "## Log" line or a stage line is refused. Once a task has a vision check, its ## Vision section is the tool's too: left out of the write-up and kept as it is, and a new "## Vision" line is refused. Refused when the write-up changed since you opened it, or when the editor fails; either way, what you typed is kept in the temp file it names. Refused too while someone else holds it in Building: their branch owns the write-up then, so add a note instead.

```
muster edit <id> [--body -]
```

| Option | What it does |
| --- | --- |
| `--body -` | read the new write-up from stdin instead of opening an editor (for scripts) |
| `--as` | who's editing; the holder of a task in Building may edit it |

Examples:

```
muster edit 0007
```

```
EDITOR="code --wait" muster edit 0007
```

```
VISUAL='"/Applications/Sublime Text.app/Contents/SharedSupport/bin/subl" -w' muster edit 0007
```

```
pbpaste | muster edit 0007 --body -
```

### muster note

Add a line to the task's log (the tool stamps the time).

```
muster note <id> "<text>"
```

Examples:

```
muster note 0007 "Checked on the Mac, works"
```

### muster comment

Add a comment to a task's thread: under ## Comments, above the Log, stamped with who and when, and logged as commented. Anyone may comment on any task at any stage, one someone else holds included: comments don't touch the write-up, and edit leaves them alone. The holder's own comment keeps their claim alive, like a note. Lines are kept (each after the first indented in the file); control characters go, at most 4,000 characters, and a line that would read as a stage change is refused.

```
muster comment <id> "<text>" | --body <text>|-
```

| Option | What it does |
| --- | --- |
| `--body` | the comment instead of the words after the id; \\n is a line break; - reads it from stdin |
| `--as` | who's commenting; default $MUSTER\_AS, then $USER |

Examples:

```
muster comment 0007 "Tried it on the Mac: the board still flickers once"
```

```
pbpaste | muster comment 0007 --body -
```

### muster link

Add an external link to a task (a spec, a PR, a design), or remove one by its number (from 1) or address. Links are https only, up to 2,000 characters, printable ASCII (percent-encode the rest), without a user name or password or a percent-encoded host; a title has no \<, \> or ;. At most 20 per task. They're in the front matter as links: title \<https://…\>; \<https://…\>, the task pane lists them, and a click opens one in your browser. Logged.

```
muster link <id> <https-url> ["<title>"] | muster link <id> --remove <n|url>
```

| Option | What it does |
| --- | --- |
| `--remove` | the link's number (1 is the first) or its address |

Examples:

```
muster link 0007 https://github.com/sweetlemonai/muster/pull/12 "The PR"
```

```
muster link 0007 --remove 1
```

### muster attach

Copy files into the task's attachments/\<id\>/ (25 MB each; never overwrites, a clash gets -2).

```
muster attach <id> <file>...
```

Examples:

```
muster attach 0007 ~/Desktop/shot.png
```

```
muster attach 0007 *.png
```

## Stages

Move a task on to the next stage, send it back, or mark it blocked. Only these commands change a stage: never move or edit a task file for that. See [Stages and roles](02-concepts.md#stages-and-roles).

### muster pass

Hand a task to the next stage: Backlog → Shaping → Ready → Building → Testing → Your check → Done. Ready → Building claims it for you (--as). Shaping → Ready needs acceptance criteria and no open vision conflict (muster vision). Building → Testing needs the gate's result line and releases your claim. Testing → Your check needs every criterion ticked except the owner's ("— verify: owner"); Your check → Done needs those ticked too (muster tick). A human task (the owner's own) goes To do (Ready) → Doing (Building) → Done, and only the owner passes it: no gate line, no Testing, no Your check. Refused while it's blocked, when someone else holds it, and into Building (Ready → Building, or the owner's Start) while a task it depends on isn't merged yet, as claim is. A task made with muster start merges when it passes from Testing, in order, stopping at the first failure: one merge at a time per repo; rebase its branch on the local main (a conflict sends it back to Building naming the files); run the project's gate in its worktree (muster projects set \<name\> gate; red sends it back with the gate line), and again if main moved while it ran (twice at most, then it stops: main keeps moving); fast-forward the local main to it (never a merge commit, so main only gets a tree the gate saw), only if the main checkout is clean and on main; push main to origin (never forced: if origin moved, it fetches and stops); remove the worktree (never a runner's, made with start --here) and delete the branch, and origin's copy only when it's the same commit; record the merge (the commit and a report: gate line, time in Building and Testing, check-ins) and pass it to Your check: the record is written even if the task was blocked or moved meanwhile, and then it says so (use muster stage); then it names the next task to start. Nothing is deleted on any failure before main has the merge. Only the command line does this: the app's Pass is refused for such a task.

```
muster pass <id> [--from <stage>] [--gate "<gate result line>"] [--commit <sha>]
```

| Option | What it does |
| --- | --- |
| `--from` | the stage you saw it in; refused if it has moved on since (use it, so a repeat never moves it twice) |
| `--gate` | Building → Testing: the gate's result line, verbatim |
| `--commit` | Your check → Done: the commit that ships it |
| `--as` | who you are; must match who claimed it to pass it out of Building |

Examples:

```
muster pass 0007 --as wt-a --from ready
```

```
muster pass 0007 --as wt-a --from building --gate "gate full: green (9 steps)"
```

```
muster pass 0007 --from your-check --commit 3f2a9c1
```

```
muster pass 0041 --from ready --as owner
```

```
muster pass 0031 --from testing --as qa
```

### muster return

Send a task back one stage, or to an earlier stage with --to, with the reason (one line, 500 characters). Back to Building gives it to whoever last claimed it there. Sent back from Your check (merged), the tasks that depend on it and haven't started show blocked by it again; ones already under way keep going, with a note on their log ("0002 went back to Building: \<why\>").

```
muster return <id> "<why>" [--to <stage>]
```

| Option | What it does |
| --- | --- |
| `--to` | an earlier stage: backlog, shaping, ready, building, testing or your-check |

Examples:

```
muster return 0007 "AC2 fails: the board still flickers"
```

```
muster return 0007 "the spec misses the empty state" --to shaping
```

### muster block

Mark a task blocked, and say why: the card, the table, the timeline and the task pane say it in words, the kind first ("Blocked on you: …"). It stays in its stage and keeps its claim; blocked is a flag, not a stage. Task ids go into its depends instead of a flag: it shows blocked by them until they're all merged (in Your check or Done; the owner's own task, Done), and clears by itself then (logged). A block on a question clears when it's answered. You, permission and outside stay until muster unblock. Blocked on you or by a permission counts as waiting for you. Refused: an unknown kind, no detail, a question that isn't open in the task.

```
muster block <id> --on <0014,0016|you|permission|question|outside> ["<what it waits on>"]
```

| Option | What it does |
| --- | --- |
| `--on` | the kind, then what it waits on (in --on after the kind, or after it): task ids 0014,0016 (added to depends); you "\<what you have to do or decide\>"; permission "\<what access, and where to grant it\>"; question Q2 (an open question in this task); outside "\<a third party, a service, a release\>". owner … and capture … still work, as you and question |

Examples:

```
muster block 0007 --on 0012,0014
```

```
muster block 0007 --on you "set the API key in the Keychain"
```

```
muster block 0007 --on permission "write to ~/playground/x/tasks (sandbox): grant in the layer's write list"
```

```
muster block 0007 --on question Q2
```

```
muster block 0007 --on outside "Apple review"
```

```
muster block 0007 --on "owner: which colour?"
```

### muster unblock

Clear the blocked flag (on you, by a permission, a question or outside). The task carries on in its stage. A block on tasks has no flag: it clears by itself once they're done.

```
muster unblock <id>
```

Examples:

```
muster unblock 0007
```

### muster stage

Put a task in any stage (the owner's or capture's override), logged as set. Leaving Building releases its claim.

```
muster stage <id> <stage> [--from <stage>]
```

| Option | What it does |
| --- | --- |
| `--from` | the stage you saw it in; refused if it has moved on since (use it, so a repeat never moves it twice) |

Examples:

```
muster stage 0007 ready
```

```
muster stage 0007 your-check --from testing
```

### muster tick

Tick one of the criteria only the owner verifies ("— verify: owner"), in Your check. Agents tick their own in the file, with the proof; these have no proof but the owner's look. Testing passes to Your check without them; Done needs them.

```
muster tick <id> <AC2|2>
```

Examples:

```
muster tick 0026 AC1
```

```
muster tick 0026 2
```

### muster untick

Undo muster tick: untick one of the owner's criteria.

```
muster untick <id> <AC2|2>
```

Examples:

```
muster untick 0026 AC1
```

### muster move

The old way to change a task, kept for scripts: inbox puts it in Backlog, todo in Ready, done in Done (and unblocks it), blocked is muster block. Use pass, return, block and stage instead.

```
muster move <id> <inbox|todo|blocked|done> [--on "<what it waits on>"]
```

| Option | What it does |
| --- | --- |
| `--on` | for blocked: as muster block's --on, the kind and what it waits on in one ("you: pick a colour", "0012") |

Examples:

```
muster move 0007 blocked --on "owner: pick a colour"
```

```
muster move 0007 todo
```

```
muster move 0007 done
```

## Claims

Who's on a task. A coding session claims a Ready task, keeps the claim alive while it works, and checks in to say how far it got. See [Claims, leases and check-ins](02-concepts.md#claims-leases-and-check-ins). Sessions that work side by side each start a task in its own worktree: `muster ready --parallel` says which tasks are safe together, `muster start` claims one with a branch and a worktree, passing it from Testing merges it into main and cleans up, and `muster release --clean` gives it up.

### muster claim

Take a Ready task: it moves to Building, held by you, with its lease started (heartbeat). Inside a Claude Code session muster adds the session to who you are (name, short id, checkout, account), so you never type it. Refused while you already hold another task in Building in this queue (pass, return or block that first), when it's blocked, when a task it depends on isn't merged yet (an agent's task counts once it's in Your check, the owner's own once it's Done), when it isn't in Ready any more, and for a human task (the owner's own) unless you're the owner. Says which queue it claimed in.

```
muster claim <id> [--as <who>]
```

| Option | What it does |
| --- | --- |
| `--as` | who you are (your branch or worktree); default $MUSTER\_AS, then the folder's git user.name |

Examples:

```
muster claim 0007 --as wt-a
```

```
muster claim 0007 --as wt-a --queue drover
```

### muster next

Claim the top Ready task you may take: not blocked, not held by someone else, everything it depends on merged (in Your check or Done; the owner's own task, Done), and never a human task (the owner's own) unless you're the owner; high priority first, then normal, then low, then the lowest id. Prints "nothing ready" and exits 1 when there's none. Refused while you already hold a task in Building.

```
muster next [--as <who>]
```

| Option | What it does |
| --- | --- |
| `--as` | who you are; default $MUSTER\_AS, then the folder's git user.name |

Examples:

```
muster next --as wt-a
```

```
muster next --as wt-a --queue muster
```

### muster heartbeat

Keep your claim alive on long work: stamps the task's heartbeat, no log line. Only the holder, only in Building. Your note, comment, ask, pass, return and block stamp it too, and so does an unblock. A claim quiet past the lease (12 h, unless changed in the app's Settings on this Mac) can be released to Ready.

```
muster heartbeat <id> [--as <who>]
```

| Option | What it does |
| --- | --- |
| `--as` | who you are; must be who holds it |

Examples:

```
muster heartbeat 0007 --as wt-a
```

### muster checkin

Say how the work is going: a line in the task's log that its card shows ("60% · gate green, writing tests · 12 min ago"), and the heartbeat with words, so it keeps your claim alive too. Only the holder, only in Building. Check in at each milestone and at least every 30 minutes: a card quiet for longer says "no check-in for 45 min".

```
muster checkin <id> "<what's done, what's next>" [--pct <0-100>]
```

| Option | What it does |
| --- | --- |
| `--pct` | how far along you think you are, 0 to 100 (your own estimate) |
| `--as` | who you are; must be who holds it |

Examples:

```
muster checkin 0007 "gate green, writing tests" --pct 60 --as wt-a
```

```
muster checkin 0007 "found the cause, fixing it" --as wt-a
```

### muster reclaim

Release every claim whose lease ran out: a task in Building, held, not blocked, with no heartbeat (or, without one, no change to its file) for longer than --after goes back to Ready, its claim cleared and the release logged as a stage change. Live claims are left alone. Prints what it released.

```
muster reclaim [--after 12h] [--json]
```

| Option | What it does |
| --- | --- |
| `--after` | the lease: 30m, 2h, 1d (a day is 24 h); default this Mac's lease from the app's Settings, 12h unless changed |
| `--json` | what was released, for scripts |

Examples:

```
muster reclaim
```

```
muster reclaim --after 1d --queue drover
```

### muster ready

The tasks a session could start now: in Ready, not blocked, nobody's claim, an agent's (never a human task), everything they depend on merged (in Your check or Done); high priority first, then the lowest id. With --parallel, up to n of them (default 3) that are safe side by side, and beside the tasks already in Building or Testing: none depends on another, and what they touch doesn't overlap. A task says where it works in touches: (folders and globs, set at shaping or with muster set \<id\> touches); two entries overlap when the part before the first wildcard of one starts the other's (app/src overlaps app/src/TaskPanel\*), and a task without touches never pairs. Then the ones left out, with why.

```
muster ready [--parallel [<n>]] [--json]
```

| Option | What it does |
| --- | --- |
| `--parallel` | only a set that can be built at the same time; how many goes after it (1 to 20, default 3) |
| `--json` | {"tasks": \[…\], "left": \[{"id", "why"}\]}, for a dispatcher |

Examples:

```
muster ready
```

```
muster ready --parallel
```

```
muster ready --parallel 2 --json
```

### muster start

Claim a Ready task (claim's rules) in its own git worktree, to work beside other sessions: a branch task/\<id\>-\<slug\> from the local main's commit (however dirty the main checkout is) and a worktree for it, at \<project\>/worktrees/\<repo\>/\<id\>-\<slug\> unless --at says where. Run it in the project's repo or a worktree of it. The task records worktree: and branch:, and start prints both with the brief: the task file, what it touches, its criteria. Work there; pass from Building as always. Passing it from Testing merges it (muster pass). A task started before (sent back to Ready, reclaimed) is picked up in its worktree again. Refused claims leave nothing behind.

```
muster start <id> [--at <path>|--here]
```

| Option | What it does |
| --- | --- |
| `--at` | where to make the worktree instead (a path that doesn't exist yet) |
| `--here` | use the worktree you're in (one a runner like a Drover lane made, with nothing uncommitted): it's switched to the task's branch, and it stays the runner's: muster never removes it |
| `--as` | who you are; default $MUSTER\_AS, then the folder's git user.name |

Examples:

```
muster start 0031 --as wt-a
```

```
muster start 0031 --here --as lane-2
```

```
muster start 0031 --at ~/wt/0031 --as wt-a
```

### muster release

Give up a started task: back to Ready with its claim released, its worktree removed and its branch deleted (the local one; a branch pushed to origin stays, and so does its work). It first lists what that would lose (uncommitted files in the worktree, commits on no other branch) and refuses it unless --force. Works when the worktree is already gone too. A runner's worktree (start --here) is never removed: it's left detached with its files, and only the branch goes. In Building only its holder or the owner releases it.

```
muster release <id> --clean [--force]
```

| Option | What it does |
| --- | --- |
| `--clean` | remove the worktree and the branch (required: to send a task back and keep them, use muster return) |
| `--force` | throw away the uncommitted files and unpushed commits it listed |

Examples:

```
muster release 0031 --clean
```

```
muster release 0031 --clean --force --as owner
```

## Questions

Ask in a task without stopping. The answer is written into the task and pushed back to the session that asked. See [Questions and answers](02-concepts.md#questions-and-answers).

### muster ask

Ask a question in a task without stopping: numbered, written under ## Questions, and your session recorded so the answer is pushed to you (the answer hook) or waits in muster inbox. Lists the task's earlier questions, and refuses one that's already answered there.

```
muster ask <id> "<question>" [--as <who>] [--session <id>]
```

| Option | What it does |
| --- | --- |
| `--as` | who's asking (your branch); default $MUSTER\_AS, then the folder's git user.name |
| `--session` | your session id; default $CLAUDE\_CODE\_SESSION\_ID |

Examples:

```
muster ask 0007 "Child webview or a separate window?" --as wt-a
```

```
muster ask 0007 -- "--verbose or --quiet by default?"
```

### muster answer

Answer a question in a task. It's written under the question, and the agent that asked gets it pushed into its session.

```
muster answer <id> <Qn> "<text>"
```

Examples:

```
muster answer 0008 Q2 "B, the window beside the app"
```

### muster escalate

Mark a question "Escalated to owner": it shows as waiting for you. Capture then adds it to WAITING-FOR-OWNER.md. Once per question.

```
muster escalate <id> <Qn> [--as capture]
```

| Option | What it does |
| --- | --- |
| `--as` | who takes it to the owner, usually capture |

Examples:

```
muster escalate 0008 Q2 --as capture
```

### muster inbox

Answers waiting for your session, each handed over once and moved to delivered/ (unless --keep). Without the answer hook, run it between steps.

```
muster inbox [--session <id>] [--keep]
```

| Option | What it does |
| --- | --- |
| `--session` | your session id; default $CLAUDE\_CODE\_SESSION\_ID |
| `--keep` | show them without marking them delivered |

Examples:

```
muster inbox
```

```
muster inbox --session 3fba2222 --keep
```

## Backlog

One-line ideas that aren't tasks yet, shaping them into tasks, and checking a shaped task against the project's vision before it goes to Ready. See [The backlog and shaping](02-concepts.md#the-backlog-and-shaping) and [The vision check](02-concepts.md#the-vision-check).

### muster add

Drop a one-liner in the backlog, to be shaped into a task later. It's a file in the backlog folder (inbox/ today), named by the time and the line; prints its name. No id, no front matter. Line breaks in the line become spaces, and it's refused over 200 characters; the body, if any, is Markdown up to 50,000. Checked like a claim: with $MUSTER\_ROOT at another project's queue, pass --queue to mean it.

```
muster add "<one line>" [--body <text>|-] [--as <who>]
```

| Option | What it does |
| --- | --- |
| `--body` | more detail in Markdown; \\n is a line break; - reads it from stdin |
| `--as` | who found it; default $MUSTER\_AS, then the folder's git user.name |

Examples:

```
muster add "Board flickers when the pane opens"
```

```
muster add "Export as CSV" --body "The owner asked for it on the call."
```

```
pbpaste | muster add "From the clipboard" --body -
```

### muster shape

The shaping brief for a backlog note: its full text, what the product, tech and UI roles each write, the project's docs to read (CLAUDE.md, docs/VISION.md, docs/DESIGN.md, docs/decisions/), open tasks that share words with it (to catch a duplicate), and the exact muster file command with the task template. Writes nothing.

```
muster shape <note-file> [--json]
```

| Option | What it does |
| --- | --- |
| `--json` | the same as JSON, for Drover and scripts |

Examples:

```
muster shape 2026-10-12-1405-board-flickers.md
```

```
muster shape 2026-10-12-1405-board-flickers.md --json --queue drover
```

### muster file

File a shaped backlog note as a task: the next id, in Shaping, with your body after a "From the backlog" line quoting the note. The note moves into the task's attachments/\<id\>/, so nothing is lost; all under the queue's lock, so a note is filed once. Then muster pass \<id\> --from shaping puts it in Ready (it needs acceptance criteria).

```
muster file <note-file> "<title>" [--type code|bug|chore|tech-debt|spike|design|docs|research] [--priority high|normal|low] [--tags a,b] [--time 2h] [--test-time 30m] [--tokens 400k] [--touches <folders,globs>] --body <text>|-
```

| Option | What it does |
| --- | --- |
| `--type` | code (default), bug, chore, tech-debt, spike, design, docs or research |
| `--priority` | high, normal (default) or low |
| `--tags` | comma-separated tags |
| `--time` | estimated agent time: 45m, 2h, 1.5d |
| `--test-time` | estimated test time, the QA agent plus your check: 30m, 1h |
| `--tokens` | estimated tokens: 150k, 1.2M |
| `--touches` | where in the repo it works, folders and globs, comma-separated: what muster ready --parallel keeps apart |
| `--body` | the shaped task in Markdown, with ## Acceptance criteria (required); - reads it from stdin |

Examples:

```
muster file 2026-10-12-1405-board-flickers.md "Board flickers when the pane opens" --type bug --time 1h --tokens 300k --body -
```

```
muster file my-idea.md "Export as CSV" --body "**What the owner said:** …\n\n## Acceptance criteria\n- [ ] AC1: …"
```

### muster drop

Set a backlog note aside: it moves to the backlog's dropped/ folder with the reason added at its end (one line, 500 characters). It's off the board but kept.

```
muster drop <note-file> "<why>"
```

Examples:

```
muster drop 2026-10-12-1405-board-flickers.md "a duplicate of 0012"
```

### muster vision

Check a task against the project's one picture (docs/VISION.md, docs/decisions/, docs/DESIGN.md) and the tasks it touches. Muster runs no agent: muster vision \<id\> prints the vision brief for one, with the task quoted as data, the picture files that exist, the open tasks it touches (sharing two title words, named in its text, its parent and depends), what to look for (conflicts, overlaps, gaps, the cost of pivoting) and the JSON to return. Writes nothing. --record - reads that JSON from stdin (or takes it as its value): the task gets a ## Vision section under its write-up (above its Comments and Log), replacing the last one, with each conflict numbered V1, V2… and both sides quoted; the front matter's vision: \<n\> open; and a log line. A conflict whose fix changes direction is also escalated to the owner, as one question per record with the cost of pivoting. --resolve V2 "\<how\>" settles one (one line, 500 characters). While any is open, Shaping → Ready is refused; a task never checked isn't held back. --all lists the inputs for every open task.

```
muster vision <id> [--json] | --record - | --resolve V2 "<how>"   ·   muster vision --all [--json]
```

| Option | What it does |
| --- | --- |
| `--json` | the brief, the record or --all as JSON, for Drover and scripts |
| `--record -` | the agent's result, as JSON on stdin; malformed JSON is refused with what's wrong |
| `--resolve V2` | mark that conflict resolved, with how after it |
| `--all` | every open task's inputs: its vision state and the tasks it touches |

Examples:

```
muster vision 0031
```

```
muster vision 0031 --json --queue drover
```

```
pbpaste | muster vision 0031 --record - --as wt-shape
```

```
muster vision 0031 --resolve V2 "0019 keeps depends:; 0024 derives blocked by tasks from it"
```

```
muster vision --all
```

## Levels

Goals and features group tasks. They have no commands of their own: `muster new` takes a level, a parent and a goal's due date, and `muster set` changes them, and what a task depends on. See [Goals, features and tasks](02-concepts.md#goals-features-and-tasks).

Examples:

```
muster new "Ship 1.0" --level goal --due 2026-12-01
```

```
muster new "Stage track" --level feature --parent 0030
```

```
muster set 0032 parent 0031
```

```
muster set 0018 depends 0014,0016
```

## Projects and the queue

Find the queues Muster knows, start one for a project, check a queue for problems, and rebuild its master list.

### muster projects

Every queue Muster knows: found under ~/playground and added in the app, with each one's gate. projects set \<name\> gate stores the project's gate command in the app's projects.json: what a merge runs in a task's worktree (muster pass \<id\> --from testing). It's split into words like a shell would, quotes and all, but no shell runs it, so a pipe or a cd is just a word: point it at a script in the repo (bin/gate) for anything more. "" clears it; with no gate, nothing merges.

```
muster projects [--json] | muster projects set <name> gate "<command>"
```

| Option | What it does |
| --- | --- |
| `--json` | the list, for scripts |

Examples:

```
muster projects
```

```
muster projects set muster gate bin/gate
```

```
muster projects set drover gate "pnpm check"
```

### muster init

Create \<project-folder\>/tasks: status folders, README with the rules, the tool stand-ins, and its own git. Refuses inside a git repo or over an existing tasks/.

```
muster init <project-folder>
```

Examples:

```
muster init ~/playground/braindump
```

### muster check

Problems in the queue: ids used twice, status not matching the folder, a stage that isn't one, Building with no claim or quiet past the lease (12 h unless changed in the app's Settings), blocked with no reason, waiting on a task that doesn't exist, a link that isn't a plain https link, a backlog note over 1 MB or that's a symlink, a parent that doesn't exist or is at the wrong level, parents in a loop, a for that isn't human or ai, a vision count that doesn't match the task's Vision section, an INDEX.md that shows something the task files no longer say (rebuild with muster index; none yet is fine). Exit 1 if any. A goal past its due date with tasks open, a block from before kinds (read as one from what it waits on until it's blocked again with a kind), and an INDEX.md only due a refresh (the files changed outside muster, as Drover's answers do), are warnings: shown, without failing. Also lists, without counting them as problems, what waits on the owner's look, Shaping tasks with no vision check yet, and what parallel sessions left behind: a started task whose worktree is gone, a worktree of a task sent back to Ready, and a worktree on a task/\<id\>- branch whose task is past Testing, each with the command that cleans it.

```
muster check
```

Examples:

```
muster check
```

```
muster check --queue braindump
```

### muster index

Rebuild INDEX.md, the queue's master list: every goal with its features and tasks, then the tasks under none by stage, the backlog notes, and what's done, each with its stage, the tasks it still waits on, priority, type, estimate (agent time, test time, tokens), how many comments it has and a link to its file. Muster rewrites it after every change it makes, so this is for after a hand edit. It's generated: never edit it by hand.

```
muster index [--check]
```

| Option | What it does |
| --- | --- |
| `--check` | write nothing; exit 1 if INDEX.md is missing or isn't exactly what muster would write now |

Examples:

```
muster index
```

```
muster index --check --queue drover
```

### muster help

This list, or one command in detail.

```
muster help [<command>]
```

Examples:

```
muster help
```

```
muster help new
```

## On every command

| Option | What it does |
| --- | --- |
| `--queue <name\|folder>` | which queue; default $MUSTER\_ROOT, then the nearest queue from the current folder (a queue, or tasks/ beside or above it) |
| `--as <who>` | who's acting, for the log; default $MUSTER\_AS, then the folder's git user.name, then owner. Inside a Claude Code session muster adds the session (name, short id, checkout, account) |

## Inside a Claude Code session

### You, in a Claude Code session

Run any muster command with ! in front. It runs in that session's folder, so it finds the project's queue on its own:

```
! muster list
```

```
! muster new "Board flickers on resize" --type bug --priority high
```

```
! muster answer 0008 Q2 "B"
```

muster is linked into ~/.local/bin, so ! muster works in every session. Outside ~/playground, pass --queue \<name\>.

Or just say it: "file a bug: the board flickers on resize". That works where the project's CLAUDE.md points at its queue (Drover's does). The session then runs muster new.

Slash commands /muster and /mu are planned (task 0003). Until then, use ! muster or ask.

### Agents doing the work (coding sessions)

Read the queue's README.md first: ../tasks/README.md from the project's repo.

A task moves through stages: Backlog (anyone), Shaping (product, tech and UI agents), Ready (claimable), Building (code agent), Testing (QA agent), Your check (the owner), Done. Only muster changes a stage: never move or edit task files for that.

Never edit stage:, status:, claimed-by: or heartbeat: by hand. Only muster writes them.

Claim with muster, under your branch name, in the queue of the project you're in (it says which queue it claimed in). One claim per session: take the top Ready task, or a given one. Agents never claim a human task (\[you\] in muster list): that's the owner's own to do:

```
muster next --as <your branch>
```

```
muster claim 0007 --as <your branch>
```

Working beside other sessions? Pick from the tasks that are safe together and start one in its own worktree (PROTOCOL.md has the whole loop). Passing it from Testing merges it into main, pushes main and cleans up:

```
muster ready --parallel
```

```
muster start 0007 --as <your branch>
```

On long work, keep the claim alive. A claim with no heartbeat for longer than the lease (12 h unless the owner changed it) goes back to Ready (muster reclaim). Your notes and passes count too:

```
muster heartbeat 0007 --as <your branch>
```

Check in at each milestone and at least every 30 minutes, with what's done and what's next. It's on the card, and it renews the claim:

```
muster checkin 0007 "gate green, writing tests" --pct 60 --as <your branch>
```

Ask without stopping. The answer is pushed into your session after your next tool call (the answer hook):

```
muster ask 0007 "Child webview or a separate window?" --as <your branch>
```

No hook in this checkout? Collect answers yourself:

```
muster inbox
```

Log progress, block, hand on, with the tool stamping the time:

```
muster note 0007 "Phase 1 done, gate green" --as <your branch>
```

```
muster block 0007 --on 0012
```

```
muster pass 0007 --as <your branch> --from building --gate "<the gate's result line>"
```

Testing found a problem? Send it back with the reason:

```
muster return 0007 "AC2 fails: …"
```

Found something new? muster add puts a one-liner in the backlog, and a shaping session files it. Something only the owner can do (an account, a secret, a decision) goes the same way: the owner makes it a human task:

```
muster add "<one line>" --as <your branch>
```

### The capture session (PM and tech lead)

Writes the owner's ideas and bugs up as tasks, answers agents' technical questions, and takes product questions to the owner in plain words (WAITING-FOR-OWNER.md).

```
muster new "…" --tags spike --body "**What the owner said:** …\n\n## Acceptance criteria\n- [ ] AC1: …"
```

```
muster answer 0007 Q1 "Use the window" --as capture
```

```
muster escalate 0007 Q2 --as capture
```

```
muster reclaim
```

Shapes backlog notes into tasks: the brief says what to read, who writes what, and the command that files it. The note goes into the task's attachments; one not worth a task is dropped with the reason:

```
muster shape <note-file>
```

```
muster file <note-file> "<title>" --type bug --time 1h --tokens 300k --body -
```

```
muster drop <note-file> "<why>"
```

Before a shaped task goes to Ready, an agent checks it against the project's picture: the brief says what to read and the JSON to record. A conflict that changes direction comes to the owner as one question:

```
muster vision <id>
```

```
muster vision <id> --record -
```

../tasks/bin/task ask, answer, escalate and inbox still work: they run the same muster commands.

The app's Helper tab does the same job without a separate session.
