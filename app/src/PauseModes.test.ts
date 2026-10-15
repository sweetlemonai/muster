import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import Card from "./Card";
import { ELEMENTS, Mode, MODES, type DisplayMode } from "./modes";
import { ProjectItem } from "./ProjectList";
import type { Task } from "./types";

// Paused (0045) in every display mode (0038): a paused task never looks worked on or vanishes,
// whatever the mode hides.

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0042", title: "T", kind: "code", priority: "normal", status: "doing", stage: "building", blocked: false, stageHistory: [],
  folder: "doing", path: "doing/0042-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "building", ...over,
});
const at = (s: string) => {
  const [y, mo, d, h, mi] = s.split(/[- :]/).map(Number);
  return new Date(y, mo - 1, d, h, mi).getTime() / 1000;
};
const NOW = at("2026-10-13 12:00");
const paused = task({ claimedBy: "wt-a (wt-0045 · session 7b5e8856 · cc-hg)", stageSince: "2026-10-13 08:00", heartbeat: "2026-10-13 09:00", paused: "2026-10-13 10:00", pausedWhy: "later", modified: NOW });
const inMode = (mode: DisplayMode, el: ReactElement) => renderToStaticMarkup(createElement(Mode.Provider, { value: mode }, el));
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").trim();
const buttons = (html: string) => [...html.matchAll(/<button([^>]*)>([^<]*)<\/button>/g)].map((m) => m[2]);

describe("paused in every display mode", () => {
  it("a card keeps its Paused chip and Resume, never a quiet flag, in ADHD, Standard and Focus", () => {
    for (const { id } of MODES) {
      for (const active of [false, true]) {
        const html = inMode(id, createElement(Card, { t: paused, now: NOW, multi: false, parent: "", onOpen: () => {}, active, words: [] }));
        expect(text(html), `${id} ${active}`).toContain("was Building · paused 2 h ago · later");
        expect(buttons(html), `${id} ${active}`).toEqual(["Resume"]);
        expect(text(html), `${id} ${active}`).not.toContain("no check-in");
      }
      const ready = inMode(id, createElement(Card, { t: task({ stage: "ready" }), now: NOW, multi: false, parent: "", onOpen: () => {}, active: false, words: [] }));
      expect(buttons(ready), id).toEqual(["Pause"]);
    }
  });
  it("the sidebar row counts paused tasks in Focus's narrow sidebar too", () => {
    for (const { id } of MODES) {
      const html = inMode(id, createElement(ProjectItem, { name: "demo", total: 1, tasks: [paused], on: false, onClick: () => {}, children: null }));
      expect(html, id).toContain('<span class="side-paused" title="1 paused">⏸︎ 1</span>');
    }
  });
  it("Settings' words for Focus say so", () => {
    expect(ELEMENTS["card-lines"].focus.says).toContain("paused");
    expect(ELEMENTS.sidebar.focus.says).toContain("paused");
  });
});
