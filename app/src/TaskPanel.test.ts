import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import TaskPanel from "./TaskPanel";
import type { Task } from "./types";

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0007", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "todo/0007-t.md", claimedBy: "", heartbeat: "", waitingOn: "", commit: "", captured: "", questions: [],
  openQuestions: 0, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTokens: "", estMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", attachments: [], logTail: [], modified: 0,
  level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});

/** The pane as first drawn: its stage line, the stage row's buttons and the notes under it. */
function pane(over: Partial<Task>, now = 0, childTasks?: Task[]) {
  const html = renderToStaticMarkup(createElement(TaskPanel, { task: task(over), now, onClose: () => {}, onChanged: () => {}, childTasks }));
  const row = /<section class="controls">\s*<div class="control-row">([\s\S]*?)<\/div>/.exec(html)?.[1] ?? "";
  const buttons = [...row.matchAll(/<button([^>]*)>([^<]*)<\/button>/g)].map((m) => `${m[2]}${m[1].includes("disabled") ? " (disabled)" : ""}`);
  const line = /<div class="stage-line">([\s\S]*?)<\/div>/.exec(html)?.[1].replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").trim() ?? "";
  return { html, buttons, line };
}

describe("task pane stage controls", () => {
  it("shows the stage with its role", () => {
    expect(pane({ stage: "building", claimedBy: "owner" }).line).toBe("Building code agent");
    expect(pane({ stage: "your-check" }).line).toBe("Your check owner");
  });
  it("passes to the next stage, and Backlog can't be sent back", () => {
    expect(pane({ stage: "backlog" }).buttons).toEqual(["Pass to Shaping", "Block"]);
    expect(pane({ stage: "ready" }).buttons).toEqual(["Pass to Building", "Send back", "Block"]);
  });
  it("in Your check reads Accept and Send back", () => {
    expect(pane({ stage: "your-check" }).buttons).toEqual(["Accept", "Send back", "Block"]);
  });
  it("Done has no pass, only Send back", () => {
    expect(pane({ stage: "done" }).buttons).toEqual(["Send back", "Block"]);
  });
  it("names who else holds Building and points to the stage picker instead of a Pass that would fail", () => {
    const p = pane({ stage: "building", claimedBy: "drover/main (session 3fba2222)" });
    expect(p.buttons).toEqual(["Send back", "Block"]);
    expect(p.html).toContain("drover/main holds it in Building, so only they can pass it on. To move it yourself, pick a stage above.");
    expect(pane({ stage: "building", claimedBy: "owner" }).buttons).toEqual(["Pass to Testing", "Send back", "Block"]);
  });
  it("a blocked task keeps its stage; Pass waits for Unblock", () => {
    const p = pane({ stage: "your-check", blocked: true, status: "blocked", waitingOn: "owner pick a colour" });
    expect(p.line).toBe("Your check owner blocked: owner pick a colour");
    expect(p.buttons).toEqual(["Accept (disabled)", "Send back", "Unblock"]);
    expect(p.html).toContain("Blocked on owner pick a colour. Unblock it to pass it on.");
  });
  it("offers Release on a claim whose lease ran out, and only then", () => {
    const held = { stage: "building", claimedBy: "drover/main (session 3fba2222)", heartbeat: "", modified: 0 };
    const late = 12 * 3600 + 60;
    const p = pane(held, late);
    expect(p.buttons).toEqual(["Release", "Send back", "Block"]);
    expect(p.html).toContain("drover/main hasn&#x27;t been heard from for 12 h. Release puts it back in Ready, logged as reclaimed.");
    expect(pane(held, 12 * 3600).buttons).toEqual(["Send back", "Block"]);
    expect(pane({ ...held, blocked: true, status: "blocked", waitingOn: "owner" }, late).buttons).toEqual(["Send back", "Unblock"]);
    expect(pane({ ...held, stage: "testing" }, late).buttons).toEqual(["Pass to Your check", "Send back", "Block"]);
    // The owner's own quiet claim: Pass stays the primary action, Release sits beside it.
    expect(pane({ ...held, claimedBy: "owner" }, late).buttons).toEqual(["Pass to Testing", "Release", "Send back", "Block"]);
  });
  it("has Edit beside the description, in every stage and whoever holds it", () => {
    for (const over of [{ stage: "backlog" }, { stage: "building", claimedBy: "drover/main (session 3fba2222)" }, { stage: "done" }]) {
      const bar = /<div class="body-bar">([\s\S]*?)<\/div>/.exec(pane(over).html)?.[1] ?? "";
      expect([...bar.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1])).toEqual(["Rendered", "Raw", "Edit"]);
    }
  });
  it("a stage it doesn't know: shown as is, only the picker and Block", () => {
    const p = pane({ stage: "reviewing" });
    expect(p.line).toBe("reviewing (unknown)");
    expect(p.html).toContain('<option value="" disabled="" selected="">reviewing (unknown)</option>');
    expect(p.buttons).toEqual(["Block"]);
  });
});

describe("task pane for a goal or feature", () => {
  const progress = { done: 3, total: 7, estMinutes: 120, estTokenCount: 0 };
  const kids = [task({ id: "0031", title: "Pass button", stage: "building" }), task({ id: "0032", title: "Undo", stage: "done" })];
  it("shows the stage its children give it, its roll-up and children, and no action the core refuses", () => {
    const p = pane({ id: "0030", level: "feature", stage: "ready", shownStage: "in-progress", progress, children: ["0031", "0032"] }, 0, kids);
    expect(p.line).toBe("In progress Feature");
    expect(p.buttons).toEqual(["Send back", "Block"]);
    const rollup = (/<section class="rollup-panel"[^>]*>([\s\S]*?)<\/section>/.exec(p.html)?.[1] ?? "").replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").trim();
    expect(rollup).toBe("Feature · 3 of 7 done ≈ 2 h left 0031 Pass button Building 0032 Undo Done");
    // The owner's stage picker stays.
    expect(p.html).toContain('<option value="ready" selected="">Ready</option>');
  });
  it("without the children's tasks at hand, lists their ids", () => {
    const p = pane({ id: "0030", level: "goal", due: "2026-12-01", progress, children: ["0031"] });
    expect(p.html).toContain("due 2026-12-01");
    expect(p.html).toContain("<li>0031</li>");
  });
  it("may still be passed on while it's shaped, but never into Building, nor out of it, nor released", () => {
    expect(pane({ level: "goal", stage: "shaping", shownStage: "shaping", progress }).buttons).toEqual(["Pass to Ready", "Send back", "Block"]);
    const held = { level: "feature", stage: "building", shownStage: "building", claimedBy: "owner", heartbeat: "", modified: 0, progress };
    expect(pane(held, 12 * 3600 + 60).buttons).toEqual(["Send back", "Block"]);
  });
});
