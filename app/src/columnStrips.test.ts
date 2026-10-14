import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import css from "./styles.css?raw";
import EffortGroups from "./EffortGroups";
import { GroupedBoard } from "./Groups";
import { groupItems, isWork } from "./levels";
import { STAGES } from "./lib";
import { ColumnHead } from "./ModeViews";
import { columnClass, columnFold, columnState, columnTrack, look, MODES, OPEN_COLUMN_MAX, plainBoard, type ColumnState, type DisplayMode } from "./modes";
import appSource from "./App.tsx?raw";
import { PausedBand, PausedColumn } from "./PausedLane";
import type { Task } from "./types";

// The board's columns (mu-83): an open column has a maximum width, and every strip, empty or
// folded, the Paused lane's too, has one anatomy: the dot and the vertical name at the top, the
// count under them, the same padding, inside its own 44px track.

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0001", title: "T", kind: "code", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});
const none: ReadonlySet<string> = new Set();
/** A column's state in `mode` with `count` cards: what the board decides. */
const stateIn = (mode: DisplayMode, stage: string, count: number, keepOpen = false, opened = none) => columnState(columnFold(look("columns", mode), stage, opened), count, keepOpen);
const head = (over: Partial<Parameters<typeof ColumnHead>[0]>) =>
  renderToStaticMarkup(createElement(ColumnHead, { stage: { id: "shaping", label: "Shaping", role: "product + tech + UI agents" }, count: 0, needs: 0, blocked: 0, fold: "fixed", onFold: () => {}, ...over }));
/** A CSS rule's declarations, by its exact selector list. */
const rule = (selector: string) => {
  const at = css.indexOf(`\n${selector} {`);
  return at < 0 ? "" : css.slice(css.indexOf("{", at) + 1, css.indexOf("}", at));
};

describe("which columns are open, empty strips or folded strips, per mode", () => {
  it("ADHD and Standard never fold: an empty column stays a narrow open strip that says empty", () => {
    for (const mode of ["adhd", "standard"] as const) {
      for (const s of STAGES) {
        expect(stateIn(mode, s.id, 0), `${mode} ${s.id}`).toBe("empty");
        expect(stateIn(mode, s.id, 3), `${mode} ${s.id}`).toBe("open");
      }
      expect(stateIn(mode, "paused", 0), mode).toBe("empty");
      // Backlog keeps its quick add, Your check its Accept all line or its limit.
      expect(stateIn(mode, "backlog", 0, true), mode).toBe("open");
    }
  });
  it("Focus folds all but Building, Testing and Your check; those, empty, are the same strip", () => {
    for (const id of ["paused", "backlog", "shaping", "ready", "done"]) {
      expect(stateIn("focus", id, 0), id).toBe("folded");
      expect(stateIn("focus", id, 5), id).toBe("folded");
      expect(stateIn("focus", id, 5, false, new Set([id])), `${id} opened`).toBe("open");
      expect(stateIn("focus", id, 0, false, new Set([id])), `${id} opened, empty`).toBe("empty");
    }
    for (const id of ["building", "testing", "your-check"]) {
      expect(stateIn("focus", id, 0), id).toBe("empty");
      expect(stateIn("focus", id, 2), id).toBe("open");
    }
  });
  it("an open column is at most the max width, a strip 44px; spare room stays at the right", () => {
    expect(OPEN_COLUMN_MAX).toBeGreaterThanOrEqual(360);
    expect(OPEN_COLUMN_MAX).toBeLessThanOrEqual(420);
    expect(columnTrack("open")).toBe(`minmax(118px, ${OPEN_COLUMN_MAX}px)`);
    expect(columnTrack("empty")).toBe("44px");
    expect(columnTrack("folded")).toBe("44px");
    for (const s of ["open", "empty", "folded"] as ColumnState[]) expect(columnTrack(s)).not.toMatch(/fr\b/);
  });
  it("names each state by a class nothing else uses: never the empty message's .empty", () => {
    expect(columnClass("ready", "open")).toBe("column col-ready");
    expect(columnClass("shaping", "empty")).toBe("column col-shaping col-strip col-empty");
    expect(columnClass("done", "folded")).toBe("column col-done col-strip folded");
    // The empty message (No tasks match) is padded 32px and centred; a column that took it grew
    // past its 44px track into its neighbours (mu-83).
    expect(rule(".empty")).toContain("padding: 32px");
    for (const s of ["open", "empty", "folded"] as ColumnState[]) expect(columnClass("x", s).split(" "), s).not.toContain("empty");
  });
});

