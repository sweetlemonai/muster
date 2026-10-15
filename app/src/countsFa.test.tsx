// mu-151, AC1: counts people read are in the digits picked: a column's head, the folded strip's
// needs and blocked counts, the sidebar's rows, the Your check count, a card's badges, the No goal
// band and the session recap's check-ins; keys stay Latin. Dates carry a <time datetime> and a
// comment's stamp keeps its order beside a Latin name (review R3–R6, R8).
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import Card from "./Card";
import Comments from "./Comments";
import { HeadCount } from "./HeadCount";
import { Stamp } from "./Dates";
import { BandHead } from "./Groups";
import { unmarked } from "./i18n";
import { setLocale, setNumbering } from "./i18n/runtime";
import { ColumnHead, FoldCounts } from "./ModeViews";
import { ProjectItem } from "./ProjectList";
import Recent from "./Recent";
import { RecapView } from "./SessionRecap";
import type { SessionRecap, Task } from "./types";
import { checkCount } from "./yourCheckLimit";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));

const task = (id: string, over: Partial<Task> = {}): Task => ({
  queue: "muster", id, prefix: "qu", title: `Task ${id}`, kind: "code", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: `todo/${id}-t.md`, claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
}) as Task;
// 14:00 local time on 2026-10-13, as the state's `now`.
const NOW = new Date(2026, 9, 13, 14, 0).getTime() / 1000;
const card = (t: Task) => renderToStaticMarkup(createElement(Card, { t, now: NOW, multi: false, parent: "", onOpen: () => {}, active: false, words: [] }));
const plain = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").trim();

afterEach(async () => {
  setNumbering({});
  await setLocale("en");
});

const head = (count: number) =>
  renderToStaticMarkup(<ColumnHead stage={{ id: "building", label: "Building", role: "" }} count={count} needs={0} blocked={0} fold="fixed" onFold={() => {}} />);

