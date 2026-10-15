# Design rules for Muster's screens

The owner has ADHD. **Out of sight is out of mind:** an option, a task or a status that's hidden
gets forgotten. Every screen is designed so the owner sees what exists and what needs them without
looking for it. These rules win over tidiness and minimalism.

**The trick is to show everything without it getting in the way** (the owner's words). Hiding is
not the way to reduce clutter here; weight, size and placement are.

Muster is meant to be sold, and not everyone wants everything on screen, so it has three display
modes (task 0038, Settings › Display, ⌘. for Focus): **ADHD**, the default and the owner's, and
**Standard** and **Focus**. The rules under "Everything visible" are ADHD mode's; Standard and Focus
have their own short sets below. Every other section here applies in all three.

## In every mode

- **Fold in place, never away.** A mode only says how much of an element shows. What it folds
  leaves a labelled chip or header in the same spot (a board column's chip is in the row above the
  board, mu-84), with a count where there is one, and one click or key opens it there. Nothing a
  mode folds is more than one click or key away.
- **What needs you never folds:** the Waiting for you count, the sidebar's counts, the needs-you
  and blocked words on a card, and a folded column's needs-you count. A folded sidebar heading is
  one line (two at the narrowest sidebar), and its own needs-you count carries what's under it
  (owner, 2026-10-14, task 0082): it comes first after the name, in the needs chip a project's row
  has with its words, and is never the part cut where the sidebar is narrow. Nothing of its projects shows under it.
- **No overlays, fixed places, no layout jumps, the colour code:** as below, the same in all three.
  Switching modes moves nothing: every control keeps its place.
- **One table:** `app/src/modes.ts` lists every element that differs, with what it does in each
  mode and how to get back what it folds. Components read it and nothing else; a test fails when
  a component reads an element the table doesn't have, or reads the mode around it.

## The board's columns (tasks 0083 and 0084)

Decided 2026-10-14. First (0083), after the owner's "mess in board UI" on a board with one full
column (Ready 66) and the rest empty: the lone open column stretched its cards into one long line
each, and the strips didn't line up. Then (0084), with the strips lined up: "the board ui for
closed and empty is still ugly". Of three designs (A, a chip row; B, short stubs; C, empty columns
stay open) the owner chose **A**.

- **One row of chips above the board** (`ColumnRow.tsx`), under the filter bar. Every place of the
  board that isn't open is a chip there, in board order: the Paused lane, the stages, then
  Recently finished. A place is in the row when the mode folds it (Focus: all but Building, Testing
  and Your check, and Recently finished), when it's empty (in every mode), or when the owner folded
  it. A chip is a button with the stage's colour dot, its name and its count, plus "2 need you" and
  "1 blocked" when there are any; a screen reader hears "Backlog: 16 items, 2 need you, 1 blocked.
  Open it". Recently finished's count is what its list shows, of the kind its chips pick. The
  row is a labelled group, always one line of a fixed height ("Every column is open" when nothing
  is folded on the plain board; nothing under Group by, whose bands fold their own columns), so
  folding, or a poll changing a count, never moves the bar or the board's top. It never wraps:
  short of room the chips' names shorten to an ellipsis, breaking anywhere so they keep what fits
  (never under 2.5em; the full name is in the label and tooltip), while their counts and
  needs-you and blocked words stay whole. They shorten only when that makes every chip fit: when
  even shortened they'd overflow, the names stay whole and the chips scroll sideways (round 1
  review; QA in round 3: "Rea…", "Buildi…" and Paused cut to its icon over a scrolling row bought
  nothing). Then a cue at the row's end, which
  never scrolls away, says what's past the edge: "+3 more · 1 needs you", the needs-you count
  being what the hidden chips need; it's a button ("3 more columns past the edge, 1 needs you.
  Show them") whose first click scrolls to the first hidden chip that needs you, else the next one
  past the edge. Each click after goes on to the next chip in board order past the one the last
  went to, measured in view or not (at a board of 400px no chip fits beside the cue, mu-86), so a chip wider than the row doesn't hold the walk (round 3, R8), and back to the start,
  the first chip, once the end is in view, so every chip comes into view once a round (QA_4: going
  back to the first that needs you never reached the chips before it). A new project or mode
  starts the row at its start. (Round 2 review: macOS shows the scrollbar
  only while scrolling, so a chip past the edge took its needs-you out of sight). Considered for
  that: shorter badges ("1 you"), which only move the edge. Considered: B, short stubs at the
  top of each column's track (still takes width per empty stage and reads as broken columns), and
  C, empty columns open (a board of empty frames pushes the work off screen).
