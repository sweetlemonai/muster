// The card and its lane in Persian (mu-147): the card, its holder ribbon, the Paused lane and
// band, Pause and Resume, and a column's +N above/below say their own words in the language
// shown, the screen-reader sentence of the ribbon is one whole sentence per case (AC4), and a
// task's own text (title, tags, holder name) shows as written, isolated. English is checked the
// same way, so the checks see the words.
import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";
import { NoteCard } from "./Backlog";
import Card from "./Card";
import { LiveDot } from "./CardLook";
import { MoreCount } from "./ColumnCards";
import { isolate, tn, unmarked } from "./i18n";
import en from "./i18n/locales/en/board.json";
import fa from "./i18n/locales/fa/board.json";
import { setLocale } from "./i18n/runtime";
import { Mode, type DisplayMode } from "./modes";
import { CardPause, cardPauseNote, PanePause, PausedChip, pausedText, wasText } from "./Pause";
import { PausedBand, PausedColumn, PAUSED_LANE } from "./PausedLane";
import { sessionWords } from "./sessionState";
import { Machine } from "./settingsData";
import css from "./styles.css?raw";
import type { Note, Task } from "./types";

const at = (s: string) => {
  const [y, mo, d, h, mi] = s.split(/[- :]/).map(Number);
  return new Date(y, mo - 1, d, h, mi).getTime() / 1000;
};
const NOW = at("2026-10-14 19:00");
const ASGHAR = "Asghar_7 (wt-0097 · session a82f2534 · cc-mg)";

const task = (over: Partial<Task>): Task => ({
  queue: "muster", id: "0097", prefix: "mu", title: "Shaping is a claim", kind: "bug", priority: "high", status: "doing", stage: "building", blocked: false, stageHistory: [],
  folder: "doing", path: "doing/0097-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "2026-10-14 18:55", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: NOW - 600,
  level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "building", ...over,
});
const note = (over: Partial<Note>): Note => ({ queue: "muster", file: "2026-10-14-1600-x.md", title: "Backlog notes show who", text: "Backlog notes show who", truncated: false, added: 0, heldBy: "", heldSince: "", heartbeat: "", ...over });
const criterion = (n: number) => ({ n, label: `AC${n}`, text: `AC${n}: the owner checks part ${n}`, ticked: false, owner: true });

const inMode = (mode: DisplayMode, el: ReactElement) => renderToStaticMarkup(createElement(Mode.Provider, { value: mode }, el));
const card = (t: Task, mode: DisplayMode = "adhd") => inMode(mode, createElement(Card, { t, now: NOW, multi: false, parent: "", onOpen: () => {}, active: false, words: [] }));
const noteCard = (n: Note) => renderToStaticMarkup(createElement(NoteCard, { n, now: NOW, multi: false, active: false, onOpen: () => {}, words: [] }));
const stub = (t: Task) => createElement("i", { className: "card-stub" }, t.id);

/** What's read out: the text without tags and without aria-hidden parts (one level of nesting), the
 *  isolation marks taken out. */
const spoken = (html: string) =>
  unmarked(html.replace(/<(\w+)[^>]*aria-hidden="true"[^>]*>(?:[^<]|<(\w+)[^>]*>[^<]*<\/\2>)*<\/\1>/g, "").replace(/<[^>]+>/g, "").replace(/\s+/g, " ").trim());
