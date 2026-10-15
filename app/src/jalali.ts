import { latinDigits } from "./i18n/numbering";

// Jalali (Solar Hijri) dates (mu-151), for the windows that show and take dates in the Persian
// calendar. Storage keeps Gregorian `YYYY-MM-DD`: these only convert at the edges. jalali-cases.json
// holds the answers, and mu-152's Rust port reads the same file.
//
// Ported from @dastafzar/calendar (~/playground/persianUtils/calendar/main/packages/calendar/src/
// conversion.ts, with year-table.ts inlined), MIT licence, Copyright (c) 2026 persianUtils
// contributors. Kept: the conversions; left out: the `Date` helpers and strict mode, which nothing
// here uses.
//
// Which day a year starts on (the source's decision 0001):
// - 1200–1600 AP: an astronomical table, the March equinox judged against true noon at 52.5° E.
// - Outside: the 33-year rule with ICU4X's correction list, the rule ICU, Safari, Chrome and Node
//   `Intl` use. It agrees with the table on every year start 1200–1601, so the seams are smooth.
//
// Every day is an integer "epoch day": days since 1970-01-01 (proleptic Gregorian). Conversions go
// through it and never through `Date`, so they don't depend on the host's time zone.

/** A calendar day: `year`, `month` (1–12) and `day`. */
export type DateFields = { year: number; month: number; day: number };

const MIN_YEAR = 1;
const MAX_YEAR = 9999;

// The astronomical table (the source's year-table.ts, generated from its data/equinox-table.json).
const TABLE_FIRST_YEAR = 1200;
const TABLE_LAST_YEAR = 1600;
/** Days from 1970-01-01 to 1 Farvardin 1200 (1821-03-21). */
const TABLE_FIRST_NOWRUZ = -54342;
/** One bit per year 1200–1600, most significant first: 1 = leap (30 Esfand). */
const TABLE_LEAP_BITS =
  "44222222221111111108888888844444444222222221111111108888888844444444222222211111111108888888844444440";

/**
 * ICU4X `NON_LEAP_CORRECTION` (utils/calendrical_calculations/src/persian.rs, commit 203a0561),
 * from 1601 on: years the plain 33-year rule calls leap that aren't; the next year is leap
 * instead. Earlier entries (1502) fall inside the table.
 */
const CORRECTIONS: ReadonlySet<number> = new Set([
  1601, 1634, 1667, 1700, 1733, 1766, 1799, 1832, 1865, 1898, 1931, 1964, 1997, 2030, 2059, 2063,
  2096, 2129, 2158, 2162, 2191, 2195, 2224, 2228, 2257, 2261, 2290, 2294, 2323, 2327, 2356, 2360,
  2389, 2393, 2422, 2426, 2455, 2459, 2488, 2492, 2521, 2525, 2554, 2558, 2587, 2591, 2620, 2624,
  2653, 2657, 2686, 2690, 2719, 2723, 2748, 2752, 2756, 2781, 2785, 2789, 2818, 2822, 2847, 2851,
  2855, 2880, 2884, 2888, 2913, 2917, 2921, 2946, 2950, 2954, 2979, 2983, 2987,
]);

/**
 * ICU4X's `PERSIAN_EPOCH - 1` as an epoch day: the Persian epoch is Julian 622-03-19, which is
 * 622-03-22 proleptic Gregorian, epoch day −492267.
 */
const RULE_BASE = -492268;

/** 1 Farvardin by the corrected 33-year rule (ICU4X `fixed_from_fast_persian`). */
function ruleNowruz(year: number): number {
  const start = RULE_BASE + 365 * (year - 1) + Math.floor((8 * year + 21) / 33);
  return CORRECTIONS.has(year - 1) ? start - 1 : start;
}

let tableStarts: Int32Array | undefined;

