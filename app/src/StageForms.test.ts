import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { BackFormView, backTarget, BlockFormView, GateFormView, submitKey } from "./StageForms";
import type { BlockKind } from "./blockWords";
import { sentBackMsg } from "./useTaskActions";
import type { Question } from "./types";

const q = (n: number, text: string, ticked = false): Question => ({ n, ticked, asker: "wt-a", text, escalated: false, ownerAnswered: false });
const view = (kind: BlockKind | null, detail = "", questions: Question[] = [], busy = false, prefix?: string) =>
  renderToStaticMarkup(createElement(BlockFormView, { kind, detail, questions, prefix, busy, onKind: () => {}, onDetail: () => {}, onSubmit: () => {}, onCancel: () => {} }));
const chips = (html: string) => [...html.matchAll(/<button type="button" aria-pressed="(true|false)"[^>]*class="ui-pill( ui-pill--on)?">([^<]*)<\/button>/g)].map((m) => `${m[3]}${m[2] ? " (on)" : ""}`);
const block = (html: string) => /<button type="submit"([^>]*)>Block<\/button>/.exec(html)?.[1].includes("disabled") ? "disabled" : "enabled";

describe("the Block form (0024)", () => {
  it("shows the five kinds as chips, none picked yet, and asks for one", () => {
    const html = view(null);
    expect(chips(html)).toEqual(["tasks", "you", "permission", "question", "outside"]);
    expect(html).not.toContain("<select");
    expect(html).toContain("Pick why it&#x27;s blocked");
    expect(block(html)).toBe("disabled");
  });
  it("asks tasks for their ids, which go into depends", () => {
    const html = view("tasks", "0014, 0016");
    expect(chips(html)).toContain("tasks (on)");
    expect(html).toContain('placeholder="Tasks by key or number, like 14, 16"');
    expect(html).toContain("added to its depends");
    expect(block(html)).toBe("enabled");
    expect(block(view("tasks", "soon"))).toBe("disabled");
    // Its project's keys too (0033), not another project's.
    expect(block(view("tasks", "mu-14, 16", [], false, "mu"))).toBe("enabled");
    expect(block(view("tasks", "br-14", [], false, "mu"))).toBe("disabled");
  });
  it("asks you, permission and outside for words", () => {
    expect(view("you")).toContain('placeholder="What you have to do or decide"');
    expect(view("permission")).toContain('placeholder="What access it lacks, and where to grant it"');
    expect(view("outside")).toContain('placeholder="What it waits on outside"');
    expect(block(view("permission", " "))).toBe("disabled");
    expect(block(view("outside", "Apple review"))).toBe("enabled");
    expect(block(view("outside", "Apple review", [], true))).toBe("disabled");
  });
  it("lists the open questions to pick from, never a dropdown", () => {
    const html = view("question", "Q1", [q(1, "Which port?"), q(2, "Which host?", true), q(3, "Child webview or window?")]);
    expect(chips(html)).toEqual(["tasks", "you", "permission", "question (on)", "outside", "Q1 Which port? (on)", "Q3 Child webview or window?"]);
    expect(block(html)).toBe("enabled");
    expect(block(view("question", "", [q(1, "Which port?")]))).toBe("disabled");
    const none = view("question", "", [q(2, "Which host?", true)]);
    expect(none).toContain("No open question in this task: ask one first");
    expect(block(none)).toBe("disabled");
  });
});

describe("the gate form out of Building asks for the summary too (mu-78)", () => {
  const gate = (g: string, summary: string, needsSummary = true, busy = false) =>
    renderToStaticMarkup(createElement(GateFormView, { gate: g, summary, needsSummary, busy, onGate: () => {}, onSummary: () => {}, onSubmit: () => {}, onCancel: () => {} }));
  const pass = (html: string) => /<button type="submit"([^>]*)>Pass to Testing<\/button>/.exec(html)?.[1].includes("disabled") ? "disabled" : "enabled";
  it("has the gate line and the one-line summary, each labelled, the summary at most 160 characters", () => {
    const html = gate("", "");
    expect(html).toContain('aria-label="The gate&#x27;s result line"');
    expect(html).toMatch(/<input[^>]*placeholder="What was built, in one plain line for the owner&#x27;s check"[^>]*aria-label="What was built, in one line"[^>]*maxLength="160"/);
  });
  it("passes only with both for a task; a goal or feature needs no summary", () => {
    expect(pass(gate("gate: green", ""))).toBe("disabled");
    expect(pass(gate("", "Built it"))).toBe("disabled");
    expect(pass(gate("gate: green", "  "))).toBe("disabled");
    expect(pass(gate("gate: green", "Built it"))).toBe("enabled");
    expect(pass(gate("gate: green", "Built it", true, true))).toBe("disabled");
    expect(pass(gate("gate: green", "", false))).toBe("enabled");
  });
});

