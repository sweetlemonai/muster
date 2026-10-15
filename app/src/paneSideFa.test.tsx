// The task pane's right column and its left column in Persian (mu-148): Next step in each case,
// the field chips (Priority, Type, For), Est, Files, Links, the branch, what each stage shows, and
// the owner's checklist. English reads as before; Persian leaves no English word but the task's
// own data (titles, keys, branch, paths, file names, the examples an input stores).
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";
import { BranchCardLine, BranchPane } from "./Branch";
import { setLocale } from "./i18n/runtime";
import Links from "./Links";
import OwnerChecklist from "./OwnerChecklist";
import Sidebar, { FilesOpen, FilesView, NextStep, typeShown, ValueChipView } from "./Sidebar";
import StageContent from "./StageContent";
import TagEditor from "./Tags";
import type { Criterion, Task } from "./types";
import type { TaskActions } from "./useTaskActions";

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0007", prefix: "de", title: "Search box", kind: "code", priority: "normal", status: "doing", stage: "building", blocked: false, stageHistory: [],
  folder: "doing", path: "tasks/0007-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: ["ui"], estTime: "2h", estTestTime: "30m", estTokens: "500k",
  estMinutes: 120, estTestMinutes: 30, estTokenCount: 500_000, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [],
  progress: null, shownStage: over.stage ?? "building", search: "", ...over,
});
const a = { busy: false, pass: () => {}, release: () => {}, open: () => {}, unblock: () => {}, releaseUnreached: async () => true, setField: () => {}, attachPick: () => {} } as unknown as TaskActions;
const crit = (n: number, ticked: boolean, owner: boolean, words = "the box keeps its words"): Criterion => ({ n, label: `AC${n}`, text: `AC${n}: ${words} — verify: command`, ticked, owner, proof: ticked && !owner ? "Vitest 12 passed" : "", proofFiles: [] });

