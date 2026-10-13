# In the app

Every view and pane of the Muster app, and what each control does. A **?** beside a pane's title or at the end of a view's toolbar opens the matching section here.

## The window at a glance

- **Left:** the projects (see [The projects sidebar](#the-projects-sidebar)).
- **Top:** **+ New task**, then the views: **Board**, **Table**, **Timeline** and **Graph** (four views of the same tasks), **Helper**, **Waiting for you**, **Activity**, **Problems**, **Projects** and **Help**. Then **Appearance**, and "live" with a dot while the app is reading the queues. If it can't read them, that spot says "Can't read the queues"; point at it to see why.
- **Right:** the **pane**, when one is open: a task, a backlog note, New task or Appearance. It docks beside the content, which shrinks and stays live, so nothing ever covers the board. Drag the pane's edge to resize it (the width is remembered), and press Esc to close it.

The app reads the queues every 2 seconds while its window is visible, and stops while it's hidden.

## The projects sidebar

**All** shows every project together. Below it is each project, with three numbers: an orange one for the tasks that need you, one in the Building colour for tasks in Building or Testing, and the total, notes included. Click a project to see only its tasks. **+ Project** at the bottom opens [Projects](#projects).

## The board

One column per stage, with the stage's role under its name. An empty column shrinks to a slim strip with its name written down it, so every stage stays in view even with the pane open.

Each card shows, top to bottom:

- the project (when you're looking at All), the id, a small dot while a session is working on it (its transcript changed in the last 5 minutes), the type, and the priority: **HIGH** filled in magenta, normal outlined, low faint;
- the title, the feature or goal it's under, and its tags;
- who's on it and how long ago they started, their latest check-in ("60% · gate green, writing tests · 12 min ago"), and a warning when they've gone quiet ("quiet 12 h+", "no check-in for 45 min");
- "⚠ 2 vision" while the task has open [vision conflicts](02-concepts.md#the-vision-check);
- badges: why it's blocked, the kind first ("Blocked on you: …", [Blocked, and why](02-concepts.md#blocked-and-why)); needs you; open questions; files; the estimate; and how long ago it last changed.

The **Sort** chips above the board order each column. The default, **priority**, puts cards that need you first, then high priority, then the lowest id, with Done newest first. Done shows the latest 8, with **Show all** for the rest.

The **Backlog** column starts with the **Add to the backlog…** field: type a line and press ↩ to add it as a note, or Esc to empty it. With more than one project in view, a "to myapp" chip says which project gets it; click it to pick another. Notes come first in the column, as quieter cards marked "note", then the numbered tasks in Backlog.

Above the board, the filter bar is shared by all four views (Board, Table, Timeline and Graph). Nothing in it folds away:

- **The search box** (⌘F from anywhere): words in any order, each found in a task's title, tags, text or holder, and a number finds that id (`19` finds 0019). Matches are marked on cards and rows. Esc clears it, and Esc again leaves the box.
- **Views:** your saved views as chips; click one to bring back its search, filters and sort, ✕ to delete it. **+ Save view** names the current ones (↩ saves, Esc cancels).
- **Group by** None · Feature · Goal (board, table and graph): one band per feature or goal, with its stage, its progress ("3 of 5 done"), the estimate left and a goal's due date, and the stage columns inside. Tasks under none come last. Click a band's title to open the feature or goal.
- The estimate left for what's shown, and how many tasks are shown ("24 of 98 tasks"), then **?**, which opens this section.
- **Filter chips** by group: Stage, Type, Priority, Tags, Held by, Flags (blocked, needs you, open questions), Captured, Changed and Done (today, this week, this month). Each group shows only the values the project's tasks have. Chips in one group mean any of them; groups together mean all.
- **Sort** (board only): priority, newest, oldest, changed, estimate or id.
- **The active line:** each search word and filter in use, with ✕ to remove it, how many tasks they hide, and **Clear all**. With none, it says "No filters: every task shows".

See [Find tasks and keep a view](03-tutorials.md#find-tasks-and-keep-a-view).

## The table

Every task as a row: the project (when you're looking at All), id, title, type, priority, stage, who's on it or what it waits on, the estimate, when it was captured, when it changed, and its tags. Click a column's header to sort by it, and again to reverse it. The Table remembers its sort, even after the app restarts. The search's words are marked in ids, titles and tags. Goals and features are rows too, with their progress and the estimate left. With Group by on, each band gets a heading row. Click a row to open it in the pane.

## The timeline

Each task's work as a bar across time, with **Scale** chips: **Day** is the last 24 hours in hours, **Week** is Monday to Sunday in quarter days, and **Month** is the calendar month in days. Week is the default, and the scale you pick is remembered. ◀ and ▶ step back or forward a day, a week or a month, and the button between them (**Now**, **This week** or **This month**) jumps back. The name of the period shown is beside them.

A bar starts when the work started, at the task's first move into Building or a later stage, and ends when the work finished, when it reached Your check or Done. It has one stretch for each stage it was in, in that stage's colour, from the times `muster` logged. While a task waits in Your check, a thin dotted tail runs on from its bar to now. Tasks still being worked run up to the vertical line, which is now. Hatching is time spent blocked. A faded start means only the day the work started is known, as for older tasks. Point at a bar or a stretch for its dates, and click a row to open the task.

Tasks whose work hasn't started, or whose log has no times, aren't drawn; a line under the chart says how many. The search's words are marked in the rows. See [Read the graph and the timeline](03-tutorials.md#read-the-graph-and-the-timeline).

## The graph

What waits on what, per project (set with `muster set <id> depends …`, see [Depends and the graph](02-concepts.md#depends-and-the-graph)). An arrow points from a task to each task that waits on it. Click a task: what it waits on lights up as "needed first", what waits on it as "waits for it", and the rest dims. "ready" marks a task with nothing left to wait on. **Hide done** drops finished tasks, and tasks that aren't linked to any other are listed under the graph.

## The task pane

Click a card to open its task beside the board. The pane keeps every control in the same place for every task, from the top:

1. **The top line:** the project and how long ago the task changed, and **×** to close.
2. **The title**, with its id. Click the title to rename it: ↩ saves, Esc keeps the old one. The **?** beside it opens this section.
3. **The status line:** the stage, why it's blocked (the kind first; each task it's blocked by opens with a click), **LIVE** (the file changed in the last 5 minutes) or how long it's been quiet, who's on it and who held the stages before, the stage's role, and what's waiting on you, or "nothing waiting on you".
4. **The stage track:** all seven stages, the task's own lit. Click another stage to put the task there. That's your override: it happens at once, and the message under it offers **Undo**. When the move would release someone's claim, the stage's tooltip says whose.
5. **Two columns.** The left one shows what this stage needs:
   - Backlog, Shaping and Ready: the acceptance criteria and the estimate.
   - Building and Testing: **Work so far**, the time spent against the estimate, the tokens estimated, and the stage history. The sessions and the tokens they used are in the **Work** section further down.
   - Your check: **Your checklist** of the criteria only you verify, and the history.
   - Done: the history.
   - A goal or feature: its tasks and their stages, its progress and the estimate left.

   The right one is the **Next step**: the one primary button, **Pass to** the next stage (**Accept** in Your check), then **Send back** and **Block** or **Unblock**. **Release** shows when the holder has been quiet past the 12-hour lease. If someone else holds the task in Building, there's no Pass: only they can pass it, and the pane says so. Under the next step are the task's settings: **Priority** and **Type** show their value as a chip (click it to lay out the choices), **Est** has agent time, test time and tokens (↩ or leaving the field saves), then **Tags**, **Files** (images as thumbnails that open large; **+ Attach** picks more) and **Links** (a click opens one in your browser, https only).
6. **The stage forms**, under both columns, when one is open: the gate's result line for Pass to Testing, **Send back** with where to and why, or **Block** with what it waits on. The core's answer, or its refusal, shows under them.
7. **The path line:** the commit that shipped it, and the task's file.

Below that come, in order:

- the **open questions**, each with a box to answer it (⌘↩ sends);
- the **write-up**, with **Rendered**, **Raw** and **Edit**;
- **Vision**: where the [vision check](02-concepts.md#the-vision-check) stands ("not checked", "no open conflicts" or "2 open conflicts"), a **Resolve V1** button for each open conflict (say how it was settled, in one line), **Copy vision brief** for a session, and the findings the check recorded;
- **Work**: every Claude Code session that worked on the task, with the stage, when it started and ended (or **live**), its time and tokens; the totals against the estimates; a guess at how far along it is; and **View** on a row to read that session's transcript below. The transcripts are read on this Mac and not sent anywhere. See [Estimates, sessions and cost](02-concepts.md#estimates-sessions-and-cost);
- the **Comments**, with a box to add one (⌘↩ sends);
- the **Log**, and any **Check-ins**;
- at the bottom, the note bar: **Add to log** puts a line in the task's log.

**Edit** turns the write-up into a text box in place. ⌘S saves and Esc cancels; once you've typed, the first Esc only warns and the second throws the changes away. A save is refused if the file changed since you opened it, or while someone else holds the task in Building (add a note instead), and your text stays in the box. While the editor or the comment box holds unsaved text, the pane won't switch to another card.

To attach files to the task, drop them anywhere on the pane, or paste them into its top part or the note bar.

## A backlog note

Click a note in the Backlog column to open it in the pane. The top says its project, that it's a backlog note and when it was added, then its first line as the title and its file name. Three buttons:

- **File as task:** a form with the title, **Type** and **Priority** chips, and the body (write the acceptance criteria here). **Save** (⌘↩) makes the task in Shaping, keeps the note in its files and opens the new task.
- **Drop:** say why, and the note moves to the backlog's `dropped/` folder with your reason.
- **Copy shaping brief:** puts the brief a shaping session starts from on the clipboard.

The note's text is below the buttons, up to 20 KB.

## New task

**+ New task** or ⌘N opens it in the pane. Fill in, top to bottom: the **Project**, **Priority**, **Agent time**, **Test time** and **Tokens**; the **Level** (Task, Feature or Goal) and, for a task or feature, its **Parent**'s id; the **Type**; a one-line title; tags; and the description in Markdown. Paste or drop screenshots anywhere in the form, or use **Add files…**. **Create** (⌘↩) makes the task in Ready and opens it. Esc or **Cancel** closes the form.

## Waiting for you

Every task waiting on you, in one list: questions escalated to you (with the question), tasks blocked on you or by a permission, and tasks in Your check. **Open to answer** opens the task in the pane. The tab's count is the number of tasks.

## Activity and Problems

**Activity** lists what changed, newest first: commits to the queues, and task files that changed, with their stage and last log line. **Problems** lists what `muster check` finds wrong in the queues, such as an id used twice or a claim with no one behind it; the tab shows how many. Below them, marked **Vision**, are the Shaping tasks with no vision check yet. They're warnings, not problems, and their count on the tab is the quieter second number.

## The Helper

A chat that files tasks for you, per project. Tell it ideas, bugs and tasks, paste screenshots, and press ⌘↩ (or **Send**). It checks the queue for duplicates, reads the project to understand the code, files one task per point through `muster`, and asks in plain words when something's unclear.

The first time, pick which Claude account it uses. It runs your own Claude Code with that account's login, restricted to `muster` and reading, so it can't change files. What you write goes to Anthropic, as in any Claude Code session. The bar has the **Project**, **Change account**, **Stop** (while it's working), **New conversation**, and **?**. Each reply shows "n steps" with the commands it ran folded inside, and what the turn cost. See decision [0002](../decisions/0002-helper-runs-the-owners-claude-restricted.md).

## Projects

Every project Muster shows, with its folder and task count:

- **New queue…** picks a project's folder and makes its queue there, like `muster init`.
- **Add existing queue…** picks a queue kept outside `~/playground`.
- **Rename** a project, **Hide** or **Show** it, and **Remove** one you added.

None of them deletes files. A project marked "not a queue any more" points at a folder that no longer has a queue's `README.md` and `todo/` folder.

## Appearance

Opens in the pane, and every change applies at once and is remembered:

- **Match the Mac's light or dark:** pick a dark theme and a light one, and the app follows the Mac. Off, one theme stays.
- **Theme:** Violet night, Violet day, Graphite, Midnight, Paper, Snow and High contrast.
- **Text size:** 90%, 100%, 115%, 130% or 150%, for everything in the window, this manual too. ⌘+ and ⌘− step through the sizes and ⌘0 goes back to 100%; the size is remembered.
- **Accent:** Violet, Teal, Rose, Gold, Green or Graphite. Building takes the accent colour, unless it would look like another stage's; then Building stays violet, and Appearance says so.
- **Cards:** how a card shows its colour (Stripe, Tint, Top bar, Dot or Outline), and whether the colour shows its **Stage** or its **Priority**. Blocked and "needs you" always take over the colour.
- **Back to Muster's violet** resets the theme and accent.

## Help

This manual. The sidebar has the search box, then every page with its sections; the page you're on is lit. ◀ and ▶ at the top and bottom of a page go to the page before and after. Links between pages and sections move here; a link to another file in the repo says where it is. Click a command line in a grey block to copy it. A **?** anywhere in the app opens its section here, and the pane stays where it was.

## Colours

One colour, one meaning, everywhere. A stage's colour only ever means that stage, the same in every theme, on column tops, cards, pills and timeline bars. Words carry everything else.

| Colour | Means |
| --- | --- |
| Grey | Backlog |
| Blue | Shaping |
| Cyan | Ready |
| The accent (violet by default) | Building, and what's selected or has the focus |
| Amber | Testing |
| Orange | Your check, and anything that needs you |
| Green | Done |
| Red | Blocked, and errors |
| Magenta | High priority |

Priority has colours of its own, never a stage's: high is magenta (a filled chip), normal a light slate (an outlined chip) and low grey (faint text). They colour the cards when Appearance's **Cards** is set to show priority. Tags and types are neutral, outlined labels: they never take a stage's colour, so a bug is marked by the word "bug", not by red. The rules are in [DESIGN.md](../DESIGN.md#the-colour-code).
