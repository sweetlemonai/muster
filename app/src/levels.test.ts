import { describe, expect, it } from "vitest";
import { estimateLeft, groupHead, groupItems, indexTasks, isWork, levelLabel, parentLine, parseGroupBy, progressText, shownStageClass, shownStageLabel } from "./levels";
import type { Task } from "./types";

const task = (over: Partial<Task>): Task => ({
  queue: "muster", id: "0001", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], commit: "", captured: "", capturedAt: "", blockHistory: [], questions: [],
  openQuestions: 0, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTokens: "", estMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", attachments: [], logTail: [], modified: 0,
  level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});
const progress = (done: number, total: number, estMinutes = 0, estTokenCount = 0) => ({ done, total, estMinutes, estTokenCount });

// Goal 0001 → feature 0002 → tasks 0004, 0005; feature 0003 (no tasks yet) under 0001; task 0006
// straight under the goal; task 0007 under nothing; feature 0008 under no goal, with task 0009.
const goal = task({ id: "0001", title: "Ship 1.0", level: "goal", progress: progress(1, 3), children: ["0002", "0003", "0006"] });
const stageTrack = task({ id: "0002", title: "Stage track", level: "feature", parent: "0001", progress: progress(1, 2, 120, 200_000), shownStage: "in-progress" });
const empty = task({ id: "0003", title: "Empty", level: "feature", parent: "0001", progress: progress(0, 0) });
const t4 = task({ id: "0004", parent: "0002", stage: "done" });
const t5 = task({ id: "0005", parent: "0002", stage: "building" });
const t6 = task({ id: "0006", parent: "0001" });
const t7 = task({ id: "0007" });
const loose = task({ id: "0008", title: "Loose", level: "feature", progress: progress(0, 1) });
const t9 = task({ id: "0009", parent: "0008" });
const all = [goal, stageTrack, empty, t4, t5, t6, t7, loose, t9];
const index = indexTasks(all);
const ids = (ts: Task[]) => ts.map((t) => t.id);

describe("levels", () => {
  it("only tasks are work for the stage columns", () => {
    expect(ids(all.filter(isWork))).toEqual(["0004", "0005", "0006", "0007", "0009"]);
    expect([levelLabel("goal"), levelLabel("feature"), levelLabel("task"), levelLabel("")]).toEqual(["Goal", "Feature", "Task", "Task"]);
  });
  it("names a card's parent in a quiet line, found in its own queue", () => {
    expect(parentLine(t5, index)).toBe("▸ Feature 0002 Stage track");
    expect(parentLine(t6, index)).toBe("▸ Goal 0001 Ship 1.0");
    expect(parentLine(t7, index)).toBe("");
    expect(parentLine(task({ parent: "0099" }), index)).toBe("▸ 0099 (not found)");
    // The same id in another project isn't its parent.
    expect(parentLine(task({ queue: "drover", parent: "0002" }), index)).toBe("▸ 0002 (not found)");
  });
  it("finds the band a task belongs in", () => {
    expect(groupHead(t5, index, "feature")?.id).toBe("0002");
    expect(groupHead(t6, index, "feature")).toBeNull();
    expect(groupHead(t5, index, "goal")?.id).toBe("0001");
    expect(groupHead(t6, index, "goal")?.id).toBe("0001");
    expect(groupHead(t9, index, "goal")).toBeNull();
    expect(groupHead(stageTrack, index, "goal")?.id).toBe("0001");
  });
  it("groups by feature: a band per feature, then the tasks with none", () => {
    const g = groupItems(all.filter(isWork), all, "feature");
    expect(g.map((b) => [b.head?.id ?? "none", ids(b.items)])).toEqual([
      ["0002", ["0004", "0005"]],
      ["0003", []],
      ["0008", ["0009"]],
      ["none", ["0006", "0007"]],
    ]);
  });
  it("groups by goal: tasks under its features and its own", () => {
    const g = groupItems(all.filter(isWork), all, "goal");
    expect(g.map((b) => [b.head?.id ?? "none", ids(b.items)])).toEqual([
      ["0001", ["0004", "0005", "0006"]],
      ["none", ["0007", "0009"]],
    ]);
  });
  it("a filter that leaves a band nothing hides it, but a feature with no tasks yet stays in view", () => {
    const g = groupItems([t7], all, "feature");
    expect(g.map((b) => b.head?.id ?? "none")).toEqual(["0003", "none"]);
    expect(groupItems([], all, "goal")).toEqual([]);
  });
  it("a band whose feature matches the filter stays, even when none of its tasks do", () => {
    const bands = (g: ReturnType<typeof groupItems>) => g.map((b) => [b.head?.id ?? "none", ids(b.items)]);
    const want = [["0002", []], ["0003", []], ["none", ["0007"]]];
    // The board: its items are the tasks shown; the feature 0002 is shown too, its tasks aren't.
    expect(bands(groupItems([t7], all, "feature", [stageTrack, t7]))).toEqual(want);
    // The table: the feature's row is shown, as its band's heading.
    expect(bands(groupItems([stageTrack, t7], all, "feature"))).toEqual(want);
  });
  it("in the table, the other levels' rows go in their band too", () => {
    const g = groupItems(all.filter((t) => t.level !== "goal"), all, "goal");
    expect(g.map((b) => [b.head?.id ?? "none", ids(b.items)])).toEqual([
      ["0001", ["0002", "0003", "0004", "0005", "0006"]],
      ["none", ["0007", "0008", "0009"]],
    ]);
  });
  it("shows the roll-up: progress, estimate left and the shown stage", () => {
    expect(progressText(stageTrack.progress)).toBe("1 of 2 done");
    expect(progressText(empty.progress)).toBe("no tasks yet");
    expect(progressText(null)).toBe("");
    expect(estimateLeft(stageTrack.progress)).toBe("≈ 2 h · 200k tokens left");
    expect(estimateLeft(progress(0, 1, 45))).toBe("≈ 45 min left");
    expect(estimateLeft(progress(1, 1))).toBe("");
    expect(estimateLeft(null)).toBe("");
    expect([shownStageLabel(stageTrack), shownStageClass(stageTrack)]).toEqual(["In progress", "building"]);
    expect([shownStageLabel(goal), shownStageClass(goal)]).toEqual(["Ready", "ready"]);
    expect([shownStageLabel(task({ shownStage: "done" })), shownStageClass(task({ shownStage: "your-check" }))]).toEqual(["Done", "your-check"]);
  });
  it("remembers Group by, and reads anything else as None", () => {
    expect([parseGroupBy("feature"), parseGroupBy("goal"), parseGroupBy("none"), parseGroupBy(null), parseGroupBy("epic")]).toEqual(["feature", "goal", "none", "none", "none"]);
  });
});
