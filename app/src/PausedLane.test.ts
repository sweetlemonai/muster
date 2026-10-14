import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import Card from "./Card";
import { applyQuery, dayContext, GROUPS, groupValues } from "./filters";
import { GroupedBoard } from "./Groups";
import { groupItems, isWork } from "./levels";
import { columns, stampMinutes } from "./lib";
import { columnFold, columnState, columnTrack, ELEMENTS, look, Mode, MODES, type ColumnFold, type ColumnFolds, type DisplayMode } from "./modes";
import { wasText } from "./Pause";
import { PausedColumn } from "./PausedLane";
import { ProjectItem } from "./ProjectList";
import type { Task } from "./types";
import { sortTasks } from "./viewData";
import { TableView, TimelineChart } from "./Views";

// The Paused lane (mu-59): a paused task waits beside Backlog, out of the working columns, and
// says where it was; the stage is kept, so Resume puts it back.

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0042", title: "T", kind: "code", priority: "normal", status: "doing", stage: "building", blocked: false, stageHistory: [],
  folder: "doing", path: "doing/0042-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "building", ...over,
});
const at = (s: string) => {
  const [y, mo, d, h, mi] = s.split(/[- :]/).map(Number);
  return new Date(y, mo - 1, d, h, mi).getTime() / 1000;
};
const NOW = at("2026-10-13 12:00");
const A = "wt-a (wt-0045 · session 7b5e8856 · cc-hg)";
const paused = { stage: "building", claimedBy: A, stageSince: "2026-10-13 08:00", paused: "2026-10-13 10:00", pausedWhy: "the design changes first", modified: NOW };
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/\s+/g, " ").trim();
const inMode = (mode: DisplayMode, el: ReactElement) => renderToStaticMarkup(createElement(Mode.Provider, { value: mode }, el));
const stub = (t: Task) => createElement("i", { className: "card-stub" }, t.id);
const column = (tasks: Task[], fold: ColumnFold = "fixed") => renderToStaticMarkup(createElement(PausedColumn, { tasks, state: columnState(fold, tasks.length), fold, onFold: () => {}, renderCard: stub }));
/** The lane's track in the board's grid, as App works it out. */
const laneWidth = (count: number, fold: ColumnFold) => columnTrack(columnState(fold, count));

describe("what a card in the Paused lane says", () => {
  it("where it was, since when and why", () => {
    expect(wasText(task(paused), NOW)).toBe("was Building · paused 2 h ago · the design changes first");
    expect(wasText(task({ ...paused, stage: "ready", pausedWhy: "" }), NOW)).toBe("was Ready · paused 2 h ago");
    expect(wasText(task({ ...paused, paused: "2026-10-13 12:00" }), NOW)).toBe("was Building · paused just now · the design changes first");
    // A stamp it can't read is shown as it is.
    expect(wasText(task({ ...paused, paused: "someday" }), NOW)).toBe("was Building · paused since someday · the design changes first");
    expect(wasText(task({}), NOW)).toBe("");
  });
  it("on the card, in every display mode, with the pause glyph and Resume", () => {
    for (const { id } of MODES) {
      const html = inMode(id, createElement(Card, { t: task(paused), now: NOW, multi: false, parent: "", onOpen: () => {}, active: false, words: [] }));
      expect(text(html), id).toContain("⏸︎ was Building · paused 2 h ago · the design changes first");
      expect(html, id).toContain('<span class="paused-chip">⏸︎</span>');
      expect(html, id).toContain(">Resume</button>");
    }
  });
});

