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
- **One wording for the needs-you count:** "1 needs you", "2 need you", everywhere it's said (the
  sidebar's rows and headings, the column chips and their cue, a folded strip's, band's and Paused
  lane's badge), from one helper, `needsWords` in `app/src/human.ts` (task 0159). A folded strip or
  band is read to a screen reader with its counts in words ("Shaping: 4 items, 1 needs you, 2
  blocked"), never as a bare digit.
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
  folding, or a poll changing a count, never moves the bar or the board's top. (The one
  exception is set by its width alone: under 30em, beside the board, the board's edge cue below
  gets a second line, and the chips and the row's own cue keep the first.) The chips never wrap:
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
- **The board says which columns are past its edge** (mu-198, the UX spike's R4). The board
  scrolls sideways when its open columns need more than its width (each at least 118px), and
  macOS hides the scrollbar, so Your check, last but Done, was the column cut with nothing saying
  so. At the row's end, after the row's own cue, a second Pill names the open columns past the
  board's edge, what needs you first: "Past the edge: Your check · 21 need you, Done". A column
  the edge cuts counts as past it, and so does one scrolled out at the start. It's a button
  ("2 columns past the edge: Your check, 21 need you; Done. Show them"); a click scrolls only the
  board, never the page, so the first hidden column that needs you is wholly in view at the
  board's start (else the next past the edge), and each click after walks on in board order and
  back to the start: the row's walk (`overflowCue`), measured the same way (`watchBoard`, on
  resize and scroll only). The focus stays on the cue, and the row's polite live region says
  "Your check is in view". With every open column in view it isn't there. Its slot is there as
  long as the board is and takes the room the row has free, never less than 13em whether the cue
  shows or not, and the chips never have those 13em, so nothing moves when it comes, goes or
  changes its words. It never cuts a name or drops a column out of sight (review R1, R2): every
  name whole on one line where they fit, else the same words on two lines, broken only between
  words, else counted: "4 columns past the edge · 265 need you". The needs-you words never break.
  Too narrow for the chips, the row's cue and this one on one line (under 30em: the task pane
  open), it has a line of its own under them, so the row is two lines there, set by its width
  alone. In right to left it sits at the row's left end and the far edge is the board's left. Considered: pinning
  Your check at the board's start (breaks the stage order and fixed places: a column that moves
  when it gets cards), and "+2 columns" (a count without its words).
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

## The board's two layouts (task mu-143)

Decided 2026-10-14, from the owner's mockup ("just a different idea, do not remove the current one
just add this"; `muster-board-mockup.html` in the task's attachments). The board has a **Layout**
choice in the filter bar, **Columns** (the board above, the default, unchanged) and **Feature
grid**, kept per project on this Mac (`muster.layout.<project>`, beside its saved views; All is a
project here).

- **Features down the side, stages across.** One row per feature in flight (a task not done yet,
  something done today, or no tasks yet, so a new feature stays in view), then **Other** with the
  tasks under none and the backlog notes; Backlog to Your check across, then **Done** as today's
  count. The rows follow the board's Group by: **Feature · Goal · None** (Group by's own words), a segmented
  control in the grid's header line, which takes the filter bar's Group by chips' place while the
  grid shows (one control, not two). A row's head is the feature's name (it opens the feature),
  its progress bar and "3 of 11 done".
- **Each column head says who works there:** its count, then the holders of its tasks by name
  ("asghar_13 and akbar_11", three at most, then "and 2 more"), else the stage's role; Backlog
  "notes, not shaped yet", Ready how many can start, Your check "3 of 5" against the project's
  limit and "5 of 5, full" in the needs-you colour at it, and **Done · 45 today** on one line.
- **Small cards:** the title (three lines at most, whole in its tooltip), any trouble on its own
  line under it ("sent back twice", "no check-in for 2 h", "needs a tester": red words, something
  is wrong), why it waits on its own line when it does ("After mu-99", the block's own words and
  look), then one status and the key: who's on it with the live dot, **needs you**, or paused.
  A card that waits on you has the needs-you edge. Two cards a cell, then **+N more**, which
  expands the cell in place (Show fewer folds it). An empty cell is blank, its outline kept.
