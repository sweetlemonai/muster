import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { TitleEdit } from "./PaneHeader";
import { ValueChipView } from "./Sidebar";
import StageTrack from "./StageTrack";
import TaskPanel from "./TaskPanel";
import type { StageStep, Task } from "./types";

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0007", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "todo/0007-t.md", claimedBy: "", heartbeat: "", waitingOn: "", commit: "", captured: "", questions: [],
  openQuestions: 0, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTokens: "", estMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", attachments: [], logTail: [], modified: 0,
  level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/\s+/g, " ").trim();
/** The inside of the first `<div … data-part="…">` (none of these parts nests a div). */
const part = (html: string, name: string) => new RegExp(`<div[^>]*data-part="${name}"[^>]*>([\\s\\S]*?)</div>`).exec(html)?.[1] ?? "";

/** The pane as first drawn: its status line, the next step's buttons and notes, its sections. */
function pane(over: Partial<Task>, now = 0, childTasks?: Task[]) {
  const html = renderToStaticMarkup(createElement(TaskPanel, { task: task(over), now, onClose: () => {}, onChanged: () => {}, childTasks }));
  const next = part(html, "next-step");
  const buttons = [...next.matchAll(/<button([^>]*)>([^<]*)<\/button>/g)].map((m) => `${m[2]}${m[1].includes("disabled") ? " (disabled)" : ""}`);
  const line = text(part(html, "status"));
  const parts = [...html.matchAll(/data-part="([\w-]+)"/g)].map((m) => m[1]);
  const primary = [...html.matchAll(/<button[^>]*class="primary"[^>]*>([^<]*)<\/button>/g)].map((m) => m[1]);
  return { html, buttons, line, next, parts, primary };
}

describe("task pane stage controls", () => {
  it("shows the stage with its role", () => {
    expect(pane({ stage: "building", claimedBy: "owner" }).line).toBe("Building on it: owner LIVE code agent nothing waiting on you");
    expect(pane({ stage: "your-check" }).line).toBe("Your check owner Waiting on you: accept or send back");
  });
  it("passes to the next stage, and Backlog can't be sent back", () => {
    expect(pane({ stage: "backlog" }).buttons).toEqual(["Pass to Shaping", "Block"]);
    expect(pane({ stage: "ready" }).buttons).toEqual(["Pass to Building", "Send back", "Block"]);
  });
  it("in Your check reads Accept and Send back", () => {
    expect(pane({ stage: "your-check" }).buttons).toEqual(["Accept", "Send back", "Block"]);
  });
  it("Done has no pass, only Send back", () => {
    const p = pane({ stage: "done" });
    expect(p.buttons).toEqual(["Send back", "Block"]);
    expect(text(p.next)).toContain("Done. Send it back to reopen it.");
  });
  it("names who else holds Building and points to the track instead of a Pass that would fail", () => {
    const p = pane({ stage: "building", claimedBy: "drover/main (session 3fba2222)" });
    expect(p.buttons).toEqual(["Send back", "Block"]);
    expect(p.html).toContain("drover/main holds it in Building, so only they can pass it on. To move it yourself, click a stage on the track.");
    expect(pane({ stage: "building", claimedBy: "owner" }).buttons).toEqual(["Pass to Testing", "Send back", "Block"]);
  });
  it("a blocked task keeps its stage; Pass waits for Unblock", () => {
    const p = pane({ stage: "your-check", blocked: true, status: "blocked", waitingOn: "owner pick a colour" });
    expect(p.line).toBe("Your check blocked: owner pick a colour owner Waiting on you: accept or send back · blocked on you");
    expect(p.buttons).toEqual(["Accept (disabled)", "Send back", "Unblock"]);
    expect(p.html).toContain("Blocked on owner pick a colour. Unblock it to pass it on.");
  });
  it("offers Release on a claim whose lease ran out, and only then", () => {
    const held = { stage: "building", claimedBy: "drover/main (session 3fba2222)", heartbeat: "", modified: 0 };
    const late = 12 * 3600 + 60;
    const p = pane(held, late);
    expect(p.buttons).toEqual(["Release", "Send back", "Block"]);
    expect(p.primary).toEqual(["Release"]);
    expect(p.html).toContain("drover/main hasn&#x27;t been heard from for 12 h. Release puts it back in Ready, logged as reclaimed.");
    expect(pane(held, 12 * 3600).buttons).toEqual(["Send back", "Block"]);
    expect(pane({ ...held, blocked: true, status: "blocked", waitingOn: "owner" }, late).buttons).toEqual(["Send back", "Unblock"]);
    expect(pane({ ...held, stage: "testing" }, late).buttons).toEqual(["Pass to Your check", "Send back", "Block"]);
    // The owner's own quiet claim: Pass stays the primary action, Release sits beside it.
    const own = pane({ ...held, claimedBy: "owner" }, late);
    expect(own.buttons).toEqual(["Pass to Testing", "Release", "Send back", "Block"]);
    expect(own.primary).toEqual(["Pass to Testing"]);
  });
  it("has Edit beside the description, in every stage and whoever holds it", () => {
    for (const over of [{ stage: "backlog" }, { stage: "building", claimedBy: "drover/main (session 3fba2222)" }, { stage: "done" }]) {
      const bar = /<div class="body-bar">([\s\S]*?)<\/div>/.exec(pane(over).html)?.[1] ?? "";
      expect([...bar.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1])).toEqual(["Rendered", "Raw", "Edit"]);
    }
  });
  it("a stage it doesn't know: shown as is, no stage lit on the track, only Block", () => {
    const p = pane({ stage: "reviewing" });
    expect(p.line).toBe("reviewing (unknown) nothing waiting on you");
    expect(p.html).not.toContain('aria-pressed="true"');
    expect(p.buttons).toEqual(["Block"]);
    expect(text(p.next)).toContain("A stage this window doesn't know: click one on the track.");
  });
});

