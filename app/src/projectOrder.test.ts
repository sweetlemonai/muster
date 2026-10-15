import { describe, expect, it } from "vitest";
import { accepts, dropMove, foldHeading, grouped, headingCounts, countsText, headingFolds, headingRanks, needsOf, placeSteps, readBack, readFolds, runMove, settleFolds, trackFold, trackUnfold, unfoldFor, type Layout, layoutOf, orderHeadings, type Pending, planMove, settle, shownLayout, sidebarGroups, track } from "./projectOrder";
import { byCategory } from "./labels";

// The sidebar's own arrangement (0057): headings and projects as the owner puts them, by drag or
// ⌥↑/⌥↓, and what the core is asked to keep.

const LAYOUT: Layout = [
  { name: "Apps", keys: ["/a", "/b", "/c"] },
  { name: "Tools", keys: ["/t"] },
  { name: "Other", keys: ["/o1", "/o2"] },
];
const names: Record<string, string> = { "/a": "Alpha", "/b": "Beta", "/c": "Gamma", "/t": "Tee", "/o1": "One", "/o2": "Two", "/n": "New" };
const label = (k: string) => names[k] ?? k;
const shape = (l: Layout) => l.map((g) => `${g.name}: ${g.keys.join(" ")}`);

describe("headings in the owner's order (0057)", () => {
  const row = (key: string, category: string, categoryRank: number | null = null) => ({ key, category, categoryRank });
  it("put the placed headings first, by place, then the rest A–Z with Other last", () => {
    const rows = [row("/a", "Apps"), row("/t", "Tools", 1), row("/o", "Other", 0), row("/z", "Zed"), row("/b", "Books")];
    expect(sidebarGroups(rows, []).map((g) => g.name)).toEqual(["Other", "Tools", "Apps", "Books", "Zed"]);
    expect([...headingRanks(rows)]).toEqual([["Tools", 1], ["Other", 0]]);
    // Nothing placed: as before 0057, A–Z with Other last.
    expect(sidebarGroups([row("/o", "Other"), row("/z", "Zed"), row("/a", "Apps")], []).map((g) => g.name)).toEqual(["Apps", "Zed", "Other"]);
  });
  it("keep each heading's rows in the order given", () => {
    const rows = [row("/2", "Apps"), row("/1", "Apps"), row("/t", "Tools", 0)];
    expect(sidebarGroups(rows, []).map((g) => [g.name, g.rows.map((r) => r.key)])).toEqual([["Tools", ["/t"]], ["Apps", ["/2", "/1"]]]);
  });
  it("add a new, still empty heading last, once, and never over one that has projects", () => {
    const rows = [row("/a", "Apps")];
    expect(sidebarGroups(rows, ["Later", "Apps", "Later"]).map((g) => [g.name, g.rows.length])).toEqual([["Apps", 1], ["Later", 0]]);
  });
  it("orderHeadings leaves byCategory's order for headings without a place", () => {
    const groups = byCategory([{ category: "b" }, { category: "a" }]);
    expect(orderHeadings(groups, new Map()).map((g) => g.name)).toEqual(["a", "b"]);
    expect(orderHeadings(groups, new Map([["b", 3]])).map((g) => g.name)).toEqual(["b", "a"]);
  });
});

describe("rows regrouped as a move says, while it's saved (0057)", () => {
  const rows = [{ key: "/a", category: "Apps" }, { key: "/b", category: "Apps" }, { key: "/t", category: "Tools" }, { key: "/x", category: "Extra" }].map((r) => ({ ...r, categoryRank: null }));
  it("in the move's headings and order; a row it doesn't have under its own heading, at the end", () => {
    const g = grouped(rows, [{ name: "Tools", keys: ["/b", "/t"] }, { name: "Apps", keys: ["/a", "/gone"] }]);
    expect(g.map((x) => [x.name, x.rows.map((r) => r.key)])).toEqual([["Tools", ["/b", "/t"]], ["Apps", ["/a"]], ["Extra", ["/x"]]]);
  });
  it("leaves out a heading the move emptied", () => {
    expect(grouped(rows.slice(0, 2), [{ name: "Apps", keys: [] }, { name: "Tools", keys: ["/a", "/b"] }]).map((x) => x.name)).toEqual(["Tools"]);
  });
  it("layoutOf reads a layout back from groups", () => {
    expect(layoutOf(sidebarGroups(rows, []))).toEqual([
      { name: "Apps", keys: ["/a", "/b"] },
      { name: "Extra", keys: ["/x"] },
      { name: "Tools", keys: ["/t"] },
    ]);
  });
});

