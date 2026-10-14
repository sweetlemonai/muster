// "More above / below" for the board's columns (owner, 2026-10-13: "hide the scrollbar, in the
// columns doesn't feel good"). A column's list scrolls without a scrollbar, so it says itself how
// many cards are out of sight: out of sight is out of mind (docs/DESIGN.md).

/** A card's place in its list: its top in the list's content coordinates (0 = the top of the
 *  list's padding box, scrolled or not) and its height. */
export type Span = { top: number; height: number };
/** How many cards are wholly or partly out of sight at each end. */
export type More = { above: number; below: number };
/** And whether the list overflows at all: only then does the column draw its count strips. */
export type MoreState = More & { overflow: boolean };

export const NO_MORE: More = { above: 0, below: 0 };
export const NOTHING_MORE: MoreState = { above: 0, below: 0, overflow: false };
/** A card cut by less than this is in view: layout rounds to fractions of a pixel. */
const EDGE = 1;
/** What a column's list counts: task cards and the Backlog's note cards. */
const CARD = ".card, .note-card";

/** The cards wholly or partly above and below the visible part of a list scrolled by `scrollTop`
 *  that shows `clientHeight` of itself. Nothing counts in a list that shows nothing (folded). */
export function moreAround(scrollTop: number, clientHeight: number, cards: Span[]): More {
  if (clientHeight <= 0) return NO_MORE;
  const bottom = scrollTop + clientHeight;
  let above = 0;
  let below = 0;
  for (const c of cards) {
    if (c.top < scrollTop - EDGE) above++;
    if (c.top + c.height > bottom + EDGE) below++;
  }
  return { above, below };
}

export const sameMore = (a: MoreState, b: MoreState) => a.above === b.above && a.below === b.below && a.overflow === b.overflow;

/** What `measureMore` reads of a list element. */
type ListBox = Pick<HTMLElement, "scrollTop" | "clientHeight" | "scrollHeight" | "clientTop"> & {
  getBoundingClientRect(): { top: number };
  querySelectorAll(selector: string): ArrayLike<{ getBoundingClientRect(): { top: number; height: number } }>;
};

/** Reads a list's scroll and its cards' boxes, and counts. */
export function measureMore(el: ListBox): MoreState {
  const origin = el.getBoundingClientRect().top + el.clientTop - el.scrollTop;
  const spans = Array.from(el.querySelectorAll(CARD), (c) => {
    const r = c.getBoundingClientRect();
    return { top: r.top - origin, height: r.height };
  });
  return { ...moreAround(el.scrollTop, el.clientHeight, spans), overflow: el.scrollHeight > el.clientHeight + EDGE };
}

/** Watches a list and calls `onChange` with its counts: on scroll (a passive listener), when it or
 *  one of its children changes size (a ResizeObserver; children are followed as they come and
 *  go), and once at the start. Measuring waits for the next frame, so a burst of events measures
 *  once. Returns the function that stops it all. */
export function watchMore(el: HTMLElement, onChange: (m: MoreState) => void): () => void {
  let frame = 0;
  const measure = () => {
    frame = 0;
    onChange(measureMore(el));
  };
  const schedule = () => {
    if (!frame) frame = requestAnimationFrame(measure);
  };
  const sizes = new ResizeObserver(schedule);
  sizes.observe(el);
  for (const child of Array.from(el.children)) sizes.observe(child);
  // Only elements have a size to watch; a text node would make observe() throw.
  const isElement = (n: Node): n is Element => n.nodeType === 1;
  const children = new MutationObserver((records) => {
    for (const r of records) {
      for (const n of Array.from(r.removedNodes)) if (isElement(n)) sizes.unobserve(n);
      for (const n of Array.from(r.addedNodes)) if (isElement(n)) sizes.observe(n);
    }
    schedule();
  });
  children.observe(el, { childList: true });
  el.addEventListener("scroll", schedule, { passive: true });
  schedule();
  return () => {
    el.removeEventListener("scroll", schedule);
    children.disconnect();
    sizes.disconnect();
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
  };
}
