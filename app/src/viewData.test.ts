import { describe, expect, it } from "vitest";
import { stampMinutes } from "./lib";
import {
  axis, barTitle, DAY, layout, legendMarks, nowAt, parseScale, segmentTitle, sortTasks, spans, taskSpan, timeWindow, wallMinutes, windowLabel,
  type Placed, type Scale, type Span,
  parseTableSort,
} from "./viewData";
import type { Task } from "./types";

const t = (over: Partial<Task>): Task => ({
  queue: "q", id: "0001", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [], folder: "todo", path: "", claimedBy: "", heldBy: "", stageSince: "", checkins: [],
  heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "2026-10-11", capturedAt: "", doneAt: "", blockHistory: [], questions: [], openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false,
  tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0,
  level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", search: "", ...over,
});

describe("sortTasks", () => {
  it("sorts by priority rank, stage order and change time", () => {
    const list = [t({ id: "1", priority: "low" }), t({ id: "2", priority: "high" }), t({ id: "3" })];
    expect(sortTasks(list, "priority", false, {}).map((x) => x.id)).toEqual(["2", "3", "1"]);
    expect(sortTasks(list, "priority", true, {}).map((x) => x.id)).toEqual(["1", "3", "2"]);
    const st = [t({ id: "a", stage: "done" }), t({ id: "b", stage: "backlog" }), t({ id: "c", stage: "your-check" }), t({ id: "d", stage: "shaping" }), t({ id: "e", stage: "testing", blocked: true })];
    expect(sortTasks(st, "stage", false, {}).map((x) => x.id)).toEqual(["b", "d", "e", "c", "a"]);
    expect(sortTasks([t({ id: "x", modified: 5 }), t({ id: "y", modified: 9 })], "changed", true, {}).map((x) => x.id)).toEqual(["y", "x"]);
    // "On it / waiting on" sorts by what the cell shows: a Done task's old claim counts as nobody.
    const who = [t({ id: "a", stage: "done", claimedBy: "aaa" }), t({ id: "b", stage: "building", claimedBy: "zed" }), t({ id: "c", claimedBy: "zzz", blockedKind: "tasks", blockedBy: [{ id: "0012", stage: "ready", holder: "" }] })];
    expect(sortTasks(who, "who", false, {}).map((x) => x.id)).toEqual(["a", "c", "b"]);
  });
  it("sorts by estimate as agent time plus test time, then tokens", () => {
    const a = t({ id: "a", estMinutes: 60, estTestMinutes: 0 });
    const b = t({ id: "b", estMinutes: 30, estTestMinutes: 60 });
    const c = t({ id: "c", estMinutes: 60, estTokenCount: 500_000 });
    expect(sortTasks([b, c, a], "estimate", false, {}).map((x) => x.id)).toEqual(["a", "c", "b"]);
    expect(sortTasks([a, b, c], "estimate", true, {}).map((x) => x.id)).toEqual(["b", "c", "a"]);
  });
});

describe("sortTasks by stage for goals and features", () => {
  it("sorts a goal or feature by the stage it shows, In progress with Building", () => {
    const list = [
      t({ id: "b", level: "feature", stage: "ready", shownStage: "done" }),
      t({ id: "d", stage: "building" }),
      t({ id: "c", level: "goal", stage: "ready", shownStage: "in-progress" }),
      t({ id: "a", stage: "ready" }),
    ];
    expect(sortTasks(list, "stage", false, {}).map((x) => x.id)).toEqual(["a", "c", "d", "b"]);
  });
});

const M = (s: string) => stampMinutes(s)!;
/** Midnight starting 2026-10-<d>. */
const D = (d: number) => M(`2026-10-${String(d).padStart(2, "0")} 00:00`);
const step = (at: string, from: string, to: string) => ({ at, who: "agent", from, to });
const segs = (s: Span) => s.segments.map((g) => [g.stage, g.from, g.to, g.fuzzy]);
// Wednesday 2026-10-14, 17:20.
const NOW = M("2026-10-14 17:20");
// Captured that morning, claimed at 13:00, passed to Testing at 15:00, to Your check at 15:30, accepted at 16:00.
const built = t({
  id: "b3", captured: "2026-10-14", claimedOn: "2026-10-14", doneOn: "2026-10-14", stage: "done",
  stageHistory: [
    step("2026-10-14 13:00", "ready", "building"), step("2026-10-14 15:00", "building", "testing"),
    step("2026-10-14 15:30", "testing", "your-check"), step("2026-10-14 16:00", "your-check", "done"),
  ],
});

