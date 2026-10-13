# Design rules for Muster's screens

The owner has ADHD. **Out of sight is out of mind:** an option, a task or a status that's hidden
gets forgotten. Every screen is designed so the owner sees what exists and what needs them without
looking for it. These rules win over tidiness and minimalism.

**The trick is to show everything without it getting in the way** (the owner's words). Hiding is
not the way to reduce clutter here; weight, size and placement are. Both halves below apply to
every screen.

## Everything visible

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
- **Side by side over tabs** when two things are used together (the Human and AI boards, a task
  and its board). Tabs only for things you don't need at the same time.
- **What needs you is always in view:** a visible count and a highlighted place for anything
  waiting on the owner (Your check, escalated questions, blocked on owner, human tasks), on every
  screen, plus the Dock badge.
- **Status on the card:** stage, blocked, who holds it, waiting-on and open questions show on the
  card or row itself, not only in the task panel.

## The colour code

One colour, one meaning, everywhere (owner, 2026-10-13: "the color are mixing together, we need
to have a solid color code"). `themes.ts` holds the values; tests keep them apart.

| Colour | Means | Nothing else uses it for |
|---|---|---|
| grey · light cyan · blue | Backlog · Shaping · Ready | tags, types |
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
checks its screenshots against these rules.
