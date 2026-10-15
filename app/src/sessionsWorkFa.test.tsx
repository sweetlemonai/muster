// Sessions and work in Persian (mu-147): the Run line and block, the Sessions view and its recap,
// reaching a session, the old-pauses offer and Lessons, drawn with the Persian files loaded. No
// English UI word is left; every count reads right for 0, 1 and many; and the data (names, ids,
// commands, the core's words) shows as written, isolated. English is checked the same way, so the
// check itself sees text.
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";
import type { OldPause } from "./api";
import { tn } from "./i18n";
import { setLocale } from "./i18n/runtime";
import LessonsWaiting, { decidedText, lessonMeta } from "./Lessons";
import { Mode } from "./modes";
import { OldPausesNotice } from "./OldPauses";
import { failedLine, oldPauseLine } from "./oldPauseData";
import { CardPing, MessageCount, SessionPing } from "./Ping";
import { RunBlock, RunButtons, RunLine, RunStatusLive } from "./Run";
import { fmtRunTime, statusWords } from "./runData";
import { RecapView } from "./SessionRecap";
import { SessionsView } from "./Sessions";
import { chainLine, groupSessions, headerCount, historyKind, rowLabel, WORDS } from "./sessionsData";
import type { RunSnap, RunView, SessionItem, SessionRecap, SessionsList, Task } from "./types";

const FSI = "\u2068";
const PDI = "\u2069";
/** `s` without the isolation marks a right-to-left language puts around filled-in values. */
const bare = (s: string) => s.replace(/[\u2066-\u2069]/g, "");
const noop = () => {};

/** Product names that stay Latin in every language (docs/i18n/glossary-fa.md). */
const KEPT = new Set(["Muster", "Claude", "Code", "Mac"]);
/** The fixtures' own data: project, command and folder, session names and ids, accounts, the
 *  core's words for a state, task keys, a lesson's kind and ids, and the run's output lines. */
const DATA = new Set(["muster", "main", "bin", "run", "checkin", "Asghar", "Gholi", "Asker", "aaaa", "cc-mg", "cc-hg", "mg", "wt", "mu", "dr", "stopped", "blocked", "unbounded-output", "src-tauri", "app", "src", "Compiling", "line", "Pets", "L1"]);

/** The Latin words a person sees or hears: text and the text attributes, without code, paths
 *  and the fixtures' data. */
