// The docked pane's width (0021), the projects sidebar's (0058) and where Recently finished sits
// (0051), the parts without React: how wide each may be beside what has to stay in view. Tested in paneSize.test.ts, which also
// holds styles.css to these numbers.
import { parseZoom } from "./zoom";

/** The narrowest the owner can drag the pane to, and its width until they do. */
export const PANE_MIN = 360;
export const PANE_DEFAULT = 560;
/** The narrowest the pane gets to fit a narrow window, under ⌘+ zoom too (styles.css `.pane`
 *  min-width): it gives way before the board does. */
export const PANE_FLOOR = 240;
/** The projects sidebar's width until the owner drags it (styles.css `.sidebar`), in Focus
 *  (`.sidebar[data-sidebar="narrow"]`), and the narrowest and widest they can drag it to: a name
 *  never shrinks to nothing, and the pane and the board keep their room. */
export const SIDEBAR = 210;
export const SIDEBAR_NARROW = 150;
export const SIDEBAR_MIN = 150;
export const SIDEBAR_MAX = 320;
/** The sidebar hidden (mu-167): the rail in its place (styles.css `.sidebar[data-sidebar="rail"]`),
 *  wide enough for a 3-digit needs-you badge whole. */
export const SIDEBAR_RAIL = 56;
/** The content's narrowest beside the pane: the board's, on the board view. */
export const BOARD_MIN = 210;
/** Recently finished's narrowest beside the board (styles.css `.recent`). */
export const RAIL_MIN = 200;

/** The sidebar's width for `w` asked: at least SIDEBAR_MIN and at most SIDEBAR_MAX, and never more
 *  than leaves the board and the pane at its floor their room in a `windowWidth` px window (the
 *  default width is always allowed). Whole pixels. It counts the pane whether one is open or not, so
 *  opening the pane never moves the sidebar's edge. */
export function clampSidebar(w: number, windowWidth: number): number {
  const widest = Math.min(SIDEBAR_MAX, Math.max(SIDEBAR, windowWidth - BOARD_MIN - PANE_FLOOR));
  return Math.min(Math.round(Math.max(w, SIDEBAR_MIN)), Math.floor(widest));
}

/** What the sidebar takes of the window: its `kept` width (`clampSidebar`'s) while shown, the
 *  rail's while hidden; the kept width stays for Show. The pane and Recently finished are
 *  fitted beside this. */
export function sidebarRoom(shown: boolean, kept: number): number {
  return shown ? kept : SIDEBAR_RAIL;
}

/** The pane's width for `w` asked, beside a sidebar of `sidebar` px: at least PANE_MIN, and at
 *  most what leaves the sidebar and the content (the board) their room in a `windowWidth` px
 *  window, down to PANE_FLOOR when the window is too narrow for the board and PANE_MIN (the pane
 *  gives way first), and never past the window. The same in every view, so the pane doesn't jump
 *  when the view changes; on the board, Recently finished goes under the board when there's no
 *  room for it beside (`railBeside`). Whole pixels, rounded down when it fits the window, so the
 *  board keeps its full room. */
export function clampPane(w: number, windowWidth: number, sidebar: number = SIDEBAR): number {
  const fit = Math.min(Math.max(PANE_FLOOR, windowWidth - sidebar - BOARD_MIN), Math.max(0, windowWidth - sidebar));
  return Math.min(Math.round(Math.max(w, PANE_MIN)), Math.floor(fit));
}

/** Recently finished sits beside the board when the board's row (the window past the sidebar and
 *  a pane of `paneWidth` px, 0 for none) holds the board's narrowest and its own; else it goes
 *  under the board, full width, still docked and in view (owner: nothing out of sight). */
export function railBeside(windowWidth: number, paneWidth: number, sidebar: number = SIDEBAR): boolean {
  return windowWidth - sidebar - paneWidth >= BOARD_MIN + RAIL_MIN;
}

/** The window's width in the page's own pixels: `innerWidth` over the ⌘+ zoom on the page root
 *  (zoom.ts, `zoom` as the root's style says it, "" for none), which the sidebar, the pane and
 *  the board are measured in. */
export function layoutWidth(innerWidth: number, zoom: string): number {
  return innerWidth / parseZoom(zoom);
}

/** What a splitter reports and keeps to: its width's narrowest and widest in this window. The
 *  narrowest is the drag minimum, or the widest when the window leaves less. */
export type Range = { min: number; max: number };
export function sidebarRange(windowWidth: number): Range {
  return { min: SIDEBAR_MIN, max: clampSidebar(Infinity, windowWidth) };
}
export function paneRange(windowWidth: number, sidebar: number): Range {
  const max = clampPane(Infinity, windowWidth, sidebar);
  return { min: Math.min(PANE_MIN, max), max };
}
