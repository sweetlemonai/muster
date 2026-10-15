import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { answerId, focusAnswer } from "./answerFocus";
import type { Question, Task } from "./types";
import WaitingList, { BlockLine, counted, fromShaping, unblockNow, waitRank } from "./WaitingList";
import source from "./WaitingList.tsx?raw";

const task = (id: string, over: Partial<Task> = {}): Task => ({
  queue: "muster", id, title: `Task ${id}`, kind: "code", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: `todo/${id}-t.md`, claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});
const q = (n: number, text: string, over: Partial<Question> = {}): Question => ({ n, ticked: false, asker: "wt-a", text, escalated: true, ownerAnswered: false, ...over });
const pick: Question = q(2, "which colour for the button?\n(a) red\n(b) blue", { choices: { options: ["red", "blue"], pick: 1, noAnswer: 1, question: "which colour for the button?" } });
const answer = task("0001", { blocked: true, blockedKind: "question", blockDetail: "Q2", questions: [pick], waitingForOwner: true });
const action = task("0002", { blocked: true, blockedKind: "you", blockDetail: "set the API key", waitingForOwner: true });
const permission = task("0003", { blocked: true, blockedKind: "permission", blockDetail: "repo write", waitingForOwner: true });
const asked = task("0004", { questions: [q(1, "which port?")], openQuestions: 1, waitingForOwner: true });
const own = task("0005", { forWho: "human", stage: "ready" });
const html = (tasks: Task[]) => renderToStaticMarkup(createElement(WaitingList, { tasks, multi: false, openKey: null, onOpen: () => {}, onChanged: () => {}, now: 0 }));
const order = (out: string) => [...out.matchAll(/(\d{4})<\/span> · Task/g)].map((m) => m[1]);

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
    expect(out).toContain('<b>Your answer</b>: Q2 which colour for the button? <button type="button" class="ui-button block-answer">Answer</button>');
    expect(out).toContain('<b>Your action</b>: set the API key</span></p><button type="button" class="ui-button block-unblock">Done, unblock</button>');
    expect(out).toContain('<b>Permission</b>: repo write</span></p><button type="button" class="ui-button block-unblock">Granted, unblock</button>');
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
    expect(open).toContain("<textarea");
    // Only the pane's box has the id Answer focuses: never two with one id when both are open.
    expect(open).not.toContain(`id="${answerId(answer, 2)}"`);
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

describe("the shapers' batch in Waiting for you (mu-103)", () => {
  const shaped = task("0011", { stage: "shaping", questions: [pick, q(3, "which port?"), q(4, "not sent yet", { escalated: false }), q(5, "answered", { ticked: true })], waitingForOwner: true });
  it("groups Shaping and Ready tasks with sent questions under one heading that counts them", () => {
    const out = html([action, check(), asked, shaped]);
    const group = /<div class="effort-group effort-other from-shaping" role="group" aria-label="From shaping">([\s\S]*?)<\/div><div class="effort-group/.exec(out)?.[1] ?? "";
    expect(group).toContain('<h3 class="effort-head">From shaping: 3 questions on 2 tasks</h3>');
    // Both tasks are in it, in the order given, and not again in the band below.
    expect(order(group)).toEqual(["0004", "0011"]);
    expect(out.match(/0011<\/span> · Task/g)).toHaveLength(1);
    // Each question keeps its chips, the default in words, answered in place.
    expect(group).toContain('aria-label="Options for Q2"');
    expect(group).toContain("going with (b) unless you answer");
    expect(group).not.toContain("not sent yet");
    // It comes first: above what blocks agents and Your check.
    expect(out.indexOf("From shaping")).toBeLessThan(out.indexOf('aria-label="Questions, blocks and your own tasks"'));
  });
  it("leaves out started, blocked and the owner's own tasks, and shows no heading without a batch", () => {
    expect([shaped, asked].map(fromShaping)).toEqual([true, true]);
    expect(fromShaping({ ...shaped, stage: "building" })).toBe(false);
    expect(fromShaping(answer)).toBe(false);
    expect(fromShaping({ ...shaped, forWho: "human" })).toBe(false);
    // A blocked task waits in its own band; muster clarify leaves it out alike (R7).
    expect(fromShaping({ ...shaped, blocked: true, blockedKind: "you", blockDetail: "x" })).toBe(false);
    expect(fromShaping({ ...shaped, questions: [q(1, "x", { escalated: false })] })).toBe(false);
    expect(html([action])).not.toContain("From shaping");
    expect(counted(1, 1)).toBe("1 question on 1 task");
  });
});

const check = () => task("0010", { stage: "your-check", waitingForOwner: true });

describe("what blocks agents comes first (0060, QA)", () => {
  it("puts questions, blocks and your own tasks above Your check's groups", () => {
    const check = task("0010", { stage: "your-check", waitingForOwner: true });
    const out = html([check, action]);
    const band = out.indexOf('aria-label="Questions, blocks and your own tasks"');
    expect(band).toBeGreaterThanOrEqual(0);
    expect(out.indexOf("0010</span> · Task")).toBeGreaterThan(band);
    expect(out.indexOf("0002</span> · Task")).toBeLessThan(out.indexOf("0010</span> · Task"));
  });
});

describe("Waiting for you stays out of the startup chunk's way (0060 review)", () => {
  it("takes the answer box from its own file, not from the lazy task pane", () => {
    expect(source).not.toMatch(/from "\.\/TaskPanel"/);
    expect(source).toMatch(/from "\.\/AnswerBox"/);
  });
});

describe("Waiting for you says what you're accepting (mu-78)", () => {
  const proven = (n: number) => ({ n, label: `AC${n}`, text: `AC${n}: works — verify: command`, ticked: true, owner: false, proof: "ok", proofFiles: [] });
  it("says All N proven: Accept with the summary under it, for every queue", () => {
    const check = task("0010", { queue: "ai", stage: "your-check", waitingForOwner: true, criteria: [1, 2, 3, 4].map(proven), summary: "A chat you step through; works <offline>" });
    const out = html([check]);
    expect(out).toContain('<p class="next-step-line">All 4 proven: Accept</p><p class="summary-line" dir="auto" title="A chat you step through; works &lt;offline&gt;">A chat you step through; works &lt;offline&gt;</p>');
  });
  it("adds the summary under a step of yours, and nothing without one", () => {
    const open = { n: 2, label: "AC2", text: "AC2: the owner opens the board. — verify: owner", ticked: false, owner: true };
    const out = html([task("0011", { stage: "your-check", waitingForOwner: true, criteria: [open], ownerCriteria: [open], summary: "Built it" })]);
    expect(out).toContain('<p class="next-step-line">Try: open the board</p><p class="summary-line" dir="auto" title="Built it">Built it</p>');
    expect(html([task("0012", { stage: "your-check", waitingForOwner: true, criteria: [proven(1)] })])).not.toContain("summary-line");
  });
});
