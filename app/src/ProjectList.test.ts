import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import appSource from "./App.tsx?raw";
import css from "./styles.css?raw";
import dashafzar from "./dashafzar-queues.json";
import listSource from "./ProjectList.tsx?raw";
import projectsSource from "./Projects.tsx?raw";
import sidebarSource from "./SidebarProjects.tsx?raw";
import ProjectList, { ActivityLine, labelToSave, OrderChips, type ProjectEdits, SideLabelEdit, SumsLine } from "./ProjectList";
import { ColourDot, Swatches } from "./ProjectSwatches";
import { cleanHeading, foldKey, HeadingEdit, HeadingFrame, NewCategory, SideRow, stepOf } from "./SidebarProjects";
import { type DisplayMode, Mode } from "./modes";
import type { ProjectActivity, Sums } from "./activity";
import type { Activity, QueueInfo, Task } from "./types";

const task = (over: Partial<Task>): Task => ({
  queue: "drover", id: "0001", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "todo/0001-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", search: "", ...over,
});

const secs = (h: number, m: number) => new Date(2026, 9, 13, h, m).getTime() / 1000;
const NOW = secs(14, 0);
const at = (h: number, m: number) => `2026-10-13 ${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;

const line = (a: ProjectActivity) => renderToStaticMarkup(createElement(ActivityLine, { a, now: NOW }));
const sums = (s: Sums) => renderToStaticMarkup(createElement(SumsLine, { s, now: NOW }));
const state = (html: string) => /<span class="side-state">(.*?)<\/span><span class="side-when"/.exec(html)?.[1];
const when = (html: string) => /<span class="side-when"[^>]*>([^<]*)<\/span>/.exec(html)?.[1];
const plain = (html: string) => html.replace(/<[^>]+>/g, "");

describe("a project row's activity line", () => {
  it("working: the Building-colour dot and how many, then the last activity", () => {
    const html = line({ working: 2, stalled: false, last: 0 });
    expect(state(html)).toMatch(/^<span class="side-working" title="2 tasks being worked on now: [^"]+"><i class="dot" aria-hidden="true"><\/i>2 working<\/span>$/);
    expect(when(html)).toBe("now");
    expect(html).not.toContain("side-stalled");
  });
  it("one working says task, not tasks, in its title", () => {
    expect(line({ working: 1, stalled: false, last: 4 })).toContain('title="1 task being worked on now');
  });
  it("stalled: the word in the attention colour, no dot", () => {
    const html = line({ working: 0, stalled: true, last: 45 });
    expect(state(html)).toMatch(/^<span class="side-stalled" title="Stalled: [^"]+">stalled<\/span>$/);
    expect(when(html)).toBe("45 min ago");
    expect(html).not.toContain('class="dot"');
  });
  it("quiet: no dot and no mark, only when it last moved", () => {
    const html = line({ working: 0, stalled: false, last: 3 * 60 + 5 });
    expect(state(html)).toBe("");
    expect(when(html)).toBe("3 h ago");
  });
  it("never moved: says so, and keeps the line so the row doesn't change height", () => {
    const html = line({ working: 0, stalled: false, last: null });
    expect(state(html)).toBe("");
    expect(when(html)).toBe("no activity");
  });
});

describe("the All projects row's sums", () => {
  it("says how many projects are working and how many stalled", () => {
    const html = sums({ working: 3, stalled: 1, last: 0 });
    expect(plain(state(html) ?? "")).toBe("3 working · 1 stalled");
    expect(html).toContain('title="3 projects being worked on now');
    expect(html).toContain('<span class="side-stalled" title="1 project stalled: ');
    expect(html).toContain('<i class="dot" aria-hidden="true">');
  });
  it("leaves out what's zero", () => {
    expect(plain(state(sums({ working: 1, stalled: 0, last: 2 })) ?? "")).toBe("1 working");
    expect(sums({ working: 1, stalled: 0, last: 2 })).toContain('title="1 project being worked on now');
    const stalled = sums({ working: 0, stalled: 2, last: 40 });
    expect(plain(state(stalled) ?? "")).toBe("2 stalled");
    expect(stalled).not.toContain('class="dot"');
    expect(state(sums({ working: 0, stalled: 0, last: 1500 }))).toBe("");
    expect(when(sums({ working: 0, stalled: 0, last: 1500 }))).toBe("yesterday");
  });
});

describe("the order chips", () => {
  it("are all on screen, the current one pressed", () => {
    const html = renderToStaticMarkup(createElement(OrderChips, { value: "az", onChange: () => {} }));
    expect(html).toContain('role="group" aria-label="Order projects"');
    expect(html.match(/<button[^>]*>([^<]*)<\/button>/g)?.map(plain)).toEqual(["Working", "A–Z", "Needs you", "Mine"]);
    expect(html.match(/aria-pressed="true"[^>]*class="ui-pill ui-pill--on">([^<]*)</)?.[1]).toBe("A–Z");
    expect(html.match(/aria-pressed="false"/g)).toHaveLength(3);
  });
});

describe("the sidebar", () => {
  const queues: QueueInfo[] = [
    { name: "alpha", path: "/p/alpha/tasks", counts: { ready: 1 } },
    { name: "zeta", path: "/p/zeta/tasks", counts: { building: 1, ready: 1 } },
    { name: "muster", path: "/p/muster/tasks", counts: { building: 1 } },
  ];
  const tasks = [
    task({ queue: "alpha", id: "0001", waitingForOwner: true, modified: secs(9, 0) }),
    task({ queue: "zeta", id: "0001", stage: "building", claimedBy: "zeta/wt-a (session 7b5e8856)", checkins: [{ at: at(13, 50), who: "zeta/wt-a", pct: 40, text: "tests" }] }),
    task({ queue: "zeta", id: "0002", live: true }),
    task({ queue: "muster", id: "0001", stage: "building", claimedBy: "muster/wt-b", heartbeat: at(12, 0) }),
  ];
  const activity: Activity[] = [{ queue: "alpha", at: secs(13, 0), what: "commit", source: "commit" }];
  const render = () =>
    renderToStaticMarkup(createElement(ProjectList, { queues, tasks, notes: [], activity, now: NOW, current: "zeta", onPick: () => {} }));
  const rows = (html: string) => [...html.matchAll(/<span class="side-name">([^<]*)<\/span>/g)].map((m) => m[1]);
  const row = (html: string, name: string) => html.split('<button class="side-item').find((r) => r.includes(`side-name">${name}<`)) ?? "";

  afterEach(() => vi.unstubAllGlobals());

  it("orders Working first by default, with nothing stored (storage absent, as in a test run)", () => {
    const html = render();
    expect(rows(html)).toEqual(["All", "zeta", "alpha", "muster"]);
    expect(html.match(/aria-pressed="true"[^>]*class="ui-pill ui-pill--on">([^<]*)</)?.[1]).toBe("Working");
  });
  it("shows each row's state: working with its count, quiet, stalled", () => {
    const html = render();
    expect(plain(row(html, "zeta"))).toContain("2 working");
    expect(row(html, "zeta")).toContain(" on");
    expect(when(row(html, "zeta"))).toBe("now");
    expect(row(html, "alpha")).not.toContain("side-working");
    expect(row(html, "alpha")).not.toContain("side-stalled");
    expect(when(row(html, "alpha"))).toBe("1 h ago");
    expect(row(html, "muster")).toContain('class="side-stalled"');
    expect(when(row(html, "muster"))).toBe("2 h ago");
  });
  it("under a stalled project, asks its quiet sessions for a check-in; disabled with why when none can be reached (0039)", () => {
    const html = render();
    const pings = [...html.matchAll(/<div class="side-ping">([\s\S]*?)<\/div>/g)].map((m) => plain(m[1].replace(/></g, "> <")));
    // Only muster is stalled; its one quiet claim has no session stamp.
    expect(pings).toEqual(["Ask all quiet sessions for a check-in none of its quiet claims has a session to reach"]);
    expect(html).toMatch(/<button type="button" disabled="" class="ui-button">Ask all quiet sessions for a check-in<\/button>/);
    expect(html.indexOf('class="side-ping"')).toBeGreaterThan(html.indexOf('side-name">muster<'));
  });
  it("keeps the counts it had: needs you, in progress and the total", () => {
    const html = render();
    expect(row(html, "alpha")).toContain('<span class="count warn" title="1 need you">1</span>');
    expect(row(html, "zeta")).toContain('<span class="side-doing" title="1 in progress (Building or Testing)">1</span>');
    expect(row(html, "zeta")).toContain("<small>2</small>");
  });
  it("sums the projects in the All row", () => {
    const all = row(render(), "All");
    expect(plain(all)).toContain("1 working · 1 stalled");
    expect(when(all)).toBe("now");
  });
  it("uses the remembered order", () => {
    vi.stubGlobal("localStorage", { getItem: (k: string) => (k === "muster.sidebarOrder" ? "az" : null) });
    expect(rows(render())).toEqual(["All", "alpha", "muster", "zeta"]);
    vi.stubGlobal("localStorage", { getItem: () => "needs" });
    expect(rows(render())).toEqual(["All", "alpha", "zeta", "muster"]);
  });
  it("falls back to Working first when storage throws or holds something else", () => {
    vi.stubGlobal("localStorage", { getItem: () => { throw new Error("SecurityError"); } });
    expect(rows(render())).toEqual(["All", "zeta", "alpha", "muster"]);
    vi.stubGlobal("localStorage", { getItem: () => "sideways" });
    expect(rows(render())).toEqual(["All", "zeta", "alpha", "muster"]);
  });
});

