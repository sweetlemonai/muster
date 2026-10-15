// Right to left (Drover's task 0020): a value filled into a sentence (a
// path, a branch, a command, a name) keeps its own direction in Persian,
// isolated by the layer itself, so no translation has to remember the
// marks; and a Latin run written into a Persian string that starts with
// punctuation (`~/.ssh`, `.muster/config.toml`, `/login`) has a mark before
// it, or its dot or tilde lands on the wrong side.
import { afterEach, describe, expect, it } from "vitest";
import css from "../styles.css?raw";
import bidiCases from "./fixtures/bidi-controls.json";
import { isolate, sentences, tl } from "./lib";
import { dirOf, LOCALES } from "./locales";
import { FSI, PDI, setLocale, translate, type Messages } from "./runtime";

afterEach(async () => {
  await setLocale("en");
});

describe("filled-in values", () => {
  it("are isolated in Persian, in every namespace", async () => {
    await setLocale("fa");
    const s = translate("fa", "shell", "language.failed", { error: "~/.config/muster: read-only" });
    expect(s).toContain(`${FSI}~/.config/muster: read-only${PDI}`);
    // Numbers are the language's digits, on their own.
    expect(translate("fa", "native", "tray.waiting", { count: 3 })).not.toContain(FSI);
    expect(tl("sentences", { a: "id_ed25519", b: "" })).toContain(`${FSI}id_ed25519${PDI}`);
    // Nothing to isolate: no marks around nothing.
    expect(tl("sentences", { a: "", b: "" })).not.toContain(FSI);
  });

  it("aren't touched in a left-to-right language", () => {
    for (const l of LOCALES.filter((x) => dirOf(x) === "ltr")) {
      expect(translate(l, "shell", "language.failed", { error: "~/.x" }), l).not.toContain(FSI);
    }
  });

  it("put together in code are isolated the same way", async () => {
    expect(isolate("~/.ssh")).toBe("~/.ssh");
    await setLocale("fa");
    expect(isolate("~/.ssh")).toBe(`${FSI}~/.ssh${PDI}`);
    expect(isolate("")).toBe("");
  });
});

