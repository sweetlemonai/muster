import { describe, expect, it } from "vitest";
import cases from "./search-cases.json";
import { applyQuery, dayContext, EMPTY_QUERY, type Filters, type Query } from "./filters";
import { isBoardSort, sortBoard } from "./lib";
import { fold } from "./search";
import type { Task } from "./types";

// The app's half of the shared cases: tests/search_cases.rs runs `muster list` on the same
// tasks and expects the same ids, so the CLI and the app can't drift apart.

type FixtureQuery = { search?: string; stage?: string[]; type?: string[]; tag?: string[]; holder?: string[]; blocked?: boolean; sort?: string };

/** A task as the core gives it to the window: the file's fields, plus what it derives. */
const tasks: Task[] = cases.tasks.map((f) => ({
  queue: "q", id: f.id, title: f.title, kind: f.type, priority: f.priority, status: "", stage: f.stage, blocked: f.parsed.blocked, stageHistory: [], folder: "", path: "",
  claimedBy: f.claimedBy, heldBy: f.heldBy, stageSince: "", checkins: [], heartbeat: "", waitingOn: f.waitingOn, depends: [], commit: "", captured: f.captured, capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: f.parsed.waitingForOwner, tags: f.tags,
  estTime: f.estTime, estTestTime: "", estTokens: "", estMinutes: f.parsed.estMinutes, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: f.modified, level: "task", parent: "", due: "", children: [], progress: null, shownStage: f.stage, search: f.parsed.search,
}));

/** The CLI's options as the bar's chips: --stage, --type, --tag, --holder and --blocked. */
function toQuery(q: FixtureQuery): Query {
  const filters: Filters = {};
  for (const [group, vals] of [["stage", q.stage], ["type", q.type], ["tag", q.tag], ["holder", q.holder]] as const) if (vals?.length) filters[group] = [...vals];
  if (q.blocked) filters.state = ["blocked"];
  return { ...EMPTY_QUERY, search: q.search ?? "", filters, sort: isBoardSort(q.sort) ? q.sort : EMPTY_QUERY.sort };
}

describe("the shared search cases (the CLI gives the same)", () => {
  const ctx = dayContext(Date.UTC(2026, 9, 14) / 1000);
  for (const c of cases.cases) {
    const q = c.query as FixtureQuery;
    it(JSON.stringify(q), () => {
      const kept = applyQuery(tasks, toQuery(q), ctx);
      // The CLI lists in id order unless it's given a sort; the fixture's tasks are in id order.
      const shown = q.sort ? sortBoard(kept, toQuery(q).sort) : kept;
      expect(shown.map((t) => t.id)).toEqual(c.ids);
    });
  }
  it("folds as the core does", () => {
    for (const [from, to] of cases.fold) expect(fold(from)).toBe(to);
  });
});
