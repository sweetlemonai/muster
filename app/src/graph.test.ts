import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { downstream, edgeLit, layoutGraph, readyToStart, upstream } from "./graph";
import { GraphView } from "./GraphView";
import type { Task } from "./types";

const task = (id: string, depends: string[] = [], over: Partial<Task> = {}): Task => ({
  queue: "muster", id, title: `Task ${id}`, kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: `todo/${id}-t.md`, claimedBy: "", heartbeat: "", waitingOn: "", depends, commit: "", captured: "", capturedAt: "", blockHistory: [], questions: [],
  openQuestions: 0, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTokens: "", estMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});
// Muster's own chain (0019's AC2): 0014 → 0015, 0016, 0017; 0016 → 0018; 0002 → 0005 → 0004.
const MUSTER = [
  task("0014", [], { stage: "your-check" }),
  task("0015", ["0014"]), task("0016", ["0014"]), task("0017", ["0014"]),
  task("0018", ["0014", "0016"]),
  task("0002", [], { stage: "building", claimedBy: "wt-a" }), task("0005", ["0014", "0002"]), task("0004", ["0005"]),
  task("0021"), task("0026", [], { stage: "done" }),
];
const rankOf = (g: ReturnType<typeof layoutGraph>) => Object.fromEntries(g.nodes.map((n) => [n.id, n.rank]));

describe("layoutGraph", () => {
  it("ranks each task after everything it waits on, longest chain first", () => {
    const g = layoutGraph(MUSTER);
    expect(rankOf(g)).toEqual({ "0014": 0, "0002": 0, "0015": 1, "0016": 1, "0017": 1, "0005": 1, "0018": 2, "0004": 2 });
    expect(g.ranks).toBe(3);
    expect(g.edges).toContainEqual({ from: "0016", to: "0018" });
    expect(g.edges).toHaveLength(8);
  });
  it("lists tasks with no link apart, and draws only links inside the set", () => {
    const g = layoutGraph(MUSTER);
    expect(g.loose.map((t) => t.id).sort()).toEqual(["0021", "0026"]);
    const partial = layoutGraph([task("0005", ["0014", "0002"]), task("0004", ["0005"])]);
    expect(partial.edges).toEqual([{ from: "0005", to: "0004" }]);
  });
  it("finishes on a loop and still places every task", () => {
    const g = layoutGraph([task("0001", ["0003"]), task("0002", ["0001"]), task("0003", ["0002"]), task("0004", ["0004"])]);
    expect(g.nodes.map((n) => n.id).sort()).toEqual(["0001", "0002", "0003"]);
    expect(g.loose.map((t) => t.id)).toEqual(["0004"]); // depending on itself isn't a link
  });
  it("orders a rank by where its dependencies sit, so arrows don't cross", () => {
    // 0001 and 0002 start; 0010 waits on 0002 and 0011 on 0001. Ids alone would cross them.
    const g = layoutGraph([task("0001"), task("0002"), task("0010", ["0002"]), task("0011", ["0001"])]);
    const pos = Object.fromEntries(g.nodes.map((n) => [n.id, n.order]));
    expect(Math.sign(pos["0001"] - pos["0002"])).toBe(Math.sign(pos["0011"] - pos["0010"]));
  });
  it("lays out 100 tasks in under 50 ms", () => {
    const many = Array.from({ length: 100 }, (_, i) => {
      const id = String(i + 1).padStart(4, "0");
      const deps = [i - 1, i - 7, i - 13].filter((d) => d >= 0).map((d) => String(d + 1).padStart(4, "0"));
      return task(id, deps);
    });
    layoutGraph(many); // warm up
    const t0 = performance.now();
    const g = layoutGraph(many);
    expect(performance.now() - t0).toBeLessThan(50);
    expect(g.nodes).toHaveLength(100);
  });
});

describe("chains and readiness", () => {
  const g = layoutGraph(MUSTER);
  it("upstream is everything a task waits on, downstream everything waiting on it", () => {
    expect([...upstream(g, "0018")].sort()).toEqual(["0014", "0016"]);
    expect([...downstream(g, "0014")].sort()).toEqual(["0004", "0005", "0015", "0016", "0017", "0018"]);
    expect(upstream(g, "0014").size).toBe(0);
  });
  it("lights only the arrows on the selected task's chain", () => {
    const up = upstream(g, "0005"), down = downstream(g, "0005");
    const lit = g.edges.filter((e) => edgeLit(e, "0005", up, down)).map((e) => `${e.from}>${e.to}`).sort();
    expect(lit).toEqual(["0002>0005", "0005>0004", "0014>0005"]);
    expect(g.edges.some((e) => edgeLit(e, null, up, down))).toBe(false);
  });
  it("ready to start: in Ready, nobody on it, not blocked, everything it waits on done", () => {
    const byId = new Map(MUSTER.map((t) => [t.id, t]));
    expect(readyToStart(byId.get("0021")!, byId)).toBe(true);
    expect(readyToStart(byId.get("0015")!, byId)).toBe(false); // 0014 is in Your check
    const done = new Map(byId).set("0014", task("0014", [], { stage: "done" }));
    expect(readyToStart(byId.get("0015")!, done)).toBe(true);
    expect(readyToStart(task("0030", [], { blocked: true }), byId)).toBe(false);
    expect(readyToStart(task("0030", [], { claimedBy: "wt-b" }), byId)).toBe(false);
    expect(readyToStart(byId.get("0002")!, byId)).toBe(false); // Building
  });
});

