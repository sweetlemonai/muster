---
covers: app/src/Sidebar.tsx, app/src/SidebarProjects.tsx, app/src/ProjectList, app/src/projectOrder, app/src-tauri/src/arrange.rs, app/src-tauri/src/labels.rs, app/src-tauri/src/projects.rs
---
# The projects sidebar

- The sidebar lists All, the queues found under `~/playground` and ones added by hand. (AS_BUILT "Projects sidebar")
- Discovery looks two levels deep, and at `.muster/` only in main checkouts. (AS_BUILT "Projects")
- A row shows its tasks, needs you and "N working", which counts tasks everywhere. (mu-197)
- Working means a live session, or a check-in or heartbeat in the last 30 minutes. (mu-37)
- Stalled means work in Building, not blocked, and nothing in the project for 30 minutes. (mu-37)
- Order chips: Working (the default), A–Z, Needs you, and Mine after a drag. (mu-57)
- Projects and headings drag to reorder; ⌥↑ and ⌥↓ move a focused row. (mu-57)
- A label is at most 40 characters, a category 24; commands keep the queue's name. (mu-44)
- A project's colour is one of five, kept apart from every state colour. (mu-57)
- A heading folds to one line; nothing of its projects shows under it. (mu-70)
- A project put under a folded heading unfolds it. (mu-70)
- A heading's counts are words, "77 need you · 17 projects · 18 working", open or folded. (mu-82)
- Names and headings sort in the language's order (Persian order in فارسی), not by code point. (mu-151)
- "N need you" comes first after the name and is never cut. (mu-82)
- Short of room, they take a full-width second line, then words wrap. (mu-212)
- In Focus the sidebar narrows to headings, names, the working dot, stalled and counts. (DESIGN.md "Focus mode")
- A stalled project keeps Ask all quiet sessions for a check-in. (mu-39)
- One builder makes the sidebar's state, in the core. (mu-73)
- Labels, categories, Hide and Remove never touch the queue's files. (AS_BUILT "Projects")
