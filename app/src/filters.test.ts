import { describe, expect, it } from "vitest";
import {
  activeItems, applyQuery, dayContext, deleteView, EMPTY_QUERY, GROUPS, groupValues, hiddenLine, isActive, loadViews, MAX_VIEWS, removeFilter, sameQuery, saveView, storeViews,
  notesFor, toggleFilter, viewsKey, type Query, type SavedView,
} from "./filters";
import type { Note, Task } from "./types";

const t = (over: Partial<Task>): Task => ({
  queue: "q", id: "0001", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [], folder: "todo", path: "", claimedBy: "", heldBy: "", stageSince: "", checkins: [],
  heartbeat: "", waitingOn: "", depends: [], commit: "", captured: "", capturedAt: "", blockHistory: [], questions: [], openQuestions: 0, ownerCriteria: [], waitingForOwner: false,
  tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", search: "", ...over,
});
const ids = (list: Task[]) => list.map((x) => x.id);
// Wednesday 2026-10-14, noon local time.
const NOW = new Date(2026, 9, 14, 12, 0).getTime() / 1000;
const ctx = dayContext(NOW);
const q = (over: Partial<Query>): Query => ({ ...EMPTY_QUERY, ...over });

const tasks = [
  t({ id: "0001", stage: "building", kind: "bug", priority: "high", tags: ["design", "ui"], claimedBy: "drover/main (session 1)", captured: "2026-10-14", search: "board flickers" }),
  t({ id: "0002", stage: "ready", kind: "task", priority: "low", tags: ["docs"], blocked: true, waitingOn: "0001", captured: "2026-10-12", search: "write the guide" }),
  t({ id: "0003", stage: "your-check", kind: "idea", priority: "P0", waitingForOwner: true, openQuestions: 2, captured: "2026-09-30", search: "export csv" }),
  t({ id: "0004", stage: "done", kind: "bug", tags: ["design"], doneOn: "2026-10-13", captured: "2026-10-01", modified: NOW - 3600, search: "fix the crash" }),
];

describe("dayContext", () => {
  it("is today, the Monday this week started and the first of the month, local time", () => {
    expect(ctx).toEqual({ today: "2026-10-14", week: "2026-10-12", month: "2026-10-01" });
    expect(dayContext(new Date(2026, 9, 18, 23, 0).getTime() / 1000).week).toBe("2026-10-12");
    expect(dayContext(new Date(2026, 9, 12, 0, 5).getTime() / 1000).week).toBe("2026-10-12");
  });
});

describe("filter groups", () => {
  const values = (id: string, list = tasks, selected: string[] = []) => groupValues(GROUPS.find((g) => g.id === id)!, list, selected, ctx);
  it("come in a fixed order", () => {
    expect(GROUPS.map((g) => g.id)).toEqual(["stage", "type", "priority", "tag", "holder", "state", "captured", "changed", "done"]);
  });
  it("show only the values that exist, in their natural order", () => {
    expect(values("stage")).toEqual(["ready", "building", "your-check", "done"]);
    expect(values("type")).toEqual(["bug", "idea", "task"]);
    expect(values("priority")).toEqual(["high", "normal", "low"]);
    expect(values("priority", [tasks[1]])).toEqual(["low"]);
    expect(values("tag")).toEqual(["design", "docs", "ui"]);
    // Your check is the owner's when `held-by` names nobody (lib.ts `holder`, 0030).
    expect(values("holder")).toEqual(["drover/main", "owner"]);
    expect(values("state")).toEqual(["blocked", "needs-you", "questions"]);
    expect(values("captured")).toEqual(["today", "week", "month"]);
    expect(values("changed")).toEqual(["today", "week", "month"]);
    expect(values("done")).toEqual(["week", "month"]);
  });
  it("show nothing for an empty project", () => {
    for (const g of GROUPS) expect(groupValues(g, [], [], ctx)).toEqual([]);
  });
  it("keep a selected value that no longer exists, so it can be cleared", () => {
    expect(values("tag", tasks, ["gone"])).toEqual(["design", "docs", "ui", "gone"]);
  });
});