describe("a project dragged (0057)", () => {
  it("before or after another row, in its heading: the order to keep, and Mine is chosen", () => {
    const plan = planMove(LAYOUT, { kind: "project", key: "/c", to: { before: "/a" } }, label)!;
    expect(shape(plan.layout)).toEqual(["Apps: /c /a /b", "Tools: /t", "Other: /o1 /o2"]);
    expect(plan.order).toEqual(["/c", "/a", "/b", "/t", "/o1", "/o2"]);
    expect(plan.categories).toEqual(["Apps", "Tools", "Other"]);
    expect(plan.moved).toBeNull();
    expect(plan.chip).toBe("mine");
    expect(plan.said).toBe("Gamma moved to 1 of 3 in Apps");
    expect(shape(planMove(LAYOUT, { kind: "project", key: "/a", to: { after: "/b" } }, label)!.layout)[0]).toBe("Apps: /b /a /c");
  });
  it("onto another heading's row or the heading itself: moved under it, the emptied heading gone", () => {
    const plan = planMove(LAYOUT, { kind: "project", key: "/t", to: { after: "/o1" } }, label)!;
    expect(shape(plan.layout)).toEqual(["Apps: /a /b /c", "Other: /o1 /t /o2"]);
    expect(plan.moved).toEqual({ key: "/t", category: "Other" });
    expect(plan.categories).toEqual(["Apps", "Other"]);
    expect(plan.said).toBe("Tee moved to 2 of 3 in Other");
    const into = planMove(LAYOUT, { kind: "project", key: "/b", to: { into: "Tools" } }, label)!;
    expect(shape(into.layout)).toEqual(["Apps: /a /c", "Tools: /b /t", "Other: /o1 /o2"]);
    expect(into.moved).toEqual({ key: "/b", category: "Tools" });
  });
  it("onto a new, empty heading: it has its first project, and is kept last", () => {
    const withNew = [...LAYOUT, { name: "Later", keys: [] }];
    const plan = planMove(withNew, { kind: "project", key: "/a", to: { into: "Later" } }, label)!;
    expect(shape(plan.layout)).toEqual(["Apps: /b /c", "Tools: /t", "Other: /o1 /o2", "Later: /a"]);
    expect(plan.categories).toEqual(["Apps", "Tools", "Other", "Later"]);
    expect(plan.moved).toEqual({ key: "/a", category: "Later" });
    // A heading the layout doesn't have yet is made, last.
    expect(shape(planMove(LAYOUT, { kind: "project", key: "/a", to: { into: "Fresh" } }, label)!.layout).at(-1)).toBe("Fresh: /a");
  });
  it("changes nothing dropped on itself, back in its own place, or for a project it doesn't know", () => {
    expect(planMove(LAYOUT, { kind: "project", key: "/b", to: { before: "/b" } }, label)).toBeNull();
    expect(planMove(LAYOUT, { kind: "project", key: "/b", to: { after: "/a" } }, label)).toBeNull();
    expect(planMove(LAYOUT, { kind: "project", key: "/a", to: { into: "Apps" } }, label)).toBeNull();
    expect(planMove(LAYOUT, { kind: "project", key: "/nope", to: { before: "/a" } }, label)).toBeNull();
    expect(planMove(LAYOUT, { kind: "project", key: "/a", to: { before: "/nope" } }, label)).toBeNull();
  });
  it("leaves the layout it was given alone", () => {
    const copy = JSON.stringify(LAYOUT);
    planMove(LAYOUT, { kind: "project", key: "/t", to: { into: "Apps" } }, label);
    expect(JSON.stringify(LAYOUT)).toBe(copy);
  });
});