describe("timeline time", () => {
  it("reads a Date as wall-clock minutes, the same count as the stamp the tools write", () => {
    expect(wallMinutes(new Date(2026, 9, 14, 13, 5))).toBe(M("2026-10-14 13:05"));
    expect(wallMinutes(new Date(2026, 2, 8, 4, 0))).toBe(M("2026-03-08 04:00"));
  });
  it("counts every day as 1440 minutes, across a clock change too", () => {
    // US clocks jump 02:00 → 03:00 on 2026-03-08 and EU ones on 2026-03-29; wall minutes don't.
    expect(M("2026-03-08 04:00") - M("2026-03-08 01:00")).toBe(180);
    expect(M("2026-03-30 00:00") - M("2026-03-29 00:00")).toBe(DAY);
    const w = timeWindow("week", M("2026-03-25 12:00"), 0);
    expect(w).toEqual({ from: M("2026-03-23 00:00"), to: M("2026-03-30 00:00") });
    expect(axis("week", w, NOW).map((k) => k.label)).toEqual(["Mon 23", "Tue 24", "Wed 25", "Thu 26", "Fri 27", "Sat 28", "Sun 29"]);
    const day = timeWindow("day", M("2026-03-08 12:00"), 0);
    expect(axis("day", day, NOW)).toHaveLength(24);
  });
});

