import { describe, expect, it } from "vitest";
import muRouter from "../../commands/mu.md?raw";
import { AREAS, pressed, SHORTCUTS } from "./shortcuts";
import { SLASH, shortcutsPage } from "./shortcutsPage";

// Every source file of the window, as text: the key handlers are found in them.
const sources = Object.fromEntries(
  Object.entries(import.meta.glob<string>("./*.{ts,tsx}", { query: "?raw", import: "default", eager: true }))
    .filter(([path]) => !path.endsWith(".test.ts") && path !== "./shortcuts.ts")
    .map(([path, text]) => [path.slice(2), text]),
);

/** Every key a file compares an event's key with: `e.key === "Escape"`, `key !== "Enter"` (a
 *  helper's parameter), `e.key.toLowerCase() === "s"`. A field of something else (`c.key`, a
 *  table column's) isn't a key press. */
function keysHandledIn(text: string): string[] {
  return [...text.matchAll(/(?:\be\.|(?<![\w.]))key(?:\.toLowerCase\(\))?\s*[!=]==\s*"([^"]+)"/g)].map((m) => m[1]);
}

/** Every shortcut a file reads from the table: `pressed(e, "close-pane")`. */
function idsReadIn(text: string): string[] {
  return [...text.matchAll(/\bpressed\(\s*\w+(?:\.nativeEvent)?\s*,\s*"([^"]+)"\s*\)/g)].map((m) => m[1]);
}

describe("the shortcut table", () => {
  it("lists every key a handler in the window acts on, for that file", () => {
    const missing: string[] = [];
    for (const [file, text] of Object.entries(sources)) {
      for (const key of keysHandledIn(text)) {
        if (!SHORTCUTS.some((s) => s.codes.includes(key) && s.files.includes(file))) missing.push(`${file}: ${key}`);
      }
    }
    expect(missing).toEqual([]);
  });

  it("finds the keys of App.tsx, NewTask.tsx, TaskPanel.tsx, EditBody.tsx, keys.ts and FilterBar.tsx", () => {
    // The scan above has something to check in the files the owner named, and in the window's
    // shortcuts and the filter bar (task 0032).
    for (const file of ["App.tsx", "NewTask.tsx", "TaskPanel.tsx", "EditBody.tsx", "keys.ts", "FilterBar.tsx"]) {
      expect(keysHandledIn(sources[file]).length + idsReadIn(sources[file]).length, file).toBeGreaterThan(0);
    }
  });

  it("names only files that really handle the key, by its name or through the table", () => {
    const wrong: string[] = [];
    for (const s of SHORTCUTS) {
      for (const file of s.files) {
        const text = sources[file];
        if (text === undefined) wrong.push(`${s.id}: no file ${file}`);
        else if (!idsReadIn(text).includes(s.id) && !s.codes.some((c) => keysHandledIn(text).includes(c))) wrong.push(`${s.id}: ${file}`);
      }
    }
    expect(wrong).toEqual([]);
  });

  it("knows every shortcut a handler reads through it", () => {
    const ids = new Set(SHORTCUTS.map((s) => s.id));
    const unknown = Object.entries(sources).flatMap(([file, text]) => idsReadIn(text).filter((id) => !ids.has(id)).map((id) => `${file}: ${id}`));
    expect(unknown).toEqual([]);
    expect(ids.size).toBe(SHORTCUTS.length);
  });

  it("puts every shortcut in an area the page has", () => {
    for (const s of SHORTCUTS) expect(AREAS, s.id).toContain(s.area);
  });
});

describe("pressed", () => {
  const ev = (key: string, mod = false) => ({ key, metaKey: mod, ctrlKey: false });
  it("matches the key, and ⌘ (or Ctrl) when the shortcut needs it", () => {
    expect(pressed(ev("n", true), "new-task")).toBe(true);
    expect(pressed({ key: "n", metaKey: false, ctrlKey: true }, "new-task")).toBe(true);
    expect(pressed(ev("n"), "new-task")).toBe(false);
    expect(pressed(ev("N", true), "new-task")).toBe(false);
    expect(pressed(ev("Enter", true), "send")).toBe(true);
    expect(pressed(ev("Enter"), "send")).toBe(false);
  });
  it("takes ⌘F with ⌘ alone: Ctrl+F moves a text field's caret, ⌃⌘F is full screen", () => {
    expect(pressed(ev("f", true), "search")).toBe(true);
    expect(pressed({ key: "f", metaKey: false, ctrlKey: true }, "search")).toBe(false);
    expect(pressed({ key: "f", metaKey: true, ctrlKey: true }, "search")).toBe(false);
    expect(pressed(ev("f"), "search")).toBe(false);
  });
  it("takes Esc with or without a modifier, as the handlers always have", () => {
    expect(pressed(ev("Escape"), "close-pane")).toBe(true);
    expect(pressed(ev("Escape", true), "close-pane")).toBe(true);
    expect(pressed(ev("Enter"), "close-pane")).toBe(false);
  });
});

describe("the Shortcuts page", () => {
  const page = shortcutsPage();
  it("shows every shortcut, with its keys, what it does and where", () => {
    for (const s of SHORTCUTS) {
      expect(page).toContain(`| ${s.keys} | ${s.what} | ${s.where} |`);
    }
  });
  it("says which shortcut doesn't take Ctrl for ⌘", () => {
    expect(page).toContain("Ctrl works too wherever ⌘ is shown, except ⌘F, ⌘+, ⌘− and ⌘0, which take ⌘ only.");
  });
  it("has a section per area and the slash commands", () => {
    for (const a of AREAS) expect(page).toContain(`\n## ${a}\n`);
    expect(page).toContain("\n## Slash commands in Claude Code\n");
    for (const r of SLASH) expect(page).toContain(`| \`${r.route}\` |`);
  });
  it("is the copy in docs/manual that GitHub shows (run `pnpm test -u` after changing the table)", async () => {
    await expect(page).toMatchFileSnapshot("../../docs/manual/06-shortcuts.md");
  });
});

describe("the slash commands", () => {
  /** The routes in the router's table: `| \`/mu add <text>\` | owner | …`. */
  const routes = [...muRouter.matchAll(/^\| `(\/mu[^`]*)` \| (\w+) \|/gm)].map((m) => ({ route: m[1], who: m[2] }));
  it("are every route of /mu, with who it's for", () => {
    expect(routes.length).toBeGreaterThan(10);
    expect(SLASH.map((r) => ({ route: r.route, who: r.who }))).toEqual(routes);
  });
});
