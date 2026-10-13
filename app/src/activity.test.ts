import { stampMinutes } from "./lib";
import { describe, expect, it } from "vitest";
import {
  isWorking, lastMinutes, lastText, nextOrder, loadOrder, ORDER_KEY, orderProjects, projectActivity, projectRows, SIDEBAR_ORDERS, storeOrder, sumActivity,
  type ProjectActivity, type SidebarOrder,
} from "./activity";
import type { Activity, Task } from "./types";

const task = (over: Partial<Task>): Task => ({
  queue: "drover", id: "0001", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "todo/0001-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", search: "", ...over,
});

// The fake clock: 14:00 local time on 2026-10-13, in seconds since 1970, like the state's `now`.
// Stamps are local wall-clock time, as the tools write them.
const secs = (h: number, m: number, day = 13) => new Date(2026, 9, day, h, m).getTime() / 1000;
const NOW = secs(14, 0);
const p = (n: number) => String(n).padStart(2, "0");
const at = (h: number, m: number, day = 13) => `2026-10-${p(day)} ${p(h)}:${p(m)}`;
const checkin = (when: string) => ({ at: when, who: "muster/wt-a", pct: 50, text: "tests" });
const building = (over: Partial<Task> = {}) => task({ stage: "building", claimedBy: "muster/wt-a (session 7b5e8856)", ...over });

describe("a task's last activity", () => {
  it("is the newest of a stage change, a check-in, the heartbeat and the file change, in minutes", () => {
    const t = task({
      stageHistory: [{ at: at(9, 0), who: "x", from: "ready", to: "building" }],
      stageSince: at(9, 0),
      checkins: [checkin(at(11, 0)), checkin(at(12, 30))],
      heartbeat: at(12, 0),
      modified: secs(10, 0),
    });
    expect(lastMinutes(t, NOW)).toBe(90);
    expect(lastMinutes({ ...t, heartbeat: at(13, 48) }, NOW)).toBe(12);
    expect(lastMinutes({ ...t, modified: secs(13, 55) }, NOW)).toBe(5);
    expect(lastMinutes({ ...t, stageHistory: [{ at: at(13, 59), who: "x", from: "building", to: "testing" }] }, NOW)).toBe(1);
  });
  it("is now while a session on it is live", () => {
    expect(lastMinutes(task({ live: true, modified: secs(9, 0) }), NOW)).toBe(0);
  });
  it("is unknown with nothing to go on, and skips stamps it can't read", () => {
    expect(lastMinutes(task({}), NOW)).toBeNull();
    expect(lastMinutes(task({ heartbeat: "yesterday", checkins: [checkin("soon")] }), NOW)).toBeNull();
    expect(lastMinutes(task({ heartbeat: "bad", modified: secs(13, 0) }), NOW)).toBe(60);
  });
  it("counts a stamp ahead of the clock as now", () => {
    expect(lastMinutes(task({ heartbeat: at(14, 5) }), NOW)).toBe(0);
    expect(lastMinutes(task({ modified: NOW + 300 }), NOW)).toBe(0);
  });
  it("counts across midnight", () => {
    const midnight = secs(0, 10, 14);
    expect(lastMinutes(task({ heartbeat: at(23, 50) }), midnight)).toBe(20);
  });
});

describe("a task being worked on now", () => {
  it("is one with a live session", () => {
    expect(isWorking(building({ live: true }), NOW)).toBe(true);
    expect(isWorking(task({ stage: "testing", live: true }), NOW)).toBe(true);
  });
  it("is one with a check-in or heartbeat in the last 30 minutes, the 30th included", () => {
    expect(isWorking(building({ checkins: [checkin(at(13, 30))] }), NOW)).toBe(true);
    expect(isWorking(building({ checkins: [checkin(at(13, 29))] }), NOW)).toBe(false);
    expect(isWorking(building({ heartbeat: at(13, 30) }), NOW)).toBe(true);
    expect(isWorking(building({ heartbeat: at(13, 29) }), NOW)).toBe(false);
  });
  it("isn't one with only a recent stage change or file change", () => {
    expect(isWorking(building({ stageHistory: [{ at: at(13, 59), who: "x", from: "ready", to: "building" }], modified: NOW }), NOW)).toBe(false);
  });
  it("isn't one in Done, or one handed to the owner in Your check, however recent its heartbeat", () => {
    expect(isWorking(task({ stage: "done", heartbeat: at(13, 55) }), NOW)).toBe(false);
    expect(isWorking(task({ stage: "your-check", heartbeat: at(13, 55) }), NOW)).toBe(false);
    expect(isWorking(task({ stage: "your-check", live: true }), NOW)).toBe(true);
  });
});