describe("a task's work: from when it started to when it finished (owner, 2026-10-13)", () => {
  const span = (x: Task) => taskSpan(x, NOW)!;
  it("starts at the claim, has one segment per stage, and finishes at Done; waiting before isn't drawn", () => {
    const s = span(built);
    expect(segs(s)).toEqual([
      ["building", M("2026-10-14 13:00"), M("2026-10-14 15:00"), false],
      ["testing", M("2026-10-14 15:00"), M("2026-10-14 15:30"), false],
      ["your-check", M("2026-10-14 15:30"), M("2026-10-14 16:00"), false],
    ]);
    expect([s.start, s.end, s.finish, s.open, s.timed]).toEqual([M("2026-10-14 13:00"), M("2026-10-14 16:00"), M("2026-10-14 16:00"), false, true]);
  });
  it("runs a task still being worked to now, unfinished", () => {
    const s = span(t({ captured: "2026-10-14", stage: "testing", stageHistory: [step("2026-10-14 09:00", "ready", "building"), step("2026-10-14 11:00", "building", "testing")] }));
    expect(s.segments.at(-1)).toEqual({ stage: "testing", from: M("2026-10-14 11:00"), to: NOW, fuzzy: false });
    expect([s.start, s.end, s.finish, s.open]).toEqual([M("2026-10-14 09:00"), NOW, NOW, true]);
  });
  it("finishes when it reached Your check, then waits for you as a tail to now", () => {
    const s = span(t({ stage: "your-check", stageHistory: [step("2026-10-14 09:00", "ready", "building"), step("2026-10-14 10:00", "building", "testing"), step("2026-10-14 10:30", "testing", "your-check")] }));
    expect(s.finish).toBe(M("2026-10-14 10:30"));
    expect(s.segments.at(-1)).toEqual({ stage: "your-check", from: M("2026-10-14 10:30"), to: NOW, fuzzy: false });
  });
  it("shows a return to Ready as its own stage, between the two stints", () => {
    const s = span(
      t({
        stage: "building",
        stageHistory: [
          step("2026-10-13 10:00", "backlog", "shaping"), step("2026-10-13 12:00", "shaping", "ready"), step("2026-10-14 09:00", "ready", "building"),
          step("2026-10-14 10:00", "building", "ready"), step("2026-10-14 11:00", "ready", "building"),
        ],
      }),
    );
    expect(s.segments.map((g) => g.stage)).toEqual(["building", "ready", "building"]);
    expect(s.start).toBe(M("2026-10-14 09:00"));
  });
  it("starts a history that begins mid-work (a claim written by hand) on the claim day, marked as a date only", () => {
    // 0014's shape: claimed by hand on the 12th; the first tool line is building → testing.
    const s = span(t({ captured: "2026-10-11", claimedOn: "2026-10-12", stage: "your-check", stageHistory: [step("2026-10-12 18:32", "building", "testing"), step("2026-10-12 18:40", "testing", "your-check")] }));
    expect(segs(s)).toEqual([
      ["building", D(12), M("2026-10-12 18:32"), true],
      ["testing", M("2026-10-12 18:32"), M("2026-10-12 18:40"), false],
      ["your-check", M("2026-10-12 18:40"), NOW, false],
    ]);
    expect(s.finish).toBe(M("2026-10-12 18:40"));
  });
  it("puts a task passed straight to Done at that moment", () => {
    const s = span(t({ stage: "done", stageHistory: [step("2026-10-13 09:00", "ready", "done")] }));
    expect([s.start, s.end, s.finish, s.segments.length]).toEqual([M("2026-10-13 09:00"), M("2026-10-13 09:00"), M("2026-10-13 09:00"), 0]);
  });
  it("orders steps by time and ignores stamps that aren't the tools'", () => {
    const s = span(t({ stage: "testing", stageHistory: [step("2026-10-14 11:00", "building", "testing"), step("yesterday", "testing", "done"), step("2026-10-14 09:00", "ready", "building")] }));
    expect(s.segments.map((g) => g.stage)).toEqual(["building", "testing"]);
    expect(taskSpan(t({ stage: "building", stageHistory: [step("2026-10-14 25:00", "ready", "building")] }), NOW)).toBeNull();
  });
  it("draws a task done before stages were logged from its logged capture to its logged done time, with no segment", () => {
    // No stage was logged, so no stretch of it is drawn in a stage's colour: it's the done bar's
    // outline, as every done task's bar ends (mu-173 R1).
    const s = span(t({ stage: "done", captured: "2026-10-12", capturedAt: "2026-10-12 04:47", doneAt: "2026-10-12 04:55" }));
    expect([s.start, s.end]).toEqual([M("2026-10-12 04:47"), M("2026-10-12 04:55")]);
    expect(segs(s)).toEqual([]);
  });
  it("gives a done-before-stages task captured and done in the same minute a bar a minute long", () => {
    const s = span(t({ stage: "done", captured: "2026-10-12", capturedAt: "2026-10-12 04:47", doneAt: "2026-10-12 04:47" }));
    expect([s.start, s.end, s.segments.length]).toEqual([M("2026-10-12 04:47"), M("2026-10-12 04:48"), 0]);
  });
  it("leaves out work not started, and tasks the log has no times for", () => {
    expect(taskSpan(t({ captured: "2026-10-12", stage: "ready" }), NOW)).toBeNull();
    expect(taskSpan(t({ captured: "2026-10-11", claimedOn: "2026-10-12", stage: "done", doneOn: "2026-10-13" }), NOW)).toBeNull();
    expect(taskSpan(t({ stage: "shaping", stageHistory: [step("2026-10-13 10:00", "backlog", "shaping")] }), NOW)).toBeNull();
  });
  it("sorts by start, then project and id; counts what isn't drawn; and is empty-safe", () => {
    const at = (id: string, h: string) => t({ id, stage: "building", stageHistory: [step(`2026-10-14 ${h}`, "ready", "building")] });
    const list = spans([at("b", "10:00"), at("a", "10:00"), at("c", "08:00"), t({ id: "n", stage: "ready" })], NOW);
    expect(list.map((s) => s.task.id)).toEqual(["c", "a", "b"]);
    expect(list.notStarted).toBe(1);
    expect(spans([], NOW)).toHaveLength(0);
  });
});