function latinWords(html: string): string[] {
  const attrs = [...html.matchAll(/\s(?:title|aria-label|placeholder|alt)="([^"]*)"/g)].map((m) => m[1]);
  const body = html.replace(/<code[^>]*>[\s\S]*?<\/code>/g, " ").replace(/<[^>]+>/g, " ");
  const text = [body, ...attrs]
    .join(" ")
    .replace(/&[a-z#0-9]+;/gi, " ")
    .replace(/[~/][^\s,،)]*/g, " ")
    .replace(/\b\d+(\.\d+)?[kM]\b/g, " ");
  const words = (text.match(/[A-Za-z][A-Za-z'’-]*/g) ?? []).map((w) => w.replace(/[-'’]+$/, ""));
  return [...new Set(words)].filter((w) => w.length > 1 && !KEPT.has(w) && !DATA.has(w) && !/^[a-f0-9]+$/.test(w));
}

afterEach(async () => {
  await setLocale("en");
});

// ── Run ───────────────────────────────────────────────────

const snap = (over: Partial<RunSnap> = {}): RunSnap => ({
  project: "muster", key: "/q/muster/.muster", id: 1, command: "bin/run", folder: "/Users/someone/playground/muster/main",
  status: { kind: "running" }, since: 1000, ended: null, first: 0, lines: [], ...over,
});
const runView = (over: Partial<RunView> = {}): RunView => ({
  project: "muster", key: "/q/muster/.muster", command: "bin/run", folder: "/Users/someone/playground/muster/main", refused: "", run: null, ...over,
});
const lines = (n: number) => Array.from({ length: n }, (_, i) => `line ${i}`);
const runAll = () =>
  [
    renderToStaticMarkup(<RunLine project="muster" info={runView()} run={null} busy={false} error="" onRun={noop} />),
    renderToStaticMarkup(<RunLine project="muster" info={runView()} run={snap()} busy={false} error="" onRun={noop} />),
    renderToStaticMarkup(<RunLine project="muster" info={runView({ command: "" })} run={null} busy={false} error="" onRun={noop} />),
    renderToStaticMarkup(<RunBlock project="muster" run={snap({ lines: lines(20) })} nowSecs={1080} busy={false} all={false} onAll={noop} onRun={noop} onStop={noop} />),
    renderToStaticMarkup(<RunBlock project="muster" run={snap({ lines: lines(20) })} nowSecs={1080} busy={false} all={true} onAll={noop} onRun={noop} onStop={noop} />),
    renderToStaticMarkup(<RunBlock project="muster" run={snap({ status: { kind: "stopped" }, ended: 1062 })} nowSecs={1080} busy={false} all={false} onAll={noop} onRun={noop} onStop={noop} />),
    renderToStaticMarkup(<RunStatusLive run={snap({ status: { kind: "exited", code: 0 }, ended: 1130 })} />),
    renderToStaticMarkup(<RunButtons project="muster" run={snap()} busy={false} refused="" error="" onRun={noop} onStop={noop} />),
  ].join("\n");

describe("Run in Persian", () => {
  it("draws English in English: the check sees the Run's words", () => {
    const words = latinWords(runAll());
    for (const w of ["Run", "Restart", "Stop", "again", "Running", "output", "Stopped", "Exited"]) expect(words).toContain(w);
  });

  it("shows no English UI text in the Run line, the block or the buttons", async () => {
    await setLocale("fa");
    const html = runAll();
    expect(latinWords(html)).toEqual([]);
    for (const w of ["اجرا", "اجرای دوباره", "متوقف کردن", "هنوز خروجی‌ای نیست."]) expect(html).toContain(w);
    // How to set a run command: the command is code, the project in it as written.
    expect(html).toContain("فرمان اجرا تعریف نشده است: <code class=\"ltr\">muster projects set muster run …</code>");
  });

  it("says a run's time in full words, the units joined with و", async () => {
    await setLocale("fa");
    expect(fmtRunTime(0)).toBe("۰ ثانیه");
    expect(fmtRunTime(59)).toBe("۵۹ ثانیه");
    expect(fmtRunTime(80)).toBe("۱ دقیقه و ۲۰ ثانیه");
    expect(fmtRunTime(3900)).toBe("۱ ساعت و ۵ دقیقه");
  });

  it("words the state as whole sentences, the command, folder and exit code isolated as written", async () => {
    await setLocale("fa");
    expect(statusWords(snap())).toBe(`در حال اجرای ${FSI}bin/run${PDI} در ${FSI}muster/main${PDI}`);
    expect(statusWords(snap({ status: { kind: "exited", code: 101 }, ended: 1004 }))).toBe(`پس از ${FSI}۴ ثانیه${PDI} با کد ${FSI}101${PDI} پایان یافت`);
    expect(statusWords(snap({ status: { kind: "exited", code: null }, ended: 1004 }))).toBe(`پس از ${FSI}۴ ثانیه${PDI} با یک سیگنال پایان یافت`);
    expect(statusWords(snap({ status: { kind: "stopped" }, ended: 1062 }))).toBe(`پس از ${FSI}۱ دقیقه و ۲ ثانیه${PDI} متوقف شد`);
    // A run the core hasn't timed: no "after".
    expect(statusWords(snap({ status: { kind: "stopped" } }))).toBe("متوقف شد");
    expect(statusWords(snap({ status: { kind: "exited", code: 0 } }))).toBe(`با کد ${FSI}0${PDI} پایان یافت`);
  });

  it("counts the output's lines for 0, 1 and many", async () => {
    expect([0, 1, 20].map((count) => tn("board", "run.allOutput", { count }))).toEqual(["All output (0 lines)", "All output (1 line)", "All output (20 lines)"]);
    await setLocale("fa");
    expect([0, 1, 20].map((count) => tn("board", "run.allOutput", { count }))).toEqual(["همه‌ی خروجی (۰ خط)", "همه‌ی خروجی (۱ خط)", "همه‌ی خروجی (۲۰ خط)"]);
    expect(tn("board", "run.lastLines", { count: 12 })).toBe("۱۲ خط آخر");
  });
});

// ── Sessions and the recap ────────────────────────────────

const NOW = 1_800_000_000;
let n = 0;
const row = (over: Partial<SessionItem> = {}): SessionItem => ({
  account: "mg", id: `aaaa${String(n++).padStart(4, "0")}`, name: "Asghar_6", group: "completed", word: "stopped", message: "کار تمام شد", project: "muster", started: NOW - 3600, at: NOW - 660, ...over,
});
const sessionsList = (rows: SessionItem[]): SessionsList => ({ rows, reading: false, problems: [], accounts: ["cc-mg", "cc-hg"] });
const FIVE = () => [
  row({ name: "Asker", group: "needs-input", word: "blocked", at: NOW - 60 }),
  row({ name: "Gholi_1", group: "working", word: "working", at: NOW - 120 }),
  row({ name: "Asghar_1", at: NOW - 600 }),
  row({ name: "Asghar_2", at: NOW - 900 }),
  row({ name: "Asghar_3", at: NOW - 3 * 86_400 }),
];
const sessionsView = (mode: "adhd" | "focus") =>
  renderToStaticMarkup(
    <Mode.Provider value={mode}>
      <SessionsView list={sessionsList(FIVE())} error="" now={NOW} all={true} selected={null} onOpen={noop} />
    </Mode.Provider>,
  );

const recap = (over: Partial<SessionRecap> = {}): SessionRecap => ({
  who: { account: "mg", id: "aaaa0005", name: "Asghar_5", project: "muster", checkout: "wt-0098", group: "completed", word: "stopped", at: NOW - 660 },
  tasks: [{ queue: "muster", id: "0098", key: "mu-98", title: "نمای نشست‌ها", stage: "testing", holds: true }],
  history: [
    { at: NOW - 7200, key: "", kind: "handoff-from", text: "از Asghar_4 تحویل گرفت", pct: null },
    { at: NOW - 7100, key: "mu-98", kind: "takeover", text: "برداشتن را در دست گرفت", pct: null },
    { at: NOW - 3600, key: "mu-98", kind: "check-in", text: "هسته تمام شد", pct: 40 },
    { at: NOW - 3500, key: "mu-98", kind: "claim", text: "برداشت", pct: null },
    { at: NOW - 3400, key: "mu-98", kind: "note", text: "یادداشت", pct: null },
    { at: NOW - 3300, key: "mu-98", kind: "return", text: "برگشت", pct: null },
    { at: NOW - 3200, key: "mu-98", kind: "stage", text: "مرحله", pct: null },
    { at: NOW - 700, key: "mu-98", kind: "pass", text: "به تست رفت", pct: null },
  ],
  chain: [
    { id: "aaaa0004", name: "Asghar_4", current: false },
    { id: "aaaa0005", name: "Asghar_5", current: true },
    { id: "aaaa0006", name: "", current: false },
  ],
  cost: {
    found: "read", start: NOW - 7200, end: NOW - 660, minutes: 109, tokens: { input: 1000, output: 2000, cacheRead: 300_000, cacheWrite: 50_000 }, totalTokens: 353_000, countedTokens: 53_000,
    chainMinutes: 300, chainTokens: { input: 3000, output: 6000, cacheRead: 900_000, cacheWrite: 150_000 }, chainTotalTokens: 1_059_000, chainCountedTokens: 159_000, chainRead: 2, chainSessions: 3,
    estTokens: 1_500_000, estPct: 4, chainEstPct: 11,
  },
  progress: [
    { key: "mu-98", pct: 40, text: "هسته تمام شد", guess: false },
    { key: "mu-99", pct: 60, text: "دو معیار از سه", guess: true },
  ],
  ...over,
});
const recapView = (r: SessionRecap | null, more: { showing?: boolean; error?: string } = {}) =>
  renderToStaticMarkup(
    <Mode.Provider value="adhd">
      <RecapView recap={r} error={more.error ?? ""} now={NOW} showing={more.showing ?? false} transcript={more.showing ? { md: "# سلام" } : null} onTranscript={noop} onClose={noop} onOpenTask={noop} />
    </Mode.Provider>,
  );
const recapAll = () =>
  [
    recapView(recap(), { showing: true }),
    recapView(null),
    recapView(recap({ tasks: [], history: [], progress: [] })),
    ...(["missing", "too-big", "not-read"] as const).map((found) => recapView(recap({ cost: { ...recap().cost, found } }))),
  ].join("\n");

describe("the Sessions view in Persian", () => {
  it("draws English in English: the check sees the view's words", () => {
    const words = latinWords(sessionsView("adhd") + sessionsView("focus"));
    for (const w of ["Sessions", "Needs", "input", "Working", "Completed", "Older", "Read", "nowhere"]) expect(words).toContain(w);
  });

  it("shows no English UI text in the list, its groups, chips and rows' names, in any mode", async () => {
    await setLocale("fa");
    const html = sessionsView("adhd") + sessionsView("focus");
    expect(latinWords(html)).toEqual([]);
    for (const w of ["نشست‌ها", "نیاز به ورودی", "در حال کار", "انجام‌شده", "قدیمی‌تر: ۱", "روی این مک خوانده شد"]) expect(html).toContain(w);
    // The accounts read, each isolated, in a Persian list.
    expect(WORDS.accounts(["cc-mg", "cc-hg"])).toBe(`از ${FSI}${FSI}cc-mg${PDI} و ${FSI}cc-hg${PDI}${PDI} روی این مک خوانده شد و به هیچ‌جا فرستاده نشد.`);
    // A Focus chip's tooltip, with and without its count.
    expect(bare(WORDS.open("در حال کار", 1))).toBe("باز کردن «در حال کار» در همین‌جا (۱)");
    expect(bare(WORDS.open("در حال کار", null))).toBe("باز کردن «در حال کار» در همین‌جا");
  });

  it("counts each group for 0, 1 and many", async () => {
    const g = (counts: [number, number, number]) =>
      groupSessions(
        [
          ...Array.from({ length: counts[0] }, () => row({ group: "needs-input" })),
          ...Array.from({ length: counts[1] }, () => row({ group: "working" })),
          ...Array.from({ length: counts[2] }, () => row({ group: "completed" })),
        ],
        NOW,
      );
    expect(headerCount(g([0, 1, 3]))).toBe("0 need input · 1 working · 3 completed");
    expect(headerCount(g([1, 3, 0]))).toBe("1 need input · 3 working · 0 completed");
    await setLocale("fa");
    expect(headerCount(g([0, 1, 3]))).toBe("۰ نیاز به ورودی · ۱ در حال کار · ۳ انجام‌شده");
    expect(headerCount(g([1, 3, 0]))).toBe("۱ نیاز به ورودی · ۳ در حال کار · ۰ انجام‌شده");
    expect(WORDS.older(0)).toBe("قدیمی‌تر: ۰");
  });

  it("names a row in one Persian sentence, its state in words, the name and message as written", async () => {
    await setLocale("fa");
    const r = row({ name: "Asghar_6", group: "completed", message: "کار تمام شد" });
    expect(rowLabel(r, NOW, "Muster")).toMatch(/^Asghar_6، Muster، انجام‌شده، .+، کار تمام شد$/);
  });
});

describe("the recap in Persian", () => {
  it("draws English in English: the check sees the recap's words", () => {
    const words = latinWords(recapAll());
    for (const w of ["Who", "Tasks", "History", "Time", "Tokens", "Progress", "Transcript", "Close", "Recap"]) expect(words).toContain(w);
  });

  it("shows no English UI text in any section, the strip, the transcript or a session not read", async () => {
    await setLocale("fa");
    const html = recapAll();
    expect(latinWords(html)).toEqual([]);
    for (const w of ["چه کسی", "تسک‌هایی که روی آن‌ها کار کرد", "تاریخچه", "زمان", "توکن‌ها", "پیشرفت", "رونوشت", "بستن", "(حدس)", "روی این مک نیست"]) expect(html).toContain(w);
  });

  it("says each history kind in Persian, and one it doesn't know as the core named it", async () => {
    await setLocale("fa");
    expect(["claim", "check-in", "pass", "return", "stage", "takeover", "note", "handoff-from", "handoff-to"].map(historyKind)).toEqual([
      "برداشتن", "گزارش وضعیت", "فرستادن به بعد", "برگرداندن", "مرحله", "در دست گرفتن", "یادداشت", "واگذاری", "واگذاری",
    ]);
    expect(historyKind("odd")).toBe("odd");
  });

  it("draws the chain right to left: the arrow points the way it reads, each name isolated", async () => {
    await setLocale("fa");
    const r = recap();
    expect(chainLine(r.chain)).toBe(`${FSI}Asghar_4${PDI} ← ${FSI}Asghar_5${PDI} ← ${FSI}aaaa0006${PDI}`);
    expect(recapView(r)).toContain('<span class="meta" aria-hidden="true"> ← </span>');
  });

  it("counts the chain for 1 and many, and the sessions it couldn't read", async () => {
    await setLocale("fa");
    const t = bare(recapView(recap()).replace(/<[^>]+>/g, " ").replace(/\s+/g, " "));
    expect(t).toContain("زنجیره‌ی ۳ نشست: ۵ ساعت");
    expect(t).toContain("زنجیره‌ی ۳ نشست: ۱۵۹k از ۱٫۵M (۱۱٪) + ۹۰۰k خواندن از کش، شمرده نشده · ۱ از ۳ روی این مک خوانده نشد");
    expect(t).toContain("۴۰٪");
    expect(tn("board", "recap.chainUnread", { unread: 1, count: 1 })).toBe("۱ از ۱ روی این مک خوانده نشد");
    expect(tn("board", "recap.chainUnread", { unread: 0, count: 2 })).toBe("۰ از ۲ روی این مک خوانده نشد");
  });

  it("puts the core's word for the state beside the group's only when it's another", async () => {
    const who = recap().who;
    const state = (r: SessionRecap) => /class="recap-state[^"]*">([^<]*)</.exec(recapView(r))?.[1];
    expect(state(recap({ who: { ...who, group: "working", word: "working" } }))).toBe("working");
    expect(state(recap({ who: { ...who, group: "completed", word: "stopped" } }))).toBe("completed (stopped)");
    await setLocale("fa");
    expect(state(recap({ who: { ...who, group: "working", word: "working" } }))).toBe("در حال کار");
    expect(bare(state(recap({ who: { ...who, group: "completed", word: "stopped" } })) ?? "")).toBe("انجام‌شده (stopped)");
  });
});

