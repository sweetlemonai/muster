import { describe, expect, it } from "vitest";
import { focusSearchBox, isSearchKey, paneEscape, searchBoxKey, searchView, windowKey } from "./keys";

// A window with the New task pane open and the filter bar's search box, wired the way App,
// NewTask and FilterBar wire these functions. `press` runs a key through them in the browser's
// order: the pane's capture listener on the window, the focused field's own handler (React's,
// at the root), then the window's shortcuts; stopPropagation ends it where it's called.

type El = { name: string };
const titleInput: El = { name: "New task title" };
const searchBox: El = { name: "search box" };
const viewName: El = { name: "Name for this view" };
const board: El = { name: "board" };

/** `newTask`: New task is the open pane; otherwise a task's pane, which has no Esc listener of
 *  its own (only the window's Esc closes it). */
function windowWith(search: string, newTask = true) {
  const s = { newTaskOpen: true, search, blurred: false, searched: 0, log: [] as string[] };
  const pane = { contains: (n: unknown) => n === titleInput };
  const closePane = () => {
    s.newTaskOpen = false;
    s.log.push("pane closed");
  };
  const press = (target: El, key: string, mods: { metaKey?: boolean; ctrlKey?: boolean } = {}) => {
    let stopped = false;
    let prevented = false;
    const e = {
      key,
      target,
      currentTarget: { blur: () => (s.blurred = true) },
      metaKey: mods.metaKey ?? false,
      ctrlKey: mods.ctrlKey ?? false,
      stopPropagation: () => (stopped = true),
      preventDefault: () => (prevented = true),
    };
    // NewTask's capture listener.
    if (newTask) paneEscape(e, pane, closePane);
    // The search box's own handler.
    if (!stopped && target === searchBox) searchBoxKey(e, s.search, () => (s.search = ""));
    // The "Name for this view" field stops Esc itself (FilterBar).
    if (!stopped && target === viewName && key === "Escape") e.stopPropagation();
    // App's shortcuts.
    if (!stopped) windowKey(e, { closePane, newTask: () => s.log.push("new task"), search: () => s.searched++, focusMode: () => s.log.push("focus") });
    return { stopped, prevented };
  };
  return { s, press };
}

describe("Esc with the New task pane open", () => {
  it("in the search box clears the search and keeps New task open", () => {
    const w = windowWith("flick");
    w.press(searchBox, "Escape");
    expect(w.s.search).toBe("");
    expect(w.s.newTaskOpen).toBe(true);
  });
  it("in an empty search box leaves the box and keeps New task open", () => {
    const w = windowWith("");
    const r = w.press(searchBox, "Escape");
    expect(r.stopped).toBe(true);
    expect(w.s.blurred).toBe(true);
    expect(w.s.newTaskOpen).toBe(true);
  });
  it("in the name for a view keeps New task open", () => {
    const w = windowWith("");
    w.press(viewName, "Escape");
    expect(w.s.newTaskOpen).toBe(true);
  });
  it("in the pane closes it, once", () => {
    const w = windowWith("flick");
    w.press(titleInput, "Escape");
    expect(w.s.newTaskOpen).toBe(false);
    expect(w.s.log).toEqual(["pane closed"]);
    expect(w.s.search).toBe("flick");
  });
  it("anywhere else still closes the pane, through the window's Esc", () => {
    const w = windowWith("flick");
    w.press(board, "Escape");
    expect(w.s.newTaskOpen).toBe(false);
  });
  it("lets other keys typed in the pane through", () => {
    const w = windowWith("");
    expect(w.press(titleInput, "a").stopped).toBe(false);
    expect(w.s.newTaskOpen).toBe(true);
  });
});

describe("Esc in the search box with a task pane open", () => {
  it("doesn't close the pane when the box is empty either", () => {
    const w = windowWith("", false);
    const r = w.press(searchBox, "Escape");
    expect(r.stopped).toBe(true);
    expect(w.s.blurred).toBe(true);
    expect(w.s.log).not.toContain("pane closed");
  });
  it("with words clears them and doesn't close the pane", () => {
    const w = windowWith("flick", false);
    w.press(searchBox, "Escape");
    expect(w.s.search).toBe("");
    expect(w.s.log).not.toContain("pane closed");
  });
});

describe("⌘F", () => {
  it("is ⌘F only: not Ctrl+F (forward a character in a text field), not ⌃⌘F (full screen)", () => {
    expect(isSearchKey({ key: "f", metaKey: true, ctrlKey: false })).toBe(true);
    expect(isSearchKey({ key: "f", metaKey: false, ctrlKey: true })).toBe(false);
    expect(isSearchKey({ key: "f", metaKey: true, ctrlKey: true })).toBe(false);
    expect(isSearchKey({ key: "g", metaKey: true, ctrlKey: false })).toBe(false);
  });
  it("goes to the search box from anywhere and keeps the browser's own find away", () => {
    const w = windowWith("");
    const r = w.press(board, "f", { metaKey: true });
    expect(w.s.searched).toBe(1);
    expect(r.prevented).toBe(true);
  });
  it("Ctrl+F in an input, a textarea or editable text does nothing special", () => {
    const fields: El[] = [titleInput, { name: "textarea" }, { name: "contenteditable" }, searchBox];
    for (const f of fields) {
      const w = windowWith("flick");
      const r = w.press(f, "f", { ctrlKey: true });
      expect(r, f.name).toEqual({ stopped: false, prevented: false });
      expect(w.s.searched, f.name).toBe(0);
      expect(w.s.search, f.name).toBe("flick");
      expect(w.s.newTaskOpen, f.name).toBe(true);
    }
  });
  it("stays in the project view showing, the Graph too, and goes to the board from any other tab", () => {
    for (const v of ["board", "table", "timeline", "graph"]) expect(searchView(v), v).toBe(v);
    for (const v of ["helper", "waiting", "activity", "problems", "projects", "help"]) expect(searchView(v), v).toBe("board");
  });
  it("focuses the box, then selects its words (select alone may not focus in WKWebView)", () => {
    const calls: string[] = [];
    focusSearchBox({ focus: () => calls.push("focus"), select: () => calls.push("select") });
    expect(calls).toEqual(["focus", "select"]);
    expect(() => focusSearchBox(null)).not.toThrow();
  });
});
