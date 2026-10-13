import { describe, expect, it } from "vitest";
import type { QueueInfo } from "./types";
import { checkCount, LIMIT_DEFAULT, limitText, overLimit, overLine, sideLimit } from "./yourCheckLimit";

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
    expect(overLine(over, "muster")).toBe("Over your limit of 5: check these before more start.");
  });

  it("within the limit: the plain count and no line", () => {
    expect(checkCount(4, [], "muster")).toEqual({ text: "4", over: false, title: "" });
    expect(overLine([], "muster")).toBe("");
    expect(overLine([], "all")).toBe("");
  });

  it("All projects: each checked on its own, the line names the ones over", () => {
    const queues = [q("muster", 7, 5), q("drover", 9, 8), q("germ", 2, 5)];
    const over = overLimit(queues, "all");
    expect(over.map((o) => o.name)).toEqual(["muster", "drover"]);
    expect(checkCount(18, over, "all")).toEqual({ text: "18", over: true, title: "Over the limit in muster (7 of 5) and drover (9 of 8)" });
    expect(overLine(over, "all")).toBe("Over your limit in muster (7 of 5) and drover (9 of 8): check these before more start.");
    expect(overLine(overLimit([q("muster", 7, 5), q("germ", 1, 5)], "all"), "all")).toBe("Over your limit in muster (7 of 5): check these before more start.");
    expect(overLine(overLimit([q("a", 2, 1), q("b", 3, 1), q("c", 4, 1)], "all"), "all")).toBe("Over your limit in a (2 of 1), b (3 of 1) and c (4 of 1): check these before more start.");
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
