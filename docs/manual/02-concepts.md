# Concepts

Each idea in Muster, in the order you meet it. Every section says what the idea is, why it's there, where you see it in the app, the command for it, and where the decision behind it is written down. Ideas that aren't built yet are marked **coming**, with the task that builds them.

## Projects and queues

A **queue** is the folder that holds one project's tasks: one Markdown file per task. Today it sits beside the project's code, at `<project>/tasks` (for example `~/playground/myapp/tasks`, next to the repo at `~/playground/myapp/main`), and has its own git history. A **project** is the name the app shows for a queue, taken from the folder it's in.

Why a folder of files: you, every agent and every tool can read a task with nothing but a text editor, and git keeps its history. The queue stays outside the code repo so a code commit can't sweep it up.

In the app, the projects are listed down the left side, with **All** at the top. The app finds every queue at `~/playground/<project>/tasks` on its own, and one level deeper (`~/playground/<group>/<project>/tasks`); add a queue kept anywhere else in **Settings**, under **Projects**. Inside a queue, the folders `inbox/`, `todo/`, `doing/`, `blocked/` and `done/` mirror each task's stage for older tools. Don't move files between them yourself.

Commands: `muster projects` lists the queues, and `muster init <project-folder>` starts one. Every command takes `--queue <name>`; without it, `muster` uses the queue of the folder you're in. See [Projects and the queue](05-commands.md#projects-and-the-queue).

**Coming (task 0005):** the queue moves into the repo as `.muster/`, and task files stop moving between folders. Decisions: [0001](../decisions/0001-app-on-drovers-stack.md) for how the app finds queues, [0003](../decisions/0003-muster-in-the-repo-backlog-and-stages.md) for `.muster/`.

## A task and its file

A **task** is one piece of work small enough for one agent to build. Its file is named by its id and title, like `0012-board-flickers-when-the-pane-opens.md`. The id is four digits, given out in order by `muster`, so two tasks never share one.

Everywhere you see a task, it's named by its **key**: its project's prefix and its number, like `mu-12` for Muster's `0012`. Type either one: `muster show mu-12`, `muster show 12` and `muster show 0012` find the same task, and `muster show br-7` finds Braindump's task 7 from any folder. The file keeps its id, so nothing is renamed. A project's prefix is 2 to 4 lowercase letters: the first two of its name unless another project starts the same, or the one you set in **Settings › Projects** or with `muster projects set <name> prefix <p>`. A key written in a task's text links to that task in the app.

The file has four parts:

- The **front matter**, between two `---` lines at the top: the task's fields, such as `type: bug`, `priority: high`, `stage: building` and `claimed-by: wt-a`.
- The **write-up**: the title, what you asked for, the details, and the **acceptance criteria**, a checklist that says when it's done.
- **Questions** and **Comments**, when there are any.
- The **Log**: one line per thing that happened, each stamped with the time and who did it.

A task has a **type** (code, bug, chore, tech-debt, spike, design, docs or research), a **priority** (high, normal or low), and may have tags, estimates, links and attached files.

Only `muster` writes the stage, the claim and the times. That's the rule that keeps the board true: an agent that edited `stage:` by hand could say a task was tested when it wasn't. You and agents may edit the write-up, with `muster edit` or **Edit** in the task pane.

