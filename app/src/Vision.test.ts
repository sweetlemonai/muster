import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { withoutComments } from "./fields";
import { copyNow } from "./NotePanel";
import TaskPanel from "./TaskPanel";
import type { Task } from "./types";
import { conflictTitles, visionBadge, VisionBadge, visionSection, visionSpan, visionStatus, visionWarnings, withoutVision } from "./Vision";
import VisionPanel from "./VisionPanel";

const task = (over: Partial<Task>): Task => ({
  queue: "muster", id: "0024", title: "Blocked says why", kind: "task", priority: "normal", status: "todo", stage: "shaping", blocked: false, stageHistory: [],
  folder: "todo", path: "todo/0024-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], commit: "", captured: "", capturedAt: "",
  blockHistory: [], questions: [], openQuestions: 0, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "shaping", ...over,
});

// As vision.rs writes it: the last "## Vision" above the last "## Log".
const SECTION = "## Vision\n\nChecked 2026-10-12 14:05 by wt-shape: 2 conflicts (V1–V2), no overlaps, 1 gap.\n\n### Conflicts\n\n- [ ] **V1: Two records of task-on-task waits**\n  - 0024: “blocked-by: lists them”\n  - 0019: “depends: names them”\n  - Fix: keep depends:\n- [x] **V2: Two spellings: --on and --by**\n  - Fix: keep --on\n  - Resolved by owner, 2026-10-12 15:00: done\n- [ ] **V3: <img src=x onerror=alert(1)>**\n  - Fix: f\n\n### Gaps\n\n- [ ] **V9: a gap, not a conflict**\n";
const BODY = `# Blocked says why\n\n## Vision\n\nThe owner's own words.\n\n## Questions\n\n${SECTION}\n## Log\n- 2026-10-12 14:05 wt-shape: vision check\n## Vision\n- [ ] **V7: in the log**\n`;

describe("the Vision section in a task's text", () => {
  it("is the last ## Vision above the last ## Log, up to the next heading", () => {
    const span = visionSpan(BODY);
    expect(span).not.toBeNull();
    const [a, b] = span!;
    expect(BODY.slice(a, b)).toBe(`${SECTION}\n`);
    expect(BODY.slice(b).startsWith("## Log\n")).toBe(true);
    expect(visionSpan("# T\n\n## Log\n- x\n")).toBeNull();
    expect(visionSpan("# T\n## Vision\nend")).toEqual([4, 17]);
  });
  it("shows the section without its heading, and the body without the section once checked", () => {
    expect(visionSection(BODY)?.startsWith("Checked 2026-10-12 14:05 by wt-shape")).toBe(true);
    expect(visionSection("# T\n\n## Log\n")).toBeNull();
    const rest = withoutVision(BODY, true);
    expect(rest).not.toContain("Checked 2026-10-12");
    expect(rest).toContain("## Vision\n\nThe owner's own words.");
    expect(rest).toContain("## Questions\n\n## Log\n");
    // Never checked: a "## Vision" of the owner's is just the description.
    expect(withoutVision(BODY, false)).toBe(BODY);
  });
  it("sits above the tool's Comments, and the write-up the pane renders has neither", () => {
    const text = `# T\n\nThe write-up.\n\n${SECTION}\n## Comments\n- **owner, 2026-10-12 10:00:** Hi.\n\n## Log\n- x\n`;
    expect(text.slice(...visionSpan(text)!)).toBe(`${SECTION}\n`);
    expect(visionSection(text)?.startsWith("Checked 2026-10-12 14:05 by wt-shape")).toBe(true);
    expect(withoutComments(withoutVision(text, true))).toBe("# T\n\nThe write-up.\n\n## Log\n- x\n");
  });
  it("titles the conflicts from the Conflicts list only, open or not", () => {
    expect(conflictTitles(BODY)).toEqual({ V1: "Two records of task-on-task waits", V2: "Two spellings: --on and --by", V3: "<img src=x onerror=alert(1)>" });
    expect(conflictTitles("# T\n")).toEqual({});
  });
});

