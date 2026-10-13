import { holder, key } from "./lib";
import { parseKey } from "./taskKey";
import type { Task } from "./types";

// Search (task 0032; the board's orders are in lib.ts). The core has the same rules (search.rs)
// for `muster list`; a shared fixture (search-cases.json) holds both to the same results.
//
// The rules, kept simple so the two can't drift:
// - Fold: each character is lowercased on its own, combining accents (U+0300–U+036F) are dropped,
//   the letters in FOLD lose their accent, and anything that isn't a letter or a digit becomes a
//   space. "Café-RÉSUMÉ" → "cafe resume". No Unicode normalization, as in the core (Rust's std has
//   none): a letter outside FOLD keeps its accent when it's one character ("ếạ" stays) and loses
//   it when it's written decomposed (e, U+0302, U+0301 → "e"). Digits of other scripts ("١٩",
//   "²", "１９") are kept as letters of a word.
// - A query is its folded words. A task matches when every word does, in any order.
// - A word of digits only (0–9) is an id: it matches the task whose id is all digits and the same
//   number (19, 0019 and 00019 all find 0019). It never matches text, so dates don't swamp ids.
// - A key (0033: a word between spaces that taskKey.ts `parseKey` reads with a prefix, mu-12,
//   MU-0012) is kept whole, as `mu-0012`: it matches the task with that prefix and number, and
//   never text. Only spaces, tabs and line breaks separate a key from the words beside it.
// - Any other word matches when it's part of the folded holder (lib.ts `holder`) or of the task's
//   `search` text (the core's: title, tags and body, each folded word once, digit-only words left
//   out), so "flick" finds "flickers".

/** Accented letters and what they fold to, after lowercasing. Mirrors search.rs FOLD. */
export const FOLD: [string, string][] = [
  ["àáâãäåāăąǎ", "a"], ["çćĉċč", "c"], ["ďđð", "d"], ["èéêëēĕėęě", "e"], ["ĝğġģ", "g"], ["ĥħ", "h"],
  ["ìíîïĩīĭįıǐ", "i"], ["ĵ", "j"], ["ķ", "k"], ["ĺļľŀł", "l"], ["ñńņňŉ", "n"], ["òóôõöøōŏőǒ", "o"],
  ["ŕŗř", "r"], ["śŝşšș", "s"], ["ţťŧț", "t"], ["ùúûüũūŭůűųǔ", "u"], ["ŵ", "w"], ["ýÿŷ", "y"], ["źżž", "z"],
  ["ß", "ss"], ["æ", "ae"], ["œ", "oe"], ["þ", "th"],
];
const FOLD_MAP = new Map(FOLD.flatMap(([from, to]) => [...from].map((c) => [c, to] as const)));
// Rust's char::is_alphanumeric: the Alphabetic property, or a number (Nd, Nl, No).
const ALNUM = /^[\p{Alphabetic}\p{N}]$/u;
const DIGITS = /^[0-9]+$/;

/** One character, folded: "" for a combining accent, " " for anything not a letter or digit. */
function foldChar(ch: string): string {
  // Most text is ASCII: the same answer without the table.
  const code = ch.charCodeAt(0);
  if (ch.length === 1 && code < 128) {
    if (code >= 65 && code <= 90) return String.fromCharCode(code + 32);
    return (code >= 97 && code <= 122) || (code >= 48 && code <= 57) ? ch : " ";
  }
  let out = "";
  for (const c of ch.toLowerCase()) {
    const cp = c.codePointAt(0) ?? 0;
    if (cp >= 0x300 && cp <= 0x36f) continue;
    const m = FOLD_MAP.get(c);
    out += m ?? (ALNUM.test(c) ? c : " ");
  }
  return out;
}

export function fold(s: string): string {
  let out = "";
  for (const ch of s) out += foldChar(ch);
  return out;
}

const split = (folded: string) => folded.split(" ").filter(Boolean);

/** The words of a query, folded, a key kept whole (`mu-0012`). None for an empty query: it finds
 *  everything. */
export function queryWords(q: string): string[] {
  return q.split(/[ \t\n\r\f]/).flatMap((raw) => {
    const k = parseKey(raw);
    return k?.prefix ? [`${k.prefix}-${k.id}`] : split(fold(raw));
  });
}

/** A text as the core stores it in `search`: each folded word once, in order, without words of
 *  digits only. (The core also caps it at 4 KB.) */
export function searchWords(text: string): string {
  return [...new Set(split(fold(text)).filter((w) => !DIGITS.test(w)))].join(" ");
}