describe("a heading dragged, its projects with it (0057)", () => {
  it("before another heading, or last", () => {
    const plan = planMove(LAYOUT, { kind: "heading", name: "Other", before: "Apps" }, label)!;
    expect(shape(plan.layout)).toEqual(["Other: /o1 /o2", "Apps: /a /b /c", "Tools: /t"]);
    expect(plan.order).toEqual(["/o1", "/o2", "/a", "/b", "/c", "/t"]);
    expect(plan.categories).toEqual(["Other", "Apps", "Tools"]);
    expect(plan.moved).toBeNull();
    expect(plan.chip).toBe("mine");
    expect(plan.said).toBe("Other moved to 1 of 3");
    expect(planMove(LAYOUT, { kind: "heading", name: "Apps", before: null }, label)!.categories).toEqual(["Tools", "Other", "Apps"]);
  });
  it("changes nothing in its own place, or for a heading it doesn't know", () => {
    expect(planMove(LAYOUT, { kind: "heading", name: "Tools", before: "Other" }, label)).toBeNull();
    expect(planMove(LAYOUT, { kind: "heading", name: "Apps", before: "Apps" }, label)).toBeNull();
    expect(planMove(LAYOUT, { kind: "heading", name: "Other", before: null }, label)).toBeNull();
    expect(planMove(LAYOUT, { kind: "heading", name: "Nope", before: "Apps" }, label)).toBeNull();
  });
});

describe("⌥↑ and ⌥↓ (0057)", () => {
  const step = (key: string, by: -1 | 1) => planMove(LAYOUT, { kind: "step-project", key, by }, label);
  it("move a project one place in its heading", () => {
    expect(shape(step("/b", -1)!.layout)[0]).toBe("Apps: /b /a /c");
    expect(shape(step("/b", 1)!.layout)[0]).toBe("Apps: /a /c /b");
    expect(step("/b", 1)!.chip).toBe("mine");
    expect(step("/b", 1)!.moved).toBeNull();
  });
  it("past its heading's edge: to the end of the heading above, or the top of the one below", () => {
    const up = step("/t", -1)!;
    expect(shape(up.layout)).toEqual(["Apps: /a /b /c /t", "Other: /o1 /o2"]);
    expect(up.moved).toEqual({ key: "/t", category: "Apps" });
    expect(up.said).toBe("Tee moved to 4 of 4 in Apps");
    const down = step("/c", 1)!;
    expect(shape(down.layout)).toEqual(["Apps: /a /b", "Tools: /c /t", "Other: /o1 /o2"]);
    expect(down.moved).toEqual({ key: "/c", category: "Tools" });
  });
  it("do nothing at the top of the first heading or the bottom of the last", () => {
    expect(step("/a", -1)).toBeNull();
    expect(step("/o2", 1)).toBeNull();
  });
  it("go into a new, empty heading below the last", () => {
    const withNew = [...LAYOUT, { name: "Later", keys: [] }];
    const plan = planMove(withNew, { kind: "step-project", key: "/o2", by: 1 }, label)!;
    expect(shape(plan.layout).slice(-2)).toEqual(["Other: /o1", "Later: /o2"]);
    expect(plan.moved).toEqual({ key: "/o2", category: "Later" });
  });
  it("move a heading past the one above or below, and do nothing at the ends", () => {
    const up = planMove(LAYOUT, { kind: "step-heading", name: "Tools", by: -1 }, label)!;
    expect(up.categories).toEqual(["Tools", "Apps", "Other"]);
    expect(up.order).toEqual(["/t", "/a", "/b", "/c", "/o1", "/o2"]);
    expect(planMove(LAYOUT, { kind: "step-heading", name: "Tools", by: 1 }, label)!.categories).toEqual(["Apps", "Other", "Tools"]);
    expect(planMove(LAYOUT, { kind: "step-heading", name: "Apps", by: -1 }, label)).toBeNull();
    expect(planMove(LAYOUT, { kind: "step-heading", name: "Other", by: 1 }, label)).toBeNull();
  });
  it("don't count a new, empty heading among the ones kept", () => {
    const withNew: Layout = [{ name: "Later", keys: [] }, ...LAYOUT];
    expect(planMove(withNew, { kind: "step-project", key: "/a", by: 1 }, label)!.categories).toEqual(["Apps", "Tools", "Other"]);
  });
});