describe("the status line", () => {
  const held = { stage: "building", claimedBy: "drover/main (session 3fba2222)", modified: 1000 };
  it("who's on it, live while the file changed in the last 5 minutes", () => {
    expect(pane(held, 1000 + 60).line).toBe("Building on it: drover/main LIVE code agent nothing waiting on you");
    expect(pane(held, 1000 + 3 * 3600).line).toBe("Building on it: drover/main · quiet 3 h code agent nothing waiting on you");
  });
  it("the project and when the file changed lead the pane", () => {
    expect(text(/<div class="topline">([\s\S]*?)<\/div>/.exec(pane(held, 1000 + 3 * 3600).html)?.[1] ?? "")).toBe("demo · changed 3 h ago ×");
    expect(pane(held, 1000).html).toContain("· changed just now");
  });
  it("a claim left on a stage nobody holds isn't shown", () => {
    expect(pane({ ...held, stage: "testing" }, 1000).line).toBe("Testing QA agent nothing waiting on you");
  });
  it("what's waiting on the owner, in gold", () => {
    const crit = (n: number, ticked: boolean) => ({ n, label: `AC${n}`, text: "x — verify: owner", ticked, owner: true });
    const p = pane({ stage: "your-check", ownerCriteria: [crit(1, true), crit(2, false)] });
    expect(p.html).toContain('<span class="needs">Waiting on you: 1 of 2 criteria to check</span>');
  });
});

