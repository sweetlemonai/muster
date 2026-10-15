---
covers: app/src/App.tsx, app/src/TaskPanel, app/src/StageTrack.tsx
---
# The task pane and the header

- No overlays: a task, a note and New task open in a pane docked beside the board, which stays live. (DESIGN.md "ADHD mode")
- Account meter: [accounts](accounts.md). (mu-113)
- The header's toolbar and how it wraps: [toolbar](toolbar.md). (mu-202)
- Modals only to confirm something destructive. (DESIGN.md "ADHD mode")
- A field shows its value as one chip; a click lays out the options in place. (DESIGN.md "ADHD mode")
- One primary action per place, the next step. (DESIGN.md "Out of the way")
- The pane leads with title and status, then the stage track, then the stage's work beside the next step. (mu-21)
- The stage track refuses a move if the task moved since. (mu-21)
- Only one action runs at a time. (AS_BUILT "Board")
- The app never claims a task; its changes are logged as "owner". (AS_BUILT "What the app writes")
- Edit is refused while someone else holds the task in Building. (mu-20)
- A claim with no session to reach has Release in the Next step, with its confirm in place, no modal. (mu-184)
- Files open at the pane's full width, no lightbox. (mu-91)
- Sessions is a project view beside Graph; its recap opens in the pane's dock, the list still live. (mu-98)
- The pane's words in every language, and task text in its own direction: [pane-words](pane-words.md). (mu-148)
