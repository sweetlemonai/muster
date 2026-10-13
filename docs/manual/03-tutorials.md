# Tutorials

Step by step, with the real commands and what they print. The examples use a made-up project, `myapp`, at `~/playground/myapp`, with its code in `~/playground/myapp/main`. Type the commands in a terminal, or in a Claude Code session with `!` in front (`! muster list`).

## Before you start

You need the app and the `muster` command.

1. Build the app (it holds the `muster` command too):

   ```
   cd ~/playground/muster/main/app && pnpm install && pnpm release:build
   ```

2. Open it from `app/src-tauri/target/release/bundle/macos/Muster.app`.
3. Put `muster` on your PATH, so it works in every terminal and every Claude Code session:

   ```
   ln -s ~/playground/muster/main/bin/muster ~/.local/bin/muster
   ```

4. Check it: `muster help` lists every command.

## Your first project

A project's tasks live in a **queue**, a folder beside its code (see [Projects and queues](02-concepts.md#projects-and-queues)).

1. Pick the folder that holds the project's repo, not the repo itself: `~/playground/myapp`, not `~/playground/myapp/main`. Muster refuses a folder inside a git repo, so a code commit can never sweep the tasks up.
2. Make the queue:

   ```
   muster init ~/playground/myapp
   ```

   It prints `created /Users/you/playground/myapp/tasks`. The new folder has the status folders, a `README.md` with the rules for agents, a `WAITING-FOR-OWNER.md`, the tools in `bin/`, and its own git history with a first commit.
3. Look at the app. A queue under `~/playground` shows up on the left on its own. For a queue anywhere else, go to **Projects** and click **Add existing queue…**. (**New queue…** there does what `muster init` does.)
4. Add a first task. From inside the project, `muster` finds the queue by itself:

   ```
   cd ~/playground/myapp/main
   muster new "Add a dark mode toggle" --type code --priority normal --time 2h
   ```

   It prints `0001 todo/0001-add-a-dark-mode-toggle.md`. The task is in Ready, and its card is on the board.
5. List the queue: `muster list`.

If agents will work on this project, give them write access to the queue's folder first: see [The sandbox and worktrees](08-faq.md#the-sandbox-and-worktrees).

## Add a one-liner and shape it

An idea doesn't need to be a task yet. Put it in the backlog, and shape it when there's time.

1. In the app, type the idea into **Add to the backlog…** at the top of the Backlog column and press ↩. Or in a terminal:

   ```
   muster add "Board flickers when the pane opens"
   ```

   That prints the note's file name, such as `2026-10-13-0105-board-flickers-when-the-pane-opens.md`.
2. Click the note on the board. It opens in the pane, with three buttons: **File as task**, **Drop** and **Copy shaping brief**.
3. To have a session shape it, click **Copy shaping brief** and paste it into a Claude Code session in the project. The brief has the note, what the product, tech and UI roles each write, the project's docs to read, open tasks that look like duplicates, and the exact command that files it. `muster shape <note-file>` prints the same brief.
4. To shape it yourself, click **File as task**. Give it a title, a type and a priority, and write the body with its acceptance criteria:

   ```
   **What the owner said:** the board flickers when the pane opens.

   ## Acceptance criteria
   - [ ] AC1: no flicker when the pane opens. — verify: command (pnpm test)
   - [ ] AC2: it looks steady on the Mac. — verify: owner (open a task and watch the board)
   ```

   Then **Save** (⌘↩). The task is made in Shaping and opens in the pane, with the note kept in its files. The same from a terminal, with that body saved in `body.md`:

   ```
   muster file 2026-10-13-0105-board-flickers-when-the-pane-opens.md "Board flickers when the pane opens" --type bug --time 1h --test-time 20m --tokens 300k --body - < body.md
   ```

   which prints `filed 0001 in myapp (Shaping)`.
