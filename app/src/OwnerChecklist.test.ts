import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import OwnerChecklist, { criterionWords, ownerOpen } from "./OwnerChecklist";
import TaskPanel from "./TaskPanel";
import type { Criterion, Task } from "./types";

const crit = (n: number, ticked: boolean, text = `AC${n}: tried it — verify: owner`): Criterion => ({ n, label: `AC${n}`, text, ticked, owner: true });
const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0028", title: "T", kind: "task", priority: "normal", status: "doing", stage: "your-check", blocked: false, stageHistory: [],
  folder: "doing", path: "doing/0028-t.md", claimedBy: "", heartbeat: "", waitingOn: "", commit: "", captured: "", capturedAt: "", blockHistory: [], questions: [],
  openQuestions: 0, ownerCriteria: [], waitingForOwner: true, tags: [], estTime: "", estTokens: "", estMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", attachments: [], logTail: [], modified: 0,
  level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "your-check", ...over,
});
const list = (t: Task) => renderToStaticMarkup(createElement(OwnerChecklist, { task: t, busy: false, onTick: () => {} }));

describe("criterionWords", () => {
  it("drops the verify tail and its dash", () => {
    expect(criterionWords("the owner tried it — verify: owner")).toBe("the owner tried it");
    expect(criterionWords("looks right. — verify: owner (screenshots)")).toBe("looks right.");
    expect(criterionWords("no tail")).toBe("no tail");
    expect(criterionWords("İstanbul build runs — Verify: owner")).toBe("İstanbul build runs");
    expect(criterionWords("quotes \"verify: owner\" in its words — verify: command")).toBe("quotes \"verify: owner\" in its words");
  });
});

describe("the owner's checklist in Your check", () => {
  it("shows only in Your check, and only when there's something of yours", () => {
    expect(list(task({ stage: "testing", ownerCriteria: [crit(1, false)] }))).toBe("");
    expect(list(task({ ownerCriteria: [] }))).toBe("");
  });
  it("lists each with its label, words and state, and counts what's left", () => {
    const html = list(task({ ownerCriteria: [crit(1, true), crit(2, false, "AC2: looks right in dark — verify: owner (screenshots)")] }));
    expect(html).toContain("Yours to verify: 1 left, then Accept.");
    expect(html).toMatch(/class="criterion ticked" aria-pressed="true"[^>]*>.*<b>AC1<\/b> tried it<\/button>/);
    expect(html).toMatch(/class="criterion" aria-pressed="false"[^>]*>.*<b>AC2<\/b> looks right in dark<\/button>/);
    for (const b of html.match(/<button[\s\S]*?<\/button>/g) ?? []) expect(b).not.toContain("verify");
  });
  it("says when all are ticked", () => {
    expect(list(task({ ownerCriteria: [crit(1, true)] }))).toContain("All yours are ticked. Accept to finish.");
    expect(ownerOpen(task({ ownerCriteria: [crit(1, true), crit(2, false), crit(3, false)] }))).toBe(2);
  });
});

describe("Accept waits for the owner's criteria", () => {
  const accept = (t: Task) => /<button[^>]*class="primary"[^>]*>\s*Accept\s*<\/button>/.exec(renderToStaticMarkup(createElement(TaskPanel, { task: t, now: 0, onClose: () => {}, onChanged: () => {} })))?.[0] ?? "";
  it("is disabled while one is open, with the reason", () => {
    const b = accept(task({ ownerCriteria: [crit(1, true), crit(2, false)] }));
    expect(b).toContain("disabled");
    expect(b).toContain('title="Tick yours below first"');
  });
  it("is enabled once all are ticked, or when there are none", () => {
    expect(accept(task({ ownerCriteria: [crit(1, true)] }))).not.toContain("disabled");
    expect(accept(task({}))).not.toContain("disabled");
    expect(accept(task({}))).not.toBe("");
  });
});
