---
covers: app/src-tauri/src/backlog.rs, app/src-tauri/src/lenses.rs, app/src/Backlog
---
# Shaping

- A backlog note is any `.md` in `.muster/backlog/` that isn't a task file; it has no id. (mu-15)
- Notes come from `muster add` or the Backlog column's quick add. (mu-15)
- An idea goes in the backlog; `muster new` and `set` refuse `type: idea`. (mu-17)
- Shaping is for product, tech and UI agents: criteria, type, parent, estimates. (decision 0003)
- `muster shape <note>` prints the brief and writes nothing. (mu-15)
- The brief quotes the note as data, not instructions. (PROTOCOL.md "Shaping lenses")
- `muster file <note> "<title>" --body -` makes the task in Shaping and moves the note to its attachments. (mu-15)
- A note is filed once, under the queue's lock. (mu-15)
- `muster drop <note> "<why>"` moves a note to `backlog/dropped/` with the reason. (mu-15)
- Every task answers ten lenses: accessibility, i18n and RTL, themes, payments, architecture, security, privacy, performance, tests, docs. (mu-76)
- Each lens line says what the task must do, or "n/a" and why. (mu-76)
- A project skips a lens only with a reason, in `.muster/config.toml`'s `[shaping]`. (mu-76)
- A skip with no reason, or of an unknown lens, doesn't count and `muster check` reports it. (mu-76)
- `muster file` warns about missing lens lines and files the task anyway. (mu-76)
- Set `touches:` at shaping, so the task can run beside others. (mu-23)
- `muster pass <id> --from shaping` needs acceptance criteria. (mu-15)
- It's refused while a vision conflict is open; overlaps and gaps never hold it. (mu-29)
- It's refused while a `[NEEDS CLARIFICATION: …]` marker is left or a question without a default is open; `muster file` warns. (mu-103)
- A shaping run's questions go to the owner once, at its end: `muster clarify --send`. (mu-103)
- The brief shows the estimates, measured on this queue: actual ÷ `est-time` at p50/p80 and median hours, overall, by type and by size; under 5 tasks says "too few (n)". (mu-168)
- `--time` is agent time (first claim to leaving Testing, review and QA included, blocked and paused left out); `--test-time` is the owner's check only; the wait in Your check is neither. (mu-168)
- `--tokens` counts input, output and cache writes, never cache reads; `--time` is agent hours (1d = 8 h). (mu-172)
- `muster new`, `file` and `edit` refuse a new `— verify: owner` criterion unless `--as owner`: make it provable, or `qa: walkthrough`. A hand edit of the task file isn't checked. (mu-132)
