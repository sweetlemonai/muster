import { describe, expect, it } from "vitest";
import { ago, claimant, columns, feed, fileName, stamp } from "./lib";
import type { Task } from "./types";

const task = (over: Partial<Task>): Task => ({
  queue: "drover", id: "0001", title: "T", kind: "task", priority: "normal", status: "todo", folder: "todo",
  path: "todo/0001-t.md", claimedBy: "", heartbeat: "", waitingOn: "", commit: "", captured: "", questions: [],
  openQuestions: 0, waitingForOwner: false, attachments: [], logTail: [], modified: 0, ...over,
});

describe("columns", () => {
  it("puts each task in its status column, unknown ones in Inbox", () => {
    const c = columns([task({ id: "0001" }), task({ id: "0002", status: "doing" }), task({ id: "0003", status: "weird" })]);
    expect(c.todo.map((t) => t.id)).toEqual(["0001"]);
    expect(c.doing.map((t) => t.id)).toEqual(["0002"]);
    expect(c.inbox.map((t) => t.id)).toEqual(["0003"]);
  });
  it("sorts waiting-for-owner first, then priority, then id", () => {
    const c = columns([
      task({ id: "0003" }),
      task({ id: "0002", priority: "high" }),
      task({ id: "0004", waitingForOwner: true, priority: "low" }),
      task({ id: "0001", priority: "low" }),
    ]);
    expect(c.todo.map((t) => t.id)).toEqual(["0004", "0002", "0003", "0001"]);
  });
  it("sorts done by most recently changed", () => {
    const c = columns([task({ id: "0001", status: "done", modified: 1 }), task({ id: "0002", status: "done", modified: 9 })]);
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
  });
});

describe("fileName", () => {
  it("keeps real names and names pasted images", () => {
    expect(fileName({ name: "shot.png", type: "image/png" }, 0, "2026-10-12 14:05")).toBe("shot.png");
    expect(fileName({ name: "image.png", type: "image/png" }, 0, "2026-10-12 14:05")).toBe("pasted-2026-10-12-14-05.png");
    expect(fileName({ name: "", type: "image/jpeg" }, 1, "2026-10-12 14:05")).toBe("pasted-2026-10-12-14-05-2.jpg");
  });
});
