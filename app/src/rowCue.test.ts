import { afterEach, describe, expect, it, vi } from "vitest";
import { setLocale } from "./i18n/runtime";
import { ALL_IN_VIEW, type ChipSpan, cutBy, type EdgeColumn, edgeCue, edgeFit, edgeText, edgeWords, inView, measureBoard, measureEdge, scrollDelta, slotReserve, watchBoard, watchEdge } from "./rowCue";

// The board's own cue (mu-198, the spike's R4): the open columns past the board's edge, named in
// the row above it with what needs the owner, and the walk its clicks take, the row's own.

const col = (id: string, label: string, needs = 0): EdgeColumn => ({ id, label, needs });
/** ADHD on All at 1440×900, sidebar 320px (the spike's numbers): eight open columns at their
 *  118px least, 10px apart, after the board's 14px padding: 1042px of content in an 851px view. */
const ADHD8 = [
  col("paused", "Paused", 1),
  col("backlog", "Backlog"),
  col("shaping", "Shaping", 2),
  col("ready", "Ready"),
  col("building", "Building"),
  col("testing", "Testing"),
  col("your-check", "Your check", 21),
  col("done", "Done"),
];
const VIEW = 851;
/** Where each column is, left to right, with the board scrolled `scroll` px. */
const ltr = (scroll = 0): ChipSpan[] => ADHD8.map((c, i) => ({ id: c.id, start: 14 + 128 * i - scroll, end: 132 + 128 * i - scroll }));
/** The same right to left: Paused at the right edge, scrolled `scroll` px towards the left. */
const rtl = (scroll = 0): ChipSpan[] => ADHD8.map((c, i) => ({ id: c.id, start: VIEW - 132 - 128 * i + scroll, end: VIEW - 14 - 128 * i + scroll }));
const ids = (cols: readonly EdgeColumn[]) => cols.map((c) => c.id);

describe("which columns are past the board's edge (AC1)", () => {
  it("ADHD at 1440: Your check cut by the edge and Done past it; the cue names them with Your check's 21", () => {
    const spans = ltr();
    expect(cutBy(spans, 0, VIEW)).toEqual(["your-check"]);
    const cue = edgeCue(ADHD8, inView(spans, 0, VIEW))!;
    expect(ids(cue.hidden)).toEqual(["your-check", "done"]);
    expect(cue).toMatchObject({ needs: 21, target: "your-check" });
    expect(edgeText(cue)).toBe("Past the edge: Your check · 21 need you, Done");
    expect(edgeWords(cue)).toBe("2 columns past the edge: Your check, 21 need you; Done. Show them");
  });
  it("reads the same right to left: the far edge is the left one", () => {
    const spans = rtl();
    expect(cutBy(spans, 0, VIEW)).toEqual(["your-check"]);
    const cue = edgeCue(ADHD8, inView(spans, 0, VIEW))!;
    expect(ids(cue.hidden)).toEqual(["your-check", "done"]);
    expect(cue.target).toBe("your-check");
  });
  it("a column scrolled out at the start counts too, and what needs you is named first", () => {
    // Scrolled to the end (191px): Paused and Backlog are out at the start, Backlog cut.
    const spans = ltr(191);
    expect(cutBy(spans, 0, VIEW)).toEqual(["backlog"]);
    const cue = edgeCue(ADHD8, inView(spans, 0, VIEW))!;
    expect(ids(cue.hidden)).toEqual(["paused", "backlog"]);
    expect(edgeText(cue)).toBe("Past the edge: Paused · 1 needs you, Backlog");
    // Backlog and Done hidden, only Done needs you: Done is named first.
    const some = edgeCue([col("backlog", "Backlog"), col("ready", "Ready"), col("done", "Done", 3)], ["ready"])!;
    expect(edgeText(some)).toBe("Past the edge: Done · 3 need you, Backlog");
    expect(edgeWords(some)).toBe("2 columns past the edge: Done, 3 need you; Backlog. Show them");
    expect(some.target).toBe("done");
  });
  it("one column, nothing needing you: its name alone, and the singular", () => {
    const cue = edgeCue([col("ready", "Ready"), col("done", "Done")], ["ready"])!;
    expect(edgeText(cue)).toBe("Past the edge: Done");
    expect(edgeWords(cue)).toBe("1 column past the edge: Done. Show it");
    expect(cue).toMatchObject({ needs: 0, target: "done" });
  });
  it("no cue when every open column is wholly in view, or there are none", () => {
    expect(edgeCue(ADHD8, ids(ADHD8))).toBeNull();
    expect(edgeCue([], [])).toBeNull();
  });
});