describe("where a drop lands (0057 review R1)", () => {
  const project = { kind: "project", key: "/c" } as const;
  const heading = { kind: "heading", name: "Other" } as const;
  const row = (key: string) => ({ kind: "row", key }) as const;
  const head = (name: string) => ({ kind: "heading", name }) as const;
  const group = (name: string) => ({ kind: "group", name }) as const;
  it("a row takes a project, a heading takes both, a group around them takes a heading", () => {
    expect([accepts(project, row("/a")), accepts(project, head("Apps")), accepts(project, group("Apps"))]).toEqual([true, true, false]);
    expect([accepts(heading, row("/a")), accepts(heading, head("Apps")), accepts(heading, group("Apps"))]).toEqual([false, true, true]);
    expect(accepts(null, head("Apps"))).toBe(false);
  });
  it("a heading dropped on another group's row: the row lets it go on, and the group takes it", () => {
    // The pointer is over Apps' row /a, inside the Apps group: the drop bubbles from the row.
    expect(accepts(heading, row("/a"))).toBe(false);
    expect(accepts(heading, group("Apps"))).toBe(true);
    expect(dropMove(LAYOUT, heading, row("/a"), "before")).toBeNull();
    const before = dropMove(LAYOUT, heading, group("Apps"), "before")!;
    expect(before).toEqual({ kind: "heading", name: "Other", before: "Apps" });
    expect(planMove(LAYOUT, before, label)!.categories).toEqual(["Other", "Apps", "Tools"]);
    // On the lower half: after Apps, before the heading below it.
    expect(dropMove(LAYOUT, heading, group("Apps"), "after")).toEqual({ kind: "heading", name: "Other", before: "Tools" });
    expect(dropMove(LAYOUT, { kind: "heading", name: "Apps" }, group("Other"), "after")).toEqual({ kind: "heading", name: "Apps", before: null });
  });
  it("a project lands before or after the row, at the top of a heading, and nowhere else", () => {
    expect(dropMove(LAYOUT, project, row("/a"), "before")).toEqual({ kind: "project", key: "/c", to: { before: "/a" } });
    expect(dropMove(LAYOUT, project, row("/t"), "after")).toEqual({ kind: "project", key: "/c", to: { after: "/t" } });
    expect(dropMove(LAYOUT, project, head("Tools"), "after")).toEqual({ kind: "project", key: "/c", to: { into: "Tools" } });
    expect(dropMove(LAYOUT, project, group("Tools"), "before")).toBeNull();
    expect(dropMove(LAYOUT, heading, row("/a"), "before")).toBeNull();
    expect(dropMove(LAYOUT, null, row("/a"), "before")).toBeNull();
  });
  it("a heading dropped on a heading goes before or after it", () => {
    expect(dropMove(LAYOUT, heading, head("Tools"), "before")).toEqual({ kind: "heading", name: "Other", before: "Tools" });
    expect(dropMove(LAYOUT, heading, head("Apps"), "after")).toEqual({ kind: "heading", name: "Other", before: "Tools" });
  });
});

