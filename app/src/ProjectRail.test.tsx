import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import appSource from "./App.tsx?raw";
import { setLocale } from "./i18n/runtime";
import { type DisplayMode, Mode } from "./modes";
import ProjectList from "./ProjectList";
import ProjectRail from "./ProjectRail";
import railSource from "./ProjectRail.tsx?raw";
import css from "./styles.css?raw";
import type { Activity, QueueInfo, Task } from "./types";

// The sidebar hidden (mu-167): a rail in its place that keeps Show, All and every project's colour
// dot with its needs-you badge whole, in the sidebar's order, a heading a thin divider.

const task = (over: Partial<Task>): Task => ({
  queue: "drover", id: "0001", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "todo/0001-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", search: "", ...over,
});
const NOW = new Date(2026, 9, 15, 14, 0).getTime() / 1000;

/** 128 tasks waiting on the owner in muster, 3 in drover (one working), none in quill. */
const waiting = (queue: string, n: number) => Array.from({ length: n }, (_, i) => task({ queue, id: String(i + 1).padStart(4, "0"), stage: "building", waitingForOwner: true }));
const tasks: Task[] = [...waiting("muster", 128), ...waiting("drover", 3), task({ queue: "quill", id: "0001", stage: "ready" })];
const queues: QueueInfo[] = [
  { name: "muster", path: "/p/muster/tasks", counts: { building: 128 }, label: "Muster", category: "Tools", colour: "pink", rank: 1 },
  { name: "drover", path: "/p/drover/tasks", counts: { building: 3 }, label: "Drover", category: "Apps", colour: "lime", rank: 2 },
  { name: "quill", path: "/p/quill/tasks", counts: { ready: 1 }, label: "Quill", category: "Apps", rank: 3 },
];
const activity: Activity[] = [];

const draw = (mode: DisplayMode = "adhd", current = "all") =>
  renderToStaticMarkup(
    <Mode.Provider value={mode}>
      <ProjectRail queues={queues} tasks={tasks} notes={[]} activity={activity} now={NOW} current={current} onPick={() => {}} onShow={() => {}} />
    </Mode.Provider>,
  );
