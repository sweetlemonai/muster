// Settings › Language's Digits and Calendar (mu-151): two chip rows under the language chips,
// each following the language until picked, saved through the core (mocked at the IPC boundary)
// and shown at once.
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import * as fmt from "./i18n/format";
import { getNumbering } from "./i18n/numbering";
import { followNumbering } from "./i18n/boot";
import { setLocale, setNumbering } from "./i18n/runtime";
import { DEFAULT_MODE } from "./modes";
import Settings, { NumberingView, pickNumbering } from "./Settings";
import { numberingSetting, type SettingsInfo } from "./settingsData";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

const invoked = vi.mocked(invoke);

const INFO: SettingsInfo = {
  settings: { leaseMinutes: 720, liveMinutes: 5, claudeAccounts: ["mg", "hg"] },
  leaseBounds: [60, 10080],
  liveBounds: [1, 60],
  version: "0.1.0",
  home: "/Users/someone",
  configDir: "/Users/someone/Library/Application Support/ai.sweetlemon.muster",
  base: "/Users/someone/playground",
  transcripts: [],
  transcriptsUnder: "/Users/someone/playground",
};

/** The core as the window sees it: settings.rs's digits and calendar, in memory. */
function core() {
  const saved: Record<string, string> = {};
  invoked.mockImplementation(async (cmd, args) => {
    if (cmd === "set_setting") {
      const { name, value } = args as { name: string; value: string };
      const ok = name === "digits" ? ["persian", "latin", ""] : name === "calendar" ? ["jalali", "gregorian", ""] : [];
      if (!ok.includes(value)) throw new Error(`${name} is not ${value}`);
      if (value) saved[name] = value;
      else delete saved[name];
      return { ...INFO, settings: { ...INFO.settings, ...saved } };
    }
    if (cmd === "settings_info") return INFO;
    throw new Error(`unexpected ${cmd}`);
  });
  return saved;
}

const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/\s+/g, " ").trim();

beforeEach(() => {
  invoked.mockReset();
  vi.stubGlobal("localStorage", { getItem: () => null, setItem: () => {} });
});

afterEach(async () => {
  setNumbering({});
  await setLocale("en");
  vi.unstubAllGlobals();
});

describe("the Digits and Calendar chips", () => {
  it("two labelled rows, the digits as themselves, the ones in effect on", () => {
    const html = renderToStaticMarkup(<NumberingView digits="latin" calendar="gregorian" onDigits={() => {}} onCalendar={() => {}} />);
    expect(html).toContain('<div role="group" aria-label="Digits" class="ui-segmented">');
    expect(html).toContain('<div role="group" aria-label="Calendar" class="ui-segmented">');
    expect(html).toContain('aria-pressed="false" type="button" lang="fa" dir="ltr" class="ui-segment">۱۲۳</');
    expect(html).toContain('aria-pressed="true" type="button" lang="en" dir="ltr" class="ui-segment ui-segment--active">123</');
    expect(text(html)).toBe("Digits ۱۲۳ 123 Calendar Jalali Gregorian For dates and numbers you read. Keys, commands, paths and what's saved stay as typed.");
  });

  it("in Persian, with the glossary's words", async () => {
    await setLocale("fa");
    const html = renderToStaticMarkup(<NumberingView digits="persian" calendar="jalali" onDigits={() => {}} onCalendar={() => {}} />);
    expect(text(html)).toContain("ارقام ۱۲۳ 123 تقویم خورشیدی میلادی");
    expect(html).toContain('class="ui-segment ui-segment--active">خورشیدی</');
  });

  it("show a refusal", () => {
    const html = renderToStaticMarkup(<NumberingView digits="latin" calendar="gregorian" error="disk full" onDigits={() => {}} onCalendar={() => {}} />);
    expect(html).toContain('<p class="err" role="alert">Couldn&#x27;t change it: disk full</p>');
  });

  it("are in Settings under the language chips, whatever group is open, in effect for the language", async () => {
    vi.stubGlobal("window", { matchMedia: () => ({ matches: true }) });
    vi.stubGlobal("document", { documentElement: { lang: "en", dir: "ltr" } });
    await setLocale("fa");
    const html = renderToStaticMarkup(<Settings group="appearance" onGroup={() => {}} queues={[]} onChanged={() => {}} mode={DEFAULT_MODE} onMode={() => {}} />);
    const at = html.indexOf("settings-numbering");
    expect(at).toBeGreaterThan(html.indexOf('class="settings-lang"'));
    expect(at).toBeLessThan(html.indexOf('class="settings-groups"'));
    expect(html).toContain('lang="fa" dir="ltr" class="ui-segment ui-segment--active">۱۲۳</');
  });
});

describe("the Digits and Calendar settings", () => {
  it("save apart from the language, and show at once", async () => {
    const saved = core();
    await setLocale("fa");
    expect(fmt.num(12)).toBe("۱۲");
    const info = await pickNumbering("digits", "latin", "fa");
    expect(invoked).toHaveBeenCalledWith("set_setting", { name: "digits", value: "latin" });
    expect(info.settings.digits).toBe("latin");
    expect(fmt.num(12)).toBe("12");
    expect(fmt.wallDate(Date.UTC(2026, 9, 14) / 60000)).toBe("22 مهر 1405");
    await pickNumbering("calendar", "gregorian", "fa");
    expect(fmt.wallDate(Date.UTC(2026, 9, 14) / 60000)).toBe("14 اکتبر 2026");
    expect(saved).toEqual({ digits: "latin", calendar: "gregorian" });
    // Picking what the language gives by itself goes back to following it.
    await pickNumbering("digits", "persian", "fa");
    expect(invoked).toHaveBeenLastCalledWith("set_setting", { name: "digits", value: "" });
    expect(getNumbering()).toEqual({ digits: null, calendar: "gregorian" });
    expect(fmt.num(12)).toBe("۱۲");
  });

  it("follow every poll, but not one asked for before a save in Settings", async () => {
    core();
    followNumbering({ digits: "latin" }, performance.now());
    expect(getNumbering().digits).toBe("latin");
    const asked = performance.now();
    await pickNumbering("digits", "persian", "en");
    // A poll that was out while the save went through carries what the file said before it.
    followNumbering({}, asked);
    expect(getNumbering().digits).toBe("persian");
    // The next poll has the saved file's.
    followNumbering({ digits: "persian" }, performance.now());
    expect(getNumbering().digits).toBe("persian");
    // And a later one, after the setting went back to following the language, its.
    followNumbering({}, performance.now());
    expect(getNumbering().digits).toBe(null);
    followNumbering(null, performance.now());
    expect(getNumbering().digits).toBe(null);
  });

  it("stay as they are when the core refuses", async () => {
    core();
    invoked.mockRejectedValueOnce("can't write settings.json");
    await expect(pickNumbering("digits", "persian", "en")).rejects.toBe("can't write settings.json");
    expect(getNumbering()).toEqual({ digits: null, calendar: null });
  });

  it("what a chip saves: empty for the language's own", () => {
    expect(numberingSetting("digits", "persian", "fa")).toBe("");
    expect(numberingSetting("digits", "latin", "fa")).toBe("latin");
    expect(numberingSetting("calendar", "gregorian", "en")).toBe("");
    expect(numberingSetting("calendar", "jalali", "en")).toBe("jalali");
    expect(numberingSetting("digits", "latin", "pseudo")).toBe("");
  });
});
