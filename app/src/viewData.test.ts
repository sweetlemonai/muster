import { describe, expect, it } from "vitest";
import { stampMinutes } from "./lib";
import {
  axis, DAY, layout, nowAt, parseScale, segmentTitle, sortTasks, spans, taskSpan, timeWindow, wallMinutes, windowLabel,
  type Placed, type Scale, type Span,
} from "./viewData";
import type { Task } from "./types";

const t = (over: Partial<Task>): Task => ({
  queue: "q", id: "0001", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [], folder: "todo", path: "", claimedBy: "",
  heartbeat: "", waitingOn: "", depends: [], commit: "", captured: "2026-10-11", capturedAt: "", blockHistory: [], questions: [], openQuestions: 0, ownerCriteria: [], waitingForOwner: false,
  tags: [], estTime: "", estTokens: "", estMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", attachments: [], logTail: [], modified: 0,
  level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});

describe("sortTasks", () => {
  it("sorts by priority rank, stage order and change time", () => {
    const list = [t({ id: "1", priority: "low" }), t({ id: "2", priority: "high" }), t({ id: "3" })];
    expect(sortTasks(list, "priority", false).map((x) => x.id)).toEqual(["2", "3", "1"]);
    expect(sortTasks(list, "priority", true).map((x) => x.id)).toEqual(["1", "3", "2"]);
    const st = [t({ id: "a", stage: "done" }), t({ id: "b", stage: "backlog" }), t({ id: "c", stage: "your-check" }), t({ id: "d", stage: "shaping" }), t({ id: "e", stage: "testing", blocked: true })];
    expect(sortTasks(st, "stage", false).map((x) => x.id)).toEqual(["b", "d", "e", "c", "a"]);
    expect(sortTasks([t({ id: "x", modified: 5 }), t({ id: "y", modified: 9 })], "changed", true).map((x) => x.id)).toEqual(["y", "x"]);
    // "On it / waiting on" sorts by what the cell shows: a Done task's old claim counts as nobody.
    const who = [t({ id: "a", stage: "done", claimedBy: "aaa" }), t({ id: "b", stage: "building", claimedBy: "zed" }), t({ id: "c", blocked: true, waitingOn: "0012" })];
    expect(sortTasks(who, "who", false).map((x) => x.id)).toEqual(["a", "c", "b"]);
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
    expect(sortTasks(list, "stage", false).map((x) => x.id)).toEqual(["a", "c", "d", "b"]);
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

describe("a task's segments", () => {
  it("waits from capture to the claim, then has one segment per stage from the stage history", () => {
    const s = taskSpan(built, NOW);
    expect(segs(s)).toEqual([
      ["waiting", D(14), M("2026-10-14 13:00"), true],
      ["building", M("2026-10-14 13:00"), M("2026-10-14 15:00"), false],
      ["testing", M("2026-10-14 15:00"), M("2026-10-14 15:30"), false],
      ["your-check", M("2026-10-14 15:30"), M("2026-10-14 16:00"), false],
    ]);
    expect([s.start, s.work, s.end, s.open, s.timed]).toEqual([D(14), M("2026-10-14 13:00"), M("2026-10-14 16:00"), false, true]);
  });
  it("runs a task still in progress to now", () => {
    const s = taskSpan(t({ captured: "2026-10-14", stage: "testing", stageHistory: [step("2026-10-14 09:00", "ready", "building"), step("2026-10-14 11:00", "building", "testing")] }), NOW);
    expect(s.segments.at(-1)).toEqual({ stage: "testing", from: M("2026-10-14 11:00"), to: NOW, fuzzy: false });
    expect([s.end, s.open]).toEqual([NOW, true]);
  });
  it("keeps Backlog, Shaping and Ready in the waiting tint until the claim, and shows a return as its own stage", () => {
    const s = taskSpan(
      t({
        captured: "2026-10-13", stage: "building",
        stageHistory: [
          step("2026-10-13 10:00", "backlog", "shaping"), step("2026-10-13 12:00", "shaping", "ready"), step("2026-10-14 09:00", "ready", "building"),
          step("2026-10-14 10:00", "building", "ready"), step("2026-10-14 11:00", "ready", "building"),
        ],
      }),
      NOW,
    );
    expect(s.segments.map((g) => g.stage)).toEqual(["waiting", "building", "ready", "building"]);
    expect(s.work).toBe(M("2026-10-14 09:00"));
    expect(s.segments[0]).toEqual({ stage: "waiting", from: D(13), to: M("2026-10-14 09:00"), fuzzy: true });
  });
  it("starts a history that begins mid-work (a claim written by hand) on the claim day, marked as a date only", () => {
    // 0014's shape: claimed by hand on the 12th; the first tool line is building → testing.
    const s = taskSpan(
      t({ captured: "2026-10-11", claimedOn: "2026-10-12", stage: "your-check", stageHistory: [step("2026-10-12 18:32", "building", "testing"), step("2026-10-12 18:40", "testing", "your-check")] }),
      NOW,
    );
    expect(segs(s)).toEqual([
      ["waiting", D(11), D(12), true],
      ["building", D(12), M("2026-10-12 18:32"), true],
      ["testing", M("2026-10-12 18:32"), M("2026-10-12 18:40"), false],
      ["your-check", M("2026-10-12 18:40"), NOW, false],
    ]);
  });
  it("ends a task that went straight to Done without a working segment", () => {
    const s = taskSpan(t({ captured: "2026-10-13", stage: "done", stageHistory: [step("2026-10-13 09:00", "ready", "done")] }), NOW);
    expect(segs(s)).toEqual([["waiting", D(13), M("2026-10-13 09:00"), true]]);
    expect([s.work, s.end]).toEqual([s.end, M("2026-10-13 09:00")]);
  });
  it("orders steps by time and ignores stamps that aren't the tools'", () => {
    const s = taskSpan(
      t({ captured: "2026-10-14", stage: "testing", stageHistory: [step("2026-10-14 11:00", "building", "testing"), step("yesterday", "testing", "done"), step("2026-10-14 09:00", "ready", "building")] }),
      NOW,
    );
    expect(s.segments.map((g) => g.stage)).toEqual(["waiting", "building", "testing"]);
    // Nothing usable left: whole days, like an old task.
    expect(taskSpan(t({ captured: "2026-10-14", stage: "building", stageHistory: [step("2026-10-14 25:00", "ready", "building")] }), NOW).timed).toBe(false);
  });
  it("falls back to whole days for a task with dates only", () => {
    const s = taskSpan(t({ captured: "2026-10-11", claimedOn: "2026-10-12", stage: "done", doneOn: "2026-10-13" }), NOW);
    expect(segs(s)).toEqual([["waiting", D(11), D(12), true], ["done", D(12), D(14), true]]);
    expect([s.timed, s.start, s.end, s.open]).toEqual([false, D(11), D(14), false]);
    // Open: runs to now in its stage's colour; still waiting with an old claim outside the bar: all waiting.
    expect(segs(taskSpan(t({ captured: "2026-10-12", stage: "your-check" }), NOW))).toEqual([["your-check", D(12), NOW, true]]);
    expect(segs(taskSpan(t({ captured: "2026-10-12", stage: "ready", claimedOn: "2020-01-01" }), NOW))).toEqual([["waiting", D(12), NOW, true]]);
  });
  it("uses the file's change day when the capture date is missing", () => {
    const modified = new Date(2026, 9, 13, 9, 30).getTime() / 1000;
    const s = taskSpan(t({ captured: "", capturedAt: "", blockHistory: [], modified, stage: "done" }), NOW);
    expect([s.start, s.end]).toEqual([D(13), D(14)]);
  });
  it("sorts by start, then project and id, and is empty-safe", () => {
    const list = spans([t({ id: "b", captured: "2026-10-12" }), t({ id: "a", captured: "2026-10-12" }), t({ id: "c", captured: "2026-10-10" })], NOW);
    expect(list.map((s) => s.task.id)).toEqual(["c", "a", "b"]);
    expect(spans([], NOW)).toEqual([]);
  });
});

describe("timeline windows and axis", () => {
  it("is one day at Day, a Monday-to-Sunday week and a calendar month, stepping by each", () => {
    expect(timeWindow("day", NOW, 0)).toEqual({ from: D(14), to: D(15) });
    expect(timeWindow("day", NOW, -2)).toEqual({ from: D(12), to: D(13) });
    expect(timeWindow("week", NOW, 0)).toEqual({ from: D(12), to: D(19) });
    expect(timeWindow("week", M("2026-10-18 23:59"), 0)).toEqual({ from: D(12), to: D(19) });
    expect(timeWindow("week", NOW, 1)).toEqual({ from: D(19), to: D(26) });
    expect(timeWindow("month", NOW, 0)).toEqual({ from: M("2026-10-01 00:00"), to: M("2026-11-01 00:00") });
    expect(timeWindow("month", NOW, -10)).toEqual({ from: M("2025-12-01 00:00"), to: M("2026-01-01 00:00") });
    expect(timeWindow("month", NOW, 3)).toEqual({ from: M("2027-01-01 00:00"), to: M("2027-02-01 00:00") });
  });
  it("names the window", () => {
    expect(windowLabel("day", timeWindow("day", NOW, 0))).toBe("Wed 14 Oct 2026");
    expect(windowLabel("week", timeWindow("week", NOW, 0))).toBe("12 – 18 Oct 2026");
    expect(windowLabel("week", timeWindow("week", M("2026-10-01 12:00"), 0))).toBe("28 Sep – 4 Oct 2026");
    expect(windowLabel("week", timeWindow("week", M("2026-01-01 12:00"), 0))).toBe("29 Dec 2025 – 4 Jan 2026");
    expect(windowLabel("month", timeWindow("month", NOW, 0))).toBe("October 2026");
  });
  it("labels hours at Day, weekdays at Week and dates at Month, marking now", () => {
    const day = axis("day", timeWindow("day", NOW, 0), NOW);
    expect(day.map((k) => k.label)).toEqual(Array.from({ length: 24 }, (_, h) => String(h).padStart(2, "0")));
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
    expect(nowAt(timeWindow("day", NOW, 0), NOW)).toBeCloseTo((17 * 60 + 20) / DAY);
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
  const place = (scale: Scale, list: Task[] = [built]) => layout(spans(list, NOW), timeWindow(scale, NOW, 0), scale);
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
    expect([p.left, p.width]).toEqual([0, 16 / 24]);
    expect(p.segments.map((g) => g.seg.stage)).toEqual(["waiting", "building", "testing", "your-check"]);
    expect(p.segments[1].width * p.width).toBeCloseTo(2 / 24);
    expect(p.segments[2].width * p.width).toBeCloseTo(0.5 / 24);
  });
  it("draws it as one quarter-day slot at Week", () => {
    const [p] = place("week");
    const [at, wide] = worked(p);
    expect(at).toBeCloseTo(10 / 28);
    expect(wide).toBeCloseTo(1 / 28);
    expect(p.left).toBeCloseTo(8 / 28);
    expect(p.width).toBeCloseTo(3 / 28);
  });
  it("draws it as one day at Month", () => {
    const [p] = place("month");
    const [at, wide] = worked(p);
    expect(at).toBeCloseTo(13 / 31);
    expect(wide).toBeCloseTo(1 / 31);
  });
  it("shows the same tasks at each scale when they fall inside every window", () => {
    const list = [built, t({ id: "x", captured: "2026-10-14", stage: "building", stageHistory: [step("2026-10-14 08:00", "ready", "building")] })];
    for (const s of ["day", "week", "month"] as Scale[]) expect(place(s, list).map((p) => p.span.task.id)).toEqual(["b3", "x"]);
  });
  it("leaves out tasks outside the window and clips the ones that run across its edge", () => {
    const list = [
      t({ id: "old", captured: "2026-10-01", stage: "done", doneOn: "2026-10-09" }),
      t({ id: "edge", captured: "2026-10-11", stage: "done", doneOn: "2026-10-11" }),
      t({ id: "later", captured: "2026-10-20", stage: "ready" }),
      t({ id: "long", captured: "2026-09-30", stage: "building", claimedOn: "2026-10-01" }),
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
    const late: Span = { task: t({ id: "late" }), start: D(14) + 1435, work: D(14) + 1435, end: D(14) + 1438, open: true, timed: true, segments: [{ stage: "building", from: D(14) + 1435, to: D(14) + 1438, fuzzy: false }], blocks: [] };
    const [q] = layout([late], timeWindow("day", NOW, 0), "day");
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

describe("captured to the minute (0031)", () => {
  it("starts the waiting part at the logged capture time, not midnight, and not fuzzy", () => {
    const s = taskSpan(t({ captured: "2026-10-14", capturedAt: "2026-10-14 15:04", stage: "testing", stageHistory: [step("2026-10-14 16:00", "ready", "building"), step("2026-10-14 17:30", "building", "testing")] }), NOW);
    expect(s.start).toBe(stampMinutes("2026-10-14 15:04"));
    expect(s.segments[0]).toMatchObject({ stage: "waiting", from: stampMinutes("2026-10-14 15:04"), to: stampMinutes("2026-10-14 16:00"), fuzzy: false });
  });
  it("still starts at the day when only the date is known", () => {
    const s = taskSpan(t({ captured: "2026-10-14", capturedAt: "", blockHistory: [], stage: "building", stageHistory: [step("2026-10-14 16:00", "ready", "building")] }), NOW);
    expect(s.segments[0]).toMatchObject({ stage: "waiting", fuzzy: true });
    expect(s.start).toBe(stampMinutes("2026-10-14 00:00"));
  });
});

describe("blocked stretches (0031)", () => {
  const building = (blockHistory: Task["blockHistory"], blocked = false) =>
    t({ captured: "2026-10-14", capturedAt: "2026-10-14 08:00", stage: "building", blocked, blockHistory, stageHistory: [step("2026-10-14 09:00", "ready", "building")] });
  it("keeps a block that ended, clipped to the bar", () => {
    const s = taskSpan(building([{ from: "2026-10-14 10:00", to: "2026-10-14 14:00" }]), NOW);
    expect(s.blocks).toEqual([{ from: stampMinutes("2026-10-14 10:00"), to: stampMinutes("2026-10-14 14:00") }]);
  });
  it("runs a block that's still on to the end of the bar", () => {
    const s = taskSpan(building([{ from: "2026-10-14 16:00", to: "" }], true), NOW);
    expect(s.blocks).toEqual([{ from: stampMinutes("2026-10-14 16:00"), to: s.end }]);
  });
  it("drops blocks outside the bar and bad stamps", () => {
    const s = taskSpan(building([{ from: "2026-10-01 10:00", to: "2026-10-02 10:00" }, { from: "nope", to: "" }]), NOW);
    expect(s.blocks).toEqual([]);
  });
  it("places them on the bar at every scale, within it", () => {
    for (const scale of ["day", "week", "month"] as const) {
      const [p] = layout([taskSpan(building([{ from: "2026-10-14 10:00", to: "2026-10-14 14:00" }]), NOW)], timeWindow(scale, NOW, 0), scale);
      expect(p.blocks).toHaveLength(1);
      const b = p.blocks[0];
      expect(b.left).toBeGreaterThan(0);
      expect(b.left + b.width).toBeLessThanOrEqual(1);
    }
  });
});
