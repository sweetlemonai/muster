// The header's toolbar never clips (mu-202). New task and the views share line one; Help,
// Settings, the account meter, the mode chip and live are one trailing unit that drops whole to
// line two, at its inline end, when the line is too narrow. The counts keep three digits' room, so
// a poll never changes where it wraps. Headless WebKit shots prove the boxes; these pin the rules.
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import appSource from "./App.tsx?raw";
import { HeadCount, HeadCounts } from "./HeadCount";
import css from "./styles.css?raw";

const esc = (s: string) => s.replace(/[.*+?^${}()|[\]\\>]/g, "\\$&");
/** Every rule written for exactly this selector, joined. */
const rules = (sel: string) => [...css.matchAll(new RegExp(`(?:^|\\n)${esc(sel)} \\{([^}]*)\\}`, "g"))].map((m) => m[1]).join(";");
const header = /<header className="top">([\s\S]*?)<\/header>/.exec(appSource)?.[1] ?? "";
/** The source between an opening tag and its matching close (no nesting of the same tag inside). */
const inside = (open: string, close: string) => {
  const at = header.indexOf(open);
  return at < 0 ? "" : header.slice(at + open.length, header.indexOf(close, at));
};

describe("the toolbar's groups, in DOM order (mu-202)", () => {
  it("starts with New task and the views in one unit that keeps them on one line", () => {
    expect(header).toMatch(/^\s*(?:\{\/\*[^]*?\*\/\}\s*)?<span className="top-start">\s*<Button variant="primary" className="new"[^]*?<nav className="views" aria-label=\{t\("app\.views"\)\}>/);
    const start = rules(".top-start");
    expect(start).toContain("display: flex;");
    expect(start).toContain("flex: 1 1 auto;");
    // Never wider than the header, so it always shares a line with nothing but itself at worst.
    expect(start).toContain("max-width: 100%;");
    expect(start).not.toMatch(/flex-wrap: wrap|overflow(?:-x)?: hidden/);
  });

  it("ends with one trailing unit right after the views: Help, Settings, then the status area", () => {
    const after = header.slice(header.indexOf("</nav>"));
    expect(after).toMatch(/^<\/nav>\s*<\/span>\s*(?:\{\/\*[^]*?\*\/\}\s*)?<span className="top-end">\s*<nav className="end-views" aria-label=\{t\("app\.helpAndSettings"\)\}>\s*<Pill on=\{view === "help"\}[^]*?<\/Pill>\s*<Pill on=\{view === "settings"\}[^]*?<\/Pill>\s*<\/nav>\s*<span className="top-status">/);
    // Help and Settings aren't in the views any more, and each is there once.
    const nav = inside('<nav className="views"', "</nav>");
    expect(nav).not.toMatch(/view === "help"|view === "settings"/);
    expect(header.match(/setView\("help"\)/g)).toHaveLength(1);
    expect(header.match(/setView\("settings"\)/g)).toHaveLength(1);
    // They're views too, so they keep a navigation landmark of their own, laid out like the views.
    expect(rules(".end-views")).toMatch(/display: flex;[^]*gap: 4px;[^]*flex: none;/);
    // Nothing after the trailing unit: it closes the header.
    expect(header).toMatch(/<\/span>\s*<\/span>\s*<\/span>\s*$/);
  });

  it("keeps the trailing unit whole: it drops to line two as one piece, at the line's end", () => {
    const end = rules(".top-end");
    expect(end).toContain("display: flex;");
    expect(end).toContain("align-items: center;");
    // Placed on a line by its whole width (flex-basis auto), never wider than the header: only
    // when it alone is wider does it wrap inside, at its end, rather than run past the start.
    expect(end).toContain("flex: 0 1 auto;");
    expect(end).toContain("max-width: 100%;");
    expect(end).toContain("flex-wrap: wrap;");
    expect(end).toContain("justify-content: flex-end;");
    expect(end).not.toMatch(/overflow(?:-x)?: hidden/);
    // The status area inside it stays whole.
    expect(rules(".top-status")).toMatch(/display: flex;[^]*flex: none;/);
    // The header puts a wrapped line's items at its inline end (start or end follows the direction).
    expect(rules(".top")).toContain("justify-content: flex-end;");
    expect(rules(".top")).toContain("flex-wrap: wrap;");
  });

  it("uses logical sides, so the views sit beside the trailing unit in right to left too", () => {
    expect(rules(".views")).not.toContain("margin-left");
    expect(rules(".top .views")).toMatch(/flex: 0 1 auto;[^]*flex-wrap: wrap;/);
  });
});