describe("timeline windows and axis", () => {
  it("is the last 24 hours at Day (now near the right), a Monday-to-Sunday week and a calendar month, stepping by each", () => {
    // NOW is 14 Oct 17:20: 20 hours before 17:00 to 4 hours after.
    expect(timeWindow("day", NOW, 0)).toEqual({ from: M("2026-10-13 21:00"), to: M("2026-10-14 21:00") });
    expect(timeWindow("day", NOW, -2)).toEqual({ from: M("2026-10-11 21:00"), to: M("2026-10-12 21:00") });
    // Just after midnight it still shows the day that just ended.
    expect(timeWindow("day", M("2026-10-14 00:20"), 0)).toEqual({ from: M("2026-10-13 04:00"), to: M("2026-10-14 04:00") });
    expect(timeWindow("week", NOW, 0)).toEqual({ from: D(12), to: D(19) });
    expect(timeWindow("week", M("2026-10-18 23:59"), 0)).toEqual({ from: D(12), to: D(19) });
    expect(timeWindow("week", NOW, 1)).toEqual({ from: D(19), to: D(26) });
    expect(timeWindow("month", NOW, 0)).toEqual({ from: M("2026-10-01 00:00"), to: M("2026-11-01 00:00") });
    expect(timeWindow("month", NOW, -10)).toEqual({ from: M("2025-12-01 00:00"), to: M("2026-01-01 00:00") });
    expect(timeWindow("month", NOW, 3)).toEqual({ from: M("2027-01-01 00:00"), to: M("2027-02-01 00:00") });
  });
  it("names the window", () => {
    expect(windowLabel("day", timeWindow("day", NOW, 0))).toBe("Tue 13 Oct 21:00 – Wed 14 Oct 21:00");
    expect(windowLabel("week", timeWindow("week", NOW, 0))).toBe("12 – 18 Oct 2026");
    expect(windowLabel("week", timeWindow("week", M("2026-10-01 12:00"), 0))).toBe("28 Sep – 4 Oct 2026");
    expect(windowLabel("week", timeWindow("week", M("2026-01-01 12:00"), 0))).toBe("29 Dec 2025 – 4 Jan 2026");
    expect(windowLabel("month", timeWindow("month", NOW, 0))).toBe("October 2026");
  });
  it("labels hours at Day, weekdays at Week and dates at Month, marking now", () => {
    const day = axis("day", timeWindow("day", NOW, 0), NOW);
    // The clock hour, and the weekday where a new day starts.
    expect(day.map((k) => k.label)).toEqual(["21", "22", "23", "Wed", ...Array.from({ length: 20 }, (_, h) => String(h + 1).padStart(2, "0"))]);
    expect(day.filter((k) => k.current).map((k) => k.label)).toEqual(["17"]);
    expect([day[13].left, day[13].width]).toEqual([13 / 24, 1 / 24]);
    const week = axis("week", timeWindow("week", NOW, 0), NOW);
    expect(week.map((k) => k.label)).toEqual(["Mon 12", "Tue 13", "Wed 14", "Thu 15", "Fri 16", "Sat 17", "Sun 18"]);
    expect(week.filter((k) => k.current).map((k) => k.label)).toEqual(["Wed 14"]);
    const month = axis("month", timeWindow("month", NOW, 0), NOW);
    expect(month).toHaveLength(31);
    expect([month[0].label, month[30].label, month.find((k) => k.current)?.label]).toEqual(["1", "31", "14"]);
    expect(axis("day", timeWindow("day", NOW, -1), NOW).some((k) => k.current)).toBe(false);
  });
  it("puts the now line where now falls, and nowhere in a window without it", () => {
    expect(nowAt(timeWindow("day", NOW, 0), NOW)).toBeCloseTo((20 * 60 + 20) / DAY);
    expect(nowAt(timeWindow("week", NOW, 0), NOW)).toBeCloseTo((2 * DAY + 17 * 60 + 20) / (7 * DAY));
    expect(nowAt(timeWindow("day", NOW, 1), NOW)).toBeNull();
    expect(nowAt(timeWindow("day", NOW, -1), NOW)).toBeNull();
  });
  it("reads a saved scale, Week for anything else", () => {
    expect((["day", "week", "month", "year", "", null] as (string | null)[]).map((v) => parseScale(v))).toEqual(["day", "week", "month", "week", "week", "week"]);
  });
  it("titles a segment with its stage and times, and dates only when that's all there is", () => {
    expect(segmentTitle({ stage: "building", from: M("2026-10-14 13:00"), to: M("2026-10-14 15:00"), fuzzy: false })).toBe("Building 14 Oct 13:00 – 15:00");
    expect(segmentTitle({ stage: "your-check", from: M("2026-10-13 22:00"), to: M("2026-10-14 01:30"), fuzzy: false })).toBe("Your check 13 Oct 22:00 – 14 Oct 01:30");
    expect(segmentTitle({ stage: "waiting", from: D(11), to: M("2026-10-14 13:00"), fuzzy: true })).toBe("Waiting 11 Oct – 14 Oct 13:00");
    expect(segmentTitle({ stage: "done", from: D(12), to: D(14), fuzzy: true })).toBe("Done 12 Oct – 13 Oct");
  });
});

