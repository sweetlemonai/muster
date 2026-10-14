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
    const r = rule(".column.folded .col-fold");
    expect(r).toContain("display: flex;");
    expect(r).toContain("gap: 6px;");
    // Counts pad along the strip (top and bottom), not across it, so a count's text never touches the next one's edge.
    const count = rule(".column.folded .col-fold .count");
    expect(count).toContain("margin: 0;");
    expect(count).toContain("padding: 5px 0;");
    expect(count).toContain("min-width: 0;");
  });

  it("the buttons under a waiting task (Done, unblock; Open to answer) have a gap between them", () => {
    expect(rule(".waiting > button")).toMatch(/margin: 0 8px 4px 0;/);
  });
});
