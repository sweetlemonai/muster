// One class name, one component (mu-81): mu-57's project colour picker reused the theme picker's
// `.swatches`/`.swatch`, and its later rules turned every theme card into a 16px dot. A class that
// two components use must not get rules that set the same property two ways.
import { describe, expect, it } from "vitest";
import css from "./styles.css?raw";

const sources = import.meta.glob<string>(["./*.tsx", "./ui/*.tsx", "!./**/*.test.tsx"], {
  query: "?raw",
  import: "default",
  eager: true,
});

type Decls = Record<string, string>;

/** Each top-level rule whose selector list holds a lone class (`.x`), with its declarations. */
function classRules(text: string): Map<string, Decls[]> {
  const src = text.replace(/\/\*[\s\S]*?\*\//g, "");
  const rules = new Map<string, Decls[]>();
  let depth = 0;
  let head = "";
  let body = "";
  let selectors: string[] = [];
  for (const c of src) {
    if (c === "{") {
      if (depth === 0) {
        selectors = head.split(",").map((s) => s.trim());
        body = "";
      }
      depth++;
      head = "";
    } else if (c === "}") {
      depth--;
      if (depth === 0) {
        const decls: Decls = {};
        for (const d of body.split(";")) {
          const at = d.indexOf(":");
          if (at > 0) decls[d.slice(0, at).trim()] = d.slice(at + 1).trim();
        }
        for (const s of selectors) {
          if (/^\.[A-Za-z][\w-]*$/.test(s)) rules.set(s.slice(1), [...(rules.get(s.slice(1)) ?? []), decls]);
        }
      }
      head = "";
    } else if (depth === 0) head += c;
    else if (depth === 1) body += c;
  }
  return rules;
}

/** The properties a class's rules set two different ways. */
function clashes(rules: Decls[]): string[] {
  const values = new Map<string, Set<string>>();
  for (const r of rules) for (const [k, v] of Object.entries(r)) values.set(k, (values.get(k) ?? new Set()).add(v));
  return [...values].filter(([, v]) => v.size > 1).map(([k]) => k);
}

/** The class names a file's `className=` values can hold: every word of the string and template
 *  literals inside each value (`"swatch on"`, `{on ? "a b" : "a"}`, `` {`x ${y}`} ``). */
function classNames(text: string): Set<string> {
  const out = new Set<string>();
  for (const m of text.matchAll(/className=(?:"([^"]*)"|\{((?:[^{}]|\{[^{}]*\})*)\})/g)) {
    const value = m[1] ?? m[2] ?? "";
    const literals = m[1] !== undefined ? [value] : [...value.matchAll(/"([^"]*)"|'([^']*)'|`([^`]*)`/g)].map((l) => l[1] ?? l[2] ?? l[3] ?? "");
    for (const l of literals) for (const w of l.replace(/\$\{[^}]*\}/g, " ").split(/\s+/)) if (w) out.add(w);
  }
  return out;
}

/** The component files that use `cls` as a whole class name. */
function users(cls: string, files: Record<string, string>): string[] {
  return Object.entries(files)
    .filter(([, text]) => classNames(text).has(cls))
    .map(([name]) => name);
}

/** Classes two components share whose rules disagree: the bug mu-81 fixed. */
function shared(text: string, files: Record<string, string>): string[] {
  return [...classRules(text)]
    .filter(([cls, rules]) => rules.length > 1 && clashes(rules).length > 0 && users(cls, files).length > 1)
    .map(([cls]) => cls);
}

describe("one class name, one component (mu-81)", () => {
  it("no class two components use gets rules that set a property two ways", () => {
    expect(shared(css, sources)).toEqual([]);
  });

  it("the theme picker and the project colour picker have their own classes", () => {
    const rules = classRules(css);
    expect(rules.get("swatches")).toHaveLength(1);
    expect(rules.get("swatch")).toHaveLength(1);
    expect(users("swatches", sources)).toEqual(["./Appearance.tsx"]);
    expect(users("proj-swatches", sources)).toEqual(["./ProjectSwatches.tsx"]);
  });

  it("catches the clash mu-57 made", () => {
    const before = `${css}\n.swatches { display: flex; gap: 5px; }\n.swatch { width: 16px; border-radius: 50%; }\n`;
    const files = { ...sources, "./Other.tsx": 'return <div className="swatches"><button className="swatch" /></div>;' };
    expect(shared(before, files)).toEqual(["swatches", "swatch"]);
  });

  it("lets one component override its own rule, and reads class tokens whole", () => {
    const text = ".top { gap: 16px; }\n.top { gap: 10px; }\n";
    expect(shared(text, { "./A.tsx": '<header className="top">', "./B.tsx": '<i className="card-top">' })).toEqual([]);
  });
});