describe("the stage track", () => {
  const track = (stage: string, busy = false) => renderToStaticMarkup(createElement(StageTrack, { stage, busy, onPick: () => {} }));
  it("lays out all seven stages, the task's lit and the ones before it marked done", () => {
    const html = track("building");
    const buttons = [...html.matchAll(/<button type="button" class="(\w+) s-([\w-]+)" aria-pressed="(\w+)"[^>]*title="([^"]*)"[^>]*>/g)].map((m) => `${m[1]} ${m[2]} ${m[3]} ${m[4]}`);
    expect(buttons).toEqual([
      "past backlog false Put it in Backlog",
      "past shaping false Put it in Shaping",
      "past ready false Put it in Ready",
      "cur building true It&#x27;s in Building",
      "next testing false Put it in Testing",
      "next your-check false Put it in Your check",
      "next done false Put it in Done",
    ]);
    expect(text(html)).toBe("Stage click any stage to move it there Backlog Shaping Ready Building Testing Your check Done");
  });
  it("says which moves release someone else's claim on Building", () => {
    const titles = (html: string) => [...html.matchAll(/class="(\w+) s-[\w-]+" aria-pressed="\w+"[^>]*title="([^"]*)"/g)].map((m) => m[2].replace(/&#x27;/g, "'"));
    expect(titles(pane({ stage: "building", claimedBy: "drover/main (session 3fba2222)" }).html)).toEqual([
      "Put it in Backlog (releases drover/main's claim)",
      "Put it in Shaping (releases drover/main's claim)",
      "Put it in Ready (releases drover/main's claim)",
      "It's in Building",
      "Put it in Testing (releases drover/main's claim)",
      "Put it in Your check (releases drover/main's claim)",
      "Put it in Done (releases drover/main's claim)",
    ]);
    expect(pane({ stage: "building", claimedBy: "owner" }).html).not.toContain("releases");
    expect(pane({ stage: "ready", claimedBy: "drover/main (session 3fba2222)" }).html).not.toContain("releases");
  });
  it("is off while a stage action runs", () => {
    expect(track("ready", true).match(/disabled=""/g)?.length).toBe(7);
    expect(track("ready")).not.toContain("disabled");
  });
});

describe("priority and type: the current value as one chip, the options laid out in place", () => {
  const chip = (open: boolean, value = "high") =>
    renderToStaticMarkup(createElement(ValueChipView, { label: "Priority", value, options: ["high", "normal", "low"], open, onOpen: () => {}, onClose: () => {}, onPick: () => {} }));
  it("shows only the current value until clicked", () => {
    expect(chip(false)).toBe('<button type="button" class="vchip" aria-label="Priority: high, click to change">high</button>');
  });
  it("clicked: every option in the same spot, the current one pressed", () => {
    const html = chip(true);
    expect(html).toContain('role="group" aria-label="Priority"');
    expect([...html.matchAll(/aria-pressed="(\w+)"[^>]*>(\w+)</g)].map((m) => `${m[2]} ${m[1]}`)).toEqual(["high true", "normal false", "low false"]);
  });
  it("a value the options don't name still shows, pressed", () => {
    expect([...chip(true, "P1").matchAll(/aria-pressed="(\w+)"[^>]*>(\w+)</g)].map((m) => `${m[2]} ${m[1]}`)).toEqual(["P1 true", "high false", "normal false", "low false"]);
  });
  it("in the pane, both show their current value", () => {
    const html = pane({ priority: "low", kind: "bug" }).html;
    expect(html).toContain('aria-label="Priority: low, click to change">low</button>');
    expect(html).toContain('aria-label="Type: bug, click to change">bug</button>');
  });
});

describe("the title, renamed in place", () => {
  it("is a button on the title itself, no Rename button", () => {
    const html = pane({ title: "Stages with roles" }).html;
    expect(html).toContain('<button type="button" class="title-btn" title="Click to rename">Stages with roles</button>');
    expect(html).not.toContain(">Rename<");
  });
  it("opens as a text box with Save and Cancel", () => {
    const html = renderToStaticMarkup(createElement(TitleEdit, { value: "Stages", onChange: () => {}, onSave: () => {}, onCancel: () => {} }));
    expect(html).toContain('value="Stages"');
    expect([...html.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1])).toEqual(["Save", "Cancel"]);
  });
});

