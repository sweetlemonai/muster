import { backlogColumn, type BacklogItem } from "./Backlog";
import { isBlocked } from "./blockWords";
import type { RowChip } from "./ColumnRow";
import { isHuman, needsYou } from "./human";
import { tl, tn } from "./i18n";
import { groupItems, type GroupBy } from "./levels";
import { boardColumn, type BoardSort, holder, needsTester, quietCheckin, quietText, type RoleNames, sortBoard, STAGES } from "./lib";
import type { Over } from "./yourCheckLimit";
import type { Note, QueueInfo, Task } from "./types";

// The Feature grid (mu-143): the board's second layout, beside the columns, which stay the
// default. Features (or goals) down the side, the stages across, small cards in the cells and Done
// as today's count. A second renderer over the board's own data: the same filtered tasks, the same
// grouping (levels.ts `groupItems`), the same sort. These are its pure parts (featureGridData.test.ts).

/** The board's layouts, remembered per project on this Mac; their names in the language shown (shell.json). */
export type Layout = "columns" | "grid";
export const LAYOUTS: { readonly id: Layout; readonly label: string }[] = (["columns", "grid"] as const).map((id) => ({
  id,
  get label() {
    return tn("shell", `app.layouts.${id}`);
  },
}));
type Store = { getItem: (k: string) => string | null; setItem: (k: string, v: string) => void };
/** Kept beside the project's saved views (filters.ts `viewsKey`); "all" is a project here too. */
export const layoutKey = (project: string) => `muster.layout.${project}`;

/** A project's layout: Columns unless it chose the grid, and Columns when storage can't say. */
export function loadLayout(store: Store, project: string): Layout {
  try {
    return store.getItem(layoutKey(project)) === "grid" ? "grid" : "columns";
  } catch {
    return "columns";
  }
}

/** Keeps a project's layout; false when storage refuses (it then lasts this session only). */
export function storeLayout(store: Store, project: string, layout: Layout): boolean {
  try {
    store.setItem(layoutKey(project), layout);
    return true;
  } catch {
    return false;
  }
}

/** The grid's stage columns, Backlog to Your check; Done is a count after them. */
export const GRID_STAGES = STAGES.filter((s) => s.id !== "done");
export type GridStage = (typeof GRID_STAGES)[number]["id"];

/** A cell's item: a task, or a backlog note (Backlog only). */
export type GridItem = BacklogItem;

/** One row: a feature or goal (`head`), or the tasks in none ("Other"), or every task. Progress
 *  is the core's for a feature or goal, its own tasks' for the rest. */
export type GridRow = {
  id: string;
  head: Task | null;
  title: string;
  progress: { done: number; total: number };
  cells: Record<GridStage, GridItem[]>;
  doneToday: number;
};

/** When a task was finished, as a day: its move into Done, else the day the log knows. */
const doneDay = (t: Pick<Task, "doneAt" | "doneOn">) => (t.doneAt || t.doneOn).slice(0, 10);

/** How many of `tasks` went into Done on `today` ("YYYY-MM-DD", local). */
export function doneTodayCount(tasks: Pick<Task, "stage" | "doneAt" | "doneOn">[], today: string): number {
  return tasks.filter((t) => t.stage === "done" && doneDay(t) === today).length;
}

/** A row's title, read when it's shown: App keeps the rows (`useMemo`), and Other, All tasks and
 *  "(no title)" follow a language switch. */
function rowOf(id: string, head: Task | null, title: () => string, items: Task[], notes: Note[], today: string, sort: BoardSort): GridRow {
  const cells = Object.fromEntries(GRID_STAGES.map((s) => [s.id, [] as Task[]])) as Record<GridStage, Task[]>;
  // A paused task (mu-59) keeps its stage, so it stays in its stage's cell, last: it's calm.
  const sorted = sortBoard(items, sort);
  for (const t of [...sorted.filter((t) => !t.paused), ...sorted.filter((t) => t.paused)]) {
    const c = boardColumn(t);
    if (c !== "done") cells[c].push(t);
  }
  const out = Object.fromEntries(GRID_STAGES.map((s) => [s.id, cells[s.id].map((task) => ({ kind: "task" as const, task }))])) as Record<GridStage, GridItem[]>;
  out.backlog = backlogColumn(notes, cells.backlog);
  const own = { done: items.filter((t) => t.stage === "done").length, total: items.length };
  const p = head?.progress;
  return {
    id,
    head,
    get title() {
      return title();
    },
    progress: p ? { done: p.done, total: p.total } : own,
    cells: out,
    doneToday: doneTodayCount(items, today),
  };
}

