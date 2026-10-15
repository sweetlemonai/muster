// The digits and calendar the window shows (mu-151): Settings › Language's Digits (۱۲۳ · 123)
// and Calendar (Jalali · Gregorian), each following the language until picked (Persian digits
// and Jalali in فارسی, Latin and Gregorian in English). Every Intl formatter is made here, for
// format.ts and for the numbers runtime.ts fills into text, cached by its tag and options:
// making one is the costly part. Only what people read goes through it; keys, commands, paths,
// stored values and what's copied stay Latin.
import { intlTag, type AnyLocale } from "./locales";

export type Digits = "persian" | "latin";
export type Calendar = "jalali" | "gregorian";
/** What's picked; null follows the language. */
export type Numbering = { digits: Digits | null; calendar: Calendar | null };

export const DIGITS: readonly Digits[] = ["persian", "latin"];
export const CALENDARS: readonly Calendar[] = ["jalali", "gregorian"];
export const isDigits = (x: unknown): x is Digits => x === "persian" || x === "latin";
export const isCalendar = (x: unknown): x is Calendar => x === "jalali" || x === "gregorian";

let picked: Numbering = { digits: null, calendar: null };

/** What the settings say (an unknown or missing value follows the language). True when that
 *  changes anything. */
export function setNumbering(n: { digits?: unknown; calendar?: unknown }): boolean {
  const next: Numbering = { digits: isDigits(n.digits) ? n.digits : null, calendar: isCalendar(n.calendar) ? n.calendar : null };
  if (next.digits === picked.digits && next.calendar === picked.calendar) return false;
  picked = next;
  return true;
}

export const getNumbering = (): Numbering => picked;

/** The language's own: Persian digits and Jalali for فارسی, Latin and Gregorian otherwise. */
const persianByDefault = (l: AnyLocale) => l === "fa";
export const digitsIn = (l: AnyLocale): Digits => picked.digits ?? (persianByDefault(l) ? "persian" : "latin");
export const calendarIn = (l: AnyLocale): Calendar => picked.calendar ?? (persianByDefault(l) ? "jalali" : "gregorian");

/** The BCP 47 tag Intl gets: the language with its calendar and digits, `fa-u-ca-persian-nu-arabext`. */
export function formatTag(l: AnyLocale): string {
  return `${intlTag(l)}-u-ca-${calendarIn(l) === "jalali" ? "persian" : "gregory"}-nu-${digitsIn(l) === "persian" ? "arabext" : "latn"}`;
}

type AnyFormat = Intl.DateTimeFormat | Intl.NumberFormat | Intl.ListFormat | Intl.RelativeTimeFormat | Intl.Collator;
const cache = new Map<string, AnyFormat>();
let made = 0;

/** Formatters made so far (tests count them: one per tag and options). */
export const madeCount = () => made;

/** The cached formatter of `kind` for language `l` with `opts`. */
export function formatter<T extends AnyFormat>(kind: string, l: AnyLocale, opts: object, make: (tag: string) => T): T {
  const tag = formatTag(l);
  const id = `${kind}|${tag}|${JSON.stringify(opts)}`;
  let f = cache.get(id) as T | undefined;
  if (!f) {
    f = make(tag);
    made++;
    // Options come from code, so this stays small; the cap is a guard.
    if (cache.size > 200) cache.clear();
    cache.set(id, f);
  }
  return f;
}

/** A number in language `l`'s digits (what runtime.ts fills into `{{count}}`). */
export function numberIn(l: AnyLocale, n: number, opts: Intl.NumberFormatOptions = {}): string {
  return formatter("n", l, opts, (tag) => new Intl.NumberFormat(tag, opts)).format(n);
}

const PERSIAN_ZERO = 0x6f0;
const ARABIC_ZERO = 0x660;

/** Persian (۰–۹) and Arabic-Indic (٠–٩) digits as 0–9, and the Arabic decimal separator (٫, the
 *  Persian keyboard's) as `.`; everything else as it is: what an input stores, whatever digits
 *  were typed. */
export function latinDigits(s: string): string {
  return s.replace(/[۰-۹٠-٩٫]/g, (d) => {
    if (d === "٫") return ".";
    const c = d.charCodeAt(0);
    return String.fromCharCode(48 + c - (c >= PERSIAN_ZERO ? PERSIAN_ZERO : ARABIC_ZERO));
  });
}

/** `s`'s Latin digits in language `l`'s (for a number already written out, like `2h`). */
export function shownDigits(l: AnyLocale, s: string): string {
  return digitsIn(l) === "persian" ? s.replace(/[0-9]/g, (d) => String.fromCharCode(PERSIAN_ZERO + d.charCodeAt(0) - 48)) : s;
}

const MIRROR = "muster.numbering";

/** The copy kept in localStorage, so the first render has the picked digits and calendar. */
export function readNumberingMirror() {
  try {
    const v: unknown = JSON.parse(localStorage.getItem(MIRROR) ?? "null");
    if (v && typeof v === "object") setNumbering(v);
  } catch {
    // No storage, or a broken copy: the language's own until the core answers.
  }
}

export function writeNumberingMirror() {
  try {
    localStorage.setItem(MIRROR, JSON.stringify(picked));
  } catch {
    // Without storage the next start shows the language's own until the core answers.
  }
}
