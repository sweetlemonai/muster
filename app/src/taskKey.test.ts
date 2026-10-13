import { describe, expect, it } from "vitest";
import cases from "./key-cases.json";
import { keyIndex, keyOf, keysIn, parseKey, siblingKey, taskHref, taskKey, taskOfHref } from "./taskKey";

// The window's half of the shared cases: key.rs's tests read the same file.

describe("task keys (the core gives the same)", () => {
  it("formats the shared cases", () => {
    for (const [prefix, id, key] of cases.format) expect(keyOf(prefix, id)).toBe(key);
  });
  it("parses the shared cases", () => {
    for (const [typed, prefix, id] of cases.parse) expect(parseKey(typed as string)).toEqual({ prefix, id });
    for (const bad of cases.bad) expect(parseKey(bad)).toBeNull();
  });
  it("names a task, and the tasks of its project, by their keys", () => {
    expect(taskKey({ prefix: "mu", id: "0012" })).toBe("mu-12");
    expect(taskKey({ id: "0012" })).toBe("0012");
    expect(siblingKey({ prefix: "br" }, "0007")).toBe("br-7");
  });
});

describe("keys in text", () => {
  const known = keyIndex([
    { name: "muster", prefix: "mu" },
    { name: "braindump", prefix: "br" },
    { name: "old", prefix: undefined },
    { name: "muster-2", prefix: "mu" },
  ]);
  const found = (text: string) => keysIn(text, known).map((k) => [text.slice(k.start, k.end), k.queue, k.id]);

  it("maps each prefix to its project, the first one when two have it", () => {
    expect([...known]).toEqual([["mu", "muster"], ["br", "braindump"]]);
  });
  it("finds keys of known projects, written either case", () => {
    expect(found("See mu-12 and BR-7, then mu-0003.")).toEqual([
      ["mu-12", "muster", "0012"],
      ["BR-7", "braindump", "0007"],
      ["mu-0003", "muster", "0003"],
    ]);
    expect(found("(mu-1), \"br-2\": mu-3!")).toEqual([
      ["mu-1", "muster", "0001"],
      ["br-2", "braindump", "0002"],
      ["mu-3", "muster", "0003"],
    ]);
  });
  it("leaves alone what only looks like one", () => {
    for (const text of [
      "utf-8 and iso-8859", // not a project's prefix
      "xmu-12", // inside a word
      "mu-12345", // a longer number
      "task/mu-12-board", // a branch
      "mu-12-board",
      "docs/mu-12.md", // a path
      "mu-12.md",
      "2026-10-12",
      "mu_12 mu12 mu- 12",
    ]) expect(found(text)).toEqual([]);
    expect(keysIn("mu-12", new Map())).toEqual([]);
  });
  it("links to a task and reads the link back", () => {
    const href = taskHref("my queue/x", "0012");
    expect(href).toBe("#task/my%20queue%2Fx/0012");
    expect(taskOfHref(href)).toEqual({ queue: "my queue/x", id: "0012" });
    for (const other of ["https://example.com", "#task/muster/12", "#task//0012", "#task/%E0%A4%A/0012", "#tasks/muster/0012"]) {
      expect(taskOfHref(other)).toBeNull();
    }
  });
});
