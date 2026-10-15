// The board's places right to left (mu-147): what the board, views and sessions need from
// styles.css so Persian reads on its own side and never moves a control. The screenshots in the
// task's attachments show them; these hold the rules in place.
import { describe, expect, it } from "vitest";
import css from "./styles.css?raw";

/** The declarations of the rule whose selector list is exactly `sel`, at the start of a line. */
function rule(sel: string): string {
  const at = css.indexOf(`\n${sel} {`);
  expect(at, `${sel} is in styles.css`).toBeGreaterThanOrEqual(0);
  return css.slice(at + sel.length + 3, css.indexOf("}", at));
}

describe("the board right to left", () => {
  it("never letter-spaces Persian, whatever head, band or chip asks for it", () => {
    expect(css).toMatch(/:root:lang\(fa\) \* \{ letter-spacing: normal !important; \}/);
    // The rules it overrides are still there for English.
    expect(rule(".tasks-table th button")).toContain("letter-spacing: 0.5px");
  });

  it("puts the card's stripe on the reading side, as its corner ribbon is", () => {
    expect(rule(".card")).not.toContain("border-left:");
    expect(css).toContain("border-inline-start: var(--card-left-w, 5px) solid var(--card-left, var(--card-c));");
  });

  it("starts the table's heads, the graph's node words and the level's parent field on the reading side", () => {
    expect(rule(".tasks-table th")).toContain("text-align: start");
    expect(rule(".tasks-table th button")).toContain("text-align: start");
    expect(rule(".g-node")).toContain("text-align: start");
    expect(rule(".tasks-table .rollup")).toContain("margin-inline-start");
    expect(rule(".level-pick .parent-field")).toContain("margin-inline-start");
  });

  it("keeps the column's Fold and the grid's Only what needs me on one line in any language", () => {
    expect(rule(".column h2 .col-hide")).toContain("white-space: nowrap");
    expect(rule(".fg-mine-pick")).toContain("white-space: nowrap");
  });

  it("points the recap's fold arrow along the reading direction", () => {
    expect(rule(".recap-fold::before")).toContain('"▸ "');
    expect(rule(".recap-fold:dir(rtl)::before")).toContain('"◂ "');
    expect(rule('.recap-fold[aria-expanded="true"]:dir(rtl)::before')).toContain('"▾ "');
  });
});

