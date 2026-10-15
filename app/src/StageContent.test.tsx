// Your check's "What you're accepting" (mu-78): what was built, what Accept does, and each
// criterion's proof with its screenshots, rendered as the pane draws it. The fixture is ai-10's
// shape with synthetic text: four criteria ticked with proofs, two naming screenshots, none the
// owner's, and a Persian quote in a proof.
import { isValidElement, type ReactElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import css from "./styles.css?raw";
import StageContent, { AcceptingBlock, toggled } from "./StageContent";
import type { Criterion, Task } from "./types";

const PERSIAN = "headless Chromium: Tab focused 'پیام بعدی', Enter took it from 1 to 4; attachments/0010/0010-ac2-keyboard.png";
const agent = (n: number, words: string, proof: string, proofFiles: string[] = []): Criterion => ({
  n, label: `AC${n}`, text: `AC${n}: ${words} — verify: command`, ticked: true, owner: false, proof, proofFiles,
});
const AI10: Criterion[] = [
  agent(1, "unit tests cover the chat block.", "pnpm test 36/36 pass"),
  agent(2, "the chat works with only the keyboard.", PERSIAN, ["0010-ac2-keyboard.png"]),
  agent(3, "the chat plays with the network off.", "offline after load: 1 → 3 messages; attachments/0010/0010-ac3-offline.png", ["0010-ac3-offline.png"]),
  agent(4, "the component adds at most 5 KB gzipped.", "one extra chunk of 988 B gzipped"),
];
const task = (over: Partial<Task> = {}): Task => ({
  queue: "ai", id: "0010", prefix: "ai", title: "Example-chat component", kind: "task", priority: "high", status: "doing", stage: "your-check", blocked: false,
  stageHistory: [{ at: "2026-10-13 02:53", who: "qa", from: "testing", to: "your-check" }], folder: "doing", path: "doing/0010-example-chat-component.md", claimedBy: "", heldBy: "owner",
  stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: true, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0,
  claimedOn: "", doneOn: "", links: [], comments: [], attachments: ["0010-ac2-keyboard.png", "0010-ac3-offline.png", "notes.txt"], logTail: [], modified: 0, level: "task",
  parent: "", due: "", children: [], progress: null, shownStage: "your-check", criteria: AI10,
  summary: "A chat example the reader steps through with Next or the keyboard; works offline; under 1 KB",
  mergeReport: "merged task/0010-example-chat into main as 3f2a9c1 (gate: green · review: clean · 2 h in Building and Testing)", ...over,
});

const pane = (t: Task) => renderToStaticMarkup(<StageContent task={t} nowSecs={0} body={null} busy={false} onTick={() => {}} />);
const block = (t: Task, open: string[] = [], urls: Record<string, string> = {}) =>
  renderToStaticMarkup(<AcceptingBlock task={t} urls={urls} open={new Set(open)} onToggle={() => {}} />);
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/&gt;/g, ">").replace(/&lt;/g, "<").replace(/&amp;/g, "&").replace(/\s+/g, " ").trim();
/** Each proof line: the inside of its `<li class="proof-line…">`. */
const lines = (html: string) => [...html.matchAll(/<li class="proof-line[^"]*">([\s\S]*?)<\/li>/g)].map((m) => m[1]);
/** Every element that says whether it's expanded: its tag and attributes. */
const toggles = (html: string) => [...html.matchAll(/<(\w+)([^>]*\baria-expanded="(?:true|false)"[^>]*)>/g)].map((m) => ({ tag: m[1], attrs: m[2] }));
const attr = (attrs: string, name: string) => new RegExp(`\\b${name}="([^"]*)"`).exec(attrs)?.[1];
/** The elements a stateless component draws, its own components called through (none of
 *  AcceptingBlock's holds state), so a test can fire a handler without a DOM. */
type Props = { children?: ReactNode; onClick?: () => void; "aria-label"?: string };
function elements(node: ReactNode): ReactElement<Props>[] {
  if (Array.isArray(node)) return node.flatMap(elements);
  if (!isValidElement<Props>(node)) return [];
  if (typeof node.type === "function") return elements((node.type as (p: Props) => ReactNode)(node.props));
  return [node, ...elements(node.props.children)];
}

describe("What you're accepting, in Your check (mu-78)", () => {
  it("comes first, above Your checklist and the history, as a labelled group", () => {
    const html = pane(task());
    const parts = [...html.matchAll(/data-part="([\w-]+)"/g)].map((m) => m[1]);
    expect(parts).toEqual(["accepting", "checklist", "history"]);
    expect(html).toMatch(/<div class="stage-block accepting" data-part="accepting" role="group" aria-label="What you&#x27;re accepting">/);
    expect(text(html)).toMatch(/^What you're accepting Built /);
  });

  it("says what was built, what Accept does, and each criterion with its proof, in words", () => {
    const html = block(task());
    expect(text(html)).toContain("Built A chat example the reader steps through with Next or the keyboard; works offline; under 1 KB");
    expect(text(html)).toContain("Accept Every criterion is proven: Accept moves it to Done.");
    expect(lines(html).map(text)).toEqual([
      "AC1 · unit tests cover the chat block. · ✓ proven: pnpm test 36/36 pass",
      `AC2 · the chat works with only the keyboard. · ✓ proven: ${PERSIAN} 0010-ac2-keyboard.png`,
      "AC3 · the chat plays with the network off. · ✓ proven: offline after load: 1 → 3 messages; attachments/0010/0010-ac3-offline.png 0010-ac3-offline.png",
      "AC4 · the component adds at most 5 KB gzipped. · ✓ proven: one extra chunk of 988 B gzipped",
    ]);
    // The verify part is who checks it, not what was built.
    expect(text(html)).not.toContain("verify:");
    // The merge line goes last, in small type.
    expect(html).toMatch(/<p class="meta small merge-line" dir="auto">merged task\/0010-example-chat into main as 3f2a9c1 \(gate: green · review: clean · 2 h in Building and Testing\)<\/p><\/div>$/);
  });

  it("shows each screenshot its proof names as a thumbnail under its line, and no other file", () => {
    const html = block(task(), [], { "0010-ac2-keyboard.png": "data:image/png;base64,AAA" });
    const [ac1, ac2, ac3, ac4] = lines(html);
    expect(toggles(ac1)).toEqual([]);
    expect(toggles(ac4)).toEqual([]);
    expect(toggles(ac2).map((b) => attr(b.attrs, "aria-label"))).toEqual(["0010-ac2-keyboard.png"]);
    expect(toggles(ac3).map((b) => attr(b.attrs, "aria-label"))).toEqual(["0010-ac3-offline.png"]);
    // Files' size: the small thumbnail, its image once loaded, a placeholder until then.
    expect(ac2).toMatch(/class="mini-thumb proof-thumb"[^>]*><img src="data:image\/png;base64,AAA" alt=""\/>/);
    expect(ac3).toContain('<span class="mini-img"></span>');
    // A file the task has but no proof names isn't shown here; Files lists it.
    expect(html).not.toContain("notes.txt");
    // A proof file that isn't an image is named, not drawn.
    const log = block(task({ criteria: [agent(1, "logs it.", "see run.log", ["run.log"])] }));
    expect(toggles(log)).toEqual([]);
    expect(text(log)).toContain("✓ proven: see run.log run.log");
  });

  it("expands a thumbnail in place, in the pane, never in an overlay", () => {
    const urls = { "0010-ac2-keyboard.png": "data:image/png;base64,AAA" };
    const shut = toggles(block(task(), [], urls));
    const open = toggles(block(task(), ["2/0010-ac2-keyboard.png"], urls));
    expect(shut.map((b) => attr(b.attrs, "aria-expanded"))).toEqual(["false", "false"]);
    expect(open.map((b) => attr(b.attrs, "aria-expanded"))).toEqual(["true", "false"]);
    const html = block(task(), ["2/0010-ac2-keyboard.png"], urls);
    expect(lines(html)[1]).toMatch(/class="mini-thumb proof-thumb open"[^>]*><img class="proof-big" src="data:image\/png;base64,AAA" alt="0010-ac2-keyboard.png"\/><span class="name" dir="auto">0010-ac2-keyboard.png<\/span>/);
    expect(html).not.toMatch(/role="dialog"|aria-modal|position: ?fixed|overlay/);
    // Clicking toggles that one: open, then shut again.
    const once = toggled(new Set(), "2/0010-ac2-keyboard.png");
    expect([...once]).toEqual(["2/0010-ac2-keyboard.png"]);
    expect([...toggled(once, "3/0010-ac3-offline.png")]).toEqual(["2/0010-ac2-keyboard.png", "3/0010-ac3-offline.png"]);
    expect([...toggled(once, "2/0010-ac2-keyboard.png")]).toEqual([]);
  });

  it("opens a thumbnail on a click (Enter and Space click a button), then shows it large", () => {
    const onToggle = vi.fn();
    const urls = { "0010-ac2-keyboard.png": "data:image/png;base64,AAA" };
    const tree = elements(<AcceptingBlock task={task()} urls={urls} open={new Set()} onToggle={onToggle} />);
    const thumb = (name: string) => tree.find((e) => e.type === "button" && e.props["aria-label"] === name);
    thumb("0010-ac2-keyboard.png")?.props.onClick?.();
    expect(onToggle).toHaveBeenCalledWith("2/0010-ac2-keyboard.png");
    thumb("0010-ac3-offline.png")?.props.onClick?.();
    expect(onToggle.mock.calls).toEqual([["2/0010-ac2-keyboard.png"], ["3/0010-ac3-offline.png"]]);
    // With that key open, the same button says so and holds the large image.
    const [first] = toggles(block(task(), [onToggle.mock.calls[0][0]], urls));
    expect(attr(first.attrs, "aria-expanded")).toBe("true");
    expect(lines(block(task(), [onToggle.mock.calls[0][0]], urls))[1]).toContain('<img class="proof-big" src="data:image/png;base64,AAA" alt="0010-ac2-keyboard.png"/>');
  });

  it("makes each thumbnail a button named by its file, that Tab reaches and Enter or Space works", () => {
    for (const b of toggles(block(task()))) {
      // A native button: in the tab order, and Enter and Space click it, which toggles it.
      expect(b.tag).toBe("button");
      expect(attr(b.attrs, "type")).toBe("button");
      expect(attr(b.attrs, "tabindex")).toBeUndefined();
      expect(attr(b.attrs, "aria-label")).toMatch(/^0010-ac[23]-[a-z]+\.png$/);
      expect(attr(b.attrs, "title")).toBe(attr(b.attrs, "aria-label"));
    }
  });

  it("has no expand animation under reduced motion", () => {
    const rules = [...css.replace(/\/\*[\s\S]*?\*\//g, "").matchAll(/([^{}]+)\{([^{}]*)\}/g)].filter((m) => /proof-/.test(m[1]));
    expect(rules.length).toBeGreaterThan(0);
    const motion = /@media \(prefers-reduced-motion: reduce\) \{\s*([^{}]*\.proof-thumb[^{}]*)\{([^}]*)\}/.exec(css);
    expect(motion?.[2]).toMatch(/transition: none/);
    expect(motion?.[2]).toMatch(/animation: none/);
  });

  it("isolates each line's words for right-to-left text, with the label left in place", () => {
    const ac2 = lines(block(task()))[1];
    // The label first, outside any isolate; the words and the proof each in their own.
    expect(ac2).toMatch(/^<b class="proof-label">AC2<\/b>/);
    expect(ac2).toContain('<span dir="auto">the chat works with only the keyboard.</span>');
    expect(ac2).toContain(`<span class="proof-text" dir="auto">${PERSIAN.replace(/'/g, "&#x27;")}</span>`);
    // The summary too.
    expect(block(task())).toContain('<span dir="auto">A chat example the reader steps through');
  });

  it("shows the summary as plain text, never as markup", () => {
    const html = block(task({ summary: "A chat with <b>bold</b> & *stars*" }));
    expect(html).toContain('<span dir="auto">A chat with &lt;b&gt;bold&lt;/b&gt; &amp; *stars*</span>');
    expect(html).not.toContain("<b>bold</b>");
  });

  it("without a summary, says the title and how to add one", () => {
    const html = block(task({ summary: "" }));
    expect(text(html)).toContain("Built Example-chat component (no summary was written; muster set ai-10 summary adds one)");
    expect(text(block(task({ summary: undefined })))).toContain("(no summary was written; muster set ai-10 summary adds one)");
  });

  it("says what's not proven, or what's yours to check, instead of all proven", () => {
    const open = { ...agent(2, "looks right in dark.", ""), ticked: false, owner: true, text: "AC2: looks right in dark. — verify: owner" };
    const yours = block(task({ criteria: [AI10[0], open], ownerCriteria: [open] }));
    expect(text(yours)).toContain("Accept Accept after you check 1: AC2.");
    expect(lines(yours).map(text)[1]).toBe("AC2 · looks right in dark. · yours to check, in Your checklist");
    const notYet = block(task({ criteria: [AI10[0], { ...AI10[1], ticked: false, proof: "", proofFiles: [] }] }));
    expect(text(notYet)).toContain("Accept Not proven yet: AC2. Send it back, or check it yourself before you accept it.");
    expect(lines(notYet).map(text)[1]).toBe("AC2 · the chat works with only the keyboard. · not proven yet");
    const mine = { ...open, ticked: true };
    expect(lines(block(task({ criteria: [mine], ownerCriteria: [mine] }))).map(text)).toEqual(["AC2 · looks right in dark. · ✓ checked by you"]);
    const bare = block(task({ criteria: [{ ...AI10[0], proof: "" }] }));
    expect(lines(bare).map(text)).toEqual(["AC1 · unit tests cover the chat block. · ✓ ticked, no proof written"]);
    expect(text(block(task({ criteria: [] })))).toContain("Proof No acceptance criteria are written.");
  });
});

describe("the pane loads each image once (mu-78 R6)", () => {
  const sources = import.meta.glob<string>("./*.tsx", { query: "?raw", import: "default", eager: true });
  it("in TaskPanel's one map, which Files and What you're accepting both read", () => {
    const fetching = Object.entries(sources).filter(([path, text]) => !path.endsWith(".test.tsx") && text.includes("api.image(")).map(([path]) => path);
    expect(fetching).toEqual(["./TaskPanel.tsx"]);
    expect(sources["./TaskPanel.tsx"]).toContain("images={images}");
  });
});