- **No strips on the plain board.** The grid holds only the open columns, each at most 400px
  (`modes.ts` `OPEN_COLUMN_MAX`), about a card's reading width; the room left over stays empty at
  the board's right, and a card never stretches. With every column folded, the board says so.
- **Show and hide per column, remembered per mode.** A chip's click opens its column in place; an
  open column's head, the Paused lane's and Recently finished's have a quiet **Fold** that sends it
  to the row. The choice is kept on this Mac per display mode (`muster.columns` in the window's
  storage), only where it differs from what the mode does: folding a column Focus opened forgets
  the open, and opening a full column the owner hid forgets the hide. So Focus's folds are its
  defaults, and switching modes resets nothing. Backlog stays open, empty, for its quick add (until
  the owner or Focus folds it), and Your check while it has Accept all's result or its limit to say.
- **Group by bands keep 0083's strips** (out of scope): one strip anatomy, 44px in its own track,
  the dot at the top, the name down the strip, then the count, a folded one with its needs-you and
  blocked counts. A column the owner hid on the plain board is folded in the bands too. A strip's
  class is `col-strip` with `col-empty` or `folded`, never `empty`: that's the empty message's
  class ("No tasks match"), whose 32px padding once pushed strips out of their tracks.

## ADHD mode: everything visible

- **Options are on screen, not behind something.** No kebab or "…" menus, no hover-only buttons,
  no popovers, no "show more", no collapsed sections for things used in normal work. If a control
  matters, it's visible; if there are too many, group and label them rather than hide them.
- **Show the value, not every possible value.** A field shows its current value as one chip
  (priority, type, for human/AI). Clicking it lays the options out inline, in place, never in a
  popover or dropdown (the owner, on 0021: "priority and type don't need to show all options,
  just the selected one"). What must always show is every **state** and every **action**, not
  every value a field could take. Places whose job is choosing (Appearance, a filter, the Block
  form) show all their options.
- **No overlays.** Details, New task and editing open in a docked pane beside the content, which
  shrinks and stays live. Modals only for confirming something destructive.
- **Side by side over tabs** when two things are used together (a task and its board). Tabs only
  for things you don't need at the same time.
- **One board for your work and the agents'** (owner, 2026-10-13: a second board took space from
  the first): your own tasks sit in the same columns, hatched in gold with the YOU ribbon so they
  can't be mistaken, and the Who chips (All · You · Agents) show either side alone.
- **What needs you is always in view:** a visible count and a highlighted place for anything
  waiting on the owner (Your check, escalated questions, blocked on owner, human tasks), on every
  screen, plus the Dock badge, and one Mac notification when something new comes in (0052).
- **Status on the card:** stage, blocked, who holds it, waiting-on and open questions show on the
  card or row itself, not only in the task panel.
