// The Help pane's chrome and the generated Shortcuts page in Persian (mu-148): the search box, its
// results and empty state, the notes, ◀ ▶, the pages' titles in the sidebar, and every heading,
// sentence, what and where of the Shortcuts page. Keys, routes, commands and links stay as they
// are, the keys isolated left to right. English reads as before (shortcuts.test.ts pins its page).
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";
import { ManualView } from "./Help";
import { tn } from "./i18n";
import { FSI, PDI, setLocale } from "./i18n/runtime";
import { headings, manualPages, PAGES, resolve } from "./manual";
import { AREAS, SHORTCUTS } from "./shortcuts";
import { SECTION_IDS, SLASH, shortcutsPage } from "./shortcutsPage";

const LRI = "⁦";
const COMMANDS = "# Commands\n\nmuster help lists every command.\n\n## muster show\n\nShows one task.\n";
const view = (over: Partial<Parameters<typeof ManualView>[0]> = {}) =>
  renderToStaticMarkup(createElement(ManualView, { pages: manualPages(COMMANDS), at: { file: "02-concepts.md", slug: "", n: 0 }, query: "", onQuery: () => {}, onGo: () => {}, ...over }));
const attr = (html: string, sel: RegExp) => sel.exec(html)?.[1];
/** Text a person sees or hears, isolation marks out. */
const plain = (s: string) => s.replace(/[⁦-⁩]/g, "");

afterEach(async () => {
  await setLocale("en");
});

describe("the Help pane's chrome", () => {
  it("reads as before in English", () => {
    const html = view();
    expect(attr(html, /placeholder="([^"]*)"/)).toBe("Search the manual");
    expect(attr(html, /<nav[^>]*aria-label="([^"]*)"/)).toBe("Manual");
    expect(html).toContain("◀ Start here");
    expect(html).toContain("Tutorials ▶");
    expect(view({ query: "zebra-quokka-nothing" })).toContain("Nothing found for “zebra-quokka-nothing”.");
    expect(view({ pages: manualPages(null), at: { file: "05-commands.md", slug: "", n: 0 } })).toContain("Loading the commands…");
    expect(view({ pages: manualPages(null), at: { file: "05-commands.md", slug: "", n: 0 }, error: "boom" })).toContain("Couldn&#x27;t load the commands: boom");
  });

  it("is Persian in fa: the search box, results, the empty state, loading, errors, ◀ ▶ pointing the reading way", async () => {
    await setLocale("fa");
    const html = view();
    expect(attr(html, /placeholder="([^"]*)"/)).toBe("جستجو در راهنما");
    expect(attr(html, /<input[^>]*aria-label="([^"]*)"/)).toBe("جستجو در راهنما");
    expect(attr(html, /<nav[^>]*aria-label="([^"]*)"/)).toBe("راهنما");
    // Back points right, toward where a right-to-left page starts.
    expect(plain(html)).toContain("▶ از اینجا شروع کنید");
    expect(plain(html)).toContain("آموزش‌ها ◀");
    const hits = view({ query: "lease" });
    expect(attr(hits, /role="region" aria-label="([^"]*)"/)).toBe("نتیجه‌های جستجو");
    // › mirrors itself right to left, so the string keeps it: a ‹ would turn twice and point back.
    expect(plain(hits)).toContain("مفاهیم ›");
    expect(hits).not.toContain("‹");
    expect(plain(view({ query: "zebra" }))).toContain("چیزی برای «zebra» پیدا نشد.");
    // The query is a value: isolated, so a Latin word keeps its own direction.
    expect(view({ query: "zebra" })).toContain(`${FSI}zebra${PDI}`);
    const loading = view({ pages: manualPages(null), at: { file: "05-commands.md", slug: "", n: 0 } });
    expect(loading).toContain("در حال بارگذاری فرمان‌ها…");
    expect(plain(view({ pages: manualPages(null), at: { file: "05-commands.md", slug: "", n: 0 }, error: "boom" }))).toContain("فرمان‌ها بارگذاری نشدند: boom");
  });

  it("lists the pages by their Persian titles in fa, and back in English after a switch", async () => {
    expect(PAGES.map((p) => p.title)).toEqual(["Start here", "Concepts", "Tutorials", "In the app", "Commands", "Shortcuts", "For agents", "FAQ and troubleshooting"]);
    await setLocale("fa");
    expect(PAGES.map((p) => p.title)).toEqual(["از اینجا شروع کنید", "مفاهیم", "آموزش‌ها", "در برنامه", "فرمان‌ها", "میان‌برها", "برای ایجنت‌ها", "پرسش‌های رایج و رفع مشکل"]);
    const html = view();
    for (const p of PAGES) expect(html).toContain(`>${p.title}</button>`);
    await setLocale("en");
    expect(PAGES[0].title).toBe("Start here");
  });
});

/** The Latin words a Persian page still has, without what stays as it is: keys, routes and code,
 *  link targets, product and key names. */
