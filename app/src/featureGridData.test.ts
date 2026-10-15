// The Feature grid's model (mu-143): rows and cells, the per-cell cap, each card's one status and
// its trouble chip, the column heads' who line, the header line, the Your check limit's alert, the
// Layout setting per project, and the arrow keys between cells.
import { describe, expect, it } from "vitest";
import {
  activeCell,
  alertLine,
  cardStatus,
  CELL_CAP,
  cellShown,
  checkFull,
  checkHead,
  columnChips,
  dimmed,
  doneTodayCount,
  GRID_STAGES,
  gridRows,
  headerLine,
  layoutKey,
  loadLayout,
  moveCell,
  sentBackTimes,
  stageWho,
  storeLayout,
  troubleOf,
  viewSpan,
} from "./featureGridData";
import type { Note, QueueInfo, Task } from "./types";

const task = (over: Partial<Task>): Task => ({
  queue: "muster", id: "0001", prefix: "mu", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0,
  level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});
const note = (file: string, title = file): Note => ({ queue: "muster", file, title, text: title, truncated: false, added: 0, heldBy: "", heldSince: "", heartbeat: "" });
const feature = (id: string, title: string, done: number, total: number): Task =>
  task({ id, title, level: "feature", progress: { done, total, estMinutes: 0, estTokenCount: 0 } });

const TODAY = "2026-10-14";
// A Mac clock at 2026-10-14 12:00 local time, in seconds.
const NOW = new Date(2026, 9, 14, 12, 0).getTime() / 1000;

const orch = feature("0009", "Orchestration", 3, 11);
const plan = feature("0010", "Planning room", 0, 5);
const finished = feature("0011", "Old feature", 4, 4);
const empty = feature("0012", "Fresh feature", 0, 0);
const all = [
  orch, plan, finished, empty,
  task({ id: "0097", title: "Shaping as a claim", parent: "0009", stage: "building", claimedBy: "kaveh-2 (main · session 1234abcd · cc-mg)", live: true }),
  task({ id: "0098", title: "Sessions view", parent: "0009", stage: "ready", blockedKind: "tasks", blockedBy: [{ id: "0099", stage: "building", holder: "kaveh-3" }] }),
  task({ id: "0090", title: "Handoff keeps the owner", parent: "0009", stage: "your-check", heldBy: "owner", waitingForOwner: true }),
  task({ id: "0091", title: "Done today", parent: "0009", stage: "done", doneAt: `${TODAY} 09:12` }),
  task({ id: "0092", title: "Done last week", parent: "0009", stage: "done", doneAt: "2026-10-07 09:12" }),
  task({ id: "0041", title: "Planning room with roles", parent: "0010", stage: "ready" }),
  task({ id: "0080", title: "Old done", parent: "0011", stage: "done", doneAt: "2026-10-01 10:00" }),
  task({ id: "0094", title: "CLI fixes", stage: "building", claimedBy: "kaveh-1" }),
  task({ id: "0050", title: "Paused one", stage: "building", claimedBy: "kaveh-5", paused: "2026-10-14 10:00" }),
];
const work = all.filter((t) => t.level === "task");

