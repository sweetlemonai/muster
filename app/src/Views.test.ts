import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { createElement as h, type ReactElement, type ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import { stampMinutes } from "./lib";
import type { Task } from "./types";
import { axis, layout, nowAt, spans, timeWindow, type Scale } from "./viewData";
import { TimelineChart, TimelineControls } from "./Views";

const t = (over: Partial<Task>): Task => ({
  queue: "q", id: "0001", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [], folder: "todo", path: "", claimedBy: "",
  heartbeat: "", waitingOn: "", commit: "", captured: "2026-10-14", capturedAt: "", blockHistory: [], questions: [], openQuestions: 0, ownerCriteria: [], waitingForOwner: false,
  tags: [], estTime: "", estTokens: "", estMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});
const step = (at: string, from: string, to: string) => ({ at, who: "agent", from, to });
const NOW = stampMinutes("2026-10-14 17:20")!;

const controls = (scale: Scale, offset = 0) =>
  renderToStaticMarkup(createElement(TimelineControls, { scale, offset, label: "12 – 18 Oct 2026", onScale: () => {}, onStep: () => {}, onToday: () => {} }));

describe("the timeline's scale chips", () => {
  it("shows Day, Week and Month as a row of chips, the current one pressed, not a dropdown", () => {
    const chips = [...controls("week").matchAll(/<button class="chip( on)?" aria-pressed="(true|false)">(Day|Week|Month)<\/button>/g)];
    expect(chips.map((m) => `${m[3]}:${m[2]}`)).toEqual(["Day:false", "Week:true", "Month:false"]);
    expect(controls("week")).not.toContain("<select");
  });
  it("steps back and forward, goes back to the current period, and names the window", () => {
    const words = (html: string) => [...html.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1]).slice(3);
    expect(words(controls("day"))).toEqual(["◀", "Today", "▶"]);
    expect(words(controls("week"))).toEqual(["◀", "This week", "▶"]);
    expect(words(controls("month"))).toEqual(["◀", "This month", "▶"]);
    expect(controls("day")).toMatch(/disabled=""[^>]*>Today</);
    expect(controls("day", -1)).not.toContain("disabled");
    expect(controls("week")).toContain("12 – 18 Oct 2026");
  });
});

describe("the timeline chart", () => {
  const tasks = [
    t({
      id: "0031", title: "Timeline scales", stage: "done", doneOn: "2026-10-14",
      stageHistory: [step("2026-10-14 13:00", "ready", "building"), step("2026-10-14 15:00", "building", "testing"), step("2026-10-14 16:00", "testing", "done")],
    }),
    t({ id: "0032", title: "Still going", stage: "building", blocked: true, stageHistory: [step("2026-10-14 16:30", "ready", "building")] }),
  ];
  const chart = (scale: Scale, offset = 0) => {
    const win = timeWindow(scale, NOW, offset);
    return renderToStaticMarkup(
      createElement(TimelineChart, { scale, rows: layout(spans(tasks, NOW), win, scale), ticks: axis(scale, win, NOW), now: nowAt(win, NOW), multi: false, openKey: "q/0032", onOpen: () => {} }),
    );
  };
  const ticks = (html: string) => [...html.matchAll(/<span class="tl-tick( now)?"[^>]*>([^<]*)<\/span>/g)].map((m) => `${m[2]}${m[1] ? "*" : ""}`);
  it("labels the axis in hours at Day, weekdays at Week and dates at Month, marking now", () => {
    expect(ticks(chart("day"))).toEqual(Array.from({ length: 24 }, (_, h) => `${String(h).padStart(2, "0")}${h === 17 ? "*" : ""}`));
    expect(ticks(chart("week"))).toEqual(["Mon 12", "Tue 13", "Wed 14*", "Thu 15", "Fri 16", "Sat 17", "Sun 18"]);
    const month = ticks(chart("month"));
    expect([month.length, month[0], month[13], month[30]]).toEqual([31, "1", "14*", "31"]);
  });
  it("draws each stage as its own segment in its colour, waiting in the quiet tint", () => {
    const html = chart("day");
    const bar = /<span class="tl-bar s-done[^"]*"[^>]*>([\s\S]*?)<span class="tl-bar-text">/.exec(html)?.[1] ?? "";
    expect([...bar.matchAll(/class="tl-seg ([^"]+)"/g)].map((m) => m[1])).toEqual(["waiting fuzzy", "s-building", "s-testing"]);
    expect(bar).toContain('title="Building 14 Oct 13:00 – 15:00"');
    expect(html).toMatch(/class="tl-bar s-building open blocked"/);
  });
  it("labels each row with its id and title, and marks the open task", () => {
    const html = chart("week");
    expect(html).toMatch(/<button class="tl-row"[^>]*><span class="tl-label"> ?<span class="id">0031<\/span> Timeline scales<\/span>/);
    expect(html).toMatch(/<button class="tl-row active"[^>]*><span class="tl-label"> ?<span class="id">0032<\/span> Still going/);
  });
  it("draws the now line where now is, and none in a window without it", () => {
    expect(chart("day")).toMatch(new RegExp(`class="tl-now" style="left:${(((17 * 60 + 20) / 1440) * 100).toFixed(3)}%"`));
    expect(chart("day", -1)).not.toContain('class="tl-now"');
    expect(chart("day", -1)).not.toContain("tl-row");
  });
});

