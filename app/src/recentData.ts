import { isHuman } from "./human";
import { type RoleNames, stageHolders, stamp, stampMinutes, whoLine } from "./lib";
import { isWork } from "./levels";
import type { Task } from "./types";

// Recently finished (task 0051), the parts without React: what landed on main and what the owner
// accepted, newest first, from the stage log; when, in words; the day groups and the window.
// Pure functions of the tasks and the state's clock (`now`, seconds since 1970), tested in
// recentData.test.ts with a fake clock.

/** Merged: Testing passed and the work landed on main (Testing → Your check). Accepted: moved
 *  into Done (the owner's Accept; for the owner's own task, their Done). */
export type FinishKind = "merged" | "accepted";
/** One row: a task, what happened and when (a tool stamp, "YYYY-MM-DD HH:MM"), with the stamp as
 *  wall-clock minutes for ordering and grouping. */
export type Finish = { task: Task; kind: FinishKind; at: string; minutes: number };

/** A task's rows: its latest merge while it's still in Your check or Done, and its move into
 *  Done (`doneAt`, which also reads an older hand move) while it's still there. Goals and features
 *  finish with their tasks, so they have none; the owner's own task is never merged. */
function rowsOf(t: Task): Finish[] {
  if (!isWork(t)) return [];
  const out: Finish[] = [];
  const add = (kind: FinishKind, at: string) => {
    const minutes = stampMinutes(at);
    if (minutes !== null) out.push({ task: t, kind, at, minutes });
  };
  if (t.stage === "done") add("accepted", t.doneAt);
  if (!isHuman(t) && (t.stage === "your-check" || t.stage === "done")) {
    const merge = [...t.stageHistory].reverse().find((s) => s.from === "testing" && s.to === "your-check");
    if (merge) add("merged", merge.at);
  }
  return out;
}

// Plain code-unit order, like the board's, so the order is the same on every machine.
const cmp = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);

/** Every merged and accepted row of `tasks`, newest first; in the same minute, accepted before
 *  merged (it came after), then project and id. */
export function finishes(tasks: Task[]): Finish[] {
  return tasks
    .flatMap(rowsOf)
    .sort((a, b) => b.minutes - a.minutes || cmp(a.kind, b.kind) || cmp(a.task.queue, b.task.queue) || cmp(a.task.id, b.task.id));
}

export type KindFilter = "all" | FinishKind;
/** The kind chips above the list. */
export const KIND_FILTERS: { id: KindFilter; label: string; title: string }[] = [
  { id: "all", label: "All", title: "What landed on main and what you accepted" },
  { id: "merged", label: "Merged", title: "Testing passed and the work landed on main" },
  { id: "accepted", label: "Accepted", title: "Moved into Done: your Accept, or your own task done" },
];
export const RECENT_KEY = "muster.recentKind";
const isKind = (v: unknown): v is KindFilter => KIND_FILTERS.some((k) => k.id === v);

export const byKind = (rows: Finish[], f: KindFilter) => (f === "all" ? rows : rows.filter((r) => r.kind === f));

/** The remembered chip, read safely: storage that's off, denied or absent (a test run) and
 *  anything unknown give All. */
export function loadKind(read: () => string | null = () => localStorage.getItem(RECENT_KEY)): KindFilter {
  try {
    const v = read();
    return isKind(v) ? v : "all";
  } catch {
    return "all";
  }
}

/** Remembers the chip; false when storage refused (full, off), and the choice holds for now. */
export function storeKind(k: KindFilter, write: (v: string) => void = (v) => localStorage.setItem(RECENT_KEY, v)): boolean {
  try {
    write(k);
    return true;
  } catch {
    return false;
  }
}

const DAY = 1440;
const WEEKDAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/** The clock as wall-clock minutes, the count stamps are in (lib.ts stampMinutes). */
const wallNow = (now: number) => stampMinutes(stamp(new Date(now * 1000)));

/** A day (whole days since 1970 on the wall clock) as "Sun 11 Oct", with the year when it isn't
 *  `year`. Fixed English words, so it reads the same on every machine. */
