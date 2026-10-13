import { describe, expect, it } from "vitest";
import { criteriaSection, fmtWall, isFileDrag, isLive, LIVE_SECS, releasedBy, timeInWork, trackMove, trackPlace, undoFor, waitingOnYou } from "./paneData";
import { stampMinutes } from "./lib";
import type { Question, StageStep, Task } from "./types";

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0021", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "todo/0021-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], commit: "", captured: "", capturedAt: "",
  blockHistory: [], questions: [], openQuestions: 0, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});
const step = (at: string, from: string, to: string, who = "wt-a"): StageStep => ({ at, who, from, to });
const q = (over: Partial<Question>): Question => ({ n: 1, ticked: false, asker: "wt-a", text: "?", escalated: false, ownerAnswered: false, ...over });
const crit = (n: number, ticked: boolean) => ({ n, label: `AC${n}`, text: `AC${n}: x — verify: owner`, ticked, owner: true });

describe("live (until 0025: the task file changed in the last 5 minutes)", () => {
  it("is live up to 5 minutes after the file changed, not after", () => {
    expect(LIVE_SECS).toBe(300);
    expect(isLive(task({ modified: 1000 }), 1000)).toBe(true);
    expect(isLive(task({ modified: 1000 }), 1000 + 299)).toBe(true);
    expect(isLive(task({ modified: 1000 }), 1000 + 300)).toBe(false);
  });
  it("a file from the future (clock skew) counts as just changed", () => {
    expect(isLive(task({ modified: 2000 }), 1000)).toBe(true);
  });
});

describe("what's waiting on the owner", () => {
  it("nothing, for a task that needs nobody", () => {
    expect(waitingOnYou(task({}))).toEqual([]);
  });
  it("Your check: the owner's criteria left, or the call itself", () => {
    expect(waitingOnYou(task({ stage: "your-check", ownerCriteria: [crit(1, true), crit(2, false), crit(3, false)] }))).toEqual(["2 of 3 criteria to check"]);
    expect(waitingOnYou(task({ stage: "your-check", ownerCriteria: [crit(1, true)] }))).toEqual(["accept or send back"]);
    expect(waitingOnYou(task({ stage: "your-check" }))).toEqual(["accept or send back"]);
  });
  it("escalated open questions, counted, and a block on the owner", () => {
    const qs = [q({ n: 1, escalated: true }), q({ n: 2, escalated: true }), q({ n: 3 }), q({ n: 4, escalated: true, ticked: true })];
    expect(waitingOnYou(task({ questions: qs }))).toEqual(["2 questions for you"]);
    expect(waitingOnYou(task({ questions: [q({ escalated: true })] }))).toEqual(["1 question for you"]);
    expect(waitingOnYou(task({ blocked: true, waitingOn: "Owner: pick a colour" }))).toEqual(["blocked on you"]);
    expect(waitingOnYou(task({ blocked: true, waitingOn: "0012" }))).toEqual([]);
  });
  it("all at once, in that order", () => {
    const t = task({ stage: "your-check", blocked: true, waitingOn: "owner", questions: [q({ escalated: true })] });
    expect(waitingOnYou(t)).toEqual(["accept or send back", "1 question for you", "blocked on you"]);
  });
});

describe("time in Building and Testing, from the stage history", () => {
  const now = stampMinutes("2026-10-12 18:00")!;
  it("sums each stay, the open one up to now", () => {
    const h = [step("2026-10-12 15:00", "shaping", "ready"), step("2026-10-12 16:00", "ready", "building"), step("2026-10-12 17:00", "building", "testing")];
    expect(timeInWork(h, "testing", now)).toBe(120);
  });
  it("stops counting when it leaves, and counts a return", () => {
    const h = [
      step("2026-10-12 10:00", "ready", "building"),
      step("2026-10-12 10:30", "building", "testing"),
      step("2026-10-12 10:45", "testing", "your-check"),
      step("2026-10-12 11:00", "your-check", "building"),
      step("2026-10-12 11:10", "building", "testing"),
      step("2026-10-12 11:20", "testing", "your-check"),
    ];
    expect(timeInWork(h, "your-check", now)).toBe(45 + 20);
  });
  it("crosses midnight", () => {
    expect(timeInWork([step("2026-10-11 23:30", "ready", "building")], "building", stampMinutes("2026-10-12 00:15")!)).toBe(45);
  });
  it("is null when it was never in Building or Testing", () => {
    expect(timeInWork([], "building", now)).toBeNull();
    expect(timeInWork([step("2026-10-12 10:00", "backlog", "shaping")], "shaping", now)).toBeNull();
  });
  it("leaves out a stay it can't time: an open one the file's stage disagrees with, or an unreadable now", () => {
    expect(timeInWork([step("2026-10-12 17:00", "ready", "building")], "ready", now)).toBe(0);
    expect(timeInWork([step("2026-10-12 17:00", "ready", "building")], "building", null)).toBe(0);
  });
  it("never counts backwards (a clock change, a hand edit)", () => {
    const h = [step("2026-10-12 17:00", "ready", "building"), step("2026-10-12 16:00", "building", "testing")];
    expect(timeInWork(h, "testing", now)).toBe(120);
  });
});