describe("rows and cells", () => {
  it("has a row per feature in flight, an empty feature too, then Other; a finished feature has none", () => {
    const rows = gridRows({ work, all, notes: [], by: "feature", today: TODAY });
    expect(rows.map((r) => r.title)).toEqual(["Orchestration", "Planning room", "Fresh feature", "Other"]);
  });
  it("puts each task in its stage's cell, Done as a count of today's", () => {
    const [o, p, , other] = gridRows({ work, all, notes: [], by: "feature", today: TODAY });
    const ids = (r: typeof o) => Object.fromEntries(GRID_STAGES.map((s) => [s.id, r.cells[s.id].map((i) => (i.kind === "task" ? i.task.id : i.note.file))]));
    expect(ids(o)).toEqual({ backlog: [], shaping: [], ready: ["0098"], building: ["0097"], testing: [], "your-check": ["0090"] });
    expect(o.doneToday).toBe(1);
    expect(ids(p)).toEqual({ backlog: [], shaping: [], ready: ["0041"], building: [], testing: [], "your-check": [] });
    // A paused task stays in its stage's cell, its status saying so.
    expect(ids(other).building).toEqual(["0094", "0050"]);
  });
  it("takes a feature's progress from the core, and Other's from its own tasks", () => {
    const rows = gridRows({ work, all, notes: [], by: "feature", today: TODAY });
    expect(rows.map((r) => r.progress)).toEqual([{ done: 3, total: 11 }, { done: 0, total: 5 }, { done: 0, total: 0 }, { done: 0, total: 2 }]);
  });
  it("draws an empty feature's row with every cell empty and nothing done today", () => {
    const fresh = gridRows({ work, all, notes: [], by: "feature", today: TODAY }).find((r) => r.title === "Fresh feature");
    expect(fresh && GRID_STAGES.every((s) => fresh.cells[s.id].length === 0)).toBe(true);
    expect(fresh?.doneToday).toBe(0);
  });
  it("puts the notes in Other's Backlog first, and makes an Other row for them alone", () => {
    const rows = gridRows({ work: [], all: [], notes: [note("a.md"), note("b.md")], by: "feature", today: TODAY });
    expect(rows.map((r) => r.title)).toEqual(["Other"]);
    expect(rows[0].cells.backlog.map((i) => i.kind)).toEqual(["note", "note"]);
    const mixed = gridRows({ work: [task({ id: "0003", stage: "backlog" })], all: [], notes: [note("a.md")], by: "feature", today: TODAY });
    expect(mixed[0].cells.backlog.map((i) => i.kind)).toEqual(["note", "task"]);
  });
  it("with No grouping, is one row of every task", () => {
    const rows = gridRows({ work, all, notes: [note("a.md")], by: "none", today: TODAY });
    expect(rows.map((r) => r.title)).toEqual(["All tasks"]);
    expect(rows[0].cells.building.map((i) => (i.kind === "task" ? i.task.id : ""))).toEqual(["0094", "0097", "0050"]);
    expect(rows[0].cells.backlog).toHaveLength(1);
    expect(rows[0].doneToday).toBe(1);
  });
  it("by goal, has a row per goal and Other", () => {
    const goal = task({ id: "0001", title: "Ship it", level: "goal", progress: { done: 1, total: 2, estMinutes: 0, estTokenCount: 0 } });
    const f = task({ id: "0002", level: "feature", parent: "0001", progress: { done: 0, total: 1, estMinutes: 0, estTokenCount: 0 } });
    const items = [goal, f, task({ id: "0003", parent: "0002", stage: "testing", heldBy: "kia-1" }), task({ id: "0004" })];
    const rows = gridRows({ work: items.filter((t) => t.level === "task"), all: items, notes: [], by: "goal", today: TODAY });
    expect(rows.map((r) => [r.title, r.cells.testing.length, r.cells.ready.length])).toEqual([["Ship it", 1, 0], ["Other", 0, 1]]);
  });
  it("keeps the row of a feature the search matches, though none of its tasks do", () => {
    const rows = gridRows({ work: [], all, notes: [], by: "feature", today: TODAY, shown: [orch] });
    expect(rows.map((r) => r.title)).toEqual(["Orchestration", "Fresh feature"]);
    expect(gridRows({ work: [], all, notes: [], by: "feature", today: TODAY, shown: [] }).map((r) => r.title)).toEqual(["Fresh feature"]);
  });
  it("keeps a finished feature's row the day its last task is done, and drops it the day after (review R5)", () => {
    const shipped = feature("0013", "Shipped today", 1, 1);
    const last = task({ id: "0130", title: "Last one", parent: "0013", stage: "done", doneAt: `${TODAY} 08:00` });
    const rows = gridRows({ work: [last], all: [shipped, last], notes: [], by: "feature", today: TODAY });
    expect(rows.map((r) => [r.title, r.doneToday])).toEqual([["Shipped today", 1]]);
    expect(gridRows({ work: [last], all: [shipped, last], notes: [], by: "feature", today: "2026-10-15" })).toEqual([]);
  });
  it("is empty with nothing at all", () => {
    expect(gridRows({ work: [], all: [], notes: [], by: "feature", today: TODAY })).toEqual([]);
    expect(gridRows({ work: [], all: [], notes: [], by: "none", today: TODAY })).toEqual([]);
  });
  it("orders a cell by the board's sort", () => {
    const items = [task({ id: "0002", captured: "2026-10-01" }), task({ id: "0003", captured: "2026-10-09" })];
    const [row] = gridRows({ work: items, all: items, notes: [], by: "none", today: TODAY, sort: "newest" });
    expect(row.cells.ready.map((i) => (i.kind === "task" ? i.task.id : ""))).toEqual(["0003", "0002"]);
  });
});

