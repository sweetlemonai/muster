import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import appSource from "./App.tsx?raw";
import columnRowSource from "./ColumnRow.tsx?raw";
import ColumnRow, { BoardCue, chipWords, edgeColumnsOf, placeCount, type PlaceCounts, type RowChip, RowCue, rowChipsOf } from "./ColumnRow";
import { tn } from "./i18n";
import { setLocale } from "./i18n/runtime";
import pausedSource from "./PausedLane.tsx?raw";
import rowCueSource from "./rowCue.ts?raw";
import { columns, STAGES } from "./lib";
import { ColumnHead } from "./ModeViews";
import {
  autoOpen, bandFold, COLUMNS_KEY, loadPicks, look, MODES, modeFolds, OPEN_COLUMN_MAX, pickColumn, plainBoard, RAIL_ID, ROW_ORDER, storePicks, toggleBand,
  type BoardLooks, type ColumnPicks, type DisplayMode, type ModePicks,
} from "./modes";
import { ALL_IN_VIEW, type ChipSpan, cueWords, cutBy, inView, leastRoom, measureRow, overflowCue, sameView, watchRow } from "./rowCue";
import css from "./styles.css?raw";
import type { Task } from "./types";

// The row of folded columns (mu-84, the owner's choice A): every closed, folded, empty or hidden
// place of the plain board is a chip in one row above it, in board order; a chip opens its column
// in place, an open column's Fold sends it back, and each mode remembers its own choices on this Mac.

const looks = (mode: DisplayMode): BoardLooks => ({ columns: look("columns", mode), rail: look("rail", mode) });
/** The braindump board the owner sent (mu-83): Ready 66, Done 1, the rest empty, 129 finished this
 *  week; Backlog kept open for its quick add. */
const COUNTS: Record<string, number> = { ready: 66, done: 1, [RAIL_ID]: 129 };
const board = (mode: DisplayMode, picks: ModePicks = {}, counts = COUNTS) => plainBoard(looks(mode), picks, (id) => counts[id] ?? 0, (id) => id === "backlog");
const store = (items: Record<string, string> = {}) => ({
  items,
  getItem: (k: string) => items[k] ?? null,
  setItem: (k: string, v: string) => {
    items[k] = v;
  },
});
const row = (chips: RowChip[]) => renderToStaticMarkup(createElement(ColumnRow, { chips, onOpen: () => {} }));
const chip = (id: string, count = 0, needs = 0, blocked = 0): RowChip => ({ id, label: STAGES.find((s) => s.id === id)?.label ?? id, count, needs, blocked });
const max = `minmax(118px, ${OPEN_COLUMN_MAX}px)`;
const task = (id: string, over: Partial<Task> = {}): Task => ({
  queue: "muster", id, title: `Task ${id}`, kind: "code", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: `todo/${id}-t.md`, claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", search: "", ...over,
});

