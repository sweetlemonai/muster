import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { withoutComments } from "./fields";
import LessonsWaiting, { acceptAll, lessonMeta, lessonsWaiting } from "./Lessons";
import lessonsSource from "./Lessons.tsx?raw";
import { reviewBadge, ReviewBadge, reviewSection, reviewSpan, reviewStatus, withoutReview } from "./Review";
import ReviewPanel from "./ReviewPanel";
import TaskPanel from "./TaskPanel";
import type { Task } from "./types";
import { withoutVision } from "./Vision";
import WaitingList from "./WaitingList";

const task = (over: Partial<Task>): Task => ({
  queue: "muster", id: "0027", title: "Review engine", kind: "task", priority: "normal", status: "doing", stage: "testing", blocked: false, stageHistory: [],
  folder: "doing", path: "doing/0027-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "testing", ...over,
});

// As review.rs writes it: under the write-up, above the Comments and the Log.
const SECTION =
  "## Review\n\nThe review of record. Round 1 of 3 (light) at 1111111 by qa, 2026-10-13 12:00: CHANGES REQUESTED; 1 open, 0 answered. Open for the builder (R1): answer each in Building.\n\n### Findings\n\n- [ ] **R1** open · should · correctness · round 1\n  - <img src=x onerror=alert(1)>\n  - At: app/x.rs:42\n  - Scenario: a 2 GB output → out of memory\n\n### Rounds\n\n- Round 1 · 2026-10-13 12:00 · qa · light · 0000000..1111111 · CHANGES REQUESTED · found R1 · 120k tokens\n";
const BODY = `# Review engine\n\nThe write-up.\n\n## Review\n\nThe owner's own words.\n\n${SECTION}\n## Comments\n- **owner, 2026-10-13 10:00:** Hi.\n\n## Log\n- 2026-10-13 12:00 qa: review round 1\n## Review\n- in the log\n`;

describe("the Review section in a task's text", () => {
  it("is the last ## Review above the last ## Log, up to the next heading", () => {
    const span = reviewSpan(BODY);
    expect(span).not.toBeNull();
    expect(BODY.slice(...span!)).toBe(`${SECTION}\n`);
    expect(reviewSpan("# T\n\n## Log\n- x\n")).toBeNull();
    expect(reviewSpan("# T\n## Review\nend")).toEqual([4, 17]);
  });
  it("shows the section without its heading, and the write-up without it once reviewed", () => {
    expect(reviewSection(BODY)?.startsWith("The review of record. Round 1 of 3")).toBe(true);
    const rest = withoutComments(withoutReview(withoutVision(BODY, false), true));
    expect(rest).not.toContain("The review of record");
    expect(rest).toContain("## Review\n\nThe owner's own words.");
    expect(rest).not.toContain("## Comments");
    // No review round yet: a "## Review" of the owner's is just the description.
    expect(withoutReview(BODY, false)).toBe(BODY);
  });
});

describe("the card's review line", () => {
  it("says the round and what waits, in Building and Testing only", () => {
    expect(reviewBadge(task({ stage: "building", reviewRound: 1, reviewOpen: 2, reviewAnswered: 0 }))).toBe("review 1/3 · 2 to fix");
    expect(reviewBadge(task({ reviewRound: 2, reviewOpen: 0, reviewAnswered: 1 }))).toBe("review 2/3 · 1 to re-check");
    expect(reviewBadge(task({ reviewRound: 2, reviewOpen: 0, reviewAnswered: 0 }))).toBe("review clean · round 2");
    expect(reviewBadge(task({ branch: "task/mu-27-x" }))).toBe("not reviewed yet");
    expect(reviewBadge(task({ stage: "building", branch: "task/mu-27-x" }))).toBeNull();
    expect(reviewBadge(task({}))).toBeNull();
    expect(reviewBadge(task({ stage: "your-check", reviewRound: 2, reviewOpen: 0, reviewAnswered: 0 }))).toBeNull();
  });
  it("is quiet, with weight while findings wait for the builder", () => {
    const open = renderToStaticMarkup(createElement(ReviewBadge, { t: task({ stage: "building", reviewRound: 1, reviewOpen: 1 }) }));
    expect(open).toContain('class="card-review open"');
    expect(open).toContain("review 1/3 · 1 to fix");
    expect(renderToStaticMarkup(createElement(ReviewBadge, { t: task({ reviewRound: 1, reviewOpen: 0 }) }))).toContain('class="card-review"');
    expect(renderToStaticMarkup(createElement(ReviewBadge, { t: task({}) }))).toBe("");
  });
  it("says where it stands in the pane's header", () => {
    expect(reviewStatus(task({}))).toBe("not started on its own branch");
    expect(reviewStatus(task({ branch: "task/x" }))).toBe("not reviewed yet");
    expect(reviewStatus(task({ reviewRound: 2, reviewOpen: 1, reviewAnswered: 1 }))).toBe("round 2 of 3: 1 open, 1 answered");
    expect(reviewStatus(task({ reviewRound: 3, reviewOpen: 0, reviewAnswered: 0 }))).toBe("clean after round 3");
  });
  it("round 0 is an owner send-back before any review round (mu-101): what waits, no round", () => {
    expect(reviewBadge(task({ stage: "building", reviewRound: 0, reviewOpen: 1, reviewAnswered: 0 }))).toBe("1 to fix · not reviewed yet");
    expect(reviewBadge(task({ reviewRound: 0, reviewOpen: 0, reviewAnswered: 1 }))).toBe("1 to re-check · not reviewed yet");
    expect(reviewBadge(task({ reviewRound: 0, reviewOpen: 0, reviewAnswered: 0, branch: "task/x" }))).toBe("not reviewed yet");
    expect(reviewBadge(task({ stage: "building", reviewRound: 0, reviewOpen: 0, reviewAnswered: 0 }))).toBeNull();
    expect(reviewStatus(task({ reviewRound: 0, reviewOpen: 1, reviewAnswered: 0, branch: "task/x" }))).toBe("not reviewed yet: 1 open, 0 answered");
    expect(reviewStatus(task({ reviewRound: 0, reviewOpen: 0, reviewAnswered: 0, branch: "task/x" }))).toBe("not reviewed yet");
  });
});

