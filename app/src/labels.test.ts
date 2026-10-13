import { describe, expect, it } from "vitest";
import { byCategory, categoriesOf, keyTitle, labelOf, labelsOf, OTHER } from "./labels";

describe("project labels (0044)", () => {
  it("call a project by its label, else by its name", () => {
    const labels = labelsOf([
      { name: "muster", label: "Muster" },
      { name: "drover", label: "drover" },
      { name: "old" },
    ]);
    expect([...labels]).toEqual([["muster", "Muster"]]);
    expect(labelOf(labels, "muster")).toBe("Muster");
    expect(labelOf(labels, "drover")).toBe("drover");
    expect(labelOf(labels, "old")).toBe("old");
    expect(labelOf(new Map(), "germ")).toBe("germ");
  });
  it("name a task key's project in its tooltip", () => {
    expect(keyTitle("Drover — agents", "dr-7")).toBe("Drover — agents · dr-7");
  });
});

describe("categories (0044)", () => {
  const row = (name: string, category: string) => ({ name, category });
  it("group rows under headings A–Z with Other last, each keeping its rows' order", () => {
    const groups = byCategory([row("z", "tools"), row("a", ""), row("m", "Apps"), row("b", "Tools"), row("c", "tools"), row("o", OTHER), row("e", "Éclair")]);
    expect(groups.map((g) => [g.name, g.rows.map((r) => r.name)])).toEqual([
      ["Apps", ["m"]],
      ["Tools", ["b"]],
      ["tools", ["z", "c"]],
      ["Éclair", ["e"]],
      ["Other", ["a", "o"]],
    ]);
  });
  it("an empty or blank category is Other, and no rows make no headings", () => {
    expect(byCategory([row("a", " "), row("b", "")]).map((g) => g.name)).toEqual(["Other"]);
    expect(byCategory([])).toEqual([]);
  });
  it("offer every shown project's category once, Other last, a queue without one in Other", () => {
    expect(categoriesOf([{ category: "Tools" }, { category: "Other" }, { category: "Apps" }, { category: "Tools" }, {}])).toEqual(["Apps", "Tools", "Other"]);
  });
});
