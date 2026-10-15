import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { backlogNotes, NoteCard, QuickAdd } from "./Backlog";
import { GroupByChips, GroupedBoard } from "./Groups";
import { groupItems, isWork } from "./levels";
import type { BoardSort } from "./lib";
import type { Note, Task } from "./types";
import { TableView } from "./Views";

const task = (over: Partial<Task>): Task => ({
  queue: "muster", id: "0001", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0,
  level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});

const all = [
  task({ id: "0001", title: "Ship 1.0", level: "goal", due: "2026-12-01", progress: { done: 1, total: 3, estMinutes: 150, estTokenCount: 200_000 }, shownStage: "in-progress" }),
  task({ id: "0002", title: "Stage track", level: "feature", parent: "0001", progress: { done: 1, total: 2, estMinutes: 120, estTokenCount: 200_000 }, shownStage: "in-progress" }),
  task({ id: "0003", title: "Empty one", level: "feature", parent: "0001", progress: { done: 0, total: 0, estMinutes: 0, estTokenCount: 0 } }),
  task({ id: "0004", title: "Done task", parent: "0002", stage: "done" }),
  task({ id: "0005", title: "Built task", parent: "0002", stage: "building" }),
  task({ id: "0006", title: "Under the goal", parent: "0001" }),
  task({ id: "0007", title: "Loose task" }),
];

function board(by: "feature" | "goal", sort: BoardSort = "priority") {
  const groups = groupItems(all.filter(isWork), all, by);
  return renderToStaticMarkup(
    createElement(GroupedBoard, {
      groups, by, sort, multi: false, openKey: null, onOpen: () => {},
      renderCard: (t: Task) => createElement("i", { className: "card-stub" }, `${t.id}`),
    }),
  );
}
/** Each band as text: its heading, then the cards in each non-empty column. */
function bands(html: string): string[] {
  return html.split('<section class="band">').slice(1).map((b) => {
    const head = (/<div class="band-head">([\s\S]*?)<\/div>/.exec(b)?.[1] ?? "").replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").trim();
    const cols = [...b.matchAll(/<section class="column col-([a-z-]+)"><h2[^>]*>[\s\S]*?<\/h2><div class="cards">([\s\S]*?)<\/div><\/section>/g)]
      .map((m) => `${m[1]}: ${[...m[2].matchAll(/card-stub">(\d+)</g)].map((c) => c[1]).join(",")}`);
    return [head, ...cols].join(" | ") + (b.includes("No tasks yet") ? " | No tasks yet" : "");
  });
}