describe("timeline layout", () => {
  // Layout is tested on a fixed calendar day at Day (the window's own rules are tested apart).
  const CAL_DAY = { from: D(14), to: D(15) };
  const win = (scale: Scale) => (scale === "day" ? CAL_DAY : timeWindow(scale, NOW, 0));
  const place = (scale: Scale, list: Task[] = [built]) => layout(spans(list, NOW), win(scale), scale);
  /** Where a bar's working part starts and how wide it is, as fractions of the window. */
  const worked = (p: Placed) => {
    const w = p.segments.filter((g) => g.seg.stage !== "waiting");
    const a = w[0].left, b = w[w.length - 1].left + w[w.length - 1].width;
    return [p.left + a * p.width, (b - a) * p.width];
  };
  it("draws a task built in 3 hours as 3 hours at Day, each stage its own segment", () => {
    const [p] = place("day");
    const [at, wide] = worked(p);
    expect(at).toBeCloseTo(13 / 24);
    expect(wide).toBeCloseTo(3 / 24);
    expect(p.left).toBeCloseTo(13 / 24);
    expect(p.width).toBeCloseTo(3 / 24);
    expect(p.segments.map((g) => g.seg.stage)).toEqual(["building", "testing", "your-check"]);
    expect(p.segments[0].width * p.width).toBeCloseTo(2 / 24);
    expect(p.segments[1].width * p.width).toBeCloseTo(0.5 / 24);
  });
  it("draws it as one quarter-day slot at Week", () => {
    const [p] = place("week");
    const [at, wide] = worked(p);
    expect(at).toBeCloseTo(10 / 28);
    expect(wide).toBeCloseTo(1 / 28);
    expect(p.left).toBeCloseTo(10 / 28);
    expect(p.width).toBeCloseTo(1 / 28);
  });
  it("draws it as one day at Month", () => {
    const [p] = place("month");
    const [at, wide] = worked(p);
    expect(at).toBeCloseTo(13 / 31);
    expect(wide).toBeCloseTo(1 / 31);
  });
  it("shows the same tasks at each scale when they fall inside every window", () => {
    const list = [built, t({ id: "x", captured: "2026-10-14", stage: "building", stageHistory: [step("2026-10-14 08:00", "ready", "building")] })];
    // x started at 08:00, b3 at 13:00: earliest start first.
    for (const s of ["day", "week", "month"] as Scale[]) expect(place(s, list).map((p) => p.span.task.id)).toEqual(["x", "b3"]);
  });
  it("leaves out tasks outside the window and clips the ones that run across its edge", () => {
    const list = [
      t({ id: "old", stage: "done", stageHistory: [step("2026-10-08 10:00", "ready", "building"), step("2026-10-09 10:00", "building", "done")] }),
      t({ id: "edge", stage: "done", stageHistory: [step("2026-10-11 09:00", "ready", "building"), step("2026-10-11 23:00", "building", "done")] }),
      t({ id: "later", captured: "2026-10-20", stage: "ready" }),
      t({ id: "long", stage: "building", stageHistory: [step("2026-10-01 09:00", "ready", "building")] }),
    ];
    const out = place("week", list);
    expect(out.map((p) => p.span.task.id)).toEqual(["long"]);
    expect([out[0].left, out[0].clipStart, out[0].clipEnd]).toEqual([0, true, false]);
    // Monday 00:00 to Wednesday 18:00, the end of the slot now is in.
    expect(out[0].width).toBeCloseTo(11 / 28);
    expect(out[0].segments.map((g) => [g.seg.stage, g.left, g.width])).toEqual([["building", 0, 1]]);
    expect(place("day", list).map((p) => p.span.task.id)).toEqual(["long"]);
    expect(layout(spans(list, NOW), timeWindow("day", NOW, 1), "day")).toEqual([]);
  });
  it("keeps a sliver of a few minutes visible, inside the window", () => {
    const tail = t({ id: "tail", captured: "2026-10-13", stage: "done", stageHistory: [step("2026-10-13 22:00", "ready", "building"), step("2026-10-14 00:05", "building", "done")] });
    const [p] = place("day", [tail]);
    expect([p.left, p.width, p.clipStart]).toEqual([0, 15 / DAY, true]);
    const late: Span = { task: t({ id: "late" }), start: D(14) + 1435, work: D(14) + 1435, end: D(14) + 1438, open: true, timed: true, segments: [{ stage: "building", from: D(14) + 1435, to: D(14) + 1438, fuzzy: false }], blocks: [], pauses: [], finish: D(14) + 1438 };
    const [q] = layout([late], CAL_DAY, "day");
    expect(q.left + q.width).toBeCloseTo(1);
    expect(q.width).toBeCloseTo(15 / DAY);
  });
  it("builds and lays out 500 tasks well within a frame (best of 5, so a busy machine can't fail the gate)", () => {
    const many = Array.from({ length: 500 }, (_, i) =>
      t({
        id: String(i).padStart(4, "0"), captured: `2026-10-0${1 + (i % 9)}`, stage: i % 3 ? "done" : "building",
        stageHistory: [
          step(`2026-10-1${2 + (i % 3)} 0${i % 10}:00`, "ready", "building"), step(`2026-10-1${2 + (i % 3)} 1${i % 10}:00`, "building", "testing"),
          ...(i % 3 ? [step(`2026-10-1${2 + (i % 3)} 1${i % 10}:30`, "testing", "your-check"), step(`2026-10-1${2 + (i % 3)} 2${i % 4}:00`, "your-check", "done")] : []),
        ],
      }),
    );
    const run = () => layout(spans(many, NOW), timeWindow("week", NOW, 0), "week");
    let best = Infinity, out = run();
    for (let i = 0; i < 5; i++) {
      const started = performance.now();
      out = run();
      best = Math.min(best, performance.now() - started);
    }
    expect(out).toHaveLength(500);
    expect(best).toBeLessThan(16);
  });
});

