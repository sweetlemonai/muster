// The board cue's click, run (mu-198, review R3): the scroll it asks of the board, left to right,
// right to left and with less motion, and what the row's live region then says. No DOM here: React's
// hooks are stubbed so the row can be walked as plain elements (as SettingsLanguageClick.test.tsx
// walks its view), the cue's own onClick called, and the board is a fake with the boxes WebKit gives.
import { isValidElement, type ReactElement, type ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

/** What the stubbed useState hands out: a seed in place of an initial value, and every set. */
const hooks = { seeds: new Map<unknown, unknown>(), sets: [] as { init: unknown; value: unknown }[] };
vi.mock("react", async (real) => {
  const r = await real<typeof import("react")>();
  const useState = (init: unknown) => [hooks.seeds.has(init) ? hooks.seeds.get(init) : init, (value: unknown) => void hooks.sets.push({ init, value })];
  return { ...r, useState, useRef: (current: unknown) => ({ current }), useEffect: () => {}, useMemo: (f: () => unknown) => f(), useSyncExternalStore: (_s: unknown, get: () => unknown) => get() };
});

const { default: ColumnRow, edgeSaid, showColumn } = await import("./ColumnRow");
const { ALL_IN_VIEW } = await import("./rowCue");
const { setLocale } = await import("./i18n/runtime");

type Props = { children?: ReactNode; onClick?: () => void; className?: string };

function elements(node: ReactNode): ReactElement<Props>[] {
  if (Array.isArray(node)) return node.flatMap(elements);
  if (!isValidElement<Props>(node)) return [];
  if (typeof node.type === "function") return elements((node.type as (p: Props) => ReactNode)(node.props));
  return [node, ...elements(node.props.children)];
}

/** ADHD at 1440 (the spike's numbers): eight columns 118px wide, 10px apart, after 14px of padding,
 *  in an 851px view at x `left`, `edge` px of border before it (clientLeft). Right to left the first
 *  is at the right. */
function board(o: { rtl?: boolean; left?: number; edge?: number; still?: boolean }) {
  const left = o.left ?? 334;
  const edge = o.edge ?? 0;
  const at = (i: number) => (o.rtl ? { left: left + edge + 851 - 132 - 128 * i, right: left + edge + 851 - 14 - 128 * i } : { left: left + edge + 14 + 128 * i, right: left + edge + 132 + 128 * i });
  const ids = ["paused", "backlog", "shaping", "ready", "building", "testing", "your-check", "done"];
  const scrolls: unknown[] = [];
  const asked: string[] = [];
  const el = {
    clientWidth: 851,
    clientLeft: edge,
    getBoundingClientRect: () => ({ left, right: left + 851 + 2 * edge }),
    querySelector: (sel: string) => {
      asked.push(sel);
      const i = ids.findIndex((id) => sel === `[data-col="${id}"]`);
      return i < 0 ? null : { getBoundingClientRect: () => at(i) };
    },
    scrollBy: (o: unknown) => void scrolls.push(o),
  };
  vi.stubGlobal("CSS", { escape: (s: string) => s });
  vi.stubGlobal("getComputedStyle", () => ({ paddingInlineStart: "14px" }));
  vi.stubGlobal("window", { matchMedia: (q: string) => ({ matches: q === "(prefers-reduced-motion: reduce)" && !!o.still }) });
  return { el: el as unknown as HTMLElement, scrolls, asked };
}

const COLUMNS = [
  { id: "paused", label: "Paused", needs: 1 },
  { id: "backlog", label: "Backlog", needs: 0 },
  { id: "shaping", label: "Shaping", needs: 2 },
  { id: "ready", label: "Ready", needs: 0 },
  { id: "building", label: "Building", needs: 0 },
  { id: "testing", label: "Testing", needs: 0 },
  { id: "your-check", label: "Your check", needs: 21 },
  { id: "done", label: "Done", needs: 0 },
];

afterEach(async () => {
  await setLocale("en");
  vi.unstubAllGlobals();
  hooks.seeds.clear();
  hooks.sets.length = 0;
});

describe("showColumn: where a click scrolls the board", () => {
  it("left to right, Your check's left edge to the view's start inside the board's padding, smoothly", () => {
    const b = board({});
    showColumn(b.el, "your-check");
    expect(b.asked).toEqual(['[data-col="your-check"]']);
    expect(b.scrolls).toEqual([{ left: 768, behavior: "smooth" }]);
  });
  it("right to left (فارسی), its right edge to the view's right one: a scroll towards the left", async () => {
    await setLocale("fa");
    const b = board({ rtl: true, left: 0 });
    showColumn(b.el, "your-check");
    expect(b.scrolls).toEqual([{ left: -768, behavior: "smooth" }]);
  });
  it("counts the board's border (clientLeft) in the view's start, both ways", async () => {
    const ltr = board({ edge: 2 });
    showColumn(ltr.el, "your-check");
    expect(ltr.scrolls).toEqual([{ left: 768, behavior: "smooth" }]);
    await setLocale("fa");
    const rtl = board({ rtl: true, left: 0, edge: 2 });
    showColumn(rtl.el, "your-check");
    expect(rtl.scrolls).toEqual([{ left: -768, behavior: "smooth" }]);
  });
  it("without animation when the system asks for less motion", () => {
    const b = board({ still: true });
    showColumn(b.el, "done");
    expect(b.scrolls).toEqual([{ left: 896, behavior: "auto" }]);
  });
  it("does nothing for a column that isn't on the board", () => {
    const b = board({});
    showColumn(b.el, "gone");
    expect(b.scrolls).toEqual([]);
  });
});

describe("the cue's click in the row", () => {
  /** The row as drawn with the board measured: the first six columns in view, Your check cut. */
  function rowWith(b: ReturnType<typeof board>) {
    hooks.seeds.set(ALL_IN_VIEW, { overflow: true, visible: COLUMNS.slice(0, 6).map((c) => c.id), cut: ["your-check"] });
    return elements(ColumnRow({ chips: [], onOpen: () => {}, board: b.el, columns: COLUMNS }));
  }
  it("scrolls the board to Your check, walks on from it and says so in the live region", () => {
    const b = board({});
    const cue = rowWith(b).find((e) => e.type === "button" && e.props.className === "ui-pill col-cue col-edge");
    expect(cue).toBeDefined();
    cue!.props.onClick!();
    expect(b.scrolls).toEqual([{ left: 768, behavior: "smooth" }]);
    // The walk's last target, then the words, by the column's name, not its id.
    expect(hooks.sets).toEqual([
      { init: null, value: "your-check" },
      { init: "", value: "Your check is in view" },
    ]);
  });
  it("right to left (فارسی) the same click scrolls the other way", async () => {
    await setLocale("fa");
    const b = board({ rtl: true, left: 0 });
    rowWith(b).find((e) => e.type === "button" && e.props.className === "ui-pill col-cue col-edge")!.props.onClick!();
    expect(b.scrolls).toEqual([{ left: -768, behavior: "smooth" }]);
  });
  it("the live region's words come from the column's label, its id only when it's gone", () => {
    expect(edgeSaid(COLUMNS, "shaping")).toBe("Shaping is in view");
    expect(edgeSaid(COLUMNS, "gone")).toBe("gone is in view");
  });
});
