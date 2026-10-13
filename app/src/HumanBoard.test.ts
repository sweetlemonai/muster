import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import HumanBoard from "./HumanBoard";
import NewTask from "./NewTask";
import type { Question, Task } from "./types";

const task = (id: string, over: Partial<Task> = {}): Task => ({
  queue: "muster", id, title: `Task ${id}`, kind: "code", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: `todo/${id}-t.md`, claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});
const q = (n: number, text: string): Question => ({ n, ticked: false, asker: "wt-a", text, escalated: true, ownerAnswered: false });
const board = (tasks: Task[], openKey: string | null = null) => renderToStaticMarkup(createElement(HumanBoard, { tasks, multi: false, openKey, onOpen: () => {} }));
const headings = (html: string) => [...html.matchAll(/<h3>([^<]*) <small>(\d+)<\/small><\/h3>/g)].map((m) => `${m[1]} ${m[2]}`);
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/\s+/g, " ").trim();

describe("the Human board (0022)", () => {
  const tasks = [
    task("0041", { forWho: "human", title: "Buy the domain" }),
    task("0042", { forWho: "human", stage: "building", claimedBy: "owner", priority: "high", title: "Set the secret" }),
    task("0043", { forWho: "human", stage: "done", title: "Sign up" }),
    task("0050", { stage: "your-check", waitingForOwner: true, title: "Try the header" }),
    task("0051", { stage: "building", questions: [q(2, "Child webview or a window?")], waitingForOwner: true, title: "Pane" }),
    // As the core sends a block on you (0024): its kind, and the detail after "owner:".
    task("0052", { blocked: true, waitingOn: "owner: which colour?", blockedKind: "you", blockDetail: "which colour?", waitingForOwner: true, title: "Theme" }),
    task("0053", { stage: "building", claimedBy: "wt-b", title: "Agent work" }),
  ];
  it("shows From the agents first, then To do · Doing · Done, each with its count", () => {
    expect(headings(board(tasks))).toEqual(["From the agents 3", "To do 1", "Doing 1", "Done 1"]);
    expect(board(tasks)).toContain('aria-label="Human board"');
    // The count is the needs-you number: To do + Doing + From the agents.
    expect(text(board(tasks))).toContain("Human board 5 need you");
  });
  it("links each lane item to its task, saying why it waits on you and the stage it stays in", () => {
    const html = board(tasks, "muster/0051");
    const lane = /aria-label="From the agents">([\s\S]*?)<\/div>/.exec(html)?.[1] ?? "";
    const items = [...lane.matchAll(/<button type="button" class="hb-item lane-item s-([\w-]+)( active)?">([\s\S]*?)<\/button>/g)];
    expect(items.map((m) => `${m[1]}${m[2] ?? ""}`)).toEqual(["your-check", "building active", "ready"]);
    expect(items.map((m) => text(m[3]))).toEqual([
      "0050 Try the header Your check In Your check: accept it, or send it back",
      "0051 Pane Building Q2: Child webview or a window?",
      "0052 Theme Ready Blocked on you: which colour?",
    ]);
    // Agents' work that doesn't wait on you isn't on the Human board at all.
    expect(html).not.toContain("Agent work");
  });
  it("says why the owner's own task is blocked, in the block's words (0024)", () => {
    const own = (over: Partial<Task>) => task("0060", { forWho: "human", title: "Renew the cert", ...over });
    const why = (t: Task) => [...board([t]).matchAll(/<span class="hb-why">([^<]*)<\/span>/g)].map((m) => m[1]);
    expect(why(own({ blocked: true, blockedKind: "outside", blockDetail: "the registrar's reply" }))).toEqual(["Blocked by outside: the registrar&#x27;s reply"]);
    expect(why(own({ blockedKind: "tasks", blockedBy: [{ id: "0014", stage: "building", holder: "wt-a" }] }))).toEqual(["Blocked by tasks: 0014"]);
    expect(why(own({}))).toEqual([]);
  });
  it("collapses to one line per column when there's nothing, but never hides", () => {
    const html = board([]);
    expect(headings(html)).toEqual(["From the agents 0", "To do 0", "Doing 0", "Done 0"]);
    expect([...html.matchAll(/<p class="none">([^<]*)<\/p>/g)].map((m) => m[1])).toEqual(["Nothing waits on you", "Nothing to do", "Nothing started", "Nothing done yet"]);
    expect(text(html)).toContain("nothing needs you");
  });
  it("shows the newest few done, and the rest one click away", () => {
    const done = Array.from({ length: 7 }, (_, i) => task(`00${10 + i}`, { forWho: "human", stage: "done", modified: i }));
    const html = board(done);
    expect(html.match(/class="hb-item s-done"/g)).toHaveLength(5);
    expect(html).toContain(">Show all 7</button>");
  });
});

describe("New task's Human / AI chip pair (0022)", () => {
  it("is a chip pair, AI pressed until you pick Human", () => {
    const html = renderToStaticMarkup(createElement(NewTask, { queues: ["muster"], initial: "muster", onClose: () => {}, onCreated: () => {} }));
    const group = /role="group" aria-label="Who does it">([\s\S]*?)<\/div>/.exec(html)?.[1] ?? "";
    expect([...group.matchAll(/aria-pressed="(\w+)"[^>]*>(\w+)<\/button>/g)].map((m) => `${m[2]} ${m[1]}`)).toEqual(["Human false", "AI true"]);
    expect(html).not.toContain("<select aria-label=\"Who");
  });
});