- **The header line:** "5 waiting for you, 4 being built, 3 in testing, 66 done today", what waits
  for you first in the needs-you colour; **Only what needs me** dims the rest (never hides it).
- **One alert line** when Your check is at its limit, in a place that's always there: "Your check
  is full (5 of 5): check some before more start." The limit only warns (0050), so the line says
  what to do, not that Muster stops. It takes the plain board's line above the board in the grid.
- **Clicking a card opens the task pane beside the grid**, as on the columns. The grid scrolls
  inside its frame, never the page, the header row staying at the top and the feature names at the
  start; a cell the keys move to stops clear of both (the frame's scroll-padding). Stages are
  140px at least and Done as wide as its words, so the whole grid fits once Recently finished is
  folded. When columns are past the edge (at the default 1400px window with Recently finished
  docked, Your check and Done are), the board's cue says so at the end of the alert's line, over
  the grid's end edge: "+2 more · 1 needs you", the same button and walk as the row of folded
  columns, whose click scrolls what needs you into view first (review R1: what needs you is always
  in view). Considered: 104px stages to fit the default window (cards too narrow to read), and
  stacking Recently finished under the grid (takes the height the rows need). It's an ARIA grid and one Tab stop: row and column headers, so a screen reader says
  "Orchestration, Building" with a card; the arrow keys move between cells (← from Backlog to the
  feature's name), Home and End to a row's ends, ↩ goes into a cell, ↑ ↓ step through its cards
  and +N more (one button, Show fewer once open, so the focus stays), Esc comes back to the cell.
  Recently finished stays docked beside it; folded, it's the one chip in the row above, as under
  Group by. It mirrors in right-to-left (features
  on the right, ← moves forward), and is the same in every display mode.
- **Fixed from the mockup:** the trouble chip had squeezed the holder and key onto three lines; it
  has its own line. "Done today" wrapped; it's "Done · 66 today". "None" in every empty cell was
  noise; the cells are blank. The mockup's colours were a guide only: the grid uses the app's
  tokens in every theme, and trouble is the error red, since amber means Testing here.

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
- **A dead end gets its way out beside it.** A button the core would refuse shows disabled with
  why, and when the owner can act on that why, the action is right there: a claim with no session
  to reach (its Ping off, "no session to reach: Release it, or let a new session under Asghar_7
  take it over") has **Release** in the task pane's Next step (0184), quiet until then. Its
  confirm is in place, a labelled group where the button was ("Release Asghar_7's claim? It goes
  back to Ready; its worktree and branch stay." · Release · Cancel), the holder's name in its own
  `<bdi>`, the confirm's Release focused so ↩ confirms, Esc or Cancel back to Release; no modal.
- **Side by side over tabs** when two things are used together (a task and its board). Tabs only
  for things you don't need at the same time.
- **One board for your work and the agents'** (owner, 2026-10-13: a second board took space from
  the first): your own tasks sit in the same columns, hatched in gold with the YOU ribbon so they
  can't be mistaken, and the Who chips (All · You · Agents) show either side alone.
- **What needs you is always in view:** a visible count and a highlighted place for anything
  waiting on the owner (Your check, escalated questions, blocked on owner, human tasks), on every
  screen, plus the Dock badge, and one Mac notification when something new comes in (0052).
- **Status on the card:** stage, blocked, who holds it, what its session is doing, waiting-on and
  open questions show on the card or row itself, not only in the task panel. What Claude Code says
  the holder's session is doing (0048) is on the holder ribbon after the name, in words (mu-171:
  "working needs a ribbon not a tag"), so a working card, and one waiting on the owner in Claude
  Code, is seen from across the board.
- **Who holds it is in the card's corner** (task 0097), the YOU ribbon's spot: a task held in
  Shaping, Building or Testing wears a **holder ribbon** with the holder's short name ("Shapour",
  "Asghar_7"), and a backlog note a session is shaping says "shaping · Shapour". The owner's own
  task, and one the owner holds, wears the gold YOU ribbon instead: never two ribbons on a card.
  - The ribbon has the name and the session's state; the who line under the title keeps the rest
    (the session, since when), so neither is said twice. In Standard's brief card the ribbon is
    the one place the holder and the state show; ADHD, Standard and Focus all show it (`modes.ts`).
  - The name is an identity, not a state: the panel fill, the line border and the text colour,
    the YOU ribbon's size and weight, never a stage colour or the gold. After it comes the
    session's state in words, and only the state carries a colour, the one it has in the task
    pane (mu-171): "Asghar_7 · working" with "working" in the Building accent, "Asghar_7 · session
    ended" in the words' own colour (Building only), and **waiting for input in the needs-you
    orange**: a block of `--st-needs` with `--on-status` words, from after the name to the card's
    edge, because it is the owner's turn in Claude Code and the colour code's orange means exactly
    that; its edge parts it from the name, so it has no "·", and the card's own "needs you" badge
    doesn't come with it. Unknown and paused say nothing. The state is Claude Code's, matched by
    session id in every account Muster reads, so a builder resumed on another account than its
    stamp's still shows working (0182); a shaper's claim gets one too ("Shapour_3 · working").
    The live dot isn't in it any more: it sits in the token ribbon, by the number it moves
    (mu-172), and still not while the session waits for input or has ended, where Claude Code's
    word wins. A
    lease run out turns the name the attention colour with "quiet", as the card's flag says it;
    beside a state, "quiet" gives way first, and beside waiting for input it's in the tooltip and
    the card's flag line only.
  - In place, no hover-only words: the ribbon has the strip above the top row to itself, so it
    may run nearly the card's width (half of it when the stage band shares the strip), and the
    card's width less 8px when it says a state, over the band too. Where that's tight, "shaping"
    gives way first while its "·" stays, so what's left never reads as two names; then the name
    ends in "…" but keeps about four characters; "quiet" never shrinks without a state. A name
    that ends in a number keeps the number, its last three digits at most, since that's what tells
    asghar_16 from asghar_13: only the part before it gives way ("as…16"). The state's
    words are never cut: where they and about four letters of the name don't fit, the short forms
    show ("needs you", "ended"; "working" is already short), with the full words in the tooltip
    and read out ("held by Asghar_7, waiting for input in Claude Code"). At the owner's 1400px
    window (106px cards) that's "gh…22 needs you", "asg…16 · working", "asgh…15 · ended"; at
    the 820px window's 100px cards, "g…22 needs you".
    A note the owner shapes says "You · quiet" once its lease ran out, since reclaim ends it too. The whole label is in the tooltip and read out with the card
    ("held by Shapour", "being shaped by Shapour"). It isn't a control, so the keyboard order is the
    card's. Right to left, it sits top-left, like the YOU ribbon.
  - Fixed place: a card in Shaping, Building or Testing, and every note, keeps a strip the
    ribbon's height above its top row, held or not. The top row (key, kind, priority) and a note's
    title run the card's full width under it, and a claim arriving with the next poll moves nothing.
- **What the work spent is in the card's other corner** (task mu-172): a card someone holds in
  Shaping, Building or Testing, not the owner's own, wears a **token ribbon** at the bottom, on
  the side the text ends (bottom-left right to left): the tokens its own sessions used against
  the estimate, "210k / 600k". Tokens are what `est-tokens` counts: input, output and cache
  writes, never cache reads. A session another task names too isn't counted (its share can't be
  told apart): the ribbon adds "· shared", it's read out ("a session shared with other tasks not
  counted") and the tooltip says it in full.
  - In words, not colour: past the estimate it adds "over" in weight 800 ("720k / 600k · over"),
    never the attention colour, which is the owner's turn and errors' alone. "210k · no est" and
    "600k est · not read" say the rest, short on the card and whole when read out and in the
    tooltip. Persian writes the pair «۲۱۰k از ۶۰۰k» (از, "of"): the Persian decimal ٫ beside a
    slash reads as another slash. Each figure sits in its own left-to-right isolate, so a pair
    never flips right to left.
  - Never cut, never partly shown: the dot, the figures, "over" and the note ("no est", "not
    read", the whole message when nothing was read) stay whole on a card down to 100px wide, and
    the ribbon never runs past the card's border. On a narrow card a part gives way whole or not
    at all, never ending in "…" or leaving a stray dot or gap: "shared" wraps out of sight when it
    doesn't fit (it comes last, so the dot sits first, at the ribbon's start). The "·" before "over"
    stays: the dot and "9.9M / 999k · over" fit a 100px card whole.
  - The live dot (a session's transcript changed in the live window, so tokens are being spent
    now) sits in it, first, by the number it moves: one dot per card. On a card nobody holds yet the
    dot sits alone in the ribbon's corner; a card with no token ribbon spot keeps its dot in the
    top row. The dot is still, under reduced motion too.
  - Neutral like the holder ribbon: the panel fill, the line border and the text colour. Not a
    control. The card reads it out last: "210k of 600k tokens used, over the estimate, being used
    now"; the exact counts and the share are in the tooltip.
  - Fixed place: every card in Shaping, Building or Testing that isn't the owner's keeps a strip
    its height at the bottom, held or not, in every mode, so a claim or a first count arriving
    with a poll moves nothing. ADHD fills it on every held card; Standard only while live or over,
    and on the selected card; Focus only while live or over. Its words are in the shared
    namespace, loaded before the first paint, so it never paints a blank word.
