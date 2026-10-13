import { describe, expect, it } from "vitest";
import { byWho, FOR_FILTERS, humanColumn, isHuman, needsYou, needsYouCount, parseForFilter, waitingForYou, whoFiltersOn } from "./human";
import { columns } from "./lib";
import type { Question, Task } from "./types";

const task = (id: string, over: Partial<Task> = {}): Task => ({
  queue: "muster", id, title: `Task ${id}`, kind: "code", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: `todo/${id}-t.md`, claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});
const mine = (id: string, over: Partial<Task> = {}) => task(id, { forWho: "human", ...over });
const question = (n: number, text: string, over: Partial<Question> = {}): Question => ({ n, ticked: false, asker: "wt-a", text, escalated: true, ownerAnswered: false, ...over });
const ids = (ts: Task[]) => ts.map((t) => t.id);

// A queue with the owner's own tasks, agents' tasks that wait on the owner, and agents' work that doesn't.
const QUEUE = [
  mine("0041", { title: "Buy the domain" }),
  mine("0042", { stage: "building", claimedBy: "owner", priority: "high" }),
  mine("0043", { stage: "done", modified: 10 }),
  mine("0044", { stage: "done", modified: 20 }),
  mine("0045", { stage: "backlog" }),
  task("0050", { stage: "your-check", waitingForOwner: true }),
  task("0051", { stage: "building", claimedBy: "wt-a", questions: [question(2, "Child webview or a window?")], openQuestions: 1, waitingForOwner: true }),
  // As the core sends a block on you (0024): its kind, and the detail after "owner:".
  task("0052", { blocked: true, waitingOn: "owner: which colour?", blockedKind: "you", blockDetail: "which colour?", waitingForOwner: true }),
  task("0053", { stage: "building", claimedBy: "wt-b" }),
  // Waits on the owner's 0041 through depends: blocked by tasks, as the core derives it.
  task("0054", { depends: ["0041"], blockedKind: "tasks", blockedBy: [{ id: "0041", stage: "ready", holder: "" }] }),
];

describe("who does a task (0022)", () => {
  it("is the owner only for for: human; every older task is an agent's", () => {
    expect(isHuman(mine("0001"))).toBe(true);
    expect(isHuman(task("0001"))).toBe(false);
    expect(isHuman(task("0001", { forWho: "" }))).toBe(false);
  });
  it("puts the owner's own task in To do until Building, Doing until Done, then Done", () => {
    expect(["backlog", "shaping", "ready", "building", "testing", "your-check", "done", "nowhere"].map(humanColumn)).toEqual([
      "todo", "todo", "todo", "doing", "doing", "doing", "done", "todo",
    ]);
  });
});

describe("one board: the owner's own tasks among the agents' (0022, owner 2026-10-13)", () => {
  it("shows on the board with the agents' tasks, To do in Ready, Doing in Building, Done in Done", () => {
    const c = columns(QUEUE);
    // In the column's sort: what waits on the owner first, then priority, then id.
    expect(ids(c.ready)).toEqual(["0052", "0041", "0045", "0054"]);
    expect(ids(c.building)).toEqual(["0051", "0042", "0053"]);
    expect(ids(c["your-check"])).toEqual(["0050"]);
    expect(ids(c.done)).toEqual(["0044", "0043"]);
  });
  it("Who → You shows only those, Agents only the agents', All both", () => {
    const you = columns(byWho(QUEUE, "human"));
    expect(Object.values(you).flat().every(isHuman)).toBe(true);
    expect(ids(Object.values(you).flat()).sort()).toEqual(["0041", "0042", "0043", "0044", "0045"]);
    const agents = columns(byWho(QUEUE, "ai"));
    expect(Object.values(agents).flat().some(isHuman)).toBe(false);
    expect(ids(Object.values(agents).flat()).sort()).toEqual(["0050", "0051", "0052", "0053", "0054"]);
    expect(Object.values(columns(byWho(QUEUE, "all"))).flat()).toHaveLength(QUEUE.length);
  });
  it("has the Who chips on the Board, Table and Timeline", () => {
    expect(["board", "table", "timeline"].every(whoFiltersOn)).toBe(true);
    expect(["graph", "waiting", "activity", "problems", "projects", "help", "helper"].some(whoFiltersOn)).toBe(false);
  });
});

describe("the needs-you count: one definition everywhere", () => {
  it("is the owner's own tasks to do and doing, plus what the agents wait on them for", () => {
    expect(needsYouCount(QUEUE)).toBe(6);
    expect(needsYouCount([])).toBe(0);
    // Your check, an escalated question and a block on you count; the agents' other work, a block
    // by tasks (even the owner's) and the owner's done tasks don't.
    expect(QUEUE.filter(needsYou).map((t) => t.id)).toEqual(["0041", "0042", "0045", "0050", "0051", "0052"]);
  });
  it("counts a task once, and the owner's done tasks not at all", () => {
    expect(needsYou(mine("0001", { waitingForOwner: true }))).toBe(true);
    expect(needsYouCount([mine("0001", { waitingForOwner: true })])).toBe(1);
    expect(needsYou(mine("0001", { stage: "done" }))).toBe(false);
    expect(needsYou(task("0001"))).toBe(false);
    expect(needsYou(task("0001", { waitingForOwner: true }))).toBe(true);
    // A block by a permission waits on the owner too (the core sets waitingForOwner).
    expect(needsYou(task("0001", { blocked: true, blockedKind: "permission", waitingForOwner: true }))).toBe(true);
  });
});

describe("Waiting for you: the count's tasks, as a list", () => {
  it("lists what the agents wait on you for first, then your own To do, then Doing, each by attention", () => {
    expect(ids(waitingForYou(QUEUE))).toEqual(["0050", "0051", "0052", "0041", "0045", "0042"]);
    expect(waitingForYou(QUEUE)).toHaveLength(needsYouCount(QUEUE));
  });
  it("puts what waits on the owner first, then by priority, then by project and id", () => {
    const x = waitingForYou([mine("0003"), mine("0002", { priority: "low" }), mine("0001", { priority: "high" }), mine("0004", { questions: [question(1, "?")], waitingForOwner: true })]);
    expect(ids(x)).toEqual(["0004", "0001", "0003", "0002"]);
  });
  it("is empty for a queue with nothing waiting on the owner", () => {
    expect(waitingForYou([task("0001"), mine("0002", { stage: "done" })])).toEqual([]);
    expect(waitingForYou([])).toEqual([]);
  });
});

describe("the Who chips: All · You · Agents (Board, Table, Timeline)", () => {
  it("shows all three, All first", () => {
    expect(FOR_FILTERS.map((f) => f.label)).toEqual(["All", "You", "Agents"]);
    expect(FOR_FILTERS.map((f) => f.id)).toEqual(["all", "human", "ai"]);
  });
  it("keeps the owner's own tasks, the agents', or both", () => {
    expect(ids(byWho(QUEUE, "human"))).toEqual(["0041", "0042", "0043", "0044", "0045"]);
    expect(ids(byWho(QUEUE, "ai"))).toEqual(["0050", "0051", "0052", "0053", "0054"]);
    expect(byWho(QUEUE, "all")).toEqual(QUEUE);
  });
  it("reads a remembered choice, All for anything else", () => {
    expect(parseForFilter("human")).toBe("human");
    expect(parseForFilter("ai")).toBe("ai");
    expect(parseForFilter(null)).toBe("all");
    expect(parseForFilter("robots")).toBe("all");
  });
});
