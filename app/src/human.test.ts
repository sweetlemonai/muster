import { describe, expect, it } from "vitest";
import { byWho, FOR_FILTERS, humanBoard, humanColumn, isHuman, laneLines, needsYou, needsYouCount, parseForFilter, waitsOnYou } from "./human";
import { indexTasks } from "./levels";
import type { Question, Task } from "./types";

const task = (id: string, over: Partial<Task> = {}): Task => ({
  queue: "muster", id, title: `Task ${id}`, kind: "code", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: `todo/${id}-t.md`, claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});
const mine = (id: string, over: Partial<Task> = {}) => task(id, { forWho: "human", ...over });
const question = (n: number, text: string, over: Partial<Question> = {}): Question => ({ n, ticked: false, asker: "wt-a", text, escalated: true, ownerAnswered: false, ...over });
const ids = (ts: Task[]) => ts.map((t) => t.id);

// A queue with every kind of thing the Human board shows, and things it doesn't.
const QUEUE = [
  mine("0041", { title: "Buy the domain" }),
  mine("0042", { stage: "building", claimedBy: "owner", priority: "high" }),
  mine("0043", { stage: "done", modified: 10 }),
  mine("0044", { stage: "done", modified: 20 }),
  mine("0045", { stage: "backlog" }),
  task("0050", { stage: "your-check", waitingForOwner: true }),
  task("0051", { stage: "building", claimedBy: "wt-a", questions: [question(2, "Child webview or a window?")], openQuestions: 1, waitingForOwner: true }),
  task("0052", { blocked: true, waitingOn: "owner: which colour?", waitingForOwner: true }),
  task("0053", { stage: "building", claimedBy: "wt-b" }),
  task("0054", { blocked: true, waitingOn: "0041 the domain" }),
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

describe("the Human board", () => {
  const b = humanBoard(QUEUE);
  it("has the owner's own tasks in To do · Doing · Done, and only theirs", () => {
    expect(ids(b.todo)).toEqual(["0041", "0045"]);
    expect(ids(b.doing)).toEqual(["0042"]);
    // Done: newest first.
    expect(ids(b.done)).toEqual(["0044", "0043"]);
  });
  it("pulls into From the agents what an agent's task waits on the owner for, without moving it", () => {
    expect(ids(b.lane)).toEqual(["0050", "0051", "0052"]);
    expect(b.lane.map((t) => t.stage)).toEqual(["your-check", "building", "ready"]);
    // Agents' work that doesn't wait on the owner stays on the AI board only.
    expect(ids(b.lane)).not.toContain("0053");
    expect(ids(b.lane)).not.toContain("0054");
  });
  it("never lists the owner's own task in the lane, even when it waits on them", () => {
    const x = humanBoard([mine("0001", { blocked: true, waitingOn: "owner: later", waitingForOwner: true })]);
    expect(ids(x.lane)).toEqual([]);
    expect(ids(x.todo)).toEqual(["0001"]);
  });
  it("puts what waits on the owner first, then by priority, then by project and id", () => {
    const x = humanBoard([mine("0003"), mine("0002", { priority: "low" }), mine("0001", { priority: "high" }), mine("0004", { questions: [question(1, "?")], waitingForOwner: true })]);
    expect(ids(x.todo)).toEqual(["0004", "0001", "0003", "0002"]);
  });
  it("is empty, not missing, for a queue with nothing of the owner's", () => {
    expect(humanBoard([task("0001")])).toEqual({ lane: [], todo: [], doing: [], done: [] });
    expect(humanBoard([])).toEqual({ lane: [], todo: [], doing: [], done: [] });
  });
  it("says why each lane item waits on the owner, in words", () => {
    expect(laneLines(QUEUE[5])).toEqual(["In Your check: accept it, or send it back"]);
    expect(laneLines(QUEUE[6])).toEqual(["Q2: Child webview or a window?"]);
    expect(laneLines(QUEUE[7])).toEqual(["Blocked on you: owner: which colour?"]);
    // An answered or not-escalated question isn't the owner's; a long one shows its first line.
    const t = task("0001", { questions: [question(1, "done", { ticked: true }), question(2, "agents'", { escalated: false }), question(3, "First line\n\nmore")] });
    expect(laneLines(t)).toEqual(["Q3: First line"]);
  });
});

describe("the needs-you count: one definition everywhere", () => {
  it("is the Human board's To do + Doing + From the agents", () => {
    const b = humanBoard(QUEUE);
    expect(needsYouCount(QUEUE)).toBe(b.todo.length + b.doing.length + b.lane.length);
    expect(needsYouCount(QUEUE)).toBe(6);
    expect(needsYouCount([])).toBe(0);
  });
  it("counts a task once, and the owner's done tasks not at all", () => {
    expect(needsYou(mine("0001", { waitingForOwner: true }))).toBe(true);
    expect(needsYouCount([mine("0001", { waitingForOwner: true })])).toBe(1);
    expect(needsYou(mine("0001", { stage: "done" }))).toBe(false);
    expect(needsYou(task("0001"))).toBe(false);
    expect(needsYou(task("0001", { waitingForOwner: true }))).toBe(true);
  });
});

describe("hand-offs: an agent's task waits on the owner's", () => {
  it("is blocked by the owner's tasks it depends on until they're done, derived from depends", () => {
    const agent = task("0060", { depends: ["0041", "0053", "0099", "0060"] });
    const index = indexTasks([...QUEUE, agent]);
    // 0041 is the owner's and not done; 0053 is an agent's; 0099 doesn't exist; itself never counts.
    expect(waitsOnYou(agent, index)).toEqual(["0041"]);
    const done = indexTasks([...QUEUE.map((t) => (t.id === "0041" ? { ...t, stage: "done" } : t)), agent]);
    expect(waitsOnYou(agent, done)).toEqual([]);
  });
  it("only within its own project", () => {
    const agent = task("0060", { queue: "drover", depends: ["0041"] });
    expect(waitsOnYou(agent, indexTasks([...QUEUE, agent]))).toEqual([]);
  });
});

describe("the Human · AI · All filter (Table, Timeline)", () => {
  it("shows all three, Human first", () => {
    expect(FOR_FILTERS.map((f) => f.label)).toEqual(["Human", "AI", "All"]);
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
