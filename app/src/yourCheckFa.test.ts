// Your check's words in Persian (mu-147): the limit's count, lines and titles, each task's next
// step, what Accept does, the effort groups' and views' names, and Accept all's result line. Each
// is a whole sentence per case, counts are said for 0, 1 and many, and project names, task keys and
// the core's refusals come in as values. English is checked the same way, so the check sees text.
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";
import { BandCount, CheckCount, ColumnLine, OverLine } from "./CheckLimit";
import { tn } from "./i18n";
import { FSI, PDI, setLocale } from "./i18n/runtime";
import type { Criterion, Question, Task } from "./types";
import { acceptAllRun, acceptWords, EFFORTS, nextStep, refusalText, tryText, VIEW_NAMES } from "./yourCheck";
import { bandLimit, checkCount, COLUMN_LINE_MAX, columnLine, overLimit, overLine, overTitle } from "./yourCheckLimit";
import type { QueueInfo } from "./types";

const q = (name: string, yourCheck: number, yourCheckLimit: number | null, label?: string): QueueInfo => ({ name, path: `/p/${name}/tasks`, counts: {}, yourCheck, yourCheckLimit, label });
const question = (n: number, text: string): Question => ({ n, ticked: false, asker: "wt-a", text, escalated: true, ownerAnswered: false });
const crit = (n: number, ticked: boolean, words: string): Criterion => ({ n, label: `AC${n}`, text: `AC${n}: ${words}`, ticked, owner: true });
const agent = (n: number, ticked: boolean, proof = ""): Criterion => ({ n, label: `AC${n}`, text: `AC${n}: works — verify: command`, ticked, owner: false, proof, proofFiles: [] });
const task = (id: string, over: Partial<Task> = {}): Task => ({
  queue: "muster", id, title: `Task ${id}`, kind: "code", priority: "normal", status: "doing", stage: "your-check", blocked: false, stageHistory: [],
  folder: "doing", path: `doing/${id}-t.md`, claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: true, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "your-check", ...over,
});
const html = (el: Parameters<typeof renderToStaticMarkup>[0]) => renderToStaticMarkup(el);
/** A value as a Persian sentence holds it: isolated. */
const iso = (s: string) => `${FSI}${s}${PDI}`;
const plain = (s: string) => s.replace(/[⁦-⁩]/g, "");

/** The data the sentences carry: project names and labels, task keys and criterion labels, the
 *  criterion's own words (task text, written in English), and the core's refusal (its text). */
