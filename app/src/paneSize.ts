// The docked pane's width (0021) and where Recently finished sits (0051), the parts without React:
// how wide the pane may be beside what has to stay in view. Tested in paneSize.test.ts, which also
// holds styles.css to these numbers.
import { parseZoom } from "./zoom";

/** The narrowest the owner can drag the pane to, and its width until they do. */
export const PANE_MIN = 360;
export const PANE_DEFAULT = 560;
/** The narrowest the pane gets to fit a narrow window, under ⌘+ zoom too (styles.css `.pane`
 *  min-width): it gives way before the board does. */
export const PANE_FLOOR = 240;
/** The projects sidebar's width (styles.css `.sidebar`). */
export const SIDEBAR = 210;
/** The content's narrowest beside the pane: the board's, on the board view. */
export const BOARD_MIN = 210;
/** Recently finished's narrowest beside the board (styles.css `.recent`). */
export const RAIL_MIN = 200;

/** The pane's width for `w` asked: at least PANE_MIN, and at most what leaves the sidebar and the
 *  content (the board) their room in a `windowWidth` px window, down to PANE_FLOOR when the window
 *  is too narrow for the board and PANE_MIN (the pane gives way first), and never past the window.
 *  The same in every view, so the pane doesn't jump when the view changes; on the board, Recently
 *  finished goes under the board when there's no room for it beside (`railBeside`). Whole pixels,
 *  rounded down when it fits the window, so the board keeps its full room. */
export function clampPane(w: number, windowWidth: number): number {
  const fit = Math.min(Math.max(PANE_FLOOR, windowWidth - SIDEBAR - BOARD_MIN), Math.max(0, windowWidth - SIDEBAR));
  return Math.min(Math.round(Math.max(w, PANE_MIN)), Math.floor(fit));
}

/** Recently finished sits beside the board when the board's row (the window past the sidebar and
 *  a pane of `paneWidth` px, 0 for none) holds the board's narrowest and its own; else it goes
 *  under the board, full width, still docked and in view (owner: nothing out of sight). */
export function railBeside(windowWidth: number, paneWidth: number): boolean {
  return windowWidth - SIDEBAR - paneWidth >= BOARD_MIN + RAIL_MIN;
}

/** The window's width in the page's own pixels: `innerWidth` over the ⌘+ zoom on the page root
 *  (zoom.ts, `zoom` as the root's style says it, "" for none), which the sidebar, the pane and
 *  the board are measured in. */
export function layoutWidth(innerWidth: number, zoom: string): number {
  return innerWidth / parseZoom(zoom);
}
