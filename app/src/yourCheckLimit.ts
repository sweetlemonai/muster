// The Your check limit (0050): when more of the agents' tasks wait in Your check than the owner
// set for a project, the board, the sidebar and Settings › Projects say so. It only warns. The
// core counts the waiting tasks and keeps the limits (check_limit.rs, in projects.json by queue
// folder); these are the words. Tested in yourCheckLimit.test.ts.
import { fmt, tl, tn } from "./i18n";
import type { QueueInfo } from "./types";

/** The default limit and its bounds, as the core's check_limit::DEFAULT, MIN and MAX. */
export const LIMIT_DEFAULT = 5;
export const LIMIT_MIN = 1;
export const LIMIT_MAX = 50;

/** A project over its limit: its name, what the window calls it (0044), how many wait, and the limit. */
export type Over = { name: string; label: string; waiting: number; limit: number };

/** The project over its limit, or null: within it, the limit off, or nothing said (an older core). */
function overOf(q: QueueInfo): Over | null {
  const limit = q.yourCheckLimit;
  const waiting = q.yourCheck ?? 0;
  return typeof limit === "number" && waiting > limit ? { name: q.name, label: q.label || q.name, waiting, limit } : null;
}

/** The projects over their limit in the place shown: every project for "all", else the one
 *  named, each checked on its own, in the queues' order. */
export function overLimit(queues: QueueInfo[], queue: string): Over[] {
  return queues.filter((q) => queue === "all" || q.name === queue).flatMap((q) => overOf(q) ?? []);
}

/** "Muster (7 of 5) and drover (9 of 8)", by their labels. */
const named = (over: Over[]) => {
  const each = over.map((o) => tn("board", "check.limit.each", { label: o.label, waiting: o.waiting, limit: o.limit }));
  return each.length > 1 ? tn("board", "check.limit.and", { rest: each.slice(0, -1).join(tl("listSep")), last: each[each.length - 1] }) : each.join("");
};

/** The Your check column's header count: "7 of 5" when the project shown is over its limit (the
 *  whole pile, whatever the filters show), in the attention colour; in All projects the column's
 *  count, in that colour when any is over; else the plain count. */
export function checkCount(count: number, over: Over[], queue: string): { text: string; over: boolean; title: string } {
  if (over.length === 0) return { text: fmt.num(count), over: false, title: "" };
  if (queue !== "all") {
    const o = over[0];
    return {
      text: tn("board", "check.limit.count", { waiting: o.waiting, limit: o.limit }),
      over: true,
      title: tn("board", "check.limit.pileSettings", { count: o.waiting, limit: o.limit }),
    };
  }
  return { text: fmt.num(count), over: true, title: tn("board", "check.limit.overIn", { list: named(over) }) };
}

/** The longest the column's own line gets: it has two short lines of room in a narrow column. */
export const COLUMN_LINE_MAX = 24;

/** The short line under the Your check header, to fit its reserved two lines at any column
 *  width: "Over your limit of 5", or in All projects "2 projects over limit" (the names are in
 *  the line above the board); "" within the limit. */
export function columnLine(over: Over[], queue: string): string {
  if (over.length === 0) return "";
  if (queue !== "all") return tn("board", "check.limit.column", { limit: over[0].limit });
  return tn("board", "check.limit.columnAll", { count: over.length });
}

/** "7 tasks wait in Your check, over your limit of 5". */
const pile = (o: Over) => tn("board", "check.limit.pile", { count: o.waiting, limit: o.limit });

/** The line above the board, short enough for one line at any width it's likely to have: the
 *  numbers first for one project ("7 of 5 in Your check: …"), how many projects in All (each
 *  sidebar row says its own); "" within the limit. `overTitle` has it whole. */
export function overLine(over: Over[], queue: string): string {
  if (over.length === 0) return "";
  if (queue !== "all") return tn("board", "check.limit.line", { waiting: over[0].waiting, limit: over[0].limit });
  return tn("board", "check.limit.lineAll", { count: over.length });
}

/** The line above the board in full, its title: the pile and the limit, or in All each project
 *  over with its count and limit; "" within the limit. */
export function overTitle(over: Over[], queue: string): string {
  if (over.length === 0) return "";
  if (queue !== "all") return tn("board", "check.limit.title", { count: over[0].waiting, limit: over[0].limit });
  return tn("board", "check.limit.titleAll", { list: named(over) });
}

/** A Group by band's Your check count: its own, in orange when it holds tasks (`queues`, one per
 *  task in the band's column) of a project over its limit, with that project's pile as its title. */
export function bandLimit(queues: string[], over: Over[], queue: string): { over: boolean; title: string } {
  const here = over.filter((o) => queues.includes(o.name));
  if (here.length === 0) return { over: false, title: "" };
  const title = here.map((o) => (queue === "all" ? tn("board", "check.limit.pileIn", { label: o.label, count: o.waiting, limit: o.limit }) : pile(o))).join(tl("clauseSep"));
  return { over: true, title };
}

/** A sidebar row's line when its project is over: "7 in Your check, limit 5"; else "". */
export function sideLimit(q: QueueInfo | undefined): string {
  const o = q ? overOf(q) : null;
  return o ? tn("shell", "limit.side", { waiting: o.waiting, limit: o.limit }) : "";
}

/** A project's limit with its count, for Settings › Projects: "limit 5 · 7 waiting". */
export function limitText(q: QueueInfo): string {
  const limit = typeof q.yourCheckLimit === "number" ? tn("shell", "limit.on", { limit: q.yourCheckLimit }) : tn("shell", "limit.off");
  const waiting = q.yourCheck ?? 0;
  return tn("shell", "limit.cell", { limit, waiting: waiting === 0 ? tn("shell", "limit.noneWaiting") : tn("shell", "limit.waiting", { count: waiting }) });
}