- **The task's Work section is a table by stage** (mu-172, the owner: "break down of tokens per
  stage and time per stage and sessions each then total"): a row per stage the task was in
  (Shaping, Building, Testing, Your check, and "Other" for Backlog, Ready and Done), each with its
  counted tokens, cache reads beside them, and its time; under it the sessions that spent in it,
  each with its counted tokens and time, a shared one marked "shared with N other tasks, not
  counted"; then the total: "Total: 1.8M of 1.5M tokens (120%) · 2.5 h of 8 h (31%)", with the
  cache reads and the shared sessions' tokens beside it so no token vanishes from the table.
  - The same count as the card: the total's tokens are the stage rows' own tokens summed, the
    ribbon's number. No number mixes counted tokens with cache reads.
  - Time in hours, never days ("26.5 h", an estimate of `1d` is "8 h"). The time against the
    estimate is agent time, Building + Testing, said in words; Your check's time is the owner's
    wait, on its row only, never in the total.
  - Right to left, every figure keeps its own left-to-right run.

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

## The Sessions view (task 0098)

Claude Code's agents list, inside Muster: a project view beside Board, Table, Timeline and Graph,
for one project or All.

- **Three groups, always on screen, in one order:** Needs input, Working, Completed, each heading
  with its count, and the header's count of all three ("8 need input · 0 working · 37
  completed"). An empty group says "none" and keeps its place, so nothing jumps when a session
  moves.
