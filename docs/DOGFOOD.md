# Dogfooding notes: the agent task system, run by hand on Drover

What went wrong or felt wrong while running this queue (one capture session, several coding
sessions, the owner), and what it means for chit
(`~/playground/chit`, `SPEC.md`; design `~/playground/izzy/docs/agent-task-system.md` rev 5;
the earlier draft in `~/playground/@download/tasky/`). Newest first. Each entry: what
happened → what it shows → what chit has or lacks.

## 2026-10-12

43. **Stage history is only as good as the log it's read from** (0014 review, five rounds). Once
    the stage became the truth, every line of free text the tool writes into a task turned into a
    way to fake a claim: a note or title shaped like a stage line, a multi-line answer that planted
    its own `## Log` heading, a `who` with a line break. Each fix was narrow: the exact stage-line
    check on one-line text, readers using the last `## Log`, indenting multi-line question and
    answer text, and cleaning `who` in Rust and Python. It shows the same thing as note 1: the
    file is shared by every writer, so one rule set has to live in one tool. The live state in
    `.git/muster/` (0005) takes the stage out of the file for good.
42. **A guard that matched a phrase refused ordinary prose.** The first fix for forged stage lines
    refused any note containing ": stage ", and capture's own note on 0021 ("…: stage track full
    width") was refused within the hour. Fixed by checking whether the whole log line would read as
    a stage change. A guard on free text has to match the exact thing it protects against, or
    people word around it.
