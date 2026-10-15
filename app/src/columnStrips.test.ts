import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import css from "./styles.css?raw";
import EffortGroups from "./EffortGroups";
import { GroupedBoard } from "./Groups";
import { groupItems, isWork } from "./levels";
import { STAGES } from "./lib";
import { ColumnHead } from "./ModeViews";
import { columnClass, columnFold, columnState, columnTrack, look, MODES, OPEN_COLUMN_MAX, type ColumnState, type DisplayMode } from "./modes";
import { PausedBand } from "./PausedLane";
import type { Task } from "./types";

// The board's columns (mu-83): an open column has a maximum width. The Group by bands keep its strips:
// every one, empty or folded, has one anatomy: the dot and the vertical name at the top, the
// count under them, the same padding, inside its own 44px track. The plain board has none: its
// folded and empty columns are chips in one row above it (mu-84, columnRow.test.ts).

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0001", title: "T", kind: "code", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
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
    expect(full?.[5]).toBe('<b class="count warn" title="1 needs you">1</b><b class="count blocked-count" title="2 blocked">2 blocked</b>');
    // No role line on a strip: it's for an open column's head.
    expect(head({ empty: true, children: createElement("span", { className: "role" }, "agents") })).not.toContain("role");
  });
  it("a strip's button says its counts in words to a screen reader; an empty strip has no button to name", () => {
    expect(head({ fold: "folded", count: 4, needs: 1, blocked: 2 })).toContain('<button type="button" class="col-fold" aria-expanded="false" aria-label="Shaping: 4 items, 1 needs you, 2 blocked"');
    expect(head({ fold: "folded", count: 1 })).toContain('aria-label="Shaping: 1 item"');
    expect(head({ fold: "folded" })).toContain('aria-label="Shaping: empty"');
    expect(head({ empty: true })).not.toContain("aria-label");
    expect(head({ count: 3, needs: 1 })).not.toContain("aria-label");
  });
  it("an open column's head is as it was, 0 included", () => {
    expect(head({ count: 0 })).toBe('<h2 title="Shaping: product + tech + UI agents">Shaping <small>0</small></h2>');
    expect(head({ count: 3 })).toBe('<h2 title="Shaping: product + tech + UI agents">Shaping <small>3</small></h2>');
  });
  it("a Group by band's columns are the same strips", () => {
    const items = [task({ id: "0001", stage: "ready" })];
    const html = renderToStaticMarkup(createElement(GroupedBoard, { groups: groupItems(items.filter(isWork), items, "feature"), by: "feature", sort: "priority", multi: false, openKey: null, onOpen: () => {}, renderCard: () => null }));
    expect(html).toContain('<section class="column col-shaping col-strip col-empty"><h2 title="Shaping: product + tech + UI agents"><span class="col-head">Shaping <small>empty</small></span></h2>');
    expect(html).toContain('<section class="column col-ready">');
    expect(html).toContain("grid-template-columns:44px 44px minmax(118px, 400px) 44px 44px 44px 44px");
    expect(html).not.toMatch(/class="[^"]*(?<![\w-])empty(?![\w-])[^"]*"/);
  });
  it("the folded Paused and Backlog bands title the badge \"1 needs you\" and \"2 need you\", and name their button in words (mu-159)", () => {
    const fold = { of: () => "folded" as const, toggle: () => {} };
    const waiting = (n: number) => Array.from({ length: n }, (_, i) => task({ id: `000${i + 1}`, stage: "backlog", paused: "2026-10-14 09:00", waitingForOwner: true }));
    const paused = (n: number) => renderToStaticMarkup(createElement(PausedBand, { tasks: waiting(n), renderCard: () => null, fold }));
    expect(paused(1)).toContain('<b class="count warn" title="1 needs you">1</b>');
    expect(paused(1)).toContain('aria-label="⏸︎ Paused: 1 task, 1 needs you"');
    expect(paused(2)).toContain('<b class="count warn" title="2 need you">2</b>');
    expect(paused(2)).toContain('aria-label="⏸︎ Paused: 2 tasks, 2 need you"');
    expect(renderToStaticMarkup(createElement(PausedBand, { tasks: [], renderCard: () => null, fold }))).toContain('aria-label="⏸︎ Paused: 0 tasks"');
    const backlog = (n: number) => {
      const items = waiting(n).map((t) => ({ ...t, paused: undefined }));
      return renderToStaticMarkup(createElement(GroupedBoard, {
        groups: [{ head: null, items }], by: "goal", sort: "priority", multi: false, openKey: null, onOpen: () => {}, renderCard: () => null,
        backlog: { notes: [], renderNote: () => null, quickAdd: null }, fold,
      }));
    };
    expect(backlog(1)).toContain('aria-label="Backlog · not in a goal yet: 1 item, 1 needs you"');
    expect(backlog(1)).toContain('<b class="count warn" title="1 needs you">1</b>');
    expect(backlog(2)).toContain('aria-label="Backlog · not in a goal yet: 2 items, 2 need you"');
    expect(backlog(2)).toContain('<b class="count warn" title="2 need you">2</b>');
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

describe("a strip's focus ring (review R2)", () => {
  it("is drawn inside the strip, which clips what spills past its 44px", () => {
    expect(rule(".column.col-strip")).toContain("overflow: hidden");
    // button:focus-visible's 2px ring sits 1px outside the button; on a strip it goes inside.
    expect(rule(".ui-pill:focus-visible, button:focus-visible")).toContain("outline: 2px solid var(--focus); outline-offset: 1px;");
    expect(rule(".column.col-strip .col-fold:focus-visible")).toContain("outline-offset: -3px");
  });
});