describe("blocked stretches (0031)", () => {
  const building = (blockHistory: Task["blockHistory"], blocked = false) =>
    t({ captured: "2026-10-14", capturedAt: "2026-10-14 08:00", stage: "building", blocked, blockHistory, stageHistory: [step("2026-10-14 09:00", "ready", "building")] });
  it("keeps a block that ended, clipped to the bar, with its kind (0024)", () => {
    const s = taskSpan(building([{ from: "2026-10-14 10:00", to: "2026-10-14 14:00", kind: "permission" }]), NOW)!;
    expect(s.blocks).toEqual([{ from: stampMinutes("2026-10-14 10:00"), to: stampMinutes("2026-10-14 14:00"), kind: "permission" }]);
  });
  it("runs a block that's still on to the end of the bar", () => {
    const s = taskSpan(building([{ from: "2026-10-14 16:00", to: "" }], true), NOW)!;
    expect(s.blocks).toEqual([{ from: stampMinutes("2026-10-14 16:00"), to: s.end, kind: "" }]);
  });
  it("labels each placed block with its kind's words", () => {
    const [p] = layout([taskSpan(building([{ from: "2026-10-14 10:00", to: "2026-10-14 14:00", kind: "tasks" }]), NOW)!], timeWindow("day", NOW, 0), "day");
    expect(p.blocks[0].kind).toBe("tasks");
  });
  it("says why it's blocked in the bar's tooltip", () => {
    const base = { captured: "2026-10-14", stage: "building", claimedOn: "2026-10-14" };
    expect(barTitle(t({ ...base, blocked: true, blockedKind: "you", blockDetail: "pick a colour" }), true)).toBe(
      "Building · Your action: pick a colour · captured 2026-10-14 · started 2026-10-14",
    );
    expect(barTitle(t({ ...base, blockedKind: "tasks", blockedBy: [{ id: "0014", stage: "testing", holder: "qa" }] }), false)).toBe(
      "Building · After 0014 (Testing, qa) · captured 2026-10-14 · started 2026-10-14 · dates only",
    );
    expect(barTitle(t({ ...base, doneOn: "2026-10-15", stage: "done" }), true)).toBe("Done · captured 2026-10-14 · started 2026-10-14 · done 2026-10-15");
  });
  it("drops blocks outside the bar and bad stamps", () => {
    const s = taskSpan(building([{ from: "2026-10-01 10:00", to: "2026-10-02 10:00" }, { from: "nope", to: "" }]), NOW)!;
    expect(s.blocks).toEqual([]);
  });
  it("places them on the bar at every scale, within it", () => {
    for (const scale of ["day", "week", "month"] as const) {
      const [p] = layout([taskSpan(building([{ from: "2026-10-14 10:00", to: "2026-10-14 14:00" }]), NOW)!], timeWindow(scale, NOW, 0), scale);
      expect(p.blocks).toHaveLength(1);
      const b = p.blocks[0];
      expect(b.left).toBeGreaterThan(0);
      expect(b.left + b.width).toBeLessThanOrEqual(1);
    }
  });
});

