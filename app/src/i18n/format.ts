// Dates, times, numbers and lists in the chosen language, through Intl.
// Call these while rendering a component that uses useT, so it renders
// again when the language changes.
import { intlTag } from "./locales";
import { getLocale } from "./runtime";

const cache = new Map<string, Intl.DateTimeFormat | Intl.NumberFormat | Intl.ListFormat | Intl.RelativeTimeFormat>();

function made<T extends Intl.DateTimeFormat | Intl.NumberFormat | Intl.ListFormat | Intl.RelativeTimeFormat>(kind: string, opts: object, make: (tag: string) => T): T {
  const tag = intlTag(getLocale());
  const id = `${kind}|${tag}|${JSON.stringify(opts)}`;
  let f = cache.get(id) as T | undefined;
  if (!f) {
    f = make(tag);
    // Options come from code, so this stays small; the cap is a guard.
    if (cache.size > 200) cache.clear();
    cache.set(id, f);
  }
  return f;
}

export function number(n: number, opts: Intl.NumberFormatOptions = {}): string {
  return made("n", opts, (tag) => new Intl.NumberFormat(tag, opts)).format(n);
}

export function dateTime(at: number | Date, opts: Intl.DateTimeFormatOptions = {}): string {
  return made("d", opts, (tag) => new Intl.DateTimeFormat(tag, opts)).format(at);
}

/** "14:05" or "2:05 PM", as the language writes it. */
export const time = (at: number | Date) => dateTime(at, { hour: "2-digit", minute: "2-digit" });
/** "Oct 12". */
export const shortDate = (at: number | Date) => dateTime(at, { month: "short", day: "numeric" });
/** The date alone, in full numbers. */
export const date = (at: number | Date) => dateTime(at, { year: "numeric", month: "numeric", day: "numeric" });
/** Date and time together. */
export const dateAndTime = (at: number | Date) => dateTime(at, { dateStyle: "medium", timeStyle: "short" });

/** "a, b and c". */
export function list(items: readonly string[], type: Intl.ListFormatType = "conjunction"): string {
  return made("l", { type }, (tag) => new Intl.ListFormat(tag, { type })).format(items);
}

/** "3 minutes ago", "in 2 days". */
export function relative(value: number, unit: Intl.RelativeTimeFormatUnit, numeric: "always" | "auto" = "always"): string {
  return made("r", { numeric }, (tag) => new Intl.RelativeTimeFormat(tag, { numeric })).format(value, unit);
}