/** Epoch days of 1 Farvardin TABLE_FIRST_YEAR … TABLE_LAST_YEAR + 1, built on first use. */
function tableNowruz(year: number): number {
  if (tableStarts === undefined) {
    const count = TABLE_LAST_YEAR - TABLE_FIRST_YEAR + 2;
    const starts = new Int32Array(count);
    starts[0] = TABLE_FIRST_NOWRUZ;
    for (let i = 1; i < count; i++) {
      const bit = (Number.parseInt(TABLE_LEAP_BITS.charAt((i - 1) >> 2), 16) >> (3 - ((i - 1) & 3))) & 1;
      starts[i] = (starts[i - 1] ?? 0) + 365 + bit;
    }
    tableStarts = starts;
  }
  return tableStarts[year - TABLE_FIRST_YEAR] ?? Number.NaN;
}

/** Epoch day of 1 Farvardin of `year` (no range check). */
function nowruz(year: number): number {
  return year >= TABLE_FIRST_YEAR && year <= TABLE_LAST_YEAR + 1 ? tableNowruz(year) : ruleNowruz(year);
}

function checkYear(year: number): void {
  if (!Number.isInteger(year) || year < MIN_YEAR || year > MAX_YEAR) {
    throw new RangeError(`Jalali year ${year} is outside ${MIN_YEAR}–${MAX_YEAR} AP, the years this converts`);
  }
}

/** True when `year` is a leap year (Esfand has 30 days). */
export function isJalaliLeapYear(year: number): boolean {
  checkYear(year);
  return nowruz(year + 1) - nowruz(year) === 366;
}

/** 31 for months 1–6, 30 for 7–11, 29 or 30 for Esfand (12). */
export function daysInJalaliMonth(year: number, month: number): number {
  checkYear(year);
  if (!Number.isInteger(month) || month < 1 || month > 12) throw new RangeError(`Jalali month ${month} is not 1–12`);
  if (month <= 6) return 31;
  if (month <= 11) return 30;
  return nowruz(year + 1) - nowruz(year) - 336;
}

/** True when the fields name a real Jalali day in 1–9999 AP. Never throws. */
export function isValidJalaliDate(year: number, month: number, day: number): boolean {
  if (!Number.isInteger(year) || year < MIN_YEAR || year > MAX_YEAR) return false;
  if (!Number.isInteger(month) || month < 1 || month > 12) return false;
  return Number.isInteger(day) && day >= 1 && day <= daysInJalaliMonth(year, month);
}

/** Days since 1970-01-01 of a Jalali date. Throws `RangeError` for an impossible date. */
export function jalaliToEpochDay(year: number, month: number, day: number): number {
  const length = daysInJalaliMonth(year, month);
  if (!Number.isInteger(day) || day < 1 || day > length) {
    throw new RangeError(`${year}/${month}/${day} is not a Jalali date: month ${month} of ${year} has ${length} days`);
  }
  return nowruz(year) + (month <= 7 ? 31 * (month - 1) : 186 + 30 * (month - 7)) + day - 1;
}

/** The Jalali date of an epoch day (days since 1970-01-01). */
export function epochDayToJalali(epochDay: number): DateFields {
  if (!Number.isInteger(epochDay)) throw new RangeError(`epoch day ${epochDay} is not an integer`);
  if (epochDay < nowruz(MIN_YEAR) || epochDay >= nowruz(MAX_YEAR + 1)) {
    throw new RangeError(`epoch day ${epochDay} is outside ${MIN_YEAR}–${MAX_YEAR} AP, the years this converts`);
  }
  let year = Math.floor((epochDay - RULE_BASE) / 365.2422) + 1;
  if (year < MIN_YEAR) year = MIN_YEAR;
  if (year > MAX_YEAR) year = MAX_YEAR;
  while (nowruz(year) > epochDay) year--;
  while (nowruz(year + 1) <= epochDay) year++;
  const dayOfYear = epochDay - nowruz(year);
  if (dayOfYear < 186) return { year, month: Math.floor(dayOfYear / 31) + 1, day: (dayOfYear % 31) + 1 };
  const rest = dayOfYear - 186;
  return { year, month: Math.floor(rest / 30) + 7, day: (rest % 30) + 1 };
}

