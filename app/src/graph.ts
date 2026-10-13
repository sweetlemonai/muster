// The dependency graph (task 0019): a layered layout, left to right, in our own code (no library).
// Ranks are the longest chain of dependencies before a task, so what can start now sits on the
// left; within a rank, a few barycentre sweeps keep linked tasks near each other.
import { isBlocked } from "./blockWords";
import { stageOf } from "./lib";
import type { Task } from "./types";

export type GraphNode = { task: Task; id: string; rank: number; order: number };
/** An arrow from a dependency to the task that waits on it. */
export type GraphEdge = { from: string; to: string };
export type Graph = {
  nodes: GraphNode[];
  edges: GraphEdge[];
  /** Tasks with no link either way, listed apart so they don't crowd the first rank. */
  loose: Task[];
  ranks: number;
  /** The most tasks in one rank. */
  rows: number;
};

const SWEEPS = 4;
const PRIORITY: Record<string, number> = { high: 0, normal: 1, low: 2 };

/** Lay out one queue's tasks. Dependencies on tasks not in `tasks` (another queue, filtered out,
 *  or hidden because done) are left out; a loop doesn't stop the layout (check reports it). */
export function layoutGraph(tasks: Task[]): Graph {
  const byId = new Map(tasks.map((t) => [t.id, t]));
  const deps = new Map<string, string[]>();
  const users = new Map<string, string[]>();
  const edges: GraphEdge[] = [];
  for (const t of tasks) {
    const ds = t.depends.filter((d) => d !== t.id && byId.has(d));
    deps.set(t.id, ds);
    for (const d of ds) {
      edges.push({ from: d, to: t.id });
      users.set(d, [...(users.get(d) ?? []), t.id]);
    }
  }
  // Linked by its own depends (even when what it waits on isn't shown: filtered out, hidden as
  // done, another queue), or by something here waiting on it.
  const linkedIds = new Set(tasks.filter((t) => t.depends.some((d) => d !== t.id) || users.has(t.id)).map((t) => t.id));
  const linked = tasks.filter((t) => linkedIds.has(t.id));
  const loose = tasks.filter((t) => !linkedIds.has(t.id)).sort(byStageThenPriority);

  // Longest path from the sources, ignoring the edge that closes a loop.
  const rank = new Map<string, number>();
  const visiting = new Set<string>();
  const rankOf = (id: string): number => {
    const known = rank.get(id);
    if (known !== undefined) return known;
    visiting.add(id);
    let r = 0;
    for (const d of deps.get(id) ?? []) if (!visiting.has(d)) r = Math.max(r, rankOf(d) + 1);
    visiting.delete(id);
    rank.set(id, r);
    return r;
  };
  for (const t of [...linked].sort((a, b) => a.id.localeCompare(b.id))) rankOf(t.id);

  const ranks = linked.length ? Math.max(...linked.map((t) => rank.get(t.id) ?? 0)) + 1 : 0;
  const layers: Task[][] = Array.from({ length: ranks }, () => []);
  for (const t of linked) layers[rank.get(t.id) ?? 0].push(t);
  for (const l of layers) l.sort(byStageThenPriority);

  // Barycentre sweeps: order each rank by the mean position of its neighbours in the rank before
  // (left to right) or after (right to left). Ties keep the current order.
  const pos = new Map<string, number>();
  const place = () => layers.forEach((l) => l.forEach((t, i) => pos.set(t.id, i)));
  place();
  const mean = (ids: string[]) => (ids.length ? ids.reduce((s, id) => s + (pos.get(id) ?? 0), 0) / ids.length : Infinity);
  for (let s = 0; s < SWEEPS; s++) {
    const leftToRight = s % 2 === 0;
    const order = leftToRight ? layers.keys() : [...layers.keys()].reverse();
    for (const r of order) {
      const l = layers[r];
      const weight = new Map(l.map((t, i) => [t.id, { w: mean(leftToRight ? deps.get(t.id) ?? [] : users.get(t.id) ?? []), i }]));
      l.sort((a, b) => {
        const x = weight.get(a.id)!, y = weight.get(b.id)!;
        return x.w === y.w ? x.i - y.i : x.w === Infinity ? 1 : y.w === Infinity ? -1 : x.w - y.w;
      });
      l.forEach((t, i) => pos.set(t.id, i));
    }
  }

  const nodes = layers.flatMap((l, r) => l.map((task, order) => ({ task, id: task.id, rank: r, order })));
  return { nodes, edges, loose, ranks, rows: Math.max(0, ...layers.map((l) => l.length)) };
}

function byStageThenPriority(a: Task, b: Task): number {
  const s = STAGE_ORDER.indexOf(stageOf(a)) - STAGE_ORDER.indexOf(stageOf(b));
  if (s) return s;
  const p = (PRIORITY[a.priority] ?? 1) - (PRIORITY[b.priority] ?? 1);
  return p || a.id.localeCompare(b.id);
}
const STAGE_ORDER = ["done", "your-check", "testing", "building", "ready", "shaping", "backlog"];

/** What a task waits on that isn't drawn here (filtered out, hidden as done, or missing). */
export function hiddenDeps(t: Task, shown: Set<string>): string[] {
  return t.depends.filter((d) => d !== t.id && !shown.has(d));
}

/** Everything `id` waits on, directly or through others. */
export function upstream(g: Pick<Graph, "edges">, id: string): Set<string> {
  return reach(g.edges, id, (e) => [e.to, e.from]);
}
/** Everything that waits on `id`, directly or through others. */
export function downstream(g: Pick<Graph, "edges">, id: string): Set<string> {
  return reach(g.edges, id, (e) => [e.from, e.to]);
}
function reach(edges: GraphEdge[], start: string, dir: (e: GraphEdge) => [string, string]): Set<string> {
  const next = new Map<string, string[]>();
  for (const e of edges) {
    const [a, b] = dir(e);
    next.set(a, [...(next.get(a) ?? []), b]);
  }
  const seen = new Set<string>();
  const todo = [start];
  while (todo.length) {
    for (const n of next.get(todo.pop()!) ?? []) {
      if (n !== start && !seen.has(n)) {
        seen.add(n);
        todo.push(n);
      }
    }
  }
  return seen;
}

/** Is this arrow on the selected task's chain: between two tasks it waits on (or into it), or
 *  out of it and between two tasks that wait on it? */
export function edgeLit(e: GraphEdge, selected: string | null, up: Set<string>, down: Set<string>): boolean {
  if (!selected) return false;
  const upward = up.has(e.from) && (up.has(e.to) || e.to === selected);
  const downward = (down.has(e.from) || e.from === selected) && down.has(e.to);
  return upward || downward;
}

/** Ready to start: in Ready, nobody holds it, and not blocked, by the flag or by the tasks it
 *  depends on. That last is the core's verdict (`blockedBy`, block.rs `holds`: a dependency counts
 *  once it's merged, in Your check or Done, the owner's own once Done, 0042), so the Graph marks
 *  exactly what `muster claim` takes. A paused one (0045) isn't. */
export function readyToStart(t: Task): boolean {
  return stageOf(t) === "ready" && !t.claimedBy && !isBlocked(t) && !t.paused;
}
