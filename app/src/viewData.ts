import { blockSentence, isBlocked } from "./blockWords";
import { holder, stageLabel, stageOf, stageRank, stampMinutes, type StageId } from "./lib";
import type { Task } from "./types";
import { isWork, shownStageRank } from "./levels";

// Pure helpers for the Table and Timeline views (tested in viewData.test.ts).

export type SortKey = "queue" | "id" | "title" | "kind" | "priority" | "stage" | "who" | "estimate" | "captured" | "changed";
const PRIORITY_RANK: Record<string, number> = { high: 0, normal: 1, low: 2 };

const SORT_KEYS: SortKey[] = ["queue", "id", "title", "kind", "priority", "stage", "who", "estimate", "captured", "changed"];
/** The Table's saved sort (owner, 2026-10-13: "tables should save what I sorted it last"), read
 *  back safely: anything unknown is the default, Stage ascending. */
export function parseTableSort(raw: string | null): { by: SortKey; desc: boolean } {
  try {
    const v = JSON.parse(raw ?? "") as { by?: unknown; desc?: unknown };
    if (SORT_KEYS.includes(v.by as SortKey)) return { by: v.by as SortKey, desc: v.desc === true };
  } catch {
    /* not saved yet, or not ours: the default */
  }
  return { by: "stage", desc: false };
}
export const TABLE_SORT_KEY = "muster.tableSort";