function isGregorianLeapYear(year: number): boolean {
  return (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
}

/** Days since 1970-01-01 of a proleptic Gregorian date. Throws `RangeError` for an impossible date. */
export function gregorianToEpochDay(year: number, month: number, day: number): number {
  if (!Number.isSafeInteger(year)) throw new RangeError(`Gregorian year ${year} is not an integer`);
  if (!Number.isInteger(month) || month < 1 || month > 12) throw new RangeError(`Gregorian month ${month} is not 1–12`);
  const length = month === 2 ? (isGregorianLeapYear(year) ? 29 : 28) : 30 + ((month + (month >> 3)) & 1);
  if (!Number.isInteger(day) || day < 1 || day > length) {
    throw new RangeError(`${year}-${month}-${day} is not a Gregorian date: the month has ${length} days`);
  }
  // Howard Hinnant's days_from_civil.
  const y = month <= 2 ? year - 1 : year;
  const era = Math.floor(y / 400);
  const yearOfEra = y - era * 400;
  const dayOfYear = Math.floor((153 * (month + (month > 2 ? -3 : 9)) + 2) / 5) + day - 1;
  const dayOfEra = yearOfEra * 365 + Math.floor(yearOfEra / 4) - Math.floor(yearOfEra / 100) + dayOfYear;
  return era * 146097 + dayOfEra - 719468;
}

/** The proleptic Gregorian date of an epoch day. */
export function epochDayToGregorian(epochDay: number): DateFields {
  if (!Number.isSafeInteger(epochDay)) throw new RangeError(`epoch day ${epochDay} is not an integer`);
  // Howard Hinnant's civil_from_days.
  const z = epochDay + 719468;
  const era = Math.floor(z / 146097);
  const dayOfEra = z - era * 146097;
  const yearOfEra = Math.floor(
    (dayOfEra - Math.floor(dayOfEra / 1460) + Math.floor(dayOfEra / 36524) - Math.floor(dayOfEra / 146096)) / 365,
  );
  const dayOfYear = dayOfEra - (365 * yearOfEra + Math.floor(yearOfEra / 4) - Math.floor(yearOfEra / 100));
  const mp = Math.floor((5 * dayOfYear + 2) / 153);
  const day = dayOfYear - Math.floor((153 * mp + 2) / 5) + 1;
  const month = mp < 10 ? mp + 3 : mp - 9;
  return { year: yearOfEra + era * 400 + (month <= 2 ? 1 : 0), month, day };
}

/** Gregorian → Jalali. Throws `RangeError` for an impossible date. */
export function gregorianToJalali(year: number, month: number, day: number): DateFields {
  return epochDayToJalali(gregorianToEpochDay(year, month, day));
}

/** Jalali → Gregorian. Throws `RangeError` for an impossible date (30 Esfand of a common year). */
export function jalaliToGregorian(year: number, month: number, day: number): DateFields {
  return epochDayToGregorian(jalaliToEpochDay(year, month, day));
}

// Checked after latinDigits, so \d (0–9 only, in JavaScript) covers the three scripts. One
// separator throughout: 1405/07-22 is a typo, not a date.
const TYPED_JALALI = /^(\d{4})([/-])(\d{1,2})\2(\d{1,2})$/;

const pad = (n: number, width: number) => String(n).padStart(width, "0");

/** A typed Jalali date, `YYYY/MM/DD` or `YYYY-MM-DD` (month and day of one or two digits, in
 *  Persian, Arabic-Indic or Latin digits, spaces around it aside), as the Gregorian `YYYY-MM-DD`
 *  storage keeps. "format" when it isn't written that way, "date" when it's no day (1404/12/30,
 *  1405/07/31, month 13) or its Gregorian year needs five digits, which `YYYY-MM-DD` can't hold. */
export function parseJalali(text: string): { iso: string } | { error: "format" | "date" } {
  const m = TYPED_JALALI.exec(latinDigits(text.trim()));
  if (!m) return { error: "format" };
  const [year, month, day] = [Number(m[1]), Number(m[3]), Number(m[4])];
  if (!isValidJalaliDate(year, month, day)) return { error: "date" };
  const g = jalaliToGregorian(year, month, day);
  if (g.year > 9999) return { error: "date" };
  return { iso: `${pad(g.year, 4)}-${pad(g.month, 2)}-${pad(g.day, 2)}` };
}