describe("grouped board", () => {
  it("has a band per feature with its progress and estimate left, the stage columns inside, and No feature last", () => {
    expect(bands(board("feature"))).toEqual([
      "Feature 0002 Stage track In progress 1 of 2 done ≈ 2 h · 200k tokens left | building: 0005 | done: 0004",
      "Feature 0003 Empty one Ready no tasks yet | No tasks yet",
      "No feature 2 items | ready: 0006,0007",
    ]);
  });
  it("by goal, a goal's band holds its features' tasks and its own, and shows its due date", () => {
    expect(bands(board("goal"))).toEqual([
      "Goal 0001 Ship 1.0 In progress 1 of 3 done ≈ 2.5 h · 200k tokens left due 2026-12-01 | ready: 0006 | building: 0005 | done: 0004",
      "No goal 1 item | ready: 0007",
    ]);
  });
  it("shows a goal In progress when the core says so, though every task under it is done (an empty feature isn't)", () => {
    const items = [
      task({ id: "0001", title: "G", level: "goal", progress: { done: 1, total: 1, estMinutes: 0, estTokenCount: 0 }, shownStage: "in-progress" }),
      task({ id: "0002", title: "Done one", level: "feature", parent: "0001", progress: { done: 1, total: 1, estMinutes: 0, estTokenCount: 0 }, shownStage: "done" }),
      task({ id: "0003", title: "Empty one", level: "feature", parent: "0001", progress: { done: 0, total: 0, estMinutes: 0, estTokenCount: 0 } }),
      task({ id: "0004", parent: "0002", stage: "done" }),
    ];
    const html = renderToStaticMarkup(
      createElement(GroupedBoard, {
        groups: groupItems(items.filter(isWork), items, "goal"), by: "goal", sort: "priority", multi: false, openKey: null, onOpen: () => {},
        renderCard: (t: Task) => createElement("i", { className: "card-stub" }, `${t.id}`),
      }),
    );
    expect(bands(html)).toEqual(["Goal 0001 G In progress 1 of 1 done | done: 0004"]);
  });
  it("never draws a goal or a feature as a card", () => {
    const html = board("feature") + board("goal");
    expect(html).not.toMatch(/card-stub">000[123]</);
  });
  it("orders each column of each band by the board's sort (task 0032)", () => {
    const items = [
      task({ id: "0010", parent: "0002", captured: "2026-10-01", priority: "high" }),
      task({ id: "0011", parent: "0002", captured: "2026-10-05", priority: "low" }),
      task({ id: "0012", captured: "2026-10-06", priority: "low" }),
      task({ id: "0013", captured: "2026-10-04", priority: "high" }),
    ];
    const heads = all.filter((t) => !isWork(t));
    const columnsOf = (sort: BoardSort) =>
      bands(renderToStaticMarkup(createElement(GroupedBoard, {
        groups: groupItems(items, [...heads, ...items], "feature"), by: "feature", sort, multi: false, openKey: null, onOpen: () => {},
        renderCard: (t: Task) => createElement("i", { className: "card-stub" }, `${t.id}`),
      }))).map((b) => b.split(" | ").slice(1).join(" | "));
    expect(columnsOf("priority")).toEqual(["ready: 0010,0011", "No tasks yet", "ready: 0013,0012"]);
    expect(columnsOf("newest")).toEqual(["ready: 0011,0010", "No tasks yet", "ready: 0012,0013"]);
    expect(bands(board("feature", "id"))).toEqual(bands(board("feature")));
  });
  it("says so when nothing matches", () => {
    const html = renderToStaticMarkup(createElement(GroupedBoard, { groups: [], by: "feature", sort: "priority", multi: false, openKey: null, onOpen: () => {}, renderCard: () => null }));
    expect(html).toContain("No tasks match.");
  });
});

describe("the grouped board's Backlog band (task 0046)", () => {
  const note = (over: Partial<Note>): Note => ({ queue: "muster", file: "n.md", title: "A note", text: "A note", truncated: false, added: 0, heldBy: "", heldSince: "", heartbeat: "", ...over });
  // Two backlog tasks: one with no feature or goal (the band's), one under feature 0002 (its band's).
  const items = [...all, task({ id: "0008", title: "Loose idea", stage: "backlog" }), task({ id: "0009", title: "Featured idea", parent: "0002", stage: "backlog" }), task({ id: "0010", title: "Older idea", stage: "backlog", priority: "high" })];
  const notes = backlogNotes([note({ file: "old.md", title: "Old note", added: 10 }), note({ file: "new.md", title: "Board flickers", added: 30 })], "all");
  function grouped(by: "feature" | "goal", shown: Note[] = notes, words: string[] = [], list: Task[] = items) {
    return renderToStaticMarkup(
      createElement(GroupedBoard, {
        groups: groupItems(list.filter(isWork), list, by), by, sort: "priority", multi: false, openKey: null, onOpen: () => {},
        renderCard: (t: Task) => createElement("i", { className: "card-stub" }, `${t.id}`),
        backlog: {
          notes: shown,
          renderNote: (n: Note) => createElement(NoteCard, { key: n.file, n, now: 0, multi: false, active: false, onOpen: () => {}, words }),
          quickAdd: createElement(QuickAdd, { queues: ["muster"], onAdded: () => {} }),
        },
      }),
    );
  }
  /** The Backlog band: its heading, whether the quick add leads it, and its items in order. */
  function backlogBand(html: string) {
    const b = /<section class="band backlog-band">([\s\S]*?)<\/section><\/div><\/section>/.exec(html)?.[1] ?? "";
    const head = (/<div class="band-head">([\s\S]*?)<\/div>/.exec(b)?.[1] ?? "").replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").trim();
    const quickAddFirst = /^<div class="band-head">[\s\S]*?<\/div><div class="board band-board backlog-board"><section class="column col-backlog"><div class="quick-add">/.test(b);
    const order = [...b.matchAll(/class="note-card"[^>]*title="([^"]+)"|card-stub">(\d+)</g)].map((m) => m[1] ?? m[2]);
    return { head, quickAddFirst, order, none: b.includes("Nothing here") };
  }

  it("by feature: comes first, the quick add, the notes newest first, then the backlog tasks with no feature", () => {
    const html = grouped("feature");
    expect(html.indexOf("backlog-band")).toBeGreaterThan(-1);
    expect(html.indexOf("backlog-band")).toBeLessThan(html.indexOf('<section class="band">'));
    expect(backlogBand(html)).toEqual({ head: "Backlog · not in a feature yet 4 items", quickAddFirst: true, order: ["new.md", "old.md", "0010", "0008"], none: false });
    // The featured backlog task stays in its feature's band; No feature keeps the loose tasks in later stages.
    expect(bands(html)).toEqual([
      "Feature 0002 Stage track In progress 1 of 2 done ≈ 2 h · 200k tokens left | backlog: 0009 | building: 0005 | done: 0004",
      "Feature 0003 Empty one Ready no tasks yet | No tasks yet",
      "No feature 2 items | ready: 0006,0007",
    ]);
  });

  it("by goal: titled for goals, with the backlog tasks under no goal", () => {
    const html = grouped("goal");
    expect(backlogBand(html)).toEqual({ head: "Backlog · not in a goal yet 4 items", quickAddFirst: true, order: ["new.md", "old.md", "0010", "0008"], none: false });
    expect(bands(html)).toEqual([
      "Goal 0001 Ship 1.0 In progress 1 of 3 done ≈ 2.5 h · 200k tokens left due 2026-12-01 | backlog: 0009 | ready: 0006 | building: 0005 | done: 0004",
      "No goal 1 item | ready: 0007",
    ]);
  });

  it("is drawn with its quick add when there are no notes and no backlog tasks", () => {
    const html = grouped("feature", [], [], all);
    expect(backlogBand(html)).toEqual({ head: "Backlog · not in a feature yet 0 items", quickAddFirst: true, order: [], none: true });
    expect(html).toContain('placeholder="Add to the backlog…"');
  });

  it("shows a note added from its quick add once the refresh brings it", () => {
    const before = grouped("feature", []);
    expect(backlogBand(before).order).toEqual(["0010", "0008"]);
    const added = backlogNotes([note({ file: "2026-10-13-0316-fix-it.md", title: "Fix it", added: 50 })], "all");
    expect(backlogBand(grouped("feature", added)).order).toEqual(["2026-10-13-0316-fix-it.md", "0010", "0008"]);
  });

  it("marks the search's words in the note titles", () => {
    const html = grouped("feature", notes.slice(0, 1), ["flick"]);
    expect(html).toContain('<span class="title">Board <mark>flick</mark>ers</span>');
  });

  it("takes the place of No feature when every loose task is in Backlog, and doesn't say nothing matches", () => {
    const list = [task({ id: "0001", title: "Loose idea", stage: "backlog" })];
    const html = grouped("feature", [], [], list);
    expect(backlogBand(html).order).toEqual(["0001"]);
    expect(bands(html)).toEqual([]);
    expect(html).not.toContain("No tasks match.");
  });

  it("says nothing matches below the band when no task does", () => {
    const html = grouped("feature", notes, [], []);
    expect(backlogBand(html).order).toEqual(["new.md", "old.md"]);
    expect(html).toContain("No tasks match.");
  });
});

