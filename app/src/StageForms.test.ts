import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { BlockFormView } from "./StageForms";
import type { BlockKind } from "./blockWords";
import type { Question } from "./types";

const q = (n: number, text: string, ticked = false): Question => ({ n, ticked, asker: "wt-a", text, escalated: false, ownerAnswered: false });
const view = (kind: BlockKind | null, detail = "", questions: Question[] = [], busy = false, prefix?: string) =>
  renderToStaticMarkup(createElement(BlockFormView, { kind, detail, questions, prefix, busy, onKind: () => {}, onDetail: () => {}, onSubmit: () => {}, onCancel: () => {} }));
const chips = (html: string) => [...html.matchAll(/<button type="button" class="chip( on)?" aria-pressed="(true|false)"[^>]*>([^<]*)<\/button>/g)].map((m) => `${m[3]}${m[1] ? " (on)" : ""}`);
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