/** The grid's rows: with Group by Feature (or Goal), one per feature in flight, by project and id
 *  (a task not yet done, something done today, or no tasks yet: a new feature stays in view; or
 *  the feature itself matching the search), then
 *  Other with the tasks in none and the backlog notes; with No grouping, one row of everything.
 *  `work` is the board's tasks after the filters and the Who chips, `all` every item of the place
 *  shown (to find parents), `shown` what the filters let through (a feature lit by a search keeps
 *  its row). No rows when there's nothing at all. */
export function gridRows({ work, all, notes, by, today, sort = "priority", shown = work }: {
  work: Task[];
  all: Task[];
  notes: Note[];
  by: GroupBy;
  today: string;
  sort?: BoardSort;
  shown?: Task[];
}): GridRow[] {
  if (by === "none") return work.length || notes.length ? [rowOf("all", null, () => tn("board", "grid.allTasks"), work, notes, today, sort)] : [];
  const groups = groupItems(work, all, by, shown);
  const lit = new Set(shown.filter((t) => t.level === by).map((t) => `${t.queue}/${t.id}`));
  const rows: GridRow[] = [];
  for (const g of groups) {
    if (!g.head) continue;
    const inFlight = g.items.some((t) => t.stage !== "done") || doneTodayCount(g.items, today) > 0 || (g.head.progress?.total ?? 0) === 0 || lit.has(`${g.head.queue}/${g.head.id}`);
    const head = g.head;
    if (inFlight) rows.push(rowOf(`${head.queue}/${head.id}`, head, () => head.title || tn("board", "groups.noTitle"), g.items, [], today, sort));
  }
  const rest = groups.find((g) => !g.head)?.items ?? [];
  if (rest.length || notes.length) rows.push(rowOf("other", null, () => tn("board", "grid.other"), rest, notes, today, sort));
  return rows;
}

/** The most cards a cell shows before "+N more". */
export const CELL_CAP = 2;

/** A cell's cards: its first CELL_CAP, and how many more; all of them once it's expanded. */
export function cellShown<T>(items: T[], expanded: boolean): { shown: T[]; more: number } {
  if (expanded || items.length <= CELL_CAP) return { shown: items, more: 0 };
  return { shown: items.slice(0, CELL_CAP), more: items.length - CELL_CAP };
}

/** Whether Only what needs me dims a card: every task that doesn't wait on the owner, and every
 *  note; nothing while it's off. Dimmed, never hidden. */
export function dimmed(item: GridItem, mine: boolean): boolean {
  return mine && !(item.kind === "task" && needsYou(item.task));
}

/** The grid's columns as the off-edge cue counts them (rowCue.ts, as for the board's folded
 *  columns): each stage's items, how many of its tasks need the owner and how many are blocked,
 *  then Done with today's count. */
export function columnChips(rows: GridRow[]): RowChip[] {
  const stages = GRID_STAGES.map((s) => {
    const items = rows.flatMap((r) => r.cells[s.id]);
    const tasks = items.flatMap((i) => (i.kind === "task" ? [i.task] : []));
    return { id: s.id, label: s.label, count: items.length, needs: tasks.filter(needsYou).length, blocked: tasks.filter(isBlocked).length };
  });
  return [...stages, { id: "done", label: tl("stage.done"), count: rows.reduce((n, r) => n + r.doneToday, 0), needs: 0, blocked: 0 }];
}

/** The part of the grid's frame that shows the stage columns: the frame less the sticky feature
 *  names (`corner`), which sit at the start whichever way the window reads. */
export function viewSpan(frame: { left: number; right: number }, corner: { left: number; right: number }): { start: number; end: number } {
  return corner.left - frame.left <= frame.right - corner.right ? { start: corner.right, end: frame.right } : { start: frame.left, end: corner.left };
}

