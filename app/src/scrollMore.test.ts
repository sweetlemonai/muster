import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import appSource from "./App.tsx?raw";
import groupsSource from "./Groups.tsx?raw";
import css from "./styles.css?raw";
import ColumnCards, { MoreCount } from "./ColumnCards";
import { measureMore, moreAround, NO_MORE, sameMore, watchMore } from "./scrollMore";

// Five 100px cards with 8px gaps from the list's 4px top padding: tops 4, 112, 220, 328, 436.
const CARDS = [4, 112, 220, 328, 436].map((top) => ({ top, height: 100 }));

describe("counting the cards out of sight in a column (owner: hide the scrollbar, keep 'more below')", () => {
  it("counts what's below at the top of a list that overflows", () => {
    // 250px of view shows the first two cards whole and the third in part.
    expect(moreAround(0, 250, CARDS)).toEqual({ above: 0, below: 3 });
  });
  it("counts both ways in the middle, a card cut by an edge counting on that side", () => {
    // View 150..400: card 2 (112..212) is cut at the top, card 4 (328..428) at the bottom.
    expect(moreAround(150, 250, CARDS)).toEqual({ above: 2, below: 2 });
  });
  it("counts nothing below at the end, nothing above at the top", () => {
    const end = 436 + 100 + 10 - 250; // the last card's bottom plus the list's 10px padding
    expect(moreAround(end, 250, CARDS)).toEqual({ above: 3, below: 0 });
    expect(moreAround(0, 1000, CARDS)).toEqual({ above: 0, below: 0 });
  });
  it("doesn't count a card the edge cuts by under a pixel (rounding)", () => {
    expect(moreAround(0, 211.5, [{ top: 4, height: 100 }, { top: 112, height: 100 }])).toEqual({ above: 0, below: 0 });
    expect(moreAround(4.6, 300, [{ top: 4, height: 100 }])).toEqual({ above: 0, below: 0 });
  });
  it("counts nothing for an empty or hidden list, and survives the trackpad's bounce past the ends", () => {
    expect(moreAround(0, 250, [])).toEqual(NO_MORE);
    expect(moreAround(0, 0, CARDS)).toEqual(NO_MORE); // folded or empty column: display none
    expect(moreAround(-30, 250, CARDS)).toEqual({ above: 0, below: 3 });
    expect(moreAround(900, 250, CARDS)).toEqual({ above: 5, below: 0 });
  });
  it("compares two results by value, so an unchanged count doesn't re-render", () => {
    expect(sameMore({ above: 1, below: 2, overflow: true }, { above: 1, below: 2, overflow: true })).toBe(true);
    expect(sameMore({ above: 1, below: 2, overflow: true }, { above: 1, below: 2, overflow: false })).toBe(false);
  });
});

/** A list element as far as the measuring reads it: its box, its scroll and its cards' boxes. */
const list = (o: { top: number; scrollTop: number; clientHeight: number; scrollHeight: number; cards: { top: number; height: number }[] }) => ({
  scrollTop: o.scrollTop,
  clientHeight: o.clientHeight,
  scrollHeight: o.scrollHeight,
  clientTop: 0,
  getBoundingClientRect: () => ({ top: o.top }),
  querySelectorAll: (sel: string) => {
    expect(sel).toBe(".card, .note-card");
    return o.cards.map((c) => ({ getBoundingClientRect: () => c }));
  },
});

describe("measuring a column's list", () => {
  it("reads the cards' boxes in the list's own coordinates and says whether it overflows", () => {
    // The list sits at y=200 on screen, scrolled by 150: card tops on screen are 200 + top - 150.
    const el = list({ top: 200, scrollTop: 150, clientHeight: 250, scrollHeight: 546, cards: CARDS.map((c) => ({ top: 200 + c.top - 150, height: c.height })) });
    expect(measureMore(el)).toEqual({ above: 2, below: 2, overflow: true });
  });
  it("says no overflow when everything fits, so no strip is drawn", () => {
    const el = list({ top: 0, scrollTop: 0, clientHeight: 600, scrollHeight: 600, cards: CARDS });
    expect(measureMore(el)).toEqual({ above: 0, below: 0, overflow: false });
  });
});

/** Fakes for the browser pieces watchMore uses: requestAnimationFrame, ResizeObserver, MutationObserver. */
function browser() {
  const frames = new Map<number, () => void>();
  let next = 1;
  const ro = { observed: new Set<unknown>(), disconnected: false, fire: () => {} };
  const mo = { disconnected: false, fire: (_records: unknown[]) => {} };
  vi.stubGlobal("requestAnimationFrame", (f: () => void) => { frames.set(next, f); return next++; });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => void frames.delete(id));
  vi.stubGlobal("ResizeObserver", class {
    constructor(cb: () => void) { ro.fire = cb; }
    observe(t: unknown) { ro.observed.add(t); }
    unobserve(t: unknown) { ro.observed.delete(t); }
    disconnect() { ro.disconnected = true; ro.observed.clear(); }
  });
  vi.stubGlobal("MutationObserver", class {
    constructor(cb: (r: unknown[]) => void) { mo.fire = cb; }
    observe() {}
    disconnect() { mo.disconnected = true; }
  });
  const run = () => { const fs = [...frames.values()]; frames.clear(); fs.forEach((f) => f()); };
  return { frames, ro, mo, run };
}

