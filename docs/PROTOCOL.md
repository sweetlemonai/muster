# Muster — task queue protocol (v2, with stages)

> **Parallel sessions: one worktree per task (Muster 0023, 2026-10-13).** Several sessions can
> work one queue at once, each on its own task in its own git worktree. "Work Muster's queue in
> parallel-safe worktrees" means this loop:
>
> 1. **Pick.** `muster ready --parallel` lists up to 3 tasks (`--parallel <n>` for more) that are
>    safe side by side: claimable now (Ready, not blocked, nobody's claim, an agent's, everything
>    they depend on done), none depending on another or on a task in Building or Testing, and
>    their `touches:` not overlapping each other's or those of the tasks in Building or Testing. A task
>    says where it works in `touches:` (folders and globs, comma-separated, set at shaping with
>    `muster file … --touches` or `muster set <id> touches "app/src/TaskPanel*, docs/"`); two
>    entries overlap when the part of one before its first wildcard starts the other's, and **a
>    task without `touches:` never pairs**. Take one no other session is on.
> 2. **Start.** From the project's repo: `muster start <id> --as <you>`. It claims the task by
>    `claim`'s rules, makes a branch `task/<id>-<slug>` from the local main's commit (the main
>    checkout may be dirty: only its commit is used) and a worktree for it at
>    `<project>/worktrees/<repo>/<id>-<slug>` (`--at <path>` for another place, `--here` when a
>    runner already put you in a worktree of your own: it's switched to the task's branch), records
>    both as `worktree:` and `branch:` in the task, and prints them with the brief: the task file,
>    what it touches, its criteria. Work in that worktree only.
> 3. **Build.** Check in at each milestone and at least every 30 minutes
>    (`muster checkin <id> "<done, next>" --pct <n>`). Commit on the task's branch. When the
>    project's gate is green, `muster pass <id> --from building --gate "<its result line>"`, as
>    always: nothing is pushed and no PR is opened. The worktree stays for Testing.
> 4. **Review and fix.** Testing reviews it and ticks each criterion with its proof. A finding the
>    builder must fix is `muster return <id> "<why>"`: back to Building, in the same worktree.
> 5. **Merge: `muster pass <id> --from testing`.** For a started task this pass is the merge, in
>    order, stopping at the first failure: it takes the repo's merge lock (a flock on
>    `<git common dir>/muster/merge.lock`, so one merge runs at a time); rebases the branch on the
>    local main (**a conflict aborts the rebase and returns the task to Building with the files
>    named**); runs the project's gate in the worktree (**red returns it with the gate line**;
>    any gate line saying RED is red), without `MUSTER_ROOT` or `TASKS_ROOT`; if main moved while
>    the gate ran, rebases and gates again, twice at most, then stops with "main keeps moving;
>    pass again", so **main only ever gets a tree the gate saw**; fast-forwards the local main
>    to the branch (never a merge commit), only if the main checkout is clean and on main, else it
>    stops with "the main checkout isn't clean" or "isn't on main"; pushes main to origin, plainly
>    (if origin moved, it fetches and stops: "origin moved; rebase main and pass again"); removes
>    the worktree (never a runner's, made with `--here`: that one is left, detached) and deletes
>    the branch, and origin's copy only when it's the same commit; records the merge in the task
>    (`commit:` the merge, the report logged: `merged task/… into main as <sha> · <gate line> ·
>    Building 2h 10m · Testing 35m · 4 check-ins`) and passes it to Your check. Once main has the
>    merge the record is written whatever: if the task was blocked or moved meanwhile, it keeps its
>    stage and the command says "merged into main as <sha>; the task is <stage>: use muster
>    stage". Last, it names the next task to start that doesn't clash with the tasks still in
>    Building or Testing. On any stop before main has the merge nothing is deleted: fix what it says
>    and pass again. It's never forced. The project's gate is the
>    command in the app's `projects.json`, set with `muster projects set <name> gate "<command>"`
>    and run as words, without a shell (this repo's: `bin/gate`); with none set, nothing merges.
>    The app's Pass is refused for a started task in Testing: the merge runs from the command line.
>    The worktree is removed (unless it's a runner's), so run the pass from outside it (the main
>    checkout is fine).
> 6. **Next.** Start the task it named, or stop when it says there's none.
>
> **Giving up:** `muster release <id> --clean` puts the task back in Ready, removes its worktree
> and deletes its local branch, after listing what that would lose; uncommitted files or commits
> on no other branch refuse it without `--force`. A runner's worktree (`--here`) is never
> removed: it's left detached, its files as they are, and only the branch goes. **Left behind:** `muster check` lists a started
> task whose worktree is gone, a worktree of a task sent back to Ready, and a worktree on a
> `task/<id>-…` branch whose task is past Testing, each with the command that cleans it. Never
> delete a worktree or a branch by hand while its task is in Building or Testing.

> **Human tasks (Muster 0022, 2026-10-13).** `for: human` in the front matter makes a task the
> owner's own: something only they can do (an account, a secret, a decision, a try on a device).
> **Agents never claim one**: `muster claim` refuses it ("0041 is the owner's to do") and
> `muster next` passes it by. It goes To do (Ready) → Doing (Building) → Done by the owner's
> `muster pass … --as owner`, with no gate line, no Testing and no Your check, and no lease.
> Only the owner makes one (`muster new "<title>" --human --as owner`, or New task in the app):
> **an agent that finds something only the owner can do puts it in the backlog** with
> `muster add "<what the owner needs to do>"`, and the owner files it. `muster set <id> for
> human|ai` switches who does a task while nobody is on it (made human in Backlog or Shaping, it moves to To do). An agent's task can wait on a human
> task with `depends:` ("needs the API key first"): it can't be claimed until the human task is
> Done, with nothing more written. `muster list` marks human tasks `[you]`, and INDEX.md "you".
> A file without `for:` (every older one) is an agent's.
>
> **INDEX.md, the master list (Muster 0018, 2026-10-13).** `INDEX.md` in the queue folder lists
> every task with a link to its file: goals with their features and tasks, then the tasks under
> none by stage, the backlog notes, and what's done. muster writes it after every change it makes,
> under the queue's lock, so **never edit it by hand**. After you edit a task file by hand, run
> `muster index`. `muster check` fails on a list that shows something the files no longer say.
> Tools that write task files outside muster (Drover's answers, under Drover's own lock) leave the
> list's refresh to the next muster write: until then `muster check` and `manifest.py --check`
> only warn "INDEX.md: refresh due", without failing. A queue committed with its project (`.muster/`) would see branches conflict on it; the
> pre-commit hook that rebuilds it and the git merge driver that does the same come with task 0005.
>
> **Stages (Muster 0014, 2026-10-12).** A task goes Backlog → Shaping (product, tech and UI
> agents) → Ready → Building (code agent) → Testing (QA agent) → Your check (the owner) → Done.
> `stage:` in the front matter is the truth, and only `muster` writes it. Hand a task on with
> `muster pass <id> --from <the stage you saw> --as <you>`. `--from` makes a pass refuse if the task
> has moved on since you looked, and the app always sends it. Ready → Building claims it for you, and Building → Testing needs
> `--gate "<the gate's result line>"`. Testing → Your check needs every criterion ticked except
> the owner's (`— verify: owner`, checked in Your check), and Your check → Done needs those too
> (the owner ticks them: `muster tick <id> AC2`, or in the task pane) and can take `--commit <sha>`. Send it back with `muster return <id> "<why>"`. Blocked
> is a flag on any stage: `muster block <id> --on "<task id|owner|capture> …"`, then `muster unblock <id>`.
> The owner or capture can override with `muster stage <id> <stage>`. Until task 0005 moves queues
> into `.muster/`, the tool also keeps `status:` and the status folder below as a mirror of the
> stage (Shaping and Ready → `todo/`, Building, Testing and Your check → `doing/`, blocked →
> `blocked/`), so older readers keep working. **Don't move task files or edit `stage:`, `status:`
> or `claimed-by:` by hand.** `muster check` and `manifest.py --check` report a stage whose status
> doesn't match it, which is what a hand move leaves behind. A note, title or block reason is
> refused only when its log line would read as a stage change (`<who>: stage <from> → <to>`),
> because only the tool writes those. The text below is the earlier v2 protocol (written for Drover's queue).
> Where it says to move files, use the commands above. It's rewritten with 0005.
>
> **Claims, leases and questions (Muster 0002, 2026-10-12).** Claim with `muster next --as <you>`
> (the top Ready task you may take: not blocked, high priority first, then the lowest id) or
> `muster claim <id> --as <you>`. One claim per holder (the owner may hold several): anything that
> would give you a second unblocked task in Building in that queue is refused, a claim, a pass from
> Ready, an unblock or a return to you alike, so pass, return or block the first. It's checked under
> the queue's lock (two claims at once: one wins), and a claim says which queue it was made in. A
> claim has a lease: `heartbeat:` is stamped by the tool on the claim, on `muster heartbeat <id>`
> (the holder, on long work), on the holder's note, comment, ask, pass, return and block, and when the task
> is unblocked (waiting isn't going quiet). `muster check` warns about the same quiet claims. `muster reclaim [--after 12h]` puts every
> Building claim quiet for longer than the lease (no heartbeat, or without one no change to the
> file) back in Ready, logged as `stage building → ready (reclaimed from <holder> (no heartbeat for
> <n> h))`, and the app's **Release** does it for one stale card. Claims and hand-ons refuse a queue
> that came from `$MUSTER_ROOT` while you're in another project's folder; `--queue <name>` says you
> mean it. **Agents never edit `stage:`, `status:`, `claimed-by:` or `heartbeat:` by hand.** Ask
> with `muster ask <id> "<question>" --as <you>` and collect answers with `muster inbox` (the hook
> delivers them anyway); capture answers with `muster answer` and escalates with `muster escalate`.
> `bin/task ask|answer|escalate|inbox` still works and runs the same commands, with the same file
> contract (below).
>
> **Backlog notes and shaping (Muster 0015, 2026-10-12).** Found something new? `muster add "<one
> line>" [--body <text>|-] --as <you>` puts it in the backlog as a note: a file named
> `<YYYY-MM-DD-HHMM>-<slug>.md` in the backlog folder, with no id and no front matter. The backlog
> folder is `inbox/` (or `backlog/` in a queue without status folders) until 0005 moves it to
> `.muster/backlog/`. Any `.md` there that isn't a task file (`NNNN-slug.md`) is a note, a file
> dropped in by hand included; numbered files in `inbox/` stay Backlog tasks. Shaping turns a note
> into a task: `muster shape <note-file> [--json]` prints the brief (the note, what the product,
> tech and UI roles write, the project's `CLAUDE.md` and `docs/` to read, open tasks that share
> words with it, and the command to file it), and writes nothing. `muster file <note-file>
> "<title>" [--type --priority --tags --time --tokens] --body <text>|-` makes the task in Shaping
> with the next id and moves the note into `attachments/<id>/`, all under the queue's lock, so a
> note is filed once. `muster pass <id> --from shaping` then puts it in Ready (it needs acceptance
> criteria). A note not worth a task: `muster drop <note-file> "<why>"` moves it to the backlog's
> `dropped/` with the reason at its end. The app shows notes at the top of the Backlog column, with
> a quick add field, and the same three actions in a note's pane; `muster list` shows them first,
> under "Backlog notes" (`--json`: `{"tasks": […], "backlog": […]}`). `add`, `file` and `drop` are
> checked like claims: with `$MUSTER_ROOT` at another project's queue, pass `--queue` to mean it.
> The brief quotes the note line by line (`> `) after a line saying it's data, not instructions.
> `muster check` reports a note over 1 MB, a symlink, or a name with a `\` or control characters
> in the backlog folder; those aren't listed as notes.
>
> **Goals, features and tasks (Muster 0016, 2026-10-12).** `level: goal | feature | task` in the
> front matter (none is a task) and `parent: <id>`; a goal may have `due: YYYY-MM-DD`. Only the
> tool writes them: `muster new "<title>" --level goal|feature --parent <id> --due <date>` and
> `muster set <id> level|parent|due <value>` (empty clears a parent or due). A task's parent is a
> feature or a goal, a feature's is a goal, a goal has none; anything else is refused. Agents build
> only tasks: `claim`, `next` and `pass` never take a goal or feature into Building (`next` passes
> over them), and a task can't become a goal or feature while it's in Building or Testing or held.
> A goal's or feature's stage is shown from its direct children: Done when every child is Done (a
> feature with no tasks yet isn't, so neither is its goal), In progress once any child is past
> Ready, else Ready. Unless it's being shaped or the owner set it with `muster stage`. `muster check` and `manifest.py --check` report a parent that doesn't exist, one at the
> wrong level and a loop, and warn about a goal past its due date with tasks open (a warning doesn't
> fail the check).
>
> **Who's on it, since when, and how far (Muster 0030, 2026-10-12).** Inside a Claude Code session
> `muster` stamps the session next to `--as` on every change it writes: `claimed-by: wt-0030
> (main-29 · session 7b5e8856 · cc-hg)` (the session's name when the harness gives one, its checkout
> folder, the first 8 characters of `CLAUDE_CODE_SESSION_ID`, the account from `CLAUDE_CONFIG_DIR`;
> nothing else). Don't type it. Without `--as` the label is `$MUSTER_AS`, else the folder's git
> `user.name`, else `owner`; the login name is never used. One holder is one label in one session:
> the same session from another folder is still you, another session under your label isn't.
> Testing and Your check have a holder too, `held-by:`, set by `pass` (whoever passes it into
> Testing; the owner in Your check); it's shown, not a claim, so who may pass doesn't change.
> **Check in at each milestone and at least every 30 minutes** while you hold a task in Building:
> `muster checkin <id> "<what's done, what's next>" [--pct 60] --as <you>`. It logs
> `<who>: check-in 60% · <text>`, renews your claim like `muster heartbeat`, and the card shows it
> ("60% · gate green, writing tests · 12 min ago"). The percentage is your own estimate. A card
> quiet for longer than 30 minutes says "no check-in for 45 min". A note, title or reason that would
> read as a check-in is refused, like one that would read as a stage change.

> **Task fields: type, estimates, links, comments (Muster 0017, 2026-10-12).** `type:` is one of
> code, bug, chore, tech-debt, spike, design, docs, research (`muster new --type`, `muster set <id>
> type`). An old `type: task` reads as code, and an old `type: idea` stays as it is, but `new` and
> `set` refuse idea: an idea goes in the backlog (`muster add`). Estimates are `est-time:` (agent
> time), `est-test-time:` (the QA agent plus the owner's check, the same format: 30m, 2h, 1.5d) and
> `est-tokens:`; `muster new --time --test-time --tokens`, `muster set <id> time|test-time|tokens`.
> **Links** are one front-matter line, `links: The spec <https://…>; <https://…>`: each entry an
> optional title and the address in `<…>`, entries separated by `;`. Only `muster link <id>
> <https-url> ["title"]` and `muster link <id> --remove <n|url>` write it: https only, at most 2,000
> characters of printable ASCII (percent-encode the rest), no user name or password and no
> percent-encoding in the host, a title without `<`, `>` or `;`, at most 20 links. `muster check`
> reports a link that isn't a plain https one, and
> the app opens only those. **Comments** go in a `## Comments` section that only the tool writes,
> right above the Log (the last `## Log`), made there by the first comment: `muster comment <id>
> "<text>"` (or `--body -`) adds `- **<who>, <YYYY-MM-DD HH:MM>:** <text>`, with the text cleaned and
> indented like an answer (rule 3 below: control characters out, 4,000 characters, every line after
> the first indented by two spaces), a line that would read as a stage change refused, and logged as
> `commented`; the holder's comment keeps their claim alive, like a note. Anyone may comment on any
> task at any stage, one someone else holds included:
> comments aren't the write-up, so `muster edit` and the app's Edit leave them out and keep them as
> they are, and a description or write-up can't have a `## Comments` line of its own. A new
> `## Questions` goes above the Comments. Decision 0003 keeps new comments in the live state and
> writes them into the file at each stage change; until task 0005 builds that state, they're written
> to the file straight away.
>
> **Vision check (Muster 0029, 2026-10-12).** Before a shaped task goes to Ready, an agent checks
> it against the project's one picture (`docs/VISION.md`, `docs/decisions/`, `docs/DESIGN.md`) and
> the tasks it touches. Muster runs no agent: `muster vision <id> [--json]` prints the brief (the
> task quoted as data, the picture files, the open tasks it touches by title words, ids named in
> its text, its `parent:` and `depends:`, what to look for, and the JSON to return) and writes
> nothing. `muster vision <id> --record - --as <you>` takes that JSON on stdin: unknown fields and
> malformed JSON are refused with what's wrong, and every text is cleaned to one line and capped.
> The tool writes the task's `## Vision` section under the write-up, above its Comments and Log
> (replacing the last one), with the conflicts numbered V1, V2… and both sides quoted, plus
> `vision: <n> open` in the front matter and a log line; INDEX.md shows "⚠ 2 vision" on its line.
> A conflict whose fix changes direction (a decision, the spine, a design rule, an owner's answer)
> carries a pivot: its cost-of-pivoting table goes in the section, and the record asks the owner
> one escalated question with all of them and the recommendation. No session asked it, so it
> carries your label without the session stamp and the answer stays in the task. Settle a
> conflict with `muster vision <id> --resolve V2 "<how>"`. **`muster pass <id> --from shaping`
> refuses while a conflict is open;** overlaps and gaps never hold it, and a task never checked
> isn't held back (`muster check` lists Shaping tasks with no check, as a warning). **Don't edit
> the Vision section or `vision:` by hand:** `muster check` reports a count that doesn't match the
> section. Once a task is checked, `muster edit` and the app's Edit leave the section out of the
> write-up and keep it as it is, and refuse a write-up that adds a `## Vision` line (a
> description's own `## Vision` from before the first check stays editable). `muster vision --all` lists every open task's inputs for a whole-queue check.

Ideas, bugs and tasks from the owner, written up by the capture session and picked up by coding
sessions. This queue is also a test bed for chit (`~/playground/chit`, `SPEC.md`). Its rules are
taken from chit one at a time, tried here, and recorded in `SYSTEM.md` (what's adopted) and
`DOGFOOD.md` (what went wrong).

**One queue, one place:** `/Users/imassih/playground/drover/tasks/`, outside every code repo.
Every checkout shares it. It has its own git history (the capture session commits). Never
delete a task file.

## Roles

| Role | Who | Does |
|---|---|---|
| Owner | the human | product decisions, answers what's escalated |
| Capture (PM + tech lead) | one session | writes tasks, triages `inbox/`, answers questions, escalates product questions to the owner, keeps the queue healthy |
| Coder | any other session | claims a task, does it, logs, asks |

Coders never ask the owner directly about a task. They ask in the task file. Capture answers
or escalates. That way every answer lands in the file, where the next session finds it.

## Layout: the folder is the status

| Folder | Status | Meaning |
|---|---|---|
| `inbox/` | inbox | Raw notes from coders (new bugs, ideas found while working). Capture turns them into tasks. |
| `todo/` | todo | Ready to claim. |
| `doing/` | doing | Claimed by one session. |
| `blocked/` | blocked | Waiting on something (`waiting_on`). Nobody holds it. |
| `done/` | done | Shipped, with the commit. |

**Find a task by its id, never by a remembered path:** files move between folders. Before every
read or edit, locate it fresh:
`ls /Users/imassih/playground/drover/tasks/*/0007-*`

## The manifest: `tasks.json`

`tasks.json` lists every task (id, title, status, folder, path, claim, heartbeat, waiting_on, open
questions, attachments), plus counts, the inbox, what's waiting for the owner, the next free id,
and any problems found. **It's generated from the task files, never edited by hand.** The files
are the source of truth, so nobody fights over one shared file.

- Build it: `python3 -I /Users/imassih/playground/drover/tasks/bin/manifest.py`
- Check only (exit 1 on problems): `... bin/manifest.py --check`. It catches a status that doesn't
  match the folder, ids used twice or not matching the file name, `doing/` with no claim,
  `blocked/` with no `waiting_on`, `waiting_on` pointing at a task that doesn't exist, and a
  heartbeat older than 12 hours.
- Capture rebuilds it with every commit. Anyone may run it to read the queue.

## Task file

```markdown
---
id: 0012
type: code | bug | chore | tech-debt | spike | design | docs | research
priority: high | normal | low
status: todo | doing | blocked | done
captured: 2026-10-11
claimed-by:            # branch/worktree + session id, set when claimed
heartbeat:             # date-time of the owner's last log line; set on every log line
waiting_on:            # owner | capture | 00NN (another task), when blocked
commit:                # set when done
---

# One-line title

**What the owner said:** their words, close to verbatim.
**Details:** what exists today, where in the code, what's wanted.

## Acceptance criteria
- [ ] AC1: what must be true. — verify: command (`pnpm check`) | screenshot (what) | owner (what they check)

A criterion whose verify part starts with `owner` is the owner's: agents don't tick it, Testing
passes to Your check without it, and the owner ticks it there. "command (test) and owner" is
not the owner's alone: tick it with the command's proof.

## Questions
## Log
- 2026-10-11 captured
```

**Screenshots and other files** go in `attachments/<id>/` (e.g. `attachments/0008/2026-10-11-settings-today.png`),
named by date and what they show. The task links them by that path and also describes what
they show in words, for sessions that can't view images. Anything pasted into a chat is saved
right away, because chat paste files are temporary.

Ids: only the tool numbers tasks. Coders who find something new run `muster add "<one line>"`,
which drops a note in `inbox/` named `<date-time>-<short-slug>.md` (no number), and a shaping
session files it with `muster file` (see the note at the top).

## Working a task (coder)

1. **Pick:** `todo/`, high priority first, then lowest id. Skip tasks whose `depends on` (in
   Details) isn't done.
2. **One claim at a time per session.** Finish, block or release your current task before you
   claim another.
3. **Claim:** `mv /Users/imassih/playground/drover/tasks/todo/00NN-*.md /Users/imassih/playground/drover/tasks/doing/`.
   If it fails, someone else got it. Then set `status: doing`, `claimed-by`, `heartbeat`, and
   add a log line.
4. **Work** in the repo with its rules (`CLAUDE.md`, gate, work log). Never copy task files into
   the repo.
5. **Log** each meaningful step (one line) and update `heartbeat`. A task in `doing/` with no
   heartbeat for 12 hours is returned to `todo/` by capture, with a note.
6. **Waiting on something** (the themes branch, another task, an answer that blocks everything):
   set `status: blocked` and `waiting_on`, log why, and move it to `blocked/`. That releases your
   claim. Don't sit on it in `doing/`.
7. **Done:** tick the acceptance criteria with their proof, set `status: done` and `commit`,
   log, and move it to `done/`.

## Questions and answers

**Use the `task` command.** Answers then reach you on their own:

```sh
T=/Users/imassih/playground/drover/tasks/bin/task
$T ask 0016 "Child webview or a separate window for phase 2?" --from <your branch>
$T inbox            # answers waiting for this session (the hook delivers them anyway)
```

`ask` numbers the question, writes it into the task, lists the task's earlier questions (read
them: yours may be answered), refuses a question that's already answered, and records your
session. When capture answers (`$T answer 0016 Q3 "…"`), the answer is written into the task
and put in your session's mailbox. A Claude Code hook (`bin/answer-hook.py`, on PostToolUse,
UserPromptSubmit and SessionStart) hands it to you after your next tool call, mid-work. You
don't have to re-read anything to see it. Without the hook, run `$T inbox` between steps.

### File contract (v1): for tools that write answers without running `bin/task`

`tasks.json` is replaced atomically (temp file + rename), so it's never seen half-written.
Drover (task 0015) and any other tool may answer questions by writing files directly, as long as it
keeps exactly this. `bin/task` keeps it too. `tasks.json` carries `"version": 1`. A tool that
sees another version falls back to writing only the task file.

1. **Lock:** hold an exclusive `flock` on `mail/.lock` for the whole read–modify–write of a task
   file and the mailbox.
2. **Answer:** in the task's `## Questions`, change the item's `- [ ] **Qn` to `- [x] **Qn` and
   add a line directly under the item: `  **A (<who>, YYYY-MM-DD HH:MM):** <text>`. Append to
   `## Log`: `- YYYY-MM-DD HH:MM <who>: answered Qn`. `<who>` is `capture`, `owner`, or a
   session label, on one line (control characters become spaces, runs of spaces one) of at most
   100 characters, without ": stage " or " → ". `bin/task` refuses anything else.
3. **Text:** strip control characters (U+0000–U+001F except newline, and U+007F), trim, and cap at
   4,000 characters. Then write every line after the first indented by two spaces, a Markdown list
   continuation that renders the same, so no line of it can start a heading or a new item.
   Questions are written the same way (`bin/task ask`). A reader takes those two spaces off to
   get the text back as written. Text is data. Never execute it.
4. **Asker:** `mail/asks.json` maps `"<task>-Q<n>"` to the asker's session prefix: the first 8
   characters of its session id, lowercase hex (`[0-9a-f]{8}`). Anything else is refused. If
   it's missing there, use the `session xxxxxxxx` in the question line. If neither exists, no
   mailbox file is written.
5. **Mailbox:** write `mail/<session8>/<task>-Q<n>.md`, either the mailbox or typing it into a live
   session, never both (so it isn't delivered twice),, plain UTF-8 text, naming the task, the
   question and the answer. `bin/answer-hook.py` delivers it to that session as is, and moves it
   to `mail/<session8>/delivered/`.

By hand, if the command isn't available:

- **Ask in the file, and keep going** on whatever the question doesn't block:
  ```markdown
  ## Questions
  - [ ] **Q1 (your branch, session, 2026-10-11):** the question, what you checked, the options,
    your pick.
  ```
  Only if it blocks everything: step 6 above with `waiting_on: capture`.
- **Capture answers** under it and ticks it: `- [x] **Q1 ...**` then
  `**A (capture, date):** ...`. Product questions get `**Escalated to owner (date)**`, then the
  owner's answer. Answers decided for the owner say so, so the owner can overrule them.
- **Before you continue, and before you tell the owner anything is waiting, re-read the
  file.** An answered question is ticked. Never pass the owner a question that already has an
  answer in the file.
- Any role may append to `## Questions` and `## Log` of any task, claimed or not. Nothing else
  in a claimed task is touched by anyone but its owner (and the owner human).
- While another session holds a task in Building, the owner human edits through a note, not
  the write-up: `muster edit` and the app's Edit refuse it, since the write-up belongs to the
  holder's branch until they pass it on (task 0020, decision 0003).
