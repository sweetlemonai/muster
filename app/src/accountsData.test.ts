// The header's account meter and Settings › Accounts (task 0113): what a bar says and where its
// marks go, the row's words, when the window asks again, and the profile edits checked before the
// core sees them.
import { describe, expect, it } from "vitest";
import { unmarked } from "./i18n";
import { setLocale } from "./i18n/runtime";
import {
  type AccountView,
  addEdit,
  barLabel,
  headroomText,
  intoReserve,
  nextAsk,
  parseList,
  profileEdit,
  type ProfileForm,
  readingText,
  resetText,
  reserveAt,
  rowLabel,
  rowStatus,
  rowStatusWidest,
  shownPercent,
  type WindowView,
  windowLine,
} from "./accountsData";

/** Unix seconds of a local wall-clock time, so the tests read the same in any time zone. */
const at = (y: number, mo: number, d: number, h: number, mi: number) => new Date(y, mo - 1, d, h, mi).getTime() / 1000;
const NOW = at(2026, 10, 14, 11, 5);
const win = (over: Partial<WindowView> = {}): WindowView => ({ usedPercentage: 42, resetsAt: at(2026, 10, 14, 14, 20), stale: false, headroom: 38, ...over });
const account = (over: Partial<AccountView> = {}): AccountView => ({
  provider: "claude", label: "mg", name: "cc-mg", plan: "Max 5x", roles: [], models: [], concurrency: 2, reserve: 20, paused: false, projects: [],
  fiveHour: win(), sevenDay: win({ usedPercentage: 12, resetsAt: at(2026, 10, 18, 9, 0), headroom: 68 }), readingAt: NOW - 180, source: "statusline", ...over,
});

describe("a bar's numbers", () => {
  it("shows the percent used clamped to 0 to 100, whole, never past the bar's end", () => {
    expect([shownPercent(42), shownPercent(41.6), shownPercent(0), shownPercent(100)]).toEqual([42, 42, 0, 100]);
    // Over the cap (utilization 1.04 is 104.0): full, never wider.
    expect(shownPercent(104)).toBe(100);
    expect(shownPercent(-3)).toBe(0);
    expect(shownPercent(Number.NaN)).toBe(0);
  });

  it("marks the reserve where it starts: at 100 minus the reserve", () => {
    expect([reserveAt(20), reserveAt(0), reserveAt(90)]).toEqual([80, 100, 10]);
    expect(reserveAt(-5)).toBe(100);
    expect(reserveAt(150)).toBe(0);
  });

  it("is into the reserve once the use passes 100 minus the reserve, not at it", () => {
    expect(intoReserve(80, 20)).toBe(false);
    expect(intoReserve(80.1, 20)).toBe(true);
    expect(intoReserve(104, 20)).toBe(true);
    expect(intoReserve(99, 0)).toBe(false);
    expect(intoReserve(100.5, 0)).toBe(true);
  });
});

describe("reset times", () => {
  it("are the local clock today, and the date too on another day", () => {
    expect(resetText(at(2026, 10, 14, 14, 20), NOW)).toBe("14:20");
    expect(resetText(at(2026, 10, 14, 9, 5), NOW)).toBe("09:05");
    expect(resetText(at(2026, 10, 15, 0, 30), NOW)).toBe("15 Oct 00:30");
    expect(resetText(at(2026, 11, 2, 9, 0), NOW)).toBe("2 Nov 09:00");
  });
  it("in فارسی, in Persian digits and the Jalali calendar (mu-151)", async () => {
    await setLocale("fa");
    try {
      expect(resetText(at(2026, 10, 14, 9, 5), NOW)).toBe("۰۹:۰۵");
      expect(resetText(at(2026, 10, 15, 0, 30), NOW)).toBe("۲۳ مهر ۰۰:۳۰");
    } finally {
      await setLocale("en");
    }
  });
});

describe("a bar's accessible name", () => {
  it("says the account, the window, the use, the reset and the reserve", () => {
    expect(barLabel("cc-mg", "5 h", win(), 20, NOW)).toBe("cc-mg 5 h: 42% used, resets 14:20; reserve 20%");
    expect(barLabel("cc-hg", "7 d", win({ usedPercentage: 12, resetsAt: at(2026, 10, 18, 9, 0) }), 25, NOW)).toBe("cc-hg 7 d: 12% used, resets 18 Oct 09:00; reserve 25%");
  });

  it("says stale in words, after the reserve", () => {
    expect(barLabel("cc-mg", "5 h", win({ stale: true, resetsAt: at(2026, 10, 14, 10, 0) }), 20, NOW)).toBe("cc-mg 5 h: 42% used, was due to reset 10:00; reserve 20%, stale");
  });

  it("says when the use is into the reserve, and over the limit at 100% shown", () => {
    expect(barLabel("cc-mg", "5 h", win({ usedPercentage: 85 }), 20, NOW)).toBe("cc-mg 5 h: 85% used, resets 14:20; reserve 20%, into the reserve");
    expect(barLabel("cc-mg", "5 h", win({ usedPercentage: 104 }), 20, NOW)).toBe("cc-mg 5 h: 100% used, over the limit, resets 14:20; reserve 20%, into the reserve");
  });

  it("says no reading for a window it has none of", () => {
    expect(barLabel("cc-mg", "7 d", null, 20, NOW)).toBe("cc-mg 7 d: no reading; reserve 20%");
  });
});

