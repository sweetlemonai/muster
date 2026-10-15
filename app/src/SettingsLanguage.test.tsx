// Settings › Language (mu-146): the chips, the setting's round trip through the core (mocked at
// the IPC boundary), <html lang dir> following it live, and the saved language before the first
// render.
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getLocale, setLocale } from "./i18n/runtime";
import { DEFAULT_MODE } from "./modes";
import Settings, { LanguageView, pickLanguage } from "./Settings";
import type { SettingsInfo } from "./settingsData";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

const invoked = vi.mocked(invoke);
const listened = vi.mocked(listen);

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

/** The core as the window sees it: settings.json in memory, macOS in Persian unless told. */
function core(system: string[] = ["fa-IR", "en-US"]) {
  let lang: string | undefined;
  const effective = () => (lang ?? (system[0]?.startsWith("fa") ? "fa" : "en"));
  const info = (): SettingsInfo => ({ ...INFO, settings: { ...INFO.settings, ...(lang ? { lang } : {}) }, locale: { language: lang ?? "", system, effective: effective() } });
  invoked.mockImplementation(async (cmd, args) => {
    if (cmd === "settings_info") return info();
    if (cmd === "locale_info") return info().locale;
    if (cmd === "set_setting") {
      const { name, value } = args as { name: string; value: string };
      if (name !== "lang") throw new Error(`unexpected ${name}`);
      if (value !== "" && value !== "en" && value !== "fa") throw new Error(`no language "${value}"`);
      lang = value || undefined;
      return info();
    }
    throw new Error(`unexpected ${cmd}`);
  });
  return { saved: () => lang };
}

/** <html> and localStorage as the window has them (vitest runs without a DOM). */
function page(mirror: string | null = null) {
  const html = { lang: "en", dir: "ltr" };
  const store = new Map<string, string>(mirror ? [["muster.locale", mirror]] : []);
  vi.stubGlobal("document", { documentElement: html });
  vi.stubGlobal("localStorage", { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => void store.set(k, v) });
  return { html, store };
}

const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/\s+/g, " ").trim();

beforeEach(() => {
  invoked.mockReset();
  listened.mockClear();
});

afterEach(async () => {
  await setLocale("en");
  vi.unstubAllGlobals();
});

describe("the Language chips", () => {
  it("name each language in its own script, with its lang, the one shown on, under an aria-label", () => {
    const html = renderToStaticMarkup(<LanguageView shown="en" following={false} onPick={() => {}} />);
    expect(html).toContain('<section class="settings-lang" aria-label="Language">');
    expect(html).toContain('<div role="group" aria-label="Language" class="ui-segmented">');
    expect(html).toContain('aria-pressed="true" type="button" lang="en" dir="ltr" class="ui-segment ui-segment--active">English</');
    expect(html).toContain('aria-pressed="false" type="button" lang="fa" dir="rtl" class="ui-segment">فارسی</');
    expect(text(html)).toBe("Language English فارسی Menus, notifications and help follow it. Agents and task files stay in English.");
  });

  it("keep their names in Persian, where the rest is Persian", async () => {
    await setLocale("fa");
    const html = renderToStaticMarkup(<LanguageView shown="fa" following={false} onPick={() => {}} />);
    expect(html).toContain('aria-label="زبان"');
    expect(html).toContain('lang="en" dir="ltr" class="ui-segment">English</');
    expect(html).toContain('lang="fa" dir="rtl" class="ui-segment ui-segment--active">فارسی</');
    expect(text(html)).toBe("زبان English فارسی منوها، اعلان‌ها و راهنما از آن پیروی می‌کنند. ایجنت‌ها و فایل‌های تسک به انگلیسی می‌مانند.");
  });

  it("say when the language is macOS's (no setting yet), and show a refusal", () => {
    const html = renderToStaticMarkup(<LanguageView shown="fa" following error="disk full" onPick={() => {}} />);
    expect(text(html)).toContain("Following this Mac's language until you pick one.");
    expect(html).toContain('<p class="err" role="alert">Couldn&#x27;t change the language: disk full</p>');
  });

  it("are at the top of Settings, whatever group is open", () => {
    vi.stubGlobal("window", { matchMedia: () => ({ matches: true }) });
    page();
    for (const group of ["appearance", "about"] as const) {
      const html = renderToStaticMarkup(<Settings group={group} onGroup={() => {}} queues={[]} onChanged={() => {}} mode={DEFAULT_MODE} onMode={() => {}} />);
      expect(html.indexOf('class="settings-lang"'), group).toBeGreaterThan(-1);
      expect(html.indexOf('class="settings-lang"')).toBeLessThan(html.indexOf('class="settings-groups"'));
      expect(html.indexOf('class="settings-lang"')).toBeLessThan(html.indexOf('class="settings-body"'));
    }
  });
});

