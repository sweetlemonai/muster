// Every keyboard shortcut in the window, in one table: the key handlers read it (`pressed`), the
// manual's Shortcuts page is made from it (shortcutsPage.ts), and a test fails when a handler acts
// on a key the table doesn't list for that file (shortcuts.test.ts). What each one does and where
// is help.json's (mu-148), in every language.
import { tn } from "./i18n";

/** The Shortcuts page's sections, in order; their headings are help.json's `areas.<id>`. */
export const AREAS = ["anywhere", "search", "writing", "pane", "board", "projects"] as const;
export type Area = (typeof AREAS)[number];

type Row = {
  id: string;
  /** How the page writes the keys. */
  keys: string;
  /** The `KeyboardEvent.key` values it answers to. */
  codes: readonly string[];
  /** Needs ⌘ or Ctrl (true), ⌘ alone with Ctrl not held ("cmd"), ⌥ without ⌘ or Ctrl ("alt"), or
   *  none of them (false). */
  mod: boolean | "cmd" | "alt";
  area: Area;
  /** The source files whose handlers act on it. */
  files: readonly string[];
};

/** A row with its words: what it does and where, read from help.json (mu-148) in the language
 *  shown each time they're asked, so they follow a switch. Only the Help tab and the Shortcuts
 *  page read them (help.json comes with the Help tab); the key handlers need none of it. */
export type Shortcut = Row & { readonly what: string; readonly where: string };