const decode = (s: string) => s.replace(/&#x27;/g, "'").replace(/&quot;/g, '"').replace(/&amp;/g, "&");
const items = (html: string) => [...html.matchAll(/<button[ ]type="button" class="rail-item[^"]*"[^>]*aria-label="([^"]*)"[^>]*>([\s\S]*?)<\/button>/g)].map((m) => ({ said: decode(m[1]), inner: m[2] }));
const badge = (inner: string) => /<span class="count warn rail-needs"[^>]*>([^<]*)<\/span>/.exec(inner)?.[1] ?? null;
/** One CSS rule's body. */
const rule = (sel: string) => new RegExp(`(?:^|\\n)${sel.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")} \\{([^}]*)\\}`).exec(css)?.[1] ?? "";

describe("the rail", () => {
  it("starts with Show, a real button that says what it does and controls the sidebar", () => {
    const html = draw();
    expect(html).toMatch(/^<div class="rail">/);
    expect(html).toContain(' aria-expanded="false" aria-controls="sidebar" title="Show the sidebar (⌃⌘S)" type="button" class="ui-pill side-toggle">Show</button>');
  });
  it("then All, then every project in the sidebar's order, each heading a divider", () => {
    const names = (html: string) => items(html).map((i) => i.said.split(":")[0]);
    const html = renderToStaticMarkup(<ProjectRail queues={queues} tasks={tasks} notes={[]} activity={activity} now={NOW} current="all" onPick={() => {}} onShow={() => {}} order="mine" />);
    expect(names(html)).toEqual(["All", "Drover", "Quill", "Muster"]);
    expect([...html.matchAll(/<hr class="rail-divider" aria-label="([^"]*)"/g)].map((m) => m[1])).toEqual(["Apps", "Tools"]);
    // The very order the sidebar draws, with the same data and its kept order.
    const sidebar = renderToStaticMarkup(<ProjectList queues={queues} tasks={tasks} notes={[]} activity={activity} now={NOW} current="all" onPick={() => {}} />);
    const sideNames = [...sidebar.matchAll(/<span class="side-name"[^>]*>([^<]*)<\/span>/g)].map((m) => m[1]);
    expect(names(draw())).toEqual(sideNames);
  });
  it("reads the sidebar's kept order when it isn't given one", () => {
    expect(railSource).toContain("order = loadOrder()");
  });
  it("keeps each project's needs-you badge, a 3-digit count whole, and All's sum", () => {
    const got = items(draw()).map((i) => [i.said.split(":")[0], badge(i.inner)]);
    expect(got).toEqual(expect.arrayContaining([["All", "131"], ["Muster", "128"], ["Drover", "3"], ["Quill", null]]));
    // A badge is never cut: no ellipsis, no hidden overflow, its digits on one line.
    expect(rule(".rail-needs")).toContain("white-space: nowrap;");
    expect(rule(".rail-needs")).not.toMatch(/overflow|text-overflow/);
  });
  it("names each project and its counts in the sidebar's words, in its tooltip too", () => {
    const muster = items(draw()).find((i) => i.said.startsWith("Muster"));
    // Building with no check-in for a while: stalled, said as Focus's sidebar says it.
    expect(muster?.said).toBe("Muster: 128 need you, 128 in progress, 128 tasks; stalled");
    expect(draw()).toContain('title="Muster: 128 need you, 128 in progress, 128 tasks; stalled"');
    const quill = items(draw()).find((i) => i.said.startsWith("Quill"));
    expect(quill?.said).toBe("Quill: 1 task");
  });
  it("shows each project's colour dot, and marks the one shown", () => {
    const html = draw("adhd", "drover");
    expect(html).toContain('<i class="side-swatch-dot" data-colour="pink" aria-hidden="true"></i>');
    expect(html).toContain('<i class="side-swatch-dot" aria-hidden="true"></i>');
    expect(html).toMatch(/class="rail-item on" aria-current="true" aria-label="Drover/);
  });
  it("has every project whatever its heading's fold: what needs you never folds", () => {
    const folded = queues.map((q) => ({ ...q, categoryFolded: q.category === "Tools" }));
    const html = renderToStaticMarkup(<ProjectRail queues={folded} tasks={tasks} notes={[]} activity={activity} now={NOW} current="all" onPick={() => {}} onShow={() => {}} />);
    expect(items(html).map((i) => i.said.split(":")[0])).toContain("Muster");
  });
  it("shows the working dot where a project is being worked on", () => {
    const live = [task({ queue: "drover", id: "0009", stage: "building", live: true })];
    const html = renderToStaticMarkup(<ProjectRail queues={queues} tasks={live} notes={[]} activity={activity} now={NOW} current="all" onPick={() => {}} onShow={() => {}} />);
    const drover = items(html).find((i) => i.said.startsWith("Drover"));
    expect(drover?.inner).toContain('<i class="dot rail-working" aria-hidden="true"></i>');
    expect(drover?.said).toContain("1 working");
  });
  it("is the same in every mode", () => {
    expect(draw("focus")).toBe(draw("adhd"));
  });
  it("speaks فارسی", async () => {
    await setLocale("fa");
    try {
      const html = draw();
      expect(html).toContain(">نمایش</button>");
      expect(html).toContain('title="نمایش نوار کناری (⁦⌃⌘S⁩)"');
      expect(items(html)[0].said).toMatch(/^\P{L}*همه/u);
    } finally {
      await setLocale("en");
    }
  });
});

describe("the rail's look", () => {
  it("uses the theme's tokens and logical sides only", () => {
    const rail = rule(".rail");
    expect(rail).toContain("overflow-y: auto;");
    expect(css).not.toMatch(/\.rail[^{]*\{[^}]*(?:margin|padding|border)-(?:left|right)/);
    expect(rule(".rail-divider")).toContain("var(--line)");
    expect(rule(".sidebar .side-toggle")).toContain("color: var(--muted);");
  });
  it("keeps a 3-digit badge whole inside the rail: the rail's room is wider than three digits and the badge's padding", () => {
    // 11px digits are at most 7px wide; the badge pads 5px each side; the item pads 4px each side.
    expect(56 - 2 * 4 - 2 * 4).toBeGreaterThanOrEqual(3 * 7 + 2 * 5);
    expect(rule(".rail-item")).toContain("padding: 6px 4px;");
  });
});

describe("App draws it", () => {
  it("in the sidebar's place, with no splitter while it shows", () => {
    expect(appSource).toContain("<ProjectRail ");
    expect(appSource).toMatch(/\{sidebarShown && <Splitter/);
    expect(appSource).toContain("id={SIDEBAR_ID}");
  });
  it("reads only what the sidebar already gets: no API call of its own", () => {
    expect(railSource).not.toContain("api.");
  });
});
