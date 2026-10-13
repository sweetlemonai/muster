import { describe, expect, it } from "vitest";
import type { QueueInfo } from "./types";
import { bandLimit, checkCount, COLUMN_LINE_MAX, columnLine, LIMIT_DEFAULT, limitText, overLimit, overLine, overTitle, sideLimit } from "./yourCheckLimit";

const q = (name: string, yourCheck: number, yourCheckLimit: number | null | undefined): QueueInfo => ({ name, path: `/p/${name}/tasks`, counts: {}, yourCheck, yourCheckLimit });

describe("the Your check limit (0050)", () => {
  it("is over only past the limit, never at it, and never with it off or from an older core", () => {
    const queues = [q("muster", 7, 5), q("drover", 5, 5), q("germ", 40, null), { name: "old", path: "/p/old/tasks", counts: {} }];
    expect(overLimit(queues, "all")).toEqual([{ name: "muster", waiting: 7, limit: 5 }]);
    expect(overLimit(queues, "muster")).toEqual([{ name: "muster", waiting: 7, limit: 5 }]);
    expect(overLimit(queues, "drover")).toEqual([]);
    expect(overLimit(queues, "germ")).toEqual([]);
    expect(overLimit(queues, "old")).toEqual([]);
    expect(overLimit([], "all")).toEqual([]);
    expect(LIMIT_DEFAULT).toBe(5);
  });

  it("one project over: the header reads N of L and the line names the limit", () => {
    const over = overLimit([q("muster", 7, 5)], "muster");
    // The column may show fewer under a filter: the header still counts the whole pile.
    expect(checkCount(3, over, "muster")).toEqual({ text: "7 of 5", over: true, title: "7 tasks wait in Your check, over your limit of 5 (Settings › Projects)" });
    // Above the board: short, leading with the numbers; the whole sentence is its title.
    expect(overLine(over, "muster")).toBe("7 of 5 in Your check: check these before more start.");
    expect(overTitle(over, "muster")).toBe("7 tasks wait in Your check, over your limit of 5: check these before more start.");
  });

  it("within the limit: the plain count and no line", () => {
    expect(checkCount(4, [], "muster")).toEqual({ text: "4", over: false, title: "" });
    expect(overLine([], "muster")).toBe("");
    expect(overLine([], "all")).toBe("");
    expect(overTitle([], "muster")).toBe("");
  });

  it("All projects: each checked on its own, the line names the ones over", () => {
    const queues = [q("muster", 7, 5), q("drover", 9, 8), q("germ", 2, 5)];
    const over = overLimit(queues, "all");
    expect(over.map((o) => o.name)).toEqual(["muster", "drover"]);
    expect(checkCount(18, over, "all")).toEqual({ text: "18", over: true, title: "Over the limit in muster (7 of 5) and drover (9 of 8)" });
    // The line counts the projects (each sidebar row names its own); the title names them.
    expect(overLine(over, "all")).toBe("Over your limit in 2 projects: check these before more start.");
    expect(overTitle(over, "all")).toBe("Over your limit in muster (7 of 5) and drover (9 of 8): check these before more start.");
    expect(overLine(overLimit([q("muster", 7, 5), q("germ", 1, 5)], "all"), "all")).toBe("Over your limit in 1 project: check these before more start.");
    expect(overTitle(overLimit([q("a", 2, 1), q("b", 3, 1), q("c", 4, 1)], "all"), "all")).toBe("Over your limit in a (2 of 1), b (3 of 1) and c (4 of 1): check these before more start.");
    // Never longer than the one project's at its largest, whatever the names.
    const long = Array.from({ length: 120 }, (_, i) => q(`a-very-long-project-name-${i}`, 60, 50));
    expect(overLine(overLimit(long, "all"), "all")).toBe("Over your limit in 120 projects: check these before more start.");
  });

  it("a Group by band's count: its own, orange only when it holds tasks of a project over its limit", () => {
    const over = overLimit([q("muster", 7, 5), q("drover", 2, 5)], "all");
    expect(bandLimit(["muster", "muster"], over, "all")).toEqual({ over: true, title: "muster: 7 tasks wait in Your check, over your limit of 5" });
    expect(bandLimit(["drover"], over, "all")).toEqual({ over: false, title: "" });
    expect(bandLimit([], over, "all")).toEqual({ over: false, title: "" });
    expect(bandLimit(["muster"], overLimit([q("muster", 7, 5)], "muster"), "muster")).toEqual({ over: true, title: "7 tasks wait in Your check, over your limit of 5" });
    expect(bandLimit(["muster"], [], "muster")).toEqual({ over: false, title: "" });
  });

  it("the column's own line is short enough for its reserved two lines; the full sentence goes above the board", () => {
    expect(columnLine(overLimit([q("muster", 7, 5)], "muster"), "muster")).toBe("Over your limit of 5");
    expect(columnLine(overLimit([q("muster", 7, 5)], "all"), "all")).toBe("1 project over limit");
    expect(columnLine(overLimit([q("a", 2, 1), q("b", 3, 1)], "all"), "all")).toBe("2 projects over limit");
    expect(columnLine([], "muster")).toBe("");
    // The longest it can be: the largest limit, or many projects with long names (names never go in it).
    const long = Array.from({ length: 120 }, (_, i) => q(`a-very-long-project-name-${i}`, 60, 50));
    for (const [over, queue] of [[overLimit([q("x", 60, 50)], "x"), "x"], [overLimit(long, "all"), "all"]] as const) {
      expect(columnLine(over, queue).length).toBeLessThanOrEqual(COLUMN_LINE_MAX);
    }
    expect(COLUMN_LINE_MAX).toBeLessThanOrEqual(24);
  });

  it("the sidebar says it on an over row only", () => {
    expect(sideLimit(q("muster", 7, 5))).toBe("7 in Your check, limit 5");
    expect(sideLimit(q("muster", 5, 5))).toBe("");
    expect(sideLimit(q("muster", 70, null))).toBe("");
    expect(sideLimit(undefined)).toBe("");
  });

  it("Settings › Projects says each limit with its count", () => {
    expect(limitText(q("muster", 7, 5))).toBe("limit 5 · 7 waiting");
    expect(limitText(q("muster", 1, 12))).toBe("limit 12 · 1 waiting");
    expect(limitText(q("muster", 3, null))).toBe("off · 3 waiting");
    expect(limitText(q("muster", 0, 5))).toBe("limit 5 · none waiting");
  });
});