- **Who holds it is in the card's corner** (task 0097), the YOU ribbon's spot: a task held in
  Shaping, Building or Testing wears a **holder ribbon** with the holder's short name ("Shapour",
  "Asghar_7"), and a backlog note a session is shaping says "shaping · Shapour". The owner's own
  task, and one the owner holds, wears the gold YOU ribbon instead: never two ribbons on a card.
  - The ribbon has the name; the who line under the title keeps the rest (the session, since
    when, what it's doing), so the name isn't said twice. In Standard's brief card the ribbon is
    the one place the holder shows; ADHD, Standard and Focus all show it (`modes.ts`).
  - An identity, not a state: the panel fill, the line border and the text colour, the YOU
    ribbon's size and weight, never a stage colour or the gold. A working session's dot sits in
    it beside the name; a lease run out turns its words the attention colour with "quiet", as the
    card's flag says it.
  - In place, no hover-only words: the ribbon has the strip above the top row to itself, so it
    may run nearly the card's width (half of it when the stage band shares the strip). Where
    that's tight, "shaping" gives way first while its "·" stays, so what's left never reads as two
    names; then the name ends in "…" but keeps about four characters; "quiet" never shrinks.
    A note the owner shapes says "You · quiet" once its lease ran out, since reclaim ends it too. The whole label is in the tooltip and read out with the card
    ("held by Shapour", "being shaped by Shapour"). It isn't a control, so the keyboard order is the
    card's. Right to left, it sits top-left, like the YOU ribbon.
  - Fixed place: a card in Shaping, Building or Testing, and every note, keeps a strip the
    ribbon's height above its top row, held or not. The top row (key, kind, priority) and a note's
    title run the card's full width under it, and a claim arriving with the next poll moves nothing.

## Standard mode: what most apps do

- **Filters behind one chip.** Search and saved views stay in the bar; the filter groups and Sort
  fold behind **Filters** with the number on, and open in place under the bar. The active line
  (what's on, how much it hides, Clear all) always shows.
- **Show the value** for Who and Group by too: the current choice as one chip.
- **Brief cards:** title, kind, priority, age, and anything wrong or waiting on the owner. Who,
  the check-in, branch, tags, parent, estimate and files show on the selected card. Not on hover:
  a card growing under the pointer would push the cards below it.
- **The task pane leads with the work:** Vision, Work, and the Log and check-ins fold to a header
  each, with a word on what's inside.

## Focus mode: only what needs doing now

- **The board is Building, Testing and Your check.** The other columns are chips in the row above
  the board with their name, count, needs-you count and blocked count (the grouped board's Paused
  and Backlog bands fold to their heads); a click opens one, and Focus remembers it until it's
  folded again. Blocked never hides behind a fold.
- **A card is its title and one status line** (who's on it, or Your turn), plus blocked, quiet,
  needs a tester or needs you when it is.
- **No filter bar until ⌘F,** only a button in its place, and the bar stays while a search or
  filter is on, so nothing is hidden without saying so.
- **The sidebar narrows** to the category headings with their needs-you count, the project names, the working
  dot, stalled and the needs-you and paused counts;
  a stalled project keeps its Ask for a check-in.
- **Recently finished folds to a chip** at the end of the row above the board, with its name and
  count, opened in place with a click: finished work isn't what needs doing now, but it stays one
  click away.
- **The task pane folds as in Standard.**

## Sidebar categories (task 0082)

- **A heading reads as a heading.** It starts its group with a thin divider, its name is plainly
  larger and heavier than its projects' names (15px 750 against 13px 550: at 13px 650 the owner
  saw the same font), in the text colour, and its chevron is a real button (Pill-size, saying open
  or folded) with a drawn chevron in the text colour, turned down when open, not a text glyph. Its projects are indented under it, on the side the text
  starts (mirrored for right-to-left names), in every mode.
- **Its counts are words, the same open and folded,** in every mode: "77 need you · 17 projects ·
  18 working", what needs you first, in the tokens a project's row uses (the needs chip, the
  Building dot); what's 0 is left out, but never the projects. Its tasks are in the tooltip.
- **One line, open or folded** (owner, 2026-10-14, round 3: no number without its words): the
  chevron, the name and those counts.
  - **The name gives way first:** it ends in "…" down to about 4em (60px), and its tooltip and
    the heading's aria-label say it whole.
  - **"N need you" always shows whole, in words,** the chip and "need you" together, at every
    width down to 150px; it never shrinks, wraps or ends in "…".
  - **"· N projects" and "· N working"** each show whole where there's room after that; otherwise
    they're left out whole (never "· 1…") and are in the tooltip and the aria-label.
  - **Where even the name's 4em and "N need you" don't fit beside the chevron** (the narrowest
    sidebar, 150px, in Focus or with the splitter at its minimum), the counts take a second line
    under the name, whole and labelled. At the default 210px and wider it's one line, a 3-digit
    count included.
  - The whole line, and the heading's tasks, are in the tooltip and read out on the heading.
- **A project's row follows the same rule** (task 0087): one line, the name giving way first,
  ending in "…", its full name in the tooltip. Its badges (running, needs you, in progress, paused
  "⏸︎ N", tasks) are never cut, dropped or split. A name never vanishes, at any width: it keeps
  about 4em (a shorter name its own width), and where that and the badges don't fit, the badges
  take the line under the name, whole, at the row's end. A row that fits stays one line, as
  before; at the default 210px only a row whose name would otherwise vanish changes (drover's,
  with stalled, running and its counts). "All" keeps its word. The name
  starts beside its dot in any script, a Persian one too. The row reads out its full name and
  every count in words, then the real project and folder. The stalled project's check-in button
  wraps its words inside the sidebar, never past its edge.
- **The needs-you badge's digits are dark on the orange** (`--on-status`, 4.5:1 in every theme),
  wherever the badge is.

## The colour code

One colour, one meaning, everywhere (owner, 2026-10-13: "the color are mixing together, we need
to have a solid color code"). `themes.ts` holds the values; tests keep them apart.

| Colour | Means | Nothing else uses it for |
|---|---|---|
| grey · blue · cyan | Backlog · Shaping · Ready (cyan stays well apart from Building's violet) | tags, types |
| the accent (violet by default) | Building, and selection or focus | tags |
| amber | Testing | |
| orange | Your check and "needs you" (the same thing: waiting on the owner) | priority |
| green | Done | |
| red | Blocked, and errors (a wait on other tasks alone clears by itself: neutral, with a small red mark, 0060) | a task's type (a bug is the word "bug") |
| magenta | High priority (a filled chip); normal is an outlined grey chip, low faint text | stages |
| neutral grey, outlined | tags and types: labels, not states | |
| the panel, outlined, in the text colour | Who holds a task or is shaping a note (the holder ribbon, 0097): an identity, not a state | stages, the YOU gold |
| muted text grey, outlined, with ⏸ | Paused (0045): a calm state, never a stage's fill or the blocked red | |
| lime · orchid · pink · tan · mint, a small dot | A project's own colour (0057), by its name wherever it's named: an identity, not a state | stages, priority, needs you, blocked (a test keeps each 30 apart from all of them in every theme) |

A stage colour only ever says a stage. Words carry everything else.

## Names: Drover's tokens and atoms (task 0053)

Muster keeps its own palette and these rules, and takes Drover's names (owner, 2026-10-13, option
a), so its screens can move into Drover later. The values are Muster's; only the names are shared.

| Drover token | What it is | Muster's old name (still an alias) |
|---|---|---|
| `--bg` | the window background | |
| `--panel` | cards, panes, bars | `--surface` |
| `--line` | borders and rules | `--border` |
| `--hover` | a hovered or quiet fill | `--tint` |
| `--selected` | the chosen chip or row | `--accent-soft` |
| `--accent`, `--accent-ink` | the accent, and words on it | `--on-accent` |
| `--focus` | the keyboard focus ring (the accent) | |
| `--ok`, `--bad`, `--warn` | green words, red words (errors), needs-you orange words | `--err`, `--needs-text` |
| `--text`, `--muted` | words | |

Muster's own: `--popover`, `--faint`, `--accent-strong`, `--accent-text`, `--gold-text`, the stage
colours `--st-*` and their edges `--edge-*`, priority `--pri-*`, `--on-status`, `--radius`.
`themes.ts` `OLD_TOKEN_NAMES` lists the aliases; nothing in the repo uses them.

The patterns are atoms in `app/src/ui`, with Drover's names, props and classes (`ui-<name>`,
`ui-<name>--<variant>`): **Button** (a bordered button; `variant="primary"` for a place's one next
step), **Pill** (a rounded chip; `on` for the chosen one), **StatusPill** (a stage, blocked or
paused, in its colour), **Badge** (a small label: needs you, blocked, paused, a note, an estimate,
a level), **Segmented** with **Segment**s (the task pane's joined value picker). Screens use them;
a test fails on a screen that draws one of these with its own class, or leaves a plain `<button>`.
Drover's Menu, Modal, Overlay, Select and Lightbox are not taken: they hide or cover things.

## Out of the way

Visible doesn't mean loud. Controls are always there but never between the owner and the work:

- **Quiet by default, clear on use.** Secondary controls are small and low-contrast (text-weight
  chips, muted colour) and come to full contrast on hover or focus. They don't disappear.
- **One primary action per place** (the next step), the rest as plain secondary buttons beside it.
- **Fixed places.** Every control lives in the same spot on every task and screen, so the eye
  learns to skip it. Nothing jumps: no layout shift when data loads or a value changes; inline
  forms open in reserved space or push content predictably, never reflow the whole pane.
- **Edges, not the middle.** Controls sit in a compact strip at the top or side of their area;
  the content (description, cards) gets the bulk of the space. In the task panel the description
  starts above the fold.
- **Compact, not cramped.** Chips and segmented controls instead of labelled form rows; one line
  where one line will do.
- **Group by purpose** with short labels, in the same order everywhere.
- **Colour and weight mark what needs attention;** everything else stays quiet. Attention colour
  is spent only on what's waiting on the owner or broken.
- **Lead with what matters** (title, stage, what's waiting); details below, still on screen.

## Roles (task 0112)

Settings › Agents › Roles is the last section of Agents: a list of the team's roles, a row each
with its name (in bold, `dir="auto"` so a Persian name reads right to left) and, quieter, its id
and slot count ("coder · 3 slots"). Selecting a row opens its fields inline under it, the row
lit as Settings' selected group is (the selected tint and the accent bar); selecting it again closes them. No overlay, no modal. The open role
shows one quiet line (title, stage, permission, stage moves) and then a labelled box per field
with its Save and what it changes; a refusal shows in red under its field, as Claims' do.
Existing tokens only.

On cards and in the task pane, a claim made for a slot shows the slot's name where the holder's
label shows today, same size and colour: "Kaveh-2 · main · 7b5e8856". It's text, read with the
card, and the role's name now, so a rename shows everywhere at the next poll, as it does in
`muster team`.

## The icon (task 0085)

The owner's robot head with a check, in violet. One drawing everywhere: the app icon (light or
dark with macOS), the sidebar's logo (light or dark with the theme's mode, never the system's),
and the menu bar icon, a template macOS tints, plain while nothing waits and with its antenna
tips filled in when something does: a change of shape, not only of colour. At menu bar size the
tips are a few pixels, so while something waits the count stands beside the head too ("3", in
the menu bar's own font and tint, like Mail's), and nothing does while idle (task 0089). The
owner's drawing stays as drawn. The sources are in
`app/src-tauri/icons/original/`; a new drawing replaces them and `icons/build-icons.sh` runs.

## Checking a design

A design passes when the owner, looking at the screen without clicking, can answer: what is this,
what state is it in, what's waiting on me, and what can I do here. And when they're reading or
working the content, the controls around it don't pull their eye. Every task that changes the UI
checks its screenshots against these rules, in ADHD mode first. A new element that should show
differently in Standard or Focus gets its row in `modes.ts`, with what it does in each mode.