describe("grouped table", () => {
  const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").trim();
  const table = (groups?: ReturnType<typeof groupItems>, by: "none" | "feature" | "goal" = "none") =>
    renderToStaticMarkup(createElement(TableView, { tasks: all, now: 0, multi: false, openKey: null, onOpen: () => {}, groups, by, words: [] }));
  /** Each row as text: the band heading rows, and the id and title cells of the others. */
  const rows = (html: string) =>
    [...html.matchAll(/<tr( class="([^"]*)")?[^>]*>([\s\S]*?)<\/tr>/g)].slice(1).map((m) =>
      m[2]?.includes("group-row") ? `# ${text(m[3])}` : text(m[3]).split(" ").slice(0, 2).join(" "),
    );
  it("shows a goal or feature row with its level, roll-up and the stage its tasks give it", () => {
    const html = table();
    expect(text(html)).toContain("0002 Feature Stage track 1 of 2 done task normal In progress ≈ 2 h · 200k tokens left");
    // A task's row is as before: its own stage, no level badge.
    expect(text(html)).toContain("0005 Built task task normal Building");
    expect(html.match(/level-item/g)?.length).toBe(3);
  });
  it("by feature: a heading row per feature, its rows under it, and the rest last", () => {
    const by = groupItems(all, all, "feature");
    expect(rows(table(by, "feature"))).toEqual([
      "# Feature 0002 Stage track In progress 1 of 2 done ≈ 2 h · 200k tokens left", "0005 Built", "0004 Done",
      "# Feature 0003 Empty one Ready no tasks yet",
      // Sorted by stage: the goal shows In progress, so it comes after the Ready tasks.
      "# No feature 3 items", "0006 Under", "0007 Loose", "0001 Goal",
    ]);
  });
  it("marks the search's words in grouped rows, beside the level badge (task 0032)", () => {
    const html = renderToStaticMarkup(
      createElement(TableView, { tasks: all, now: 0, multi: false, openKey: null, onOpen: () => {}, groups: groupItems(all, all, "feature"), by: "feature", words: ["ship", "5"] }),
    );
    // The goal is a row in No feature: its level badge, then its title with the word marked.
    expect(html).toContain('<span class="ui-badge ui-badge--level">Goal</span> <mark>Ship</mark> 1.0');
    expect(html).toContain('<td class="id" title="muster · 0005"><mark>0005</mark></td>');
  });
});

describe("Group by chips", () => {
  it("shows every choice, the current one lit", () => {
    const html = renderToStaticMarkup(createElement(GroupByChips, { value: "goal", onChange: () => {} }));
    const chips = [...html.matchAll(/<button[^>]*class="([^"]+)">([^<]+)<\/button>/g)].map((m) => `${m[2]}${m[1].includes("ui-pill--on") ? "*" : ""}`);
    expect(chips).toEqual(["None", "Feature", "Goal*"]);
  });
});