describe("counts in the digits picked", () => {
  it("a column's head: 12 in English, ۱۲ in فارسی, 12 with 123 picked", async () => {
    expect(head(12)).toContain("<small>12</small>");
    await setLocale("fa");
    expect(head(12)).toContain("<small>۱۲</small>");
    setNumbering({ digits: "latin" });
    expect(head(12)).toContain("<small>12</small>");
  });

  it("a folded strip's needs-you and blocked counts", async () => {
    await setLocale("fa");
    const html = renderToStaticMarkup(<FoldCounts needs={3} blocked={14} />);
    expect(html).toContain(">۳</b>");
    expect(html).toContain(">۱۴ مسدود</b>");
  });

  it("a sidebar row's total, needs-you, in progress and paused counts", async () => {
    await setLocale("fa");
    const task = (id: string, stage: string, paused = false) => ({ id, stage, paused, forWho: "human", waitingForOwner: false, queue: "q" });
    const tasks = [task("0001", "your-check"), task("0002", "building"), task("0003", "building", true)] as never[];
    const html = renderToStaticMarkup(<ProjectItem name="quill" total={198} tasks={tasks} on={false} onClick={() => {}}>{null}</ProjectItem>);
    expect(html).toContain("<small>۱۹۸</small>");
    expect(html).not.toMatch(/>\s*198\s*</);
  });

  it("a toolbar count (Waiting for you, Problems): 12, ۱۲ in فارسی; empty at zero", async () => {
    const count = (n: number) => renderToStaticMarkup(<HeadCount n={n} className="warn" />);
    expect(count(12)).toBe('<b class="count count-reserve warn">12</b>');
    await setLocale("fa");
    expect(count(12)).toBe('<b class="count count-reserve warn">۱۲</b>');
    expect(count(0)).toBe('<b class="count count-reserve warn" aria-hidden="true"></b>');
    setNumbering({ digits: "latin" });
    expect(count(12)).toBe('<b class="count count-reserve warn">12</b>');
  });

  it("the Your check count", async () => {
    expect(checkCount(7, [], "q").text).toBe("7");
    await setLocale("fa");
    expect(checkCount(7, [], "q").text).toBe("۷");
  });

  it("R5: the Your check count over its limit, and its tooltip", async () => {
    const over = [{ queue: "q", label: "quill", waiting: 7, limit: 5 }] as never[];
    expect(checkCount(7, over, "q")).toMatchObject({ text: "7 of 5", over: true });
    await setLocale("fa");
    const c = checkCount(7, over, "q");
    expect(c.text).toBe("۷ از ۵");
    expect(unmarked(c.title)).toContain("۷ تسک در «بررسی شما» منتظر است، بیش از سقف شما (۵)");
    expect(unmarked(checkCount(12, over, "all").title)).toBe("بیش از سقف در quill (۷ از ۵)");
  });

  it("R6: a card's open-question and file badges", async () => {
    const t = task("0146", { openQuestions: 2, attachments: ["a.png", "b.png", "c.png"] as never });
    expect(plain(card(t))).toContain("2 open Q");
    await setLocale("fa");
    const html = plain(card(t));
    expect(html).toContain("۲ پرسش باز");
    expect(html).toContain("۳ فایل");
  });

  it("R6: the No goal band's count", async () => {
    await setLocale("fa");
    const html = renderToStaticMarkup(<BandHead group={{ head: null, items: [task("0001"), task("0002"), task("0003")] } as never} by="goal" multi={false} active={false} onOpen={() => {}} />);
    expect(plain(html)).toContain("بدون هدف ۳ مورد");
  });

  it("R6: the session recap's check-in percentages, as the pane's", async () => {
    const recap = {
      who: { account: "mg", id: "aaaa0005", name: "Asghar_5", project: "muster", checkout: "wt", group: "completed", word: "stopped", at: NOW - 660 },
      tasks: [],
      history: [{ at: NOW - 3600, key: "mu-98", kind: "check-in", text: "core done", pct: 40 }],
      chain: [],
      cost: {
        found: "read", start: NOW - 7200, end: NOW - 660, minutes: 109, tokens: { input: 1000, output: 2000, cacheRead: 0, cacheWrite: 0 }, totalTokens: 3000, countedTokens: 3000,
        chainMinutes: 109, chainTokens: { input: 1000, output: 2000, cacheRead: 0, cacheWrite: 0 }, chainTotalTokens: 3000, chainCountedTokens: 3000, chainRead: 1, chainSessions: 1,
        estTokens: 0, estPct: null, chainEstPct: null,
      },
      progress: [{ key: "mu-98", pct: 40, text: "core done", guess: false }],
    } as unknown as SessionRecap;
    const view = () => renderToStaticMarkup(<RecapView recap={recap} error="" now={NOW} showing={false} transcript={null} onTranscript={() => {}} onClose={() => {}} onOpenTask={() => {}} />);
    expect(view().match(/<b>40%<\/b>/g)?.length).toBe(2);
    await setLocale("fa");
    const html = view();
    expect(html.match(/<b>۴۰٪<\/b>/g)?.length).toBe(2);
    expect(html).not.toContain("40%");
    // The keys beside them stay Latin.
    expect(html).toContain("mu-98");
  });

  it("R11: the session recap's History times and its started and last-active times, as the pane's", async () => {
    const recap = {
      who: { account: "mg", id: "aaaa0005", name: "Asghar_5", project: "muster", checkout: "wt", group: "completed", word: "stopped", at: NOW - 660 },
      tasks: [],
      history: [{ at: NOW - 3600, key: "mu-98", kind: "check-in", text: "core done", pct: null }],
      chain: [],
      cost: {
        found: "read", start: NOW - 7200 as number | null, end: NOW - 660, minutes: 109, tokens: { input: 1000, output: 2000, cacheRead: 0, cacheWrite: 0 }, totalTokens: 3000, countedTokens: 3000,
        chainMinutes: 109, chainTokens: { input: 1000, output: 2000, cacheRead: 0, cacheWrite: 0 }, chainTotalTokens: 3000, chainCountedTokens: 3000, chainRead: 1, chainSessions: 1,
        estTokens: 0, estPct: null, chainEstPct: null,
      },
      progress: [],
    };
    const view = () => renderToStaticMarkup(<RecapView recap={recap as unknown as SessionRecap} error="" now={NOW} showing={false} transcript={null} onTranscript={() => {}} onClose={() => {}} onOpenTask={() => {}} />);
    const time = (html: string) => plain(/<section class="recap-time"[\s\S]*?<\/section>/.exec(html)?.[0] ?? "");
    // English reads the stamps as the files write them.
    let html = view();
    expect(html).toContain('<time class="when" dateTime="2026-10-13T13:00" dir="auto">2026-10-13 13:00</time>');
    expect(time(html)).toContain("Started 2026-10-13 12:00 · last active 2026-10-13 13:49");
    await setLocale("fa");
    html = view();
    expect(unmarked(html)).toContain('<time class="when" dateTime="2026-10-13T13:00" dir="auto">۲۱ مهر ۱۴۰۵ ۱۳:۰۰</time>');
    expect(unmarked(time(html))).toContain("شروع ۲۱ مهر ۱۴۰۵ ۱۲:۰۰ · آخرین فعالیت ۲۱ مهر ۱۴۰۵ ۱۳:۴۹");
    expect(html).not.toContain("2026-10-13 ");
    // No time known: a dash, as before.
    recap.cost.start = null;
    expect(unmarked(time(view()))).toContain("شروع — ·");
  });
});

