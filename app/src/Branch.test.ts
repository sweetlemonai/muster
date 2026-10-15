import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { BranchPane, branchLine, folderName } from "./Branch";
import CardStatus from "./CardStatus";
import type { Task } from "./types";

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0031", title: "T", kind: "code", priority: "normal", status: "doing", stage: "building", blocked: false, stageHistory: [],
  folder: "doing", path: "doing/0031-t.md", claimedBy: "wt-a", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "building", ...over,
});

const STARTED = { branch: "task/0031-board-flickers", worktree: "/Users/x/playground/muster/worktrees/main/0031-board-flickers" };
const REPORT = "merged task/0031-board-flickers into main as 3f2a9c1 (fast-forward, pushed to origin) · gate: green (4 steps) · Building 2h 10m · Testing 35m · 4 check-ins";

/** The card's lines as [class, text]. */
function cardLines(over: Partial<Task>): [string, string][] {
  const html = renderToStaticMarkup(createElement(CardStatus, { t: task(over), now: 0 }));
  return [...html.matchAll(/<span class="([^"]+)"[^>]*>([^<]*)<\/span>/g)].map((m) => [m[1], m[2]]);
}

const pane = (over: Partial<Task>) => renderToStaticMarkup(createElement(BranchPane, { task: task(over) }));

describe("a started task (0023) on the card and in the pane", () => {
  it("names a worktree by its last folder", () => {
    expect(folderName(STARTED.worktree)).toBe("0031-board-flickers");
    expect(folderName("/a/b/")).toBe("b");
    expect(folderName("")).toBe("");
  });

  it("the card shows the branch and the worktree's folder as a quiet line, with the path on hover", () => {
    expect(branchLine(task(STARTED))).toBe("task/0031-board-flickers · 0031-board-flickers");
    expect(branchLine(task({ branch: "task/0031-x" }))).toBe("task/0031-x");
    expect(cardLines({ ...STARTED, stage: "testing", heldBy: "qa" })).toEqual([
      ["card-who", "qa"],
      // Left to right in any language, as a branch and a path are (mu-149).
      ["card-branch ltr", "task/0031-board-flickers · 0031-board-flickers"],
      // In Testing, its review of record (0027) hasn't run yet.
      ["card-review", "not reviewed yet"],
    ]);
    const html = renderToStaticMarkup(createElement(CardStatus, { t: task(STARTED), now: 0 }));
    expect(html).toContain(`title="Worktree: ${STARTED.worktree}"`);
  });

  it("a task that wasn't started in a worktree, or is merged, has no branch line", () => {
    expect(branchLine(task({}))).toBe("");
    expect(cardLines({}).map(([c]) => c)).not.toContain("card-branch ltr");
    expect(cardLines({ stage: "your-check", mergeReport: REPORT, commit: "3f2a9c1d" }).map(([c]) => c)).not.toContain("card-branch ltr");
  });

  it("the pane shows the branch and the whole worktree path while it's started", () => {
    const html = pane(STARTED);
    expect(html).toContain('data-part="worktree"');
    expect(html).toContain('<code class="ltr">task/0031-board-flickers</code>');
    expect(html).toContain(`<code class="path ltr">${STARTED.worktree}</code>`);
    expect(html).not.toContain("Merged");
  });

  it("after the merge the pane shows the merge commit and the report", () => {
    const html = pane({ stage: "your-check", commit: "3f2a9c1d0e", mergeReport: REPORT });
    expect(html).toContain('data-part="merged"');
    expect(html).toContain('<code class="ltr">3f2a9c1</code>');
    expect(html).toContain(REPORT);
    expect(pane({ stage: "your-check", mergeReport: REPORT })).toContain("no commit recorded");
  });

  it("shows nothing for a task that never had a worktree", () => {
    expect(pane({})).toBe("");
    expect(pane({ branch: "", worktree: "", mergeReport: "" })).toBe("");
  });
});
