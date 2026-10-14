import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { answerId, focusAnswer } from "./answerFocus";
import type { Question, Task } from "./types";
import WaitingList, { BlockLine, unblockNow, waitRank } from "./WaitingList";

const task = (id: string, over: Partial<Task> = {}): Task => ({
  queue: "muster", id, title: `Task ${id}`, kind: "code", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: `todo/${id}-t.md`, claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});
const q = (n: number, text: string, over: Partial<Question> = {}): Question => ({ n, ticked: false, asker: "wt-a", text, escalated: true, ownerAnswered: false, ...over });
const pick: Question = q(2, "which colour for the button?\n(a) red\n(b) blue", { choices: { options: ["red", "blue"], pick: 1, noAnswer: 1, question: "which colour for the button?" } });
const answer = task("0001", { blocked: true, blockedKind: "question", blockDetail: "Q2", questions: [pick], waitingForOwner: true });
const action = task("0002", { blocked: true, blockedKind: "you", blockDetail: "set the API key", waitingForOwner: true });
const permission = task("0003", { blocked: true, blockedKind: "permission", blockDetail: "repo write", waitingForOwner: true });
const asked = task("0004", { questions: [q(1, "which port?")], openQuestions: 1, waitingForOwner: true });
const own = task("0005", { forWho: "human", stage: "ready" });
const html = (tasks: Task[]) => renderToStaticMarkup(createElement(WaitingList, { tasks, multi: false, openKey: null, onOpen: () => {}, onChanged: () => {}, now: 0 }));
const order = (out: string) => [...out.matchAll(/(\d{4}) · Task/g)].map((m) => m[1]);

describe("Waiting for you, by what the owner has to do (0060)", () => {
  it("ranks answers, then actions, then permissions, then the rest", () => {
    expect([answer, asked, action, permission, own].map(waitRank)).toEqual([0, 0, 1, 2, 3]);
    // A question capture still has isn't the owner's to answer yet; tasks and outside never are.
    expect(waitRank(task("0006", { blocked: true, blockedKind: "question", blockDetail: "Q1", questions: [q(1, "x", { escalated: false })] }))).toBe(3);
    expect(waitRank(task("0007", { blockedKind: "tasks", blockedBy: [{ id: "0001", stage: "ready", holder: "" }] }))).toBe(3);
  });
  it("lists answers first, then actions, then permissions, each with its button", () => {
    const out = html([own, permission, action, asked, answer]);
    expect(order(out)).toEqual(["0004", "0001", "0002", "0003", "0005"]);
    expect(out).toContain('<b>Your answer</b>: Q2 which colour for the button? <button type="button" class="block-answer">Answer</button>');
    expect(out).toContain('<b>Your action</b>: set the API key</span></p><button type="button" class="block-unblock">Done, unblock</button>');
    expect(out).toContain('<b>Permission</b>: repo write</span></p><button type="button" class="block-unblock">Granted, unblock</button>');
    // The question the block names is said once, in the block's line, not again above it.
    expect(out.match(/which colour for the button\?/g)).toHaveLength(1);
  });
  it("never lists a task that only waits on other tasks or something outside as the owner's", () => {
    const after = task("0008", { blockedKind: "tasks", blockedBy: [{ id: "0001", stage: "ready", holder: "" }], waitingForOwner: false });
    const out = renderToStaticMarkup(createElement(BlockLine, { t: after, answering: false, onAnswer: () => {}, onChanged: () => {} }));
    expect(out).toBe("");
    const outside = task("0009", { blocked: true, blockedKind: "outside", blockDetail: "Apple review" });
    expect(renderToStaticMarkup(createElement(BlockLine, { t: outside, answering: false, onAnswer: () => {}, onChanged: () => {} }))).toBe("");
  });
});

describe("a question block's Answer opens the answer box in place (0060)", () => {
  const line = (answering: boolean) => renderToStaticMarkup(createElement(BlockLine, { t: answer, answering, onAnswer: () => {}, onChanged: () => {} }));
  it("shows the box right under the block's line, with the question's options as chips", () => {
    expect(line(false)).not.toContain("<textarea");
    const open = line(true);
    expect(open).toContain(`<textarea id="${answerId(answer, 2)}"`);
    expect(open.indexOf("<textarea")).toBeGreaterThan(open.indexOf("Your answer"));
    expect(open).toContain("<b>(a)</b> red");
    expect(open).toContain("<b>(b)</b> blue");
  });
  it("in the pane, takes the owner to the question's box there", () => {
    const box = { scrollIntoView: vi.fn(), focus: vi.fn() };
    const getElementById = vi.fn((id: string) => (id === answerId(answer, 2) ? (box as unknown as HTMLElement) : null));
    focusAnswer(answer, 2, { getElementById });
    expect(box.scrollIntoView).toHaveBeenCalledWith({ block: "nearest" });
    expect(box.focus).toHaveBeenCalledOnce();
    // Not on screen: nothing to do, and nothing thrown.
    expect(() => focusAnswer(answer, 9, { getElementById })).not.toThrow();
  });
});

describe("Done, unblock (0060)", () => {
  it("unblocks the task through the core and says so", async () => {
    const send = vi.fn(async () => "unblocked");
    expect(await unblockNow(send, action, new Date(2026, 9, 13, 18, 30))).toEqual({ ok: true, text: "Unblocked." });
    expect(send).toHaveBeenCalledWith("muster", "0002", "2026-10-13 18:30");
  });
  it("shows the core's refusal, and leaves it to try again", async () => {
    const send = vi.fn(async () => {
      throw new Error("0002 isn't blocked");
    });
    expect(await unblockNow(send, action, new Date(2026, 9, 13, 18, 30))).toEqual({ ok: false, text: "Error: 0002 isn't blocked" });
  });
});