function fakeList(children: unknown[]) {
  const listeners = new Map<string, { fn: () => void; opts: unknown }>();
  const el = {
    ...list({ top: 0, scrollTop: 0, clientHeight: 250, scrollHeight: 546, cards: CARDS }),
    children,
    addEventListener: (type: string, fn: () => void, opts: unknown) => void listeners.set(type, { fn, opts }),
    removeEventListener: (type: string, fn: () => void) => { if (listeners.get(type)?.fn === fn) listeners.delete(type); },
  };
  return { el, listeners };
}

describe("watching a column's list", () => {
  afterEach(() => vi.unstubAllGlobals());
  it("measures once per frame however many scroll and resize events come, with a passive listener", () => {
    const b = browser();
    const { el, listeners } = fakeList([]);
    const seen: unknown[] = [];
    watchMore(el as unknown as HTMLElement, (m) => seen.push(m));
    expect(listeners.get("scroll")?.opts).toEqual({ passive: true });
    b.run(); // the first measure
    expect(seen).toEqual([{ above: 0, below: 3, overflow: true }]);
    listeners.get("scroll")?.fn();
    listeners.get("scroll")?.fn();
    b.ro.fire();
    expect(b.frames.size).toBe(1);
    b.run();
    expect(seen).toHaveLength(2);
  });
  it("watches the list and its children's sizes, following children that come and go", () => {
    const b = browser();
    const first = { nodeType: 1 }, second = { nodeType: 1 }, text = { nodeType: 3 };
    const { el } = fakeList([first]);
    watchMore(el as unknown as HTMLElement, () => {});
    expect([...b.ro.observed]).toEqual([el, first]);
    b.mo.fire([{ addedNodes: [second, text], removedNodes: [first] }]);
    expect([...b.ro.observed]).toEqual([el, second]);
    expect(b.frames.size).toBe(1);
  });
  it("lets everything go when the column unmounts", () => {
    const b = browser();
    const { el, listeners } = fakeList([]);
    const stop = watchMore(el as unknown as HTMLElement, () => {});
    stop();
    expect(listeners.size).toBe(0);
    expect(b.ro.disconnected && b.mo.disconnected).toBe(true);
    expect(b.frames.size).toBe(0);
  });
});

describe("the column's list on screen", () => {
  it("keeps the cards in a .cards list, with no count before anything is measured", () => {
    const html = renderToStaticMarkup(createElement(ColumnCards, null, createElement("i", { className: "card" }, "0001")));
    expect(html).toBe('<div class="cards"><i class="card">0001</i></div>');
  });
  it("says +N below and +N above in a button that scrolls a screen, and keeps its place empty at an end", () => {
    const below = renderToStaticMarkup(createElement(MoreCount, { side: "below", n: 3, onPage: () => {} }));
    expect(below).toMatch(/^<div class="col-more col-more-below"><button type="button" class="more-count"[^>]*>\+3 below<\/button><\/div>$/);
    expect(below).toContain('title="Scroll down one screen"');
    expect(renderToStaticMarkup(createElement(MoreCount, { side: "above", n: 2, onPage: () => {} }))).toContain(">+2 above</button>");
    expect(renderToStaticMarkup(createElement(MoreCount, { side: "below", n: 0, onPage: () => {} }))).toBe('<div class="col-more col-more-below"></div>');
  });
  it("is how every board column draws its cards, grouped or not", () => {
    for (const src of [appSource, groupsSource]) {
      expect(src).not.toMatch(/className="cards"/);
      expect(src).toMatch(/<ColumnCards>/);
    }
  });
});

describe("the board's columns scroll without a scrollbar (owner, 2026-10-13)", () => {
  const rule = (selector: string) => css.match(new RegExp(`(?:^|\\n)${selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")} \\{([^}]*)\\}`))?.[1] ?? "";
  it("hides the scrollbar on the board's card lists only, in WebKit too, and keeps them scrolling", () => {
    expect(rule(".board .cards")).toContain("scrollbar-width: none");
    expect(rule(".board .cards::-webkit-scrollbar")).toContain("display: none");
    expect(rule(".cards")).toContain("overflow-y: auto");
    // Nothing else loses its scrollbar: no global rule.
    expect(css).not.toMatch(/(^|\n)(\*|html|body|:root)[^{\n]*::-webkit-scrollbar/);
  });
  it("fades the edge that has more and draws the count in the theme's muted text, never a stage colour", () => {
    expect(rule(".cards.fade-bottom")).toContain("mask-image");
    expect(rule(".cards.fade-top")).toContain("mask-image");
    expect(rule(".more-count")).toContain("color: var(--muted)");
    expect(rule(".more-count")).not.toMatch(/--st-|--status/);
    // The strip's height is fixed, so the count coming and going moves nothing.
    expect(rule(".col-more")).toMatch(/height: \d+px/);
  });
});