// ── Reaching a session ────────────────────────────────────

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0022", title: "T", kind: "code", priority: "normal", status: "doing", stage: "building", blocked: false, stageHistory: [],
  folder: "doing", path: "doing/0022-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "building", ...over,
});
const at = (s: string) => {
  const [y, mo, d, h, mi] = s.split(/[- :]/).map(Number);
  return new Date(y, mo - 1, d, h, mi).getTime() / 1000;
};
const PING_NOW = at("2026-10-13 10:40");
const quiet = { stage: "building", claimedBy: "wt-a (wt-0030 · session 7b5e8856 · cc-hg)", stageSince: "2026-10-13 09:00", heartbeat: "2026-10-13 10:00", modified: PING_NOW };
const pingAll = () =>
  [
    renderToStaticMarkup(createElement(CardPing, { t: task(quiet), now: PING_NOW })),
    renderToStaticMarkup(createElement(SessionPing, { task: task(quiet), now: PING_NOW, onChanged: noop })),
    renderToStaticMarkup(createElement(MessageCount, { text: "x".repeat(501) })),
  ].join("\n");

describe("reaching a session in Persian", () => {
  it("draws English in English: the check sees the strip's words", () => {
    const words = latinWords(pingAll());
    for (const w of ["Ask", "check-in", "Send", "Message", "reaches", "Writes", "shorten"]) expect(words).toContain(w);
  });

  it("shows no English UI text on the card's button, the pane's strip or the count", async () => {
    await setLocale("fa");
    const html = pingAll();
    expect(latinWords(html)).toEqual([]);
    for (const w of ["درخواست گزارش وضعیت", "ارسال", "پیام به نشستی که روی این تسک کار می‌کند", "حداکثر ۵۰۰ نویسه"]) expect(html).toContain(w);
    // The session's id and the command it's asked to run are isolated, as written.
    expect(html).toContain(`به نشست ${FSI}7b5e8856${PDI} می‌رسد`);
    expect(html).toContain(`${FSI}muster checkin${PDI} را اجرا کند`);
    // The count stays as the core counts it, in its own digits: a count chip.
    expect(html).toContain('<span class="ping-count">0/500</span>');
  });

  it("says why a check-in can't be asked in the same Persian sentence", async () => {
    await setLocale("fa");
    const html = renderToStaticMarkup(createElement(SessionPing, { task: task({ stage: "testing", heldBy: quiet.claimedBy }), now: PING_NOW, onChanged: noop }));
    expect(bare(html)).toMatch(/به‌صورت نقل‌قولی از یادداشت شما\. درخواست گزارش وضعیت: [^<]+\./);
  });
});