- **One line per session,** in the board's sizes: the name in `--text` and bold, under All the
  project's dot and name (Other for none), the message in `--muted`, the age at the inline end.
  The age is `--muted` too, not `--faint`: `--faint` is under 4.5:1 and the age is information.
- **Needs input is the needs-you orange** (the `--st-needs` edge at the row's inline start, and
  `--warn` heading and message): it waits on the owner. Working and Completed stay neutral and
  their words say which, because violet means Building and green means Done.
- **Fold in place.** Completed rows older than a day are one "Older: N" chip at the group's end;
  a click opens them there, and Fewer folds them back. Focus mode shows Needs input's rows and
  the other two groups as chips with their counts, opened in place.
- **The recap, never the transcript first.** A click opens the session's recap in the task pane's
  dock, the list still live beside it (no overlay). A strip at the top holds Transcript and Close.
  The sections never move: the chain line, then Who, Tasks it worked on, History, Time, Tokens,
  Progress. Transcript shows under the recap, never in its place. Standard and Focus fold the
  History to its header with its count.
- **Plain text, either direction.** Names and messages are the owner's own text, maybe Persian:
  `dir="auto"`, cut by characters, never drawn as Markdown. The row's layout uses logical
  properties, so RTL mirrors it.
- **No spinner.** "Reading sessions…" says it in place until the first read lands.

