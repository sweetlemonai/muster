import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import appSource from "./App.tsx?raw";
import Card from "./Card";
import { DisplayModes } from "./Display";
import FilterBar from "./FilterBar";
import { dayContext, EMPTY_QUERY, type Query } from "./filters";
import { GroupByChips, GroupedBoard } from "./Groups";
import groupsSource from "./Groups.tsx?raw";
import { windowKey } from "./keys";
import { blockedCount, ChoiceChips, ColumnHead, Fold, ModeChip } from "./ModeViews";
import { autoOpen, columnFold, ELEMENTS, look, Mode, MODES, RAIL_ID, ROW_ORDER, type DisplayMode } from "./modes";
import { type KindFilter, weekCount } from "./recentData";
import { logSummary } from "./paneData";
import ProjectList from "./ProjectList";
import Recent from "./Recent";
import TaskPanel from "./TaskPanel";
import paneSource from "./TaskPanel.tsx?raw";
import css from "./styles.css?raw";
import { pressed, SHORTCUTS } from "./shortcuts";
import { SETTINGS_GROUPS } from "./settingsData";
import type { Activity, QueueInfo, Task } from "./types";

const task = (id: string, over: Partial<Task> = {}): Task => ({
  queue: "muster", id, title: `Task ${id}`, kind: "code", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: `todo/${id}-t.md`, claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", search: "", ...over,
});
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/&quot;/g, '"').replace(/\s+/g, " ").trim();
const ALL: DisplayMode[] = MODES.map((m) => m.id);
const inMode = (mode: DisplayMode, el: ReactElement) => renderToStaticMarkup(createElement(Mode.Provider, { value: mode }, el));

// A building card with everything a card can carry.
const NOW = 1_800_000_000;
const stamp = (secs: number) => {
  const d = new Date(secs * 1000);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
};
const busy = task("0038", {
  stage: "building", claimedBy: "muster/wt-0038 (w-0038)", stageSince: stamp(NOW - 3600), heartbeat: stamp(NOW - 60),
  checkins: [{ at: stamp(NOW - 120), who: "muster/wt-0038", pct: 40, text: "mockups done" }],
  tags: ["design"], parent: "0005", estTime: "1d", attachments: ["a.png"], modified: NOW - 60,
});
const card = (mode: DisplayMode, t: Task, active = false, parent = "") =>
  inMode(mode, createElement(Card, { t, now: NOW, multi: false, parent, onOpen: () => {}, active, words: [] }));