describe("the walk its clicks take (AC1)", () => {
  it("first the hidden column that needs you, then on in board order, then back to the start", () => {
    // Click 1 from the start goes to Your check. Scrolling it to the start stops at the end
    // (191px), where Your check and Done are wholly in view.
    expect(edgeCue(ADHD8, inView(ltr(), 0, VIEW))!.target).toBe("your-check");
    const end = inView(ltr(191), 0, VIEW);
    expect(end).toContain("your-check");
    expect(end).toContain("done");
    // Click 2: nothing past Your check is hidden, so back to the start.
    expect(edgeCue(ADHD8, end, "your-check")!.target).toBe("paused");
    // From the start, with Paused shown, each click steps on past the last one.
    const start = inView(ltr(), 0, VIEW);
    expect(edgeCue(ADHD8, start, "paused")!.target).toBe("your-check");
    expect(edgeCue(ADHD8, start, "your-check")!.target).toBe("done");
    expect(edgeCue(ADHD8, start, "done")!.target).toBe("your-check");
  });
  it("a last target whose column was folded meanwhile walks on from the start", () => {
    // Scrolled 100px: Paused cut at the start, Done past the end. From a column that's gone the
    // walk starts again at Paused, not at Done, the next past what's in view (review R5).
    const mid = inView(ltr(100), 0, VIEW);
    expect(ADHD8.filter((c) => !mid.includes(c.id)).map((c) => c.id)).toEqual(["paused", "done"]);
    expect(edgeCue(ADHD8, mid, "gone")!.target).toBe("paused");
    expect(edgeCue(ADHD8, mid, "your-check")!.target).toBe("done");
  });
  it("right to left the walk is the same, in board order", () => {
    expect(edgeCue(ADHD8, inView(rtl(), 0, VIEW))!.target).toBe("your-check");
    expect(edgeCue(ADHD8, inView(rtl(), 0, VIEW), "your-check")!.target).toBe("done");
    expect(edgeCue(ADHD8, inView(rtl(191), 0, VIEW), "your-check")!.target).toBe("paused");
  });
});

describe("where a click scrolls the board (AC1, AC2)", () => {
  it("puts the column at the view's inline start, inside the board's padding", () => {
    const yc = ltr()[6];
    expect(scrollDelta(yc, { start: 0, end: VIEW }, 14, false)).toBe(768);
    // Right to left the column's right edge goes to the view's right edge: a scroll to the left.
    const r = rtl()[6];
    expect(scrollDelta(r, { start: 0, end: VIEW }, 14, true)).toBe(-768);
    // A column at the start already, scrolled to: nothing moves.
    expect(scrollDelta(ltr()[0], { start: 0, end: VIEW }, 14, false)).toBe(0);
    expect(scrollDelta(rtl()[0], { start: 0, end: VIEW }, 14, true)).toBe(0);
    // Out past the start: back towards it.
    expect(scrollDelta(ltr(191)[0], { start: 0, end: VIEW }, 14, false)).toBe(-191);
  });
});

/** A board as measureBoard reads it: its box at `left`, its content `scroll` wide, and its columns. */
const board = (o: { width: number; scroll: number; left?: number; cols: ChipSpan[] }) => {
  const left = o.left ?? 334;
  const rect = (l: number, r: number) => ({ left: l + left, right: r + left, width: r - l });
  return {
    clientWidth: o.width,
    clientLeft: 0,
    scrollWidth: o.scroll,
    getBoundingClientRect: () => rect(0, o.width),
    querySelectorAll: () => o.cols.map((c) => ({ dataset: { col: c.id }, getBoundingClientRect: () => rect(c.start, c.end) })),
  };
};

describe("measuring the board (AC1)", () => {
  it("in the window's coordinates: the columns wholly in view and the one the edge cuts", () => {
    expect(measureBoard(board({ width: VIEW, scroll: 1042, cols: ltr() }))).toEqual({
      overflow: true,
      visible: ["paused", "backlog", "shaping", "ready", "building", "testing"],
      cut: ["your-check"],
    });
    expect(measureBoard(board({ width: VIEW, scroll: 1042, left: 0, cols: rtl() }))).toEqual({
      overflow: true,
      visible: ["paused", "backlog", "shaping", "ready", "building", "testing"],
      cut: ["your-check"],
    });
  });
  it("everything in view when the content fits, a pixel of rounding included", () => {
    expect(measureBoard(board({ width: 851, scroll: 851, cols: ltr() }))).toBe(ALL_IN_VIEW);
    expect(measureBoard(board({ width: 851, scroll: 852, cols: ltr() }))).toBe(ALL_IN_VIEW);
    expect(measureBoard(board({ width: 851, scroll: 853, cols: ltr() })).overflow).toBe(true);
  });
  it("Focus at 1100 with Backlog, Shaping and Ready opened: Testing cut, Your check past the edge", () => {
    // The spike: six columns, 786px of content in 580.
    const six = ["backlog", "shaping", "ready", "building", "testing", "your-check"].map((id, i) => ({ id, start: 14 + 128 * i, end: 132 + 128 * i }));
    const v = measureBoard(board({ width: 580, scroll: 786, cols: six }));
    expect(v).toEqual({ overflow: true, visible: ["backlog", "shaping", "ready", "building"], cut: ["testing"] });
    const cue = edgeCue([col("backlog", "Backlog"), col("shaping", "Shaping"), col("ready", "Ready"), col("building", "Building"), col("testing", "Testing"), col("your-check", "Your check", 21)], v.visible)!;
    expect(edgeText(cue)).toBe("Past the edge: Your check · 21 need you, Testing");
    expect(cue.target).toBe("your-check");
  });
});