// ── The old-pauses offer ──────────────────────────────────

const old = (over: Partial<OldPause> = {}): OldPause => ({
  queue: "drover", id: "0022", key: "dr-22", title: "Pets", stage: "building", reason: "", since: "2026-10-10 14:00", path: "doing/0022-pets.md", ...over,
});
const notice = (list: OldPause[], error = "") => renderToStaticMarkup(createElement(OldPausesNotice, { list, busy: false, error, onFix: noop, onLater: noop }));
const buttons = (html: string) => [...html.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1]);

describe("the old-pauses offer in Persian", () => {
  it("draws English in English: the check sees the offer's words", () => {
    const words = latinWords(notice([old(), old({ id: "0023", key: "dr-23" })]));
    for (const w of ["tasks", "paused", "Make", "pauses", "Not", "now", "Either"]) expect(words).toContain(w);
  });

  it("shows no English UI text, for one task and for several", async () => {
    await setLocale("fa");
    const one = notice([old({ reason: "طراحی اول" })]);
    const two = notice([old(), old({ id: "0023", key: "dr-23", title: "" })]);
    expect(latinWords(one + two)).toEqual([]);
    expect(one).toContain("۱ تسک پیش از وجود قابلیت «توقف» متوقف شده بود");
    expect(two).toContain("۲ تسک پیش از وجود قابلیت «توقف» متوقف شده بودند");
    expect(buttons(one)).toEqual(["تبدیل به توقف", "فعلاً نه"]);
    expect(buttons(two)).toEqual(["تبدیل همه به توقف", "فعلاً نه"]);
    expect(two).toContain("<code>muster check --fix-pauses</code>");
  });

  it("counts for 0, 1 and many", async () => {
    expect([0, 1, 4].map((count) => tn("board", "oldPauses.offer", { count }).split(" paused")[0])).toEqual(["0 tasks were", "1 task was", "4 tasks were"]);
    await setLocale("fa");
    expect([0, 1, 4].map((count) => tn("board", "oldPauses.offer", { count }).split(" پیش")[0])).toEqual(["۰ تسک", "۱ تسک", "۴ تسک"]);
  });

  it("lists each one in Persian, its key, title, since and reason as written", async () => {
    await setLocale("fa");
    expect(bare(oldPauseLine(old()))).toBe("dr-22 · Pets · از 2026-10-10 14:00 به‌عنوان متوقف‌شده مسدود است");
    expect(bare(oldPauseLine(old({ since: "", reason: "طراحی اول", title: " " })))).toBe("dr-22 · (بدون عنوان) · به‌عنوان متوقف‌شده مسدود است · طراحی اول");
    expect(failedLine(["a", "b"])).toBe(`این‌ها به توقف تبدیل نشدند: ${FSI}a؛ b${PDI}. ${FSI}muster check --fix-pauses${PDI} دوباره امتحان می‌کند.`);
    expect(failedLine([])).toBe("");
  });
});