## Sidebar categories (task 0082)

- **A heading reads as a heading.** It starts its group with a thin divider, its name is plainly
  larger and heavier than its projects' names (15px 750 against 13px 550: at 13px 650 the owner
  saw the same font), in the text colour, and its chevron is a real button (Pill-size, saying open
  or folded) with a drawn chevron in the text colour, turned down when open, not a text glyph. Its projects are indented under it, on the side the text
  starts (mirrored for right-to-left names), in every mode.
- **Its counts are words, the same open and folded,** in every mode: "77 need you · 17 projects ·
  18 working", what needs you first, in the tokens a project's row uses (the needs chip, the
  Building dot); what's 0 is left out, but never the projects. Its tasks are in the tooltip.
- **One wording for the working count** (mu-197), as for needs you: "N working" counts tasks
  being worked on now, on a project's row, a heading and All, from one helper (`workingWords`).
  All adds the projects they're in, in words, one sentence: "29 working in 9 projects · 2
  projects stalled". "N working" and the stalled count never give way: the stalled count is
  All's one always-visible stall signal (out of sight is out of mind). Where All's line is short
  of room, the when goes first, then "in 9 projects", each whole, never cut mid-word; a piece
  that goes gives its room back, so where "in 9 projects" has gone the when shows again if it
  fits, and nothing leaves a gap between the words. How many lines All takes follows the
  sidebar's width, never its counts, so a poll that brings a stall or changes a count moves no
  row below: in a sidebar under 230px the rest always has a second line under "N working", and
  no line starts with a "·". In Focus All's badge says the short "2 stalled" and, under 200px,
  always sits on the line under the name. All of it stays in the tooltip and the aria-label.
- **One line, open or folded** (owner, 2026-10-14, round 3: no number without its words): the
  chevron, the name and those counts.
  - **The name gives way first:** it ends in "…" down to about 4em (60px), and its tooltip and
    the heading's aria-label say it whole.
  - **"N need you" always shows whole, in words,** the chip and "need you" together, at every
    width down to 150px; it never shrinks, ends in "…" or runs past the sidebar's edge. Its words
    wrap only as the last resort, below (mu-212).
  - **"· N projects" and "· N working"** each show whole where there's room after that; otherwise
    they're left out whole (never "· 1…") and are in the tooltip and the aria-label.
  - **Where even the name's 4em and "N need you" don't fit beside the chevron** (the narrowest
    sidebar, 150px, in Focus or with the splitter at its minimum), the counts take a second line,
    whole and labelled. That second line runs the heading's full width, from under the chevron to
    the inline end (mu-212), so Persian's "۴۴ مورد به شما نیاز دارد" fits it whole (a heading's
    inline margins are 4px, which leaves that line 125px at 150px). At the default 210px and wider
    English is one line, a 3-digit count included; Persian takes the second line there too.
  - **Only where even that full-width second line is too narrow** (a 4-digit count in Persian at
    150px, a larger text size), the needs words wrap between words onto one more line, the chip
    beside the first line. Never mid-word, never cut, never "…", never past the edge. A poll
    changes a heading's height only when the count gains a digit that crosses that limit.
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
| the panel, outlined, in the text colour | Who holds a task or is shaping a note (the holder ribbon, 0097): an identity, not a state; the session's state after the name keeps its own colour (working in the accent, waiting for input in the orange, mu-171) | stages, the YOU gold |
| muted text grey, outlined, with ⏸ | Paused (0045): a calm state, never a stage's fill or the blocked red | |
| lime · orchid · pink · tan · mint, a small dot | A project's own colour (0057), by its name wherever it's named: an identity, not a state | stages, priority, needs you, blocked (a test keeps each 30 apart from all of them in every theme) |
| accent · orange, on the account meter's bars (0113) | an account's window use, and orange once it passes into the owner's reserve: the one place orange means "near the limit" rather than "needs you", always with the words ("5% into the reserve" in Settings and the bar's name) | |

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

