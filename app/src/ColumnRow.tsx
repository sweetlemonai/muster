import { useEffect, useRef, useState } from "react";
import { needsWords, needsYouCount } from "./human";
import { tn, Trans, useT } from "./i18n";
import { STAGES } from "./lib";
import { blockedCount, itemWords, placeWords } from "./ModeViews";
import { RAIL_ID } from "./modes";
import { PAUSED_LANE } from "./PausedLane";
import { ALL_IN_VIEW, type Cue, cueWords, type EdgeColumn, type EdgeCue, edgeCue, type EdgeFit, edgeNeeds, edgeText, edgeWords, overflowCue, sameView, scrollDelta, watchBoard, watchEdge, watchRow } from "./rowCue";
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
  return `${placeWords(c.label, itemWords(c.count), c.needs, c.blocked)}. Open it`;
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
          <b className="count warn">{needsWords(cue.needs)}</b>
        </>
      )}
    </Pill>
  );
}

/** The board's open columns as its edge cue names them, in board order: each one's name and what
 *  of it needs the owner, as its head counts it (`needsYouCount`). */
export function edgeColumnsOf(open: readonly string[], cols: Record<string, Task[]>): EdgeColumn[] {
  return open.map((id) => ({
    id,
    label: id === "paused" ? tn("board", "legend.paused") : STAGES.find((s) => s.id === id)?.label ?? id,
    needs: needsYouCount(cols[id] ?? []),
  }));
}

/** The board cue's words, every name whole: "Past the edge: Your check · 21 need you, Done". */
function EdgeList({ cue }: { cue: EdgeCue }) {
  const items = cue.hidden.map((c, i) => (
    <span key={c.id} className="col-edge-item">
      {i > 0 && tn("board", "edge.sep")}
      {c.needs > 0 ? (
        <Trans ns="board" k="edge.item" params={{ name: c.label, needs: edgeNeeds(c.needs) }} components={{ name: <span className="col-edge-name" />, needs: <b className="count warn" /> }} />
      ) : (
        <span className="col-edge-name">{c.label}</span>
      )}
    </span>
  ));
  return <Trans ns="board" k="edge.lead" components={{ list: <span className="col-edge-list">{items}</span> }} />;
}

/** The board cue counted, where its names don't fit even on two lines: "2 columns past the edge ·
 *  21 need you". */
function EdgeCount({ cue }: { cue: EdgeCue }) {
  const count = cue.hidden.length;
  if (cue.needs === 0) return <>{tn("board", "edge.count", { count })}</>;
  return <Trans ns="board" k="edge.countNeeds" params={{ count, needs: edgeNeeds(cue.needs) }} components={{ needs: <b className="count warn" /> }} />;
}

/** The board's own cue (mu-198), after the row's: "Past the edge: Your check · 21 need you, Done"
 *  on one line, the same on two (`fit` "wrap") or counted ("count"), never a name cut short. Its
 *  click brings the next column in view. */
export function BoardCue({ cue, fit = "line", onShow }: { cue: EdgeCue; fit?: EdgeFit; onShow: () => void }) {
  useT("board");
  return (
    <Pill type="button" className={fit === "line" ? "col-cue col-edge" : "col-cue col-edge col-edge-two"} aria-label={edgeWords(cue)} title={edgeText(cue)} onClick={onShow}>
      {fit === "count" ? <EdgeCount cue={cue} /> : <EdgeList cue={cue} />}
    </Pill>
  );
}

/** The board cue's slot at the row's end. It takes the room the row has free, at least its own
 *  min-width, whether the cue shows or not, so the chips' room never changes with the cue. Two
 *  hidden probes hold the cue's whole words on one line and on two, and `watchEdge` picks the form
 *  that fits from them. */