describe("the per-cell cap", () => {
  const items = [1, 2, 3, 4, 5].map((n) => n);
  it("shows the first two, then +N more", () => {
    expect(CELL_CAP).toBe(2);
    expect(cellShown(items, false)).toEqual({ shown: [1, 2], more: 3 });
  });
  it("shows all of an expanded cell", () => {
    expect(cellShown(items, true)).toEqual({ shown: items, more: 0 });
  });
  it("has nothing more at or under the cap", () => {
    expect(cellShown([1, 2], false)).toEqual({ shown: [1, 2], more: 0 });
    expect(cellShown([], false)).toEqual({ shown: [], more: 0 });
  });
});

describe("a card's one status", () => {
  it("is who's on it, live or not", () => {
    expect(cardStatus(task({ stage: "building", claimedBy: "kaveh-2 (main · session 1234abcd · cc-mg)", live: true }), {})).toEqual({ kind: "who", text: "kaveh-2", live: true });
    expect(cardStatus(task({ stage: "testing", heldBy: "kia-1" }), {})).toEqual({ kind: "who", text: "kia-1", live: false });
  });
  it("is the dependency for a task waiting on others", () => {
    expect(cardStatus(task({ blockedKind: "tasks", blockedBy: [{ id: "0099", stage: "building", holder: "" }] }), {}).kind).toBe("blocked");
  });
  it("is needs you on Your check and the owner's own task", () => {
    expect(cardStatus(task({ stage: "your-check", heldBy: "owner", waitingForOwner: true }), {})).toEqual({ kind: "you", text: "needs you" });
    expect(cardStatus(task({ stage: "ready", forWho: "human" }), {})).toEqual({ kind: "you", text: "needs you" });
  });
  it("is blocked before needs you, so why it waits shows", () => {
    expect(cardStatus(task({ stage: "building", blockedKind: "you", blocked: true, waitingForOwner: true }), {}).kind).toBe("blocked");
  });
  it("is paused for a paused task", () => {
    expect(cardStatus(task({ stage: "building", claimedBy: "kaveh-5", paused: "2026-10-14 10:00" }), {})).toEqual({ kind: "paused", text: "paused" });
  });
  it("is nothing for a task nobody is on", () => {
    expect(cardStatus(task({ stage: "ready" }), {})).toEqual({ kind: "none", text: "" });
  });
  it("says you for the owner's claim", () => {
    expect(cardStatus(task({ stage: "testing", heldBy: "owner (main · session 1234abcd · cc-mg)" }), {})).toEqual({ kind: "who", text: "you", live: false });
  });
  it("names a role slot by its role's name now (0112)", () => {
    const t = task({ stage: "building", claimedBy: "coder-2 (main · session 7b5e8856 · slot coder-2 · cc-hg)" });
    expect(cardStatus(t, { coder: "Kaveh" })).toEqual({ kind: "who", text: "Kaveh-2", live: false });
    expect(stageWho("building", [{ kind: "task", task: t }], { coder: "Kaveh" })).toBe("Kaveh-2");
  });
});

