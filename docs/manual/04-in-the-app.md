# In the app

Every view and pane of the Muster app, and what each control does. A **?** beside a pane's title or at the end of a view's toolbar opens the matching section here.

## The window at a glance

- **Left:** the projects (see [The projects sidebar](#the-projects-sidebar)).
- **Top:** **+ New task**, then the views: **Board**, **Table**, **Timeline** and **Graph** (four views of the same tasks), **Helper**, **Waiting for you**, **Activity**, **Problems**, **Help** and **Settings** (where [Appearance](#appearance) is). Then the display mode, **ADHD**, **Standard** or **Focus**, as a chip that opens [Display](#display), and "live" with a dot while the app is reading the queues. If it can't read them, that spot says "Can't read the queues"; point at it to see why.
- **Right:** the **pane**, when one is open: a task, a backlog note or New task. It docks beside the content, which shrinks and stays live, so nothing ever covers the board. Drag the pane's edge to resize it (the width is remembered; it stops where the board would get too narrow, and in a narrow window, or with the text zoomed in, it's made narrower to fit, down to 240 px), and press Esc to close it.

The app reads the queues every 2 seconds while its window is visible, and stops while it's hidden.

**The Dock badge:** Muster's icon in the Dock shows how many tasks need you, across every project you show, so you see it without opening the window. It's the same count as the sidebar's with **All** selected (see [What needs you](02-concepts.md#what-needs-you)), and there's no badge at 0. While the window is hidden or minimized, the app still reads the queues every 15 seconds for the badge alone. It's there while the app runs; quitting Muster takes it away.

## The projects sidebar

**All** shows every project together. Below it is each project, with its numbers: an orange one for the tasks that need you, one in the Building colour for tasks in Building or Testing, "⏸ 2" in grey when some are [paused](02-concepts.md#paused), and the total, notes included. Click a project to see only its tasks. **+ Project** at the bottom opens [Settings](#settings) at [Projects](#projects). A project with more waiting in Your check than its [limit](#your-check) says so under its name, in orange: "7 in Your check, limit 5".

**Categories and labels.** The projects sit under their category's heading, A to Z with **Other** last, each heading with how many of its tasks need you, how many are being worked on, and its tasks. The chips above them (**Working**, **A–Z**, **Needs you**) order the projects within each heading. A project shows by its label; point at it for its real name and folder. Double-click a project's name to change its label right there: ↩ or clicking away saves it, Esc keeps it as it was, and the folder's name typed back means no label. Set labels and categories in [Settings › Projects](#projects).

Under a stalled project is **Ask all quiet sessions for a check-in**, with how many it can reach: it asks each quiet card's session, as the card's own button does. It's shown off, with why, when none of its quiet claims has a session to reach, and it says how many it can't reach and how many wait for input in Claude Code. A project counts as working while Claude Code says one of its sessions is working, and not while its only active session waits for input or has ended.

## The board

One column per stage, with the stage's role under its name. An empty column shrinks to a slim strip with its name written down it, so every stage stays in view even with the pane open.

Each card shows, top to bottom:

- the project (when you're looking at All), the id, a small dot while a session is working on it (its transcript changed in the last 5 minutes, the [LIVE window](#agents)), the type, and the priority: **HIGH** filled in magenta, normal outlined, low faint;
- the title, the feature or goal it's under, and its tags;
- who's on it and how long ago they started, what Claude Code says the session is doing when it can say (**working** with the Building dot, **waiting for input in Claude Code** in orange, which counts as needs you, or **session ended, task still in Building**; see [Claude Code's session states](02-concepts.md#claims-leases-and-check-ins)), their latest check-in ("60% · gate green, writing tests · 12 min ago"), and a warning when they've gone quiet ("quiet 12 h+" past the [claim lease](#agents), "no check-in for 45 min"). After you ask for a check-in, "asked 2 min ago" takes the check-in's place until one comes. In Your check: **Your turn** and the next step (see [Your check](#your-check));
- a paused task's grey **⏸ Paused** chip with when and why ("Paused 2 h ago · the design changes first"); a paused card is never quiet or red ([Paused](02-concepts.md#paused));
- under a quiet card, **Ask for a check-in**: it puts a request in the session's mailbox, and the answer hook hands it over after that session's next tool call. A claim with no session stamp (made before stamps, or outside Claude Code) can't be reached: the button is shown off, with why ("ask its session to re-claim"). While the session waits for input in Claude Code it's off too: "answer it there first";
- under every card from Backlog through Testing, **Pause**, and under a paused one **Resume**: one click, no dialog, always first in the same spot (a quiet text button, with the check-in ask after it on a quiet card). If its session can't be told, it says why beside it;
- "⚠ 2 vision" while the task has open [vision conflicts](02-concepts.md#the-vision-check);
- in Building and Testing, where its [review of record](02-concepts.md#the-review-of-record) stands: "review 1/3 · 2 to fix", "review 2/3 · 1 to re-check", "review clean · round 2", or "not reviewed yet" on a started task in Testing;
- badges: why it's blocked, the kind first ("Blocked on you: …", [Blocked, and why](02-concepts.md#blocked-and-why)); needs you; open questions; files; the estimate; and how long ago it last changed.

**Your own tasks** (for: human: an account, a secret, a decision, a try on a device) sit in the same columns as the agents' tasks: To do in Ready, Doing in Building, Done in Done. Their cards are hatched with faint gold stripes and carry a gold **You** ribbon, in every card look, so they can't be mistaken for the agents' work. What the agents wait on you for stays where it is, as a card marked **needs you**: in Your check, with a question escalated to you, or blocked on you or by a permission. An agent's task that waits on one of your unfinished tasks says "Blocked by tasks: 0041" until you've done it.

The **Sort** chips above the board order each column. The default, **priority**, puts cards that need you first, then high priority, then the lowest id, with Done newest finished first: each Done card says when ("done 4 min ago"). Done shows the latest 8, with **Show all** for the rest.

**Recently finished**, docked at the board's right edge, answers "what got finished lately, and when?" without a click. It lists what was **merged** (Testing passed and the work landed on main, so it's in Your check) and what was **accepted** (moved into Done; your own task's Done too), each a row of its own, newest first, under Today, Yesterday and then the date, each day with its count. Each row says when, from the log: "4 min ago", "2 h ago", "yesterday 18:20", then the date (point at it for the exact time). Under it are who built it (the session that took it into Building) and the time and tokens its sessions used, as in the pane's [Work](#the-task-pane) section. Click a row to open the task in the pane. The **All · Merged · Accepted** chips pick what it lists, and are remembered. When the window is too narrow for it beside the board (with the pane open, say), it moves under the board, still in view. It shows the last 7 days; **Show older** adds a week at a time and says how many more there are. With **All** projects selected, it mixes them, each row with its project. It follows the project you picked, not the filter bar, and lists only what's still finished: a task sent back from Your check drops out until it lands again.

The **Backlog** column starts with the **Add to the backlog…** field: type a line and press ↩ to add it as a note, or Esc to empty it. With more than one project in view, a "to myapp" chip says which project gets it; click it to pick another. Notes come first in the column, as quieter cards marked "note", then the numbered tasks in Backlog. With **Group by** on, the same field and notes lead the board in its Backlog band.

Above the board, the filter bar is shared by all four views (Board, Table, Timeline and Graph). Nothing in it folds away:

- **The search box** (⌘F from anywhere): words in any order, each found in a task's title, tags, text or holder, and a number or a key finds that task (`19` or `mu-19` finds mu-19). Matches are marked on cards and rows. Esc clears it, and Esc again leaves the box.
- **Views:** your saved views as chips; click one to bring back its search, filters and sort, ✕ to delete it. **+ Save view** names the current ones (↩ saves, Esc cancels).
- **Who** All · You · Agents (board, table and timeline): **You** shows only your own tasks, **Agents** only the agents', **All** both. The backlog's notes show under every choice: they're nobody's yet. The choice is remembered.
- **Group by** None · Feature · Goal (board, table and graph): one band per feature or goal, with its stage, its progress ("3 of 5 done"), the estimate left and a goal's due date, and the stage columns inside. The **Backlog** band comes first, "Backlog · not in a feature yet" (or goal): the **Add to the backlog…** field, every backlog note, then the Backlog tasks under no feature or goal. It's there even with nothing in it. Other tasks under none come last. Click a band's title to open the feature or goal.
- The estimate left for what's shown, and how many tasks are shown ("24 of 98 tasks"), then **?**, which opens this section.
- **Filter chips** by group: Stage, Type, Priority, Tags, Held by, Flags (blocked, paused, needs you, open questions), Captured, Changed and Done (today, this week, this month). Each group shows only the values the project's tasks have. Chips in one group mean any of them; groups together mean all.
- **Sort** (board only): priority, newest, oldest, changed, estimate or id.
- **The active line:** each search word and filter in use, with ✕ to remove it, how many tasks they hide, and **Clear all**. With none, it says "No filters: every task shows".

See [Find tasks and keep a view](03-tutorials.md#find-tasks-and-keep-a-view).

## The table

Every task as a row: the project (when you're looking at All), id, title, type, priority, stage, who's on it or what it waits on, the estimate, when it was captured, when it changed, and its tags. Click a column's header to sort by it, and again to reverse it. The Table remembers its sort, even after the app restarts. The search's words are marked in ids, titles and tags. Goals and features are rows too, with their progress and the estimate left. With Group by on, each band gets a heading row. Click a row to open it in the pane.

## The timeline

Each task's work as a bar across time, with **Scale** chips: **Day** is the last 24 hours in hours, **Week** is Monday to Sunday in quarter days, and **Month** is the calendar month in days. Week is the default, and the scale you pick is remembered. ◀ and ▶ step back or forward a day, a week or a month, and the button between them (**Now**, **This week** or **This month**) jumps back. The name of the period shown is beside them.

A bar starts when the work started, at the task's first move into Building or a later stage, and ends when the work finished, when it reached Your check or Done. It has one stretch for each stage it was in, in that stage's colour, from the times `muster` logged. While a task waits in Your check, a thin dotted tail runs on from its bar to now. Tasks still being worked run up to the vertical line, which is now. Red hatching is time spent blocked, grey hatching time paused (it isn't counted as work). A faded start means only the day the work started is known, as for older tasks. Point at a bar or a stretch for its dates, and click a row to open the task.

Tasks whose work hasn't started, or whose log has no times, aren't drawn; a line under the chart says how many. The search's words are marked in the rows. See [Read the graph and the timeline](03-tutorials.md#read-the-graph-and-the-timeline).

## The graph

What waits on what, per project (set with `muster set <id> depends …`, see [Depends and the graph](02-concepts.md#depends-and-the-graph)). An arrow points from a task to each task that waits on it. Click a task: what it waits on lights up as "needed first", what waits on it as "waits for it", and the rest dims. "ready" marks a task with nothing left to wait on. **Hide done** drops finished tasks, and tasks that aren't linked to any other are listed under the graph.

## Your check

Your check is where the agents' finished work waits for you. Each card there says what to do, so you don't have to open it to find out:

- **Your turn**, in orange: the task waits on you. Nobody else holds it.
- **The next step:** a question an agent escalated to you comes first ("Answer Q2: …"), then a block on you or by a permission ("Unblock: …"), then the first criterion only you verify (`— verify: owner`) that you haven't ticked, as an instruction: "Try: group the board by Feature and check a goal's progress", or "Check: …" when it isn't written as something you do. "· 2 more" says how many others are open. A long one is cut on the card; point at it to read all of it, or open the task. With nothing of yours open, it says **Nothing to try: Accept**.

The column is grouped by how much each task asks of you, always in this order, each group with its count:

- **Just accept:** none of your criteria are open, no question waits on you, and nothing is blocked on you. **Accept all** accepts each of them that isn't blocked, as Accept does: it goes to Done with the commit its merge recorded, and its log gets the line. The first click names them in place ("Accept 3: 0014, 0021, 0030?") and puts the focus on **Cancel**. **Yes, accept 3** does it; **Cancel** or Esc leaves them, and Esc there never closes the pane. It accepts only the tasks it named, and never a goal or a feature. One that changed in the meantime (a criterion opened, a question for you, a block, it moved on) stays where it is, and the line beside the button says which and why. That line stays, even when the column empties, until something new comes into Your check there, a task it couldn't accept changes (you ticked, answered or unblocked it), or you click Accept all again.
- **One look:** something to check in the app, a question to answer or a block to clear.
- **Needs a session:** an open criterion mentions a session, a terminal or a command, so you'll run something outside the app.

A criterion can set its own effort with `— effort: look` or `— effort: session`, before or after its `— verify: owner`. With Group by on, each band's Your check column is grouped the same way, and its Accept all takes that band's tasks. [Waiting for you](#waiting-for-you) groups its Your check tasks the same way.

**The limit.** Your review is what holds parallel work up, so each project has a limit on how many of the agents' tasks may wait in Your check: 5 unless you change it in [Settings › Projects](#projects). Your own tasks don't count. Past it, the column's count reads "7 of 5" in orange, a short line under its header says "Over your limit of 5", and the line above the board says "7 of 5 in Your check: check these before more start." Both lines always have their places and the one above the board never takes a second line (at the narrowest it's cut at the end; point at it for all of it), so nothing moves when they come. With **All** projects, each project is checked against its own limit: the column says how many are over ("2 projects over limit"), the line above the board says "Over your limit in 2 projects: …" (point at it for their names and counts), and each of those projects' rows in the sidebar says its own. With Group by on, each band's Your check keeps its own count, in orange when it holds tasks of a project over its limit. The sessions hear it too: `muster next`, `muster start`, `muster ready --parallel` and `muster list` print "muster: 7 tasks wait in Your check (limit 5): the owner's review is the bottleneck." It only warns: nothing is refused, and the commands still do their work. Accept all is the quick way back under it.

## The task pane

Click a card to open its task beside the board. The pane keeps every control in the same place for every task, from the top:

1. **The top line:** the project and how long ago the task changed, and **×** to close.
2. **The title**, with its id. Click the title to rename it: ↩ saves, Esc keeps the old one. The **?** beside it opens this section.
3. **The status line:** the stage, **Paused** with when and why, **Pause** (with a "why (optional)" box beside it; ↩ pauses too) or **Resume**, why it's blocked (the kind first; each task it's blocked by opens with a click), **LIVE** (the file changed in the last 5 minutes, the [LIVE window](#agents)) or how long it's been quiet, who's on it ("Your turn" in Your check) and who held the stages before, what Claude Code says the session is doing (working, waiting for input in Claude Code, session ended), the stage's role, and what's waiting on you, or "nothing waiting on you".
4. **The stage track** (off while the task is paused): all seven stages, the task's own lit; for your own task only **To do**, **Doing** and **Done**. Click another stage to put the task there. That's your override: it happens at once, and the message under it offers **Undo**. When the move would release someone's claim, the stage's tooltip says whose.
5. **Two columns.** The left one shows what this stage needs:
   - Backlog, Shaping and Ready: the acceptance criteria and the estimate.
   - Building and Testing: **Work so far**, the time spent against the estimate, the tokens estimated, and the stage history. The sessions and the tokens they used are in the **Work** section further down.
   - Your check: **Your checklist** of the criteria only you verify, and the history. The open ones come first, each with its effort (One look or Needs a session), **Tick**, and a button for each view it names (**Open Graph**, **Open Timeline**, **Open Appearance**…). A view opens beside the task, which stays open; **Open Appearance** and **Open Projects** open [Settings](#settings) on that group, and **Open Settings** on the group it showed last. Ticked ones follow, with **Untick**.
   - Done: the history.
   - A goal or feature: its tasks and their stages, its progress and the estimate left.

   The right one is the **Next step**: the one primary button, **Pass to** the next stage (**Accept** in Your check), then **Send back** and **Block** or **Unblock**. **Release** shows when the holder has been quiet past the [claim lease](#agents), 12 hours unless you changed it. If someone else holds the task in Building, there's no Pass: only they can pass it, and the pane says so. While it's paused, Pass is off until you resume it. Under the next step are the task's settings: **Priority** and **Type** show their value as a chip (click it to lay out the choices), **Est** has agent time, test time and tokens (↩ or leaving the field saves), then **Tags**, **Files** (images as thumbnails that open large; **+ Attach** picks more) and **Links** (a click opens one in your browser, https only).
6. **The stage forms**, under both columns, when one is open: the gate's result line for Pass to Testing, **Send back** with where to and why, or **Block** with what it waits on. The core's answer, or its refusal, shows under them.
7. **Session**, in Building and Testing while an agent's session holds the task: **Ask for a check-in** (Building; "asked 2 min ago" beside it until one comes) and a one-line box, **Message the session working on this**, with a count ("120/500": at most 500 characters, and Send is off past that), and **Send**. Both reach only the session in the holder's stamp, through its mailbox, after its next tool call, and both are logged on the task. The message arrives quoted as your note, not as instructions. When the session can't be reached, they're shown off, with why.
8. **The path line:** the commit that shipped it, and the task's file.

Below that come, in order:

- the **open questions**, each with a box to answer it (⌘↩ sends). A question asked with options shows every option as a chip, the agent's pick marked **pick** and what it does with no answer after them ("with no answer: a"). Click a chip to answer with that option, plus whatever is in the box; the box alone answers with something else;
- the **write-up**, with **Rendered**, **Raw** and **Edit**;
- **Vision**: where the [vision check](02-concepts.md#the-vision-check) stands ("not checked", "no open conflicts" or "2 open conflicts"), a **Resolve V1** button for each open conflict (say how it was settled, in one line), **Copy vision brief** for a session, and the findings the check recorded;
- **Review**: where the [review of record](02-concepts.md#the-review-of-record) stands ("not reviewed yet", "round 2 of 3: 1 open, 0 answered", "clean after round 2"), **Copy review brief** for a session that didn't build it (a task started on its own branch), and the findings with their state and both sides' words, then each round;
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

Every task waiting on you, in one list. At the top, **Lessons to approve** when reviews have proposed any: each lesson in full (the rule, bad and good, its kind, who it's for, its folders and the findings it came from), **Reject…** to turn one down with why, and **Accept all** for the ones shown, in one batch (one proposed since waits for your next look). Tasks in Your check come next, grouped as the [Your check](#your-check) column is (Just accept with **Accept all**, One look, Needs a session), each with its next step in full. Then **Questions, blocks and your own tasks**: questions escalated to you (with the question, and its options as chips that answer in one click), tasks blocked on you or by a permission, tasks whose session waits for input in Claude Code ("answer it there"), and your own tasks to do and doing. **Open** or **Open to answer** opens the task in the pane. The tab's count is the number of tasks and lessons waiting, the same needs-you count as the sidebar's and the Dock's.

## Activity and Problems

**Activity** lists what changed, newest first: commits to the queues, and task files that changed, with their stage and last log line. **Problems** lists what `muster check` finds wrong in the queues, such as an id used twice or a claim with no one behind it; the tab shows how many. Below them, marked **Vision**, are the Shaping tasks with no vision check yet. They're warnings, not problems, and their count on the tab is the quieter second number.

## The Helper

A chat that files tasks for you, per project. Tell it ideas, bugs and tasks, paste screenshots, and press ⌘↩ (or **Send**). It checks the queue for duplicates, reads the project to understand the code, files one task per point through `muster`, and asks in plain words when something's unclear.

The first time, pick which Claude account it uses. It runs your own Claude Code with that account's login, restricted to `muster` and reading, so it can't change files. What you write goes to Anthropic, as in any Claude Code session. The bar has the **Project**, **Change account**, **Stop** (while it's working), **New conversation**, and **?**, and names the account and the model it uses. Each reply shows "n steps" with the commands it ran folded inside, and what the turn cost. The account and the model are also in [Settings](#settings), under [Agents](#agents). See decision [0002](../decisions/0002-helper-runs-the-owners-claude-restricted.md).

## Help

This manual. The sidebar has the search box, then every page with its sections; the page you're on is lit. ◀ and ▶ at the top and bottom of a page go to the page before and after. Links between pages and sections move here; a link to another file in the repo says where it is. Click a command line in a grey block to copy it. A **?** anywhere in the app opens its section here, and the pane stays where it was.

## Settings

One place for how the app looks and works on this Mac: **Settings** in the top row. Settings is a view like the others, not a window. The groups are listed down its left side, each with what it holds: **Appearance**, **Display**, **Projects**, **Agents** and **About**. Click one and all of its settings show on the right, each with its current value and what it changes. Every change applies at once and is remembered after the app restarts. The **?** beside a group's name opens its section here. The manual isn't a group: it's the [Help](#help) tab in the top row. A folder in your home folder shows as `~/…`; point at it to see the full path, and copying it copies the full path.

### Appearance

- **Match the Mac's light or dark:** pick a dark theme and a light one, and the app follows the Mac. Off, one theme stays.
- **Theme:** Violet night, Violet day, Graphite, Midnight, Paper, Snow and High contrast.
- **Accent:** Violet, Teal, Rose, Gold, Green or Graphite. Building takes the accent colour, unless it would look like another stage's; then Building stays violet, and Appearance says so.
- **Text size:** 90%, 100%, 115%, 130% or 150%, for everything in the window, this manual too. ⌘+ and ⌘− step through the sizes and ⌘0 goes back to 100%; the size is remembered.
- **Cards:** how a card shows its colour (Stripe, Tint, Top bar, Dot or Outline), and whether the colour shows its **Stage** or its **Priority**. Blocked and "needs you" always take over the colour.
- **Back to Muster's violet** resets the theme and accent.

### Display

How much the app shows at once. Three modes, as chips, the current one lit; a pick applies at once and is remembered. Below them is a table of every place that differs between the modes, with what each mode does there, the current mode's column lit. ⌘. turns Focus on from any mode, and off again back to the mode you were in. The colours, the docked panes and where every control sits are the same in all three, and in every mode what waits on you keeps its counts (Waiting for you, the sidebar, a folded column's strip) and its badge on the card.

- **ADHD** (the default): everything on screen. Every filter group, saved view and Sort above the board; every choice of Who and Group by as a chip; every line on a card; the task pane's sections open; the sidebar with its order chips and activity lines.
- **Standard:** what most apps do. The filter groups and Sort fold behind one **Filters** chip, with the number of filters on; click it and they open under the bar. Who and Group by show their current choice as one chip; click it to lay the choices out in its place. A card shows its title, kind, priority, age and anything wrong (blocked, quiet, needs you, open questions), and what Claude Code says the session is doing; who's on it, the check-in, branch, tags, parent, estimate and files show on the selected card. In the task pane, **Vision**, **Work** and **Log and check-ins** fold to a header each, the Log's with how many lines and check-ins; click one to open it. What you typed in one stays when you fold it or change mode.
- **Focus:** only what needs doing now. The board keeps **Building**, **Testing** and **Your check** open; the other columns are thin strips with their names, their counts, how many need you and how many are blocked; click a strip to open that column until you leave Focus. With Group by, the Backlog band folds the same way, to its head with its counts. A card is its title, one status line (who's on it, or Your turn) and blocked, quiet, needs a tester, needs you or what Claude Code says the session is doing (working, waiting for input) when it is. The filter bar is one **Search and filters** button until ⌘F brings it out (**Hide** puts it away); it stays out while a search or filter is on, so nothing is hidden without saying so. The sidebar narrows to the category headings with their needs-you count, the project names, the dot for one being worked on now, **stalled** for one that's stalled (with **Ask all quiet sessions for a check-in** still under it), and the needs-you count. **Recently finished** folds to a thin strip beside the board with its name and count; click it to open it there, and **Fold** puts it back. A Done card still says when it was done. The task pane folds as in Standard.

### Projects

Every project Muster shows, with its label and **Category**, its **Prefix** (what its tasks' keys start with: `mu` in `mu-12`), folder, task count and **Gate**: the command a merge runs in a task's worktree (see [Run parallel sessions in worktrees](03-tutorials.md#run-parallel-sessions-in-worktrees)). A merge never runs without one, so a project with none says so, with the line that sets it: `muster projects set <project> gate "<command>"`. The command line is the one place to set it.

- **New queue…** picks a project's folder and makes its queue there, like `muster init`.
- **Add existing queue…** picks a queue kept anywhere else. Muster finds the ones at `~/playground/<project>/tasks` on its own, and one level deeper (`~/playground/<group>/<project>/tasks`), but not in folders starting with `.` or `_`, or in `node_modules`, `worktrees`, `target` or `dist`.
- **Project:** its label, what the app calls it everywhere: the sidebar, the cards and lists with All, Waiting for you, the task pane, the window's title and the tooltip of its tasks' keys. Up to 40 characters, saved as you type; Esc puts back what was there, and empty shows the folder's name. Under it, small, is the project's real name: `muster` commands, `--queue`, the queue and its folder keep that one.
- **Category:** the heading it goes under in the sidebar ("Apps", "Tools"), up to 24 characters, saved as you type. While you're in the field, the categories in use lay out under it as chips: click one to pick it. Empty puts it under its group's heading (the folder that holds a queue one level deeper, like `persianUtils`), else **Other**.
- **Hide** or **Show** a project, and **Remove** one you added.
- **Set** beside a prefix changes it: 2 to 4 lowercase letters that no other project has (`muster projects set <project> prefix <p>` does the same). Why one is refused shows under the field. The task files keep their ids, so nothing moves, but the old keys stop finding its tasks.
- **Your check:** the project's [limit](#your-check) with how many wait now ("limit 5 · 7 waiting", orange when over). **Off** turns the warning off; type a number from 1 to 50 and **Set** (or ↩) to change it, or to turn it back on. The board uses it at once. It's saved on this Mac in `projects.json` with the project's folder, beside its gate, so renaming the project keeps it, and two projects shown under the same name keep their own. `muster` reads the same file. A hidden project shows none.

Labels and categories are saved on this Mac in `projects.json` with the project's folder, never in the queue or the repo; a missing or broken entry shows the folder's name under **Other**. None of them deletes files. A project marked "not a queue any more" points at a folder that no longer has a queue's `README.md` and `todo/` folder.

### Agents

Claims come first, then the helper, whose accounts take a moment to check.

- **Claim lease** (12 h unless you change it): a claim in Building that nobody has heard from for longer than this can go back to Ready. The card then says "quiet 12 h+", the task pane offers **Release**, `muster check` lists it, and `muster reclaim` takes it. It can be from 1 hour to 7 days; under an hour would take tasks from sessions that check in every 30 minutes.
- **LIVE window** (5 min unless you change it): how recent a change counts as working now, for the card's dot (a session's transcript) and the task pane's **LIVE** (the task's file). From 1 minute to 1 hour.
- **Claude Code's session states:** one line under them says whether the app reads them ("on (2 accounts)", one per account the session stamps name) or "not available" and why (`claude` isn't installed, its answer didn't read, it took over 10 seconds…), or that nothing is read while no task in Building or Testing has a session stamp. When they aren't available, the cards show the check-in lines as before. See [Claims, leases and check-ins](02-concepts.md#claims-leases-and-check-ins).
- **The helper:** which Claude account [the Helper](#the-helper) uses, with each account's login (**Log in…** opens Terminal on that account's own login), and its **Model**: **Sonnet** unless you pick another, **Haiku** (quicker, costs less) or **Opus** (thinks hardest, costs most). The chip lights as soon as the choice is saved, and the helper uses it from its next message.

Type a span the way `muster reclaim --after` takes it: `30m`, `2h`, `1.25h` or `1d` (a day is 24 hours; decimals count as typed), then ↩ or **Save**. A value out of bounds, or one that isn't a span, is refused with why, and nothing changes. **Back to 12 h** and **Back to 5 min** go back to the defaults. Both are saved on this Mac in `settings.json` in the app's settings folder (see [About](#about)); the `muster` command reads the same file, so the app, `muster reclaim` and `muster check` agree. A missing or broken file means the defaults.

### About

- **Version:** the app's version.
- **Settings on this Mac:** the app's settings folder (`~/Library/Application Support/ai.sweetlemon.muster`) and what's in it: `projects.json` (the projects you added or hid, their labels, categories, gates and Your check limits), `settings.json` (the claim lease and the LIVE window) and `helper/` (the helper's account, model and conversations). Appearance, the display mode, text size, Group by, saved views and the pane's width are kept by the window itself.
- **What it reads:** the queues it found on its own (at `~/playground/<project>/tasks`, and one level deeper), then the ones you added (or that you added none), each with its folder, and the folders of the Claude Code session transcripts. All of it is read on this Mac and nothing is sent anywhere; only what you write to the helper goes to Anthropic, through your own Claude Code.

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
| Muted grey outline with ⏸ | Paused (a state, not a stage) |

Priority has colours of its own, never a stage's: high is magenta (a filled chip), normal a light slate (an outlined chip) and low grey (faint text). They colour the cards when Appearance's **Cards** is set to show priority. Tags and types are neutral, outlined labels: they never take a stage's colour, so a bug is marked by the word "bug", not by red. The rules are in [DESIGN.md](../DESIGN.md#the-colour-code).
