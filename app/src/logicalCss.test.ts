// In فارسی the whole layout mirrors (mu-149), so the style sheets and the inline styles say
// "start" and "end", never "left" and "right": the gate fails on a physical property in any CSS
// file under src, or a physical key in a TSX `style={{…}}`. What has to stay physical says why in
// a comment containing `physical:`, right after it on its line or on the line above it; a note
// excuses that one declaration, never the rest of its line or the line after. Ported from Drover's
// src/i18n/logicalCss.test.ts, with the TSX half added.
import { describe, expect, it } from "vitest";

const sheets = import.meta.glob("./**/*.css", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
const components = import.meta.glob(["./**/*.tsx", "!./**/*.test.tsx"], { query: "?raw", import: "default", eager: true }) as Record<string, string>;

/** A value split where `sep` matches outside parentheses (spaces by default). */
function words(value: string, sep = /\s/): string[] {
  const out: string[] = [];
  let depth = 0;
  let cur = "";
  for (const ch of value.trim()) {
    if (ch === "(") depth++;
    if (ch === ")") depth--;
    if (sep.test(ch) && depth === 0) {
      if (cur) out.push(cur);
      cur = "";
    } else cur += ch;
  }
  if (cur) out.push(cur);
  return out;
}

const BOX = /^(margin|padding|inset|scroll-margin|scroll-padding|border-width|border-style|border-color)$/;
const SIDE = /\b(left|right)\b/;

/** A length that isn't zero, or one that can't be read here (a var(), a calc()). */
const moves = (v: string | undefined) => v !== undefined && !/^-?0*\.?0*([a-z%]+)?$/i.test(v.trim());
/** A number below zero: a scale that flips. */
const flips = (v: string | undefined) => v !== undefined && v.trim().startsWith("-");

/** Whether a transform, or a translate or scale property, shifts or flips along the line. */
function horizontal(p: string, value: string): boolean {
  if (p === "translate") return moves(words(value)[0]);
  if (p === "scale") return flips(words(value)[0]);
  if (p !== "transform") return false;
  for (const m of value.matchAll(/\b(translateX|scaleX|translate3d|translate|scale3d|scale|matrix3d|matrix)\(/gi)) {
    // The call's arguments, nested parentheses and all.
    let depth = 1;
    let to = m.index + m[0].length;
    while (to < value.length && depth > 0) {
      if (value[to] === "(") depth++;
      if (value[to] === ")") depth--;
      to++;
    }
    const args = words(value.slice(m.index + m[0].length, to - 1), /,/);
    const fn = m[1].toLowerCase();
    if (fn === "translatex" || fn === "scalex") return true;
    if ((fn === "translate" || fn === "translate3d") && moves(args[0])) return true;
    if ((fn === "scale" || fn === "scale3d") && flips(args[0])) return true;
    // matrix(a, b, c, d, tx, ty): a below zero flips, tx shifts; matrix3d's are its 1st and 13th.
    if (fn === "matrix" && (flips(args[0]) || moves(args[4]))) return true;
    if (fn === "matrix3d" && (flips(args[0]) || moves(args[12]))) return true;
  }
  return false;
}

/** Why a declaration is physical, or null when it reads the same either way round. */
export function physical(prop: string, value: string): string | null {
  const p = prop.toLowerCase();
  if (p.startsWith("--")) return null;
  if (/^(margin|padding|border|inset|scroll-margin|scroll-padding)-(left|right)(-|$)/.test(p)) return "side longhand";
  if (/^border-(top|bottom)-(left|right)-radius$/.test(p)) return "corner longhand";
  if (p === "left" || p === "right") return "offset";
  if (/^(text-align|text-align-last|float|clear)$/.test(p) && SIDE.test(value)) return "left/right keyword";
  if (/^(background|background-position|background-position-x|mask|mask-position|object-position|transform-origin|perspective-origin)$/.test(p) && SIDE.test(value)) return "left/right position";
  if (/\bto\s+((top|bottom)\s+)?(left|right)\b/.test(value)) return "gradient toward a side";
  // An angle runs toward a side unless it's straight up or down (0deg, 180deg, a whole turn…).
  const DEG: Record<string, number> = { deg: 1, grad: 0.9, rad: 180 / Math.PI, turn: 360 };
  for (const m of value.matchAll(/linear-gradient\(\s*(-?[\d.]+)(deg|grad|rad|turn)\s*,/g)) {
    const deg = (((Number(m[1]) * DEG[m[2]]) % 180) + 180) % 180;
    if (deg > 1e-6 && deg < 180 - 1e-6) return "gradient angle toward a side";
  }
  if (horizontal(p, value)) return "horizontal transform";
  if (p === "box-shadow") {
    // An edge drawn as an inset shadow (a selected row's accent) sits on a side; a drop shadow is light.
    for (const shadow of words(value, /,/)) {
      const w = words(shadow);
      const x = w.find((v) => /^-?(\d|\.\d)/.test(v));
      if (w.includes("inset") && x && parseFloat(x) !== 0) return "inset shadow on a side";
    }
  }
  if (BOX.test(p)) {
    const w = words(value);
    if (w.length === 4 && w[1] !== w[3]) return "different left and right";
  }
  if (p === "border-radius") {
    for (const part of words(value, /\//)) {
      const w = words(part);
      const [tl, tr = tl, br = tl, bl = tr] = w;
      if (tl !== tr || br !== bl) return "different left and right corners";
    }
  }
  return null;
}

const lineOf = (text: string, at: number) => text.slice(0, at).split("\n").length - 1;

/** A comment holding a `physical:` note: where it is, and whether it has a line of its own. */
interface Note {
  from: number;
  to: number;
  alone: boolean;
}

/** The `physical:` notes among the `comments` in `text`; one is `alone` when only `lead` comes
 *  before it on its line. */
function notes(text: string, comments: RegExp, lead: RegExp): Note[] {
  return [...text.matchAll(comments)]
    .filter((c) => c[0].includes("physical:"))
    .map((c) => ({ from: c.index, to: c.index + c[0].length, alone: lead.test(text.slice(text.lastIndexOf("\n", c.index - 1) + 1, c.index)) }));
}

/** The physical declarations (`at` is where each starts) that no note excuses. A note excuses one
 *  declaration, never a line: on a line of its own, the first physical one after it, when that
 *  starts on its line or the next; after code, the last physical one before it on its line. */
function unexcused<T extends { at: number }>(text: string, hits: T[], found: Note[]): T[] {
  const excused = new Set<T>();
  for (const n of found) {
    if (n.alone) {
      const next = hits.find((h) => h.at >= n.to);
      if (next && lineOf(text, next.at) - lineOf(text, n.to) <= 1) excused.add(next);
    } else {
      const line = lineOf(text, n.from);
      const before = hits.filter((h) => h.at < n.from && lineOf(text, h.at) === line).pop();
      if (before) excused.add(before);
    }
  }
  return hits.filter((h) => !excused.has(h));
}

/** Each physical declaration in a sheet without a `physical:` note, as "file:line  prop: value". */
export function scan(file: string, text: string): string[] {
  // Comments out, keeping every offset so positions and line numbers hold.
  const code = text.replace(/\/\*[\s\S]*?\*\//g, (c) => c.replace(/[^\n]/g, " "));
  const hits: { at: number; text: string }[] = [];
  for (const m of code.matchAll(/([-\w]+)\s*:\s*([^;{}]*)[;}]/g)) {
    const why = physical(m[1], m[2]);
    if (!why) continue;
    // A flip or shift is what a :dir() rule is for: the rule it sits in, from its selector.
    const open = code.lastIndexOf("{", m.index);
    const selector = code.slice(Math.max(code.lastIndexOf("}", open), code.lastIndexOf("{", open - 1)) + 1, open);
    if (why === "horizontal transform" && /:dir\(/.test(selector)) continue;
    hits.push({ at: m.index, text: `${m[1]}: ${m[2].trim().replace(/\s+/g, " ")}  (${why})` });
  }
  return unexcused(text, hits, notes(text, /\/\*[\s\S]*?\*\//g, /^\s*$/)).map((h) => `${file}:${lineOf(text, h.at) + 1}  ${h.text}`);
}

/** The string and template literals in an expression: a ternary's or a call's, each one. */
const literals = (expr: string): string[] =>
  [...expr.matchAll(/"((?:[^"\\\n]|\\.)*)"|'((?:[^'\\\n]|\\.)*)'|`((?:[^`\\]|\\.)*)`/g)].map((m) => m[1] ?? m[2] ?? m[3]);

/** A React style key (`marginLeft`) as its CSS property (`margin-left`). */
const kebab = (key: string) => key.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`);

/** Each physical key in a component's `style={{…}}` objects without a `physical:` note: by its
 *  name, or by any string in its value (`textAlign: n ? "right" : "left"`). */
export function scanTsx(file: string, text: string): string[] {
  const hits: { at: number; text: string }[] = [];
  for (const m of text.matchAll(/style=\{\{/g)) {
    // The object's body: up to the brace that closes `{{`, nested braces and all.
    const from = m.index + m[0].length;
    let depth = 1;
    let to = from;
    while (to < text.length && depth > 0) {
      if (text[to] === "{") depth++;
      if (text[to] === "}") depth--;
      to++;
    }
    // Its entries: split at the commas outside any nested call, object or array.
    const body = text.slice(from, to - 1);
    let nested = 0;
    let start = 0;
    for (let i = 0; i <= body.length; i++) {
      const ch = body[i] ?? ",";
      if ("{([".includes(ch)) nested++;
      if ("})]".includes(ch)) nested--;
      if (ch !== "," || nested !== 0) continue;
      const entry = body.slice(start, i);
      const at = from + start + entry.search(/\S|$/);
      start = i + 1;
      const key = /^\s*([A-Za-z_$][\w$]*)\s*:\s*([\s\S]*?)\s*$/.exec(entry);
      if (!key) continue;
      const why = [...literals(key[2]), ""].map((v) => physical(kebab(key[1]), v)).find(Boolean);
      if (why) hits.push({ at, text: `${key[1]}: ${key[2]}  (${why})` });
    }
  }
  const found = notes(text, /\/\*[\s\S]*?\*\/|\/\/[^\n]*/g, /^[\s{]*$/);
  return unexcused(text, hits, found).map((h) => `${file}:${lineOf(text, h.at) + 1}  ${h.text}`);
}

const GLYPH_OPEN = '<span className="dir-glyph">';

/** Each glyph pointing along the line (◀ ▶ ◂ ▸ → ←) written in a component that isn't a
 *  `.dir-glyph` span of its own, so it wouldn't point the reading way in فارسی (mu-149). A
 *  translated string writes its arrow the right way per language instead; comments don't count. */
export function bareGlyphs(file: string, text: string): string[] {
  const blank = (c: string) => c.replace(/[^\n]/g, " ");
  const code = text.replace(/\/\*[\s\S]*?\*\//g, blank).replace(/(^|[^:])\/\/[^\n]*/g, (c, pre: string) => pre + blank(c.slice(pre.length)));
  const out: string[] = [];
  for (const m of code.matchAll(/[◀▶◂▸→←]/g)) {
    if (code.slice(m.index - GLYPH_OPEN.length, m.index) === GLYPH_OPEN && code.startsWith("</span>", m.index + 1)) continue;
    out.push(`${file}:${code.slice(0, m.index).split("\n").length}  ${m[0]}`);
  }
  return out;
}

describe("logical CSS", () => {
  it("reads every style sheet and component under src", () => {
    expect(Object.keys(sheets)).toEqual(expect.arrayContaining(["./styles.css", "./fonts.css"]));
    expect(Object.keys(components)).toEqual(expect.arrayContaining(["./App.tsx", "./Views.tsx", "./GraphView.tsx", "./ui/Splitter.tsx"]));
    expect(Object.keys(components).filter((f) => f.includes(".test."))).toEqual([]);
  });

  it("flags what mirrors wrong and passes what doesn't", () => {
    const flagged = [
      ["margin-left", "4px"],
      ["border-right-color", "red"],
      ["border-top-left-radius", "4px"],
      ["right", "0"],
      ["text-align", "left"],
      ["float", "right"],
      ["clear", "left"],
      ["padding", "0 8px 0 4px"],
      ["inset", "0 auto 0 0"],
      ["border-radius", "4px 0 0 4px"],
      ["border-radius", "4px 4px 0"],
      ["background-position", "right 8px center"],
      ["background", "linear-gradient(to right, red, blue)"],
      ["box-shadow", "inset 3px 0 0 red"],
      ["box-shadow", "0 1px 2px black, inset -2px 0 0 var(--accent)"],
      ["background", "repeating-linear-gradient(90deg, red 0 4px, transparent 4px 7px)"],
      ["background-image", "linear-gradient(0.25turn, red, blue)"],
      ["mask-image", "linear-gradient(-45deg, red, blue)"],
      ["background", "linear-gradient(180deg, red, blue), linear-gradient(135deg, red, blue)"],
      ["transform", "translateX(4px)"],
      ["transform", "rotate(90deg) scaleX(-1)"],
      ["transform", "translate(-50%, 0)"],
      ["translate", "4px 0"],
      ["translate", "var(--dx)"],
      ["scale", "-1 1"],
      ["transform", "scale(-1, 1)"],
      ["transform", "scale(-1)"],
      ["transform", "translate3d(4px, 0, 0)"],
      ["transform", "rotate(45deg) translate(calc(1px + 2px), 0)"],
      ["transform", "matrix(-1, 0, 0, 1, 0, 0)"],
      ["transform", "matrix(1, 0, 0, 1, 4, 0)"],
      ["transform", "matrix3d(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 4, 0, 0, 1)"],
    ];
    expect(flagged.filter(([p, v]) => !physical(p, v))).toEqual([]);
    const fine = [
      ["margin-inline-start", "4px"],
      ["inset-inline-end", "0"],
      ["text-align", "start"],
      ["padding", "0 8px"],
      ["padding", "1px 8px 2px"],
      ["margin", "0 calc(1px + 2px) 4px calc(1px + 2px)"],
      ["border-radius", "4px"],
      ["border-radius", "4px 4px 0 0"],
      ["border-radius", "calc(var(--r) * 8 / 3) calc(var(--r) * 8 / 3) 0 0"],
      ["border-radius", "50% / 40%"],
      ["background-position", "center"],
      ["background", "linear-gradient(var(--to-end), red, blue)"],
      ["box-shadow", "inset 0 -2px 0 red"],
      ["box-shadow", "inset 0 0 0 1px var(--line), 0 20px 60px rgb(0 0 0 / 0.3)"],
      ["background", "linear-gradient(180deg, red, blue)"],
      ["background", "repeating-linear-gradient(0deg, red 0 1px, transparent 1px 4px)"],
      ["background", "linear-gradient(-1turn, red, blue), linear-gradient(to bottom, red, blue)"],
      ["background", "linear-gradient(var(--to-end), red, blue)"],
      ["transform", "translateY(2px) rotate(90deg)"],
      ["transform", "translate(0, -50%)"],
      ["translate", "0 4px"],
      ["translate", "0"],
      ["scale", "1.05"],
      ["transform", "scale(2)"],
      ["transform", "scale(1, -1)"],
      ["transform", "translate3d(0, 4px, 0)"],
      ["transform", "matrix(1, 0, 0, 1, 0, 4)"],
      ["transform", "matrix3d(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 4, 0, 1)"],
      ["--edge-left", "4px"],
      ["--to-end", "to right"],
      ["flex-direction", "column"],
    ];
    expect(fine.filter(([p, v]) => physical(p, v))).toEqual([]);
  });

  it("finds a declaration across lines and honours a physical: note", () => {
    const sheet = ".a {\n  margin-left: 4px;\n}\n.b {\n  /* physical: traffic lights */\n  padding-left: 78px;\n}\n.c { left: 0; /* physical: pointer */ }\n\n.d:hover { right:\n  0; }\n";
    expect(scan("x.css", sheet)).toEqual(["x.css:2  margin-left: 4px  (side longhand)", "x.css:10  right: 0  (offset)"]);
  });

  it("lets a note excuse the one declaration it stands by, never its whole line or the next", () => {
    const sheet = [
      // 1: a trailing note on a one-line rule excuses its last physical declaration, not the first.
      ".a { left: 0; background: linear-gradient(135deg, red, blue); } /* physical: a hatch */",
      // 2-3: a trailing note says nothing about the line after it.
      ".b { right: 0; } /* physical: pinned */",
      ".c { padding-left: 8px; }",
      // 4-5: a note on its own line excuses the first physical declaration below, not a second.
      "/* physical: the pointer */",
      ".d { left: 0; margin-right: 4px; }",
      // 6-8: nor one two lines down.
      "/* physical: far */",
      "",
      ".e { right: 0; }",
      // 9: before the declaration on its line, the note excuses none of it.
      ".f { margin-left: 2px; /* physical: x */ }",
    ].join("\n");
    expect(scan("x.css", sheet)).toEqual([
      "x.css:1  left: 0  (offset)",
      "x.css:3  padding-left: 8px  (side longhand)",
      "x.css:5  margin-right: 4px  (side longhand)",
      "x.css:8  right: 0  (offset)",
    ]);
  });

  it("lets a horizontal flip or shift stand only in a :dir() rule", () => {
    const sheet = ".a:dir(rtl) { transform: scaleX(-1); }\n.b { transform: scaleX(-1); }\n@media (min-width: 1px) { .c:dir(rtl) > i { transform: translateX(2px); } }\n";
    expect(scan("x.css", sheet)).toEqual(["x.css:2  transform: scaleX(-1)  (horizontal transform)"]);
  });

  it("flags a physical key in an inline style and honours a physical: note", () => {
    const tsx = [
      "<span style={{ left: pct(x), width: pct(w) }} />",
      "<i style={{ marginRight: 4, textAlign: \"right\", insetInlineStart: 0 }} />",
      "{/* physical: the pointer's own x */}",
      "<b style={{ left: e.clientX }} />",
      "<u style={{ textAlign: \"start\", background: f({ left: 1 }), height: on ? right : 0, width }} />",
      "<s style={{\n  paddingInlineEnd: 2,\n  borderLeftColor: c,\n}} />",
      "{/* physical: the pointer again */}",
      "<b style={{ left: e.clientX, right: 0 }} />",
      "<b style={{ right: 0 }} /> {/* physical: pinned */}",
      "<b style={{ left: 0 }} />",
    ].join("\n");
    expect(scanTsx("x.tsx", tsx)).toEqual([
      "x.tsx:1  left: pct(x)  (offset)",
      "x.tsx:2  marginRight: 4  (side longhand)",
      "x.tsx:2  textAlign: \"right\"  (left/right keyword)",
      "x.tsx:8  borderLeftColor: c  (side longhand)",
      "x.tsx:11  right: 0  (offset)",
      "x.tsx:13  left: 0  (offset)",
    ]);
  });

  it("reads every string in an inline style's value, a ternary's and a template's", () => {
    const tsx = [
      "<td style={{ textAlign: numeric ? \"right\" : \"left\" }} />",
      "<td style={{ float: on ? 'start' : 'right' }} />",
      "<i style={{ backgroundPosition: `right ${x}px center` }} />",
      "<i style={{ transform: `translateX(${x}px)` }} />",
      "<i style={{ textAlign: numeric ? \"end\" : \"start\", transform: `translateY(${y}px)` }} />",
    ].join("\n");
    expect(scanTsx("x.tsx", tsx)).toEqual([
      "x.tsx:1  textAlign: numeric ? \"right\" : \"left\"  (left/right keyword)",
      "x.tsx:2  float: on ? 'start' : 'right'  (left/right keyword)",
      "x.tsx:3  backgroundPosition: `right ${x}px center`  (left/right position)",
      "x.tsx:4  transform: `translateX(${x}px)`  (horizontal transform)",
    ]);
  });

  it("has no physical property without a physical: note", () => {
    expect(Object.entries(sheets).flatMap(([file, text]) => scan(file, text))).toEqual([]);
  });

  it("has no physical key in a component's inline style without a physical: note", () => {
    expect(Object.entries(components).flatMap(([file, text]) => scanTsx(file, text))).toEqual([]);
  });
});

describe("glyphs that point along the line in the markup (mu-149)", () => {
  it("flags a bare one and passes one in its own .dir-glyph or a comment", () => {
    const tsx = [
      "<Pill>◀</Pill>",
      `<b>{a} ${GLYPH_OPEN}→</span> {b}</b>`,
      "// ◀ ▶ step a page",
      "{/* the ▸ fold */}",
      "<Pill>{`${title} ▶`}</Pill>",
      `<a href="https://x.y/z">${GLYPH_OPEN}▶</span></a>`,
      "<Pill>← Back</Pill>",
      `<Pill>${GLYPH_OPEN}←</span> Back</Pill>`,
      "<Pill>Next →</Pill>",
    ].join("\n");
    expect(bareGlyphs("x.tsx", tsx)).toEqual(["x.tsx:1  ◀", "x.tsx:5  ▶", "x.tsx:7  ←", "x.tsx:9  →"]);
  });

  it("has every one in a component mirrored", () => {
    expect(Object.entries(components).flatMap(([file, text]) => bareGlyphs(file, text))).toEqual([]);
  });
});
