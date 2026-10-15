import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { downstream, edgeLit, graphMarks, isLoopEdge, layoutGraph, readyToStart, upstream } from "./graph";
import { GraphView } from "./GraphView";
import css from "./styles.css?raw";
import type { Task } from "./types";

const task = (id: string, depends: string[] = [], over: Partial<Task> = {}): Task => ({
  queue: "muster", id, title: `Task ${id}`, kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: `todo/${id}-t.md`, claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends, forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});
// Muster's own chain (0019's AC2): 0014 → 0015, 0016, 0017; 0016 → 0018; 0002 → 0005 → 0004.
const MUSTER = [
  task("0014", [], { stage: "your-check" }),
  task("0015", ["0014"]), task("0016", ["0014"]), task("0017", ["0014"]),
  task("0018", ["0014", "0016"]),
  task("0002", [], { stage: "building", claimedBy: "wt-a" }), task("0005", ["0014", "0002"]), task("0004", ["0005"]),
  task("0021"), task("0026", [], { stage: "done" }),
];
/** One CSS rule's body. */
const rule = (sel: string) => new RegExp(`(?:^|\\n)${sel.replace(/[.*+?^${}()|[\]\\>]/g, "\\$&")} \\{([^}]*)\\}`).exec(css)?.[1] ?? "";
/** The view without its legend row, whose swatches wear the nodes' and arrows' classes (mu-173). */
const drawn = (html: string) => html.replace(/<div class="legend"[^]*?<\/ul><\/div>/, "");
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
  it("ready to start: in Ready, nobody on it, not blocked by the flag or by tasks (the core's verdict)", () => {
    const byId = new Map(MUSTER.map((t) => [t.id, t]));
    expect(readyToStart(byId.get("0021")!)).toBe(true);
    // 0014 is in Your check, merged: the core doesn't block 0015 on it (0042).
    expect(readyToStart(byId.get("0015")!)).toBe(true);
    // 0005 waits on 0002 in Building: the core says it's blocked by tasks.
    const waiting = task("0005", ["0014", "0002"], { blockedKind: "tasks", blockedBy: [{ id: "0002", stage: "building", holder: "wt-a" }] });
    expect(readyToStart(waiting)).toBe(false);
    expect(readyToStart(task("0030", [], { blocked: true }))).toBe(false);
    expect(readyToStart(task("0030", [], { claimedBy: "wt-b" }))).toBe(false);
    expect(readyToStart(byId.get("0002")!)).toBe(false); // Building
    // Blocked only by the tasks it depends on, as the core derives it (0024): not ready, even
    // when those tasks aren't among the ones given.
    expect(readyToStart(task("0030", [], { blockedKind: "tasks", blockedBy: [{ id: "0041", stage: "ready", holder: "" }] }))).toBe(false);
  });
});

