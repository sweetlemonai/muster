import { afterEach, describe, expect, it, vi } from "vitest";
import { dirOf, intlTag, isAnyLocale, isLocale, PSEUDO } from "./locales";
import { fetchFile, getLocale, interpolate, isLoaded, isMessages, pluralCategory, pseudoText, setLocale, subscribe, translate } from "./runtime";
import * as fmt from "./format";

afterEach(async () => {
  await setLocale("en");
  vi.unstubAllGlobals();
});

/** <html> as the window has it: what applyToDocument sets (vitest runs without a DOM). */
function stubDocument() {
  const root = { lang: "en", dir: "ltr" };
  vi.stubGlobal("document", { documentElement: root });
  return root;
}

describe("plural categories", () => {
  // The same table as i18n.rs's plural_categories_follow_cldr: the core's
  // hand-written rules and Intl must agree.
  const table: Record<string, [number, string][]> = {
    en: [[0, "other"], [1, "one"], [2, "other"]],
    ru: [[1, "one"], [2, "few"], [5, "many"], [11, "many"], [21, "one"], [22, "few"], [25, "many"], [100, "many"], [111, "many"], [112, "many"]],
    ar: [[0, "zero"], [1, "one"], [2, "two"], [3, "few"], [10, "few"], [11, "many"], [99, "many"], [100, "other"], [102, "other"], [103, "few"], [111, "many"]],
    fr: [[0, "one"], [1, "one"], [2, "other"], [1_000_000, "many"]],
    "pt-BR": [[0, "one"], [1, "one"], [2, "other"]],
    es: [[0, "other"], [1, "one"], [2, "other"], [2_000_000, "many"]],
    fa: [[0, "one"], [1, "one"], [2, "other"]],
    ja: [[1, "other"], [2, "other"]],
    de: [[1, "one"], [2, "other"]],
  };
  for (const [locale, rows] of Object.entries(table)) {
    it(`matches CLDR in ${locale}`, () => {
      expect(rows.map(([n]) => pluralCategory(locale as "en", n))).toEqual(rows.map(([, c]) => c));
    });
  }
});

describe("lookup", () => {
  it("finds nested keys and fills placeholders", () => {
    expect(translate("en", "shell", "language.failed", { error: "disk full" })).toBe("Couldn't change the language: disk full");
    expect(translate("en", "native", "tray.waiting", { count: 1 })).toBe("1 thing waits for you");
    expect(translate("en", "native", "tray.waiting", { count: 4 })).toBe("4 things wait for you");
  });

  it("reaches the shared namespace with common:", () => {
    expect(translate("en", "shell", "common:sentences", { a: "One.", b: "Two." })).toBe("One. Two.");
  });

  it("shows the key for a string nobody has, and warns while developing", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    expect(translate("en", "shell", "no.such.key")).toBe("no.such.key");
    expect(warn).toHaveBeenCalledOnce();
    warn.mockRestore();
  });

  it("falls back to English for a key a language lacks", async () => {
    // A fresh copy of the layer, so the stand-in Persian file stays in this test.
    vi.resetModules();
    const fresh = await import("./runtime");
    const { readFromDisk } = await import("./testReader");
    fresh.setReader((l, ns) => (l === "fa" ? Promise.resolve({ tray: { show: "نمایش" } }) : readFromDisk(l, ns)));
    await fresh.loadAll("en", ["native"]);
    await fresh.load("fa", "native");
    expect(fresh.translate("fa", "native", "tray.show")).toBe("نمایش");
    expect(fresh.translate("fa", "native", "tray.quit")).toBe("Quit Muster");
  });

  it("fills numbers in the language's digits and leaves unknown placeholders", () => {
    expect(interpolate("en", "{{n}} files, {{ who }}", { n: 1234, who: "Ann" })).toBe("1,234 files, Ann");
    expect(interpolate("fa", "{{n}}", { n: 12 })).toBe("۱۲");
    expect(interpolate("en", "{{missing}} stays", {})).toBe("{{missing}} stays");
    expect(interpolate("en", "no params")).toBe("no params");
  });

  it("gives Persian its own text and plurals, with Persian digits", async () => {
    await setLocale("fa");
    expect(translate("fa", "native", "tray.quit")).toBe("خروج از Muster");
    expect(translate("fa", "native", "tray.waiting", { count: 1 })).toBe("۱ مورد منتظر شماست");
    expect(translate("fa", "native", "tray.waiting", { count: 3 })).toBe("۳ مورد منتظر شماست");
  });
});

