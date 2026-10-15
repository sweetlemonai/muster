import { useEffect, useRef, useState } from "react";
import { needsYouCount } from "./human";
import { STAGES } from "./lib";
import { blockedCount } from "./ModeViews";
import { RAIL_ID } from "./modes";
import { PAUSED_LANE } from "./PausedLane";
import { ALL_IN_VIEW, type Cue, cueWords, overflowCue, sameView, watchRow } from "./rowCue";
import type { Task } from "./types";
import { Pill } from "./ui";

// The row of folded columns (mu-84, the owner's choice A): every place of the board that isn't open,
// whether the mode folds it, it's empty or the owner folded it, is one chip here, above the board
// and in board order, from the Paused lane to Recently finished. A click opens it in place; an open
// column's Fold sends it back. It replaces mu-83's strips, which took width and read sideways.

/** One place in the row: its name, how many it holds, and what needs the owner or is blocked. */
export type RowChip = { id: string; label: string; count: number; needs: number; blocked: number };

/** What the board's places hold: the columns' tasks (Paused among them), the Backlog's notes and
 *  how many rows Recently finished's first week shows with its kind filter. */
export type PlaceCounts = { cols: Record<string, Task[]>; notes: number; finished: number };

/** How many one place holds, as its open head and its chip both say it: Backlog counts its notes,
 *  Recently finished its week. */
export function placeCount(id: string, p: PlaceCounts): number {
  if (id === RAIL_ID) return p.finished;
  return (p.cols[id] ?? []).length + (id === "backlog" ? p.notes : 0);
}

/** The row's chips for the places `row` holds, in its order. With Group by the bands fold their
 *  own columns, so only Recently finished goes in the row. */
export function rowChipsOf(row: readonly string[], grouped: boolean, p: PlaceCounts): RowChip[] {
  return row
    .filter((id) => !grouped || id === RAIL_ID)
    .map((id) => {
      const list = id === RAIL_ID ? [] : p.cols[id] ?? [];
      const label = id === "paused" ? PAUSED_LANE.label : id === RAIL_ID ? "Recently finished" : STAGES.find((s) => s.id === id)?.label ?? id;
      return { id, label, count: placeCount(id, p), needs: needsYouCount(list), blocked: blockedCount(list) };
    });
}

/** A chip as a screen reader says it, the counts in words: "Shaping: 4 items, 1 needs you, 2
 *  blocked. Open it". */
export function chipWords(c: RowChip): string {
  const parts = [c.count === 0 ? "empty" : `${c.count} ${c.count === 1 ? "item" : "items"}`];
  if (c.needs > 0) parts.push(`${c.needs} ${c.needs === 1 ? "needs" : "need"} you`);
  if (c.blocked > 0) parts.push(`${c.blocked} blocked`);
  return `${c.label}: ${parts.join(", ")}. Open it`;
}

/** The cue at the row's end when its chips don't all fit (review R7): how many are past the edge
 *  and how many of their tasks need you. Its click shows them. */
export function RowCue({ cue, onShow }: { cue: Cue; onShow: () => void }) {
  return (
    <Pill type="button" className="col-cue" aria-label={cueWords(cue)} title="Show the columns past the edge" onClick={onShow}>
      <span>+{cue.more} more</span>
      {cue.needs > 0 && (
        <>
          <span aria-hidden="true">·</span>
          <b className="count warn">{cue.needs} {cue.needs === 1 ? "needs" : "need"} you</b>
        </>
      )}
    </Pill>
  );
}

/** Scrolls the list to a chip and puts the focus on it, smoothly unless the system asks for less
 *  motion. `start` brings the most of what follows it into view, whichever way the row reads. */
function showChip(list: HTMLElement | null, id: string) {
  const el = list?.querySelector<HTMLElement>(`[data-chip="${CSS.escape(id)}"]`);
  if (!el) return;
  const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  el.scrollIntoView({ inline: "start", block: "nearest", behavior: still ? "auto" : "smooth" });
  el.focus({ preventScroll: true });
}

/** The row, under the filter bar. It's always there, one line tall, so a column folding or opening
 *  never moves the bar: with every place open it says so. With Group by (`grouped`) the bands fold
 *  their own columns, so an empty row says nothing rather than "Every column is open". The chips
 *  scroll sideways in a list of their own; when they don't all fit, the cue after it says what's
 *  past the edge (`watchRow` measures on resize and scroll, not on every poll). */
export default function ColumnRow({ chips, grouped = false, onOpen }: { chips: RowChip[]; grouped?: boolean; onOpen: (id: string) => void }) {
  const row = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const [view, setView] = useState(ALL_IN_VIEW);
  // The chip the cue's last click went to: the walk goes on past it whether or not the view measures
  // it in view (R8, mu-86: at a board of 400px no chip fits beside the cue). A new project or mode
  // remounts the row (App keys it), which starts a new walk.
  const [from, setFrom] = useState<string | null>(null);
  const any = chips.length > 0;
  useEffect(() => {
    if (!any || !row.current || !list.current) return;
    return watchRow(row.current, list.current, (v) => setView((prev) => (sameView(prev, v) ? prev : v)));
  }, [any]);
  const cue = any && view.overflow ? overflowCue(chips, view.visible, from) : null;
  return (
    <div ref={row} className="col-row" role="group" aria-label="Folded and empty columns">
      {!any && !grouped && <span className="col-row-none">Every column is open</span>}
      {any && (
        <div ref={list} className={view.overflow ? "col-row-chips col-row-scrolls" : "col-row-chips"}>
          {chips.map((c) => (
            <Pill key={c.id} type="button" className={`col-chip col-${c.id}`} aria-expanded={false} aria-label={chipWords(c)} title={`Open ${c.label} here`} data-chip={c.id} onClick={() => onOpen(c.id)}>
              <span className="col-chip-name">{c.label}</span> <small>{c.count}</small>
              {c.needs > 0 && <b className="count warn">{c.needs} {c.needs === 1 ? "needs" : "need"} you</b>}
              {c.blocked > 0 && <b className="count blocked-count">{c.blocked} blocked</b>}
            </Pill>
          ))}
        </div>
      )}
      {cue && (
        <RowCue
          cue={cue}
          onShow={() => {
            setFrom(cue.target);
            showChip(list.current, cue.target);
          }}
        />
      )}
    </div>
  );
}