describe("Settings › Display (0038, AC1)", () => {
  it("is a Settings group after Appearance, with its ? into the manual", () => {
    expect(SETTINGS_GROUPS.map((g) => g.id)).toEqual(["appearance", "display", "projects", "agents", "accounts", "notifications", "about"]);
    expect(SETTINGS_GROUPS[1]).toMatchObject({ label: "Display", topic: "display" });
  });

  it("shows the three modes as chips, the current one pressed, each saying what it's for", () => {
    for (const mode of ALL) {
      const html = renderToStaticMarkup(createElement(DisplayModes, { mode, onMode: () => {} }));
      const chips = [...html.matchAll(/<button[^>]*aria-pressed="(true|false)"[^>]*>([\s\S]*?)<\/button>/g)];
      expect(chips.map((m) => text(m[2]).split(" ")[0])).toEqual(["ADHD", "Standard", "Focus"]);
      expect(chips.filter((m) => m[1] === "true").map((m) => text(m[2]).split(" ")[0])).toEqual([MODES.find((m) => m.id === mode)?.label]);
      for (const m of MODES) expect(text(html)).toContain(m.says);
    }
  });

  it("previews every element in every mode, side by side, the current mode's column marked", () => {
    const html = renderToStaticMarkup(createElement(DisplayModes, { mode: "standard", onMode: () => {} }));
    const rows = [...html.matchAll(/<tr><th scope="row">([^<]*)<\/th>([\s\S]*?)<\/tr>/g)];
    expect(rows.map((r) => r[1])).toEqual(Object.values(ELEMENTS).map((e) => e.label));
    for (const [, , cells] of rows) expect([...cells.matchAll(/<td( class="on")?>/g)].map((c) => !!c[1])).toEqual([false, true, false]);
    expect(text(html)).toContain(ELEMENTS.filters.standard.reach);
    expect(text(html)).toContain("⌘.");
  });

  it("says the mode in the header as a chip that opens Settings › Display, with no menu", () => {
    for (const mode of ALL) {
      const html = renderToStaticMarkup(createElement(ModeChip, { mode, onOpen: () => {} }));
      expect(html).toMatch(/^<button type="button" title="[^"]*" class="ui-pill mode-chip"/);
      expect(text(html)).toBe(MODES.find((m) => m.id === mode)?.label);
      expect(html).toContain("Settings, Display");
    }
    expect(appSource).toContain('<ModeChip mode={modes.mode} onOpen={() => openSettings("display")} />');
    expect(appSource).toContain("<Mode.Provider value={modes.mode}>");
  });

  it("⌘. toggles Focus from anywhere in the window (Ctrl+. too, as ⌘N)", () => {
    expect(SHORTCUTS.find((s) => s.id === "focus-mode")).toMatchObject({ keys: "⌘.", area: "anywhere", files: ["keys.ts"] });
    expect(pressed({ key: ".", metaKey: true, ctrlKey: false }, "focus-mode")).toBe(true);
    expect(pressed({ key: ".", metaKey: false, ctrlKey: true }, "focus-mode")).toBe(true);
    expect(pressed({ key: ".", metaKey: false, ctrlKey: false }, "focus-mode")).toBe(false);
    const log: string[] = [];
    const act = { closePane: () => log.push("close"), newTask: () => log.push("new"), search: () => log.push("search"), focusMode: () => log.push("focus") };
    let prevented = false;
    windowKey({ key: ".", metaKey: true, ctrlKey: false, preventDefault: () => (prevented = true) }, act);
    expect(log).toEqual(["focus"]);
    expect(prevented).toBe(true);
  });
});

describe("the filter bar in each mode", () => {
  const tasks = [task("0001", { stage: "building", priority: "high", tags: ["design"] }), task("0002", { tags: ["docs"] })];
  const bar = (mode: DisplayMode, over: { query?: Partial<Query>; hidden?: number; out?: boolean } = {}) =>
    inMode(
      mode,
      createElement(FilterBar, {
        tasks, query: { ...EMPTY_QUERY, ...over.query }, onQuery: () => {}, ctx: dayContext(NOW, {}), sortable: true, views: [], onViews: () => {},
        shown: 2, total: 2, hidden: over.hidden ?? 0, estimate: "", out: over.out ?? false, onSummon: () => {}, onHide: () => {},
      }),
    );

  it("ADHD: every filter group and Sort on screen, as before", () => {
    const html = bar("adhd");
    expect(html).toContain('class="filters groups"');
    expect(html).toContain('aria-label="Sort each column"');
    expect(html).not.toContain("filters-fold");
  });

  it("Standard: the groups behind one Filters chip that says how many are on; the active line stays", () => {
    const html = bar("standard", { query: { filters: { priority: ["high"], tag: ["design"] } } });
    expect(html).not.toContain('class="filters groups"');
    expect(html).toMatch(/<button type="button" aria-expanded="false"[^>]*class="ui-pill filters-fold">Filters <b class="count">2<\/b><\/button>/);
    expect(text(html)).toContain("Clear all");
    expect(bar("standard")).toMatch(/aria-expanded="false"[^>]*class="ui-pill filters-fold">Filters<\/button>/);
  });

  it("Focus: one Search and filters button until ⌘F, a search or a filter brings the bar back", () => {
    const quiet = bar("focus");
    expect(quiet).not.toContain('type="search"');
    expect(text(quiet)).toBe("Search and filters ⌘F");
    for (const html of [bar("focus", { out: true }), bar("focus", { query: { search: "flick" } }), bar("focus", { query: { filters: { tag: ["docs"] } } }), bar("focus", { hidden: 1 })]) {
      expect(html).toContain('type="search"');
    }
    // Brought back by ⌘F, it can go again; brought back by a filter, it stays until the filter goes.
    expect(text(bar("focus", { out: true }))).toContain("Hide");
    expect(text(bar("focus", { query: { search: "flick" } }))).not.toContain("Hide");
  });
});

