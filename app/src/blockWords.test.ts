import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import BlockWhy from "./BlockWhy";
import { BLOCK_KINDS, blockLead, blockRequest, blockSentence, blockWords, isBlocked } from "./blockWords";
import type { BlockedBy, Task } from "./types";

const by = (id: string, stage: string, holder = ""): BlockedBy => ({ id, stage, holder });
type Block = Pick<Task, "blockedKind" | "blockDetail" | "blockedBy">;
const b = (blockedKind: string, blockDetail = "", blockedBy: BlockedBy[] = []): Block => ({ blockedKind, blockDetail, blockedBy });

describe("why a task is blocked, in words (the same cases as the core's block.rs)", () => {
  it("says the kind first", () => {
    const tasks = [by("0014", "building", "codex"), by("0016", "ready")];
    expect(blockSentence(b("tasks", "", tasks))).toBe("Blocked by tasks: 0014 (Building, codex), 0016 (Ready)");
    expect(blockSentence(b("tasks", "", tasks), true)).toBe("Blocked by tasks: 0014, 0016");
    expect(blockSentence(b("tasks", "", [by("0099", "")]))).toBe("Blocked by tasks: 0099");
    expect(blockSentence(b("you", "set the API key in the Keychain"))).toBe("Blocked on you: set the API key in the Keychain");
    expect(blockSentence(b("permission", "write to ~/playground/x/tasks (sandbox), grant in the layer's write list"))).toBe(
      "Blocked by permission: write to ~/playground/x/tasks (sandbox), grant in the layer's write list",
    );
    expect(blockSentence(b("question", "Q2"))).toBe("Blocked by a question: Q2, waiting on capture");
    expect(blockSentence(b("question", "Q2"), true)).toBe("Blocked by a question: Q2");
    expect(blockSentence(b("question", ""))).toBe("Blocked by a question: waiting on capture");
    expect(blockSentence(b("outside", "Apple review"))).toBe("Blocked by outside: Apple review");
    expect(blockSentence(b("you", ""))).toBe("Blocked on you");
    // No kind, nothing blocks it: no words (the core's sentence_of).
    expect(blockSentence(b("", "x"))).toBe("");
    const long = "é".repeat(70);
    expect(blockSentence(b("outside", long), true)).toBe(`Blocked by outside: ${"é".repeat(60)}…`);
    expect(blockSentence(b("outside", long))).toBe(`Blocked by outside: ${long}`);
    // Astral characters count once, as the core counts them.
    expect(blockSentence(b("outside", "😀".repeat(61)), true)).toBe(`Blocked by outside: ${"😀".repeat(60)}…`);
  });
  it("splits into the bold lead and the rest, the tasks kept for links", () => {
    expect(blockWords(b("tasks", "", [by("0014", "testing")]))).toEqual({ lead: "Blocked by tasks", text: "0014 (Testing)", tasks: [by("0014", "testing")] });
    expect(blockWords(b("you", "pick"))).toEqual({ lead: "Blocked on you", text: "pick", tasks: [] });
    expect(blockLead("permission")).toBe("Blocked by permission");
    expect(blockLead("")).toBe("Blocked");
  });
  it("counts a task blocked by tasks as blocked, flag or not", () => {
    expect(isBlocked({ blocked: false, blockedKind: "tasks" })).toBe(true);
    expect(isBlocked({ blocked: true, blockedKind: "you" })).toBe(true);
    expect(isBlocked({ blocked: false, blockedKind: "" })).toBe(false);
    expect(isBlocked({ blocked: false })).toBe(false);
  });
});

describe("the Block form's request", () => {
  it("offers the five kinds in a fixed order", () => {
    expect(BLOCK_KINDS.map((k) => k.id)).toEqual(["tasks", "you", "permission", "question", "outside"]);
  });
  it("asks each kind for the detail it needs", () => {
    expect(blockRequest("tasks", " 0014, 0016 ")).toEqual({ ok: true, kind: "tasks", detail: "0014, 0016" });
    expect(blockRequest("tasks", "soon")).toEqual({ ok: false, why: "Task ids, like 0014, 0016" });
    expect(blockRequest("tasks", "")).toEqual({ ok: false, why: "Task ids, like 0014, 0016" });
    expect(blockRequest("question", "Q2")).toEqual({ ok: true, kind: "question", detail: "Q2" });
    expect(blockRequest("question", "")).toEqual({ ok: false, why: "Pick the open question it waits on" });
    expect(blockRequest("you", "  pick a colour ")).toEqual({ ok: true, kind: "you", detail: "pick a colour" });
    expect(blockRequest("permission", " ")).toEqual({ ok: false, why: "What access it lacks, and where to grant it" });
    expect(blockRequest("outside", "")).toEqual({ ok: false, why: "What it waits on outside" });
    expect(blockRequest("you", "")).toEqual({ ok: false, why: "What you have to do or decide" });
  });
});

describe("the block's words on screen", () => {
  const html = (t: Block, opts: { short?: boolean; onOpenTask?: (id: string) => void } = {}) =>
    renderToStaticMarkup(createElement(BlockWhy, { t, ...opts }));
  it("puts the kind in bold, and nothing when it isn't blocked", () => {
    expect(html(b("you", "pick a colour"))).toBe('<span class="status-pill blocked block-why"><b>Blocked on you</b>: pick a colour</span>');
    expect(html(b("", ""))).toBe("");
    expect(html(b("you", ""))).toBe('<span class="status-pill blocked block-why"><b>Blocked on you</b></span>');
  });
  it("makes each task a link that opens it, with its stage, when it can open tasks", () => {
    const t = b("tasks", "", [by("0014", "building", "codex"), by("0016", "ready")]);
    const out = html(t, { onOpenTask: () => {} });
    expect(out).toContain("<b>Blocked by tasks</b>: ");
    expect(out).toContain('<button type="button" class="link" title="Open 0014">0014</button> (Building, codex), ');
    expect(out).toContain('<button type="button" class="link" title="Open 0016">0016</button> (Ready)');
    // The card's short form: ids only, plain text (the card itself is the button).
    expect(html(t, { short: true })).toBe('<span class="badge blocked block-why"><b>Blocked by tasks</b>: 0014, 0016</span>');
  });
});
