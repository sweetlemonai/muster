# FAQ and troubleshooting

What goes wrong most often, what it means, and what to do. The quoted messages are what `muster` and the app say.

## A claim looks stale

A card in Building that says **quiet 12 h+** belongs to a session nobody has heard from for longer than the lease (12 hours, unless changed in **Settings**, under **Agents**): it probably crashed or was closed. Open the task and click **Release**, and it goes back to Ready, logged as reclaimed, for another session to take. `muster reclaim` releases every such claim in the queue at once; `muster reclaim --after 1d` waits longer.

"no check-in for 45 min" is only a nudge: the session hasn't said how it's going for a while. It still holds the task. A blocked task never counts as quiet, since it's waiting.

## A task won't pass

`muster` refuses a pass and says why. The usual reasons:

- "it moved on since you looked": someone else moved the task after you last saw it. Look at its stage again and pass it from there (`--from <stage>`).
- It's **blocked**: unblock it first (**Unblock**, or `muster unblock <id>`).
- Someone else holds it in Building: only they can pass it on. You can still put it in any stage yourself with the stage track or `muster stage`.
- "needs the gate's result line": out of Building, add `--gate "<the line your checks printed>"`.
- "acceptance criteria aren't ticked": out of Testing, every criterion except yours needs a tick with its proof.
- "of the owner's criteria aren't ticked": out of Your check, tick yours first (`muster tick <id> AC2`, or the checklist in the pane).
- Out of Shaping, the task needs acceptance criteria.
- "has 2 open vision conflicts (V1, V2): resolve them or record a new check": settle each one with **Resolve** in the task pane's Vision section (or `muster vision <id> --resolve V1 "<how>"`), or have a session check the task again. See [The vision check](02-concepts.md#the-vision-check).
- "depends on 0004, not merged yet": a task can't be claimed until everything it depends on is merged, in Your check or Done ("not done yet" when it waits on one of your own tasks, which only counts once Done).
- A claim is refused while the same session already holds another task in Building: pass, send back or block that one first.
- Goals and features never go into Building: build their tasks.

## Tasks are missing from the board

A search or a filter is probably still on. The line under the filter chips says what's active and how many tasks it hides ("12 hidden by filters"). Click ✕ on one, or **Clear all**. Clicking a saved view turns its search and filters back on, so check that line after you pick one.

To find one task, press ⌘F and type its key or its number: `mu-19` or `19` finds mu-19. See [Find tasks and keep a view](03-tutorials.md#find-tasks-and-keep-a-view).

## An agent didn't get my answer

The answer is always written into the task, under the question, so it's never lost. Getting it into the session that asked is the part that can fail:

- **The answer hook isn't installed** in that checkout. The session runs `muster inbox` to collect it.
- **The session changed its id**, after `/clear` or a handoff. The answer waits in the old session's mailbox: `muster inbox --session <the old 8-character id>`.
- **No session was on record** for the question (it was asked by hand, not with `muster ask`). The app then says "No asking session on record, so it waits there".

## Where is a session's transcript?

Open the task and look at its **Work** section: every session that worked on it has a row, and **View** opens that session's transcript in the pane, one line per prompt, reply and tool call. The latest 1,500 entries are shown, with tool output and thinking left out.

Muster finds the sessions by the `session 7b5e8856` that `muster` writes on the claim, on who holds the task, and in the log. Claude Code keeps each session's transcript in its account's folder under `projects/`, in a file named by the session's full id, which starts with those 8 characters. Muster reads it there, on this Mac, and sends it nowhere. If a row says "not on this Mac", the session ran somewhere else, or its transcript has been deleted. A transcript over 128 MB isn't read. **View** opens only sessions the task names.

The Helper's own conversations are kept by the app, one file per project, in `~/Library/Application Support/ai.sweetlemon.muster/helper/`. **New conversation** starts a fresh one.

## muster says there's no queue

"no queue here: pass --queue <name|folder> or set MUSTER_ROOT" means the folder you're in isn't a queue and has no `tasks/` queue beside it or beside a folder above it. Either `cd` into the project, or name the queue: `muster list --queue myapp` (`muster projects` lists the names) or `--queue ~/path/to/tasks`.

"no queue named …" means that name isn't a project Muster knows. Check `muster projects`, or add the queue in the app's **Settings**, under **Projects**.

## muster says another project's queue

"MUSTER_ROOT points at drover/tasks but you're in muster/…; pass --queue drover to mean it". The `MUSTER_ROOT` variable points at one project's queue while you're working in another, and a claim, pass or edit there would land in the wrong project. Unset `MUSTER_ROOT`, or pass `--queue` to say you mean it.

## muster says to build it first, or doesn't know a command

"muster: build it first" means the app hasn't been built in this checkout, and the `muster` command lives inside it. Build it: `cd ~/playground/muster/main/app && pnpm install && pnpm release:build`.

`unknown command "checkin"` (or another command this manual lists) means the `muster` on your PATH is an older build. Rebuild it the same way.

## The sandbox and worktrees

Claude Code sessions here run in a sandbox that only writes to the folders it allows. A `.muster/` queue is inside its repo, so a session in the main checkout can claim, log and ask with nothing set up. A session in a **worktree** writes the main checkout's `.muster/`, which is outside the worktree: that needs the main checkout's `.muster/` in the session's write list, which the personal layer's linker adds to every worktree it links (task 0004, once the layer has the change). An older queue beside the repo (`<project>/tasks`) has to be in the layer's write list (`~/playground/.claude/layer-settings.json`, `allowWrite`) itself.

Every repo the personal layer is linked into (`link-personal-layer --install`, once per repo; new worktrees are linked on creation) gets two things from Muster: a short rule that tells each session `.muster/` is the project's queue and where its rules are (`rules/muster-queue.md` in Muster's repo), and the answer hook (`bin/answer-hook.py`), which finds the queue from the session's folder. Until the layer carries the hook, only checkouts where it was installed by hand get it, and sessions elsewhere collect answers with `muster inbox`.

## INDEX.md says a refresh is due

`INDEX.md` is the queue's master list, which `muster` rewrites after every change it makes. When another tool changes a task file (Drover's answers do), `muster check` warns "INDEX.md: refresh due" until the next `muster` write. `muster index` rebuilds it now. Never edit it by hand.

## The app says it can't read the queues

Point at "Can't read the queues", top right, to see the error. A queue whose folder was moved or deleted shows as "not a queue any more" in **Settings**, under **Projects**: hide it, or remove it if you added it. **Problems** lists what `muster check` finds wrong inside the queues.
