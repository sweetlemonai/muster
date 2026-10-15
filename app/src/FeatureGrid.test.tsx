// The Feature grid as it's drawn (mu-143): an ARIA grid with its row and column headers, two cards a
// cell then "+N more", blank empty cells, the trouble chip on its own line, the one-line Done head,
// the header line and the full-limit alert, and task text rendered as text.
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";
import css from "./styles.css?raw";
import FeatureGrid, { type FeatureGridProps } from "./FeatureGrid";
import { gridRows, headerLine, LAYOUTS } from "./featureGridData";
import { setLocale } from "./i18n/runtime";
import { ChoiceChips } from "./ModeViews";
import type { QueueInfo, Task } from "./types";

const task = (over: Partial<Task>): Task => ({
  queue: "muster", id: "0001", prefix: "mu", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0,
  level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});
const TODAY = "2026-10-14";
const NOW = new Date(2026, 9, 14, 12, 0).getTime() / 1000;
const orch = task({ id: "0009", title: "Orchestration", level: "feature", progress: { done: 3, total: 11, estMinutes: 0, estTokenCount: 0 } });
const back = [
  { at: "2026-10-13 10:00", who: "kaveh-3", from: "building", to: "testing" },
  { at: "2026-10-13 11:00", who: "kia-1", from: "testing", to: "building" },
  { at: "2026-10-14 08:00", who: "kaveh-3", from: "building", to: "testing" },
  { at: "2026-10-14 09:00", who: "kia-1", from: "testing", to: "building" },
];
const all = [
  orch,
  task({ id: "0097", title: "Shaping as a claim", parent: "0009", stage: "building", claimedBy: "kaveh-2 (main · session 1234abcd · cc-mg)", live: true, heartbeat: "2026-10-14 11:50" }),
  task({ id: "0099", title: "Agent account allowlist", parent: "0009", stage: "building", claimedBy: "kaveh-3", sentBack: true, stageHistory: back, heartbeat: "2026-10-14 11:50" }),
  task({ id: "0090", title: "Handoff keeps the owner", parent: "0009", stage: "your-check", heldBy: "owner", waitingForOwner: true }),
  task({ id: "0091", title: "Done today", parent: "0009", stage: "done", doneAt: `${TODAY} 09:12` }),
  ...[1, 2, 3, 4, 5].map((n) => task({ id: `010${n}`, title: `Ready ${n}`, parent: "0009", stage: "ready" })),
  task({ id: "0094", title: "<img src=x onerror=alert(1)>", stage: "testing", heldBy: "kia-1" }),
];
const work = all.filter((t) => t.level === "task");
const q = (waiting: number, limit: number | null): QueueInfo => ({ name: "muster", path: "", counts: {}, yourCheck: waiting, yourCheckLimit: limit, label: "Muster" });

function draw(over: Partial<FeatureGridProps> = {}) {
  const props: FeatureGridProps = {
    rows: gridRows({ work, all, notes: [], by: "feature", today: TODAY }),
    queue: "muster", queues: [q(5, 5)], title: "Muster",
    header: headerLine(work, 5, TODAY), needs: 5, by: "feature", onBy: () => {},
    multi: false, mine: false, onMine: () => {}, now: NOW, words: [], openKey: null, openNoteKey: null, onOpen: () => {}, onOpenNote: () => {}, ...over,
  };
  return renderToStaticMarkup(<FeatureGrid {...props} />);
}
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&").replace(/\s+/g, " ").trim();
const all_ = (html: string, re: RegExp) => [...html.matchAll(re)];
/** Each element with the role, as its text. */
const byRole = (html: string, role: string) =>
  all_(html, new RegExp(`<div[^>]*role="${role}"[^>]*>([\\s\\S]*?)(?=<div[^>]*role="(?:columnheader|rowheader|gridcell|row)"|</div></div></div></section>)`, "g")).map((m) => text(m[1]));