describe("Send back: in Your check it's the verdict, to the builder (mu-101)", () => {
  const back = (stage: string, why = "", choice = "", human = false, busy = false) =>
    renderToStaticMarkup(createElement(BackFormView, { stage, human, choice, why, busy, onChoice: () => {}, onWhy: () => {}, onSubmit: () => {}, onCancel: () => {} }));
  const options = (html: string) => [...html.matchAll(/<option value="([^"]+)"( selected="")?>/g)].map((m) => `${m[1]}${m[2] ? " (on)" : ""}`);
  const send = (html: string) => /<button type="submit"([^>]*)>([^<]*)<\/button>/.exec(html);
  it("defaults to Building from Your check, Testing still in the list; other stages one back", () => {
    expect(backTarget("your-check", false, "")).toBe("building");
    expect(backTarget("your-check", false, "testing")).toBe("testing");
    expect(backTarget("your-check", false, "done")).toBe("building");
    expect(backTarget("testing", false, "")).toBe("building");
    expect(backTarget("done", false, "")).toBe("your-check");
    expect(backTarget("ready", false, "")).toBe("shaping");
    // The owner's own task has no builder: one back, as before.
    expect(backTarget("your-check", true, "")).toBe("testing");
    expect(options(back("your-check"))).toEqual(["testing", "building (on)", "ready", "shaping", "backlog"]);
    expect(options(back("done"))).toEqual(["your-check (on)", "testing", "building", "ready", "shaping", "backlog"]);
  });
  it("asks what's wrong for the builder, labelled, right to left when it's Persian", () => {
    const html = back("your-check", "آیکون بد است");
    expect(html).toMatch(/<input[^>]*placeholder="What&#x27;s wrong \(the builder gets this\)"[^>]*aria-label="What&#x27;s wrong"[^>]*maxLength="500"[^>]*dir="auto"[^>]*value="آیکون بد است"/);
    expect(send(html)?.[2]).toBe("Send back to builder");
    // Sent to Testing instead, it's a plain send-back.
    const testing = back("your-check", "x", "testing");
    expect(testing).toContain('placeholder="Why: what&#x27;s wrong or missing"');
    expect(send(testing)?.[2]).toBe("Send back");
    // Elsewhere, the form as before.
    const done = back("done");
    expect(done).toMatch(/aria-label="Why it goes back"[^>]*dir="auto"/);
    expect(send(done)?.[2]).toBe("Send back");
  });
  it("waits for a why and while a change is in flight", () => {
    expect(send(back("your-check"))?.[1]).toContain("disabled");
    expect(send(back("your-check", "  "))?.[1]).toContain("disabled");
    expect(send(back("your-check", "the icon is bad"))?.[1]).not.toContain("disabled");
    expect(send(back("your-check", "the icon is bad", "", false, true))?.[1]).toContain("disabled");
  });
  it("says the builder gets it first, with the reason as a finding; other send-backs say where", () => {
    expect(sentBackMsg("your-check", "building")).toBe("Sent back to its builder: first in line, with what's wrong as a finding to fix or dispute.");
    expect(sentBackMsg("your-check", "testing")).toBe("Sent back to Testing.");
    expect(sentBackMsg("testing", "building")).toBe("Sent back to Building.");
  });
  it("sends on ⌘↩ (or Ctrl+↩), never mid-composition", () => {
    const ev = (key: string, metaKey: boolean, ctrlKey = false, isComposing = false) => ({ key, metaKey, ctrlKey, isComposing });
    expect(submitKey(ev("Enter", true))).toBe(true);
    expect(submitKey(ev("Enter", false, true))).toBe(true);
    expect(submitKey(ev("Enter", false))).toBe(false);
    expect(submitKey(ev("a", true))).toBe(false);
    expect(submitKey(ev("Enter", true, false, true))).toBe(false);
  });
});
