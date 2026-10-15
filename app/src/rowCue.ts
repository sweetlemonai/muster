// "+2 more · 1 needs you" for the row of folded columns (mu-84, review R7). The row is one line
// that scrolls sideways past its room (R1), and macOS shows its scrollbar only while scrolling, so
// a chip past the edge took its count and its needs-you out of sight with nothing saying so. When
// the chips don't fit, a cue at the row's end, outside the scrolling part, counts what's out of
// sight and what of that needs the owner: what needs you is always in view (docs/DESIGN.md).

import { needsWords } from "./human";
import { tn } from "./i18n";

/** A chip's place along the row: where it starts and ends, in the same coordinates as the view. */
export type ChipSpan = { id: string; start: number; end: number };
/** What the cue says: how many chips are out of sight, how many of their tasks need the owner,
 *  and the chip its click scrolls to. */
export type Cue = { more: number; needs: number; target: string };
/** What the row's watcher reports: whether the chips need more room than the row has, which chips
 *  are wholly in view and which are cut by an edge (partly in view). */
export type RowView = { overflow: boolean; visible: string[]; cut: string[] };

export const ALL_IN_VIEW: RowView = { overflow: false, visible: [], cut: [] };
/** A chip cut by less than this is in view: layout rounds to fractions of a pixel. */
const EDGE = 1;

/** The chips wholly inside the view from `start` to `end`. Both edges count, so it reads the same
 *  left to right and right to left, and a chip scrolled past the start is out of sight too. */
export function inView(spans: readonly ChipSpan[], start: number, end: number): string[] {
  return spans.filter((s) => s.start >= start - EDGE && s.end <= end + EDGE).map((s) => s.id);
}

/** The chips an edge cuts: partly in view, not wholly. A chip wider than the view is one of them
 *  wherever it's scrolled to. */
export function cutBy(spans: readonly ChipSpan[], start: number, end: number): string[] {
  const whole = new Set(inView(spans, start, end));
  return spans.filter((s) => !whole.has(s.id) && s.end > start + EDGE && s.start < end - EDGE).map((s) => s.id);
}

/** The cue for the chips not in `visible`, or null when every chip is in view. Its first click
 *  goes to what needs the owner: the first chip past the far end of what's in view with something
 *  that needs them, else the first hidden one that does, else the next chip past the far end.
 *  After that, `from` is the chip the last click went to, measured in view or not (mu-86), and each
 *  click steps in board order to the next hidden chip past both it and what's in view, even when it
 *  can't be wholly seen (a chip wider than the row, review R8). With nothing past that, the walk
 *  goes back to the start, the first hidden chip, so every chip comes into view once per round
 *  (QA_4: going back to what needs you first never reached the chips before it). A `from` that's no
 *  longer a chip (its column was opened) goes back to the start too. */
export function overflowCue(chips: readonly { id: string; needs: number }[], visible: readonly string[], from: string | null = null): Cue | null {
  const seen = new Set(visible);
  const hidden = chips.filter((c) => !seen.has(c.id));
  if (hidden.length === 0) return null;
  const at = chips.findIndex((c) => c.id === from);
  const last = Math.max(
    chips.reduce((end, c, i) => (seen.has(c.id) ? i : end), -1),
    at,
  );
  const beyond = chips.slice(last + 1).filter((c) => !seen.has(c.id));
  const first = from === null ? beyond.find((c) => c.needs > 0) ?? hidden.find((c) => c.needs > 0) ?? beyond[0] : at < 0 ? hidden[0] : beyond[0];
  const target = first ?? hidden[0];
  return { more: hidden.length, needs: hidden.reduce((n, c) => n + c.needs, 0), target: target.id };
}

/** The cue as a screen reader says it: "3 more columns past the edge, 1 needs you. Show them". */
export function cueWords(c: Cue): string {
  const what = `${c.more} more ${c.more === 1 ? "column" : "columns"} past the edge`;
  const needs = c.needs > 0 ? `, ${needsWords(c.needs)}` : "";
  return `${what}${needs}. Show ${c.more === 1 ? "it" : "them"}`;
}

const sameIds = (a: readonly string[], b: readonly string[]) => a.length === b.length && a.every((id, i) => id === b[i]);
export const sameView = (a: RowView, b: RowView) => a.overflow === b.overflow && sameIds(a.visible, b.visible) && sameIds(a.cut, b.cut);

