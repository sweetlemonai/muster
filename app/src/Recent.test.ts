import { createElement as h, type ReactElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import Recent, { costOf, costText, RecentList } from "./Recent";
import { dayGroups, finishes } from "./recentData";
import type { StageStep, Task } from "./types";

const task = (id: string, over: Partial<Task> = {}): Task => ({
  queue: "muster", id, title: `Task ${id}`, kind: "code", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: `todo/${id}-t.md`, claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});

// The fake clock: 14:00 local time on Tuesday 2026-10-13, in seconds since 1970, like the state's `now`.
const NOW = new Date(2026, 9, 13, 14, 0).getTime() / 1000;
const step = (at: string, from: string, to: string, who = "muster/main (merge) (session 7b5e8856 · cc-hg)"): StageStep => ({ at, who, from, to });
const finished = (id: string, merged: string, accepted = "", over: Partial<Task> = {}): Task => {
  const history = [step("2026-10-01 09:00", "ready", "building", "muster/wt-0051 (w-0051) (main · session 531266d4 · cc-hg)"), step("2026-10-01 10:00", "building", "testing"), step(merged, "testing", "your-check")];
  if (accepted) history.push(step(accepted, "your-check", "done", "owner"));
  return task(id, { stage: accepted ? "done" : "your-check", stageHistory: history, doneAt: accepted, ...over });
};

const TASKS = [
  finished("0050", "2026-10-13 13:56", "", { title: "Recently finished" }),
  finished("0043", "2026-10-12 18:20", "2026-10-13 09:15", { title: "Your check says what to do" }),
  finished("0031", "2026-10-11 09:00", "2026-10-11 09:05", { title: "Timeline" }),
  finished("0019", "2026-09-20 12:00", "2026-09-21 12:00", { title: "Graph" }),
  task("0060", { stage: "building", claimedBy: "wt-b", title: "Still building" }),
];

const plain = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").trim();
const render = (props: Partial<Parameters<typeof Recent>[0]> = {}) =>
  renderToStaticMarkup(h(Recent, { tasks: TASKS, now: NOW, kind: "all", onKind: () => {}, multi: false, openKey: null, onOpen: () => {}, onFold: () => {}, ...props }));
const heads = (html: string) => [...html.matchAll(/<h3 class="recent-day-head">([\s\S]*?)<\/h3>/g)].map((m) => plain(m[1]));
const rowsOf = (html: string) => [...html.matchAll(/<button type="button" class="recent-row[^"]*"[^>]*>([\s\S]*?)<\/button>/g)].map((m) => plain(m[1]));

/** Every <button> in an element tree a hook-free component returned, with its words. */
function buttons(node: ReactNode): { words: string; click: () => void }[] {
  if (node === null || node === undefined || typeof node === "boolean" || typeof node === "string" || typeof node === "number") return [];
  if (Array.isArray(node)) return node.flatMap(buttons);
  const el = node as ReactElement<{ children?: ReactNode; onClick?: () => void }>;
  if (typeof el.type === "function") return buttons((el.type as (p: unknown) => ReactNode)(el.props));
  // A context's Consumer (a project's label, 0044) renders its function child, here with no provider.
  if (typeof el.props.children === "function") return buttons((el.props.children as (v: ReadonlyMap<string, string>) => ReactNode)(new Map()));
  const words = (n: ReactNode): string => {
    if (Array.isArray(n)) return n.map(words).join(" ");
    if (typeof n === "string" || typeof n === "number") return String(n);
    if (!n || typeof n !== "object" || !("props" in n)) return "";
    const e = n as ReactElement<{ children?: ReactNode }>;
    if (typeof e.props.children === "function") return words((e.props.children as (v: ReadonlyMap<string, string>) => ReactNode)(new Map()));
    return typeof e.type === "function" ? words((e.type as (p: unknown) => ReactNode)(e.props)) : words(e.props.children);
  };
  const own = el.type === "button" ? [{ words: words(el.props.children).replace(/\s+/g, " ").trim(), click: () => el.props.onClick?.() }] : [];
  return [...own, ...buttons(el.props.children)];
}

describe("Recently finished on the board (AC1)", () => {
  it("is docked and labelled, with its window and the kind chips on screen", () => {
    const html = render();
    expect(html).toMatch(/^<aside class="recent" aria-label="Recently finished">/);
    expect(plain(html)).toContain("Recently finished last 7 days");
    expect(html).toContain('role="group" aria-label="Show what finished"');
    expect(html.match(/aria-pressed="true"[^>]*class="ui-pill ui-pill--on">([^<]*)</)?.[1]).toBe("All");
  });
  it("lists merged and accepted rows newest first, grouped by day with counts, each saying when from the log", () => {
    const html = render();
    expect(heads(html)).toEqual(["Today 2", "Yesterday 1", "Sun 11 Oct 2"]);
    expect(rowsOf(html)).toEqual([
      "0050 Recently finished merged 4 min ago built by muster/wt-0051 · main · 531266d4 · time and tokens…",
      "0043 Your check says what to do accepted 4 h ago built by muster/wt-0051 · main · 531266d4 · time and tokens…",
      "0043 Your check says what to do merged yesterday 18:20 built by muster/wt-0051 · main · 531266d4 · time and tokens…",
      "0031 Timeline accepted Sun 11 Oct 09:05 built by muster/wt-0051 · main · 531266d4 · time and tokens…",
      "0031 Timeline merged Sun 11 Oct 09:00 built by muster/wt-0051 · main · 531266d4 · time and tokens…",
    ]);
    expect(html).toContain('<span class="recent-kind merged">merged</span>');
    expect(html).toContain('<span class="recent-kind accepted">accepted</span>');
    expect(html).toContain('<time class="recent-when" title="2026-10-13 13:56" dateTime="2026-10-13T13:56">4 min ago</time>');
  });
  it("says how many older rows there are, with Show older to add a week", () => {
    expect(plain(render())).toContain("Show older · 2 more");
  });
  it("lists the kind App gives it, the remembered chip (App keeps it, so its chip in the row counts the same rows)", () => {
    const html = render({ kind: "accepted" });
    expect(html.match(/aria-pressed="true"[^>]*class="ui-pill ui-pill--on">([^<]*)</)?.[1]).toBe("Accepted");
    expect(rowsOf(html).map((r) => r.split(" ")[0])).toEqual(["0043", "0031"]);
    expect(heads(html)).toEqual(["Today 1", "Sun 11 Oct 1"]);
    const merged = rowsOf(render({ kind: "merged" }));
    expect(merged.map((r) => r.split(" ")[0])).toEqual(["0050", "0043", "0031"]);
    expect(merged.every((r) => r.includes(" merged ") && !r.includes(" accepted "))).toBe(true);
  });
  it("says so when nothing finished in the window, in the chip's words", () => {
    expect(plain(render({ tasks: [task("0060", { stage: "building" })] }))).toContain("Nothing merged or accepted in the last 7 days.");
    expect(plain(render({ tasks: [finished("0050", "2026-10-13 13:56")], kind: "accepted" }))).toContain("Nothing accepted in the last 7 days.");
  });
  it("doesn't offer Show older when there's nothing older", () => {
    expect(plain(render({ tasks: TASKS.slice(0, 3) }))).not.toContain("Show older");
  });
});

describe("a row (AC3)", () => {
  const list = (over: Partial<Parameters<typeof RecentList>[0]> = {}) => {
    const { groups, older } = dayGroups(finishes(TASKS), NOW, 1);
    return RecentList({ groups, older, kind: "all", onKind: () => {}, weeks: 1, onOlder: () => {}, now: NOW, multi: false, openKey: null, onOpen: () => {}, costs: new Map(), onFold: () => {}, ...over });
  };
  it("opens its task", () => {
    const onOpen = vi.fn();
    buttons(list({ onOpen })).find((b) => b.words.startsWith("0043") && b.words.includes("merged"))!.click();
    expect(onOpen).toHaveBeenCalledWith(TASKS[1]);
  });
  it("marks the task open in the pane", () => {
    const html = renderToStaticMarkup(list({ openKey: "muster/0050" }));
    expect(html.match(/class="recent-row active"/g)).toHaveLength(1);
    expect(html).toMatch(/class="recent-row active"[^>]*>[\s\S]*?0050/);
  });
  it("carries its project with All selected", () => {
    const html = renderToStaticMarkup(list({ multi: true }));
    expect(rowsOf(html)[0]).toMatch(/^muster 0050 Recently finished/);
    expect(html).toContain('<span class="q">muster</span>');
    expect(renderToStaticMarkup(list())).not.toContain('class="q"');
  });
  it("mixes projects, newest first, with All selected", () => {
    const other = finished("0007", "2026-10-13 13:59", "", { queue: "drover", title: "Drover's" });
    const html = render({ tasks: [...TASKS, other], multi: true });
    expect(rowsOf(html).slice(0, 2).map((r) => r.split(" ").slice(0, 2).join(" "))).toEqual(["drover 0007", "muster 0050"]);
  });
  it("shows its time and tokens once read, and says when the transcripts aren't on this Mac", () => {
    const costs = new Map([["muster/0050", { minutes: 95, tokens: 1_234_000, read: 2 }], ["muster/0043", { minutes: 0, tokens: 0, read: 0 }]]);
    const rows = rowsOf(renderToStaticMarkup(list({ costs })));
    expect(rows[0]).toContain("built by muster/wt-0051 · main · 531266d4 · 1.6 h · 1.2M tokens");
    expect(rows[1]).toContain("no transcripts on this Mac");
  });
  it("the chips pick the kind and Show older adds a week", () => {
    const onKind = vi.fn(), onOlder = vi.fn();
    const b = buttons(list({ onKind, onOlder }));
    b.find((x) => x.words === "Merged")!.click();
    b.find((x) => x.words.startsWith("Show older"))!.click();
    expect(onKind).toHaveBeenCalledWith("merged");
    expect(onOlder).toHaveBeenCalledOnce();
  });
});

describe("time and tokens", () => {
  it("reads as the Work section's totals, or why not", () => {
    expect(costText(undefined)).toBe("time and tokens…");
    expect(costText({ error: "no such task" })).toBe("time and tokens unknown");
    expect(costText({ minutes: 0, tokens: 0, read: 0 })).toBe("no transcripts on this Mac");
    expect(costText({ minutes: 12, tokens: 450_000, read: 1 })).toBe("12 min · 450k tokens");
    expect(costText({ minutes: 0, tokens: 900, read: 1 })).toBe("0 min · 900 tokens");
    // In hours, never days (mu-172, D4).
    expect(costText({ minutes: 600, tokens: 1_802_000, read: 2 })).toBe("10 h · 1.8M tokens");
    // No agent time from the stage log (never in Building): the tokens alone.
    expect(costText({ minutes: null, tokens: 900, read: 1 })).toBe("900 tokens");
  });
  it("takes the agent time and the counted tokens, as the Work section compares them (mu-172)", () => {
    const none = { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 };
    const totals = { sessions: 2, read: 2, live: 0, shared: 1, tokens: { input: 2000, output: 600_000, cacheRead: 40_000_000, cacheWrite: 1_200_000 }, totalTokens: 41_802_000, minutes: 440, countedTokens: 1_802_000, sharedTokens: none, sharedRead: 1, agentMinutes: 150 };
    const work = { rows: [], stages: [], totals, estimate: { minutes: 480, tokens: 1_500_000, minutesPct: 31, tokensPct: 120 }, guess: { pct: 90, basis: "" } };
    expect(costOf(work)).toEqual({ minutes: 150, tokens: 1_802_000, read: 2 });
    expect(costText(costOf(work))).toBe("2.5 h · 1.8M tokens");
    expect(costOf({ ...work, totals: { ...totals, agentMinutes: null } })).toEqual({ minutes: null, tokens: 1_802_000, read: 2 });
  });
});

describe("Recently finished names tasks by their keys (0033)", () => {
  it("shows mu-50 where the project has a prefix", () => {
    const keyed = TASKS.map((t) => ({ ...t, prefix: "mu" }));
    const rows = rowsOf(render({ tasks: keyed }));
    expect(rows[0]).toMatch(/^mu-50 Recently finished/);
    expect(rows.join(" ")).not.toMatch(/\b0050\b/);
  });
});