describe("the grid's structure, for a screen reader and the keyboard", () => {
  it("is a labelled grid with a header row and a row per feature", () => {
    const html = draw();
    expect(html).toMatch(/role="grid" aria-label="Feature grid: Muster" aria-rowcount="3" aria-colcount="8"/);
    expect(all_(html, /role="row"/g)).toHaveLength(3);
  });
  it("heads each column with its stage, count and who works there, and Done on one line", () => {
    expect(byRole(draw(), "columnheader")).toEqual([
      "Feature",
      "Backlog 0 notes, not shaped yet",
      "Shaping 0 product + tech + UI agents",
      "Ready 5 5 can start",
      "Building 2 kaveh-2 and kaveh-3",
      "Testing 1 kia-1",
      "Your check 1 5 of 5, full",
      "Done · 1 today",
    ]);
  });
  it("heads each row with the feature, its progress and done count", () => {
    expect(byRole(draw(), "rowheader")).toEqual(["Orchestration 3 of 11 done", "Other 0 of 1 done"]);
  });
  it("isolates a feature's title from the project's name before it in All projects (review R3)", () => {
    const html = draw({ multi: true, queue: "all", queues: [q(3, 5), { ...q(0, null), name: "drover", label: "Drover" }] });
    expect(html).toMatch(/<button[^>]*class="fg-rowtitle link" tabindex="-1"[^>]*><span class="q">muster<\/span> <bdi dir="auto">Orchestration<\/bdi><\/button>/);
    // The button takes no direction of its own: the name's would decide the title's.
    expect(html).not.toMatch(/class="fg-rowtitle[^"]*" dir=/);
  });
  it("gives each column head's who line its own direction, and marks the column for the cue (review R7, R1)", () => {
    const html = draw();
    expect(all_(html, /<small dir="auto">/g)).toHaveLength(6);
    expect(all_(html, /role="columnheader" class="fg-colhead[^"]*" data-col="([a-z-]+)"/g).map((m) => m[1])).toEqual(["backlog", "shaping", "ready", "building", "testing", "your-check", "done"]);
  });
  it("draws no off-edge cue before it has measured anything", () => {
    expect(draw()).not.toContain("col-cue");
    expect(draw()).toMatch(/<div class="fg-over"><p class="fg-alert fg-alert-on" role="status">/);
  });
  it("has a cell for every stage and Done in each row, and only one cell in the tab order", () => {
    const html = draw();
    expect(all_(html, /role="gridcell"/g)).toHaveLength(14);
    expect(all_(html, /role="gridcell"[^>]*tabindex="0"/g)).toHaveLength(1);
    // The cards, "+N more" and the feature names are reached with the keys, never with Tab.
    expect(all_(html, /<button(?![^>]*tabindex="-1")[^>]*>/g).filter((m) => !/ui-segment|ui-pill/.test(m[0]))).toEqual([]);
    expect(html).toMatch(/role="rowheader" data-rowhead="0"><button[^>]*class="fg-rowtitle link" tabindex="-1"/);
    expect(html).toMatch(/role="gridcell" class="fg-cell" data-cell="0:0" tabindex="0"/);
  });
});

describe("the cells", () => {
  it("leave an empty cell blank, never 'None'", () => {
    const html = draw();
    expect(byRole(html, "gridcell").filter((c) => /\bNone\b/.test(c))).toEqual([]);
    expect(html).toMatch(/<div role="gridcell" class="fg-cell" data-cell="0:0" tabindex="0"><\/div>/);
  });
  it("show two cards, then +N more", () => {
    const cells = byRole(draw(), "gridcell");
    expect(cells[2]).toBe("Ready 1 mu-101 Ready 2 mu-102 +3 more");
    expect(draw()).toMatch(/<button[^>]*class="fg-more" tabindex="-1" aria-expanded="false" aria-label="3 more in Orchestration, Ready: show them here">\+3 more<\/button>/);
  });
  it("show Done today as a count, blank for none", () => {
    const cells = byRole(draw(), "gridcell");
    expect(cells[6]).toBe("1");
    expect(cells[13]).toBe("");
    expect(draw()).toMatch(/aria-label="none done today"/);
  });
});

describe("the cards", () => {
  it("say who's on it with a live dot, and a needs-you card has its edge and word", () => {
    const html = draw();
    expect(html).toMatch(/<span class="fg-who" title="kaveh-2, working now"><i class="dot fg-dot" aria-hidden="true"><\/i>kaveh-2<\/span>/);
    expect(html).toMatch(/<button[^>]*class="fg-card fg-you" tabindex="-1">[\s\S]*?needs you[\s\S]*?mu-90/);
  });
  it("put the trouble chip on its own line under the title, the who line and key after it", () => {
    const html = draw();
    expect(html).toMatch(/<span class="fg-t" dir="auto" title="Agent account allowlist">Agent account allowlist<\/span><span class="fg-trouble" title="sent back twice">sent back twice<\/span><span class="fg-meta"><span class="fg-who"[^>]*>.*?kaveh-3<\/span><span class="fg-key">mu-99<\/span>/);
  });
  it("put why a task waits on its own line, the key still in the line under it", () => {
    const waits = task({ id: "0098", title: "Sessions view", stage: "ready", blockedKind: "tasks", blockedBy: [{ id: "0099", stage: "building", holder: "" }, { id: "0112", stage: "testing", holder: "" }] });
    const html = draw({ rows: gridRows({ work: [waits], all: [waits], notes: [], by: "none", today: TODAY }) });
    expect(html).toMatch(/Sessions view<\/span><span class="fg-wait"><span class="ui-badge block-why not-you look-tasks"><b>After<\/b> mu-99, mu-112<\/span><\/span><span class="fg-meta"><span class="fg-key">mu-98<\/span><\/span>/);
  });
  it("give a card blocked on you the needs-you edge, with why it waits", () => {
    const t = task({ id: "0110", title: "Branch check", stage: "testing", heldBy: "kia-1", blocked: true, blockedKind: "you", blockDetail: "settle R2", waitingForOwner: true });
    const html = draw({ rows: gridRows({ work: [t], all: [t], notes: [], by: "none", today: TODAY }) });
    expect(html).toMatch(/<button[^>]*class="fg-card fg-you" tabindex="-1"><span class="fg-t"[^>]*>Branch check<\/span><span class="fg-wait"><span class="ui-badge block-why for-you look-action"><b>Your action<\/b>: settle R2/);
  });
  it("keep a long title whole in its tooltip", () => {
    expect(draw()).toMatch(/<span class="fg-t" dir="auto" title="Shaping as a claim">/);
  });
  it("draw task text as text", () => {
    const html = draw();
    expect(html).not.toContain("<img");
    expect(html).toContain("&lt;img src=x onerror=alert(1)&gt;");
  });
  it("mark the one open in the pane", () => {
    expect(draw({ openKey: "muster/0097" })).toMatch(/class="fg-card fg-active" tabindex="-1">[\s\S]*?Shaping as a claim/);
  });
});

describe("the header line, the row choice and the alert", () => {
  it("says what waits for you first, in its own colour, then the rest", () => {
    const html = draw();
    expect(html).toMatch(/<p class="fg-summary"><b class="fg-needs">5 waiting for you<\/b>, 2 being built, 1 in testing, 1 done today<\/p>/);
  });
  it("lights the rows' choice", () => {
    expect(draw()).toMatch(/aria-pressed="true" type="button" class="ui-segment ui-segment--active">Feature</);
    expect(draw({ by: "none" })).toMatch(/aria-pressed="true" type="button" class="ui-segment ui-segment--active">None</);
  });
  it("has Only what needs me off at first, dimming nothing", () => {
    const html = draw();
    expect(html).toMatch(/aria-pressed="false"[^>]*class="ui-pill fg-mine-pick">Only what needs me</);
    expect(html).not.toContain("fg-dim");
  });
  it("with Only what needs me on, dims every card but the ones waiting on you (review R4)", () => {
    const html = draw({ mine: true });
    expect(html).toMatch(/aria-pressed="true"[^>]*class="ui-pill ui-pill--on fg-mine-pick">Only what needs me</);
    // Your check's card waits on you: lit. The rest, a note too, dimmed but still there.
    expect(html).toMatch(/<button[^>]*class="fg-card fg-you" tabindex="-1">[\s\S]*?mu-90/);
    expect(html).toMatch(/<button[^>]*class="fg-card fg-dim" tabindex="-1"><span class="fg-t"[^>]*>Shaping as a claim/);
    expect(all_(html, /class="fg-card[^"]*fg-dim"/g)).toHaveLength(5);
    const n = draw({ mine: true, rows: gridRows({ work: [], all: [], notes: [{ queue: "muster", file: "idea.md", title: "An idea", text: "An idea", truncated: false, added: 0, heldBy: "", heldSince: "", heartbeat: "" }], by: "none", today: TODAY }) });
    expect(n).toMatch(/class="fg-card fg-note fg-dim"/);
  });
  it("says Your check is full in one line, and keeps the line's place empty when it isn't", () => {
    expect(draw()).toMatch(/<p class="fg-alert fg-alert-on" role="status">Your check is full \(5 of 5\): check some before more start\.<\/p>/);
    const within = draw({ queues: [q(3, 5)] });
    expect(within).toMatch(/<p class="fg-alert" role="status"><\/p>/);
    expect(within).toMatch(/class="fg-colhead col-your-check"[^>]*><b>Your check<\/b> <span class="fg-n">1<\/span><small dir="auto">3 of 5<\/small>/);
  });
  it("turns the full Your check head to the warning colour, and only it (review R6)", () => {
    expect(draw()).toMatch(/class="fg-colhead col-your-check fg-full"[^>]*><b>Your check<\/b> <span class="fg-n">1<\/span><small dir="auto">5 of 5, full<\/small>/);
    expect(all_(draw(), /fg-full/g)).toHaveLength(1);
    expect(draw({ queues: [q(3, 5)] })).not.toContain("fg-full");
  });
  it("puts what waits for you in the needs-you colour only when something does (review R6)", () => {
    expect(draw()).toMatch(/<b class="fg-needs">5 waiting for you<\/b>/);
    const none = draw({ needs: 0, header: headerLine(work, 0, TODAY) });
    expect(none).toMatch(/<p class="fg-summary"><b>nothing waiting for you<\/b>/);
    expect(none).not.toContain("fg-needs");
  });
  it("says so when nothing matches", () => {
    expect(text(draw({ rows: [] }))).toContain("No tasks match.");
  });
});

describe("the grid's styles", () => {
  const rule = (sel: string) => new RegExp(`(^|\\n)${sel.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")} \\{([^}]*)\\}`).exec(css)?.[2] ?? "";
  it("scrolls inside its frame, never the page", () => {
    expect(rule(".fg-frame")).toMatch(/overflow: auto/);
    // The grid is never narrower than its columns' floors, so the sticky names stay over it.
    expect(rule(".fg-grid")).toMatch(/min-width: min-content/);
  });
  it("keeps the stages 140px at least, to fit Backlog to Done in more windows, and Done as wide as its words (review R1)", () => {
    expect(rule(".fg-grid")).toMatch(/grid-template-columns: var\(--fg-names\) repeat\(6, minmax\(140px, 1fr\)\) max-content;/);
  });
  it("keeps a cell the keys move to clear of the sticky names and header row (review R2)", () => {
    expect(rule(".fg-frame")).toMatch(/--fg-names: 160px;/);
    expect(rule(".fg-frame")).toMatch(/scroll-padding-inline-start: var\(--fg-names\);/);
    expect(rule(".fg-frame")).toMatch(/scroll-padding-top: var\(--fg-head-h, \d+px\);/);
  });
  it("lets a column head's who line take its own direction, not plaintext (review R7)", () => {
    const plain = /([^\n{}]+)\{ unicode-bidi: plaintext;/.exec(css.split("\n").filter((l) => l.startsWith(".fg-summary")).join("\n"))?.[1] ?? "";
    expect(plain).toContain(".fg-alert");
    expect(plain).not.toContain(".fg-colhead small");
  });
  it("keeps a column head on one line", () => {
    expect(rule(".fg-colhead")).toMatch(/white-space: nowrap/);
  });
  it("keeps a faint outline on every cell", () => {
    expect(rule(".fg-cell")).toMatch(/border-inline-start: 1px solid var\(--line\)/);
    expect(rule(".fg-cell")).toMatch(/border-bottom: 1px solid var\(--line\)/);
  });
  it("mirrors in right-to-left: logical sides only", () => {
    const fg = css.split("\n").filter((l) => l.startsWith(".fg-"));
    expect(fg.length).toBeGreaterThan(10);
    expect(fg.filter((l) => /(margin|padding|border)-(left|right)|[^-]left:|[^-]right:|text-align: (left|right)/.test(l))).toEqual([]);
  });
  it("uses the app's tokens, no colours of its own", () => {
    const fg = css.split("\n").filter((l) => l.startsWith(".fg-"));
    expect(fg.filter((l) => /#[0-9a-fA-F]{3,8}\b|rgba?\(/.test(l))).toEqual([]);
  });
});

// Review round 2, R11: the shell's Layout chips and the grid's own Rows by speak the language shown,
// with the Group by's words (common.json) rather than an English copy of them.
describe("in Persian", () => {
  afterEach(async () => {
    await setLocale("en");
  });
  const chips = () => renderToStaticMarkup(<ChoiceChips label="" className="layout-pick" aria="" value="columns" options={LAYOUTS} onPick={() => {}} />);
  it("names the Layout chips in the language shown, each time they're drawn", async () => {
    expect(text(chips())).toContain("Columns");
    await setLocale("fa");
    expect(LAYOUTS.map((l) => l.label)).toEqual(["ستون‌ها", "شبکه‌ی قابلیت‌ها"]);
    expect(text(chips())).toContain("ستون‌ها");
    expect(text(chips())).not.toMatch(/Columns|Feature grid/);
  });
  it("labels the grid's rows and its corner with the Group by's own words", async () => {
    await setLocale("fa");
    const html = draw({ by: "goal" });
    const segments = all_(html, /class="ui-segment(?: ui-segment--active)?">([^<]*)</g).map((m) => m[1]);
    expect(segments).toEqual(["قابلیت", "هدف", "هیچ‌کدام"]);
    expect(html).toMatch(/ui-segment--active">هدف</);
    expect(byRole(html, "columnheader")[0]).toBe("هدف");
    expect(text(html)).not.toMatch(/By feature|By goal|No grouping/);
  });

  // mu-147: every word of the grid is in the language shown; what stays Latin is the tasks' own
  // text (titles, keys, holders) and the project's label.
  const DATA = new Set(all.flatMap((t) => [t.title, t.id, t.prefix ?? "", t.claimedBy, t.heldBy, t.queue]).concat("Muster").join(" ").match(/[A-Za-z]+/g));
  /** The Latin words a person sees or hears that aren't the fixture's own data. */
  const latin = (html: string) => {
    const attrs = [...html.matchAll(/\s(?:title|aria-label)="([^"]*)"/g)].map((m) => m[1]);
    const words = [text(html), ...attrs].join(" ").replace(/&[a-z#0-9]+;/gi, " ").match(/[A-Za-z]+/g) ?? [];
    return [...new Set(words)].filter((w) => !DATA.has(w));
  };
  it("English: the check sees the grid's own words", () => {
    for (const w of ["more", "waiting", "built", "testing", "Only", "needs", "sent", "full", "Feature"]) expect(latin(draw())).toContain(w);
  });
  it("Persian: no English word in the grid, its header line, its alert, its cells or its cards", async () => {
    await setLocale("fa");
    const html = draw();
    expect(latin(html)).toEqual([]);
    expect(text(html)).toContain("۵ مورد در انتظار شما");
    expect(text(html)).toContain("فقط نیاز به شما");
    expect(text(html)).toContain("«بررسی شما» پر است (۵ از ۵)");
    // "+3 more" in a cell: the sign with its number, left to right, before "دیگر".
    expect(html).toContain(">⁦+۳⁩ دیگر</button>");
    expect(html).toContain('aria-label="شبکه‌ی قابلیت‌ها"');
    // With no grouping, the one row is All tasks, said in Persian; Other too.
    expect(latin(draw({ by: "none", rows: gridRows({ work, all, notes: [], by: "none", today: TODAY }) }))).toEqual([]);
  });
  it("Persian: a row kept from before the switch reads its Other and All tasks in the language shown", async () => {
    const rows = gridRows({ work: [task({ id: "0200", title: "Loose" })], all, notes: [], by: "feature", today: TODAY });
    expect(rows.map((r) => r.title)).toEqual(["Other"]);
    await setLocale("fa");
    expect(rows.map((r) => r.title)).toEqual(["سایر"]);
  });
});
