import { priorityName, stageRank } from "./lib";
import type { Task } from "./types";

// The owner's own tasks (0022), as pure helpers (tested in human.test.ts). A task is the owner's
// when its file says `for: human`: something only they can do. Agents never take one (the core
// refuses). It sits on the one board with the agents' tasks, hatched and with the You ribbon
// (owner, 2026-10-13); lib.ts `boardColumn` says which column. Everything that needs the owner
// is counted by `needsYou`, the one definition the sidebar, the header and the needs-you filter
// chip share. The Dock badge (0006) counts in the core, which can read while the window is hidden:
// badge.rs `needs_you` is this rule, and `needs-cases.json` holds the two to the same answers.

/** The owner's own task (`for: human`); every other one, every older file included, is an agent's. */
export const isHuman = (t: Pick<Task, "forWho">) => t.forWho === "human";

/** Where the owner's own task is in its own words: To do, Doing or Done. */
export type HumanColumn = "todo" | "doing" | "done";

/** To do before Building (Ready, or a stage before it), Doing from Building until Done, then Done.
 *  A stage the window doesn't know reads as To do. On the board: Ready, Building and Done. */
export function humanColumn(stage: string): HumanColumn {
  if (stage === "done") return "done";
  return stageRank(stage) >= stageRank("building") ? "doing" : "todo";
}

/** Does this task need the owner? Their own task until it's done, unless they paused it (0045), or
 *  an agent's waiting on them (the core's needs_owner: Your check, a question escalated to them,
 *  blocked on you or by a permission; and in the app, its session waiting for input in Claude
 *  Code, 0048). */
export function needsYou(t: Pick<Task, "forWho" | "waitingForOwner" | "stage" | "paused">): boolean {
  return isHuman(t) ? humanColumn(t.stage) !== "done" && !t.paused : t.waitingForOwner;
}

/** How many tasks need the owner: their own to do and doing, plus what the agents wait on them for. */
export const needsYouCount = (tasks: Pick<Task, "forWho" | "waitingForOwner" | "stage" | "paused">[]) => tasks.filter(needsYou).length;

/** The needs-you count's words without the number, for a heading whose digit is its own chip:
 *  "needs you" for one, "need you" for any other. The one place the app chooses them (mu-159), so
 *  translating it (mu-147) is one plural key. */
export const needsVerb = (n: number) => (n === 1 ? "needs you" : "need you");

/** A needs-you count in words, the same everywhere it's said: "1 needs you", "2 need you" (mu-87,
 *  mu-159): the sidebar's rows and headings, the column chips and their cue, and the folded strips,
 *  bands and Paused lane. */
export const needsWords = (n: number) => `${n} ${needsVerb(n)}`;

const PRIORITY_RANK = { high: 0, normal: 1, low: 2 } as const;
/** What waits on the owner first, then priority, then project and id (as the board's columns). */
const byAttention = (a: Task, b: Task) =>
  Number(b.waitingForOwner) - Number(a.waitingForOwner) ||
  PRIORITY_RANK[priorityName(a.priority)] - PRIORITY_RANK[priorityName(b.priority)] ||
  a.queue.localeCompare(b.queue) ||
  a.id.localeCompare(b.id);

/** The Waiting for you tab's list, the tasks `needsYouCount` counts: what the agents wait on the
 *  owner for first, then the owner's own To do, then Doing (not the ones they paused, 0045), each
 *  by attention. */
export function waitingForYou(tasks: Task[]): Task[] {
  const agents = tasks.filter((t) => !isHuman(t) && t.waitingForOwner).sort(byAttention);
  const own = (c: HumanColumn) => tasks.filter((t) => isHuman(t) && humanColumn(t.stage) === c && needsYou(t)).sort(byAttention);
  return [...agents, ...own("todo"), ...own("doing")];
}

/** The Who chips on the Board, Table and Timeline: both kinds, the owner's own tasks, or the agents'. */
export type ForFilter = "human" | "ai" | "all";
export const FOR_FILTERS: { id: ForFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "human", label: "You" },
  { id: "ai", label: "Agents" },
];
export const FOR_KEY = "muster.forFilter";
/** A remembered filter; All for anything else. */
export const parseForFilter = (v: string | null): ForFilter => (v === "human" || v === "ai" ? v : "all");

/** The views that show the Who chips and follow them. */
export const whoFiltersOn = (view: string) => view === "board" || view === "table" || view === "timeline";

export function byWho<T extends Pick<Task, "forWho">>(tasks: T[], f: ForFilter): T[] {
  return f === "all" ? tasks : tasks.filter((t) => isHuman(t) === (f === "human"));
}

/** What a Who choice shows: the tasks it picks, and every backlog note. A note is nobody's yet,
 *  and hiding it would make one just added vanish (out of sight is out of mind). */
export function whoShows<T extends Pick<Task, "forWho">, N>(tasks: T[], notes: N[], f: ForFilter): { tasks: T[]; notes: N[] } {
  return { tasks: byWho(tasks, f), notes };
}
