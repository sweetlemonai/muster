# Muster — task queue protocol (v2, with stages)

> **Stages (Muster 0014, 2026-10-12).** A task goes Backlog → Shaping (product, tech and UI
> agents) → Ready → Building (code agent) → Testing (QA agent) → Your check (the owner) → Done.
> `stage:` in the front matter is the truth, and only `muster` writes it. Hand a task on with
> `muster pass <id> --from <the stage you saw> --as <you>`. `--from` makes a pass refuse if the task
> has moved on since you looked, and the app always sends it. Ready → Building claims it for you, and Building → Testing needs
> `--gate "<the gate's result line>"`. Testing → Your check needs every criterion ticked, and Your
> check → Done can take `--commit <sha>`. Send it back with `muster return <id> "<why>"`. Blocked
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
> (the holder, on long work), on the holder's note, ask, pass, return and block, and when the task
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
type: bug | idea | task
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

## Questions
## Log
- 2026-10-11 captured
```

**Screenshots and other files** go in `attachments/<id>/` (e.g. `attachments/0008/2026-10-11-settings-today.png`),
named by date and what they show. The task links them by that path and also describes what
they show in words, for sessions that can't view images. Anything pasted into a chat is saved
right away, because chat paste files are temporary.

Ids: only capture numbers tasks. Coders who find something new drop a note in `inbox/` named
`<date-time>-<short-slug>.md` (no number), and capture files it.

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