In the app, click a card to open the task in the pane beside the board. Commands: `muster new`, `muster show`, `muster set`, `muster edit`. See [Tasks](05-commands.md#tasks) and the file format in [PROTOCOL.md](../PROTOCOL.md#task-file).

## Stages and roles

A **stage** is the step a task has reached, and each stage has a **role**: the kind of worker whose job it is.

| Stage | Role | To move on |
| --- | --- | --- |
| Backlog | anyone | Shaping starts it. |
| Shaping | product, tech and UI agents | It needs acceptance criteria, and no open [vision conflict](#the-vision-check). |
| Ready | none: it waits to be claimed | Passing it claims it for whoever passes it. |
| Building | a coding agent | It needs the gate's result line: the summary the project's checks print. |
| Testing | a testing (QA) agent, never the one that built it | Every criterion is ticked with its proof, except yours, and a session that didn't build it passes it on (or you do). |
| Your check | you | You tick your criteria, then Accept. |
| Done | none | It can still be sent back. |

**Passing** moves a task to the next stage: `muster pass <id> --from <stage>`. The `--from` says which stage you saw it in, so a pass made twice, or after someone else moved it, is refused instead of skipping a stage. **Sending back** moves it to an earlier stage with a reason: `muster return <id> "<why>"`. You can also put a task in any stage at all: that's your **override**, `muster stage <id> <stage>`, or a click on the stage track in the task pane, and it's logged like everything else.

Why stages: each kind of work gets its own worker and its own check, and you only see a task when it's your turn. See [Stages](05-commands.md#stages), [the spine in VISION.md](../VISION.md#the-spine-one-task-start-to-finish) and decision [0003](../decisions/0003-muster-in-the-repo-backlog-and-stages.md).

## Blocked, and why

**Blocked** means a task is waiting for something. It isn't a stage: a blocked task stays in its stage, keeps whoever holds it, and can't be passed on until it's unblocked.

You block a task by saying what kind of wait it is, and what it waits on. There are five kinds:

- **tasks**: other tasks have to be merged first. Their ids go into its `depends:`, so it's written once, and it clears by itself once they're all merged: in Your check or Done (see [Depends and the graph](#depends-and-the-graph)).
- **you**: you have to do or decide something.
- **permission**: the agent lacks some access; say what, and where to grant it.
- **question**: an open question in the task. It clears when the question is answered.
- **outside**: a third party, a service, a release.

```
muster block 0012 --on you "which colour for the button?"
muster block 0012 --on 0014,0016
```

The card, the table, the timeline and the task pane say why in words, the kind first: "Blocked on you: which colour for the button?", "Blocked by tasks: mu-14 (Building, codex), mu-16 (Ready)". In the pane each task's key opens that task. A task blocked on you or by a permission also counts as waiting for you. `muster unblock 0012` lets it go on. In the task pane, use **Block** (pick the kind from its chips) and **Unblock**.

## Paused

**Paused** means "not now, keep it as it is". You pause a task from Backlog through Testing, with a reason if you like, and resume it when it's time:

```
muster pause 0042 "the design changes first"
muster resume 0042
```

Only tasks pause; goals and features follow their tasks. A paused task keeps its stage, its write-up, whoever holds it and its worktree, and it stays in its column with a grey **Paused** chip: "Paused 2 h ago · the design changes first". It isn't blocked: it isn't red and doesn't wait on anything. While it's paused nobody can claim it, pass it, pick it next, send it back, move its stage or release it (you included: resume it first), and its holder's lease doesn't run out. Tasks that depend on it still wait for it, and your own paused task doesn't count as waiting on you: it leaves Waiting for you and stays on the board.

If a session is working on it (Building or Testing), that session is asked, through its mailbox, to stop at a safe point, check in what's done and next, and end its turn. When you resume it, the session is told to carry on from its last check-in, and in Building its claim gets a fresh lease: if that session doesn't come back within one lease, whoever claims it next can have it. The log says `owner: paused (why)` and `owner: resumed after 2 h`. Paused time is hatched grey on the timeline and isn't counted as work time.

## Goals, features and tasks

Tasks can be grouped in two **levels** above them:

- A **goal** is an outcome you want, such as "Ship 1.0". It may have a due date.
- A **feature** groups the tasks that build one thing, such as "Stage track". Its parent is a goal.
- A **task** sits under a feature or a goal, or under nothing.

Agents build only tasks. A goal or feature never goes into Building; its stage comes from its tasks. It's Done when every task under it is Done, and in progress once any of them is past Ready.

In the app, **Group by Feature** or **Goal**, above the board and the table, gives each feature or goal a band of its own with its progress ("3 of 5 done") and the estimate left. Each card names its parent.

```
muster new "Ship 1.0" --level goal --due 2026-12-01
muster new "Stage track" --level feature --parent 0030
muster new "Pass button" --parent 0031 --time 2h
```

See [Levels](05-commands.md#levels) and decision [0003](../decisions/0003-muster-in-the-repo-backlog-and-stages.md) (task 0016).

## Depends and the graph

A task can **depend on** other tasks in the same queue: it shouldn't start until they're merged. You set it with `muster set 0018 depends 0014,0016` (an empty value clears it).

A dependency counts once it's **merged**. An agent's work merges into main after Testing passes, then waits in Your check for your look, so the next tasks can build on it: a task in **Your check or Done** counts. One in Testing or earlier doesn't. Your own tasks (human tasks) count only when they're **Done**, since nothing merges for them. `muster claim` and `muster next` won't take a task while anything it depends on isn't merged, and the card, the table, `muster list` and the Graph all show it "Blocked by tasks" until then. When the last one reaches Your check, the waiting task's log says so: `unblocked: 0014 in Your check`.

If you send a task back from Your check (**Send back**, or `muster return 0014 "<why>" --to building`), the tasks that depend on it and haven't started show blocked by it again. Ones already under way (in Building with someone on them, or in Testing or Your check) keep going: nothing moves them back, and their log gets a note naming where it went, `0014 went back to Building: <why>`.

The **Graph** tab draws it: an arrow points from a task to each task that waits on it. Click a task and what it waits on, and what waits on it, light up. A task with nothing left to wait on is marked "ready to start". See [The graph](04-in-the-app.md#the-graph) (task 0019).

## The backlog and shaping

The **backlog** is where an idea goes before it's a task. A **backlog note** is a one-line Markdown file, with no id. Anyone can add one: you from the app or the command line, an agent that found something while working, or a file dropped into the backlog folder by hand.

**Shaping** turns a note into a task. A shaping session reads the note and the project, then writes what done looks like (the acceptance criteria), the type and an estimate. Notes exist so that capturing an idea costs one line, and only shaped work reaches an agent.

In the app, notes sit at the top of the Backlog column under the **Add to the backlog…** field. Open one to **File as task**, **Drop** it with a reason, or **Copy shaping brief** for a session.

```
muster add "Board flickers when the pane opens"
muster shape 2026-10-13-0105-board-flickers-when-the-pane-opens.md
muster file 2026-10-13-0105-board-flickers-when-the-pane-opens.md "Board flickers when the pane opens" --type bug --body -
muster drop 2026-10-13-0105-board-flickers-when-the-pane-opens.md "a duplicate of 0012"
```

`muster file` makes the task in Shaping and keeps the note in the task's files. Before it goes on to Ready, it's checked against the project's picture ([The vision check](#the-vision-check)). See [Backlog](05-commands.md#backlog), the tutorial [Add a one-liner and shape it](03-tutorials.md#add-a-one-liner-and-shape-it), and decision [0003](../decisions/0003-muster-in-the-repo-backlog-and-stages.md) (task 0015).

## Claims, leases and check-ins

A **claim** says who is building a task. A coding session takes a Ready task with `muster next` (the top one it may take) or `muster claim <id>`, and the task moves to Building, held by that session. Each session holds one task at a time: it passes, sends back or blocks the first before it claims another. Two sessions claiming at once can't both win.

Who is acting is the `--as` name, usually the session's branch or worktree. Inside a Claude Code session, `muster` adds the session itself (its short id, its folder and its account), so you see `wt-0030 (main-29 · session 7b5e8856 · cc-hg)` without anyone typing it.

A claim has a **lease** of 12 hours, unless you change it on this Mac in the app's **Settings**, under **Agents** (from 1 hour to 7 days; the `muster` command uses the same one). Everything the holder does through `muster` (a note, a question, a pass, a check-in or `muster heartbeat`) renews it. A claim nobody has heard from for longer can be released: `muster reclaim` puts every such task back in Ready, and the task pane shows **Release** on one. That way a session that crashed never holds a task forever.

A **check-in** is the holder saying how far it got: `muster checkin 0012 "gate green, writing tests" --pct 60`. The card shows it ("60% · gate green, writing tests · 12 min ago"). Sessions check in at each milestone and at least every 30 minutes, and a card quiet for longer says "no check-in for 45 min". The percentage is the session's own guess.

**Claude Code's session states** add what Claude Code itself says about the holder's session, when it can say. The app reads `claude agents --json` (every 15 seconds at most, while the window shows, only for accounts named by a session stamp in Building or Testing) and the card says **working**, **waiting for input in Claude Code** (a permission prompt or a question waiting on you, so it counts as needing you) or **session ended, task still in Building** (in Building only: in Testing the holder is the builder that passed it, and its session ending is the normal course). The state applies only when the holder's session holds nothing else, counted across every project: in Building, no other claim; in Testing, no claim and no other task in Testing. A coordinator's subagents share its session, so their cards don't take its state; a builder that passed a task to Testing and claimed the next shows on the new one. When Claude Code can't say (it isn't installed, its answer doesn't read, the session isn't listed, or the state is one Muster doesn't know), the card shows the check-in lines as before; with a state, "no check-in for 45 min" stays under it, quiet. Sessions still check in. `muster list --json` gives each task the same `sessionState`.

The **holder** is who the task is with now: the claim in Ready and Building, and `held-by:` in Testing (who passed it there) and Your check (you). The card and the pane's status line show it. The status line adds **LIVE** when the task's file changed in the last 5 minutes (the LIVE window, also in **Settings**, under **Agents**), and "quiet" with how long when it didn't.

**The tester isn't the builder.** The session that built a task (claimed it, or passed it into Testing) can't pass it out of Testing: `muster pass` refuses it, names the builder and says what to do, and a builder's `muster stage <id> your-check` or `muster move <id> done` is refused the same way. A fresh session, such as a QA subagent with its own `--as` (`muster/qa-0042`), checks each criterion and passes it, or you do. Your pass is logged as "owner's override: no separate tester". A coordinator's subagents share its session id, so a tester under another label in the builder's session is let through, and the log says "tested under another label in the builder's session". A Testing card still held by its builder says "needs a tester". The Your-check card says "built by … · tested by …", and so does the task pane's status line from Testing on.

See [Claims](05-commands.md#claims) and decision [0001](../decisions/0001-app-on-drovers-stack.md) (how a quiet claim is told apart; tasks 0002 and 0030).

## Your criteria and Your check

An acceptance criterion ends with how it's verified. `— verify: command (pnpm test)` means a command proves it; an agent ticks it with the proof. `— verify: owner (open a task and watch the board)` means only you can judge it: it's **your criterion**.

Testing passes a task to Your check without your criteria ticked: they're yours to tick. In **Your check** the card names the next one to try, and the task pane shows **Your checklist**, the open ones first. Try each thing, click its **Tick**, then click **Accept**, and the task is Done. Accept stays off until all of yours are ticked. A criterion verified by "command (test) and owner" isn't yours alone: the agent ticks it with the command's proof.

Each task in Your check also has an **effort**, from your criteria still open: **Just accept** (none, and no question or block waiting on you), **Needs a session** (one mentions a session, a terminal or a command) or **One look** (the rest, a question for you or a block on you included). A criterion can say its own with `— effort: look` or `— effort: session`. The board and Waiting for you group Your check by it, and **Accept all** accepts the Just accept ones at once ([Your check](04-in-the-app.md#your-check)).

```
muster tick 0012 AC2
muster pass 0012 --from your-check --commit 3f2a9c1
```

See the tutorial [Do your check and accept](03-tutorials.md#do-your-check-and-accept), and [the task file format](../PROTOCOL.md#task-file) (task 0028).

## Questions and answers

An agent that needs an answer asks in the task and keeps working on what the question doesn't block:

```
muster ask 0012 "Flicker on resize too, or only on open?" --as wt-a
```

The question is numbered (Q1, Q2…) and written under `## Questions`, and `muster` records which session asked. The **capture** session, the one that writes tasks up for you, answers what it can and **escalates** the rest to you. An escalated question shows in **Waiting for you** and in the task pane, with a box to answer it (⌘↩ sends).

When the answer is a choice, the agent asks with **options** (2 to 6), its **pick** and what it does with **no answer**:

```
muster ask 0012 "Fix the flicker now or after the redesign?" --option "Now, in this task" --option "After the redesign (0020)" --pick a --no-answer a --as wt-a
```

The options show as chips under the question, the agent's pick marked **pick**. One click answers with that option; the box below still takes "something else", or words to add to a pick. In a terminal: `muster answer 0012 Q1 --pick b`.

Your answer is written into the task and handed to the session that asked. Where the **answer hook** is installed (a Claude Code hook that ships with Muster), the answer appears in that session after its next step, without it looking. Without the hook, the session runs `muster inbox`. Why: an agent never stops to wait, and every answer ends up in the file, where the next session finds it.

See [Questions](05-commands.md#questions), the tutorial [Answer an agent's question](03-tutorials.md#answer-an-agents-question), and [PROTOCOL.md](../PROTOCOL.md#questions-and-answers).

## Comments, notes and the log

Three places hold words about a task besides its write-up:

- **Comments** are a conversation: anyone can add one at any stage with `muster comment <id> "<text>"` or the comment box in the pane, even while someone else holds the task.
- A **note** is one line in the log: `muster note <id> "<text>"`, or the note bar at the bottom of the pane.
- The **log** is the task's history. `muster` adds a stamped line for every change, and the stage lines (`owner: stage testing → your-check`) are what the pane's History and the Timeline read.

## Estimates, sessions and cost

A task can carry three **estimates**, set while shaping: agent time (`--time 2h`), test time for the testing agent plus your check (`--test-time 30m`), and tokens (`--tokens 400k`). Cards show them, the filter bar adds up what's left ("≈ 2 d · 3.2M tokens left"), and the pane's **Work so far** shows the time a task has spent in Building and Testing next to its estimate. An agent day counts 8 hours.

What a task actually cost comes from the Claude Code sessions that worked on it. `muster` stamps the session's id on the claim, on who holds the task, and on every log line it writes, so Muster knows which sessions to look at. It reads their transcripts on this Mac, where Claude Code keeps them, and sends nothing anywhere.

The task pane's **Work** section shows what it found:

- one row per session: who it was, the stage it worked in, when it started and ended (or **live** while it's still going), its time and its tokens;
- the totals against the estimates, such as "Tokens 70k of 100k (70%)", and the tokens by kind;
- a guess at how far along the task is, from its ticked criteria and its stage, labelled as a guess;
- **View** on a row, which opens that session's transcript below the rows: one line per prompt, reply and tool call.

A card shows a small dot beside its id while one of its sessions is working, meaning its transcript changed in the last 5 minutes (the LIVE window). A session that ran on another Mac shows as "not on this Mac". A session that also worked on other tasks is counted in full on each, and the section says so, so the task's own cost is lower than shown.

See [The task pane](04-in-the-app.md#the-task-pane) (task 0025).

## The vision check

`docs/VISION.md` is the project's one picture: what it is, how a task moves, and which task builds what. Together with `docs/decisions/` and `docs/DESIGN.md`, it's what every task has to fit.

Before a shaped task goes to Ready, a session checks it against that picture and against the open tasks it touches. Muster doesn't run an agent for this. `muster vision <id>` prints a brief for a session: the task, the picture's files, the tasks it touches (by shared title words, by id in its text, its parent, and what it depends on), and what to look for, which is conflicts, overlaps, gaps and the cost of changing course. The session hands its findings back with `muster vision <id> --record -`, and Muster writes them into the task.

The result is the task's **Vision** section, under its write-up. Each conflict is numbered (V1, V2…) with both sides quoted. While a conflict is open, the card shows "⚠ 2 vision", and the task can't pass from Shaping to Ready: `muster` says "0024 has 2 open vision conflicts (V1, V2): resolve them or record a new check". Overlaps and gaps are listed but never hold a task back. Nor does a missing check: **Problems** and `muster check` list the Shaping tasks that haven't been checked yet.

Sometimes fixing a conflict would change the project's direction, because it goes against a decision, the spine, a design rule or an answer you gave earlier. Then the check adds the **cost of pivoting**: the tasks to rewrite, the built work that would change or be thrown away, what's been spent, what redoing it would take, what waiting costs and what keeping course costs, with the session's recommendation. That reaches you as one question in **Waiting for you**.

To settle a conflict, click **Resolve V2** in the task pane's Vision section and say how in one line, or use `muster vision <id> --resolve V2 "<how>"`. **Copy vision brief** puts the brief on the clipboard for a session. Don't edit the Vision section by hand: Edit leaves it out, and `muster check` reports one whose count doesn't match.

```
muster vision 0031
pbpaste | muster vision 0031 --record - --as wt-shape
muster vision 0031 --resolve V2 "0019 keeps depends:; 0024 derives blocked by tasks from it"
```

See [muster vision](05-commands.md#muster-vision) (task 0029).

## What needs you

One list, everywhere, of what's waiting on you:

- tasks in **Your check**,
- questions **escalated** to you,
- tasks **blocked on you** or **by a permission**,
- tasks whose agent's session is **waiting for your input** (a permission prompt, as Claude Code reports it),
- your own tasks (for: human) to do and doing, except the ones you paused.

They're counted on the **Waiting for you** tab, beside each project on the left in orange, and on Muster's icon in the Dock (every project you show), and the agents' ones are marked "needs you" on their cards, which sort to the top of their columns.

Why: you shouldn't have to look for what needs you, in any display mode. See [DESIGN.md](../DESIGN.md#in-every-mode).