type Box = { getBoundingClientRect(): { left: number; right: number; width: number } };
type ChipBox = Box & { dataset: { chip?: string }; querySelector(selector: string): Box | null };
/** What `measureRow` reads of the row and of its scrolling list of chips. `padding` is the row's
 *  inline padding, `nameMin` the least a chip's name shrinks to (its min-width, 2.5em), and
 *  `reserved` the least room the board's edge cue's slot and its gap take at the row's end
 *  (mu-198): a fixed width, the same whether the cue shows or what it says, so the chips' answer
 *  never changes with it. */
type RowBox = Pick<HTMLElement, "clientWidth"> & { padding: number; nameMin: number; reserved?: number };
type ListBox = Pick<HTMLElement, "clientWidth" | "clientLeft"> & Box & { querySelectorAll(selector: string): ArrayLike<ChipBox> };

/** The least room the chips can take: their boxes end to end, less what each name could still
 *  give up before it reaches `nameMin`. It's the same whether the names are shortened or whole. */
export function leastRoom(chips: readonly { start: number; end: number; name: number }[], nameMin: number): number {
  if (chips.length === 0) return 0;
  const first = Math.min(...chips.map((c) => c.start));
  const last = Math.max(...chips.map((c) => c.end));
  return last - first - chips.reduce((n, c) => n + Math.max(0, c.name - nameMin), 0);
}

/** Reads the chips' boxes. The chips overflow when even with every name shortened they need more
 *  than the whole row: only then does the cue show, and then the names stay whole, since
 *  shortening them wouldn't spare the scroll (QA, round 3). Short of that the names shorten just
 *  enough and nothing scrolls. Either way the answer doesn't depend on which is drawn now, so
 *  neither the cue nor the names keep themselves there. */
export function measureRow(row: RowBox, list: ListBox): RowView {
  const room = row.clientWidth - row.padding - (row.reserved ?? 0);
  const chips = Array.from(list.querySelectorAll("[data-chip]"), (c) => {
    const r = c.getBoundingClientRect();
    return { id: c.dataset.chip ?? "", start: r.left, end: r.right, name: c.querySelector(".col-chip-name")?.getBoundingClientRect().width ?? 0 };
  });
  if (leastRoom(chips, row.nameMin) <= room + EDGE) return ALL_IN_VIEW;
  const start = list.getBoundingClientRect().left + list.clientLeft;
  const end = start + list.clientWidth;
  return { overflow: true, visible: inView(chips, start, end), cut: cutBy(chips, start, end) };
}

/** Watches what `observe` holds and calls `onChange` with `measure()`: when one of them or a child
 *  of `list` changes size (a ResizeObserver; children are followed as they come and go), on
 *  `scroller`'s scroll (passive) and once at the start, each burst measured once on the next frame.
 *  A poll that leaves every size as it was measures nothing. Returns the function that stops it. */
function watchScroll(observe: readonly HTMLElement[], list: HTMLElement, scroller: HTMLElement, measure: () => RowView, onChange: (v: RowView) => void): () => void {
  let frame = 0;
  const run = () => {
    frame = 0;
    onChange(measure());
  };
  const schedule = () => {
    if (!frame) frame = requestAnimationFrame(run);
  };
  const sizes = new ResizeObserver(schedule);
  for (const el of observe) sizes.observe(el);
  for (const child of Array.from(list.children)) sizes.observe(child);
  const isElement = (n: Node): n is Element => n.nodeType === 1;
  const children = new MutationObserver((records) => {
    for (const r of records) {
      for (const n of Array.from(r.removedNodes)) if (isElement(n)) sizes.unobserve(n);
      for (const n of Array.from(r.addedNodes)) if (isElement(n)) sizes.observe(n);
    }
    schedule();
  });
  children.observe(list, { childList: true });
  scroller.addEventListener("scroll", schedule, { passive: true });
  schedule();
  return () => {
    scroller.removeEventListener("scroll", schedule);
    children.disconnect();
    sizes.disconnect();
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
  };
}

/** The least room the board's edge cue's slot takes from the chips: its min-width and the gap
 *  before it while it's on the chips' line, nothing when the row is too narrow and the slot has a
 *  line of its own, under theirs (`top` at or below the list's bottom). */
