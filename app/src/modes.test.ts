import { describe, expect, it } from "vitest";
import { cardLines, columnFold, DEFAULT_MODE, ELEMENTS, loadMode, look, MODE_KEY, MODES, NOW_STAGES, pickMode, storeMode, toggleFocus, type DisplayMode, type ElementId } from "./modes";

// Every source file of the window, as text: the elements a component reads are found in them.
const sources = Object.fromEntries(
  Object.entries(import.meta.glob<string>("./*.{ts,tsx}", { query: "?raw", import: "default", eager: true }))
    .filter(([path]) => !/\.test\.tsx?$/.test(path) && path !== "./modes.ts")
    .map(([path, text]) => [path.slice(2), text]),
);

/** Every element a file asks the table about: `useLook("filters")`, `look("columns", mode)`. */
function elementsReadIn(text: string): string[] {
  return [...text.matchAll(/\b(?:useLook|look)\(\s*"([^"]+)"/g)].map((m) => m[1]);
}

/** Ways a file could read the mode around the table: the context itself, imported under another
 *  name, or a mode's name in any quotes (`mode === "focus"`, 'focus', `focus`). Only modes.ts may. */
function rawReadsIn(text: string): string[] {
  const names = MODES.map((m) => m.id).join("|");
  return [...text.matchAll(new RegExp(`useContext\\(\\s*Mode\\s*\\)|\\bMode\\s+as\\s+\\w+|(["'\`])(?:${names})\\1`, "g"))].map((m) => m[0]);
}

const IDS = Object.keys(ELEMENTS) as ElementId[];
const ALL: DisplayMode[] = MODES.map((m) => m.id);

describe("the display modes' table (0038)", () => {
  it("offers ADHD, Standard and Focus, in that order, ADHD by default", () => {
    expect(MODES.map((m) => m.label)).toEqual(["ADHD", "Standard", "Focus"]);
    expect(DEFAULT_MODE).toBe("adhd");
    for (const m of MODES) expect(m.says.length, m.id).toBeGreaterThan(20);
  });

  it("gives every element a behaviour and words for each mode", () => {
    for (const id of IDS) {
      expect(ELEMENTS[id].label, id).toBeTruthy();
      for (const m of ALL) {
        expect(ELEMENTS[id][m].how, `${id} ${m}`).toBeTruthy();
        expect(ELEMENTS[id][m].says, `${id} ${m}`).toBeTruthy();
      }
    }
  });

  it("is the one place a mode is read: every element a component reads has an entry (AC2)", () => {
    const unknown = Object.entries(sources).flatMap(([file, text]) => elementsReadIn(text).filter((id) => !(id in ELEMENTS)).map((id) => `${file}: ${id}`));
    expect(unknown).toEqual([]);
  });

  it("finds the reads it checks: a read the table lacks would be caught", () => {
    expect(elementsReadIn('const how = useLook("tags");')).toEqual(["tags"]);
    expect(elementsReadIn('look("columns", mode)')).toEqual(["columns"]);
    expect(rawReadsIn('if (mode === "focus") x(); const m = useContext(Mode);')).toEqual(['"focus"', "useContext(Mode)"]);
    expect(rawReadsIn("if (m === 'adhd' || m === `standard`) x();")).toEqual(["'adhd'", "`standard`"]);
    expect(rawReadsIn('import { Mode as Shown } from "./modes"; useContext(Shown);')).toEqual(["Mode as Shown"]);
    // Words that only contain a mode's name aren't reads.
    expect(rawReadsIn('const f = "focused"; setFocus(true); className="standard-x";')).toEqual([]);
  });

  it("has no component that reads the mode around the table", () => {
    const raw = Object.entries(sources).flatMap(([file, text]) => rawReadsIn(text).map((r) => `${file}: ${r}`));
    expect(raw).toEqual([]);
  });

  it("has no entry nothing reads", () => {
    const read = new Set(Object.values(sources).flatMap(elementsReadIn));
    expect(IDS.filter((id) => !read.has(id))).toEqual([]);
  });

  it("says, for everything a mode hides, how to get it back in one click or key (AC3)", () => {
    for (const id of IDS) {
      expect(ELEMENTS[id].adhd.reach, `${id}: ADHD hides nothing`).toBeUndefined();
      for (const m of ALL) {
        if (ELEMENTS[id][m].how === ELEMENTS[id].adhd.how) continue;
        expect(ELEMENTS[id][m].reach, `${id} ${m}`).toBeTruthy();
      }
    }
  });

  it("keeps ADHD as today's screens: nothing folded, narrowed or behind a click", () => {
    expect(IDS.map((id) => [id, look(id, "adhd")])).toEqual([
      ["filters", "open"],
      ["choices", "all"],
      ["card-lines", "all"],
      ["done-line", "show"],
      ["holder-ribbon", "show"],
      ["columns", "all"],
      ["rail", "open"],
      ["pane-sections", "open"],
      ["sidebar", "full"],
      ["session-groups", "all"],
      ["session-history", "open"],
    ]);
  });

  it("folds what the task says in Standard and Focus", () => {
    expect(IDS.map((id) => [id, look(id, "standard"), look(id, "focus")])).toEqual([
      ["filters", "folded", "on-key"],
      ["choices", "value", "value"],
      ["card-lines", "selected", "status"],
      ["done-line", "show", "show"],
      ["holder-ribbon", "show", "show"],
      ["columns", "all", "now"],
      ["rail", "open", "strip"],
      ["pane-sections", "folded", "folded"],
      ["sidebar", "full", "narrow"],
      ["session-groups", "all", "needs-input"],
      ["session-history", "folded", "folded"],
    ]);
  });
});

describe("the holder ribbon in the modes' table (0097)", () => {
  it("shows in every mode: who holds a card is status, and in Standard's brief card the one place it shows", () => {
    for (const m of ALL) expect(look("holder-ribbon", m), m).toBe("show");
    expect(ELEMENTS["holder-ribbon"].label).toBe("Holder ribbon");
    for (const m of ALL) expect(ELEMENTS["holder-ribbon"][m].says, m).toMatch(/Shaping, Building or Testing.*backlog note/);
    expect(ELEMENTS["holder-ribbon"].standard.says).toContain("the one place the holder shows");
    // Nothing folded, so nothing to get back.
    for (const m of ALL) expect(ELEMENTS["holder-ribbon"][m].reach, m).toBeUndefined();
  });
  it("is read by the ribbons themselves, on cards and notes", () => {
    expect(elementsReadIn(sources["HolderRibbon.tsx"])).toEqual(["holder-ribbon", "holder-ribbon"]);
  });
});

describe("the mode as it's kept and switched", () => {
  const store = (items: Record<string, string>) => ({
    items,
    getItem: (k: string) => items[k] ?? null,
    setItem: (k: string, v: string) => {
      items[k] = v;
    },
  });

  it("reads ADHD when nothing or something unknown is kept", () => {
    expect(loadMode(store({}))).toEqual({ mode: "adhd", before: "adhd" });
    expect(loadMode(store({ [MODE_KEY]: "compact" }))).toEqual({ mode: "adhd", before: "adhd" });
    expect(loadMode(store({ [MODE_KEY]: "{}" }))).toEqual({ mode: "adhd", before: "adhd" });
  });

  it("remembers the mode and the one before Focus", () => {
    const s = store({});
    storeMode(s, { mode: "focus", before: "standard" });
    expect(loadMode(s)).toEqual({ mode: "focus", before: "standard" });
    // A kept "before" that's Focus itself (a hand edit) goes back to ADHD.
    expect(loadMode(store({ [MODE_KEY]: "focus", [`${MODE_KEY}.before`]: "focus" }))).toEqual({ mode: "focus", before: "adhd" });
  });

  it("⌘. turns Focus on from any mode and back off to that mode", () => {
    expect(toggleFocus({ mode: "standard", before: "standard" })).toEqual({ mode: "focus", before: "standard" });
    expect(toggleFocus({ mode: "focus", before: "standard" })).toEqual({ mode: "standard", before: "standard" });
    expect(toggleFocus({ mode: "adhd", before: "adhd" })).toEqual({ mode: "focus", before: "adhd" });
    expect(toggleFocus(toggleFocus({ mode: "adhd", before: "adhd" }))).toEqual({ mode: "adhd", before: "adhd" });
  });

  it("picking Focus in Settings keeps the mode to go back to; picking another forgets Focus", () => {
    expect(pickMode({ mode: "standard", before: "standard" }, "focus")).toEqual({ mode: "focus", before: "standard" });
    expect(pickMode({ mode: "focus", before: "standard" }, "focus")).toEqual({ mode: "focus", before: "standard" });
    expect(pickMode({ mode: "focus", before: "standard" }, "adhd")).toEqual({ mode: "adhd", before: "adhd" });
  });
});

describe("what a mode does to the board and the cards", () => {
  it("shows every card line in ADHD, the selected card's in Standard, a status line in Focus", () => {
    expect(cardLines("all", false)).toBe("all");
    expect(cardLines("selected", true)).toBe("all");
    expect(cardLines("selected", false)).toBe("brief");
    expect(cardLines("status", true)).toBe("status");
  });

  it("folds every column but Building, Testing and Your check in Focus, until one is opened", () => {
    expect(NOW_STAGES).toEqual(["building", "testing", "your-check"]);
    const none = new Set<string>();
    for (const s of ["backlog", "shaping", "ready", "building", "testing", "your-check", "done"]) expect(columnFold("all", s, none), s).toBe("fixed");
    expect(columnFold("now", "building", none)).toBe("fixed");
    expect(columnFold("now", "done", none)).toBe("folded");
    expect(columnFold("now", "done", new Set(["done"]))).toBe("opened");
    expect(columnFold("now", "backlog", new Set(["done"]))).toBe("folded");
  });
});