describe("the trouble chip", () => {
  const back = (n: number) => Array.from({ length: n }, (_, i) => [
    { at: `2026-10-1${i} 10:00`, who: "kaveh-3", from: "building", to: "testing" },
    { at: `2026-10-1${i} 11:00`, who: "kia-1", from: "testing", to: "building" },
  ]).flat();
  it("counts the sends back to Building", () => {
    expect(sentBackTimes(task({ stageHistory: back(2) }))).toBe(2);
    expect(sentBackTimes(task({ stageHistory: [{ at: "", who: "", from: "your-check", to: "building" }] }))).toBe(1);
    expect(sentBackTimes(task({ stageHistory: [{ at: "", who: "", from: "ready", to: "building" }] }))).toBe(0);
  });
  it("says sent back once, twice or N times while the task is back", () => {
    const t = (n: number) => task({ stage: "building", claimedBy: "kaveh-3", sentBack: true, stageHistory: back(n), heartbeat: "2026-10-14 11:55" });
    expect(troubleOf(t(1), NOW)).toBe("sent back once");
    expect(troubleOf(t(2), NOW)).toBe("sent back twice");
    expect(troubleOf(t(3), NOW)).toBe("sent back 3 times");
    expect(troubleOf({ ...t(2), sentBack: false }, NOW)).toBe("");
  });
  it("says how long a claim has been quiet", () => {
    expect(troubleOf(task({ stage: "building", claimedBy: "kaveh-4", heartbeat: "2026-10-14 09:40" }), NOW)).toBe("no check-in for 2 h");
    expect(troubleOf(task({ stage: "building", claimedBy: "kaveh-4", heartbeat: "2026-10-14 11:50" }), NOW)).toBe("");
  });
  it("says needs a tester while a builder holds Testing", () => {
    expect(troubleOf(task({ stage: "testing", heldBy: "kaveh-1", builtBy: "kaveh-1" }), NOW)).toBe("needs a tester");
  });
  it("is nothing for a task going well", () => {
    expect(troubleOf(task({ stage: "ready" }), NOW)).toBe("");
  });
});

describe("the column heads", () => {
  it("names who works a stage, from its tasks' holders, else its role", () => {
    const items = [
      task({ stage: "building", claimedBy: "kaveh-2 (main · session 1234abcd · cc-mg)" }),
      task({ stage: "building", claimedBy: "kaveh-2 (other · session 2234abcd · cc-mg)" }),
      task({ stage: "building", claimedBy: "kaveh-3" }),
    ].map((t) => ({ kind: "task" as const, task: t }));
    expect(stageWho("building", items, {})).toBe("kaveh-2 and kaveh-3");
    expect(stageWho("building", [], {})).toBe("code agent");
    expect(stageWho("testing", [], {})).toBe("QA agent");
    expect(stageWho("shaping", [], {})).toBe("product + tech + UI agents");
  });
  it("names three at most, then how many more", () => {
    const items = ["a", "b", "c", "d", "e"].map((w) => ({ kind: "task" as const, task: task({ stage: "testing", heldBy: w }) }));
    expect(stageWho("testing", items, {})).toBe("a, b, c and 2 more");
  });
  it("names one, or exactly three with 'and' before the last (review R9)", () => {
    const held = (who: string[]) => who.map((w) => ({ kind: "task" as const, task: task({ stage: "testing", heldBy: w }) }));
    expect(stageWho("testing", held(["a"]), {})).toBe("a");
    expect(stageWho("testing", held(["a", "b", "c"]), {})).toBe("a, b and c");
  });
  it("says what Backlog and Ready hold", () => {
    expect(stageWho("backlog", [], {})).toBe("notes, not shaped yet");
    const ready = [task({}), task({ blockedKind: "tasks" }), task({})].map((t) => ({ kind: "task" as const, task: t }));
    expect(stageWho("ready", ready, {})).toBe("2 can start");
    expect(stageWho("ready", [], {})).toBe("none can start");
  });
});

describe("Your check's limit", () => {
  const q = (name: string, waiting: number, limit: number | null): QueueInfo => ({ name, path: "", counts: {}, yourCheck: waiting, yourCheckLimit: limit, label: name === "muster" ? "Muster" : name });
  it("is full at the limit and over it, not under it or off", () => {
    expect(checkFull([q("muster", 5, 5)], "muster")).toEqual([{ name: "muster", label: "Muster", waiting: 5, limit: 5 }]);
    expect(checkFull([q("muster", 7, 5)], "muster")).toHaveLength(1);
    expect(checkFull([q("muster", 4, 5)], "muster")).toEqual([]);
    expect(checkFull([q("muster", 9, null)], "muster")).toEqual([]);
  });
  it("heads the column with the count against the limit in one project", () => {
    expect(checkHead([q("muster", 5, 5)], "muster")).toEqual({ text: "5 of 5, full", full: true });
    expect(checkHead([q("muster", 3, 5)], "muster")).toEqual({ text: "3 of 5", full: false });
    expect(checkHead([q("muster", 3, null)], "muster")).toEqual({ text: "yours to check", full: false });
  });
  it("heads it with how many projects are full in All projects", () => {
    expect(checkHead([q("muster", 5, 5), q("drover", 9, 8), q("chit", 0, 5)], "all")).toEqual({ text: "full in 2 projects", full: true });
    expect(checkHead([q("muster", 5, 5), q("chit", 0, 5)], "all")).toEqual({ text: "full in 1 project", full: true });
    expect(checkHead([q("chit", 0, 5)], "all")).toEqual({ text: "yours to check", full: false });
  });
  it("says so in one alert line, plainly, and nothing within the limit", () => {
    expect(alertLine(checkFull([q("muster", 5, 5)], "muster"), "muster")).toBe("Your check is full (5 of 5): check some before more start.");
    expect(alertLine(checkFull([q("muster", 7, 5)], "muster"), "muster")).toBe("Your check is over its limit (7 of 5): check some before more start.");
    expect(alertLine(checkFull([q("muster", 5, 5), q("drover", 9, 8)], "all"), "all")).toBe("Your check is full in Muster (5 of 5) and drover (9 of 8): check some before more start.");
    expect(alertLine([], "muster")).toBe("");
  });
});

