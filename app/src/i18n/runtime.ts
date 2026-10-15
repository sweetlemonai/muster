// Muster's translation layer, ported from Drover's (Drover task 0007,
// decision 0021; Drover commit a990fc8; task mu-146 here). The files are in
// i18next's JSON format (nested keys, `{{name}}`, plural suffixes from
// Intl.PluralRules), so i18next could replace this later without
// translating anything again. No React here: ./react.tsx binds it.
//
// The files ship as they are, beside the pages (`/locales/<language>/<ns>.json`,
// put there by vite.config.ts), and load on use: nothing of them is in the
// startup JavaScript. The window loads the namespaces it shows first before
// its first render; a screen that loads later brings its own (useT loads
// them and renders again when they're in).

import { dirOf, intlTag, PSEUDO, READY, type AnyLocale } from "./locales";
import { numberIn, setNumbering as setPicked, type Numbering } from "./numbering";
import type { Ns } from "./types";

/** A file's content: strings, nested. */
export interface Messages {
  [key: string]: string | Messages;
}

export function isMessages(x: unknown): x is Messages {
  return typeof x === "object" && x !== null && !Array.isArray(x) && Object.values(x).every((v) => typeof v === "string" || isMessages(v));
}

/** Reads one file; null when the language doesn't have it. */
type Reader = (locale: string, ns: string) => Promise<unknown>;

export const fetchFile: Reader = async (locale, ns) => {
  const res = await fetch(`${import.meta.env.BASE_URL}locales/${locale}/${ns}.json`);
  // Tauri and Vite answer a file that isn't there with the page itself
  // (200, HTML): anything but JSON is a missing file.
  if (res.status === 404 || !res.headers.get("content-type")?.includes("json")) return null;
  if (!res.ok) throw new Error(`locales/${locale}/${ns}.json: ${res.status}`);
  return res.json();
};
let read: Reader = fetchFile;

/** Tests read the files from disk instead (testSetup.ts). */
export function setReader(r: Reader) {
  read = r;
}

/** Loaded files, by `locale/ns`. */
const bundles = new Map<string, Messages>();
/** Loads in flight (and done), by `locale/ns`, so each happens once and
 *  every render asks for the same promise. */
const loading = new Map<string, Promise<void>>();

let current: AnyLocale = "en";
/** The language last asked for (current once its files are in). */
let wanted: AnyLocale = "en";
const listeners = new Set<() => void>();

export const getLocale = (): AnyLocale => current;

/** Bumped when a file of the current language arrives after a switch's
 *  (a screen's own namespace, loaded on first use). */
let version = 0;
/** What a component re-renders on: the language, and its files so far. */
export const getTextVersion = (): string => `${current}#${version}`;

export function subscribe(f: () => void): () => void {
  listeners.add(f);
  return () => listeners.delete(f);
}

export const isLoaded = (locale: AnyLocale, ns: Ns) => bundles.has(`${locale}/${ns}`);

/** The namespaces loaded so far in the current language. */
export function loadedNamespaces(): Ns[] {
  const prefix = `${current}/`;
  return [...bundles.keys()].filter((k) => k.startsWith(prefix)).map((k) => k.slice(prefix.length) as Ns);
}

/** Load one file (once). A language without the file gets an empty one:
 *  its lookups fall back to English, which then loads too. */
export function load(locale: AnyLocale, ns: Ns): Promise<void> {
  const id = `${locale}/${ns}`;
  let p = loading.get(id);
  if (!p) {
    p = fetchMessages(locale, ns).then((m) => {
      bundles.set(id, m);
      // Parts showing this language's text from elsewhere until now
      // render again with it.
      if (locale === current) {
        version++;
        listeners.forEach((f) => f());
      }
    });
    // A failed load can be tried again (the next render or switch).
    p.catch(() => loading.delete(id));
    loading.set(id, p);
  }
  return p;
}

async function fetchMessages(locale: AnyLocale, ns: Ns): Promise<Messages> {
  if (import.meta.env.DEV && locale === PSEUDO) {
    await load("en", ns);
    return pseudoize(bundles.get(`en/${ns}`) ?? {});
  }
  // A language still being translated may miss keys: English stands in.
  if (locale !== "en" && !(READY as readonly string[]).includes(locale)) await load("en", ns);
  const file = await read(locale, ns);
  if (isMessages(file)) return file;
  if (locale === "en" || file !== null) throw new Error(`locales/${locale}/${ns}.json is missing or not text`);
  return {};
}

export function loadAll(locale: AnyLocale, namespaces: readonly Ns[]): Promise<void> {
  return Promise.all(namespaces.map((ns) => load(locale, ns))).then(() => {});
}

