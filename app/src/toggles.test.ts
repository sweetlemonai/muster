import { describe, expect, it } from "vitest";
import appSource from "./App.tsx?raw";
import { setLocale } from "./i18n/runtime";
import { look, MODES, type DisplayMode } from "./modes";
import {
  flip, focusInside, isShown, loadToggles, moveFocus, pickToggle, REGIONS, regionDefault, regionOf, storeToggles, toggled, toggleRegion, toggleWords, TOGGLES_KEY, viewMenu, type TogglePicks,
} from "./toggles";

// The shared toggle (mu-167): the sidebar, Recently finished and the filters each shown or hidden,
// the mode's default unless the owner picked otherwise in that mode, kept on this Mac the way the
// board's columns are (mu-84).

const store = (items: Record<string, string> = {}) => ({
  items,
  getItem(k: string) {
    return k in this.items ? this.items[k] : null;
  },
  setItem(k: string, v: string) {
    this.items[k] = v;
  },
});
const ALL: DisplayMode[] = MODES.map((m) => m.id);

describe("the toggle model's defaults", () => {
  it("shows the sidebar in every mode until the owner hides it; Focus keeps it narrow, not hidden", () => {
    for (const m of ALL) expect(regionDefault("sidebar", m), m).toBe(true);
    expect(look("sidebar", "focus")).toBe("narrow");
  });
  it("takes Recently finished's and the filters' defaults from the modes' table", () => {
    expect(ALL.map((m) => regionDefault("recent", m))).toEqual(ALL.map((m) => look("rail", m) === "open"));
    expect(ALL.map((m) => regionDefault("filters", m))).toEqual(ALL.map((m) => look("filters", m) === "open"));
  });
  it("is shown as the mode says with no pick", () => {
    for (const m of ALL) for (const r of REGIONS) expect(isShown({}, m, r), `${m} ${r}`).toBe(regionDefault(r, m));
  });
});

