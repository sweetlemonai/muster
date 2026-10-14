import { describe, expect, it } from "vitest";
import { accepts, dropMove, grouped, headingFolds, headingRanks, peeks, settleFolds, type Layout, layoutOf, orderHeadings, type Pending, planMove, settle, shownLayout, sidebarGroups, track } from "./projectOrder";
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
  const row = (name: string, category: string, over: Partial<{ categoryFolded: boolean; needs: number }> = {}) => ({ name, category, categoryFolded: false, needs: 0, ...over });
  it("are the headings the core says are folded, with a fold set here winning until the core agrees", () => {
    const rows = [row("a", "Apps", { categoryFolded: true }), row("b", "Apps", { categoryFolded: true }), row("t", "Tools")];
    expect([...headingFolds(rows, {})]).toEqual([["Apps", true], ["Tools", false]]);
    expect([...headingFolds(rows, { Tools: true, Apps: false })]).toEqual([["Apps", false], ["Tools", true]]);
    // Once the core says the same, the one set here goes.
    expect(settleFolds({ Tools: true, Apps: false }, headingFolds(rows, {}))).toEqual({ Tools: true, Apps: false });
    expect(settleFolds({ Tools: false, Apps: true }, headingFolds(rows, {}))).toEqual({});
  });
  it("still show the selected project and every one that needs you, in order, on a folded heading", () => {
    const rows = [row("a", "Apps"), row("b", "Apps", { needs: 2 }), row("c", "Apps"), row("d", "Apps", { needs: 1 })];
    expect(peeks(rows, "c").map((r) => r.name)).toEqual(["b", "c", "d"]);
    expect(peeks(rows, "all").map((r) => r.name)).toEqual(["b", "d"]);
    expect(peeks([row("a", "Apps")], "zeta")).toEqual([]);
  });
});
