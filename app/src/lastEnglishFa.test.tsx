// The last English words Persian screenshots still showed (mu-147): the estimate line of the card
// and the Table, the owner as a holder in the Table and on the Graph, the tag box of New task and
// the pane, and the session recap's History kinds. English reads as before; Persian has no English
// word left, and the figures (2h, 30m, 300k) are isolated, in Persian digits and words (mu-151).
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";
import Card from "./Card";
import { estimateText } from "./fields";
import { GraphView } from "./GraphView";
import { FSI, PDI, setLocale } from "./i18n/runtime";
import { historyKind } from "./sessionsData";
import TagEditor, { MAX_TAGS } from "./Tags";
import type { Task } from "./types";
import { sortTasks } from "./viewData";
import { TableView } from "./Views";

const task = (over: Partial<Task>): Task => ({
  queue: "alpha", id: "0001", title: "T", kind: "bug", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [], folder: "todo", path: "", claimedBy: "", heldBy: "", stageSince: "", checkins: [],
  heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "2026-10-14", capturedAt: "", doneAt: "", blockHistory: [], questions: [], openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false,
  tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", search: "", ...over,
});
const NOW = new Date(2026, 9, 14, 17, 20).getTime() / 1000;
const ESTIMATED = task({ id: "0002", title: "Sized", estTime: "2h", estTestTime: "20m", estTokens: "300k", estMinutes: 120, estTestMinutes: 20, estTokenCount: 300_000 });
// In Your check with nobody named: the owner's (lib.ts holderOf).
const YOURS = task({ id: "0003", title: "Yours", stage: "your-check", depends: ["0002"] });
const OWNER_STAMPED = task({ id: "0004", title: "Stamped", stage: "your-check", heldBy: "owner (main-29 · session 7b5e8856 · cc-hg)" });

