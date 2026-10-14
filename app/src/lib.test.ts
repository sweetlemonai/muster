import { describe, expect, it, vi } from "vitest";
import { humanColumn } from "./human";
import {
  ago, boardColumn, checkinLine, claimant, columns, currentCheckin, earlierStages, estimateTotal, feed, fileName, fmtMinutes, fmtTokens, heldByOther, holder, holderOf, inProgress, LEASE_SECS, leaseRunOut,
  builtTested, homePath, copiedPath, needsTester, nextStage, ofEstimate, oneAtATime, passLabel, queueTotal, quietCheckin, quietMinutes, sameHolder, shortTokens, splitHolder, stageHolders, stageOf, STAGES, stamp, stampMinutes, startedAgo, whoLine,
} from "./lib";
import type { Task } from "./types";

const task = (over: Partial<Task>): Task => ({
  queue: "drover", id: "0001", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "todo/0001-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", search: "", ...over,
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
  it("takes the owner's own task To do → Doing → Done: Start, then Done, no Testing (0022)", () => {
    expect(nextStage("ready", true)?.id).toBe("building");
    for (const s of ["building", "testing", "your-check"]) expect(nextStage(s, true)?.id, s).toBe("done");
    expect(nextStage("done", true)).toBeNull();
    expect(nextStage("weird", true)).toBeNull();
    expect(passLabel("ready", true)).toBe("Start");
    expect(passLabel("building", true)).toBe("Done");
    expect(passLabel("your-check", true)).toBe("Done");
    // From Backlog or Shaping (an override put it there) the owner's Start goes straight to Doing.
    expect(nextStage("backlog", true)?.id).toBe("building");
    expect(nextStage("shaping", true)?.id).toBe("building");
    expect(passLabel("backlog", true)).toBe("Start");
    expect(passLabel("shaping", true)).toBe("Start");
    expect(passLabel("done", true)).toBeNull();
  });
  it("lists the stages a task can be sent back to, nearest first", () => {
    expect(earlierStages("testing").map((s) => s.id)).toEqual(["building", "ready", "shaping", "backlog"]);
    expect(earlierStages("backlog")).toEqual([]);
    expect(earlierStages("weird")).toEqual([]);
  });
  it("in progress is Building and Testing, not while paused (mu-59)", () => {
    expect(STAGES.filter((s) => inProgress(task({ stage: s.id }))).map((s) => s.id)).toEqual(["building", "testing"]);
    expect(STAGES.filter((s) => inProgress(task({ stage: s.id, paused: "2026-10-13 10:00" })))).toEqual([]);
  });
  it("says who else holds Ready or Building (the owner can't pass those)", () => {
    expect(heldByOther(task({ stage: "building", claimedBy: "drover/main (session 3fba2222)" }))).toBe("drover/main");
    expect(heldByOther(task({ stage: "ready", claimedBy: "wt-b" }))).toBe("wt-b");
    expect(heldByOther(task({ stage: "building", claimedBy: "owner" }))).toBe("");
    expect(heldByOther(task({ stage: "building", claimedBy: "" }))).toBe("");
    expect(heldByOther(task({ stage: "testing", claimedBy: "qa" }))).toBe("");
  });
  it("shows who holds a task from Ready to Your check: the claim, then held-by (0030)", () => {
    expect(holder(task({ stage: "building", claimedBy: "drover/main (session 3fba2222)" }))).toBe("drover/main");
    expect(holder(task({ stage: "ready", claimedBy: "owner" }))).toBe("owner");
    expect(holder(task({ stage: "done", claimedBy: "wt-a", heldBy: "qa" }))).toBe("");
    expect(holder(task({ stage: "testing", claimedBy: "wt-a" }))).toBe("");
    expect(holder(task({ stage: "testing", claimedBy: "wt-a", heldBy: "qa (wt-0030 · session 7b5e8856)" }))).toBe("qa");
    expect(holder(task({ stage: "your-check" }))).toBe("owner");
    expect(holder(task({ stage: "building", claimedBy: " " }))).toBe("");
    expect(holderOf(task({ stage: "testing", heldBy: "qa (session 7b5e8856)" }))).toBe("qa (session 7b5e8856)");
    // Only Ready and Building are claims: the owner's pass is refused only there.
    expect(heldByOther(task({ stage: "testing", heldBy: "qa" }))).toBe("");
    expect(heldByOther(task({ stage: "your-check", heldBy: "owner" }))).toBe("");
    expect(heldByOther(task({ stage: "building", claimedBy: "owner (session 7b5e8856)" }))).toBe("");
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
  it("has the Paused lane, then one column per stage, in order (mu-59)", () => {
    expect(Object.keys(columns([]))).toEqual(["paused", ...STAGES.map((s) => s.id)]);
  });
  it("puts a paused task only in the Paused lane, out of its stage's column; resumed, it's back where it was (mu-59)", () => {
    const claim = "wt-a (wt-0045 · session 7b5e8856 · cc-hg)";
    const held = task({ id: "0002", stage: "building", claimedBy: claim, paused: "2026-10-13 10:00", pausedWhy: "later" });
    const c = columns([task({ id: "0001", stage: "building" }), held, task({ id: "0003", stage: "backlog", paused: "2026-10-13 09:00" }), task({ id: "0004", forWho: "human", stage: "ready", paused: "2026-10-13 09:00" })]);
    expect(c.paused.map((t) => t.id)).toEqual(["0002", "0003", "0004"]);
    expect(c.building.map((t) => t.id)).toEqual(["0001"]);
    expect(c.backlog).toEqual([]);
    expect(c.ready).toEqual([]);
    // The lane is only a view: the stage and the claim are the task's, untouched.
    expect(c.paused[0]).toMatchObject({ stage: "building", claimedBy: claim });
    // Resume clears the pause and nothing else: the next poll puts it back in its column.
    const back = columns([task({ id: "0001", stage: "building" }), { ...held, paused: "", pausedWhy: "" }]);
    expect(back.paused).toEqual([]);
    expect(back.building.map((t) => [t.id, t.claimedBy])).toEqual([["0001", ""], ["0002", claim]]);
    // The lane keeps the board's sort.
    const sorted = columns([task({ id: "0005", paused: "x" }), task({ id: "0006", paused: "x", priority: "high" })]);
    expect(sorted.paused.map((t) => t.id)).toEqual(["0006", "0005"]);
    expect(columns([task({ id: "0005", paused: "x" }), task({ id: "0006", paused: "x", priority: "high" })], "id").paused.map((t) => t.id)).toEqual(["0005", "0006"]);
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
  it("sorts every column by the board's Sort chip when given one, Done included", () => {
    const list = [
      task({ id: "0001", captured: "2026-10-02" }),
      task({ id: "0002", captured: "2026-10-05", priority: "low" }),
      task({ id: "0003", stage: "done", captured: "2026-10-01", modified: 9 }),
      task({ id: "0004", stage: "done", captured: "2026-10-03", modified: 1 }),
    ];
    const c = columns(list, "newest");
    expect(c.ready.map((t) => t.id)).toEqual(["0002", "0001"]);
    expect(c.done.map((t) => t.id)).toEqual(["0004", "0003"]);
    expect(columns(list, "id").ready.map((t) => t.id)).toEqual(["0001", "0002"]);
    expect(columns(list).done.map((t) => t.id)).toEqual(["0003", "0004"]);
  });
  it("sorts done by most recently changed", () => {
    const c = columns([task({ id: "0001", stage: "done", modified: 1 }), task({ id: "0002", stage: "done", status: "blocked", blocked: true, modified: 9 })]);
    expect(c.done.map((t) => t.id)).toEqual(["0002", "0001"]);
  });
  it("puts the owner's own tasks in the same columns: To do in Ready, Doing in Building, Done in Done (0022)", () => {
    const stages = ["backlog", "shaping", "ready", "building", "testing", "your-check", "done", "weird"];
    const c = columns([...stages.map((stage, i) => task({ id: `000${i}`, stage, forWho: "human" })), task({ id: "0010", stage: "shaping" }), task({ id: "0011", stage: "testing" })]);
    expect(c.ready.map((t) => t.id)).toEqual(["0000", "0001", "0002", "0007"]);
    expect(c.building.map((t) => t.id)).toEqual(["0003", "0004", "0005"]);
    expect(c.done.map((t) => t.id)).toEqual(["0006"]);
    // Never in Backlog, Shaping, Testing or Your check: those are the agents' stages.
    expect(c.backlog).toEqual([]);
    expect(c.shaping.map((t) => t.id)).toEqual(["0010"]);
    expect(c.testing.map((t) => t.id)).toEqual(["0011"]);
    expect(c["your-check"]).toEqual([]);
    // Each in the column's sort with the agents' tasks around it.
    const mixed = columns([task({ id: "0001" }), task({ id: "0002", forWho: "human", priority: "high" }), task({ id: "0003", priority: "low" })]);
    expect(mixed.ready.map((t) => t.id)).toEqual(["0002", "0001", "0003"]);
  });
  it("matches the column the window names the owner's own task by (human.ts humanColumn)", () => {
    for (const stage of ["backlog", "shaping", "ready", "building", "testing", "your-check", "done", "weird"]) {
      expect(boardColumn({ stage, forWho: "human" }), stage).toBe({ todo: "ready", doing: "building", done: "done" }[humanColumn(stage)]);
      expect(boardColumn({ stage, forWho: "ai" }), stage).toBe(stageOf({ stage }));
    }
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
    // Blocked only by the tasks it depends on (0024): no flag, no waiting_on, still blocked.
    const d = feed([], [task({ blockedKind: "tasks", blockedBy: [{ id: "0041", stage: "ready", holder: "" }, { id: "0042", stage: "building", holder: "wt-a" }] })]);
    expect(d[0].what).toBe("0001 T · Ready, blocked on 0041, 0042");
    // By keys (0033), with the project's prefix.
    const k = feed([], [task({ prefix: "mu", blockedKind: "tasks", blockedBy: [{ id: "0041", stage: "ready", holder: "" }] })]);
    expect(k[0].what).toBe("mu-1 T · Ready, blocked on mu-41");
  });
});

describe("lease (the core's ops::reclaim rule)", () => {
  /** The moment the wall clock here reads `s`, in seconds since 1970 (the state's `now`). */
  const at = (s: string) => {
    const [y, mo, d, h, mi] = s.split(/[- :]/).map(Number);
    return new Date(y, mo - 1, d, h, mi).getTime() / 1000;
  };
  const m = (s: string) => stampMinutes(s) ?? NaN;
  it("reads tool stamps as wall-clock minutes, and nothing else (ops::stamp_minutes)", () => {
    expect(stampMinutes("2026-10-12 14:05")).toBe(Date.UTC(2026, 9, 12, 14, 5) / 60000);
    expect(m("2027-01-01 01:00") - m("2026-12-31 23:00")).toBe(120);
    expect(m("2028-03-01 12:00") - m("2028-02-28 12:00")).toBe(2 * 1440);
    for (const bad of ["", "2026-10-12", "2026-10-12T14:05", "2026-13-01 10:00", "2026-02-30 10:00", "2026-10-12 24:00", "2026-10-12 10:60", " 2026-10-12 10:00"]) {
      expect(stampMinutes(bad)).toBeNull();
    }
  });
  it("counts wall-clock minutes across a clock change, as the core does", () => {
    vi.stubEnv("TZ", "America/New_York");
    try {
      // 2026-11-01 02:00 the clocks go back an hour: 12 wall-clock hours are 13 real ones.
      const now = at("2026-11-01 12:30");
      expect(now - at("2026-11-01 00:30")).toBe(13 * 3600);
      const held = task({ stage: "building", claimedBy: "wt-a", heartbeat: "2026-11-01 00:30", modified: now });
      expect(quietMinutes(held, now)).toBe(720);
      expect(leaseRunOut(held, now)).toBe(false);
    } finally {
      vi.unstubAllEnvs();
    }
  });
  it("is quiet since the heartbeat, else since the file changed, in whole minutes", () => {
    const now = at("2026-10-12 14:05");
    expect(quietMinutes({ heartbeat: "2026-10-12 01:00", modified: now - 60 }, now)).toBe(13 * 60 + 5);
    expect(quietMinutes({ heartbeat: "", modified: now - 3 * 3600 - 59 }, now)).toBe(180);
    expect(quietMinutes({ heartbeat: "yesterday", modified: now - 600 }, now)).toBe(10);
    expect(quietMinutes({ heartbeat: "2026-10-13 01:00", modified: 0 }, now)).toBe(0);
  });
  it("runs out on a held, unblocked Building claim quiet for more than 12 h", () => {
    const now = at("2026-10-12 14:05");
    const held = { stage: "building", claimedBy: "wt-a (session aaaa1111)", modified: now };
    expect(leaseRunOut(task({ ...held, heartbeat: "2026-10-12 02:04" }), now)).toBe(true);
    // 12 h exactly, or 12 h and 59 s: not yet (the core counts whole minutes).
    expect(leaseRunOut(task({ ...held, heartbeat: "2026-10-12 02:05" }), now)).toBe(false);
    expect(leaseRunOut(task({ ...held, heartbeat: "2026-10-12 02:05" }), now + 59)).toBe(false);
    expect(leaseRunOut(task({ ...held, heartbeat: "2026-10-12 02:05" }), now + 60)).toBe(true);
    // A fresh heartbeat wins over an old file, and an old file counts without one.
    expect(leaseRunOut(task({ ...held, heartbeat: "2026-10-12 14:00", modified: 0 }), now)).toBe(false);
    expect(leaseRunOut(task({ ...held, heartbeat: "", modified: now - 13 * 3600 }), now)).toBe(true);
    // Blocked (waiting), nobody holding it, or another stage: no lease.
    const old = { ...held, heartbeat: "2026-10-01 10:00" };
    expect(leaseRunOut(task({ ...old, blocked: true, waitingOn: "owner" }), now)).toBe(false);
    expect(leaseRunOut(task({ ...old, blockedKind: "tasks" }), now)).toBe(false);
    expect(leaseRunOut(task({ ...old, claimedBy: " " }), now)).toBe(false);
    expect(leaseRunOut(task({ ...old, stage: "testing" }), now)).toBe(false);
    // The owner's own task (0022) has no lease, as the core's reclaim leaves it.
    expect(leaseRunOut(task({ ...old, claimedBy: "owner", forWho: "human" }), now)).toBe(false);
    expect(leaseRunOut(task({ ...old, claimedBy: "owner" }), now)).toBe(true);
    expect(LEASE_SECS).toBe(12 * 3600);
  });
  it("uses this machine's lease from Settings when given (0036)", () => {
    const now = at("2026-10-12 14:05");
    const held = { stage: "building", claimedBy: "wt-a (session aaaa1111)", modified: now };
    // Two hours quiet: within the default 12 h, past a 1-hour lease, not past 2 h exactly.
    const twoHours = task({ ...held, heartbeat: "2026-10-12 12:05" });
    expect(leaseRunOut(twoHours, now)).toBe(false);
    expect(leaseRunOut(twoHours, now, 60)).toBe(true);
    expect(leaseRunOut(twoHours, now, 120)).toBe(false);
    expect(leaseRunOut(twoHours, now, 119)).toBe(true);
  });
});

describe("who's on it, since when, and how far (0030)", () => {
  const at = (s: string) => {
    const [y, mo, d, h, mi] = s.split(/[- :]/).map(Number);
    return new Date(y, mo - 1, d, h, mi).getTime() / 1000;
  };
  const now = at("2026-10-12 14:05");
  const A = "wt-a (main-29 · wt-0030 · session 7b5e8856 · cc-hg)";
  it("reads the core's session stamp, and nothing else, as one (session.rs split)", () => {
    expect(splitHolder(A)).toEqual({ label: "wt-a", parts: ["main-29", "wt-0030"], id: "7b5e8856", account: "hg" });
    expect(splitHolder("drover/main (session 3fba2222)")).toEqual({ label: "drover/main", parts: [], id: "3fba2222", account: "" });
    for (const who of ["wt-a", "capture (drover/main, session 349bc9c3)", "x (session 7B5E8856)", "x (session 7b5e8856 · hg)", "x (a · b · c · session 7b5e8856)", "(session 7b5e8856)", "x (session 7b5e8856) more"]) {
      expect(splitHolder(who)).toEqual({ label: who, parts: [], id: "", account: "" });
    }
  });
  it("puts who's on it in one short line: name, checkout, short session; you for the owner", () => {
    expect(whoLine(task({ stage: "building", claimedBy: A }))).toBe("wt-a · main-29 · wt-0030 · 7b5e8856");
    expect(whoLine(task({ stage: "building", claimedBy: "muster/wt-0030 (who's on it) (session 7b5e8856)" }))).toBe("muster/wt-0030 · 7b5e8856");
    expect(whoLine(task({ stage: "building", claimedBy: "capture (drover/main, session 349bc9c3)" }))).toBe("capture · 349bc9c3");
    expect(whoLine(task({ stage: "testing", heldBy: "wt-0030 (session 7b5e8856)" }))).toBe("wt-0030 · 7b5e8856");
    expect(whoLine(task({ stage: "your-check" }))).toBe("you");
    expect(whoLine(task({ stage: "building", claimedBy: "owner (session 7b5e8856)" }))).toBe("you · 7b5e8856");
    expect(whoLine(task({ stage: "ready" }))).toBe("");
    expect(whoLine(task({ stage: "done", claimedBy: A }))).toBe("");
  });
  it("says when Building or Testing started, from the stage line into it", () => {
    expect(startedAgo(task({ stage: "building", stageSince: "2026-10-12 12:00" }), now)).toBe("started 2 h ago");
    expect(startedAgo(task({ stage: "testing", stageSince: "2026-10-12 13:53" }), now)).toBe("started 12 min ago");
    expect(startedAgo(task({ stage: "building", stageSince: "2026-10-12 14:05" }), now)).toBe("started just now");
    expect(startedAgo(task({ stage: "building", stageSince: "2026-10-10 09:00" }), now)).toBe("started 2 d ago");
    expect(startedAgo(task({ stage: "ready", stageSince: "2026-10-12 12:00" }), now)).toBe("");
    expect(startedAgo(task({ stage: "your-check", stageSince: "2026-10-12 12:00" }), now)).toBe("");
    expect(startedAgo(task({ stage: "building", stageSince: "" }), now)).toBe("");
  });
  it("shows the latest check-in of this Building stage with its percentage and age", () => {
    const c = (at: string, pct: number | null, text: string) => ({ at, who: A, pct, text });
    const t = task({ stage: "building", stageSince: "2026-10-12 12:00", checkins: [c("2026-10-12 11:00", 90, "last round"), c("2026-10-12 13:53", 60, "gate green, writing tests")] });
    expect(currentCheckin(t)?.text).toBe("gate green, writing tests");
    expect(checkinLine(currentCheckin(t)!, now)).toBe("60% · gate green, writing tests · 12 min ago");
    expect(checkinLine(c("2026-10-12 14:05", null, "started"), now)).toBe("started · just now");
    expect(currentCheckin(task({ ...t, checkins: [c("2026-10-12 11:00", 90, "last round")] }))).toBeNull();
    expect(currentCheckin({ ...t, stage: "testing" })).toBeNull();
  });
  it("flags a Building session quiet for more than 30 min, by its last check-in or heartbeat", () => {
    const held = task({ stage: "building", claimedBy: A, modified: now });
    expect(quietCheckin({ ...held, heartbeat: "2026-10-12 13:20" }, now)).toBe(45);
    expect(quietCheckin({ ...held, heartbeat: "2026-10-12 13:35" }, now)).toBeNull();
    expect(quietCheckin({ ...held, heartbeat: "2026-10-12 13:34" }, now)).toBe(31);
    expect(quietCheckin({ ...held, heartbeat: "", modified: now - 3600 }, now)).toBe(60);
    // Waiting isn't going quiet, the owner doesn't check in, and only Building is worked on.
    expect(quietCheckin(task({ ...held, heartbeat: "2026-10-12 10:00", blocked: true, waitingOn: "owner" }), now)).toBeNull();
    // Blocked by tasks it depends on (0024) is waiting too, with no flag.
    expect(quietCheckin(task({ ...held, heartbeat: "2026-10-12 10:00", blockedKind: "tasks" }), now)).toBeNull();
    expect(quietCheckin({ ...held, heartbeat: "2026-10-12 10:00", claimedBy: "owner" }, now)).toBeNull();
    expect(quietCheckin({ ...held, heartbeat: "2026-10-12 10:00", stage: "testing" }, now)).toBeNull();
    expect(quietCheckin({ ...held, heartbeat: "2026-10-12 10:00", claimedBy: "" }, now)).toBeNull();
  });
  it("lists the holder of each stage so far, the current one live", () => {
    const history = [
      { at: "2026-10-12 10:00", who: A, from: "ready", to: "building" },
      { at: "2026-10-12 11:00", who: A, from: "building", to: "testing" },
      { at: "2026-10-12 12:00", who: "qa", from: "testing", to: "your-check" },
    ];
    expect(stageHolders(task({ stage: "your-check", stageHistory: history, heldBy: "owner" }))).toEqual([
      { stage: "building", who: A },
      { stage: "testing", who: A },
      { stage: "your-check", who: "owner" },
    ]);
    expect(stageHolders(task({ stage: "testing", stageHistory: history.slice(0, 2), heldBy: "qa (session 11111111)" }))).toEqual([
      { stage: "building", who: A },
      { stage: "testing", who: "qa (session 11111111)" },
    ]);
    expect(stageHolders(task({ stage: "ready" }))).toEqual([]);
    expect(stageHolders(task({ stage: "building", claimedBy: "wt-b" }))).toEqual([{ stage: "building", who: "wt-b" }]);
  });
});

describe("the tester isn't the builder (0047)", () => {
  const A = "wt-a (wt-0030 · session 7b5e8856 · cc-hg)";
  it("matches holders as the core does (session.rs same_holder)", () => {
    expect(sameHolder(A, A)).toBe(true);
    expect(sameHolder(A, "wt-a (main · session 7b5e8856 · cc-hg)")).toBe(true);
    expect(sameHolder(A, "wt-a")).toBe(true);
    expect(sameHolder("wt-a", A)).toBe(true);
    expect(sameHolder(A, "wt-a (session 11111111)")).toBe(false);
    expect(sameHolder(A, "wt-b (session 7b5e8856)")).toBe(false);
    expect(sameHolder("", "wt-a")).toBe(false);
  });
  it("a Testing task still held by its builder needs a tester", () => {
    expect(needsTester(task({ stage: "testing", heldBy: A, builtBy: A }))).toBe(true);
    // Rebuilt after a return by a new session under the builder's label, which gated it in.
    expect(needsTester(task({ stage: "testing", heldBy: "wt-a (session 33333333)", builtBy: A, gatedBy: "wt-a (session 33333333)" }))).toBe(true);
    // The builders are the claimer and the last gated passer, as the core's (ops.rs tester_rule):
    // back in Building by an override with no claim, then gated in by wt-b; or never claimed.
    expect(needsTester(task({ stage: "testing", heldBy: "wt-b", builtBy: A, gatedBy: "wt-b" }))).toBe(true);
    expect(needsTester(task({ stage: "testing", heldBy: "wt-b", gatedBy: "wt-b" }))).toBe(true);
    // Another session under the claimer's label isn't the claimer (session.rs same_holder).
    expect(needsTester(task({ stage: "testing", heldBy: "wt-a (session 33333333)", builtBy: A }))).toBe(false);
    // Put in Testing by an override: its holder didn't build it.
    expect(needsTester(task({ stage: "testing", heldBy: "capture", builtBy: A, gatedBy: A }))).toBe(false);
    expect(needsTester(task({ stage: "testing", heldBy: "owner", builtBy: A }))).toBe(false);
    expect(needsTester(task({ stage: "testing", heldBy: A }))).toBe(false);
    expect(needsTester(task({ stage: "your-check", heldBy: A, builtBy: A }))).toBe(false);
    expect(needsTester(task({ stage: "testing", heldBy: "owner", builtBy: "owner", forWho: "human" }))).toBe(false);
  });
  it("says who built it and who tested it, short, or that it needs a tester", () => {
    expect(builtTested(task({ stage: "your-check", builtBy: A, testedBy: "qa-0030 (session 99999999)" }))).toBe("built by wt-a · tested by qa-0030");
    expect(builtTested(task({ stage: "done", builtBy: A, testedBy: "owner (session 22222222)" }))).toBe("built by wt-a · tested by you");
    expect(builtTested(task({ stage: "testing", heldBy: A, builtBy: A }))).toBe("built by wt-a · needs a tester");
    expect(builtTested(task({ stage: "testing", heldBy: "owner", builtBy: A }))).toBe("built by wt-a");
    expect(builtTested(task({ stage: "your-check", builtBy: A }))).toBe("built by wt-a");
    expect(builtTested(task({ stage: "your-check", builtBy: A, gatedBy: "wt-a (main · session 7b5e8856 · cc-hg)" }))).toBe("built by wt-a");
    expect(builtTested(task({ stage: "testing", heldBy: "wt-b", builtBy: A, gatedBy: "wt-b" }))).toBe("built by wt-a and wt-b · needs a tester");
    expect(builtTested(task({ stage: "your-check", gatedBy: "wt-b", testedBy: "qa" }))).toBe("built by wt-b · tested by qa");
    // Nobody built it on record, it isn't that far yet, or it's the owner's own task: nothing.
    expect(builtTested(task({ stage: "your-check", testedBy: "qa" }))).toBe("");
    expect(builtTested(task({ stage: "building", builtBy: A }))).toBe("");
    expect(builtTested(task({ stage: "done", builtBy: "owner", testedBy: "owner", forWho: "human" }))).toBe("");
    // The pane names each in full.
    expect(builtTested(task({ stage: "your-check", builtBy: A, testedBy: "qa" }), (w) => `[${w}]`)).toBe(`built by [${A}] · tested by [qa]`);
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
    const t = (m: number, k: number, stage = "ready") => ({ estMinutes: m, estTestMinutes: 0, estTokenCount: k, stage });
    expect(estimateTotal([t(60, 100_000), t(30, 50_000), t(0, 0), t(600, 0, "done")])).toBe("≈ 1.5 h · 150k tokens left (2 of 3 open tasks estimated)");
    expect(estimateTotal([t(0, 0)])).toBe("");
  });
});

describe("work against an estimate (0025)", () => {
  it("shortTokens: a count without the word", () => {
    expect([0, 16, 999, 1000, 3500, 6000, 9_960, 11_501, 69_516, 999_499, 999_500, 1_234_567, 12_000_000].map(shortTokens)).toEqual([
      "0", "16", "999", "1k", "3.5k", "6k", "10k", "12k", "70k", "999k", "1M", "1.2M", "12M",
    ]);
  });
  it("ofEstimate: the actual, of the estimate, with the share; the actual alone without one", () => {
    expect(ofEstimate("1.2M", "2M", 60)).toBe("1.2M of 2M (60%)");
    expect(ofEstimate("3 h", "2 h", 150)).toBe("3 h of 2 h (150%)");
    expect(ofEstimate("40 min", "", null)).toBe("40 min");
    expect(ofEstimate("40 min", "2 h", null)).toBe("40 min");
  });
});

describe("homePath: a path under the home folder shown as ~ (owner, 2026-10-13)", () => {
  const home = "/Users/someone";
  it("shortens a path under home, and home itself", () => {
    expect(homePath("/Users/someone/playground/muster/tasks", home)).toBe("~/playground/muster/tasks");
    expect(homePath("/Users/someone/Library/Application Support/ai.sweetlemon.muster", home)).toBe("~/Library/Application Support/ai.sweetlemon.muster");
    expect(homePath("/Users/someone", home)).toBe("~");
    expect(homePath("/Users/someone/", home)).toBe("~/");
  });
  it("takes home with or without a trailing slash", () => {
    expect(homePath("/Users/someone/.claude/projects", "/Users/someone/")).toBe("~/.claude/projects");
    expect(homePath("/Users/someone/.claude/projects", "/Users/someone//")).toBe("~/.claude/projects");
    expect(homePath("/Users/someone", "/Users/someone/")).toBe("~");
  });
  it("leaves a path not under home as it is", () => {
    expect(homePath("/Volumes/Work/tasks", home)).toBe("/Volumes/Work/tasks");
    // A sibling whose name starts with home's isn't under it.
    expect(homePath("/Users/someone2/tasks", home)).toBe("/Users/someone2/tasks");
    expect(homePath("/Users", home)).toBe("/Users");
    expect(homePath("relative/Users/someone/x", home)).toBe("relative/Users/someone/x");
  });
  it("shortens nothing when home is unknown or the root", () => {
    expect(homePath("/Users/someone/x", "")).toBe("/Users/someone/x");
    expect(homePath("/Users/someone/x", "/")).toBe("/Users/someone/x");
  });
  it("copying a shown path, or part of it, gives the real path's characters", () => {
    const full = "/Users/someone/playground/x";
    const shown = homePath(full, home);
    const within = (start: number, end: number) => copiedPath(shown, full, { text: shown.slice(start, end), start, end });
    expect(within(0, shown.length)).toBe(full);
    // From the ~ to partway: the home part comes along.
    expect(within(0, 12)).toBe("/Users/someone/playground");
    // After the ~: the same characters as shown.
    expect(within(2, 12)).toBe("playground");
    expect(within(1, 1)).toBe("");
    expect(within(0, 0)).toBe("");
  });
  it("copying the whole path from outside it (a triple click) gives the real path; anything else is left alone", () => {
    const full = "/Users/someone/playground/x";
    const shown = homePath(full, home);
    expect(copiedPath(shown, full, { text: `${shown}\n` })).toBe(full);
    expect(copiedPath(shown, full, { text: `Settings ${shown}` })).toBeNull();
    // A path that wasn't shortened copies as selected.
    expect(copiedPath("/Volumes/x", "/Volumes/x", { text: "Volumes", start: 1, end: 8 })).toBeNull();
  });
});
