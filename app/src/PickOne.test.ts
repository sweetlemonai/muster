import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { answeredText, chipsOff, letter, pickArgs, sendPick } from "./PickOne";
import TaskPanel from "./TaskPanel";
import type { Question, Task } from "./types";
import WaitingList from "./WaitingList";

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0007", title: "T", kind: "code", priority: "normal", status: "doing", stage: "building", blocked: false, stageHistory: [],
  folder: "doing", path: "doing/0007-t.md", claimedBy: "wt-a", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "building", ...over,
});
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/\s+/g, " ").trim();

/** Q2, escalated to the owner, asked with three options: the asker picks (a), and does (c)
 *  with no answer. `text` is the question as written, the option lines included. */
const LEASE: Question = {
  n: 2, ticked: false, asker: "wt-a, session aaaa1111, 2026-10-13 09:00", escalated: true, ownerAnswered: false,
  text: "How long may a lease be?\n- (a) 1 h to 7 d (my pick)\n- (b) Any length\n- (c) No lease\nWith no answer: (c)\n**Escalated to owner (capture, 2026-10-13 09:05).**",
  choices: { options: ["1 h to 7 d", "Any length", "No lease"], pick: 0, noAnswer: 2, question: "How long may a lease be?\n**Escalated to owner (capture, 2026-10-13 09:05).**" },
};
const PORT: Question = { n: 1, ticked: false, asker: "wt-a", text: "Which port?", escalated: true, ownerAnswered: false };

/** The chips in a rendered `pick-one` part: each one's text, and whether it's a button that answers. */
const chips = (html: string) =>
  [...html.matchAll(/<button type="button"([^>]*) class="ui-pill pick-chip[^"]*">([\s\S]*?)<\/button>/g)].map((m) => `${text(m[2])}${m[1].includes("disabled") ? " (disabled)" : ""}`);
const pickPart = (html: string) => /<div class="pick-one"[^>]*>([\s\S]*?)<\/div>/.exec(html)?.[1] ?? "";

describe("pick-one questions in the task pane (0049)", () => {
  const pane = (questions: Question[]) =>
    renderToStaticMarkup(createElement(TaskPanel, { task: task({ questions, openQuestions: questions.length }), now: 0, onClose: () => {}, onChanged: () => {} }));

  it("shows every option as a chip that answers, the pick marked and the fallback named", () => {
    const html = pane([LEASE]);
    expect(chips(html)).toEqual(["(a) 1 h to 7 d pick", "(b) Any length", "(c) No lease"]);
    expect(text(pickPart(html))).toContain("with no answer: c");
    expect(html).toContain('aria-label="Options for Q2"');
  });
  it("shows the question without its option lines, still orange as needs you, with the box below for something else", () => {
    const html = pane([LEASE]);
    const answers = /<section class="answers"[^>]*>([\s\S]*?)<\/section>/.exec(html)?.[1] ?? "";
    expect(answers).toContain('<div class="answer needs-you">');
    expect(text(answers)).toContain("How long may a lease be?");
    expect(answers).not.toContain("- (a)");
    expect(answers).not.toContain("With no answer: (c)");
    // The chips come before the box, which still answers in words.
    expect(answers.indexOf("pick-one")).toBeLessThan(answers.indexOf("<textarea"));
    expect(answers).toMatch(/<button disabled="" class="ui-button">Answer<\/button>/);
    expect(answers).toContain('placeholder="Something else, or words to add to a pick (⌘↩ sends them as your answer)"');
  });
  it("leaves a question in words as it was: no chips, its own placeholder", () => {
    const html = pane([PORT]);
    expect(html).not.toContain("pick-one");
    expect(html).toContain('placeholder="Your answer (⌘↩ to send)"');
  });
});

describe("pick-one questions in Waiting for you (0049)", () => {
  const list = (questions: Question[]) =>
    renderToStaticMarkup(createElement(WaitingList, { tasks: [task({ questions, openQuestions: questions.length })], multi: false, openKey: null, onOpen: () => {}, onChanged: () => {}, now: 0 }));

  it("lists the options as chips under the question, so one click answers there too", () => {
    const html = list([LEASE, PORT]);
    expect(chips(html)).toEqual(["(a) 1 h to 7 d pick", "(b) Any length", "(c) No lease"]);
    const item = text(/<article[^>]*>([\s\S]*?)<\/article>/.exec(html)?.[1] ?? "");
    expect(item).toBe(
      "0007 · T Q2 How long may a lease be? **Escalated to owner (capture, 2026-10-13 09:05).** (a) 1 h to 7 d pick (b) Any length (c) No lease with no answer: c Q1 Which port? Open to answer",
    );
  });
});

describe("what a click sends", () => {
  it("is the option's letter and the box's words, for this task and question", () => {
    const t = task({});
    expect(pickArgs(t, LEASE, 1, "  but warn after 7 d ")).toEqual(["demo", "0007", 2, "b", "but warn after 7 d"]);
    expect(pickArgs(t, LEASE, 0, "")).toEqual(["demo", "0007", 2, "a", ""]);
    expect([0, 1, 2, 3, 4, 5].map(letter).join("")).toBe("abcdef");
  });
  it("sends the pick to the core once and says what came of it (review: one click, one answer)", async () => {
    const calls: unknown[][] = [];
    const ok = async (...args: unknown[]) => (calls.push(args), "aaaa1111");
    const at = new Date(2026, 9, 13, 9, 30);
    expect(await sendPick(ok, task({}), LEASE, 2, " now ", at)).toEqual({ ok: true, text: "Answered. The asking agent gets it after its next step." });
    expect(calls).toEqual([["demo", "0007", 2, "c", "now", "2026-10-13 09:30"]]);
    const refused = async () => {
      throw "Q2 is already answered";
    };
    expect(await sendPick(refused, task({}), LEASE, 0, "", at)).toEqual({ ok: false, text: "Q2 is already answered" });
    expect(await sendPick(ok, task({}), { ...LEASE, n: null }, 0, "", at)).toEqual({ ok: false, text: "This question has no number, so it can't be answered here." });
    expect(calls.length).toBe(1);
  });
  it("keeps the chips off once a pick is answered, and while one is on its way", () => {
    expect(chipsOff({ busy: false, result: null }, 2)).toBe(false);
    expect(chipsOff({ busy: true, result: null }, 2)).toBe(true);
    expect(chipsOff({ busy: false, result: { ok: true, text: "Answered." } }, 2)).toBe(true);
    // A refusal leaves them on, to try another.
    expect(chipsOff({ busy: false, result: { ok: false, text: "no" } }, 2)).toBe(false);
    expect(chipsOff({ busy: false, result: null }, null)).toBe(true);
  });
  it("says whether the asking session gets it", () => {
    expect(answeredText("aaaa1111")).toBe("Answered. The asking agent gets it after its next step.");
    expect(answeredText(null)).toBe("Answered, and written in the task. No asking session on record, so it waits there.");
  });
});