// mu-152 AC9: a value's own bidi controls never reach the text, isolated or
// not, so a title's lone PDI can't close its isolate early, an unclosed LRI
// can't swallow the end, and an override can't reorder the line. The cases
// are i18n.rs's too (fixtures/bidi-controls.json). Unlike the core, the
// window keeps an isolate pair a value opens and closes itself: the layer
// fills its own isolated text into other text all over (`runtime.cleanValue`).
describe("bidi controls in filled-in values", () => {
  const controls = bidiCases.controls.map((c) => String.fromCodePoint(Number.parseInt(c.replace(/^U\+/, ""), 16)));
  const fill = (locale: "en" | "fa", value: string) => translate(locale, "native", "notify.line.own", { id: "mu-1", title: value });
  const iso = (s: string) => (s ? `${FSI}${s}${PDI}` : "");

  it("are the twelve the core drops", () => {
    expect(controls).toHaveLength(12);
    expect(controls.join("")).toMatch(/^[\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069]+$/);
  });

  it("go, each on its own, in Persian and English", async () => {
    await setLocale("fa");
    for (const c of controls) {
      const name = `U+${c.codePointAt(0)?.toString(16).toUpperCase()}`;
      expect(fill("fa", c), name).toBe(`${FSI}mu-1${PDI} : تسک خودتان`);
      expect(fill("en", c), name).toBe("mu-1 : your task");
      expect(fill("fa", `Fix${c} it`), name).toBe(`${FSI}mu-1${PDI} ${FSI}Fix it${PDI}: تسک خودتان`);
      expect(isolate(`a${c}b`), name).toBe(`${FSI}ab${PDI}`);
      expect(isolate(c), name).toBe("");
    }
  });

  it("keep, unlike the core, a pair the value closes itself (the split, pinned)", () => {
    const split = bidiCases.cases.filter((c) => c.window !== c.clean);
    expect(split.length).toBeGreaterThan(0);
    for (const { value, clean, window } of split) {
      expect(isolate(value)).toBe(window);
      expect(isolate(value)).not.toBe(clean);
    }
  });

  it.each(bidiCases.cases)("go from $about", async ({ value, window }) => {
    expect(fill("en", value)).toBe(`mu-1 ${window}: your task`);
    expect(isolate(value)).toBe(window);
    await setLocale("fa");
    expect(fill("fa", value)).toBe(`${FSI}mu-1${PDI} ${iso(window)}: تسک خودتان`);
    expect(isolate(value)).toBe(iso(window));
  });

  it("stay out, while a sentence a translation made keeps its own isolates beside another", async () => {
    await setLocale("fa");
    const one = fill("fa", `${PDI}x`);
    expect(one).toBe(`${FSI}mu-1${PDI} ${FSI}x${PDI}: تسک خودتان`);
    expect(sentences([one, "دو."])).toBe(`${FSI}${one}${PDI} ${FSI}دو.${PDI}`);
  });

  it("keep a title's own closed pair inside its value, where it can't reorder the line", async () => {
    await setLocale("fa");
    /** The line's isolates balance; returns the pairs at the top level (one per value)
     *  and the text outside every isolate (the template's own words). */
    const shape = (s: string) => {
      let depth = 0;
      let top = 0;
      let outside = "";
      for (const c of s) {
        if (c === FSI || c === LRI || c === "\u2067") depth++;
        else if (c === PDI) {
          depth--;
          expect(depth, s).toBeGreaterThanOrEqual(0);
          if (depth === 0) top++;
        } else if (depth === 0) outside += c;
      }
      expect(depth, s).toBe(0);
      return { top, outside };
    };
    for (const title of [`${LRI}evil${PDI} board`, `\u2067one two${PDI}`, `${FSI}${LRI}a${PDI}${PDI}b`]) {
      const line = fill("fa", title);
      expect(line).toBe(`${FSI}mu-1${PDI} ${FSI}${title}${PDI}: تسک خودتان`);
      expect(shape(line)).toEqual({ top: 2, outside: " : تسک خودتان" });
      // Filled again into a line naming its project: still one pair per value, its words in order.
      const at = translate("fa", "native", "notify.inProject", { project: "Muster", line });
      expect(shape(at)).toEqual({ top: 2, outside: "در : " });
    }
  });

  it("keep an isolate pair a value opens and closes itself, and only that", async () => {
    const figure = `${LRI}210k${PDI}`;
    expect(fill("en", `${figure}${PDI} x`)).toBe(`mu-1 ${figure} x: your task`);
    await setLocale("fa");
    expect(isolate(figure)).toBe(`${FSI}${figure}${PDI}`);
    expect(isolate(`${FSI}a${PDI}${PDI}${LRI}b`)).toBe(`${FSI}${FSI}a${PDI}b${PDI}`);
    expect(isolate(`${LRI}${FSI}a${PDI}`)).toBe(`${FSI}${FSI}a${PDI}${PDI}`);
    expect(isolate(`${LRI}a\u202eb${PDI}`)).toBe(`${FSI}${LRI}ab${PDI}${PDI}`);
  });
});

const files = import.meta.glob<Messages>("./locales/fa/*.json", { import: "default", eager: true });
const leaves = (m: Messages, p = ""): [string, string][] => Object.entries(m).flatMap(([k, v]) => (typeof v === "string" ? [[p + k, v] as [string, string]] : leaves(v, `${p}${k}.`)));
/** Marks that set a Latin run's direction: FSI, LRI, LRM. */
const MARKS = new Set(["⁨", "⁦", "‎"]);

/** Latin runs starting with punctuation, outside code, tags and
 *  placeholders, and the character before each. */
