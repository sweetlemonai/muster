import { describe, expect, it } from "vitest";
import cases from "./jalali-cases.json";
import {
  daysInJalaliMonth, epochDayToGregorian, epochDayToJalali, gregorianToEpochDay, gregorianToJalali, isJalaliLeapYear,
  isValidJalaliDate, jalaliToEpochDay, jalaliToGregorian, parseJalali,
} from "./jalali";

const MS_PER_DAY = 86_400_000;

/** "1404/07/20" or "2025-10-12" as numbers. */
const fields = (s: string) => {
  const [year, month, day] = s.split(/[/-]/).map(Number);
  return { year, month, day };
};

describe("every day 1990–2040 against the platform's Persian calendar", () => {
  // One formatter for the whole sweep: building one per day would make this the slowest test here.
  const persian = new Intl.DateTimeFormat("en-u-ca-persian", { timeZone: "UTC", year: "numeric", month: "numeric", day: "numeric" });

  it("is really the Persian calendar, so the sweep can't pass against Gregorian dates", () => {
    expect(persian.resolvedOptions().calendar).toBe("persian");
  });

  it("agrees both ways on each of the 18,628 days", () => {
    const first = gregorianToEpochDay(1990, 1, 1);
    const last = gregorianToEpochDay(2040, 12, 31);
    let checked = 0;
    for (let day = first; day <= last; day++) {
      const date = new Date(day * MS_PER_DAY);
      const g = { year: date.getUTCFullYear(), month: date.getUTCMonth() + 1, day: date.getUTCDate() };
      const parts = persian.formatToParts(date);
      const part = (type: string) => Number(parts.find((p) => p.type === type)?.value);
      const j = { year: part("year"), month: part("month"), day: part("day") };
      // Checked by hand rather than with expect, which costs too much 18,628 times over.
      const toJ = gregorianToJalali(g.year, g.month, g.day);
      const toG = jalaliToGregorian(j.year, j.month, j.day);
      if (toJ.year !== j.year || toJ.month !== j.month || toJ.day !== j.day) {
        expect(toJ, `${g.year}-${g.month}-${g.day} to Jalali`).toEqual(j);
      }
      if (toG.year !== g.year || toG.month !== g.month || toG.day !== g.day) {
        expect(toG, `${j.year}/${j.month}/${j.day} to Gregorian`).toEqual(g);
      }
      if (jalaliToEpochDay(j.year, j.month, j.day) !== day) expect(jalaliToEpochDay(j.year, j.month, j.day)).toBe(day);
      checked++;
    }
    expect(checked).toBe(18_628);
  });
});

describe("the shared cases", () => {
  it.each(cases.known)("%s is %s, both ways", (jalali, gregorian) => {
    const j = fields(jalali);
    const g = fields(gregorian);
    expect(jalaliToGregorian(j.year, j.month, j.day)).toEqual(g);
    expect(gregorianToJalali(g.year, g.month, g.day)).toEqual(j);
    expect(parseJalali(jalali)).toEqual({ iso: gregorian });
  });

  it.each(cases.impossible)("%s is no day", (jalali) => {
    const j = fields(jalali);
    expect(isValidJalaliDate(j.year, j.month, j.day)).toBe(false);
    expect(() => jalaliToEpochDay(j.year, j.month, j.day)).toThrow(RangeError);
    expect(parseJalali(jalali)).toEqual({ error: "date" });
  });
});

describe("months and leap years", () => {
  it("has 31 days in months 1–6, 30 in 7–11, and 29 or 30 in Esfand", () => {
    expect([1, 6, 7, 11].map((m) => daysInJalaliMonth(1404, m))).toEqual([31, 31, 30, 30]);
    expect(daysInJalaliMonth(1403, 12)).toBe(30);
    expect(daysInJalaliMonth(1404, 12)).toBe(29);
    expect(() => daysInJalaliMonth(1404, 13)).toThrow(RangeError);
  });

  it("knows 1403 is leap and 1404 isn't", () => {
    expect(isJalaliLeapYear(1403)).toBe(true);
    expect(isJalaliLeapYear(1404)).toBe(false);
  });

  it("never throws from isValidJalaliDate", () => {
    expect(isValidJalaliDate(1404, 7, 30)).toBe(true);
    expect(isValidJalaliDate(1404.5, 1, 1)).toBe(false);
    expect(isValidJalaliDate(10_000, 1, 1)).toBe(false);
    expect(isValidJalaliDate(Number.NaN, 1, 1)).toBe(false);
  });

  it("counts epoch days from 1970-01-01", () => {
    expect(gregorianToEpochDay(1970, 1, 1)).toBe(0);
    expect(epochDayToGregorian(-1)).toEqual({ year: 1969, month: 12, day: 31 });
    expect(epochDayToJalali(0)).toEqual({ year: 1348, month: 10, day: 11 });
    expect(() => gregorianToEpochDay(2025, 2, 29)).toThrow(RangeError);
  });
});

describe("parseJalali", () => {
  it.each([
    ["1405/07/22", "2026-10-14"],
    ["1405-07-22", "2026-10-14"],
    ["1405/7/22", "2026-10-14"],
    ["1404/1/1", "2025-03-21"],
    ["  1405/07/22 ", "2026-10-14"],
    ["۱۴۰۵/۰۷/۲۲", "2026-10-14"],
    ["١٤٠٥/٠٧/٢٢", "2026-10-14"],
    ["۱۴۰۵-7-٢٢", "2026-10-14"],
    ["1403/12/30", "2025-03-20"],
    // The first day there is, as Intl has it too: a short Gregorian year is padded to four digits.
    ["0001/01/01", "0622-03-21"],
  ])("reads %j as %s", (typed, iso) => {
    expect(parseJalali(typed)).toEqual({ iso });
  });

  it.each([
    "", "   ", "1405", "1405/07", "1405/07/22/1", "1405.07.22", "1405/07-22", "05/07/22", "14050/07/22", "1405/007/22",
    "1405/07/222", "+1405/07/22", "1405 / 07 / 22", "1405/07/22x", "۱۴۰۵/۰۷/۲۲ق", "１４０５/０７/２２", "1405/٫7/22", "2026-10-14T00:00",
  ])("refuses %j as no date", (typed) => {
    expect(parseJalali(typed)).toEqual({ error: "format" });
  });

  it.each(["1404/12/30", "1405/07/31", "1404/13/01", "1405-02-32"])("refuses %j as a day that doesn't exist", (typed) => {
    expect(parseJalali(typed)).toEqual({ error: "date" });
  });

  it("refuses a day whose Gregorian year needs five digits, which no due date can hold", () => {
    expect(parseJalali("9999/01/01")).toEqual({ error: "date" });
  });

  // Between them these use all ten digits, so each Persian and Arabic-Indic one is read as its own.
  it.each(["1398/04/25", "1367/06/10"])("reads %s the same in Persian and Arabic-Indic digits", (latin) => {
    const inScript = (zero: number) => latin.replace(/[0-9]/g, (d) => String.fromCharCode(zero + d.charCodeAt(0) - 48));
    const iso = parseJalali(latin);
    expect(iso).toHaveProperty("iso");
    expect(parseJalali(inScript(0x06f0))).toEqual(iso);
    expect(parseJalali(inScript(0x0660))).toEqual(iso);
  });
});