describe("the toolbar's counts keep their room (mu-202)", () => {
  it("draws a count with the reserved-width class", () => {
    expect(renderToStaticMarkup(<HeadCount n={275} />)).toBe('<b class="count count-reserve">275</b>');
    expect(renderToStaticMarkup(<HeadCount n={9} className="warn" />)).toBe('<b class="count count-reserve warn">9</b>');
  });

  it("keeps the room at zero: an empty, hidden count that screen readers skip", () => {
    expect(renderToStaticMarkup(<HeadCount n={0} className="warn" />)).toBe('<b class="count count-reserve warn" aria-hidden="true"></b>');
  });

  it("keeps the quiet count's room too: always there, outlined, empty and hidden at zero", () => {
    expect(renderToStaticMarkup(<HeadCount n={3} quiet title="Unchecked" />)).toBe('<small class="count count-reserve quiet" title="Unchecked">3</small>');
    expect(renderToStaticMarkup(<HeadCount n={0} quiet title="Unchecked" />)).toBe('<small class="count count-reserve quiet" aria-hidden="true"></small>');
    // Its outline is inside the box, so it reserves the border's 2 px on top of three digits.
    expect(rules(".count-reserve.quiet")).toContain("min-width: calc(3ch + 12px);");
  });

  it("puts the shown counts first and the empty slots after them, never between label and count", () => {
    const counts = (p: number, u: number) => renderToStaticMarkup(<HeadCounts counts={[{ id: "p", n: p }, { id: "u", n: u, quiet: true }]} />);
    const hidden = '<b class="count count-reserve" aria-hidden="true"></b>';
    const hiddenQuiet = '<small class="count count-reserve quiet" aria-hidden="true"></small>';
    expect(counts(2, 3)).toBe('<b class="count count-reserve">2</b><small class="count count-reserve quiet">3</small>');
    expect(counts(2, 0)).toBe(`<b class="count count-reserve">2</b>${hiddenQuiet}`);
    expect(counts(0, 3)).toBe(`<small class="count count-reserve quiet">3</small>${hidden}`);
    expect(counts(0, 0)).toBe(hidden + hiddenQuiet);
  });

  it("is what Waiting for you and Problems show, the unchecked-visions count included", () => {
    const nav = inside('<nav className="views"', "</nav>");
    expect(nav).toMatch(/\{t\("app\.view\.waiting"\)\}<HeadCount n=\{needs\} className="warn" \/>/);
    expect(nav).toMatch(/\{t\("app\.view\.problems"\)\}<HeadCounts counts=\{\[\{ id: "problems", n: problems\.length \}, \{ id: "unchecked", n: unchecked\.length, quiet: true, title: t\("app\.view\.uncheckedTitle"\) \}\]\} \/>/);
    // No count in the toolbar comes and goes with the data.
    expect(nav).not.toMatch(/<b className="count|<small className="count|\? <small|: null\}/);
  });

  it("reserves three tabular digits and hides the empty one without giving up its place", () => {
    const r = rules(".count-reserve");
    expect(r).toContain("font-variant-numeric: tabular-nums;");
    // .count pads 5 px a side; border-box, so the minimum holds three digits plus that padding.
    expect(r).toContain("min-width: calc(3ch + 10px);");
    expect(r).toContain("text-align: center;");
    expect(rules(".count-reserve:empty")).toContain("visibility: hidden;");
    expect(rules(".count-reserve:empty")).not.toContain("display: none");
    // The empty one holds a zero-width text strut, so it sits on the same line box as a filled one.
    expect(rules(".count-reserve:empty::before")).toContain('content: "\\200b";');
  });
});
