import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TextSize } from "./Appearance";
import { loadZoom, parseZoom, setZoom, stepZoom, ZOOM_KEY, ZOOMS, zoomDirection } from "./zoom";

const key = (k: string, meta = true, ctrl = false) => ({ key: k, metaKey: meta, ctrlKey: ctrl });

describe("text size (owner, 2026-10-13: zoom the help page and all pages)", () => {
  afterEach(() => vi.unstubAllGlobals());
  it("reads ⌘+ (also ⌘=), ⌘− and ⌘0, with ⌘ only", () => {
    expect([zoomDirection(key("=")), zoomDirection(key("+")), zoomDirection(key("-")), zoomDirection(key("0"))]).toEqual([1, 1, -1, 0]);
    expect(zoomDirection(key("=", false))).toBeNull(); // no ⌘
    expect(zoomDirection(key("=", false, true))).toBeNull(); // Ctrl alone
    expect(zoomDirection(key("a"))).toBeNull();
  });
  it("steps through the sizes and stops at the ends", () => {
    expect(stepZoom(1, 1)).toBe(1.15);
    expect(stepZoom(1.5, 1)).toBe(1.5);
    expect(stepZoom(0.9, -1)).toBe(0.9);
    expect(stepZoom(1.3, 0)).toBe(1);
  });
  it("reads a saved size back as the nearest step, 100% for anything odd", () => {
    expect([parseZoom("1.3"), parseZoom("1.29"), parseZoom(null), parseZoom("big"), parseZoom("-2"), parseZoom("9")]).toEqual([1.3, 1.3, 1, 1, 1, 1.5]);
    expect(loadZoom(() => { throw new Error("no storage"); })).toBe(1);
  });
  it("sets the size on the page and remembers it", () => {
    const store = new Map<string, string>();
    vi.stubGlobal("localStorage", { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => void store.set(k, v) });
    vi.stubGlobal("window", { dispatchEvent: () => true });
    vi.stubGlobal("CustomEvent", class { constructor(public type: string, public init: unknown) {} });
    const set: [string, string][] = [];
    expect(setZoom(1.29, { style: { setProperty: (k, v) => void set.push([k, v]) } })).toBe(1.3);
    expect(set).toEqual([["zoom", "1.3"]]);
    expect(store.get(ZOOM_KEY)).toBe("1.3");
  });
  it("shows every size as a chip in Appearance, the current one pressed", () => {
    const html = renderToStaticMarkup(createElement(TextSize, { initial: 1.15 }));
    expect(html.match(/class="ui-pill( ui-pill--on)?"/g)).toHaveLength(ZOOMS.length);
    expect(html).toMatch(/aria-pressed="true" class="ui-pill ui-pill--on">115%</);
  });
});
