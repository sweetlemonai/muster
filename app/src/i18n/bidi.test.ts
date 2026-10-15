// Right to left (Drover's task 0020): a value filled into a sentence (a
// path, a branch, a command, a name) keeps its own direction in Persian,
// isolated by the layer itself, so no translation has to remember the
// marks; and a Latin run written into a Persian string that starts with
// punctuation (`~/.ssh`, `.muster/config.toml`, `/login`) has a mark before
// it, or its dot or tilde lands on the wrong side.
import { afterEach, describe, expect, it } from "vitest";
import css from "../styles.css?raw";
import { isolate, tl } from "./lib";
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
      for (const [key, text] of leaves(m)) {
        for (const r of punctuatedRuns(text)) if (!MARKS.has(r.before)) bare.push(`${path} ${key}: ${r.run}`);
      }
    }
    expect(bare).toEqual([]);
  });
});

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
