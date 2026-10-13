import { describe, expect, it } from "vitest";
import { bars, dayNumber, sortTasks } from "./viewData";
import type { Task } from "./types";

const t = (over: Partial<Task>): Task => ({
  queue: "q", id: "0001", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [], folder: "todo", path: "", claimedBy: "",
  heartbeat: "", waitingOn: "", commit: "", captured: "2026-10-11", questions: [], openQuestions: 0, ownerCriteria: [], waitingForOwner: false,
  tags: [], estTime: "", estTokens: "", estMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", attachments: [], logTail: [], modified: 0, ...over,
});

describe("sortTasks", () => {
  it("sorts by priority rank, stage order and change time", () => {
    const list = [t({ id: "1", priority: "low" }), t({ id: "2", priority: "high" }), t({ id: "3" })];
    expect(sortTasks(list, "priority", false).map((x) => x.id)).toEqual(["2", "3", "1"]);
    expect(sortTasks(list, "priority", true).map((x) => x.id)).toEqual(["1", "3", "2"]);
    const st = [t({ id: "a", stage: "done" }), t({ id: "b", stage: "backlog" }), t({ id: "c", stage: "your-check" }), t({ id: "d", stage: "shaping" }), t({ id: "e", stage: "testing", blocked: true })];
    expect(sortTasks(st, "stage", false).map((x) => x.id)).toEqual(["b", "d", "e", "c", "a"]);
    expect(sortTasks([t({ id: "x", modified: 5 }), t({ id: "y", modified: 9 })], "changed", true).map((x) => x.id)).toEqual(["y", "x"]);
    // "On it / waiting on" sorts by what the cell shows: a Done task's old claim counts as nobody.
    const who = [t({ id: "a", stage: "done", claimedBy: "aaa" }), t({ id: "b", stage: "building", claimedBy: "zed" }), t({ id: "c", blocked: true, waitingOn: "0012" })];
    expect(sortTasks(who, "who", false).map((x) => x.id)).toEqual(["a", "c", "b"]);
  });
});

describe("timeline bars", () => {
  const today = dayNumber("2026-10-14")!;
  it("runs from capture to done, or to today while open", () => {
    const r = bars([t({ id: "a", captured: "2026-10-11", claimedOn: "2026-10-12", stage: "done", doneOn: "2026-10-13" }), t({ id: "b", captured: "2026-10-12", stage: "your-check", status: "doing" })], today);
    expect(r.from).toBe(dayNumber("2026-10-11"));
    expect(r.to).toBe(today);
    const [a, b] = r.bars;
    expect([a.start, a.work, a.end, a.open]).toEqual([dayNumber("2026-10-11"), dayNumber("2026-10-12"), dayNumber("2026-10-13"), false]);
    expect([b.end, b.open, b.work]).toEqual([today, true, null]);
  });
  it("ignores a claim day outside the bar and a missing capture date", () => {
    const r = bars([t({ captured: "", modified: 86400 * dayNumber("2026-10-13")!, claimedOn: "2020-01-01" })], today);
    expect(r.bars[0].start).toBe(dayNumber("2026-10-13"));
    expect(r.bars[0].work).toBeNull();
  });
  it("is empty-safe", () => {
    expect(bars([], today)).toEqual({ bars: [], from: today, to: today });
  });
});