describe("wall-clock time, in 24-hour days (the estimate stays in 8-hour agent days)", () => {
  it("in Building since 18:00 yesterday, at 10:00 today: 16 h", () => {
    const h = [step("2026-10-12 18:00", "ready", "building")];
    expect(fmtWall(timeInWork(h, "building", stampMinutes("2026-10-13 10:00"))!)).toBe("16 h");
  });
  it("minutes, then hours, then 24-hour days", () => {
    expect([0, 1, 45, 59, 60, 90, 23 * 60 + 54, 24 * 60, 25 * 60, 49 * 60 + 30].map(fmtWall)).toEqual([
      "under a minute", "1 min", "45 min", "59 min", "1 h", "1.5 h", "23.9 h", "1 d", "1 d 1 h", "2 d 1.5 h",
    ]);
  });
});

describe("a move on the track", () => {
  const held = task({ stage: "building", claimedBy: "drover/main (session 3fba2222)" });
  it("out of Building held by someone else releases their claim: said so, and no Undo", () => {
    expect(releasedBy(held, "testing")).toBe("drover/main");
    expect(trackMove(held, "testing")).toEqual({ done: "Put in Testing. drover/main's claim on Building was released.", undo: undefined });
  });
  it("any other move offers Undo back to where it was", () => {
    expect(releasedBy(task({ stage: "building", claimedBy: "owner" }), "testing")).toBe("");
    expect(trackMove(task({ stage: "building", claimedBy: "owner" }), "testing")).toEqual({ done: "Put in Testing.", undo: { back: "building", to: "testing" } });
    expect(releasedBy(task({ stage: "ready", claimedBy: "wt-a" }), "shaping")).toBe("");
    expect(trackMove(task({ stage: "ready", claimedBy: "wt-a" }), "shaping").undo).toEqual({ back: "ready", to: "shaping" });
    expect(releasedBy(held, "building")).toBe("");
  });
  it("from a stage the window doesn't know: no Undo", () => {
    expect(trackMove(task({ stage: "reviewing" }), "ready")).toEqual({ done: "Put in Ready.", undo: undefined });
  });
});

describe("drags the pane takes", () => {
  it("only drags that carry files; text goes to the box it's dropped on", () => {
    expect(isFileDrag(["Files"])).toBe(true);
    expect(isFileDrag(["text/uri-list", "Files"])).toBe(true);
    expect(isFileDrag(["text/plain"])).toBe(false);
    expect(isFileDrag(["text/plain", "text/html"])).toBe(false);
    expect(isFileDrag([])).toBe(false);
  });
});

describe("the criteria from the write-up", () => {
  it("is the section under its heading, up to the next one", () => {
    const text = "# T\n\nWhy.\n\n## Acceptance criteria\n- [ ] AC1: it works — verify: command\n- [x] AC2: it's fast\n\n## Log\n- 2026-10-12 x: y\n";
    expect(criteriaSection(text)).toBe("- [ ] AC1: it works — verify: command\n- [x] AC2: it's fast");
  });
  it("runs to the end when it's the last section; ignores a deeper heading inside", () => {
    expect(criteriaSection("## Acceptance criteria\n- [ ] AC1\n### Notes\nmore\n")).toBe("- [ ] AC1\n### Notes\nmore");
  });
  it("is null without the heading or with nothing under it", () => {
    expect(criteriaSection("# T\n\nNo criteria here.\n")).toBeNull();
    expect(criteriaSection("## Acceptance criteria\n\n## Log\n")).toBeNull();
    expect(criteriaSection("Text about ## Acceptance criteria inline\n")).toBeNull();
  });
});

describe("Undo after a move on the track", () => {
  const msg = { ok: true, text: "Put in Testing.", undo: { back: "building", to: "testing" } };
  it("is offered while the task is where the move put it", () => {
    expect(undoFor(msg, "testing")).toEqual({ back: "building", to: "testing" });
  });
  it("is gone once it moved on, before the refresh shows the move, or for any other message", () => {
    expect(undoFor(msg, "your-check")).toBeNull();
    expect(undoFor(msg, "building")).toBeNull();
    expect(undoFor({ ok: true, text: "Priority set." }, "testing")).toBeNull();
    expect(undoFor(null, "testing")).toBeNull();
  });
});

describe("the stage track", () => {
  it("marks the stages before, at and after the task's", () => {
    expect(["backlog", "ready", "building", "done"].map((s) => trackPlace(s, "building"))).toEqual(["past", "past", "cur", "next"]);
  });
  it("an unknown stage has no current place: every stage reads as ahead", () => {
    expect(["backlog", "done"].map((s) => trackPlace(s, "reviewing"))).toEqual(["next", "next"]);
  });
});
