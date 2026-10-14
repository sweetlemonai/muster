import { createElement, type ReactElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import BlockWhy from "./BlockWhy";
import { askedOf, BLOCK_KINDS, BLOCK_LOOKS, blockedRed, blockLead, blockLook, blockRequest, blockSentence, blockWords, isBlocked } from "./blockWords";
import type { BlockedBy, Question, Task } from "./types";

const by = (id: string, stage: string, holder = ""): BlockedBy => ({ id, stage, holder });
type Block = Pick<Task, "blockedKind" | "blockDetail" | "blockedBy" | "prefix"> & Partial<Pick<Task, "questions">>;
const b = (blockedKind: string, blockDetail = "", blockedBy: BlockedBy[] = []): Block => ({ blockedKind, blockDetail, blockedBy });
/** Q2, escalated to the owner or not, its first line `text` (block.rs `asked`). */
const asked = (text: string, escalated: boolean, ticked = false): Question => ({
  n: 2, ticked, asker: "wt-a", text: `${text}\n**Escalated to owner** by capture.`, escalated, ownerAnswered: false,
});
const onQ = (q: Question, detail = "Q2"): Block => ({ ...b("question", detail), questions: [q] });

describe("each kind has its look (0060, the same cases as block.rs)", () => {
  it("the three the owner acts on, and the three they can't", () => {
    expect(blockLook("question", asked("which colour?", true))).toBe("answer");
    expect(blockLook("question", asked("which colour?", true, true))).toBe("question");
    expect(blockLook("question", asked("which colour?", false))).toBe("question");
    expect(blockLook("question")).toBe("question");
    expect(blockLook("you")).toBe("action");
    expect(blockLook("permission")).toBe("permission");
    expect(blockLook("tasks")).toBe("tasks");
    expect(blockLook("outside")).toBe("outside");
    expect(blockLook("")).toBe("other");
    const owner = Object.entries(BLOCK_LOOKS).filter(([, l]) => l.owner).map(([k]) => k);
    expect(owner).toEqual(["answer", "action", "permission"]);
  });
  it("finds the question a question block waits on, by its number", () => {
    const q = asked("which colour?", true);
    expect(askedOf(onQ(q))).toBe(q);
    expect(askedOf(onQ(q, "q2"))).toBe(q);
    expect(askedOf(onQ(q, "Q3"))).toBeUndefined();
    expect(askedOf(onQ(q, "which port"))).toBeUndefined();
    expect(askedOf({ ...b("you", "Q2"), questions: [q] })).toBeUndefined();
    expect(askedOf(b("question", "Q2"))).toBeUndefined();
  });
});

describe("why a task is blocked, in words (the same cases as the core's block.rs)", () => {
  it("says plainly what it waits for", () => {
    const tasks = [by("0014", "building", "codex"), by("0016", "ready")];
    expect(blockSentence(b("tasks", "", tasks))).toBe("After 0014 (Building, codex), 0016 (Ready)");
    expect(blockSentence(b("tasks", "", tasks), true)).toBe("After 0014, 0016");
    expect(blockSentence(b("tasks", "", [by("0099", "")]))).toBe("After 0099");
    expect(blockSentence(b("tasks", "", []))).toBe("After other tasks");
    expect(blockSentence(b("you", "set the API key in the Keychain"))).toBe("Your action: set the API key in the Keychain");
    expect(blockSentence(b("permission", "write to ~/playground/x/tasks (sandbox), grant in the layer's write list"))).toBe(
      "Permission: write to ~/playground/x/tasks (sandbox), grant in the layer's write list",
    );
    // A question escalated to the owner waits on their answer, and says which.
    const theirs = asked("which colour for the button?", true);
    expect(blockSentence(onQ(theirs))).toBe("Your answer: Q2 which colour for the button?");
    expect(blockSentence(onQ(theirs), true)).toBe("Your answer: Q2 which colour for the button?");
    // One the capture session still has isn't theirs yet.
    const capture = asked("which colour for the button?", false);
    expect(blockSentence(onQ(capture))).toBe("Question: Q2 which colour for the button?, waiting on capture");
    expect(blockSentence(onQ(capture), true)).toBe("Question: Q2 which colour for the button?");
    expect(blockSentence(b("question", "Q2"))).toBe("Question: Q2, waiting on capture");
    expect(blockSentence(b("question", "Q2"), true)).toBe("Question: Q2");
    expect(blockSentence(b("question", ""))).toBe("Question: waiting on capture");
    expect(blockSentence(b("outside", "Apple review"))).toBe("Outside: Apple review");
    expect(blockSentence(b("you", ""))).toBe("Your action");
    // No kind, nothing blocks it: no words (the core's sentence_of).
    expect(blockSentence(b("", "x"))).toBe("");
    const long = "é".repeat(70);
    expect(blockSentence(b("outside", long), true)).toBe(`Outside: ${"é".repeat(60)}…`);
    expect(blockSentence(b("outside", long))).toBe(`Outside: ${long}`);
    expect(blockSentence(onQ(asked(long, true)), true)).toBe(`Your answer: Q2 ${"é".repeat(57)}…`);
    // Astral characters count once, as the core counts them.
    expect(blockSentence(b("outside", "😀".repeat(61)), true)).toBe(`Outside: ${"😀".repeat(60)}…`);
  });
  it("splits into the look, the bold lead and the rest, the tasks kept for links", () => {
    expect(blockWords(b("tasks", "", [by("0014", "testing")]))).toEqual({ look: "tasks", lead: "After", text: "0014 (Testing)", tasks: [by("0014", "testing")], asked: undefined });
    expect(blockWords(b("you", "pick"))).toEqual({ look: "action", lead: "Your action", text: "pick", tasks: [], asked: undefined });
    const q = asked("which?", true);
    expect(blockWords(onQ(q))).toEqual({ look: "answer", lead: "Your answer", text: "Q2 which?", tasks: [], asked: q });
    expect(blockLead("permission")).toBe("Permission");
    expect(blockLead("question")).toBe("Question");
    expect(blockLead("")).toBe("Blocked");
  });
  it("counts a task blocked by tasks as blocked, flag or not", () => {
    expect(isBlocked({ blocked: false, blockedKind: "tasks" })).toBe(true);
    expect(isBlocked({ blocked: true, blockedKind: "you" })).toBe(true);
    expect(isBlocked({ blocked: false, blockedKind: "" })).toBe(false);
    expect(isBlocked({ blocked: false })).toBe(false);
    // Red only when it doesn't clear by itself: a flag, or any kind but tasks (0060).
    expect(blockedRed({ blocked: false, blockedKind: "tasks" })).toBe(false);
    expect(blockedRed({ blocked: true, blockedKind: "tasks" })).toBe(true);
    expect(blockedRed({ blocked: false, blockedKind: "outside" })).toBe(true);
    expect(blockedRed({ blocked: false, blockedKind: "" })).toBe(false);
  });
});

describe("the Block form's request", () => {
  it("offers the five kinds in a fixed order", () => {
    expect(BLOCK_KINDS.map((k) => k.id)).toEqual(["tasks", "you", "permission", "question", "outside"]);
  });
  it("asks each kind for the detail it needs", () => {
    expect(blockRequest("tasks", " 0014, 0016 ")).toEqual({ ok: true, kind: "tasks", detail: "0014, 0016" });
    const need = "Tasks by key or number, like 14, 16";
    expect(blockRequest("tasks", "soon")).toEqual({ ok: false, why: need });
    expect(blockRequest("tasks", "")).toEqual({ ok: false, why: need });
    // Keys of the task's own project, or numbers, go as ids (0033); another project's key doesn't.
    expect(blockRequest("tasks", "mu-14, MU-16 7", "mu")).toEqual({ ok: true, kind: "tasks", detail: "0014, 0016, 0007" });
    expect(blockRequest("tasks", "mu-14, br-7", "mu")).toEqual({ ok: false, why: need });
    expect(blockRequest("tasks", "mu-14")).toEqual({ ok: false, why: need });
    expect(blockRequest("tasks", "mu-14 soon", "mu")).toEqual({ ok: false, why: need });
    expect(blockRequest("question", "Q2")).toEqual({ ok: true, kind: "question", detail: "Q2" });
    expect(blockRequest("question", "")).toEqual({ ok: false, why: "Pick the open question it waits on" });
    expect(blockRequest("you", "  pick a colour ")).toEqual({ ok: true, kind: "you", detail: "pick a colour" });
    expect(blockRequest("permission", " ")).toEqual({ ok: false, why: "What access it lacks, and where to grant it" });
    expect(blockRequest("outside", "")).toEqual({ ok: false, why: "What it waits on outside" });
    expect(blockRequest("you", "")).toEqual({ ok: false, why: "What you have to do or decide" });
  });
});


describe("keys in the block's words (0033)", () => {
  const keyed = (blockedBy: BlockedBy[]): Block => ({ blockedKind: "tasks", blockDetail: "", blockedBy, prefix: "mu" });
  it("names the tasks by their keys, as the core's sentence_of does with a prefix", () => {
    const t = keyed([by("0014", "building", "codex"), by("0099", "")]);
    expect(blockSentence(t)).toBe("After mu-14 (Building, codex), mu-99");
    expect(blockSentence(t, true)).toBe("After mu-14, mu-99");
  });
  it("links each by its key, opening it by its id", () => {
    const opened: string[] = [];
    const html = renderToStaticMarkup(createElement(BlockWhy, { t: keyed([by("0014", "ready"), by("0016", "building")]), onOpenTask: (id: string) => opened.push(id) }));
    expect(html).toContain('<b>After</b> <button type="button" class="link" title="Open mu-14">mu-14</button> (Ready), <button type="button" class="link" title="Open mu-16">mu-16</button> (Building)');
    const links = buttons(BlockWhy({ t: keyed([by("0014", "ready"), by("0016", "building")]), onOpenTask: (id: string) => opened.push(id) }));
    links.find((x) => x.words === "mu-16")!.click();
    expect(opened).toEqual(["0016"]);
  });
});

/** The buttons a component's elements hold, with their words and a click (as Views.test.ts). */
function buttons(node: ReactNode): { words: string; click: () => void }[] {
  if (!node || typeof node !== "object") return [];
  if (Array.isArray(node)) return node.flatMap(buttons);
  const el = node as ReactElement<{ children?: ReactNode; onClick?: () => void }>;
  if (typeof el.type === "function") return buttons((el.type as (p: unknown) => ReactNode)(el.props));
  const words = (n: ReactNode): string => (Array.isArray(n) ? n.map(words).join("") : typeof n === "string" ? n : "");
  const own = el.type === "button" ? [{ words: words(el.props.children).trim(), click: () => el.props.onClick?.() }] : [];
  return [...own, ...buttons(el.props.children)];
}

describe("the block's words on screen, each kind its own look (0060)", () => {
  const html = (t: Block, opts: { short?: boolean; pill?: boolean; onOpenTask?: (id: string) => void; onAnswer?: (q: Question) => void } = {}) =>
    renderToStaticMarkup(createElement(BlockWhy, { t, ...opts }));
  const theirs = asked("which colour for the button?", true);
  // Every kind in the pane (a pill), on the card (a badge) and in the table (words alone): the
  // owner's in the needs-you look, the rest neutral; the look's own class for its mark.
  const cases: [Block, string, string, string][] = [
    [onQ(theirs), "for-you look-answer", "Your answer", ": Q2 which colour for the button?"],
    [b("you", "pick a colour"), "for-you look-action", "Your action", ": pick a colour"],
    [b("permission", "repo write"), "for-you look-permission", "Permission", ": repo write"],
    [onQ(asked("which port?", false)), "not-you look-question", "Question", ": Q2 which port?, waiting on capture"],
    [b("tasks", "", [by("0014", "ready")]), "not-you look-tasks", "After", " 0014 (Ready)"],
    [b("outside", "Apple review"), "not-you look-outside", "Outside", ": Apple review"],
  ];
  it("gives every kind its words and its look, in the pane, on the card and in the table", () => {
    for (const [t, look, lead, rest] of cases) {
      expect(html(t)).toBe(`<span class="status-pill block-why ${look}"><b>${lead}</b>${rest}</span>`);
      expect(html(t, { short: true })).toMatch(new RegExp(`^<span class="badge block-why ${look}"><b>${lead}</b>`));
      expect(html(t, { short: true, pill: false })).toMatch(new RegExp(`^<span class="block-why plain ${look}"><b>${lead}</b>`));
    }
    // Never the needs-you look for what the owner can't act on, and never the old all-red one.
    for (const [t, look] of cases) expect(html(t)).not.toContain(look.startsWith("for-you") ? "not-you" : "for-you");
    for (const [t] of cases) expect(html(t)).not.toMatch(/\bblocked\b/);
    expect(html(b("", ""))).toBe("");
    expect(html(b("you", ""))).toBe('<span class="status-pill block-why for-you look-action"><b>Your action</b></span>');
  });
  it("puts an Answer button on a question it can answer, which answers that question", () => {
    const got: Question[] = [];
    const onAnswer = (q: Question) => got.push(q);
    expect(html(onQ(theirs), { onAnswer })).toContain('<b>Your answer</b>: Q2 which colour for the button? <button type="button" class="block-answer">Answer</button>');
    buttons(BlockWhy({ t: onQ(theirs), onAnswer })).find((x) => x.words === "Answer")!.click();
    expect(got).toEqual([theirs]);
    // The owner may answer one capture still has, too.
    expect(html(onQ(asked("which port?", false)), { onAnswer })).toContain(">Answer</button>");
    // Not on the card (it's a button itself), not without a way to answer, not on an answered or
    // missing question, not on another kind.
    expect(html(onQ(theirs), { onAnswer, short: true })).not.toContain("Answer</button>");
    expect(html(onQ(theirs))).not.toContain("<button");
    expect(html(onQ(asked("which?", true, true)), { onAnswer })).not.toContain("<button");
    expect(html(b("question", "Q2"), { onAnswer })).not.toContain("<button");
    expect(html(b("you", "pick"), { onAnswer })).not.toContain("<button");
  });
  it("makes each task a link that opens it, with its stage, when it can open tasks", () => {
    const t = b("tasks", "", [by("0014", "building", "codex"), by("0016", "ready")]);
    const out = html(t, { onOpenTask: () => {} });
    expect(out).toContain("<b>After</b> ");
    expect(out).toContain('<button type="button" class="link" title="Open 0014">0014</button> (Building, codex), ');
    expect(out).toContain('<button type="button" class="link" title="Open 0016">0016</button> (Ready)');
    // The card's short form: ids only, plain text (the card itself is the button).
    expect(html(t, { short: true })).toBe('<span class="badge block-why not-you look-tasks"><b>After</b> 0014, 0016</span>');
  });
});
