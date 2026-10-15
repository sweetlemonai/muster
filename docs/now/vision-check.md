---
covers: app/src-tauri/src/vision.rs, app/src-tauri/src/now.rs, app/src-tauri/src/backlog.rs, app/src/Vision
---
# The vision check and now pages

- Before Ready, an agent checks the task against `docs/VISION.md`, `docs/decisions/` and `docs/DESIGN.md`. (mu-29)
- `muster vision <id>` prints the brief; `--record -` stores the result. (mu-29)
- A task never checked isn't held back; `muster check` warns about it. (mu-29)
- Settle a conflict with `muster vision <id> --resolve V2 "<how>"`. (mu-29)
- A conflict that changes direction asks the owner one question, with the cost of pivoting. (mu-29)
- "n/a" for a lens in VISION.md's `**Lenses needed:**` line is recorded as a conflict. (mu-76)
- Never edit the Vision section or `vision:` by hand. (mu-29)
- A now page is `docs/now/<area>.md`: `covers: <globs>` in its front matter, one rule per line, each citing its task or decision. (mu-102)
- A task's now pages are those whose `covers:` overlap its `touches:`, by the `touches:` rule. (mu-102)
- The vision brief quotes the task's now pages as data; a task without `touches:` gets their names only. (mu-102)
- Breaking a now page's rule unsaid is a vision conflict quoting the page's line. (mu-102)
- `muster shape` names the now pages its matched open tasks cover. (mu-102)
- `muster check` warns about a page over 60 lines or 4 KB, without `covers:`, sharing another's, or a symlink; never fails. (mu-102)