/** A small card's one status: why it waits (the block's own words, "After mu-99" for a task it
 *  depends on), needs you, paused, or who's on it with whether its session is live. */
export type CardStatusInfo =
  | { kind: "blocked" }
  | { kind: "you"; text: string }
  | { kind: "paused"; text: string }
  | { kind: "who"; text: string; live: boolean }
  | { kind: "none"; text: "" };

export function cardStatus(t: Task, roles: RoleNames): CardStatusInfo {
  if (isBlocked(t) && t.blockedKind) return { kind: "blocked" };
  if (needsYou(t)) return { kind: "you", text: tn("board", "grid.needsYou") };
  if (t.paused) return { kind: "paused", text: tn("board", "grid.paused") };
  const who = holder(t, roles);
  if (who && !(t.stage === "your-check" && who === "owner")) return { kind: "who", text: who === "owner" ? tn("board", "grid.you") : who, live: t.live };
  return { kind: "none", text: "" };
}

/** How many times the task was sent back into Building from a later stage. */
export function sentBackTimes(t: Pick<Task, "stageHistory">): number {
  return t.stageHistory.filter((h) => h.to === "building" && (h.from === "testing" || h.from === "your-check")).length;
}

/** "sent back once", "sent back twice", "sent back 3 times". */
function sentBackWords(n: number): string {
  if (n === 1) return tn("board", "grid.sentBackOnce");
  if (n === 2) return tn("board", "grid.sentBackTwice");
  return tn("board", "grid.sentBackTimes", { n });
}

/** The card's trouble, on its own line under the title: sent back (while it's back), a claim gone
 *  quiet, or a builder still holding Testing; "" when it's going well. */
export function troubleOf(t: Task, now: number): string {
  if (t.sentBack && t.stage === "building") return sentBackWords(Math.max(1, sentBackTimes(t)));
  const quiet = quietCheckin(t, now);
  if (quiet !== null) return quietText(quiet);
  if (needsTester(t)) return tn("board", "grid.needsTester");
  return "";
}

/** "a and b", "a, b and c": the last two joined by "and", the rest by the list separator. */
function andList(list: string[]): string {
  if (list.length <= 1) return list.join("");
  return tn("board", "grid.and", { a: list.slice(0, -1).join(tl("listSep")), b: list[list.length - 1] });
}

/** "a", "a and b", "a, b, c and 2 more". */
function names(list: string[]): string {
  if (list.length <= 3) return andList(list);
  return tn("board", "grid.andMore", { list: list.slice(0, 3).join(tl("listSep")), count: list.length - 3 });
}

/** A stage column's second line, who works there: the holders of its tasks by name (the owner as
 *  "you"), else the stage's role; Backlog says what it holds and Ready how many can start. */
export function stageWho(stage: GridStage, items: GridItem[], roles: RoleNames): string {
  const tasks = items.flatMap((i) => (i.kind === "task" ? [i.task] : []));
  if (stage === "backlog") return tn("board", "grid.backlogWho");
  if (stage === "ready") {
    const n = tasks.filter((t) => !isBlocked(t) && !t.paused && !isHuman(t)).length;
    return n ? tn("board", "grid.canStart", { count: n }) : tn("board", "grid.noneCanStart");
  }
  const you = tn("board", "grid.you");
  const who = [...new Set(tasks.map((t) => holder(t, roles)).filter(Boolean).map((w) => (w === "owner" ? you : w)))];
  return who.length ? names(who) : (STAGES.find((s) => s.id === stage)?.role ?? "");
}

/** The projects whose Your check is at or over its limit (0050), in the place shown: "full". */
export function checkFull(queues: QueueInfo[], queue: string): Over[] {
  return queues
    .filter((q) => queue === "all" || q.name === queue)
    .flatMap((q) => {
      const limit = q.yourCheckLimit;
      const waiting = q.yourCheck ?? 0;
      return typeof limit === "number" && waiting >= limit ? [{ name: q.name, label: q.label || q.name, waiting, limit }] : [];
    });
}