// ── Lessons ───────────────────────────────────────────────

const queues = [{ name: "muster", path: "/q/muster", counts: {}, lessonsWaiting: 2, label: "Muster" }];
const lessonsHtml = (queue: string) => renderToStaticMarkup(createElement(LessonsWaiting, { queues, queue, onChanged: noop }));

describe("Lessons in Persian", () => {
  it("draws English in English: the check sees the block's words", () => {
    const words = latinWords(lessonsHtml("all"));
    for (const w of ["Lessons", "approve", "Kinds", "Loading"]) expect(words).toContain(w);
  });

  it("shows no English UI text in the block, for one project or all", async () => {
    await setLocale("fa");
    const html = lessonsHtml("muster") + lessonsHtml("all");
    expect(latinWords(html)).toEqual([]);
    expect(html).toContain(`aria-label="درس‌هایی برای تأیید در ${FSI}Muster${PDI}"`);
    expect(html).toContain("<h3>درس‌هایی برای تأیید <small");
    expect(html).toContain("در حال بارگذاری…");
  });

  it("says who a lesson is for and where it came from in Persian, its kind and paths as written", async () => {
    await setLocale("fa");
    expect(bare(lessonMeta({ kind: "unbounded-output", for: "builders", areas: ["app/src/", "core/"], from: ["0027 R1", "0031 R2"] }))).toBe(
      "unbounded-output · برای سازنده‌ها · app/src/، core/ · از 0027 R1، 0031 R2",
    );
    expect(lessonMeta({ kind: "style-taste", for: "reviewers", areas: [], from: [] })).toBe("style-taste · برای بازبین‌ها: گزارشش نکنید");
  });

  it("says what a decision did, each part in its own clause", async () => {
    const done = { accepted: ["L1", "L2"], rejected: ["L3"], expired: ["L0"] };
    expect(decidedText(done)).toBe("Accepted L1, L2; rejected L3; L0 expired to stay under the cap of 40.");
    expect(decidedText({ accepted: [], rejected: ["L3"], expired: [] })).toBe("rejected L3.");
    await setLocale("fa");
    expect(bare(decidedText(done))).toBe("L1، L2 پذیرفته شد؛ L3 رد شد؛ L0 برای ماندن زیر سقف ۴۰ منقضی شد.");
  });

  it("counts Accept all for 0, 1 and many", async () => {
    expect([0, 1, 3].map((count) => tn("board", "lessons.acceptAll", { count }))).toEqual(["Accept all 0", "Accept all 1", "Accept all 3"]);
    await setLocale("fa");
    expect([0, 1, 3].map((count) => tn("board", "lessons.acceptAll", { count }))).toEqual(["پذیرفتن همه (۰)", "پذیرفتن همه (۱)", "پذیرفتن همه (۳)"]);
  });
});