/** Text and the text attributes a person sees or hears, entities decoded, isolation marks out. */
const shown = (html: string) => {
  const attrs = [...html.matchAll(/\s(?:title|aria-label|placeholder|alt)="([^"]*)"/g)].map((m) => m[1]);
  return [html.replace(/<[^>]+>/g, " "), ...attrs].join(" ").replace(/&#x27;/g, "'").replace(/&amp;/g, "&").replace(/[⁦-⁩]/g, "").replace(/\s+/g, " ");
};
const table = (tasks: Task[]) => renderToStaticMarkup(createElement(TableView, { tasks, now: NOW, multi: false, openKey: null, onOpen: () => {}, words: [] }));
const graph = (tasks: Task[]) => renderToStaticMarkup(createElement(GraphView, { tasks, all: tasks, multi: false, openKey: null, onOpen: () => {} }));
const card = (t: Task) => renderToStaticMarkup(createElement(Card, { t, now: NOW, multi: false, parent: "", onOpen: () => {}, active: false, words: [] }));
const whoCell = (html: string) => [...html.matchAll(/<td class="meta" dir="auto">([^<]*)<\/td>/g)].map((m) => m[1].replace(/[⁦-⁩]/g, ""));
const graphWho = (html: string) => [...html.matchAll(/<span class="g-who" dir="auto">([^<]*)<\/span>/g)].map((m) => m[1]);
const tagBox = (tags: string[]) => renderToStaticMarkup(createElement(TagEditor, { tags, onChange: () => {} }));
const input = (html: string) => /<input[^>]*>/.exec(html)?.[0] ?? "";
const attr = (html: string, name: string) => new RegExp(`\\s${name}="([^"]*)"`).exec(html)?.[1];

afterEach(async () => {
  await setLocale("en");
});

describe("the estimate line", () => {
  it("reads as before in English", () => {
    expect(estimateText({ estTime: "30m", estTestTime: "", estTokens: "20k" })).toBe("agent 30m · 20k");
    expect(estimateText({ estTime: "2h", estTestTime: "20m", estTokens: "300k" })).toBe("agent 2h · test 20m · 300k");
    expect(estimateText({ estTime: "8h", estTestTime: "", estTokens: "1.2M" })).toBe("agent 8h · 1.2M");
    expect(estimateText({ estTime: "", estTestTime: "1h", estTokens: "" })).toBe("test 1h");
    expect(estimateText({ estTime: "", estTestTime: "", estTokens: "" })).toBe("");
  });

  it("says agent and test in Persian, each figure in Persian digits and words, isolated", async () => {
    await setLocale("fa");
    expect(estimateText({ estTime: "30m", estTestTime: "", estTokens: "20k" })).toBe(`ایجنت ${FSI}۳۰ دقیقه${PDI} · ${FSI}۲۰\u00a0هزار${PDI}`);
    expect(estimateText({ estTime: "2h", estTestTime: "20m", estTokens: "300k" })).toBe(`ایجنت ${FSI}۲ ساعت${PDI} · تست ${FSI}۲۰ دقیقه${PDI} · ${FSI}۳۰۰\u00a0هزار${PDI}`);
    expect(estimateText({ estTime: "", estTestTime: "1h", estTokens: "" })).toBe(`تست ${FSI}۱ ساعت${PDI}`);
    expect(estimateText({ estTime: "", estTestTime: "", estTokens: "" })).toBe("");
  });

  it("leaves no English word on the card or in the Table's estimate cell in Persian", async () => {
    expect(shown(card(ESTIMATED))).toContain("agent 2h · test 20m · 300k");
    expect(shown(table([ESTIMATED]))).toContain("agent 2h · test 20m · 300k");
    await setLocale("fa");
    for (const html of [card(ESTIMATED), table([ESTIMATED])]) {
      expect(shown(html)).toContain("ایجنت ۲ ساعت · تست ۲۰ دقیقه · ۳۰۰ هزار");
      expect(shown(html)).not.toMatch(/\b(agent|test)\b/);
    }
  });
});

describe("the owner as a holder in the Table and on the Graph", () => {
  it("says you, as the card does, never the raw owner", () => {
    expect(whoCell(table([YOURS, OWNER_STAMPED]))).toEqual(["you", "you"]);
    expect(graphWho(graph([ESTIMATED, YOURS]))).toEqual(["you"]);
  });

  it("says شما in Persian", async () => {
    await setLocale("fa");
    expect(whoCell(table([YOURS, OWNER_STAMPED]))).toEqual(["شما", "شما"]);
    expect(graphWho(graph([ESTIMATED, YOURS]))).toEqual(["شما"]);
    expect(shown(table([YOURS]) + graph([ESTIMATED, YOURS]))).not.toContain("owner");
  });

  it("keeps any other holder as written", async () => {
    const held = task({ id: "0005", stage: "building", claimedBy: "gholi (wt-0005 · session 1234abcd · cc-mg)" });
    expect(whoCell(table([held]))).toEqual(["gholi"]);
    await setLocale("fa");
    expect(whoCell(table([held]))).toEqual(["gholi"]);
  });

  it("sorts the Table's who column by the word it shows", () => {
    const zed = task({ id: "0006", stage: "building", claimedBy: "zed" });
    const pat = task({ id: "0007", stage: "building", claimedBy: "pat" });
    expect(sortTasks([zed, YOURS, pat], "who", false, {}).map((t) => t.id)).toEqual(["0007", "0003", "0006"]);
  });
});

describe("the tag box", () => {
  it("reads as before in English", () => {
    const box = input(tagBox([]));
    expect(attr(box, "placeholder")).toBe("+ tag");
    expect(attr(box, "aria-label")).toBe("Add a tag");
    expect(attr(input(tagBox(Array.from({ length: MAX_TAGS }, (_, i) => `t${i}`))), "placeholder")).toBe(`${MAX_TAGS} tags max`);
  });

  it("says its placeholder and its label in Persian", async () => {
    await setLocale("fa");
    const box = input(tagBox([]));
    expect(attr(box, "placeholder")).toBe("+ تگ");
    expect(attr(box, "aria-label")).toBe("افزودن تگ");
    expect(attr(input(tagBox(Array.from({ length: MAX_TAGS }, (_, i) => `t${i}`))), "placeholder")).toBe("حداکثر ۸ تگ");
  });
});

describe("the session recap's History kinds", () => {
  // Every kind the core's sessions.rs sends: history_event's ev(…) and the chain's handoffs.
  const CORE_KINDS = ["claim", "return", "pass", "check-in", "takeover", "note", "handoff-from", "handoff-to"];

  it("names every kind the core sends in English, an unknown one as written", () => {
    expect(CORE_KINDS.map(historyKind)).toEqual(["claim", "return", "pass", "check-in", "takeover", "note", "handoff", "handoff"]);
    expect(historyKind("checkin")).toBe("checkin");
  });

  it("names every kind the core sends in Persian, an unknown one as written", async () => {
    await setLocale("fa");
    expect(CORE_KINDS.map(historyKind)).toEqual(["برداشتن", "برگرداندن", "فرستادن به بعد", "گزارش وضعیت", "در دست گرفتن", "یادداشت", "واگذاری", "واگذاری"]);
    expect(historyKind("checkin")).toBe("checkin");
  });
});