const RIBBON = /<span class="holder-ribbon.*?(?=<span class="card-band"|<span class="title")/;
const ribbon = (html: string) => RIBBON.exec(html)?.[0] ?? "";
/** Text and the text attributes a person sees or hears, entities decoded, marks out. */
const shown = (html: string) => {
  const attrs = [...html.matchAll(/\s(?:title|aria-label|placeholder|alt)="([^"]*)"/g)].map((m) => m[1]);
  return unmarked([html.replace(/<[^>]+>/g, " "), ...attrs].join(" ").replace(/&#x27;/g, "'").replace(/&quot;/g, '"').replace(/&amp;/g, "&").replace(/\s+/g, " "));
};

/** Every card and lane state this area draws, in the language loaded now. */
function everything(): string {
  const held = { claimedBy: ASGHAR, stageSince: "2026-10-14 17:00" };
  const quiet = { ...held, heartbeat: "2026-10-14 06:00", stageSince: "2026-10-14 05:00" };
  const paused = { ...held, paused: "2026-10-14 17:00", pausedWhy: "the design changes first" };
  const parts = [
    card(task({})),
    card(task({ ...held, sessionState: "working", live: true })),
    card(task({ ...held, sessionState: "needs-input", waitingForOwner: true })),
    card(task({ ...held, sessionState: "ended" })),
    card(task(quiet)),
    card(task({ ...quiet, sessionState: "needs-input", waitingForOwner: true })),
    card(task({ stage: "building", claimedBy: "owner" })),
    card(task({ stage: "ready", forWho: "human", title: "" })),
    card(task({ stage: "ready", live: true, openQuestions: 1, attachments: ["a.png"], tags: ["ui"], estTime: "2h" })),
    card(task({ stage: "ready", openQuestions: 3, attachments: ["a.png", "b.png"], waitingForOwner: true })),
    card(task({ stage: "testing", claimedBy: ASGHAR, heldBy: "" })),
    card(task({ stage: "testing", heldBy: ASGHAR, claimedBy: ASGHAR })),
    card(task({ stage: "your-check", waitingForOwner: true, claimedBy: "owner", ownerCriteria: [criterion(1), criterion(2), criterion(3)] })),
    card(task({ stage: "your-check", waitingForOwner: true, claimedBy: "owner", ownerCriteria: [criterion(1), criterion(2)] })),
    card(task({ stage: "done", doneAt: "2026-10-14 18:56" })),
    card(task(paused)),
    card(task({ ...paused, paused: "someday", pausedWhy: "" })),
    noteCard(note({ heldBy: "shapour (main · session 1137d98f · cc-mg)", heldSince: "2026-10-14 18:40", heartbeat: "2026-10-14 06:00" })),
    noteCard(note({ heldBy: "owner", heldSince: "2026-10-14 18:40", heartbeat: "2026-10-14 06:00" })),
    noteCard(note({ heldBy: "shapour", heartbeat: "2026-10-14 18:50" })),
    noteCard(note({ heldBy: "owner", heartbeat: "2026-10-14 18:50" })),
    renderToStaticMarkup(createElement(PausedChip, { t: task(paused), now: NOW })),
    renderToStaticMarkup(createElement(PausedChip, { t: task({ ...paused, paused: "someday" }), now: NOW })),
    `<p>${cardPauseNote({ path: "", session: "", unreached: "no session" })}</p>`,
    renderToStaticMarkup(createElement(PausedChip, { t: task({ ...paused, pausedWhy: "" }), now: NOW })),
    renderToStaticMarkup(createElement(CardPause, { t: task(paused) })),
    renderToStaticMarkup(createElement(CardPause, { t: task({}) })),
    renderToStaticMarkup(createElement(PanePause, { task: task(paused), busy: false, onPause: async () => true, onResume: () => {} })),
    renderToStaticMarkup(createElement(PanePause, { task: task({}), busy: false, onPause: async () => true, onResume: () => {} })),
    renderToStaticMarkup(createElement(PausedColumn, { tasks: [task(paused)], onHide: () => {}, renderCard: stub })),
    renderToStaticMarkup(createElement(PausedColumn, { tasks: [], onHide: () => {}, renderCard: stub })),
    renderToStaticMarkup(createElement(PausedBand, { tasks: [], renderCard: stub })),
    renderToStaticMarkup(createElement(PausedBand, { tasks: [task(paused)], renderCard: stub, fold: { of: () => "folded" as const, toggle: () => {} } })),
    renderToStaticMarkup(createElement(PausedBand, { tasks: [task(paused), task(paused)], renderCard: stub, fold: { of: () => "opened" as const, toggle: () => {} } })),
    renderToStaticMarkup(createElement(MoreCount, { side: "above", n: 1, onPage: () => {} })),
    renderToStaticMarkup(createElement(MoreCount, { side: "below", n: 12, onPage: () => {} })),
    renderToStaticMarkup(createElement(Machine.Provider, { value: { leaseMinutes: 720, liveMinutes: 1 } }, createElement(LiveDot, { t: { live: true } }))),
    renderToStaticMarkup(createElement(Machine.Provider, { value: { leaseMinutes: 720, liveMinutes: 5 } }, createElement(LiveDot, { t: { live: true } }))),
  ];
  return parts.join("\n");
}

type Tree = { [k: string]: string | Tree };
const leaves = (m: Tree): string[] => Object.values(m).flatMap((v) => (typeof v === "string" ? [v] : leaves(v)));
const MINE = ["card", "ribbon", "lane", "pause"] as const;
/** The English words of this area's strings, as phrases between their placeholders and tags. */
const englishPhrases = () =>
  [...new Set(MINE.flatMap((s) => leaves(en[s] as Tree)).flatMap((v) => v.split(/\{\{[^}]+\}\}|<\/?\w+\s*\/?>/)).map((p) => p.trim().replace(/^[\s·:,.;+()]+|[\s·:,.;+()]+$/g, "")).filter((p) => /[A-Za-z]{2,}/.test(p)))];
/** `phrase` in `text` as whole words. */
const has = (text: string, phrase: string) => new RegExp(`(^|[^A-Za-z'])${phrase.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}($|[^A-Za-z'])`).test(text);

afterEach(async () => {
  await setLocale("en");
});

describe("the card's area in English", () => {
  it("draws every phrase this area has, so the Persian check below sees text", () => {
    // Sent back from Your check (mu-101): its reason's words are lib.ts's sendBackLine's too.
    const sentBack = card(task({ sentBack: true, sentBackBy: "owner", sentBackWhy: "the icon is wrong" }));
    const text = shown(`${everything()}\n${sentBack}`);
    expect(englishPhrases().filter((p) => !has(text, p))).toEqual([]);
  });
  it("names the type in words, and an unknown type as written", () => {
    expect(card(task({ kind: "tech-debt" }))).toContain('<span class="kind tech-debt">tech-debt</span>');
    expect(card(task({ kind: "epic-ish" }))).toContain('<span class="kind epic-ish">epic-ish</span>');
  });
});

describe("the card's area in Persian", () => {
  it("leaves none of its English words: every phrase of its strings is gone", async () => {
    await setLocale("fa");
    const text = shown(everything());
    expect(englishPhrases().filter((p) => has(text, p))).toEqual([]);
    for (const w of ["در دست", "نوبت شما", "پرسش باز", "فایل", "متوقف شده", "ادامه", "توقف", "زنده", "بالا", "پایین", "اولویت", "باگ", "بالا"]) expect(text, w).toContain(w);
  });
  it("says the type and the priority in Persian, an unknown type as written", async () => {
    await setLocale("fa");
    const html = unmarked(card(task({ kind: "bug", priority: "high" })));
    expect(html).toContain('<span class="kind bug">باگ</span>');
    expect(html).toContain('<span class="pri pri-high" title="اولویت: بالا">بالا</span>');
    expect(card(task({ kind: "epic-ish" }))).toContain('<span class="kind epic-ish">epic-ish</span>');
    // Waiting on the owner: the needs-you badge, in the glossary's words.
    expect(card(task({ stage: "ready", waitingForOwner: true }))).toContain('<span class="ui-badge ui-badge--warn">نیاز به شما</span>');
    // A priority the board reads but has no word for stays as written.
    expect(unmarked(card(task({ priority: "P0" })))).toContain('<span class="pri pri-P0" title="اولویت: P0">P0</span>');
  });
  it("counts 0, 1 and many in its plurals", async () => {
    await setLocale("fa");
    const bottom = (t: Task) => shown(/<span class="card-bottom">.*$/.exec(card(t))?.[0] ?? "");
    // None: no badge at all.
    expect(bottom(task({ stage: "ready" }))).not.toMatch(/پرسش|فایل/);
    expect(bottom(task({ stage: "ready", openQuestions: 1, attachments: ["a"] }))).toContain("۱ پرسش باز");
    expect(bottom(task({ stage: "ready", openQuestions: 1, attachments: ["a"] }))).toContain("۱ فایل");
    expect(bottom(task({ stage: "ready", openQuestions: 12, attachments: ["a", "b", "c"] }))).toContain("۱۲ پرسش باز");
    expect(bottom(task({ stage: "ready", openQuestions: 12, attachments: ["a", "b", "c"] }))).toContain("۳ فایل");
    // The lane's band: 0, 1 and many tasks.
    const band = (n: number) => shown(renderToStaticMarkup(createElement(PausedBand, { tasks: Array.from({ length: n }, (_, i) => task({ id: `00${i}`, paused: "2026-10-14 17:00" })), renderCard: stub })));
    expect(band(0)).toContain("۰ تسک");
    expect(band(1)).toContain("۱ تسک");
    expect(band(4)).toContain("۴ تسک");
    // +N above/below: none is no button; 1 and many keep the sign with the number, isolated left to right.
    const more = (n: number) => renderToStaticMarkup(createElement(MoreCount, { side: "above", n, onPage: () => {} }));
    expect(more(0)).toBe('<div class="col-more col-more-above"></div>');
    expect(more(1)).toContain(">⁦+۱⁩ بالاتر</button>");
    expect(more(23)).toContain(">⁦+۲۳⁩ بالاتر</button>");
    // Your check's next step and how many more: 0, 1 and many.
    const next = (n: number) => /<span class="card-next"[^>]*>([^<]*)<\/span>/.exec(card(task({ stage: "your-check", waitingForOwner: true, claimedBy: "owner", ownerCriteria: Array.from({ length: n + 1 }, (_, i) => criterion(i + 1)) })))?.[1] ?? "";
    expect(next(0)).not.toContain("·");
    expect(unmarked(next(1))).toMatch(/ · ۱ مورد دیگر$/);
    expect(unmarked(next(5))).toMatch(/ · ۵ مورد دیگر$/);
    // The live dot's tooltip: 1 and many minutes.
    const live = (liveMinutes: number) => renderToStaticMarkup(createElement(Machine.Provider, { value: { leaseMinutes: 720, liveMinutes } }, createElement(LiveDot, { t: { live: true } })));
    expect(live(1)).toContain("۱ دقیقه");
    expect(live(90)).toContain("۹۰ دقیقه");
    expect(live(90)).toContain('aria-label="زنده"');
  });
  it("in English, the same plurals: one and other", () => {
    const bottom = (t: Task) => shown(/<span class="card-bottom">.*$/.exec(card(t))?.[0] ?? "");
    expect(bottom(task({ stage: "ready", openQuestions: 1, attachments: ["a"] }))).toMatch(/1 open Q.*1 file\b/);
    expect(bottom(task({ stage: "ready", openQuestions: 2, attachments: ["a", "b"] }))).toMatch(/2 open Q.*2 files/);
    expect(shown(renderToStaticMarkup(createElement(PausedBand, { tasks: [task({ paused: "2026-10-14 17:00" })], renderCard: stub })))).toContain("1 task");
    const live = (liveMinutes: number) => renderToStaticMarkup(createElement(Machine.Provider, { value: { leaseMinutes: 720, liveMinutes } }, createElement(LiveDot, { t: { live: true } })));
    expect(live(1)).toContain("changed in the last 1 minute\"");
    expect(live(5)).toContain("changed in the last 5 minutes\"");
  });
});

describe("the holder ribbon's screen-reader sentence, one per case (AC4)", () => {
  const held = { claimedBy: ASGHAR, stageSince: "2026-10-14 17:00" };
  const cases: [string, Partial<Task>][] = [
    ["held", held],
    ["held, working", { ...held, sessionState: "working" }],
    ["held, waiting for input", { ...held, sessionState: "needs-input", waitingForOwner: true }],
    ["held, ended", { ...held, sessionState: "ended" }],
  ];
  it("reads in English as before", () => {
    expect(spoken(ribbon(card(task(cases[0][1]))))).toBe("held by Asghar_7");
    for (const [name, over] of cases.slice(1)) expect(spoken(ribbon(card(task(over)))), name).toBe(`held by Asghar_7, ${sessionWords(task(over))}`);
  });
  it("reads in Persian as one Persian sentence, the name first after «در دست» and the state after it", async () => {
    await setLocale("fa");
    expect(spoken(ribbon(card(task(cases[0][1]))))).toBe("در دست Asghar_7");
    for (const [name, over] of cases.slice(1)) expect(spoken(ribbon(card(task(over)))), name).toBe(`در دست Asghar_7، ${unmarked(sessionWords(task(over)))}`);
    // The whole sentence is one screen-reader span: the name, the state and the words around them.
    const r = ribbon(card(task(cases[1][1])));
    expect(r.match(/class="sr-only"/g)).toHaveLength(1);
    expect(r).toContain(`<span class="sr-only">در دست ${isolate("Asghar_7")}، `);
    // A note being shaped, and the owner's You ribbon.
    expect(spoken(ribbon(noteCard(note({ heldBy: "shapour (main · session 1137d98f · cc-mg)" }))))).toBe("در حال شکل‌دهی توسط shapour");
    expect(spoken(/<span class="you-ribbon"[^>]*>(?:<span[^>]*>[^<]*<\/span>)*[^<]*<\/span>/.exec(card(task({ claimedBy: "owner" })))?.[0] ?? "")).toBe("در دست شما");
    // The owner shaping a note: one sentence, and one with quiet once the lease ran out.
    const you = /<span class="you-ribbon"[^>]*>(?:<span[^>]*>[^<]*<\/span>)*[^<]*<\/span>/;
    expect(spoken(you.exec(noteCard(note({ heldBy: "owner" })))?.[0] ?? "")).toBe("در حال شکل‌دهی توسط شما");
    expect(spoken(you.exec(noteCard(note({ heldBy: "owner", heldSince: "2026-10-14 06:00", heartbeat: "2026-10-14 06:00" })))?.[0] ?? "")).toBe("در حال شکل‌دهی توسط شما، ساکت");
  });
  it("names the holder whole for a screen reader, the shown name hidden from it, in any script", async () => {
    await setLocale("fa");
    const r = ribbon(card(task({ claimedBy: "قلی_۳۱ (main · session 3c09f6c7 · cc-mg)" })));
    expect(spoken(r)).toBe("در دست قلی_۳۱");
    expect(r).toContain('<span aria-hidden="true" class="holder-name tail-2" dir="auto">');
  });
  it("puts the tooltip in whole sentences, quiet as its own", async () => {
    const quiet = { ...held, heartbeat: "2026-10-14 06:00", stageSince: "2026-10-14 05:00" };
    expect(ribbon(card(task(quiet)))).toContain(`title="Held by ${ASGHAR}. Quiet: no heartbeat for over 12 h, its lease ran out"`);
    await setLocale("fa");
    const title = unmarked(/title="([^"]*)"/.exec(ribbon(card(task(quiet))))?.[1] ?? "");
    expect(title).toBe(`در دست ${ASGHAR}. ساکت: بیش از ۱۲ ساعت بدون ضربان، مهلتش تمام شده است`);
  });
});

