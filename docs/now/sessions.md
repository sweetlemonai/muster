---
covers: app/src-tauri/src/sessions.rs, app/src-tauri/src/transcripts.rs, app/src/Sessions, app/src/SessionRecap.tsx, app/src/sessionsData.ts
---
# The Sessions view and its recap

- It lists the sessions of the allowed accounts only, never `~/.claude`. (decision 0012)
- Three groups, always on screen in one order: Needs input, Working, Completed. (mu-98)
- An empty group says "none" and keeps its place. (mu-98)
- Completed older than 24 h folds into one Older chip that opens in place. (mu-98)
- Needs input is the needs-you orange; Working and Completed stay neutral. (DESIGN.md "The Sessions view")
- Each row is one line: name, project under All, message, age at the inline end. (mu-98)
- Names and messages are plain text with `dir="auto"`, never Markdown. (decision 0017)
- `cwd` is matched to a project in the core; only the name reaches the window. (decision 0017)
- Nothing about sessions is written to queue files, logs or disk. (decision 0017)
- The list is read off the main thread, at most every 15 s, and only while the view polls. (mu-98)
- A click opens the recap in the pane's dock, never the transcript first. (mu-98)
- The recap's sections keep one order: chain, Who, Tasks, History, Time, Tokens, Progress. (mu-98)
- The view's and recap's words follow the language shown; the core's own state words and history stay as the core wrote them. (mu-147)
- The recap holds counted tokens (input, output, cache writes) against `est-tokens`; cache reads are shown apart, never counted. (mu-172)
- A handoff is read only as a regular file directly in `.claude/work/handoffs/` under `~/playground`, at most 256 KB, front matter only. (decision 0017)
- Transcripts are read only under `~/playground` project keys of the allowed accounts. (decision 0012)
- A transcript that only grew (same inode, same first 4 KB) is read on from its last line end; anything else is read in full. (mu-210)
