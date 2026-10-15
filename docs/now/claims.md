---
covers: app/src-tauri/src/session.rs, app/src-tauri/src/pause.rs
---
# Claims

- Claim with `muster next` or `muster claim <id>` from inside the session so it carries the stamp. (PROTOCOL.md "Sessions")
- One claim per holder (the owner may hold several): a second unblocked task in Building is refused. (mu-2)
- A task sent back from Testing goes to its builder and comes first, even beside another claim. (mu-66)
- Claims are checked under the queue's lock: of two at once, one wins. (mu-2)
- muster stamps the holder (label, checkout, session, account); don't type it. (mu-30)
- A holder is one label in one session and one agent; a teammate passes `--agent <id>`. (mu-90)
- `--role coder` on claim, next or start hands out the role's lowest free slot (`Kaveh-2`); act as it with `--as Kaveh-2` from then on. (mu-112)
- One session is one slot: beside a slot's claim it holds no second Building task under any label, and takes no other slot while its own is in Building or Testing. (mu-112)
- After a rename a slot's old name still acts as that slot. (mu-112)
- The claim lease is 12 h by default, set from 1 h to 7 d in Settings. (AS_BUILT "Settings")
- Check-ins, notes, comments, asks, passes, returns and blocks renew the lease. (mu-2)
- `muster reclaim` returns Building claims quiet past the lease to Ready; nothing runs it on a timer. (AS_BUILT §7)
- Told to pause: stop at a safe point, check in, end the turn, keep the claim; its lease doesn't run. (mu-45)
- The successor runs `muster claim <id> --takeover --as <same name>`, then the old inbox. (PROTOCOL.md "Sessions")
- Without a handoff, a takeover waits for 30 quiet minutes; the owner may always take over. (mu-90)
- Nobody takes over a paused claim. (mu-90)
- A takeover brings that task's mail and repoints its questions. (mu-90)
- A bare claim is stamped by its holder's first act from a session, logged once "(stamped)". (mu-184)
- The pane's Release lets the owner free a claim or Testing hold with no session to reach; Building keeps its worktree. (mu-184)
