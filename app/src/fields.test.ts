import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import Comments from "./Comments";
import { estimateText, KINDS, splitLog, withoutComments } from "./fields";
import { estimateTotal } from "./lib";
import Links from "./Links";
import NewTask from "./NewTask";
import TaskPanel from "./TaskPanel";
import type { Task } from "./types";
import { ValueChip, ValueChipView } from "./Sidebar";
import { anyDirty, commentNotice, editKey, mayLeave, UNSENT, withPart } from "./writeUp";

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0017", title: "T", kind: "code", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "todo/0017-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0,
  claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});

const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/\s+/g, " ").trim();

describe("types", () => {
  it("are the core's list (ops::KINDS), in its order", () => {
    expect(KINDS).toEqual(["code", "bug", "chore", "tech-debt", "spike", "design", "docs", "research"]);
  });
  it("show only the current value until clicked", () => {
    const html = renderToStaticMarkup(createElement(ValueChip, { label: "Type", value: "tech-debt", options: KINDS, onPick: () => {} }));
    const chips = [...html.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1]);
    expect(chips).toEqual(["tech-debt"]);
    expect(html).toContain('aria-label="Type: tech-debt, click to change"');
  });
  it("show an old file's idea as it is", () => {
    const html = renderToStaticMarkup(createElement(ValueChip, { label: "Type", value: "idea", options: KINDS, onPick: () => {} }));
    expect(text(html)).toBe("idea");
    // Laid out, it's the one pressed, before the new list.
    const open = renderToStaticMarkup(createElement(ValueChipView, { label: "Type", value: "idea", options: KINDS, open: true, onOpen: () => {}, onClose: () => {}, onPick: () => {} }));
    const chips = [...open.matchAll(/<button[^>]*aria-pressed="(true|false)"[^>]*>([^<]*)<\/button>/g)].map((m) => `${m[2]}${m[1] === "true" ? " (on)" : ""}`);
    expect(chips).toEqual(["idea (on)", ...KINDS]);
  });
  it("in New task: every type as a chip, code picked, and a test time field", () => {
    const html = renderToStaticMarkup(createElement(NewTask, { queues: ["demo"], initial: "demo", onClose: () => {}, onCreated: () => {} }));
    const row = /<div class="chip-row" role="group" aria-label="Type">([\s\S]*?)<\/div>/.exec(html)?.[1] ?? "";
    const chips = [...row.matchAll(/<button[^>]*aria-pressed="(true|false)"[^>]*>([^<]*)<\/button>/g)].map((m) => `${m[2]}${m[1] === "true" ? " (on)" : ""}`);
    expect(chips).toEqual(["code (on)", "bug", "chore", "tech-debt", "spike", "design", "docs", "research"]);
    expect(text(html)).toContain("Test time");
    expect(html).not.toContain(">Idea<");
  });
  it("in the task pane: a type chip, not a select", () => {
    const html = renderToStaticMarkup(createElement(TaskPanel, { task: task({ kind: "bug" }), now: 0, onClose: () => {}, onChanged: () => {} }));
    expect(html).toContain('aria-label="Type: bug, click to change"');
    expect(html).not.toMatch(/<option value="idea">/);
    expect(html).not.toMatch(/<option value="task">/);
  });
});

describe("estimates", () => {
  it("read agent time, test time and tokens, in that order, only the parts set", () => {
    expect(estimateText(task({ estTime: "2h", estTestTime: "30m", estTokens: "400k" }))).toBe("agent 2h · test 30m · 400k");
    expect(estimateText(task({ estTestTime: "1h" }))).toBe("test 1h");
    expect(estimateText(task({ estTime: "45m", estTokens: "150k" }))).toBe("agent 45m · 150k");
    expect(estimateText(task({}))).toBe("");
  });
  it("totals count test time with agent time", () => {
    const t = (m: number, tm: number, k: number, stage = "ready") => ({ estMinutes: m, estTestMinutes: tm, estTokenCount: k, stage });
    expect(estimateTotal([t(120, 30, 0), t(0, 0, 0)])).toBe("≈ 2.5 h left (1 of 2 open tasks estimated)");
    expect(estimateTotal([t(0, 60, 0)])).toBe("≈ 1 h left");
    expect(estimateTotal([t(0, 0, 0)])).toBe("");
  });
});

describe("the write-up without its comments", () => {
  const comments = "## Comments\n- **owner, 2026-10-12 10:00:** hi\n  ## Log\n  more\n\n";
  it("leaves out the tool's Comments section, the one above the last Log", () => {
    expect(withoutComments(`# T\n\nText.\n\n${comments}## Log\n- x\n`)).toBe("# T\n\nText.\n\n## Log\n- x\n");
  });
  it("takes the last Comments above the Log, and works without a Log", () => {
    expect(withoutComments(`# T\n\n${comments}`)).toBe("# T\n\n");
    expect(withoutComments("# T\n\n## Log\n- x\n\n## Comments\nafter the log\n")).toBe("# T\n\n## Log\n- x\n\n## Comments\nafter the log\n");
    expect(withoutComments("# T\n\nNo comments.\n\n## Log\n- x\n")).toBe("# T\n\nNo comments.\n\n## Log\n- x\n");
  });
  it("splits at the last Log, so the thread can sit between the write-up and the Log", () => {
    expect(splitLog("# T\n\n## Log\n- quoted\n\nText.\n\n## Log\n- x\n")).toEqual(["# T\n\n## Log\n- quoted\n\nText.\n\n", "## Log\n- x\n"]);
    expect(splitLog("# T\n\nNo log.")).toEqual(["# T\n\nNo log.", ""]);
  });
  it("keeps a section that follows the Comments, and CRLF files", () => {
    expect(withoutComments(`# T\n\n${comments}## Notes\nkept\n\n## Log\n`)).toBe("# T\n\n## Notes\nkept\n\n## Log\n");
    expect(withoutComments("# T\r\n\r\n## Comments  \r\n- **o, 2026-10-12 10:00:** a\r\n\r\n## Log\r\n")).toBe("# T\r\n\r\n## Log\r\n");
  });
});