describe("which places are open and which are chips (AC1)", () => {
  it("ADHD and Standard: the columns with tasks open, every empty one a chip, in board order", () => {
    for (const mode of ["adhd", "standard"] as const) {
      const b = board(mode);
      expect(b.open, mode).toEqual(["backlog", "ready", "done"]);
      expect(b.row, mode).toEqual(["paused", "shaping", "building", "testing", "your-check"]);
      expect(b.tracks, mode).toBe([max, max, max].join(" "));
      expect(b.isOpen(RAIL_ID), mode).toBe(true);
    }
  });
  it("Focus: Building, Testing and Your check open when they hold tasks, everything else a chip", () => {
    const b = board("focus", {}, { ...COUNTS, building: 2, testing: 0 });
    expect(b.open).toEqual(["building"]);
    expect(b.row).toEqual(["paused", "backlog", "shaping", "ready", "testing", "your-check", "done", RAIL_ID]);
    expect(b.tracks).toBe(max);
  });
  it("an empty column goes to the row in every mode, and a kept-open one (Backlog's quick add) doesn't", () => {
    for (const { id } of MODES) {
      for (const s of STAGES) if (!modeFolds(looks(id), s.id)) expect(autoOpen(looks(id), s.id, 0), `${id} ${s.id}`).toBe(false);
      expect(autoOpen(looks(id), "paused", 0), id).toBe(false);
      expect(autoOpen(looks(id), RAIL_ID, 0), id).toBe(false);
    }
    expect(autoOpen(looks("adhd"), "backlog", 0, true)).toBe(true);
    expect(autoOpen(looks("focus"), "backlog", 0, true)).toBe(false);
  });
  it("a hidden full column and a shown empty one in the same mode swap places (review R6)", () => {
    // Standard, the braindump board: Ready (66) hidden, Shaping (empty) shown.
    const b = board("standard", { ready: "hidden", shaping: "shown" });
    expect(b.open).toEqual(["backlog", "shaping", "done"]);
    expect(b.row).toEqual(["paused", "ready", "building", "testing", "your-check"]);
    expect(b.isOpen(RAIL_ID)).toBe(true);
    // Focus: Ready shown although Focus folds it, Building hidden although it holds tasks.
    const f = board("focus", { ready: "shown", building: "hidden" }, { ...COUNTS, building: 2 });
    expect(f.open).toEqual(["ready"]);
    expect(f.row).toEqual(["paused", "backlog", "shaping", "building", "testing", "your-check", "done", RAIL_ID]);
    expect(f.tracks).toBe(max);
  });
  it("with everything folded, the grid has one track for the line that says so", () => {
    const hidden: ModePicks = Object.fromEntries(ROW_ORDER.map((id) => [id, "hidden"]));
    const b = board("adhd", hidden);
    expect(b.open).toEqual([]);
    expect(b.row).toEqual([...ROW_ORDER]);
    expect(b.tracks).toBe("minmax(0, 1fr)");
    expect(appSource).toContain('{plain.open.length === 0 && <p className="none">{t("app.allFolded")}</p>}');
    expect(tn("shell", "app.allFolded")).toBe("Every column is folded into the row above: click one to open it.");
  });
  it("App draws only the open columns, never a strip, and the row under the filter bar", () => {
    const plain = appSource.slice(appSource.indexOf('{!grid && groupBy === "none" && <main ref={setBoardEl} className="board"'), appSource.indexOf("</main>}", appSource.indexOf('{!grid && groupBy === "none" && <main ref={setBoardEl} className="board"')));
    expect(plain).toContain('<main ref={setBoardEl} className="board" style={{ gridTemplateColumns: plain.tracks }}>');
    expect(plain).toContain('{plain.isOpen("paused") && <PausedColumn');
    expect(plain).toContain("{STAGES.filter((c) => plain.isOpen(c.id)).map((c) => {");
    expect(plain).toContain('className={columnClass(c.id, "open")}');
    expect(plain).not.toMatch(/col-strip|empty=\{|"folded"|"empty"/);
    const at = (s: string) => appSource.indexOf(s);
    expect(at("<ColumnRow key=")).toBeGreaterThan(at("<FilterBar"));
    expect(at("<ColumnRow key=")).toBeLessThan(at('<div className={railSide ? "board-row" : "board-row stacked"}>'));
    // With Group by the bands fold their own columns: only Recently finished goes in the row.
    expect(appSource).toContain('const rowChips: RowChip[] = rowChipsOf(plain.row, groupBy !== "none", placeCounts);');
  });
});

describe("the row as drawn (AC1, AC3)", () => {
  const chips = [
    { id: "paused", label: "⏸︎ Paused", count: 0, needs: 0, blocked: 0 },
    chip("backlog", 16, 2, 1),
    chip("shaping"),
    { id: RAIL_ID, label: "Recently finished", count: 129, needs: 0, blocked: 0 },
  ];
  it("is a labelled group of buttons, one per chip, in the order given", () => {
    const html = row(chips);
    expect(html).toMatch(/^<div class="col-row" role="group" aria-label="Folded and empty columns"><div class="col-row-chips">/);
    const buttons = [...html.matchAll(/<button type="button" aria-expanded="false" aria-label="([^"]+)" title="Open [^"]+ here" data-chip="([\w-]+)" class="ui-pill col-chip col-([\w-]+)">/g)];
    expect(buttons.map((m) => m[3])).toEqual(buttons.map((m) => m[2]));
    expect(buttons.map((m) => m[2])).toEqual(["paused", "backlog", "shaping", RAIL_ID]);
    expect(buttons.map((m) => m[1])).toEqual([
      "⏸︎ Paused: empty. Open it",
      "Backlog: 16 items, 2 need you, 1 blocked. Open it",
      "Shaping: empty. Open it",
      "Recently finished: 129 items. Open it",
    ]);
  });
  it("each chip shows its name and count in words, and needs you and blocked only when there are any", () => {
    const html = row([chip("backlog", 16, 2, 1)]);
    expect(html).toContain('<span class="col-chip-name">Backlog</span> <small>16</small><b class="count warn">2 need you</b><b class="count blocked-count">1 blocked</b>');
    const quiet = row([chip("shaping")]);
    expect(quiet).toContain('<span class="col-chip-name">Shaping</span> <small>0</small></button>');
    expect(quiet).not.toContain("count warn");
    expect(chipWords(chip("ready", 1, 1))).toBe("Ready: 1 item, 1 needs you. Open it");
  });
  it("keeps its line with nothing folded, so the board never moves up or down", () => {
    expect(row([])).toBe('<div class="col-row" role="group" aria-label="Folded and empty columns"><span class="col-row-none">Every column is open</span></div>');
    expect(css).toMatch(/\.col-row \.col-chip small \{[^}]*tabular-nums/);
  });
  it("is always one line of a fixed height: it never wraps, so a poll changing a count never moves the board (review R1)", () => {
    const rule = (sel: string) => css.match(new RegExp(`\\n${sel.replace(/[.>*]/g, (c) => `\\${c}`)} \\{([^}]*)\\}`))?.[1] ?? "";
    const rowRule = rule(".col-row");
    expect(rowRule).toContain("flex-wrap: nowrap;");
    expect(rowRule).toMatch(/(^|; )height: 34px;/);
    expect(rowRule).not.toMatch(/min-height|max-height/);
    // Past its room the chips scroll sideways in a list of their own, never down; the cue after
    // the list stays put (R7).
    const listRule = rule(".col-row-chips");
    // The list takes its chips' width, no more: the room after them is the board cue's (mu-198).
    expect(listRule).toContain("flex: 0 1 auto; min-width: 0;");
    expect(listRule).toContain("flex-wrap: nowrap;");
    expect(listRule).toContain("overflow-x: auto; overflow-y: hidden;");
    expect(rowRule).not.toContain("overflow");
    // The one wrap: too narrow for both cues, the board's has a line of its own, set by the row's
    // width alone (a container query), never by a count (mu-198, review R1).
    const wraps = [...css.matchAll(/\n\s*(\.col-row[^{]*)\{[^}]*flex-wrap: wrap/g)].map((m) => m[1].trim());
    expect(wraps).toEqual([".col-row:has(> .col-edge-slot)"]);
    expect(css).toMatch(/@container colrow \(width < [\d.]+em\) \{\n {2}\.col-row:has\(> \.col-edge-slot\) \{ flex-wrap: wrap;/);
    // Short of room only the names shorten, to an ellipsis on one line, never under 2.5em; the
    // counts and the needs-you and blocked words keep their width.
    expect(rule(".col-row .col-chip > *")).toBe(" flex: none; ");
    const name = rule(".col-row .col-chip .col-chip-name");
    expect(name).toContain("flex: 0 1 auto; min-width: 2.5em;");
    expect(name).toContain("-webkit-line-clamp: 1;");
    // The clamp may break a name anywhere, so it keeps as much as fits ("Recently fini…"), never
    // a whole word less with an empty gap ("Recently…", "⏸︎…"): QA, round 3.
    expect(name).toContain("word-break: break-all;");
    expect(name).not.toContain("overflow-wrap");
    expect(name).toContain("overflow: hidden;");
    // The chip keeps its automatic minimum (its counts and a short name): no min-width of its own.
    expect(rule(".col-row .col-chip")).toContain("flex: 0 1 auto;");
    expect(rule(".col-row .col-chip")).not.toContain("min-width");
    // The full name stays in the chip's label and title.
    expect(row([chip("your-check", 3)])).toContain('aria-label="Your check: 3 items. Open it" title="Open Your check here"');
  });
  it("under Group by an empty row says nothing: the bands below fold their own columns (review R5)", () => {
    const grouped = (chips: RowChip[]) => renderToStaticMarkup(createElement(ColumnRow, { chips, grouped: true, onOpen: () => {} }));
    expect(grouped([])).toBe('<div class="col-row" role="group" aria-label="Folded and empty columns"></div>');
    expect(grouped([chip(RAIL_ID, 4)])).toContain("col-chip col-recent");
    expect(appSource).toContain('<ColumnRow key={`${queue}/${modes.mode}`} chips={grid ? rowChipsOf(plain.row, true, placeCounts) : rowChips} grouped={grid || groupBy !== "none"} onOpen={(id) => setColumnOpen(id, true)} board={!grid && groupBy === "none" ? boardEl : null} columns={edgeColumnsOf(plain.open, cols)} />');
  });
  it("mirrors in right to left: flex from the start, no left or right of its own", () => {
    const rules = [...css.matchAll(/\n(\.col-row[^{]*)\{([^}]*)\}/g)].map((m) => m[2]);
    expect(rules.length).toBeGreaterThan(3);
    for (const r of rules) expect(r).not.toMatch(/\b(left|right)\b|margin-(left|right)|padding-(left|right)/);
    expect(css).toMatch(/\.column h2 \.col-hide \{[^}]*float: inline-end[^}]*margin-inline-start/);
  });
  it("gives each stage chip its stage colour dot, and Paused and Recently finished none", () => {
    expect(css).toMatch(/\.col-row \.col-chip::before \{[^}]*background: var\(--status\)/);
    expect(css).toContain(".col-row .col-chip.col-paused::before, .col-row .col-chip.col-recent::before { content: none; }");
  });
});

describe("opening and folding (AC2)", () => {
  it("a chip's click opens its column, a head's Fold sends it back", () => {
    expect(appSource).toContain('<ColumnRow key={`${queue}/${modes.mode}`} chips={grid ? rowChipsOf(plain.row, true, placeCounts) : rowChips} grouped={grid || groupBy !== "none"} onOpen={(id) => setColumnOpen(id, true)} board={!grid && groupBy === "none" ? boardEl : null} columns={edgeColumnsOf(plain.open, cols)} />');
    expect(appSource).toContain("const hide = () => setColumnOpen(c.id, false);");
    expect(appSource).toContain('<PausedColumn tasks={cols.paused} onHide={() => setColumnOpen("paused", false)}');
    expect(appSource).toContain("onFold={() => setColumnOpen(RAIL_ID, false)}");
    expect(appSource).toContain("const setColumnOpen = (id: string, isOpen: boolean) => setPicks((p) => pickColumn(p, modes.mode, id, isOpen, plain.auto(id)));");
  });
  it("an open column's head has the Fold, as a button that says what it does", () => {
    const html = renderToStaticMarkup(createElement(ColumnHead, { stage: { id: "ready", label: "Ready", role: "the code agent" }, count: 3, needs: 0, blocked: 0, fold: "fixed", onFold: () => {}, onHide: () => {} }));
    expect(html).toBe('<h2 title="Ready: the code agent">Ready <small>3</small><button type="button" aria-expanded="true" aria-label="Fold Ready into the row above the board" title="Fold Ready into the row above the board" class="ui-pill col-hide">Fold</button></h2>');
  });
  it("opening an empty column keeps it open; folding it again forgets the pick", () => {
    const b = board("adhd");
    let picks: ColumnPicks = pickColumn({}, "adhd", "shaping", true, b.auto("shaping"));
    expect(picks).toEqual({ adhd: { shaping: "shown" } });
    expect(board("adhd", picks.adhd).open).toEqual(["backlog", "shaping", "ready", "done"]);
    picks = pickColumn(picks, "adhd", "shaping", false, b.auto("shaping"));
    expect(picks).toEqual({});
  });
  it("hiding a full column puts it in the row until it's opened again", () => {
    let picks = pickColumn({}, "standard", "ready", false, board("standard").auto("ready"));
    expect(picks).toEqual({ standard: { ready: "hidden" } });
    expect(board("standard", picks.standard).row).toContain("ready");
    picks = pickColumn(picks, "standard", "ready", true, board("standard", picks.standard).auto("ready"));
    expect(picks).toEqual({});
  });
  it("in Focus, opening a folded column keeps it open; folding it puts it back to Focus's own fold", () => {
    let picks = pickColumn({}, "focus", "ready", true, board("focus").auto("ready"));
    expect(picks).toEqual({ focus: { ready: "shown" } });
    expect(board("focus", picks.focus).open).toEqual(["ready"]);
    picks = pickColumn(picks, "focus", "ready", false, board("focus", picks.focus).auto("ready"));
    expect(picks).toEqual({});
  });
  it("each mode keeps its own picks: one mode's choice doesn't touch another's", () => {
    const picks = pickColumn(pickColumn({}, "adhd", "ready", false, true), "focus", "done", true, false);
    expect(picks).toEqual({ adhd: { ready: "hidden" }, focus: { done: "shown" } });
    expect(board("standard", picks.standard).open).toEqual(["backlog", "ready", "done"]);
    expect(appSource).toContain("const modePicks = picks[modes.mode] ?? NO_PICKS;");
  });
  it("is remembered on this Mac: stored and read back", () => {
    const s = store();
    const picks: ColumnPicks = { adhd: { ready: "hidden", [RAIL_ID]: "hidden" }, focus: { backlog: "shown" } };
    storePicks(s, picks);
    expect(JSON.parse(s.items[COLUMNS_KEY])).toEqual(picks);
    expect(loadPicks(s)).toEqual(picks);
    expect(appSource).toContain("const [p, setP] = useState<ColumnPicks>(() => loadPicks(localStorage));");
    expect(appSource).toContain("useEffect(() => storePicks(localStorage, p), [p]);");
  });
  it("reads nothing from a missing or broken value, and drops unknown modes, places and picks", () => {
    expect(loadPicks(store())).toEqual({});
    expect(loadPicks(store({ [COLUMNS_KEY]: "{not json" }))).toEqual({});
    expect(loadPicks(store({ [COLUMNS_KEY]: "[]" }))).toEqual({});
    expect(loadPicks(store({ [COLUMNS_KEY]: "null" }))).toEqual({});
    expect(loadPicks(store({ [COLUMNS_KEY]: JSON.stringify({ compact: { ready: "hidden" }, adhd: { ready: "open", nowhere: "hidden", done: "shown" }, focus: [] }) }))).toEqual({ adhd: { done: "shown" } });
  });
  it("switching modes keeps each mode's picks: nothing is reset", () => {
    expect(appSource).not.toContain("setOpened(");
    expect(appSource).toMatch(/const changeMode = useCallback\(\s*\(f: \(s: ModeState\) => ModeState\) => \{\s*setModes\(f\);\s*setBarOut\(false\);\s*\}/);
  });
});

describe("each chip's figures, as App builds them (review R3)", () => {
  // A Focus board: two paused (one waiting on the owner, one blocked), two in Backlog with three
  // notes, one in Ready waiting on the owner and blocked, two in Building, and 6 finished this week.
  const tasks = [
    task("0001", { stage: "building", paused: "2026-10-14 09:00", waitingForOwner: true }),
    task("0002", { stage: "testing", paused: "2026-10-14 09:00", blocked: true }),
    task("0003", { stage: "backlog" }),
    task("0004", { stage: "backlog" }),
    task("0005", { stage: "ready", waitingForOwner: true, blocked: true }),
    task("0006", { stage: "building" }),
    task("0007", { stage: "building" }),
  ];
  const counts: PlaceCounts = { cols: columns(tasks), notes: 3, finished: 6 };
  const focus = plainBoard(looks("focus"), {}, (id) => placeCount(id, counts), (id) => id === "backlog");
  const chips = rowChipsOf(focus.row, false, counts);
  const of = (id: string) => chips.find((c) => c.id === id);

  it("the folded Paused lane: its count and what needs you and is blocked", () => {
    expect(of("paused")).toEqual({ id: "paused", label: "⏸︎ Paused", count: 2, needs: 1, blocked: 1 });
    expect(row(chips)).toContain('aria-label="⏸︎ Paused: 2 items, 1 needs you, 1 blocked. Open it"');
  });
  it("Backlog counts its tasks and its notes, as its open head does", () => {
    expect(of("backlog")).toEqual({ id: "backlog", label: "Backlog", count: 5, needs: 0, blocked: 0 });
    expect(appSource).toContain("const count = placeCount(c.id, placeCounts);");
  });
  it("a stage column counts its tasks, what needs you and what's blocked", () => {
    expect(of("ready")).toEqual({ id: "ready", label: "Ready", count: 1, needs: 1, blocked: 1 });
    expect(of("shaping")).toEqual({ id: "shaping", label: "Shaping", count: 0, needs: 0, blocked: 0 });
  });
  it("Recently finished counts the week its list shows, last in the row", () => {
    expect(chips.at(-1)).toEqual({ id: RAIL_ID, label: "Recently finished", count: 6, needs: 0, blocked: 0 });
  });
  it("holds every place the row holds, in board order; Building is open, not a chip", () => {
    expect(chips.map((c) => c.id)).toEqual(["paused", "backlog", "shaping", "ready", "testing", "your-check", "done", RAIL_ID]);
    expect(focus.open).toEqual(["building"]);
  });
  it("with Group by, only Recently finished", () => {
    expect(rowChipsOf(focus.row, true, counts).map((c) => c.id)).toEqual([RAIL_ID]);
  });
});

describe("the grouped board's fold click (review R4)", () => {
  it("in Focus, a folded band column opens and is remembered; clicked again, the pick is forgotten", () => {
    let picks: ColumnPicks = {};
    expect(bandFold(looks("focus"), {}, "ready")).toBe("folded");
    picks = toggleBand(picks, "focus", looks("focus"), "ready");
    expect(picks).toEqual({ focus: { ready: "shown" } });
    expect(bandFold(looks("focus"), picks.focus ?? {}, "ready")).toBe("opened");
    picks = toggleBand(picks, "focus", looks("focus"), "ready");
    expect(picks).toEqual({});
  });
  it("in Focus, a column it keeps open folds on a click and opens again on the next", () => {
    let picks = toggleBand({}, "focus", looks("focus"), "building");
    expect(picks).toEqual({ focus: { building: "hidden" } });
    expect(bandFold(looks("focus"), picks.focus ?? {}, "building")).toBe("folded");
    picks = toggleBand(picks, "focus", looks("focus"), "building");
    expect(picks).toEqual({});
  });
  it("in ADHD, a column hidden on the plain board is folded in the bands; a click clears the hide", () => {
    const picks: ColumnPicks = { adhd: { ready: "hidden" }, focus: { done: "shown" } };
    expect(bandFold(looks("adhd"), picks.adhd ?? {}, "ready")).toBe("folded");
    expect(toggleBand(picks, "adhd", looks("adhd"), "ready")).toEqual({ focus: { done: "shown" } });
    expect(bandFold(looks("adhd"), {}, "ready")).toBe("fixed");
  });
});

// Past its room the row scrolls (R1), and nothing said so: a chip past the edge took its count and
// its needs-you out of sight (review R7). The spans below are what headless Chromium measured for
// ColumnRow with the committed styles.css, the reviewer's counts, in Focus at 690px: the row 690
// wide with 28px of padding, the cue 69px (156px with needs you) and 6px of gap after the list.
describe("past the row's room: the cue at its end (review R7)", () => {
  const p = (id: string, label: string, count: number, needs = 0, blocked = 0): RowChip => ({ id, label, count, needs, blocked });
  /** Focus's six places with the reviewer's counts. */
  const FOCUS6 = [p("paused", "⏸︎ Paused", 2, 1, 1), p("backlog", "Backlog", 16), p("shaping", "Shaping", 3, 1), p("ready", "Ready", 66, 0, 2), p("done", "Done", 129), p(RAIL_ID, "Recently finished", 12)];
  /** Every place in the row (ADHD or Standard with columns hidden): Ready, Building and Your check
   *  need you too. */
  const ALL9 = [FOCUS6[0], FOCUS6[1], FOCUS6[2], p("ready", "Ready", 66, 2, 2), p("building", "Building", 4, 1, 1), p("testing", "Testing", 2), p("your-check", "Your check", 5, 3), FOCUS6[4], FOCUS6[5]];
  const spans = (ids: string[], edges: [number, number][], shift = 0): ChipSpan[] => ids.map((id, i) => ({ id, start: edges[i][0] - shift, end: edges[i][1] - shift }));
  const FOCUS6_AT = spans(FOCUS6.map((c) => c.id), [[0, 217], [223, 308], [314, 474], [480, 637], [643, 734], [740, 811]]);
  const ALL9_AT: [number, number][] = [[0, 217], [223, 308], [314, 474], [480, 714], [720, 950], [956, 1035], [1041, 1196], [1202, 1293], [1299, 1370]];

  it("Focus at 690px: Ready, Done and Recently finished are past the edge, and the cue says so", () => {
    const visible = inView(FOCUS6_AT, 0, 587);
    expect(visible).toEqual(["paused", "backlog", "shaping"]);
    const cue = overflowCue(FOCUS6, visible);
    expect(cue).toEqual({ more: 3, needs: 0, target: "ready" });
    expect(cueWords(cue!)).toBe("3 more columns past the edge. Show them");
    expect(renderToStaticMarkup(createElement(RowCue, { cue: cue!, onShow: () => {} }))).toBe(
      '<button type="button" aria-label="3 more columns past the edge. Show them" title="Show the columns past the edge" class="ui-pill col-cue"><span>+3 more</span></button>',
    );
  });
  it("a hidden chip's needs you is summed into the cue, and its click goes there first", () => {
    // Every place at 690px: the list is 500px wide beside the 156px cue.
    const visible = inView(spans(ALL9.map((c) => c.id), ALL9_AT), 0, 500);
    expect(visible).toEqual(["paused", "backlog", "shaping"]);
    const cue = overflowCue(ALL9, visible)!;
    expect(cue).toEqual({ more: 6, needs: 6, target: "ready" });
    expect(cueWords(cue)).toBe("6 more columns past the edge, 6 need you. Show them");
    expect(renderToStaticMarkup(createElement(RowCue, { cue, onShow: () => {} }))).toContain('<span>+6 more</span><span aria-hidden="true">·</span><b class="count warn">6 need you</b></button>');
    // One hidden chip that needs you, past the one that doesn't.
    const one = overflowCue([p("a", "A", 1), p("b", "B", 1), p("c", "C", 1, 1)], ["a"])!;
    expect(one).toEqual({ more: 2, needs: 1, target: "c" });
    expect(cueWords(one)).toBe("2 more columns past the edge, 1 needs you. Show them");
    expect(cueWords({ more: 1, needs: 0, target: "c" })).toBe("1 more column past the edge. Show it");
  });
  it("its clicks walk on to the far end and then back to the start, so every chip comes into view", () => {
    const ids = ALL9.map((c) => c.id);
    // Click 1 scrolled Ready to the start (scrollLeft 480): Ready and Building in view.
    const one = overflowCue(ALL9, inView(spans(ids, ALL9_AT, 480), 0, 500))!;
    expect(one).toEqual({ more: 7, needs: 5, target: "your-check" });
    // Click 2 reached the end (scrollLeft 870): only what was scrolled past is hidden, so the
    // next click goes back to the first of those that needs you.
    const end = inView(spans(ids, ALL9_AT, 870), 0, 500);
    expect(end).toEqual(["testing", "your-check", "done", RAIL_ID]);
    expect(overflowCue(ALL9, end)).toEqual({ more: 5, needs: 5, target: "paused" });
    // Nothing past the far end and nothing needing you: back to the first chip.
    expect(overflowCue(FOCUS6.slice(1), ["shaping", "ready", "done", RAIL_ID])).toEqual({ more: 1, needs: 0, target: "backlog" });
    // No chip wholly in view (a very narrow window): the first that needs you.
    expect(overflowCue(FOCUS6, [])).toEqual({ more: 6, needs: 2, target: "paused" });
  });
  it("reads the same right to left: a chip counts as in view only when both its edges are", () => {
    // Focus at 690px in RTL: the chips run from the right edge of the 587px list.
    const rtl: ChipSpan[] = spans(FOCUS6.map((c) => c.id), [[370, 587], [279, 364], [113, 273], [-50, 107], [-147, -56], [-224, -153]]);
    expect(inView(rtl, 0, 587)).toEqual(["paused", "backlog", "shaping"]);
    expect(overflowCue(FOCUS6, inView(rtl, 0, 587))).toEqual({ more: 3, needs: 0, target: "ready" });
  });
  it("no cue when every chip is in view; a chip cut by under a pixel is in view (rounding)", () => {
    expect(overflowCue(FOCUS6, FOCUS6.map((c) => c.id))).toBeNull();
    expect(overflowCue([], [])).toBeNull();
    expect(inView([{ id: "a", start: -0.6, end: 100 }, { id: "b", start: 106, end: 500.8 }], 0, 500)).toEqual(["a", "b"]);
    expect(inView([{ id: "a", start: 0, end: 501.5 }], 0, 500)).toEqual([]);
  });

  /** A row and its list as measureRow reads them: the list's box and each chip's, its name
   *  `names[id]` wide (the 30px minimum, 2.5em of 12px, when not given). */
  const box = (o: { row: number; list: number; left?: number; chips?: ChipSpan[]; names?: Record<string, number> }) => {
    const left = o.left ?? 14;
    const rect = (l: number, r: number) => ({ left: l + left, right: r + left, width: r - l });
    return {
      row: { clientWidth: o.row, padding: 28, nameMin: 30 },
      list: {
        clientWidth: o.list,
        clientLeft: 0,
        getBoundingClientRect: () => rect(0, o.list),
        querySelectorAll: () =>
          (o.chips ?? []).map((c) => ({ dataset: { chip: c.id }, getBoundingClientRect: () => rect(c.start, c.end), querySelector: () => ({ getBoundingClientRect: () => rect(0, o.names?.[c.id] ?? 30) }) })),
      },
    };
  };
  it("measures the chips against the list's visible box (Focus at 690px)", () => {
    const b = box({ row: 690, list: 587, chips: FOCUS6_AT });
    expect(measureRow(b.row, b.list)).toEqual({ overflow: true, visible: ["paused", "backlog", "shaping"], cut: ["ready"] });
  });
  it("the cue doesn't keep itself there: chips that fit the whole row without it don't overflow", () => {
    // At 1000px Focus's chips, names whole, take 937px of the row's 972: no cue.
    const at1000 = spans(FOCUS6.map((c) => c.id), [[0, 239], [245, 345], [351, 527], [533, 695], [701, 793], [799, 937]]);
    const wide = box({ row: 1000, list: 972, chips: at1000, names: { paused: 70, backlog: 52, shaping: 54, ready: 38, done: 34, [RAIL_ID]: 112 } });
    expect(measureRow(wide.row, wide.list)).toBe(ALL_IN_VIEW);
    // The window grows while the cue shows: the list (without the cue's room) still scrolls, but
    // the row would hold every chip, so the cue goes.
    const grown = box({ row: 980, list: 880, chips: spans(["a", "b"], [[0, 400], [406, 937]]) });
    expect(measureRow(grown.row, grown.list)).toBe(ALL_IN_VIEW);
    // A pixel's rounding isn't an overflow.
    const exact = box({ row: 965, list: 936.5, chips: spans(["a", "b"], [[0, 400], [406, 937]]) });
    expect(measureRow(exact.row, exact.list)).toBe(ALL_IN_VIEW);
  });
  it("shortens names only when that makes the chips fit; past that they stay whole and the row scrolls (QA, round 3)", () => {
    // Two chips drawn with whole names (1000px end to end, 230px of names above their minimum),
    // and the same two drawn shortened to the minimum (770px): the answer is the same for both.
    const whole = { chips: spans(["a", "b"], [[0, 500], [506, 1000]]), names: { a: 150, b: 140 } };
    const short = { chips: spans(["a", "b"], [[0, 380], [386, 770]]) };
    expect(leastRoom([{ start: 0, end: 500, name: 150 }, { start: 506, end: 1000, name: 140 }], 30)).toBe(770);
    expect(leastRoom([], 30)).toBe(0);
    // Room for 770 (a row of 800 with 28px padding is 772): shorten, no scroll, no cue.
    expect(measureRow(box({ row: 800, list: 772, ...whole }).row, box({ row: 800, list: 772, ...whole }).list)).toBe(ALL_IN_VIEW);
    expect(measureRow(box({ row: 800, list: 772, ...short }).row, box({ row: 800, list: 772, ...short }).list)).toBe(ALL_IN_VIEW);
    // Room for less: it overflows either way, and ColumnRow then keeps the names whole.
    expect(measureRow(box({ row: 780, list: 600, ...whole }).row, box({ row: 780, list: 600, ...whole }).list).overflow).toBe(true);
    expect(measureRow(box({ row: 780, list: 600, ...short }).row, box({ row: 780, list: 600, ...short }).list).overflow).toBe(true);
    expect(columnRowSource).toContain('className={view.overflow ? "col-row-chips col-row-scrolls" : "col-row-chips"}');
    expect(css).toContain(".col-row .col-row-scrolls .col-chip { flex: none; }");
  });
  it("tells a changed view from the same one, so a re-measure that finds nothing new renders nothing", () => {
    const v = (visible: string[], cut: string[] = []) => ({ overflow: true, visible, cut });
    expect(sameView(v(["a", "b"], ["c"]), v(["a", "b"], ["c"]))).toBe(true);
    expect(sameView(v(["a", "b"]), v(["a"]))).toBe(false);
    expect(sameView(v(["a"]), v(["b"]))).toBe(false);
    expect(sameView(v(["a"], ["b"]), v(["a"], ["c"]))).toBe(false);
    expect(sameView(ALL_IN_VIEW, v([]))).toBe(false);
  });

  it("walks on past a chip wider than the room: every click reaches a new chip, then wraps (review R8)", () => {
    // QA's braindump board at 1400px with the pane open: Ready (264, 7 need you, 9 blocked) is
    // wider than the list, so after a click scrolls it to the start it's cut, never wholly seen.
    const chips = [p("paused", "⏸︎ Paused", 0), p("backlog", "Backlog", 1), p("ready", "Ready", 264, 7, 9), p("building", "Building", 0), p("done", "Done", 1), p(RAIL_ID, "Recently finished", 0)];
    const W = 300; // the list's width
    const widths = [90, 95, 420, 100, 80, 150];
    const at = widths.reduce<ChipSpan[]>((out, w, i) => [...out, { id: chips[i].id, start: i === 0 ? 0 : out[i - 1].end + 6, end: (i === 0 ? 0 : out[i - 1].end + 6) + w }], []);
    const view = (scroll: number) => {
      const shifted = at.map((s) => ({ ...s, start: s.start - scroll, end: s.end - scroll }));
      return { visible: inView(shifted, 0, W), cut: cutBy(shifted, 0, W) };
    };
    const max = at.at(-1)!.end - W;
    let scroll = 0;
    let from: string | null = null;
    const reachedIds: string[] = [];
    for (let click = 0; click < 6; click++) {
      const v = view(scroll);
      const cue = overflowCue(chips, v.visible, from);
      if (!cue) break;
      from = cue.target;
      reachedIds.push(from);
      // scrollIntoView({ inline: "start" }), clamped to the end of the list.
      scroll = Math.min(at.find((s) => s.id === from)!.start, max);
    }
    // Ready first (it needs you), then on past it: Building (with Done in view beside it), then
    // Recently finished at the end, then back to the start (QA_4).
    expect(reachedIds.slice(0, 4)).toEqual(["ready", "building", RAIL_ID, "paused"]);
    // Between the start and those clicks every chip was at least partly in view.
    const shown = new Set([0, ...reachedIds.slice(0, 3).map((id) => Math.min(at.find((x) => x.id === id)!.start, max))].flatMap((sc) => [...view(sc).visible, ...view(sc).cut]));
    expect([...shown].sort()).toEqual(chips.map((c) => c.id).sort());
    // Without the last target the walk was stuck on Ready (R8's symptom).
    expect(overflowCue(chips, view(at[2].start).visible)?.target).toBe("ready");
    expect(view(at[2].start)).toEqual({ visible: [], cut: ["ready"] });
  });
  it("with no chip wholly in view, the walk still moves on from the last target (review R8)", () => {
    // The reviewer's case: a list of 108px beside the cue, no chip whole.
    expect(overflowCue(FOCUS6, [])).toEqual({ more: 6, needs: 2, target: "paused" });
    expect(overflowCue(FOCUS6, [], "paused")).toEqual({ more: 6, needs: 2, target: "backlog" });
    expect(overflowCue(FOCUS6, [], "shaping")).toEqual({ more: 6, needs: 2, target: "ready" });
    expect(overflowCue(FOCUS6, [], RAIL_ID)).toEqual({ more: 6, needs: 2, target: "paused" });
  });
  it("with no chip wholly in view, the walk reaches every chip once a round, in board order (QA_4)", () => {
    // Paused and Shaping need you; the first click goes to Paused, then each click to the next.
    const targets: string[] = [];
    let from: string | null = null;
    for (let click = 0; click < 12; click++) {
      from = overflowCue(FOCUS6, [], from)!.target;
      targets.push(from);
    }
    const round = ["paused", "backlog", "shaping", "ready", "done", RAIL_ID];
    expect(targets).toEqual([...round, ...round]);
    // With Shaping the first that needs you, the first click goes there, and the rounds after
    // still start at Paused: back to the start, not to what needs you (the bug kept to Ready on).
    const chips = FOCUS6.map((c) => (c.id === "paused" ? { ...c, needs: 0 } : c));
    let at: string | null = null;
    const walk: string[] = [];
    for (let click = 0; click < 10; click++) {
      at = overflowCue(chips, [], at)!.target;
      walk.push(at);
    }
    expect(walk).toEqual(["shaping", "ready", "done", RAIL_ID, ...round]);
  });
  it("with nothing wholly beside the cue, each click steps in board order through every chip and wraps (mu-86)", () => {
    // A board of 400px or less: no chip fits beside the cue, so `visible` is [] on every click and
    // the measured view may not hold the last target either. ColumnRow keeps `from` regardless.
    const chips = [p("paused", "⏸︎ Paused", 1, 1), p("backlog", "Backlog", 0), p("shaping", "Shaping", 0), p("ready", "Ready", 2, 2), p("done", "Done", 0)];
    const targets: string[] = [];
    let from: string | null = null;
    for (let click = 0; click < 11; click++) {
      from = overflowCue(chips, [], from)!.target;
      targets.push(from);
    }
    const round = ["paused", "backlog", "shaping", "ready", "done"];
    expect(targets).toEqual([...round, ...round, "paused"]);
    // Never the same chip twice running, and the plain chip after a needs-you one isn't skipped.
    expect(targets.every((id, i) => i === 0 || id !== targets[i - 1])).toBe(true);
    expect(overflowCue(chips, [], "paused")?.target).toBe("backlog");
    expect(overflowCue(chips, [], "ready")?.target).toBe("done");
    expect(overflowCue(chips, [], "done")?.target).toBe("paused");
    // The walk's first click (no `from`) still goes to what needs you first, and on from there.
    const later = chips.map((c) => (c.id === "paused" ? { ...c, needs: 0 } : c));
    expect(overflowCue(later, [])?.target).toBe("ready");
    expect(overflowCue(later, [], "ready")?.target).toBe("done");
    expect(overflowCue(later, [], "done")?.target).toBe("paused");
  });
  it("a last target that's no longer a chip (its column opened meanwhile) walks on from the list's start (mu-86)", () => {
    const chips = [p("backlog", "Backlog", 0), p("shaping", "Shaping", 0), p("ready", "Ready", 2, 2), p("done", "Done", 0)];
    expect(overflowCue(chips, [], "paused")).toEqual({ more: 4, needs: 2, target: "backlog" });
    // With chips in view, the first hidden one: the list's start, not what needs you.
    expect(overflowCue(chips, ["backlog"], "paused")).toEqual({ more: 3, needs: 2, target: "shaping" });
  });
  it("QA_4's braindump at 1400px with the pane open: every round shows every chip once and comes back to the start", () => {
    // Ten places; Ready (264, 7 need you, 9 blocked) is wider than the 520px list.
    const chips = [
      p("paused", "⏸︎ Paused", 0),
      p("backlog", "Backlog", 1),
      p("shaping", "Shaping", 0),
      p("ready", "Ready", 264, 7, 9),
      p("building", "Building", 2, 1),
      p("testing", "Testing", 1),
      p("your-check", "Your check", 1, 1),
      p("waiting", "Waiting", 0),
      p("done", "Done", 1),
      p(RAIL_ID, "Recently finished", 0),
    ];
    const W = 520;
    const widths = [100, 95, 92, 560, 160, 90, 150, 90, 80, 150];
    const at = widths.reduce<ChipSpan[]>((out, w, i) => [...out, { id: chips[i].id, start: i === 0 ? 0 : out[i - 1].end + 6, end: (i === 0 ? 0 : out[i - 1].end + 6) + w }], []);
    const max = at.at(-1)!.end - W;
    const view = (scroll: number) => {
      const shifted = at.map((s) => ({ ...s, start: s.start - scroll, end: s.end - scroll }));
      return { visible: inView(shifted, 0, W), cut: cutBy(shifted, 0, W) };
    };
    let scroll = 0;
    let from: string | null = null;
    const targets: string[] = [];
    const scrolls: number[] = [];
    for (let click = 0; click < 16; click++) {
      const v = view(scroll);
      from = overflowCue(chips, v.visible, from)!.target;
      targets.push(from);
      scroll = Math.min(at.find((s) => s.id === from)!.start, max);
      scrolls.push(scroll);
    }
    // Ready first (it needs you), on to the end, then back to Paused at the start, and round again.
    const first = targets.indexOf("paused");
    expect(targets.slice(0, first)).toEqual(["ready", "building", "done"]);
    expect(scrolls[first]).toBe(0);
    const next = targets.indexOf("paused", first + 1);
    expect(next).toBeGreaterThan(first);
    const round = targets.slice(first, next);
    // Each target once a round, in board order, and every chip at least partly in view on the way.
    expect(new Set(round).size).toBe(round.length);
    const order = chips.map((c) => c.id);
    expect(round.map((id) => order.indexOf(id))).toEqual([...round.map((id) => order.indexOf(id))].sort((a, b) => a - b));
    const shown = new Set(scrolls.slice(first, next).flatMap((sc) => [...view(sc).visible, ...view(sc).cut]));
    expect([...shown].sort()).toEqual([...order].sort());
    expect(targets.slice(next, next + round.length)).toEqual(round);
  });
  it("tells the chips an edge cuts from the ones wholly in view and the ones out of it", () => {
    const s: ChipSpan[] = [{ id: "a", start: -50, end: 40 }, { id: "b", start: 46, end: 200 }, { id: "c", start: 206, end: 420 }, { id: "d", start: 426, end: 500 }];
    expect(inView(s, 0, 300)).toEqual(["b"]);
    expect(cutBy(s, 0, 300)).toEqual(["a", "c"]);
    // Wider than the view: cut, wherever it's scrolled.
    expect(cutBy([{ id: "w", start: -10, end: 900 }], 0, 300)).toEqual(["w"]);
    // Touching an edge by under a pixel is out of view, not cut.
    expect(cutBy([{ id: "x", start: 299.5, end: 400 }], 0, 300)).toEqual([]);
  });

  describe("watching the row", () => {
    afterEach(() => vi.unstubAllGlobals());
    function browser() {
      const frames = new Map<number, () => void>();
      let next = 1;
      const ro = { observed: new Set<unknown>(), disconnected: false, fire: () => {} };
      const mo = { disconnected: false, fire: (_records: unknown[]) => {} };
      vi.stubGlobal("requestAnimationFrame", (f: () => void) => {
        frames.set(next, f);
        return next++;
      });
      vi.stubGlobal("cancelAnimationFrame", (id: number) => void frames.delete(id));
      vi.stubGlobal("getComputedStyle", () => ({ paddingInlineStart: "14px", paddingInlineEnd: "14px", minWidth: "30px" }));
      vi.stubGlobal("ResizeObserver", class {
        constructor(cb: () => void) {
          ro.fire = cb;
        }
        observe(t: unknown) {
          ro.observed.add(t);
        }
        unobserve(t: unknown) {
          ro.observed.delete(t);
        }
        disconnect() {
          ro.disconnected = true;
          ro.observed.clear();
        }
      });
      vi.stubGlobal("MutationObserver", class {
        constructor(cb: (r: unknown[]) => void) {
          mo.fire = cb;
        }
        observe() {}
        disconnect() {
          mo.disconnected = true;
        }
      });
      const run = () => {
        const fs = [...frames.values()];
        frames.clear();
        fs.forEach((f) => f());
      };
      return { frames, ro, mo, run };
    }
    function fake(children: unknown[]) {
      const listeners = new Map<string, { fn: () => void; opts: unknown }>();
      const b = box({ row: 690, list: 587, chips: FOCUS6_AT });
      const row = { clientWidth: 690, querySelector: () => null };
      const list = {
        ...b.list,
        children,
        querySelector: () => ({}),
        addEventListener: (type: string, fn: () => void, opts: unknown) => void listeners.set(type, { fn, opts }),
        removeEventListener: (type: string, fn: () => void) => {
          if (listeners.get(type)?.fn === fn) listeners.delete(type);
        },
      };
      return { row, list, listeners };
    }
    it("measures once per frame, on resize and on a passive scroll, with the row's own padding", () => {
      const b = browser();
      const { row, list, listeners } = fake([]);
      const seen: unknown[] = [];
      watchRow(row as unknown as HTMLElement, list as unknown as HTMLElement, (v) => seen.push(v));
      expect(listeners.get("scroll")?.opts).toEqual({ passive: true });
      b.run();
      expect(seen).toEqual([{ overflow: true, visible: ["paused", "backlog", "shaping"], cut: ["ready"] }]);
      listeners.get("scroll")?.fn();
      b.ro.fire();
      b.ro.fire();
      expect(b.frames.size).toBe(1);
      b.run();
      expect(seen).toHaveLength(2);
    });
    it("watches the row, the list and each chip, following chips that come and go", () => {
      const b = browser();
      const first = { nodeType: 1 }, second = { nodeType: 1 }, text = { nodeType: 3 };
      const { row, list } = fake([first]);
      watchRow(row as unknown as HTMLElement, list as unknown as HTMLElement, () => {});
      expect([...b.ro.observed]).toEqual([row, list, first]);
      b.mo.fire([{ addedNodes: [second, text], removedNodes: [first] }]);
      expect([...b.ro.observed]).toEqual([row, list, second]);
    });
    it("takes the board cue's slot and its gap from the chips' room while it's on their line, nothing when it has its own (mu-198, review R4)", () => {
      // Three chips ending at 419px fit the 690px row's 662 alone; beside the slot's 240px and
      // its 6px gap (416 left) they don't. Without the gap (422) they would.
      const chips = spans(["paused", "backlog", "shaping"], [[0, 130], [136, 270], [276, 419]]);
      const measured = (slotTop: number) => {
        const b = browser();
        const bx = box({ row: 690, list: 587, chips });
        const slot = { getBoundingClientRect: () => ({ top: slotTop }) };
        const row = { clientWidth: 690, querySelector: (s: string) => (s === ".col-edge-slot" ? slot : null) };
        const list = { ...bx.list, getBoundingClientRect: () => ({ ...bx.list.getBoundingClientRect(), top: 4, bottom: 32 }), children: [], querySelector: () => ({}), addEventListener: () => {}, removeEventListener: () => {} };
        vi.stubGlobal("getComputedStyle", (el: unknown) => (el === slot ? { minWidth: "240px" } : { paddingInlineStart: "14px", paddingInlineEnd: "14px", minWidth: "30px", columnGap: "6px" }));
        const seen: unknown[] = [];
        watchRow(row as unknown as HTMLElement, list as unknown as HTMLElement, (v) => seen.push(v));
        b.run();
        return seen[0];
      };
      expect(measured(4)).toMatchObject({ overflow: true });
      // Narrow: the slot is on the line under the chips' (its top at their bottom).
      expect(measured(32)).toBe(ALL_IN_VIEW);
    });
    it("lets everything go when the row unmounts", () => {
      const b = browser();
      const { row, list, listeners } = fake([]);
      const stop = watchRow(row as unknown as HTMLElement, list as unknown as HTMLElement, () => {});
      stop();
      expect(listeners.size).toBe(0);
      expect(b.ro.disconnected && b.mo.disconnected).toBe(true);
      expect(b.frames.size).toBe(0);
    });
  });

  it("ColumnRow wires it: the chips scroll in their list, the cue after it scrolls to its target", () => {
    const src = columnRowSource;
    expect(src).toContain("return watchRow(row.current, list.current, (v) => setView((prev) => (sameView(prev, v) ? prev : v)));");
    // The last target is kept whether or not the view measures it (mu-86): no gate on what's in view.
    expect(src).toContain("const cue = any && view.overflow ? overflowCue(chips, view.visible, from) : null;");
    expect(src).not.toContain("reached");
    expect(src).toMatch(/onShow=\{\(\) => \{\s*setFrom\(cue\.target\);\s*showChip\(list\.current, cue\.target\);/);
    expect(src).toContain('el.scrollIntoView({ inline: "start", block: "nearest", behavior: still ? "auto" : "smooth" });');
    expect(src).toContain("el.focus({ preventScroll: true });");
    // The cue never shrinks or scrolls away, and has no left or right of its own (RTL).
    const cueRule = css.match(/\n\.col-row \.col-cue \{([^}]*)\}/)?.[1] ?? "";
    expect(cueRule).toContain("flex: none;");
    expect(cueRule).toContain("white-space: nowrap;");
  });
});

describe("the board's own cue at the row's end (mu-198)", () => {
  const edge = (hidden: [string, string, number][], target = hidden[0][0]) => ({
    hidden: hidden.map(([id, label, needs]) => ({ id, label, needs })),
    needs: hidden.reduce((n, h) => n + h[2], 0),
    target,
  });
  const draw = (cue: ReturnType<typeof edge>) => renderToStaticMarkup(createElement(BoardCue, { cue, onShow: () => {} }));
  const p = (id: string, label: string, count: number, needs = 0): RowChip => ({ id, label, count, needs, blocked: 0 });
  /** A row and its list as measureRow reads them, the list `list` wide at x 14. */
  const box = (o: { row: number; list: number; chips: ChipSpan[] }) => {
    const rect = (l: number, r: number) => ({ left: l + 14, right: r + 14, width: r - l });
    return {
      row: { clientWidth: o.row, padding: 28, nameMin: 30 },
      list: {
        clientWidth: o.list,
        clientLeft: 0,
        getBoundingClientRect: () => rect(0, o.list),
        querySelectorAll: () => o.chips.map((c) => ({ dataset: { chip: c.id }, getBoundingClientRect: () => rect(c.start, c.end), querySelector: () => ({ getBoundingClientRect: () => rect(0, 30) }) })),
      },
    };
  };
  afterEach(async () => {
    await setLocale("en");
  });
  it("is a button whose name says the columns in words, showing each name and its needs-you words (AC2)", () => {
    const html = draw(edge([["your-check", "Your check", 21], ["done", "Done", 0]]));
    expect(html).toBe(
      '<button type="button" aria-label="2 columns past the edge: Your check, 21 need you; Done. Show them" title="Past the edge: Your check · 21 need you, Done" class="ui-pill col-cue col-edge">' +
        'Past the edge: <span class="col-edge-list"><span class="col-edge-item"><span class="col-edge-name">Your check</span> · <b class="count warn">21 need you</b></span>' +
        '<span class="col-edge-item">, <span class="col-edge-name">Done</span></span></span></button>',
    );
    // Nothing that needs you: no needs-you colour at all.
    expect(draw(edge([["done", "Done", 0]]))).not.toContain("warn");
  });
  it("names the open columns as their heads do, Paused and the stages, with their needs-you counts", () => {
    const cols: Record<string, Task[]> = {
      paused: [task("2", { waitingForOwner: true })],
      "your-check": [task("1", { stage: "your-check", waitingForOwner: true }), task("3", { stage: "your-check" })],
      done: [],
    };
    expect(edgeColumnsOf(["paused", "your-check", "done"], cols)).toEqual([
      { id: "paused", label: "Paused", needs: 1 },
      { id: "your-check", label: "Your check", needs: 1 },
      { id: "done", label: "Done", needs: 0 },
    ]);
  });
  it("in Persian: the lead-in, the names and the needs-you words from the language files (AC4)", async () => {
    await setLocale("fa");
    expect(edgeColumnsOf(["paused", "your-check"], {})).toEqual([
      { id: "paused", label: "متوقف شده", needs: 0 },
      { id: "your-check", label: "بررسی شما", needs: 0 },
    ]);
    const html = draw(edge([["your-check", "بررسی شما", 21], ["done", "انجام شد", 0]]));
    expect(html).toContain("بیرون از لبه‌ی بورد: ");
    expect(html).toContain('<b class="count warn">۲۱ مورد به شما نیاز دارد</b>');
    expect(html).toContain("، ");
    expect(html).not.toMatch(/>[^<]*[A-Za-z][^<]*</);
  });
  it("only while a column is past the edge; the row's chips keep their markup either way (AC2)", () => {
    // No board measured yet: no cue, and the row is drawn as before, with its live region.
    const html = renderToStaticMarkup(createElement(ColumnRow, { chips: [chip("shaping")], onOpen: () => {}, board: {} as HTMLElement, columns: [] }));
    expect(html).not.toContain("col-edge\"");
    expect(html).not.toContain("<button type=\"button\" aria-label=\"1 column");
    // Its slot is there, empty, holding its place, then the live region.
    expect(html).toContain('<span class="col-edge-slot"></span><span class="sr-only" aria-live="polite"></span></div>');
    expect(html.replace('<span class="col-edge-slot"></span><span class="sr-only" aria-live="polite"></span>', "")).toBe(row([chip("shaping")]));
    // No board (the grid, Group by): no slot, no live region.
    expect(row([chip("shaping")])).not.toMatch(/col-edge-slot|aria-live/);
    const src = columnRowSource;
    expect(src).toContain("const stop = watchBoard(board, (v) => setBoardView((prev) => (sameView(prev, v) ? prev : v)));");
    expect(src).toContain("const edge = board && boardView.overflow ? edgeCue(columns, boardView.visible, edgeFrom) : null;");
    // A board gone (the grid, Group by) leaves no stale cue behind.
    expect(src).toMatch(/stop\(\);\s*setBoardView\(ALL_IN_VIEW\);/);
    // The row's own cue comes first, then the board's.
    const body = src.slice(src.indexOf("export default function ColumnRow"));
    expect(body.indexOf("<RowCue")).toBeGreaterThan(0);
    expect(body.indexOf("<RowCue")).toBeLessThan(body.indexOf("<EdgeSlot"));
  });
  it("its click scrolls the board, not the page, keeps the focus on the cue, walks on and says where it went (AC2)", () => {
    // Run for real in ColumnRowClick.test.tsx; here, that it's the board and nothing else.
    const src = columnRowSource;
    expect(src).toMatch(/onShow=\{\(target\) => \{\s*setEdgeFrom\(target\);\s*showColumn\(board, target\);\s*setSaid\(edgeSaid\(columns, target\)\);/);
    const show = src.slice(src.indexOf("export function showColumn"), src.indexOf("/** Scrolls the list to a chip"));
    expect(show).not.toContain("scrollIntoView");
    expect(show).not.toContain("focus(");
    expect(tn("board", "edge.live", { name: "Your check" })).toBe("Your check is in view");
  });
  it("on two lines or counted, the same button and name; the needs-you words whole in every form (review R1, R2)", async () => {
    const cue = edge([["your-check", "Your check", 258], ["testing", "Testing", 0], ["done", "Done", 0]]);
    const at = (fit: "line" | "wrap" | "count") => renderToStaticMarkup(createElement(BoardCue, { cue, fit, onShow: () => {} }));
    const open = (cls: string) => `<button type="button" aria-label="3 columns past the edge: Your check, 258 need you; Testing; Done. Show them" title="Past the edge: Your check · 258 need you, Testing, Done" class="${cls}">`;
    // Two lines: every name still there, whole.
    expect(at("wrap")).toBe(at("line").replace(open("ui-pill col-cue col-edge"), open("ui-pill col-cue col-edge col-edge-two")));
    expect(at("wrap")).toContain("Testing");
    // Counted: how many in words, and what needs you.
    expect(at("count")).toBe(`${open("ui-pill col-cue col-edge col-edge-two")}3 columns past the edge · <b class="count warn">258 need you</b></button>`);
    const one = renderToStaticMarkup(createElement(BoardCue, { cue: edge([["done", "Done", 0]]), fit: "count", onShow: () => {} }));
    expect(one).toContain(">1 column past the edge</button>");
    expect(one).not.toContain("warn");
    await setLocale("fa");
    const fa = renderToStaticMarkup(createElement(BoardCue, { cue: edge([["your-check", "بررسی شما", 258], ["done", "انجام شد", 0]]), fit: "count", onShow: () => {} }));
    expect(fa).toContain("۲ ستون بیرون از لبه‌ی بورد · ");
    expect(fa).toContain('<b class="count warn">۲۵۸ مورد به شما نیاز دارد</b>');
    expect(fa).not.toMatch(/>[^<]*[A-Za-z][^<]*</);
  });
  it("its slot holds two hidden probes of the whole words while the cue shows, out of the reading order", () => {
    const src = columnRowSource;
    const slot = src.slice(src.indexOf("function EdgeSlot"), src.indexOf("export const edgeSaid"));
    expect(slot).toContain('<span className="col-edge-probe" aria-hidden="true">');
    expect(slot).toContain('<span className="col-cue col-edge col-edge-probe-line">');
    expect(slot).toContain('<span className="col-cue col-edge col-edge-two col-edge-probe-wrap">');
    expect(slot).toContain("return watchEdge(slot.current, setFit);");
    expect(slot).toContain("}, [shows]);");
  });
  it("App hands it the plain board and its open columns, each column marked for measuring", () => {
    expect(appSource).toContain("const [boardEl, setBoardEl] = useState<HTMLElement | null>(null);");
    expect(appSource).toContain('<section key={c.id} className={columnClass(c.id, "open")} data-col={c.id}>');
    expect(pausedSource).toContain('<section className={columnClass("paused", "open")} data-col="paused">');
  });
  it("its slot's least width is the chips' never, cue or no cue, so the chips don't move; it takes the row's free room and cuts nothing (review R1, R2)", () => {
    // The 1440 row: 1120px with 28px padding; the chips need 900px; the slot's 13em and its gap 175.
    const b = box({ row: 1120, list: 872, chips: [{ id: "a", start: 0, end: 440 }, { id: "b", start: 446, end: 920 }] });
    expect(measureRow(b.row, b.list)).toBe(ALL_IN_VIEW);
    expect(measureRow({ ...b.row, reserved: 175 }, b.list).overflow).toBe(true);
    expect(overflowCue([p("a", "A", 1), p("b", "B", 1, 2)], ["a"])).toEqual({ more: 1, needs: 2, target: "b" });
    // The slot's min-width, not the cue's drawn width: the same whatever the cue says.
    expect(rowCueSource).toContain('const slot = row.querySelector(".col-edge-slot");');
    expect(rowCueSource).toContain("parseFloat(getComputedStyle(slot).minWidth)");
    const rule = (sel: string) => css.match(new RegExp(`\\n${sel.replace(/[.]/g, "\\.")} \\{([^}]*)\\}`))?.[1] ?? "";
    // It grows into the room the chips and the row's cue leave (from a 0 basis, so never by what
    // the cue says), no cap, at least 13em: the counted form on two lines, in both languages
    // (headless WebKit: 169px holds "۲ ستون بیرون از لبه‌ی بورد · ۲۵۸ مورد به شما نیاز دارد").
    expect(rule(".col-row .col-edge-slot")).toBe(" position: relative; flex: 1 1 0; display: flex; align-items: center; justify-content: flex-end; min-width: 13em; height: 100%; ");
    expect(rule(".col-row .col-edge")).toBe(" display: block; max-inline-size: 100%; text-align: start; ");
    // Two lines of 13px fill the row's 28px with the pill's 1px borders: 34 less its 4px and 2px padding.
    expect(rule(".col-row .col-edge-two")).toBe(" font-size: 11px; line-height: 13px; padding-block: 0; border-radius: 8px; white-space: normal; ");
    expect(rule(".col-row .col-edge .count")).toBe(" white-space: nowrap; ");
    // Nothing of it shortens or clips its words.
    const edgeRules = [...css.matchAll(/\n\s*(\.col-row[^{]*col-edge[^{]*)\{([^}]*)\}/g)];
    expect(edgeRules.length).toBeGreaterThan(5);
    for (const [, sel, body] of edgeRules) {
      expect(body, sel).not.toMatch(/ellipsis|line-clamp|max-inline-size: [1-9]\d?%/);
      if (!sel.includes("probe")) expect(body, sel).not.toContain("overflow");
    }
    // The probes: out of sight and of the layout, the one-line one at its whole width.
    expect(rule(".col-row .col-edge-probe")).toBe(" position: absolute; inset-inline: 0; top: 0; height: 0; overflow: hidden; visibility: hidden; pointer-events: none; ");
    expect(rule(".col-row .col-edge-probe-line")).toBe(" inline-size: max-content; max-inline-size: none; ");
    expect(rule(".col-row .col-edge-probe-wrap")).toBe(" inline-size: fit-content; ");
    // Narrow (under 30em), its own line under the chips, full width: a 62px row.
    expect(css).toContain(".col-row-box { container: colrow / inline-size; }");
    expect(css).toContain(".col-row > .col-edge-slot { flex: 1 0 100%; min-width: 0; height: 28px; }");
    // A wrapping row lines items up by their flex basis, so the chips' list has a zero one there:
    // with its content's basis it took line 1 alone, the row's cue line 2 and the board's a third,
    // past the row's 62px and over the column heads (review R6).
    expect(css).toContain(".col-row:has(> .col-edge-slot) > .col-row-chips { flex: 1 1 0; height: 28px; }");
    expect(css).toMatch(/@container colrow \(width < 30em\) \{\n {2}\.col-row:has\(> \.col-edge-slot\) \{ flex-wrap: wrap; align-content: space-between; row-gap: 0; height: 62px; \}/);
    expect(appSource).toContain('<div className="col-row-box"><ColumnRow ');
  });
});

// mu-147, AC9: the row's off-edge cue, its chips and a column head's Fold speak the language shown.
// In Persian the cue's "+2" sits in a left-to-right isolate, so its sign stays before its number
// ("+۲ دیگر", never "۲+"), on the board and in the Feature grid (FeatureGrid.test.tsx).
describe("the row in Persian (mu-147, AC9)", () => {
  const LRI = "⁦";
  const PDI = "⁩";
  const cueHtml = (more: number, needs = 0) => renderToStaticMarkup(createElement(RowCue, { cue: { more, needs, target: "ready" }, onShow: () => {} }));
  const bare = (s: string) => s.replace(/[⁦-⁩]/g, "");
  /** Text and the title and aria-label values, tags and isolation marks out. */
  const words = (html: string) => bare([html.replace(/<[^>]+>/g, " "), ...[...html.matchAll(/\s(?:title|aria-label)="([^"]*)"/g)].map((m) => m[1])].join(" "));
  afterEach(async () => {
    await setLocale("en");
  });
  it("English reads as it did: '+2 more', its title and its spoken words", () => {
    expect(cueHtml(2)).toBe('<button type="button" aria-label="2 more columns past the edge. Show them" title="Show the columns past the edge" class="ui-pill col-cue"><span>+2 more</span></button>');
    expect(cueHtml(1)).toContain("<span>+1 more</span>");
    expect(cueWords({ more: 1, needs: 0, target: "a" })).toBe("1 more column past the edge. Show it");
    expect(cueWords({ more: 3, needs: 2, target: "a" })).toBe("3 more columns past the edge, 2 need you. Show them");
  });
  it("Persian: the count with its sign in a left-to-right isolate, the title and the spoken words, for 0, 1 and many", async () => {
    await setLocale("fa");
    for (const [n, digits] of [[0, "۰"], [1, "۱"], [2, "۲"], [12, "۱۲"]] as const) {
      const html = cueHtml(n);
      expect(html, `${n}`).toContain(`<span>${LRI}+${digits}${PDI} دیگر</span>`);
      expect(html).toContain('title="نمایش ستون‌های بیرون از لبه"');
      expect(words(html), `${n}`).not.toMatch(/[A-Za-z]/);
    }
    expect(cueWords({ more: 1, needs: 0, target: "a" })).toBe("۱ ستون دیگر بیرون از لبه. نمایش آن");
    expect(cueWords({ more: 3, needs: 0, target: "a" })).toBe("۳ ستون دیگر بیرون از لبه. نمایش آن‌ها");
    const needs = bare(cueWords({ more: 3, needs: 2, target: "a" }));
    expect(needs.startsWith("۳ ستون دیگر بیرون از لبه، ")).toBe(true);
    expect(needs.endsWith(". نمایش آن‌ها")).toBe(true);
    expect(needs).not.toMatch(/[A-Za-z]/);
    // What needs you is in the cue as on the chips, and no Latin word is left.
    expect(words(cueHtml(6, 6))).not.toMatch(/[A-Za-z]/);
  });
  it("Persian: the row's name, its empty line, each chip's words and its title", async () => {
    await setLocale("fa");
    const counts: PlaceCounts = { cols: { backlog: [task("1", { stage: "backlog", waitingForOwner: true }), task("2", { stage: "backlog", blocked: true })] }, notes: 0, finished: 129 };
    const html = renderToStaticMarkup(createElement(ColumnRow, { chips: rowChipsOf(["shaping", "backlog", RAIL_ID], false, counts), onOpen: () => {} }));
    expect(html).toContain('aria-label="ستون‌های جمع‌شده و خالی"');
    expect(html).toContain('title="باز کردن ');
    expect(html).toContain("اخیراً انجام شد");
    expect(html).toContain('<b class="count blocked-count">۱ مسدود</b>');
    expect(words(html)).not.toMatch(/[A-Za-z]/);
    expect(bare(chipWords(chip("shaping")))).toBe("شکل‌دهی: خالی. باز کردن آن");
    expect(bare(chipWords(chip("ready", 4, 0, 2)))).toBe("آماده: ۴ مورد، ۲ مسدود. باز کردن آن");
    expect(bare(chipWords(chip("ready", 1, 1, 0)))).toMatch(/^آماده: ۱ مورد، [^A-Za-z]+\. باز کردن آن$/);
    expect(words(renderToStaticMarkup(createElement(ColumnRow, { chips: [], onOpen: () => {} })))).toContain("همه‌ی ستون‌ها باز هستند");
  });
  it("Persian: an open column's Fold and a folded strip's words", async () => {
    await setLocale("fa");
    const open = renderToStaticMarkup(createElement(ColumnHead, { stage: STAGES[2], count: 3, needs: 0, blocked: 0, fold: "fixed", onFold: () => {}, onHide: () => {} }));
    expect(open).toContain('aria-label="جمع کردن ');
    expect(open).toContain(">جمع کردن</button>");
    expect(words(open)).not.toMatch(/[A-Za-z]/);
    const strip = renderToStaticMarkup(createElement(ColumnHead, { stage: STAGES[1], count: 0, needs: 0, blocked: 2, fold: "folded", onFold: () => {} }));
    expect(strip).toContain("<small>خالی</small>");
    expect(words(strip)).not.toMatch(/[A-Za-z]/);
  });
});
