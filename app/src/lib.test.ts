import { describe, expect, it } from "vitest";
import {
  ago, claimant, columns, earlierStages, estimateTotal, feed, fileName, fmtMinutes, fmtTokens, heldByOther, holder, inProgress, nextStage, oneAtATime, passLabel,
  queueTotal, stageOf, STAGES, stamp,
} from "./lib";
import type { Task } from "./types";

const task = (over: Partial<Task>): Task => ({
  queue: "drover", id: "0001", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "todo/0001-t.md", claimedBy: "", heartbeat: "", waitingOn: "", commit: "", captured: "", questions: [],
  openQuestions: 0, waitingForOwner: false, tags: [], estTime: "", estTokens: "", estMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", attachments: [], logTail: [], modified: 0, ...over,
});

describe("stages", () => {
  it("are the core's seven, in its order (queue.rs STAGES)", () => {
    expect(STAGES.map((s) => s.id)).toEqual(["backlog", "shaping", "ready", "building", "testing", "your-check", "done"]);
    expect(STAGES.map((s) => s.label)).toEqual(["Backlog", "Shaping", "Ready", "Building", "Testing", "Your check", "Done"]);
    expect(STAGES.find((s) => s.id === "your-check")?.role).toBe("owner");
    expect(STAGES.find((s) => s.id === "building")?.role).toBe("code agent");
  });
  it("reads an unknown stage as Backlog", () => {
    expect(stageOf(task({ stage: "testing" }))).toBe("testing");
    expect(stageOf(task({ stage: "doing" }))).toBe("backlog");
    expect(stageOf(task({ stage: "" }))).toBe("backlog");
  });
  it("names the next stage, and Your check's pass is Accept", () => {
    expect(nextStage("ready")?.id).toBe("building");
    expect(nextStage("your-check")?.id).toBe("done");
    expect(nextStage("done")).toBeNull();
    expect(nextStage("weird")).toBeNull();
    expect(passLabel("backlog")).toBe("Pass to Shaping");
    expect(passLabel("building")).toBe("Pass to Testing");
    expect(passLabel("your-check")).toBe("Accept");
    expect(passLabel("done")).toBeNull();
    expect(passLabel("weird")).toBeNull();
  });
  it("lists the stages a task can be sent back to, nearest first", () => {
    expect(earlierStages("testing").map((s) => s.id)).toEqual(["building", "ready", "shaping", "backlog"]);
    expect(earlierStages("backlog")).toEqual([]);
    expect(earlierStages("weird")).toEqual([]);
  });
  it("in progress is Building and Testing", () => {
    expect(STAGES.filter((s) => inProgress(task({ stage: s.id }))).map((s) => s.id)).toEqual(["building", "testing"]);
  });
  it("says who else holds Ready or Building (the owner can't pass those)", () => {
    expect(heldByOther(task({ stage: "building", claimedBy: "drover/main (session 3fba2222)" }))).toBe("drover/main");
    expect(heldByOther(task({ stage: "ready", claimedBy: "wt-b" }))).toBe("wt-b");
    expect(heldByOther(task({ stage: "building", claimedBy: "owner" }))).toBe("");
    expect(heldByOther(task({ stage: "building", claimedBy: "" }))).toBe("");
    expect(heldByOther(task({ stage: "testing", claimedBy: "qa" }))).toBe("");
  });
  it("shows who holds a task only where a stage can be held (Ready, Building)", () => {
    expect(holder(task({ stage: "building", claimedBy: "drover/main (session 3fba2222)" }))).toBe("drover/main");
    expect(holder(task({ stage: "ready", claimedBy: "owner" }))).toBe("owner");
    expect(holder(task({ stage: "done", claimedBy: "wt-a" }))).toBe("");
    expect(holder(task({ stage: "testing", claimedBy: "wt-a" }))).toBe("");
    expect(holder(task({ stage: "building", claimedBy: " " }))).toBe("");
  });
  it("runs one stage action at a time: a second click while one is in flight is dropped", async () => {
    const once = oneAtATime();
    let release = () => {};
    let calls = 0;
    const slow = () => new Promise<boolean>((r) => { calls++; release = () => r(true); });
    const first = once(slow);
    expect(await once(slow)).toBe(false);
    release();
    expect(await first).toBe(true);
    expect(calls).toBe(1);
    // Free again after it settles, also after a failure.
    await expect(once(() => Promise.reject(new Error("refused")))).rejects.toThrow("refused");
    expect(await once(async () => true)).toBe(true);
  });
  it("totals a queue's counts without counting blocked tasks twice", () => {
    expect(queueTotal({ ready: 2, building: 1, testing: 1, blocked: 2 })).toBe(4);
    expect(queueTotal({})).toBe(0);
  });
});

