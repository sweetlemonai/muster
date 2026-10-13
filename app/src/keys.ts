// The window's keys, as functions of the event's fields they read, so the tests can press them
// without a DOM. The browser runs them in this order: the panes' capture listeners on the window
// (`paneEscape`), then the focused field's own handler (`searchBoxKey`), then the window's
// shortcuts (`windowKey`) unless something before stopped the key. The keys themselves come from
// the shortcut table (shortcuts.ts), which the manual's Shortcuts page is made from.

import { pressed } from "./shortcuts";

type Mods = { key: string; metaKey: boolean; ctrlKey: boolean };
type Stoppable = Mods & { stopPropagation: () => void };

/** ⌘F, and only ⌘F: on macOS Ctrl+F moves a text field's caret forward a character, and ⌃⌘F is
 *  full screen. */
export const isSearchKey = (e: Mods) => pressed(e, "search");

/** The project views, which show the filter bar and its search box. */
export const PROJECT_VIEWS = ["board", "table", "timeline", "graph"] as const;

/** Where ⌘F goes: it stays in a project view (Board, Table, Timeline or Graph), and goes to the
 *  board from any other tab. */
export function searchView<V extends string>(view: V): V | "board" {
  return (PROJECT_VIEWS as readonly string[]).includes(view) ? view : "board";
}

export type WindowActions ={ closePane: () => void; newTask: () => void; search: () => void };

/** The window's shortcuts: Esc closes the pane, ⌘N opens New task, ⌘F goes to the search box. */
export function windowKey(e: Mods & { preventDefault: () => void }, act: WindowActions): void {
  if (pressed(e, "close-pane")) act.closePane();
  if (pressed(e, "new-task")) {
    e.preventDefault();
    act.newTask();
  }
  if (isSearchKey(e)) {
    e.preventDefault();
    act.search();
  }
}

/** Puts the caret in the search box with its words selected, so typing replaces them. Focus
 *  first: select() alone may not move the focus in WKWebView. */
export function focusSearchBox(el: { focus: () => void; select: () => void } | null): void {
  if (!el) return;
  el.focus();
  el.select();
}

/** The New task pane's Esc: closes the pane when the key was typed in it. Esc in a field outside
 *  (the search box, a view's name) belongs to that field and must not throw the draft away; Esc
 *  that no field takes still closes the pane, through the window's own Esc. */
export function paneEscape(e: Stoppable & { target: unknown }, pane: { contains: (n: Node | null) => boolean } | null, onClose: () => void): void {
  if (!pressed(e, "close-pane") || !pane?.contains(e.target as Node | null)) return;
  e.stopPropagation();
  onClose();
}

/** Esc in the search box: clears the search, or leaves the box when it's already empty. Either
 *  way the key stops here, so the window's Esc doesn't also close the open pane. */
export function searchBoxKey(e: Stoppable & { currentTarget: { blur: () => void } }, search: string, clear: () => void): void {
  if (!pressed(e, "search-clear")) return;
  e.stopPropagation();
  if (search) clear();
  else e.currentTarget.blur();
}