/** Every <button> in an element tree a hook-free component returned, with its words. */
function buttons(node: ReactNode): { words: string; click: () => void }[] {
  if (node === null || node === undefined || typeof node === "boolean" || typeof node === "string" || typeof node === "number") return [];
  if (Array.isArray(node)) return node.flatMap(buttons);
  const el = node as ReactElement<{ children?: ReactNode; onClick?: () => void }>;
  if (typeof el.type === "function") return buttons((el.type as (p: unknown) => ReactNode)(el.props));
  const words = (n: ReactNode): string => (Array.isArray(n) ? n.map(words).join("") : typeof n === "string" || typeof n === "number" ? String(n) : n && typeof n === "object" && "props" in n ? words((n as ReactElement<{ children?: ReactNode }>).props.children) : "");
  const own = el.type === "button" ? [{ words: words(el.props.children).trim(), click: () => el.props.onClick?.() }] : [];
  return [...own, ...buttons(el.props.children)];
}

describe("the timeline's controls do what they say", () => {
  it("◀ steps back, ▶ forward, the middle one goes to the current period, a chip picks the scale", () => {
    const onScale = vi.fn(), onStep = vi.fn(), onToday = vi.fn();
    const b = buttons(TimelineControls({ scale: "week", offset: -2, label: "x", onScale, onStep, onToday }));
    const press = (w: string) => b.find((x) => x.words === w)!.click();
    press("◀");
    press("▶");
    press("This week");
    press("Month");
    expect(onStep.mock.calls).toEqual([[-1], [1]]);
    expect(onToday).toHaveBeenCalledOnce();
    expect(onScale).toHaveBeenCalledWith("month");
  });
  it("clicking a task's row opens it", () => {
    const onOpen = vi.fn();
    const task = t({ id: "0042", title: "Open me", stage: "building", stageHistory: [step("2026-10-14 09:00", "ready", "building")] });
    const win = timeWindow("day", NOW, 0);
    const rows = layout(spans([task], NOW), win, "day");
    const b = buttons(h(TimelineChart, { scale: "day", rows, ticks: axis("day", win, NOW), now: nowAt(win, NOW), multi: false, openKey: null, onOpen }));
    b.find((x) => x.words.includes("Open me"))!.click();
    expect(onOpen).toHaveBeenCalledWith(task);
  });
  it("draws a block that ended as a hatched stretch on the bar", () => {
    const task = t({ stage: "building", blockHistory: [{ from: "2026-10-14 10:00", to: "2026-10-14 12:00" }], stageHistory: [step("2026-10-14 09:00", "ready", "building")] });
    const win = timeWindow("day", NOW, 0);
    const html = renderToStaticMarkup(h(TimelineChart, { scale: "day", rows: layout(spans([task], NOW), win, "day"), ticks: axis("day", win, NOW), now: nowAt(win, NOW), multi: false, openKey: null, onOpen: () => {} }));
    expect(html.match(/class="tl-block"/g)).toHaveLength(1);
  });
});