/** Settings' Digits and Calendar (mu-151): every part of the window renders again with them
 *  when they change. True when they did. */
export function setNumbering(n: Partial<Record<keyof Numbering, unknown>>): boolean {
  if (!setPicked(n)) return false;
  version++;
  listeners.forEach((f) => f());
  return true;
}

/** Switch language: everything already on screen loads in the new one
 *  first, then every part of the window re-renders at once (no
 *  half-switched screen, no flash of keys). */
export async function setLocale(locale: AnyLocale): Promise<void> {
  // Switches can overlap (a quick change of mind in Settings): the last
  // one asked for wins, whichever language's files arrive first.
  if (locale === wanted) return;
  wanted = locale;
  try {
    await loadAll(locale, ["common", ...loadedNamespaces()]);
  } catch (e) {
    // Stay where we are; the same switch can be tried again.
    if (wanted === locale) wanted = current;
    throw e;
  }
  if (wanted !== locale || current === locale) return;
  current = locale;
  applyToDocument(locale);
  listeners.forEach((f) => f());
}

/** `lang` and `dir` on <html>: the layout mirrors for Persian, the
 *  browser picks fonts and line breaking for the language, and VoiceOver
 *  its voice. */
export function applyToDocument(locale: AnyLocale) {
  if (typeof document === "undefined") return;
  document.documentElement.lang = intlTag(locale);
  document.documentElement.dir = dirOf(locale);
}

// ── Lookup ──────────────────────────────────────────────

export type Params = Record<string, string | number>;

const pluralRules = new Map<string, Intl.PluralRules>();
export function pluralCategory(locale: AnyLocale, n: number): Intl.LDMLPluralRule {
  const tag = intlTag(locale);
  let r = pluralRules.get(tag);
  if (!r) pluralRules.set(tag, (r = new Intl.PluralRules(tag)));
  return r.select(n);
}

/** A number filled into text, in the digits picked (numbering.ts, mu-151): the formatters'
 *  one cache, so `{{count}}` and `fmt.number` always agree. */
export function formatNumber(locale: AnyLocale, n: number): string {
  return numberIn(locale, n);
}

function dig(m: Messages | undefined, path: string): string | undefined {
  if (!m) return undefined;
  let at: string | Messages | undefined = m;
  for (const part of path.split(".")) {
    if (typeof at !== "object") return undefined;
    at = at[part];
  }
  return typeof at === "string" ? at : undefined;
}

/** The string for `key` in one file, with i18next's plural rule: `_zero`
 *  for a count of 0 when the file has it, then the language's category,
 *  then `_other`, then the bare key. */
function pick(locale: AnyLocale, m: Messages | undefined, key: string, count: number | undefined): string | undefined {
  if (count === undefined) return dig(m, key);
  return (
    (count === 0 ? dig(m, `${key}_zero`) : undefined) ??
    dig(m, `${key}_${pluralCategory(locale, count)}`) ??
    dig(m, `${key}_other`) ??
    dig(m, key)
  );
}

const PLACEHOLDER = /\{\{\s*([\w.]+)\s*\}\}/g;

/** First-strong isolate and pop directional isolate (Unicode 6.3): what
 *  `<bdi>` does, in plain text, so it works in titles and labels too. */
export const FSI = "\u2068";
export const PDI = "\u2069";

/**
 * Whether filled-in values are isolated (Drover's task 0020): in a
 * right-to-left language a path, a branch, a command or a name keeps its own
 * direction (`~/.ssh` doesn't lose its tilde to the other side), and no
 * translation has to remember the marks. Drover leaves out its text for
 * agents here; Muster has none (what agents read stays English, outside
 * this layer), so every namespace isolates.
 */
export function isolates(locale: AnyLocale, _ns: Ns): boolean {
  return dirOf(locale) === "rtl";
}

/** The bidi controls a value never keeps (of the core's
 *  `index::is_bidi_control`): the embeddings and overrides, LRM, RLM and ALM. */
const EMBEDDINGS_AND_MARKS = /[\u061c\u200e\u200f\u202a-\u202e]/g;
/** LRI, RLI and FSI: what PDI closes. */
const INITIATORS = new Set(["\u2066", "\u2067", FSI]);

/**
 * `value` as it's filled in (mu-152): every embedding, override and mark
 * out, and every isolate control that doesn't close what it opens (a lone
 * PDI would close the isolate around the value early, an unclosed LRI would
 * swallow its end). The isolate pairs it opens and closes itself stay, and
 * that's where the window differs from the core's `isolate_in`, which drops
 * all twelve: here text this layer made nests into other text as a plain
 * param all over (a `list` of isolated names, a figure isolated left to right
 * by lib.ts's `ltr`, `sentences`, Work's totals), and dropping its pairs would
 * merge its runs into one. A closed pair can't reach past itself, so a
 * title's own pair can't reorder the text around it. The core nests its own
 * text in one place only, through `text_with`, which keeps it apart.
 */