describe("keys stay Latin in فارسی (R8)", () => {
  it("a card's key, with its counts in Persian digits around it", async () => {
    await setLocale("fa");
    const html = card(task("0146", { openQuestions: 1 }));
    expect(html).toMatch(/qu-146/);
    expect(html).not.toMatch(/qu-[۰-۹]/);
    expect(plain(html)).toContain("۱ پرسش باز");
  });
});

describe("dates people read (R3, R4)", () => {
  it("R3: a stamp is isolated, so a comment line that starts with a Latin name keeps its order", async () => {
    await setLocale("fa");
    expect(renderToStaticMarkup(<Stamp at="2026-10-14 20:22" year />)).toMatch(/^<time dateTime="2026-10-14T20:22" dir="auto">[^<]*۱۴۰۵[^<]*<\/time>$/);
    const html = renderToStaticMarkup(<Comments task={task("0001", { comments: [{ at: "2026-10-14 20:22", who: "Tester_51", text: "Looks right." }] as never })} onChanged={() => {}} />);
    expect(html).toMatch(/<p class="meta" dir="auto">Tester_51 · <time dateTime="2026-10-14T20:22" dir="auto">/);
  });

  it("R4: Recently finished and a Done card carry the stored stamp in a <time datetime>", async () => {
    const done = task("0050", { stage: "done", doneAt: "2026-10-13 13:56", stageHistory: [{ at: "2026-10-13 13:56", who: "owner", from: "your-check", to: "done" }] });
    await setLocale("fa");
    expect(card(done)).toMatch(/<time class="card-done" title="[^"]*2026-10-13 13:56[^"]*" dateTime="2026-10-13T13:56">[^<]*۴[^<]*<\/time>/);
    expect(card(task("0052", { stage: "done", doneOn: "2026-10-12" }))).toContain('dateTime="2026-10-12"');
    const recent = renderToStaticMarkup(createElement(Recent, { tasks: [done], now: NOW, kind: "all", onKind: () => {}, multi: false, openKey: null, onOpen: () => {}, onFold: () => {} }));
    expect(recent).toMatch(/<time class="recent-when" title="2026-10-13 13:56" dateTime="2026-10-13T13:56">/);
  });
});
