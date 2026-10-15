---
covers: app/src/App.tsx, app/src/HeadCount.tsx
---
# The header's toolbar

- The toolbar never clips: what doesn't fit wraps to another line, nothing is hidden, scrolled or folded into a menu. (mu-202)
- Two units in DOM order, which is the reading and Tab order: New task and the views, then Help, Settings, the account meter, the display mode and live. (mu-202)
- New task and the views never split: the views wrap pill by pill beside New task, never under it. (mu-202)
- Help, Settings, the meter, the mode and live drop to the next line together, at its inline end (the left in right to left); only when they alone are wider than the header do they wrap inside, the status under Help and Settings. (mu-202)
- Help and Settings are their own navigation landmark, "Help and Settings", beside the "Views" one. (mu-202)
- Every count in it (Waiting for you, Problems, and Problems' quiet count of unchecked visions) keeps three digits' room, empty and hidden at zero, so a poll that changes them never changes the toolbar's lines or the board's top. (mu-202)
- A pill's shown counts come first and its empty slots after them, so no blank sits between a label and its count. (mu-202)
- Where it wraps depends on the window's width, the language, Text size and how many accounts the meter shows, never on a poll's data. (mu-202)
- Nor on the meter's readings: the meter keeps its width as readings change, each account's status holding the room of its widest words in the language shown, so a reset, a stale window or a new reading never re-wraps the toolbar or moves the board's top. Pausing an account, like adding one, may. (mu-216)
- Pure CSS, logical sides only: no measuring, no resize script, no state. (mu-202)