describe("watching the board (performance)", () => {
  afterEach(() => vi.unstubAllGlobals());
  it("measures once per frame on resize and on a passive scroll, watches each column, and lets go", () => {
    const frames = new Map<number, () => void>();
    let next = 1;
    const ro = { observed: new Set<unknown>(), disconnected: false, fire: () => {} };
    vi.stubGlobal("requestAnimationFrame", (f: () => void) => {
      frames.set(next, f);
      return next++;
    });
    vi.stubGlobal("cancelAnimationFrame", (id: number) => void frames.delete(id));
    vi.stubGlobal("ResizeObserver", class {
      constructor(cb: () => void) {
        ro.fire = cb;
      }
      observe(t: unknown) {
        ro.observed.add(t);
      }
      unobserve(t: unknown) {
        ro.observed.delete(t);
      }
      disconnect() {
        ro.disconnected = true;
        ro.observed.clear();
      }
    });
    vi.stubGlobal("MutationObserver", class {
      observe() {}
      disconnect() {}
    });
    const run = () => {
      const fs = [...frames.values()];
      frames.clear();
      fs.forEach((f) => f());
    };
    const listeners = new Map<string, { fn: () => void; opts: unknown }>();
    const column = { nodeType: 1 };
    const el = {
      ...board({ width: VIEW, scroll: 1042, cols: ltr() }),
      children: [column],
      addEventListener: (type: string, fn: () => void, opts: unknown) => void listeners.set(type, { fn, opts }),
      removeEventListener: (type: string, fn: () => void) => {
        if (listeners.get(type)?.fn === fn) listeners.delete(type);
      },
    };
    const seen: unknown[] = [];
    const stop = watchBoard(el as unknown as HTMLElement, (v) => seen.push(v));
    expect(listeners.get("scroll")?.opts).toEqual({ passive: true });
    expect([...ro.observed]).toEqual([el, column]);
    run();
    expect(seen).toEqual([{ overflow: true, visible: ids(ADHD8).slice(0, 6), cut: ["your-check"] }]);
    listeners.get("scroll")?.fn();
    ro.fire();
    expect(frames.size).toBe(1);
    run();
    expect(seen).toHaveLength(2);
    ro.fire();
    stop();
    expect(listeners.size).toBe(0);
    expect(ro.disconnected).toBe(true);
    expect(frames.size).toBe(0);
  });
});

