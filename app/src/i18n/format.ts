// Dates, times, numbers and lists in the chosen language, its digits and its calendar (mu-151:
// Settings › Language's Digits and Calendar, numbering.ts), through Intl formatters cached per
// tag. Call these while rendering a component that uses useT, so it renders again when the
// language, the digits or the calendar change. Never for what's copied or stored: keys, ids,
// commands, paths and ISO dates stay as they are.
import { calendarIn, digitsIn, formatter, numberIn, shownDigits } from "./numbering";
import { getLocale, translate } from "./runtime";

export function number(n: number, opts: Intl.NumberFormatOptions = {}): string {
  return numberIn(getLocale(), n, opts);
}

/** A count or any number people read: «۱۲» in Persian digits, "1,234" in Latin. */
export const num = (n: number) => number(n);

/** A big number, short: 500k → «۵۰۰ هزار», "500K". */
export const compact = (n: number) => number(n, { notation: "compact", maximumFractionDigits: 1 });

export function dateTime(at: number | Date, opts: Intl.DateTimeFormatOptions = {}): string {
  return formatter("d", getLocale(), opts, (tag) => new Intl.DateTimeFormat(tag, opts)).format(at);
}

/** "14:05" or "2:05 PM", as the language writes it. */
export const time = (at: number | Date) => dateTime(at, { hour: "2-digit", minute: "2-digit" });
/** "Oct 12", «۲۲ مهر». */
export const shortDate = (at: number | Date) => dateTime(at, { month: "short", day: "numeric" });
/** The date alone, in full numbers. */
export const date = (at: number | Date) => dateTime(at, { year: "numeric", month: "numeric", day: "numeric" });
/** Date and time together. */
export const dateAndTime = (at: number | Date) => dateTime(at, { dateStyle: "medium", timeStyle: "short" });

// The board counts time in wall-clock minutes: the local clock written as if it were UTC (lib.ts
// stampMinutes, viewData.ts wallMinutes). These format such a count as the clock read, in UTC.
const wall = (m: number) => m * 60000;
const utc = (opts: Intl.DateTimeFormatOptions): Intl.DateTimeFormatOptions => ({ ...opts, timeZone: "UTC" });

/** A wall-clock minute's date: «۲۲ مهر ۱۴۰۵», "October 14, 2026". */
export const wallDate = (m: number) => dateTime(wall(m), utc({ year: "numeric", month: "long", day: "numeric" }));
/** A wall-clock minute's day and short month: «۲۲ مهر», "Oct 14". */
export const wallShortDate = (m: number) => dateTime(wall(m), utc({ month: "short", day: "numeric" }));
/** A wall-clock minute's day of the month: «۲۲», "14". */
export const wallDay = (m: number) => dateTime(wall(m), utc({ day: "numeric" }));
/** A wall-clock minute's weekday, short: «چهارشنبه», "Wed". */
export const wallWeekday = (m: number) => dateTime(wall(m), utc({ weekday: "short" }));
/** A wall-clock minute's month and year: «۱۴۰۵ مهر» (CLDR's order), "October 2026". */
export const wallMonth = (m: number) => dateTime(wall(m), utc({ year: "numeric", month: "long" }));
/** A wall-clock minute's month, its full name: «مهر», "October". */
export const wallMonthName = (m: number) => dateTime(wall(m), utc({ month: "long" }));
/** A wall-clock minute's year: «۱۴۰۵», "2026". */
export const wallYear = (m: number) => dateTime(wall(m), utc({ year: "numeric" }));
/** A wall-clock minute's clock, 24-hour: «۱۳:۰۵», "13:05". */
export const wallClock = (m: number) => dateTime(wall(m), utc({ hour: "2-digit", minute: "2-digit", hourCycle: "h23" }));
/** A wall-clock minute's hour, two digits: «۰۹», "09". */
export const wallHour = (m: number) => number(Math.floor((((m % 1440) + 1440) % 1440) / 60), { minimumIntegerDigits: 2 });

/** Latin digits and the Gregorian calendar: a stored date reads as it's stored (`2026-10-14`), as
 *  it always has, and what's copied matches what's shown. Drover's convention (123 picked). */
const asStored = () => digitsIn(getLocale()) === "latin" && calendarIn(getLocale()) === "gregorian";