describe("the card and the Problems list", () => {
  it("shows ⚠ and the count of open conflicts, and nothing otherwise", () => {
    expect(visionBadge(task({ visionOpen: 2 }))).toBe("⚠ 2 vision");
    for (const visionOpen of [0, null, undefined]) expect(visionBadge(task({ visionOpen }))).toBeNull();
    const html = renderToStaticMarkup(createElement(VisionBadge, { t: task({ visionOpen: 1 }) }));
    expect(html).toContain('class="card-vision"');
    expect(html).toContain("⚠ 1 vision");
    expect(renderToStaticMarkup(createElement(VisionBadge, { t: task({ visionOpen: 0 }) }))).toBe("");
  });
  it("warns about Shaping tasks with no check yet, in id order", () => {
    const tasks = [
      task({ id: "0031", title: "Later" }),
      task({ id: "0024", visionOpen: null }),
      task({ id: "0025", visionOpen: 0 }),
      task({ id: "0026", stage: "ready" }),
      task({ id: "0022", queue: "drover", title: "Human board" }),
      // A `vision:` that isn't a count is a problem already (muster check), not a warning too.
      task({ id: "0027", badVision: "lots" }),
    ];
    expect(visionWarnings(tasks)).toEqual([
      "drover: 0022 Human board is in Shaping with no vision check yet (muster vision 0022)",
      "muster: 0024 Blocked says why is in Shaping with no vision check yet (muster vision 0024)",
      "muster: 0031 Later is in Shaping with no vision check yet (muster vision 0031)",
    ]);
  });
  it("says where the check stands", () => {
    expect(visionStatus(task({}))).toBe("not checked");
    expect(visionStatus(task({ visionOpen: 0 }))).toBe("no open conflicts");
    expect(visionStatus(task({ visionOpen: 1 }))).toBe("1 open conflict");
    expect(visionStatus(task({ visionOpen: 3 }))).toBe("3 open conflicts");
  });
});

describe("the task pane's Vision section", () => {
  const panel = (t: Task, body: string | null) => renderToStaticMarkup(createElement(VisionPanel, { task: t, body, onChanged: () => {} }));
  const buttons = (html: string) => [...html.matchAll(/<button([^>]*)>([^<]*)<\/button>/g)].map((m) => `${m[2]}${m[1].includes("disabled") ? " (disabled)" : ""}`);

  it("before a check: says so, with Copy vision brief (off until the brief loads)", () => {
    const html = panel(task({}), "# T\n\n## Vision\n\nThe owner's own words.\n\n## Log\n");
    expect(html).toContain("Vision <small>not checked</small>");
    expect(buttons(html)).toEqual(["Copy vision brief (disabled)"]);
    expect(html).toContain("Not checked against the project&#x27;s picture yet");
    expect(html).not.toContain("own words");
  });
  it("with open conflicts: Resolve for each, the section as sanitized Markdown", () => {
    const html = panel(task({ visionOpen: 2, visionIds: ["V1", "V3"] }), BODY);
    expect(html).toContain('class="vision has-open"');
    expect(html).toContain("Vision <small>2 open conflicts</small>");
    expect(buttons(html)).toEqual(["Resolve V1", "Resolve V3", "Copy vision brief (disabled)"]);
    expect(html).toContain('title="Two records of task-on-task waits"');
    expect(html).toContain("<h3>Conflicts</h3>");
    expect(html).toContain("<strong>V1: Two records of task-on-task waits</strong>");
    expect(html).not.toContain("<img");
    expect(html).toContain("&lt;img src=x onerror=alert(1)&gt;");
    expect(html).not.toContain("<h2>Vision</h2>");
  });
  it("shows Resolve from the task before its text has loaded", () => {
    const html = panel(task({ visionOpen: 1, visionIds: ["V2"] }), null);
    expect(buttons(html)).toEqual(["Resolve V2", "Copy vision brief (disabled)"]);
  });
  it("sits in the task pane, and the description below leaves the section out", () => {
    const html = renderToStaticMarkup(createElement(TaskPanel, { task: task({ visionOpen: 0 }), now: 0, onClose: () => {}, onChanged: () => {} }));
    expect(html).toContain('aria-label="Vision check"');
    expect(html).toContain("Vision <small>no open conflicts</small>");
  });
  it("copies the brief in the click itself, and says which brief is still loading", async () => {
    const calls: string[] = [];
    const clipboard = { writeText: (t: string) => { calls.push(t); return Promise.resolve(); } };
    await copyNow("the vision brief", clipboard, "The vision brief");
    expect(calls).toEqual(["the vision brief"]);
    await expect(copyNow(null, clipboard, "The vision brief")).rejects.toThrow("The vision brief is still loading.");
    await expect(copyNow(null, clipboard)).rejects.toThrow("The shaping brief is still loading.");
  });
});