## The account meter (task 0113)

- **In the header's status unit**, before the display mode and "live": per account its name
  (`cc-mg`), two 3 px bars (the 5-hour window over the weekly one) and "resets 14:20". The whole
  row is one button that opens Settings › Accounts.
- **Read, never guessed** (decision 0013): a bar shows the latest reading. Over-cap readings fill
  the bar and stop at 100%. A mark sits where the reserve starts (100 − reserve), ringed in the
  header's colour so it shows over hatching.
- **Every state in words:** a stale window (past its reset) is hatched and says "stale"; no reading
  says "no reading"; a paused account says "paused". Each bar's accessible name says it all:
  "cc-mg 5 h: 42% used, resets 14:20; reserve 20%".
- **Never clipped:** the header wraps instead of hiding. The status unit (meter, mode, live) moves
  to a second line as one piece, with Help and Settings before it, when the row is too narrow (the
  320 px sidebar at 1280 wide). See "Fixed places" for the toolbar's rule (mu-202).
- **Keeps its width as readings change** (mu-216): each account's status keeps the room of the
  widest words its readings can give in the language shown ("resets 23 Oct 14:20 · 7 d stale",
  "no 5 h reading · 7 d stale"), held by hidden copies in the same cell that are never read out. So a
  reset, a stale window or a new reading never re-wraps the toolbar. Shorter words leave the blank
  after them, at the row's inline end inside the button, never between the name and the bars.
  Its width follows only the owner's own changes: adding, removing or pausing an account, the
  language, digits, calendar and Text size. The cost: the meter is as wide as its widest words all the time.
- No animation; logical CSS, so it mirrors in RTL. The numbers are the same as `muster accounts` gives.

## Out of the way

Visible doesn't mean loud. Controls are always there but never between the owner and the work:

- **Quiet by default, clear on use.** Secondary controls are small and low-contrast (text-weight
  chips, muted colour) and come to full contrast on hover or focus. They don't disappear.
- **One primary action per place** (the next step), the rest as plain secondary buttons beside it.
- **Fixed places.** Every control lives in the same spot on every task and screen, so the eye
  learns to skip it. Nothing jumps: no layout shift when data loads or a value changes; inline
  forms open in reserved space or push content predictably, never reflow the whole pane.
  **The toolbar wraps at its trailing group and never clips** (mu-202): line one keeps New task and
  the views (they wrap pill by pill beside New task when even they don't fit); Help, Settings, the
  meter, the mode chip and live are the toolbar's end, and on a narrow window that end is a second
  line, at its inline end, in the same order. No shortening, sideways scroll or "…" menu. The
  counts in it (Waiting for you, Problems and its quiet count of unchecked visions) keep three
  digits' room, the empty ones after the shown ones, so a poll that changes them never moves the
  board's top. The lines follow the width, the language, Text size and how many accounts the meter
  shows, never the data on a poll.
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

## Languages (task mu-146)

Muster's screens speak English and Persian (Settings › Language; the layer is in AS_BUILT.md ›
Languages).

- **What people read follows the language; what agents read stays English.** Task files, briefs,
  `--json`, the protocol and every word a tool parses back are Muster's own format, read by its
  own tools and by agents, so they're English whatever the screen shows. Menus, notifications,
  the window and the help follow the language. (When Muster moves into Drover, Drover's Agent
  language setting covers its briefs.)
- **Each language's name in its own script, never translated:** the Language chips read
  `English · فارسی` in every language, each with its own `lang`, so a person who can't read the
  language shown can still find theirs. They sit at the top of Settings, whatever group is open.