describe("the language setting", () => {
  it("in Settings, lights the language shown and says nothing about following macOS before the settings come", async () => {
    vi.stubGlobal("window", { matchMedia: () => ({ matches: true }) });
    page();
    // What boot leaves when nothing is saved and macOS is in Persian.
    await setLocale("fa");
    const html = renderToStaticMarkup(<Settings group="about" onGroup={() => {}} queues={[]} onChanged={() => {}} mode={DEFAULT_MODE} onMode={() => {}} />);
    expect(html).toContain('lang="fa" dir="rtl" class="ui-segment ui-segment--active">فارسی</');
    expect(html).toContain('aria-pressed="false" type="button" lang="en" dir="ltr" class="ui-segment">English</');
    // Until settings_info answers, whether a language is saved isn't known: no "Following" line.
    expect(text(html)).not.toContain("زبان این مک به کار می‌رود");
    expect(html.slice(html.indexOf('class="settings-lang"'), html.indexOf("</section>")).match(/<p class="meta">/g)).toHaveLength(1);
  });

  it("saves the pick and switches the window at once: <html lang=\"fa\" dir=\"rtl\">, and back", async () => {
    const c = core(["en-US"]);
    const { html, store } = page();
    const info = await pickLanguage("fa");
    expect(invoked).toHaveBeenCalledWith("set_setting", { name: "lang", value: "fa" });
    expect(c.saved()).toBe("fa");
    expect(info.settings.lang).toBe("fa");
    expect(getLocale()).toBe("fa");
    expect(html).toEqual({ lang: "fa", dir: "rtl" });
    // Mirrored, so the next start loads Persian before its first render.
    expect(store.get("muster.locale")).toBe("fa");

    await pickLanguage("en");
    expect(c.saved()).toBe("en");
    expect(getLocale()).toBe("en");
    expect(html).toEqual({ lang: "en", dir: "ltr" });
    expect(store.get("muster.locale")).toBe("en");

    // Back to following macOS: the setting goes, the language is macOS's.
    const back = await pickLanguage("");
    expect(back.settings.lang).toBeUndefined();
    expect(getLocale()).toBe("en");
  });

  it("stays where it is when the core refuses", async () => {
    core();
    const { html } = page();
    invoked.mockRejectedValueOnce("can't write settings.json");
    await expect(pickLanguage("fa")).rejects.toBe("can't write settings.json");
    expect(getLocale()).toBe("en");
    expect(html).toEqual({ lang: "en", dir: "ltr" });
  });
});

describe("at start", () => {
  it("loads the saved language before the first render, then follows the core and its event", async () => {
    vi.resetModules();
    const runtime = await import("./i18n/runtime");
    runtime.setReader((await import("./i18n/testReader")).readFromDisk);
    const { startLocale, LOCALE_CHANGED } = await import("./i18n/boot");
    core(["en-US"]);
    // The core says Persian (the setting, saved last time).
    invoked.mockImplementation(async (cmd) => (cmd === "locale_info" ? { language: "fa", system: ["en-US"], effective: "fa" } : undefined));
    const { html, store } = page("fa");
    await startLocale(["shell"]);
    // Ready to render: Persian text in, <html> mirrored, before any answer from the core.
    expect(runtime.getLocale()).toBe("fa");
    expect(runtime.isLoaded("fa", "shell") && runtime.isLoaded("fa", "common")).toBe(true);
    expect(html).toEqual({ lang: "fa", dir: "rtl" });
    expect(listened).toHaveBeenCalledWith(LOCALE_CHANGED, expect.any(Function));
    // A change the core announces switches the window live.
    const heard = listened.mock.calls[0][1] as (e: { payload: { effective: string } }) => void;
    heard({ payload: { effective: "en" } });
    await vi.waitFor(() => expect(runtime.getLocale()).toBe("en"));
    expect(html).toEqual({ lang: "en", dir: "ltr" });
    expect(store.get("muster.locale")).toBe("en");
  });

  it("starts in English with nothing saved, and takes macOS's language from the core", async () => {
    vi.resetModules();
    const runtime = await import("./i18n/runtime");
    runtime.setReader((await import("./i18n/testReader")).readFromDisk);
    const { startLocale } = await import("./i18n/boot");
    invoked.mockImplementation(async (cmd) => (cmd === "locale_info" ? { language: "", system: ["fa-IR"], effective: "fa" } : undefined));
    const { html, store } = page();
    await startLocale(["shell"]);
    expect(html).toEqual({ lang: "en", dir: "ltr" });
    await vi.waitFor(() => expect(runtime.getLocale()).toBe("fa"));
    expect(html).toEqual({ lang: "fa", dir: "rtl" });
    expect(store.get("muster.locale")).toBe("fa");
  });
});
