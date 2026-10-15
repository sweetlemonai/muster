import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import appSource from "./App.tsx?raw";
import css from "./styles.css?raw";
import dashafzar from "./dashafzar-queues.json";
import listSource from "./ProjectList.tsx?raw";
import projectsSource from "./Projects.tsx?raw";
import sidebarSource from "./SidebarProjects.tsx?raw";
import ProjectList, { ActivityLine, labelToSave, OrderChips, ProjectItem, type ProjectEdits, SideLabelEdit, SumsLine } from "./ProjectList";
import { ColourDot, Swatches } from "./ProjectSwatches";
import { cleanHeading, foldKey, HeadingEdit, HeadingFrame, NewCategory, SideRow, stepOf } from "./SidebarProjects";
import { type DisplayMode, Mode } from "./modes";
import { SIDEBAR, SIDEBAR_MAX, SIDEBAR_MIN, SIDEBAR_NARROW } from "./paneSize";
import { setLocale } from "./i18n/runtime";
import { PAUSE_GLYPH } from "./Pause";
import type { ProjectActivity, Sums } from "./activity";
import type { Activity, QueueInfo, Task } from "./types";

const task = (over: Partial<Task>): Task => ({
  queue: "drover", id: "0001", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "todo/0001-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", search: "", ...over,
});