describe("a move shown before the core has it (0057 review R3)", () => {
  const moved: Layout = [{ name: "Apps", keys: ["/b", "/a", "/c"] }];
  const core: Layout = [{ name: "Apps", keys: ["/a", "/b", "/c"] }];
  it("stays while the save is out, whatever the queues say", () => {
    const p = { layout: moved, saved: false, reads: 0 };
    expect(settle(p, core)).toBe(p);
  });
  it("goes once the queues say the same, or at the second read after the save whatever they say", () => {
    expect(settle({ layout: moved, saved: true, reads: 0 }, moved)).toBeNull();
    expect(settle({ layout: moved, saved: false, reads: 0 }, moved)).toBeNull();
    const once = settle({ layout: moved, saved: true, reads: 0 }, core);
    expect(once).toEqual({ layout: moved, saved: true, reads: 1 });
    expect(settle(once, core)).toBeNull();
    expect(settle(null, core)).toBeNull();
  });
  it("a second move plans from it: two quick clicks both count", () => {
    const three: Layout = [{ name: "Apps", keys: ["/a", "/b", "/c"] }];
    const first = planMove(three, { kind: "step-project", key: "/a", by: 1 }, label)!;
    expect(first.order).toEqual(["/b", "/a", "/c"]);
    // Before the queues are read again, the next click plans from what the first one showed.
    const shown = shownLayout({ layout: first.layout, saved: false, reads: 0 }, three);
    const second = planMove(shown, { kind: "step-project", key: "/c", by: -1 }, label)!;
    expect(second.order).toEqual(["/b", "/c", "/a"]);
    expect(shownLayout(null, three)).toBe(three);
  });
});

describe("a move's bookkeeping, shared by the sidebar and Settings (0057 review R6)", () => {
  // The component's state, as React's functional setter keeps it.
  const holder = () => {
    let state: Pending | null = null;
    return { set: (f: (p: Pending | null) => Pending | null) => (state = f(state)), get: () => state };
  };
  const three: Layout = [{ name: "Apps", keys: ["/a", "/b", "/c"] }];
  const deferred = () => {
    let done!: () => void, fail!: (e: unknown) => void;
    const promise = new Promise<void>((res, rej) => ((done = res), (fail = rej)));
    return { promise, done, fail };
  };
  it("shows the move at once, and marks it saved when the core has it", async () => {
    const s = holder();
    const plan = planMove(three, { kind: "step-project", key: "/a", by: 1 }, label)!;
    const save = deferred();
    const out = track(plan, s.set, () => save.promise);
    expect(s.get()).toEqual({ layout: plan.layout, saved: false, reads: 0 });
    save.done();
    await out;
    expect(s.get()).toEqual({ layout: plan.layout, saved: true, reads: 0 });
    // Then the queues read back take over.
    expect(settle(s.get(), plan.layout)).toBeNull();
  });
  it("drops it, and passes the refusal on, when the core refuses", async () => {
    const s = holder();
    const plan = planMove(three, { kind: "step-project", key: "/a", by: 1 }, label)!;
    await expect(track(plan, s.set, () => Promise.reject("no such project"))).rejects.toBe("no such project");
    expect(s.get()).toBeNull();
  });
  it("two quick clicks: the second plans from the first, and the first's late answer leaves the second alone", async () => {
    const s = holder();
    const first = planMove(three, { kind: "step-project", key: "/a", by: 1 }, label)!;
    const one = deferred();
    const a = track(first, s.set, () => one.promise);
    const second = planMove(shownLayout(s.get(), three), { kind: "step-project", key: "/c", by: -1 }, label)!;
    expect(second.order).toEqual(["/b", "/c", "/a"]);
    const two = deferred();
    const b = track(second, s.set, () => two.promise);
    one.done();
    await a;
    expect(s.get()).toEqual({ layout: second.layout, saved: false, reads: 0 });
    // A refused first one doesn't drop the second either.
    const s2 = holder();
    const late = track(first, s2.set, () => Promise.reject("x"));
    void track(second, s2.set, () => new Promise<void>(() => {}));
    await late.catch(() => {});
    expect(s2.get()?.layout).toBe(second.layout);
    two.done();
    await b;
    expect(s.get()).toEqual({ layout: second.layout, saved: true, reads: 0 });
  });
});

