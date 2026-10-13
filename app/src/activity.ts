import { isBlocked } from "./blockWords";
import { needsYouCount } from "./human";
import { CHECKIN_MINUTES, splitHolder, stamp, stampMinutes } from "./lib";
import type { Activity, Task } from "./types";

// Which projects are being worked on now, and which have gone quiet (task 0037). Pure functions
// of the polled state and its clock (`now`, seconds since 1970), so the sidebar recomputes them
// every poll without reading anything else.

export type ActivityTask = Pick<Task, "stage" | "blocked" | "blockedKind" | "claimedBy" | "heldBy" | "live" | "heartbeat" | "checkins" | "stageHistory" | "stageSince" | "modified" | "paused">;

/** A project now: how many of its tasks are being worked on, whether its work in Building has
 *  gone quiet, and minutes since anything happened in it (null when nothing says). */
export type ProjectActivity = { working: number; stalled: boolean; last: number | null };

/** One sidebar row: a queue, its tasks and what's waiting on the owner in it. */
export type ProjectRow = { name: string; title: string; tasks: Task[]; needs: number; activity: ProjectActivity };

/** The All projects row: projects being worked on, projects stalled, the newest activity. */
export type Sums = { working: number; stalled: number; last: number | null };

/** The clock as wall-clock minutes (lib.ts stampMinutes), to count the tools' local stamps
 *  against the way quietMinutes does. Null for a clock that isn't a date. */
const wallMinutes = (now: number) => stampMinutes(stamp(new Date(now * 1000)));

/** Minutes from a stamp to the wall clock; null when it isn't a stamp. Ahead of the clock is now. */
function sinceStamp(at: string, wall: number | null): number | null {
  const m = stampMinutes(at);
  return m === null || wall === null ? null : Math.max(0, wall - m);
}

/** Whole minutes from seconds since 1970 to `now`; null for 0 (not known). */
const sinceSecs = (at: number, now: number): number | null => (at > 0 ? Math.floor(Math.max(0, now - at) / 60) : null);

const newest = (ms: (number | null)[]): number | null => ms.reduce<number | null>((a, m) => (m === null ? a : a === null ? m : Math.min(a, m)), null);

/** Minutes since a check-in or the heartbeat (a check-in renews it), the newest. */
function sinceBeat(t: ActivityTask, wall: number | null): number | null {
  return newest([sinceStamp(t.heartbeat, wall), ...t.checkins.map((c) => sinceStamp(c.at, wall))]);
}

function lastAt(t: ActivityTask, now: number, wall: number | null): number | null {
  if (t.live) return 0;
  return newest([
    sinceBeat(t, wall),
    sinceStamp(t.stageSince, wall),
    ...t.stageHistory.map((h) => sinceStamp(h.at, wall)),
    sinceSecs(t.modified, now),
  ]);
}

/** Minutes since a task was last worked on: the newest of a stage change, a check-in, the
 *  heartbeat and its file changing; 0 while a session on it is live. Null when nothing says. */
export function lastMinutes(t: ActivityTask, now: number): number | null {
  return lastAt(t, now, wallMinutes(now));
}

function workingAt(t: ActivityTask, wall: number | null): boolean {
  if (t.stage === "done") return false;
  if (t.live) return true;
  // A heartbeat or check-in counts only while someone holds the task and it isn't waiting: in
  // Building with a claim, or in Testing with a holder. A task handed back to Ready, blocked (by
  // tasks too, 0024), or in Your check keeps its last heartbeat, which isn't anyone working
  // (review, 2026-10-13).
  const held = !isBlocked(t) && ((t.stage === "building" && t.claimedBy.trim() !== "") || (t.stage === "testing" && (t.heldBy ?? "").trim() !== ""));
  if (!held) return false;
  const m = sinceBeat(t, wall);
  return m !== null && m <= CHECKIN_MINUTES;
}

/** A task someone is on right now: a live session (0025), or a check-in or heartbeat in the last
 *  30 minutes, the interval sessions check in at (0030). Never in Done (the core's live rule), and
 *  in Your check only while a session is still live. */
export function isWorking(t: ActivityTask, now: number): boolean {
  return workingAt(t, wallMinutes(now));
}

/** Work in Building that should be moving: held by a session, not waiting (blocked, by tasks
 *  too), not paused (0045), not the owner's own (the owner doesn't check in). lib.ts
 *  quietCheckin's rule for a card. */
const activeClaim = (t: ActivityTask) => t.stage === "building" && !isBlocked(t) && !t.paused && splitHolder(t.claimedBy.trim()).label !== "owner";

/** A project's activity from its tasks and its queue's newest commit (seconds, 0 for none).
 *  Stalled: work in Building and nothing at all in the project for over 30 minutes. */
export function projectActivity(tasks: ActivityTask[], commit: number, now: number): ProjectActivity {
  const wall = wallMinutes(now);
  let working = 0;
  let claims = false;
  const times: (number | null)[] = [sinceSecs(commit, now)];
  for (const t of tasks) {
    if (workingAt(t, wall)) working++;
    if (activeClaim(t)) claims = true;
    times.push(lastAt(t, now, wall));
  }
  const last = newest(times);
  return { working, stalled: claims && (last === null || last > CHECKIN_MINUTES), last };
}