describe("labels and categories in the sidebar (0044)", () => {
  const queues: QueueInfo[] = [
    { name: "alpha", path: "/p/alpha/tasks", counts: { ready: 1 }, label: "Alpha app", category: "Apps" },
    { name: "zeta", path: "/p/zeta/tasks", counts: { building: 1, ready: 1 }, label: "zeta", category: "Tools" },
    { name: "muster", path: "/p/muster/tasks", counts: { building: 1 }, label: "Muster", category: "Tools" },
    // An older core: no label, no category.
    { name: "beta", path: "/p/beta/tasks", counts: { ready: 2 } },
  ];
  const tasks = [
    task({ queue: "alpha", id: "0001", waitingForOwner: true, modified: secs(9, 0) }),
    task({ queue: "zeta", id: "0001", stage: "building", claimedBy: "zeta/wt-a (session 7b5e8856)", checkins: [{ at: at(13, 50), who: "zeta/wt-a", pct: 40, text: "tests" }] }),
    task({ queue: "zeta", id: "0002", live: true }),
    task({ queue: "muster", id: "0001", stage: "building", claimedBy: "muster/wt-b", heartbeat: at(12, 0), waitingForOwner: true }),
    task({ queue: "beta", id: "0001" }),
    task({ queue: "beta", id: "0002" }),
  ];
  const render = (mode: DisplayMode = "adhd") =>
    renderToStaticMarkup(createElement(Mode.Provider, { value: mode }, createElement(ProjectList, { queues, tasks, notes: [], activity: [], now: NOW, current: "all", onPick: () => {}, onLabel: async () => {} })));
  const rows = (html: string) => [...html.matchAll(/<span class="side-name"[^>]*>([^<]*)<\/span>/g)].map((m) => m[1]);
  const heads = (html: string) => [...html.matchAll(/<div class="side-cat"[^>]*>([\s\S]*?)<\/div>/g)].map((m) => plain(m[1].replace(/></g, "> <")).replace(/\s+/g, " ").trim());

  afterEach(() => vi.unstubAllGlobals());

  it("groups the projects under their category headings, A–Z with Other last, each row by its label", () => {
    const html = render();
    // Each heading's counts in words (0082): what needs you, its projects, what's being worked on.
    expect(heads(html)).toEqual(["Apps 1 needs you · 1 project", "Tools 1 needs you · 2 projects · 2 working", "Other 1 project"]);
    expect(rows(html)).toEqual(["All", "Alpha app", "zeta", "Muster", "beta"]);
    // Each heading wraps its own rows.
    expect(html).toMatch(/<div class="side-group" role="group" aria-label="Tools"><div class="side-cat" data-heading="Tools"/);
  });
  it("says each heading's counts and its tasks in its tooltip, in the tokens a row uses (0082)", () => {
    const html = render();
    expect(html).toContain('<span class="side-cat-counts" title="1 needs you · 2 projects · 2 working · 3 tasks">');
    expect(html).toContain('<span class="side-cat-counts" title="1 project · 2 tasks">');
    expect(html).toContain('<span class="side-cat-needs"><span class="count warn">1</span> <span class="side-cat-needs-words">needs you</span></span>');
    expect(html).toContain('<span class="side-cat-part side-working">\u00a0· <i class="dot" aria-hidden="true"></i>2 working</span>');
    // The name says itself whole in its tooltip, as it may end in "…" (0082 round 3).
    expect(html).toContain('<span class="side-cat-text"><span class="side-cat-name" title="Tools">Tools</span>');
  });
  it("sorts by the order chips within each heading: A–Z goes by the label", () => {
    vi.stubGlobal("localStorage", { getItem: () => "az" });
    expect(rows(render())).toEqual(["All", "Alpha app", "Muster", "zeta", "beta"]);
  });
  it("names the real project and its folder in the row's tooltip, and offers the label on a double click", () => {
    const html = render();
    const muster = html.split('<button class="side-item').find((r) => r.includes('side-name" title=')  && r.includes(">Muster<")) ?? "";
    expect(muster).toContain('title="muster · /p/muster/tasks"');
    // The name keeps the row's tooltip, where the pointer is (review): the real name first.
    expect(muster).toContain('<span class="side-name" title="muster · /p/muster/tasks\nDouble-click to change the label">Muster</span>');
  });
  it("keeps the headings and the same counts in Focus's narrow sidebar (0082: the same in every mode)", () => {
    const html = render("focus");
    expect(heads(html)).toEqual(["Apps 1 needs you · 1 project", "Tools 1 needs you · 2 projects · 2 working", "Other 1 project"]);
    expect(rows(html)).toEqual(["All", "Alpha app", "zeta", "Muster", "beta"]);
  });
});