describe("the row's words", () => {
  it("is the 5 h window's next reset", () => {
    expect(rowStatus(account(), NOW)).toEqual({ text: "resets 14:20", stale: false });
  });

  it("is stale, in words, when the 5 h reading is past its reset", () => {
    expect(rowStatus(account({ fiveHour: win({ stale: true }) }), NOW)).toEqual({ text: "stale", stale: true });
  });

  it("names a stale weekly window beside a live 5 h one", () => {
    expect(rowStatus(account({ sevenDay: win({ stale: true }) }), NOW)).toEqual({ text: "resets 14:20 · 7 d stale", stale: true });
  });

  it("is no reading when there's none at all, and says which window is missing", () => {
    expect(rowStatus(account({ fiveHour: null, sevenDay: null, readingAt: null, source: null }), NOW)).toEqual({ text: "no reading", stale: false });
    expect(rowStatus(account({ fiveHour: null }), NOW)).toEqual({ text: "no 5 h reading", stale: false });
  });

  it("says paused after the rest", () => {
    expect(rowStatus(account({ paused: true }), NOW).text).toBe("resets 14:20 · paused");
  });

  it("reads out both bars, then what a click does", () => {
    expect(rowLabel(account(), NOW)).toBe(
      "cc-mg 5 h: 42% used, resets 14:20; reserve 20%. cc-mg 7 d: 12% used, resets 18 Oct 09:00; reserve 20%. Open Settings, Accounts",
    );
    expect(rowLabel(account({ paused: true }), NOW)).toContain("reserve 20%. Paused. Open Settings, Accounts");
  });
});

describe("the row's widest words (mu-216)", () => {
  const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

  it("are a reset on another day in every month, two-digit day, with a stale week; and no 5 h reading with it", () => {
    expect(rowStatusWidest(account())).toEqual([...MONTHS.map((m) => `resets 15 ${m} 00:00 · 7 d stale`), "no 5 h reading · 7 d stale"]);
  });

  it("are the same whatever the readings say, so a reset, a stale window or a new reading never changes them", () => {
    const fresh = rowStatusWidest(account());
    for (const a of [
      account({ fiveHour: win({ stale: true }), sevenDay: win({ stale: true }) }),
      account({ fiveHour: null, sevenDay: null, readingAt: null, source: null }),
      account({ fiveHour: null }),
      account({ fiveHour: win({ resetsAt: at(2026, 10, 15, 0, 30) }) }),
    ])
      expect(rowStatusWidest(a)).toEqual(fresh);
  });

  it("hold every status the row can say, as rowStatus words it", () => {
    const room = rowStatusWidest(account()).join("\n");
    for (const a of [account(), account({ sevenDay: win({ stale: true }) }), account({ fiveHour: null, sevenDay: win({ stale: true }) })]) {
      for (const part of rowStatus(a, NOW).text.split(" · ").filter((p) => !/\d/.test(p))) expect(room).toContain(part);
    }
  });

  it("say paused after each, when the account is paused (the owner's switch, which may change the width)", () => {
    expect(rowStatusWidest(account({ paused: true }))).toEqual(rowStatusWidest(account()).map((w) => `${w} · paused`));
  });

  it("in فارسی, in Persian digits and twelve Jalali months", async () => {
    await setLocale("fa");
    try {
      const fa = rowStatusWidest(account()).map(unmarked);
      expect(fa).toHaveLength(13);
      expect(new Set(fa).size).toBe(13);
      // 15 January 2026 is 25 Dey 1404; 15 October is 23 Mehr.
      expect(fa[0]).toBe("بازنشانی ۲۵ دی ۰۰:۰۰ · خوانش هفتگی کهنه");
      expect(fa[9]).toBe("بازنشانی ۲۳ مهر ۰۰:۰۰ · خوانش هفتگی کهنه");
      expect(fa[12]).toBe("بدون خوانش ۵ ساعته · خوانش هفتگی کهنه");
      for (const w of fa) expect(w).not.toMatch(/[0-9]/);
    } finally {
      await setLocale("en");
    }
  });
});

