---
covers: app/src/ColumnRow.tsx, app/src/rowCue.ts, app/src/App.tsx, app/src/modes.ts
---
# The column row above the board

- Every place not open is a chip in one row above the board, in board order. (mu-84)
- A chip shows its name, count, and needs-you and blocked counts. (mu-84)
- The chips never wrap; names shorten, counts and needs-you words stay whole. (mu-84)
- The row is one line of a fixed height; a fold or a poll never moves the board's top. (mu-84)
- Under 30em, set by its width alone, the row is two lines: the chips and the row's cue, then the board's cue. (mu-198)
- Columns past the board's edge are named whole at the row's end, what needs you first; a click scrolls the board to them and walks on. (mu-198)
- The owner's column opens and folds are kept per display mode. (mu-84)