41. **Agents couldn't claim: the sandbox blocked writes to the queue** (the review session's claim of
    Drover 0019 failed; note 16 predicted it). Each session's sandbox only writes inside its own
    repo, and queues sit outside repos on purpose. Fixed in the personal layer (owner's yes):
    `layer-settings.json` → `sandbox.filesystem.allowWrite` now lists `~/playground/{drover,muster,braindump}/tasks`,
    re-applied to all 13 checkouts (the linker refreshes the sandbox block it wrote and keeps the answer
    hook). It applies without a restart. → `muster init` / New queue must add the new queue's
    folder there too (task 0004), or the next project hits the same wall.

40. **Confirmed: an agent typed made-up times all day.** The drover/main session wrote it in a
    note itself: its heartbeats and log lines from 08:00 to 19:00 on four tasks were guessed. The
    real clock at its note was 04:29. The order of events was right, the times weren't. So
    heartbeat-based "stale" checks were meaningless for that session. → Already acted on: the
    app judges staleness by file modification time, and `muster` stamps every change itself.
    Remaining: agents still edit task files by hand for claims and logs, until `muster
    claim/heartbeat/log` exist (Muster task 0002). That's the fix, and it raises 0002's priority.

39. **The answer push works live.** With the hook installed, a self-test question asked and
    answered through `bin/task` showed up in the asking session's context right after its next
    Bash call, with no file read. Sessions already running pick the hook up without a restart (this
    one did). → If the next 0008-style miss doesn't happen, close 7/21/22/34. Watch for: worktrees
    created later (no hook until their settings get it), and `link-personal-layer` rewriting
    `settings.local.json` and dropping the entry.

38. **Asks to the owner were jargon and bare menus.** "Install the answer hook? Drover
    checkouts / personal layer / not yet" and "0015: AC3's owner part" told the owner nothing.
    Their words: "asking me to make a choice with little to no info or background". → Every
    escalation must stand on its own: what it is, why it matters, what each answer leads to,
    the pick, and what happens with no answer. Before asking at all, check whether the PM can
    decide it. Don't escalate checks that tests already cover. chit's `waiting_on: human`
    needs a required "context" field, and the human's view should show it, not the task id.

## 2026-10-11 (night)

36. **The first consumer found three bugs in the contract within an hour.** Drover's 0015
    implementation, written against the README's file contract, caught that `bin/task` kept tabs
    the README strips, that `tasks.json` was rewritten in place (a reader could see it
    half-written), and that the session format was looser in the script than in the contract.
    All fixed, with tests. → A written, versioned contract plus a second implementation is what
    finds these. chit should keep its protocol as a spec with conformance tests, not only code.
37. **Agents make up timestamps.** The drover/main session's log lines run about two hours
    ahead of the clock (it wrote 21:45 at 19:20 local). → Times in task files must come from
    the tool (`bin/task` stamps them), never typed by the agent. Heartbeat staleness checks are
    only as good as the clock that wrote them.

35. **Answer push built** after 34: `task ask` records the asking session (from
    `CLAUDE_CODE_SESSION_ID`), and `task answer` writes into the task and the asker's mailbox. A
    PostToolUse / UserPromptSubmit / SessionStart hook adds the answer to that session's
    context (about 10 ms with an empty mailbox). Known gap: a new session id after `/clear` or a
    handoff misses answers to the old one's questions. → The handoff note should carry open
    question ids. Later, route by task claim as well as by session. *Fixed in part (mu-75,
    decision 0011):* the handoff carries the old session id, and PROTOCOL.md has its writer put
    `muster inbox --session <old>` first in its Next steps, which the successor follows. Nothing
    in the skill or hooks enforces that step, and routing by claim is still open.

34. **The same question came back to the owner after it was answered and confirmed.** The 0008
    session relayed Q2 (Settings window, A/B/C) to the owner again, with its own pick (A),
    though Q2 was ticked with the owner's answer (B) and confirmed. That's the third time an
    asker didn't see an answer (7, 21, 22), even with the v2 rule "re-read the file before
    telling the owner anything is waiting". → Rules in a README don't hold under load. The
    answer has to be pushed into the asking session, or the CLI refuses to raise a question that's
    already answered. This is the top item to build (`SYSTEM.md` 12).

33. **The owner answered the same question in two sessions, differently.** In a coding session
    0002 got "Claude only". Asked again by capture, the owner said "both". Neither session knew
    about the other's question. → Before asking the owner, check the task's log and answers
    (the manifest could list answered questions). When answers conflict, the latest wins, the
    change is written down explicitly ("overrides …"), and the builder is pointed at it.

30. **The shared counter collided, as predicted (3).** A coding session filed the embedded
    browser task as 0015 minutes after capture filed 0015 (the owner's waiting list). The README
    says only capture numbers tasks, but the coder was working directly with the owner, who
    asked it to "add the task". → Two capturers are normal: any session the owner talks to
    will file tasks. Ids must be collision-proof (chit's random/time ids), or allocated by a
    single atomic step (`mkdir ids/0016` style). Renumbered to 0016.
31. **Capture's own `git add -A` swept the other session's new task into capture's commit**
    (`ed5fe64`), the same problem the owner moved the queue out of the repo to avoid. → Commit
    only named files. Better: the CLI commits each change as it's made, by whoever made it.
32. **"Need your approval" for a coder-filed task is a review gate.** The owner asked capture
    to approve a task another session wrote. Capture checked its facts against the code
    (Tauri 2.12.2) and added a security criterion it missed. → A PM review step for tasks
    filed by coders (chit's `inbox` → `todo` promote) works and catches real gaps.

28. **The owner never saw a question waiting for them.** Capture escalated 0008's mockup pick in
    a chat message. Later messages pushed it out of view, and the owner didn't know where the
    mockups were (11 PNGs in `attachments/0008/`, which the sandbox won't let the session open).
    → Escalations need a place the owner already looks, that doesn't scroll away and shows the
    images. For now: `WAITING-FOR-OWNER.md`, kept by capture. In Drover: task 0015. In chit:
    `waiting_on_you` (SPEC, the human's commands) should include attachments, and a terminal
    session should end every reply with the waiting count.
29. **Chat answers rely on the capturer's guess at what was meant.** "2 different users can
    build more, but the 2 can be duplicated" needed a reading, which the owner confirmed later.
    → Writing "Capture's reading: …" in the task and asking for a yes worked. Keep it.

## 2026-10-11 (evening)

25. **v2 worked once the session re-read the README.** The owner told the drover/main session
    once. It read the answer to its question, moved its three waiting tasks to `blocked/` with
    `waiting_on`, released the claims, set heartbeats, and asked capture (in the task logs) to
    block three more. Nobody had to relay anything after that. → The rules are fine. Getting a
    running session to read them is the gap (24).
26. **Nobody owns the bottleneck.** Six tasks (0002, 0003, 0005, 0006, 0007, and 0008's build)
    wait on "the themes branch on main", but there's no task for landing it, so nobody is
    working on unblocking them. → When `waiting_on` names something that isn't a task, capture
    files a task for it, or asks the owner who owns it. chit's `waiting_on` should take only
    task ids or a role, never free text.
27. **"For capture to block it" in a log line worked as a request.** The coder can't claim
    a second task to block it (one claim rule), so it asked capture in the log. → A request
    channel to the PM (an `inbox/` note or a log line with a marker) is worth making explicit.
    It's how coders ask for moves they aren't allowed to make.

## 2026-10-11 (later)

22. **The owner became the message bus.** A coding session in `drover/main` told the owner that
    three questions were waiting (0007: translation layer; 0008: sidebar, window, mockups). All
    three were already answered in the files by capture. The session asked the owner instead
    of re-reading the file, so the owner had to relay it back by hand. → Protocol v2: coders
    never take task questions to the owner. They re-read the file before saying anything is
    waiting. The real fix is the answer push (`SYSTEM.md` 12).
23. **Decision: grow this queue instead of switching to chit** (owner). chit is half done. Its
    ideas are adopted one at a time here and tracked in `SYSTEM.md`. Protocol v2 adds `inbox/`,
    `blocked/`, find-by-id, one claim per session, heartbeat, `waiting_on`, acceptance criteria,
    roles, and git history for the queue.
24. **Rule changes don't reach running sessions.** v2 applies from the next claim. Sessions
    working now read v1 and won't re-read the README on their own. → Same family as 7: the
    protocol version needs to be visible on every call (a CLI prints it, or Drover tells the
    session).

## 2026-10-11

**This queue is a hand-made copy of chit.** The capture session built it before knowing that
chit existed. It has the same shape: one file per task, folder = status, claim by `mv`. But it
has no CLI, no lock, no leases, no git, and its numbering is shared. So every finding below is
also a reason to run the real thing.

### Paths and claiming

1. **A moved file broke an edit.** A session claimed 0007 (`todo/` → `doing/`) while the
   capturer was answering a question on it. The capturer's edit failed because the file was gone.
   → Folder-as-status is fine *if nobody holds paths*. Agents should name tasks by id and let a
   CLI find the file (chit does: `find(ctx, ref)`). Raw file edits are where it breaks.
2. **The owner asked for "a ledger instead of moving files."** The real need: one place to
   see everything, with paths that don't change. → `chit list` (and `--json`, `--watch`) is
   that ledger, generated from the files, so it can't drift. Check that it shows open
   questions and who's waiting on whom.
3. **Shared counter for ids (0001, 0002…).** Safe only while one capturer writes. Coders were
   told to file new ideas too, which is a race. → chit's collision-proof ids are right.

### Questions between sessions (the weakest part)

4. **A question shouldn't have to stop the work.** Two coders (0007, 0008) asked questions and
   said "going ahead meanwhile with the parts that don't depend on it". That was the right
   behaviour. → chit only has `block` (stop, release the claim, wait). It needs a
   non-blocking **ask**: a question recorded on the task, the work continues, and the owner of
   the claim is told when it's answered.
5. **Only the owner may write to a claimed task, so the answerer can't answer.** The capturer
   answered 0007 by editing a file another session had claimed. Under chit's rule (§6.1) that's
   not allowed, and `log` requires the owner. → Questions and answers need their own channel
   that any role may append to, without counting as changing the task. For example, an
   append-only `## Questions` section, or a messages file per task, written through the CLI.
6. **`unblock` releases the claim.** That's fine for an unattended runner. For a live
   interactive session that asked and is waiting, the task drops back to the queue and
   someone else may take it. → An answer should return the task to the session that asked if
   it's still alive (lease), and release it only if not.
7. **Nobody tells the asker that an answer arrived.** The coder only finds out if it happens to
   re-read the file. → Needs a push: the CLI prints pending answers on every call (cheap and
   reliable), and a harness (Drover, `chit run`) types them into the session between turns.
8. **Questions the owner had already answered.** The 0008 coder asked three questions whose
   answers were already in the file, and its own picks went against two of them. It had read
   the task before the answers were added, and didn't re-read before writing. → Before an ask,
   the CLI shows the task's answers and decisions since the claim. Writes that are based on an
   old read (file changed since) are refused or warned.
9. **Routing technical vs product questions works, if someone plays tech lead.** The capturer
   answered a technical question (own i18n layer vs i18next) from the owner's standing rules,
   marked "decided for the owner, who can overrule", and escalated nothing. → Matches SPEC
   §6.4's "technical questions never go to the human". But `block --on` only takes `human` or a
   task id, not `tech-lead`, and no role is defined to answer `tech-lead`. Make the PM (or a
   tech-lead role) the answerer. Every decision made for the owner is listed so the owner can
   overrule it.

### Watching and liveness

20. **One session holds four claims at once.** Session 3fba2222 in `drover/main` has 0004,
    0005, 0006 and 0007 in `doing/`, each partly done and waiting on something (the themes
    branch, an answer). Other sessions can't pick them up, and nobody can tell which one it's
    actually working on. → A claim limit per session (one, or a small number). A task that
    waits on something goes to `blocked/` with `waiting_on`, releasing its claim. Split
    research-done tasks into a finished task plus follow-ups.
21. **An answer may have gone unseen.** The 0007 owner logged "Next: the translation layer
    (question above)" after the capturer had already answered that question in the file. →
    Same as 7: answers need a push, not a re-read.

10. **The watcher dies with the capture session.** A background poll in the capture session
    is the only thing watching for questions. Close the session and questions wait. → Watching
    belongs in the harness (Drover, a launchd agent, `chit run`), not in a chat session.
11. **The watcher fires on its own writes.** Every answer the capturer writes wakes the
    capturer again. → Events should say who wrote them (role and session), so a watcher skips its own.
12. **No heartbeat in the hand-made queue.** 0005 and 0006 sat in `doing/` and nobody could
    tell whether their sessions were alive. → chit's leases fix it. Show "last seen" in the list.

### Input (owner → capture → tasks)

13. **Talking to a capture session works well.** The owner sends one idea per message, often
    with a screenshot or a pointer ("check braindump's settings"). The capturer reads the code,
    writes the task with the owner's words, what exists today, what done looks like and open
    questions, then folds the answers back in. → This is chit's PM pass (`inbox` → `todo` /
    `needs-human`), driven by chat instead of inbox files. chit should accept both: chat
    capture writes through `chit new` and `chit section`.
14. **Screenshots get lost.** The Orca screenshot for 0003 exists only in the chat, so the
    capturer had to describe it in words. → Attachments (`--attach`, `attachments/<id>/`) are
    needed from day one, including images pasted into a chat.
15. **Answers often come several to a message** ("1. … 2. … 3. …") and sometimes touch several
    tasks. → Answers are keyed by task and question number, so a reply maps cleanly.

### Environment friction

16. **Writing outside the repo needs the sandbox lifted.** Bash writes to
    `~/playground/drover/tasks/` are blocked in a session started in `drover/main`.
    The Write and Edit tools work. → A tasks repo outside every code repo needs a standing
    sandbox allowance, or the CLI runs as an allowed command.
17. **The secrets guard blocks text, not just access.** A bash heredoc that only *mentioned* the
    SSH folder in a task's prose was refused. → Write task text through files or stdin, not
    shell strings with prose in them.
18. **`<(…)` fails in the sandbox** (`/dev/fd/11: Operation not permitted`). Tooling should
    use temp files.
19. **Unversioned.** The owner chose to keep the queue outside git so no session's commit
    sweeps it up. That also means no history and no undo. → chit's own tasks repo, committed
    by the CLI, gives both.
