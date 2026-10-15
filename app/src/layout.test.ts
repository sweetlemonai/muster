import { describe, expect, it } from "vitest";
import css from "./styles.css?raw";

/** One CSS rule's body (the selector as written, on its own line). */
const rule = (sel: string) => new RegExp(`(?:^|\\n)${sel.replace(/[.*+?^${}()|[\]\\>]/g, "\\$&")} \\{([^}]*)\\}`).exec(css)?.[1] ?? "";

// Layout defects the visual QA of 2026-10-13 found (task 0061), pinned where a headless render
// isn't part of the gate: each says what the rule must hold, not what it looks like.
describe("layout fixes (0061)", () => {
  it("the task pane's 'quiet 21 min' keeps its line instead of wrapping a word each", () => {
    const r = rule(".task-pane .holder > .meta, .task-pane .holder > .live-badge");
    expect(r).toContain("flex: none;");
    expect(r).toContain("white-space: nowrap;");
  });

  it("the Table's header sticks flush with the top: no padding above it for rows to scroll through", () => {
    expect(rule(".table-view")).toMatch(/padding: 0 14px 14px;/);
    expect(rule(".tasks-table th")).toMatch(/position: sticky;\s*top: 0;\s*background: var\(--bg\);/);
  });

  it("a folded column's strip lays its name, counts and Blocked out in a gapped line, not as overlapping inline-blocks", () => {
    // One rule for every strip, empty or folded (mu-83).
    const r = rule(".column.col-strip .col-fold, .column.col-strip .col-head");
    expect(r).toContain("display: flex;");
    expect(r).toContain("gap: 6px;");
    // Counts pad along the strip (top and bottom), not across it, so a count's text never touches the next one's edge.
    const count = rule(".column.col-strip .col-fold .count");
    expect(count).toContain("margin: 0;");
    expect(count).toContain("padding: 5px 0;");
    expect(count).toContain("min-width: 0;");
  });

  it("the buttons under a waiting task (Done, unblock; Open to answer) have a gap between them", () => {
    // On the inline end, so the gap stays between them in فارسی too (mu-149).
    expect(rule(".waiting > button")).toMatch(/margin-block: 0 4px; margin-inline: 0 8px;/);
  });
});

// The header (task 0113's QA): at a 960 px header (a 1280 px window with the widest sidebar) its
// items didn't fit one line and overflow: hidden cut the account meter, the display mode and
// "live" off. It wraps instead: the status area (meter, mode, live) is one unit that drops whole
// to a second line, at the end; New task stays at the start; nothing is ever cut.
describe("the header never cuts anything off (0113)", () => {
  const all = (sel: string) => [...css.matchAll(new RegExp(`(?:^|\\n)${sel.replace(/[.*+?^${}()|[\]\\>]/g, "\\$&")} \\{([^}]*)\\}`, "g"))].map((m) => m[1]).join(";");

  it("wraps rather than hides", () => {
    const top = all(".top");
    expect(top).toContain("flex-wrap: wrap;");
    expect(top).not.toMatch(/overflow(?:-x)?: hidden/);
    expect(top).not.toContain("flex-wrap: nowrap");
    // A wrapped line's items sit at its end, like the status area on one line.
    expect(top).toContain("justify-content: flex-end;");
    // New task keeps the start of the first line, wherever the rest wraps.
    expect(rule(".top .new")).toContain("margin-inline-end: auto;");
    // The views shrink and wrap only when even they don't fit a line alone.
    expect(rule(".top .views")).toMatch(/flex: 0 1 auto;[^]*flex-wrap: wrap;/);
  });

  it("keeps the status area whole, as one unit", () => {
    expect(rule(".top-status")).toMatch(/display: flex;[^]*flex: none;/);
    expect(rule(".meter")).toContain("flex: none;");
  });
});
