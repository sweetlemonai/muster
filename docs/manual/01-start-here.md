# Start here

Muster is a task list for work that AI agents do for you. You say what you want in one line. Agents turn that line into a task, build it, test it, and hand it back for you to try. You answer only the questions that only you can answer.

This page is the whole idea on one screen. The other pages go deeper: [Concepts](02-concepts.md) explains each idea, [Tutorials](03-tutorials.md) walks through real work step by step, and [In the app](04-in-the-app.md) explains every button.

## Two ways in, one set of rules

You use Muster in two places:

- **The Muster app**, a Mac app with a board of every task, a pane that shows one task in full, and a Help tab (this manual).
- **The `muster` command**, which you type in a terminal, or in a Claude Code session as `! muster …`. Agents use it too.

Both are the same program underneath, so they follow the same rules and can't disagree. An agent can't skip a rule, such as passing a task to Testing without the result of its checks. You can override a stage when you need to, and the override goes in the task's log.

## The board

The board has one column for each **stage**, the step a task has reached. A task moves left to right:

```
Backlog → Shaping → Ready → Building → Testing → Your check → Done
```

Each task is a card in its column. The card says what the task is, who is working on it, and whether it needs you. Cards that need you are orange and sort to the top. See [The board](04-in-the-app.md#the-board) for everything on a card.

## Who does what

Each stage has someone whose job it is:

| Stage | Who works it | What happens there |
| --- | --- | --- |
| Backlog | anyone | A one-line idea, not a task yet. |
| Shaping | product, tech and UI agents | The idea becomes a task: what done looks like, an estimate, a type. |
| Ready | nobody yet | Finished shaping, waiting for an agent to take it. |
| Building | a coding agent | It writes the code and runs the checks (the gate). |
| Testing | a testing agent | It checks every acceptance criterion and ticks each one with its proof. |
| Your check | you | You try it yourself, tick what only you can judge, and accept it. |
| Done | nobody | Finished, with the commit that shipped it. |

An agent here is a Claude Code session working on your project. You can do any step yourself as well: the app and the `muster` command let you move a task anywhere.

## One task, start to finish

Here is a small bug going all the way through:

1. You notice the board flickers when a pane opens. You type "Board flickers when the pane opens" into the Backlog column's field. It's now a **backlog note**.
2. A shaping session reads the note and the project, and files it as task 0012 in Shaping, with **acceptance criteria**: the checks that say it's done. One says "no flicker when the pane opens". Another is marked for you: "it looks steady on the Mac".
3. The task passes to Ready. A coding session takes it with `muster next`. That's a **claim**: the task is in Building and the card shows who has it.
4. The coding session fixes the bug, runs the project's checks, and passes the task to Testing with the checks' result line.
5. A testing session confirms the flicker is gone, ticks that criterion with its proof, and passes the task to Your check.
6. The card turns orange: it needs you. You open it, try it on your Mac, tick "it looks steady", and click **Accept**. It's Done.

You did two things: wrote one line and tried the fix. If an agent had a question only you could answer, it would have shown up under **Waiting for you**, and the agent would have kept working on the rest meanwhile.

## Where to go next

- New to Muster: read [Concepts](02-concepts.md), then try [your first project](03-tutorials.md#your-first-project).
- Looking for a button: [In the app](04-in-the-app.md).
- Looking for a command: [Commands](05-commands.md). Keys: [Shortcuts](06-shortcuts.md).
- Setting up an agent: [For agents](07-for-agents.md).
- Something's wrong: [FAQ and troubleshooting](08-faq.md).