/** Text and the text attributes a person sees or hears, entities decoded, isolation marks out. */
const shown = (html: string) => {
  const attrs = [...html.matchAll(/\s(?:title|aria-label|placeholder|alt)="([^"]*)"/g)].map((m) => m[1]);
  return [html.replace(/<[^>]+>/g, " "), ...attrs].join(" ").replace(/&#x27;/g, "'").replace(/&amp;/g, "&").replace(/&quot;/g, '"').replace(/[⁦-⁩]/g, "").replace(/\s+/g, " ");
};
/** The task's own data, as written: titles, keys, the branch, paths, files, criterion words, stored examples. */
const DATA = new Set(["Search", "box", "de", "ui", "task", "build", "search", "Users", "owner", "projects", "demo", "worktrees", "AC", "the", "keeps", "its", "words", "Vitest", "passed", "Builder", "main", "session", "cc", "hg",
  "shot", "png", "notes", "txt", "Design", "https", "example", "com", "design", "http", "old", "muster", "set", "summary", "Child", "one", "two", "Graph", "verify", "command", "abc", "Worktree", "merged", "works", "looks", "right"]);
const latin = (html: string) => [...new Set(shown(html).match(/[A-Za-z][A-Za-z'’]*/g) ?? [])].filter((w) => w.length > 1 && !DATA.has(w));

const next = (over: Partial<Task>, nowSecs = 0) => renderToStaticMarkup(createElement(NextStep, { task: task(over), nowSecs, a }));
const side = (over: Partial<Task>) =>
  renderToStaticMarkup(createElement(Sidebar, { task: task(over), nowSecs: 0, a, images: {}, files: { open: new Set<string>(), onToggle: () => {}, openId: "f" } }));
const stage = (over: Partial<Task>, body: string | null = "## Acceptance criteria\n- [ ] AC1: works\n") =>
  renderToStaticMarkup(createElement(StageContent, { task: task(over), nowSecs: 1_792_054_168, body, busy: false, onTick: () => {}, onView: () => {} }));
const checklist = (over: Partial<Task>) => renderToStaticMarkup(createElement(OwnerChecklist, { task: task({ stage: "your-check", ...over }), busy: false, onTick: () => {}, onView: () => {} }));
const HISTORY = [{ at: "2026-10-14 10:40", from: "ready", to: "building", who: "Builder_69 (main · session 1f685b88 · cc-hg)" }];

/** Every case the column and the stages draw. */
const CASES = (): [string, string][] => [
  ["next: pass", next({ stage: "ready" })],
  ["next: held", next({ stage: "building", claimedBy: "Builder_69 (main · session 1f685b88 · cc-hg)" })],
  ["next: stale", next({ stage: "building", claimedBy: "Builder_69 (main · session 1f685b88 · cc-hg)", heartbeat: "2026-10-01 10:00" }, 1_792_054_168)],
  ["next: blocked", next({ stage: "ready", blocked: true })],
  ["next: paused", next({ stage: "ready", paused: "2026-10-14 13:00" })],
  ["next: done", next({ stage: "done" })],
  ["next: unknown", next({ stage: "limbo" })],
  ["next: feature", next({ stage: "building", level: "feature" })],
  ["next: your check", next({ stage: "your-check", ownerCriteria: [crit(3, false, true)] })],
  ["side: human", side({ forWho: "human", kind: "", priority: "high", links: [{ title: "Design", url: "https://example.com/design" }, { title: "", url: "http://example.com/old" }], attachments: ["shot.png", "notes.txt"] })],
  ["side: started", side({ branch: "task/de-7-build-search", worktree: "/Users/owner/projects/demo-worktrees/de-7" })],
  ["side: merged", side({ mergeReport: "merged", commit: "" })],
  ["stage: backlog", stage({ stage: "backlog" })],
  ["stage: ready, nothing written", stage({ stage: "ready", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0 }, "")],
  ["stage: shaping loading", stage({ stage: "shaping" }, null)],
  ["stage: building", stage({ stage: "building", stageHistory: HISTORY })],
  ["stage: building, untimed", stage({ stage: "testing", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0 })],
  ["stage: your check", stage({ stage: "your-check", criteria: [crit(1, true, false), crit(2, true, false, ""), crit(3, false, true), crit(4, false, false)], ownerCriteria: [crit(3, false, true)], stageHistory: HISTORY })],
  ["stage: your check, no summary", stage({ stage: "your-check", criteria: [], ownerCriteria: [] })],
  ["stage: done", stage({ stage: "done", stageHistory: HISTORY })],
  ["stage: done, no moves", stage({ stage: "done" })],
  ["stage: goal", stage({ level: "goal", progress: null, children: ["0001", "0002"] })],
  ["stage: feature", stage({ level: "feature", progress: { done: 1, total: 3, estMinutes: 240, estTokenCount: 400_000 } as Task["progress"], children: [] })],
  ["checklist: one", checklist({ ownerCriteria: [crit(3, false, true), crit(4, true, true)] })],
  ["checklist: many", checklist({ ownerCriteria: [crit(3, false, true, "the Graph looks right"), crit(4, false, true)] })],
  ["checklist: none open", checklist({ ownerCriteria: [crit(3, true, true)] })],
  ["chip: type", renderToStaticMarkup(createElement(ValueChipView, { label: "نوع", value: "", options: ["code", "bug"], shown: typeShown, open: true, onOpen: () => {}, onClose: () => {}, onPick: () => {} }))],
  ["files", renderToStaticMarkup(createElement(FilesView, { task: task({ attachments: ["shot.png"] }), images: {}, open: new Set(["shot.png"]), onToggle: () => {}, onAttach: () => {}, openId: "f" }))],
  ["files open", renderToStaticMarkup(createElement(FilesOpen, { task: task({ attachments: ["shot.png"] }), images: {}, open: new Set(["shot.png"]), onToggle: () => {}, id: "f" }))],
  ["links", renderToStaticMarkup(createElement(Links, { task: task({ links: [{ title: "", url: "http://example.com/old" }] }) }))],
  ["branch", renderToStaticMarkup(createElement(BranchPane, { task: task({ branch: "task/de-7-build-search", worktree: "" }) }))],
  ["tags", renderToStaticMarkup(createElement(TagEditor, { tags: ["ui"], onChange: () => {} }))],
];

afterEach(async () => {
  await setLocale("en");
});

describe("the pane's columns in English", () => {
  it("read as before", () => {
    expect(shown(next({ stage: "done" }))).toContain("Done. Send it back to reopen it.");
    expect(shown(next({ stage: "building", level: "feature" }))).toContain("Its tasks move it on.");
    expect(shown(next({ stage: "ready", blocked: true }))).toContain("Blocked on something it doesn't name. Unblock it to pass it on.");
    expect(shown(next({ stage: "ready", paused: "x" }))).toContain("Paused: resume it (beside its stage, above) to pass it on.");
    expect(shown(next({ stage: "building", claimedBy: "Kia" }))).toContain("Kia holds it in Building, so only they can pass it on. To move it yourself, click a stage on the track.");
    const s = shown(side({ kind: "", forWho: "human" }));
    for (const w of ["Priority: normal, click to change", "Type: no type, click to change", "For: human, click to change", "Est", "Tags", "Files", "+ Attach"]) expect(s).toContain(w);
    expect(shown(stage({ stage: "building", stageHistory: HISTORY }))).toContain("estimate 2 h of agent time and 30 min of test time");
    expect(shown(stage({ stage: "done", stageHistory: HISTORY }))).toContain("Ready → Building");
    expect(shown(stage({ stage: "ready" }, ""))).toContain("No acceptance criteria written yet. Edit adds them.");
    expect(shown(checklist({ ownerCriteria: [crit(3, false, true)] }))).toContain("Yours to verify: 1 left, then Accept.");
    expect(shown(renderToStaticMarkup(createElement(Links, { task: task({ links: [{ title: "", url: "http://x.org" }] }) })))).toContain("http://x.org (not https, so it won't open)");
    expect(renderToStaticMarkup(createElement(BranchCardLine, { t: task({ branch: "b", worktree: "/w/x" }) }))).toContain('title="Worktree: /w/x"');
    expect(shown(renderToStaticMarkup(createElement(TagEditor, { tags: ["ui"], onChange: () => {} })))).toContain("Remove tag ui");
  });
});

describe("the pane's columns in Persian", () => {
  it("leave no English word but the task's own data, in every case", async () => {
    await setLocale("fa");
    const left = CASES().map(([what, html]) => [what, latin(html)] as const).filter(([, w]) => w.length);
    expect(left).toEqual([]);
  });

  it("name the chips' values in Persian, and keep what's saved as it is", async () => {
    await setLocale("fa");
    const s = shown(side({ kind: "", forWho: "human", priority: "high" }));
    expect(s).toContain("اولویت: بالا، برای تغییر کلیک کنید");
    expect(s).toContain("نوع: بدون نوع، برای تغییر کلیک کنید");
    expect(s).toContain("برای: انسان، برای تغییر کلیک کنید");
    const chip = renderToStaticMarkup(createElement(ValueChipView, { label: "نوع", value: "bug", options: ["code", "bug"], shown: typeShown, open: true, onOpen: () => {}, onClose: () => {}, onPick: () => {} }));
    expect(shown(chip)).toContain("کد");
    expect(shown(chip)).toContain("باگ");
  });

  it("say the next step's sentences whole, with the holder and the stage as values", async () => {
    await setLocale("fa");
    expect(shown(next({ stage: "building", claimedBy: "Kia" }))).toContain("Kia آن را در «در حال ساخت» در دست دارد");
    expect(shown(next({ stage: "done" }))).toContain("انجام شد. برای باز کردن دوباره، آن را برگردانید.");
  });

  it("count the checklist in Persian digits, and each Tick reads its criterion", async () => {
    await setLocale("fa");
    const html = checklist({ ownerCriteria: [crit(3, false, true, "the Graph looks right"), crit(4, false, true)] });
    expect(shown(html)).toContain("۲ مورد");
    const tick = /<button[^>]*aria-describedby="([^"]+)"[^>]*>/.exec(html);
    expect(tick).not.toBeNull();
    expect(html).toContain(`id="${tick?.[1]}"`);
  });

  it("keep the copyable summary command verbatim in fa, with no isolation marks inside its <code>", async () => {
    await setLocale("fa");
    const raw = stage({ stage: "your-check", criteria: [], ownerCriteria: [] });
    expect(raw).toContain("<code>muster set de-7 summary</code>");
  });

  it("give a Persian criterion its own direction, not its Latin label's", async () => {
    await setLocale("fa");
    const html = checklist({ ownerCriteria: [crit(3, false, true, "پنل راست‌به‌چپ است")] });
    expect(html).toMatch(/<span class="words" id="[^"]+"><b>AC3<\/b> <span dir="auto">پنل راست‌به‌چپ است/);
  });

  it("draw a stage move with the arrow the reading way", async () => {
    await setLocale("fa");
    expect(shown(stage({ stage: "done", stageHistory: HISTORY }))).toContain("آماده ← در حال ساخت");
  });
});
