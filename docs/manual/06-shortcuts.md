# Shortcuts

Every keyboard shortcut in the app, and the slash commands for Claude Code. This page is made from the app's own table of shortcuts, and a test checks every key the app reacts to against that table, so a shortcut can't exist without being listed here. ⌘ is the Command key. Ctrl works too wherever ⌘ is shown, except ⌘F, ⌘+, ⌘− and ⌘0, which take ⌘ only. ↩ is Return.

## Anywhere

| Keys | What it does | Where |
| --- | --- | --- |
| ⌘N | Open New task in the pane beside the board | Anywhere in the window |
| Esc | Close the pane: a task, a note or New task | Anywhere while the pane is open, except in the search box or a view's name, which take Esc for themselves |
| ⌘F | Put the caret in the search box, with its words selected so typing replaces them. Ctrl+F is left to text fields, where it moves the caret | Anywhere in the window. From a tab without a search box, it opens the board first |
| ⌘+ | Make everything larger: the board, the panes and the manual (the size is remembered; also in Settings, Appearance, Text size) | Anywhere in the window |
| ⌘− | Make everything smaller | Anywhere in the window |
| ⌘0 | Back to 100% | Anywhere in the window |
| ← → | Widen or narrow the pane by 40 pixels (the width is remembered) | The pane's edge, once Tab has put the focus on it |

## Search and views

| Keys | What it does | Where |
| --- | --- | --- |
| Esc | Clear the search, or leave the box when it's already empty. It never closes the pane | The search box above Board, Table, Timeline and Graph |
| ↩ | Save the search, filters and sort as a view under the name you typed | A view's name, after + Save view |
| Esc | Stop naming the view; nothing is saved | A view's name, after + Save view |

## Writing and sending

| Keys | What it does | Where |
| --- | --- | --- |
| ⌘↩ | Create the task, send the answer or the comment, file the note, or send to the helper | New task's description, an answer box, the comment box, a note's File as task form, the Helper |
| ⌘S | Save the write-up | A task's write-up, after Edit |
| Esc, twice | Leave the editor or empty the comment box. With typed text, the first Esc only warns and the second throws the text away | A task's write-up after Edit, and the comment box |

## The task pane

| Keys | What it does | Where |
| --- | --- | --- |
| Esc | Stop renaming and keep the old title (↩ saves the new one) | A task's title, after clicking it |
| Esc | Fold the choices back without changing the value | Priority and Type, while their choices are laid out |
| ↩ | Save the estimate (leaving the field saves it too) | The Est fields: agent time, test time, tokens |
| ↩ or , | Add the tag you typed | The tag field, in a task or in New task |
| ← → | Show the previous or next image | A task's image, opened large |
| Esc | Close the large image | A task's image, opened large |

## The board

| Keys | What it does | Where |
| --- | --- | --- |
| ↩ | Add the line to the backlog as a note | Add to the backlog…, at the top of the Backlog column |
| Esc | Empty the field | Add to the backlog…, at the top of the Backlog column |
| Esc | Don't accept them: nothing changes | Accept all's confirm, in Your check's Just accept group (the board and Waiting for you) |

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