describe("one strip anatomy", () => {
  /** The strip's head: its h2, then one button or span holding the name, the count and the flags. */
  const STRIP = /^<h2 title="[^"]+"><(button|span) (?:type="button" )?class="(col-fold|col-head)"[^>]*>([^<]+) <small>([^<]+)<\/small>(.*)<\/\1><\/h2>$/;
  it("an empty strip and a folded one have the same parts in the same order", () => {
    const empty = STRIP.exec(head({ empty: true }));
    const folded = STRIP.exec(head({ fold: "folded" }));
    const full = STRIP.exec(head({ fold: "folded", count: 4, needs: 1, blocked: 2 }));
    expect(empty?.slice(2, 6)).toEqual(["col-head", "Shaping", "empty", ""]);
    expect(folded?.slice(2, 6)).toEqual(["col-fold", "Shaping", "empty", ""]);
    expect(full?.slice(2, 5)).toEqual(["col-fold", "Shaping", "4"]);
    expect(full?.[5]).toBe('<b class="count warn" title="1 need you">1</b><b class="count blocked-count" title="2 blocked">2 blocked</b>');
    // No role line on a strip: it's for an open column's head.
    expect(head({ empty: true, children: createElement("span", { className: "role" }, "agents") })).not.toContain("role");
  });
  it("an open column's head is as it was, 0 included", () => {
    expect(head({ count: 0 })).toBe('<h2 title="Shaping: product + tech + UI agents">Shaping <small>0</small></h2>');
    expect(head({ count: 3 })).toBe('<h2 title="Shaping: product + tech + UI agents">Shaping <small>3</small></h2>');
  });
  it("the Paused lane's strips are the same", () => {
    const empty = renderToStaticMarkup(createElement(PausedColumn, { tasks: [], state: "empty", fold: "fixed", onFold: () => {}, renderCard: () => null }));
    expect(empty).toMatch(/^<section class="column col-paused col-strip col-empty"><h2/);
    expect(STRIP.exec(/<h2[\s\S]*?<\/h2>/.exec(empty)![0])?.slice(2, 5)).toEqual(["col-head", "⏸︎ Paused", "empty"]);
    const folded = renderToStaticMarkup(createElement(PausedColumn, { tasks: [task({ paused: "x" })], state: "folded", fold: "folded", onFold: () => {}, renderCard: () => null }));
    expect(folded).toMatch(/^<section class="column col-paused col-strip folded"><h2/);
    expect(STRIP.exec(/<h2[\s\S]*?<\/h2>/.exec(folded)![0])?.slice(2, 5)).toEqual(["col-fold", "⏸︎ Paused", "1"]);
  });
  it("a Group by band's columns are the same strips", () => {
    const items = [task({ id: "0001", stage: "ready" })];
    const html = renderToStaticMarkup(createElement(GroupedBoard, { groups: groupItems(items.filter(isWork), items, "feature"), by: "feature", sort: "priority", multi: false, openKey: null, onOpen: () => {}, renderCard: () => null }));
    expect(html).toContain('<section class="column col-shaping col-strip col-empty"><h2 title="Shaping: product + tech + UI agents"><span class="col-head">Shaping <small>empty</small></span></h2>');
    expect(html).toContain('<section class="column col-ready">');
    expect(html).toContain("grid-template-columns:44px 44px minmax(118px, 400px) 44px 44px 44px 44px");
    expect(html).not.toMatch(/class="[^"]*(?<![\w-])empty(?![\w-])[^"]*"/);
  });
  it("the Paused band and Your check's effort groups don't take the empty message's class either", () => {
    expect(renderToStaticMarkup(createElement(PausedBand, { tasks: [], renderCard: () => null }))).toMatch(/^<section class="band paused-band paused-none">/);
    const groups = renderToStaticMarkup(createElement(EffortGroups, { tasks: [task({ id: "0014", stage: "your-check" })], place: "board", renderCard: () => null, onChanged: () => {} }));
    expect(groups).toContain("effort-group effort-look effort-empty");
    expect(groups).not.toMatch(/class="[^"]*(?<![\w-])empty(?![\w-])[^"]*"/);
  });
  it("in CSS: one head rule for both kinds, the dot in the head, and a strip can't spill out of its track", () => {
    const shared = rule(".column.col-strip .col-fold, .column.col-strip .col-head");
    expect(shared).toContain("writing-mode: vertical-rl");
    expect(shared).toContain("padding: 12px 4px");
    expect(rule(".column.col-strip .col-fold::before, .column.col-strip .col-head::before")).toContain("background: var(--status)");
    expect(rule(".column.col-strip h2::before")).toContain("content: none");
    const strip = rule(".column.col-strip");
    expect(strip).toContain("min-width: 0");
    expect(strip).toContain("overflow: hidden");
    // Neighbours keep apart: the board's gap, never a negative margin pulling them together.
    expect(rule(".board")).toContain("gap: 10px");
    expect(css).not.toMatch(/\.column[^{]*\{[^}]*margin-(left|right): -/);
    // The old per-kind rules are gone, so the two kinds can't drift apart again.
    expect(css).not.toMatch(/\.column\.empty\b/);
    expect(css).not.toMatch(/\.column\.folded h2 \{/);
  });
});

describe("every mode draws the same anatomy", () => {
  it("in ADHD, Standard and Focus", () => {
    for (const { id } of MODES) {
      const state = stateIn(id, "shaping", 0);
      const html = head({ empty: state === "empty", fold: columnFold(look("columns", id), "shaping", none) });
      expect(html, id).toMatch(/<(span|button)[^>]*class="(col-head|col-fold)"[^>]*>Shaping <small>empty<\/small>/);
    }
  });
});

describe("the plain project board (review R1)", () => {
  /** The braindump board the owner sent: Ready 66, Done 1, the rest empty; Backlog kept open. */
  const counts: Record<string, number> = { ready: 66, done: 1 };
  const board = (mode: DisplayMode) => plainBoard((id) => columnFold(look("columns", mode), id, none), (id) => counts[id] ?? 0, (id) => id === "backlog");
  it("caps every open column and draws every empty one as a strip, in its order, Paused first", () => {
    const max = `minmax(118px, ${OPEN_COLUMN_MAX}px)`;
    for (const mode of ["adhd", "standard"] as const) {
      expect(board(mode).tracks, mode).toBe(["44px", max, "44px", max, "44px", "44px", "44px", max].join(" "));
      expect(board(mode).className("shaping"), mode).toBe("column col-shaping col-strip col-empty");
      expect(board(mode).className("paused"), mode).toBe("column col-paused col-strip col-empty");
      expect(board(mode).className("ready"), mode).toBe("column col-ready");
    }
    const focus = board("focus");
    expect(focus.tracks).toBe(Array(8).fill("44px").join(" "));
    expect(focus.className("ready")).toBe("column col-ready col-strip folded");
    expect(focus.className("building")).toBe("column col-building col-strip col-empty");
    expect(focus.state("paused")).toBe("folded");
  });
  it("is what App draws: its grid, the Paused lane's state and each column's classes come from it", () => {
    expect(appSource).toContain("const plain = plainBoard(");
    expect(appSource).toContain('<main className="board" style={{ gridTemplateColumns: plain.tracks }}>');
    expect(appSource).toContain('state={plain.state("paused")}');
    expect(appSource).toContain("<section key={c.id} className={plain.className(c.id)}>");
    const board = appSource.slice(appSource.indexOf('{groupBy === "none" && <main className="board"'), appSource.indexOf("<Recent "));
    expect(board).not.toMatch(/\b1fr\b|" empty"/);
  });
});

describe("a strip's focus ring (review R2)", () => {
  it("is drawn inside the strip, which clips what spills past its 44px", () => {
    expect(rule(".column.col-strip")).toContain("overflow: hidden");
    // button:focus-visible's 2px ring sits 1px outside the button; on a strip it goes inside.
    expect(rule(".ui-pill:focus-visible, button:focus-visible")).toContain("outline: 2px solid var(--focus); outline-offset: 1px;");
    expect(rule(".column.col-strip .col-fold:focus-visible")).toContain("outline-offset: -3px");
  });
});
