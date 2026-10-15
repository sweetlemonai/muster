import { afterEach, describe, expect, it } from "vitest";
import * as fmt from "./format";
import { calendarIn, digitsIn, formatTag, getNumbering, latinDigits, madeCount, readNumberingMirror, shownDigits, writeNumberingMirror } from "./numbering";
import { getTextVersion, setLocale, setNumbering, subscribe, translate } from "./runtime";

// mu-151: Persian digits and Jalali dates where people read them, through one set of cached
// formatters; Digits and Calendar each follow the language until picked.

afterEach(async () => {
  setNumbering({});
  await setLocale("en");
});

/** 14 October 2026, 13:05 on the wall clock (minutes, as the board counts them). */
const OCT14 = Date.UTC(2026, 9, 14, 13, 5) / 60000;

describe("digits and calendar follow the language until picked", () => {
  it("fa: Persian digits and Jalali; en: Latin and Gregorian", async () => {
    expect([digitsIn("en"), calendarIn("en"), formatTag("en")]).toEqual(["latin", "gregorian", "en-u-ca-gregory-nu-latn"]);
    await setLocale("fa");
    expect([digitsIn("fa"), calendarIn("fa"), formatTag("fa")]).toEqual(["persian", "jalali", "fa-u-ca-persian-nu-arabext"]);
  });

  it("each can be picked on its own, in either language", async () => {
    await setLocale("fa");
    setNumbering({ digits: "latin" });
    expect(formatTag("fa")).toBe("fa-u-ca-persian-nu-latn");
    setNumbering({ calendar: "gregorian" });
    expect(formatTag("fa")).toBe("fa-u-ca-gregory-nu-arabext");
    setNumbering({ digits: "persian", calendar: "jalali" });
    expect(formatTag("en")).toBe("en-u-ca-persian-nu-arabext");
  });

  it("an unknown value follows the language", () => {
    setNumbering({ digits: "arabext", calendar: 3 });
    expect(getNumbering()).toEqual({ digits: null, calendar: null });
  });

  it("a change renders the window again, live; the same values don't", () => {
    let calls = 0;
    const off = subscribe(() => calls++);
    const before = getTextVersion();
    setNumbering({ digits: "persian" });
    expect(calls).toBe(1);
    expect(getTextVersion()).not.toBe(before);
    setNumbering({ digits: "persian" });
    expect(calls).toBe(1);
    off();
  });
});

