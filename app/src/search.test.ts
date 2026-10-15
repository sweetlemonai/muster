import { describe, expect, it } from "vitest";
import { BOARD_SORTS, sortBoard } from "./lib";
import { fold, highlights, indexStamp, needsIndex, queryWords, searchWords, taskMatches, withSearch } from "./search";
import type { Task } from "./types";

const t = (over: Partial<Task>): Task => ({
  queue: "q", id: "0001", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [], folder: "todo", path: "", claimedBy: "", heldBy: "", stageSince: "", checkins: [],
  heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [], openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false,
  tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", search: "", ...over,
});
const ids = (list: Task[]) => list.map((x) => x.id);
const finds = (task: Task, q: string) => taskMatches(task, queryWords(q), {});

describe("fold", () => {
  it("lowercases and drops accents, precomposed or decomposed", () => {
    expect(fold("Café RÉSUMÉ")).toBe("cafe resume");
    expect(fold("Café")).toBe("cafe");
    expect(fold("Ærøskøbing ß Œuvre þorn")).toBe("aeroskobing ss oeuvre thorn");
    expect(fold("İstanbul ŁÓDŹ")).toBe("istanbul lodz");
  });
  it("turns everything that isn't a letter or a digit into a space", () => {
    expect(fold("tech-debt/#UI!")).toBe("tech debt  ui ");
    expect(fold("0019's")).toBe("0019 s");
    expect(fold("")).toBe("");
  });
  it("keeps letters of other scripts", () => {
    expect(fold("Задача 任务")).toBe("задача 任务");
  });
  it("folds Persian as people type it: Arabic letters, hamza, heh, alef, ZWNJ, tatweel, harakat", () => {
    expect(fold("كتاب علي مصطفى مسئول")).toBe("کتاب علی مصطفی مسیول");
    expect(fold("خانۀ خانهٔ مدرسة")).toBe("خانه خانه مدرسه");
    expect(fold("آب أحمد إسلام ٱسم مؤسسه شيء")).toBe("اب احمد اسلام اسم موسسه شی");
    expect(fold("می\u200cشود کـتاب کِتاب هٰذا")).toBe("میشود کتاب کتاب هذا");
  });
  it("reads Persian and Arabic-Indic digits as 0–9", () => {
    expect(fold("۰۱۲۳۴۵۶۷۸۹ ٠١٢٣٤٥٦٧٨٩")).toBe("0123456789 0123456789");
    expect(fold("۱2٣")).toBe("123");
  });
});

describe("queryWords", () => {
  it("splits the folded query on spaces; empty and punctuation give none", () => {
    expect(queryWords("  Flicker  RÉSUMÉ ")).toEqual(["flicker", "resume"]);
    expect(queryWords("#design")).toEqual(["design"]);
    expect(queryWords("")).toEqual([]);
    expect(queryWords(" -- ")).toEqual([]);
  });
});

describe("searchWords", () => {
  it("is what the core builds: each folded word once, no words of digits only", () => {
    expect(searchWords("Board flickers 2026-10-12 board v2 0019 Café")).toBe("board flickers v2 cafe");
  });
  it("leaves out words of Persian digits too, and keeps a word once however it was typed", () => {
    expect(searchWords("كتاب کتاب صفحهٔ ۱۲ ١٤٠٥/٠٧/٢٢")).toBe("کتاب صفحه");
  });
});