describe("a pick", () => {
  it("is kept only where it differs from the mode's default", () => {
    const hidden = pickToggle({}, "adhd", "sidebar", false);
    expect(hidden).toEqual({ adhd: { sidebar: "hidden" } });
    expect(isShown(hidden, "adhd", "sidebar")).toBe(false);
    // Showing it again is the default: the pick goes, and the mode with it.
    expect(pickToggle(hidden, "adhd", "sidebar", true)).toEqual({});
    // Picking the default changes nothing.
    expect(pickToggle({}, "standard", "sidebar", true)).toEqual({});
  });
  it("is per mode: hiding in ADHD leaves Standard and Focus as they were, and a mode switch resets nothing", () => {
    const picks = pickToggle({}, "adhd", "sidebar", false);
    expect(isShown(picks, "adhd", "sidebar")).toBe(false);
    expect(isShown(picks, "standard", "sidebar")).toBe(true);
    expect(isShown(picks, "focus", "sidebar")).toBe(true);
    const both = pickToggle(picks, "focus", "sidebar", false);
    expect(both).toEqual({ adhd: { sidebar: "hidden" }, focus: { sidebar: "hidden" } });
    // The mode change is App's setModes alone: the picks aren't touched by it.
    expect(appSource).toMatch(/const changeMode = useCallback\(\s*\(f: \(s: ModeState\) => ModeState\) => \{\s*setModes\(f\);\s*setBarOut\(false\);\s*\}/);
  });
  it("keeps the other regions' picks in the mode", () => {
    const picks: TogglePicks = { focus: { filters: "shown" } };
    expect(pickToggle(picks, "focus", "sidebar", false)).toEqual({ focus: { filters: "shown", sidebar: "hidden" } });
  });
  it("toggles to the other side of what shows now", () => {
    let p: TogglePicks = {};
    p = toggleRegion(p, "standard", "sidebar");
    expect(isShown(p, "standard", "sidebar")).toBe(false);
    p = toggleRegion(p, "standard", "sidebar");
    expect(p).toEqual({});
  });
});

describe("the kept picks", () => {
  it("survive a reload", () => {
    const s = store();
    const picks: TogglePicks = { adhd: { sidebar: "hidden" }, focus: { sidebar: "hidden", filters: "shown" } };
    storeToggles(s, picks);
    expect(JSON.parse(s.items[TOGGLES_KEY])).toEqual(picks);
    expect(loadToggles(s)).toEqual(picks);
  });
  it("load a missing, broken or unknown value as no pick", () => {
    expect(loadToggles(store())).toEqual({});
    expect(loadToggles(store({ [TOGGLES_KEY]: "{not json" }))).toEqual({});
    expect(loadToggles(store({ [TOGGLES_KEY]: "[]" }))).toEqual({});
    expect(loadToggles(store({ [TOGGLES_KEY]: "null" }))).toEqual({});
    expect(loadToggles(store({ [TOGGLES_KEY]: "7" }))).toEqual({});
    expect(
      loadToggles(store({ [TOGGLES_KEY]: JSON.stringify({ compact: { sidebar: "hidden" }, adhd: { sidebar: "folded", nowhere: "hidden", recent: "hidden" }, standard: [], focus: { sidebar: true } }) })),
    ).toEqual({ adhd: { recent: "hidden" } });
  });
  it("drop a stored pick that says what the mode does anyway", () => {
    expect(loadToggles(store({ [TOGGLES_KEY]: JSON.stringify({ adhd: { sidebar: "shown" } }) }))).toEqual({});
  });
  it("load as no pick when storage itself throws", () => {
    const broken = {
      getItem(): string | null {
        throw new Error("denied");
      },
    };
    expect(loadToggles(broken)).toEqual({});
  });
  it("are read once at start and stored on each change, in App", () => {
    expect(appSource).toContain("useState<TogglePicks>(() => loadToggles(localStorage))");
    expect(appSource).toContain("storeToggles(localStorage, toggles)");
  });
});

describe("what the menu and a press send", () => {
  it("is a region the window knows, or nothing", () => {
    expect(regionOf("sidebar")).toBe("sidebar");
    expect(regionOf("recent")).toBe("recent");
    expect(regionOf("filters")).toBe("filters");
    for (const bad of ["Sidebar", "", null, undefined, 1, { region: "sidebar" }, ["sidebar"]]) expect(regionOf(bad), String(bad)).toBeNull();
  });
  it("toggles the sidebar exactly once per event, and ignores the regions not wired yet", () => {
    let p: TogglePicks = {};
    p = toggled(p, "adhd", "sidebar");
    expect(isShown(p, "adhd", "sidebar")).toBe(false);
    // Recently finished and the filters are the follow-up's (mu-169): their items are disabled.
    expect(toggled(p, "adhd", "recent")).toBe(p);
    expect(toggled(p, "adhd", "filters")).toBe(p);
    expect(toggled(p, "adhd", "nonsense")).toBe(p);
  });
  it("has one path for the key: the menu's accelerator, no keydown handler of the window's", () => {
    // A ⌃⌘S handled in the window too would toggle twice.
    expect(appSource).toContain('listen<unknown>("view-toggle"');
    expect(appSource).not.toMatch(/key\s*===\s*"s"/);
  });
  it("leaves ⌃⌘S to the menu everywhere: the one S handler (the write-up's ⌘S) takes ⌘ or Ctrl, never both", () => {
    const sources = import.meta.glob<string>("./*.{ts,tsx}", { query: "?raw", import: "default", eager: true });
    const sKeys = Object.entries(sources).filter(([path, text]) => !path.includes(".test.") && /key(?:\.toLowerCase\(\))?\s*===\s*"s"/.test(text)).map(([path]) => path);
    expect(sKeys).toEqual(["./writeUp.ts"]);
    expect(sources["./writeUp.ts"]).toContain('e.key.toLowerCase() === "s" && e.metaKey !== e.ctrlKey && !e.altKey');
  });
  it("tells the menu what's shown now, three booleans", () => {
    expect(viewMenu({ adhd: { sidebar: "hidden" } }, "adhd")).toEqual({ sidebar: false, recent: true, filters: true });
    expect(viewMenu({ adhd: { sidebar: "hidden" } }, "standard")).toEqual({ sidebar: true, recent: true, filters: false });
  });
});

describe("one toggle, as App runs it (flip)", () => {
  it("hides, says so, and asks for the focus on Show when it was inside", () => {
    const done = flip({}, "adhd", "sidebar", true);
    expect(done).toEqual({ picks: { adhd: { sidebar: "hidden" } }, said: "Sidebar hidden", refocus: false });
  });
  it("shows again, says so, and asks for the focus on Hide", () => {
    const done = flip({ adhd: { sidebar: "hidden" } }, "adhd", "sidebar", true);
    expect(done).toEqual({ picks: {}, said: "Sidebar shown", refocus: true });
  });
  it("leaves the focus where it is when it was elsewhere (a menu click from the board)", () => {
    expect(flip({}, "focus", "sidebar", false)?.refocus).toBeNull();
  });
  it("two events in a row toggle twice, each from the last one's picks", () => {
    const one = flip({}, "standard", "sidebar", false);
    const two = one && flip(one.picks, "standard", "sidebar", false);
    expect(one?.said).toBe("Sidebar hidden");
    expect(two).toEqual({ picks: {}, said: "Sidebar shown", refocus: null });
  });
  it("does nothing for a region not wired yet or a payload that isn't one", () => {
    for (const p of ["recent", "filters", "", null, 7, { region: "sidebar" }]) expect(flip({}, "adhd", p, true), String(p)).toBeNull();
  });
  it("in a mode, from that mode's pick: hidden in Focus shows from Focus's", () => {
    const done = flip({ focus: { sidebar: "hidden" }, adhd: { sidebar: "hidden" } }, "focus", "sidebar", false);
    expect(done?.picks).toEqual({ adhd: { sidebar: "hidden" } });
  });
  it("is what App's flipView runs", () => {
    expect(appSource).toContain("const done = flip(was, mode, payload, focusInside(document.activeElement, sidebarRef.current));");
    expect(appSource).toContain("const moved = refocus.current === sidebarShown || lost;");
  });
});

describe("the announcement", () => {
  it("says what changed, in English and in فارسی", async () => {
    expect(toggleWords("sidebar", false)).toBe("Sidebar hidden");
    expect(toggleWords("sidebar", true)).toBe("Sidebar shown");
    await setLocale("fa");
    try {
      expect(toggleWords("sidebar", false)).toBe("نوار کناری پنهان شد");
      expect(toggleWords("sidebar", true)).toBe("نوار کناری نمایش داده شد");
    } finally {
      await setLocale("en");
    }
  });
});

describe("where the focus goes", () => {
  // A stand-in for the DOM: an element contains itself and what's listed under it.
  type El = { id: string; under: El[]; focused: number; contains: (n: unknown) => boolean; focus: () => void };
  const el = (id: string, under: El[] = []): El => {
    const e: El = {
      id,
      under,
      focused: 0,
      contains: (n) => n === e || under.some((u) => u.contains(n)),
      focus: () => {
        e.focused++;
      },
    };
    return e;
  };
  it("moves to the rail's Show button when it was inside the sidebar that hides", () => {
    const row = el("project");
    const sidebar = el("sidebar", [row]);
    const show = el("show");
    // Asked before the sidebar's rows go: after, the focus is on the page's body.
    const inside = focusInside(row, sidebar);
    expect(inside).toBe(true);
    expect(moveFocus(inside, show)).toBe(true);
    expect(show.focused).toBe(1);
  });
  it("moves back to the Hide button when it was inside the rail that shows", () => {
    const railShow = el("show");
    const rail = el("rail", [railShow]);
    const hide = el("hide");
    expect(moveFocus(focusInside(railShow, rail), hide)).toBe(true);
    expect(hide.focused).toBe(1);
  });
  it("stays where it is when it was elsewhere (the board, a menu click)", () => {
    const sidebar = el("sidebar", [el("project")]);
    const show = el("show");
    expect(focusInside(el("card"), sidebar)).toBe(false);
    expect(focusInside(null, sidebar)).toBe(false);
    expect(focusInside(el("card"), null)).toBe(false);
    expect(moveFocus(false, show)).toBe(false);
    expect(show.focused).toBe(0);
  });
  it("does nothing without a target", () => {
    expect(moveFocus(true, null)).toBe(false);
  });
  it("is asked before the toggle and moved after it renders, in App", () => {
    expect(appSource).toMatch(/focusInside\(document\.activeElement, sidebarRef\.current\)/);
    expect(appSource).toMatch(/moveFocus\(/);
    // A mode switch that hides or shows it moves the focus too (e2e/rail.mjs checks it in WebKit).
    expect(appSource).toContain("onBlur={(e) => (inSidebar.current = focusInside(e.relatedTarget, sidebarRef.current))}");
  });
});
