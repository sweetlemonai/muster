import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import OwnerChecklist, { criterionWords, ownerOpen } from "./OwnerChecklist";
import TaskPanel from "./TaskPanel";
import type { Criterion, Task } from "./types";

const crit = (n: number, ticked: boolean, text = `AC${n}: tried it — verify: owner`): Criterion => ({ n, label: `AC${n}`, text, ticked, owner: true });
const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0028", title: "T", kind: "task", priority: "normal", status: "doing", stage: "your-check", blocked: false, stageHistory: [],
  folder: "doing", path: "doing/0028-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: true, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "your-check", search: "", ...over,
});
const list = (t: Task) => renderToStaticMarkup(createElement(OwnerChecklist, { task: t, busy: false, onTick: () => {} }));
const withViews = (t: Task) => renderToStaticMarkup(createElement(OwnerChecklist, { task: t, busy: false, onTick: () => {}, onView: () => {} }));
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/\s+/g, " ").trim();

describe("criterionWords", () => {
  it("drops the verify tail and its dash", () => {
    expect(criterionWords("the owner tried it — verify: owner")).toBe("the owner tried it");
    expect(criterionWords("looks right. — verify: owner (screenshots)")).toBe("looks right.");
    expect(criterionWords("no tail")).toBe("no tail");
    expect(criterionWords("İstanbul build runs — Verify: owner")).toBe("İstanbul build runs");
    expect(criterionWords("quotes \"verify: owner\" in its words — verify: command")).toBe("quotes \"verify: owner\" in its words");
  });
  it("drops an effort override too (0043), before or after the verify part", () => {
    expect(criterionWords("opens INDEX.md — effort: session — verify: owner")).toBe("opens INDEX.md");
    expect(criterionWords("a session's transcript — verify: owner — effort: look")).toBe("a session's transcript");
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
    expect(html).toMatch(/<li class="criterion ticked">.*<b>AC1<\/b> <span dir="auto">tried it<\/span><\/span>/);
    expect(html).toMatch(/<li class="criterion">.*<b>AC2<\/b> <span dir="auto">looks right in dark<\/span><\/span>/);
    for (const row of html.match(/<li[\s\S]*?<\/li>/g) ?? []) expect(row).not.toContain("verify");
  });
  it("lists the open ones first (0043), each with Tick and its effort; the ticked ones after, with Untick", () => {
    const t = task({ ownerCriteria: [crit(1, true), crit(2, false, "AC2: runs in a session — verify: owner"), crit(3, false), crit(4, true)] });
    const rows = [...list(t).matchAll(/<li class="([^"]+)">([\s\S]*?)<\/li>/g)].map((m) => `${m[1]}: ${text(m[2])}`);
    expect(rows).toEqual([
      "criterion: AC2 runs in a session Needs a session Tick",
      "criterion: AC3 tried it One look Tick",
      "criterion ticked: ✓ AC1 tried it Untick",
      "criterion ticked: ✓ AC4 tried it Untick",
    ]);
    // Tick says which, for the core; it's pressed when ticked.
    expect(list(t)).toMatch(/<button type="button" aria-pressed="false"[^>]*class="ui-button tick">Tick<\/button>/);
    expect(list(t)).toMatch(/<button type="button" aria-pressed="true"[^>]*class="ui-button tick">Untick<\/button>/);
  });
  it("gives an open criterion a button for each view it names (0043)", () => {
    const t = task({ ownerCriteria: [crit(1, false, "AC1: the Graph tab and the Timeline agree, then Appearance — verify: owner"), crit(2, true, "AC2: the Graph — verify: owner")] });
    const views = [...withViews(t).matchAll(/<button type="button" class="ui-button view-btn"[^>]*>([^<]*)<\/button>/g)].map((m) => m[1]);
    expect(views).toEqual(["Open Timeline", "Open Graph", "Open Appearance"]);
    // Without a way to open views, none are drawn.
    expect(list(t)).not.toContain("view-btn");
  });
  it("says when all are ticked", () => {
    expect(list(task({ ownerCriteria: [crit(1, true)] }))).toContain("All yours are ticked. Accept to finish.");
    expect(ownerOpen(task({ ownerCriteria: [crit(1, true), crit(2, false), crit(3, false)] }))).toBe(2);
  });
});

describe("Accept waits for the owner's criteria", () => {
  const accept = (t: Task) => /<button[^>]*class="ui-button ui-button--primary"[^>]*>\s*Accept\s*<\/button>/.exec(renderToStaticMarkup(createElement(TaskPanel, { task: t, now: 0, onClose: () => {}, onChanged: () => {} })))?.[0] ?? "";
  it("is disabled while one is open, with the reason", () => {
    const b = accept(task({ ownerCriteria: [crit(1, true), crit(2, false)] }));
    expect(b).toContain("disabled");
    expect(b).toContain('title="Tick yours in the checklist first"');
  });
  it("is enabled once all are ticked, or when there are none", () => {
    expect(accept(task({ ownerCriteria: [crit(1, true)] }))).not.toContain("disabled");
    expect(accept(task({}))).not.toContain("disabled");
    expect(accept(task({}))).not.toBe("");
  });
});
