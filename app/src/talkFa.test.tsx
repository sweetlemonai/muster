// Talking and checking in Persian (mu-148): the comment thread and its box, an open question's
// answer box and pick-one chips (in the pane and in Waiting for you), the review and vision badges
// on a card, the pane's Review and Vision sections, the Problems list's vision lines and the
// Helper's words. English reads as before; Persian has no English word left but the data (task
// text, ids, commands), which comes in as values, isolated.
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";
import AnswerBox from "./AnswerBox";
import Comments, { commentDone } from "./Comments";
import { helperEmpty, helperSteps, NoClaude } from "./Helper";
import { FSI, PDI, setLocale } from "./i18n/runtime";
import PickOne, { answeredText, sendPick } from "./PickOne";
import { reviewBadge, ReviewBadge, reviewStatus } from "./Review";
import ReviewPanel from "./ReviewPanel";
import type { Question, Task } from "./types";
import { visionBadge, VisionBadge, visionStatus, visionWarnings } from "./Vision";
import VisionPanel from "./VisionPanel";

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0007", title: "T", kind: "code", priority: "normal", status: "doing", stage: "building", blocked: false, stageHistory: [],
  folder: "doing", path: "doing/0007-t.md", claimedBy: "wt-a", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "building", ...over,
});
const LEASE: Question = {
  n: 2, ticked: false, asker: "wt-a", escalated: true, ownerAnswered: false, text: "How long?",
  choices: { options: ["1 h", "Any length"], pick: 0, noAnswer: 1, question: "How long?" },
};
const PORT: Question = { n: 1, ticked: false, asker: "wt-a", text: "Which port?", escalated: true, ownerAnswered: false };

