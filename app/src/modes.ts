// Display modes (0038): ADHD, Standard and Focus decide how much of each element shows. One table
// says, for every element a mode could change, what it does in each mode (the same in all three
// says so) and how to get back what a mode hides. Components ask it through `useLook` and never read the mode themselves:
// a test fails when a component reads an element the table doesn't have, or the mode around it
// (modes.test.ts). Everything else, the colour code, no overlays and fixed places, is the same in
// all three (docs/DESIGN.md).
import { createContext, useContext } from "react";
import { STAGES } from "./lib";

export type DisplayMode = "adhd" | "standard" | "focus";
export const DEFAULT_MODE: DisplayMode = "adhd";

/** The modes in the order Settings shows them, each with what it's for. */
export const MODES: readonly { id: DisplayMode; label: string; says: string }[] = [
  { id: "adhd", label: "ADHD", says: "Everything on screen: every filter, option and state, nothing behind a click. Out of sight is out of mind." },
  { id: "standard", label: "Standard", says: "What most apps do: the filters behind one chip, quieter cards, and the task pane's secondary sections folded." },
  { id: "focus", label: "Focus", says: "Only what needs doing now: Building, Testing and Your check, each card its title and one status line." },
];

/** What an element does in one mode: `how` is what the component reads, `says` and `reach` are
 *  Settings' words. `reach` says how to get back what the mode hides, and only ADHD hides nothing. */
type Behaviour = { how: string; says: string; reach?: string };
type Entry = { label: string } & Record<DisplayMode, Behaviour>;

const TABLE = {
  filters: {
    label: "Filter bar",
    adhd: { how: "open", says: "Search, saved views, every filter group and Sort, on screen" },
    standard: { how: "folded", says: "Search and saved views; the filter groups and Sort behind one Filters chip with the number on", reach: "Click Filters: they open in place, under the bar" },
    focus: { how: "on-key", says: "Hidden, and back while a search or filter is on", reach: "⌘F, or the Search and filters button" },
  },
  choices: {
    label: "Who and Group by",
    adhd: { how: "all", says: "Every choice as a chip" },
    standard: { how: "value", says: "The current choice as one chip", reach: "Click the chip: the choices lay out in its place" },
    focus: { how: "value", says: "The current choice as one chip, in the filter bar", reach: "Click the chip: the choices lay out in its place" },
  },
  "card-lines": {
    label: "Card lines",
    adhd: { how: "all", says: "Who and since when, the check-in, branch, tags, parent, estimate, files and age on every card" },
    standard: { how: "selected", says: "Title, kind, priority, age, anything wrong and what Claude Code says the session is doing; who, the check-in, tags, estimate and the rest on the selected card", reach: "Select the card" },
    focus: { how: "status", says: "Title and one status line, and blocked, paused, quiet, needs you or what Claude Code says the session is doing, when it is", reach: "Open the card: the task pane has it all" },
  },
  "done-line": {
    label: "Done line",
    adhd: { how: "show", says: "When it was done (done 4 min ago), on every Done card" },
    standard: { how: "show", says: "When it was done, on every Done card: it's the card's status line" },
    focus: { how: "show", says: "When it was done, on every Done card: it's the card's one status line" },
  },
  "holder-ribbon": {
    label: "Holder ribbon",
    adhd: { how: "show", says: "Who holds a task in Shaping, Building or Testing, and who is shaping a backlog note, in the card's corner" },
    standard: { how: "show", says: "Who holds a task in Shaping, Building or Testing, and who is shaping a backlog note, in the card's corner: on the brief card it's the one place the holder shows" },
    focus: { how: "show", says: "Who holds a task in Shaping, Building or Testing, and who is shaping a backlog note, in the card's corner: who's on it is status" },
  },
  columns: {
    label: "Board columns",
    adhd: { how: "all", says: "Every stage with tasks, and Paused before Backlog; an empty one, or one you folded, is a chip in the row above the board with its count, needs you and blocked" },
    standard: { how: "all", says: "Every stage with tasks, and Paused before Backlog; an empty one, or one you folded, is a chip in the row above the board with its count, needs you and blocked" },
    focus: { how: "now", says: "Building, Testing and Your check; the others and Paused are chips in the row above the board with their counts, needs you and blocked (the grouped board's Paused and Backlog bands fold to their heads)", reach: "Click a chip to open that column; Fold puts it back" },
  },
  rail: {
    label: "Recently finished",
    adhd: { how: "open", says: "Docked beside the board: what merged or was accepted, by day, with time and tokens" },
    standard: { how: "open", says: "Docked beside the board: what merged or was accepted, by day, with time and tokens" },
    focus: { how: "strip", says: "A chip at the end of the row above the board, with its name and count", reach: "Click the chip to open it in place; Fold puts it back" },
  },
  "pane-sections": {
    label: "Task pane",
    adhd: { how: "open", says: "Vision, Review, Work, the Log and Check-ins open, one under another" },
    standard: { how: "folded", says: "Vision, Review, Work, and the Log and check-ins folded to a header each", reach: "Click a header to open it" },
    focus: { how: "folded", says: "Vision, Review, Work, and the Log and check-ins folded to a header each", reach: "Click a header to open it" },
  },
  sidebar: {
    label: "Sidebar",
    adhd: { how: "full", says: "Projects under their category headings (each with its tasks, needs you and working), their counts, the order chips, each one's colour dot and activity line, and + Category; each heading one line of labelled counts, what needs you first, the same folded" },
    standard: { how: "full", says: "Projects under their category headings (each with its tasks, needs you and working), their counts, the order chips, each one's colour dot and activity line, and + Category; each heading one line of labelled counts, what needs you first, the same folded" },
    focus: { how: "narrow", says: "Category headings with the same one line of labelled counts, what needs you first, open or folded, project names with their colour dot, the working dot, stalled and the needs-you and paused counts, and a project over its Your check limit says so; a stalled project keeps its Ask for a check-in", reach: "Leave Focus: ⌘. or the mode chip in the header" },
  },
} as const satisfies Record<string, Entry>;

