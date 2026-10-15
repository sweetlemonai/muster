import { tl, tn } from "./i18n";
import { fmtMinutes, fmtTokens, key, type StageId, stageLabel, stageOf, stageRank } from "./lib";
import { siblingKey, taskKey } from "./taskKey";
import type { Progress, Task } from "./types";

// Goal → Feature → Task (decision 0003): a task names its parent, and the board and the table can
// group tasks by their feature or goal. The roll-ups come from the core (queue::rollup); these are
// pure helpers for showing them (tested in levels.test.ts).

/** The levels as the New task form offers them, the default first, their words in the language
 *  shown (board.json `levels`, mu-147). */
export const LEVELS = (["task", "feature", "goal"] as const).map((id) => ({
  id,
  get label() {
    return tn("board", `levels.${id}`);
  },
}));

export type GroupBy = "none" | "feature" | "goal";
/** Group by's choices, in the filter bar: their words in the language shown (common.json, mu-146). */
export const GROUP_BYS: { readonly id: GroupBy; readonly label: string }[] = (["none", "feature", "goal"] as const).map((id) => ({
  id,
  get label() {
    return tl(`groupBy.${id}`);
  },
}));
export const GROUP_KEY = "muster.groupBy";

/** A remembered Group by; anything else is None. */
export function parseGroupBy(value: string | null): GroupBy {
  return GROUP_BYS.find((g) => g.id === value)?.id ?? "none";
}

/** "Goal", "Feature", "Task" (no level is a task). */
export function levelLabel(level: string): string {
  return (LEVELS.find((l) => l.id === level) ?? LEVELS[0]).label;
}

/** Work for the stage columns: tasks. Goals and features show as bands and table rows instead. */
export const isWork = (t: Pick<Task, "level">) => t.level !== "goal" && t.level !== "feature";

/** Every item by project and id, to find parents (a parent is always in its child's project). */
export function indexTasks(tasks: Task[]): Map<string, Task> {
  const out = new Map<string, Task>();
  for (const t of tasks) if (!out.has(key(t))) out.set(key(t), t);
  return out;
}

export function parentOf(t: Pick<Task, "queue" | "parent">, index: Map<string, Task>): Task | null {
  return t.parent ? index.get(key({ queue: t.queue, id: t.parent })) ?? null : null;
}

/** A goal's or feature's children (the ids the core rolled up) that are at hand, in id order. */
export function childrenOf(t: Pick<Task, "queue" | "children">, index: Map<string, Task>): Task[] {
  return t.children.flatMap((id) => index.get(key({ queue: t.queue, id })) ?? []);
}

/** A card's quiet parent line: "▸ Feature mu-30 Stage track", or "" without a parent. */
export function parentLine(t: Pick<Task, "queue" | "parent" | "prefix">, index: Map<string, Task>): string {
  if (!t.parent) return "";
  const p = parentOf(t, index);
  return p ? tn("board", "levels.parent", { level: levelLabel(p.level), key: taskKey(p), title: p.title }) : tn("board", "levels.parentMissing", { key: siblingKey(t, t.parent) });
}

/** The band an item goes in: its feature (its parent, if that's a feature), or its goal (its
 *  parent if that's a goal, or its feature's goal). Null for the last band, "No feature". */
export function groupHead(t: Pick<Task, "queue" | "parent">, index: Map<string, Task>, by: "feature" | "goal"): Task | null {
  const p = parentOf(t, index);
  if (!p) return null;
  if (p.level === by) return p;
  if (by === "goal" && p.level === "feature") {
    const g = parentOf(p, index);
    return g?.level === "goal" ? g : null;
  }
  return null;
}

export type Group = { head: Task | null; items: Task[] };

/** Bands for the board (items: its tasks) or the table (items: every other row). One per feature
 *  or goal of `all`, by project and id, that holds any of `items`, is itself among `shown` (what
 *  the filters let through, which for the table is its rows), or has no tasks yet so it stays in
 *  view; then the items with none, in a last band (head null) when there are any. */
export function groupItems(items: Task[], all: Task[], by: "feature" | "goal", shown: Task[] = items): Group[] {
  const index = indexTasks(all);
  const lit = new Set(shown.filter((t) => t.level === by).map(key));
  const heads = all.filter((t) => t.level === by).sort((a, b) => a.queue.localeCompare(b.queue) || a.id.localeCompare(b.id));
  const into = new Map<string, Task[]>(heads.map((h) => [key(h), [] as Task[]]));
  const rest: Task[] = [];
  for (const t of items) {
    if (t.level === by) continue;
    const head = groupHead(t, index, by);
    const list = head ? into.get(key(head)) : undefined;
    if (list) list.push(t);
    else rest.push(t);
  }
  const out: Group[] = heads
    .map((h) => ({ head: h, items: into.get(key(h)) ?? [] }))
    .filter((g) => g.items.length > 0 || lit.has(key(g.head)) || (g.head.progress?.total ?? 0) === 0);
  if (rest.length) out.push({ head: null, items: rest });
  return out;
}

/** "3 of 7 done", "no tasks yet", or "" for a task. */
export function progressText(p: Pick<Progress, "done" | "total"> | null): string {
  if (!p) return "";
  return p.total ? tn("board", "levels.progress", { done: p.done, count: p.total }) : tn("board", "levels.noTasks");
}

/** The estimate of its open tasks: "≈ 2 h · 200k tokens left", or "" without one. The time is
 *  agent time plus test time (the core sums both into `estMinutes`, as `estimateTotal` does). */
export function estimateLeft(p: Progress | null): string {
  if (!p) return "";
  const parts = [fmtMinutes(p.estMinutes), fmtTokens(p.estTokenCount)].filter(Boolean).join(" · ");
  return parts ? tl("estimate", { parts }) : "";
}

/** The stage to show: "In progress" for a goal or feature with tasks under way. */
export function shownStageLabel(t: Pick<Task, "stage" | "shownStage">): string {
  return t.shownStage === "in-progress" ? tn("board", "levels.inProgress") : stageLabel(t.shownStage || t.stage);
}

/** Where the stage it shows sorts: a stage's place, In progress with Building. */
export function shownStageRank(t: Pick<Task, "stage" | "shownStage">): number {
  return stageRank(t.shownStage === "in-progress" ? "building" : t.shownStage || t.stage);
}

/** The stage colour to show it in (In progress takes Building's). */
export function shownStageClass(t: Pick<Task, "stage" | "shownStage">): StageId {
  return t.shownStage === "in-progress" ? "building" : stageOf({ stage: t.shownStage || t.stage });
}