describe("the Paused lane on the board", () => {
  it("holds the paused tasks with its count and what Resume does", () => {
    const html = column([task({ ...paused, id: "0001" }), task({ ...paused, id: "0002", stage: "backlog" })]);
    expect(html).toMatch(/^<section class="column col-paused">/);
    expect(text(html)).toMatch(/^⏸︎ Paused 2 Resume puts it back in its column/);
    expect([...html.matchAll(/card-stub">(\d+)</g)].map((m) => m[1])).toEqual(["0001", "0002"]);
  });
  it("is there with nothing in it, as a thin strip with its name that says empty (mu-83)", () => {
    const html = column([]);
    expect(html).toMatch(/^<section class="column col-paused col-strip col-empty">/);
    expect(text(html)).toMatch(/^⏸︎ Paused empty/);
    expect(laneWidth(0, "fixed")).toBe("44px");
    expect(laneWidth(2, "fixed")).toBe("minmax(118px, 400px)");
  });
  it("folds to its strip in Focus, with its count and what needs you, and opens with a click", () => {
    const list = [task({ ...paused, id: "0001", waitingForOwner: true }), task({ ...paused, id: "0002" })];
    const folded = column(list, "folded");
    expect(folded).toMatch(/^<section class="column col-paused col-strip folded">/);
    expect(folded).toContain('<button type="button" class="col-fold" aria-expanded="false"');
    expect(text(folded)).toContain("⏸︎ Paused 2 1");
    expect(laneWidth(2, "folded")).toBe("44px");
    expect(column(list, "opened")).toContain('aria-expanded="true"');
    expect(laneWidth(2, "opened")).toBe("minmax(118px, 400px)");
  });
  it("is open in ADHD and Standard and a strip in Focus, as Settings' Display says", () => {
    expect(MODES.map((m) => columnFold(look("columns", m.id), "paused", new Set()))).toEqual(["fixed", "fixed", "folded"]);
    expect(columnFold(look("columns", "focus"), "paused", new Set(["paused"]))).toBe("opened");
    for (const { id } of MODES) expect(ELEMENTS.columns[id].says, id).toContain("Paused");
  });
  it("takes the stage columns' counts down: they count only their working tasks", () => {
    const c = columns([task({ id: "0001" }), task({ ...paused, id: "0002" }), task({ ...paused, id: "0003", stage: "testing" })]);
    expect([c.paused.length, c.building.length, c.testing.length]).toEqual([2, 1, 0]);
  });
});

describe("the grouped board's Paused band", () => {
  const all = [
    task({ id: "0001", title: "Stage track", level: "feature", stage: "ready", progress: { done: 0, total: 2, estMinutes: 0, estTokenCount: 0 } }),
    task({ id: "0002", title: "Quiet", level: "feature", stage: "ready", progress: { done: 0, total: 1, estMinutes: 0, estTokenCount: 0 } }),
    task({ id: "0003", parent: "0001" }),
    task({ ...paused, id: "0004", parent: "0001" }),
    task({ ...paused, id: "0005", parent: "0002", stage: "ready" }),
    task({ ...paused, id: "0006", stage: "backlog" }),
    task({ id: "0007", stage: "backlog" }),
  ];
  const grouped = (list: Task[], fold?: ColumnFolds) =>
    renderToStaticMarkup(
      createElement(GroupedBoard, {
        groups: groupItems(list.filter(isWork), list, "feature"), by: "feature", sort: "priority", multi: false, openKey: null, onOpen: () => {}, renderCard: stub, fold,
        backlog: { notes: [], renderNote: () => null, quickAdd: null },
      }),
    );
  const ids = (html: string) => [...html.matchAll(/card-stub">(\d+)</g)].map((m) => m[1]);
  const band = (html: string) => /<section class="band paused-band[^"]*">([\s\S]*?)<\/section>(?=<section class="band)/.exec(html)?.[1] ?? "";
  it("comes first and holds every paused task, from every feature, and no other band does", () => {
    const html = grouped(all);
    expect(html.indexOf("paused-band")).toBeGreaterThan(-1);
    expect(html.indexOf("paused-band")).toBeLessThan(html.indexOf("backlog-band"));
    expect(text(band(html))).toMatch(/^⏸︎ Paused 3 tasks/);
    expect(ids(band(html))).toEqual(["0004", "0005", "0006"]);
    const rest = html.slice(html.indexOf("backlog-band"));
    expect(ids(rest)).toEqual(["0007", "0003"]);
    // Only paused work in view: the band holds it, so it isn't "No tasks match".
    const only = grouped([task({ ...paused, id: "0008" })]);
    expect(ids(band(only))).toEqual(["0008"]);
    expect(only).not.toContain("No tasks match.");
  });
  it("says so on a feature whose every task is paused, rather than drawing empty columns", () => {
    const html = grouped(all);
    const quiet = html.slice(html.indexOf("Quiet"));
    expect(text(quiet)).toContain("Every task here is paused: they're in Paused, at the top");
    expect(quiet.slice(0, quiet.indexOf("</section>"))).not.toContain("col-ready");
  });
  it("is a thin labelled line with nothing paused", () => {
    const html = grouped(all.filter((t) => !t.paused));
    expect(html).toContain('<section class="band paused-band paused-none">');
    expect(text(band(html))).toBe("⏸︎ Paused 0 tasks");
    expect(band(html)).not.toContain("col-paused");
  });
  it("folds to its head in Focus, with its count, and opens in place", () => {
    const folds = (state: "folded" | "opened"): ColumnFolds => ({ of: (s) => (s === "building" || s === "testing" || s === "your-check" ? "fixed" : state), toggle: () => {} });
    const folded = band(grouped(all, folds("folded")));
    expect(folded).toContain('<button type="button" class="band-fold" aria-expanded="false"');
    expect(text(folded)).toMatch(/^⏸︎ Paused 3 tasks/);
    expect(ids(folded)).toEqual([]);
    expect(ids(band(grouped(all, folds("opened"))))).toEqual(["0004", "0005", "0006"]);
  });
});

describe("the other views treat the lane the same way", () => {
  const ctx = dayContext(NOW);
  const list = [task({ ...paused, id: "0001" }), task({ id: "0002" }), task({ ...paused, id: "0003", stage: "ready" })];
  it("a Paused chip in the filter bar's Flags finds the lane's tasks, and with a stage, what was paused there", () => {
    const flags = GROUPS.find((g) => g.id === "state")!;
    expect(groupValues(flags, list, [], ctx)).toContain("paused");
    const ids = (filters: Record<string, string[]>) => applyQuery(list, { search: "", filters, sort: "priority" }, ctx).map((t) => t.id);
    expect(ids({ state: ["paused"] })).toEqual(columns(list).paused.map((t) => t.id).sort());
    expect(ids({ state: ["paused"], stage: ["building"] })).toEqual(["0001"]);
  });
  it("the table's stage cell says Paused first, then where it was, and sorts paused rows first, as the lane sits", () => {
    const html = renderToStaticMarkup(createElement(TableView, { tasks: [task(paused)], now: NOW, multi: false, openKey: null, onOpen: () => {}, words: [] }));
    expect(html).toContain('<span class="ui-status-pill ui-status-pill--paused">⏸︎ Paused</span><span class="ui-status-pill ui-status-pill--building was">was Building</span>');
    expect(html).not.toContain('<span class="ui-status-pill ui-status-pill--building">Building</span>');
    const rows = [task({ id: "0001", stage: "backlog" }), task({ ...paused, id: "0002" }), task({ id: "0003", stage: "ready" })];
    expect(sortTasks(rows, "stage", false).map((t) => t.id)).toEqual(["0002", "0001", "0003"]);
  });
  it("the timeline doesn't call a paused bar its stage's: it says Paused and where it was", () => {
    const t = task({ ...paused, stageHistory: [{ at: "2026-10-13 08:00", who: A, from: "ready", to: "building" }], pauseHistory: [{ from: "2026-10-13 10:00", to: "" }] });
    const span = { task: t, start: stampMinutes("2026-10-13 08:00")!, work: 0, end: 0, open: true, timed: true, segments: [], blocks: [], pauses: [], finish: 0 };
    const html = renderToStaticMarkup(
      createElement(TimelineChart, { scale: "day", rows: [{ span, left: 0, width: 0.5, clipStart: false, clipEnd: false, segments: [], blocks: [], pauses: [] }], ticks: [], now: null, multi: false, openKey: null, onOpen: () => {} }),
    );
    expect(html).toContain('<span class="tl-bar-text">Paused · was Building</span>');
  });
  it("the sidebar doesn't count a paused task as in progress, and keeps its paused count", () => {
    const row = renderToStaticMarkup(createElement(ProjectItem, { name: "demo", total: 3, tasks: list, on: false, onClick: () => {}, children: null }));
    expect(row).toContain('<span class="side-doing" title="1 in progress (Building or Testing)">1</span>');
    expect(row).toContain('<span class="side-paused" title="2 paused">⏸︎ 2</span>');
  });
});
