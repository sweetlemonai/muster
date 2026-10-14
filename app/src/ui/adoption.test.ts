// Screens use the atoms for their patterns (0053), as Drover's do: no old per-screen classes for
// what an atom draws, and no plain <button> left to its container's styling, which is a Button.
import { describe, expect, it } from "vitest";

const screens = import.meta.glob<string>("../*.tsx", { query: "?raw", import: "default", eager: true });

const OLD = new Set(["chip", "tab", "status-pill", "badge", "seg", "primary"]);

/** The text of every `className=` value: a string, or each string and template literal inside a
 *  `{…}` expression, however many lines it spans. */
export function classNameTexts(text: string): string[] {
  const out: string[] = [];
  for (const m of text.matchAll(/className=/g)) {
    let i = m.index + m[0].length;
    if (text[i] === '"') {
      out.push(text.slice(i + 1, text.indexOf('"', i + 1)));
      continue;
    }
    if (text[i] !== "{") continue;
    let depth = 0;
    const start = i;
    for (; i < text.length; i++) {
      if (text[i] === "{") depth++;
      else if (text[i] === "}" && --depth === 0) break;
    }
    const expr = text.slice(start, i + 1);
    for (const s of expr.matchAll(/"([^"]*)"|`([^`]*)`/g)) out.push((s[1] ?? s[2]).replace(/\$\{[^}]*\}/g, " "));
  }
  return out;
}

export function oldClasses(text: string): string[] {
  return classNameTexts(text).flatMap((t) => t.split(/\s+/).filter((c) => OLD.has(c)));
}

/** Every raw `<button …>` opening tag with no className, however many lines it spans. */
export function bareButtons(text: string): string[] {
  const out: string[] = [];
  for (const m of text.matchAll(/<button(?=[\s>])/g)) {
    let depth = 0;
    let i = m.index + m[0].length;
    for (; i < text.length; i++) {
      if (text[i] === "{") depth++;
      else if (text[i] === "}") depth--;
      else if (text[i] === '"' && depth === 0) i = text.indexOf('"', i + 1);
      else if (text[i] === ">" && depth === 0) break;
    }
    const tag = text.slice(m.index, i + 1);
    if (!/\sclassName=/.test(tag)) out.push(tag.replace(/\s+/g, " ").slice(0, 80));
  }
  return out;
}

const all = (f: (text: string) => string[]) => Object.entries(screens).flatMap(([file, text]) => f(text).map((hit) => `${file}: ${hit}`));

describe("the checks themselves", () => {
  it("find old classes in strings, templates and multi-line expressions", () => {
    expect(oldClasses('<b className="chip on">')).toEqual(["chip"]);
    expect(oldClasses("<b className={`status-pill s-${st}`}>")).toEqual(["status-pill"]);
    expect(oldClasses('<b\n  className={\n    on\n      ? "tab on"\n      : "x"\n  }\n>')).toEqual(["tab"]);
    expect(oldClasses('<b className="chip-row pick-chip">')).toEqual([]);
  });

  it("find plain buttons, in one line or many, and not named ones or atoms", () => {
    expect(bareButtons('<button onClick={() => go(a > b)}>Go</button>')).toEqual(["<button onClick={() => go(a > b)}>"]);
    expect(bareButtons('<button\n  type="button"\n  title="a > b"\n  disabled\n>')).toEqual(['<button type="button" title="a > b" disabled >']);
    expect(bareButtons('<button\n  type="button"\n  className="link"\n>')).toEqual([]);
    expect(bareButtons("<Button onClick={go}>Go</Button><buttons />")).toEqual([]);
  });
});

describe("screens", () => {
  it("are all checked", () => {
    expect(Object.keys(screens).length).toBeGreaterThan(50);
    expect(Object.keys(screens)).toEqual(expect.arrayContaining(["../App.tsx", "../TaskPanel.tsx", "../Settings.tsx"]));
  });

  it("don't draw an atom's pattern with their own classes", () => {
    expect(all(oldClasses)).toEqual([]);
  });

  it("have no plain buttons: a bordered one is a Button, anything else names its class", () => {
    expect(all(bareButtons)).toEqual([]);
  });
});
