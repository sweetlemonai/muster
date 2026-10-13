import { describe, expect, it } from "vitest";
import css from "./styles.css?raw";
import { BOARD_MIN, clampPane, layoutWidth, PANE_FLOOR, PANE_MIN, RAIL_MIN, railBeside, SIDEBAR } from "./paneSize";

/** The board's width on the board view in a `win` px window with a pane of `pane` px (0 for
 *  none): what's left past the sidebar and the pane, less the rail's narrowest when it's beside. */
const boardRoom = (win: number, pane: number) => win - SIDEBAR - pane - (railBeside(win, pane) ? RAIL_MIN : 0);
/** A CSS rule's body, by its exact selector at the start of a line. */
const rule = (selector: string) => new RegExp(`^${selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")} \\{([^}]*)\\}`, "m").exec(css)?.[1] ?? "";

// 820 is the window's narrowest (tauri.conf.json minWidth); 960 is half a 1920 screen.
const WINDOWS = [820, 900, 960, 1000, 1040, 1280, 1440, 1920, 2560];

describe("the pane's width and Recently finished (0051): beside the board or under it", () => {
  it("keeps the board at least 210 px at every window width from 820, the pane at any width it's dragged to", () => {
    expect(BOARD_MIN).toBe(210);
    for (const win of WINDOWS) {
      for (const asked of [PANE_MIN, 560, 10_000]) {
        expect(boardRoom(win, clampPane(asked, win)), `${win}px window, pane asked ${asked}`).toBeGreaterThanOrEqual(BOARD_MIN);
      }
    }
  });
  it("pins the board's room with the pane at its widest: the rail goes under the board", () => {
    const widest = (win: number) => boardRoom(win, clampPane(10_000, win));
    expect(WINDOWS.map(widest)).toEqual(WINDOWS.map(() => 210));
    expect(WINDOWS.map((win) => railBeside(win, clampPane(10_000, win)))).toEqual(WINDOWS.map(() => false));
    expect(clampPane(10_000, 820)).toBe(400);
    expect(clampPane(10_000, 1440)).toBe(1020);
  });
  it("pins the board's room with the default pane: the rail beside it once the row holds both", () => {
    const room = (win: number) => boardRoom(win, clampPane(560, win));
    expect(WINDOWS.map(room)).toEqual([210, 210, 210, 230, 270, 310, 470, 950, 1590]);
    expect(WINDOWS.map((win) => railBeside(win, clampPane(560, win)))).toEqual([false, false, false, false, false, true, true, true, true]);
  });
  it("puts the rail beside the board with the pane closed, even in the narrowest window", () => {
    expect(railBeside(820, 0)).toBe(true);
    expect(boardRoom(820, 0)).toBe(410);
  });
  it("keeps the width the owner dragged in every view, so the pane doesn't jump when the view changes", () => {
    expect(clampPane(1000, 1440)).toBe(1000);
    expect(clampPane(560, 1000)).toBe(560);
  });
  it("never goes under the minimum you can drag it to, unless the window is too narrow, and rounds", () => {
    expect(clampPane(100, 1440)).toBe(PANE_MIN);
    expect(clampPane(10_000, 600)).toBe(PANE_FLOOR);
    expect(clampPane(560.4, 1440)).toBe(560);
  });
});

describe("the window's width the layout has (0051)", () => {
  it("is the window's width in the page's own pixels: under ⌘+ zoom there are fewer of them", () => {
    expect(layoutWidth(1440, "")).toBe(1440);
    expect(layoutWidth(1440, "1")).toBe(1440);
    expect(layoutWidth(1440, "1.5")).toBe(960);
    expect(layoutWidth(1230, "1.15")).toBeCloseTo(1069.57, 2);
    // A zoom that isn't one of the steps reads as the nearest, as zoom.ts does.
    expect(layoutWidth(1440, "sideways")).toBe(1440);
  });
  it("keeps the board 210 px at 150% in a 1440 px window, the rail under it", () => {
    const win = layoutWidth(1440, "1.5");
    const pane = clampPane(560, win);
    expect(pane).toBe(540);
    expect(win - SIDEBAR - pane).toBe(BOARD_MIN);
    expect(railBeside(win, pane)).toBe(false);
  });
});

describe("the pane gives way first under ⌘+ zoom (0051)", () => {
  // The window's own width less the sidebar and the pane: what the board's row gets, in page pixels.
  const row = (win: number, pane: number) => win - SIDEBAR - pane;
  const at = (inner: number, zoom: string) => {
    const win = layoutWidth(inner, zoom);
    const pane = clampPane(560, win);
    return { pane, board: Math.round(row(win, pane)), overflow: SIDEBAR + pane > win };
  };
  it("shrinks below the drag minimum so the board keeps 210 px while the window allows", () => {
    expect(PANE_FLOOR).toBe(240);
    expect(at(820, "1.15")).toEqual({ pane: 293, board: 210, overflow: false });
    expect(at(1000, "1.5")).toEqual({ pane: 246, board: 211, overflow: false });
    expect(row(layoutWidth(820, "1.15"), clampPane(560, layoutWidth(820, "1.15")))).toBeGreaterThanOrEqual(BOARD_MIN);
    expect(row(layoutWidth(1000, "1.5"), clampPane(560, layoutWidth(1000, "1.5")))).toBeGreaterThanOrEqual(BOARD_MIN);
  });
  it("stops at its floor below 660 page px: the board narrower, nothing past the window's edge", () => {
    expect(at(820, "1.3")).toEqual({ pane: 240, board: 181, overflow: false });
    expect(at(820, "1.5")).toEqual({ pane: 240, board: 97, overflow: false });
  });
  it("never runs past the window, down to the narrowest the app can be (820 px at 150%)", () => {
    const narrowest = layoutWidth(820, "1.5");
    expect(narrowest).toBeGreaterThanOrEqual(SIDEBAR + PANE_FLOOR);
    for (const win of [narrowest, 600, 659, 660, 700, 780]) {
      for (const asked of [PANE_MIN, 560, 10_000]) expect(SIDEBAR + clampPane(asked, win), `${win} / ${asked}`).toBeLessThanOrEqual(win);
    }
  });
  it("draws no wider than its floor allows: the CSS minimum is the floor", () => {
    expect(rule(".pane")).toContain(`min-width: ${PANE_FLOOR}px;`);
  });
});

describe("the rail gives way (0051)", () => {
  it("shrinks to its narrowest beside the board, and the CSS says the same narrowest", () => {
    const recent = rule(".recent");
    expect(recent).toMatch(/flex: 0 1 clamp\(200px, [0-9]+%, 272px\);/);
    expect(recent).toContain(`min-width: ${RAIL_MIN}px;`);
    expect(recent).not.toMatch(/flex: none/);
  });
  it("goes under the board when stacked, full width and still shown", () => {
    expect(rule(".board-row.stacked")).toContain("flex-direction: column;");
    const under = rule(".board-row.stacked > .recent");
    expect(under).toContain("min-width: 0;");
    expect(under).toMatch(/max-height: \d+vh;/);
    expect(css).not.toMatch(/\.recent[^{]*\{[^}]*display: none/);
  });
  it("matches the sidebar's width the clamp counts on", () => {
    expect(rule(".sidebar")).toContain(`width: ${SIDEBAR}px;`);
  });
});