/** Your check's second line: "3 of 5", "5 of 5, full" in one project with a limit; in All
 *  projects how many are full; else whose it is. `full` turns it to the needs-you colour. */
export function checkHead(queues: QueueInfo[], queue: string): { text: string; full: boolean } {
  const full = checkFull(queues, queue);
  if (queue !== "all") {
    const q = queues.find((x) => x.name === queue);
    const limit = q?.yourCheckLimit;
    if (!q || typeof limit !== "number") return { text: tn("board", "grid.yoursToCheck"), full: false };
    return { text: tn("board", full.length ? "grid.checkFull" : "grid.checkOf", { n: q.yourCheck ?? 0, limit }), full: full.length > 0 };
  }
  if (!full.length) return { text: tn("board", "grid.yoursToCheck"), full: false };
  return { text: tn("board", "grid.fullIn", { count: full.length }), full: true };
}

/** The one alert line over the grid when Your check is full; "" when it isn't. */
export function alertLine(full: Over[], queue: string): string {
  if (!full.length) return "";
  if (queue !== "all") {
    const o = full[0];
    return tn("board", o.waiting > o.limit ? "grid.overLimit" : "grid.full", { n: o.waiting, limit: o.limit });
  }
  const list = andList(full.map((o) => tn("board", "grid.project", { label: o.label, n: o.waiting, limit: o.limit })));
  return tn("board", "grid.fullInList", { list });
}

/** "2 done today", "none done today": a row's Done cell and the header line say it alike. */
export const doneTodayWords = (n: number) => (n ? tn("board", "grid.doneToday", { count: n }) : tn("board", "grid.noneDoneToday"));

/** The header line: what waits for you (in its own part, to stand out), then what's being built,
 *  in testing and done today, each whole in words. `tasks` is the place's work (no features or
 *  goals), before the filters: the line says where the project stands. */
export function headerLine(tasks: Task[], needs: number, today: string): { needs: string; rest: string } {
  const live = (stage: string) => tasks.filter((t) => t.stage === stage && !t.paused).length;
  const building = live("building");
  const testing = live("testing");
  return {
    needs: needs ? tn("board", "grid.waiting", { count: needs }) : tn("board", "grid.nothingWaiting"),
    rest: [
      building ? tn("board", "grid.building", { count: building }) : tn("board", "grid.noneBuilding"),
      testing ? tn("board", "grid.testing", { count: testing }) : tn("board", "grid.noneTesting"),
      doneTodayWords(doneTodayCount(tasks, today)),
    ].join(tl("listSep")),
  };
}

/** A cell's place: the row (0 for the first feature) and the column (0 for Backlog, the last for Done). */
export type Cell = { r: number; c: number };

/** The grid's Tab stop as it's kept: the row by its id, so it stays on its feature when a poll adds
 *  or drops rows above it (review R10), and the column. */
export type ActiveCell = { row: string; c: number };

/** Where the Tab stop is now: its row's place, or the first row when that row is gone. */
export function activeCell(rows: GridRow[], active: ActiveCell): Cell {
  return { r: Math.max(0, rows.findIndex((x) => x.id === active.row)), c: active.c };
}

/** Where an arrow key, Home or End moves the focus from a cell, stopping at the edges; left and
 *  right swap in right-to-left. Null for any other key. */
export function moveCell(at: Cell, key: string, size: { rows: number; cols: number }, rtl: boolean): Cell | null {
  const clamp = (n: number, max: number) => Math.max(0, Math.min(max - 1, n));
  const forward = rtl ? "ArrowLeft" : "ArrowRight";
  const back = rtl ? "ArrowRight" : "ArrowLeft";
  switch (key) {
    case "ArrowUp": return { r: clamp(at.r - 1, size.rows), c: at.c };
    case "ArrowDown": return { r: clamp(at.r + 1, size.rows), c: at.c };
    case forward: return { r: at.r, c: clamp(at.c + 1, size.cols) };
    case back: return { r: at.r, c: clamp(at.c - 1, size.cols) };
    case "Home": return { r: at.r, c: 0 };
    case "End": return { r: at.r, c: size.cols - 1 };
    default: return null;
  }
}