export type ElementId = keyof typeof TABLE;
/** What an element can do, by mode: `useLook("columns")` is "all" or "now". */
export type Look<E extends ElementId> = (typeof TABLE)[E][DisplayMode]["how"];
/** Every element a mode could change, in Settings' order. */
export const ELEMENTS: Readonly<Record<ElementId, Entry>> = TABLE;

// The table as a mapped type, so `look` returns each element's own behaviours.
const ROWS: { [E in ElementId]: Record<DisplayMode, { how: Look<E> }> } = TABLE;

export function look<E extends ElementId>(el: E, mode: DisplayMode): Look<E> {
  return ROWS[el][mode].how;
}

/** The mode the window is in. Read it with `useLook`, never directly. */
export const Mode = createContext<DisplayMode>(DEFAULT_MODE);

/** What an element does in the window's mode. */
export function useLook<E extends ElementId>(el: E): Look<E> {
  return look(el, useContext(Mode));
}

export const modeLabel = (m: DisplayMode) => MODES.find((x) => x.id === m)?.label ?? m;

/** The mode, and the one ⌘. goes back to when Focus is on (the mode itself otherwise). */
export type ModeState = { mode: DisplayMode; before: DisplayMode };

export const MODE_KEY = "muster.displayMode";
const BEFORE_KEY = `${MODE_KEY}.before`;
const parse = (s: string | null): DisplayMode | null => MODES.find((m) => m.id === s)?.id ?? null;

/** The kept mode, ADHD when there's none or it's not a mode. */
export function loadMode(store: Pick<Storage, "getItem">): ModeState {
  const mode = parse(store.getItem(MODE_KEY)) ?? DEFAULT_MODE;
  const before = parse(store.getItem(BEFORE_KEY));
  return { mode, before: before && before !== "focus" ? before : mode === "focus" ? DEFAULT_MODE : mode };
}

export function storeMode(store: Pick<Storage, "setItem">, s: ModeState): void {
  store.setItem(MODE_KEY, s.mode);
  store.setItem(BEFORE_KEY, s.before);
}

/** ⌘.: Focus from any mode, and out of it back to the mode it came from. */
export function toggleFocus(s: ModeState): ModeState {
  return s.mode === "focus" ? { mode: s.before, before: s.before } : { mode: "focus", before: s.mode };
}

/** A mode picked in Settings. Focus keeps the mode to go back to. */
export function pickMode(s: ModeState, m: DisplayMode): ModeState {
  if (m !== "focus") return { mode: m, before: m };
  return { mode: m, before: s.mode === "focus" ? s.before : s.mode };
}

