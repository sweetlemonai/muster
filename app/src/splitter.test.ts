import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { tn } from "./i18n";
import { setLocale } from "./i18n/runtime";
import { clampPane, clampSidebar, PANE_MIN, paneRange, SIDEBAR, SIDEBAR_MAX, SIDEBAR_MIN, sidebarRange } from "./paneSize";
import { dragWidth, loadSplit, PANE_KEY, PANE_LEGACY_KEY, SIDEBAR_KEY, SPLIT_STEP, SPLIT_STEP_BIG, splitKey, storeSplit } from "./splitter";
import { Splitter, type SplitterProps } from "./ui";
import appSource from "./App.tsx?raw";
import css from "./styles.css?raw";

const props = (over: Partial<SplitterProps> = {}): SplitterProps => ({ width: 300, min: 150, max: 320, side: "left", label: "Projects width", onWidth: () => {}, onDrag: () => {}, onDragEnd: () => {}, onReset: () => {}, ...over });
/** The handle's element, from calling the (hook-free) component: its handlers can be called. */
const handle = (over: Partial<SplitterProps> = {}) => Splitter(props(over)) as ReactElement<Record<string, (e: unknown) => void>>;

describe("the width a drag asks for", () => {
  it("follows the pointer: the edge of a pane on the left moves with it, one on the right against it", () => {
    expect(dragWidth(210, 100, 150, "left", false)).toBe(260);
    expect(dragWidth(210, 100, 50, "left", false)).toBe(160);
    expect(dragWidth(560, 800, 700, "right", false)).toBe(660);
    expect(dragWidth(560, 800, 900, "right", false)).toBe(460);
  });
  it("is measured from the width at the start, so it doesn't jump and a long drag can come back", () => {
    expect(dragWidth(560, 800, 800, "right", false)).toBe(560);
    expect(dragWidth(210, 100, 100, "left", false)).toBe(210);
  });
  it("counts layout pixels under Text size: at 150% the pointer moves 1.5 client px per layout px", () => {
    expect(dragWidth(210, 100, 190, "left", false, 1.5)).toBe(270);
    expect(dragWidth(560, 800, 650, "right", false, 1.5)).toBe(660);
    // A scale that makes no sense counts as 1.
    expect(dragWidth(210, 100, 150, "left", false, 0)).toBe(260);
    expect(dragWidth(210, 100, 150, "left", false, Number.NaN)).toBe(260);
  });
  it("follows the pointer right to left too (mu-149): the sidebar is on the right there and the pane on the left", () => {
    // The sidebar ("left": the reading start) sits right of its splitter: dragging right narrows it.
    expect(dragWidth(210, 100, 150, "left", true)).toBe(160);
    expect(dragWidth(210, 100, 50, "left", true)).toBe(260);
    // The pane ("right": the reading end) sits left of its splitter: dragging right widens it.
    expect(dragWidth(560, 800, 900, "right", true)).toBe(660);
    expect(dragWidth(560, 800, 700, "right", true)).toBe(460);
    expect(dragWidth(560, 800, 650, "right", true, 1.5)).toBe(460);
  });
  it("stays within the bounds right to left, dragged past either end either way", () => {
    // Sidebar: far right (narrower than its least) and far left (wider than its most).
    expect(clampSidebar(dragWidth(210, 1300, 1440, "left", true), 1440)).toBe(SIDEBAR_MIN);
    expect(clampSidebar(dragWidth(210, 1300, 0, "left", true), 1440)).toBe(SIDEBAR_MAX);
    // Pane: far right (wider than the window leaves) and far left (under its least).
    expect(clampPane(dragWidth(560, 560, 1440, "right", true), 1440, SIDEBAR)).toBe(paneRange(1440, SIDEBAR).max);
    expect(clampPane(dragWidth(560, 560, 0, "right", true), 1440, SIDEBAR)).toBe(PANE_MIN);
  });
});