describe("the pane's layout by stage (D2)", () => {
  const at = (s: string) => Math.floor(new Date(`${s.replace(" ", "T")}:00`).getTime() / 1000);
  const step = (when: string, from: string, to: string, who = "wt-a (session aaaa1111)"): StageStep => ({ at: when, who, from, to });
  const history = [step("2026-10-12 15:00", "shaping", "ready", "capture"), step("2026-10-12 16:00", "ready", "building")];
  const after = ["path", "body", "note-bar"];
  it("Building: title and status, the track, the work so far, then the next step: Pass to Testing", () => {
    const p = pane({ stage: "building", claimedBy: "owner", stageHistory: history, estMinutes: 480, estTime: "1d", estTokenCount: 2_000_000, modified: at("2026-10-12 17:58") }, at("2026-10-12 18:00"));
    expect(p.parts).toEqual(["title", "status", "track", "work", "next-step", "settings", ...after]);
    expect(p.primary).toEqual(["Pass to Testing"]);
    const work = text(part(p.html, "work"));
    expect(work).toContain("Time 2 h in Building and Testing · estimate 1 d of agent time");
    expect(work).toContain("Tokens 2M tokens estimated · used: with 0025");
    expect(work).toContain("Sessions: coming with 0025");
    expect(work).toContain("10-12 16:00 Ready → Building · wt-a 10-12 15:00 Shaping → Ready · capture");
    // Wall-clock time and agent time aren't the same unit: no bar pretends to compare them.
    expect(p.html).not.toContain("of the time estimate");
  });
  it("Building for a day of wall-clock time reads in 24-hour days, the estimate in agent days", () => {
    const p = pane({ stage: "building", claimedBy: "owner", stageHistory: [step("2026-10-11 10:00", "ready", "building")], estMinutes: 480 }, at("2026-10-12 12:00"));
    expect(text(part(p.html, "work"))).toContain("Time 1 d 2 h in Building and Testing · estimate 1 d of agent time");
  });
  it("Building with nothing to time says so, and makes nothing up", () => {
    const work = text(part(pane({ stage: "building", claimedBy: "owner" }).html, "work"));
    expect(work).toContain("not timed: no move into Building in the log · no time estimate");
    expect(work).toContain("No stage moves in the log yet.");
    expect(work).not.toContain("Tokens");
  });
  it("Your check: the owner's checklist and the history first, then the next step: Accept", () => {
    const crit = (n: number, ticked: boolean) => ({ n, label: `AC${n}`, text: `AC${n}: tried it — verify: owner`, ticked, owner: true });
    const p = pane({ stage: "your-check", ownerCriteria: [crit(1, true), crit(2, true)], stageHistory: [...history, step("2026-10-12 17:00", "building", "testing"), step("2026-10-12 17:40", "testing", "your-check", "qa")] });
    expect(p.parts).toEqual(["title", "status", "track", "checklist", "history", "next-step", "settings", ...after]);
    expect(p.primary).toEqual(["Accept"]);
    expect(p.html).toContain('aria-label="Yours to verify"');
    expect(text(part(p.html, "history"))).toMatch(/^History from the log, newest first 10-12 17:40 Testing → Your check · qa 10-12 17:00 Building → Testing/);
  });
  it("Your check with none of the criteria the owner's says so", () => {
    expect(text(part(pane({ stage: "your-check" }).html, "checklist"))).toBe("Your checklist None of the criteria are yours to verify. Accept it, or send it back.");
  });
  it("Ready: the criteria and the estimate first, then the next step: Pass to Building", () => {
    const p = pane({ stage: "ready", estMinutes: 120, estTokenCount: 500_000 });
    expect(p.parts).toEqual(["title", "status", "track", "criteria", "estimates", "next-step", "settings", ...after]);
    expect(p.primary).toEqual(["Pass to Building"]);
    expect(text(part(p.html, "estimates"))).toBe("Estimate ≈ 2 h · 500k tokens");
    // The write-up loads after the first draw.
    expect(text(part(p.html, "criteria"))).toBe("Criteria Loading…");
  });
  it("Shaping without an estimate says where to set one", () => {
    expect(text(part(pane({ stage: "shaping" }).html, "estimates"))).toBe("Estimate Not estimated yet: Est, on the right.");
  });
  it("a feature: its roll-up first; passed on only while it's shaped", () => {
    const progress = { done: 1, total: 2, estMinutes: 0, estTokenCount: 0 };
    const shaping = pane({ level: "feature", stage: "shaping", shownStage: "shaping", progress, children: ["0031"] });
    expect(shaping.parts).toEqual(["title", "status", "track", "rollup", "next-step", "settings", ...after]);
    expect(shaping.primary).toEqual(["Pass to Ready"]);
    const ready = pane({ level: "feature", stage: "ready", shownStage: "in-progress", progress, children: ["0031"] });
    expect(ready.primary).toEqual([]);
    expect(text(ready.next)).toContain("Its tasks move it on.");
  });
  it("Done: the history first", () => {
    expect(pane({ stage: "done" }).parts).toEqual(["title", "status", "track", "history", "next-step", "settings", ...after]);
  });
  it("open questions come between the header and the write-up", () => {
    const p = pane({ questions: [{ n: 1, ticked: false, asker: "wt-a", text: "Which?", escalated: true, ownerAnswered: false }] });
    expect(p.parts.slice(-4)).toEqual(["path", "questions", "body", "note-bar"]);
  });
  it("the path and commit stay on one quiet line; files and Attach are always there", () => {
    const p = pane({ commit: "abc1234", attachments: ["shot.png", "notes.txt"] });
    expect(text(part(p.html, "path"))).toBe("commit abc1234 · todo/0007-t.md");
    expect(text(pane({}).html)).toContain("commit none · todo/0007-t.md");
    expect(p.html).toContain('<span class="lbl">Files <small>2</small></span>');
    expect(p.html).toContain('<span class="other-file" title="notes.txt">notes.txt</span>');
    expect(p.html).toContain('<button type="button" class="attach">+ Attach</button>');
  });
  it("every control is in the markup, none behind a menu or popover", () => {
    const html = pane({ stage: "ready" }).html;
    for (const label of ['aria-label="Stage"', 'aria-label="Agent time"', 'aria-label="Tokens"', 'aria-label="Add a tag"', 'aria-label="Note to the log"', ">Add to log<", ">Edit<", 'class="title-btn"']) {
      expect(html, label).toContain(label);
    }
    expect(html).not.toMatch(/popover|<select|<details|<dialog/);
  });
});

describe("task pane for a goal or feature", () => {
  const progress = { done: 3, total: 7, estMinutes: 120, estTokenCount: 0 };
  const kids = [task({ id: "0031", title: "Pass button", stage: "building" }), task({ id: "0032", title: "Undo", stage: "done" })];
  it("shows the stage its children give it, its roll-up and children, and no action the core refuses", () => {
    const p = pane({ id: "0030", level: "feature", stage: "ready", shownStage: "in-progress", progress, children: ["0031", "0032"] }, 0, kids);
    expect(p.line).toBe("In progress Feature nothing waiting on you");
    expect(p.buttons).toEqual(["Send back", "Block"]);
    const rollup = (/<section class="rollup-panel"[^>]*>([\s\S]*?)<\/section>/.exec(p.html)?.[1] ?? "").replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").trim();
    expect(rollup).toBe("Feature · 3 of 7 done ≈ 2 h left 0031 Pass button Building 0032 Undo Done");
    // The owner's stage override stays: its own stage is lit on the track.
    expect(p.html).toMatch(/class="cur s-ready" aria-pressed="true"/);
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