/** Which of a card's lines show: all of them, Standard's brief card (no who, check-in, branch,
 *  tags, parent, estimate or files), or Focus's title and status line. */
export type CardLines = "all" | "brief" | "status";
export function cardLines(how: Look<"card-lines">, active: boolean): CardLines {
  if (how === "all" || (how === "selected" && active)) return "all";
  return how === "selected" ? "brief" : "status";
}

/** The stages Focus keeps open: the work going on now and what waits on the owner. */
export const NOW_STAGES: readonly string[] = ["building", "testing", "your-check"];

/** A grouped board band's column in a mode: "fixed" when the mode doesn't fold it (it shows as
 *  ever), "folded" to a strip, or "opened" from its strip by a click. A column the owner hid on the
 *  plain board (mu-84) is folded in the bands too. */
export function columnFold(how: Look<"columns">, stage: string, opened: ReadonlySet<string>, hidden: ReadonlySet<string> = NO_IDS): ColumnFold {
  if (hidden.has(stage)) return "folded";
  if (how === "all" || NOW_STAGES.includes(stage)) return "fixed";
  return opened.has(stage) ? "opened" : "folded";
}
const NO_IDS: ReadonlySet<string> = new Set();
export type ColumnFold = "fixed" | "folded" | "opened";

/** The board's column folds, for the grouped board's bands: each column's, and the click on its strip. */
export type ColumnFolds = { of: (stage: string) => ColumnFold; toggle: (stage: string) => void };

/** What a board column is (mu-83): open with its cards, an empty strip, or a strip Focus folded.
 *  No mode folds an empty column away: ADHD and Standard keep it as a narrow strip that says
 *  empty, so the board keeps its shape; Focus folds what it folds, and its never-folded empty
 *  columns are the same strip. `keepOpen` for a column that has something to show with no cards
 *  (Backlog's quick add, Your check's Accept all line or its limit). */
export type ColumnState = "open" | "empty" | "folded";
export function columnState(fold: ColumnFold, count: number, keepOpen = false): ColumnState {
  if (fold === "folded") return "folded";
  return count > 0 || keepOpen ? "open" : "empty";
}

/** The widest an open column gets, in px: about a card's reading width. Spare room stays empty at
 *  the board's right, so a card never stretches into one long line. */
export const OPEN_COLUMN_MAX = 400;

/** A column's track in the board's grid: open up to OPEN_COLUMN_MAX, a strip 44px. */
export const columnTrack = (s: ColumnState) => (s === "open" ? `minmax(118px, ${OPEN_COLUMN_MAX}px)` : "44px");

/** A column's classes. A strip is `col-strip`, with `col-empty` or `folded`: never `empty`, the
 *  empty message's class, whose padding once pushed a strip out of its track. */
export const columnClass = (stage: string, s: ColumnState) => `column col-${stage}${s === "folded" ? " col-strip folded" : s === "empty" ? " col-strip col-empty" : ""}`;

/** The plain board's columns in their order: the Paused lane, then every stage. */
export const BOARD_COLUMNS: readonly string[] = ["paused", ...STAGES.map((s) => s.id)];

/** Recently finished, docked at the board's right: the last place in the row of folded columns. */
export const RAIL_ID = "recent";
/** Every place the row of folded columns can hold, in board order. */
export const ROW_ORDER: readonly string[] = [...BOARD_COLUMNS, RAIL_ID];

/** The owner's pick for one column in one mode (mu-84): "shown" keeps open a column the mode or
 *  its emptiness would put in the row, "hidden" puts an open one there. No pick: the mode decides. */
export type ColumnPick = "shown" | "hidden";
export type ModePicks = Readonly<Record<string, ColumnPick>>;
/** The picks per mode, kept on this Mac, so each mode remembers its own board. */
export type ColumnPicks = Partial<Record<DisplayMode, ModePicks>>;
export const COLUMNS_KEY = "muster.columns";

/** The kept picks: none for a missing or broken value, and only known modes, places and picks. */
export function loadPicks(store: Pick<Storage, "getItem">): ColumnPicks {
  let raw: unknown;
  try {
    raw = JSON.parse(store.getItem(COLUMNS_KEY) ?? "{}");
  } catch {
    return {};
  }
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return {};
  const out: ColumnPicks = {};
  for (const { id } of MODES) {
    const m: unknown = (raw as Record<string, unknown>)[id];
    if (!m || typeof m !== "object" || Array.isArray(m)) continue;
    const kept: Record<string, ColumnPick> = {};
    for (const [col, v] of Object.entries(m)) if (ROW_ORDER.includes(col) && (v === "shown" || v === "hidden")) kept[col] = v;
    if (Object.keys(kept).length) out[id] = kept;
  }
  return out;
}

