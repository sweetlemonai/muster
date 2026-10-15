import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { GRAPH_MARKS } from "./graph";
import en from "./i18n/locales/en/board.json";
import fa from "./i18n/locales/fa/board.json";
import { GRAPH_RESERVE, graphLegend, Legend, type LegendItem, TIMELINE_RESERVE, timelineLegend } from "./Legend";
import legendSource from "./Legend.tsx?raw";
import css from "./styles.css?raw";
import { TIMELINE_MARKS } from "./viewData";
import viewDataSource from "./viewData.ts?raw";
import viewsSource from "./Views.tsx?raw";

const LEGEND_LABELS = en.legend;
const render = (items: readonly LegendItem[], reserve: readonly LegendItem[] = []) => renderToStaticMarkup(createElement(Legend, { items, reserve }));
const EMPTY_RESERVE = '<ul class="legend-list legend-reserve" aria-hidden="true"></ul>';
/** Each item as its swatch's markup and the words after it. */
const items = (html: string) => [...html.matchAll(/<li class="legend-item"><span class="legend-swatch" aria-hidden="true">(.*?)<\/span>([^<]*)<\/li>/g)].map((m) => [m[1], m[2]]);
/** One CSS rule's body. */
const rule = (sel: string) => new RegExp(`(?:^|\\n)${sel.replace(/[.*+?^${}()|[\]\\>]/g, "\\$&")} \\{([^}]*)\\}`).exec(css)?.[1] ?? "";

describe("the legend's names (mu-173)", () => {
  it("are plain English, one table for both views, in the board's text (board:legend)", () => {
    expect(LEGEND_LABELS).toEqual({
      label: "What the colours mean",
      backlog: "Backlog", shaping: "Shaping", ready: "Ready", building: "Building", testing: "Testing", "your-check": "Waiting for your check", done: "Done",
      fuzzy: "Start not certain", blocked: "Blocked", paused: "Paused", now: "Now",
      "ready-to-start": "Ready to start", human: "Your own task", loop: "A loop",
    });
  });
  it("have a name for every mark either view can draw, in English and Persian", () => {
    for (const m of [...TIMELINE_MARKS, ...GRAPH_MARKS]) {
      expect(LEGEND_LABELS[m], m).toBeTruthy();
      expect(fa.legend[m], m).toBeTruthy();
    }
    expect(Object.keys(fa.legend).sort()).toEqual(Object.keys(en.legend).sort());
  });
});

