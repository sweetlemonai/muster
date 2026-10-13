import { holder, isBoardSort, priorityName, stageLabel, stageOf, STAGES, stamp, type BoardSort } from "./lib";
import { queryWords, searchWords, taskMatches, textMatches } from "./search";
import { needsYou } from "./human";
import type { Note, Task } from "./types";

// The filter bar's model (task 0032): a search, filters by group, and the board's sort. A group
// is a kind of value a task has (stage, type, tag…). Within a group any selected value will do;
// across groups every group must match. A group shows only the values its project has, so new
// groups (level and parent 0016, human or AI 0022, blocked kinds 0024) slot in as entries here.

/** Selected values by group id. A group with nothing selected isn't in it. */
export type Filters = Record<string, string[]>;
export type Query = { search: string; filters: Filters; sort: BoardSort };
export const EMPTY_QUERY: Query = { search: "", filters: {}, sort: "priority" };

/** Days as "YYYY-MM-DD", local time: today, the Monday this week started, the 1st of the month. */
export type DayContext = { today: string; week: string; month: string };

export function dayContext(now: number): DayContext {
  const d = new Date(now * 1000);
  const day = (x: Date) => stamp(x).slice(0, 10);
  const monday = new Date(d.getFullYear(), d.getMonth(), d.getDate() - ((d.getDay() + 6) % 7));
  return { today: day(d), week: day(monday), month: day(new Date(d.getFullYear(), d.getMonth(), 1)) };
}

export type Group = {
  id: string;
  /** The group's name on the bar. */
  label: string;
  /** The values these tasks have, in the group's order. */
  present: (tasks: Task[], ctx: DayContext) => string[];
  has: (t: Task, value: string, ctx: DayContext) => boolean;
  /** A value as its chip reads in the group. */
  chip: (value: string) => string;
  /** A value as the active line reads it, on its own. */
  active: (value: string) => string;
};

const cmp = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);
const same = (v: string) => v;

/** A group of free values (types, tags, holders): those the tasks have, sorted. */
function listGroup(id: string, label: string, get: (t: Task) => string[], chip = same, active = chip): Group {
  return {
    id,
    label,
    present: (tasks) => [...new Set(tasks.flatMap(get).filter(Boolean))].sort(cmp),
    has: (t, v) => get(t).includes(v),
    chip,
    active,
  };
}

/** A group of known values in a fixed order: those some task has. */
function fixedGroup(id: string, label: string, order: readonly string[], has: Group["has"], chip: (v: string) => string, active = chip): Group {
  return { id, label, present: (tasks, ctx) => order.filter((v) => tasks.some((t) => has(t, v, ctx))), has, chip, active };
}

const STATES: Record<string, { label: string; has: (t: Task) => boolean }> = {
  blocked: { label: "blocked", has: (t) => t.blocked },
  // The one needs-you definition (0022): an agent's task waiting on the owner, or the owner's own until done.
  "needs-you": { label: "needs you", has: needsYou },
  questions: { label: "open questions", has: (t) => t.openQuestions > 0 },
};
const SPANS: Record<string, { label: string; from: keyof DayContext }> = {
  today: { label: "today", from: "today" },
  week: { label: "this week", from: "week" },
  month: { label: "this month", from: "month" },
};
const changedDay = (t: Task) => (t.modified > 0 ? stamp(new Date(t.modified * 1000)).slice(0, 10) : "");

/** A date group: today, this week, this month (each since its start, local time). */
function dateGroup(id: string, label: string, verb: string, day: (t: Task) => string): Group {
  return fixedGroup(
    id,
    label,
    Object.keys(SPANS),
    (t, v, ctx) => {
      const d = day(t);
      const span = SPANS[v];
      return d !== "" && span !== undefined && d >= ctx[span.from];
    },
    (v) => SPANS[v]?.label ?? v,
    (v) => `${verb} ${SPANS[v]?.label ?? v}`,
  );
}

export const GROUPS: Group[] = [
  fixedGroup("stage", "Stage", STAGES.map((s) => s.id), (t, v) => stageOf(t) === v, stageLabel),
  listGroup("type", "Type", (t) => [t.kind]),
  fixedGroup("priority", "Priority", ["high", "normal", "low"], (t, v) => priorityName(t.priority) === v, same),
  listGroup("tag", "Tags", (t) => t.tags, (v) => `#${v}`),
  listGroup("holder", "Held by", (t) => [holder(t)], same, (v) => `held by ${v}`),
  fixedGroup("state", "Flags", Object.keys(STATES), (t, v) => STATES[v]?.has(t) ?? false, (v) => STATES[v]?.label ?? v),
  dateGroup("captured", "Captured", "captured", (t) => t.captured.slice(0, 10)),
  dateGroup("changed", "Changed", "changed", changedDay),
  dateGroup("done", "Done", "done", (t) => t.doneOn.slice(0, 10)),
];

/** A group's chips: the values the tasks have, plus any selected one they no longer have (so it
 *  can still be cleared). */
export function groupValues(g: Group, tasks: Task[], selected: string[], ctx: DayContext): string[] {
  const present = g.present(tasks, ctx);
  return [...present, ...selected.filter((v) => !present.includes(v))];
}

/** The tasks the search and filters keep, in the order given. */
export function applyQuery(tasks: Task[], q: Query, ctx: DayContext): Task[] {
  const words = queryWords(q.search);
  const active = GROUPS.map((g) => [g, q.filters[g.id] ?? []] as const).filter(([, v]) => v.length > 0);
  if (!words.length && !active.length) return tasks;
  return tasks.filter((t) => active.every(([g, vals]) => vals.some((v) => g.has(t, v, ctx))) && taskMatches(t, words));
}