describe("the arrow keys", () => {
  it("move the edge 40 px, 120 with Shift: → widens a pane on the left and narrows one on the right", () => {
    expect([SPLIT_STEP, SPLIT_STEP_BIG]).toEqual([40, 120]);
    expect(splitKey("ArrowRight", false, "left", false, 210)).toBe(250);
    expect(splitKey("ArrowLeft", false, "left", false, 210)).toBe(170);
    expect(splitKey("ArrowLeft", false, "right", false, 560)).toBe(600);
    expect(splitKey("ArrowRight", false, "right", false, 560)).toBe(520);
    expect(splitKey("ArrowRight", true, "left", false, 210)).toBe(330);
    expect(splitKey("ArrowLeft", true, "right", false, 560)).toBe(680);
  });
  it("stay physical right to left (mu-149): → still moves the edge right, which narrows the sidebar there and widens the pane", () => {
    expect(splitKey("ArrowRight", false, "left", true, 210)).toBe(170);
    expect(splitKey("ArrowLeft", false, "left", true, 210)).toBe(250);
    expect(splitKey("ArrowRight", false, "right", true, 560)).toBe(600);
    expect(splitKey("ArrowLeft", false, "right", true, 560)).toBe(520);
    expect(splitKey("ArrowLeft", true, "left", true, 210)).toBe(330);
    expect(splitKey("ArrowRight", true, "right", true, 560)).toBe(680);
  });
  it("leave every other key alone", () => {
    for (const rtl of [false, true]) for (const k of ["ArrowUp", "ArrowDown", "Enter", " ", "Home", "a"]) expect(splitKey(k, false, "left", rtl, 210), k).toBeNull();
  });
});

describe("the remembered width", () => {
  const keys = { PANE_KEY, SIDEBAR_KEY };
  it("has a key of its own for each pane, and a version in it", () => {
    expect(keys).toEqual({ PANE_KEY: "muster.pane.v1", SIDEBAR_KEY: "muster.sidebar.v1" });
    expect(PANE_LEGACY_KEY).toBe("muster.paneWidth");
  });
  it("is read back as the whole pixels it was stored as", () => {
    expect(loadSplit(SIDEBAR_KEY, () => "260")).toBe(260);
    expect(loadSplit(PANE_KEY, () => "560")).toBe(560);
  });
  it("is the default (null) when nothing is stored or what's stored is not a width", () => {
    for (const bad of [null, "", " ", "0", "-5", "abc", "12px", "1e3", "NaN", "Infinity", "3.5", "999999", "٣٠٠"]) expect(loadSplit(SIDEBAR_KEY, () => bad), String(bad)).toBeNull();
  });
  it("is the default when storage is off or denied", () => {
    expect(loadSplit(SIDEBAR_KEY, () => { throw new Error("denied"); })).toBeNull();
  });
  it("is written as whole pixels, and forgotten with null", () => {
    const wrote: [string, string | null][] = [];
    expect(storeSplit(SIDEBAR_KEY, 260.4, (k, v) => wrote.push([k, v]))).toBe(true);
    expect(storeSplit(SIDEBAR_KEY, null, (k, v) => wrote.push([k, v]))).toBe(true);
    expect(wrote).toEqual([[SIDEBAR_KEY, "260"], [SIDEBAR_KEY, null]]);
  });
  it("says so when storage refuses, and does not throw", () => {
    expect(storeSplit(SIDEBAR_KEY, 260, () => { throw new Error("full"); })).toBe(false);
  });
  it("reads the pane's width from before the key was versioned, when there's none under the new one", () => {
    const store: Record<string, string> = { [PANE_LEGACY_KEY]: "612" };
    const read = (k: string) => store[k] ?? null;
    expect(loadSplit(PANE_KEY, read, PANE_LEGACY_KEY)).toBe(612);
    // Not for a splitter that has no old key, and the new key wins.
    expect(loadSplit(SIDEBAR_KEY, read)).toBeNull();
    store[PANE_KEY] = "480";
    expect(loadSplit(PANE_KEY, read, PANE_LEGACY_KEY)).toBe(480);
    // A damaged old width is the default.
    expect(loadSplit(PANE_KEY, (k) => (k === PANE_KEY ? null : "wide"), PANE_LEGACY_KEY)).toBeNull();
  });
  it("drops the old key once the new one is written, and with a reset, so a reset doesn't bring it back", () => {
    const store: Record<string, string> = { [PANE_LEGACY_KEY]: "612" };
    const write = (k: string, v: string | null) => (v === null ? delete store[k] : (store[k] = v));
    expect(storeSplit(PANE_KEY, 640, write, PANE_LEGACY_KEY)).toBe(true);
    expect(store).toEqual({ [PANE_KEY]: "640" });
    store[PANE_LEGACY_KEY] = "612";
    expect(storeSplit(PANE_KEY, null, write, PANE_LEGACY_KEY)).toBe(true);
    expect(store).toEqual({});
    expect(loadSplit(PANE_KEY, (k) => store[k] ?? null, PANE_LEGACY_KEY)).toBeNull();
  });
  it("keeps the old key when the new one couldn't be written", () => {
    const store: Record<string, string> = { [PANE_LEGACY_KEY]: "612" };
    expect(storeSplit(PANE_KEY, 640, () => { throw new Error("full"); }, PANE_LEGACY_KEY)).toBe(false);
    expect(store[PANE_LEGACY_KEY]).toBe("612");
  });
  it("round-trips through the real storage calls of the default reader and writer", () => {
    const mem = new Map<string, string>();
    (globalThis as { localStorage?: unknown }).localStorage = { getItem: (k: string) => mem.get(k) ?? null, setItem: (k: string, v: string) => void mem.set(k, v), removeItem: (k: string) => void mem.delete(k) };
    try {
      expect(storeSplit(SIDEBAR_KEY, 275)).toBe(true);
      expect(mem.get(SIDEBAR_KEY)).toBe("275");
      expect(loadSplit(SIDEBAR_KEY)).toBe(275);
      expect(storeSplit(SIDEBAR_KEY, null)).toBe(true);
      expect(mem.has(SIDEBAR_KEY)).toBe(false);
      expect(loadSplit(SIDEBAR_KEY)).toBeNull();
    } finally {
      delete (globalThis as { localStorage?: unknown }).localStorage;
    }
  });
});