describe("the header line", () => {
  it("says what waits for you, what's being built, in testing and done today", () => {
    const items = [
      task({ stage: "building" }), task({ stage: "building" }), task({ stage: "building", paused: "2026-10-14 10:00" }),
      task({ stage: "testing" }),
      task({ stage: "done", doneAt: `${TODAY} 08:00` }), task({ stage: "done", doneOn: TODAY }), task({ stage: "done", doneAt: "2026-10-13 23:59" }),
    ];
    expect(headerLine(items, 5, TODAY)).toEqual({ needs: "5 waiting for you", rest: "2 being built, 1 in testing, 2 done today" });
  });
  it("counts done today from the stamp, or the day the log knows", () => {
    expect(doneTodayCount([task({ stage: "done", doneAt: `${TODAY} 08:00` }), task({ stage: "done", doneOn: TODAY }), task({ stage: "ready", doneOn: TODAY })], TODAY)).toBe(2);
  });
  it("says none for zero, whole", () => {
    expect(headerLine([], 0, TODAY)).toEqual({ needs: "nothing waiting for you", rest: "none being built, none in testing, none done today" });
  });
});

describe("the Layout setting", () => {
  const store = () => {
    const m = new Map<string, string>();
    return { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v), m };
  };
  it("is Columns by default, for a project without one or a broken value", () => {
    const s = store();
    expect(loadLayout(s, "muster")).toBe("columns");
    s.setItem(layoutKey("muster"), "sideways");
    expect(loadLayout(s, "muster")).toBe("columns");
  });
  it("is kept per project", () => {
    const s = store();
    storeLayout(s, "muster", "grid");
    expect(loadLayout(s, "muster")).toBe("grid");
    expect(loadLayout(s, "drover")).toBe("columns");
    expect(loadLayout(s, "all")).toBe("columns");
    expect(s.m.get("muster.layout.muster")).toBe("grid");
  });
  it("keeps working when the storage refuses", () => {
    const broken = { getItem: () => { throw new Error("denied"); }, setItem: () => { throw new Error("full"); } };
    expect(loadLayout(broken, "muster")).toBe("columns");
    expect(storeLayout(broken, "muster", "grid")).toBe(false);
  });
});

