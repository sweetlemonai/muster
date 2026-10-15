// The Table, the Timeline, the Graph, the Backlog's notes and quick add, a note's pane and New
// task in Persian (mu-147): drawn with the Persian files loaded they show no English UI text, every
// count says itself in the plural it needs, and English reads as it did. What's left in Latin
// letters is the fixture's own data (titles, ids, projects, a holder, a file name), product names
// and code.
import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, onTestFinished, vi } from "vitest";
import { backlogColumn, NoteCard, noteTitle, QuickAdd } from "./Backlog";
import { GraphView } from "./GraphView";
import { unmarked } from "./i18n";
import en from "./i18n/locales/en/board.json";
import { FSI, PDI, setLocale, setNumbering } from "./i18n/runtime";
import { stampMinutes } from "./lib";
import NewTask, { FOR_CHOICES, parentRequest } from "./NewTask";
import NotePanel, { copyNow } from "./NotePanel";
import type { Note, Task } from "./types";
import { axis, barTitle, layout, nowAt, SCALES, segmentTitle, spans, timeWindow, windowLabel, type Scale } from "./viewData";
import { TableView, TimelineChart, TimelineControls, TimelineView } from "./Views";

const t = (over: Partial<Task>): Task => ({
  queue: "alpha", id: "0001", title: "T", kind: "bug", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [], folder: "todo", path: "", claimedBy: "", heldBy: "", stageSince: "", checkins: [],
  heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "2026-10-14", capturedAt: "", doneAt: "", blockHistory: [], questions: [], openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false,
  tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", search: "", ...over,
});
const step = (at: string, from: string, to: string) => ({ at, who: "agent", from, to });
const note = (over: Partial<Note>): Note => ({ queue: "alpha", file: "2026-10-12-1405-x.md", title: "Flicker", text: "Flicker\n\nmore", truncated: false, added: 0, heldBy: "", heldSince: "", heartbeat: "", ...over });

/** 2026-10-14 17:20 on the local clock: in seconds (the views' `now`) and in wall minutes. */
const SECONDS = new Date(2026, 9, 14, 17, 20).getTime() / 1000;
const NOW = stampMinutes("2026-10-14 17:20")!;
const started = (at: string) => ({ stage: "building", stageHistory: [step(at, "ready", "building")] });

// One of each thing the views say: a high-priority task that needs you with open questions and a
// tag, one paused, one done, one not started (no times in its log), and links for the graph.
const TASKS: Task[] = [
  t({ id: "0001", title: "Alpha", priority: "high", waitingForOwner: true, openQuestions: 2, tags: ["ui"], claimedBy: "gholi", modified: SECONDS - 7200, ...started("2026-10-14 13:00") }),
  t({ id: "0002", title: "Beta", depends: ["0001", "0009"], paused: "2026-10-14 15:00", pausedWhy: "later", pauseHistory: [{ from: "2026-10-14 15:00", to: "" }], ...started("2026-10-14 14:00") }),
  t({ id: "0003", title: "Gamma", stage: "done", depends: ["0002"], doneOn: "2026-10-14", stageHistory: [step("2026-10-14 09:00", "ready", "building"), step("2026-10-14 12:00", "building", "done")] }),
  t({ id: "0004", title: "Delta", kind: "docs", priority: "low", stage: "ready" }),
  t({ id: "0005", title: "Epsilon", kind: "research", stage: "backlog" }),
];
// Two more projects for the graph: one with nothing linked, one whose links are all filtered out.
const OTHERS: Task[] = [t({ queue: "beta", id: "0010", title: "Lone" }), t({ queue: "gamma", id: "0020", title: "Waits", depends: ["0021"] })];