describe("the formatters, in each language, digits and calendar", () => {
  it("fa with its defaults: «۳ کار», «۲۲ مهر ۱۴۰۵», «۳ ساعت پیش», «۲ ساعت», «۵۰۰ هزار»", async () => {
    await setLocale("fa");
    expect(fmt.num(1234)).toBe("۱٬۲۳۴");
    expect(fmt.compact(500_000)).toBe("۵۰۰\u00a0هزار");
    expect(fmt.wallDate(OCT14)).toBe("۲۲ مهر ۱۴۰۵");
    expect(fmt.wallShortDate(OCT14)).toBe("۲۲ مهر");
    expect(fmt.wallDay(OCT14)).toBe("۲۲");
    // CLDR's order for a month and year in Persian.
    expect(fmt.wallMonth(OCT14)).toBe("۱۴۰۵ مهر");
    expect(fmt.wallClock(OCT14)).toBe("۱۳:۰۵");
    expect(fmt.wallHour(OCT14)).toBe("۱۳");
    expect(fmt.isoDay("2026-10-14")).toBe("۲۲ مهر ۱۴۰۵");
    expect(fmt.ago(3 * 3600 + 59)).toBe("۳ ساعت پیش");
    expect(fmt.duration("2h")).toBe("۲ ساعت");
    expect(fmt.duration("30m")).toBe("۳۰ دقیقه");
    expect(fmt.tokens("400k")).toBe("۴۰۰\u00a0هزار");
    // Filled into text: the same digits.
    expect(translate("fa", "common", "time.min", { count: 12 })).toBe("۱۲ دقیقه");
  });

  it("fa with 123 and Gregorian: Latin digits, Gregorian months in Persian", async () => {
    await setLocale("fa");
    setNumbering({ digits: "latin", calendar: "gregorian" });
    expect(fmt.num(12)).toBe("12");
    expect(fmt.wallDate(OCT14)).toBe("14 اکتبر 2026");
    expect(fmt.duration("2h")).toBe("2 ساعت");
    expect(translate("fa", "common", "time.min", { count: 12 })).toBe("12 دقیقه");
  });

  it("en with its defaults, and with ۱۲۳ and Jalali picked", () => {
    expect(fmt.num(1234)).toBe("1,234");
    expect(fmt.wallDate(OCT14)).toBe("October 14, 2026");
    expect(fmt.wallShortDate(OCT14)).toBe("Oct 14");
    expect(fmt.wallWeekday(OCT14)).toBe("Wed");
    expect(fmt.ago(90)).toBe("1 minute ago");
    expect(fmt.duration("1.5d")).toBe("1.5d");
    expect(fmt.tokens("400k")).toBe("400k");
    expect(fmt.isoDay("not a date")).toBe("not a date");
    setNumbering({ digits: "persian", calendar: "jalali" });
    expect(fmt.num(12)).toBe("۱۲");
    expect(fmt.wallDate(OCT14)).toBe("Mehr ۲۲, ۱۴۰۵ AP");
  });

  it("tool stamps and stored days: Jalali in فارسی, as stored in English, anything else as written", async () => {
    expect(fmt.stamp("2026-10-14 20:22")).toBe("10-14 20:22");
    expect(fmt.stamp("2026-10-14 20:22", true)).toBe("2026-10-14 20:22");
    expect(fmt.isoDay("2026-10-14")).toBe("2026-10-14");
    await setLocale("fa");
    expect(fmt.stamp("2026-10-14 20:22")).toBe("۲۲ مهر ۲۰:۲۲");
    expect(fmt.stamp("2026-10-14 20:22", true)).toBe("۲۲ مهر ۱۴۰۵ ۲۰:۲۲");
    expect(fmt.stamp("2026-10-14")).toBe("2026-10-14");
    expect(fmt.stamp("soon")).toBe("soon");
    // A day that isn't one (a hand-edited file) shows as written, not rolled over to March.
    expect(fmt.isoDay("2026-02-30")).toBe("2026-02-30");
    expect(fmt.isoDay("2026-13-01")).toBe("2026-13-01");
    setNumbering({ digits: "latin", calendar: "gregorian" });
    expect(fmt.stamp("2026-10-14 20:22")).toBe("10-14 20:22");
  });

  it("an estimate that isn't a cleaned one stays as written", () => {
    expect(fmt.duration("soon")).toBe("soon");
    expect(fmt.tokens("lots")).toBe("lots");
  });

  it("sorts names in the language's order", async () => {
    const names = ["پروژه", "بازار", "آب", "zeta", "Alpha"];
    expect([...names].sort(fmt.compare).slice(0, 2)).toEqual(["Alpha", "zeta"]);
    await setLocale("fa");
    expect([...names].filter((n) => /[^a-z]/i.test(n)).sort(fmt.compare)).toEqual(["آب", "بازار", "پروژه"]);
  });
});

describe("formatters are made once per language, digits, calendar and options", () => {
  it("formatting again makes no new Intl object", async () => {
    await setLocale("fa");
    fmt.num(1);
    fmt.wallDate(OCT14);
    const made = madeCount();
    for (let i = 0; i < 100; i++) {
      fmt.num(i);
      fmt.wallDate(OCT14 + i * 1440);
      translate("fa", "common", "time.min", { count: i });
    }
    expect(madeCount()).toBe(made);
    setNumbering({ digits: "latin" });
    fmt.num(1);
    expect(madeCount()).toBe(made + 1);
  });
});

describe("digits in and out", () => {
  it("inputs store Latin whatever digits were typed", () => {
    expect(latinDigits("۱۲۳٤٥٦ 7.5h")).toBe("123456 7.5h");
    expect(latinDigits("mu-12")).toBe("mu-12");
  });
  it("a written-out value shows in the picked digits", async () => {
    expect(shownDigits("en", "2h")).toBe("2h");
    await setLocale("fa");
    expect(shownDigits("fa", "2h")).toBe("۲h");
  });
});

describe("the copy kept for the next start", () => {
  it("round-trips through localStorage, and a broken one follows the language", () => {
    const store = new Map<string, string>();
    const ls = { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => void store.set(k, v) };
    (globalThis as { localStorage?: unknown }).localStorage = ls;
    try {
      setNumbering({ digits: "latin", calendar: "jalali" });
      writeNumberingMirror();
      setNumbering({});
      readNumberingMirror();
      expect(getNumbering()).toEqual({ digits: "latin", calendar: "jalali" });
      store.set("muster.numbering", "{broken");
      setNumbering({});
      readNumberingMirror();
      expect(getNumbering()).toEqual({ digits: null, calendar: null });
    } finally {
      delete (globalThis as { localStorage?: unknown }).localStorage;
    }
  });
});
