import type { Task } from "./types";

// Pure helpers for the Table and Timeline views (tested in viewData.test.ts).

export type SortKey = "queue" | "id" | "title" | "kind" | "priority" | "status" | "who" | "estimate" | "captured" | "changed";
const PRIORITY_RANK: Record<string, number> = { high: 0, normal: 1, low: 2 };
const STATUS_RANK: Record<string, number> = { inbox: 0, todo: 1, doing: 2, blocked: 3, done: 4 };

export function sortTasks(tasks: Task[], by: SortKey, desc: boolean): Task[] {
  const val = (t: Task): string | number => {
    switch (by) {
      case "priority": return PRIORITY_RANK[t.priority || "normal"] ?? 1;
      case "status": return STATUS_RANK[t.status] ?? 9;
      case "changed": return t.modified;
      case "estimate": return t.estMinutes * 1e7 + t.estTokenCount;
      case "who": return t.claimedBy;
      case "kind": return t.kind;
      default: return t[by];
    }
  };
  const out = [...tasks].sort((a, b) => {
    const x = val(a), y = val(b);
    const c = typeof x === "number" && typeof y === "number" ? x - y : String(x).localeCompare(String(y));
    return c || a.queue.localeCompare(b.queue) || a.id.localeCompare(b.id);
  });
  return desc ? out.reverse() : out;
}

/** Days since 1970 for "YYYY-MM-DD" (UTC), or null. */
export function dayNumber(date: string): number | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(date);
  if (!m) return null;
  return Math.floor(Date.UTC(+m[1], +m[2] - 1, +m[3]) / 86400000);
}
export function dayLabel(day: number): string {
  const d = new Date(day * 86400000);
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric", timeZone: "UTC" });
}

export type Bar = { task: Task; start: number; work: number | null; end: number; open: boolean };

/** One bar per task: captured → done (or today), with the working part from the claim day. */
export function bars(tasks: Task[], today: number): { bars: Bar[]; from: number; to: number } {
  const out: Bar[] = [];
  for (const t of tasks) {
    const start = dayNumber(t.captured) ?? Math.floor(t.modified / 86400);
    const done = dayNumber(t.doneOn);
    const end = Math.max(start, t.status === "done" && done !== null ? done : today);
    const work = dayNumber(t.claimedOn);
    out.push({ task: t, start, work: work !== null && work >= start && work <= end ? work : null, end, open: t.status !== "done" });
  }
  out.sort((a, b) => a.start - b.start || a.task.queue.localeCompare(b.task.queue) || a.task.id.localeCompare(b.task.id));
  const from = out.length ? Math.min(...out.map((b) => b.start)) : today;
  const to = Math.max(today, ...out.map((b) => b.end));
  return { bars: out, from, to };
}