export function slotReserve(slot: { minWidth: number; top: number } | null, list: { bottom: number }, gap: number): number {
  if (!slot || slot.top >= list.bottom - EDGE) return 0;
  return slot.minWidth + gap;
}

/** Watches the row and calls `onChange` with what's in view: when the row, the list or a chip
 *  changes size, on scroll and once at the start (`watchScroll`). The board's edge cue's slot, while
 *  the board shows, takes its least width from the chips' room (`slotReserve`). Returns the
 *  function that stops it. */
export function watchRow(row: HTMLElement, list: HTMLElement, onChange: (v: RowView) => void): () => void {
  return watchScroll([row, list], list, list, () => {
    const s = getComputedStyle(row);
    const name = list.querySelector(".col-chip-name");
    const nameMin = name ? parseFloat(getComputedStyle(name).minWidth) || 0 : 0;
    const slot = row.querySelector(".col-edge-slot");
    const at = slot && { minWidth: parseFloat(getComputedStyle(slot).minWidth) || 0, top: slot.getBoundingClientRect().top };
    const reserved = slotReserve(at, list.getBoundingClientRect(), parseFloat(s.columnGap) || 0);
    return measureRow({ clientWidth: row.clientWidth, padding: parseFloat(s.paddingInlineStart) + parseFloat(s.paddingInlineEnd), nameMin, reserved }, list);
  }, onChange);
}

// The board's own cue (mu-198, the spike's R4): the board is a grid of its open columns that
// scrolls sideways past its room, and macOS shows the scrollbar only while scrolling, so Your check,
// last but Done, was the column past the edge with nothing saying so. The same rule as the row's
// cue: the columns past either edge, cut ones too, are named in a Pill at the row's end, what needs
// the owner first, and each click walks to the next one (`overflowCue`).

/** One open column of the board as its cue names it: its id, its name and how many of its tasks
 *  need the owner (`needsYouCount`, as its head says). */
export type EdgeColumn = { id: string; label: string; needs: number };
/** The board's cue: the columns not wholly in view, what needs the owner first, then board order;
 *  their needs-you total; and the column its click brings in view. */
export type EdgeCue = { hidden: EdgeColumn[]; needs: number; target: string };

/** The cue for the open columns not in `visible`, or null when every one is in view. The walk is
 *  the row's (`overflowCue`): the first click goes to a hidden column that needs the owner, else the
 *  next past the edge; each after walks on from `from` in board order, then back to the start. */
export function edgeCue(cols: readonly EdgeColumn[], visible: readonly string[], from: string | null = null): EdgeCue | null {
  const cue = overflowCue(cols, visible, from);
  if (!cue) return null;
  const seen = new Set(visible);
  const hidden = cols.filter((c) => !seen.has(c.id));
  return { hidden: [...hidden.filter((c) => c.needs > 0), ...hidden.filter((c) => c.needs === 0)], needs: cue.needs, target: cue.target };
}

/** "21 need you", in the language shown. */
export const edgeNeeds = (n: number) => tn("board", "edge.needYou", { count: n });

const TAGS = /<\/?\w+\s*\/?>/g;
/** One column as the cue's text says it: "Your check · 21 need you", or its name alone. */
const edgeItem = (c: EdgeColumn) => (c.needs > 0 ? tn("board", "edge.item", { name: c.label, needs: edgeNeeds(c.needs) }).replace(TAGS, "") : c.label);

/** The cue's words as it shows them, every name whole (its tooltip): "Past the edge: Your check ·
 *  21 need you, Done". */
export function edgeText(cue: EdgeCue): string {
  const list = cue.hidden.map(edgeItem).join(tn("board", "edge.sep"));
  return tn("board", "edge.lead").split("<list/>").join(list);
}

/** The cue as a screen reader says it: "2 columns past the edge: Your check, 21 need you; Done.
 *  Show them". */
export function edgeWords(cue: EdgeCue): string {
  const list = cue.hidden.map((c) => (c.needs > 0 ? tn("board", "edge.labelItem", { name: c.label, needs: edgeNeeds(c.needs) }) : c.label)).join(tn("board", "edge.labelSep"));
  return tn("board", "edge.label", { count: cue.hidden.length, list });
}

/** What `measureBoard` reads of the board: its visible box, how wide its content is, and each open
 *  column's box (`data-col`). */
