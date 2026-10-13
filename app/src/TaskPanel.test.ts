import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { TitleEdit } from "./PaneHeader";
import { ValueChipView } from "./Sidebar";
import StageTrack from "./StageTrack";
import { trackSteps } from "./paneData";
import TaskPanel from "./TaskPanel";
import type { StageStep, Task } from "./types";
import { Machine } from "./settingsData";

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0007", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "todo/0007-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", search: "", ...over,
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
    expect(pane({ stage: "building", claimedBy: "owner" }).line).toBe("Building LIVE on it: you code agent nothing waiting on you");
    expect(pane({ stage: "your-check" }).line).toBe("Your check LIVE Your turn owner Waiting on you: accept or send back");
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
    const p = pane({ stage: "your-check", blocked: true, status: "blocked", waitingOn: "pick a colour", blockedKind: "you", blockDetail: "pick a colour" });
    expect(p.line).toBe("Your check Blocked on you : pick a colour LIVE Your turn owner Waiting on you: accept or send back · blocked on you");
    expect(p.html).toContain('<span class="status-pill blocked block-why"><b>Blocked on you</b>: pick a colour</span>');
    expect(p.buttons).toEqual(["Accept (disabled)", "Send back", "Unblock"]);
    expect(p.html).toContain("Blocked on you: pick a colour. Unblock it to pass it on.");
  });
  it("says why it's blocked by tasks, each a link that opens it, with Block still there (0024)", () => {
    const blockedBy = [{ id: "0014", stage: "building", holder: "codex" }, { id: "0016", stage: "ready", holder: "" }];
    const html = renderToStaticMarkup(
      createElement(TaskPanel, { task: task({ blockedKind: "tasks", blockedBy, depends: ["0014", "0016"] }), now: 0, onClose: () => {}, onChanged: () => {}, onOpenTask: () => {} }),
    );
    expect(text(part(html, "status"))).toContain("Blocked by tasks : 0014 (Building, codex), 0016 (Ready)");
    expect(part(html, "status")).toContain('<button type="button" class="link" title="Open 0014">0014</button>');
    // Not a flag: there's nothing to unblock; it clears by itself.
    const next = part(html, "next-step");
    expect(next).toContain(">Block</button>");
    expect(next).not.toContain("Unblock");
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
  it("judges Release and LIVE by this machine's settings (0036)", () => {
    const held = { stage: "building", claimedBy: "drover/main (session 3fba2222)", heartbeat: "", modified: 0 };
    const withSettings = (now: number) =>
      renderToStaticMarkup(
        createElement(Machine.Provider, { value: { leaseMinutes: 90, liveMinutes: 10 } }, createElement(TaskPanel, { task: task(held), now, onClose: () => {}, onChanged: () => {} })),
      );
    // 91 minutes quiet: past a 90-minute lease.
    const late = withSettings(91 * 60);
    expect(part(late, "next-step")).toContain(">Release</button>");
    expect(text(late)).toContain("drover/main hasn't been heard from for 1 h 30 min. Release puts it back in Ready");
    expect(part(withSettings(90 * 60), "next-step")).not.toContain(">Release</button>");
    // LIVE for 10 minutes after the file changed.
    expect(text(part(withSettings(9 * 60), "status"))).toContain("LIVE");
    expect(text(part(withSettings(10 * 60), "status"))).not.toContain("LIVE");
  });
  it("has Edit beside the description, in every stage and whoever holds it", () => {
    for (const over of [{ stage: "backlog" }, { stage: "building", claimedBy: "drover/main (session 3fba2222)" }, { stage: "done" }]) {
      const bar = /<div class="body-bar">([\s\S]*?)<\/div>/.exec(pane(over).html)?.[1] ?? "";
      expect([...bar.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1])).toEqual(["Rendered", "Raw", "Edit"]);
    }
  });
  it("says who's on it in full, since when, and who held each stage so far (0030)", () => {
    const A = "wt-a (wt-0030 · session 7b5e8856 · cc-hg)";
    const now = new Date(2026, 9, 12, 14, 5).getTime() / 1000;
    const history = [
      { at: "2026-10-12 10:00", who: A, from: "ready", to: "building" },
      { at: "2026-10-12 12:05", who: A, from: "building", to: "testing" },
    ];
    const holders = (over: Partial<Task>) => /<span class="holders">([\s\S]*?)<\/span>/.exec(pane(over, now).html)?.[1].replace(/<[^>]+>/g, "").replace(/&#x27;/g, "'") ?? "";
    expect(holders({ stage: "testing", heldBy: A, stageHistory: history, stageSince: "2026-10-12 12:05" })).toBe(`on it: ${A} · started 2 h ago · Building: ${A}`);
    expect(holders({ stage: "your-check", heldBy: "owner", stageHistory: [...history, { at: "2026-10-12 13:00", who: "qa", from: "testing", to: "your-check" }] })).toBe(
      `Your turn · Building: ${A} · Testing: ${A}`,
    );
    expect(holders({ stage: "ready" })).toBe("");
  });
  it("lists every check-in in order, the percentage labelled as the session's own estimate", () => {
    const A = "wt-a (session 7b5e8856)";
    const html = pane({ stage: "building", claimedBy: A, checkins: [{ at: "2026-10-12 10:30", who: A, pct: 40, text: "gate green" }, { at: "2026-10-12 11:00", who: A, pct: null, text: "tests next" }] }).html;
    const list = /<section class="checkins">([\s\S]*?)<\/section>/.exec(html)?.[1] ?? "";
    expect(list).toContain("The percentages are each session&#x27;s own estimate.");
    const items = [...list.matchAll(/<li>([\s\S]*?)<\/li>/g)].map((m) => m[1].replace(/<[^>]+>/g, "").replace(/\s+/g, " ").trim());
    expect(items).toEqual([`2026-10-12 10:30 40% gate green · ${A}`, `2026-10-12 11:00 tests next · ${A}`]);
    expect(pane({ stage: "building" }).html).not.toContain("checkins");
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
    expect(pane(held, 1000 + 60).line).toBe("Building LIVE on it: drover/main (session 3fba2222) code agent nothing waiting on you");
    expect(pane(held, 1000 + 3 * 3600).line).toBe("Building quiet 3 h · on it: drover/main (session 3fba2222) code agent nothing waiting on you");
  });
  it("the project and when the file changed lead the pane", () => {
    expect(text(/<div class="topline">([\s\S]*?)<\/div>/.exec(pane(held, 1000 + 3 * 3600).html)?.[1] ?? "")).toBe("demo · changed 3 h ago ×");
    expect(pane(held, 1000).html).toContain("· changed just now");
  });
  it("who's on it in full, since when and who held the stages before, all in the status line (0030)", () => {
    const A = "wt-a (wt-0030 · session 7b5e8856 · cc-hg)";
    const now = new Date(2026, 9, 12, 14, 5).getTime() / 1000;
    const p = pane({ stage: "testing", heldBy: A, stageSince: "2026-10-12 12:05", modified: now - 60, stageHistory: [{ at: "2026-10-12 10:00", who: A, from: "ready", to: "building" }] }, now);
    expect(p.line).toBe(`Testing LIVE on it: ${A} · started 2 h ago · Building: ${A} QA agent nothing waiting on you`);
    // Done: nobody on it, the stages' holders still named, no live mark.
    const done = pane({ stage: "done", stageHistory: [{ at: "2026-10-12 10:00", who: A, from: "ready", to: "building" }] }, now);
    expect(done.line).toBe(`Done Building: ${A} · Your check: you nothing waiting on you`);
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
  it("shows the owner's own task only its three steps, To do · Doing · Done, as the board places it (0022)", () => {
    const steps = (stage: string) => {
      const html = renderToStaticMarkup(createElement(StageTrack, { stage, busy: false, onPick: () => {}, human: true }));
      return [...html.matchAll(/<button type="button" class="(\w+) s-([\w-]+)" aria-pressed="(\w+)"[^>]*title="([^"]*)"[^>]*>.*?<span>([^<]*)<\/span>/g)].map((m) => `${m[1]} ${m[2]} ${m[3]} ${m[5]} · ${m[4].replace(/&#x27;/g, "'")}`);
    };
    expect(steps("ready")).toEqual(["cur ready true To do · It's in To do", "next building false Doing · Put it in Doing", "next done false Done · Put it in Done"]);
    expect(steps("building")).toEqual(["past ready false To do · Put it in To do", "cur building true Doing · It's in Doing", "next done false Done · Put it in Done"]);
    // A stage it never takes reads as the step the board shows it in, so no click looks like nothing.
    expect(steps("shaping").map((s) => s.split(" ")[0])).toEqual(["cur", "next", "next"]);
    expect(steps("your-check").map((s) => s.split(" ")[0])).toEqual(["past", "cur", "next"]);
    expect(steps("done").map((s) => s.split(" ")[0])).toEqual(["past", "past", "cur"]);
  });
  it("moves to the step's stage: To do is Ready, Doing Building, Done Done", () => {
    expect(trackSteps("shaping", true)).toEqual([
      { id: "ready", label: "To do", place: "cur" },
      { id: "building", label: "Doing", place: "next" },
      { id: "done", label: "Done", place: "next" },
    ]);
    expect(trackSteps("testing", false).map((s) => `${s.id} ${s.label} ${s.place}`)).toEqual([
      "backlog Backlog past", "shaping Shaping past", "ready Ready past", "building Building past", "testing Testing cur", "your-check Your check next", "done Done next",
    ]);
  });
  it("is the pane's track for the owner's own task", () => {
    const html = pane({ forWho: "human", stage: "building", claimedBy: "owner" }).html;
    const track = /aria-label="Stage">([\s\S]*?)<\/div>/.exec(html)?.[1] ?? "";
    expect([...track.matchAll(/<span>([^<]*)<\/span>/g)].map((m) => m[1])).toEqual(["To do", "Doing", "Done"]);
    const agent = /aria-label="Stage">([\s\S]*?)<\/div>/.exec(pane({ stage: "building" }).html)?.[1] ?? "";
    expect(agent.match(/<button/g)).toHaveLength(7);
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
  const after = ["path", "body", "vision", "note-bar"];
  it("Building: title and status, the track, the work so far, then the next step: Pass to Testing", () => {
    const p = pane({ stage: "building", claimedBy: "owner", stageHistory: history, estMinutes: 480, estTime: "1d", estTokenCount: 2_000_000, modified: at("2026-10-12 17:58") }, at("2026-10-12 18:00"));
    expect(p.parts).toEqual(["title", "status", "track", "work", "next-step", "settings", ...after]);
    expect(p.primary).toEqual(["Pass to Testing"]);
    const work = text(part(p.html, "work"));
    expect(work).toContain("Time 2 h in Building and Testing · estimate 1 d of agent time");
    expect(work).toContain("Tokens 2M tokens estimated · used: in Work below");
    expect(work).toContain("Sessions, tokens used and transcripts: in Work below.");
    expect(work).not.toContain("coming with 0025");
    expect(work).toContain("10-12 16:00 Ready → Building · wt-a 10-12 15:00 Shaping → Ready · capture");
    // Wall-clock time and agent time aren't the same unit: no bar pretends to compare them.
    expect(p.html).not.toContain("of the time estimate");
  });
  it("Building with a session on it: the strip to reach that session under the next step (0039)", () => {
    const p = pane({ stage: "building", claimedBy: "wt-a (session aaaa1111)" });
    expect(p.parts).toEqual(["title", "status", "track", "work", "next-step", "settings", "ping", ...after]);
    expect(text(part(p.html, "ping"))).toContain("Session Ask for a check-in 0/500 Send It reaches session aaaa1111 after its next tool call, quoted as your note.");
    // A claim without a session stamp: the same strip, its buttons off, with why.
    expect(text(part(pane({ stage: "building", claimedBy: "wt-a" }).html, "ping"))).toContain("this claim has no session to reach; ask its session to re-claim");
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
  it("Your check lists the open criteria first, each with Tick and the views it names (0043)", () => {
    const crit = (n: number, ticked: boolean, words: string) => ({ n, label: `AC${n}`, text: `AC${n}: ${words} — verify: owner`, ticked, owner: true });
    const t = task({ stage: "your-check", ownerCriteria: [crit(1, true, "it builds"), crit(2, false, "the owner opens the Graph and checks the chain")] });
    const html = renderToStaticMarkup(createElement(TaskPanel, { task: t, now: 0, onClose: () => {}, onChanged: () => {}, onOpenView: () => {} }));
    const rows = [...html.matchAll(/<li class="(criterion[^"]*)">([\s\S]*?)<\/li>/g)].map((m) => `${m[1]}: ${text(m[2])}`);
    expect(rows).toEqual(["criterion: AC2 the owner opens the Graph and checks the chain One look Tick Open Graph", "criterion ticked: ✓ AC1 it builds Untick"]);
    // The checklist comes first in the pane's columns, before the history and the next step.
    expect(pane({ stage: "your-check", ownerCriteria: t.ownerCriteria }).parts.slice(3, 6)).toEqual(["checklist", "history", "next-step"]);
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
    expect(p.parts.slice(-5)).toEqual(["path", "questions", "body", "vision", "note-bar"]);
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

describe("the pane's 0017 fields in the D2 layout", () => {
  it("counts test time with agent time in the estimate", () => {
    const p = pane({ stage: "ready", estMinutes: 120, estTestMinutes: 30, estTokenCount: 500_000 });
    expect(text(part(p.html, "estimates"))).toBe("Estimate ≈ 2.5 h · 500k tokens");
    expect(text(part(pane({ stage: "shaping", estTestMinutes: 45 }).html, "estimates"))).toBe("Estimate ≈ 45 min");
  });
  it("names test time beside agent time in the work so far", () => {
    const work = (over: Partial<Task>) => text(part(pane({ stage: "building", claimedBy: "owner", ...over }).html, "work"));
    expect(work({ estMinutes: 480, estTestMinutes: 60 })).toContain("· estimate 1 d of agent time and 1 h of test time");
    expect(work({ estTestMinutes: 30 })).toContain("· estimate 30 min of test time");
  });
  it("has a test time field beside agent time and tokens", () => {
    const html = pane({ estTestTime: "30m" }).html;
    expect(html).toMatch(/<input[^>]*aria-label="Test time"[^>]*value="30m"/);
    expect(html.indexOf('aria-label="Agent time"')).toBeLessThan(html.indexOf('aria-label="Test time"'));
    expect(html.indexOf('aria-label="Test time"')).toBeLessThan(html.indexOf('aria-label="Tokens"'));
  });
  it("lists the links in the sidebar, after the files", () => {
    const html = pane({ links: [{ title: "The spec", url: "https://example.com/spec" }] }).html;
    const files = html.indexOf('aria-label="Files"'), links = html.indexOf('aria-label="Links"');
    expect(files).toBeGreaterThan(html.indexOf("<aside"));
    expect(links).toBeGreaterThan(files);
    expect(links).toBeLessThan(html.indexOf("</aside>"));
    expect(html).toContain('title="https://example.com/spec">The spec</button>');
  });
  it("puts the comment thread below the write-up and above the note bar", () => {
    const html = pane({ comments: [{ who: "owner", at: "2026-10-12 10:00", text: "Hi." }] }).html;
    expect(html.indexOf('data-part="body"')).toBeLessThan(html.indexOf('class="comments"'));
    expect(html.indexOf('class="comments"')).toBeLessThan(html.indexOf('data-part="note-bar"'));
  });
  it("puts the vision check right under the write-up and above the comment thread (0029)", () => {
    const html = pane({ visionOpen: 1, visionIds: ["V1"], comments: [{ who: "owner", at: "2026-10-12 10:00", text: "Hi." }] }).html;
    expect(html.indexOf('data-part="body"')).toBeLessThan(html.indexOf('data-part="vision"'));
    expect(html.indexOf('data-part="vision"')).toBeLessThan(html.indexOf('class="comments"'));
    expect(html).toContain("Vision <small>1 open conflict</small>");
  });
});

describe("task pane Work section (0025)", () => {
  it("sits below the description and says it's reading the transcripts until the core answers", () => {
    const { html } = pane({ stage: "building" });
    expect(html.indexOf('<section class="work" aria-label="Work">')).toBeGreaterThan(html.indexOf('<section class="body">'));
    expect(html).toContain("Reading the sessions&#x27; transcripts…");
  });
});

describe("the owner's own task in the pane (0022)", () => {
  it("is Start in To do and Done in Doing: no gate line, no Testing", () => {
    expect(pane({ forWho: "human" }).buttons).toEqual(["Start", "Send back", "Block"]);
    expect(pane({ forWho: "human", stage: "shaping" }).buttons).toEqual(["Start", "Send back", "Block"]);
    expect(pane({ forWho: "human", stage: "building", claimedBy: "owner" }).buttons).toEqual(["Done", "Send back", "Block"]);
    expect(pane({ forWho: "human", stage: "building", claimedBy: "owner" }).primary).toEqual(["Done"]);
  });
  it("never offers Release: the owner's own work has no lease", () => {
    const p = pane({ forWho: "human", stage: "building", claimedBy: "owner", heartbeat: "", modified: 0 }, 12 * 3600 + 60);
    expect(p.buttons).toEqual(["Done", "Send back", "Block"]);
  });
  it("says it's yours, with the You ribbon by the project and its role in the status line", () => {
    const p = pane({ forWho: "human" });
    expect(p.line).toBe("Ready your own task nothing waiting on you");
    expect(pane({ forWho: "human", stage: "building", claimedBy: "owner" }).line).toBe("Building LIVE on it: you your own task nothing waiting on you");
    expect(p.html).toContain('<span class="you-ribbon"');
    expect(pane({}).html).not.toContain("you-ribbon");
  });
  it("shows who does it as one chip in the settings, human or ai", () => {
    expect(pane({ forWho: "human" }).html).toContain('aria-label="For: human, click to change">human</button>');
    expect(pane({}).html).toContain('aria-label="For: ai, click to change">ai</button>');
    // A goal or feature is nobody's to do: no chip.
    expect(pane({ level: "feature" }).html).not.toContain('aria-label="For:');
  });
});

describe("keys in the pane's comments and questions (0033)", () => {
  const keys = new Map([["mu", "muster"]]);
  const t = task({
    prefix: "mu",
    comments: [{ who: "owner", at: "2026-10-13 09:00", text: "Same as mu-12, see there." }],
    questions: [{ n: 1, ticked: false, asker: "wt-a", text: "Should this wait for mu-12?", escalated: true, ownerAnswered: false }],
    openQuestions: 1,
  });
  const link = '<a href="#task/muster/0012" class="task-key">mu-12</a>';
  const pane = (withKeys: boolean) =>
    renderToStaticMarkup(createElement(TaskPanel, { task: t, now: 0, onClose: () => {}, onChanged: () => {}, ...(withKeys ? { keys, onOpenAt: () => {} } : {}) }));
  it("link a key in a comment and in an open question to its task", () => {
    const html = pane(true);
    // Once in the open question, once in the comment.
    expect(html.split(link).length - 1).toBe(2);
    expect(/<section class="answers"[\s\S]*?<\/section>/.exec(html)?.[0]).toContain(link);
  });
  it("leave them as text without the projects' prefixes", () => {
    expect(pane(false)).not.toContain('class="task-key"');
  });
});