const DATA = new Set(["muster", "drover", "germ", "Muster", "Drover", "agents", "AC", "Settings", "group", "the", "board", "by", "Feature", "no", "queue", "named", "gone", "is", "now", "in", "Done", "not", "Your", "check", "it", "moved", "on", "since", "you", "looked", "of", "owner's", "criteria", "aren't", "ticked", "pick", "colour", "Child", "webview", "or", "window", "Apple", "review"]);
/** The Latin words left, without the fixture's data. */
const latin = (text: string) => [...new Set(plain(text).match(/[A-Za-z][A-Za-z'’]*/g) ?? [])].filter((w) => w.length > 1 && !DATA.has(w));

afterEach(async () => {
  await setLocale("en");
});

describe("the Your check limit in Persian", () => {
  it("one project over: the count, the column's line, the line above the board and its title", async () => {
    await setLocale("fa");
    const over = overLimit([q("muster", 7, 5)], "muster");
    expect(checkCount(3, over, "muster")).toEqual({ text: "۷ از ۵", over: true, title: "۷ تسک در «بررسی شما» منتظر است، بیش از سقف شما (۵) (تنظیمات ← پروژه‌ها)" });
    expect(columnLine(over, "muster")).toBe("بیش از سقف ۵");
    expect(overLine(over, "muster")).toBe("۷ از ۵ در «بررسی شما»: پیش از شروع تسک‌های بیشتر، این‌ها را بررسی کنید.");
    expect(overTitle(over, "muster")).toBe("۷ تسک در «بررسی شما» منتظر است، بیش از سقف شما (۵): پیش از شروع تسک‌های بیشتر، این‌ها را بررسی کنید.");
    expect(bandLimit(["muster"], over, "muster")).toEqual({ over: true, title: "۷ تسک در «بررسی شما» منتظر است، بیش از سقف شما (۵)" });
  });

  it("All projects: counts the projects for 1 and many, names each by its label, isolated, joined with و", async () => {
    await setLocale("fa");
    const one = overLimit([q("muster", 7, 5), q("germ", 1, 5)], "all");
    expect(columnLine(one, "all")).toBe("۱ پروژه بیش از سقف");
    expect(overLine(one, "all")).toBe("بیش از سقف در ۱ پروژه: پیش از شروع تسک‌های بیشتر، این‌ها را بررسی کنید.");
    expect(overTitle(one, "all")).toBe(`بیش از سقف در ${iso(`${iso("muster")} (۷ از ۵)`)}: پیش از شروع تسک‌های بیشتر، این‌ها را بررسی کنید.`);
    const three = overLimit([q("muster", 7, 5, "Muster"), q("drover", 9, 8, "Drover — agents"), q("germ", 4, 3)], "all");
    expect(columnLine(three, "all")).toBe("۳ پروژه بیش از سقف");
    expect(overLine(three, "all")).toBe("بیش از سقف در ۳ پروژه: پیش از شروع تسک‌های بیشتر، این‌ها را بررسی کنید.");
    expect(plain(overTitle(three, "all"))).toBe("بیش از سقف در Muster (۷ از ۵)، Drover — agents (۹ از ۸) و germ (۴ از ۳): پیش از شروع تسک‌های بیشتر، این‌ها را بررسی کنید.");
    // Each label is a value: isolated, so "Drover — agents" keeps its own direction.
    expect(overTitle(three, "all")).toContain(iso("Drover — agents"));
    expect(plain(checkCount(20, three, "all").title)).toBe("بیش از سقف در Muster (۷ از ۵)، Drover — agents (۹ از ۸) و germ (۴ از ۳)");
    expect(plain(bandLimit(["muster", "germ"], three, "all").title)).toBe(
      "Muster: ۷ تسک در «بررسی شما» منتظر است، بیش از سقف شما (۵)؛ germ: ۴ تسک در «بررسی شما» منتظر است، بیش از سقف شما (۳)",
    );
  });

  it("says the counts for 0, 1 and many in whole sentences", async () => {
    await setLocale("fa");
    for (const [n, digits] of [[0, "۰"], [1, "۱"], [12, "۱۲"]] as const) {
      expect(tn("board", "check.limit.columnAll", { count: n })).toBe(`${digits} پروژه بیش از سقف`);
      expect(tn("board", "check.limit.lineAll", { count: n })).toBe(`بیش از سقف در ${digits} پروژه: پیش از شروع تسک‌های بیشتر، این‌ها را بررسی کنید.`);
      expect(tn("board", "check.limit.pile", { count: n, limit: 5 })).toBe(`${digits} تسک در «بررسی شما» منتظر است، بیش از سقف شما (۵)`);
    }
    await setLocale("en");
    expect(tn("board", "check.limit.columnAll", { count: 1 })).toBe("1 project over limit");
    expect(tn("board", "check.limit.columnAll", { count: 0 })).toBe("0 projects over limit");
    expect(tn("board", "check.limit.pile", { count: 1, limit: 1 })).toBe("1 task waits in Your check, over your limit of 1");
  });

  it("keeps the column's line within its two short lines in Persian too", async () => {
    await setLocale("fa");
    const long = Array.from({ length: 120 }, (_, i) => q(`a-very-long-project-name-${i}`, 60, 50));
    for (const [over, queue] of [[overLimit([q("x", 60, 50)], "x"), "x"], [overLimit(long, "all"), "all"]] as const) {
      expect(columnLine(over, queue).length).toBeLessThanOrEqual(COLUMN_LINE_MAX);
    }
  });

  it("draws the board's places in Persian with no English UI word left", async () => {
    await setLocale("fa");
    const over = overLimit([q("muster", 7, 5), q("drover", 9, 8)], "all");
    const one = overLimit([q("muster", 7, 5)], "muster");
    const drawn = [
      html(createElement(CheckCount, { count: 7, over: one, queue: "muster" })),
      html(createElement(ColumnLine, { over: one, queue: "muster" })),
      html(createElement(OverLine, { over: one, queue: "muster" })),
      html(createElement(CheckCount, { count: 16, over, queue: "all" })),
      html(createElement(ColumnLine, { over, queue: "all" })),
      html(createElement(OverLine, { over, queue: "all" })),
      html(createElement(BandCount, { tasks: [{ queue: "muster" }], over, queue: "all" })),
    ].join(" ");
    const titles = [...drawn.matchAll(/\stitle="([^"]*)"/g)].map((m) => m[1]);
    expect(titles.length).toBe(5);
    expect(latin([drawn.replace(/<[^>]+>/g, " "), ...titles].join(" ").replace(/&[a-z#0-9]+;/gi, " "))).toEqual([]);
    expect(drawn).toContain("۷ از ۵");
    expect(drawn).toContain("بیش از سقف");
  });
});

describe("each task's next step and what Accept does, in Persian", () => {
  it("says every kind of next step as a whole sentence", async () => {
    await setLocale("fa");
    expect(nextStep(task("0001", { questions: [question(2, "Child webview or a window?")] })).text).toBe(`پاسخ به پرسش ۲: ${iso("Child webview or a window?")}`);
    expect(nextStep(task("0001", { blocked: true, blockedKind: "you", blockDetail: "pick a colour" })).text).toBe(`رفع انسداد: ${iso("pick a colour")}`);
    expect(nextStep(task("0001")).text).toBe("چیزی برای امتحان نیست: پذیرفتن");
    const four = [agent(1, true, "ok"), agent(2, true, "ok"), agent(3, true, "ok"), agent(4, true, "ok")];
    expect(nextStep(task("0010", { criteria: four })).text).toBe("هر ۴ معیار اثبات شده‌اند: پذیرفتن");
    expect(nextStep(task("0010", { criteria: [agent(1, true, "ok")] })).text).toBe("تنها معیار اثبات شده است: پذیرفتن");
    expect(nextStep(task("0009", { criteria: [agent(1, true, "ok"), agent(2, true)] })).text).toBe("هر ۲ معیار تیک خورده‌اند: پذیرفتن");
    expect(nextStep(task("0009", { criteria: [agent(1, true)] })).text).toBe("تنها معیار تیک خورده است: پذیرفتن");
    const unproven = { ...agent(3, false), label: "" };
    expect(plain(nextStep(task("0001", { criteria: [agent(1, true, "ok"), agent(2, false), unproven] })).full)).toBe("اثبات‌نشده: AC2، معیار ۳ · آن را برگردانید، یا خودتان بررسی کنید");
    const open = crit(7, false, "the owner groups the board by Feature — verify: owner");
    expect(plain(nextStep(task("0001", { criteria: [open], ownerCriteria: [open] })).text)).toBe("امتحان کنید: group the board by Feature");
    expect(plain(tryText("the owner's screenshots pass the check"))).toBe("بررسی کنید: the owner's screenshots pass the check");
    expect(tryText("")).toBe(`بررسی کنید: ${iso("(بدون متن)")}`);
  });

  it("says what Accept does for each case", async () => {
    await setLocale("fa");
    const four = [agent(1, true, "ok"), agent(2, true, "ok")];
    expect(acceptWords(task("0010", { criteria: four })).text).toBe("همه‌ی معیارها اثبات شده‌اند: «پذیرفتن» آن را به «انجام شد» می‌برد.");
    expect(acceptWords(task("0009", { criteria: [agent(1, true, "ok"), agent(2, true)] })).text).toBe("همه‌ی معیارها تیک خورده‌اند: «پذیرفتن» آن را به «انجام شد» می‌برد.");
    expect(acceptWords(task("0001", { criteria: [] })).text).toBe("هیچ معیار پذیرشی نوشته نشده، پس چیزی اثبات نشده است. «پذیرفتن» آن را به «انجام شد» می‌برد.");
    const a2 = crit(2, false, "x — verify: owner");
    const c3 = { ...crit(3, false, "y — verify: owner"), label: "" };
    expect(plain(acceptWords(task("0001", { criteria: [agent(1, true, "ok"), a2, c3], ownerCriteria: [a2, c3] })).text)).toBe("پس از بررسی ۲ مورد بپذیرید: AC2، معیار ۳.");
    expect(plain(acceptWords(task("0001", { criteria: [agent(1, true, "ok"), a2], ownerCriteria: [a2] })).text)).toBe("پس از بررسی ۱ مورد بپذیرید: AC2.");
    expect(plain(acceptWords(task("0001", { criteria: [agent(1, true, "ok"), agent(2, false)] })).text)).toBe("هنوز اثبات نشده: AC2. آن را برگردانید، یا پیش از پذیرفتن، خودتان بررسی‌اش کنید.");
    const blocked = acceptWords(task("0001", { criteria: four, blocked: true, blockedKind: "you" }));
    expect(blocked.kind).toBe("blocked");
    expect(blocked.text.startsWith("مسدود (")).toBe(true);
    expect(blocked.text.endsWith("): «پذیرفتن» تا رفع آن منتظر می‌ماند.")).toBe(true);
  });

  it("names the effort groups and the views in Persian, read each time", async () => {
    expect(EFFORTS.map((e) => e.label)).toEqual(["Just accept", "One look", "Needs a session"]);
    expect(VIEW_NAMES.map((v) => v.label)).toEqual(["Board", "Table", "Timeline", "Graph", "Helper", "Waiting for you", "Activity", "Problems", "Projects", "Help", "Settings", "Appearance"]);
    await setLocale("fa");
    expect(EFFORTS.map((e) => e.label)).toEqual(["فقط پذیرفتن", "یک نگاه", "نیاز به یک نشست"]);
    const views = VIEW_NAMES.map((v) => v.label);
    expect(latin(views.join(" "))).toEqual([]);
    expect(views).toContain("تنظیمات");
    expect(views.every((v) => v.length > 0)).toBe(true);
  });

  it("no English UI word in any next step or Accept sentence", async () => {
    await setLocale("fa");
    const a2 = crit(2, false, "x — verify: owner");
    const said = [
      nextStep(task("0001", { questions: [question(1, "pick")] })).full,
      nextStep(task("0001", { blocked: true, blockedKind: "you", blockDetail: "pick a colour" })).full,
      nextStep(task("0001")).full,
      nextStep(task("0001", { criteria: [agent(1, false)] })).full,
      nextStep(task("0001", { criteria: [agent(1, true, "ok")] })).full,
      acceptWords(task("0001", { criteria: [agent(1, true, "ok"), a2], ownerCriteria: [a2] })).text,
      acceptWords(task("0001", { criteria: [agent(1, true, "ok"), agent(2, false)] })).text,
      acceptWords(task("0001", { blocked: true, blockedKind: "outside", blockDetail: "Apple review" })).text,
    ].join(" ");
    expect(latin(said)).toEqual([]);
  });
});

describe("Accept all's result line in Persian", () => {
  it("says how many were accepted for 0, 1 and many, and names each refused task by its project and key", async () => {
    await setLocale("fa");
    const all = async (ids: string[]) => ({ accepted: ids, refused: [] as [string, string][] });
    expect((await acceptAllRun([task("0001")], (_q, ids) => all(ids))).text).toBe("۱ مورد پذیرفته شد: انجام شد.");
    expect((await acceptAllRun([task("0001"), task("0002")], (_q, ids) => all(ids))).text).toBe("۲ مورد پذیرفته شد: همه انجام شدند.");
    const some = await acceptAllRun([task("0001"), task("0003")], async () => ({ accepted: ["0001"], refused: [["0003", "0003 is now in Done, not Your check; it moved on since you looked"]] }));
    expect(plain(some.text)).toBe("۱ مورد پذیرفته شد. پذیرفته‌نشده: muster 0003: it is now in Done, not Your check; it moved on since you looked.");
    expect(some.text).toContain(iso("muster"));
    const most = await acceptAllRun([task("0001"), task("0002"), task("0003")], async () => ({ accepted: ["0001", "0002"], refused: [["0003", "0003 is now in Done, not Your check"]] }));
    expect(plain(most.text)).toBe("۲ مورد پذیرفته شد. پذیرفته‌نشده: muster 0003: it is now in Done, not Your check.");
    const none = await acceptAllRun([task("0001", { queue: "gone" })], async () => {
      throw new Error("no queue named \"gone\"");
    });
    expect(plain(none.text)).toBe("هیچ موردی پذیرفته نشد. پذیرفته‌نشده: gone: Error: no queue named \"gone\".");
    expect(latin(none.text.replace(/Error: no queue named "gone"/, ""))).toEqual([]);
  });

  it("joins several refusals with the Persian semicolon", async () => {
    await setLocale("fa");
    const r = await acceptAllRun([task("0001"), task("0002")], async () => ({ accepted: [], refused: [["0001", "0001 is now in Done, not Your check"], ["0002", "0002 is now in Done, not Your check"]] }));
    expect(plain(r.text)).toBe("هیچ موردی پذیرفته نشد. پذیرفته‌نشده: muster 0001: it is now in Done, not Your check؛ muster 0002: it is now in Done, not Your check.");
  });

  it("says the core's tick hint the app's way, in the language shown", async () => {
    await setLocale("fa");
    expect(plain(refusalText("0003", "0003: 1 of the owner's criteria aren't ticked; tick each once you've checked it (muster tick 0003 <AC>), then pass it to Done"))).toBe(
      "1 of the owner's criteria aren't ticked؛ ابتدا معیارهایش را تیک بزنید",
    );
  });
});