describe("a label set in place in the sidebar (0044)", () => {
  it("is a field in the row's place with the label, at most 40 characters, the real name under it", () => {
    const html = renderToStaticMarkup(createElement(SideLabelEdit, { name: "muster", label: "Muster", onSave: async () => {}, onDone: () => {} }));
    expect(html).toContain('<div class="side-item on side-editing">');
    expect(html).toContain('value="Muster"');
    expect(html).toContain('maxLength="40"');
    expect(html).toContain('aria-label="Label for muster"');
    expect(html).toContain('placeholder="muster"');
    expect(plain(html)).toContain("muster · ↩ saves, Esc keeps it");
  });
  it("saves the folder's name typed back as no label at all", () => {
    expect(labelToSave("  muster ", "muster")).toBe("");
    expect(labelToSave("Muster", "muster")).toBe("Muster");
    expect(labelToSave("", "muster")).toBe("");
  });
});

describe("the sidebar arranged in place (0057)", () => {
  const queues: QueueInfo[] = [
    { name: "alpha", path: "/p/alpha/tasks", counts: { ready: 1 }, label: "Alpha app", category: "Apps", colour: "lime", rank: 2, categoryRank: 1 },
    { name: "zeta", path: "/p/zeta/tasks", counts: { ready: 1 }, label: "zeta", category: "Tools", rank: 0, categoryRank: 0 },
    { name: "muster", path: "/p/muster/tasks", counts: { ready: 1 }, label: "Muster", category: "Apps", colour: "orchid", rank: 1, categoryRank: 1 },
    // Never placed: after the placed ones, and its heading after the placed headings.
    { name: "beta", path: "/p/beta/tasks", counts: { ready: 2 } },
  ];
  const tasks = [task({ queue: "zeta", id: "0001", stage: "building", claimedBy: "zeta/wt-a", live: true }), task({ queue: "beta", id: "0001" })];
  const edits: ProjectEdits = { arrange: async () => {}, renameCategory: async () => {}, colour: async () => {}, fold: async () => {} };
  // `null` for no way to save.
  const render = (mode: DisplayMode = "adhd", edit: ProjectEdits | null = edits) =>
    renderToStaticMarkup(createElement(Mode.Provider, { value: mode }, createElement(ProjectList, { queues, tasks, notes: [], activity: [], now: NOW, current: "all", onPick: () => {}, onLabel: async () => {}, edit: edit ?? undefined })));
  const rows = (html: string) => [...html.matchAll(/<span class="side-name"[^>]*>([^<]*)<\/span>/g)].map((m) => m[1]);
  const headings = (html: string) => [...html.matchAll(/<div class="side-cat[^"]*" data-heading="([^"]*)"/g)].map((m) => m[1]);
  const pressedChip = (html: string) => html.match(/aria-pressed="true"[^>]*class="ui-pill ui-pill--on">([^<]*)</)?.[1];

  afterEach(() => vi.unstubAllGlobals());

  it("in Mine order, shows the owner's headings and projects in their places, the rest after", () => {
    vi.stubGlobal("localStorage", { getItem: () => "mine" });
    const html = render();
    expect(pressedChip(html)).toBe("Mine");
    expect(headings(html)).toEqual(["Tools", "Apps", "Other"]);
    expect(rows(html)).toEqual(["All", "zeta", "Muster", "Alpha app", "beta"]);
  });
  it("keeps the owner's headings in every order; the chips order the projects under them", () => {
    vi.stubGlobal("localStorage", { getItem: () => "az" });
    const html = render();
    expect(headings(html)).toEqual(["Tools", "Apps", "Other"]);
    expect(rows(html)).toEqual(["All", "zeta", "Alpha app", "Muster", "beta"]);
  });
  it("makes every project row draggable, and says ⌥↑ ⌥↓ move it", () => {
    const html = render();
    expect(html).toContain('<div class="side-row" data-key="/p/alpha/tasks" draggable="true">');
    expect(html.match(/<button class="side-item"[^>]*aria-keyshortcuts="Alt\+ArrowUp Alt\+ArrowDown"/g)).toHaveLength(4);
    // All isn't a project: it doesn't move.
    expect(html).not.toContain('data-key="all"');
  });
  it("makes every heading focusable and draggable, says how to move and rename it", () => {
    const html = render();
    // A heading, focusable for its keys (0070: its chevron is a button inside it, so not a button itself).
    expect(html).toContain('<div class="side-cat" data-heading="Apps" tabindex="0" role="heading" aria-level="2" aria-keyshortcuts="ArrowLeft ArrowRight Alt+ArrowUp Alt+ArrowDown" aria-label="Apps, category heading: 2 projects; ↩ renames it, ⌥↑ ⌥↓ move it, ← → fold it" title="Double-click or ↩ to rename; drag it, or ⌥↑ ⌥↓, to move it with its projects; ← → fold it" draggable="true">');
    expect(html).toContain('aria-label="Tools, category heading: 1 project, 1 working; ↩ renames it, ⌥↑ ⌥↓ move it, ← → fold it"');
  });
  it("puts each project's colour as a dot at the left of its row, a button that lays the colours out", () => {
    const html = render();
    expect(html).toContain('<button type="button" class="side-swatch-dot" data-colour="lime" aria-expanded="false" aria-label="Colour for Alpha app: Lime. Click to choose" title="Colour: Lime. Click to choose"></button>');
    expect(html).toContain('<button type="button" class="side-swatch-dot" aria-expanded="false" aria-label="Colour for zeta: no colour. Click to choose"');
    // The dot comes before the row's name.
    const alpha = html.slice(html.indexOf('data-key="/p/alpha/tasks"'));
    expect(alpha.indexOf("side-swatch-dot")).toBeLessThan(alpha.indexOf('class="side-name"'));
  });
  it("offers + Category at the bottom, and a line a screen reader hears moves on", () => {
    const html = render();
    expect(html).toMatch(/<button type="button" title="A new heading: then drag a project onto it" class="ui-pill side-new-cat">\+ Category<\/button><p class="sr-only" role="status" aria-live="polite"><\/p><\/nav>$/);
    // Focus narrows the sidebar: no + Category, but rows and headings still move, with their dots.
    const focus = render("focus");
    expect(focus).not.toContain("+ Category");
    expect(focus).toContain('draggable="true"');
    expect(focus).toContain('data-colour="lime"');
  });
  it("without a way to save, nothing moves: the dots only show the colour", () => {
    const html = render("adhd", null);
    expect(html).not.toContain("draggable");
    expect(html).not.toContain("tabindex");
    expect(html).not.toContain("+ Category");
    expect(html).toContain('<i class="side-swatch-dot" data-colour="lime" title="Colour: Lime" aria-hidden="true"></i>');
  });
});

describe("a project's colours, laid out in place (0057)", () => {
  const swatches = (value: string, onClose?: () => void) => renderToStaticMarkup(createElement(Swatches, { name: "Muster", value, onPick: () => {}, onClose }));
  it("are None and the palette as a group of buttons, the current one pressed", () => {
    const html = swatches("tan", () => {});
    expect(html).toMatch(/^<div class="proj-swatches" role="group" aria-label="Colour for Muster">/);
    expect([...html.matchAll(/aria-label="([^"]+)" title/g)].map((m) => m[1])).toEqual(["None", "Lime", "Orchid", "Pink", "Tan", "Mint"]);
    expect(html.match(/aria-pressed="true"[^>]*aria-label="([^"]+)"/)?.[1]).toBe("Tan");
    expect(html).toContain('<button type="button" class="proj-swatch" data-colour="tan" aria-pressed="true" aria-label="Tan" title="Tan" autofocus=""></button>');
    expect(swatches("")).toMatch(/<button type="button" class="proj-swatch" aria-pressed="true" aria-label="None" title="None"><\/button>/);
  });
  it("as the row's dot: open says so", () => {
    expect(renderToStaticMarkup(createElement(ColourDot, { name: "Muster", colour: "mint", open: true, onClick: () => {} }))).toContain('aria-expanded="true" aria-label="Colour for Muster: Mint. Click to choose"');
  });
  it("a row with its colours laid out shows them under it", () => {
    const html = renderToStaticMarkup(createElement(SideRow, { rowKey: "/m", label: "Muster", colour: "pink", swatches: true, onDot: () => {}, onColour: () => {}, onFold: () => {}, children: createElement("b", null, "row") }));
    expect(html).toMatch(/^<div class="side-row" data-key="\/m"><span class="side-row-line"><button[^>]*side-swatch-dot[^>]*aria-expanded="true"[^>]*><\/button><b>row<\/b><\/span><div class="proj-swatches" role="group" aria-label="Colour for Muster">/);
  });
});

describe("headings made and renamed in place (0057)", () => {
  it("+ Category opens a field in its place", () => {
    expect(renderToStaticMarkup(createElement(NewCategory, { onAdd: () => "" }))).toBe('<button type="button" title="A new heading: then drag a project onto it" class="ui-pill side-new-cat">+ Category</button>');
    const open = renderToStaticMarkup(createElement(NewCategory, { onAdd: () => "", open: true }));
    expect(open).toContain('<input class="side-label-input" autofocus="" maxLength="24" aria-label="New category" placeholder="New category"/>');
    expect(plain(open).replace(/&#x27;/g, "'")).toContain("↩ makes it, Esc doesn't · then drag a project onto it");
  });
  it("a new heading has no projects yet: it says how to give it one, and Remove takes it away", () => {
    const html = renderToStaticMarkup(createElement(HeadingFrame, { name: "Later", count: 0, fresh: true, onRename: () => {}, onRemove: () => {}, children: createElement("span", { className: "side-cat-name" }, "Later") }));
    expect(html).toContain('aria-label="Later, category heading, new, no projects yet: ↩ renames it"');
    // Its row wraps, so the hint goes under the name and never squeezes it out (review R2).
    expect(html).toMatch(/^<div class="side-cat fresh" data-heading="Later"[^>]*><span class="side-cat-name">Later<\/span><span class="side-cat-fresh">/);
    expect(css).toContain(".side-cat.fresh { flex-wrap: wrap; }");
    expect(css).toMatch(/\.side-cat-fresh \{ flex-basis: 100%;/);
    expect(plain(html)).toContain("empty: drag a project here, or ⌥↓ the last one above");
    expect(html).toContain('<button type="button" title="Take this new heading away" class="ui-pill side-cat-remove">Remove</button>');
  });
  it("renaming is a field in the heading's place with its name, at most 24 characters", () => {
    const html = renderToStaticMarkup(createElement(HeadingEdit, { name: "Tools", onSave: async () => {}, onDone: () => {} }));
    expect(html).toMatch(/^<div class="side-cat side-editing"><input class="side-label-input" autofocus="" maxLength="24" aria-label="Name of the heading Tools" placeholder="Tools" value="Tools"\/>/);
    expect(plain(html).replace(/&#x27;/g, "'")).toContain("↩ renames, Esc keeps it · another heading's name joins the two");
  });
  it("cleans a heading's name as the core does: no control characters, trimmed, 24 at most, Other in any case", () => {
    expect(cleanHeading("  Tools\u0007 ")).toBe("Tools");
    expect(cleanHeading("é".repeat(30))).toBe("é".repeat(24));
    expect(cleanHeading("oTHer")).toBe("Other");
    expect(cleanHeading(" \n")).toBe("");
  });
  it("← folds an open heading, → unfolds a folded one, and nothing else changes it (0070)", () => {
    const ev = (key: string, altKey = false) => ({ key, altKey, metaKey: false, ctrlKey: false });
    expect([foldKey(ev("ArrowLeft"), false), foldKey(ev("ArrowRight"), true)]).toEqual([true, false]);
    expect([foldKey(ev("ArrowLeft"), true), foldKey(ev("ArrowRight"), false)]).toEqual([null, null]);
    expect([foldKey(ev("ArrowLeft", true), false), foldKey(ev("ArrowUp"), false), foldKey(ev("Enter"), true)]).toEqual([null, null, null]);
  });
  it("⌥↑ steps up, ⌥↓ down, a plain arrow neither", () => {
    const ev = (key: string, altKey = true) => ({ key, altKey, metaKey: false, ctrlKey: false });
    expect([stepOf(ev("ArrowUp")), stepOf(ev("ArrowDown")), stepOf(ev("ArrowUp", false)), stepOf(ev("Enter"))]).toEqual([-1, 1, null, null]);
  });
});

describe("a heading's line: the name gives way first, what needs you is always whole and in words (0082 round 3)", () => {
  const all = (sel: string) => {
    const esc = sel.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const out = new Map<string, string>();
    for (const m of css.matchAll(new RegExp(`(?:^|\\n)\\s*${esc} \\{([^}]*)\\}`, "g")))
      for (const d of m[1].split(";").map((x) => x.trim()).filter(Boolean)) out.set(d.slice(0, d.indexOf(":")).trim(), d.slice(d.indexOf(":") + 1).trim());
    return out;
  };
  const px = (v: string | undefined) => Number(/^(\d+(?:\.\d+)?)px$/.exec(v ?? "")?.[1] ?? NaN);
  const em = (v: string | undefined) => Number(/(\d+(?:\.\d+)?)em$/.exec(v ?? "")?.[1] ?? NaN);
  // The widths, from styles.css: room = sidebar − 1 border − 16 nav padding − 16 heading margins;
  // the name and counts get what the chevron and its gap leave.
  const fold = px(all(".side-cat-fold.ui-pill").get("min-width"));
  const gap = px(all(".side-cat").get("gap"));
  const textGap = px(all(".side-cat-text").get("gap")?.split(" ")[1]);
  const nameLeast = em(all(".side-cat-name").get("flex")) * px(all(".side-cat-name").get("font-size"));
  const box = (sidebar: number) => sidebar - 1 - 16 - 16 - fold - gap;
  // "N need you": the chip (a digit about 0.62em of its 11px) and the words (about 0.55em a
  // character), measured in headless Chrome at 73px for 17 and 80px for 120.
  const count = all(".count");
  const chipFont = px(count.get("font-size"));
  const needs = (digits: number) =>
    Math.max(px(count.get("min-width")), digits * 0.62 * chipFont + 2 * px(count.get("padding")?.split(" ")[1])) + 0.3 * chipFont + 8 * 0.55 * chipFont;

  it("keeps \"N need you\" one piece that never shrinks, wraps or ends in \"…\", and never hides its words", () => {
    expect(all(".side-cat-needs").get("flex")).toBe("none");
    expect(all(".side-cat-needs .count.warn").get("flex")).toBe("none");
    for (const sel of [".side-cat-needs", ".side-cat-needs-words", ".side-cat-counts"]) {
      const d = all(sel);
      expect(d.has("text-overflow") || d.has("flex-wrap") || d.get("overflow") === "hidden", sel).toBe(false);
    }
    // The counts may grow but never get narrower than what needs you: their least width is their
    // content's (no min-width: 0), and the rest after it adds nothing to it.
    const counts = all(".side-cat-counts");
    expect(counts.get("flex")).toBe("1 1 0");
    expect(counts.has("min-width")).toBe(false);
    expect(counts.get("white-space")).toBe("nowrap");
    expect(all(".side-cat-rest").get("contain")).toBe("inline-size");
    // No mode or width hides the words some other way (round 2 dropped them at 150px).
    expect(css).not.toMatch(/\.side-cat-needs(-words)? \{[^}]*display: none/);
    expect(css).not.toContain("@container");
  });
  it("lets the name give way first, down to about 4em with an ellipsis, and never grows it past itself", () => {
    const name = all(".side-cat-name");
    // Grows a thousand times faster than the counts, so it takes the room up to its own width
    // (max-content) before the rest after "need you" gets any.
    expect(name.get("flex")).toMatch(/^1000 1 4em$/);
    expect([name.get("max-width"), name.get("min-width"), name.get("overflow"), name.get("text-overflow"), name.get("white-space")]).toEqual(["max-content", "0", "hidden", "ellipsis", "nowrap"]);
    expect(nameLeast).toBe(60);
  });
  it("shows \"· N projects\" and \"· N working\" each whole or not at all", () => {
    const rest = all(".side-cat-rest");
    expect([rest.get("flex-wrap"), rest.get("overflow"), rest.get("align-content"), rest.get("min-width")]).toEqual(["wrap", "hidden", "flex-start", "0"]);
    expect(px(rest.get("height"))).toBe(px(all(".side-cat-counts").get("line-height")));
    const part = all(".side-cat-part");
    expect(part.get("flex")).toBe("none");
    expect(part.has("text-overflow")).toBe(false);
    // A flex line always takes its first item: an empty first piece of the line's height makes a
    // piece too wide for the room wrap out of sight instead of showing cut ("· 1" at 150px).
    const first = all(".side-cat-rest::before");
    expect([first.get("content"), first.get("flex"), first.get("height")]).toEqual(['""', "none", "100%"]);
  });
  it("is one line at the default 210px, a long name and a 3-digit count; under the name only where that can't fit (150px)", () => {
    expect(all(".side-cat-text").get("flex-wrap")).toBe("wrap");
    // 210: the name's least and "120 need you" fit beside each other.
    expect(nameLeast + textGap + needs(3)).toBeLessThanOrEqual(box(210));
    // 150: they don't, so the counts take the line under the name, where even "1200 need you" fits.
    expect(nameLeast + textGap + needs(1)).toBeGreaterThan(box(150));
    expect(needs(4)).toBeLessThanOrEqual(box(150));
  });
  it("says the name and every count whole in the tooltip and the aria-label, whatever shows", () => {
    const qs = [{ name: "a", path: "/p/a/tasks", counts: { ready: 120, building: 1 }, label: "a", category: "ابزارهای فارسی" }] as QueueInfo[];
    const ts = [...Array.from({ length: 120 }, (_, i) => task({ queue: "a", id: String(i + 1).padStart(4, "0"), waitingForOwner: true })), task({ queue: "a", id: "0200", stage: "building", live: true })];
    const html = renderToStaticMarkup(createElement(ProjectList, { queues: qs, tasks: ts, notes: [], activity: [], now: NOW, current: "a", onPick: () => {}, onLabel: async () => {}, edit: { arrange: async () => {}, renameCategory: async () => {}, colour: async () => {}, fold: async () => {} } }));
    expect(html).toContain('<span class="side-cat-name" title="ابزارهای فارسی">ابزارهای فارسی</span>');
    expect(html).toContain('title="120 need you · 1 project · 1 working · 121 tasks"');
    expect(html).toMatch(/aria-label="ابزارهای فارسی, category heading: 120 need you, 1 project, 1 working;/);
  });
});

describe("a heading reads as a heading (0082 AC1)", () => {
  /** Every declaration of `sel`, in the order styles.css has them; the last one of a property wins. */
  const decls = (sel: string) => {
    const esc = sel.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const out = new Map<string, string>();
    for (const m of css.matchAll(new RegExp(`(?:^|\\n)${esc} \\{([^}]*)\\}`, "g")))
      for (const d of m[1].split(";").map((x) => x.trim()).filter(Boolean)) out.set(d.slice(0, d.indexOf(":")).trim(), d.slice(d.indexOf(":") + 1).trim());
    return out;
  };
  const px = (v: string | undefined) => Number(/^(\d+(?:\.\d+)?)px$/.exec(v ?? "")?.[1] ?? NaN);
  it("its name is plainly larger and heavier than a project's name, in the text colour (owner, 0082 round 2)", () => {
    // A project's name: the body's 13px, weight 550 (.side-name). The owner, 2026-10-14: "the
    // category and projects seems the same font and font size" at 13px 650 against 13px 550.
    const body = px(/font: (\d+px)\//.exec(css)?.[1]);
    expect(body).toBe(13);
    const project = Number(decls(".side-name").get("font-weight"));
    expect(project).toBe(550);
    expect(decls(".side-cat").get("color")).toBe("var(--text)");
    const name = decls(".side-cat-name");
    expect(px(name.get("font-size"))).toBeGreaterThanOrEqual(body + 2);
    expect(Number(name.get("font-weight"))).toBeGreaterThanOrEqual(project + 150);
    // Nothing later in the sheet (a mode, the narrow sidebar) brings it back down.
    expect(css).not.toMatch(/\.side-cat-name \{[^}]*font-size: 1[0-4]px/);
  });
  it("its chevron is a drawn chevron in the text colour, turned down when open", () => {
    const icon = decls(".side-cat-chevron");
    expect(px(icon.get("width"))).toBeGreaterThanOrEqual(14);
    expect(px(icon.get("height"))).toBeGreaterThanOrEqual(14);
    expect(decls(".side-cat-fold.ui-pill").get("color")).toBe("var(--text)");
    expect(decls('.side-cat-fold[aria-expanded="true"] .side-cat-chevron').get("transform")).toBe("rotate(90deg)");
  });
  it("starts its group with a divider, and its chevron is a Pill-size button", () => {
    expect(decls(".side-cat").get("border-top")).toBe("1px solid var(--line)");
    const fold = decls(".side-cat-fold.ui-pill");
    expect(px(fold.get("min-width"))).toBeGreaterThanOrEqual(22);
    expect(px(fold.get("min-height"))).toBeGreaterThanOrEqual(22);
  });
  it("indents its projects under it in every mode, mirrored in right-to-left text", () => {
    // Logical properties only, so the indent sits on the start side whichever way the text runs.
    for (const sel of [".side-group > .side-row", ".side-group > .side-item", ".side-group > .side-ping"]) {
      const d = decls(sel);
      expect(px(d.get("margin-inline-start")), sel).toBeGreaterThanOrEqual(12);
      expect([...d.keys()].some((k) => /margin-(left|right)/.test(k)), sel).toBe(false);
    }
    // No mode's rule takes the indent away.
    expect(css).not.toMatch(/data-sidebar="narrow"\][^{]*\.side-group > \.side-row \{[^}]*margin-inline-start: 0/);
    expect(decls(".side-group .side-item").get("text-align")).toBe("start");
    // The folded chevron points to where the text starts.
    expect(decls(".side-cat-fold:dir(rtl)").get("transform")).toBe("scaleX(-1)");
    expect(decls(".side-cat-name").get("unicode-bidi")).toBe("plaintext");
  });
  it("has no more of the chips mu-70 folded under it (owner, 0082: choice A)", () => {
    expect(css).not.toMatch(/\.side-peek/);
    expect(listSource).not.toMatch(/Peeks|peeks\(/);
  });
});

describe("the fold wiring follows the tested rules (0070 review R4, R5, R7)", () => {
  it("a move under a folded heading opens it at once and says both", () => {
    expect(listSource).toContain("const opened = unfoldFor(p, folds);");
    expect(listSource).toContain("setSaid(opened ? `${p.said}; ${opened} unfolded` : p.said);");
    expect(listSource).toContain("trackUnfold(opened, setFoldSet, track(p, setPending, () => edit.arrange(p.order, p.categories, p.moved)))");
  });
  it("a fold goes through trackFold, and each read of the queues through readFolds", () => {
    expect(listSource).toContain("trackFold(name, folded, setFoldSet, () => edit.fold(name, folded))");
    expect(listSource).toContain("useEffect(() => setFoldSet((f) => readFolds(f, coreFolds.current)), [queues]);");
    expect(listSource).not.toMatch(/setFoldSet\(\(f\) => Object\.fromEntries/);
  });
});

describe("the window wires it (0057)", () => {
  it("gives the sidebar its edits, and every place a project is named its colours", () => {
    expect(appSource).toMatch(/<ProjectList [^\n]*edit=\{projectEdits\(refresh\)\} \/>/);
    expect(appSource).toContain("const colours = useMemo(() => coloursOf(state?.queues ?? []), [colourText]);");
    // Inside the labels' provider, around the whole window.
    expect(appSource).toMatch(/<Labels\.Provider value=\{labels\}>\s*<ProjectColours\.Provider value=\{colours\}>\s*<div className="app shell">/);
    expect(appSource).toMatch(/<\/ProjectColours\.Provider>\s*<\/Labels\.Provider>/);
  });
});

describe("the drag wiring follows the tested rules (0057 review R1, R3, R5)", () => {
  it("a row or heading that doesn't take the drag lets its dragover and its drop go on to the group", () => {
    const attrs = sidebarSource.slice(sidebarSource.indexOf("function dragAttrs"), sidebarSource.indexOf("/** A project's row in the sidebar"));
    const over = attrs.slice(attrs.indexOf("onDragOver:"), attrs.indexOf("onDrop:"));
    const drop = attrs.slice(attrs.indexOf("onDrop:"), attrs.indexOf("onKeyDown:"));
    for (const h of [over, drop]) {
      const guard = h.indexOf("if (!d.accepts) return;");
      expect(guard, h).toBeGreaterThanOrEqual(0);
      expect(guard, h).toBeLessThan(h.indexOf("e.stopPropagation()"));
    }
  });
  it("the sidebar routes drops through dropMove and accepts, and settles a move as settle says", () => {
    expect(listSource).toContain("const m = dropMove(layout, drag, t, side);");
    expect(listSource).toContain("accepts: accepts(drag, t),");
    expect(listSource).toContain("useEffect(() => setPending((p) => settle(p, core.current)), [queues]);");
    // Over nothing that takes the drag, or off the sidebar: no drop line.
    expect(listSource).toContain("onDragOver={drag ? (e) => !e.isDefaultPrevented() && hover(null) : undefined}");
    expect(listSource).toMatch(/onDragLeave=\{drag \? \(e\) => !\(e\.relatedTarget instanceof Node && e\.currentTarget\.contains\(e\.relatedTarget\)\) && hover\(null\) : undefined\}/);
  });
  it("both keep a move's bookkeeping through track, never by hand (review R6)", () => {
    expect(listSource).toContain("track(p, setPending, () => edit.arrange(p.order, p.categories, p.moved))");
    expect(projectsSource).toContain("track(plan, setPending, () => api.arrangeProjects(plan.order, plan.categories, plan.moved))");
    for (const src of [listSource, projectsSource]) expect(src).not.toMatch(/setPending\(\{|setPending\(\(x\)/);
  });
  it("Settings plans each ↑ ↓ from the move it showed last", () => {
    expect(projectsSource).toContain("const layout = shownLayout(pending, core);");
    expect(projectsSource).toContain("useEffect(() => setPending((p) => settle(p, core)), [core]);");
    expect(projectsSource).toContain('planMove(layout, { kind: "step-project", key: p.key, by }, named)');
  });
});

describe("folded headings in the sidebar (0070)", () => {
  const queues: QueueInfo[] = [
    { name: "alpha", path: "/p/alpha/tasks", counts: { ready: 1 }, label: "Alpha app", category: "Apps", colour: "lime", categoryFolded: true },
    { name: "muster", path: "/p/muster/tasks", counts: { ready: 1 }, label: "Muster", category: "Apps", categoryFolded: true },
    { name: "beta", path: "/p/beta/tasks", counts: { building: 1 }, label: "beta", category: "Apps", categoryFolded: true },
    { name: "zeta", path: "/p/zeta/tasks", counts: { ready: 1 }, label: "zeta", category: "Tools" },
  ];
  const tasks = [
    task({ queue: "muster", id: "0001", waitingForOwner: true }),
    task({ queue: "beta", id: "0001", stage: "building", claimedBy: "beta/wt-a", live: true }),
    task({ queue: "zeta", id: "0001" }),
    task({ queue: "alpha", id: "0001" }),
  ];
  const edits: ProjectEdits = { arrange: async () => {}, renameCategory: async () => {}, colour: async () => {}, fold: async () => {} };
  const render = (mode: DisplayMode = "adhd", current = "alpha") =>
    renderToStaticMarkup(createElement(Mode.Provider, { value: mode }, createElement(ProjectList, { queues, tasks, notes: [], activity: [], now: NOW, current, onPick: () => {}, onLabel: async () => {}, edit: edits })));
  const rows = (html: string) => [...html.matchAll(/<span class="side-name"[^>]*>([^<]*)<\/span>/g)].map((m) => m[1]);
  const heading = (html: string, name: string) => {
    const at = html.indexOf(`data-heading="${name}"`);
    return html.slice(html.lastIndexOf("<div", at), html.indexOf("</div>", at));
  };

  afterEach(() => vi.unstubAllGlobals());

  /** A heading's labelled counts, as read: "1 needs you · 3 projects · 1 working". */
  const counts = (h: string) => plain(/<span class="side-cat-counts"[^>]*>([\s\S]*)$/.exec(h.split("</div>")[0])?.[1] ?? "").replace(/\u00a0/g, " ");
  const countsOf = (h: string) => /<span class="side-cat-counts"[\s\S]*$/.exec(h)?.[0] ?? "";
  it("folds to one line: its chevron, its name and its labelled counts, what needs you first (0082)", () => {
    const html = render();
    const apps = heading(html, "Apps");
    expect(apps).toMatch(/^<div class="side-cat folded" data-heading="Apps"/);
    expect(apps).toMatch(/<button type="button" aria-expanded="false" aria-label="Unfold Apps" title="Unfold Apps \(→\)" class="ui-pill side-cat-fold"><svg class="side-cat-chevron" viewBox="0 0 16 16" aria-hidden="true" focusable="false"><path d="M6 3.5 10.5 8 6 12.5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><\/path><\/svg><\/button><span class="side-cat-text"><span class="side-cat-name" title="Apps">Apps<\/span><span class="side-cat-counts"/);
    expect(counts(apps)).toBe("1 needs you · 3 projects · 1 working");
    // The counts in words for a screen reader, on the heading itself; all of them, and its tasks, in the tooltip.
    expect(apps).toContain('aria-label="Apps, category heading, folded: 1 needs you, 3 projects, 1 working; ↩ renames it, ⌥↑ ⌥↓ move it, ← → fold it"');
    expect(apps).toContain('title="1 needs you · 3 projects · 1 working · 3 tasks"');
    // What needs you is the same chip a project's row has; working the same dot.
    expect(apps).toContain('<span class="side-cat-needs"><span class="count warn">1</span> <span class="side-cat-needs-words">needs you</span></span>');
    expect(apps).toContain('<span class="side-cat-rest"><span class="side-cat-part">\u00a0· 3 projects</span><span class="side-cat-part side-working">\u00a0· <i class="dot" aria-hidden="true"></i>1 working</span></span>');
    // Its rows are folded away, and nothing of them shows under it: the heading's count carries it.
    expect(rows(html)).toEqual(["All", "zeta"]);
    expect(html).not.toContain("side-peek");
    expect(heading(html, "Tools")).toMatch(/aria-expanded="true" aria-label="Fold Tools"[^>]*><svg class="side-cat-chevron"/);
    // One drawn chevron, turned by CSS: no text glyph (▸ ▾) left, which was tiny and faint.
    expect(html).not.toMatch(/[▸▾]/);
  });
  it("says its counts the same way open and folded (0082 AC2)", () => {
    const open = renderToStaticMarkup(createElement(ProjectList, { queues: queues.map((q) => ({ ...q, categoryFolded: false })), tasks, notes: [], activity: [], now: NOW, current: "alpha", onPick: () => {}, edit: edits }));
    expect(countsOf(heading(open, "Apps"))).toBe(countsOf(heading(render(), "Apps")));
    expect(counts(heading(open, "Tools"))).toBe("1 project");
  });
  it("says its keys: ← folds, → unfolds, as well as moving and renaming", () => {
    expect(heading(render(), "Apps")).toContain('aria-keyshortcuts="ArrowLeft ArrowRight Alt+ArrowUp Alt+ArrowDown"');
    expect(heading(render(), "Apps")).toContain('role="heading" aria-level="2"');
  });
  for (const mode of ["adhd", "standard", "focus"] as const) {
    for (const chip of ["working", "az", "needs", "mine"]) {
      it(`holds in ${mode} under ${chip}, one line with its counts`, () => {
        vi.stubGlobal("localStorage", { getItem: () => chip });
        const html = render(mode);
        expect(rows(html)).toEqual(["All", "zeta"]);
        expect(counts(heading(html, "Apps"))).toBe("1 needs you · 3 projects · 1 working");
      });
    }
  }
  it("without a way to save, nothing folds", () => {
    const html = renderToStaticMarkup(createElement(ProjectList, { queues, tasks, notes: [], activity: [], now: NOW, current: "all", onPick: () => {} }));
    expect(rows(html).sort()).toEqual(["All", "Alpha app", "Muster", "beta", "zeta"]);
    expect(html).not.toContain("side-cat-fold");
  });
  it("draws the persianUtils group renamed DashAfzar as the core sends it: one heading, its projects under it (AC2)", () => {
    // The state arrange.rs's test holds the core to (dashafzar-queues.json): colors and dates by
    // their group's name alone, drover and muster by the category they had by hand.
    const html = renderToStaticMarkup(createElement(ProjectList, { queues: dashafzar as QueueInfo[], tasks: [], notes: [], activity: [], now: NOW, current: "all", onPick: () => {}, edit: edits }));
    const groups = [...html.matchAll(/<div class="side-group[^"]*" role="group" aria-label="([^"]*)">([\s\S]*?)(?=<div class="side-group|<button type="button" title="A new heading)/g)].map((m) => [m[1], rows(m[2]).sort()]);
    // In the owner's heading order the fixture has: Tools, then DashAfzar (0073).
    expect(groups).toEqual([
      ["Tools", ["alpha"]],
      ["DashAfzar", ["colors", "dates", "drover", "muster"]],
    ]);
    // The folder keeps its name (in the rows' tooltips); no heading has it any more.
    expect(html).not.toContain('data-heading="persianUtils"');
  });
  it("counts a project whose only needs-you is lessons waiting on its folded heading (0070 review R2)", () => {
    const withLessons = queues.map((q) => (q.name === "beta" ? { ...q, lessonsWaiting: 2 } : q));
    const html = renderToStaticMarkup(createElement(ProjectList, { queues: withLessons, tasks, notes: [], activity: [], now: NOW, current: "zeta", onPick: () => {}, edit: edits }));
    expect(counts(heading(html, "Apps"))).toBe("3 need you · 3 projects · 1 working");
  });
  it("17 projects that all need you fold to the heading alone, one line (0082 AC3)", () => {
    const many: QueueInfo[] = Array.from({ length: 17 }, (_, i) => ({ name: `p${i}`, path: `/p/p${i}/tasks`, counts: { ready: 1 }, label: `Project ${i}`, category: "Dast Afzar", categoryFolded: true }));
    const waiting = many.flatMap((q) => [task({ queue: q.name, id: "0001", waitingForOwner: true }), task({ queue: q.name, id: "0002", waitingForOwner: true })]);
    const html = renderToStaticMarkup(createElement(ProjectList, { queues: many, tasks: waiting, notes: [], activity: [], now: NOW, current: "p3", onPick: () => {}, edit: edits }));
    const group = /<div class="side-group" role="group" aria-label="Dast Afzar">([\s\S]*?)<\/div><\/div>(?=<button)/.exec(html)?.[1] ?? "";
    // The group is its heading and nothing else: no row, no chip, not even the selected project's.
    expect(group).toMatch(/^<div class="side-cat folded" data-heading="Dast Afzar"/);
    expect(group.match(/<div/g)).toHaveLength(1);
    expect(group).not.toMatch(/side-row|side-item|side-peek/);
    expect(counts(group + "</div>")).toBe("34 need you · 17 projects");
    // And the heading is its line: what needs you never shrinks, and its counts after it are whole
    // or wrap out of sight (the widths are tested in "a heading's line").
    const rule = (sel: string) => [...css.matchAll(new RegExp(`(?:^|\\n)${sel.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")} \\{([^}]*)\\}`, "g"))].map((m) => m[1]).join(";");
    expect(rule(".side-cat-needs")).toContain("flex: none");
    expect(rule(".side-cat-rest")).toMatch(/flex-wrap: wrap;.*height: 16px;.*overflow: hidden/);
  });
});
