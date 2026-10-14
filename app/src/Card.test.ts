import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import Card from "./Card";
import type { Question, Task } from "./types";

const task = (id: string, over: Partial<Task> = {}): Task => ({
  queue: "muster", id, title: `Task ${id}`, kind: "code", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: `todo/${id}-t.md`, claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});
const q = (n: number, text: string): Question => ({ n, ticked: false, asker: "wt-a", text, escalated: true, ownerAnswered: false });
const card = (t: Task) => renderToStaticMarkup(createElement(Card, { t, now: 0, multi: false, parent: "", onOpen: () => {}, active: false, words: [] }));
const classOf = (html: string) => /^<button class="([^"]*)"/.exec(html)?.[1] ?? "";
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").trim();

describe("a card on the one board (0022, owner 2026-10-13)", () => {
  it("draws the owner's own task with the human look: the hatch class and the You ribbon", () => {
    const html = card(task("0041", { forWho: "human", title: "Buy the domain" }));
    expect(classOf(html).split(" ")).toContain("human");
    expect(html).toMatch(/^<button class="[^"]*"><span class="you-ribbon"[^>]*>You<\/span>/);
    // An agent's task carries nothing extra.
    const agent = card(task("0053"));
    expect(classOf(agent).split(" ")).not.toContain("human");
    expect(agent).not.toContain("you-ribbon");
  });
  it("shows Your check, an escalated question and a block on you as needs-you cards, where they are", () => {
    const needs = [
      task("0050", { stage: "your-check", waitingForOwner: true }),
      task("0051", { stage: "building", questions: [q(2, "Child webview or a window?")], openQuestions: 1, waitingForOwner: true }),
      task("0052", { blocked: true, waitingOn: "owner: which colour?", blockedKind: "you", blockDetail: "which colour?", waitingForOwner: true }),
      task("0053", { blocked: true, blockedKind: "permission", blockDetail: "repo write", waitingForOwner: true }),
    ];
    for (const t of needs) {
      const html = card(t);
      expect(classOf(html).split(" "), t.id).toContain("needs-you");
      expect(html, t.id).toContain('<span class="ui-badge ui-badge--warn">needs you</span>');
    }
    expect(text(card(needs[1]))).toContain("1 open Q");
    expect(card(needs[2])).toContain("<b>Your action</b>: which colour?");
    expect(card(needs[3])).toContain("<b>Permission</b>: repo write");
    // Agents' work that doesn't wait on the owner isn't a needs-you card.
    expect(card(task("0060", { stage: "building", claimedBy: "wt-b" }))).not.toContain("needs you");
  });
  it("says an agent's task waiting on the owner's unfinished task is blocked by it, in 0024's one sentence (AC8)", () => {
    const html = card(task("0054", { depends: ["0041"], blockedKind: "tasks", blockedBy: [{ id: "0041", stage: "ready", holder: "" }] }));
    // Neutral, not red: it clears by itself (0060).
    expect(classOf(html).split(" ")).not.toContain("blocked");
    expect(html).toContain('<span class="ui-badge block-why not-you look-tasks"><b>After</b> 0041</span>');
    expect(html.match(/0041/g)).toHaveLength(1);
    expect(html).not.toContain("blocked by yours");
    expect(html).not.toContain("waits-you");
  });
});

describe("a card names its task by its key (0033)", () => {
  it("shows mu-12, not the file's 0012, and marks it when the search names it", () => {
    const t = task("0012", { prefix: "mu", title: "Keys" });
    expect(card(t)).toContain('<span class="id" title="muster · mu-12">mu-12</span>');
    expect(card(t)).not.toContain("0012");
    const searched = (words: string[]) => renderToStaticMarkup(createElement(Card, { t, now: 0, multi: false, parent: "", onOpen: () => {}, active: false, words }));
    for (const words of [["12"], ["mu-0012"]]) expect(searched(words), words.join()).toContain('<span class="id" title="muster · mu-12"><mark>mu-12</mark></span>');
    expect(searched(["br-0012"])).toContain('<span class="id" title="muster · mu-12">mu-12</span>');
    // The tasks it waits on, by their keys too.
    const blocked = task("0013", { prefix: "mu", blockedKind: "tasks", blockedBy: [{ id: "0012", stage: "ready", holder: "" }] });
    expect(card(blocked)).toContain("<b>After</b> mu-12");
  });
  it("shows the id as it is when the task has no prefix", () => {
    expect(card(task("0012"))).toContain('<span class="id" title="muster · 0012">0012</span>');
  });
});

describe("a Done card (0051)", () => {
  // The fake clock: 14:00 local time on 2026-10-13.
  const NOW = new Date(2026, 9, 13, 14, 0).getTime() / 1000;
  const at = (t: Task) => renderToStaticMarkup(createElement(Card, { t, now: NOW, multi: false, parent: "", onOpen: () => {}, active: false, words: [] }));
  it("says when it was done, from the log, with the stamp in its tooltip", () => {
    expect(at(task("0050", { stage: "done", doneAt: "2026-10-13 13:56" }))).toContain('<span class="card-done" title="Done 2026-10-13 13:56">done 4 min ago</span>');
    expect(at(task("0051", { stage: "done", doneAt: "2026-10-12 18:20" }))).toContain(">done yesterday 18:20<");
    expect(at(task("0052", { stage: "done", doneOn: "2026-10-12" }))).toContain('<span class="card-done" title="Done 2026-10-12">done yesterday</span>');
  });
  it("says nothing when the log doesn't know, or the task isn't done", () => {
    expect(at(task("0053", { stage: "done" }))).not.toContain("card-done");
    expect(at(task("0054", { stage: "building", doneAt: "2026-10-13 13:56" }))).not.toContain("card-done");
  });
});
