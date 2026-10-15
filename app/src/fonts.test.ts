// Vazirmatn for Persian text (mu-150): one face limited to the Arabic script by unicode-range, the
// text and code stacks with it first (Latin keeps SF and SF Mono), and the bundled subset within
// its budget, licence and rebuild notes beside it, the licence in the built app too.
import { describe, expect, it } from "vitest";
import tauri from "../src-tauri/tauri.conf.json";
import fonts from "./fonts.css?raw";
import styles from "./styles.css?raw";
import main from "./main.tsx?raw";
import readme from "./assets/fonts/README.md?raw";
import ofl from "./assets/fonts/OFL.txt?raw";
import woff2 from "./assets/fonts/Vazirmatn-fa.woff2?inline";

const RANGE = "U+0600-06FF, U+200C-200F, U+FB50-FDFF, U+FE70-FEFF, U+AB, U+BB";
const BUDGET = 90 * 1024;

type Rule = { selector: string; decls: Record<string, string> };

/** Top-level rules (comments dropped), with each declaration's value as written. */
function rules(text: string): Rule[] {
  const out: Rule[] = [];
  for (const m of text.replace(/\/\*[\s\S]*?\*\//g, "").matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const decls: Record<string, string> = {};
    for (const d of m[2].split(";")) {
      const at = d.indexOf(":");
      if (at > 0) decls[d.slice(0, at).trim()] = d.slice(at + 1).trim().replace(/\s+/g, " ");
    }
    out.push({ selector: m[1].trim().replace(/\s+/g, " "), decls });
  }
  return out;
}

/** The family list of a `font` shorthand or a `font-family`: what follows the size (and line height). */
function family(decls: Record<string, string>): string | undefined {
  if (decls["font-family"]) return decls["font-family"];
  const f = decls.font;
  if (!f || f === "inherit") return undefined;
  const m = f.match(/^(?:[\w-]+\s+)*?[\d.]+px(?:\/[\d.]+)?\s+(.+)$/);
  return m?.[1];
}

const split = (stack: string) => stack.split(",").map((s) => s.trim());
const faces = rules(fonts).filter((r) => r.selector === "@font-face");
const face = faces[0]?.decls ?? {};
const overrides = new Map(rules(fonts).filter((r) => r.selector !== "@font-face").map((r) => [r.selector, r.decls]));

describe("the Vazirmatn face", () => {
  it("is one face for the Persian ranges only, all weights, swapped in", () => {
    expect(faces).toHaveLength(1);
    expect(face["font-family"]).toBe('"Vazirmatn"');
    expect(face.src).toBe('url("./assets/fonts/Vazirmatn-fa.woff2") format("woff2")');
    expect(face["font-weight"]).toBe("100 900");
    expect(face["font-style"]).toBe("normal");
    expect(face["font-display"]).toBe("swap");
    expect(face["unicode-range"]).toBe(RANGE);
  });

  it("leaves Latin letters and digits to SF: no ASCII in its range", () => {
    for (const part of split(face["unicode-range"] ?? "")) {
      const [lo, hi = lo] = part.replace("U+", "").split("-").map((h) => parseInt(h, 16));
      expect(lo).toBeLessThanOrEqual(hi);
      expect(lo).toBeGreaterThan(0x7f);
    }
  });

  it("is scaled by a measured size-adjust, close to 100%", () => {
    const m = (face["size-adjust"] ?? "").match(/^([\d.]+)%$/);
    expect(m).not.toBeNull();
    const pct = Number(m![1]);
    expect(pct).toBeGreaterThan(95);
    expect(pct).toBeLessThan(115);
    // The README gives the measurement it comes from.
    expect(readme).toContain(face["size-adjust"]!);
  });

  it("covers the same characters the subset was cut to", () => {
    const cmd = readme.match(/pyftsubset[^\n]*--unicodes=([^\s'"]+)/);
    expect(cmd).not.toBeNull();
    const norm = (s: string) => split(s.replace(/,/g, ", ")).map((p) => p.replace(/U\+0*(?=[0-9A-F])/gi, "U+").toUpperCase()).sort();
    expect(norm(cmd![1])).toEqual(norm(RANGE));
  });
});

describe("the font stacks", () => {
  const root = rules(styles).find((r) => r.selector === ":root" && r.decls.font);
  const text = family(root!.decls)!;

  it("put Vazirmatn first for text, then the stack styles.css has", () => {
    expect(overrides.get(":root")?.["font-family"]).toBe(`"Vazirmatn", ${text}`);
  });

  // Every rule in styles.css that names a monospace stack gets the same stack with Vazirmatn first,
  // under the same selector, so the later rule wins. First, not last: ui-monospace's own fallback
  // draws Arabic (measured in WebKit: Persian as wide as SF Mono's), so a later Vazirmatn is never
  // reached. Latin isn't in its range and stays SF Mono.
  const mono = rules(styles).filter((r) => family(r.decls)?.includes("monospace"));

  it("finds the monospace rules it has to follow", () => {
    expect(mono.map((r) => r.selector)).toContain("code");
    expect(mono.length).toBeGreaterThanOrEqual(5);
  });

  it.each(mono.map((r) => [r.selector, family(r.decls)!]))("puts Vazirmatn before the monospace fonts in %s", (selector, stack) => {
    expect(overrides.get(selector)?.["font-family"]).toBe(`"Vazirmatn", ${stack}`);
  });

  it("changes nothing but font-family outside the face", () => {
    for (const [selector, decls] of overrides) {
      expect(Object.keys(decls), selector).toEqual(["font-family"]);
    }
    const known = new Set([":root", "input, textarea, select", ...mono.map((r) => r.selector)]);
    for (const selector of overrides.keys()) expect(known.has(selector), selector).toBe(true);
  });

  // Whole lines, so a commented-out import doesn't count.
  it("loads after styles.css", () => {
    const lines = main.split("\n");
    const s = lines.indexOf('import "./styles.css";');
    const f = lines.indexOf('import "./fonts.css";');
    expect(s).toBeGreaterThanOrEqual(0);
    expect(f).toBeGreaterThan(s);
  });

  // WebKit gives inputs, textareas and selects the system's control font, not the page's; a rule in
  // styles.css that sets only a size (the pane's Pause box) would leave Persian typed there in it.
  it("lets every input, textarea and select take the page's stack", () => {
    expect(overrides.get("input, textarea, select")).toEqual({ "font-family": "inherit" });
    // Before the monospace overrides, so those (same or higher specificity) still win.
    const at = [...overrides.keys()].indexOf("input, textarea, select");
    expect(at).toBeGreaterThanOrEqual(0);
    expect(at).toBeLessThan([...overrides.keys()].indexOf("code"));
  });
});

describe("the bundled file", () => {
  const b64 = woff2.slice(woff2.indexOf(",") + 1);
  const bytes = atob(b64);

  it("is a WOFF2 within its budget", () => {
    expect(bytes.slice(0, 4)).toBe("wOF2");
    expect(bytes.length).toBeGreaterThan(10 * 1024);
    expect(bytes.length).toBeLessThanOrEqual(BUDGET);
  });

  // R2: the bytes are the cut the README records, so another cut (the FD one, other ranges) fails.
  it("is byte for byte the cut the README records", async () => {
    const want = readme.match(/same each run, sha256\s+`([0-9a-f]{64})`/)?.[1];
    expect(want).toBeDefined();
    const buf = Uint8Array.from(bytes, (c) => c.charCodeAt(0));
    const got = [...new Uint8Array(await crypto.subtle.digest("SHA-256", buf))].map((b) => b.toString(16).padStart(2, "0")).join("");
    expect(got).toBe(want);
    expect(readme).toContain(`${buf.length.toLocaleString("en-US")} bytes`);
  });

  // OFL-1.1 wants the licence with every copy: the font's own name table keeps it (IDs 13, 14).
  it("carries its licence in its name table", async () => {
    const names = await nameRecords(Uint8Array.from(bytes, (c) => c.charCodeAt(0)));
    expect(names.get(13)).toContain("SIL Open Font License, Version 1.1");
    expect(names.get(14)).toContain("scripts.sil.org/OFL");
    expect(readme).toMatch(/pyftsubset[^\n]*--name-IDs=0,1,2,3,4,5,6,13,14/);
  });

  it("ships its licence file in the built app, as a bundle resource", () => {
    const resources = (tauri.bundle as { resources?: Record<string, string> }).resources ?? {};
    expect(resources).toEqual({ "../src/assets/fonts/OFL.txt": "fonts/Vazirmatn-OFL.txt" });
    expect(readme).toContain("Contents/Resources/fonts/Vazirmatn-OFL.txt");
  });

  it("ships with its licence and how it was made", () => {
    expect(ofl).toContain("SIL OPEN FONT LICENSE Version 1.1");
    expect(ofl).toContain("The Vazirmatn Project Authors");
    expect(readme).toContain("33.003");
    expect(readme).toContain("6e553e33489a8f9dfaccc76860a2e3f3c1e66de7");
    expect(readme).toContain("--flavor=woff2");
    expect(readme).not.toMatch(/Vazirmatn-FD/);
  });
});

/** The name table's Windows (platform 3) strings by name ID, read from a WOFF2: its table
 *  directory, then the one Brotli stream holding every table back to back. */
async function nameRecords(font: Uint8Array): Promise<Map<number, string>> {
  const v = new DataView(font.buffer, font.byteOffset, font.byteLength);
  const numTables = v.getUint16(12);
  const compressed = v.getUint32(20);
  let at = 48;
  const base128 = () => {
    let n = 0;
    for (let i = 0; i < 5; i++) {
      const b = font[at++];
      n = n * 128 + (b & 0x7f);
      if (!(b & 0x80)) return n;
    }
    throw new Error("bad UIntBase128");
  };
  const tables: { tag: number; length: number }[] = [];
  for (let t = 0; t < numTables; t++) {
    const flags = font[at++];
    const tag = flags & 0x3f;
    if (tag === 63) at += 4;
    const version = flags >> 6;
    const orig = base128();
    // glyf (10) and loca (11) are transformed unless version 3; every other table only if not 0.
    const transformed = tag === 10 || tag === 11 ? version !== 3 : version !== 0;
    tables.push({ tag, length: transformed ? base128() : orig });
  }
  const stream = new Blob([font.slice(at, at + compressed)]).stream().pipeThrough(new DecompressionStream("brotli" as CompressionFormat));
  const data = new Uint8Array(await new Response(stream).arrayBuffer());
  let offset = 0;
  for (const t of tables) {
    if (t.tag !== 5) {
      offset += t.length;
      continue;
    }
    const n = new DataView(data.buffer, offset, t.length);
    const out = new Map<number, string>();
    const count = n.getUint16(2);
    const strings = n.getUint16(4);
    for (let r = 0; r < count; r++) {
      const rec = 6 + r * 12;
      if (n.getUint16(rec) !== 3) continue;
      const len = n.getUint16(rec + 8);
      const from = strings + n.getUint16(rec + 10);
      let s = "";
      for (let i = 0; i < len; i += 2) s += String.fromCharCode(n.getUint16(from + i));
      out.set(n.getUint16(rec + 6), s);
    }
    return out;
  }
  throw new Error("no name table");
}