describe("the task pane's Review section", () => {
  const panel = (t: Task, body: string | null) => renderToStaticMarkup(createElement(ReviewPanel, { task: t, body }));
  it("before a round: says how it gets reviewed, with Copy review brief for a started task", () => {
    const started = panel(task({ branch: "task/mu-27-x" }), "# T\n\n## Log\n");
    expect(started).toContain("Review <small>not reviewed yet</small>");
    expect(started).toContain("Copy review brief");
    expect(started).toContain("the merge waits for a clean review");
    const plain = panel(task({}), "# T\n\n## Log\n");
    expect(plain).not.toContain("Copy review brief");
    expect(plain).toContain("Reviewed only when it");
  });
  it("after a round: the findings and rounds as sanitized Markdown", () => {
    const html = panel(task({ branch: "task/x", reviewRound: 1, reviewOpen: 1, reviewAnswered: 0 }), BODY);
    expect(html).toContain('class="review has-open"');
    expect(html).toContain("<strong>R1</strong> open · should · correctness · round 1");
    expect(html).toContain("<h3 dir=\"auto\">Rounds</h3>");
    expect(html).not.toContain("<img");
    expect(html).toContain("&lt;img src=x onerror=alert(1)&gt;");
  });
  it("sits in the task pane", () => {
    const html = renderToStaticMarkup(createElement(TaskPanel, { task: task({ reviewRound: 1, reviewOpen: 0, reviewAnswered: 0 }), now: 0, onClose: () => {}, onChanged: () => {} }));
    expect(html).toContain('aria-label="Review of record"');
    expect(html).toContain("Review <small>clean after round 1</small>");
  });
});

describe("lessons waiting for the owner", () => {
  const queues = [
    { name: "muster", path: "/q/muster", counts: {}, lessonsWaiting: 2 },
    { name: "drover", path: "/q/drover", counts: {} },
  ];
  it("count in the needs-you count of their queue, and of All", () => {
    expect(lessonsWaiting(queues, "all")).toBe(2);
    expect(lessonsWaiting(queues, "muster")).toBe(2);
    expect(lessonsWaiting(queues, "drover")).toBe(0);
  });
  it("say their kind, who they're for, where and where they came from", () => {
    expect(lessonMeta({ kind: "unbounded-output", for: "builders", areas: ["app/src-tauri/src/"], from: ["0027 R1", "0031 R2"] })).toBe(
      "unbounded-output · for builders · app/src-tauri/src/ · from 0027 R1, 0031 R2",
    );
    expect(lessonMeta({ kind: "style-taste", for: "reviewers", areas: [], from: [] })).toBe("style-taste · for reviewers: don't flag");
  });
  it("are accepted as shown, by id: one proposed after the list was read waits for the next look", () => {
    expect(acceptAll([{ id: "L1" }, { id: "L3" }])).toEqual(["L1", "L3"]);
    expect(acceptAll([])).toEqual([]);
    expect(lessonsSource).toContain("decide(acceptAll(lessons), [], \"\")");
    expect(lessonsSource).toContain("api.decideLessons(queue, accept, false, reject,");
    expect(lessonsSource).not.toMatch(/decideLessons\([^)]*\btrue\b/);
  });
  it("lead Waiting for you, which isn't empty while they wait", () => {
    const html = renderToStaticMarkup(createElement(WaitingList, { tasks: [], multi: false, openKey: null, onOpen: () => {}, onChanged: () => {}, now: 0, queues, queue: "muster" }));
    expect(html).toContain('aria-label="Lessons to approve in muster"');
    expect(html).toContain("Lessons to approve");
    expect(html).not.toContain("Nothing is waiting for you.");
    expect(renderToStaticMarkup(createElement(LessonsWaiting, { queues, queue: "drover", onChanged: () => {} }))).toBe("");
    const none = renderToStaticMarkup(createElement(WaitingList, { tasks: [], multi: false, openKey: null, onOpen: () => {}, onChanged: () => {}, now: 0, queues, queue: "drover" }));
    expect(none).toContain("Nothing is waiting for you.");
  });
  it("name their project by its label (0044), in All too", () => {
    const labelled = [{ ...queues[0], label: "Muster app" }, queues[1]];
    const one = renderToStaticMarkup(createElement(LessonsWaiting, { queues: labelled, queue: "muster", onChanged: () => {} }));
    expect(one).toContain('aria-label="Lessons to approve in Muster app"');
    const all = renderToStaticMarkup(createElement(LessonsWaiting, { queues: labelled, queue: "all", onChanged: () => {} }));
    expect(all).toContain("Lessons to approve in Muster app");
    expect(all).not.toContain("in muster");
  });
});