const secs = (h: number, m: number) => new Date(2026, 9, 13, h, m).getTime() / 1000;
const NOW = secs(14, 0);
const at = (h: number, m: number) => `2026-10-13 ${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;

const line = (a: ProjectActivity) => renderToStaticMarkup(createElement(ActivityLine, { a, now: NOW }));
const sums = (s: Sums) => renderToStaticMarkup(createElement(SumsLine, { s, now: NOW }));
const state = (html: string) =>
  /<span class="side-state[^"]*">([\s\S]*?)<\/span><span class="side-when"/.exec(html)?.[1] ?? /<span class="side-activity side-sums-line">([\s\S]*?)<span class="side-when"/.exec(html)?.[1];
const when = (html: string) => /<span class="side-when"[^>]*>([^<]*)<\/span>/.exec(html)?.[1];
// The words as one line shows them: All's " · " at the end of "in N projects" is the second
// line's (styles.css hides it on one line, round 3).
const plain = (html: string) => html.replace(/<span class="side-sum-sep"> · <\/span>(?=<\/span><span class="side-sum-tail">)/g, "").replace(/<[^>]+>/g, "");

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
  it("says the whole line, state and when, in both pieces' tooltips, since the when may give way (mu-197)", () => {
    const whole = "8 tasks being worked on now: a live session, or a check-in or heartbeat in the last 30 minutes\nLast activity: 4 min ago";
    expect([...line({ working: 8, stalled: false, last: 4 }).matchAll(/title="([^"]*)"/g)].map((m) => m[1])).toEqual([whole, whole]);
    expect([...line({ working: 0, stalled: false, last: null }).matchAll(/title="([^"]*)"/g)].map((m) => m[1])).toEqual(["Nothing says when this was last worked on"]);
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
  it("says the tasks being worked on and, in words, the projects they're in, and the projects stalled (mu-197)", () => {
    const html = sums({ working: 29, projects: 9, stalled: 2, last: 0 });
    expect(plain(state(html) ?? "")).toBe("29 working in 9 projects · 2 projects stalled");
    expect(html).toContain('title="Tasks being worked on now: 29 working in 9 projects. Working: a live session');
    expect(html).toContain("\n2 projects stalled: work in Building");
    expect(html).toContain('<i class="dot" aria-hidden="true">');
  });
  it("puts \"N working\" first, then the rest: \"in N projects\", then the tail of the stalled count and the when (round 2)", () => {
    const html = sums({ working: 29, projects: 9, stalled: 2, last: 0 });
    expect(html).toMatch(
      /^<span class="side-sum-box"><span class="side-activity side-sums-line"><span class="side-working side-sum-working" title="[^"]+"><i class="dot" aria-hidden="true"><\/i>29 working<\/span><span class="side-sum-rest has-stalled"><span class="side-sum-in" title="[^"]+"> in 9 projects<span class="side-sum-sep"> · <\/span><\/span><span class="side-sum-tail"><span class="side-sum-stalled"><span class="side-sum-sep"> · <\/span><span class="side-stalled" title="[^"]+">2 projects stalled<\/span><\/span><span class="side-when" title="[^"]+">now<\/span><\/span><\/span><\/span><\/span>$/,
    );
    // Stalled alone has nothing to follow, and no "in N projects".
    expect(sums({ working: 0, projects: 0, stalled: 2, last: 0 })).toMatch(/^<span class="side-sum-box"><span class="side-activity side-sums-line"><span class="side-sum-rest has-stalled"><span class="side-sum-tail"><span class="side-sum-stalled"><span class="side-stalled"/);
    // Nothing stalled: no least width of its own, and no separator either side.
    expect(sums({ working: 29, projects: 9, stalled: 0, last: 0 })).toContain('<span class="side-sum-rest"><span class="side-sum-in" title="Tasks being worked on now: 29 working in 9 projects. Working: a live session, or a check-in or heartbeat in the last 30 minutes\nLast activity: now"> in 9 projects</span><span class="side-sum-tail"><span class="side-when"');
    expect(sums({ working: 29, projects: 9, stalled: 0, last: 0 })).not.toContain("side-sum-sep");
    expect(sums({ working: 0, projects: 0, stalled: 0, last: 0 })).toBe('<span class="side-sum-box"><span class="side-activity side-sums-line"><span class="side-sum-rest"><span class="side-sum-tail"><span class="side-when" title="Last activity: now">now</span></span></span></span></span>');
  });
  it("in Persian too: the same pieces, in the sentence's order", async () => {
    await setLocale("fa");
    try {
      const html = sums({ working: 29, projects: 9, stalled: 2, last: 0 });
      expect(plain(state(html) ?? "")).toBe("۲۹ در حال کار در ۹ پروژه · ۲ پروژه راکد");
      expect(html).toMatch(/<\/i>۲۹ در حال کار<\/span><span class="side-sum-rest has-stalled"><span class="side-sum-in" title="[^"]+"> در ۹ پروژه<span class="side-sum-sep"> · <\/span><\/span><span class="side-sum-tail"><span class="side-sum-stalled"><span class="side-sum-sep"> · <\/span>/);
    } finally {
      await setLocale("en");
    }
  });
  it("says the whole line in every piece's tooltip, the when's too, since any piece but \"N working\" may give way", () => {
    const html = sums({ working: 29, projects: 9, stalled: 2, last: 12 });
    const whole = [
      "Tasks being worked on now: 29 working in 9 projects. Working: a live session, or a check-in or heartbeat in the last 30 minutes",
      "2 projects stalled: work in Building, but nothing for over 30 minutes: no check-in, heartbeat, stage change, commit or file change",
      "Last activity: 12 min ago",
    ].join("\n");
    const titles = [...html.matchAll(/title="([^"]*)"/g)].map((m) => m[1]);
    expect(titles).toEqual([whole, whole, whole, whole]);
    // Stalled alone, and nothing at all: what there is, then when.
    expect([...sums({ working: 0, projects: 0, stalled: 1, last: null }).matchAll(/title="([^"]*)"/g)].map((m) => m[1])).toEqual(Array(2).fill("1 project stalled: work in Building, but nothing for over 30 minutes: no check-in, heartbeat, stage change, commit or file change\nNothing says when this was last worked on"));
    expect(/class="side-when" title="([^"]*)"/.exec(sums({ working: 0, projects: 0, stalled: 0, last: 0 }))?.[1]).toBe("Last activity: now");
  });
  it("says one task in one project in the singular", () => {
    const html = sums({ working: 1, projects: 1, stalled: 1, last: 0 });
    expect(plain(state(html) ?? "")).toBe("1 working in 1 project · 1 project stalled");
    expect(html).toContain("\n1 project stalled: ");
  });
  it("leaves out what's zero", () => {
    expect(plain(state(sums({ working: 1, projects: 1, stalled: 0, last: 2 })) ?? "")).toBe("1 working in 1 project");
    expect(sums({ working: 1, projects: 1, stalled: 0, last: 2 })).not.toContain("side-stalled");
    const stalled = sums({ working: 0, projects: 0, stalled: 2, last: 40 });
    expect(plain(state(stalled) ?? "")).toBe("2 projects stalled");
    expect(stalled).not.toContain('class="dot"');
    expect(plain(state(sums({ working: 0, projects: 0, stalled: 0, last: 1500 })) ?? "-")).toBe("");
    expect(when(sums({ working: 0, projects: 0, stalled: 0, last: 1500 }))).toBe("yesterday");
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
    expect(row(html, "alpha")).toContain('<span class="count warn" title="1 needs you">1</span>');
    expect(row(html, "zeta")).toContain('<span class="side-doing" title="1 in progress (Building or Testing)">1</span>');
    expect(row(html, "zeta")).toContain("<small>2</small>");
  });
  it("sums the tasks in the All row, and says the projects they're in (mu-197)", () => {
    const all = row(render(), "All");
    expect(plain(all)).toContain("2 working in 1 project · 1 project stalled");
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
    expect(html).toContain('<span class="side-cat-name" title="Tools">Tools</span><span class="side-cat-counts"');
  });
  it("sorts by the order chips within each heading: A–Z goes by the label", () => {
    vi.stubGlobal("localStorage", { getItem: () => "az" });
    expect(rows(render())).toEqual(["All", "Alpha app", "Muster", "zeta", "beta"]);
  });
  it("names the real project and its folder in the row's tooltip, and offers the label on a double click", () => {
    const html = render();
    const muster = html.split('<button class="side-item').find((r) => r.includes('side-name" title=')  && r.includes(">Muster<")) ?? "";
    expect(muster).toContain('title="Muster\nmuster · /p/muster/tasks"');
    // The name keeps the row's tooltip, where the pointer is (review): the label in full (0087), the
    // real name, then the double click.
    expect(muster).toContain('<span class="side-name" title="Muster\nmuster · /p/muster/tasks\nDouble-click to change the label">Muster</span>');
  });
  it("keeps the headings and the same counts in Focus's narrow sidebar (0082: the same in every mode)", () => {
    const html = render("focus");
    expect(heads(html)).toEqual(["Apps 1 needs you · 1 project", "Tools 1 needs you · 2 projects · 2 working", "Other 1 project"]);
    expect(rows(html)).toEqual(["All", "Alpha app", "zeta", "Muster", "beta"]);
  });
});

describe("one unit for \"N working\": tasks, on All, the headings and the rows (mu-197)", () => {
  // Three projects under one heading, with 8, 8 and 0 tasks being worked on.
  const names = ["harbor", "lantern", "quill"];
  const queues: QueueInfo[] = names.map((name): QueueInfo => ({ name, path: `/p/${name}/tasks`, counts: name === "quill" ? { ready: 1 } : { building: 8 }, label: name, category: "Apps" }));
  const tasks = [
    ...Array.from({ length: 8 }, (_, i) => task({ queue: "harbor", id: `00${10 + i}`, stage: "building", live: true })),
    ...Array.from({ length: 8 }, (_, i) => task({ queue: "lantern", id: `00${10 + i}`, stage: "building", live: true })),
    task({ queue: "quill", id: "0001" }),
  ];
  const render = (mode: DisplayMode) =>
    renderToStaticMarkup(createElement(Mode.Provider, { value: mode }, createElement(ProjectList, { queues, tasks, notes: [], activity: [], now: NOW, current: "all", onPick: () => {}, onLabel: async () => {} })));
  const row = (html: string, name: string) => html.split('<button class="side-item').find((r) => new RegExp(`side-name"[^>]*>${name}<`).test(r)) ?? "";
  const label = (r: string) => /aria-label="([^"]*)"/.exec(r)?.[1];
  const titles = (r: string) => [...r.matchAll(/title="([^"]*)"/g)].map((m) => m[1]);

  it("All says 16 working in 2 projects, the heading 16 working, each row 8 working", () => {
    const html = render("adhd");
    expect(plain(state(row(html, "All")) ?? "")).toBe("16 working in 2 projects");
    expect(plain(state(row(html, "harbor")) ?? "")).toBe("8 working");
    expect(plain(state(row(html, "lantern")) ?? "")).toBe("8 working");
    expect(state(row(html, "quill"))).toBe("");
    const head = /<div class="side-cat"[^>]*>([\s\S]*?)<\/div>/.exec(html)?.[1] ?? "";
    expect(plain(head.replace(/></g, "> <")).replace(/\s+/g, " ").trim()).toBe("Apps 3 projects · 16 working");
  });
  it("says the same unit in their tooltips and aria-labels", () => {
    const html = render("adhd");
    expect(titles(row(html, "All"))).toContain("Tasks being worked on now: 16 working in 2 projects. Working: a live session, or a check-in or heartbeat in the last 30 minutes\nLast activity: now");
    expect(titles(row(html, "harbor"))).toContain("8 tasks being worked on now: a live session, or a check-in or heartbeat in the last 30 minutes\nLast activity: now");
    expect(html).toContain('title="3 projects · 16 working · 17 tasks"');
    expect(html).toMatch(/aria-label="Apps, category heading: 3 projects, 16 working"/);
    // Focus shows the dot, not the line: the aria-label says the line's words.
    const focus = render("focus");
    expect(label(row(focus, "All"))).toBe("All: 16 in progress, 17 tasks; 16 working in 2 projects");
    expect(label(row(focus, "harbor"))).toBe("harbor: 8 in progress, 8 tasks; 8 working");
    expect(label(row(focus, "quill"))).toBe("quill: 1 task");
  });
  const decls = (body: string) => new Map(body.split(";").map((d) => d.trim()).filter(Boolean).map((d) => [d.slice(0, d.indexOf(":")).trim(), d.slice(d.indexOf(":") + 1).trim()]));
  const esc = (sel: string) => sel.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  // A top-level rule's declarations (a rule inside an @container block is indented, so not this).
  const rule = (sel: string) => decls(new RegExp(`(?:^|\\n)${esc(sel)} \\{([^}]*)\\}`).exec(css)?.[1] ?? "");
  // An @container block's rules, each selector to its declarations; and every block of that name.
  const container = (name: string) => {
    const blocks = [...css.matchAll(new RegExp(`@container ${name} \\(([^)]*)\\) \\{\\n((?:  [^\\n]*\\n)*)\\}`, "g"))];
    return blocks.map((b) => ({ when: b[1], rules: Object.fromEntries([...b[2].matchAll(/^ {2}(\S[^{]*?) \{([^}]*)\}$/gm)].map((r) => [r[1], Object.fromEntries(decls(r[2]))])) }));
  };
  it("keeps \"N working\" and the stalled count whole; the when gives way first, then \"in N projects\", and a piece gone gives its room back (styles.css, round 2)", () => {
    const get = (sel: string, ...props: string[]) => props.map((p) => rule(sel).get(p));
    expect(rule(".side-state").get("min-width")).toBe("min-content");
    // A row's when gives way whole, never "8 worki…".
    expect(get(".side-when", "flex", "flex-wrap", "overflow")).toEqual(["0 1 auto", "wrap", "hidden"]);
    expect(rule(".side-state").has("text-overflow")).toBe(false);
    // All's line wraps only where "N working" and the stalled count can't sit side by side: the
    // rest's least width is the stalled count's, none without one.
    expect(get(".side-sums-line", "flex-wrap", "gap")).toEqual(["wrap", "0"]);
    expect(rule(".side-sum-working").get("flex")).toBe("none");
    // The rest shows its last line: "in N projects" wraps onto the first, out of sight, when it
    // and the tail's least width (the stalled count) don't both fit. Each rule whole (round 3), so
    // an added declaration (overflow, a max-width) that could clip the stalled count fails here.
    expect(Object.fromEntries(rule(".side-sum-rest"))).toEqual({ flex: "1 1 0", "min-width": "0", display: "flex", "flex-wrap": "wrap", "align-content": "flex-end", height: "1.35em", overflow: "hidden" });
    expect(Object.fromEntries(rule(".side-sum-rest.has-stalled"))).toEqual({ "min-width": "min-content" });
    expect(Object.fromEntries(rule(".side-sum-in"))).toEqual({ flex: "none", "white-space": "pre" });
    expect(Object.fromEntries(rule(".side-sum-tail"))).toEqual({ flex: "1 1 0", "min-width": "min-content", display: "flex" });
    expect(Object.fromEntries(rule(".side-sum-stalled"))).toEqual({ flex: "none", "white-space": "pre" });
    expect(Object.fromEntries(rule(".side-sum-working"))).toEqual({ flex: "none" });
    // Nothing else styles the stalled count's pieces: no other rule names them.
    expect(css.match(/[^\n]*\.side-sum-(tail|stalled)\b[^{\n]*\{/g)).toEqual([".side-sum-tail {", ".side-sum-stalled {", ".side-sum-tail > .side-when {", ".side-sum-tail > .side-when::before {", "  .side-sum-stalled > .side-sum-sep {"]);
    // The when takes no least width (so it never pushes "in N projects" out), grows into what the
    // tail has left up to its own width, and wraps out of sight on its own line where it's short.
    expect(get(".side-sum-tail > .side-when", "flex", "width", "max-width", "margin-inline-start")).toEqual(["1 1 0", "0", "max-content", "auto"]);
    expect(rule(".side-sum-tail > .side-when").has("padding-inline-start")).toBe(false);
    expect(rule(".side-sum-tail > .side-when::before").get("width")).toBe("6px");
    expect(get(".side-when::before", "content", "flex", "height")).toEqual(['""', "none", "100%"]);
    expect(rule(".side-activity").get("line-height")).toBe("1.35");
    // Nothing of round 1's grow-and-hide is left: no piece takes room it can't show in.
    expect(css).not.toMatch(/\.side-sum-part|\.side-sums \{|flex-grow: 1000/);
  });
  it("takes as many lines as the sidebar's width says, never the counts, and no line starts with \" · \" (styles.css, round 3)", () => {
    // All's box is the sidebar less 35px: under 195px (a sidebar under 230px) the rest has its own
    // second line, stall or not, and the line keeps two lines' height.
    expect(Object.fromEntries(rule(".side-sum-box"))).toEqual({ display: "block", container: "sums / inline-size" });
    expect(container("sums")).toEqual([
      {
        when: "width < 195px",
        rules: {
          ".side-sums-line": { "min-height": "2.7em", "align-content": "flex-start" },
          ".side-sum-rest": { flex: "1 0 100%" },
          ".side-sum-in > .side-sum-sep": { display: "inline" },
          ".side-sum-stalled > .side-sum-sep": { display: "none" },
        },
      },
    ]);
    // Two lines of the line's own height (line-height 1.35), whatever is on them.
    expect(rule(".side-activity").get("line-height")).toBe("1.35");
    // On one line the separator is the tail's; "in N projects"'s own is hidden.
    expect(Object.fromEntries(rule(".side-sum-in > .side-sum-sep"))).toEqual({ display: "none" });
    expect(rule(".side-sum-stalled > .side-sum-sep").size).toBe(0);
    // Focus: All's badges take the line under the name in a sidebar under 200px, as tall as "2 stalled".
    expect(Object.fromEntries(rule(".side-all-focus"))).toEqual({ container: "side-all / inline-size" });
    expect(container("side-all")).toEqual([{ when: "width < 165px", rules: { ".side-all-focus .side-badges": { flex: "0 0 100%", "line-height": "1.45", "min-height": "1.45em" } } }]);
  });
  it("on All in Focus, the badge says the short \"1 stalled\", its tooltip and aria-label the projects (round 3)", () => {
    const stalled = [task({ queue: "quill", id: "0002", stage: "building", claimedBy: "quill/wt-q", heartbeat: at(9, 0), modified: secs(9, 0) })];
    const list = (mode: DisplayMode) => renderToStaticMarkup(createElement(Mode.Provider, { value: mode }, createElement(ProjectList, { queues, tasks: [...tasks, ...stalled], notes: [], activity: [], now: NOW, current: "all", onPick: () => {}, onLabel: async () => {} })));
    const focus = list("focus");
    expect(row(focus, "All")).toContain('<span class="side-stalled" title="1 project stalled: work in Building, but nothing for over 30 minutes: no check-in, heartbeat, stage change, commit or file change">1 stalled</span>');
    expect(label(row(focus, "All"))).toBe("All: 17 in progress, 18 tasks; 16 working in 2 projects, 1 project stalled");
    expect(row(focus, "All")).toMatch(/^ on side-all-focus"/);
    // A project's own badge is unchanged: the word, with the stalled hint.
    expect(row(focus, "quill")).toContain('<span class="side-stalled" title="Stalled: work in Building');
    expect(row(focus, "quill")).toContain(">stalled</span>");
    expect(focus.match(/side-all-focus/g)).toHaveLength(1);
    // The full look has no badge for it and no container: the line says it.
    expect(list("adhd")).not.toContain("side-all-focus");
    expect(row(list("adhd"), "All")).not.toContain(">1 stalled<");
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
    // Every heading wraps now (mu-212), the new one with it.
    expect(css).toMatch(/\n\.side-cat \{[^}]*flex-wrap: wrap;/);
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
    expect([foldKey(ev("ArrowLeft"), false, false), foldKey(ev("ArrowRight"), true, false)]).toEqual([true, false]);
    expect([foldKey(ev("ArrowLeft"), true, false), foldKey(ev("ArrowRight"), false, false)]).toEqual([null, null]);
    expect([foldKey(ev("ArrowLeft", true), false, false), foldKey(ev("ArrowUp"), false, false), foldKey(ev("Enter"), true, false)]).toEqual([null, null, null]);
  });
  it("right to left the arrows follow the reading direction: → folds, ← unfolds (mu-149)", () => {
    const ev = (key: string, altKey = false) => ({ key, altKey, metaKey: false, ctrlKey: false });
    expect([foldKey(ev("ArrowRight"), false, true), foldKey(ev("ArrowLeft"), true, true)]).toEqual([true, false]);
    expect([foldKey(ev("ArrowRight"), true, true), foldKey(ev("ArrowLeft"), false, true)]).toEqual([null, null]);
    expect([foldKey(ev("ArrowRight", true), false, true), foldKey(ev("ArrowUp"), false, true), foldKey(ev("Enter"), true, true)]).toEqual([null, null, null]);
  });
  it("a heading's keys follow the language shown: → folds it in فارسی, ← in English", async () => {
    const press = (key: string) => {
      const folds: boolean[] = [];
      const el = HeadingFrame({ name: "Tools", count: 2, onFold: (f) => folds.push(f), children: null }) as { props: { onKeyDown: (e: unknown) => void } };
      el.props.onKeyDown({ key, altKey: false, metaKey: false, ctrlKey: false, target: null, currentTarget: {}, preventDefault: () => {} });
      return folds;
    };
    expect([press("ArrowLeft"), press("ArrowRight")]).toEqual([[true], []]);
    const chevron = () => renderToStaticMarkup(createElement(HeadingFrame, { name: "Tools", count: 2, onFold: () => {}, children: null })).match(/title="([^"]*)" class="ui-pill side-cat-fold"/)?.[1];
    expect(chevron()).toBe("Fold Tools (←)");
    await setLocale("fa");
    try {
      expect([press("ArrowRight"), press("ArrowLeft")]).toEqual([[true], []]);
      // Its chevron's tooltip names the key that folds it there.
      expect(chevron()).toMatch(/\(→\)$/);
      expect(renderToStaticMarkup(createElement(HeadingFrame, { name: "Tools", count: 2, folded: true, onFold: () => {}, children: null }))).toMatch(/title="[^"]*\(←\)" class="ui-pill side-cat-fold"/);
    } finally {
      await setLocale("en");
    }
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
  // The widths, from styles.css: a heading's line = sidebar − 1 border − 16 nav padding − its two
  // inline margins; the name and counts get what the chevron and its gap leave.
  const fold = px(all(".side-cat-fold.ui-pill").get("min-width"));
  const gap = px(all(".side-cat").get("gap")?.split(" ")[1]);
  const [rowGap, textGap] = (all(".side-cat").get("gap") ?? "").split(" ").map(px);
  const nameLeast = em(all(".side-cat-name").get("flex")) * px(all(".side-cat-name").get("font-size"));
  const margin = px(all(".side-cat").get("margin")?.split(" ")[1]);
  const line = (sidebar: number) => sidebar - 1 - 16 - 2 * margin;
  const box = (sidebar: number) => line(sidebar) - fold - gap;
  // "N need you": the chip (a digit about 0.62em of its 11px) and the words (about 0.55em a
  // character), measured in headless Chrome at 73px for 17 and 80px for 120.
  const count = all(".count");
  const chipFont = px(count.get("font-size"));
  const needs = (digits: number) =>
    Math.max(px(count.get("min-width")), digits * 0.62 * chipFont + 2 * px(count.get("padding")?.split(" ")[1])) + 0.3 * chipFont + 8 * 0.55 * chipFont;

  it("keeps \"N need you\" whole: never cut, never in \"…\", its words wrapping only between words and only on their own line (mu-212)", () => {
    // The chip never shrinks; the needs piece shrinks only below its one line, down to the chip and
    // its longest word (no min-width: 0), so a word is never cut.
    expect(all(".side-cat-needs .count.warn").get("flex")).toBe("none");
    const needsPiece = all(".side-cat-needs");
    expect(needsPiece.get("flex")).toBe("0 1 auto");
    expect(needsPiece.get("align-items")).toBe("baseline");
    for (const sel of [".side-cat-needs", ".side-cat-needs-words", ".side-cat-counts"]) {
      const d = all(sel);
      expect(d.has("text-overflow") || d.has("min-width") || d.has("word-break") || d.has("overflow-wrap") || d.get("overflow") === "hidden", sel).toBe(false);
    }
    // The words wrap between words, the rest of the counts never.
    expect(all(".side-cat-needs-words").get("white-space")).toBe("normal");
    // The counts ask for what needs you on one line (their max-content: the rest after it adds
    // nothing to it), so where that doesn't fit beside the name they take their own line before
    // the words wrap; only on that line do they give way, down to the chip and its longest word.
    const counts = all(".side-cat-counts");
    expect([counts.get("flex"), counts.get("width")]).toEqual(["1 1 auto", "max-content"]);
    expect(counts.has("min-width")).toBe(false);
    expect(counts.get("white-space")).toBe("nowrap");
    expect(all(".side-cat-rest").get("contain")).toBe("inline-size");
    // No mode or width hides the words some other way (round 2 dropped them at 150px).
    expect(css).not.toMatch(/\.side-cat-needs(-words)? \{[^}]*display: none/);
    // No container query reaches the sidebar (the card's holder ribbon has its own, mu-171).
    for (const q of css.matchAll(/@container[^{]*\{([\s\S]*?)\n\}/g)) expect(q[1]).not.toContain("side-cat");
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
  it("is one line at the default 210px, a long name and a 3-digit count; a full-width second line only where that can't fit (150px)", () => {
    // The chevron, the name and the counts wrap in one box, the heading's (mu-212): the counts'
    // second line starts under the chevron, not only under the name.
    expect(all(".side-cat").get("flex-wrap")).toBe("wrap");
    expect(rowGap).toBe(2);
    expect(css).not.toMatch(/\.side-cat-text\b/);
    // 210: the name's least and "120 need you" fit beside each other.
    expect(nameLeast + textGap + needs(3)).toBeLessThanOrEqual(box(210));
    // 150: they don't, so the counts take the line under them, where even "1200 need you" fits.
    expect(nameLeast + textGap + needs(1)).toBeGreaterThan(box(150));
    expect(needs(4)).toBeLessThanOrEqual(line(150));
    // Persian's "۴۴ مورد به شما نیاز دارد", 118.2px in headless WebKit (mu-212; "۴" 113.0, "۴۴۴"
    // 124.9), fits that line whole with room to spare: 4px margins, not 8 (117px, so "۹" → "۱۰"
    // wrapped its words and grew the heading on a poll).
    expect(margin).toBe(4);
    expect(line(150) - 118.2).toBeGreaterThanOrEqual(4);
  });
  it("puts the chevron, the name and the counts side by side in the heading, so the counts' line runs its full width (mu-212)", () => {
    // 44 need you, 12 projects, 3 working.
    const names = Array.from({ length: 12 }, (_, i) => `p${i}`);
    const qs: QueueInfo[] = names.map((n) => ({ name: n, path: `/p/${n}/tasks`, counts: { ready: 4 }, label: n, category: "Apps" }));
    const ts = [
      ...Array.from({ length: 44 }, (_, i) => task({ queue: names[i % 12], id: String(i + 1).padStart(4, "0"), waitingForOwner: true })),
      ...Array.from({ length: 3 }, (_, i) => task({ queue: "p0", id: String(100 + i).padStart(4, "0"), stage: "building", live: true })),
    ];
    const html = renderToStaticMarkup(createElement(ProjectList, { queues: qs, tasks: ts, notes: [], activity: [], now: NOW, current: "all", onPick: () => {}, onLabel: async () => {}, edit: { arrange: async () => {}, renameCategory: async () => {}, colour: async () => {}, fold: async () => {} } }));
    const at = html.indexOf('data-heading="Apps"');
    const head = html.slice(html.lastIndexOf("<div", at), html.indexOf("</div>", at));
    expect(head).not.toContain("side-cat-text");
    expect(head).toMatch(/<\/svg><\/button><span class="side-cat-name" title="Apps">Apps<\/span><span class="side-cat-counts" title="44 need you · 12 projects · 3 working · \d+ tasks"><span class="side-cat-needs"><span class="count warn">44<\/span> <span class="side-cat-needs-words">need you<\/span><\/span><span class="side-cat-rest"><span class="side-cat-part">\u00a0· 12 projects<\/span><span class="side-cat-part side-working">\u00a0· <i class="dot" aria-hidden="true"><\/i>3 working<\/span><\/span><\/span>$/);
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

describe("a project's row holds at the narrowest sidebar (0087)", () => {
  const all = (sel: string) => {
    const esc = sel.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const out = new Map<string, string>();
    for (const m of css.matchAll(new RegExp(`(?:^|\\n)\\s*${esc} \\{([^}]*)\\}`, "g")))
      for (const d of m[1].split(";").map((x) => x.trim()).filter(Boolean)) out.set(d.slice(0, d.indexOf(":")).trim(), d.slice(d.indexOf(":") + 1).trim());
    return out;
  };
  const px = (v: string | undefined) => Number(/^(\d+(?:\.\d+)?)px$/.exec(v ?? "")?.[1] ?? NaN);
  const body = px(/font: (\d+px)\//.exec(css)?.[1]);
  // The row's line, from styles.css: sidebar − 1 border − 16 nav padding − 14 group indent − the
  // colour dot and its margin − the row's padding and border.
  const dot = all(".side-swatch-dot");
  const item = all(".side-item");
  const line = (sidebar: number) =>
    sidebar - 1 - 16 - px(all(".side-group > .side-row").get("margin-inline-start")) - px(dot.get("width")) - px(dot.get("margin-inline")?.split(" ")[0]) - 2 * px(item.get("padding")?.split(" ")[1]) - 2;
  const [rowGap, gap] = (all(".side-line").get("gap") ?? "").split(" ").map(px);
  const nameLeast = 4 * body;
  // drover's badges in ADHD: "17" needs you, "2" in progress, "‖ 1" paused and "48" tasks, at about
  // 0.62em a digit of their 11px, 5px apart.
  const count = all(".count");
  const chip = (digits: number) => Math.max(px(count.get("min-width")), digits * 0.62 * 11 + 2 * px(count.get("padding")?.split(" ")[1]));
  const badgeGap = px(all(".side-badges").get("gap"));
  const drover = chip(2) + badgeGap + 0.62 * 11 + badgeGap + 3 * 0.62 * 11 + badgeGap + 2 * 0.62 * 11;

  it("lets the name give way first, with an ellipsis, its words in their own direction from the dot", () => {
    const name = all(".side-name");
    expect([name.get("overflow"), name.get("text-overflow"), name.get("white-space"), name.get("unicode-bidi")]).toEqual(["hidden", "ellipsis", "nowrap", "plaintext"]);
    // A name starts beside its dot whatever its script, on the window's start side in either
    // direction (mu-149): match-parent, not `start`, which under plaintext follows the name's own
    // script and would put a Persian name at the far end in English, a Latin one in فارسی.
    expect(name.get("text-align")).toBe("match-parent");
  });
  it("never lets a name go under about 4em, or its own width if that's less, at any sidebar width", () => {
    const name = all(".side-name");
    // Its share of the line, when the line breaks, is 4em, capped at its own width ("All" doesn't
    // claim 4em); then it grows into what the badges leave, up to its own width.
    expect([name.get("flex"), name.get("width"), name.get("max-width"), name.get("min-width")]).toEqual(["1000 1 auto", "4em", "max-content", "0"]);
    // The same rule at every width: no sidebar-width switch (round 2's `data-tight` below 210 left
    // drover's name 0px wide in Focus from 210 to about 229px).
    expect(css).not.toContain("data-tight");
    expect(appSource).not.toContain("data-tight");
    expect(css).not.toMatch(/\.sidebar[^{]*\.side-(line|name)[^{]*\{/);
  });
  it("keeps every badge whole, the needs-you count and \"‖ N\" included, and moves them only as one group", () => {
    const badges = all(".side-badges");
    expect([badges.get("flex"), badges.get("margin-inline-start"), badges.get("max-width")]).toEqual(["none", "auto", "100%"]);
    const each = all(".side-badges > *");
    expect([each.get("flex"), each.get("white-space")]).toEqual(["none", "nowrap"]);
    // The line wraps, the badges after the name, the rows closer than the words beside each other.
    expect(all(".side-line").get("flex-wrap")).toBe("wrap");
    expect(rowGap).toBeLessThan(gap);
    expect(gap).toBe(8);
    expect(css).not.toMatch(/\.side-(badges|paused|doing)[^{]*\{[^}]*(text-overflow|display: none)/);
    expect(css).not.toMatch(/\.count\.warn[^{]*\{[^}]*(text-overflow|flex-shrink: [1-9])/);
  });
  it("puts Focus's \"stalled\" first in the badges' group, as far from them as the name is", () => {
    const html = renderToStaticMarkup(createElement(Mode.Provider, { value: "focus" }, createElement(ProjectItem, { name: "zeta", total: 1, tasks: [], on: false, onClick: () => {}, stalled: "stalled", running: true, children: null })));
    expect(html).toMatch(/<span class="side-name">zeta<\/span><span class="side-badges"><span class="side-stalled" title="Stalled: [^"]+">stalled<\/span><span class="run-chip"/);
    expect(px(all(".side-badges").get("gap")) + px(all(".side-badges > .side-stalled").get("margin-inline-end"))).toBe(gap);
  });
  it("is one line where the name's 4em and the badges fit, as before; drover's take the line under its name at 210 too", () => {
    // drover with its running chip ("running", about 0.55em a letter of its 10.5px, its padding and border).
    const chipCss = all(".run-chip");
    const running = 7 * 0.55 * px(chipCss.get("font-size")) + 2 * px(chipCss.get("padding")?.split(" ")[1]) + 2 + badgeGap;
    // The widest sidebar: drover's whole row is one line (ADHD's badges).
    expect(nameLeast + gap + running + drover).toBeLessThanOrEqual(line(SIDEBAR_MAX));
    // The default 210px: its name and badges don't fit, so the badges go under the name instead of
    // squeezing it to nothing (Focus's "stalled" in their place keeps that up to about 240px).
    expect(nameLeast + gap + running + drover).toBeGreaterThan(line(SIDEBAR));
    expect(running + drover).toBeLessThanOrEqual(line(SIDEBAR));
    // A row with one 2-digit count is one line at 210 and at 150, its name at least 4em.
    expect(nameLeast + gap + chip(2)).toBeLessThanOrEqual(line(SIDEBAR_MIN));
    // 150: drover's badges fit the line under the name, whole.
    expect(nameLeast + gap + drover).toBeGreaterThan(line(SIDEBAR_NARROW));
    expect(drover).toBeLessThanOrEqual(line(SIDEBAR_MIN));
  });
  it("wraps the check-in button's words inside the sidebar, never past its edge", () => {
    const button = all(".side-ping button");
    expect([button.get("white-space"), button.get("max-width"), button.get("text-align")]).toEqual(["normal", "100%", "start"]);
    // The board's and the session's buttons keep their one line.
    expect(all(".card-ping button, .session-ping button, .side-ping button").get("white-space")).toBe("nowrap");
  });

  const queues: QueueInfo[] = [
    { name: "drover", path: "/p/drover/tasks", counts: { building: 2, ready: 3 }, label: "Drover the long-named app", category: "Apps", running: true },
    { name: "calm", path: "/p/calm/tasks", counts: { ready: 1 } },
  ];
  const tasks = [
    task({ queue: "drover", id: "0001", waitingForOwner: true }),
    task({ queue: "drover", id: "0002", waitingForOwner: true }),
    task({ queue: "drover", id: "0003", stage: "building", claimedBy: "drover/wt-a", heartbeat: at(12, 0) }),
    task({ queue: "drover", id: "0004", stage: "building", paused: at(11, 0) }),
    task({ queue: "drover", id: "0005" }),
    task({ queue: "calm", id: "0001" }),
  ];
  const render = (mode: DisplayMode) =>
    renderToStaticMarkup(createElement(Mode.Provider, { value: mode }, createElement(ProjectList, { queues, tasks, notes: [], activity: [], now: NOW, current: "all", onPick: () => {} })));
  const button = (html: string, name: string) => /<button class="side-item[^"]*"[^>]*>/.exec(html.slice(html.lastIndexOf("<button", html.indexOf(`>${name}</span>`))))?.[0] ?? "";
  const badges = (html: string, name: string) => {
    const from = html.indexOf(`>${name}</span>`);
    return /<span class="side-badges">([\s\S]*?)<\/span><\/span><span id=/.exec(html.slice(from))?.[1] ?? "";
  };

  for (const mode of ["adhd", "focus"] as const) {
    it(`keeps every badge in the markup (${mode})`, () => {
      const html = render(mode);
      const b = badges(html, "Drover the long-named app");
      expect(b).toContain(">running</span>");
      expect(b).toContain('<span class="count warn" title="2 need you">2</span>');
      expect(b).toContain(`<span class="side-paused" title="1 paused">${PAUSE_GLYPH} 1</span>`);
      if (mode === "adhd") {
        expect(b).toContain('<span class="side-doing" title="1 in progress (Building or Testing)">1</span>');
        expect(b).toContain("<small>5</small>");
      }
    });
    it(`says the full name and every count in words on the row (${mode})`, () => {
      const html = render(mode);
      const said = mode === "focus" ? ", stalled" : "";
      expect(button(html, "Drover the long-named app")).toContain(`aria-label="Drover the long-named app: 2 need you, 1 in progress, 1 paused, 5 tasks; running${said}"`);
      expect(button(html, "calm")).toContain('aria-label="calm: 1 task"');
      expect(button(html, "All")).toContain(`aria-label="All: 2 need you, 1 in progress, 1 paused, 6 tasks${mode === "focus" ? "; 1 project stalled" : ""}"`);
    });
  }
  it("puts the label's full name in the row's tooltip, with the real project and its folder", () => {
    expect(button(render("adhd"), "Drover the long-named app")).toContain('title="Drover the long-named app\ndrover · /p/drover/tasks"');
    expect(button(render("adhd"), "calm")).toContain('title="calm · /p/calm/tasks"');
  });
  /** The text from the element with `id` in `html` to `end`, its tags dropped. */
  const byId = (html: string, id: string, end: string) => plain(new RegExp(`<span id="${id.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}"[^>]*>([\\s\\S]*?)${end}`).exec(html)?.[1] ?? "");
  /** The row's description: the tooltip's words (a span of text alone), then what's under its line. */
  const described = (html: string, name: string) => {
    const [where, more] = (/aria-describedby="([^"]+)"/.exec(button(html, name))?.[1] ?? "").split(" ");
    return [byId(html, where, "</span>"), byId(html, more, "</button>")];
  };
  it("still reads the real queue name and folder, then its activity line and its limit, as the row's description", () => {
    const html = render("adhd");
    const [where, more] = described(html, "Drover the long-named app");
    expect(where).toBe("drover · /p/drover/tasks");
    expect(more).toContain("stalled");
    expect(described(html, "calm")[0]).toBe("calm · /p/calm/tasks");
    // The tooltip's words, read out but not shown: the line under the name is the activity's.
    expect(html).toContain('hidden="">drover · /p/drover/tasks</span>');
    expect(all(".side-more:empty").get("display")).toBe("none");
  });
  it("has no tooltip description where the row has no tooltip", () => {
    const html = renderToStaticMarkup(createElement(Mode.Provider, { value: "adhd" }, createElement(ProjectItem, { name: "zeta", total: 1, tasks: [], on: false, onClick: () => {}, children: null })));
    expect(/aria-describedby="([^"]+)"/.exec(html)?.[1].split(" ")).toHaveLength(1);
    expect(html).not.toContain("hidden=");
  });

  const row = (mode: DisplayMode, over: { working?: string; tasks?: Task[] }) =>
    renderToStaticMarkup(createElement(Mode.Provider, { value: mode }, createElement(ProjectItem, { name: "zeta", total: 1, tasks: [], on: false, onClick: () => {}, children: null, ...over })));
  it("says the working count, in the activity line's words, where the row shows the working dot (Focus), and not where it doesn't (mu-197)", () => {
    const focus = row("focus", { working: "8 working" });
    expect(focus).toContain('<i class="dot side-dot"');
    expect(focus).toContain('aria-label="zeta: 1 task; 8 working"');
    const adhd = row("adhd", { working: "8 working" });
    expect(adhd).not.toContain("side-dot");
    expect(adhd).toContain('aria-label="zeta: 1 task"');
    expect(row("focus", { working: "" })).toContain('aria-label="zeta: 1 task"');
    expect(row("focus", { working: "" })).not.toContain("side-dot");
  });
  it("says \"1 needs you\", as the headings do, and \"2 need you\"", () => {
    const one = row("adhd", { tasks: [task({ queue: "zeta", id: "0001", waitingForOwner: true })] });
    expect(one).toContain('aria-label="zeta: 1 needs you, 1 task"');
    expect(one).toContain('<span class="count warn" title="1 needs you">1</span>');
    const two = row("adhd", { tasks: [task({ queue: "zeta", id: "0001", waitingForOwner: true }), task({ queue: "zeta", id: "0002", waitingForOwner: true })] });
    expect(two).toContain('aria-label="zeta: 2 need you, 1 task"');
    expect(two).toContain('<span class="count warn" title="2 need you">2</span>');
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
  // What each call does is tested in projectOrder.test.ts (runMove, foldHeading, readBack, mu-88).
  it("a move under a folded heading opens it at once and says both", () => {
    expect(listSource).toContain("void runMove(p, { folds, setPending, setFolds: setFoldSet, setFresh, say: setSaid, problem: setProblem }, () => edit.arrange(p.order, p.categories, p.moved));");
  });
  it("a fold goes through trackFold, and each read of the queues through readFolds", () => {
    expect(listSource).toContain("void foldHeading(name, folded, { setFolds: setFoldSet, say: setSaid, problem: setProblem }, () => edit.fold(name, folded));");
    expect(listSource).toContain("useEffect(() => readBack(core.current, coreFolds.current, setPending, setFoldSet), [queues]);");
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
    expect(listSource).toContain("useEffect(() => readBack(core.current, coreFolds.current, setPending, setFoldSet), [queues]);");
    // Over nothing that takes the drag, or off the sidebar: no drop line.
    expect(listSource).toContain("onDragOver={drag ? (e) => !e.isDefaultPrevented() && hover(null) : undefined}");
    expect(listSource).toMatch(/onDragLeave=\{drag \? \(e\) => !\(e\.relatedTarget instanceof Node && e\.currentTarget\.contains\(e\.relatedTarget\)\) && hover\(null\) : undefined\}/);
  });
  it("both keep a move's bookkeeping through track, never by hand (review R6)", () => {
    // runMove and placeSteps call track (projectOrder.test.ts, mu-88).
    expect(listSource).toContain("void runMove(p, { folds, setPending,");
    expect(projectsSource).toContain("placeSteps(pending, core, p.key, named, setPending, (plan) => api.arrangeProjects(plan.order, plan.categories, plan.moved))");
    for (const src of [listSource, projectsSource]) expect(src).not.toMatch(/setPending\(\{|setPending\(\(x\)/);
  });
  it("Settings plans each ↑ ↓ from the move it showed last", () => {
    expect(projectsSource).toContain("const layout = shownLayout(pending, core);");
    expect(projectsSource).toContain("useEffect(() => setPending((p) => settle(p, core)), [core]);");
    expect(projectsSource).toContain("const { up, down } = placeSteps(pending, core, p.key, named, setPending,");
    expect(projectsSource).not.toContain("planMove(");
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
    expect(apps).toMatch(/<button type="button" aria-expanded="false" aria-label="Unfold Apps" title="Unfold Apps \(→\)" class="ui-pill side-cat-fold"><svg class="side-cat-chevron" viewBox="0 0 16 16" aria-hidden="true" focusable="false"><path d="M6 3.5 10.5 8 6 12.5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><\/path><\/svg><\/button><span class="side-cat-name" title="Apps">Apps<\/span><span class="side-cat-counts"/);
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
    // And the heading is its line: what needs you shrinks only on a line of its own (mu-212), and
    // its counts after it are whole or wrap out of sight (the widths are tested in "a heading's line").
    const rule = (sel: string) => [...css.matchAll(new RegExp(`(?:^|\\n)${sel.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")} \\{([^}]*)\\}`, "g"))].map((m) => m[1]).join(";");
    expect(rule(".side-cat-counts")).toContain("width: max-content");
    expect(rule(".side-cat-needs .count.warn")).toContain("flex: none");
    expect(rule(".side-cat-rest")).toMatch(/flex-wrap: wrap;.*height: 16px;.*overflow: hidden/);
  });
});