describe("a project's activity", () => {
  it("counts the tasks being worked on now, and takes the newest activity of any task", () => {
    const tasks = [
      building({ id: "0001", heartbeat: at(13, 50) }),
      task({ id: "0002", stage: "testing", live: true }),
      task({ id: "0003", stage: "ready", modified: secs(8, 0) }),
    ];
    expect(projectActivity(tasks, 0, NOW)).toEqual({ working: 2, stalled: false, last: 0 });
  });
  it("is quiet without either: no count", () => {
    expect(projectActivity([task({ modified: secs(11, 0) })], 0, NOW)).toEqual({ working: 0, stalled: false, last: 180 });
  });
  it("takes the queue's newest commit as activity", () => {
    expect(projectActivity([task({ modified: secs(11, 0) })], secs(13, 48), NOW).last).toBe(12);
    expect(projectActivity([], secs(13, 48), NOW)).toEqual({ working: 0, stalled: false, last: 12 });
    expect(projectActivity([], 0, NOW)).toEqual({ working: 0, stalled: false, last: null });
  });
  it("is stalled with work in Building and nothing for over 30 minutes, not at 30", () => {
    const quiet = (m: number) => projectActivity([building({ heartbeat: at(13, 60 - m) })], 0, NOW);
    expect(quiet(30)).toEqual({ working: 1, stalled: false, last: 30 });
    expect(quiet(31)).toEqual({ working: 0, stalled: true, last: 31 });
  });
  it("isn't stalled while anything else in the project moved in the last 30 minutes", () => {
    const tasks = [building({ heartbeat: at(10, 0) }), task({ id: "0002", stage: "shaping", modified: secs(13, 40) })];
    expect(projectActivity(tasks, 0, NOW)).toEqual({ working: 0, stalled: false, last: 20 });
    expect(projectActivity([building({ heartbeat: at(10, 0) })], secs(13, 40), NOW).stalled).toBe(false);
  });
  it("isn't stalled for work that's blocked (waiting) or the owner's own, or with nothing in Building", () => {
    expect(projectActivity([building({ heartbeat: at(10, 0), blocked: true })], 0, NOW).stalled).toBe(false);
    expect(projectActivity([building({ heartbeat: at(10, 0), claimedBy: "owner" })], 0, NOW).stalled).toBe(false);
    expect(projectActivity([task({ stage: "testing", modified: secs(10, 0) })], 0, NOW).stalled).toBe(false);
  });
  it("is stalled for a Building task nobody can date", () => {
    expect(projectActivity([building({})], 0, NOW)).toEqual({ working: 0, stalled: true, last: null });
  });
});

