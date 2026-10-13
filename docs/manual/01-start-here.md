# Start here

Muster is a task list for work that AI agents do for you. You say what you want in one line. Agents turn that line into a task, build it, test it, and hand it back for you to try. You answer only the questions that only you can answer.

This page is the whole idea on one screen. The other pages go deeper: [Concepts](02-concepts.md) explains each idea, [Tutorials](03-tutorials.md) walks through real work step by step, and [In the app](04-in-the-app.md) explains every button.

## The problem

Coding agents can now build real features on their own. One person can have several of them
working at once. The hard part stops being the code and becomes the running of it: what each
agent is doing, what it's waiting for, whether its work is done, and which questions really
need you.

Run that with a chat window per agent, and the state lives in your head. You forget which
session had which task, you answer the same question twice, two agents start the same work,
and "done" means whatever the agent said.

Muster is where that work lives instead: every task, who's on it, what stage it's in, what it's
waiting on, what it cost, and what needs you. Agents and you read and change the same list,
under the same rules.

## Why an ordinary board doesn't work for agents

Jira, Linear and Trello are built for people. People remember yesterday, update a card when
they feel like it, ask a colleague when stuck, and rarely grab the same ticket. Agents are
different in each of those ways:

| A board for people assumes… | With agents… | So Muster… |
| --- | --- | --- |
| The person remembers the context. | Every session starts from nothing. | puts everything in the task: what and why, the criteria for done, the decisions, the log. |
| People move cards honestly. | An agent will say "done" when it isn't. | only lets the tool move a task, and only when the rule for that step is met: the checks' result to leave Building, every criterion ticked with its proof to leave Testing. |
| Two people rarely take the same ticket. | Several agents start at the same second. | makes a claim one locked step: exactly one wins, and a claim that goes quiet expires. |
| You can ask a colleague. | An agent can't walk over to your desk. | lets it ask in the task and keep working; the answer is pushed into its session. Only questions that are really yours reach you. |
| Effort is story points. | Effort is tokens and minutes. | records both, from the sessions' own transcripts, against the estimate. |
| You can see who's working. | Sessions are invisible. | stamps each step with the session that took it, and shows check-ins, "working now" and "stalled". |
| Tickets are written once and drift. | Agents write many tasks fast, and they can contradict each other. | checks every new task against the project's one picture (the vision check) and tells you what changing course would cost. |
| The board is a website. | Agents work with files and commands. | keeps tasks as plain files with their own git history, and gives agents a command line, `muster`. |

## The big picture

```
you: one line ──▶ shaping agents ──▶ coding agent ──▶ testing agent ──▶ you: try it ──▶ done
                  (task, criteria,    (claims it,      (ticks each
                   estimate, checks    checks in,       criterion
                   it fits the plan)   runs the gate)   with proof)
```

Your attention is the scarce thing, so Muster spends it carefully. Everything that needs you is
in one place (**Waiting for you**, the orange cards, the count in the sidebar), everything else
carries on without you, and you can see at a glance what is moving and what has stopped.

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
