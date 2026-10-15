import { afterEach, describe, expect, it, vi } from "vitest";
import { builtBy, byKind, dayGroups, finishes, finishText, KIND_FILTERS, loadKind, RECENT_KEY, storeKind, whenText } from "./recentData";
import { columns } from "./lib";
import type { StageStep, Task } from "./types";

const task = (id: string, over: Partial<Task> = {}): Task => ({
  queue: "muster", id, title: `Task ${id}`, kind: "code", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: `todo/${id}-t.md`, claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});

// The fake clock: 14:00 local time on Tuesday 2026-10-13, in seconds since 1970, like the state's `now`.
const NOW = new Date(2026, 9, 13, 14, 0).getTime() / 1000;
const step = (at: string, from: string, to: string, who = "muster/wt-a (main · session 7b5e8856 · cc-hg)"): StageStep => ({ at, who, from, to });
/** A task's log from Building to where it is: built by `builder`, merged at `merged`, accepted at `accepted`. */
const finished = (id: string, merged: string, accepted = "", over: Partial<Task> = {}): Task => {
  const history = [
    step("2026-10-01 09:00", "ready", "building", "muster/wt-0051 (w-0051) (main · session 531266d4 · cc-hg)"),
    step("2026-10-01 10:00", "building", "testing"),
    step(merged, "testing", "your-check", "muster/main (merge) (session 7b5e8856 · cc-hg)"),
  ];
  if (accepted) history.push(step(accepted, "your-check", "done", "owner"));
  return task(id, { stage: accepted ? "done" : "your-check", stageHistory: history, doneAt: accepted, ...over });
};
const rows = (list: Task[]) => finishes(list).map((f) => `${f.task.id} ${f.kind} ${f.at}`);

describe("what finished: merged and accepted, from the stage log", () => {
  it("gives a merged row and an accepted row per task, newest first, at the log's times", () => {
    const list = [
      finished("0010", "2026-10-13 13:56"),
      finished("0011", "2026-10-12 18:20", "2026-10-13 09:15"),
      finished("0012", "2026-10-13 11:00", "2026-10-13 12:30"),
    ];
    expect(rows(list)).toEqual([
      "0010 merged 2026-10-13 13:56",
      "0012 accepted 2026-10-13 12:30",
      "0012 merged 2026-10-13 11:00",
      "0011 accepted 2026-10-13 09:15",
      "0011 merged 2026-10-12 18:20",
    ]);
  });
  it("takes the latest merge when a task came back from Your check and landed again", () => {
    const t = finished("0020", "2026-10-12 10:00");
    t.stageHistory.push(step("2026-10-12 11:00", "your-check", "building"), step("2026-10-12 12:00", "building", "testing"), step("2026-10-13 08:00", "testing", "your-check"));
    expect(rows([t])).toEqual(["0020 merged 2026-10-13 08:00"]);
  });
  it("lists only what is still finished: a task sent back to Building has no row", () => {
    const back = finished("0021", "2026-10-13 09:00", "", { stage: "building" });
    const reopened = finished("0022", "2026-10-13 09:00", "2026-10-13 10:00", { stage: "testing" });
    expect(rows([back, reopened])).toEqual([]);
  });
  it("counts a merge only from Testing: a hand move into Your check is not one", () => {
    const set = task("0023", { stage: "your-check", stageHistory: [step("2026-10-13 09:00", "building", "your-check")] });
    expect(rows([set])).toEqual([]);
  });
  it("takes Done's time from doneAt, which also reads an older hand move into Done", () => {
    const old = task("0024", { stage: "done", doneAt: "2026-10-12 04:55" });
    expect(rows([old])).toEqual(["0024 accepted 2026-10-12 04:55"]);
    // A Done task whose log has only a date has no time to list it by.
    expect(rows([task("0025", { stage: "done", doneOn: "2026-10-12" })])).toEqual([]);
  });
  it("lists the owner's own task once, as accepted (their Done), and never as merged", () => {
    const own = task("0026", { forWho: "human", stage: "done", doneAt: "2026-10-13 13:00", stageHistory: [step("2026-10-13 12:00", "ready", "building", "owner"), step("2026-10-13 13:00", "building", "done", "owner")] });
    expect(finishes([own]).map((x) => `${x.kind} ${x.at}`)).toEqual(["accepted 2026-10-13 13:00"]);
  });
  it("leaves out goals and features: they finish when their tasks do", () => {
    expect(rows([finished("0030", "2026-10-13 09:00", "2026-10-13 10:00", { level: "feature" })])).toEqual([]);
  });
  it("orders the same minute by accepted first, then project and id", () => {
    const list = [finished("0041", "2026-10-13 09:00"), finished("0040", "2026-10-13 09:00"), task("0042", { stage: "done", doneAt: "2026-10-13 09:00" })];
    expect(rows(list)).toEqual(["0042 accepted 2026-10-13 09:00", "0040 merged 2026-10-13 09:00", "0041 merged 2026-10-13 09:00"]);
  });
});