const isIdWord = (w: string) => DIGITS.test(w);
const number = (s: string) => s.replace(/^0+/, "");
const sameId = (w: string, id: string) => DIGITS.test(id) && number(w) === number(id);
/** A query's key word (`mu-0012`) as its prefix and number; folded words never have a `-`. */
function keyWord(w: string): [string, string] | null {
  const at = w.indexOf("-");
  return at < 0 ? null : [w.slice(0, at), w.slice(at + 1)];
}
/** Whether a word is this task: digits as its id, a key as its prefix and id; null for a word of text. */
function isTask(w: string, t: Pick<Task, "id" | "prefix">): boolean | null {
  if (isIdWord(w)) return sameId(w, t.id);
  const k = keyWord(w);
  return k ? (t.prefix ?? "") === k[0] && sameId(k[1], t.id) : null;
}

// Holders are few and repeat on every keystroke; their folds are kept (and dropped when many).
const folded = new Map<string, string>();
function foldHolder(t: Task): string {
  const who = holder(t);
  if (!who) return "";
  let f = folded.get(who);
  if (f === undefined) {
    if (folded.size > 500) folded.clear();
    f = fold(who);
    folded.set(who, f);
  }
  return f;
}

/** Whether a task has every word: digits as its id, a key as its prefix and id, the rest in its
 *  holder or its search text. */
export function taskMatches(t: Task, words: string[]): boolean {
  let hay: string | null = null;
  for (const w of words) {
    const it = isTask(w, t);
    if (it !== null) {
      if (!it) return false;
      continue;
    }
    hay ??= `${foldHolder(t)} ${t.search ?? ""}`;
    if (!hay.includes(w)) return false;
  }
  return true;
}

// The search text isn't in the polled state: the window asks the core for it (`search_index`)
// only while the search has words, and again only when a task changed, so the 2 s poll doesn't
// carry up to 4 KB a task that rarely changes.

/** Each task's search text by `queue/id` (lib.ts `key`), as the core's `search_index` gives it. */
export type SearchIndex = Record<string, string>;
/** Each task's `modified` when the index was asked for, by `queue/id`. */
export type IndexStamp = Map<string, number>;

export const indexStamp = (tasks: Task[]): IndexStamp => new Map(tasks.map((t) => [key(t), t.modified]));

/** Whether to ask for the search index: only while the search has words, and then only when none
 *  was asked for yet, or a task in view is new or changed since. A task that's gone changes
 *  nothing (its text just goes unused), and an index for All covers each project in it. */
export function needsIndex(searching: boolean, tasks: Task[], last: IndexStamp | null): boolean {
  if (!searching) return false;
  return last === null || tasks.some((t) => last.get(key(t)) !== t.modified);
}

/** The tasks with their search text from the index; "" for one it doesn't have (yet), which
 *  then matches by its id and holder only. */
export function withSearch(tasks: Task[], index: SearchIndex): Task[] {
  return tasks.map((t) => ({ ...t, search: Object.hasOwn(index, key(t)) ? index[key(t)] : "" }));
}

/** Whether a word of the query names this task, by its id or its key (so its key is marked where
 *  it shows). */
export const idHit = (t: Pick<Task, "id" | "prefix">, words: string[]) => words.some((w) => isTask(w, t) === true);

/** Whether a folded text (a backlog note's `searchWords`) has every word. A note has no id, so a
 *  word of digits, or a key, never matches one. */
export function textMatches(hay: string, words: string[]): boolean {
  return words.every((w) => !isIdWord(w) && !keyWord(w) && hay.includes(w));
}

/** Where the words are in a text as shown: [start, end) in UTF-16 units, merged, in order. The
 *  marks cover the original characters, a decomposed accent included. Digit words are ids and
 *  and keys mark nothing. */
export function highlights(text: string, words: string[]): [number, number][] {
  const look = words.filter((w) => w && !isIdWord(w) && !keyWord(w));
  if (!look.length || !text) return [];
  let f = "";
  const from: number[] = [];
  let i = 0;
  for (const ch of text) {
    const c = foldChar(ch);
    f += c;
    for (let k = 0; k < c.length; k++) from.push(i);
    i += ch.length;
  }
  const isMark = (at: number) => {
    const cp = text.codePointAt(at) ?? 0;
    return cp >= 0x300 && cp <= 0x36f;
  };
  const found: [number, number][] = [];
  for (const w of look) {
    for (let at = f.indexOf(w); at >= 0; at = f.indexOf(w, at + 1)) {
      const last = from[at + w.length - 1];
      let end = last + ((text.codePointAt(last) ?? 0) > 0xffff ? 2 : 1);
      while (end < text.length && isMark(end)) end++;
      found.push([from[at], end]);
    }
  }
  found.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const out: [number, number][] = [];
  for (const r of found) {
    const prev = out[out.length - 1];
    if (prev && r[0] <= prev[1]) prev[1] = Math.max(prev[1], r[1]);
    else out.push([r[0], r[1]]);
  }
  return out;
}