describe("the Splitter", () => {
  it("renders a focusable vertical separator with its width, minimum and maximum", () => {
    expect(renderToStaticMarkup(createElement(Splitter, props({ width: 210, min: SIDEBAR_MIN, max: SIDEBAR_MAX })))).toContain(
      'role="separator" aria-orientation="vertical" aria-label="Projects width" aria-valuenow="210" aria-valuemin="150" aria-valuemax="320" tabindex="0"',
    );
    expect(renderToStaticMarkup(createElement(Splitter, props()))).toContain('class="ui-splitter"');
    expect(renderToStaticMarkup(createElement(Splitter, props({ className: "x" })))).toContain('class="ui-splitter x"');
  });
  it("sends the key's width on an arrow, and keeps the event from scrolling", () => {
    const asked: number[] = [];
    let prevented = 0;
    const el = handle({ width: 210, side: "left", onWidth: (w) => asked.push(w) });
    el.props.onKeyDown({ key: "ArrowRight", shiftKey: false, preventDefault: () => prevented++ });
    el.props.onKeyDown({ key: "ArrowLeft", shiftKey: true, preventDefault: () => prevented++ });
    expect(asked).toEqual([250, 90]);
    expect(prevented).toBe(2);
  });
  it("ignores other keys, and doesn't swallow them", () => {
    const asked: number[] = [];
    let prevented = 0;
    handle({ onWidth: (w) => asked.push(w) }).props.onKeyDown({ key: "Tab", shiftKey: false, preventDefault: () => prevented++ });
    expect([asked, prevented]).toEqual([[], 0]);
  });
  it("goes back to the default on a double-click", () => {
    let reset = 0;
    handle({ onReset: () => reset++ }).props.onDoubleClick({});
    expect(reset).toBe(1);
  });

  /** A handle element that records what the drag does to it. */
  const fakeEl = (rect = 6, offset = 6) => {
    const listeners = new Map<string, (e: unknown) => void>();
    const log: string[] = [];
    const attrs = new Set<string>();
    const el = {
      offsetWidth: offset,
      getBoundingClientRect: () => ({ width: rect }),
      setPointerCapture: (id: number) => log.push(`capture ${id}`),
      hasPointerCapture: () => true,
      releasePointerCapture: (id: number) => log.push(`release ${id}`),
      setAttribute: (k: string) => attrs.add(k),
      removeAttribute: (k: string) => attrs.delete(k),
      addEventListener: (t: string, f: (e: unknown) => void) => listeners.set(t, f),
      removeEventListener: (t: string) => listeners.delete(t),
    };
    return { el, listeners, log, attrs };
  };
  const down = (h: ReturnType<typeof handle>, el: unknown, over: object = {}) => {
    let prevented = false;
    h.props.onPointerDown({ button: 0, pointerId: 7, clientX: 800, currentTarget: el, preventDefault: () => (prevented = true), ...over });
    return prevented;
  };

  it("captures the pointer on press, follows it, and lets go on release", () => {
    const asked: number[] = [];
    const keys: number[] = [];
    let ended = 0;
    const { el, listeners, log, attrs } = fakeEl();
    expect(down(handle({ width: 560, side: "right", onWidth: (w) => keys.push(w), onDrag: (w) => asked.push(w), onDragEnd: () => ended++ }), el)).toBe(true);
    expect(log).toEqual(["capture 7"]);
    expect(attrs.has("data-dragging")).toBe(true);
    listeners.get("pointermove")?.({ clientX: 700 });
    listeners.get("pointermove")?.({ clientX: 900 });
    listeners.get("pointermove")?.({ clientX: 800 });
    // A drag reports every move as a drag, not as a width to keep, and says when it ends.
    expect(asked).toEqual([660, 460, 560]);
    expect([keys, ended]).toEqual([[], 0]);
    listeners.get("pointerup")?.({});
    expect(ended).toBe(1);
    expect(log).toEqual(["capture 7", "release 7"]);
    expect(attrs.size).toBe(0);
    expect(listeners.size).toBe(0);
  });
  it("also stops when the pointer is cancelled or taken away", () => {
    for (const end of ["pointercancel", "lostpointercapture"]) {
      let ended = 0;
      const { el, listeners, attrs } = fakeEl();
      down(handle({ onDragEnd: () => ended++ }), el);
      listeners.get(end)?.({});
      expect([end, listeners.size, attrs.size, ended]).toEqual([end, 0, 0, 1]);
    }
  });
  it("counts layout pixels under Text size: the box is 9 client px for 6 layout px at 150%", () => {
    const asked: number[] = [];
    const { el, listeners } = fakeEl(9, 6);
    down(handle({ width: 210, side: "left", onDrag: (w) => asked.push(w) }), el, { clientX: 100 });
    listeners.get("pointermove")?.({ clientX: 190 });
    expect(asked).toEqual([270]);
  });
  it("only drags with the main button", () => {
    const { el, log } = fakeEl();
    expect(down(handle(), el, { button: 2 })).toBe(false);
    expect(log).toEqual([]);
  });
  it("in فارسی drags the sidebar's edge with the pointer and keeps the arrows physical (mu-149)", async () => {
    await setLocale("fa");
    try {
      const asked: number[] = [];
      const { el, listeners } = fakeEl();
      // The sidebar is on the right: the pointer going left (800 → 750) widens it.
      down(handle({ width: 210, side: "left", onDrag: (w) => asked.push(w) }), el);
      listeners.get("pointermove")?.({ clientX: 750 });
      // The pane is on the left: the pointer going right (800 → 850) widens it.
      const pane = fakeEl();
      down(handle({ width: 560, side: "right", onDrag: (w) => asked.push(w) }), pane.el);
      pane.listeners.get("pointermove")?.({ clientX: 850 });
      expect(asked).toEqual([260, 610]);
      const keyed: number[] = [];
      const key = (key: string) => ({ key, shiftKey: false, preventDefault: () => {} });
      handle({ width: 210, side: "left", onWidth: (w) => keyed.push(w) }).props.onKeyDown(key("ArrowRight"));
      handle({ width: 560, side: "right", onWidth: (w) => keyed.push(w) }).props.onKeyDown(key("ArrowRight"));
      expect(keyed).toEqual([170, 600]);
    } finally {
      await setLocale("en");
    }
  });
});

