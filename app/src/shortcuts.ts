// Every keyboard shortcut in the window, in one table: the key handlers read it (`pressed`), the
// manual's Shortcuts page is made from it (shortcutsPage.ts), and a test fails when a handler acts
// on a key the table doesn't list for that file (shortcuts.test.ts).

/** The Shortcuts page's sections, in order. */
export const AREAS = ["Anywhere", "Search and views", "Writing and sending", "The task pane", "The board", "Projects"] as const;
export type Area = (typeof AREAS)[number];

export type Shortcut = {
  id: string;
  /** How the page writes the keys. */
  keys: string;
  /** The `KeyboardEvent.key` values it answers to. */
  codes: readonly string[];
  /** Needs ⌘ or Ctrl (true), ⌘ alone with Ctrl not held ("cmd"), or neither (false). */
  mod: boolean | "cmd";
  what: string;
  where: string;
  area: Area;
  /** The source files whose handlers act on it. */
  files: readonly string[];
};

const TABLE = [
  { id: "new-task", keys: "⌘N", codes: ["n"], mod: true, what: "Open New task in the pane beside the board", where: "Anywhere in the window", area: "Anywhere", files: ["keys.ts"] },
  { id: "close-pane", keys: "Esc", codes: ["Escape"], mod: false, what: "Close the pane: a task, a note or New task", where: "Anywhere while the pane is open, except in the search box or a view's name, which take Esc for themselves", area: "Anywhere", files: ["keys.ts"] },
  { id: "search", keys: "⌘F", codes: ["f"], mod: "cmd", what: "Put the caret in the search box, with its words selected so typing replaces them. Ctrl+F is left to text fields, where it moves the caret", where: "Anywhere in the window. From a tab without a search box, it opens the board first", area: "Anywhere", files: ["keys.ts"] },
  { id: "focus-mode", keys: "⌘.", codes: ["."], mod: true, what: "Turn Focus on, or back off to the mode you were in (the display modes are in Settings, Display)", where: "Anywhere in the window", area: "Anywhere", files: ["keys.ts"] },
  { id: "zoom-in", keys: "⌘+", codes: ["=", "+"], mod: "cmd", what: "Make everything larger: the board, the panes and the manual (the size is remembered; also in Settings, Appearance, Text size)", where: "Anywhere in the window", area: "Anywhere", files: ["zoom.ts"] },
  { id: "zoom-out", keys: "⌘−", codes: ["-", "_"], mod: "cmd", what: "Make everything smaller", where: "Anywhere in the window", area: "Anywhere", files: ["zoom.ts"] },
  { id: "zoom-reset", keys: "⌘0", codes: ["0"], mod: "cmd", what: "Back to 100%", where: "Anywhere in the window", area: "Anywhere", files: ["zoom.ts"] },
  { id: "resize-pane", keys: "← →", codes: ["ArrowLeft", "ArrowRight"], mod: false, what: "Widen or narrow the pane by 40 pixels (the width is remembered)", where: "The pane's edge, once Tab has put the focus on it", area: "Anywhere", files: ["App.tsx"] },
  { id: "send", keys: "⌘↩", codes: ["Enter"], mod: true, what: "Create the task, send the answer or the comment, file the note, or send to the helper", where: "New task's description, an answer box, the comment box, a note's File as task form, the Helper", area: "Writing and sending", files: ["NewTask.tsx", "TaskPanel.tsx", "Comments.tsx", "NotePanel.tsx", "Helper.tsx"] },
  { id: "save-write-up", keys: "⌘S", codes: ["s"], mod: true, what: "Save the write-up", where: "A task's write-up, after Edit", area: "Writing and sending", files: ["writeUp.ts"] },
  { id: "discard-text", keys: "Esc, twice", codes: ["Escape"], mod: false, what: "Leave the editor or empty the comment box. With typed text, the first Esc only warns and the second throws the text away", where: "A task's write-up after Edit, and the comment box", area: "Writing and sending", files: ["EditBody.tsx", "Comments.tsx", "writeUp.ts"] },
  { id: "rename-cancel", keys: "Esc", codes: ["Escape"], mod: false, what: "Stop renaming and keep the old title (↩ saves the new one)", where: "A task's title, after clicking it", area: "The task pane", files: ["PaneHeader.tsx"] },
  { id: "choice-close", keys: "Esc", codes: ["Escape"], mod: false, what: "Fold the choices back without changing the value", where: "Priority and Type, while their choices are laid out", area: "The task pane", files: ["Sidebar.tsx"] },
  { id: "estimate-save", keys: "↩", codes: ["Enter"], mod: false, what: "Save the estimate (leaving the field saves it too)", where: "The Est fields: agent time, test time, tokens", area: "The task pane", files: ["Sidebar.tsx"] },
  { id: "tag-add", keys: "↩ or ,", codes: ["Enter", ","], mod: false, what: "Add the tag you typed", where: "The tag field, in a task or in New task", area: "The task pane", files: ["Tags.tsx"] },
  { id: "image-step", keys: "← →", codes: ["ArrowLeft", "ArrowRight"], mod: false, what: "Show the previous or next image", where: "A task's image, opened large", area: "The task pane", files: ["Sidebar.tsx"] },
  { id: "image-close", keys: "Esc", codes: ["Escape"], mod: false, what: "Close the large image", where: "A task's image, opened large", area: "The task pane", files: ["Sidebar.tsx"] },
  { id: "search-clear", keys: "Esc", codes: ["Escape"], mod: false, what: "Clear the search, or leave the box when it's already empty. It never closes the pane", where: "The search box above Board, Table, Timeline and Graph", area: "Search and views", files: ["keys.ts"] },
  { id: "view-save", keys: "↩", codes: ["Enter"], mod: false, what: "Save the search, filters and sort as a view under the name you typed", where: "A view's name, after + Save view", area: "Search and views", files: ["FilterBar.tsx"] },
  { id: "view-cancel", keys: "Esc", codes: ["Escape"], mod: false, what: "Stop naming the view; nothing is saved", where: "A view's name, after + Save view", area: "Search and views", files: ["FilterBar.tsx"] },
  { id: "quick-add", keys: "↩", codes: ["Enter"], mod: false, what: "Add the line to the backlog as a note", where: "Add to the backlog…, at the top of the Backlog column", area: "The board", files: ["Backlog.tsx"] },
  { id: "quick-add-clear", keys: "Esc", codes: ["Escape"], mod: false, what: "Empty the field", where: "Add to the backlog…, at the top of the Backlog column", area: "The board", files: ["Backlog.tsx"] },
  { id: "accept-all-cancel", keys: "Esc", codes: ["Escape"], mod: false, what: "Don't accept them: nothing changes", where: "Accept all's confirm, in Your check's Just accept group (the board and Waiting for you)", area: "The board", files: ["EffortGroups.tsx"] },
  { id: "label-save", keys: "↩", codes: ["Enter"], mod: false, what: "Save the label or the category (leaving the field saves it too; in Settings it's saved as you type)", where: "A project's label or category in Settings, Projects, and a project's name in the sidebar after a double click", area: "Projects", files: ["InlineField.tsx", "ProjectList.tsx"] },
  { id: "label-revert", keys: "Esc", codes: ["Escape"], mod: false, what: "Keep the label or the category as it was before you started typing", where: "A project's label or category in Settings, Projects, and a project's name in the sidebar after a double click", area: "Projects", files: ["InlineField.tsx", "ProjectList.tsx"] },
] as const satisfies readonly Shortcut[];

/** A shortcut's id: `pressed` takes only one the table has. */
export type ShortcutId = (typeof TABLE)[number]["id"];
export const SHORTCUTS: readonly Shortcut[] = TABLE;

type KeyEvent = { key: string; metaKey: boolean; ctrlKey: boolean };

/** Whether the modifiers a shortcut needs are held. ⌘-only (⌘F): on macOS Ctrl+F moves a text
 *  field's caret forward a character, and ⌃⌘F is full screen. */
const modsHeld = (s: Shortcut, e: KeyEvent) => (s.mod === "cmd" ? e.metaKey && !e.ctrlKey : !s.mod || e.metaKey || e.ctrlKey);

/** Whether a key event is this shortcut: one of its keys, exactly, with ⌘ or Ctrl held when it
 *  needs one (⌘ alone for a ⌘-only one). Other modifiers don't matter, as they never have in
 *  these handlers. */
export function pressed(e: KeyEvent, id: ShortcutId): boolean {
  return SHORTCUTS.some((s) => s.id === id && s.codes.includes(e.key) && modsHeld(s, e));
}