describe("Who and Group by in each mode", () => {
  const who = (mode: DisplayMode) =>
    inMode(mode, createElement(ChoiceChips, { label: "Who", className: "for-filter", aria: "Who does it", value: "all", options: [{ id: "all", label: "All" }, { id: "human", label: "You" }, { id: "ai", label: "Agents" }], onPick: () => {} }));

  it("ADHD lays out every choice; Standard and Focus show the current one as a chip that opens them", () => {
    expect([...who("adhd").matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1])).toEqual(["All", "You", "Agents"]);
    for (const mode of ["standard", "focus"] as const) {
      const html = who(mode);
      expect([...html.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1])).toEqual(["All"]);
      expect(html).toContain('aria-label="Who: All, click to change"');
    }
  });

  it("Group by reads the same table", () => {
    const html = inMode("standard", createElement(GroupByChips, { value: "feature", onChange: () => {} }));
    expect([...html.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1])).toEqual(["Feature"]);
    expect(inMode("adhd", createElement(GroupByChips, { value: "feature", onChange: () => {} }))).toContain('<span class="group-by" role="group" aria-label="Group by"><span class="meta">Group by</span>');
  });
});

describe("a card in each mode", () => {
  it("ADHD: every line, as before", () => {
    const t = text(card("adhd", busy, false, "0005 Display"));
    for (const words of ["muster/wt-0038", "40%", "#design", "0005 Display", "1d", "1 file"]) expect(t).toContain(words);
  });

  it("Standard: who, the check-in, tags, parent, estimate and files on the selected card only", () => {
    const html = card("standard", busy, false, "0005 Display");
    const quiet = text(html);
    expect(quiet).toContain("Task 0038");
    for (const words of ["40%", "#design", "0005 Display", "1d", "1 file"]) expect(quiet, words).not.toContain(words);
    // Who holds it is in the holder ribbon, in every mode (0097): the one place the brief card names it.
    expect(html).not.toContain("card-who");
    expect(quiet.match(/muster\/wt-0038/g)).toHaveLength(1);
    // Read out whole in the ribbon's one sentence; shown as its head and number (mu-147).
    expect(html).toContain('<span class="sr-only">held by muster/wt-0038</span><span aria-hidden="true" class="holder-name tail-3" dir="auto"><span class="holder-head">muster/wt-0</span><span class="holder-tail">038</span></span>');
    const picked = text(card("standard", busy, true, "0005 Display"));
    for (const words of ["muster/wt-0038", "40%", "#design", "0005 Display", "1d", "1 file"]) expect(picked, words).toContain(words);
  });

  it("Focus: the title and one status line, even when selected", () => {
    const html = card("focus", busy, true, "0005 Display");
    const t = text(html);
    expect(t).toContain("Task 0038");
    expect(t).toContain("muster/wt-0038");
    for (const words of ["40%", "#design", "0005 Display", "1d", "1 file"]) expect(t, words).not.toContain(words);
    // No kind or priority chip in the top row (the colour band still names the stage for screen readers).
    expect(html).not.toMatch(/class="(kind|pri) /);
    expect(html).not.toContain('class="age"');
  });

  it("says a Testing task held by its builder needs a tester in every mode, so it doesn't look worked on (review)", () => {
    const t = task("0047", { stage: "testing", claimedBy: "muster/wt-0047 (w-0047)", heldBy: "muster/wt-0047 (w-0047)", builtBy: "muster/wt-0047 (w-0047)" });
    for (const mode of ALL) expect(card(mode, t), mode).toContain('<span class="card-tester">needs a tester</span>');
  });

  it("keeps needs you, blocked and quiet on the card in every mode (AC3)", () => {
    const needs = task("0050", { stage: "your-check", waitingForOwner: true });
    const blocked = task("0052", { blocked: true, waitingOn: "owner: which colour?", blockedKind: "you", blockDetail: "which colour?", waitingForOwner: true });
    const quiet = task("0053", { stage: "building", claimedBy: "muster/wt-x", heartbeat: stamp(NOW - 5 * 3600), stageSince: stamp(NOW - 6 * 3600) });
    for (const mode of ALL) {
      expect(card(mode, needs), mode).toContain('<span class="ui-badge ui-badge--warn">needs you</span>');
      expect(text(card(mode, needs)), mode).toContain("Your turn");
      expect(card(mode, blocked), mode).toContain("<b>Your action</b>: which colour?");
      expect(card(mode, quiet), mode).toContain('class="card-quiet"');
    }
  });
});

describe("board columns in each mode", () => {
  const head = (fold: "fixed" | "folded" | "opened", needs = 0, blocked = 0) =>
    renderToStaticMarkup(createElement(ColumnHead, { stage: { id: "done", label: "Done", role: "" }, count: 9, needs, blocked, fold, onFold: () => {} }));

  it("a column the mode doesn't fold heads as before", () => {
    expect(head("fixed")).toBe('<h2 title="Done">Done <small>9</small></h2>');
  });

  it("a folded column is a strip that says its name and count, and opens with a click (AC3)", () => {
    const html = head("folded");
    expect(html).toMatch(/^<h2 title="Done"><button type="button" class="col-fold" aria-expanded="false" aria-label="Done: 9 items" title="Open Done here">Done <small>9<\/small><\/button><\/h2>$/);
    expect(head("opened")).toContain('aria-expanded="true"');
  });

  it("counts only the red blocks: a wait on other tasks alone clears by itself (0060)", () => {
    const byTasks = { blocked: false, blockedKind: "tasks" };
    expect(blockedCount([byTasks, { blocked: true, blockedKind: "you" }, { blocked: false, blockedKind: "outside" }, { blocked: false, blockedKind: "" }])).toBe(2);
    expect(blockedCount([byTasks, byTasks])).toBe(0);
  });

  it("a folded column still says how many in it need you, and how many are blocked (review)", () => {
    expect(head("folded", 2)).toContain('<b class="count warn" title="2 need you">2</b>');
    // One says it as the sidebar does (mu-159).
    expect(head("folded", 1)).toContain('<b class="count warn" title="1 needs you">1</b>');
    expect(head("folded", 0, 3)).toContain('<b class="count blocked-count" title="3 blocked">3 blocked</b>');
    // A screen reader hears the counts in words, not a bare digit (mu-159).
    expect(head("folded", 1, 3)).toContain('aria-label="Done: 9 items, 1 needs you, 3 blocked"');
    expect(head("opened", 2)).toContain('aria-label="Done: 9 items, 2 need you"');
    expect(head("folded")).not.toContain("blocked");
    // An open column's cards say it themselves.
    expect(head("fixed", 2, 3)).toBe('<h2 title="Done">Done <small>9</small></h2>');
  });

  it("the board and the bands count blocked tasks for the strips", () => {
    expect(appSource).toContain("blocked={blockedCount(list)}");
    expect(groupsSource).toContain("blocked={blockedCount(list)}");
  });

  it("Focus folds the grouped board's Backlog band like the column, with its counts (review)", () => {
    const board = (opened: string[]) =>
      inMode(
        "focus",
        createElement(GroupedBoard, {
          groups: [{ head: null, items: [task("0009", { stage: "backlog", blocked: true, blockedKind: "you", waitingForOwner: true })] }],
          by: "feature", sort: "priority", multi: false, openKey: null, onOpen: () => {}, renderCard: (t: Task) => createElement("i", { key: t.id }, t.id),
          backlog: { notes: [], renderNote: () => null, quickAdd: createElement("span", null, "QUICK") },
          fold: { of: (id: string) => columnFold("now", id, new Set(opened)), toggle: () => {} },
        }),
      );
    const folded = board([]);
    expect(folded).toContain('<button type="button" class="band-fold" aria-expanded="false" aria-label="Backlog · not in a feature yet: 1 item, 1 needs you, 1 blocked"');
    expect(folded).toMatch(/<button type="button" class="band-fold" aria-expanded="false"[^>]*>Backlog · not in a feature yet <small>1 item<\/small><b class="count warn" title="1 needs you">1<\/b><b class="count blocked-count" title="1 blocked">1 blocked<\/b><\/button>/);
    expect(folded).not.toContain("QUICK");
    expect(folded).not.toContain("<i>0009</i>");
    const open = board(["backlog"]);
    expect(open).toContain('class="band-fold" aria-expanded="true"');
    expect(open).toContain("QUICK");
    expect(open).toContain("<i>0009</i>");
    // Not folded by the mode: the band as 0046 made it.
    expect(inMode("adhd", createElement(GroupedBoard, {
      groups: [], by: "feature", sort: "priority", multi: false, openKey: null, onOpen: () => {}, renderCard: () => null,
      backlog: { notes: [], renderNote: () => null, quickAdd: createElement("span", null, "QUICK") },
      fold: { of: (id: string) => columnFold("all", id, new Set()), toggle: () => {} },
    }))).toContain('<div class="band-head"><span class="band-title">Backlog · not in a feature yet</span>');
  });

  it("the board and every band fold the same columns: the mode's, and the owner's picks in it (mu-84)", () => {
    expect(appSource).toContain("of: (id) => bandFold(boardLooks, modePicks, id),");
    expect(appSource).toContain("toggle: (id) => setPicks((p) => toggleBand(p, modes.mode, boardLooks, id)),");
    expect(appSource).toContain("const plain = plainBoard(boardLooks, modePicks,");
    expect(appSource).toMatch(/<GroupedBoard[\s\S]*?fold=\{columnFoldOf\}/);
    // A column the owner hid on the plain board is folded in the bands too, in any mode.
    expect(columnFold("all", "ready", new Set(), new Set(["ready"]))).toBe("folded");
    expect(columnFold("now", "building", new Set(), new Set(["building"]))).toBe("folded");
    expect(columnFold("all", "ready", new Set(), new Set(["done"]))).toBe("fixed");
  });
});

describe("Recently finished and the Done line in each mode", () => {
  const RNOW = new Date(2026, 9, 13, 14, 0).getTime() / 1000;
  const step = (at: string, from: string, to: string, who = "muster/main (merge)") => ({ at, who, from, to });
  const done = (id: string, merged: string, accepted: string) =>
    task(id, { stage: "done", doneAt: accepted, stageHistory: [step("2026-10-13 09:00", "ready", "building", "muster/wt-x"), step(merged, "testing", "your-check"), step(accepted, "your-check", "done", "owner")] });
  const finished = [done("0043", "2026-10-13 10:00", "2026-10-13 13:56"), done("0031", "2026-10-12 09:00", "2026-10-12 09:05")];
  const rail = (mode: DisplayMode, tasks = finished, kind: KindFilter = "all") =>
    inMode(mode, createElement(Recent, { tasks, now: RNOW, kind, onKind: () => {}, multi: false, openKey: null, onOpen: () => {}, onFold: () => {} }));
  const looks = (mode: DisplayMode) => ({ columns: look("columns", mode), rail: look("rail", mode) });

  it("open, it's the rail 0051 made in every mode, with a Fold that sends it to the row (mu-84)", () => {
    for (const mode of ALL) {
      const html = rail(mode);
      expect(html, mode).toContain('class="recent-row');
      expect(html, mode).toContain('class="ui-pill rail-unfold"');
      expect(html, mode).toContain('aria-expanded="true"');
      expect(html, mode).not.toContain("rail-fold\"");
    }
  });

  it("ADHD and Standard open it beside the board while it has a week's rows; Focus folds it to the row's last chip", () => {
    for (const mode of ["adhd", "standard"] as const) {
      expect(autoOpen(looks(mode), RAIL_ID, 4), mode).toBe(true);
      expect(autoOpen(looks(mode), RAIL_ID, 0), `${mode}, nothing this week`).toBe(false);
    }
    expect(autoOpen(looks("focus"), RAIL_ID, 4)).toBe(false);
    expect(ROW_ORDER.at(-1)).toBe(RAIL_ID);
    expect(weekCount(finished, RNOW, "all")).toBe(4);
    expect(appSource).toContain("{plain.isOpen(RAIL_ID) && <Recent key={`${queue}/${modes.mode}`}");
  });

  it("its chip counts what its list shows: the remembered kind's rows, not every kind (review R2)", () => {
    const rows = (html: string) => html.match(/class="recent-row/g)?.length ?? 0;
    // Two tasks merged and accepted, one merged and waiting in Your check.
    const two = [...finished, task("0050", { stage: "your-check", stageHistory: [step("2026-10-13 09:00", "ready", "building", "muster/wt-x"), step("2026-10-13 11:00", "testing", "your-check")] })];
    for (const kind of ["all", "merged", "accepted"] as const) expect(weekCount(two, RNOW, kind), kind).toBe(rows(rail("adhd", two, kind)));
    expect([weekCount(two, RNOW, "all"), weekCount(two, RNOW, "merged"), weekCount(two, RNOW, "accepted")]).toEqual([5, 3, 2]);
    // App keeps the kind, so the chip, the list and whether the rail opens by itself use the same rows.
    expect(appSource).toContain("const [recentKind, setRecentKind] = useState<KindFilter>(() => loadKind());");
    expect(appSource).toContain("const finishedWeek = useMemo(() => weekCount(inQueue, now, recentKind), [inQueue, now, recentKind]);");
    expect(appSource).toContain("kind={recentKind} onKind={pickRecentKind}");
    expect(appSource).toMatch(/const pickRecentKind = \(k: KindFilter\) => \{\s*setRecentKind\(k\);\s*storeKind\(k\);/);
  });

  it("keeps the Done card's line in every mode: it's the card's status line", () => {
    for (const mode of ALL) expect(text(card(mode, finished[0])), mode).toContain("done");
    expect(card("focus", finished[0])).toContain('class="card-done"');
  });
});

describe("the task pane's secondary sections in each mode", () => {
  const fold = (mode: DisplayMode) => inMode(mode, createElement(Fold, { label: "Vision", summary: "1 open conflict" }, createElement("section", { className: "vision" }, "inside")));

  it("ADHD shows the section as it is; Standard and Focus fold it to a header that opens it", () => {
    // ADHD's wrapper takes no room (display: contents), so the section lays out as it always has.
    expect(fold("adhd")).toBe('<div class="fold all"><div class="fold-body"><section class="vision">inside</section></div></div>');
    for (const mode of ["standard", "focus"] as const) {
      expect(fold(mode)).toBe('<div class="fold"><button type="button" class="fold-head" aria-expanded="false">Vision <small>1 open conflict</small></button><div class="fold-body" hidden=""><section class="vision">inside</section></div></div>');
    }
    expect(css).toMatch(/\.fold\.all, \.fold\.all > \.fold-body \{ display: contents; \}/);
  });

  it("keeps the section mounted in every mode and folded or not, so typed text survives a switch (review)", () => {
    // The same elements at the same places: the header is only added in front of the body.
    const shape = (html: string) => html.replace(/<button[\s\S]*?<\/button>/, "").replace(/ class="[^"]*"| hidden=""/g, "");
    expect(new Set(ALL.map((m) => shape(fold(m))))).toEqual(new Set(["<div><div><section>inside</section></div></div>"]));
  });

  it("an opened fold hides the section's own heading, which its header already says (review)", () => {
    expect(css).toContain(".fold.open > .fold-body > section:first-child > h2:first-child { display: none; }");
  });

  it("the Log and check-ins header counts both", () => {
    const t = task("0007", { stage: "building", checkins: [{ at: "2026-10-13 04:00", who: "w", pct: 5, text: "start" }, { at: "2026-10-13 05:00", who: "w", pct: 50, text: "half" }] });
    expect(logSummary("- 2026-10-13 03:58 claimed\n- 2026-10-13 04:00 note\n\n- 2026-10-13 05:00 passed\n", t.checkins.length)).toBe("3 log lines · 2 check-ins");
    expect(logSummary("- one\n", 0)).toBe("1 log line");
    expect(logSummary("", 1)).toBe("1 check-in");
    expect(paneSource).toContain('<Fold label={t("pane.fold.log")} summary={logSummary(log && !raw && editing === null ? log : "", task.checkins.length)}>');
  });

  it("folds Vision, Review, Work, and the Log and check-ins in the pane, and leaves the write-up and comments open", () => {
    const t = task("0007", { stage: "building", checkins: [{ at: "2026-10-13 04:00", who: "w", pct: 5, text: "start" }] });
    const html = inMode("standard", createElement(TaskPanel, { task: t, now: 0, onClose: () => {}, onChanged: () => {} }));
    expect([...html.matchAll(/class="fold-head"[^>]*>([^<]*)</g)].map((m) => m[1].trim())).toEqual(["Vision", "Review", "Work", "Log and check-ins"]);
    expect(html).toContain('data-part="body"');
    expect(html).toMatch(/<div class="fold-body" hidden=""><section class="checkins">/);
    expect(inMode("adhd", createElement(TaskPanel, { task: t, now: 0, onClose: () => {}, onChanged: () => {} }))).toContain('class="checkins"');
  });
});

describe("the sidebar in each mode", () => {
  const queues: QueueInfo[] = [
    { name: "alpha", path: "/p/alpha/tasks", counts: { ready: 1 } },
    { name: "zeta", path: "/p/zeta/tasks", counts: { building: 1 } },
  ];
  const tasks = [
    task("0001", { queue: "alpha", waitingForOwner: true }),
    task("0002", { queue: "zeta", stage: "building", live: true, claimedBy: "zeta/wt-a" }),
  ];
  const activity: Activity[] = [];
  const side = (mode: DisplayMode) => inMode(mode, createElement(ProjectList, { queues, tasks, notes: [], activity, now: NOW, current: "all", onPick: () => {} }));

  it("Focus: names, the working dot and the needs-you count; no order chips or activity lines", () => {
    const html = side("focus");
    expect(html).not.toContain("side-activity");
    expect(html).not.toContain('aria-label="Order projects"');
    expect(html).toMatch(/<i class="dot side-dot" title="Being worked on now: [^"]*"><\/i><span class="side-name">zeta<\/span>/);
    expect(html).not.toMatch(/side-dot[^>]*><\/i><span class="side-name">alpha</);
    expect(appSource).toContain('const sidebarLook = look("sidebar", modes.mode);');
    expect(appSource).toContain("data-sidebar={sidebarLook}");
  });

  it("Focus still marks a stalled project, and keeps its Ask for a check-in (review)", () => {
    const stalled = [...tasks, task("0003", { queue: "zeta", stage: "building", claimedBy: "zeta/wt-b", heartbeat: stamp(NOW - 3 * 3600), stageSince: stamp(NOW - 4 * 3600) })];
    const html = inMode("focus", createElement(ProjectList, { queues, tasks: stalled.map((t) => (t.id === "0002" ? { ...t, live: false, stage: "ready", claimedBy: "" } : t)), notes: [], activity, now: NOW, current: "all", onPick: () => {} }));
    expect(html).toMatch(/<span class="side-name">zeta<\/span><span class="side-badges"><span class="side-stalled" title="Stalled: [^"]*">stalled<\/span>/);
    expect(html).toMatch(/<span class="side-name">All<\/span><span class="side-badges"><span class="side-stalled" title="1 project stalled: [^"]*">1 stalled<\/span>/);
    expect(html).toContain('class="side-ping"');
    expect(html).not.toMatch(/side-name">alpha<\/span><span class="side-badges"><span class="side-stalled/);
  });

  it("keeps the needs-you count in every mode (AC3)", () => {
    for (const mode of ALL) expect(side(mode), mode).toContain('<span class="count warn" title="1 needs you">1</span>');
    expect(side("adhd")).toContain("side-activity");
  });
});

describe("what needs you, in every mode (AC3)", () => {
  it("the header's Waiting for you count doesn't read the mode", () => {
    const tab = /<Pill on=\{view === "waiting"[\s\S]*?<\/Pill>/.exec(appSource)?.[0] ?? "";
    // The count keeps its room at zero (mu-202): HeadCount is the same in every mode.
    expect(tab).toContain("<HeadCount n={needs}");
    expect(tab).not.toMatch(/look\(|modes\./);
  });
});