describe("a task's own text shows as written, isolated", () => {
  it("isolates the title and each tag, in either language", async () => {
    for (const lang of ["en", "fa"] as const) {
      await setLocale(lang);
      const html = card(task({ stage: "ready", title: "شروع the build", tags: ["ui", "فارسی"] }));
      expect(html, lang).toContain('<span class="title"><bdi>شروع the build</bdi></span>');
      // dir="auto" on the tag itself: its own first letter sets its direction, the # with it.
      expect(html, lang).toContain('<span class="tag" dir="auto">#ui</span>');
      expect(html, lang).toContain('<span class="tag" dir="auto">#فارسی</span>');
    }
  });
  it("keeps the type, priority and age at the line's end in either direction (logical CSS)", () => {
    const rule = (selector: string) => new RegExp(`(?:^|\\n)${selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")} \\{([^}]*)\\}`).exec(css)?.[1] ?? "";
    expect(rule(".pri")).toContain("margin-inline-start: auto;");
    expect(rule(".age")).toContain("margin-inline-start: auto;");
    expect(rule(".card")).toContain("text-align: start;");
    expect(css).not.toMatch(/\n\.(pri|age) \{[^}]*margin-left/);
  });
});

describe("the pause words", () => {
  const paused = { stage: "building", paused: "2026-10-14 17:00", pausedWhy: "the design changes first" };
  it("say where it was, since when and why, in Persian order", async () => {
    await setLocale("fa");
    expect(unmarked(wasText(task(paused), NOW))).toBe("پیش‌تر در حال ساخت · ۲ ساعت پیش متوقف شد · the design changes first");
    expect(unmarked(wasText(task({ ...paused, pausedWhy: "" }), NOW))).toBe("پیش‌تر در حال ساخت · ۲ ساعت پیش متوقف شد");
    expect(unmarked(wasText(task({ ...paused, paused: "someday", pausedWhy: "" }), NOW))).toBe("پیش‌تر در حال ساخت · متوقف‌شده از someday");
    expect(unmarked(pausedText(task(paused), NOW))).toBe("۲ ساعت پیش · the design changes first");
    expect(unmarked(cardPauseNote({ path: "", session: "", unreached: "no session" }))).toBe("نمی‌توان به نشست آن خبر داد: no session");
    expect(PAUSED_LANE.label).toBe("⏸︎ متوقف شده");
  });
  it("follow a switch back to English (no word kept from before)", async () => {
    await setLocale("fa");
    expect(PAUSED_LANE.role).not.toMatch(/[A-Za-z]/);
    await setLocale("en");
    expect(PAUSED_LANE.label).toBe("⏸︎ Paused");
    expect(PAUSED_LANE.role).toBe("Resume puts it back in its column");
    expect(tn("board", "pause.paused")).toBe("Paused");
  });
});

it("keeps this area's Persian keys in English's order", () => {
  type T = { [k: string]: unknown };
  const order = (m: T): string[] => Object.entries(m).flatMap(([k, v]) => (typeof v === "string" ? [k] : order(v as T).map((x) => `${k}.${x}`)));
  for (const s of MINE) expect(order(fa[s] as T).map((k) => k.replace(/_(zero|one|two|few|many|other)$/, "")).filter((k, i, a) => a.indexOf(k) === i), s).toEqual(order(en[s] as T).map((k) => k.replace(/_(one|other)$/, "")).filter((k, i, a) => a.indexOf(k) === i));
});
