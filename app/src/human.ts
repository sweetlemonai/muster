import { blockSentence } from "./blockWords";
import { key, priorityName, stageRank } from "./lib";
import type { Task } from "./types";

// The owner's own tasks and the Human board (0022), as pure helpers (tested in human.test.ts).
// A task is the owner's when its file says `for: human`: something only they can do. Agents
// never take one (the core refuses). Everything that needs the owner is counted by `needsYou`,
// the one definition the sidebar, the header, the Human board and 0006's Dock badge share.

/** The owner's own task (`for: human`); every other one, every older file included, is an agent's. */
export const isHuman = (t: Pick<Task, "forWho">) => t.forWho === "human";

/** The Human board's columns, in order. */
export const HUMAN_COLUMNS = [
  { id: "todo", label: "To do" },
  { id: "doing", label: "Doing" },
  { id: "done", label: "Done" },
] as const;
export type HumanColumn = (typeof HUMAN_COLUMNS)[number]["id"];

/** Where the owner's own task sits: To do before Building (Ready, or a stage before it), Doing
 *  from Building until Done, then Done. A stage the window doesn't know reads as To do. */
export function humanColumn(stage: string): HumanColumn {
  if (stage === "done") return "done";
  return stageRank(stage) >= stageRank("building") ? "doing" : "todo";
}

/** An agent's task that waits on the owner (the core's needs_owner: Your check, a question
 *  escalated to them, blocked on you or by a permission): the Human board's "From the agents" lane. */
export const fromTheAgents = (t: Pick<Task, "forWho" | "waitingForOwner">) => !isHuman(t) && t.waitingForOwner;

/** Does this task need the owner? Their own task until it's done, or an agent's waiting on them. */
export function needsYou(t: Pick<Task, "forWho" | "waitingForOwner" | "stage">): boolean {
  return isHuman(t) ? humanColumn(t.stage) !== "done" : t.waitingForOwner;
}

/** How many tasks need the owner: the Human board's To do + Doing + From the agents. */
export const needsYouCount = (tasks: Pick<Task, "forWho" | "waitingForOwner" | "stage">[]) => tasks.filter(needsYou).length;

export type HumanBoard = { lane: Task[]; todo: Task[]; doing: Task[]; done: Task[] };

const PRIORITY_RANK = { high: 0, normal: 1, low: 2 } as const;
/** What waits on the owner first, then priority, then project and id (as the board's columns). */
const byAttention = (a: Task, b: Task) =>
  Number(b.waitingForOwner) - Number(a.waitingForOwner) ||
  PRIORITY_RANK[priorityName(a.priority)] - PRIORITY_RANK[priorityName(b.priority)] ||
  a.queue.localeCompare(b.queue) ||
  a.id.localeCompare(b.id);

/** The Human board of these tasks: the lane (agents' tasks waiting on the owner, where they are)
 *  and the owner's own tasks by column; Done newest first. */
export function humanBoard(tasks: Task[]): HumanBoard {
  const out: HumanBoard = { lane: [], todo: [], doing: [], done: [] };
  for (const t of tasks) {
    if (isHuman(t)) out[humanColumn(t.stage)].push(t);
    else if (fromTheAgents(t)) out.lane.push(t);
  }
  out.lane.sort(byAttention);
  out.todo.sort(byAttention);
  out.doing.sort(byAttention);
  out.done.sort((a, b) => b.modified - a.modified || a.id.localeCompare(b.id));
  return out;
}

/** Why a lane item waits on the owner, one line each: Your check, each open question escalated to
 *  them (its first line), and a block on you or by a permission in its sentence (0024). */
export function laneLines(t: Pick<Task, "stage" | "questions" | "blockedKind" | "blockDetail" | "blockedBy">): string[] {
  const out: string[] = [];
  if (t.stage === "your-check") out.push("In Your check: accept it, or send it back");
  for (const q of t.questions) {
    if (q.ticked || !q.escalated) continue;
    const first = q.text.split("\n").find((l) => l.trim())?.trim() ?? "";
    out.push(`Q${q.n ?? "?"}: ${first}`);
  }
  if (t.blockedKind === "you" || t.blockedKind === "permission") out.push(blockSentence(t));
  return out;
}

/** The owner's own tasks this one depends on that aren't done yet (same project): it can't be
 *  claimed until they are. Derived from `depends:`; nothing is written when they finish. */
export function waitsOnYou(t: Pick<Task, "queue" | "id" | "depends">, index: Map<string, Task>): string[] {
  return t.depends.filter((d) => {
    const dep = d === t.id ? undefined : index.get(key({ queue: t.queue, id: d }));
    return !!dep && isHuman(dep) && dep.stage !== "done";
  });
}

/** Table's and Timeline's filter: the owner's own tasks, the agents', or both. */
export type ForFilter = "human" | "ai" | "all";
export const FOR_FILTERS: { id: ForFilter; label: string }[] = [
  { id: "human", label: "Human" },
  { id: "ai", label: "AI" },
  { id: "all", label: "All" },
];
export const FOR_KEY = "muster.forFilter";
/** A remembered filter; All for anything else. */
export const parseForFilter = (v: string | null): ForFilter => (v === "human" || v === "ai" ? v : "all");

export function byWho<T extends Pick<Task, "forWho">>(tasks: T[], f: ForFilter): T[] {
  return f === "all" ? tasks : tasks.filter((t) => isHuman(t) === (f === "human"));
}