5. Check it against the project's picture. In the task pane's **Vision** section, click **Copy vision brief** and paste it into a Claude Code session in the project (`muster vision 0001` prints the same brief). The session records what it found, and the section shows it. If it found a conflict, settle it with **Resolve V1** and a line saying how. See [The vision check](02-concepts.md#the-vision-check).
6. Pass it to Ready with **Pass to Ready** in the pane, or `muster pass 0001 --from shaping`. That's refused while the task has no acceptance criteria or has an open vision conflict. A task that was never checked isn't held back.
7. Not worth a task? **Drop** it with the reason, or `muster drop <note-file> "a duplicate of 0012"`. It moves to the backlog's `dropped/` folder, kept, with the reason at its end.

## Take a task through every stage by hand

This walks one task from Ready to Done with commands, playing every role yourself. The `--as` names who is acting in each step; it goes in the log. In the app, each step is the primary button in the task pane (**Pass to Building**, **Pass to Testing**…, then **Accept**).

1. **Ready → Building.** Passing a Ready task claims it:

   ```
   muster pass 0001 --from ready --as wt-a
   ```

   It prints `0001: Building (doing/0001-board-flickers-when-the-pane-opens.md)`. The card now says who's on it.
2. **Say how it's going.** While building, check in:

   ```
   muster checkin 0001 "found the cause, fixing it" --pct 40 --as wt-a
   ```

   The card shows "40% · found the cause, fixing it · just now".
3. **Building → Testing.** This needs the result line of the project's checks (the gate). Without it, `muster` says `passing 0001 Building → Testing needs the gate's result line (--gate "…")`. With it:

   ```
   muster pass 0001 --from building --gate "gate full: green (9 steps)" --as wt-a
   ```

4. **Testing.** The tester ticks each criterion it has proved. Open the write-up with `muster edit 0001` (or **Edit** in the pane), change `- [ ] AC1:` to `- [x] AC1:` and add the proof at the end of the line, then save. Passing before that is refused: `1 of 2 acceptance criteria aren't ticked; tick each with its proof first (1 more are the owner's, checked in Your check)`.
5. **Testing → Your check:**

   ```
   muster pass 0001 --from testing --as qa
   ```

6. **Your check → Done.** Tick your criterion, then pass it with the commit that ships it:

   ```
   muster tick 0001 AC2
   muster pass 0001 --from your-check --commit 3f2a9c1
   ```

   It prints `0001: Done (done/0001-board-flickers-when-the-pane-opens.md)`.

At any step, `muster return 0001 "AC1 fails: it still flickers once"` sends the task back one stage with the reason, and `--to shaping` sends it further. In the pane that's **Send back**. Every `--from` makes a repeated or late pass harmless: `muster` refuses it with "it moved on since you looked".

## Run parallel sessions in worktrees

Several Claude Code sessions can build different tasks of one project at the same time, each in its own git worktree so their changes don't collide.

1. Make a worktree per session, beside the project's main checkout:

   ```
   cd ~/playground/myapp/main
   git worktree add ../wt-a -b wt-a
   git worktree add ../wt-b -b wt-b
   ```

2. Start a Claude Code session in each. A worktree finds the project's queue on its own, as `~/playground/myapp/tasks` is beside a folder above it.
3. In each session, take the top task this session may take, under the worktree's name:

   ```
   ! muster next --as wt-a
   ```

   Each holds one task at a time, and `next` skips tasks someone else holds, tasks that are blocked, and tasks whose dependencies aren't Done. Two sessions asking at once get different tasks.
4. Watch the board: each card in Building shows who has it, how long ago it started and its latest check-in, and a dot beside its id while the session is working. A session quiet for more than 30 minutes shows "no check-in for …". Open a task to see in its **Work** section what each session has used so far.
5. Merge each branch when its task has passed your check, as you would any branch.

**Coming (task 0023):** one command that claims a task and makes its branch and worktree, a pick of tasks that are safe to build at the same time, and the merge and clean-up after Testing.

## Answer an agent's question

1. An agent asks in a task and keeps working:

   ```
   muster ask 0012 "Flicker on resize too, or only on open?" --as wt-a
   ```

   The capture session answers what it can, and passes the rest to you with `muster escalate 0012 Q1 --as capture`.
2. **Waiting for you** gets a count. Open it: each escalated question is listed with its task. Click **Open to answer**.
3. The pane shows the question with a box under it. Type your answer and press ⌘↩ (or **Answer**). It says "Answered. The asking agent gets it after its next step." when the asking session is on record.
4. From a terminal instead:

   ```
   muster answer 0012 Q1 "Only on open" --as owner
   ```

   which prints `0012: answered Q1; delivered to session 3fba2222`.
5. The answer is in the task under the question, so any later session reads it too. If the asking session doesn't have the answer hook, it collects the answer with `muster inbox`.

## Do your check and accept

1. A task in **Your check** has an orange card and is counted on **Waiting for you**. Click it.
2. The pane's status line says what's left, such as "Waiting on you: 1 of 2 criteria to check". On the left, **Your checklist** lists the criteria only you verify.
3. Try each one on your Mac. Click a criterion to tick it once it's right; click again to untick. Each tick goes in the log.
4. When they're all ticked, click **Accept**. The task is Done.
5. Something's wrong? Click **Send back**, pick where it goes (one stage back is the default) and say why. Back in Building, it goes to whoever built it last.

From a terminal: `muster tick 0012 AC2`, then `muster pass 0012 --from your-check --commit <sha>`, or `muster return 0012 "<what's wrong>"`.

## Read the graph and the timeline

The **Graph** tab shows what waits on what (see [Depends and the graph](02-concepts.md#depends-and-the-graph)):

1. Each box is a task, in its stage's colour. An arrow goes from a task to each task that waits on it, so work flows left to right.
2. Click a box. What it waits on is marked "needed first", what waits on it "waits for it", and everything else dims. The task opens in the pane.
3. A task with nothing left to wait on is marked "ready" (the legend above the graph says "Ready to start").
4. **Hide done** drops finished tasks. Tasks with no links are listed below the graph.

The **Timeline** tab shows when the work happened:

1. Pick a scale: **Day** is the last 24 hours, hour by hour; **Week** is Monday to Sunday in quarter days; **Month** is the calendar month in days. ◀ and ▶ step back and forward, and the button between them (**Now**, **This week** or **This month**) jumps back.
2. Each bar is one task's work. It starts when the work started (the task's first move into Building or a later stage) and ends when the work finished (it reached Your check or Done). Each stage it went through is a stretch in that stage's colour.
3. A thin dotted tail after a bar is the time the task has been waiting for your check.
4. The vertical line is now, and tasks still being worked run up to it. Hatching marks time spent blocked. A faded start means only the day the work started is known, as for older tasks.
5. Tasks whose work hasn't started, or whose log has no times, aren't drawn. A line under the chart says how many there are.

## Find tasks and keep a view

Board, Table, Timeline and Graph share one bar above them, with a search box, filters, and views you can save.

**Find a task by its id:**

1. Press ⌘F, from anywhere in the window. The caret goes to the search box. From a tab that has no search box, such as Help, the board opens first.
2. Type the number. `19` finds task 0019, and so does `0019`. A word made only of digits always means an id, so dates in other tasks' text don't get in the way.
3. Every other task is hidden, and the card's id is marked. Click the card to open it.
4. Press Esc to clear the search, and Esc again to leave the box. Esc in the box never closes the pane.

**Search by words.** Type words in any order. A task matches when each word is somewhere in its title, tags, text or who holds it, and part of a word is enough: "flick" finds "flickers". Capitals and accents don't matter. The words are marked on cards, table rows and timeline rows. Backlog notes are searched by their text as well.

**Filter.** Under the search box are the filter groups: **Stage**, **Type**, **Priority**, **Tags**, **Held by**, **Flags** (blocked, needs you, open questions), and **Captured**, **Changed** and **Done** (today, this week, this month). A group shows only the values that the project's tasks have. Click a chip to keep only those tasks. Two chips in one group mean either; chips in different groups must all match. The line below says what's active, with a ✕ on each, how many tasks it hides ("12 hidden by filters"), and **Clear all**.

**Sort.** On the board, the **Sort** chips order each column: **priority** (the default: what needs you first, then high to low, and Done newest first), **newest**, **oldest**, **changed**, **estimate** or **id**. In the **Table**, click a column's header to sort by it, and again to reverse it; the Table remembers its sort between launches. The Timeline is always in date order.

**Keep a view:**

1. Set up the search, filters and sort you want, such as Stage: Your check.
2. Click **+ Save view**, type a name ("My checks") and press ↩. Esc cancels.
3. The view is now a chip under **Views**. Click it to come back to exactly that search, those filters and that sort. ✕ deletes it.

Views are kept for each project, on this Mac, up to 20 per project. **Group by** and the timeline's scale are remembered too.

**From a terminal**, `muster list` takes the same search, filters and sort, with the same rules, so it lists the same tasks:

```
muster list --search "board flicker" --stage ready,building --sort changed
```

In this manual, the search box above the pages searches every page.