describe("taskMatches", () => {
  const task = t({
    id: "0019",
    title: "Board flickers on resize",
    tags: ["ui", "tech-debt"],
    stage: "building",
    claimedBy: "drover/main (session 3fba2222)",
    search: "board flickers on resize ui tech debt the columns jump when the window shrinks capture created",
  });
  it("finds a word of the title, the text, a tag or the holder, as part of a word too", () => {
    expect(finds(task, "flickers")).toBe(true);
    expect(finds(task, "flick")).toBe(true);
    expect(finds(task, "shrinks")).toBe(true);
    expect(finds(task, "debt")).toBe(true);
    expect(finds(task, "drover")).toBe(true);
    expect(finds(task, "missing")).toBe(false);
  });
  it("wants every word, in any order", () => {
    expect(finds(task, "resize board")).toBe(true);
    expect(finds(task, "board crash")).toBe(false);
  });
  it("ignores case and accents on both sides", () => {
    expect(finds(task, "BOARD")).toBe(true);
    expect(finds(t({ search: "cafe resume" }), "Café résumé")).toBe(true);
  });
  it("reads a word of digits as an id: 19 and 0019 find 0019, not 0119 or 0190", () => {
    expect(finds(task, "19")).toBe(true);
    expect(finds(task, "0019")).toBe(true);
    expect(finds(task, "00019")).toBe(true);
    expect(finds(t({ id: "0119" }), "19")).toBe(false);
    expect(finds(t({ id: "0190" }), "19")).toBe(false);
    expect(finds(task, "19 flick")).toBe(true);
    expect(finds(task, "20 flick")).toBe(false);
  });
  it("a word of digits never matches text, so dates in the log don't swamp an id", () => {
    expect(finds(t({ id: "0001", search: "created 2026 10 19" }), "19")).toBe(false);
  });
  it("Persian digits are an id as typed Latin ones are: ۱۹ and ٠٠١٩ find 0019", () => {
    expect(queryWords("۱۹")).toEqual(["19"]);
    expect(finds(task, "۱۹")).toBe(true);
    expect(finds(task, "٠٠١٩ flick")).toBe(true);
    expect(finds(t({ id: "0119" }), "۱۹")).toBe(false);
  });
  it("finds Persian text whichever way it was typed", () => {
    const fa = t({ id: "0012", search: searchWords("كتاب راهنما چاپ می\u200cشود") });
    expect(finds(fa, "میشود")).toBe(true);
    expect(finds(fa, "می\u200cشود")).toBe(true);
    expect(finds(fa, "کتاب")).toBe(true);
    expect(finds(fa, "کِتاب")).toBe(true);
    expect(finds(fa, "دفتر")).toBe(false);
  });
  it("a zero id: 0 finds 0000", () => {
    expect(finds(t({ id: "0000" }), "0")).toBe(true);
  });
  it("an empty query finds everything", () => {
    expect(finds(task, "")).toBe(true);
    expect(finds(t({}), "  ")).toBe(true);
  });
  it("the holder counts only while the task is held in its stage", () => {
    expect(finds(t({ stage: "done", claimedBy: "drover/main" }), "drover")).toBe(false);
  });
  it("the holder is the stamp's label (0030), in Testing and Your check too; the session isn't searched", () => {
    const testing = t({ stage: "testing", heldBy: "wt-a (wt-0030 · session 7b5e8856 · cc-hg)" });
    expect(finds(testing, "wt-a")).toBe(true);
    expect(finds(testing, "7b5e8856")).toBe(false);
    expect(finds(testing, "cc-hg")).toBe(false);
    expect(finds(t({ stage: "your-check" }), "owner")).toBe(true);
    expect(finds(t({ stage: "testing", claimedBy: "wt-a" }), "wt-a")).toBe(false);
  });
});

describe("the search index (the polled state has no search text)", () => {
  const a = t({ queue: "muster", id: "0001", modified: 100 });
  const b = t({ queue: "muster", id: "0002", modified: 200 });
  const c = t({ queue: "drover", id: "0001", modified: 300 });
  // A poll brings new objects, with the same values when nothing changed.
  const poll = (list: Task[]) => list.map((x) => ({ ...x }));
  it("isn't asked for while the search has no words", () => {
    expect(needsIndex(false, [a, b], null)).toBe(false);
    expect(needsIndex(false, [a, b], indexStamp([]))).toBe(false);
  });
  it("is asked for once there are words and none was fetched", () => {
    expect(needsIndex(true, [a, b], null)).toBe(true);
    expect(needsIndex(true, [], null)).toBe(true);
  });
  it("isn't asked for again while no task's modified changed, poll after poll", () => {
    const last = indexStamp([a, b]);
    expect(needsIndex(true, poll([a, b]), last)).toBe(false);
    expect(needsIndex(true, poll([b, a]), last)).toBe(false);
  });
  it("is asked for again when a task changed or is new", () => {
    const last = indexStamp([a, b]);
    expect(needsIndex(true, [a, { ...b, modified: 201 }], last)).toBe(true);
    expect(needsIndex(true, [a, b, t({ queue: "muster", id: "0003" })], last)).toBe(true);
  });
  it("isn't asked for again when a task is gone: its text just goes unused", () => {
    expect(needsIndex(true, [a], indexStamp([a, b]))).toBe(false);
  });
  it("tells projects apart: All after one project asks again, one project after All doesn't", () => {
    expect(needsIndex(true, [a, b, c], indexStamp([a, b]))).toBe(true);
    expect(needsIndex(true, [c], indexStamp([a, b, c]))).toBe(false);
    expect(needsIndex(true, [{ ...a, queue: "drover" }], indexStamp([a]))).toBe(true);
  });
  it("gives each task its text by queue and id, and none to one it doesn't have", () => {
    const bare = [t({ queue: "muster", id: "0001", search: undefined }), t({ queue: "drover", id: "0001", search: undefined })];
    const got = withSearch(bare, { "muster/0001": "board flickers", "drover/0002": "other" });
    expect(got.map((x) => x.search)).toEqual(["board flickers", ""]);
    expect(bare[0].search).toBeUndefined();
    expect(finds(got[0], "flick")).toBe(true);
    expect(finds(got[1], "flick")).toBe(false);
  });
  it("a task without its text yet matches only by id and holder", () => {
    const bare = t({ id: "0019", stage: "ready", claimedBy: "drover/main", search: undefined });
    expect(finds(bare, "undefined")).toBe(false);
    expect(finds(bare, "flick")).toBe(false);
    expect(finds(bare, "19 drover")).toBe(true);
  });
});