describe("applyQuery", () => {
  const run = (over: Partial<Query>, list = tasks) => ids(applyQuery(list, q(over), ctx));
  it("keeps everything with no search and no filters", () => {
    expect(run({})).toEqual(["0001", "0002", "0003", "0004"]);
  });
  it("any value within a group, every group across", () => {
    expect(run({ filters: { type: ["bug", "idea"] } })).toEqual(["0001", "0003", "0004"]);
    expect(run({ filters: { type: ["bug"], tag: ["design"] } })).toEqual(["0001", "0004"]);
    expect(run({ filters: { type: ["bug"], stage: ["done"] } })).toEqual(["0004"]);
    expect(run({ filters: { type: ["bug"], stage: ["ready"] } })).toEqual([]);
  });
  it("search and filters together", () => {
    expect(run({ search: "the", filters: { type: ["bug"] } })).toEqual(["0004"]);
    expect(run({ search: "1", filters: { stage: ["building"] } })).toEqual(["0001"]);
  });
  it("reads priority the way the board does (P0 is high)", () => {
    expect(run({ filters: { priority: ["high"] } })).toEqual(["0001", "0003"]);
  });
  it("the state flags: blocked, needs you, open questions", () => {
    expect(run({ filters: { state: ["blocked"] } })).toEqual(["0002"]);
    expect(run({ filters: { state: ["needs-you"] } })).toEqual(["0003"]);
    expect(run({ filters: { state: ["questions"] } })).toEqual(["0003"]);
    expect(run({ filters: { state: ["blocked", "needs-you"] } })).toEqual(["0002", "0003"]);
  });
  it("dates: today, this week, this month", () => {
    expect(run({ filters: { captured: ["today"] } })).toEqual(["0001"]);
    expect(run({ filters: { captured: ["week"] } })).toEqual(["0001", "0002"]);
    expect(run({ filters: { captured: ["month"] } })).toEqual(["0001", "0002", "0004"]);
    expect(run({ filters: { done: ["week"] } })).toEqual(["0004"]);
    expect(run({ filters: { changed: ["today"] } })).toEqual(["0004"]);
  });
  it("the holder", () => {
    expect(run({ filters: { holder: ["drover/main"] } })).toEqual(["0001"]);
    expect(run({ filters: { holder: ["owner"] } })).toEqual(["0003"]);
  });
  it("the holder of a session stamp (0030) is its label, in Building, Testing and Your check", () => {
    const stamped = [
      t({ id: "0011", stage: "building", claimedBy: "wt-a (wt-0030 · session 7b5e8856 · cc-hg)" }),
      t({ id: "0012", stage: "testing", heldBy: "wt-a (main-29 · session 1a2b3c4d)" }),
      t({ id: "0013", stage: "your-check", heldBy: "wt-b (wt-0031 · session 0badcafe · cc-mg)" }),
      // A claim left over on a stage that isn't one is nobody's.
      t({ id: "0014", stage: "done", claimedBy: "wt-a (wt-0030 · session 7b5e8856)" }),
    ];
    expect(groupValues(GROUPS.find((g) => g.id === "holder")!, stamped, [], ctx)).toEqual(["wt-a", "wt-b"]);
    expect(applyQuery(stamped, q({ filters: { holder: ["wt-a"] } }), ctx).map((x) => x.id)).toEqual(["0011", "0012"]);
    expect(activeItems(q({ filters: { holder: ["wt-a"] } }))[0].label).toBe("held by wt-a");
  });
  it("ignores a group it doesn't know (a view saved by a later version)", () => {
    expect(run({ filters: { level: ["epic"] } })).toEqual(["0001", "0002", "0003", "0004"]);
  });
  it("filters 1,000 tasks within a frame (16 ms)", () => {
    const words = "board column flicker resize window export import guide crash design review agent owner check".split(" ");
    const many = Array.from({ length: 1000 }, (_, i) =>
      t({
        id: String(i).padStart(4, "0"),
        stage: ["ready", "building", "testing", "done"][i % 4],
        kind: ["bug", "task", "idea"][i % 3],
        priority: ["high", "normal", "low"][i % 3],
        tags: [`tag${i % 7}`, "design"],
        claimedBy: i % 5 ? "" : `agent/${i % 3}`,
        captured: `2026-10-${String((i % 28) + 1).padStart(2, "0")}`,
        modified: NOW - i * 600,
        search: Array.from({ length: 300 }, (_, k) => `${words[(i + k) % words.length]}${k}`).join(" "),
      }),
    );
    const query = q({ search: "crash review", filters: { stage: ["ready", "building"], tag: ["design"], captured: ["month"], changed: ["month"] } });
    let best = Infinity;
    let found = 0;
    for (let run = 0; run < 7; run++) {
      const at = performance.now();
      found = applyQuery(many, query, ctx).length;
      best = Math.min(best, performance.now() - at);
    }
    expect(found).toBeGreaterThan(0);
    expect(best).toBeLessThan(16);
  });
});

