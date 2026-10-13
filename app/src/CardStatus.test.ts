import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import CardStatus from "./CardStatus";
import type { Task } from "./types";

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0030", title: "T", kind: "task", priority: "normal", status: "doing", stage: "building", blocked: false, stageHistory: [],
  folder: "doing", path: "doing/0030-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], commit: "", captured: "", capturedAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "building", ...over,
});

/** The wall clock here reading `s`, as the state's `now` (seconds since 1970): a fake clock. */
const at = (s: string) => {
  const [y, mo, d, h, mi] = s.split(/[- :]/).map(Number);
  return new Date(y, mo - 1, d, h, mi).getTime() / 1000;
};
const NOW = at("2026-10-12 14:05");
const A = "wt-a (wt-0030 · session 7b5e8856 · cc-hg)";

/** The card's lines as [class, text]. */
function lines(over: Partial<Task>, now = NOW): [string, string][] {
  const html = renderToStaticMarkup(createElement(CardStatus, { t: task(over), now }));
  return [...html.matchAll(/<span class="([^"]+)"[^>]*>([^<]*)<\/span>/g)].map((m) => [m[1], m[2].replace(/&#x27;/g, "'")]);
}

describe("the card's who and progress lines", () => {
  const building = { stage: "building", claimedBy: A, stageSince: "2026-10-12 12:00", heartbeat: "2026-10-12 13:53", modified: NOW };
  it("in Building: who, since when, and the latest check-in with its percentage and age", () => {
    const checkins = [{ at: "2026-10-12 13:53", who: A, pct: 60, text: "gate green, writing tests" }];
    expect(lines({ ...building, checkins })).toEqual([
      ["card-who", "wt-a · wt-0030 · 7b5e8856 · started 2 h ago"],
      ["card-checkin", "60% · gate green, writing tests · 12 min ago"],
    ]);
  });
  it("flags a session quiet past 30 min in the attention colour, and not at 30", () => {
    expect(lines({ ...building, heartbeat: "2026-10-12 13:20" })).toEqual([
      ["card-who", "wt-a · wt-0030 · 7b5e8856 · started 2 h ago"],
      ["card-quiet", "no check-in for 45 min"],
    ]);
    expect(lines({ ...building, heartbeat: "2026-10-12 13:35" }).map(([c]) => c)).toEqual(["card-who"]);
    // The clock moves on, the same card goes quiet.
    expect(lines({ ...building, heartbeat: "2026-10-12 13:35" }, NOW + 60).at(-1)).toEqual(["card-quiet", "no check-in for 31 min"]);
    // Past the lease too: it says so, as the Release button does.
    expect(lines({ ...building, heartbeat: "2026-10-12 01:00" }).at(-1)).toEqual(["card-quiet", "quiet 12 h+ · no check-in for 13 h"]);
    // Blocked is waiting, not quiet; the owner's own claim only shows the lease.
    expect(lines({ ...building, heartbeat: "2026-10-12 10:00", blocked: true, waitingOn: "owner" }).map(([c]) => c)).toEqual(["card-who"]);
    expect(lines({ ...building, claimedBy: "owner", heartbeat: "2026-10-12 01:00" })).toEqual([
      ["card-who", "you · started 2 h ago"],
      ["card-quiet", "quiet 12 h+"],
    ]);
  });
  it("in Testing and Your check: the holder from held-by, and Testing's start", () => {
    expect(lines({ stage: "testing", heldBy: "wt-0030 (session 7b5e8856)", stageSince: "2026-10-12 13:00" })).toEqual([
      ["card-who", "wt-0030 · 7b5e8856 · started 1 h ago"],
    ]);
    expect(lines({ stage: "your-check", heldBy: "owner", stageSince: "2026-10-12 13:00" })).toEqual([["card-who", "you"]]);
  });
  it("in Ready says nobody is on it yet, and Backlog, Shaping and Done say nothing", () => {
    expect(lines({ stage: "ready" })).toEqual([["card-who nobody", "nobody on it"]]);
    expect(lines({ stage: "ready", claimedBy: "wt-b" })).toEqual([["card-who", "wt-b"]]);
    for (const stage of ["backlog", "shaping", "done"]) expect(lines({ stage, claimedBy: A, heldBy: A })).toEqual([]);
  });
  it("adds open vision conflicts (0029) as one more line, at any stage, and nothing without them", () => {
    expect(lines({ stage: "shaping", visionOpen: 2, visionIds: ["V1", "V2"] })).toEqual([["card-vision", "⚠ 2 vision"]]);
    expect(lines({ ...building, visionOpen: 1, visionIds: ["V3"] })).toEqual([
      ["card-who", "wt-a · wt-0030 · 7b5e8856 · started 2 h ago"],
      ["card-vision", "⚠ 1 vision"],
    ]);
    for (const visionOpen of [0, null, undefined]) expect(lines({ stage: "shaping", visionOpen })).toEqual([]);
    const html = renderToStaticMarkup(createElement(CardStatus, { t: task({ stage: "shaping", visionOpen: 2 }), now: NOW }));
    expect(html).toContain("can&#x27;t pass from Shaping to Ready");
  });
});