describe("links", () => {
  it("list each one with its title, and nothing when there are none", () => {
    expect(renderToStaticMarkup(createElement(Links, { task: task({}) }))).toBe("");
    const html = renderToStaticMarkup(createElement(Links, {
      task: task({ links: [{ title: "The spec", url: "https://example.com/spec" }, { title: "", url: "https://github.com/o/r/pull/7" }] }),
    }));
    const buttons = [...html.matchAll(/<button[^>]*title="([^"]*)"[^>]*>([^<]*)<\/button>/g)].map((m) => [m[2], m[1]]);
    expect(buttons).toEqual([
      ["The spec", "https://example.com/spec"],
      ["github.com/o/r/pull/7", "https://github.com/o/r/pull/7"],
    ]);
  });
  it("show one that isn't https without a way to open it", () => {
    const html = renderToStaticMarkup(createElement(Links, { task: task({ links: [{ title: "Old", url: "http://example.com" }] }) }));
    expect(html).not.toContain("<button");
    expect(text(html)).toContain("Old http://example.com (not https, so it won't open)");
  });
});

describe("comments", () => {
  it("show the thread oldest first, then the box", () => {
    const html = renderToStaticMarkup(createElement(Comments, {
      task: task({ comments: [{ who: "owner", at: "2026-10-12 10:00", text: "First." }, { who: "wt-a", at: "2026-10-12 11:00", text: "Second\n- a list" }] }),
      onChanged: () => {},
    }));
    expect(html.indexOf("First.")).toBeLessThan(html.indexOf("Second"));
    expect(text(html)).toContain("owner · 2026-10-12 10:00");
    expect(html).toContain("<li>a list</li>");
    expect(html).toContain('placeholder="Add a comment (⌘↩ to send)"');
    expect(html).toContain('maxLength="4000"');
    expect(html.indexOf("Second")).toBeLessThan(html.indexOf("<textarea"));
  });
  it("say there are none yet, with the box still there", () => {
    const html = renderToStaticMarkup(createElement(Comments, { task: task({}), onChanged: () => {} }));
    expect(text(html)).toContain("No comments yet.");
    expect(html).toContain("<textarea");
  });
  it("render a comment's text as data: no raw HTML", () => {
    const html = renderToStaticMarkup(createElement(Comments, { task: task({ comments: [{ who: "x", at: "2026-10-12 10:00", text: "<img src=x onerror=alert(1)>" }] }), onChanged: () => {} }));
    expect(html).not.toContain("<img");
  });
  it("keep typed text: the pane is unsaved while either the editor or the comment box has some", () => {
    let parts: Record<string, boolean> = {};
    expect(anyDirty(parts)).toBe(false);
    parts = withPart(parts, "comment", true);
    expect(anyDirty(parts)).toBe(true);
    // The editor closing (clean) doesn't clear the comment box's typing.
    parts = withPart(parts, "edit", false);
    expect(anyDirty(parts)).toBe(true);
    expect(withPart(parts, "edit", false)).toBe(parts);
    parts = withPart(parts, "comment", false);
    expect(anyDirty(parts)).toBe(false);
    // Unsaved typing keeps the pane: Esc (closing) and another card are refused.
    const here = { kind: "task", key: "demo/0017" };
    expect(mayLeave(anyDirty({ comment: true }), here, null)).toBe(false);
    expect(mayLeave(anyDirty({ comment: true }), here, { kind: "task", key: "demo/0018" })).toBe(false);
  });
  it("arm on the first Esc and say so inline; the second throws the text away", () => {
    const esc = { key: "Escape", metaKey: false, ctrlKey: false, altKey: false, isComposing: false };
    expect(editKey(esc, true, false)).toBe("arm");
    expect(editKey(esc, true, true)).toBe("cancel");
    expect(commentNotice(true, true, false)).toBe(UNSENT);
    expect(commentNotice(true, false, true)).toBe(UNSENT);
    expect(commentNotice(true, false, false)).toBeNull();
    expect(commentNotice(false, true, true)).toBeNull();
    expect(UNSENT).toBe("Unsent comment: send it (⌘↩), or press Esc twice to throw it away");
  });
  it("say nothing unsent while the box is empty, even after a refused switch", () => {
    const html = renderToStaticMarkup(createElement(Comments, { task: task({}), onChanged: () => {}, nudge: 3 }));
    expect(html).not.toContain("Unsent comment");
  });
  it("sit under the description in the task pane", () => {
    const html = renderToStaticMarkup(createElement(TaskPanel, { task: task({ comments: [{ who: "owner", at: "2026-10-12 10:00", text: "Hi." }] }), now: 0, onClose: () => {}, onChanged: () => {} }));
    expect(html.indexOf('class="body"')).toBeLessThan(html.indexOf('class="comments"'));
  });
});