describe("GraphView", () => {
  const view = (openKey: string | null, tasks = MUSTER) =>
    renderToStaticMarkup(createElement(GraphView, { tasks, all: tasks, multi: false, openKey, onOpen: () => {} }));
  it("draws a node per linked task, an arrow per link, and the rest in the not-linked strip", () => {
    const html = view(null);
    expect(drawn(html).match(/class="g-node /g)).toHaveLength(8);
    expect(drawn(html).match(/class="g-edge/g)).toHaveLength(8);
    expect(html).toContain("Not linked to any task (2)");
    expect(html).toContain('aria-pressed="false" class="ui-pill">Hide done</button>');
  });
  it("marks what's ready to start, and what's blocked by the tasks it waits on", () => {
    // As the core gives them: 0002 waits on 0001, which is only in Ready.
    const two = [task("0001"), task("0002", ["0001"], { blockedKind: "tasks", blockedBy: [{ id: "0001", stage: "ready", holder: "" }] })];
    const ready = layoutGraph(two);
    expect(ready.nodes).toHaveLength(2);
    const html = view(null, two);
    expect(html).toMatch(/class="g-node s-ready ready"[^>]*>.*?0001/);
    expect(html).not.toMatch(/class="g-node s-ready ready"[^>]*>.*?<span class="id">0002/);
    expect(html).toMatch(/class="g-node s-ready"[^>]*><span class="g-top"><span class="id">0002/);
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
    expect(chip("0022")).toContain('<span class="ui-badge ui-badge--blocked">blocked</span>');
    expect(chip("0022")).not.toContain("g-ready");
    expect(chip("0023")).toContain('<span class="g-stage">Building</span>');
    expect(chip("0023")).toContain('<span class="g-who">wt-b</span>');
    expect(chip("0024")).toContain('<span class="ui-badge ui-badge--warn">needs you</span>');
  });
  it("marks a task blocked only by the tasks it depends on as its card does: neutral, after them (0024, 0060)", () => {
    const byTasks = { blockedKind: "tasks", blockedBy: [{ id: "0001", stage: "ready", holder: "" }] };
    const html = view(null, [task("0001", [], { forWho: "human" }), task("0002", ["0001"], byTasks), task("0009", [], byTasks)]);
    expect(html).toMatch(/class="g-node s-ready"[^>]*><span class="g-top"><span class="id">0002<\/span>.*?<span class="ui-badge block-why not-you look-tasks"><b>After<\/b> 0001<\/span>/);
    expect(html).not.toContain("ui-badge--blocked");
    const chip = new RegExp(`<button[^>]*g-loose[^>]*><span class="id">0009</span>.*?</button>`).exec(html)?.[0] ?? "";
    expect(chip).toContain('<span class="ui-badge block-why not-you look-tasks"><b>After</b> 0001</span>');
    expect(chip).not.toContain("g-ready");
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
  it("sets the owner's own tasks apart with the You ribbon, as a node and as a chip (0022)", () => {
    const tasks = [task("0001", [], { forWho: "human" }), task("0002", ["0001"]), task("0003", [], { forWho: "human" }), task("0004")];
    const html = view(null, tasks);
    expect(html).toMatch(/class="g-node s-ready human ready"[^>]*><span class="g-top"><span class="id">0001<\/span><span class="you-ribbon"[^>]*>You<\/span>/);
    expect(html).toMatch(/class="ui-pill g-loose s-ready human"[^>]*><span class="id">0003<\/span><span class="you-ribbon"/);
    expect(drawn(html).match(/class="you-ribbon"/g)).toHaveLength(2);
  });
  it("says how to start when nothing is linked, with a task key as the example", () => {
    expect(view(null, [task("0001"), task("0002")])).toContain("No task here depends on another yet.");
    const html = view(null, [task("0001", [], { prefix: "mu" }), task("0014", [], { prefix: "mu" })]);
    expect(html).toContain("<code>muster set &lt;id&gt; depends mu-1</code>");
    expect(html).not.toContain("depends 0014");
  });
  it("puts a space between the parts of a chip's state, so the words don't run together (Donehigh)", () => {
    const loose = [task("0021", [], { stage: "done", priority: "high" }), task("0024", [], { stage: "your-check", priority: "high", waitingForOwner: true, claimedBy: "wt-x" })];
    const chip = (id: string) => (new RegExp(`<button[^>]*g-loose[^>]*><span class="id">${id}</span>.*?</button>`).exec(view(null, loose))?.[0] ?? "").replace(/<[^>]+>/g, "").replace(/\s+/g, " ");
    expect(chip("0021")).toContain("Done high");
    expect(chip("0024")).toContain("Your check high needs you");
  });
  it("lays a chip's parts out with a gap, and a node's top line wraps whole words, so needs you never clips to nee", () => {
    expect(rule(".g-loose")).toMatch(/display: inline-flex;[^]*gap: 6px;/);
    expect(rule(".g-top")).toContain("flex-wrap: wrap;");
    expect(rule(".g-top")).not.toContain("overflow: hidden");
    expect(rule(".g-top > *")).toMatch(/flex: none;\s*white-space: nowrap;/);
  });
});

describe("the graph's legend lists what the graph draws (mu-173)", () => {
  const marks = (...queues: Task[][]) => graphMarks(queues.map(layoutGraph));
  const LOOP = [task("0001", ["0002"]), task("0002", ["0001"])];
  it("names the stages of nodes and of unlinked tasks, and ready to start, in a fixed order", () => {
    // Nodes in Your check, Building and Ready (six of them ready to start); Done only on an unlinked chip.
    expect(marks(MUSTER)).toEqual(["ready", "building", "your-check", "done", "ready-to-start"]);
    expect(marks([task("0001", [], { stage: "backlog" }), task("0002", ["0001"], { stage: "shaping" }), task("0003", [], { stage: "testing" })])).toEqual(["backlog", "shaping", "testing"]);
  });
  it("names ready to start only for a node: an unlinked chip has no outline", () => {
    expect(marks([task("0021"), task("0022")])).toEqual(["ready"]);
  });
  it("names blocked for a node drawn blocked, not for a chip or a wait on other tasks", () => {
    expect(marks([task("0001", [], { stage: "building", blocked: true }), task("0002", ["0001"], { stage: "building" })])).toEqual(["building", "blocked"]);
    expect(marks([task("0009", [], { stage: "building", blocked: true })])).toEqual(["building"]);
    const byTasks = { blockedKind: "tasks", blockedBy: [{ id: "0001", stage: "building", holder: "" }] };
    expect(marks([task("0001", [], { stage: "building" }), task("0002", ["0001"], byTasks)])).toEqual(["ready", "building"]);
  });
  it("names the owner's own task as a node and as a chip", () => {
    expect(marks([task("0001", [], { stage: "building", forWho: "human" }), task("0002", ["0001"], { stage: "building" })])).toEqual(["building", "human"]);
    expect(marks([task("0009", [], { stage: "building", forWho: "human" })])).toEqual(["building", "human"]);
  });
  it("names a loop when two tasks wait on each other", () => {
    expect(marks(LOOP)).toEqual(["ready", "ready-to-start", "loop"]);
  });
  it("lists each mark once across queues, and nothing for no graphs", () => {
    expect(marks([task("0001", [], { stage: "done" })], [task("0002", [], { stage: "done" })], [task("0003", [], { stage: "building" })])).toEqual(["building", "done"]);
    expect(graphMarks([])).toEqual([]);
    expect(marks([])).toEqual([]);
  });
  it("one rule says which arrow closes a loop, and the drawing uses it", () => {
    const g = layoutGraph(LOOP);
    expect(g.edges.filter((e) => isLoopEdge(g, e))).toHaveLength(1);
    const m = layoutGraph(MUSTER);
    expect(m.edges.some((e) => isLoopEdge(m, e))).toBe(false);
    // A three-task loop closes once too.
    const three = layoutGraph([task("0001", ["0003"]), task("0002", ["0001"]), task("0003", ["0002"])]);
    expect(three.edges.filter((e) => isLoopEdge(three, e))).toHaveLength(1);
    const html = drawn(renderToStaticMarkup(createElement(GraphView, { tasks: LOOP, all: LOOP, multi: false, openKey: null, onOpen: () => {} })));
    expect(html.match(/class="g-edge[^"]*"/g)).toHaveLength(2);
    expect(html.match(/class="g-edge loop"/g)).toHaveLength(1);
  });
});

describe("the Graph view's legend (mu-173)", () => {
  const view = (tasks: Task[]) => renderToStaticMarkup(createElement(GraphView, { tasks, all: tasks, multi: false, openKey: null, onOpen: () => {} }));
  it("sits under the bar, after Hide done and one short line, and names what every queue draws", () => {
    const other = [task("0001", [], { queue: "other", stage: "testing", forWho: "human" })];
    const html = view([...MUSTER, ...other]);
    expect(html).toMatch(/<div class="graph-bar"><button[^>]*>Hide done<\/button><span class="meta">Arrows point from a task to what waits on it\.<\/span><\/div><div class="legend" role="group" aria-label="What the colours mean">/);
    const names = [...(/<ul class="legend-list">[^]*?<\/ul>/.exec(html)?.[0] ?? "").matchAll(/<\/span>([^<]+)<\/li>/g)].map((m) => m[1]);
    expect(names).toEqual(["Ready", "Building", "Testing", "Waiting for your check", "Done", "Ready to start", "Your own task"]);
    expect(html).not.toContain("g-legend");
  });
  it("stays in view: the bar and the legend sit in a strip outside the scrolling graphs (R2)", () => {
    const html = view(MUSTER);
    expect(html).toMatch(/^<main class="graph-view"><div class="graph-strip"><div class="graph-bar">.*?<\/div><div class="legend" role="group"[^>]*><ul class="legend-list">.*?<\/ul><ul class="legend-list legend-reserve" aria-hidden="true">.*?<\/ul><\/div><\/div><div class="graph-scroll">/);
    const scroll = html.slice(html.indexOf('<div class="graph-scroll">'));
    expect(html.slice(0, html.indexOf('<div class="graph-scroll">'))).not.toContain("graph-queue");
    expect(scroll).toContain("graph-queue");
    expect(scroll).not.toContain('class="legend');
    expect(view([])).toMatch(/<div class="graph-scroll"><p class="empty">No tasks to show\.<\/p><\/div><\/main>$/);
  });
  it("scrolls only the graphs: the view is a column, the strip keeps its size, the scroller takes the rest", () => {
    const rule = (sel: string) => new RegExp(`(?:^|\\n)\\${sel} \\{([^}]*)\\}`).exec(css)?.[1] ?? "";
    expect(rule(".graph-view")).toMatch(/flex: 1;[^]*display: flex;[^]*flex-direction: column;[^]*min-block-size: 0;/);
    expect(rule(".graph-view")).not.toContain("overflow");
    expect(rule(".graph-strip")).toContain("flex: none");
    expect(rule(".graph-scroll")).toMatch(/flex: 1;[^]*min-block-size: 0;[^]*overflow: auto;/);
  });
  it("leaves room above the top node for its selection outline and focus ring, keeping the 10px gap (R5)", () => {
    const rule = (sel: string) => new RegExp(`(?:^|\\n)${sel.replace(/[.>]/g, "\\$&")} \\{([^}]*)\\}`).exec(css)?.[1] ?? "";
    // .g-node.sel reaches 4px out (2px outline at 2px offset), the focus ring 3px: 6px holds both.
    expect(rule(".g-node.sel")).toMatch(/outline: 2px solid var\(--accent\);[^]*outline-offset: 2px;/);
    expect(rule(".graph-scroll")).toContain("padding: 6px 14px 16px;");
    expect(rule(".graph-strip > .legend")).toContain("margin-block-end: 4px;");
  });
  it("keeps the row, its list empty and every item reserved, when there's nothing to draw (R4)", () => {
    const html = view([]);
    expect(html).toContain('<div class="legend" role="group" aria-label="What the colours mean"><ul class="legend-list"></ul><ul class="legend-list legend-reserve" aria-hidden="true">');
    const reserved = /<ul class="legend-list legend-reserve"[^]*?<\/ul>/.exec(html)?.[0] ?? "";
    expect(reserved.match(/<li /g)).toHaveLength(11);
  });
  it("has no rule left for the old inline legend", () => {
    expect(css).not.toContain(".g-legend");
  });
});
