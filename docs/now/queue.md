---
covers: app/src-tauri/src/queue.rs, app/src-tauri/src/index.rs, app/src-tauri/src/key.rs, app/src-tauri/src/layout.rs, bin/manifest.py, app/src-tauri/src/ops.rs
---
# The queue and its files

- A repo's queue is its `.muster/`, and every worktree uses the main checkout's. (PROTOCOL.md "Where tasks live")
- A local queue, the default, keeps all but `config.toml` out of git. (PROTOCOL.md "Where tasks live")
- A local queue keeps a history of every write in `.git/muster/history.git`; `muster restore` puts files back. (mu-109)
- A file deleted on purpose is recorded with `muster history removed`; a removed task's id is never given again. (mu-109)
- The queue lives in local files, not GitHub Issues. (decision 0010)
- A task file never moves; `stage:` in it is the only truth. (decision 0003)
- `INDEX.md` and `tasks.json` are generated, never edited, never in git. (PROTOCOL.md "INDEX.md")
- After a hand edit to a task file, run `muster index`. (mu-18)
- Only the tool numbers tasks; a task is typed by its key, `mu-12`, its file keeps 4 digits. (mu-33)
- A prefix is 2 to 4 lowercase letters that no other project has. (mu-33)
- Something new goes in the backlog: `muster add "<one line>"`, a note with no id. (mu-15)
- Agents build only tasks; a goal's or feature's stage comes from its children. (mu-16)
- Agents never claim a `for: human` task, and only the owner makes one. (mu-22)
- Comments are written only by `muster comment`; Edit leaves them out. (mu-17)
- While a session holds a task in Building, the owner adds a note, not an edit. (mu-20)
- Questions go through `muster ask`; the answer reaches the asking session's mailbox. (PROTOCOL.md "Questions and answers")
- An answer to a question with a `--no-answer` default reaches whoever holds the task: the claim, `held-by` in Testing and Your check; the asker only when nobody does. (mu-103)