describe("the sidebar's and the pane's ranges, which the separator reports", () => {
  it("run from the drag minimum to what the window leaves", () => {
    expect(sidebarRange(1440)).toEqual({ min: SIDEBAR_MIN, max: SIDEBAR_MAX });
    expect(sidebarRange(820)).toEqual({ min: SIDEBAR_MIN, max: 320 });
    expect(paneRange(1440, SIDEBAR)).toEqual({ min: PANE_MIN, max: 1020 });
  });
  it("narrow with the window: the pane's maximum falls under its minimum only when the window leaves no more", () => {
    expect(paneRange(820, SIDEBAR)).toEqual({ min: PANE_MIN, max: 400 });
    expect(paneRange(820, SIDEBAR_MAX)).toEqual({ min: 290, max: 290 });
  });
  it("hold the styles: a splitter is 6 px, straddling its edge", () => {
    expect(css).toMatch(/^\.ui-splitter \{[^}]*width: 6px;[^}]*margin-inline: -3px;/m);
  });
});

describe("both panes use the one splitter (0058)", () => {
  it("puts it on the sidebar's right edge and the pane's left, each with its own key and range, fitted to the window", () => {
    expect(appSource.match(/<Splitter /g)).toHaveLength(2);
    // Their names come from the translation files (mu-146), in English here.
    expect(appSource).toContain('side="left" label={t("app.sidebarWidth")}');
    expect(appSource).toContain('side="right" label={t("app.detailsWidth")}');
    expect([tn("shell", "app.sidebarWidth"), tn("shell", "app.detailsWidth")]).toEqual(["Projects width", "Details width"]);
    expect(appSource).toContain("{...sidebarRange(win)} width={sidebarWidth}");
    expect(appSource).toContain("{...paneRange(win, sidebarWidth)} width={paneWidth}");
    expect(appSource).toContain("useSplitWidth(SIDEBAR_KEY)");
    expect(appSource).toContain("useSplitWidth(PANE_KEY, PANE_LEGACY_KEY)");
    expect(appSource).toContain("onReset={sideSplit.reset}");
    expect(appSource).toContain("onReset={paneSplit.reset}");
    // Keys keep at once; a drag keeps once, where it ends.
    expect(appSource).toContain("onDrag={(w) => sideSplit.drag(");
    expect(appSource).toContain("onDragEnd={sideSplit.settle}");
    expect(appSource).toContain("onDragEnd={paneSplit.settle}");
  });
  it("sits the sidebar's handle right after the sidebar, before the content", () => {
    const side = appSource.indexOf('<aside className="sidebar"');
    const end = appSource.indexOf("</aside>", side);
    expect(appSource.slice(end, end + 200)).toMatch(/^<\/aside>\s*<Splitter /);
  });
  it("draws them again when the window resizes or the Text size changes, not only at the next poll", () => {
    expect(appSource).toContain('addEventListener("resize", update)');
    expect(appSource).toContain("addEventListener(ZOOM_EVENT, update)");
    expect(appSource).toContain("const win = usePageWidth();");
  });
  it("keeps no splitter of its own in App any more", () => {
    expect(appSource).not.toContain("function Splitter");
    expect(appSource).not.toContain("onMouseDown");
  });
});