describe("folded headings (0070)", () => {
  const row = (name: string, category: string, over: Partial<{ categoryFolded: boolean; needs: number; lessons: number }> = {}) => ({ name, category, categoryFolded: false, needs: 0, lessons: 0, ...over });
  it("are the headings the core says are folded, with a fold set here winning until the core agrees", () => {
    const rows = [row("a", "Apps", { categoryFolded: true }), row("b", "Apps", { categoryFolded: true }), row("t", "Tools")];
    expect([...headingFolds(rows, {})]).toEqual([["Apps", true], ["Tools", false]]);
    expect([...headingFolds(rows, { Tools: true, Apps: false })]).toEqual([["Apps", false], ["Tools", true]]);
    // Once the core says the same, the one set here goes.
    expect(settleFolds({ Tools: true, Apps: false }, headingFolds(rows, {}))).toEqual({ Tools: true, Apps: false });
    expect(settleFolds({ Tools: false, Apps: true }, headingFolds(rows, {}))).toEqual({});
  });
  it("count, for a heading, what needs you first, then its projects and what's being worked on, in words (0082)", () => {
    const busy = (name: string, needs: number, lessons: number, working: number) => ({ ...row(name, "Apps", { needs, lessons }), activity: { working, stalled: false, last: null }, tasks: [] });
    // Lessons waiting need you too, as the row's badge counts them (0070 review R2).
    const c = headingCounts([busy("a", 2, 1, 0), busy("b", 0, 0, 3), busy("c", 0, 0, 0)]);
    expect(c).toEqual({ needs: 3, projects: 3, working: 3 });
    expect(countsText(c)).toBe("3 need you · 3 projects · 3 working");
    expect(countsText({ needs: 1, projects: 1, working: 1 })).toBe("1 needs you · 1 project · 1 working");
    // What's 0 isn't said, but a heading always says how many projects it holds.
    expect(countsText({ needs: 0, projects: 4, working: 0 })).toBe("4 projects");
    expect(countsText({ needs: 77, projects: 17, working: 18 })).toBe("77 need you · 17 projects · 18 working");
    expect([0, 2].map((lessons) => needsOf({ needs: 1, lessons }))).toEqual([1, 3]);
  });
  it("a move under a folded heading opens it; one under an open heading, or a reorder, opens nothing (review R4)", () => {
    const folds = new Map([["Apps", true], ["Tools", false]]);
    const plan = (moved: { key: string; category: string } | null) => ({ layout: [], order: [], categories: [], moved, chip: "mine" as const, said: "" });
    expect(unfoldFor(plan({ key: "/t", category: "Apps" }), folds)).toBe("Apps");
    expect(unfoldFor(plan({ key: "/a", category: "Tools" }), folds)).toBeNull();
    expect(unfoldFor(plan({ key: "/a", category: "New" }), folds)).toBeNull();
    expect(unfoldFor(plan(null), folds)).toBeNull();
  });
  // The component's fold state, as React's functional setter keeps it.
  const holder = (start: Record<string, boolean> = {}) => {
    let state = start;
    return { set: (f: (s: Record<string, boolean>) => Record<string, boolean>) => (state = f(state)), get: () => state };
  };
  it("a fold shows at once and stays while the core keeps it (review R5)", async () => {
    const s = holder();
    let done!: () => void;
    const out = trackFold("Apps", true, s.set, () => new Promise<void>((res) => (done = res)));
    expect(s.get()).toEqual({ Apps: true });
    done();
    await out;
    expect(s.get()).toEqual({ Apps: true });
  });
  it("a fold the core refuses is taken back, and the refusal passed on; a later one is left alone (review R5)", async () => {
    const s = holder({ Tools: false });
    await expect(trackFold("Apps", true, s.set, () => Promise.reject("no category named \"Apps\""))).rejects.toBe("no category named \"Apps\"");
    expect(s.get()).toEqual({ Tools: false });
    // Folded, then unfolded before the first answer came back refused: the unfold stays.
    const t = holder();
    let fail!: (e: unknown) => void;
    const first = trackFold("Apps", true, t.set, () => new Promise<void>((_, rej) => (fail = rej)));
    void trackFold("Apps", false, t.set, () => new Promise<void>(() => {}));
    fail("x");
    await first.catch(() => {});
    expect(t.get()).toEqual({ Apps: false });
  });
  it("a move's unfold shows at once, stays when the core keeps the move, and is taken back when it refuses (review R11)", async () => {
    const s = holder({ Apps: true });
    await trackUnfold("Apps", s.set, Promise.resolve());
    expect(s.get()).toEqual({ Apps: false });
    const t = holder({ Apps: true });
    await expect(trackUnfold("Apps", t.set, Promise.reject("no such project"))).rejects.toBe("no such project");
    expect(t.get()).toEqual({ Apps: true });
    // Folded again meanwhile: that later fold stays.
    const u = holder();
    let fail!: (e: unknown) => void;
    const out = trackUnfold("Apps", u.set, new Promise<void>((_, rej) => (fail = rej)));
    u.set((x) => ({ ...x, Apps: true }));
    fail("x");
    await out.catch(() => {});
    expect(u.get()).toEqual({ Apps: true });
    // Nothing to open: the move's answer passes as it is.
    const v = holder({ Tools: true });
    await trackUnfold(null, v.set, Promise.resolve());
    expect(v.get()).toEqual({ Tools: true });
  });
  it("each read of the queues settles the folds set here, keeping the same object when nothing went (review R5)", () => {
    const set = { Apps: true, Tools: false };
    expect(readFolds(set, new Map([["Apps", false], ["Tools", true]]))).toBe(set);
    expect(readFolds(set, new Map([["Apps", true], ["Tools", true]]))).toEqual({ Tools: false });
    expect(readFolds(set, new Map([["Apps", true], ["Tools", false]]))).toEqual({});
  });
});

