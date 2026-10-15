---
covers: app/src/Views.tsx, app/src/viewData.ts, app/src/GraphView.tsx, app/src/graph.ts, app/src/Legend*
---
# The Table, Timeline and Graph views

- Board, Table, Timeline and Graph share one filter bar and open a task in the docked pane. (mu-32)
- The Timeline's scale strip stays put above the scrolling chart. (mu-31)
- A bar runs from the claim to Your check or Done, from the times muster logged, one stretch per stage. (mu-31)
- The Timeline's legend is the strip's second line, always in view; too long for one line, it wraps to a second, never scrolls. (mu-173)
- A legend always keeps the height of every item it can name at this width, so stepping windows never moves the chart. (mu-173)
- The Graph's bar and legend are a strip outside the scrolling graphs, always in view. (mu-173)
- Every done bar is the Done outline; a task done before stage history has no segment, as no stage is known. (mu-173)
- A legend names only what the view draws, in a fixed order, so a new period moves nothing. (mu-173)
- Every swatch is drawn with the view's own classes; legend CSS sets only size and place. (mu-173)
- Legend names are the i18n layer's `board:legend.*`; nothing from a task file reaches the legend. (mu-173)
- The Graph's legend follows Hide done; a loop is the one rule `isLoopEdge`, for drawing and legend. (mu-173)
- One colour, one meaning: a stage colour only ever says a stage. (DESIGN.md "The colour code")