describe("notesFor", () => {
  const note = (title: string, text = title): Note => ({ queue: "q", file: `${title}.md`, title, text, truncated: false, added: 0 });
  const notes = [note("Flicker on resize", "Flicker on resize\nthe columns jump"), note("Export CSV")];
  const files = (over: Partial<Query>) => notesFor(notes, q(over)).map((n) => n.file);
  it("keeps every note with nothing set", () => {
    expect(files({})).toEqual(["Flicker on resize.md", "Export CSV.md"]);
  });
  it("matches the search on the title and the text, accents and case aside", () => {
    expect(files({ search: "COLUMNS flicker" })).toEqual(["Flicker on resize.md"]);
    expect(files({ search: "export missing" })).toEqual([]);
  });
  it("a word of digits never finds a note (notes have no id)", () => {
    expect(notesFor([note("Release 19")], q({ search: "19" }))).toEqual([]);
  });
  it("only a Stage filter that includes Backlog keeps notes", () => {
    expect(files({ filters: { stage: ["backlog", "ready"] } })).toHaveLength(2);
    expect(files({ filters: { stage: ["ready"] } })).toEqual([]);
    expect(files({ filters: { tag: ["ui"] } })).toEqual([]);
    expect(files({ filters: { stage: ["backlog"], tag: ["ui"] } })).toEqual([]);
  });
});

describe("toggling and clearing", () => {
  it("toggles a value on and off, and drops an emptied group", () => {
    const one = toggleFilter({}, "tag", "ui");
    expect(one).toEqual({ tag: ["ui"] });
    expect(toggleFilter(one, "tag", "docs")).toEqual({ tag: ["ui", "docs"] });
    expect(toggleFilter(one, "tag", "ui")).toEqual({});
    expect(removeFilter({ tag: ["ui", "docs"], type: ["bug"] }, "tag", "ui")).toEqual({ tag: ["docs"], type: ["bug"] });
  });
  it("is active with a search or any filter", () => {
    expect(isActive(EMPTY_QUERY)).toBe(false);
    expect(isActive(q({ search: "  " }))).toBe(false);
    expect(isActive(q({ search: "x" }))).toBe(true);
    expect(isActive(q({ filters: { tag: ["ui"] } }))).toBe(true);
    expect(isActive(q({ sort: "newest" }))).toBe(false);
  });
});

describe("activeItems", () => {
  it("reads the active filters in the groups' order, as the chips name them", () => {
    const items = activeItems(q({ search: "flicker", filters: { state: ["blocked"], tag: ["design"], priority: ["high"], stage: ["building"], holder: ["drover/main"], captured: ["week"] } }));
    expect(items.map((i) => i.label)).toEqual(["“flicker”", "Building", "high", "#design", "held by drover/main", "blocked", "captured this week"]);
    expect(items[1]).toMatchObject({ group: "stage", value: "building" });
    expect(items[0]).toMatchObject({ group: "search" });
  });
  it("is empty with nothing set", () => {
    expect(activeItems(EMPTY_QUERY)).toEqual([]);
  });
});

