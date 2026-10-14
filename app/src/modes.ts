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
  columns: {
    label: "Board columns",
    adhd: { how: "all", says: "Every stage, and Paused before Backlog; an empty one stays a narrow strip that says empty" },
    standard: { how: "all", says: "Every stage, and Paused before Backlog; an empty one stays a narrow strip that says empty" },
    focus: { how: "now", says: "Building, Testing and Your check; the others and Paused (the grouped board's Paused and Backlog bands too) are thin strips with their counts, needs you and blocked", reach: "Click a strip to open that column" },
  },
  rail: {
    label: "Recently finished",
    adhd: { how: "open", says: "Docked beside the board: what merged or was accepted, by day, with time and tokens" },
    standard: { how: "open", says: "Docked beside the board: what merged or was accepted, by day, with time and tokens" },
    focus: { how: "strip", says: "A thin strip beside the board with its name and count", reach: "Click the strip to open it in place; Fold puts it back" },
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

/** A board column in a mode: "fixed" when the mode doesn't fold it (it shows as ever), "folded"
 *  to a strip, or "opened" from its strip by a click. */
export function columnFold(how: Look<"columns">, stage: string, opened: ReadonlySet<string>): ColumnFold {
  if (how === "all" || NOW_STAGES.includes(stage)) return "fixed";
  return opened.has(stage) ? "opened" : "folded";
}
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

/** The plain project board (mu-83): each column's state from its fold, its count and whether it's
 *  kept open with no cards, the grid's track list, and each column's classes. App draws exactly
 *  this, so the board the owner looks at is the one the tests check. */
export function plainBoard(fold: (id: string) => ColumnFold, count: (id: string) => number, keepOpen: (id: string) => boolean) {
  const states = new Map(BOARD_COLUMNS.map((id) => [id, columnState(fold(id), count(id), keepOpen(id))]));
  const state = (id: string): ColumnState => states.get(id) ?? "empty";
  return { state, tracks: BOARD_COLUMNS.map((id) => columnTrack(state(id))).join(" "), className: (id: string) => columnClass(id, state(id)) };
}
