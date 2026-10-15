import { describe, expect, it } from "vitest";
import { setLocale } from "./i18n/runtime";
import { byCategory, categoriesOf, keyTitle, labelOf, labelsOf, OTHER, otherCategories } from "./labels";

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
  it("sorts Persian headings in Persian order in فارسی, not by code point (mu-151)", async () => {
    await setLocale("fa");
    try {
      // By code point ل (U+0644) would come before ک (U+06A9) and گ (U+06AF).
      const groups = byCategory([row("a", "لوح"), row("b", "گل"), row("c", "کتاب")]);
      expect(groups.map((g) => g.name)).toEqual(["کتاب", "گل", "لوح"]);
    } finally {
      await setLocale("en");
    }
  });
  it("group rows under headings A–Z with Other last, each keeping its rows' order", () => {
    const groups = byCategory([row("z", "tools"), row("a", ""), row("m", "Apps"), row("b", "Tools"), row("c", "tools"), row("o", OTHER), row("e", "Éclair")]);
    // In the language's order (Intl.Collator, mu-151): É with the E's, a lowercase word before
    // its capitalized twin.
    expect(groups.map((g) => [g.name, g.rows.map((r) => r.name)])).toEqual([
      ["Apps", ["m"]],
      ["Éclair", ["e"]],
      ["tools", ["z", "c"]],
      ["Tools", ["b"]],
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
  it("offer a row only the other projects' categories, so what it's typing never becomes a chip (review)", () => {
    const queues = [{ path: "/a", category: "Ap" }, { path: "/b", category: "Tools" }, { path: "/c", category: "Apps" }];
    expect(otherCategories(queues, "/a")).toEqual(["Apps", "Tools"]);
    expect(otherCategories(queues, "/z")).toEqual(["Ap", "Apps", "Tools"]);
  });
});
