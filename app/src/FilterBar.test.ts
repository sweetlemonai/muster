import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import FilterBar, { Hl } from "./FilterBar";
import { dayContext, EMPTY_QUERY, type Query, type SavedView } from "./filters";
import { TableView, TimelineView } from "./Views";
import type { Task } from "./types";

const t = (over: Partial<Task>): Task => ({
  queue: "q", id: "0001", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [], folder: "todo", path: "", claimedBy: "", heldBy: "", stageSince: "", checkins: [],
  heartbeat: "", waitingOn: "", depends: [], commit: "", captured: "2026-10-01", capturedAt: "", blockHistory: [], questions: [], openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false,
  tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", search: "", ...over,
});
const NOW = new Date(2026, 9, 14, 12, 0).getTime() / 1000;
const tasks = [
  t({ id: "0001", stage: "building", kind: "bug", priority: "high", tags: ["design"], blocked: true, waitingOn: "owner", waitingForOwner: true }),
  t({ id: "0002", stage: "ready", tags: ["docs"] }),
];
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").trim();

function bar(over: { query?: Partial<Query>; sortable?: boolean; views?: SavedView[]; shown?: number; total?: number; hidden?: number } = {}) {
  const html = renderToStaticMarkup(
    createElement(FilterBar, {
      tasks,
      query: { ...EMPTY_QUERY, ...over.query },
      onQuery: () => {},
      ctx: dayContext(NOW),
      sortable: over.sortable ?? false,
      views: over.views ?? [],
      onViews: () => {},
      shown: over.shown ?? 2,
      total: over.total ?? 2,
      hidden: over.hidden ?? 0,
      estimate: "",
    }),
  );
  const group = (label: string) => {
    const m = new RegExp(`<span class="meta[^"]*">${label}</span>((?:<button[^>]*>[^<]*</button>)*)`).exec(html);
    return m ? [...m[1].matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((x) => x[1]) : null;
  };
  const active = /<div class="active-line"[^>]*>([\s\S]*?)<\/div>/.exec(html)?.[1] ?? "";
  return { html, group, active };
}

describe("the filter bar", () => {
  it("starts with the search box, its value and ⌘F", () => {
    const { html } = bar({ query: { search: "flick" } });
    expect(html).toMatch(/^<div class="filter-bar"[^>]*><div class="filters"[^>]*><input type="search" class="search"/);
    expect(html).toContain('value="flick"');
    expect(html).toContain('aria-label="Search tasks"');
    expect(html).toContain("⌘F");
  });
  it("shows each group's values that exist, and no group without any", () => {
    const b = bar();
    expect(b.group("Stage")).toEqual(["Ready", "Building"]);
    expect(b.group("Type")).toEqual(["bug", "task"]);
    expect(b.group("Priority")).toEqual(["high", "normal"]);
    expect(b.group("Tags")).toEqual(["#design", "#docs"]);
    expect(b.group("Flags")).toEqual(["blocked", "needs you"]);
    expect(b.group("Captured")).toEqual(["this month"]);
    expect(b.group("Held by")).toBeNull();
    expect(b.group("Done")).toBeNull();
  });
  it("marks selected chips as pressed", () => {
    const { html } = bar({ query: { filters: { tag: ["design"] } } });
    expect(html).toMatch(/<button class="chip on" aria-pressed="true">#design<\/button>/);
    expect(html).toMatch(/<button class="chip" aria-pressed="false">#docs<\/button>/);
  });
  it("offers the board's sort as chips only where it sorts, the current one pressed", () => {
    expect(bar().group("Sort")).toBeNull();
    const b = bar({ sortable: true, query: { sort: "changed" } });
    expect(b.group("Sort")).toEqual(["priority", "newest", "oldest", "changed", "estimate", "id"]);
    expect(b.html).toMatch(/<button class="chip on" aria-pressed="true" title="[^"]*">changed<\/button>/);
  });
  it("reads the active filters as one line, each with ✕, then the hidden count and Clear all", () => {
    const b = bar({ query: { search: "flick", filters: { stage: ["building"], tag: ["design"], state: ["blocked"] } }, shown: 12, total: 48, hidden: 36 });
    expect(text(b.active)).toBe("“flick” ✕ · Building ✕ · #design ✕ · blocked ✕ 36 hidden by filters Clear all");
    expect(b.active).toContain('aria-label="Remove Building"');
    expect(b.active).toContain('aria-label="Remove “flick”"');
    expect(text(b.html)).toContain("12 of 48 tasks");
  });
  it("keeps the line's place with nothing active, and says nothing is filtered", () => {
    const b = bar();
    expect(text(b.active)).toBe("No filters: every task shows");
    expect(b.active).not.toContain("Clear all");
    expect(text(b.html)).toContain("2 tasks");
  });
  it("says nothing is hidden when filters keep everything", () => {
    expect(text(bar({ query: { filters: { tag: ["design"] } }, hidden: 0 }).active)).toContain("nothing hidden");
  });
  it("shows saved views as chips, the one in use pressed, each with ✕", () => {
    const views: SavedView[] = [
      { name: "My checks", search: "", filters: { stage: ["your-check"] }, sort: "priority" },
      { name: "Docs", search: "", filters: { tag: ["docs"] }, sort: "priority" },
    ];
    const { html } = bar({ views, query: { filters: { tag: ["docs"] } } });
    expect(html).toMatch(/<button class="chip view" aria-pressed="false">My checks<\/button>/);
    expect(html).toMatch(/<button class="chip view on" aria-pressed="true">Docs<\/button>/);
    expect(html).toContain('aria-label="Delete view Docs"');
    expect(html).toContain("Save view");
  });
});

describe("Hl", () => {
  const hl = (s: string, words: string[]) => renderToStaticMarkup(createElement(Hl, { text: s, words }));
  it("marks the matched words, as shown", () => {
    expect(hl("Board flickers on Résumé", ["flick", "resume"])).toBe("Board <mark>flick</mark>ers on <mark>Résumé</mark>");
  });
  it("is the plain text without words", () => {
    expect(hl("A <b> title", [])).toBe("A &lt;b&gt; title");
  });
});

describe("the views highlight what the search matched", () => {
  const list = [t({ id: "0019", title: "Board flickers", tags: ["design"] })];
  it("in the Table's rows: title, tags and id", () => {
    const html = renderToStaticMarkup(createElement(TableView, { tasks: list, now: NOW, multi: false, openKey: null, onOpen: () => {}, words: ["flick", "design", "19"] }));
    expect(html).toContain("Board <mark>flick</mark>ers");
    expect(html).toContain("#<mark>design</mark>");
    expect(html).toContain('<td class="id"><mark>0019</mark></td>');
  });
  it("in the Timeline's labels", () => {
    // The timeline remembers its scale in local storage, which tests run without.
    vi.stubGlobal("localStorage", { getItem: () => null, setItem: () => {} });
    try {
      const html = renderToStaticMarkup(createElement(TimelineView, { tasks: list, now: NOW, multi: false, openKey: null, onOpen: () => {}, words: ["board", "19"] }));
      expect(html).toContain("<mark>Board</mark> flickers");
      expect(html).toContain('<span class="id"><mark>0019</mark></span>');
    } finally {
      vi.unstubAllGlobals();
    }
  });
});
