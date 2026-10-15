---
covers: app/src-tauri/src/ops.rs, app/src-tauri/src/block.rs, app/src-tauri/src/queue.rs
---
# Stages, passes and blocks

- Stages: Backlog, Shaping, Ready, Building, Testing, Your check, Done. (decision 0003)
- Blocked and paused are flags on a stage, not stages. (AS_BUILT §1)
- Only `muster` writes `stage:`, `claimed-by:`, `heartbeat:`, `blocked-kind:` and `waiting_on:`. (PROTOCOL.md "Stages")
- Hand a task on with `muster pass <id> --from <the stage you saw>`; it refuses if the task moved. (PROTOCOL.md "Stages")
- Building to Testing needs the gate's line and a one-line summary, 160 characters at most. (mu-78)
- Building to Testing is refused while a now page the task covers is unchanged on its branch, unless `--now-unchanged "<why>"`; the log names the pages. (mu-102)
- Testing to Your check needs every criterion ticked except the owner's. (PROTOCOL.md "Stages")
- In Testing, a session that didn't build the task records its gates with `muster gate <id> criteria|visual --record -`. (mu-131)
- `muster start` writes `gates: on`; `qa:` (reference, walkthrough or both) is set at shaping; once the task is started or from Building on, only the owner changes it. (mu-131)
- The `## Gates` section and `gate-records:` are the tool's; an edit can't add a `## Gates` line once a record exists. (mu-131)
- `muster return <id> "<why>"` sends a task back; Building goes to its last builder. (AS_BUILT "Stages")
- A send-back gives Building to its builder's exact session; a builder with no session gets its label. (mu-184)
- Until that session acts on it, and while it holds no other unblocked Building task, `next` sends a session under its label there and `claim --takeover` takes it at once; not while paused. (mu-184)
- A builder that acted as a role slot gets its task back with its exact stamp, slot and all. (mu-112)
- A block has a kind: tasks, you, permission, question or outside. (mu-24)
- Tasks is never a flag: the ids go into `depends:`, which clears once they're merged. (mu-24)
- A depend on an agent's task is met in Your check or Done; on a human task only at Done. (mu-42)
- You, permission and outside stay until `muster unblock`; a question clears when answered. (mu-24)
- A paused task can't be claimed, passed or moved by anyone until resumed. (mu-45)