export function sortTasks(tasks: Task[], by: SortKey, desc: boolean): Task[] {
  const val = (t: Task): string | number => {
    switch (by) {
      case "priority": return PRIORITY_RANK[t.priority || "normal"] ?? 1;
      // A goal or feature sorts by the stage it shows (from its children).
      case "stage": return isWork(t) ? stageRank(t.stage) : shownStageRank(t);
      case "changed": return t.modified;
      case "estimate": return (t.estMinutes + t.estTestMinutes) * 1e7 + t.estTokenCount;
      case "who": return isBlocked(t) ? blockSentence(t, true) : holder(t);
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

// ---- Timeline (0031) ------------------------------------------------------------------------
// Every time here is in wall-clock minutes: a local stamp ("2026-10-12 14:05", as the tools write
// them) read as if it were UTC (lib.stampMinutes), and "now" converted the same way
// (wallMinutes). Every day is then exactly 1440 minutes, so day, week and month windows, hour
// slots and durations come out the same on either side of a clock change, and nothing depends
// on the machine's time zone. The price: a span across a clock change is off by that hour of
// real time, which a timeline drawn in wall-clock hours doesn't show anyway.

export const DAY = 1440;
export type Scale = "day" | "week" | "month";
/** The scales in the order the chips show them. `slot` is the smallest unit a bar snaps to:
 *  hours at Day (bars are exact there), quarter days at Week, days at Month. */
export const SCALES: { id: Scale; label: string; slot: number }[] = [
  { id: "day", label: "Day", slot: 60 },
  { id: "week", label: "Week", slot: 360 },
  { id: "month", label: "Month", slot: DAY },
];
const slotOf = (scale: Scale) => SCALES.find((s) => s.id === scale)?.slot ?? DAY;
/** A saved scale, or Week (the default) for anything else. */
export const parseScale = (v: string | null): Scale => (v === "day" || v === "month" ? v : "week");

/** A Date's local wall clock as minutes, the same count stampMinutes gives its stamp. */
export function wallMinutes(d: Date): number {
  return Date.UTC(d.getFullYear(), d.getMonth(), d.getDate(), d.getHours(), d.getMinutes()) / 60000;
}

/** A stretch of a task's bar: waiting (captured → first claim, in a quiet tint) or a stage it
 *  was in. `fuzzy` when its start is only known to the day. */
export type Segment = { stage: StageId | "waiting"; from: number; to: number; fuzzy: boolean };
/** A task on the timeline: from `start` to `end`, the working part from `work` (= `end` when it
 *  never left waiting). `timed` when it comes from the stage history, else from dates only. */
export type Span = {
  task: Task; start: number; work: number; end: number; open: boolean; timed: boolean; segments: Segment[]; blocks: { from: number; to: number; kind: string }[];
  /** Paused stretches (0045), drawn hatched grey: time paused isn't time worked. */
  pauses: { from: number; to: number }[];
  /** When the work finished (into Your check or Done); `end` when it's still going. */
  finish: number;
};

const WORKING = new Set<string>(["building", "testing", "your-check"]);
const dayStart = (date: string) => {
  const d = dayNumber(date);
  return d === null ? null : d * DAY;
};

/** One task's work on the timeline at wall-clock minute `now` (owner, 2026-10-13: "hour based,
 *  each task when started and finished"): from when work started (its first move into Building,
 *  Testing, Your check or Done) to when it finished (into Your check or Done), one segment per
 *  stage it held; while it waits in Your check, that segment runs on to now as a quiet tail
 *  (drawn thin). Time before the start isn't drawn. A task done before stage history was logged
 *  runs from its logged capture to its logged done time. Null when work hasn't started, or the
 *  log has no times for it. */
export function taskSpan(t: Task, now: number): Span | null {
  const steps = t.stageHistory
    .flatMap((s) => {
      const at = stampMinutes(s.at);
      return at === null ? [] : [{ at, from: stageOf({ stage: s.from }), to: stageOf({ stage: s.to }) }];
    })
    .sort((a, b) => a.at - b.at);
  const open = t.stage !== "done";
  const segments: Segment[] = [];
  const push = (stage: Segment["stage"], from: number, to: number, fuzzy: boolean) => {
    if (to > from) segments.push({ stage, from, to, fuzzy });
  };
  const exactCapture = t.capturedAt ? stampMinutes(t.capturedAt) : null;
  const doneAt = t.doneAt ? stampMinutes(t.doneAt) : null;
  let start: number, end: number;
  const first = steps.findIndex((s) => WORKING.has(s.to) || s.to === "done");
  if (steps.length && WORKING.has(steps[0].from)) {
    // A history that starts mid-work (claimed by hand before the tools logged stages): from the
    // claim day when that fits (fuzzy), else from the first logged step.
    const claim = dayStart(t.claimedOn);
    start = claim !== null && claim <= steps[0].at ? claim : steps[0].at;
    let cur: StageId = steps[0].from as StageId, since = start, fuzzy = start !== steps[0].at;
    for (const s of steps) {
      if (cur !== "done") push(cur, since, s.at, fuzzy);
      [cur, since, fuzzy] = [s.to as StageId, s.at, false];
    }
    end = open ? Math.max(now, since) : since;
    if (open) push(cur, since, end, false);
  } else if (first >= 0) {
    start = steps[first].at;
    let cur: StageId = steps[first].to as StageId, since = start;
    for (const s of steps.slice(first + 1)) {
      if (cur !== "done") push(cur, since, s.at, false);
      [cur, since] = [s.to as StageId, s.at];
    }
    end = open ? Math.max(now, since) : since;
    if (open) push(cur, since, end, false);
  } else if (!open && exactCapture !== null && doneAt !== null && doneAt >= exactCapture) {
    // Done before stages were logged: captured and done, both to the minute.
    [start, end] = [exactCapture, doneAt];
    push("done", start, Math.max(end, start + 1), false);
    end = Math.max(end, start + 1);
  } else {
    return null;
  }
  // Finished: the last move into Your check or Done (the work is done; what follows is the tail).
  const finish = [...steps].reverse().find((s) => s.to === "your-check" || s.to === "done")?.at ?? end;
  // Blocked stretches, clipped to the bar; one still open runs to its end.
  const blocks = (t.blockHistory ?? []).flatMap((b) => {
    const from = stampMinutes(b.from);
    const to = b.to ? stampMinutes(b.to) : end;
    if (from === null || to === null) return [];
    const a = Math.max(from, start), z = Math.min(to, end);
    return z > a ? [{ from: a, to: z, kind: b.kind ?? "" }] : [];
  });
  // Paused stretches (0045), clipped the same way.
  const pauses = (t.pauseHistory ?? []).flatMap((p) => {
    const from = stampMinutes(p.from);
    const to = p.to ? stampMinutes(p.to) : end;
    if (from === null || to === null) return [];
    const a = Math.max(from, start), z = Math.min(to, end);
    return z > a ? [{ from: a, to: z }] : [];
  });
  return { task: t, start, work: start, end, open, timed: steps.length > 0, segments, blocks, pauses, finish };
}

/** A timeline bar's tooltip: the stage, that it's paused and why (0045), why it's blocked (0024),
 *  and its days. */
export function barTitle(t: Task, timed: boolean): string {
  const why = blockSentence(t);
  const paused = t.paused ? ` · paused${t.pausedWhy ? `: ${t.pausedWhy}` : ""}` : "";
  return `${stageLabel(t.stage)}${paused}${why ? ` · ${why}` : ""} · captured ${t.captured}${t.claimedOn ? ` · started ${t.claimedOn}` : ""}${t.doneOn ? ` · done ${t.doneOn}` : ""}${timed ? "" : " · dates only"}`;
}

/** The tasks with work to show, earliest start first, and how many have none yet (not started,
 *  or no times in the log). */
export function spans(tasks: Task[], now: number): Span[] & { notStarted?: number } {
  const out: Span[] & { notStarted?: number } = [];
  let none = 0;
  for (const t of tasks) {
    const s = taskSpan(t, now);
    if (s) out.push(s);
    else none++;
  }
  out.sort((a, b) => a.start - b.start || a.task.queue.localeCompare(b.task.queue) || a.task.id.localeCompare(b.task.id));
  out.notStarted = none;
  return out;
}

export type TimeWindow = { from: number; to: number };

/** The window `offset` days, weeks (Monday to Sunday) or months away from the one holding `now`. */
export function timeWindow(scale: Scale, now: number, offset: number): TimeWindow {
  const day = Math.floor(now / DAY);
  // Day is the last 24 hours, not the calendar day (owner, 2026-10-13: just after midnight the
  // calendar day was nearly empty): from 20 hours before the current hour to 4 hours after it,
  // so now sits near the right with room ahead; ◀ ▶ move by 24 hours.
  if (scale === "day") {
    const hour = Math.floor(now / 60) * 60 + offset * DAY;
    return { from: hour - 20 * 60, to: hour + 4 * 60 };
  }
  const date = new Date(day * 86400000);
  if (scale === "week") {
    const monday = day - ((date.getUTCDay() + 6) % 7) + offset * 7;
    return { from: monday * DAY, to: (monday + 7) * DAY };
  }
  const y = date.getUTCFullYear(), m = date.getUTCMonth() + offset;
  return { from: Date.UTC(y, m, 1) / 60000, to: Date.UTC(y, m + 1, 1) / 60000 };
}

// English names, like the rest of the app's words; no locale dependence.
const WEEKDAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const dateOf = (m: number) => new Date(Math.floor(m / DAY) * 86400000);
const pad2 = (n: number) => String(n).padStart(2, "0");
const clock = (m: number) => `${pad2(Math.floor((m % DAY) / 60))}:${pad2(m % 60)}`;
const shortDate = (m: number) => {
  const d = dateOf(m);
  return `${d.getUTCDate()} ${MONTHS[d.getUTCMonth()].slice(0, 3)}`;
};

/** "Wed 14 Oct 2026", "12 – 18 Oct 2026", "28 Sep – 4 Oct 2026", "October 2026". */
export function windowLabel(scale: Scale, w: TimeWindow): string {
  const a = dateOf(w.from), b = dateOf(w.to - DAY);
  if (scale === "month") return `${MONTHS[a.getUTCMonth()]} ${a.getUTCFullYear()}`;
  if (scale === "day") {
    const at = (m: number) => `${WEEKDAYS[dateOf(m).getUTCDay()]} ${shortDate(m)} ${clock(m)}`;
    return `${at(w.from)} – ${at(w.to)}`;
  }
  const end = `${shortDate(w.to - DAY)} ${b.getUTCFullYear()}`;
  if (a.getUTCFullYear() !== b.getUTCFullYear()) return `${shortDate(w.from)} ${a.getUTCFullYear()} – ${end}`;
  return a.getUTCMonth() === b.getUTCMonth() ? `${a.getUTCDate()} – ${end}` : `${shortDate(w.from)} – ${end}`;
}

/** One axis label: where it sits (fractions of the window) and whether now is in it. */
export type Tick = { at: number; left: number; width: number; label: string; current: boolean };

/** Hours (00–23) at Day, weekday and date at Week, the date at Month. */
export function axis(scale: Scale, w: TimeWindow, now: number): Tick[] {
  const step = scale === "day" ? 60 : DAY;
  const len = w.to - w.from;
  const out: Tick[] = [];
  for (let at = w.from; at < w.to; at += step) {
    const d = dateOf(at);
    // Day: the clock hour, and the weekday at midnight so the change of day shows.
    const hour = Math.floor((((at % DAY) + DAY) % DAY) / 60);
    const label = scale === "day" ? (hour === 0 ? WEEKDAYS[d.getUTCDay()] : pad2(hour)) : scale === "week" ? `${WEEKDAYS[d.getUTCDay()]} ${d.getUTCDate()}` : String(d.getUTCDate());
    out.push({ at, left: (at - w.from) / len, width: step / len, label, current: now >= at && now < at + step });
  }
  return out;
}

/** Where the now line sits in the window, as a fraction, or null when now is outside it. */
export function nowAt(w: TimeWindow, now: number): number | null {
  return now >= w.from && now < w.to ? (now - w.from) / (w.to - w.from) : null;
}

/** "Building 14 Oct 13:00 – 15:00"; dates only where that's all that's known. */
export function segmentTitle(g: Segment): string {
  const name = g.stage === "waiting" ? "Waiting" : stageLabel(g.stage);
  const from = g.fuzzy ? shortDate(g.from) : `${shortDate(g.from)} ${clock(g.from)}`;
  let to: string;
  if (g.fuzzy && g.to % DAY === 0) to = shortDate(g.to - DAY);
  else if (!g.fuzzy && Math.floor(g.to / DAY) === Math.floor(g.from / DAY)) to = clock(g.to);
  else to = `${shortDate(g.to)} ${clock(g.to)}`;
  return `${name} ${from} – ${to}`;
}

/** A bar placed in a window: `left` and `width` as fractions of the window, its segments as
 *  fractions of the bar. `clipStart`/`clipEnd` when it runs on past the window's edge. */
export type Placed = { span: Span; left: number; width: number; clipStart: boolean; clipEnd: boolean; segments: { seg: Segment; left: number; width: number }[]; blocks: { left: number; width: number; kind: string }[]; pauses: { left: number; width: number }[] };

/** Linear map of `m` from [a, b] onto [da, db]; the end when the range is empty. */
const lerp = (a: number, b: number, da: number, db: number, m: number) => (m >= b ? db : da + ((m - a) * (db - da)) / (b - a));

/** The spans that show in window `w` at `scale`, in order. At Day a bar is exact (at least a
 *  quarter hour wide); at Week and Month its waiting part and its working part each snap out to
 *  whole slots, so a 3-hour task fills one quarter-day at Week and one day at Month, and the
 *  stages inside keep their proportions. */
export function layout(list: Span[], w: TimeWindow, scale: Scale): Placed[] {
  const slot = slotOf(scale);
  const len = w.to - w.from;
  const snap = scale !== "day";
  const down = (m: number) => (snap ? Math.floor(m / slot) * slot : m);
  const up = (m: number) => (snap ? Math.ceil(m / slot) * slot : m);
  const minWidth = (snap ? slot : 15) / len;
  const out: Placed[] = [];
  for (const s of list) {
    const d0 = down(s.start);
    const d2 = Math.max(d0, up(s.end));
    const d1 = s.work < s.end ? Math.max(d0, down(s.work)) : d2;
    const shows = d2 > d0 ? d2 > w.from && d0 < w.to : d0 >= w.from && d0 < w.to;
    if (!shows) continue;
    const map = (m: number) => (m >= s.work ? lerp(s.work, s.end, d1, d2, m) : lerp(s.start, s.work, d0, d1, m));
    const c0 = Math.max(d0, w.from), c2 = Math.min(d2, w.to);
    const segments: Placed["segments"] = [];
    if (c2 > c0) {
      for (const g of s.segments) {
        const a = Math.max(map(g.from), c0), b = Math.min(map(g.to), c2);
        if (b > a) segments.push({ seg: g, left: (a - c0) / (c2 - c0), width: (b - a) / (c2 - c0) });
      }
    } else if (s.segments.length) segments.push({ seg: s.segments[s.segments.length - 1], left: 0, width: 1 });
    const blocks: Placed["blocks"] = [];
    if (c2 > c0) {
      for (const b of s.blocks) {
        const a = Math.max(map(b.from), c0), z = Math.min(map(b.to), c2);
        if (z > a) blocks.push({ left: (a - c0) / (c2 - c0), width: (z - a) / (c2 - c0), kind: b.kind });
      }
    }
    const pauses: Placed["pauses"] = [];
    if (c2 > c0) {
      for (const g of s.pauses) {
        const a = Math.max(map(g.from), c0), z = Math.min(map(g.to), c2);
        if (z > a) pauses.push({ left: (a - c0) / (c2 - c0), width: (z - a) / (c2 - c0) });
      }
    }
    let left = (c0 - w.from) / len, width = (c2 - c0) / len;
    if (width < minWidth) {
      width = minWidth;
      left = Math.min(left, 1 - minWidth);
    }
    out.push({ span: s, left, width, clipStart: d0 < w.from, clipEnd: d2 > w.to, segments, blocks, pauses });
  }
  return out;
}