const TABLE = [
  { id: "new-task", keys: "⌘N", codes: ["n"], mod: true, area: "anywhere", files: ["keys.ts"] },
  { id: "close-pane", keys: "Esc", codes: ["Escape"], mod: false, area: "anywhere", files: ["keys.ts"] },
  { id: "search", keys: "⌘F", codes: ["f"], mod: "cmd", area: "anywhere", files: ["keys.ts"] },
  { id: "focus-mode", keys: "⌘.", codes: ["."], mod: true, area: "anywhere", files: ["keys.ts"] },
  // The View menu's accelerator takes ⌃⌘S (app_menu.rs, mu-167), its one path: no handler here.
  { id: "toggle-sidebar", keys: "⌃⌘S", codes: [], mod: "cmd", area: "anywhere", files: [] },
  { id: "zoom-in", keys: "⌘+", codes: ["=", "+"], mod: "cmd", area: "anywhere", files: ["zoom.ts"] },
  { id: "zoom-out", keys: "⌘−", codes: ["-", "_"], mod: "cmd", area: "anywhere", files: ["zoom.ts"] },
  { id: "zoom-reset", keys: "⌘0", codes: ["0"], mod: "cmd", area: "anywhere", files: ["zoom.ts"] },
  { id: "resize-pane", keys: "← →", codes: ["ArrowLeft", "ArrowRight"], mod: false, area: "anywhere", files: ["splitter.ts"] },
  { id: "resize-pane-big", keys: "Shift ← →", codes: ["ArrowLeft", "ArrowRight"], mod: false, area: "anywhere", files: ["splitter.ts"] },
  { id: "send", keys: "⌘↩", codes: ["Enter"], mod: true, area: "writing", files: ["NewTask.tsx", "AnswerBox.tsx", "Comments.tsx", "NotePanel.tsx", "Helper.tsx", "StageForms.tsx"] },
  { id: "save-write-up", keys: "⌘S", codes: ["s"], mod: true, area: "writing", files: ["writeUp.ts"] },
  { id: "discard-text", keys: "Esc, twice", codes: ["Escape"], mod: false, area: "writing", files: ["EditBody.tsx", "Comments.tsx", "writeUp.ts"] }, // i18n-ignore: key names; the page reads its words from help.json
  { id: "rename-cancel", keys: "Esc", codes: ["Escape"], mod: false, area: "pane", files: ["PaneHeader.tsx"] },
  { id: "choice-close", keys: "Esc", codes: ["Escape"], mod: false, area: "pane", files: ["Sidebar.tsx"] },
  { id: "estimate-save", keys: "↩", codes: ["Enter"], mod: false, area: "pane", files: ["Sidebar.tsx"] },
  { id: "tag-add", keys: "↩ or ,", codes: ["Enter", ","], mod: false, area: "pane", files: ["Tags.tsx"] },
  { id: "release-cancel", keys: "Esc", codes: ["Escape"], mod: false, area: "pane", files: ["Sidebar.tsx"] },
  { id: "search-clear", keys: "Esc", codes: ["Escape"], mod: false, area: "search", files: ["keys.ts"] },
  { id: "view-save", keys: "↩", codes: ["Enter"], mod: false, area: "search", files: ["FilterBar.tsx"] },
  { id: "view-cancel", keys: "Esc", codes: ["Escape"], mod: false, area: "search", files: ["FilterBar.tsx"] },
  { id: "quick-add", keys: "↩", codes: ["Enter"], mod: false, area: "board", files: ["Backlog.tsx"] },
  { id: "quick-add-clear", keys: "Esc", codes: ["Escape"], mod: false, area: "board", files: ["Backlog.tsx"] },
  { id: "accept-all-cancel", keys: "Esc", codes: ["Escape"], mod: false, area: "board", files: ["EffortGroups.tsx"] },
  { id: "grid-move", keys: "← → ↑ ↓", codes: ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"], mod: false, area: "board", files: ["FeatureGrid.tsx"] },
  { id: "grid-ends", keys: "Home, End", codes: ["Home", "End"], mod: false, area: "board", files: ["FeatureGrid.tsx"] }, // i18n-ignore: key names
  { id: "grid-enter", keys: "↩ or Space", codes: ["Enter", " "], mod: false, area: "board", files: ["FeatureGrid.tsx"] },
  { id: "grid-leave", keys: "Esc", codes: ["Escape"], mod: false, area: "board", files: ["FeatureGrid.tsx"] },
  { id: "label-save", keys: "↩", codes: ["Enter"], mod: false, area: "projects", files: ["InlineField.tsx", "ProjectList.tsx", "SidebarProjects.tsx"] },
  { id: "label-revert", keys: "Esc", codes: ["Escape"], mod: false, area: "projects", files: ["InlineField.tsx", "ProjectList.tsx", "SidebarProjects.tsx"] },
  { id: "move-up", keys: "⌥↑", codes: ["ArrowUp"], mod: "alt", area: "projects", files: ["SidebarProjects.tsx"] },
  { id: "move-down", keys: "⌥↓", codes: ["ArrowDown"], mod: "alt", area: "projects", files: ["SidebarProjects.tsx"] },
  { id: "heading-rename", keys: "↩", codes: ["Enter"], mod: false, area: "projects", files: ["SidebarProjects.tsx"] },
  { id: "heading-fold", keys: "←", codes: ["ArrowLeft"], mod: false, area: "projects", files: ["SidebarProjects.tsx"] },
  { id: "heading-unfold", keys: "→", codes: ["ArrowRight"], mod: false, area: "projects", files: ["SidebarProjects.tsx"] },
  { id: "colour-close", keys: "Esc", codes: ["Escape"], mod: false, area: "projects", files: ["ProjectSwatches.tsx"] },
] as const satisfies readonly Row[];

/** A shortcut's id: `pressed` takes only one the table has. */
export type ShortcutId = (typeof TABLE)[number]["id"];
export const SHORTCUTS: readonly Shortcut[] = TABLE.map((row) => ({
  ...row,
  get what() {
    return tn("help", `shortcuts.${row.id}.what`);
  },
  get where() {
    return tn("help", `shortcuts.${row.id}.where`);
  },
}));

type KeyEvent = { key: string; metaKey: boolean; ctrlKey: boolean; altKey?: boolean };

/** Whether the modifiers a shortcut needs are held. ⌘-only (⌘F): on macOS Ctrl+F moves a text
 *  field's caret forward a character, and ⌃⌘F is full screen. */
const modsHeld = (s: Shortcut, e: KeyEvent) =>
  s.mod === "cmd" ? e.metaKey && !e.ctrlKey : s.mod === "alt" ? Boolean(e.altKey) && !e.metaKey && !e.ctrlKey : !s.mod || e.metaKey || e.ctrlKey;

/** Whether a key event is this shortcut: one of its keys, exactly, with ⌘ or Ctrl held when it
 *  needs one (⌘ alone for a ⌘-only one), and ⌥ without either for an ⌥ one. Other modifiers don't
 *  matter, as they never have in these handlers. */
export function pressed(e: KeyEvent, id: ShortcutId): boolean {
  return SHORTCUTS.some((s) => s.id === id && s.codes.includes(e.key) && modsHeld(s, e));
}
