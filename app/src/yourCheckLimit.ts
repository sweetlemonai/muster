// The Your check limit (0050): when more of the agents' tasks wait in Your check than the owner
// set for a project, the board, the sidebar and Settings › Projects say so. It only warns. The
// core counts and keeps the limits (settings.rs); these are the words. Tested in
// yourCheckLimit.test.ts.
import type { QueueInfo } from "./types";

/** The default limit and its bounds, as the core's settings::YOUR_CHECK_LIMIT, _MIN and _MAX. */
export const LIMIT_DEFAULT = 5;
export const LIMIT_MIN = 1;
export const LIMIT_MAX = 50;

/** A project over its limit: how many wait, and the limit. */
export type Over = { name: string; waiting: number; limit: number };

/** The project over its limit, or null: within it, the limit off, or nothing said (an older core). */
function overOf(q: QueueInfo): Over | null {
  const limit = q.yourCheckLimit;
  const waiting = q.yourCheck ?? 0;
  return typeof limit === "number" && waiting > limit ? { name: q.name, waiting, limit } : null;
}

/** The projects over their limit in the place shown: every project for "all", else the one
 *  named, each checked on its own, in the queues' order. */
export function overLimit(queues: QueueInfo[], queue: string): Over[] {
  return queues.filter((q) => queue === "all" || q.name === queue).flatMap((q) => overOf(q) ?? []);
}

/** "muster (7 of 5) and drover (9 of 8)". */
const named = (over: Over[]) => {
  const each = over.map((o) => `${o.name} (${o.waiting} of ${o.limit})`);
  return each.length > 1 ? `${each.slice(0, -1).join(", ")} and ${each[each.length - 1]}` : each.join("");
};

/** The Your check column's header count: "7 of 5" when the project shown is over its limit (the
 *  whole pile, whatever the filters show), in the attention colour; in All projects the column's
 *  count, in that colour when any is over; else the plain count. */
export function checkCount(count: number, over: Over[], queue: string): { text: string; over: boolean; title: string } {
  if (over.length === 0) return { text: String(count), over: false, title: "" };
  if (queue !== "all") {
    const o = over[0];
    return { text: `${o.waiting} of ${o.limit}`, over: true, title: `${o.waiting} tasks wait in Your check, over your limit of ${o.limit} (Settings › Projects)` };
  }
  return { text: String(count), over: true, title: `Over the limit in ${named(over)}` };
}

/** The line under the Your check header; "" within the limit. */
export function overLine(over: Over[], queue: string): string {
  if (over.length === 0) return "";
  const where = queue === "all" ? ` in ${named(over)}` : ` of ${over[0].limit}`;
  return `Over your limit${where}: check these before more start.`;
}

/** A sidebar row's line when its project is over: "7 in Your check, limit 5"; else "". */
export function sideLimit(q: QueueInfo | undefined): string {
  const o = q ? overOf(q) : null;
  return o ? `${o.waiting} in Your check, limit ${o.limit}` : "";
}

/** A project's limit with its count, for Settings › Projects: "limit 5 · 7 waiting". */
export function limitText(q: QueueInfo): string {
  const limit = typeof q.yourCheckLimit === "number" ? `limit ${q.yourCheckLimit}` : "off";
  const waiting = q.yourCheck ?? 0;
  return `${limit} · ${waiting === 0 ? "none" : waiting} waiting`;
}