describe("paused stretches (0045)", () => {
  const building = (pauseHistory: Task["pauseHistory"], paused = "") =>
    t({ captured: "2026-10-14", capturedAt: "2026-10-14 08:00", stage: "building", paused, pauseHistory, stageHistory: [step("2026-10-14 09:00", "ready", "building")] });
  it("keeps a pause that ended, and runs one still on to the end of the bar", () => {
    expect(taskSpan(building([{ from: "2026-10-14 10:00", to: "2026-10-14 12:00" }]), NOW)!.pauses).toEqual([
      { from: stampMinutes("2026-10-14 10:00"), to: stampMinutes("2026-10-14 12:00") },
    ]);
    const s = taskSpan(building([{ from: "2026-10-14 16:00", to: "" }], "2026-10-14 16:00"), NOW)!;
    expect(s.pauses).toEqual([{ from: stampMinutes("2026-10-14 16:00"), to: s.end }]);
  });
  it("drops pauses outside the bar and bad stamps; none without a history", () => {
    expect(taskSpan(building([{ from: "2026-10-01 10:00", to: "2026-10-02 10:00" }, { from: "nope", to: "" }]), NOW)!.pauses).toEqual([]);
    expect(taskSpan(building(undefined), NOW)!.pauses).toEqual([]);
  });
  it("places them on the bar at every scale, within it", () => {
    for (const scale of ["day", "week", "month"] as const) {
      const [p] = layout([taskSpan(building([{ from: "2026-10-14 10:00", to: "2026-10-14 14:00" }]), NOW)!], timeWindow(scale, NOW, 0), scale);
      expect(p.pauses).toHaveLength(1);
      expect(p.pauses[0].left).toBeGreaterThan(0);
      expect(p.pauses[0].left + p.pauses[0].width).toBeLessThanOrEqual(1);
    }
  });
  it("says it's paused in the bar's tooltip", () => {
    expect(barTitle(t({ captured: "2026-10-14", stage: "building", paused: "2026-10-14 10:00", pausedWhy: "later" }), true)).toBe(
      "Building · paused: later · captured 2026-10-14",
    );
  });
});

describe("a done task from before stage history (owner's screenshot, 2026-10-13)", () => {
  // 0011: captured 04:47, moved to done 04:55, no stage lines.
  const old = (doneAt: string) => t({ captured: "2026-10-12", capturedAt: "2026-10-12 04:47", doneOn: "2026-10-12", doneAt, stage: "done", stageHistory: [] });
  it("ends at the logged done time", () => {
    const s = taskSpan(old("2026-10-12 04:55"), NOW)!;
    expect([s.start, s.end]).toEqual([M("2026-10-12 04:47"), M("2026-10-12 04:55")]);
  });
  it("without one, isn't drawn: its times aren't known", () => {
    expect(taskSpan(old(""), NOW)).toBeNull();
  });
});

describe("the Table remembers its sort (owner, 2026-10-13)", () => {
  it("reads back what was saved, and the default for anything else", () => {
    expect(parseTableSort(JSON.stringify({ by: "priority", desc: true }))).toEqual({ by: "priority", desc: true });
    expect(parseTableSort(JSON.stringify({ by: "captured" }))).toEqual({ by: "captured", desc: false });
    for (const bad of [null, "", "not json", JSON.stringify({ by: "colour", desc: true }), JSON.stringify([1, 2]), "null"])
      expect(parseTableSort(bad), String(bad)).toEqual({ by: "stage", desc: false });
  });
});

