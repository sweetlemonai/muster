// The header's account meter (task 0113) as drawn: a button per account with its two bars, the
// reserve mark, the reset or stale in words, and nothing past a bar's end.
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";
import appSource from "./App.tsx?raw";
import { AccountMeterView } from "./AccountMeter";
import meterSource from "./AccountMeter.tsx?raw";
import { type AccountView, type AccountsInfo, readingText, type WindowView, windowLine } from "./accountsData";
import { unmarked } from "./i18n";
import { setLocale } from "./i18n/runtime";
import css from "./styles.css?raw";

const at = (y: number, mo: number, d: number, h: number, mi: number) => new Date(y, mo - 1, d, h, mi).getTime() / 1000;
const NOW = at(2026, 10, 14, 11, 5);
const win = (over: Partial<WindowView> = {}): WindowView => ({ usedPercentage: 42, resetsAt: at(2026, 10, 14, 14, 20), stale: false, headroom: 38, ...over });
const account = (over: Partial<AccountView> = {}): AccountView => ({
  provider: "claude", label: "mg", name: "cc-mg", plan: "", roles: [], models: [], concurrency: 2, reserve: 20, paused: false, projects: [],
  fiveHour: win(), sevenDay: win({ usedPercentage: 12, resetsAt: at(2026, 10, 18, 9, 0), headroom: 68 }), readingAt: NOW - 60, source: "statusline", ...over,
});
const info = (...accounts: AccountView[]): AccountsInfo => ({ accounts, now: NOW });
const draw = (i: AccountsInfo | null, error = "") => renderToStaticMarkup(<AccountMeterView info={i} error={error} onOpen={() => {}} />);
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/\s+/g, " ").trim();
const decode = (s: string) => s.replace(/&#x27;/g, "'").replace(/&quot;/g, '"').replace(/&amp;/g, "&");
const bars = (html: string) => [...html.matchAll(/<span class="(meter-bar[^"]*)" role="img" aria-label="([^"]*)"/g)].map((m) => ({ cls: m[1], label: decode(m[2]) }));
/** One CSS rule's body. */
const rule = (sel: string) => new RegExp(`(?:^|\\n)${sel.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")} \\{([^}]*)\\}`).exec(css)?.[1] ?? "";

describe("the header's account meter", () => {
  it("is one button per account: its short name, two bars and the 5 h window's reset", () => {
    const html = draw(info(account(), account({ label: "hg", name: "cc-hg", fiveHour: win({ usedPercentage: 7, resetsAt: at(2026, 10, 14, 15, 40) }) })));
    expect(html.match(/<button[^>]*?type="button" class="meter-account"/g)).toHaveLength(2);
    expect(text(html)).toBe("cc-mg resets 14:20 cc-hg resets 15:40");
    expect(bars(html).map((b) => b.label)).toEqual([
      "cc-mg 5 h: 42% used, resets 14:20; reserve 20%",
      "cc-mg 7 d: 12% used, resets 18 Oct 09:00; reserve 20%",
      "cc-hg 5 h: 7% used, resets 15:40; reserve 20%",
      "cc-hg 7 d: 12% used, resets 18 Oct 09:00; reserve 20%",
    ]);
    // The button's own name reads out both bars and what it opens.
    expect(decode(html)).toContain('aria-label="cc-mg 5 h: 42% used, resets 14:20; reserve 20%. cc-mg 7 d: 12% used, resets 18 Oct 09:00; reserve 20%. Open Settings, Accounts"');
    // A labelled group in the header.
    expect(html).toMatch(/^<span class="meter" role="group" aria-label="Accounts&#x27; use">/);
  });

  it("fills each bar to its use and marks the reserve at 100 minus the reserve, on logical sides", () => {
    const html = draw(info(account({ reserve: 25 })));
    expect(html).toContain('<span class="meter-fill" style="inline-size:42%"></span><span class="meter-reserve" style="inset-inline-start:75%"></span>');
    expect(html).toContain('<span class="meter-fill" style="inline-size:12%"></span><span class="meter-reserve" style="inset-inline-start:75%"></span>');
    // No physical left or right anywhere in the meter's rules, so it mirrors in right-to-left.
    for (const sel of [".meter-fill", ".meter-reserve", ".meter-account"]) expect(rule(sel)).not.toMatch(/\b(left|right)\b/);
    expect(rule(".meter-fill")).toContain("inset-inline-start: 0");
  });

  it("clamps an over-cap reading to a full bar, and says over in its name", () => {
    const html = draw(info(account({ fiveHour: win({ usedPercentage: 104, headroom: -20 }) })));
    expect(html).toContain('style="inline-size:100%"');
    expect(html).not.toContain("inline-size:104%");
    expect(bars(html)[0]).toEqual({ cls: "meter-bar warn", label: "cc-mg 5 h: 100% used, over the limit, resets 14:20; reserve 20%, into the reserve" });
  });

  it("takes the warning colour once the use passes 100 minus the reserve", () => {
    expect(bars(draw(info(account({ fiveHour: win({ usedPercentage: 80 }) }))))[0].cls).toBe("meter-bar");
    expect(bars(draw(info(account({ fiveHour: win({ usedPercentage: 81 }) }))))[0].cls).toBe("meter-bar warn");
    expect(rule(".meter-fill")).toContain("background: var(--accent)");
    expect(rule(".meter-bar.warn .meter-fill")).toContain("background: var(--warn)");
    expect(rule(".meter-bar")).toContain("background: var(--line)");
  });

  it("hatches a stale bar and says stale in words, in the row and in the bar's name", () => {
    const html = draw(info(account({ fiveHour: win({ stale: true, resetsAt: at(2026, 10, 14, 10, 0) }) })));
    expect(bars(html)[0]).toEqual({ cls: "meter-bar stale", label: "cc-mg 5 h: 42% used, was due to reset 10:00; reserve 20%, stale" });
    expect(text(html)).toBe("cc-mg stale");
    expect(html).toContain('<span class="meter-when stale">stale</span>');
    expect(rule(".meter-bar.stale")).toMatch(/repeating-linear-gradient/);
    // The reserve's mark stands out over the hatching (QA): 2 px in the text colour, taller than
    // the bar, with a ring of the header's own background between it and the hatch lines.
    const mark = rule(".meter-reserve");
    expect(mark).toContain("inline-size: 2px;");
    expect(mark).toContain("background: var(--text);");
    expect(mark).toContain("box-shadow: 0 0 0 1px var(--panel);");
    expect(mark).toContain("inset-block: -2px;");
  });

  it("says no reading when an account has none, its bars empty in their place", () => {
    const html = draw(info(account({ fiveHour: null, sevenDay: null, readingAt: null, source: null })));
    expect(text(html)).toBe("cc-mg no reading");
    expect(bars(html).map((b) => b.label)).toEqual(["cc-mg 5 h: no reading; reserve 20%", "cc-mg 7 d: no reading; reserve 20%"]);
    expect(html).toContain('<span class="meter-fill" style="inline-size:0%"></span>');
  });

  it("with no accounts, or when it can't read them, is still a button into Settings › Accounts", () => {
    expect(text(draw(info()))).toBe("no accounts");
    expect(draw(info())).toMatch(/<button[^>]*?type="button" class="meter-account"/);
    const failed = draw(null, "accounts.json: permission denied");
    expect(text(failed)).toBe("Can't read the accounts");
    expect(failed).toContain('title="accounts.json: permission denied"');
    // Loading: the group, empty, nothing to click yet.
    expect(draw(null)).toBe('<span class="meter" role="group" aria-label="Accounts&#x27; use"></span>');
  });

  it("never animates", () => {
    for (const sel of [".meter-fill", ".meter-bar", ".meter-reserve"]) expect(rule(sel)).not.toMatch(/transition|animation/);
  });

  it("sits in the header's status area, before the display mode, and opens Settings › Accounts", () => {
    const header = /<header className="top">([\s\S]*?)<\/header>/.exec(appSource)?.[1] ?? "";
    // The status area closes the toolbar's trailing unit, after Help and Settings (mu-202).
    expect(header).toMatch(/<span className="top-end">[^]*?<\/Pill>\s*<\/nav>\s*<span className="top-status">\s*<AccountMeter onOpen=\{\(\) => openSettings\("accounts"\)\} \/>\s*<ModeChip /);
  });

  it("asks the core again on accounts-changed, and never polls", () => {
    expect(meterSource).toContain('listen("accounts-changed"');
    expect(meterSource).not.toMatch(/setInterval/);
  });
});

describe("the meter and its words in Persian (mu-146's layer)", () => {
  afterEach(async () => {
    await setLocale("en");
  });

  it("says each bar, the reset and the reading in Persian, with Persian digits and the account's name kept", async () => {
    await setLocale("fa");
    const html = draw(info(account(), account({ label: "hg", name: "cc-hg", paused: true, fiveHour: win({ usedPercentage: 104, headroom: -24, stale: true, resetsAt: at(2026, 10, 14, 9, 0) }) })));
    expect(bars(html).map((b) => unmarked(b.label))).toEqual([
      "cc-mg ۵ ساعت: ۴۲٪ مصرف‌شده، بازنشانی ۱۴:۲۰؛ رزرو ۲۰٪",
      "cc-mg ۷ روز: ۱۲٪ مصرف‌شده، بازنشانی ۱۸ اکتبر ۰۹:۰۰؛ رزرو ۲۰٪",
      "cc-hg ۵ ساعت: ۱۰۰٪ مصرف‌شده، بیش از سقف، قرار بود ۰۹:۰۰ بازنشانی شود؛ رزرو ۲۰٪، کهنه، وارد رزرو",
      "cc-hg ۷ روز: ۱۲٪ مصرف‌شده، بازنشانی ۱۸ اکتبر ۰۹:۰۰؛ رزرو ۲۰٪",
    ]);
    expect(unmarked(text(html))).toBe("cc-mg بازنشانی ۱۴:۲۰ cc-hg کهنه · متوقف شده");
    expect(unmarked(decode(html))).toContain("باز کردن تنظیمات، حساب‌ها");
    expect(unmarked(windowLine("۵ ساعت", win(), NOW))).toBe("۵ ساعت: ۴۲٪ مصرف‌شده، بازنشانی ۱۴:۲۰، ۳۸٪ جا تا رزرو");
    expect(unmarked(readingText(account(), NOW))).toBe("۱ دقیقه پیش خوانده شد، از نوار وضعیت.");
    expect(unmarked(readingText(account({ readingAt: NOW - 20, source: "stream" }), NOW))).toBe("لحظاتی پیش خوانده شد، از یک اجرا.");
    expect(unmarked(text(draw(info())))).toBe("بدون حساب");
  });
});