describe("when it finished, in words", () => {
  it("says minutes and hours today, yesterday with the time, then the date", () => {
    expect(whenText("2026-10-13 14:00", NOW)).toBe("just now");
    expect(whenText("2026-10-13 13:56", NOW)).toBe("4 min ago");
    expect(whenText("2026-10-13 13:01", NOW)).toBe("59 min ago");
    expect(whenText("2026-10-13 12:00", NOW)).toBe("2 h ago");
    expect(whenText("2026-10-13 00:05", NOW)).toBe("13 h ago");
    expect(whenText("2026-10-12 18:20", NOW)).toBe("yesterday 18:20");
    expect(whenText("2026-10-11 09:05", NOW)).toBe("Sun 11 Oct 09:05");
    expect(whenText("2025-12-31 23:59", NOW)).toBe("Wed 31 Dec 2025 23:59");
  });
  it("calls a stamp ahead of the clock just now, and says nothing for one that isn't a stamp", () => {
    expect(whenText("2026-10-13 14:30", NOW)).toBe("just now");
    expect(whenText("2026-10-13", NOW)).toBe("");
    expect(whenText("", NOW)).toBe("");
  });
  it("counts yesterday by the calendar: 40 minutes past midnight is still minutes", () => {
    const early = new Date(2026, 9, 13, 0, 30).getTime() / 1000;
    expect(whenText("2026-10-12 23:50", early)).toBe("40 min ago");
    expect(whenText("2026-10-12 22:00", early)).toBe("yesterday 22:00");
  });
});

describe("a Done card's finish", () => {
  it("says when it was done, from doneAt, else the day it knows", () => {
    expect(finishText(task("0001", { stage: "done", doneAt: "2026-10-13 13:56" }), NOW)).toBe("done 4 min ago");
    expect(finishText(task("0002", { stage: "done", doneOn: "2026-10-12" }), NOW)).toBe("done yesterday");
    expect(finishText(task("0003", { stage: "done", doneOn: "2026-10-13" }), NOW)).toBe("done today");
    expect(finishText(task("0004", { stage: "done", doneOn: "2026-10-01" }), NOW)).toBe("done Thu 1 Oct");
    expect(finishText(task("0005", { stage: "done" }), NOW)).toBe("");
    expect(finishText(task("0006", { stage: "your-check", doneAt: "2026-10-13 13:56" }), NOW)).toBe("");
  });
});

describe("the Done column (AC2)", () => {
  it("sorts newest finished first, the default sort, whatever changed the files last", () => {
    const list = [
      task("0001", { stage: "done", doneAt: "2026-10-12 18:00", modified: 900 }),
      task("0002", { stage: "done", doneAt: "2026-10-13 13:56", modified: 100 }),
      task("0003", { stage: "done", doneOn: "2026-10-13", modified: 999 }),
      task("0004", { stage: "done", modified: 1000 }),
      task("0005", { stage: "done", doneAt: "2026-10-13 09:00", modified: 50 }),
    ];
    expect(columns(list).done.map((t) => t.id)).toEqual(["0002", "0005", "0003", "0001", "0004"]);
  });
});

