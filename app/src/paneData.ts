// The task pane's facts (0021), the parts without React: what's live, what waits on the owner,
// time spent from the stage history, the criteria from the write-up. Tested in paneData.test.ts.
import { stageRank, isStage, stampMinutes } from "./lib";
import { ownerOpen } from "./OwnerChecklist";
import type { StageStep, Task } from "./types";
import type { Msg, Undo } from "./useTaskActions";

/** Until sessions come with 0025, "live" means the task file changed this recently. */
export const LIVE_SECS = 5 * 60;

/** The task file changed in the last 5 minutes (`now` in seconds since 1970). */
export const isLive = (t: Pick<Task, "modified">, now: number) => now - t.modified < LIVE_SECS;

/** What waits on the owner, in words, in a fixed order: Your check, escalated questions, a block
 *  on the owner. The same three things the core counts as waiting for the owner. */
export function waitingOnYou(t: Pick<Task, "stage" | "ownerCriteria" | "questions" | "blocked" | "waitingOn">): string[] {
  const out: string[] = [];
  if (t.stage === "your-check") {
    const open = ownerOpen(t);
    out.push(open > 0 ? `${open} of ${t.ownerCriteria.length} criteria to check` : "accept or send back");
  }
  const asked = t.questions.filter((q) => !q.ticked && q.escalated).length;
  if (asked > 0) out.push(`${asked} question${asked === 1 ? "" : "s"} for you`);
  if (t.blocked && t.waitingOn.trim().toLowerCase().startsWith("owner")) out.push("blocked on you");
  return out;
}

const WORK = new Set(["building", "testing"]);

/** Wall-clock minutes the task spent in Building and Testing, from its stage history (oldest
 *  first): each stay runs to the next stage line, the open one to `now` (wall-clock minutes, as
 *  lib's stampMinutes) while the file is still in that stage. A stay it can't time (an unreadable
 *  stamp, a stamp earlier than the one before) counts nothing. Null when it never went in. */
export function timeInWork(history: StageStep[], stage: string, now: number | null): number | null {
  let total: number | null = null;
  history.forEach((s, i) => {
    if (!WORK.has(s.to)) return;
    total ??= 0;
    const start = stampMinutes(s.at);
    const next = history[i + 1];
    const end = next ? stampMinutes(next.at) : stage === s.to ? now : null;
    if (start !== null && end !== null && end >= start) total += end - start;
  });
  return total;
}

/** The write-up's "## Acceptance criteria" section, without its heading, up to the next "## "
 *  heading; null when there's none or it's empty. */
export function criteriaSection(text: string): string | null {
  const head = /^## Acceptance criteria[ \t]*$/m.exec(text);
  if (!head) return null;
  const rest = text.slice(head.index + head[0].length);
  const next = /^## /m.exec(rest);
  const body = (next ? rest.slice(0, next.index) : rest).trim();
  return body || null;
}

/** The Undo a move on the track offers, while the task is still where that move put it: once it
 *  moved on (here or elsewhere), undoing would put it somewhere nobody meant. */
export function undoFor(msg: Msg | null, stage: string): Undo | null {
  return msg?.undo && msg.undo.to === stage ? msg.undo : null;
}

/** Where a stage sits on the track against the task's: done with, the current one, or ahead.
 *  With a stage the window doesn't know, none is current and every stage reads as ahead. */
export function trackPlace(stage: string, current: string): "past" | "cur" | "next" {
  if (!isStage(current)) return "next";
  const a = stageRank(stage), b = stageRank(current);
  return a < b ? "past" : a === b ? "cur" : "next";
}