describe("GraphView", () => {
  const view = (openKey: string | null, tasks = MUSTER) =>
    renderToStaticMarkup(createElement(GraphView, { tasks, all: tasks, multi: false, openKey, onOpen: () => {} }));
  it("draws a node per linked task, an arrow per link, and the rest in the not-linked strip", () => {
    const html = view(null);
    expect(html.match(/class="g-node /g)).toHaveLength(8);
    expect(html.match(/class="g-edge/g)).toHaveLength(8);
    expect(html).toContain("Not linked to any task (2)");
    expect(html).toContain('aria-pressed="false">Hide done</button>');
  });
  it("marks what's ready to start", () => {
    const ready = layoutGraph([task("0001"), task("0002", ["0001"])]);
    expect(ready.nodes).toHaveLength(2);
    const html = view(null, [task("0001"), task("0002", ["0001"])]);
    expect(html).toMatch(/class="g-node s-ready ready"[^>]*>.*?0001/);
    expect(html).not.toMatch(/class="g-node s-ready ready"[^>]*>.*?<span class="id">0002/);
  });
  it("lights the selected task's chain and dims the rest", () => {
    const html = view("muster/0005");
    const cls = (id: string) => new RegExp(`class="g-node [^"]*"[^>]*><span class="g-top"><span class="id">${id}<`).exec(html)?.[0] ?? "";
    expect(cls("0005")).toContain(" sel");
    expect(cls("0014")).toContain(" up");
    expect(cls("0002")).toContain(" up");
    expect(cls("0004")).toContain(" down");
    expect(cls("0017")).toContain(" dim");
    expect(html.match(/class="g-edge on"/g)).toHaveLength(3);
  });
  it("keeps a task whose dependency is filtered out in the graph, and says what it waits on", () => {
    const shown = [task("0005", ["0014", "0002"], { priority: "high" }), task("0004", ["0005"])];
    const html = renderToStaticMarkup(createElement(GraphView, { tasks: shown, all: MUSTER, multi: false, openKey: null, onOpen: () => {} }));
    expect(html).toContain("waits on 0014 (not shown), 0002 (not shown) · 1 waits on it");
    expect(html).not.toContain("Not linked to any task");
    expect(layoutGraph([task("0005", ["0002"])]).loose).toHaveLength(0);
  });
  it("says in words what each task waits on and how many wait on it", () => {
    const html = view(null);
    expect(html).toContain('<span class="g-deps">waits on 0014, 0016</span>');
    expect(html).toContain('<span class="g-deps">5 wait on it</span>');
    expect(html).toContain('<span class="g-deps">waits on 0014, 0002 · 1 waits on it</span>');
  });
  it("tags what the selected task waits on and what waits on it, in words", () => {
    const html = view("muster/0005");
    expect(html).toMatch(/<span class="id">0014<\/span><span class="g-rel">needed first<\/span>/);
    expect(html).toMatch(/<span class="id">0004<\/span><span class="g-rel">waits for it<\/span>/);
    expect(html).not.toMatch(/<span class="id">0017<\/span><span class="g-rel">/);
  });
  it("shows each unlinked task's state on its chip: stage, ready, blocked, who holds it", () => {
    const loose = [task("0021"), task("0022", [], { blocked: true }), task("0023", [], { stage: "building", claimedBy: "wt-b" }), task("0024", [], { stage: "your-check", waitingForOwner: true })];
    const html = view(null, loose);
    const chip = (id: string) => new RegExp(`<button[^>]*g-loose[^>]*><span class="id">${id}</span>.*?</button>`).exec(html)?.[0] ?? "";
    expect(chip("0021")).toContain('<span class="g-ready">ready</span>');
    expect(chip("0022")).toContain('<span class="badge blocked">blocked</span>');
    expect(chip("0022")).not.toContain("g-ready");
    expect(chip("0023")).toContain('<span class="g-stage">Building</span>');
    expect(chip("0023")).toContain('<span class="g-who">wt-b</span>');
    expect(chip("0024")).toContain('<span class="badge warn">needs you</span>');
  });
  it("dims done tasks, but not when they're on the selected chain", () => {
    const tasks = [task("0001", [], { stage: "done" }), task("0002", ["0001"])];
    expect(view(null, tasks)).toMatch(/class="g-node s-done"[^>]*><span class="g-top"><span class="id">0001/);
    expect(view("muster/0002", tasks)).toMatch(/class="g-node s-done up"/);
  });
  it("doesn't claim nothing depends on anything when the links are only hidden", () => {
    const tasks = [task("0002", ["0001"])];
    const all = [task("0001", [], { stage: "done" }), ...tasks];
    // 0002 still shows as a node (it depends on something), so the graph isn't empty; with only
    // unlinked tasks shown, the message says why.
    const html = renderToStaticMarkup(createElement(GraphView, { tasks: [task("0009")], all, multi: false, openKey: null, onOpen: () => {} }));
    expect(html).toContain("Nothing linked among the tasks shown");
    expect(html).not.toContain("No task here depends on another yet");
  });
  it("says how to start when nothing is linked", () => {
    expect(view(null, [task("0001"), task("0002")])).toContain("No task here depends on another yet.");
  });
});