describe("the window's entry points, wired through (mu-88)", () => {
  // Each component's state, as React's functional setter keeps it, and what it says.
  const state = <T,>(start: T) => {
    let s = start;
    return { set: (f: (x: T) => T) => (s = f(s)), get: () => s };
  };
  const ui = () => {
    const out = { said: "", problem: "" };
    return { out, say: (t: string) => (out.said = t), problem: (t: string) => (out.problem = t) };
  };
  const three: Layout = [{ name: "Apps", keys: ["/a", "/b", "/c"] }];

  it("Settings' ↑ shows at once and a second ↑ before the core answers plans from the first (0059 R3/R5)", async () => {
    const pending = state<Pending | null>(null);
    const sent: string[][] = [];
    const save = (p: { order: string[] }) => (sent.push(p.order), new Promise<void>(() => {}));
    const first = placeSteps(pending.get(), three, "/c", label, pending.set, save);
    expect(first.down).toBeNull();
    void first.up!();
    expect(shape(pending.get()!.layout)).toEqual(["Apps: /a /c /b"]);
    // The core hasn't read back yet: the next ↑ plans from what's shown, not from the core.
    void placeSteps(pending.get(), three, "/c", label, pending.set, save).up!();
    expect(sent).toEqual([["/a", "/c", "/b"], ["/c", "/a", "/b"]]);
    expect(shape(pending.get()!.layout)).toEqual(["Apps: /c /a /b"]);
    // At the top it can't go up; a refused save takes the move back and passes the refusal on.
    expect(placeSteps(pending.get(), three, "/c", label, pending.set, save).up).toBeNull();
    const fresh = state<Pending | null>(null);
    await expect(placeSteps(null, three, "/a", label, fresh.set, () => Promise.reject("no")).down!()).rejects.toBe("no");
    expect(fresh.get()).toBeNull();
  });

  it("a fold says so, shows at once and stays when the core keeps it (0070 R5)", async () => {
    const folds = state<Record<string, boolean>>({});
    const u = ui();
    let done!: () => void;
    const out = foldHeading("Tools", true, { setFolds: folds.set, say: u.say, problem: u.problem }, () => new Promise<void>((res) => (done = res)));
    expect(folds.get()).toEqual({ Tools: true });
    expect(u.out).toEqual({ said: "Tools folded", problem: "" });
    done();
    await out;
    expect(folds.get()).toEqual({ Tools: true });
    expect(u.out.problem).toBe("");
  });

  it("a fold the core refuses is put back and the refusal shown in the problem line (0070 R4/R5)", async () => {
    const folds = state<Record<string, boolean>>({ Apps: false });
    const u = ui();
    await foldHeading("Tools", false, { setFolds: folds.set, say: u.say, problem: u.problem }, () => Promise.reject("no category named \"Tools\""));
    expect(folds.get()).toEqual({ Apps: false });
    expect(u.out).toEqual({ said: "Tools unfolded", problem: "Couldn't unfold Tools: no category named \"Tools\"" });
  });

  it("a read of the queues settles the pending move and the folds the core now says (0070 R4/R5)", () => {
    const moved: Layout = [{ name: "Apps", keys: ["/b", "/a", "/c"] }];
    const pending = state<Pending | null>({ layout: moved, saved: true, reads: 0 });
    const folds = state<Record<string, boolean>>({ Apps: true, Tools: false });
    // The core has the move and Apps' fold; Tools' unfold isn't there yet.
    readBack(moved, new Map([["Apps", true], ["Tools", true]]), pending.set, folds.set);
    expect(pending.get()).toBeNull();
    expect(folds.get()).toEqual({ Tools: false });
    // A read that doesn't have the move yet: kept, counted (settle drops it at the second).
    const other = state<Pending | null>({ layout: moved, saved: true, reads: 0 });
    readBack(three, new Map(), other.set, folds.set);
    expect(other.get()).toEqual({ layout: moved, saved: true, reads: 1 });
  });

  const intoTools: Layout = [{ name: "Apps", keys: ["/a", "/b"] }, { name: "Tools", keys: ["/t"] }];
  const moveUi = (folds: ReadonlyMap<string, boolean>) => {
    const s = { pending: state<Pending | null>(null), folds: state<Record<string, boolean>>({}), fresh: state<string[]>(["Tools", "Later"]), u: ui() };
    return { s, ui: { folds, setPending: s.pending.set, setFolds: s.folds.set, setFresh: s.fresh.set, say: s.u.say, problem: s.u.problem } };
  };

  it("a move under a folded heading opens it at once and keeps it open when the core has it (0057 R5, 0070)", async () => {
    const plan = planMove(intoTools, { kind: "project", key: "/a", to: { into: "Tools" } }, label)!;
    const { s, ui: m } = moveUi(new Map([["Apps", false], ["Tools", true]]));
    s.u.out.problem = "an old refusal";
    let done!: () => void;
    const out = runMove(plan, m, () => new Promise<void>((res) => (done = res)));
    expect(s.folds.get()).toEqual({ Tools: false });
    expect(s.pending.get()?.layout).toBe(plan.layout);
    expect(s.u.out).toEqual({ said: "Alpha moved to 1 of 2 in Tools; Tools unfolded", problem: "" });
    done();
    await out;
    expect(s.folds.get()).toEqual({ Tools: false });
    expect(s.pending.get()).toEqual({ layout: plan.layout, saved: true, reads: 0 });
    expect(s.fresh.get()).toEqual(["Later"]);
  });

  it("a move the core refuses takes back its unfold and its layout, and says why (0057 R5)", async () => {
    const plan = planMove(intoTools, { kind: "project", key: "/a", to: { into: "Tools" } }, label)!;
    const { s, ui: m } = moveUi(new Map([["Apps", false], ["Tools", true]]));
    await runMove(plan, m, () => Promise.reject("no such project"));
    expect(s.folds.get()).toEqual({});
    expect(s.pending.get()).toBeNull();
    expect(s.fresh.get()).toEqual(["Tools", "Later"]);
    expect(s.u.out).toEqual({ said: "Alpha moved to 1 of 2 in Tools; Tools unfolded", problem: "Couldn't keep that order: no such project" });
    // Under an open heading: nothing to unfold, nothing to take back.
    const open = moveUi(new Map([["Apps", false], ["Tools", false]]));
    await runMove(plan, open.ui, () => Promise.reject("no"));
    expect(open.s.folds.get()).toEqual({});
    expect(open.s.u.out.said).toBe("Alpha moved to 1 of 2 in Tools");
  });
});
