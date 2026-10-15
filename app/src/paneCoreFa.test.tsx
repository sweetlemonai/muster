// The task pane's own words in Persian (mu-148): its header and status line, the stage track, the
// stage forms (gate, Send back, Block), the write-up editor, the note bar, the pane's messages and
// Undo, and what Release, a move on the track, Send back and Pause say. English reads as before;
// in Persian no English word is left but the task's own data, counts are said for 0, 1 and many,
// and names, keys and the core's words come in as values, isolated.
import { createElement, type FunctionComponent } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";
import EditBody from "./EditBody";
import { DEFAULT_MODE, Mode, type DisplayMode } from "./modes";
import { setLocale } from "./i18n/runtime";
import { fmtWall, logSummary, releaseWords, trackMove, trackSteps, unreachedWords, waitingOnYou } from "./paneData";
import PaneMsg from "./PaneMsg";
import { StatusLine, TitleEdit } from "./PaneHeader";
import { BackFormView, BlockFormView, GateFormView } from "./StageForms";
import StageTrack from "./StageTrack";
import TaskPanel from "./TaskPanel";
import type { Criterion, Question, Task } from "./types";
import { pausedMsg, sentBackMsg } from "./useTaskActions";
import { savedText, unsavedText, unsentText } from "./writeUp";

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0007", title: "جعبه‌ی جستجو", kind: "code", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "tasks/0007-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", search: "", ...over,
});
const crit = (n: number, ticked: boolean): Criterion => ({ n, label: `AC${n}`, text: `AC${n}: x`, ticked, owner: true });
const q = (n: number, escalated = true): Question => ({ n, ticked: false, asker: "wt-a", text: "کدام رنگ؟", escalated, ownerAnswered: false });
function html<P extends object>(el: FunctionComponent<P>, props: P): string {
  return renderToStaticMarkup(createElement(el, props));
}
/** Text and the text attributes a person sees or hears, entities decoded, isolation marks out. */
const shown = (h: string) => {
  const attrs = [...h.matchAll(/\s(?:title|aria-label|placeholder|alt)="([^"]*)"/g)].map((m) => m[1]);
  return [h.replace(/<[^>]+>/g, " "), ...attrs].join(" ").replace(/&#x27;/g, "'").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&").replace(/&quot;/g, '"').replace(/[⁦-⁩]/g, "").replace(/\s+/g, " ").trim();
};
const plain = (s: string) => s.replace(/[⁦-⁩]/g, "");
/** Latin words a person sees, without the fixtures' data (keys, paths, names, code, products) or the
 *  commands typed as they are (`@accept`, `muster start`). */
const DATA = new Set(["de", "demo", "tasks", "md", "Claude", "Code", "Esc", "Markdown", "worktree", "gate", "full", "green", "muster", "next", "wt", "Builder_69", "Tester_69", "AC", "Q1"]);
const latin = (text: string) => [...new Set(plain(text).replace(/@[a-z]+|\bmuster [a-z]+/g, " ").match(/[A-Za-z][A-Za-z_'’0-9]*/g) ?? [])].filter((w) => w.length > 1 && !DATA.has(w));

afterEach(async () => {
  await setLocale("en");
});

describe("the pane's facts in words", () => {
  it("read as before in English", () => {
    expect(waitingOnYou(task({ stage: "your-check", ownerCriteria: [crit(1, true), crit(2, false)] }))).toEqual(["1 of 2 criteria to check"]);
    expect(waitingOnYou(task({ questions: [q(1)] }))).toEqual(["1 question for you"]);
    expect(waitingOnYou(task({ questions: [q(1), q(2)], sessionState: "needs-input" }))).toEqual(["2 questions for you", "input in Claude Code"]);
    expect(fmtWall(0.5)).toBe("under a minute");
    expect(fmtWall(45)).toBe("45 min");
    expect(fmtWall(90)).toBe("1.5 h");
    expect(fmtWall(1530)).toBe("1 d 1.5 h");
    expect(fmtWall(2880)).toBe("2 d");
    expect(logSummary("- a\n- b\n", 1)).toBe("2 log lines · 1 check-in");
    expect(logSummary("- a\n", 0)).toBe("1 log line");
    expect(logSummary("", 2)).toBe("2 check-ins");
    expect(releaseWords("shaping").title).toBe("Unheld, still in Shaping, with the claim released and logged as reclaimed");
    expect(unreachedWords("testing").ask).toBe("Release {{who}}'s hold? It stays in Testing for the next tester.");
    expect(trackMove(task({ stage: "building", claimedBy: "drover/main" }), "ready", {}).done).toBe("Put in Ready. drover/main's claim on Building was released.");
    expect(trackMove(task({ stage: "ready" }), "building", {}).done).toBe("Put in Building.");
    expect(trackSteps("building", true).map((s) => s.label)).toEqual(["To do", "Doing", "Done"]);
  });

  it("are Persian in fa, counts in Persian digits for 0, 1 and many", async () => {
    await setLocale("fa");
    expect(waitingOnYou(task({ stage: "your-check", ownerCriteria: [crit(1, true), crit(2, false)] })).map(plain)).toEqual(["۱ از ۲ معیار برای بررسی"]);
    expect(waitingOnYou(task({ stage: "your-check", ownerCriteria: [crit(1, true)] }))).toEqual(["پذیرفتن یا برگرداندن"]);
    for (const [n, d] of [[1, "۱"], [3, "۳"]] as const) {
      expect(waitingOnYou(task({ questions: Array.from({ length: n }, (_, i) => q(i + 1)) }))).toEqual([`${d} پرسش برای شما`]);
    }
    expect(waitingOnYou(task({ sessionState: "needs-input" }))).toEqual(["ورودی در Claude Code"]);
    expect(fmtWall(0.5)).toBe("کمتر از یک دقیقه");
    expect(fmtWall(45)).toBe("۴۵ دقیقه");
    expect(plain(fmtWall(1530))).toBe("۱ روز و ۱٫۵ ساعت");
    expect(logSummary("", 0)).toBeUndefined();
    expect(logSummary("- a\n- b\n", 1)).toBe("۲ خط لاگ · ۱ گزارش وضعیت");
    expect(releaseWords("building").done).toBe("رها شد: دوباره در «آماده».");
    expect(unreachedWords("building").ask).toBe("تسک برداشته‌شده‌ی {{who}} رها شود؟ به «آماده» برمی‌گردد؛ worktree و شاخه‌اش می‌مانند.");
    expect(plain(trackMove(task({ stage: "building", claimedBy: "drover/main" }), "ready", {}).done)).toBe("در «آماده» گذاشته شد. تسک برداشته‌شده‌ی drover/main در «در حال ساخت» رها شد.");
    expect(trackSteps("building", true).map((s) => s.label)).toEqual(["برای انجام", "در حال انجام", "انجام شد"]);
    expect(trackSteps("ready", false).map((s) => s.label)[0]).toBe("بک‌لاگ");
  });
});

describe("what the pane says after a change", () => {
  const r = (to: string, finding = "", told = "", notTold = "") => ({ to, finding, told, notTold, path: "" });
  it("reads as before in English", () => {
    expect(sentBackMsg(r("building", "", "", "no session"))).toBe("Sent back to Building. Its session can't be told (no session); muster next shows it first.");
    expect(pausedMsg({ path: "", session: "7b5e8856", unreached: "" }, true)).toBe("Paused: it's in the Paused lane with its stage and claim, and nobody works it until you resume it. Session 7b5e8856 is asked to stop at a safe point after its next tool call.");
    expect(pausedMsg({ path: "", session: "", unreached: "no session" }, false)).toBe("Resumed: it's back in its column and carries on as it was. Its session can't be told: no session.");
    expect(savedText(null)).toBe("No change: the text is the same as the file's.");
    expect(unsavedText()).toBe("Unsaved changes: Save, or press Esc twice to discard");
    expect(unsentText()).toBe("Unsent comment: send it (⌘↩), or press Esc twice to throw it away");
  });

  it("is Persian in fa, the session id and the finding as values", async () => {
    await setLocale("fa");
    expect(plain(sentBackMsg(r("building", "R3", "7b5e8856")))).toBe("به سازنده‌اش برگشت: اول صف، با ایراد به‌عنوان یافته‌ی مالک R3 برای رفع یا اعتراض. به نشست 7b5e8856 خبر داده شد.");
    expect(plain(sentBackMsg(r("testing")))).toBe("به «در حال تست» برگردانده شد.");
    expect(plain(pausedMsg({ path: "", session: "", unreached: "" }, true))).toBe("متوقف شد: با مرحله و دارنده‌اش در لین «متوقف شده» است و تا ادامه ندهید کسی روی آن کار نمی‌کند.");
    expect(savedText("x.md")).toBe("ذخیره شد. ویرایش در لاگ است.");
    expect(unsentText()).toBe("دیدگاه ارسال‌نشده: آن را بفرستید (⁦⌘↩⁩)، یا برای دور ریختن دو بار Esc بزنید");
    const undo = html(PaneMsg, { msg: { ok: true, text: "x" }, undo: { back: "ready", to: "building" }, busy: false, onUndo: () => {} });
    expect(shown(undo)).toContain("واگرد: بازگشت به «آماده»");
  });
});

describe("the status line and header", () => {
  const line = (over: Partial<Task>, now = 0) => shown(html(StatusLine, { task: task(over), now }));
  it("reads as before in English", () => {
    expect(line({ stage: "building", claimedBy: "owner" })).toBe("Building LIVE on it: you code agent nothing waiting on you");
    expect(line({ stage: "your-check" })).toBe("Your check LIVE Your turn owner Waiting on you: accept or send back");
  });

  it("is Persian in fa for an agent's task, the owner's own task and Your check", async () => {
    await setLocale("fa");
    const building = line({ stage: "building", claimedBy: "Builder_69 (main · session 1f685b88 · cc-hg)" }, 400);
    expect(building).toContain("۶ دقیقه ساکت ·");
    expect(building).toContain("در دست Builder_69");
    expect(building).toContain("چیزی منتظر شما نیست");
    expect(line({ stage: "building", forWho: "human", claimedBy: "owner" })).toContain("تسک خود شما");
    expect(line({ stage: "building", claimedBy: "owner" })).toContain("زنده");
    expect(line({ stage: "your-check" })).toContain("نوبت شما");
    expect(line({ stage: "your-check" })).toContain("منتظر شما: پذیرفتن یا برگرداندن");
    expect(line({ stage: "mystery" })).toContain("mystery (ناشناخته)");
    for (const s of ["building", "your-check", "ready", "done"]) {
      expect(latin(line({ stage: s, claimedBy: s === "building" ? "Builder_69" : "" }, 400)), s).toEqual([]);
    }
  });

  it("isolates the holder's name in its own <bdi> in fa", async () => {
    await setLocale("fa");
    const raw = html(StatusLine, { task: task({ stage: "building", claimedBy: "Builder_69 (main · session 1f685b88 · cc-hg)" }), now: 400 });
    expect(raw).toContain("در دست <bdi>Builder_69 (main · session 1f685b88 · cc-hg)</bdi>");
  });

  it("names the title box in fa", async () => {
    await setLocale("fa");
    const h = html(TitleEdit, { value: "x", onChange: () => {}, onSave: () => {}, onCancel: () => {} });
    expect(h).toContain('aria-label="عنوان"');
    expect(shown(h)).toContain("ذخیره لغو");
  });
});

describe("the stage track", () => {
  const track = (stage: string, human = false, paused = "") => html(StageTrack, { stage, busy: false, onPick: () => {}, releases: (to: string) => (to === "ready" ? "Builder_69" : ""), human, paused });
  it("reads as before in English", () => {
    const h = track("building");
    expect(h).toContain('title="It&#x27;s in Building"');
    expect(h).toContain('title="Put it in Ready (releases Builder_69&#x27;s claim)"');
    expect(shown(h)).toContain("click any stage to move it there");
  });
  it("is Persian in fa: labels, titles and the hint", async () => {
    await setLocale("fa");
    const h = track("building");
    expect(shown(h)).toContain("روی هر مرحله کلیک کنید تا به آنجا برود");
    expect(shown(h)).toContain("اکنون در «در حال ساخت»");
    expect(shown(h)).toContain("گذاشتن در «آماده» (تسک برداشته‌شده‌ی Builder_69 رها می‌شود)");
    expect(shown(track("ready", false, "2026-10-14 13:00"))).toContain("متوقف شده: برای جابه‌جا کردن، اول ادامه دهید");
    expect(shown(track("building", true))).toContain("برای انجام");
    expect(latin(shown(track("building")))).toEqual([]);
  });
});

describe("the stage forms' labels in fa", () => {
  const gate = () => html(GateFormView, { gate: "", summary: "", needsSummary: true, busy: false, onGate: () => {}, onSummary: () => {}, onSubmit: () => {}, onCancel: () => {} });
  const back = (stage: string) => html(BackFormView, { stage, human: false, choice: "", why: "", busy: false, onChoice: () => {}, onWhy: () => {}, onSubmit: () => {}, onCancel: () => {} });
  const block = (kind: "tasks" | "question" | null, questions: Question[] = []) =>
    html(BlockFormView, { kind, detail: "", questions, busy: false, onKind: () => {}, onDetail: () => {}, onSubmit: () => {}, onCancel: () => {} });
  it("the gate form: its two inputs labelled, Pass to Testing and Cancel", async () => {
    await setLocale("fa");
    const h = gate();
    expect(h).toContain('aria-label="خط نتیجه‌ی دروازه"');
    expect(h).toContain('placeholder="آنچه ساخته شد، در یک خط ساده برای بررسی مالک"');
    expect(h).toContain('aria-label="آنچه ساخته شد، در یک خط"');
    expect(shown(h)).toContain("فرستادن به «در حال تست»");
    expect(shown(h)).toContain("لغو");
    expect(latin(shown(h))).toEqual([]);
  });
  it("Send back: to the builder from Your check, why from anywhere else", async () => {
    await setLocale("fa");
    const yc = back("your-check");
    expect(shown(yc)).toContain("بازگشت به");
    expect(yc).toContain('aria-label="چه ایرادی دارد"');
    expect(shown(yc)).toContain("برگرداندن به سازنده");
    const t = back("testing");
    expect(t).toContain('placeholder="چرا: چه ایرادی دارد یا چه کم است"');
    expect(t).toContain('aria-label="چرا برمی‌گردد"');
    for (const h of [yc, t]) expect(latin(shown(h))).toEqual([]);
  });
  it("Block: its chips, what to pick, the depends note and no open question", async () => {
    await setLocale("fa");
    expect(shown(block(null))).toContain("دلیل مسدود شدن را انتخاب کنید، سپس بگویید منتظر چیست.");
    expect(block(null)).toContain('aria-label="مسدود به‌خاطر"');
    expect(shown(block("tasks"))).toContain("به وابستگی‌هایش اضافه می‌شوند");
    expect(shown(block("question"))).toContain("این تسک پرسش بازی ندارد");
    expect(block("question", [q(1)])).toContain('aria-label="پرسش بازی که منتظر آن است"');
    expect(shown(block(null))).toContain("مسدود کردن");
    for (const h of [block(null), block("tasks"), block("question")]) expect(latin(shown(h))).toEqual([]);
  });
});

describe("the write-up editor", () => {
  it("is Persian in fa, its shortcut isolated", async () => {
    await setLocale("fa");
    const h = html(EditBody, { task: task({}), opened: "متن", onClose: () => {}, onSaved: () => {} });
    expect(h).toContain('aria-label="متن تسک، به Markdown"');
    expect(shown(h)).toContain("در حال ویرایش متن تسک · ⌘S ذخیره می‌کند · Esc لغو می‌کند");
    expect(h).toContain("⁦⌘S⁩");
    expect(latin(shown(h))).toEqual([]);
  });
});

describe("the whole pane", () => {
  const pane = (over: Partial<Task>, mode: DisplayMode = DEFAULT_MODE) =>
    renderToStaticMarkup(createElement(Mode.Provider, { value: mode }, createElement(TaskPanel, { task: task(over), now: 400, onClose: () => {}, onChanged: () => {} })));
  it("leaves no English word of its own in fa: header, track, note bar, body bar, folds", async () => {
    expect(shown(pane({ stage: "ready" }))).toContain("Add a note to the log… (or paste / drop files here)");
    await setLocale("fa");
    const h = pane({ stage: "ready", questions: [q(1)], checkins: [{ at: "2026-10-14 10:40", who: "Builder_69", pct: 40, text: "نیمه‌راه" }] });
    expect(h).toContain('aria-label="تسک ⁨0007⁩"');
    expect(shown(h)).toContain("پرسش‌های باز");
    expect(shown(h)).toContain("افزودن به لاگ");
    expect(shown(h)).toContain("بدون کامیت · tasks/0007-t.md");
    expect(shown(h)).toContain("قالب‌بندی‌شده");
    expect(shown(h)).toContain("۶ دقیقه پیش تغییر کرد");
    expect(latin(shown(h))).toEqual([]);
    // Standard folds the sections to their headers, each with a word on what's in it.
    const folded = pane({ stage: "ready", questions: [q(1)], checkins: [{ at: "2026-10-14 10:40", who: "Builder_69", pct: 40, text: "نیمه‌راه" }] }, "standard");
    expect(folded).toContain('class="fold-head"');
    expect(shown(folded)).toContain("لاگ و گزارش‌های وضعیت");
    expect(latin(shown(folded))).toEqual([]);
  });
});