export function punctuatedRuns(text: string): { run: string; before: string }[] {
  const bare = text
    .replace(/<code>[\s\S]*?<\/code>/g, " ")
    .replace(/<\/?\w+\s*\/?>/g, " ")
    .replace(/`[^`]*`/g, " ")
    .replace(/\{\{[^}]+\}\}/g, "\u0000");
  const out: { run: string; before: string }[] = [];
  for (const m of bare.matchAll(/[~./][A-Za-z0-9_./~-]*[A-Za-z][A-Za-z0-9_./~-]*/g)) {
    const before = bare[m.index - 1] ?? "";
    // Inside a longer Latin word (CLAUDE.md, https://, origin/main), or a
    // filled-in value's tail ({{home}}/.ssh): its run starts earlier.
    if (/[A-Za-z0-9:/.~_-]/.test(before)) continue;
    if (before === "\u0000") {
      out.push({ run: `{{…}}${m[0]}`, before: bare[m.index - 2] ?? "" });
      continue;
    }
    out.push({ run: m[0], before });
  }
  return out;
}

describe("Latin runs written into Persian strings", () => {
  it("finds the runs it should", () => {
    expect(punctuatedRuns("در ~/.ssh و فایل .gitignore و.muster/config.toml").map((r) => r.run)).toEqual(["~/.ssh", ".gitignore", ".muster/config.toml"]);
    expect(punctuatedRuns("AGENTS.md و <code>~/.ssh</code> origin/main https://x.y </b>")).toEqual([]);
    expect(punctuatedRuns("{{home}}/.ssh/id")).toEqual([{ run: "{{…}}/.ssh/id", before: "" }]);
  });

  it("have a mark before each one that starts with punctuation", () => {
    expect(Object.keys(files).length).toBeGreaterThan(0);
    const bare: string[] = [];
    for (const [path, m] of Object.entries(files)) {
      // The command line's text is printed in a terminal, which shows a mark as a box: it has none.
      if (path === CLI) continue;
      for (const [key, text] of leaves(m)) {
        for (const r of punctuatedRuns(text)) if (!MARKS.has(r.before)) bare.push(`${path} ${key}: ${r.run}`);
      }
    }
    expect(bare).toEqual([]);
  });

  it("have no mark at all in the command line's text, which a terminal shows (mu-152)", () => {
    const cli = files[CLI];
    expect(cli).toBeDefined();
    const marked = leaves(cli ?? {}).filter(([, text]) => /[\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069]/.test(text));
    expect(marked).toEqual([]);
  });
});

const CLI = "./locales/fa/cli.json";

const LRI = "⁦";

/** The keyboard glyphs (⌘ ⌥ ⇧ ⌃) in a string that sit outside an LRI…PDI
 *  pair, or inside code, which is left to right already. */
export function bareShortcuts(text: string): string[] {
  const outside = text.replace(/<code>[\s\S]*?<\/code>/g, " ").replace(new RegExp(`${LRI}[^${LRI}${PDI}]*${PDI}`, "g"), " ");
  return [...outside.matchAll(/[⌘⌥⇧⌃]\S*/g)].map((m) => m[0]);
}

// Bidi-neutral glyphs take the paragraph's direction: in Persian, "(⌘N)"
// shows as "(N⌘)" and "⌥↑" as "↑⌥" unless the run is isolated left to right.
describe("Keyboard shortcuts written into Persian strings", () => {
  it("finds the shortcuts left bare", () => {
    expect(bareShortcuts("تسک جدید (⌘N)")).toEqual(["⌘N)"]);
    expect(bareShortcuts(`تسک جدید (${LRI}⌘N${PDI})`)).toEqual([]);
    expect(bareShortcuts(`${LRI}⌥↑${PDI} ⌥↓ بزنید`)).toEqual(["⌥↓"]);
    expect(bareShortcuts(`${LRI}⌘+ و ⌘−`)).toEqual(["⌘+", "⌘−"]);
    expect(bareShortcuts("<code>⌃⌘F</code> و ⇧")).toEqual(["⇧"]);
  });

  it("isolate every one left to right", () => {
    const bare: string[] = [];
    for (const [path, m] of Object.entries(files)) {
      for (const [key, text] of leaves(m)) {
        for (const run of bareShortcuts(text)) bare.push(`${path} ${key}: ${run}`);
      }
    }
    expect(bare).toEqual([]);
  });
});

/** A rule's body in styles.css, by its exact selector at the start of a line. */
const rule = (sel: string) => new RegExp(`(?:^|\\n)${sel.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")} \\{([^}]*)\\}`).exec(css)?.[1] ?? "";

describe("Right to left in the stylesheet", () => {
  // A path in a bare <code> inside a Persian paragraph (app.noQueues,
  // projects.finds) keeps its `~/` on the left: Drover's rule.
  it("isolates code, kbd and pre left to right", () => {
    // The one LTR-island rule, with the .ltr class every key, branch and path carries (mu-149).
    const body = rule(".ltr, code, kbd, pre");
    expect(body).toMatch(/direction: ltr;/);
    expect(body).toMatch(/unicode-bidi: isolate;/);
  });

  // Mirrored in fa, the Settings groups' divider stays between them and the body.
  it("puts the Settings groups' divider on their inner side, not the window's edge", () => {
    const body = rule(".settings-side");
    expect(body).toMatch(/border-inline-end: 1px solid var\(--line\)/);
    expect(body).not.toMatch(/border-(right|left):/);
  });
});
