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
  leaves a labelled chip, strip or header in the same spot, with a count where there is one, and
  one click or key opens it there. Nothing a mode folds is more than one click or key away.
- **What needs you never folds:** the Waiting for you count, the sidebar's counts, the needs-you
  and blocked words on a card, and a folded column's needs-you count.
- **No overlays, fixed places, no layout jumps, the colour code:** as below, the same in all three.
  Switching modes moves nothing: every control keeps its place.
- **One table:** `app/src/modes.ts` lists every element that differs, with what it does in each
  mode and how to get back what it folds. Components read it and nothing else; a test fails when
  a component reads an element the table doesn't have, or reads the mode around it.

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
  screen, plus the Dock badge.
- **Status on the card:** stage, blocked, who holds it, waiting-on and open questions show on the
  card or row itself, not only in the task panel.

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

- **The board is Building, Testing and Your check.** The other columns are thin strips with their
  name, count and needs-you count; a click opens one until Focus is left.
- **A card is its title and one status line** (who's on it, or Your turn), plus blocked, quiet or
  needs you when it is.
- **No filter bar until ⌘F,** only a button in its place, and the bar stays while a search or
  filter is on, so nothing is hidden without saying so.
- **The sidebar narrows** to the project names, the working dot and the needs-you count.
- **Recently finished folds to a strip** with its name and count beside the board, opened in place
  with a click: finished work isn't what needs doing now, but it stays one click away.
- **The task pane folds as in Standard.**

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
| red | Blocked, and errors | a task's type (a bug is the word "bug") |
| magenta | High priority (a filled chip); normal is an outlined grey chip, low faint text | stages |
| neutral grey, outlined | tags and types: labels, not states | |

A stage colour only ever says a stage. Words carry everything else.

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

## Checking a design

A design passes when the owner, looking at the screen without clicking, can answer: what is this,
what state is it in, what's waiting on me, and what can I do here. And when they're reading or
working the content, the controls around it don't pull their eye. Every task that changes the UI
checks its screenshots against these rules, in ADHD mode first. A new element that should show
differently in Standard or Focus gets its row in `modes.ts`, with what it does in each mode.
