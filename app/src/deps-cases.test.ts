import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import cases from "./deps-cases.json";
import { blockedRed, blockSentence, isBlocked } from "./blockWords";
import { cardClass } from "./CardLook";
import { readyToStart } from "./graph";
import { GraphView } from "./GraphView";
import type { Task } from "./types";

// The app's half of the shared dependency cases (0042): tests/deps_cases.rs writes the same tasks
// as files and holds `muster list`, `check`, `claim` and `next` to the same answers, so the board,
// the Graph can't drift from the core on what is blocked.

type Want = { blockedBy: { id: string; stage: string; holder: string }[]; ready: boolean };
const want = cases.want as Record<string, Want>;
const sentences = cases.sentences as Record<string, string>;

/** A task as the core gives it to the window: the file's fields, plus what it derives. */
const tasks: Task[] = cases.tasks.map((f) => {
  const by = want[f.id].blockedBy;
  return {
    queue: "q", id: f.id, title: f.title, kind: "code", priority: "normal", status: "", stage: f.stage, blocked: f.waitingOn !== "", stageHistory: [], folder: "", path: "",
    claimedBy: f.claimedBy, heldBy: f.heldBy, stageSince: "", checkins: [], heartbeat: "", waitingOn: f.waitingOn,
    blockedKind: f.waitingOn !== "" || by.length ? "tasks" : "", blockDetail: f.waitingOn, blockedBy: by,
    depends: f.depends, forWho: f.for, commit: "", captured: "", capturedAt: "", doneAt: "",
    blockHistory: [], questions: [], openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: f.stage === "your-check", tags: [],
    estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
    attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: f.stage,
  };
});

describe("the shared dependency cases (the core gives the same)", () => {
  for (const t of tasks) {
    const w = want[t.id];
    it(`${t.id} ${t.title}`, () => {
      // The board: the card's red (not for a wait on tasks alone, 0060), and its words.
      expect(isBlocked(t)).toBe(t.blocked || w.blockedBy.length > 0);
      expect(cardClass(t, false).includes(" blocked")).toBe(blockedRed(t));
      if (sentences[t.id]) expect(blockSentence(t)).toBe(sentences[t.id]);
      if (!isBlocked(t)) expect(blockSentence(t)).toBe("");
      // The Graph's ready mark is exactly what the core lets an agent claim (or the owner start).
      expect(readyToStart(t)).toBe(w.ready);
    });
  }
  it("the Graph marks ready and blocked as the core says", () => {
    const html = renderToStaticMarkup(createElement(GraphView, { tasks, all: tasks, multi: false, openKey: null, onOpen: () => {} }));
    for (const t of tasks) {
      const node = new RegExp(`class="(g-node|ui-pill(?: ui-pill--on)? g-loose) [^"]*"[^>]*><(span class="g-top"><span class="id"|span class="id")>${t.id}<`).exec(html)?.[0] ?? "";
      expect(node, t.id).not.toBe("");
      const status = html.slice(html.indexOf(node), html.indexOf("</button>", html.indexOf(node)));
      expect(status.includes('class="g-ready"'), `${t.id} ready`).toBe(want[t.id].ready);
      expect(status.includes("ui-badge ui-badge--blocked"), `${t.id} blocked`).toBe(blockedRed(t));
    }
  });
});
