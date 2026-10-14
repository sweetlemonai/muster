import { type ReactNode, useEffect, useRef, useState } from "react";
import { NOTHING_MORE, sameMore, watchMore } from "./scrollMore";

/** One screen's scroll, smooth unless the system asks for less motion. */
function page(el: HTMLElement | null, dir: 1 | -1) {
  if (!el) return;
  const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  el.scrollBy({ top: dir * el.clientHeight, behavior: still ? "auto" : "smooth" });
}

/** "+N above" or "+N below" in its strip, a button that scrolls the list a screen that way. The
 *  strip keeps its height when there's nothing that way, so the count coming and going moves
 *  nothing. */
export function MoreCount({ side, n, onPage }: { side: "above" | "below"; n: number; onPage: () => void }) {
  return (
    <div className={`col-more col-more-${side}`}>
      {n > 0 && (
        <button type="button" className="more-count" onClick={onPage} title={side === "above" ? "Scroll up one screen" : "Scroll down one screen"}>
          +{n} {side}
        </button>
      )}
    </div>
  );
}

/** A board column's cards (owner, 2026-10-13). The list scrolls without a scrollbar, so while it
 *  overflows it fades at an edge with more beyond it and says how many cards are there, in a
 *  strip above and below the list that never covers a card. */
export default function ColumnCards({ children }: { children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  const [more, setMore] = useState(NOTHING_MORE);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    return watchMore(el, (m) => setMore((prev) => (sameMore(prev, m) ? prev : m)));
  }, []);
  const fades = `${more.above ? " fade-top" : ""}${more.below ? " fade-bottom" : ""}`;
  return (
    <>
      {more.overflow && <MoreCount side="above" n={more.above} onPage={() => page(ref.current, -1)} />}
      <div ref={ref} className={`cards${fades}`}>
        {children}
      </div>
      {more.overflow && <MoreCount side="below" n={more.below} onPage={() => page(ref.current, 1)} />}
    </>
  );
}