- **A switch moves nothing but the mirror:** in Persian, `<html lang="fa" dir="rtl">` mirrors the
  layout (Right to left, below); every control keeps its place in the mirrored layout.
- **No sentence built from parts:** a whole sentence is one string with `{{placeholders}}`, and
  counts use the language's plural forms; a value filled in (a path, a name, a branch) keeps its
  own direction.
- **Persian digits and Jalali dates where people read them, Latin where they copy** (mu-151): in
  فارسی, counts, ages, estimates, dates and times read «۳ کار», «۲ ساعت پیش», «۲۲ مهر ۱۴۰۵». What
  stays Latin and Gregorian: task keys and ids (`mu-12`), commands (`muster pass`), paths,
  branch names, code, front matter and log lines, an input's stored value, and `--json`. Storage
  never changes, only display. Settings › Language's Digits (`۱۲۳ · 123`) and Calendar
  (`Jalali · Gregorian`) sit under the language chips, always shown, quiet Segmented rows; each
  follows the language until picked, and 123 gives Drover's convention. Inputs take any digits
  and store Latin ones; a due date is typed in the calendar shown, with the ISO day it saves
  beside it. Names sort in the language's order, and search folds Persian (ي/ی, ك/ک, ZWNJ,
  Persian digits) so what was meant is found.
- **The command line is the owner's to switch, never an agent's (mu-152):** `muster --lang fa`
  or `MUSTER_LANG=fa` prints Persian. `LANG` is never read, and inside an agent session only an
  explicit `--lang` counts, so agents always get English. The terminal gets Jalali dates with Latin
  digits, and no bidi marks at all.

## Right to left (task mu-149)

In فارسی the whole app is a mirror of English, and nothing else changes.

- **What mirrors:** the sidebar sits at the right and the docked pane at the left. Board columns
  run from Backlog at the right to Done at the left. The Timeline runs from the past at the right
  to now at the left, and the Graph's dependency arrows point leftwards. Accent edges that mark
  state sit on the inline-start edge in both directions.
- **One way to say a side:** styles use logical properties only (`margin-inline-start`,
  `inset-inline-end`, `border-inline-start`, `text-align: start`). `logicalCss.test.ts` fails on
  a new physical left or right in CSS or a TSX inline style. The few physical exceptions (the inset
  accent shadows with `:dir(rtl)` twins, hatch textures) each carry a `physical:` comment, which
  excuses that one declaration only. Right to left changes no colour.
- **Glyphs:** chevrons, arrows, back and forward, and the fold arrows mirror. They're listed in
  one block at the end of `styles.css`, and a `→` or `◀` in markup is a `.dir-glyph` span. The
  logo, checks, the clock, the head and the open ▾ don't mirror, and neither do key names (`←`
  is the key, whichever way the page reads).
- **What stays left to right:** task keys (`mu-12`), commands, paths, branch names, code and the
  estimate inputs (`2h`, `500k`). Each is an island with the one `.ltr` class
  (`direction: ltr; unicode-bidi: isolate`), so `mu-12` never reads `12-mu`.
- **Keys:** arrows that mean next and previous follow the reading direction. In فارسی, ← unfolds
  a sidebar heading and → folds it, and the Feature grid's moves swap the same way. Arrows on a
  focused splitter stay physical, so → moves the edge right, which is what the eye expects. A
  dragged splitter edge follows the pointer.
- **One direction source:** `isRtl()` (`app/src/rtl.ts`) reads the language. No component reads
  `document.dir` or a computed `direction`, and a test holds that.

## Checking a design

A design passes when the owner, looking at the screen without clicking, can answer: what is this,
what state is it in, what's waiting on me, and what can I do here. And when they're reading or
working the content, the controls around it don't pull their eye. Every task that changes the UI
checks its screenshots against these rules, in ADHD mode first. A new element that should show
differently in Standard or Focus gets its row in `modes.ts`, with what it does in each mode.