describe("the sidebar's rows", () => {
  it("are one per queue, each with its own tasks, needs-you count, commits and activity", () => {
    const tasks = [
      building({ queue: "drover", id: "0001", heartbeat: at(13, 50) }),
      task({ queue: "drover", id: "0002", waitingForOwner: true, modified: secs(12, 0) }),
      task({ queue: "muster", id: "0001", modified: secs(9, 0) }),
    ];
    const activity: Activity[] = [
      { queue: "muster", at: secs(11, 0), what: "older", source: "commit" },
      { queue: "muster", at: secs(13, 0), what: "newest", source: "commit" },
      { queue: "drover", at: secs(13, 59), what: "a file", source: "file" },
    ];
    const rows = projectRows([{ name: "drover", path: "/d" }, { name: "muster", path: "/m" }, { name: "empty", path: "/e" }], tasks, activity, NOW);
    expect(rows.map((r) => [r.name, r.title, r.tasks.length, r.needs])).toEqual([["drover", "/d", 2, 1], ["muster", "/m", 1, 0], ["empty", "/e", 0, 0]]);
    expect(rows.map((r) => r.activity)).toEqual([
      { working: 1, stalled: false, last: 10 },
      { working: 0, stalled: false, last: 60 },
      { working: 0, stalled: false, last: null },
    ]);
  });
});

describe("the All projects sums", () => {
  it("count projects working and stalled, and take the newest activity", () => {
    const list: ProjectActivity[] = [
      { working: 2, stalled: false, last: 0 },
      { working: 1, stalled: false, last: 4 },
      { working: 0, stalled: true, last: 45 },
      { working: 1, stalled: false, last: 12 },
      { working: 0, stalled: false, last: null },
    ];
    expect(sumActivity(list)).toEqual({ working: 3, stalled: 1, last: 0 });
  });
  it("are nothing for no projects", () => {
    expect(sumActivity([])).toEqual({ working: 0, stalled: 0, last: null });
  });
});

describe("last activity in words", () => {
  it("says now, minutes, hours, yesterday and days", () => {
    expect([0, 1, 12, 59, 60, 179, 1439, 1440, 2879, 2880, 7200].map((m) => lastText(m, null))).toEqual([
      "now", "1 min ago", "12 min ago", "59 min ago", "1 h ago", "2 h ago", "23 h ago", "yesterday", "yesterday", "2 days ago", "5 days ago",
    ]);
  });
  it("says so when nothing is known", () => {
    expect(lastText(null, null)).toBe("no activity");
  });
});

describe("the sidebar order", () => {
  const row = (name: string, working: number, last: number | null, needs = 0, stalled = false) => ({ name, needs, activity: { working, stalled, last } });
  const rows = [row("muster", 0, 300, 2), row("Drover", 1, 3), row("braindump", 0, null, 1), row("germ", 2, 0), row("chit", 0, 20, 2, true), row("Apple", 0, 300)];
  const names = (by: SidebarOrder) => orderProjects(rows, by).map((r) => r.name);

  it("offers Working first (the default), A–Z and Needs you first", () => {
    expect(SIDEBAR_ORDERS.map((o) => [o.id, o.label])).toEqual([["working", "Working"], ["az", "A–Z"], ["needs", "Needs you"]]);
    for (const o of SIDEBAR_ORDERS) expect(o.title).not.toBe("");
  });
  it("Working: projects being worked on first, then by last activity, newest first, nothing known last, then by name", () => {
    expect(names("working")).toEqual(["germ", "Drover", "chit", "Apple", "muster", "braindump"]);
  });
  it("A–Z: by name, whatever the case", () => {
    expect(names("az")).toEqual(["Apple", "braindump", "chit", "Drover", "germ", "muster"]);
  });
  it("Needs you: most waiting on the owner first, then as Working", () => {
    expect(names("needs")).toEqual(["chit", "muster", "braindump", "germ", "Drover", "Apple"]);
  });
  it("leaves its input alone", () => {
    const copy = [...rows];
    orderProjects(rows, "az");
    expect(rows).toEqual(copy);
  });
});