describe("glyphs that point along the line (mu-149)", () => {
  const at = css.indexOf("/* Glyphs that point along the line");
  const block = css.slice(at);
  /** The selector lists of the rules in `text` that name :dir(rtl). */
  const rtlRules = (text: string) => [...text.matchAll(/\n([.:#[\w][^\n{}]*:dir\(rtl\)[^\n{}]*) \{/g)].map((m) => m[1]);

  it("are mirrored in one block, and only layout, not glyphs, is mirrored outside it", () => {
    expect(at, "the block is in styles.css").toBeGreaterThan(0);
    expect(rtlRules(css.slice(0, at))).toEqual([
      ".tl-track:dir(rtl), .tl-seg:dir(rtl)",
      ".graph-canvas svg:dir(rtl)",
      ".settings-group.on:dir(rtl)",
      '.role-row[aria-expanded="true"]:dir(rtl)',
    ]);
  });

  it("gives every ▸ drawn by CSS its ◂ right to left, and keeps the open ▾", () => {
    const right = [...css.matchAll(/\n([^\n{]+)::before \{ content: "▸( ?)"/g)].map((m) => [m[1], m[2]] as const);
    expect(right.map(([sel]) => sel)).toEqual([".fold-head", ".band-fold", ".recap-fold"]);
    for (const [sel, space] of right) expect(rule(`${sel}:dir(rtl)::before`)).toContain(`content: "◂${space}"`);
    // The same specificity as the folded ◂, so it has to come after it to win.
    expect(rule('.band-fold[aria-expanded="true"]:dir(rtl)::before')).toContain('content: "▾"');
    expect(block.indexOf('.band-fold[aria-expanded="true"]:dir(rtl)')).toBeGreaterThan(block.indexOf(".band-fold:dir(rtl)"));
  });

  it("flips the heading's drawn chevron and a text arrow in the markup, whose box never changes", () => {
    expect(rule(".side-cat-fold:dir(rtl)")).toContain("transform: scaleX(-1)");
    expect(rule(".dir-glyph:dir(rtl)")).toContain("transform: scaleX(-1)");
    // Inline-block in both directions: a transform needs a box, and en and fa get the same one.
    expect(rule(".dir-glyph")).toContain("display: inline-block");
  });

  it("runs time and dependency arrows toward the inline end", () => {
    expect(rule(".tl-track, .tl-seg")).toContain("--to-end: to right");
    expect(rule(".tl-track:dir(rtl), .tl-seg:dir(rtl)")).toContain("--to-end: to left");
    expect(rule(".tl-row .tl-track")).toContain("repeating-linear-gradient(var(--to-end)");
    expect(rule(".tl-seg.fuzzy")).toContain("linear-gradient(var(--to-end)");
    expect(rule(".tl-seg.s-your-check")).toContain("repeating-linear-gradient(var(--to-end)");
    expect(rule(".tl-now-layer")).toContain("inset-inline-start: var(--label)");
    expect(rule(".graph-canvas svg:dir(rtl)")).toContain("transform: scaleX(-1)");
  });
});

describe("right to left changes no colour (mu-149)", () => {
  const code = css.replace(/\/\*[\s\S]*?\*\//g, "");
  const rules = [...code.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((m) => ({ sel: m[1].trim(), body: m[2] }));
  const decls = (body: string) =>
    body
      .split(";")
      .map((d) => /^\s*([-\w]+)\s*:\s*([\s\S]*?)\s*$/.exec(d))
      .filter((d) => d !== null)
      .map((d) => [d[1], d[2].replace(/\s*!important$/, "")] as const);
  const RTL_ONLY = /:dir\(rtl\)|:lang\(fa\)|\[dir=["']?rtl["']?\]/;
  /** Anything that paints: a colour property, a shorthand that can carry one, a colour in a variable. */
  const COLOUR = /^(color|background(-color|-image)?|border(-[a-z]+)*(-color)?|outline(-color)?|fill|stroke|caret-color|accent-color|text-decoration(-color)?|text-shadow|column-rule(-color)?|filter)$/;
  const A_COLOUR = /#[0-9a-f]{3,8}\b|\b(rgba?|hsla?|hwb|lab|lch|oklab|oklch|color-mix|color)\(|var\(--/i;
  /** A shadow with its x offsets' signs dropped: what's left is the same when only the side differs. */
  const unsided = (v: string) => v.replace(/-(?=[\d.])/g, "");

  /** What each rule right to left or in فارسی sets that would change a colour from left to right. */
  function colours(list: { sel: string; body: string }[]): string[] {
    const out: string[] = [];
    for (const { sel, body } of list.filter((r) => RTL_ONLY.test(r.sel))) {
      const ltr = sel.replace(new RegExp(RTL_ONLY.source, "g"), "");
      for (const [p, v] of decls(body)) {
        if (p.startsWith("--") ? A_COLOUR.test(v) : COLOUR.test(p)) out.push(`${sel}: ${p}: ${v}`);
        if (p !== "box-shadow") continue;
        // A side's mark set again for the other side: the same shadow, so the same colour.
        const mirrored = list.some((r) => r.sel === ltr && decls(r.body).some(([q, w]) => q === p && unsided(w) === unsided(v)));
        if (!mirrored) out.push(`${sel}: ${p}: ${v}`);
      }
    }
    return out;
  }

  it("flags a colour set only right to left and passes a mirrored mark", () => {
    const sheet = [
      { sel: ".a:dir(rtl)", body: " color: red; transform: scaleX(-1); " },
      { sel: ":root:lang(fa) .b", body: " background: var(--panel); " },
      { sel: '[dir="rtl"] .c', body: " --edge: var(--accent); --to-end: to left; " },
      { sel: ".d", body: " box-shadow: inset 3px 0 0 var(--accent); " },
      { sel: ".d:dir(rtl)", body: " box-shadow: inset -3px 0 0 var(--accent); " },
      { sel: ".e:dir(rtl)", body: " box-shadow: inset -3px 0 0 var(--bad); " },
      { sel: ".f:dir(rtl)", body: " border-inline-start: 1px solid; letter-spacing: normal !important; " },
    ];
    expect(colours(sheet)).toEqual([
      ".a:dir(rtl): color: red",
      ":root:lang(fa) .b: background: var(--panel)",
      '[dir="rtl"] .c: --edge: var(--accent)',
      ".e:dir(rtl): box-shadow: inset -3px 0 0 var(--bad)",
      ".f:dir(rtl): border-inline-start: 1px solid",
    ]);
  });

  it("in styles.css: every :dir(rtl) and :lang(fa) rule moves or turns things, never paints them", () => {
    expect(rules.filter((r) => RTL_ONLY.test(r.sel)).length).toBeGreaterThan(5);
    expect(colours(rules)).toEqual([]);
  });
});