function EdgeSlot({ cue, onShow }: { cue: EdgeCue | null; onShow: (target: string) => void }) {
  const slot = useRef<HTMLSpanElement>(null);
  const [fit, setFit] = useState<EdgeFit>("line");
  const shows = cue !== null;
  useEffect(() => {
    if (!shows || !slot.current) return;
    return watchEdge(slot.current, setFit);
  }, [shows]);
  return (
    <span ref={slot} className="col-edge-slot">
      {cue && (
        <>
          <BoardCue cue={cue} fit={fit} onShow={() => onShow(cue.target)} />
          <span className="col-edge-probe" aria-hidden="true">
            <span className="col-cue col-edge col-edge-probe-line">
              <EdgeList cue={cue} />
            </span>
            <span className="col-cue col-edge col-edge-two col-edge-probe-wrap">
              <EdgeList cue={cue} />
            </span>
          </span>
        </>
      )}
    </span>
  );
}

/** What the row's polite live region says after the cue brought a column in view: "Your check is
 *  in view", by the name its head shows. */
export const edgeSaid = (columns: readonly EdgeColumn[], id: string) => tn("board", "edge.live", { name: columns.find((c) => c.id === id)?.label ?? id });

/** Scrolls the board so a column is wholly in view at its inline start, smoothly unless the system
 *  asks for less motion. Only the board scrolls, never the page, and the focus stays on the cue. */
export function showColumn(board: HTMLElement, id: string) {
  const el = board.querySelector<HTMLElement>(`[data-col="${CSS.escape(id)}"]`);
  if (!el) return;
  const s = getComputedStyle(board);
  const r = el.getBoundingClientRect();
  const start = board.getBoundingClientRect().left + board.clientLeft;
  const left = scrollDelta({ start: r.left, end: r.right }, { start, end: start + board.clientWidth }, parseFloat(s.paddingInlineStart) || 0, s.direction === "rtl");
  const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  board.scrollBy({ left, behavior: still ? "auto" : "smooth" });
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
 *  past the edge (`watchRow` measures on resize and scroll, not on every poll). After both, at the
 *  row's end, the board's own cue names its open columns past its edge (`watchBoard`, mu-198). */
export default function ColumnRow({
  chips,
  grouped = false,
  onOpen,
  board = null,
  columns = [],
}: {
  chips: RowChip[];
  grouped?: boolean;
  onOpen: (id: string) => void;
  /** The plain board while it shows, and its open columns in board order (`edgeColumnsOf`). */
  board?: HTMLElement | null;
  columns?: EdgeColumn[];
}) {
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
  // The board's columns past its edge, measured as the chips are; the column the last click went
  // to, for the walk; and what the polite live region says after a click.
  const [boardView, setBoardView] = useState(ALL_IN_VIEW);
  const [edgeFrom, setEdgeFrom] = useState<string | null>(null);
  const [said, setSaid] = useState("");
  // The cue's words are the board's: loaded with the row, before a cue first shows.
  useT("board");
  useEffect(() => {
    if (!board) return;
    const stop = watchBoard(board, (v) => setBoardView((prev) => (sameView(prev, v) ? prev : v)));
    return () => {
      stop();
      setBoardView(ALL_IN_VIEW);
    };
  }, [board]);
  const edge = board && boardView.overflow ? edgeCue(columns, boardView.visible, edgeFrom) : null;
  return (
    <div ref={row} className="col-row" role="group" aria-label="Folded and empty columns">
      {!any && !grouped && <span className="col-row-none">Every column is open</span>}
      {any && (
        <div ref={list} className={view.overflow ? "col-row-chips col-row-scrolls" : "col-row-chips"}>
          {chips.map((c) => (
            <Pill key={c.id} type="button" className={`col-chip col-${c.id}`} aria-expanded={false} aria-label={chipWords(c)} title={`Open ${c.label} here`} data-chip={c.id} onClick={() => onOpen(c.id)}>
              <span className="col-chip-name">{c.label}</span> <small>{c.count}</small>
              {c.needs > 0 && <b className="count warn">{needsWords(c.needs)}</b>}
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
      {/* The cue's slot is there as long as the board is, at least its fixed least width whether
          the cue shows or not, so the chips' room never changes when it comes, goes or changes
          its words (`watchRow` takes that width from them). */}
      {board && (
        <EdgeSlot
          cue={edge}
          onShow={(target) => {
            setEdgeFrom(target);
            showColumn(board, target);
            setSaid(edgeSaid(columns, target));
          }}
        />
      )}
      {board && (
        <span className="sr-only" aria-live="polite">
          {said}
        </span>
      )}
    </div>
  );
}