/** Everything this area draws, in the language loaded now. */
function views(): string {
  vi.stubGlobal("localStorage", { getItem: () => "day", setItem: () => {} });
  const win = timeWindow("week", NOW, 0);
  const parts: ReactElement[] = [
    createElement(TableView, { tasks: TASKS, now: SECONDS, multi: true, openKey: null, onOpen: () => {}, words: [] }),
    createElement(TableView, { tasks: [], now: SECONDS, multi: false, openKey: null, onOpen: () => {}, words: [] }),
    createElement(TimelineView, { tasks: TASKS, now: SECONDS, multi: true, openKey: null, onOpen: () => {}, words: [] }),
    createElement(TimelineView, { tasks: [], now: SECONDS, multi: false, openKey: null, onOpen: () => {}, words: [] }),
    ...(["day", "week", "month"] as Scale[]).map((scale) => createElement(TimelineControls, { key: scale, scale, offset: -1, order: "active", label: "x", onScale: () => {}, onOrder: () => {}, onStep: () => {}, onToday: () => {} })),
    createElement(TimelineChart, { scale: "week", rows: layout(spans(TASKS, NOW), win, "week"), ticks: axis("week", win, NOW), now: nowAt(win, NOW), multi: false, openKey: null, onOpen: () => {} }),
    createElement(GraphView, { tasks: [...TASKS, ...OTHERS], all: [...TASKS, ...OTHERS, t({ queue: "gamma", id: "0021", title: "Gone" })], multi: true, openKey: "alpha/0002", onOpen: () => {} }),
    createElement(GraphView, { tasks: [], all: [], multi: false, openKey: null, onOpen: () => {} }),
    createElement(NoteCard, { n: note({}), now: 7300, multi: true, active: false, onOpen: () => {}, words: [] }),
    createElement(NoteCard, { n: note({ title: "" }), now: 30, multi: false, active: false, onOpen: () => {}, words: [] }),
    createElement(QuickAdd, { queues: ["alpha", "beta"], onAdded: () => {} }),
    createElement(QuickAdd, { queues: [], onAdded: () => {} }),
    createElement(NotePanel, { note: note({ truncated: true }), now: 3600, onClose: () => {}, onChanged: () => {}, onFiled: () => {} }),
    createElement(NotePanel, { note: note({ title: "", text: "", added: 3590 }), now: 3600, onClose: () => {}, onChanged: () => {}, onFiled: () => {} }),
  ];
  try {
    return parts.map((p) => renderToStaticMarkup(p)).join("\n");
  } finally {
    vi.unstubAllGlobals();
  }
}

const newTask = () => renderToStaticMarkup(createElement(NewTask, { queues: ["alpha"], initial: "alpha", prefixes: { alpha: "al" }, onClose: () => {}, onCreated: () => {} }));

/** The fixture's own data, and names that stay as they are: titles, projects, the holder, a tag,
 *  a note's file name, keys (al-30), product names. */
const KEPT = new Set(["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Lone", "Waits", "Gone", "Flicker", "more", "later", "alpha", "beta", "gamma", "gholi", "ui", "md", "al", "Muster", "Markdown", "Acceptance", "criteria"]);