describe("arrow keys between cells", () => {
  const size = { rows: 3, cols: 7 };
  it("moves one cell each way, and stops at the edges", () => {
    expect(moveCell({ r: 1, c: 1 }, "ArrowDown", size, false)).toEqual({ r: 2, c: 1 });
    expect(moveCell({ r: 1, c: 1 }, "ArrowUp", size, false)).toEqual({ r: 0, c: 1 });
    expect(moveCell({ r: 1, c: 1 }, "ArrowRight", size, false)).toEqual({ r: 1, c: 2 });
    expect(moveCell({ r: 1, c: 1 }, "ArrowLeft", size, false)).toEqual({ r: 1, c: 0 });
    expect(moveCell({ r: 2, c: 6 }, "ArrowDown", size, false)).toEqual({ r: 2, c: 6 });
    expect(moveCell({ r: 0, c: 0 }, "ArrowLeft", size, false)).toEqual({ r: 0, c: 0 });
  });
  it("mirrors left and right in right-to-left", () => {
    expect(moveCell({ r: 1, c: 1 }, "ArrowLeft", size, true)).toEqual({ r: 1, c: 2 });
    expect(moveCell({ r: 1, c: 1 }, "ArrowRight", size, true)).toEqual({ r: 1, c: 0 });
  });
  it("goes to the row's start and end with Home and End", () => {
    expect(moveCell({ r: 1, c: 3 }, "Home", size, false)).toEqual({ r: 1, c: 0 });
    expect(moveCell({ r: 1, c: 3 }, "End", size, false)).toEqual({ r: 1, c: 6 });
  });
  it("leaves other keys alone", () => {
    expect(moveCell({ r: 1, c: 1 }, "Enter", size, false)).toBeNull();
    expect(moveCell({ r: 1, c: 1 }, "a", size, false)).toBeNull();
  });
});

describe("Only what needs me (review R4)", () => {
  it("dims a task that doesn't wait on you and a note, never one that does", () => {
    const mine = task({ id: "0090", stage: "your-check", heldBy: "owner", waitingForOwner: true });
    const theirs = task({ id: "0097", stage: "building", claimedBy: "kaveh-2" });
    const n = { kind: "note" as const, note: note("idea.md") };
    expect(dimmed({ kind: "task", task: mine }, true)).toBe(false);
    expect(dimmed({ kind: "task", task: theirs }, true)).toBe(true);
    expect(dimmed(n, true)).toBe(true);
  });
  it("dims nothing while it's off", () => {
    expect(dimmed({ kind: "task", task: task({ stage: "building" }) }, false)).toBe(false);
    expect(dimmed({ kind: "note", note: note("idea.md") }, false)).toBe(false);
  });
});

describe("the columns past the edge (review R1)", () => {
  it("counts each column's items, what needs you and what's blocked; Done is today's count", () => {
    const rows = gridRows({ work, all, notes: [note("idea.md")], by: "feature", today: TODAY });
    expect(columnChips(rows)).toEqual([
      { id: "backlog", label: "Backlog", count: 1, needs: 0, blocked: 0 },
      { id: "shaping", label: "Shaping", count: 0, needs: 0, blocked: 0 },
      { id: "ready", label: "Ready", count: 2, needs: 0, blocked: 1 },
      { id: "building", label: "Building", count: 3, needs: 0, blocked: 0 },
      { id: "testing", label: "Testing", count: 0, needs: 0, blocked: 0 },
      { id: "your-check", label: "Your check", count: 1, needs: 1, blocked: 0 },
      { id: "done", label: "Done", count: 1, needs: 0, blocked: 0 },
    ]);
  });
  it("has every column, empty, with no rows", () => {
    expect(columnChips([]).map((c) => [c.id, c.count])).toEqual([...GRID_STAGES.map((s) => [s.id, 0]), ["done", 0]]);
  });
  it("leaves out what the feature names cover, at the start in either direction", () => {
    // Left to right: the names on the left, the view starts after them.
    expect(viewSpan({ left: 100, right: 900 }, { left: 100, right: 260 })).toEqual({ start: 260, end: 900 });
    // Right to left: the names on the right, the view ends before them.
    expect(viewSpan({ left: 100, right: 900 }, { left: 740, right: 900 })).toEqual({ start: 100, end: 740 });
  });
});

describe("the grid's one Tab stop (review R10)", () => {
  const rows = gridRows({ work, all, notes: [], by: "feature", today: TODAY });
  it("follows its feature when the rows above it change", () => {
    expect(activeCell(rows, { row: "muster/0010", c: 3 })).toEqual({ r: 1, c: 3 });
    expect(activeCell(rows.slice(1), { row: "muster/0010", c: 3 })).toEqual({ r: 0, c: 3 });
  });
  it("goes back to the first row when its feature is gone, or before any is picked", () => {
    expect(activeCell(rows, { row: "muster/0999", c: 2 })).toEqual({ r: 0, c: 2 });
    expect(activeCell(rows, { row: "", c: 0 })).toEqual({ r: 0, c: 0 });
    expect(activeCell([], { row: "muster/0010", c: 4 })).toEqual({ r: 0, c: 4 });
  });
});