/** The sidebar's rows, one per queue in the core's order, from one pass over the state. */
export function projectRows(queues: { name: string; path: string }[], tasks: Task[], activity: Activity[], now: number): ProjectRow[] {
  const byQueue = new Map<string, Task[]>();
  for (const t of tasks) {
    const list = byQueue.get(t.queue);
    if (list) list.push(t);
    else byQueue.set(t.queue, [t]);
  }
  const commits = new Map<string, number>();
  for (const a of activity) if (a.source === "commit" && a.at > (commits.get(a.queue) ?? 0)) commits.set(a.queue, a.at);
  return queues.map((q) => {
    const own = byQueue.get(q.name) ?? [];
    return {
      name: q.name,
      title: q.path,
      tasks: own,
      needs: needsYouCount(own),
      activity: projectActivity(own, commits.get(q.name) ?? 0, now),
    };
  });
}

/** The All projects row: how many projects are being worked on, how many stalled, the newest activity. */
export function sumActivity(list: ProjectActivity[]): Sums {
  return {
    working: list.filter((a) => a.working > 0).length,
    stalled: list.filter((a) => a.stalled).length,
    last: newest(list.map((a) => a.last)),
  };
}

/** "now", "12 min ago", "3 h ago", "yesterday", "5 days ago"; "no activity" when nothing says.
 *  Days are calendar days on the wall clock (`wall`, wall-clock minutes): "yesterday" is the day
 *  before today, not 24 to 48 hours ago. */
export function lastText(m: number | null, wall: number | null): string {
  if (m === null) return "no activity";
  if (m < 1) return "now";
  if (m < 60) return `${m} min ago`;
  const days = wall === null ? Math.floor(m / 1440) : Math.floor(wall / 1440) - Math.floor((wall - m) / 1440);
  if (days <= 0) return `${Math.floor(m / 60)} h ago`;
  if (days === 1) return "yesterday";
  return `${days} days ago`;
}

/** `lastText` at the state's clock (`now`, seconds since 1970). */
export const lastLabel = (m: number | null, now: number) => lastText(m, wallMinutes(now));

export type SidebarOrder = "working" | "az" | "needs";
/** The sidebar's orders, as chips above the list. Working first is the default. */
export const SIDEBAR_ORDERS: { id: SidebarOrder; label: string; title: string }[] = [
  { id: "working", label: "Working", title: "Projects being worked on now first, then the most recently worked on" },
  { id: "az", label: "A–Z", title: "By name" },
  { id: "needs", label: "Needs you", title: "The most waiting on you first, then the ones being worked on" },
];

type Ranked = Pick<ProjectRow, "name" | "needs" | "activity">;
// Plain code-unit order after folding case, so the order is the same on every machine.
const cmp = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);
const byName = (a: Ranked, b: Ranked) => cmp(a.name.toLowerCase(), b.name.toLowerCase()) || cmp(a.name, b.name);
const byLast = (a: Ranked, b: Ranked) => {
  const x = a.activity.last, y = b.activity.last;
  return x === y ? 0 : x === null ? 1 : y === null ? -1 : x - y;
};
const byWorking = (a: Ranked, b: Ranked) => Number(b.activity.working > 0) - Number(a.activity.working > 0) || byLast(a, b) || byName(a, b);
const COMPARE: Record<SidebarOrder, (a: Ranked, b: Ranked) => number> = {
  working: byWorking,
  az: byName,
  needs: (a, b) => b.needs - a.needs || byWorking(a, b),
};

/** A sorted copy. */
export function orderProjects<T extends Ranked>(rows: T[], by: SidebarOrder): T[] {
  return [...rows].sort(COMPARE[by]);
}

/** The order the sidebar shows, kept between polls so rows don't jump (docs/DESIGN.md: nothing
 *  jumps; review, 2026-10-13): it's worked out again only when what defines it changes: the
 *  order chosen, the projects listed, which projects are working, and, in Needs you order, the
 *  counts. Activity minutes ticking on don't move a row. */
export type OrderMemo = { by: SidebarOrder; names: string[]; key: string };
export function nextOrder(prev: OrderMemo | null, rows: (Ranked & { activity: ProjectActivity })[], by: SidebarOrder): OrderMemo {
  const names = rows.map((r) => r.name).sort();
  const working = rows.filter((r) => r.activity.working > 0).map((r) => r.name).sort();
  const needs = by === "needs" ? rows.map((r) => `${r.name}:${r.needs}`).sort() : [];
  const key = JSON.stringify([by, names, working, needs]);
  if (prev && prev.key === key) return prev;
  return { by, names: orderProjects(rows, by).map((r) => r.name), key };
}

export const ORDER_KEY = "muster.sidebarOrder";
const isOrder = (v: unknown): v is SidebarOrder => SIDEBAR_ORDERS.some((o) => o.id === v);

/** The remembered order, read safely: storage that's off, denied or absent (a test run) and
 *  anything unknown give the default, Working first. */
export function loadOrder(read: () => string | null = () => localStorage.getItem(ORDER_KEY)): SidebarOrder {
  try {
    const v = read();
    return isOrder(v) ? v : "working";
  } catch {
    return "working";
  }
}

/** Remembers the order; false when storage refused (full, off), and the order holds for now. */
export function storeOrder(o: SidebarOrder, write: (v: string) => void = (v) => localStorage.setItem(ORDER_KEY, v)): boolean {
  try {
    write(o);
    return true;
  } catch {
    return false;
  }
}
