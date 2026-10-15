// "+2 more · 1 needs you" for the row of folded columns (mu-84, review R7). The row is one line
// that scrolls sideways past its room (R1), and macOS shows its scrollbar only while scrolling, so
// a chip past the edge took its count and its needs-you out of sight with nothing saying so. When
// the chips don't fit, a cue at the row's end, outside the scrolling part, counts what's out of
// sight and what of that needs the owner: what needs you is always in view (docs/DESIGN.md).

import type { RowChip } from "./ColumnRow";
import { needsWords } from "./human";

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
export function overflowCue(chips: readonly RowChip[], visible: readonly string[], from: string | null = null): Cue | null {
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
 *  inline padding, `nameMin` the least a chip's name shrinks to (its min-width, 2.5em). */
type RowBox = Pick<HTMLElement, "clientWidth"> & { padding: number; nameMin: number };
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
  const room = row.clientWidth - row.padding;
  const chips = Array.from(list.querySelectorAll("[data-chip]"), (c) => {
    const r = c.getBoundingClientRect();
    return { id: c.dataset.chip ?? "", start: r.left, end: r.right, name: c.querySelector(".col-chip-name")?.getBoundingClientRect().width ?? 0 };
  });
  if (leastRoom(chips, row.nameMin) <= room + EDGE) return ALL_IN_VIEW;
  const start = list.getBoundingClientRect().left + list.clientLeft;
  const end = start + list.clientWidth;
  return { overflow: true, visible: inView(chips, start, end), cut: cutBy(chips, start, end) };
}

/** Watches the row and calls `onChange` with what's in view: when the row, the list or a chip
 *  changes size (a ResizeObserver; chips are followed as they come and go), on scroll (passive)
 *  and once at the start, each burst measured once on the next frame. A poll that leaves every
 *  size as it was measures nothing. Returns the function that stops it all. */
export function watchRow(row: HTMLElement, list: HTMLElement, onChange: (v: RowView) => void): () => void {
  let frame = 0;
  const measure = () => {
    frame = 0;
    const s = getComputedStyle(row);
    const name = list.querySelector(".col-chip-name");
    const nameMin = name ? parseFloat(getComputedStyle(name).minWidth) || 0 : 0;
    onChange(measureRow({ clientWidth: row.clientWidth, padding: parseFloat(s.paddingInlineStart) + parseFloat(s.paddingInlineEnd), nameMin }, list));
  };
  const schedule = () => {
    if (!frame) frame = requestAnimationFrame(measure);
  };
  const sizes = new ResizeObserver(schedule);
  sizes.observe(row);
  sizes.observe(list);
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
  list.addEventListener("scroll", schedule, { passive: true });
  schedule();
  return () => {
    list.removeEventListener("scroll", schedule);
    children.disconnect();
    sizes.disconnect();
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
  };
}