describe("day groups and the window", () => {
  const list = [
    finished("0001", "2026-10-13 13:56"),
    finished("0002", "2026-10-12 18:20", "2026-10-13 09:15"),
    finished("0003", "2026-10-07 08:00", "2026-10-07 09:00"),
    finished("0004", "2026-10-06 23:59", "2026-10-06 23:59"),
    finished("0005", "2026-09-29 12:00"),
  ];
  it("groups by day, Today and Yesterday then dates, each with its rows; the last 7 days by default", () => {
    const { groups, older } = dayGroups(finishes(list), NOW, 1);
    expect(groups.map((g) => `${g.label} ${g.rows.length}`)).toEqual(["Today 2", "Yesterday 1", "Wed 7 Oct 2"]);
    expect(groups[0].rows.map((r) => r.task.id)).toEqual(["0001", "0002"]);
    expect(older).toBe(3);
  });
  it("adds a week at a time", () => {
    const two = dayGroups(finishes(list), NOW, 2);
    expect(two.groups.map((g) => g.label)).toEqual(["Today", "Yesterday", "Wed 7 Oct", "Tue 6 Oct"]);
    expect(two.older).toBe(1);
    const three = dayGroups(finishes(list), NOW, 3);
    expect(three.groups.at(-1)?.label).toBe("Tue 29 Sep");
    expect(three.older).toBe(0);
  });
  it("puts a stamp ahead of the clock in Today", () => {
    expect(dayGroups(finishes([finished("0009", "2026-10-14 09:00")]), NOW, 1).groups.map((g) => g.label)).toEqual(["Today"]);
  });
  it("has no groups with nothing finished", () => {
    expect(dayGroups([], NOW, 1)).toEqual({ groups: [], older: 0 });
  });
});

describe("the kind chips", () => {
  afterEach(() => vi.unstubAllGlobals());
  const list = [finished("0001", "2026-10-13 13:56"), finished("0002", "2026-10-12 18:20", "2026-10-13 09:15")];
  it("are All, Merged and Accepted, and pick the rows", () => {
    expect(KIND_FILTERS.map((k) => k.label)).toEqual(["All", "Merged", "Accepted"]);
    const all = finishes(list);
    expect(byKind(all, "all")).toHaveLength(3);
    expect(byKind(all, "merged").map((r) => r.task.id)).toEqual(["0001", "0002"]);
    expect(byKind(all, "accepted").map((r) => r.task.id)).toEqual(["0002"]);
  });
  it("are remembered, and anything else or storage that throws reads as All", () => {
    expect(RECENT_KEY).toBe("muster.recentKind");
    expect(loadKind(() => "merged")).toBe("merged");
    expect(loadKind(() => "accepted")).toBe("accepted");
    expect(loadKind(() => "sideways")).toBe("all");
    expect(loadKind(() => null)).toBe("all");
    expect(loadKind(() => { throw new Error("SecurityError"); })).toBe("all");
    const saved: string[] = [];
    expect(storeKind("accepted", (v) => saved.push(v))).toBe(true);
    expect(saved).toEqual(["accepted"]);
    expect(storeKind("merged", () => { throw new Error("QuotaExceeded"); })).toBe(false);
  });
});

describe("who built it", () => {
  it("is the session that took it into Building, short", () => {
    expect(builtBy(finished("0001", "2026-10-13 13:56"))).toBe("muster/wt-0051 · main · 531266d4");
  });
  it("is you for the owner's own task, and empty when the log doesn't say", () => {
    expect(builtBy(task("0002", { forWho: "human", stage: "done" }))).toBe("you");
    expect(builtBy(task("0003", { stage: "done", doneAt: "2026-10-13 09:00" }))).toBe("");
  });
});