const html = (el: Parameters<typeof renderToStaticMarkup>[0]) => renderToStaticMarkup(el);
/** Text and the text attributes a person sees or hears, entities decoded, isolation marks out. */
const shown = (out: string) => {
  const attrs = [...out.matchAll(/\s(?:title|aria-label|placeholder|alt)="([^"]*)"/g)].map((m) => m[1]);
  return [out.replace(/<[^>]+>/g, " "), ...attrs].join(" ").replace(/&#x27;/g, "'").replace(/&amp;/g, "&").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/[⁦-⁩]/g, "").replace(/\s+/g, " ");
};
const iso = (s: string) => `${FSI}${s}${PDI}`;
/** The fixtures' data and the commands that stay Latin. */
const DATA = new Set(["How", "long", "Which", "port", "Any", "length", "wt", "demo", "accept", "sendback", "block", "ask", "muster", "review", "start", "fixed", "dispute", "vision", "claude", "Claude", "Code", "Terminal", "hello", "there", "V1", "V3", "Two", "records", "of", "waits", "Blocked", "says", "why", "Later", "drover", "Human", "board", "is", "in", "Shaping", "with", "no", "check", "yet", "owner", "sonnet", "hg", "mg", "Model", "added", "sent", "back", "to", "Building"]);
const latin = (text: string) => [...new Set(text.match(/[A-Za-z][A-Za-z'’]*/g) ?? [])].filter((w) => w.length > 1 && !DATA.has(w));

afterEach(async () => {
  await setLocale("en");
});

describe("the comment thread", () => {
  const thread = (t: Task) => html(createElement(Comments, { task: t, onChanged: () => {} }));
  const one = task({ comments: [{ at: "2026-10-14 11:00", who: "owner", text: "hello there" }] });

  it("reads as before in English", () => {
    const empty = shown(thread(task({})));
    expect(empty).toContain("Comments 0 No comments yet.");
    expect(empty).toContain("Start with @accept, @sendback <why>, @block <why> or @ask <question> to act.");
    expect(empty).toContain("Add a comment (⌘↩ to send)");
    expect(empty).toContain("Your comment");
    expect(thread(task({}))).toContain('aria-label="Comments"');
    expect(commentDone({ path: "p", did: "", unsaved: "" })).toBe("Comment added.");
  });

  it("is Persian in fa, the @ commands kept as they are", async () => {
    await setLocale("fa");
    const empty = shown(thread(task({})));
    expect(empty).toContain("دیدگاه‌ها ۰ هنوز دیدگاهی نیست.");
    // An LRM before each @ command keeps the @ on its left in right-to-left text.
    expect(empty).toContain("برای انجام کار، با \u200e@accept، \u200e@sendback <دلیل>، \u200e@block <دلیل> یا \u200e@ask <پرسش> شروع کنید.");
    expect(thread(task({}))).toContain(`placeholder="افزودن دیدگاه (${"⁦"}⌘↩${PDI} برای ارسال)"`);
    expect(latin(shown(thread(one)))).toEqual([]);
    expect(commentDone({ path: "p", did: "", unsaved: "" })).toBe("دیدگاه افزوده شد.");
    expect(commentDone({ path: "p", did: "sent back to Building", unsaved: "" })).toBe(`دیدگاه افزوده شد: ${iso("sent back to Building")}.`);
    expect(commentDone({ path: "p", did: "blocked", unsaved: "too big" })).toBe(`انجام شد: ${iso("blocked")}. دیدگاه ذخیره نشد (${iso("too big")})، پس در رشته‌ی دیدگاه‌ها نیست؛ دوباره ارسالش نکنید.`);
  });
});

describe("an open question's answer box and pick-one chips", () => {
  const box = (q: Question) => html(createElement(AnswerBox, { task: task({}), q, onChanged: () => {} }));
  const picks = () => html(createElement(PickOne, { task: task({}), q: LEASE, choices: LEASE.choices!, extra: "x", onChanged: () => {} }));

  it("reads as before in English", async () => {
    expect(box(PORT)).toContain('placeholder="Your answer (⌘↩ to send)"');
    expect(shown(box(PORT))).toContain("for you");
    expect(box(LEASE)).toContain('placeholder="Something else, or words to add to a pick (⌘↩ sends them as your answer)"');
    const p = picks();
    expect(p).toContain('aria-label="Options for Q2"');
    expect(p).toContain('title="Answer Q2 with (a), followed by the words in the box"');
    expect(shown(p)).toContain("going with (b) unless you answer");
    expect(shown(p)).toContain("pick");
    expect(answeredText("s")).toBe("Answered. The asking agent gets it after its next step.");
    expect(await sendPick(async () => null, task({}), { ...LEASE, n: null }, 0, "", new Date())).toEqual({ ok: false, text: "This question has no number, so it can't be answered here." });
  });

  it("is Persian in fa, every word but the question and its options", async () => {
    await setLocale("fa");
    expect(box(PORT)).toContain(`placeholder="پاسخ شما (${"⁦"}⌘↩${PDI} برای ارسال)"`);
    expect(latin(shown(box(PORT)))).toEqual([]);
    expect(latin(shown(box(LEASE)))).toEqual([]);
    const p = picks();
    expect(p).toContain(`aria-label="گزینه‌های ${iso("Q2")}"`);
    expect(shown(p)).toContain("اگر پاسخ ندهید، (b) انتخاب می‌شود");
    expect(latin(shown(p)).filter((w) => w !== "Q2")).toEqual([]);
    expect(answeredText("s")).toBe("پاسخ داده شد. ایجنتی که پرسیده پس از گام بعدی‌اش آن را دریافت می‌کند.");
    expect(answeredText(null)).toBe("پاسخ داده شد و در تسک نوشته شد. هیچ نشستی به‌عنوان پرسنده ثبت نشده، پس همان‌جا منتظر می‌ماند.");
    expect((await sendPick(async () => null, task({}), { ...LEASE, n: null }, 0, "", new Date())).text).toBe("این پرسش شماره ندارد، پس اینجا نمی‌توان به آن پاسخ داد.");
  });
});

describe("the review and vision badges on a card", () => {
  const r = (over: Partial<Task>) => reviewBadge(task({ stage: "testing", branch: "task/x", ...over }));

  it("reads as before in English", () => {
    expect(r({ reviewRound: 1, reviewOpen: 2, reviewAnswered: 0 })).toBe("review 1/3 · 2 to fix");
    expect(r({ reviewRound: 0, reviewOpen: 1 })).toBe("1 to fix · not reviewed yet");
    expect(visionBadge({ visionOpen: 1 })).toBe("⚠ 1 vision");
  });

  it("is Persian in fa, for 0, 1 and many", async () => {
    await setLocale("fa");
    expect(r({})).toBe("هنوز بازبینی نشده");
    expect(r({ reviewRound: 0, reviewOpen: 1, reviewAnswered: 0 })).toBe("۱ برای رفع · هنوز بازبینی نشده");
    expect(r({ reviewRound: 0, reviewOpen: 0, reviewAnswered: 3 })).toBe("۳ برای بررسی دوباره · هنوز بازبینی نشده");
    expect(r({ reviewRound: 1, reviewOpen: 2, reviewAnswered: 0 })).toBe("بازبینی ۱/۳ · ۲ برای رفع");
    expect(r({ reviewRound: 2, reviewOpen: 0, reviewAnswered: 1 })).toBe("بازبینی ۲/۳ · ۱ برای بررسی دوباره");
    expect(r({ reviewRound: 2, reviewOpen: 0, reviewAnswered: 0 })).toBe("بازبینی بدون یافته · دور ۲");
    expect(visionBadge({ visionOpen: 1 })).toBe("⚠ ۱ چشم‌انداز");
    expect(visionBadge({ visionOpen: 12 })).toBe("⚠ ۱۲ چشم‌انداز");
    const badges = html(createElement(ReviewBadge, { t: task({ stage: "testing", reviewRound: 1, reviewOpen: 1, reviewAnswered: 0 }) })) + html(createElement(ReviewBadge, { t: task({ stage: "testing", reviewRound: 1, reviewOpen: 0, reviewAnswered: 0 }) })) + html(createElement(VisionBadge, { t: { visionOpen: 2 } }));
    expect(latin(shown(badges))).toEqual([]);
  });
});

describe("the pane's Review and Vision sections", () => {
  const review = (t: Task) => html(createElement(ReviewPanel, { task: t, body: null }));
  const vision = (t: Task) => html(createElement(VisionPanel, { task: t, body: null, onChanged: () => {} }));

  it("reads as before in English", () => {
    expect(reviewStatus(task({ reviewRound: 2, reviewOpen: 1, reviewAnswered: 1 }))).toBe("round 2 of 3: 1 open, 1 answered");
    expect(visionStatus({ visionOpen: 3 })).toBe("3 open conflicts");
    expect(shown(review(task({ branch: "task/x" })))).toContain("Copy review brief");
    expect(shown(vision(task({ visionOpen: 1, visionIds: ["V1"] })))).toContain("Resolve V1");
  });

  it("is Persian in fa", async () => {
    await setLocale("fa");
    expect(reviewStatus(task({}))).toBe("روی شاخه‌ی خودش شروع نشده");
    expect(reviewStatus(task({ branch: "x" }))).toBe("هنوز بازبینی نشده");
    expect(reviewStatus(task({ branch: "x", reviewRound: 0, reviewOpen: 1, reviewAnswered: 0 }))).toBe("هنوز بازبینی نشده: ۱ باز، ۰ پاسخ‌داده");
    expect(reviewStatus(task({ reviewRound: 3, reviewOpen: 0, reviewAnswered: 0 }))).toBe("بدون یافته پس از دور ۳");
    expect(reviewStatus(task({ reviewRound: 2, reviewOpen: 1, reviewAnswered: 1 }))).toBe("دور ۲ از ۳: ۱ باز، ۱ پاسخ‌داده");
    expect(visionStatus({})).toBe("بررسی نشده");
    expect(visionStatus({ visionOpen: 0 })).toBe("بدون تعارض باز");
    expect(visionStatus({ visionOpen: 1 })).toBe("۱ تعارض باز");
    expect(visionStatus({ visionOpen: 3 })).toBe("۳ تعارض باز");
    for (const out of [review(task({})), review(task({ branch: "task/x" })), review(task({ branch: "task/x", reviewRound: 1 })), vision(task({})), vision(task({ visionOpen: 2, visionIds: ["V1", "V3"] }))]) {
      expect(latin(shown(out))).toEqual([]);
    }
    expect(shown(vision(task({ visionOpen: 1, visionIds: ["V1"] })))).toContain("حل V1");
  });
});

describe("the Problems list's vision lines", () => {
  const shaping = task({ stage: "shaping", queue: "muster", id: "0024", title: "Blocked says why" });

  it("reads as before in English", () => {
    expect(visionWarnings([shaping])).toEqual(["muster: 0024 Blocked says why is in Shaping with no vision check yet (muster vision 0024)"]);
  });

  it("is Persian in fa, the project, key, title and command as values", async () => {
    await setLocale("fa");
    expect(visionWarnings([shaping])).toEqual([`${iso("muster")}: ${iso("0024")} ${iso("Blocked says why")} در «شکل‌دهی» است و هنوز بررسی چشم‌انداز ندارد (${iso("muster vision 0024")})`]);
  });
});

describe("the Helper's words", () => {
  it("reads as before in English", () => {
    expect(helperSteps(1, false)).toBe("1 step");
    expect(helperSteps(3, true)).toBe("3 steps · filed tasks");
    expect(helperEmpty()).toBe("Nothing yet. Try: “The board flickers when I resize the window, and I'd like a dark mode toggle.”");
    expect(shown(html(createElement(NoClaude)))).toContain("Couldn't find the claude command.");
  });

  it("is Persian in fa", async () => {
    await setLocale("fa");
    expect(helperSteps(1, false)).toBe("۱ گام");
    expect(helperSteps(3, true)).toBe("۳ گام · تسک‌ها ثبت شد");
    expect(latin(helperEmpty())).toEqual([]);
    expect(latin(shown(html(createElement(NoClaude))))).toEqual([]);
  });
});
