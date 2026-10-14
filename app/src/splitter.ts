// The splitter between a side pane and the content (0058): the one implementation behind the
// projects sidebar's right edge and the task pane's left edge. The rules without React or the DOM:
// what a drag and a key do to the width, and the width remembered per Mac. The handle itself is
// ui/Splitter.tsx; the minimum and maximum come from paneSize.ts. Tested in splitter.test.ts.
import { useCallback, useRef, useState } from "react";

/** Which side of the splitter the pane it sizes is on: the sidebar is on its left, the task pane
 *  on its right. */
export type SplitSide = "left" | "right";

/** An arrow key moves the pane's edge this many pixels; with Shift, the bigger step. */
export const SPLIT_STEP = 40;
export const SPLIT_STEP_BIG = 120;

/** The remembered widths. The key says the format: a width changes meaning, take a new one. */
export const PANE_KEY = "muster.pane.v1";
/** The pane's key before it was versioned (0058): its width is read from here once. */
export const PANE_LEGACY_KEY = "muster.paneWidth";
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
 *  anything else, and for storage that's off, denied or absent (a test run). A width under the
 *  `legacy` key, from before the key was versioned, is read when there's none under `key`. */
export function loadSplit(key: string, read: (k: string) => string | null = (k) => localStorage.getItem(k), legacy?: string): number | null {
  try {
    return parseSplit(read(key)) ?? (legacy === undefined ? null : parseSplit(read(legacy)));
  } catch {
    return null;
  }
}

function parseSplit(raw: string | null): number | null {
  if (raw === null || !/^\d{1,5}$/.test(raw)) return null;
  const n = Number(raw);
  return n > 0 ? n : null;
}

/** Remembers a width, or forgets it (null: back to the default); false when storage refused
 *  (full, off), and the width holds for now. The `legacy` key goes with it: once the new one is
 *  written it has done its part, and left behind after a reset it would bring the old width back. */
export function storeSplit(
  key: string,
  width: number | null,
  write: (k: string, v: string | null) => void = (k, v) => (v === null ? localStorage.removeItem(k) : localStorage.setItem(k, v)),
  legacy?: string,
): boolean {
  try {
    write(key, width === null ? null : String(Math.round(width)));
    if (legacy !== undefined) write(legacy, null);
    return true;
  } catch {
    return false;
  }
}

/** A splitter's remembered width: `width` is null until the owner sets one. Keys and `reset` keep
 *  it at once; a drag `drag`s it through state alone, and `settle` keeps where it ended (once,
 *  not at every pointer move). */
export function useSplitWidth(key: string, legacy?: string) {
  const [width, setWidth] = useState(() => loadSplit(key, undefined, legacy));
  const latest = useRef(width);
  const drag = useCallback((v: number) => {
    latest.current = v;
    setWidth(v);
  }, []);
  const settle = useCallback(() => {
    if (latest.current !== null) storeSplit(key, latest.current, undefined, legacy);
  }, [key, legacy]);
  const set = useCallback(
    (v: number) => {
      drag(v);
      storeSplit(key, v, undefined, legacy);
    },
    [drag, key, legacy],
  );
  const reset = useCallback(() => {
    latest.current = null;
    setWidth(null);
    storeSplit(key, null, undefined, legacy);
  }, [key, legacy]);
  return { width, set, drag, settle, reset };
}
