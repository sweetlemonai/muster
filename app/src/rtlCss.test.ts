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