describe("sortBoard", () => {
  const list = [
    t({ id: "0001", priority: "low", captured: "2026-10-01", modified: 30, estMinutes: 120 }),
    t({ id: "0002", priority: "normal", captured: "2026-10-03", modified: 10 }),
    t({ id: "0003", captured: "2026-10-03", modified: 20, estMinutes: 30, waitingForOwner: true }),
    t({ id: "0004", priority: "P0", captured: "", modified: 40, estMinutes: 30, estTokenCount: 5 }),
  ];
  it("offers the board's orders", () => {
    expect(BOARD_SORTS.map((s) => s.id)).toEqual(["priority", "newest", "oldest", "changed", "estimate", "id"]);
  });
  it("priority: needs you first, then high to low (P0 is high), then id", () => {
    expect(ids(sortBoard(list, "priority"))).toEqual(["0003", "0004", "0002", "0001"]);
  });
  it("priority puts Done last, latest change first", () => {
    const done = [t({ id: "0005", stage: "done", modified: 1, priority: "high" }), t({ id: "0006", stage: "done", modified: 9 }), t({ id: "0007" })];
    expect(ids(sortBoard(done, "priority"))).toEqual(["0007", "0006", "0005"]);
  });
  it("newest and oldest go by the day it was captured; same day by id; no date last", () => {
    expect(ids(sortBoard(list, "newest"))).toEqual(["0003", "0002", "0001", "0004"]);
    expect(ids(sortBoard(list, "oldest"))).toEqual(["0001", "0002", "0003", "0004"]);
  });
  it("changed: the latest change first", () => {
    expect(ids(sortBoard(list, "changed"))).toEqual(["0004", "0001", "0003", "0002"]);
  });
  it("estimate: smallest first, time then tokens, not estimated last", () => {
    expect(ids(sortBoard(list, "estimate"))).toEqual(["0003", "0004", "0001", "0002"]);
  });
  it("id: by project, then id", () => {
    const multi = [t({ id: "0002", queue: "b" }), t({ id: "0001", queue: "b" }), t({ id: "0009", queue: "a" })];
    expect(sortBoard(multi, "id").map((x) => `${x.queue}/${x.id}`)).toEqual(["a/0009", "b/0001", "b/0002"]);
  });
  it("doesn't change the list it's given", () => {
    const copy = [...list];
    sortBoard(list, "id");
    expect(list).toEqual(copy);
  });
});

describe("highlights", () => {
  it("marks each match in the shown text, accents and case as typed", () => {
    expect(highlights("Board flickers on Résumé", ["flick", "resume"])).toEqual([[6, 11], [18, 24]]);
  });
  it("merges overlapping matches and finds every occurrence", () => {
    expect(highlights("abcabc", ["abc", "bca"])).toEqual([[0, 6]]);
    expect(highlights("a-a", ["a"])).toEqual([[0, 1], [2, 3]]);
  });
  it("maps back through letters that fold to two (ß) and decomposed accents", () => {
    expect(highlights("Straße", ["strasse"])).toEqual([[0, 6]]);
    expect(highlights("Café au lait", ["cafe"])).toEqual([[0, 5]]);
  });
  it("marks nothing for no words, and never for words of digits (they're ids)", () => {
    expect(highlights("Board", [])).toEqual([]);
    expect(highlights("Release 19", ["19"])).toEqual([]);
    expect(highlights("صفحهٔ ۱۲", ["12"])).toEqual([]);
  });
  it("marks Persian as shown: through a ZWNJ, tatweel or harakat, and a trailing mark with its letter", () => {
    expect(highlights("این می\u200cشود", ["میشود"])).toEqual([[4, 10]]);
    expect(highlights("كتاب", ["کتاب"])).toEqual([[0, 4]]);
    expect(highlights("کـتاب کِتاب", ["کتاب"])).toEqual([[0, 5], [6, 11]]);
    expect(highlights("خانهٔ ما", ["خانه"])).toEqual([[0, 5]]);
    expect(highlights("کتابً", ["کتاب"])).toEqual([[0, 5]]);
  });
});