export function storePicks(store: Pick<Storage, "setItem">, picks: ColumnPicks): void {
  store.setItem(COLUMNS_KEY, JSON.stringify(picks));
}

/** What the window's mode says about the board, as App reads it from the table. */
export type BoardLooks = { columns: Look<"columns">; rail: Look<"rail"> };

/** Whether the mode itself folds a place: Focus folds every column but Building, Testing and Your
 *  check, and Recently finished. */
export function modeFolds(looks: BoardLooks, id: string): boolean {
  if (id === RAIL_ID) return looks.rail === "strip";
  return looks.columns === "now" && !NOW_STAGES.includes(id);
}

/** Whether a place is open when the owner hasn't picked: the mode doesn't fold it and it has
 *  something in it. `keepOpen` for one with something to show and no cards (Backlog's quick add,
 *  Your check's Accept all line or its limit). An empty one goes to the row in every mode. */
export function autoOpen(looks: BoardLooks, id: string, count: number, keepOpen = false): boolean {
  return !modeFolds(looks, id) && (count > 0 || keepOpen);
}

/** A mode's picks after the owner opens (`open` true) or folds a place, kept only where they differ
 *  from what the mode does now (`auto`): folding a column Focus folds forgets the open, and opening
 *  a full column the owner hid forgets the hide. */
export function pickColumn(picks: ColumnPicks, mode: DisplayMode, id: string, open: boolean, auto: boolean): ColumnPicks {
  const mine: Record<string, ColumnPick> = { ...picks[mode] };
  if (open === auto) delete mine[id];
  else mine[id] = open ? "shown" : "hidden";
  const out = { ...picks };
  if (Object.keys(mine).length) out[mode] = mine;
  else delete out[mode];
  return out;
}

const idsPicked = (picks: ModePicks, pick: ColumnPick): ReadonlySet<string> => new Set(Object.keys(picks).filter((id) => picks[id] === pick));

/** A grouped board band's column in a mode with the owner's picks in it: what the mode folds, and
 *  what the owner opened or hid (mu-84), the same columns the plain board folds. */
export function bandFold(looks: BoardLooks, picks: ModePicks, id: string): ColumnFold {
  return columnFold(looks.columns, id, idsPicked(picks, "shown"), idsPicked(picks, "hidden"));
}

/** The picks after a click on a band column's head or strip: a folded one opens, an open one folds,
 *  kept only where that differs from the mode's own fold. */
export function toggleBand(picks: ColumnPicks, mode: DisplayMode, looks: BoardLooks, id: string): ColumnPicks {
  return pickColumn(picks, mode, id, bandFold(looks, picks[mode] ?? {}, id) === "folded", !modeFolds(looks, id));
}

/** The plain project board (mu-84): every place, Paused to Recently finished, is open or a chip in
 *  the row above the board; open is the owner's pick, else `autoOpen`. The grid holds only the open
 *  columns, each at most OPEN_COLUMN_MAX wide, and the row the rest, in board order. App draws
 *  exactly this, so the board the owner looks at is the one the tests check. */
export function plainBoard(looks: BoardLooks, picks: ModePicks, count: (id: string) => number, keepOpen: (id: string) => boolean) {
  const auto = new Map(ROW_ORDER.map((id) => [id, autoOpen(looks, id, count(id), keepOpen(id))]));
  const isOpen = (id: string) => {
    const pick = picks[id];
    return pick ? pick === "shown" : auto.get(id) === true;
  };
  const open = BOARD_COLUMNS.filter(isOpen);
  return {
    isOpen,
    /** What the mode does with the place now, for `pickColumn`. */
    auto: (id: string) => auto.get(id) === true,
    open,
    row: ROW_ORDER.filter((id) => !isOpen(id)),
    /** The grid's tracks; with nothing open, one track for the line that says so. */
    tracks: open.length ? open.map(() => columnTrack("open")).join(" ") : "minmax(0, 1fr)",
  };
}