function englishLeft(page: string): string[] {
  const KEPT = new Set(["Claude", "Code", "Command", "Option", "Return", "Ctrl", "Esc", "Tab", "Shift", "Home", "End", "Space", "Muster", "muster", "GitHub"]);
  const text = plain(page)
    .replace(/`[^`]*`/g, " ")
    .replace(/ \{#[a-z-]+\}$/gm, " ")
    .replace(/\]\([^)]*\)/g, "]")
    .replace(/Ctrl\+F/g, " ");
  return [...new Set(text.match(/[A-Za-z][A-Za-z'’]*/g) ?? [])].filter((w) => w.length > 1 && !KEPT.has(w));
}

describe("the Shortcuts page in Persian", () => {
  it("has its title, intro, every area's heading and the table's heads in Persian", async () => {
    const english = AREAS.map((a) => tn("help", `areas.${a}`));
    expect(english).toContain("Anywhere");
    await setLocale("fa");
    const page = shortcutsPage();
    expect(page.split("\n")[0]).toBe("# میان‌برها");
    expect(page).toContain("همه‌ی میان‌برهای صفحه‌کلید در برنامه");
    for (const h of english) expect(page).not.toContain(`\n## ${h}`);
    const fa = ["همه‌جا", "جستجو و نماها", "نوشتن و فرستادن", "پنل تسک", "بورد", "پروژه‌ها", "فرمان‌های اسلش در Claude Code"];
    for (const h of fa) expect(page).toMatch(new RegExp(`\n## ${h} \\{#[a-z-]+\\}\n`));
    expect(headings(page).filter((h) => h.level === 2).map((h) => h.title)).toEqual(fa);
    expect(page).toContain("| کلیدها | چه می‌کند | کجا |");
    expect(page).toContain("| مسیر | برای | چه می‌کند |");
  });

  it("says which shortcuts take ⌘ only, joined the Persian way, every glyph isolated", async () => {
    await setLocale("fa");
    const intro = plain(shortcutsPage().split("\n")[2]);
    expect(intro).toContain("هر جا ⌘ آمده، Ctrl هم کار می‌کند، جز ⌘F، ⌘+، ⌘− و ⌘0 که فقط ⌘ را می‌پذیرند.");
    expect(intro).toContain("⌥ کلید Option است.");
    // No ⌘ ⌥ outside a left-to-right isolate.
    const bare = shortcutsPage().split("\n")[2].replace(new RegExp(`[${LRI}⁨][^⁩]*⁩`, "g"), " ");
    expect(bare).not.toMatch(/[⌘⌥⇧⌃]/);
  });

  it("gives every shortcut its keys, isolated, and what it does and where in Persian", async () => {
    await setLocale("fa");
    const page = shortcutsPage();
    const rows = page.split("\n").filter((l) => l.startsWith(`| ${LRI}`));
    expect(rows).toHaveLength(SHORTCUTS.length);
    for (const s of SHORTCUTS) {
      const row = rows.find((r) => r.includes(s.what));
      expect(row, s.id).toBeDefined();
      expect(row, s.id).toContain(s.where);
      expect(/[؀-ۿ]/.test(s.what) && /[؀-ۿ]/.test(s.where), s.id).toBe(true);
    }
    expect(page).toContain(`| ${LRI}⌘N${PDI} |`);
    // A worded cell is isolated key by key by help.json, never a second time as a whole.
    expect(page).toContain(`| ${LRI}Esc${PDI}، دو بار |`);
    expect(page).toContain(`| ${LRI}Home${PDI}، ${LRI}End${PDI} |`);
  });

  it("keeps routes, commands and the link, and says who each route is for in Persian", async () => {
    await setLocale("fa");
    const page = shortcutsPage();
    for (const r of SLASH) expect(page).toContain(`| \`${r.route}\` |`);
    expect(page).toContain("| `/mu` | مالک | آنچه منتظر شماست، سپس صف در یک خط |");
    expect(page).toContain("| `/mu ask <id> <question>` | ایجنت |");
    expect(page).toContain("| `/mu check` | سیستم |");
    expect(page).toContain("[برای ایجنت‌ها](07-for-agents.md#in-a-claude-code-session)");
  });

  it("has no English word left but keys, routes, commands and names", async () => {
    await setLocale("fa");
    expect(englishLeft(shortcutsPage())).toEqual([]);
  });

  it("keeps each section's English anchor, so a link from an English page still lands", async () => {
    const slugs = (md: string) => headings(md).filter((h) => h.level === 2).map((h) => h.slug);
    const ids = Object.values(SECTION_IDS);
    expect(slugs(shortcutsPage())).toEqual(ids);
    await setLocale("fa");
    expect(slugs(shortcutsPage())).toEqual(ids);
    expect(resolve("07-for-agents.md", "06-shortcuts.md#slash-commands-in-claude-code")).toMatchObject({ kind: "page", slug: SECTION_IDS.slash });
  });

  it("shows a heading's own anchor nowhere in the Help pane", async () => {
    await setLocale("fa");
    const html = view({ at: { file: "06-shortcuts.md", slug: "", n: 0 } });
    expect(html).toContain('data-anchor="slash-commands-in-claude-code"');
    expect(html).not.toContain("{#");
  });

  it("is the same English page after a round trip through Persian", async () => {
    const before = shortcutsPage();
    await setLocale("fa");
    expect(shortcutsPage()).not.toBe(before);
    await setLocale("en");
    expect(shortcutsPage()).toBe(before);
  });
});