describe("Settings › Accounts' lines", () => {
  it("says the headroom left above the reserve, or how far into it", () => {
    expect(headroomText(38)).toBe("38% to spare above the reserve");
    expect(headroomText(37.6)).toBe("38% to spare above the reserve");
    expect(headroomText(0)).toBe("0% to spare above the reserve");
    expect(headroomText(-5)).toBe("5% into the reserve");
  });

  it("says each window's use, reset and headroom in a line, stale and over the limit in words", () => {
    expect(windowLine("5 h", win(), NOW)).toBe("5 h: 42% used, resets 14:20, 38% to spare above the reserve");
    expect(windowLine("5 h", win({ usedPercentage: 104, headroom: -24 }), NOW)).toBe("5 h: 100% used, over the limit, resets 14:20, 24% into the reserve");
    expect(windowLine("7 d", win({ stale: true, resetsAt: at(2026, 10, 13, 9, 0) }), NOW)).toBe("7 d: 42% used, was due to reset 13 Oct 09:00, 38% to spare above the reserve, stale");
    expect(windowLine("7 d", null, NOW)).toBe("7 d: no reading");
  });

  it("says when the reading came and from where", () => {
    expect(readingText(account(), NOW)).toBe("Read 3 min ago, from the status line.");
    expect(readingText(account({ readingAt: NOW - 20, source: "stream" }), NOW)).toBe("Read just now, from a run.");
    expect(readingText(account({ readingAt: null, source: null, fiveHour: null, sevenDay: null }), NOW)).toBe("No reading yet.");
  });
});

describe("when the window asks again without a change on disk", () => {
  it("is when the next live window resets, so it turns stale on time", () => {
    const a = account();
    expect(nextAsk([a], NOW)).toBe(at(2026, 10, 14, 14, 20) - NOW + 1);
    expect(nextAsk([a, account({ fiveHour: win({ resetsAt: NOW + 60 }) })], NOW)).toBe(61);
  });
  it("is never for stale windows or no readings", () => {
    expect(nextAsk([account({ fiveHour: win({ stale: true }), sevenDay: null })], NOW)).toBeNull();
    expect(nextAsk([], NOW)).toBeNull();
  });
});

describe("a profile edit, checked before the core sees it", () => {
  const form = (over: Partial<ProfileForm> = {}): ProfileForm => ({ plan: "Max 5x", roles: "builder, tester", models: "opus, sonnet", concurrency: "2", reserve: "20", paused: false, ...over });

  it("reads lists split by commas only, as the core does, once each (review R10)", () => {
    expect(parseList("builder, tester , builder,,")).toEqual(["builder", "tester"]);
    expect(parseList("sonnet 4.5, opus")).toEqual(["sonnet 4.5", "opus"]);
    expect(parseList(" , ")).toEqual([]);
  });

  it("makes the profile the core saves", () => {
    expect(profileEdit("mg", form())).toEqual({ profile: { label: "mg", plan: "Max 5x", roles: ["builder", "tester"], models: ["opus", "sonnet"], concurrency: 2, reserve: 20, paused: false } });
    expect(profileEdit("mg", form({ plan: "  Pro  ", reserve: " 0 ", paused: true }))).toEqual({ profile: { label: "mg", plan: "Pro", roles: ["builder", "tester"], models: ["opus", "sonnet"], concurrency: 2, reserve: 0, paused: true } });
  });

  it("refuses what's out of bounds, saying the bounds", () => {
    expect(profileEdit("mg", form({ reserve: "91" }))).toEqual({ error: "Reserve is a whole percent from 0 to 90." });
    expect(profileEdit("mg", form({ reserve: "12.5" }))).toEqual({ error: "Reserve is a whole percent from 0 to 90." });
    expect(profileEdit("mg", form({ reserve: "" }))).toEqual({ error: "Reserve is a whole percent from 0 to 90." });
    expect(profileEdit("mg", form({ concurrency: "0" }))).toEqual({ error: "Concurrency is a whole number from 1 to 8." });
    expect(profileEdit("mg", form({ concurrency: "9" }))).toEqual({ error: "Concurrency is a whole number from 1 to 8." });
    expect(profileEdit("mg", form({ plan: "x".repeat(81) }))).toEqual({ error: "The plan is at most 80 characters." });
    // Counted in characters, not UTF-16 units: 80 Persian letters or emoji pass.
    expect("profile" in profileEdit("mg", form({ plan: "😀".repeat(80) }))).toBe(true);
  });
});

describe("adding an account", () => {
  it("takes the label as typed or as its cc- name", () => {
    expect(addEdit(" mg ")).toEqual({ label: "mg" });
    expect(addEdit("cc-hg")).toEqual({ label: "hg" });
  });
  it("asks for a label when there's none; the core says the rest", () => {
    expect(addEdit("  ")).toEqual({ error: "Type an account's label, like mg for cc-mg." });
    expect(addEdit("cc-")).toEqual({ error: "Type an account's label, like mg for cc-mg." });
  });
});