export function cleanValue(value: string): string {
  const chars = [...value.replace(EMBEDDINGS_AND_MARKS, "")];
  const open: number[] = [];
  const unmatched = new Set<number>();
  chars.forEach((c, i) => {
    if (INITIATORS.has(c)) open.push(i);
    else if (c === PDI && open.pop() === undefined) unmatched.add(i);
  });
  for (const i of open) unmatched.add(i);
  return unmatched.size ? chars.filter((_, i) => !unmatched.has(i)).join("") : chars.join("");
}

/** `params`' strings go in clean (`cleanValue`), isolated or not. */
export function interpolate(locale: AnyLocale, text: string, params?: Params, isolate = false): string {
  if (!params) return text;
  return text.replace(PLACEHOLDER, (whole, name: string) => {
    const v = params[name];
    if (v === undefined) return whole;
    if (typeof v === "number") return formatNumber(locale, v);
    const s = cleanValue(v);
    return isolate && s !== "" ? `${FSI}${s}${PDI}` : s;
  });
}

/**
 * The text for `key` in namespace `ns` ("common:key" reaches the shared
 * namespace from any other). Falls back to English, then to the key itself,
 * so a missing string shows as something to fix, never as nothing.
 */
export function translate(locale: AnyLocale, ns: Ns, key: string, params?: Params): string {
  return interpolate(locale, template(locale, ns, key, params), params, isolates(locale, ns));
}

/** The string before its placeholders are filled (for <Trans>, which
 *  finds its tags first, so a value can never add one). */
export function template(locale: AnyLocale, ns: Ns, key: string, params?: Params): string {
  const colon = key.indexOf(":");
  if (colon > 0) {
    ns = key.slice(0, colon) as Ns;
    key = key.slice(colon + 1);
  }
  const count = typeof params?.count === "number" ? params.count : undefined;
  const text = pick(locale, bundles.get(`${locale}/${ns}`), key, count) ?? pick("en", bundles.get(`en/${ns}`), key, count);
  if (text === undefined) {
    // The file isn't in yet (it's loading): nothing, rather than a key,
    // for the moment until it is.
    if (!bundles.has(`${locale}/${ns}`) && !bundles.has(`en/${ns}`)) return "";
    if (import.meta.env.DEV) console.warn(`i18n: no "${ns}:${key}" in ${locale}`);
    return key;
  }
  return text;
}

// ── Pseudo-locale ───────────────────────────────────────

const ACCENTED: Record<string, string> = {
  a: "á", b: "ƀ", c: "ç", d: "ð", e: "é", f: "ƒ", g: "ĝ", h: "ĥ", i: "î", j: "ĵ", k: "ķ", l: "ļ", m: "ɱ",
  n: "ñ", o: "ö", p: "þ", q: "ǫ", r: "ŕ", s: "š", t: "ţ", u: "û", v: "ṽ", w: "ŵ", x: "ẋ", y: "ý", z: "ž",
  A: "Å", B: "Ɓ", C: "Ç", D: "Ð", E: "É", F: "Ƒ", G: "Ĝ", H: "Ĥ", I: "Î", J: "Ĵ", K: "Ķ", L: "Ļ", M: "Ṁ",
  N: "Ñ", O: "Ö", P: "Þ", Q: "Ǫ", R: "Ŕ", S: "Š", T: "Ţ", U: "Û", V: "Ṽ", W: "Ŵ", X: "Ẋ", Y: "Ý", Z: "Ž",
};

/** "Settings" → "[Šéţţîñĝš ~~~]": accented, about a third longer (German
 *  and Russian run long), bracketed so a cut-off end shows. Placeholders
 *  and tags stay as they are. */
export function pseudoText(s: string): string {
  const parts = s.split(/(\{\{[^}]*\}\}|<\/?\w+>)/);
  const body = parts.map((p, i) => (i % 2 ? p : p.replace(/[a-zA-Z]/g, (c) => ACCENTED[c] ?? c))).join("");
  const letters = s.replace(/\{\{[^}]*\}\}|<\/?\w+>/g, "").length;
  return `[${body} ${"~".repeat(Math.max(1, Math.ceil(letters / 3)))}]`;
}

export function pseudoize(m: Messages): Messages {
  return Object.fromEntries(Object.entries(m).map(([k, v]) => [k, typeof v === "string" ? pseudoText(v) : pseudoize(v)]));
}