describe("the timeline's legend lists what the window shows (mu-173)", () => {
  const day = timeWindow("day", NOW, 0);
  const marks = (list: Task[], w = day, scale: Scale = "day", now: number | null = null) => legendMarks(layout(spans(list, NOW), w, scale), now);
  const building = (over: Partial<Task>) => t({ stage: "building", stageHistory: [step("2026-10-14 09:00", "ready", "building")], ...over });
  it("names each stage a bar draws, the wait for your check, a done bar and now, in a fixed order", () => {
    expect(marks([built], day, "day", nowAt(day, NOW))).toEqual(["building", "testing", "your-check", "done", "now"]);
    // Sent back to Ready and claimed again: Ready comes before Building however the bar runs.
    const returned = t({ stage: "building", stageHistory: [step("2026-10-14 10:00", "ready", "building"), step("2026-10-14 11:00", "building", "ready"), step("2026-10-14 12:00", "ready", "building")] });
    expect(marks([returned])).toEqual(["ready", "building"]);
  });
  it("lists each mark once, however many bars show it", () => {
    expect(marks([built, { ...built, id: "b4" }, { ...built, id: "b5" }])).toEqual(["building", "testing", "your-check", "done"]);
  });
  it("names done for a done task from before stage history, whose bar is the done outline with no segment (R1)", () => {
    const old = t({ captured: "2026-10-14", capturedAt: "2026-10-14 04:47", doneAt: "2026-10-14 04:55", stage: "done" });
    const placed = layout(spans([old], NOW), day, "day");
    expect(placed.map((p) => p.segments.length)).toEqual([0]);
    expect(marks([old])).toEqual(["done"]);
  });
  it("names a start known only to the day", () => {
    // Claimed by hand on the 14th; the log starts mid-work, at its move to Testing.
    const handClaimed = t({ claimedOn: "2026-10-14", stage: "testing", stageHistory: [step("2026-10-14 13:00", "building", "testing")] });
    expect(marks([handClaimed])).toEqual(["building", "testing", "fuzzy"]);
    expect(marks([building({})])).toEqual(["building"]);
  });
  it("names blocked for a stretch blocked in the window, or a bar drawn blocked", () => {
    expect(marks([building({ blockHistory: [{ from: "2026-10-14 10:00", to: "2026-10-14 12:00" }] })])).toEqual(["building", "blocked"]);
    expect(marks([building({ blocked: true })])).toEqual(["building", "blocked"]);
    // Waiting only on other tasks isn't drawn blocked (0060), so it isn't named.
    expect(marks([building({ blockedKind: "tasks", blockedBy: [{ id: "0014", stage: "ready", holder: "" }] })])).toEqual(["building"]);
  });
  it("names paused for a paused stretch", () => {
    expect(marks([building({ pauseHistory: [{ from: "2026-10-14 10:00", to: "2026-10-14 12:00" }] })])).toEqual(["building", "paused"]);
  });
  it("names waiting for a waiting stretch", () => {
    const at = M("2026-10-14 08:00"), work = M("2026-10-14 10:00"), end = M("2026-10-14 12:00");
    const span: Span = {
      task: t({ id: "w", stage: "done" }), start: at, work, end, open: false, timed: true, blocks: [], pauses: [], finish: end,
      segments: [{ stage: "waiting", from: at, to: work, fuzzy: false }, { stage: "building", from: work, to: end, fuzzy: false }],
    };
    expect(legendMarks(layout([span], day, "day"), null)).toEqual(["building", "done", "waiting"]);
  });
  it("leaves out what only shows outside the window", () => {
    // Building since the 1st, blocked on the 2nd and paused on the 3rd: this week shows neither.
    const long = building({
      stageHistory: [step("2026-10-01 09:00", "ready", "building")],
      blockHistory: [{ from: "2026-10-02 10:00", to: "2026-10-02 12:00" }],
      pauseHistory: [{ from: "2026-10-03 10:00", to: "2026-10-03 12:00" }],
    });
    const old = t({ id: "old", stage: "done", stageHistory: [step("2026-10-08 10:00", "ready", "building"), step("2026-10-09 10:00", "building", "done")] });
    const week = timeWindow("week", NOW, 0), month = timeWindow("month", NOW, 0);
    expect(marks([long, old], week, "week")).toEqual(["building"]);
    expect(marks([long, old], month, "month")).toEqual(["building", "done", "blocked", "paused"]);
  });
  it("skips a stretch with no width", () => {
    const [p] = layout(spans([built], NOW), day, "day");
    const flat: Placed = { ...p, segments: p.segments.map((g) => ({ ...g, width: g.seg.stage === "testing" ? 0 : g.width })), blocks: [{ left: 0.5, width: 0, kind: "" }], pauses: [{ left: 0.5, width: 0 }] };
    expect(legendMarks([flat], null)).toEqual(["building", "your-check", "done"]);
  });
  it("names only now when no bar shows, and nothing when now is outside the window too", () => {
    expect(legendMarks([], null)).toEqual([]);
    expect(legendMarks([], 0.5)).toEqual(["now"]);
    expect(legendMarks([], 0)).toEqual(["now"]);
  });
});