describe("the remembered order", () => {
  it("reads back what was stored", () => {
    for (const o of ["working", "az", "needs"] as const) expect(loadOrder(() => o)).toBe(o);
  });
  it("is Working first when nothing or something unknown is stored", () => {
    expect(loadOrder(() => null)).toBe("working");
    expect(loadOrder(() => "")).toBe("working");
    expect(loadOrder(() => "priority")).toBe("working");
    expect(loadOrder(() => "toString")).toBe("working");
  });
  it("is Working first when storage can't be read: off, denied, or absent like in a test run", () => {
    expect(loadOrder(() => { throw new Error("SecurityError"); })).toBe("working");
    expect(() => loadOrder()).not.toThrow();
  });
  it("stores under its own key, and says when storage refused", () => {
    const saved: string[] = [];
    expect(storeOrder("az", (v) => saved.push(v))).toBe(true);
    expect(saved).toEqual(["az"]);
    expect(storeOrder("needs", () => { throw new Error("QuotaExceededError"); })).toBe(false);
    expect(ORDER_KEY).toBe("muster.sidebarOrder");
  });
});

describe("review fixes (2026-10-13)", () => {
  const W = (d: number, h: number, m: number) => stampMinutes(`2026-10-${String(d).padStart(2, "0")} ${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`)!;
  it("says yesterday for the day before today, by the calendar", () => {
    // Tue 13th 09:00.
    const wall = W(13, 9, 0);
    expect(lastText(wall - W(11, 23, 30), wall)).toBe("2 days ago"); // Sunday night
    expect(lastText(wall - W(12, 10, 0), wall)).toBe("yesterday"); // Monday morning
    expect(lastText(wall - W(13, 6, 0), wall)).toBe("3 h ago"); // earlier today
    // Just after midnight: 20 minutes ago is minutes, a day and a bit ago is 2 days.
    expect(lastText(20, W(14, 0, 10))).toBe("20 min ago");
    expect(lastText(W(14, 0, 10) - W(12, 23, 50), W(14, 0, 10))).toBe("2 days ago");
  });
  it("counts a heartbeat only while someone holds the task and it isn't waiting", () => {
    const now = new Date(2026, 9, 13, 14, 0).getTime() / 1000;
    const base = { blocked: false, claimedBy: "", heldBy: "", live: false, heartbeat: "2026-10-13 13:55", checkins: [], stageHistory: [], stageSince: "", modified: 0 };
    expect(isWorking({ ...base, stage: "building", claimedBy: "wt-a" }, now)).toBe(true);
    expect(isWorking({ ...base, stage: "ready" }, now)).toBe(false); // handed back
    expect(isWorking({ ...base, stage: "building", claimedBy: "wt-a", blocked: true }, now)).toBe(false); // waiting
    expect(isWorking({ ...base, stage: "testing", heldBy: "qa" }, now)).toBe(true);
    expect(isWorking({ ...base, stage: "testing" }, now)).toBe(false);
    expect(isWorking({ ...base, stage: "ready", live: true }, now)).toBe(true); // a live session still counts
  });
  it("keeps the sidebar's order between polls unless what defines it changes", () => {
    const row = (name: string, working: number, last: number, needs = 0) => ({ name, needs, activity: { working, stalled: false, last } });
    const first = nextOrder(null, [row("zeta", 1, 0), row("alpha", 1, 1)], "working");
    expect(first.names).toEqual(["zeta", "alpha"]);
    // Minutes tick on and flip which is newer: same order (no jump).
    const later = nextOrder(first, [row("zeta", 1, 1), row("alpha", 1, 0)], "working");
    expect(later).toBe(first);
    // A project stops working: worked out again.
    expect(nextOrder(later, [row("zeta", 0, 40), row("alpha", 1, 0)], "working").names).toEqual(["alpha", "zeta"]);
    // A new project, or another order chosen: worked out again.
    expect(nextOrder(first, [row("zeta", 1, 0), row("alpha", 1, 1), row("beta", 0, 5)], "working").names).toHaveLength(3);
    expect(nextOrder(first, [row("zeta", 1, 0), row("alpha", 1, 1)], "az").names).toEqual(["alpha", "zeta"]);
    // In Needs you order, a change in the counts moves rows.
    const needs = nextOrder(null, [row("a", 0, 5, 1), row("b", 0, 5, 0)], "needs");
    expect(nextOrder(needs, [row("a", 0, 5, 0), row("b", 0, 5, 2)], "needs").names).toEqual(["b", "a"]);
  });
});