describe("switching", () => {
  it("loads what's on screen in the new language first, then tells every listener, and mirrors the page", async () => {
    const html = stubDocument();
    const heard = vi.fn(() => getLocale());
    const stop = subscribe(heard);
    await setLocale("fa");
    expect(heard).toHaveBeenCalledTimes(1);
    expect(getLocale()).toBe("fa");
    expect(isLoaded("fa", "common")).toBe(true);
    expect(isLoaded("fa", "shell")).toBe(true);
    expect(html).toEqual({ lang: "fa", dir: "rtl" });
    await setLocale("fa");
    expect(heard).toHaveBeenCalledTimes(1);
    stop();
    await setLocale("en");
    expect(heard).toHaveBeenCalledTimes(1);
    expect(html).toEqual({ lang: "en", dir: "ltr" });
  });

  it("makes a pseudo-locale from English", async () => {
    await setLocale(PSEUDO);
    expect(translate(PSEUDO, "shell", "language.label")).toBe("[Ļáñĝûáĝé ~~~]");
    expect(translate(PSEUDO, "native", "tray.waiting", { count: 2 })).toMatch(/^\[2 ţĥîñĝš ŵáîţ ƒöŕ ýöû ~+\]$/);
  });
});

describe("reading the files", () => {
  const answer = (status: number, type: string, body: string) => vi.fn(async () => new Response(body, { status, headers: { "content-type": type } }));

  it("takes the page Tauri and Vite send for a file that isn't there as missing", async () => {
    vi.stubGlobal("fetch", answer(200, "text/html", "<!doctype html>"));
    expect(await fetchFile("fa", "board")).toBeNull();
    vi.stubGlobal("fetch", answer(404, "text/plain", "no"));
    expect(await fetchFile("fa", "board")).toBeNull();
  });

  it("reads a file from /locales/<language>/<ns>.json, and fails on a server error", async () => {
    const ok = answer(200, "application/json", '{"a":"b"}');
    vi.stubGlobal("fetch", ok);
    expect(await fetchFile("fa", "board")).toEqual({ a: "b" });
    expect(ok).toHaveBeenCalledWith("/locales/fa/board.json");
    vi.stubGlobal("fetch", answer(500, "application/json", "{}"));
    await expect(fetchFile("fa", "board")).rejects.toThrow("500");
  });

  it("checks what it reads", () => {
    expect(isMessages({ a: "b", c: { d: "e" } })).toBe(true);
    expect([isMessages([]), isMessages({ a: 1 }), isMessages(null), isMessages({ a: { b: [] } })]).toEqual([false, false, false, false]);
  });
});

describe("overlapping switches", () => {
  it("ends on the last language asked for, whichever loads first", async () => {
    const slow = setLocale("fa");
    const fast = setLocale("en");
    await Promise.all([slow, fast]);
    expect(getLocale()).toBe("en");
    const a = setLocale(PSEUDO);
    const b = setLocale("fa");
    await Promise.all([a, b]);
    expect(getLocale()).toBe("fa");
  });
});

describe("pseudo text", () => {
  it("keeps placeholders and tags, and pads by a third", () => {
    expect(pseudoText("Run <code>{{cmd}}</code> now")).toBe("[Ŕûñ <code>{{cmd}}</code> ñöŵ ~~~]");
    expect(pseudoText("")).toBe("[ ~]");
  });
});

describe("locales", () => {
  it("knows English and Persian and the pseudo-locale, and which is right to left", () => {
    expect(isLocale("fa") && isLocale("en") && !isLocale("ar") && !isLocale("fa-IR") && !isLocale(PSEUDO)).toBe(true);
    expect(isAnyLocale(PSEUDO) && !isAnyLocale(3)).toBe(true);
    expect([dirOf("fa"), dirOf("en"), dirOf(PSEUDO)]).toEqual(["rtl", "ltr", "ltr"]);
    expect(intlTag(PSEUDO)).toBe("en");
  });
});

describe("formats", () => {
  it("follows the language", async () => {
    const at = Date.UTC(2026, 9, 12, 14, 5);
    const en = fmt.shortDate(at);
    expect(fmt.number(1234.5)).toBe("1,234.5");
    await setLocale("fa");
    expect(fmt.shortDate(at)).not.toBe(en);
    expect(fmt.number(1234.5)).toBe("۱٬۲۳۴٫۵");
    expect(fmt.list(["a", "b", "c"])).toBe("a،\u200f b، و c");
    expect(fmt.relative(-3, "minute")).toBe("۳ دقیقه پیش");
  });
});
