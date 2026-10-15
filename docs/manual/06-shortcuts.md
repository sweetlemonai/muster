# Shortcuts

Every keyboard shortcut in the app, and the slash commands for Claude Code. This page is made from the app's own table of shortcuts, and a test checks every key the app reacts to against that table, so a shortcut can't exist without being listed here. ⌘ is the Command key. Ctrl works too wherever ⌘ is shown, except ⌘F, ⌘+, ⌘− and ⌘0, which take ⌘ only. ⌥ is the Option key. ↩ is Return.

## Anywhere

| Keys | What it does | Where |
| --- | --- | --- |
| ⌘N | Open New task in the pane beside the board | Anywhere in the window |
| Esc | Close the pane: a task, a note or New task | Anywhere while the pane is open, except in the search box or a view's name, which take Esc for themselves |
| ⌘F | Put the caret in the search box, with its words selected so typing replaces them. Ctrl+F is left to text fields, where it moves the caret | Anywhere in the window. From a tab without a search box, it opens the board first |
| ⌘. | Turn Focus on, or back off to the mode you were in (the display modes are in Settings, Display) | Anywhere in the window |
| ⌘+ | Make everything larger: the board, the panes and the manual (the size is remembered; also in Settings, Appearance, Text size) | Anywhere in the window |
| ⌘− | Make everything smaller | Anywhere in the window |
| ⌘0 | Back to 100% | Anywhere in the window |
| ← → | Move the edge of the pane or the projects sidebar 40 pixels: → to the right, ← to the left (the width is remembered) | The edge of the pane or the projects sidebar, once Tab has put the focus on it |
| Shift ← → | The same in steps of 120 pixels | The edge of the pane or the projects sidebar, once Tab has put the focus on it |

## Search and views

| Keys | What it does | Where |
| --- | --- | --- |
| Esc | Clear the search, or leave the box when it's already empty. It never closes the pane | The search box above Board, Table, Timeline and Graph |
| ↩ | Save the search, filters and sort as a view under the name you typed | A view's name, after + Save view |
| Esc | Stop naming the view; nothing is saved | A view's name, after + Save view |

## Writing and sending

| Keys | What it does | Where |
| --- | --- | --- |
| ⌘↩ | Create the task, send the answer or the comment, file the note, send to the helper, or send a task back | New task's description, an answer box, the comment box, a note's File as task form, the Helper, and the task pane's Send back form |
| ⌘S | Save the write-up | A task's write-up, after Edit |
| Esc, twice | Leave the editor or empty the comment box. With typed text, the first Esc only warns and the second throws the text away | A task's write-up after Edit, and the comment box |

## The task pane

| Keys | What it does | Where |
| --- | --- | --- |
| Esc | Stop renaming and keep the old title (↩ saves the new one) | A task's title, after clicking it |
| Esc | Fold the choices back without changing the value | Priority and Type, while their choices are laid out |
| ↩ | Save the estimate (leaving the field saves it too) | The Est fields: agent time, test time, tokens |
| ↩ or , | Add the tag you typed | The tag field, in a task or in New task |

## The board

| Keys | What it does | Where |
| --- | --- | --- |
| ↩ | Add the line to the backlog as a note | Add to the backlog…, at the top of the Backlog column, or of the Backlog band under Group by |
| Esc | Empty the field | Add to the backlog…, at the top of the Backlog column, or of the Backlog band under Group by |
| Esc | Don't accept them: nothing changes | Accept all's confirm, in Your check's Just accept group (the board and Waiting for you) |

## Projects

| Keys | What it does | Where |
| --- | --- | --- |
| ↩ | Save the label or the category (leaving the field saves it too; in Settings it's saved as you type) | A project's label or category in Settings, Projects; in the sidebar, a project's name after a double click, a heading's name after a double click or ↩, and + Category's field |
| Esc | Keep the label or the category as it was before you started typing | A project's label or category in Settings, Projects; in the sidebar, a project's name after a double click, a heading's name after a double click or ↩, and + Category's field |
| ⌥↑ | Move the project or the heading up one place, its projects with it. A project at the top of its heading goes to the end of the one above. The first move picks the Mine order | A project or a category heading in the sidebar, once Tab has put the focus on it |
| ⌥↓ | Move the project or the heading down one place. A project at the bottom of its heading goes to the top of the one below | A project or a category heading in the sidebar, once Tab has put the focus on it |
| ↩ | Rename the heading in place (a double click does too). A name another heading has puts the two together | A category heading in the sidebar, once Tab has put the focus on it |
| ← | Fold the heading to its name and counts (its chevron does too); the selected project and the ones that need you still show on it | A category heading in the sidebar, once Tab has put the focus on it |
| → | Unfold the heading | A category heading in the sidebar, once Tab has put the focus on it |
| Esc | Fold the colours back without changing the project's colour | A project's colours in the sidebar, after a click on its dot |

## Slash commands in Claude Code

`/mu` (or the long form, `/muster`) runs one `muster` command from inside a Claude Code session. The first word picks what it does, and anything that isn't a route is captured as a backlog one-liner, never guessed. Routes for the owner answer in plain words; routes for agents and the system print what `muster` printed.

The router is built (task 0003) but not installed yet: it goes into the personal layer once the owner says yes. Until then, type `! muster …` in a session, as [For agents](07-for-agents.md#in-a-claude-code-session) shows.

| Route | For | What it does |
| --- | --- | --- |
| `/mu` | owner | What's waiting for you, then the queue in one line |
| `/mu <id>` | owner | One task in plain words |
| `/mu waiting` | owner | Only what's waiting for you |
| `/mu add <text>` | owner | Put a one-liner in the backlog |
| `/mu answer <id> Q<n> <text>` | owner | Answer a question; it reaches the agent that asked |
| `/mu <anything else>` | owner | Captured as a backlog one-liner, never run as a command |
| `/mu ask <id> <question>` | agent | Ask without stopping; the answer is pushed back |
| `/mu claim <id>` | agent | Claim a Ready task |
| `/mu next` | agent | Claim the top Ready task you may take |
| `/mu heartbeat <id>` | agent | Keep a claim alive |
| `/mu note <id> <text>` | agent | A line in the task's log |
| `/mu pass <id> …` | agent | Hand the task on to the next stage |
| `/mu return <id> <why>` | agent | Send it back a stage, with the reason |
| `/mu block <id> <on what>` | agent | Mark it blocked, and why: tasks, you, a permission, a question or outside |
| `/mu unblock <id>` | agent | Clear the blocked flag |
| `/mu list [--stage <s>]` | system | Tasks by stage |
| `/mu check` | system | Problems in the queue |
| `/mu help [<command>]` | system | The routes, then muster's own help |
