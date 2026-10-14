// The splitter between a side pane and the content (0058): the one implementation behind the
// projects sidebar's right edge and the task pane's left edge. The rules without React or the DOM:
// what a drag and a key do to the width, and the width remembered per Mac. The handle itself is
// ui/Splitter.tsx; the minimum and maximum come from paneSize.ts. Tested in splitter.test.ts.
import { useCallback, useState } from "react";

/** Which side of the splitter the pane it sizes is on: the sidebar is on its left, the task pane
 *  on its right. */
export type SplitSide = "left" | "right";

/** An arrow key moves the pane's edge this many pixels; with Shift, the bigger step. */
export const SPLIT_STEP = 40;
export const SPLIT_STEP_BIG = 120;

/** The remembered widths. The key says the format: a width changes meaning, take a new one. */
export const PANE_KEY = "muster.pane.v1";
export const SIDEBAR_KEY = "muster.sidebar.v1";

/** The pane's width after dragging from x `startX` to `x`, when it was `start` wide: the edge
 *  follows the pointer. `scale` is how many client px a layout px is (the Text size zoom). */
export function dragWidth(start: number, startX: number, x: number, side: SplitSide, scale = 1): number {
  const moved = (x - startX) / (scale > 0 && Number.isFinite(scale) ? scale : 1);
  return side === "left" ? start + moved : start - moved;
}

/** The width an arrow key asks for, or null for any other key. → moves the edge right, ← left, so
 *  it widens a pane on the splitter's left and narrows one on its right. Not clamped here. */
export function splitKey(key: string, shift: boolean, side: SplitSide, width: number): number | null {
  const step = shift ? SPLIT_STEP_BIG : SPLIT_STEP;
  let right: number;
  if (key === "ArrowRight") right = step;
  else if (key === "ArrowLeft") right = -step;
  else return null;
  return side === "left" ? width + right : width - right;
}

/** A remembered width read back: a positive, whole-pixel number; null (the default width) for
 *  anything else, and for storage that's off, denied or absent (a test run). */
export function loadSplit(key: string, read: () => string | null = () => localStorage.getItem(key)): number | null {
  try {
    const raw = read();
    if (raw === null || !/^\d{1,5}$/.test(raw)) return null;
    const n = Number(raw);
    return n > 0 ? n : null;
  } catch {
    return null;
  }
}

/** Remembers a width, or forgets it (null: back to the default); false when storage refused
 *  (full, off), and the width holds for now. */
export function storeSplit(key: string, width: number | null, write: (v: string | null) => void = (v) => (v === null ? localStorage.removeItem(key) : localStorage.setItem(key, v))): boolean {
  try {
    write(width === null ? null : String(Math.round(width)));
    return true;
  } catch {
    return false;
  }
}

/** A splitter's remembered width: null until the owner sets one; `set` keeps a width, `reset` goes
 *  back to the default. */
export function useSplitWidth(key: string): [number | null, (w: number) => void, () => void] {
  const [w, setW] = useState(() => loadSplit(key));
  const set = useCallback(
    (v: number) => {
      setW(v);
      storeSplit(key, v);
    },
    [key],
  );
  const reset = useCallback(() => {
    setW(null);
    storeSplit(key, null);
  }, [key]);
  return [w, set, reset];
}