/** Backlog notes on the board under a query: matched by the search on their title and text,
 *  and hidden by any filter but a Stage one that includes Backlog (notes have no other fields). */
export function notesFor(notes: Note[], q: Query): Note[] {
  const groups = GROUPS.filter((g) => (q.filters[g.id] ?? []).length > 0).map((g) => g.id);
  if (groups.some((g) => g !== "stage") || (groups.includes("stage") && !q.filters.stage.includes("backlog"))) return [];
  const words = queryWords(q.search);
  if (!words.length) return notes;
  return notes.filter((n) => textMatches(noteText(n), words));
}

// A note's folded text, kept while the window holds that note (a poll brings new ones), so
// typing in the search doesn't fold every note again (up to 20 KB each) per key.
const noteTexts = new WeakMap<Note, string>();
function noteText(n: Note): string {
  let s = noteTexts.get(n);
  if (s === undefined) {
    s = searchWords(`${n.title}\n${n.text}`);
    noteTexts.set(n, s);
  }
  return s;
}

export const isActive = (q: Query) => q.search.trim() !== "" || Object.values(q.filters).some((v) => v.length > 0);

export function toggleFilter(f: Filters, group: string, value: string): Filters {
  const cur = f[group] ?? [];
  const next = cur.includes(value) ? cur.filter((v) => v !== value) : [...cur, value];
  const out = { ...f };
  if (next.length) out[group] = next;
  else delete out[group];
  return out;
}
export const removeFilter = (f: Filters, group: string, value: string): Filters =>
  (f[group] ?? []).includes(value) ? toggleFilter(f, group, value) : f;

export type ActiveItem = { group: string; value: string; label: string };

/** The active line: the search, then each filter in the groups' order ("Building · high · #design"). */
export function activeItems(q: Query): ActiveItem[] {
  const out: ActiveItem[] = [];
  const s = q.search.trim();
  if (s) out.push({ group: "search", value: s, label: `“${s}”` });
  for (const g of GROUPS) for (const v of q.filters[g.id] ?? []) out.push({ group: g.id, value: v, label: g.active(v) });
  return out;
}

/** What the views say when filters hide tasks, so nothing disappears silently. */
export function hiddenLine(hidden: number, active: boolean): string {
  if (!active) return "";
  return hidden > 0 ? `${hidden} hidden by filters` : "nothing hidden";
}

// Saved views: a name for a search, filters and sort, per project, in the app's local storage.

export type SavedView = Query & { name: string };
export type Store = { getItem: (k: string) => string | null; setItem: (k: string, v: string) => void };
export const MAX_VIEWS = 20;
const MAX_NAME = 40;
const MAX_SEARCH = 200;
export const viewsKey = (project: string) => `muster.views.${project}`;

function cleanFilters(raw: unknown): Filters {
  const out: Filters = {};
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return out;
  for (const [k, v] of Object.entries(raw)) {
    if (!Array.isArray(v)) continue;
    const vals = v.filter((x): x is string => typeof x === "string" && x.length > 0 && x.length <= 100).slice(0, 50);
    if (vals.length) out[k.slice(0, 40)] = vals;
  }
  return out;
}

function cleanView(raw: unknown): SavedView | null {
  if (!raw || typeof raw !== "object") return null;
  const r = raw as Record<string, unknown>;
  if (typeof r.name !== "string" || !r.name.trim()) return null;
  return {
    name: r.name.trim().slice(0, MAX_NAME),
    search: typeof r.search === "string" ? r.search.slice(0, MAX_SEARCH) : "",
    filters: cleanFilters(r.filters),
    sort: isBoardSort(r.sort) ? r.sort : "priority",
  };
}

/** A project's saved views; none when there are none, or what's stored is broken. */
export function loadViews(store: Store, project: string): SavedView[] {
  try {
    const raw: unknown = JSON.parse(store.getItem(viewsKey(project)) ?? "[]");
    if (!Array.isArray(raw)) return [];
    return raw.map(cleanView).filter((v): v is SavedView => v !== null).slice(-MAX_VIEWS);
  } catch {
    return [];
  }
}

/** Writes a project's saved views. A storage that refuses (full, denied) keeps them for this
 *  session only, which is all it can do. */
export function storeViews(store: Store, project: string, views: SavedView[]): boolean {
  try {
    store.setItem(viewsKey(project), JSON.stringify(views));
    return true;
  } catch {
    return false;
  }
}

/** Saves a view: one with the same name (any case) is replaced where it is, a new one goes last,
 *  and the oldest go past MAX_VIEWS. An empty name saves nothing. */
export function saveView(views: SavedView[], v: SavedView): SavedView[] {
  const clean = cleanView(v);
  if (!clean) return views;
  const at = views.findIndex((x) => x.name.toLowerCase() === clean.name.toLowerCase());
  if (at >= 0) return views.map((x, i) => (i === at ? clean : x));
  return [...views, clean].slice(-MAX_VIEWS);
}

export const deleteView = (views: SavedView[], name: string) => views.filter((v) => v.name !== name);

/** Whether a query is what a saved view holds (values in any order). */
export function sameQuery(a: Query, b: Query): boolean {
  if (a.search !== b.search || a.sort !== b.sort) return false;
  const keys = (f: Filters) => Object.keys(f).filter((k) => f[k].length > 0).sort();
  const ka = keys(a.filters), kb = keys(b.filters);
  if (ka.join("\n") !== kb.join("\n")) return false;
  return ka.every((k) => [...a.filters[k]].sort().join("\n") === [...b.filters[k]].sort().join("\n"));
}