/** What a person sees or hears: the text and the text attributes, without code and the isolation marks. */
function shown(html: string): string {
  const attrs = [...html.matchAll(/\s(?:title|aria-label|placeholder|alt)="([^"]*)"/g)].map((m) => m[1]);
  const body = html.replace(/<code[^>]*>[\s\S]*?<\/code>/g, " ").replace(/<[^>]+>/g, " ");
  return unmarked([body, ...attrs].join(" ").replace(/&#x27;/g, "'").replace(/&[a-z#0-9]+;/gi, " "));
}

/** The Latin words in it, but for the fixture's data and kept names. */
function latinWords(html: string): string[] {
  const words = (shown(html).match(/[A-Za-z][A-Za-z'’-]*/g) ?? []).map((w) => w.replace(/[-'’]+$/, ""));
  return [...new Set(words)].filter((w) => w.length > 1 && !KEPT.has(w));
}

type Tree = { [k: string]: string | Tree };
const AREA = ["views", "graph", "backlog", "note", "newTask"] as const;
/** This area's English words: each string's runs between its placeholders and tags. */
const ENGLISH = AREA.flatMap((s) => {
  const leaves = (m: Tree): string[] => Object.values(m).flatMap((v) => (typeof v === "string" ? [v] : leaves(v)));
  return leaves((en as unknown as Record<string, Tree>)[s]);
})
  .flatMap((v) => v.split(/\{\{[^}]+\}\}|<\/?\w+\s*\/?>/))
  .map((p) => p.trim())
  .filter((p) => /[A-Za-z]{3,}/.test(p) && p.length > 3);

afterEach(async () => {
  await setLocale("en");
});

describe("the views and the backlog in Persian (mu-147)", () => {
  it("draws English in English: the check sees the area's words", () => {
    const words = latinWords(views());
    for (const w of ["Project", "Tags", "Paused", "Scale", "Previous", "Hide", "Arrows", "note", "backlog", "Drop", "dependencies", "needed", "waits"]) expect(words).toContain(w);
    expect(latinWords(newTask())).toContain("Parent");
  });

  it("shows no English UI text in the Table, Timeline, Graph, the backlog's notes and quick add, or a note's pane", async () => {
    await setLocale("fa");
    const html = views();
    expect(latinWords(html)).toEqual([]);
    for (const w of ["پروژه", "تگ‌ها", "نیاز به شما", "متوقف شده", "مقیاس", "هفته‌ی قبل", "بدون انجام‌شده‌ها", "پیش‌نیاز", "منتظر آن", "یادداشت", "افزودن به بک‌لاگ…", "کنار گذاشتن", "باگ", "پایین", "در حال ساخت"]) expect(shown(html)).toContain(w);
  });

  it("says none of its own English in New task (its tags and level chips are other areas')", async () => {
    await setLocale("fa");
    const text = shown(newTask());
    expect(ENGLISH.length).toBeGreaterThan(100);
    expect(ENGLISH.filter((e) => text.includes(e))).toEqual([]);
    for (const w of ["تسک جدید", "والد", "انسان", "ایجنت", "ایجاد (", "زمان ایجنت", "مستندات"]) expect(text).toContain(w);
    expect(text).toContain("al-30");
  });

  it("names each of the area's columns, chips and buttons in Persian", async () => {
    await setLocale("fa");
    const table = renderToStaticMarkup(createElement(TableView, { tasks: TASKS, now: SECONDS, multi: true, openKey: null, onOpen: () => {}, words: [] }));
    const heads = [...table.matchAll(/<th [^>]*>(?:<button class="sort">)?([^<]*)/g)].map((m) => unmarked(m[1]).replace(/ [↑↓]$/, ""));
    expect(heads).toEqual(["پروژه", "شناسه", "عنوان", "نوع", "اولویت", "مرحله", "در دست / در انتظار", "تخمین", "تاریخ ثبت", "آخرین تغییر", "تگ‌ها"]);
    const controls = renderToStaticMarkup(createElement(TimelineControls, { scale: "week", offset: 0, order: "active", label: "x", onScale: () => {}, onOrder: () => {}, onStep: () => {}, onToday: () => {} }));
    expect([...controls.matchAll(/<button[^>]*>(.*?)<\/button>/g)].map((m) => m[1].replace(/<[^>]+>/g, ""))).toEqual(["روز", "هفته", "ماه", "اول کارهای جاری", "زودترین شروع", "◀", "این هفته", "▶"]);
    expect(controls).toContain('aria-label="هفته‌ی قبل"');
    expect(controls).toContain('<span class="tl-order" role="group" aria-label="ترتیب خط زمانی"><span class="meta">ترتیب</span>');
    expect(SCALES.map((s) => s.label)).toEqual(["روز", "هفته", "ماه"]);
    expect(FOR_CHOICES.map((f) => f.label)).toEqual(["انسان", "ایجنت"]);
  });

  it("says each count in the plural it needs, in both languages", async () => {
    const counts = () => {
      const chart = (tasks: Task[]) => {
        const win = timeWindow("week", NOW, 0);
        const html = renderToStaticMarkup(createElement(TimelineChart, { scale: "week", rows: layout(spans(tasks, NOW), win, "week"), ticks: axis("week", win, NOW), now: nowAt(win, NOW), multi: false, openKey: null, onOpen: () => {} }));
        return unmarked(/<span class="tl-label meta">([^<]*)<\/span>/.exec(html)?.[1] ?? "");
      };
      const notStarted = (n: number) => {
        vi.stubGlobal("localStorage", { getItem: () => "week", setItem: () => {} });
        const html = renderToStaticMarkup(createElement(TimelineView, { tasks: Array.from({ length: n }, (_, i) => t({ id: `00${i}` })), now: SECONDS, multi: false, openKey: null, onOpen: () => {}, words: [] }));
        vi.unstubAllGlobals();
        return shown(/<p class="meta tl-not-started">([^<]*)<\/p>/.exec(html)?.[1] ?? "").trim();
      };
      const openQ = (n: number) => shown(/<span class="ui-badge">([^<]*)<\/span>/.exec(renderToStaticMarkup(createElement(TableView, { tasks: [t({ openQuestions: n })], now: 0, multi: false, openKey: null, onOpen: () => {}, words: [] })))?.[1] ?? "");
      const waitOnIt = (n: number) => {
        const tasks = [t({ id: "0001" }), ...Array.from({ length: n }, (_, i) => t({ id: `010${i}`, depends: ["0001"] }))];
        const html = renderToStaticMarkup(createElement(GraphView, { tasks, all: tasks, multi: false, openKey: null, onOpen: () => {} }));
        return shown(/<span class="g-deps">([^<]*)<\/span>/.exec(html)?.[1] ?? "").trim();
      };
      return {
        rows: [chart([]), chart([TASKS[0]]), chart(TASKS)],
        notStarted: [notStarted(1), notStarted(3)],
        openQ: [openQ(1), openQ(2)],
        waitOnIt: [waitOnIt(1), waitOnIt(2)],
      };
    };
    expect(counts()).toEqual({
      rows: ["0 tasks", "1 task", "3 tasks"],
      notStarted: [
        "1 task hasn't started yet (or the log has no times for them), so they aren't drawn.",
        "3 tasks haven't started yet (or the log has no times for them), so they aren't drawn.",
      ],
      openQ: ["1 open Q", "2 open Q"],
      waitOnIt: ["1 waits on it", "2 wait on it"],
    });
    await setLocale("fa");
    expect(counts()).toEqual({
      rows: ["۰ تسک", "۱ تسک", "۳ تسک"],
      notStarted: [
        "۱ تسک هنوز شروع نشده است (یا لاگ زمانی برای آن ندارد)، پس رسم نشده است.",
        "۳ تسک هنوز شروع نشده‌اند (یا لاگ زمانی برای آن‌ها ندارد)، پس رسم نشده‌اند.",
      ],
      openQ: ["۱ پرسش باز", "۲ پرسش باز"],
      waitOnIt: ["۱ تسک منتظر آن است", "۲ تسک منتظر آن هستند"],
    });
  });

  it("writes the timeline's dates with the language's month and weekday names and digits", async () => {
    const M = (s: string) => stampMinutes(s)!;
    const dates = (calendar: "gregorian" | "jalali" = "gregorian") => ({
      day: windowLabel("day", timeWindow("day", NOW, 0)),
      week: windowLabel("week", timeWindow("week", NOW, 0)),
      weeks: windowLabel("week", timeWindow("week", M("2026-10-01 12:00"), 0)),
      years: windowLabel("week", timeWindow("week", M("2026-01-01 12:00"), 0)),
      // Across a month in Jalali only (Shahrivar into Mehr), and across Nowruz.
      months: windowLabel("week", timeWindow("week", M("2026-09-23 12:00"), 0)),
      nowruz: windowLabel("week", timeWindow("week", M("2026-03-18 12:00"), 0)),
      month: windowLabel("month", timeWindow("month", NOW, 0, calendar)),
      ticks: axis("week", timeWindow("week", NOW, 0), NOW).slice(0, 2).map((k) => k.label),
      midnight: axis("day", timeWindow("day", NOW, 0), NOW)[3].label,
      segment: segmentTitle({ stage: "building", from: M("2026-10-14 13:00"), to: M("2026-10-14 15:00"), fuzzy: false }),
      waiting: segmentTitle({ stage: "waiting", from: M("2026-10-11 00:00"), to: M("2026-10-14 13:00"), fuzzy: true }),
      waitingDays: segmentTitle({ stage: "waiting", from: M("2026-10-11 00:00"), to: M("2026-10-14 00:00"), fuzzy: true }),
      bar: barTitle(TASKS[1], false),
    });
    expect(dates()).toEqual({
      day: "Tue 13 Oct 21:00 – Wed 14 Oct 21:00",
      week: "12 – 18 Oct 2026",
      weeks: "28 Sep – 4 Oct 2026",
      years: "29 Dec 2025 – 4 Jan 2026",
      months: "21 – 27 Sep 2026",
      nowruz: "16 – 22 Mar 2026",
      month: "October 2026",
      ticks: ["Mon 12", "Tue 13"],
      midnight: "Wed",
      segment: "Building 14 Oct 13:00 – 15:00",
      waiting: "Waiting 11 Oct – 14 Oct 13:00",
      waitingDays: "Waiting 11 Oct – 13 Oct",
      bar: "Building · paused: later · captured 2026-10-14 · dates only",
    });
    await setLocale("fa");
    // The Gregorian calendar picked: فارسی's own is Jalali (mu-151, viewData.test.ts).
    setNumbering({ calendar: "gregorian" });
    onTestFinished(() => {
      setNumbering({});
    });
    expect(Object.fromEntries(Object.entries(dates()).map(([k, v]) => [k, Array.isArray(v) ? v.map(unmarked) : unmarked(v)]))).toEqual({
      day: "سه‌شنبه ۱۳ اکتبر ۲۱:۰۰ تا چهارشنبه ۱۴ اکتبر ۲۱:۰۰",
      week: "۱۲ تا ۱۸ اکتبر ۲۰۲۶",
      weeks: "۲۸ سپتامبر تا ۴ اکتبر ۲۰۲۶",
      years: "۲۹ دسامبر ۲۰۲۵ تا ۴ ژانویه ۲۰۲۶",
      months: "۲۱ تا ۲۷ سپتامبر ۲۰۲۶",
      nowruz: "۱۶ تا ۲۲ مارس ۲۰۲۶",
      month: "اکتبر ۲۰۲۶",
      ticks: ["دوشنبه ۱۲", "سه‌شنبه ۱۳"],
      midnight: "چهارشنبه",
      segment: "در حال ساخت ۱۴ اکتبر ۱۳:۰۰ تا ۱۵:۰۰",
      waiting: "در انتظار ۱۱ اکتبر تا ۱۴ اکتبر ۱۳:۰۰",
      waitingDays: "در انتظار ۱۱ اکتبر تا ۱۳ اکتبر",
      bar: "در حال ساخت · متوقف شده: later · ثبت در ۱۴ اکتبر ۲۰۲۶ · فقط تاریخ‌ها",
    });
    // فارسی's own calendar, Jalali (review R10): a week's two ends name one month when they're in
    // the same Jalali month, whatever the Gregorian months, and the year when the Jalali years differ.
    setNumbering({});
    expect(Object.fromEntries(Object.entries(dates("jalali")).map(([k, v]) => [k, Array.isArray(v) ? v.map(unmarked) : unmarked(v)]))).toEqual({
      day: "سه‌شنبه ۲۱ مهر ۲۱:۰۰ تا چهارشنبه ۲۲ مهر ۲۱:۰۰",
      week: "۲۰ تا ۲۶ مهر ۱۴۰۵",
      weeks: "۶ تا ۱۲ مهر ۱۴۰۵",
      years: "۸ تا ۱۴ دی ۱۴۰۴",
      months: "۳۰ شهریور تا ۵ مهر ۱۴۰۵",
      nowruz: "۲۵ اسفند ۱۴۰۴ تا ۲ فروردین ۱۴۰۵",
      month: "مهر ۱۴۰۵",
      ticks: ["دوشنبه ۲۰", "سه‌شنبه ۲۱"],
      midnight: "چهارشنبه",
      segment: "در حال ساخت ۲۲ مهر ۱۳:۰۰ تا ۱۵:۰۰",
      waiting: "در انتظار ۱۹ مهر تا ۲۲ مهر ۱۳:۰۰",
      waitingDays: "در انتظار ۱۹ مهر تا ۲۱ مهر",
      bar: "در حال ساخت · متوقف شده: later · ثبت در ۲۲ مهر ۱۴۰۵ · فقط تاریخ‌ها",
    });
  });

  it("says the backlog's and New task's own messages in Persian, values isolated", async () => {
    await setLocale("fa");
    expect(noteTitle({ title: " " })).toBe("(یادداشت خالی)");
    const why = (typed: string, prefix?: string) => {
      const r = parentRequest(typed, prefix);
      return r.ok ? "" : r.why;
    };
    // The key and the number are isolated, and so is the example they make.
    expect(why("soon", "al")).toBe(`والد یک قابلیت یا هدف از همین پروژه است: ${FSI}${FSI}al-30${PDI} یا ${FSI}30${PDI}${PDI}`);
    expect(unmarked(why("br-30", "al"))).toBe("br-30 تسک پروژه‌ی دیگری است: والد باید در همین پروژه باشد (al-30 یا 30)");
    expect(unmarked(why("soon"))).toBe("والد یک قابلیت یا هدف از همین پروژه است: 30");
    await expect(copyNow(null, undefined)).rejects.toThrow("شرح کار شکل‌دهی");
    await expect(copyNow("x", undefined)).rejects.toThrow("کلیپ‌بورد در اینجا در دسترس نیست.");
    expect(backlogColumn([], [])).toEqual([]);
  });

  it("says when a note was added as one sentence, just added included", async () => {
    const meta = (added: number) => {
      const html = renderToStaticMarkup(createElement(NotePanel, { note: note({ added }), now: 3600, onClose: () => {}, onChanged: () => {}, onFiled: () => {} }));
      return shown(/<div class="meta">([\s\S]*?)<\/div>/.exec(html)?.[1] ?? "").replace(/\s+/g, " ").trim();
    };
    expect([meta(0), meta(3590)]).toEqual(["alpha · backlog note · added 1 h ago", "alpha · backlog note · added just now"]);
    await setLocale("fa");
    expect([meta(0), meta(3590)]).toEqual(["alpha · یادداشت بک‌لاگ · ۱ ساعت پیش افزوده شد", "alpha · یادداشت بک‌لاگ · لحظاتی پیش افزوده شد"]);
  });

  it("isolates a task's and a note's own text, whatever its direction", async () => {
    await setLocale("fa");
    const html = views();
    expect(html).toMatch(/<span dir="auto">Alpha<\/span>/);
    expect(html).toMatch(/<span class="tag" dir="auto">#(<!-- -->)?ui<\/span>/);
    expect(html).toContain('<span class="g-title" dir="auto">Beta</span>');
    expect(html).toMatch(/<span class="title"><span dir="auto">Flicker<\/span><\/span>/);
    expect(html).toContain('<h1><span dir="auto">Flicker</span></h1>');
    expect(html).toContain('<span class="g-who" dir="auto">gholi</span>');
  });
});