describe("the legend row (mu-173)", () => {
  it("is a labelled group of items, each a hidden swatch then its name as text", () => {
    const html = render(timelineLegend(["building", "now"]));
    expect(html).toBe(
      '<div class="legend" role="group" aria-label="What the colours mean"><ul class="legend-list">' +
        '<li class="legend-item"><span class="legend-swatch" aria-hidden="true"><span class="tl-bar s-building"><span class="tl-seg s-building"></span></span></span>Building</li>' +
        '<li class="legend-item"><span class="legend-swatch" aria-hidden="true"><span class="tl-now"></span></span>Now</li>' +
        `</ul>${EMPTY_RESERVE}</div>`,
    );
  });
  it("keeps its list, empty, when nothing is drawn", () => {
    expect(render([])).toBe(`<div class="legend" role="group" aria-label="What the colours mean"><ul class="legend-list"></ul>${EMPTY_RESERVE}</div>`);
  });
  it("reserves every item the view can draw in a hidden copy after the list, so its height never follows the window's marks (R4)", () => {
    const html = render(timelineLegend(["building"]), TIMELINE_RESERVE);
    const [shown, reserved] = html.split('<ul class="legend-list legend-reserve" aria-hidden="true">');
    expect(items(shown).map(([, name]) => name)).toEqual(["Building"]);
    expect(items(reserved).map(([, name]) => name)).toEqual(TIMELINE_MARKS.map((m) => LEGEND_LABELS[m]));
    // Nothing in the copy can take focus or be read out: aria-hidden, and no buttons, links or tabindex.
    expect(reserved).not.toMatch(/<(?:button|a|input)\b|tabindex/);
    const graph = render([], GRAPH_RESERVE).split('<ul class="legend-list legend-reserve" aria-hidden="true">')[1] ?? "";
    expect(items(graph).map(([, name]) => name)).toEqual(GRAPH_MARKS.map((m) => LEGEND_LABELS[m]));
    // The copy shares the list's grid cell, unseen and not hit by the mouse, and the visible list sits at the top.
    expect(rule(".legend")).toMatch(/display: grid;[^]*align-items: start;/);
    expect(rule(".legend > .legend-list")).toContain("grid-area: 1 / 1;");
    expect(rule(".legend-reserve")).toMatch(/visibility: hidden;[^]*pointer-events: none;/);
  });
  it("draws each timeline swatch with the chart's own classes", () => {
    const all = ["backlog", "shaping", "ready", "building", "testing", "your-check", "done", "fuzzy", "blocked", "paused", "now"] as const;
    const bar = (s: string) => `<span class="tl-bar s-${s}"><span class="tl-seg s-${s}"></span></span>`;
    expect(items(render(timelineLegend([...all])))).toEqual([
      ...["backlog", "shaping", "ready", "building", "testing"].map((s) => [bar(s), LEGEND_LABELS[s as (typeof all)[number]]]),
      [bar("your-check"), "Waiting for your check"],
      ['<span class="tl-bar s-done"></span>', "Done"],
      ['<span class="tl-bar lg-neutral"><span class="tl-seg lg-neutral fuzzy"></span></span>', "Start not certain"],
      ['<span class="tl-bar lg-neutral blocked"><span class="tl-block"></span></span>', "Blocked"],
      ['<span class="tl-bar lg-neutral"><span class="tl-pause"></span></span>', "Paused"],
      ['<span class="tl-now"></span>', "Now"],
    ]);
  });
  it("has no waiting part of a bar to draw or name (mu-188)", () => {
    // `.waiting` is the Waiting for you card; a `tl-seg waiting` would take its box and padding.
    for (const [name, src] of [["Views.tsx", viewsSource], ["Legend.tsx", legendSource], ["viewData.ts", viewDataSource]]) {
      expect(src, name).not.toMatch(/tl-seg waiting|"waiting"/);
    }
    expect(css).not.toMatch(/\.tl-seg\.waiting/);
    expect(Object.keys(en.legend)).not.toContain("waiting");
    expect(Object.keys(fa.legend)).not.toContain("waiting");
  });
  it("draws each graph swatch with the graph's own classes, and the You ribbon for the owner's task", () => {
    const all = ["backlog", "shaping", "ready", "building", "testing", "your-check", "done", "ready-to-start", "blocked", "human", "loop"] as const;
    expect(items(render(graphLegend([...all])))).toEqual([
      ...["backlog", "shaping", "ready", "building", "testing", "your-check", "done"].map((s) => [`<span class="g-node s-${s}"></span>`, LEGEND_LABELS[s as (typeof all)[number]]]),
      ['<span class="g-node ready"></span>', "Ready to start"],
      ['<span class="g-node blocked"></span>', "Blocked"],
      ['<span class="you-ribbon" title="Your own task: only you do it, agents never take it">You</span>', "Your own task"],
      ['<svg class="lg-edge" viewBox="0 0 24 12"><path class="g-edge loop" d="M1,6 L23,6"></path></svg>', "A loop"],
    ]);
  });
});

describe("the legend's layout (mu-173)", () => {
  it("wraps onto another line rather than scrolling anything out of sight (R3), and keeps a line's height when empty", () => {
    const legend = rule(".legend");
    for (const d of ["min-block-size: 20px", "font-size: 12px", "color: var(--muted)", "padding-inline: 14px"]) expect(legend).toContain(d);
    // Nothing in it is hidden: no fixed height, no clipping, no sideways scroll, no hidden scrollbar.
    expect(legend).not.toMatch(/(?:^|[\s;])block-size:|overflow|nowrap|scrollbar/);
    expect(css).not.toContain(".legend::-webkit-scrollbar");
    expect(rule(".legend-list")).toMatch(/display: flex;[^]*flex-wrap: wrap;[^]*row-gap: 4px;[^]*column-gap: 12px;[^]*list-style: none;/);
    // An item never splits: its swatch and name stay on one line together.
    expect(rule(".legend-item")).toMatch(/display: inline-flex;[^]*flex: none;[^]*white-space: nowrap;/);
    expect(rule(".legend-swatch")).toMatch(/position: relative;[^]*flex: none;/);
  });
  it("sizes and places swatches with logical properties only, so right to left works", () => {
    const rules = [...css.matchAll(/(?:^|\n)([^{}\n]*\.(?:legend|lg-)[^{}\n]*)\{([^}]*)\}/g)];
    expect(rules.length).toBeGreaterThan(5);
    for (const [, sel, body] of rules) expect(body, sel).not.toMatch(/(?:^|[\s;])(?:left|right|top|bottom|margin-(?:left|right|top|bottom)|padding-(?:left|right|top|bottom)|width|height)\s*:/);
  });
  it("has no rule left for the old prose legend", () => {
    expect(css).not.toContain(".tl-legend");
  });
});
