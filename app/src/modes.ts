// Display modes (0038): ADHD, Standard and Focus decide how much of each element shows. One table
// says, for every element a mode could change, what it does in each mode (the same in all three
// says so) and how to get back what a mode hides. Components ask it through `useLook` and never read the mode themselves:
// a test fails when a component reads an element the table doesn't have, or the mode around it
// (modes.test.ts). Everything else, the colour code, no overlays and fixed places, is the same in
// all three (docs/DESIGN.md).
import { createContext, useContext } from "react";
import { tn, type Key } from "./i18n";
import { STAGES } from "./lib";

export type DisplayMode = "adhd" | "standard" | "focus";
export const DEFAULT_MODE: DisplayMode = "adhd";

/** The modes in the order Settings shows them, each with what it's for. The words are read in the
 *  language shown each time (shell.json `display.modes`, mu-146). */
export const MODES: readonly { readonly id: DisplayMode; readonly label: string; readonly says: string }[] = (["adhd", "standard", "focus"] as const).map((id) => ({
  id,
  get label() {
    return tn("shell", `display.modes.${id}.label`);
  },
  get says() {
    return tn("shell", `display.modes.${id}.says`);
  },
}));

/** What an element does in one mode: `how` is what the component reads, `says` and `reach` are
 *  Settings' words. `reach` says how to get back what the mode hides, and only ADHD hides nothing. */
type Behaviour = { how: string; readonly says: string; readonly reach?: string };
type Entry = { readonly label: string } & Record<DisplayMode, Behaviour>;

/** A behaviour with Settings' words (shell.json `display.elements`), read in the language shown
 *  each time they're asked for. */
function does<H extends string>(how: H, says: Key<"shell">, reach?: Key<"shell">): { how: H; readonly says: string; readonly reach?: string } {
  const words = {
    how,
    get says() {
      return tn("shell", says);
    },
  };
  if (reach === undefined) return words;
  return Object.defineProperty(words, "reach", { enumerable: true, get: () => tn("shell", reach) });
}

const TABLE = {
  filters: {
    get label() {
      return tn("shell", "display.elements.filters.label");
    },
    adhd: does("open", "display.elements.filters.adhd"),
    standard: does("folded", "display.elements.filters.standard", "display.elements.filters.standardReach"),
    focus: does("on-key", "display.elements.filters.focus", "display.elements.filters.focusReach"),
  },
  choices: {
    get label() {
      return tn("shell", "display.elements.choices.label");
    },
    adhd: does("all", "display.elements.choices.adhd"),
    standard: does("value", "display.elements.choices.standard", "display.elements.choices.reach"),
    focus: does("value", "display.elements.choices.focus", "display.elements.choices.reach"),
  },
  "card-lines": {
    get label() {
      return tn("shell", "display.elements.cardLines.label");
    },
    adhd: does("all", "display.elements.cardLines.adhd"),
    standard: does("selected", "display.elements.cardLines.standard", "display.elements.cardLines.standardReach"),
    focus: does("status", "display.elements.cardLines.focus", "display.elements.cardLines.focusReach"),
  },
  "done-line": {
    get label() {
      return tn("shell", "display.elements.doneLine.label");
    },
    adhd: does("show", "display.elements.doneLine.adhd"),
    standard: does("show", "display.elements.doneLine.standard"),
    focus: does("show", "display.elements.doneLine.focus"),
  },
  "holder-ribbon": {
    get label() {
      return tn("shell", "display.elements.holderRibbon.label");
    },
    adhd: does("show", "display.elements.holderRibbon.adhd"),
    standard: does("show", "display.elements.holderRibbon.standard"),
    focus: does("show", "display.elements.holderRibbon.focus"),
  },
  columns: {
    get label() {
      return tn("shell", "display.elements.columns.label");
    },
    adhd: does("all", "display.elements.columns.all"),
    standard: does("all", "display.elements.columns.all"),
    focus: does("now", "display.elements.columns.focus", "display.elements.columns.focusReach"),
  },
  rail: {
    get label() {
      return tn("shell", "display.elements.rail.label");
    },
    adhd: does("open", "display.elements.rail.open"),
    standard: does("open", "display.elements.rail.open"),
    focus: does("strip", "display.elements.rail.focus", "display.elements.rail.focusReach"),
  },
  "pane-sections": {
    get label() {
      return tn("shell", "display.elements.paneSections.label");
    },
    adhd: does("open", "display.elements.paneSections.adhd"),
    standard: does("folded", "display.elements.paneSections.folded", "display.elements.paneSections.reach"),
    focus: does("folded", "display.elements.paneSections.folded", "display.elements.paneSections.reach"),
  },
  sidebar: {
    get label() {
      return tn("shell", "display.elements.sidebar.label");
    },
    adhd: does("full", "display.elements.sidebar.full"),
    standard: does("full", "display.elements.sidebar.full"),
    focus: does("narrow", "display.elements.sidebar.focus", "display.elements.sidebar.focusReach"),
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