type BoardBox = Pick<HTMLElement, "clientWidth" | "clientLeft" | "scrollWidth"> & Box & { querySelectorAll(selector: string): ArrayLike<Box & { dataset: { col?: string } }> };

/** Which open columns are wholly in view and which an edge cuts; every one is in view when the
 *  board's content fits its box. Both edges count, so it reads the same right to left. */
export function measureBoard(board: BoardBox): RowView {
  if (board.scrollWidth <= board.clientWidth + EDGE) return ALL_IN_VIEW;
  const start = board.getBoundingClientRect().left + board.clientLeft;
  const end = start + board.clientWidth;
  const spans = Array.from(board.querySelectorAll("[data-col]"), (c) => {
    const r = c.getBoundingClientRect();
    return { id: c.dataset.col ?? "", start: r.left, end: r.right };
  });
  return { overflow: true, visible: inView(spans, start, end), cut: cutBy(spans, start, end) };
}

/** How far to scroll the board, in px along x, to put a column at the view's inline start, `pad`
 *  inside it (the board's own padding): its left edge to the left one, right to left its right
 *  edge to the right one. A column at the far end stops where the scroll does, still wholly in
 *  view when it's no wider than the view. */
export function scrollDelta(span: { start: number; end: number }, view: { start: number; end: number }, pad: number, rtl: boolean): number {
  return rtl ? span.end - (view.end - pad) : span.start - (view.start + pad);
}

/** Watches the board's columns: on resize (the board and each column), on the board's scroll and
 *  once at the start, as `watchRow` does. Returns the function that stops it. */
export function watchBoard(board: HTMLElement, onChange: (v: RowView) => void): () => void {
  return watchScroll([board], board, board, () => measureBoard(board), onChange);
}

/** How the board's cue says it in the room its slot has (review R1, R2; the owner's rule that
 *  nothing is cut out of sight): every name whole on one line; else the same words on two lines,
 *  broken only between words; else how many columns are past the edge, in words, with their
 *  needs-you words. Never a name cut short, never a column dropped off the end. */
export type EdgeFit = "line" | "wrap" | "count";

/** What `edgeFit` weighs: the slot's width, the one-line cue's width, and the two-line cue's
 *  lines and whether a word of it is wider than the slot. */
export type EdgeRoom = { room: number; line: number; lines: number; over: boolean };

export function edgeFit(r: EdgeRoom): EdgeFit {
  if (r.line <= r.room + EDGE) return "line";
  if (r.lines <= 2 && !r.over) return "wrap";
  return "count";
}

type Probe = Pick<HTMLElement, "offsetWidth" | "clientWidth" | "clientHeight" | "scrollWidth">;
/** What `measureEdge` reads: the slot's width and its two hidden probes, the whole cue on one line
 *  and on as many lines as the slot's width makes it (`lineHeight` each). The probes always hold
 *  every name, so the answer doesn't depend on which form is drawn. */
export type EdgeSlot = { clientWidth: number; line: Probe; wrap: Probe; lineHeight: number };

export function measureEdge(s: EdgeSlot): EdgeFit {
  return edgeFit({
    room: s.clientWidth,
    line: s.line.offsetWidth,
    lines: s.lineHeight > 0 ? Math.round(s.wrap.clientHeight / s.lineHeight) : Infinity,
    over: s.wrap.scrollWidth > s.wrap.clientWidth + EDGE,
  });
}

/** Watches the slot and its probes (a ResizeObserver: the row's room changing, or the cue's words)
 *  and calls `onFit` with `measureEdge` once per frame, and once at the start. Returns the
 *  function that stops it. */
export function watchEdge(slot: HTMLElement, onFit: (f: EdgeFit) => void): () => void {
  const line = slot.querySelector<HTMLElement>(".col-edge-probe-line");
  const wrap = slot.querySelector<HTMLElement>(".col-edge-probe-wrap");
  if (!line || !wrap) return () => {};
  let frame = 0;
  const run = () => {
    frame = 0;
    onFit(measureEdge({ clientWidth: slot.clientWidth, line, wrap, lineHeight: parseFloat(getComputedStyle(wrap).lineHeight) || 0 }));
  };
  const schedule = () => {
    if (!frame) frame = requestAnimationFrame(run);
  };
  const sizes = new ResizeObserver(schedule);
  for (const el of [slot, line, wrap]) sizes.observe(el);
  schedule();
  return () => {
    sizes.disconnect();
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
  };
}