describe("columns", () => {
  it("has one column per stage, in order", () => {
    expect(Object.keys(columns([]))).toEqual(STAGES.map((s) => s.id));
  });
  it("puts each task in its stage's column, blocked ones too, unknown ones in Backlog", () => {
    const c = columns([
      task({ id: "0001" }),
      task({ id: "0002", stage: "building", status: "doing" }),
      task({ id: "0003", stage: "weird" }),
      task({ id: "0004", stage: "testing", status: "blocked", blocked: true, waitingOn: "0001" }),
      task({ id: "0005", stage: "your-check", status: "blocked", blocked: true, waitingOn: "owner", waitingForOwner: true }),
    ]);
    expect(c.ready.map((t) => t.id)).toEqual(["0001"]);
    expect(c.building.map((t) => t.id)).toEqual(["0002"]);
    expect(c.backlog.map((t) => t.id)).toEqual(["0003"]);
    expect(c.testing.map((t) => t.id)).toEqual(["0004"]);
    expect(c["your-check"].map((t) => t.id)).toEqual(["0005"]);
    expect(c.blocked).toBeUndefined();
  });
  it("sorts waiting-for-owner first, then priority, then id", () => {
    const c = columns([
      task({ id: "0003" }),
      task({ id: "0002", priority: "high" }),
      task({ id: "0004", waitingForOwner: true, priority: "low" }),
      task({ id: "0001", priority: "low" }),
    ]);
    expect(c.ready.map((t) => t.id)).toEqual(["0004", "0002", "0003", "0001"]);
  });
  it("sorts done by most recently changed", () => {
    const c = columns([task({ id: "0001", stage: "done", modified: 1 }), task({ id: "0002", stage: "done", status: "blocked", blocked: true, modified: 9 })]);
    expect(c.done.map((t) => t.id)).toEqual(["0002", "0001"]);
  });
});

describe("helpers", () => {
  it("ago", () => {
    expect(ago(100, 90)).toBe("now");
    expect(ago(1000, 400)).toBe("10 min");
    expect(ago(10000, 0)).toBe("2 h");
    expect(ago(200000, 0)).toBe("2 d");
    expect(ago(0, 50)).toBe("now");
  });
  it("stamp matches the file contract", () => {
    expect(stamp(new Date(2026, 9, 12, 9, 5))).toBe("2026-10-12 09:05");
  });
  it("claimant drops the session id", () => {
    expect(claimant("drover/main (session 3fba2222)")).toBe("drover/main");
    expect(claimant("worktree-i18n (.claude/worktrees/i18n), session 41f7fe18")).toBe("worktree-i18n");
    expect(claimant("capture (drover/main, session 349bc9c3)")).toBe("capture");
    expect(claimant("themes session (outside this queue)")).toBe("themes session");
  });
  it("feed merges commits and file changes, newest first", () => {
    const f = feed([{ queue: "q", at: 5, what: "c", source: "commit" }], [task({ modified: 9, logTail: ["x done"] })]);
    expect(f.map((a) => a.source)).toEqual(["file", "commit"]);
    expect(f[0].what).toContain("x done");
    expect(f[0].what).toContain("· Ready ·");
    const b = feed([], [task({ stage: "testing", blocked: true, waitingOn: "0012" })]);
    expect(b[0].what).toBe("0001 T · Testing, blocked on 0012");
  });
});

describe("fileName", () => {
  it("keeps real names and names pasted images", () => {
    expect(fileName({ name: "shot.png", type: "image/png" }, 0, "2026-10-12 14:05")).toBe("shot.png");
    expect(fileName({ name: "image.png", type: "image/png" }, 0, "2026-10-12 14:05")).toBe("pasted-2026-10-12-14-05.png");
    expect(fileName({ name: "", type: "image/jpeg" }, 1, "2026-10-12 14:05")).toBe("pasted-2026-10-12-14-05-2.jpg");
  });
});

describe("estimates", () => {
  it("formats time and tokens", () => {
    expect([fmtMinutes(45), fmtMinutes(90), fmtMinutes(480), fmtMinutes(600), fmtMinutes(0)]).toEqual(["45 min", "1.5 h", "1 d", "1 d 2 h", ""]);
    expect([fmtTokens(450_000), fmtTokens(3_210_000), fmtTokens(900), fmtTokens(0)]).toEqual(["450k tokens", "3.2M tokens", "900 tokens", ""]);
  });
  it("totals open, estimated tasks and says how many are estimated", () => {
    const t = (m: number, k: number, stage = "ready") => ({ estMinutes: m, estTokenCount: k, stage });
    expect(estimateTotal([t(60, 100_000), t(30, 50_000), t(0, 0), t(600, 0, "done")])).toBe("≈ 1.5 h · 150k tokens left (2 of 3 open tasks estimated)");
    expect(estimateTotal([t(0, 0)])).toBe("");
  });
});