function dateText(day: number, year: number): string {
  const d = new Date(day * DAY * 60000);
  const y = d.getUTCFullYear();
  return `${WEEKDAYS[d.getUTCDay()]} ${d.getUTCDate()} ${MONTHS[d.getUTCMonth()]}${y === year ? "" : ` ${y}`}`;
}
const yearOf = (wall: number) => new Date(wall * 60000).getUTCFullYear();

/** "just now", "4 min ago", "2 h ago" today, "yesterday 18:20", then "Sun 11 Oct 09:05" (with the
 *  year when it isn't this one). Days are calendar days on the wall clock. "" for anything that
 *  isn't a stamp; a stamp ahead of the clock is just now. */
export function whenText(at: string, now: number): string {
  const from = stampMinutes(at);
  const wall = wallNow(now);
  if (from === null || wall === null) return "";
  const m = Math.max(0, wall - from);
  if (m < 1) return "just now";
  if (m < 60) return `${m} min ago`;
  const days = Math.floor(wall / DAY) - Math.floor(from / DAY);
  if (days <= 0) return `${Math.floor(m / 60)} h ago`;
  const time = at.slice(11);
  if (days === 1) return `yesterday ${time}`;
  return `${dateText(Math.floor(from / DAY), yearOf(wall))} ${time}`;
}

/** A Done card's finish: "done 4 min ago" from its `doneAt`; else "done today", "done yesterday"
 *  or "done Thu 1 Oct" from the day it knows (`doneOn`); "" when it isn't in Done or nothing says. */
export function finishText(t: Pick<Task, "stage" | "doneAt" | "doneOn">, now: number): string {
  if (t.stage !== "done") return "";
  const when = whenText(t.doneAt, now);
  if (when) return `done ${when}`;
  const day = stampMinutes(`${t.doneOn} 00:00`);
  const wall = wallNow(now);
  if (day === null || wall === null) return "";
  const days = Math.floor(wall / DAY) - day / DAY;
  return `done ${days <= 0 ? "today" : days === 1 ? "yesterday" : dateText(day / DAY, yearOf(wall))}`;
}

/** One day of the list: "Today", "Yesterday" or "Wed 7 Oct", and its rows, newest first. */
export type DayGroup = { day: number; label: string; rows: Finish[] };

/** The rows of the last `weeks` × 7 calendar days, today included, grouped by day, and how many
 *  older rows there are beyond them. A stamp ahead of the clock counts as today. */
export function dayGroups(rows: Finish[], now: number, weeks: number): { groups: DayGroup[]; older: number } {
  const wall = wallNow(now);
  if (wall === null) return { groups: [], older: 0 };
  const today = Math.floor(wall / DAY);
  const first = today - weeks * 7 + 1;
  const groups: DayGroup[] = [];
  let older = 0;
  for (const r of rows) {
    const day = Math.min(today, Math.floor(r.minutes / DAY));
    if (day < first) {
      older++;
      continue;
    }
    const last = groups.at(-1);
    if (last && last.day === day) last.rows.push(r);
    else groups.push({ day, label: day === today ? "Today" : day === today - 1 ? "Yesterday" : dateText(day, yearOf(wall)), rows: [r] });
  }
  return { groups, older };
}

/** How many rows Recently finished's first week shows with its kind filter: its count in the row
 *  of folded columns, and whether it has anything to show (mu-84), the same rows its list draws. */
export function weekCount(tasks: Task[], now: number, kind: KindFilter): number {
  return dayGroups(byKind(finishes(tasks), kind), now, 1).groups.reduce((n, g) => n + g.rows.length, 0);
}

/** Who built it: the session that took it into Building, short ("muster/wt-0051 · main ·
 *  531266d4", a role slot by its role's name now); "you" for the owner's own task; "" when the
 *  log doesn't say. */
export function builtBy(t: Task, names: RoleNames = {}): string {
  if (isHuman(t)) return "you";
  const who = stageHolders(t).find((h) => h.stage === "building")?.who ?? "";
  return who ? whoLine({ stage: "building", claimedBy: who, heldBy: "" }, names) : "";
}