/** A stored ISO day (`2026-10-14`) as people read it, «۲۲ مهر ۱۴۰۵»; as stored in Latin digits and
 *  Gregorian; anything else as written. */
export function isoDay(iso: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso.trim());
  if (!m || asStored()) return iso;
  const at = new Date(Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3])));
  // Date.UTC rolls 2026-02-30 over to 2 March: a day that isn't one shows as written.
  if (at.getUTCMonth() !== Number(m[2]) - 1 || at.getUTCDate() !== Number(m[3])) return iso;
  return wallDate(at.getTime() / 60000);
}

/** A tool stamp (`2026-10-14 20:22`, the local clock as the files write it) as people read it:
 *  «۲۲ مهر ۲۰:۲۲», with the year when `year`. In Latin digits and Gregorian, as stored (`10-14 20:22`
 *  without the year). Anything else as written. */
export function stamp(at: string, year = false): string {
  const m = /^(\d{4})-(\d{2})-(\d{2}) (\d{2}):(\d{2})$/.exec(at.trim());
  if (!m) return at;
  if (asStored()) return year ? at : at.slice(5);
  const w = Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3]), Number(m[4]), Number(m[5])) / 60000;
  if (Number.isNaN(w)) return at;
  return `${year ? wallDate(w) : wallShortDate(w)} ${wallClock(w)}`;
}

/** "a, b and c". */
export function list(items: readonly string[], type: Intl.ListFormatType = "conjunction"): string {
  return formatter("l", getLocale(), { type }, (tag) => new Intl.ListFormat(tag, { type })).format(items);
}

/** "3 minutes ago", "in 2 days". */
export function relative(value: number, unit: Intl.RelativeTimeFormatUnit, numeric: "always" | "auto" = "always"): string {
  return formatter("r", getLocale(), { numeric }, (tag) => new Intl.RelativeTimeFormat(tag, { numeric })).format(value, unit);
}

/** How long ago, in the largest whole unit: «۳ ساعت پیش», "3 hours ago". `seconds` before now. */
export function ago(seconds: number): string {
  const s = Math.max(0, seconds);
  if (s < 3600) return relative(-Math.floor(s / 60), "minute");
  if (s < 86400) return relative(-Math.floor(s / 3600), "hour");
  return relative(-Math.floor(s / 86400), "day");
}

const unitWord = (n: number, unit: "d" | "h" | "min") => translate(getLocale(), "common", `time.${unit}`, { count: n });

/** Languages that write an estimate in words: Persian has no short unit forms (the glossary's
 *  Times: «۵د» reads as a typo), so `2h` is «۲ ساعت» there. English keeps the form the file has. */
const IN_WORDS: ReadonlySet<string> = new Set(["fa"]);

/** An estimate as people read it (the core's cleaned `2h`, `30m`, `1.5d`): «۲ ساعت» and «۳۰ دقیقه»
 *  in Persian, `2h` in English (in the digits picked); anything else stays as written. The file
 *  keeps `2h`. */
export function duration(text: string): string {
  const m = /^\s*(\d+(?:\.\d+)?)\s*(m|h|d)\s*$/.exec(text);
  if (!m) return text;
  const l = getLocale();
  if (!IN_WORDS.has(l)) return shownDigits(l, text);
  return unitWord(Number(m[1]), m[2] === "m" ? "min" : m[2] === "h" ? "h" : "d");
}

/** A cleaned token estimate (`400k`, `1.2M`, `900`) as people read it: «۴۰۰ هزار» in Persian,
 *  `400k` in English (in the digits picked); anything else stays as written. */
export function tokens(text: string): string {
  const m = /^\s*(\d+(?:\.\d+)?)(k|M)?\s*$/.exec(text);
  if (!m) return text;
  const l = getLocale();
  if (!IN_WORDS.has(l)) return shownDigits(l, text);
  return compact(Number(m[1]) * (m[2] === "M" ? 1e6 : m[2] === "k" ? 1e3 : 1));
}

/** Names in the language's order (Persian order in فارسی). */
export function compare(a: string, b: string): number {
  return formatter("c", getLocale(), {}, (tag) => new Intl.Collator(tag)).compare(a, b);
}