describe("how the cue fits its slot: whole names, two lines, or counted (review R1, R2)", () => {
  it("one line while every name fits, a pixel of rounding included", () => {
    expect(edgeFit({ room: 400, line: 400, lines: 1, over: false })).toBe("line");
    expect(edgeFit({ room: 400, line: 401, lines: 2, over: false })).toBe("line");
  });
  it("two lines, broken between words, when one is too long", () => {
    expect(edgeFit({ room: 400, line: 402, lines: 2, over: false })).toBe("wrap");
    expect(edgeFit({ room: 180, line: 520, lines: 1, over: false })).toBe("wrap");
  });
  it("counted when even two lines can't hold the names, or one word is wider than the slot", () => {
    expect(edgeFit({ room: 180, line: 520, lines: 3, over: false })).toBe("count");
    expect(edgeFit({ room: 120, line: 520, lines: 2, over: true })).toBe("count");
  });
  const probe = (o: Partial<Record<"offsetWidth" | "clientWidth" | "clientHeight" | "scrollWidth", number>>) => ({ offsetWidth: 0, clientWidth: 0, clientHeight: 0, scrollWidth: 0, ...o });
  it("reads the slot's width and its probes: the one-line cue's width, the two-line one's lines and overflow", () => {
    const wrap2 = probe({ clientWidth: 300, scrollWidth: 300, clientHeight: 26 });
    expect(measureEdge({ clientWidth: 520, line: probe({ offsetWidth: 520 }), wrap: wrap2, lineHeight: 13 })).toBe("line");
    expect(measureEdge({ clientWidth: 300, line: probe({ offsetWidth: 520 }), wrap: wrap2, lineHeight: 13 })).toBe("wrap");
    // Three lines of 13px.
    expect(measureEdge({ clientWidth: 200, line: probe({ offsetWidth: 520 }), wrap: probe({ clientWidth: 200, scrollWidth: 200, clientHeight: 39 }), lineHeight: 13 })).toBe("count");
    // Two lines, but a word sticks out past the slot.
    expect(measureEdge({ clientWidth: 90, line: probe({ offsetWidth: 520 }), wrap: probe({ clientWidth: 90, scrollWidth: 104, clientHeight: 26 }), lineHeight: 13 })).toBe("count");
    // No line height to count by: never claims the words fit on two lines.
    expect(measureEdge({ clientWidth: 300, line: probe({ offsetWidth: 520 }), wrap: wrap2, lineHeight: 0 })).toBe("count");
  });
  it("takes the slot from the chips' room only while it's on their line (review R4)", () => {
    expect(slotReserve({ minWidth: 169, top: 4 }, { bottom: 32 }, 6)).toBe(175);
    // Narrow, the slot has the line under the chips: it takes nothing from them.
    expect(slotReserve({ minWidth: 169, top: 32 }, { bottom: 32 }, 6)).toBe(0);
    expect(slotReserve(null, { bottom: 32 }, 6)).toBe(0);
  });
});

describe("watching the cue's slot (performance)", () => {
  afterEach(() => vi.unstubAllGlobals());
  it("measures once per frame when the slot or a probe changes size, picks the form, and lets go", () => {
    const frames = new Map<number, () => void>();
    let next = 1;
    const ro = { observed: [] as unknown[], disconnected: false, fire: () => {} };
    vi.stubGlobal("requestAnimationFrame", (f: () => void) => {
      frames.set(next, f);
      return next++;
    });
    vi.stubGlobal("cancelAnimationFrame", (id: number) => void frames.delete(id));
    vi.stubGlobal("getComputedStyle", () => ({ lineHeight: "13px" }));
    vi.stubGlobal("ResizeObserver", class {
      constructor(cb: () => void) {
        ro.fire = cb;
      }
      observe(t: unknown) {
        ro.observed.push(t);
      }
      disconnect() {
        ro.disconnected = true;
      }
    });
    const run = () => {
      const fs = [...frames.values()];
      frames.clear();
      fs.forEach((f) => f());
    };
    const line = { offsetWidth: 520 };
    const wrap = { clientWidth: 300, scrollWidth: 300, clientHeight: 26 };
    const slot = { clientWidth: 600, querySelector: (s: string) => (s === ".col-edge-probe-line" ? line : s === ".col-edge-probe-wrap" ? wrap : null) };
    const seen: string[] = [];
    const stop = watchEdge(slot as unknown as HTMLElement, (f) => seen.push(f));
    expect(ro.observed).toEqual([slot, line, wrap]);
    run();
    expect(seen).toEqual(["line"]);
    // The pane opens: the slot is 300px.
    slot.clientWidth = 300;
    ro.fire();
    ro.fire();
    expect(frames.size).toBe(1);
    run();
    expect(seen).toEqual(["line", "wrap"]);
    ro.fire();
    stop();
    expect(ro.disconnected).toBe(true);
    expect(frames.size).toBe(0);
    // No probes (no cue): nothing to watch.
    expect(watchEdge({ querySelector: () => null } as unknown as HTMLElement, () => seen.push("x"))()).toBeUndefined();
    expect(seen).toHaveLength(2);
  });
});

describe("in Persian (AC4)", () => {
  afterEach(async () => {
    await setLocale("en");
  });
  it("says it in Persian, the list with Persian commas, one sentence each", async () => {
    await setLocale("fa");
    const cue = edgeCue([col("ready", "آماده"), col("your-check", "بررسی شما", 21), col("done", "انجام شد")], ["ready"])!;
    const text = edgeText(cue);
    expect(text.startsWith("بیرون از لبه‌ی بورد: ")).toBe(true);
    expect(text).toContain("بررسی شما");
    expect(text).toContain("مورد به شما نیاز دارد");
    expect(text).toContain("، ");
    expect(text).not.toMatch(/[A-Za-z]/);
    const words = edgeWords(cue);
    expect(words).toContain("ستون بیرون از لبه‌ی بورد");
    expect(words).toContain("؛ ");
    expect(words.endsWith("نمایش آن‌ها")).toBe(true);
  });
});