describe("hiddenLine", () => {
  it("says how many filters hide, and nothing when nothing is active", () => {
    expect(hiddenLine(36, true)).toBe("36 hidden by filters");
    expect(hiddenLine(1, true)).toBe("1 hidden by filters");
    expect(hiddenLine(0, true)).toBe("nothing hidden");
    expect(hiddenLine(0, false)).toBe("");
  });
});

describe("saved views", () => {
  const store = () => {
    const m = new Map<string, string>();
    return { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v), m };
  };
  const mine: SavedView = { name: "My checks", search: "csv", filters: { stage: ["your-check"], tag: ["design"] }, sort: "newest" };
  it("round-trip through storage, per project", () => {
    const s = store();
    storeViews(s, "muster", saveView([], mine));
    expect(loadViews(s, "muster")).toEqual([mine]);
    expect(loadViews(s, "drover")).toEqual([]);
    expect([...s.m.keys()]).toEqual([viewsKey("muster")]);
    expect(viewsKey("all")).not.toBe(viewsKey("muster"));
  });
  it("saving under a name that exists replaces it, in place", () => {
    const two = saveView(saveView([], mine), { ...EMPTY_QUERY, name: "Blocked" });
    const again = saveView(two, { ...mine, name: "my checks", search: "pdf" });
    expect(again.map((v) => [v.name, v.search])).toEqual([["my checks", "pdf"], ["Blocked", ""]]);
  });
  it("deletes by name", () => {
    expect(deleteView([mine], "My checks")).toEqual([]);
  });
  it("trims the name and refuses an empty one", () => {
    expect(saveView([], { ...mine, name: "  Mine  " })[0].name).toBe("Mine");
    expect(saveView([mine], { ...mine, name: "   " })).toEqual([mine]);
  });
  it("keeps at most MAX_VIEWS, dropping the oldest", () => {
    let list: SavedView[] = [];
    for (let i = 0; i < MAX_VIEWS + 2; i++) list = saveView(list, { ...mine, name: `v${i}` });
    expect(list).toHaveLength(MAX_VIEWS);
    expect(list[0].name).toBe("v2");
  });
  it("reads what's broken as nothing, and drops what's malformed", () => {
    const s = store();
    s.setItem(viewsKey("p"), "{not json");
    expect(loadViews(s, "p")).toEqual([]);
    s.setItem(viewsKey("p"), JSON.stringify({ name: "x" }));
    expect(loadViews(s, "p")).toEqual([]);
    s.setItem(
      viewsKey("p"),
      JSON.stringify([
        { name: "ok", search: "a", filters: { tag: ["ui", 3], stage: "ready", type: ["bug"] }, sort: "sideways" },
        { name: 5 },
        null,
        { name: "x".repeat(200), search: "y".repeat(1000), filters: null, sort: "id" },
      ]),
    );
    const got = loadViews(s, "p");
    expect(got[0]).toEqual({ name: "ok", search: "a", filters: { tag: ["ui"], type: ["bug"] }, sort: "priority" });
    expect(got[1].name).toHaveLength(40);
    expect(got[1].search).toHaveLength(200);
    expect(got[1]).toMatchObject({ filters: {}, sort: "id" });
    expect(got).toHaveLength(2);
  });
  it("a storage that throws reads as no views", () => {
    const broken = { getItem: () => { throw new Error("denied"); }, setItem: () => { throw new Error("full"); } };
    expect(loadViews(broken, "p")).toEqual([]);
    expect(() => storeViews(broken, "p", [mine])).not.toThrow();
  });
  it("knows when the current query is a saved view", () => {
    expect(sameQuery(mine, { search: "csv", filters: { tag: ["design"], stage: ["your-check"] }, sort: "newest" })).toBe(true);
    expect(sameQuery(mine, { search: "csv", filters: { stage: ["your-check"], tag: ["design"] }, sort: "id" })).toBe(false);
    expect(sameQuery(mine, { search: "csv", filters: { stage: ["your-check"] }, sort: "newest" })).toBe(false);
    expect(sameQuery({ ...mine, filters: { tag: ["a", "b"] } }, { ...mine, filters: { tag: ["b", "a"] } })).toBe(true);
  });
});
