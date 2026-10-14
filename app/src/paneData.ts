// The task pane's facts (0021), the parts without React: what's live, what waits on the owner,
// time spent from the stage history, the criteria from the write-up. Tested in paneData.test.ts.
import { BLOCK_LOOKS } from "./blockWords";
import { boardColumn, heldByOther, isStage, stageLabel, stageRank, STAGES, stampMinutes } from "./lib";
import { ownerOpen } from "./OwnerChecklist";
import type { StageStep, Task } from "./types";
import type { Msg, Undo } from "./useTaskActions";

/** The default LIVE window: the pane's LIVE means the task file changed this recently. */
export const LIVE_SECS = 5 * 60;

/** The task file changed in the last `liveSecs` (`now` in seconds since 1970): 5 minutes, or
 *  this machine's LIVE window from Settings (0036). */
export const isLive = (t: Pick<Task, "modified">, now: number, liveSecs = LIVE_SECS) => now - t.modified < liveSecs;

/** What waits on the owner, in words, in a fixed order: Your check, escalated questions, a block
 *  on the owner or by a permission (0024: a person acts on both). The same things the core counts
 *  as waiting for the owner. */
export function waitingOnYou(t: Pick<Task, "stage" | "ownerCriteria" | "questions" | "blockedKind" | "sessionState">): string[] {
  const out: string[] = [];
  if (t.stage === "your-check") {
    const open = ownerOpen(t);
    out.push(open > 0 ? `${open} of ${t.ownerCriteria.length} criteria to check` : "accept or send back");
  }
  const asked = t.questions.filter((q) => !q.ticked && q.escalated).length;
  if (asked > 0) out.push(`${asked} question${asked === 1 ? "" : "s"} for you`);
  // In the block's own words (0060).
  if (t.blockedKind === "you") out.push(BLOCK_LOOKS.action.lead.toLowerCase());
  if (t.blockedKind === "permission") out.push(BLOCK_LOOKS.permission.lead.toLowerCase());
  // Its session waits on a prompt in Claude Code (0048): the core counts it as needing the owner.
  if (t.sessionState === "needs-input") out.push("input in Claude Code");
  return out;
}

const WORK = new Set(["building", "testing"]);

/** Wall-clock minutes the task spent in Building and Testing, from its stage history (oldest
 *  first): each stay runs to the next stage line, the open one to `now` (wall-clock minutes, as
 *  lib's stampMinutes) while the file is still in that stage. A stay it can't time (an unreadable
 *  stamp, a stamp earlier than the one before) counts nothing. Null when it never went in.
 *  Paused stretches (0045, `pauses`, one still open running to `now`) aren't counted. */
export function timeInWork(history: StageStep[], stage: string, now: number | null, pauses: { from: string; to: string }[] = []): number | null {
  let total: number | null = null;
  history.forEach((s, i) => {
    if (!WORK.has(s.to)) return;
    total ??= 0;
    const start = stampMinutes(s.at);
    const next = history[i + 1];
    const end = next ? stampMinutes(next.at) : stage === s.to ? now : null;
    if (start !== null && end !== null && end >= start) total += end - start - pausedWithin(pauses, start, end, now ?? end);
  });
  return total;
}

/** Minutes of `from..to` that fall in a paused stretch (as pause.rs `paused_within`). */
function pausedWithin(pauses: { from: string; to: string }[], from: number, to: number, now: number): number {
  let sum = 0;
  for (const p of pauses) {
    const a = stampMinutes(p.from);
    const z = p.to ? stampMinutes(p.to) : now;
    if (a !== null && z !== null) sum += Math.max(0, Math.min(z, to) - Math.max(a, from));
  }
  return sum;
}

/** Wall-clock minutes as people read them: "45 min", "16 h", "1 d 1.5 h" (24-hour days, unlike
 *  lib's fmtMinutes, which counts estimates in 8-hour agent days). */
export function fmtWall(m: number): string {
  if (m < 1) return "under a minute";
  if (m < 60) return `${Math.round(m)} min`;
  const tenths = Math.round(m / 6);
  if (tenths < 240) return `${tenths / 10} h`;
  const d = Math.floor(tenths / 240), rest = (tenths % 240) / 10;
  return rest ? `${d} d ${rest} h` : `${d} d`;
}

/** Whose claim a move on the track to `to` releases: someone else's (not the owner's) when it
 *  leaves Building, as the core's set_stage does; "" otherwise. */
export function releasedBy(t: Pick<Task, "stage" | "claimedBy">, to: string): string {
  return t.stage === "building" && to !== "building" ? heldByOther(t) : "";
}

/** What a move on the track says once it's done, and its Undo: none when it released someone
 *  else's claim (Undo couldn't give it back) or from a stage the window doesn't know. */
export function trackMove(t: Pick<Task, "stage" | "claimedBy">, to: string): { done: string; undo: Undo | undefined } {
  const released = releasedBy(t, to);
  return {
    done: `Put in ${stageLabel(to)}.${released ? ` ${released}'s claim on Building was released.` : ""}`,
    undo: !released && isStage(t.stage) ? { back: t.stage, to } : undefined,
  };
}

/** A drag the pane attaches: one carrying files. Text dragged into the editor or an answer box
 *  is theirs. */
export const isFileDrag = (types: readonly string[]) => types.includes("Files");

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

/** The owner's own task's steps (0022), each the stage a click puts it in. */
const HUMAN_STEPS = [
  { id: "ready", label: "To do" },
  { id: "building", label: "Doing" },
  { id: "done", label: "Done" },
] as const;

/** The stage track's steps: the seven stages, or for the owner's own task only To do · Doing ·
 *  Done, placed by the column the board shows it in, so a stage it never takes (Shaping, Your
 *  check) reads as the step it shows in and a click always moves it somewhere you can see. */
export function trackSteps(stage: string, human: boolean): { id: string; label: string; place: "past" | "cur" | "next" }[] {
  if (!human) return STAGES.map((s) => ({ id: s.id, label: s.label, place: trackPlace(s.id, stage) }));
  const at = boardColumn({ stage, forWho: "human" });
  return HUMAN_STEPS.map((s) => ({ id: s.id, label: s.label, place: trackPlace(s.id, at) }));
}

/** The folded Log and check-ins header's count (display modes, 0038): the Log's lines (each a
 *  "- " item) and the check-ins, "3 log lines · 2 check-ins"; undefined when there's neither. */
export function logSummary(log: string, checkins: number): string | undefined {
  const lines = log.split("\n").filter((l) => /^\s*- /.test(l)).length;
  const n = (k: number, word: string) => `${k} ${word}${k === 1 ? "" : "s"}`;
  const parts = [lines ? n(lines, "log line") : "", checkins ? n(checkins, "check-in") : ""].filter(Boolean);
  return parts.length ? parts.join(" · ") : undefined;
}
