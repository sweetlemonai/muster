// The sidebar's own arrangement (task 0057): the headings and the projects under them as the
// owner puts them, by dragging or with ⌥↑/⌥↓, in the sidebar or in Settings › Projects. Pure
// functions of what's shown, so a move is worked out (and tested) without a page. The core keeps
// what a move says in projects.json (arrange.rs) and sends each queue's place back with the state
// (`QueueInfo.rank`, `categoryRank`).
import type { Moved } from "./api";
import { workingWords } from "./human";
import { tn } from "./i18n";
import { byCategory, type CategoryGroup } from "./labels";

/** The headings top to bottom, each with its projects' keys (queue folders) in order. */
export type Layout = { name: string; keys: string[] }[];

/** Each heading's place among the headings, from its projects (they share it). */
export function headingRanks(rows: { category: string; categoryRank: number | null }[]): Map<string, number> {
  const out = new Map<string, number>();
  for (const r of rows) if (r.categoryRank !== null && !out.has(r.category)) out.set(r.category, r.categoryRank);
  return out;
}

/** `groups` with the placed headings first, by place, then the rest in the order given (byCategory's:
 *  A–Z, Other last). */
export function orderHeadings<T>(groups: CategoryGroup<T>[], ranks: ReadonlyMap<string, number>): CategoryGroup<T>[] {
  const placed = groups.filter((g) => ranks.has(g.name)).sort((a, b) => (ranks.get(a.name) ?? 0) - (ranks.get(b.name) ?? 0));
  return [...placed, ...groups.filter((g) => !ranks.has(g.name))];
}

/** The sidebar's headings: `rows` (in the order chosen) under their headings in the owner's
 *  order, then the new headings still waiting for a project (`fresh`), empty, last. */
export function sidebarGroups<T extends { category: string; categoryRank: number | null }>(rows: T[], fresh: string[]): CategoryGroup<T>[] {
  const groups = orderHeadings(byCategory(rows), headingRanks(rows));
  return withFresh(groups, fresh);
}

/** `groups`, then each of the `fresh` headings they don't have yet, empty. */
export function withFresh<T>(groups: CategoryGroup<T>[], fresh: string[]): CategoryGroup<T>[] {
  const have = new Set(groups.map((g) => g.name));
  const out = [...groups];
  for (const name of fresh) {
    if (have.has(name)) continue;
    have.add(name);
    out.push({ name, rows: [] });
  }
  return out;
}

/** `rows` as `layout` puts them, while a move is saved: its headings and order, a row it doesn't
 *  have under its own heading at the end, and no heading it left empty. */
export function grouped<T extends { key: string; category: string }>(rows: T[], layout: Layout): CategoryGroup<T>[] {
  const byKey = new Map(rows.map((r) => [r.key, r]));
  const placed = new Set<string>();
  const out: CategoryGroup<T>[] = layout.map((g) => ({
    name: g.name,
    rows: g.keys.flatMap((k) => {
      const r = byKey.get(k);
      if (!r || placed.has(k)) return [];
      placed.add(k);
      return [r];
    }),
  }));
  for (const r of rows) {
    if (placed.has(r.key)) continue;
    const g = out.find((x) => x.name === r.category);
    if (g) g.rows.push(r);
    else out.push({ name: r.category, rows: [r] });
  }
  return out.filter((g) => g.rows.length > 0);
}

/** A layout read back from shown groups. */
export const layoutOf = <T extends { key: string }>(groups: CategoryGroup<T>[]): Layout => groups.map((g) => ({ name: g.name, keys: g.rows.map((r) => r.key) }));

/** Where a dragged project lands: before or after a row, or at the top of a heading. */
export type Drop = { before: string } | { after: string } | { into: string };
/** A move: a project or a heading dragged (a heading `before` another, or last for null), or
 *  stepped one place up (-1) or down (1). */
export type Move =
  | { kind: "project"; key: string; to: Drop }
  | { kind: "heading"; name: string; before: string | null }
  | { kind: "step-project"; key: string; by: -1 | 1 }
  | { kind: "step-heading"; name: string; by: -1 | 1 };

/** What a move does: the layout after it, what the core keeps (every project's key and the
 *  headings that have one, first to last, and the project that went under another heading), the
 *  order chip it picks (Mine, chosen by the first move) and what a screen reader hears. */
export type Plan = { layout: Layout; order: string[]; categories: string[]; moved: Moved | null; chip: "mine"; said: string };

const copy = (l: Layout): Layout => l.map((g) => ({ name: g.name, keys: [...g.keys] }));
const find = (l: Layout, key: string) => {
  const g = l.findIndex((x) => x.keys.includes(key));
  return g < 0 ? null : { g, i: l[g].keys.indexOf(key) };
};
/** The same headings with the same projects in the same order, empty headings aside. */
export const sameLayout = (a: Layout, b: Layout) => JSON.stringify(a.filter((g) => g.keys.length)) === JSON.stringify(b.filter((g) => g.keys.length));
const same = sameLayout;

/** A move worked out on `layout` (left as it is); null when it changes nothing: dropped in its own
 *  place, at the top or bottom edge, or naming what the layout doesn't have. `label` names a
 *  project for what's said. A heading a move empties goes; one that was empty stays, in its place. */
export function planMove(layout: Layout, move: Move, label: (key: string) => string): Plan | null {
  const l = copy(layout);
  if (move.kind === "heading" || move.kind === "step-heading") {
    const from = l.findIndex((g) => g.name === move.name);
    if (from < 0) return null;
    let to: number;
    if (move.kind === "step-heading") {
      to = from + move.by;
      if (to < 0 || to >= l.length) return null;
      const [g] = l.splice(from, 1);
      l.splice(to, 0, g);
    } else {
      if (move.before === move.name) return null;
      const [g] = l.splice(from, 1);
      to = move.before === null ? l.length : l.findIndex((x) => x.name === move.before);
      if (to < 0) return null;
      l.splice(to, 0, g);
      to = l.indexOf(g);
    }
    if (same(l, layout)) return null;
    return plan(l, null, tn("shell", "move.headingMoved", { name: move.name, at: to + 1, of: l.length }));
  }
  const at = find(l, move.key);
  if (!at) return null;
  const was = l[at.g].name;
  if (move.kind === "step-project") {
    l[at.g].keys.splice(at.i, 1);
    const i = at.i + move.by;
    if (i >= 0 && i <= l[at.g].keys.length) l[at.g].keys.splice(i, 0, move.key);
    else if (move.by < 0 && at.g > 0) l[at.g - 1].keys.push(move.key);
    else if (move.by > 0 && at.g < l.length - 1) l[at.g + 1].keys.unshift(move.key);
    else return null;
  } else {
    const to = move.to;
    if ("into" in to) {
      l[at.g].keys.splice(at.i, 1);
      let g = l.find((x) => x.name === to.into);
      if (!g) {
        g = { name: to.into, keys: [] };
        l.push(g);
      }
      g.keys.unshift(move.key);
    } else {
      const other = "before" in to ? to.before : to.after;
      if (other === move.key || !find(l, other)) return null;
      l[at.g].keys.splice(at.i, 1);
      const there = find(l, other);
      if (!there) return null;
      l[there.g].keys.splice(there.i + ("after" in to ? 1 : 0), 0, move.key);
    }
  }
  if (same(l, layout)) return null;
  // The heading a move emptied goes (empty categories disappear); a new one that never had a
  // project stays where it was.
  const out = l.filter((g) => g.keys.length > 0 || layout.some((x) => x.name === g.name && x.keys.length === 0));
  const now = find(out, move.key);
  if (!now) return null;
  const g = out[now.g];
  const moved = g.name === was ? null : { key: move.key, category: g.name };
  return plan(out, moved, tn("shell", "move.projectMoved", { name: label(move.key), at: now.i + 1, of: g.keys.length, heading: g.name }));
}

function plan(layout: Layout, moved: Moved | null, said: string): Plan {
  const kept = layout.filter((g) => g.keys.length > 0);
  return { layout, order: kept.flatMap((g) => g.keys), categories: kept.map((g) => g.name), moved, chip: "mine", said };
}

/** What's being dragged: a project by its key, or a heading by its name. */
export type Dragged = { kind: "project"; key: string } | { kind: "heading"; name: string };
/** What a drag is over: a project's row, a heading, or the group (a heading and its rows) around them. */
export type DropTarget = { kind: "row"; key: string } | { kind: "heading"; name: string } | { kind: "group"; name: string };

/** Whether `t` takes what's dragged: a row takes a project, a heading either, the group around them
 *  a heading. One that doesn't lets the drag (and the drop) go on to the group around it (review R1). */
export function accepts(drag: Dragged | null, t: DropTarget): boolean {
  if (!drag) return false;
  if (t.kind === "heading") return true;
  return t.kind === "row" ? drag.kind === "project" : drag.kind === "heading";
}

/** The move a drop on `t` makes, on its upper or lower half (`side`): a project before or after a
 *  row, or at the top of a heading; a heading before or after a heading or a group. Null for a
 *  target that doesn't take it. */
export function dropMove(layout: Layout, drag: Dragged | null, t: DropTarget, side: "before" | "after"): Move | null {
  if (!drag || !accepts(drag, t)) return null;
  if (drag.kind === "project") {
    if (t.kind === "row") return { kind: "project", key: drag.key, to: side === "before" ? { before: t.key } : { after: t.key } };
    return t.kind === "heading" ? { kind: "project", key: drag.key, to: { into: t.name } } : null;
  }
  if (t.kind === "row") return null;
  const i = layout.findIndex((g) => g.name === t.name);
  const before = side === "before" ? t.name : (layout[i + 1]?.name ?? null);
  return { kind: "heading", name: drag.name, before };
}

/** A move shown before the core has it: its layout, whether the save came back, and how many
 *  times the queues were read since (review R3). */
export type Pending = { layout: Layout; saved: boolean; reads: number };

/** The layout to show, and to plan the next move from: the pending move's, else the core's. */
export const shownLayout = (p: Pending | null, core: Layout): Layout => p?.layout ?? core;

/** A move's bookkeeping, the sidebar's and Settings' (review R6): `plan` shows at once (and the
 *  next move plans from it, `shownLayout`), `send` saves it, and then it's marked saved, or
 *  dropped if the core refused, which is passed on. A newer move's pending layout is left alone
 *  by an older one's answer. `set` is the component's functional state setter. */
export function track(plan: Plan, set: (f: (p: Pending | null) => Pending | null) => void, send: () => Promise<void>): Promise<void> {
  const ours = (p: Pending | null) => p?.layout === plan.layout;
  set(() => ({ layout: plan.layout, saved: false, reads: 0 }));
  return send().then(
    () => set((p) => (p && ours(p) ? { ...p, saved: true } : p)),
    (e: unknown) => {
      set((p) => (ours(p) ? null : p));
      throw e;
    },
  );
}

/** A pending move after the queues were read again (`core`, the layout they give): gone once they
 *  say the same, or at the second read after the save whatever they say (another window may have
 *  moved something too); kept while the save is out. */
export function settle(p: Pending | null, core: Layout): Pending | null {
  if (!p || sameLayout(core, p.layout)) return null;
  if (!p.saved) return p;
  return p.reads + 1 >= 2 ? null : { ...p, reads: p.reads + 1 };
}

/** Each heading's fold (0070): what the core says (`categoryFolded` on its projects), or what
 *  was set here (`set`) until the core agrees. */
export function headingFolds(rows: { category: string; categoryFolded: boolean }[], set: Readonly<Record<string, boolean>>): Map<string, boolean> {
  const out = new Map<string, boolean>();
  for (const r of rows) if (!out.has(r.category)) out.set(r.category, set[r.category] ?? r.categoryFolded);
  for (const [name, folded] of Object.entries(set)) if (!out.has(name)) out.set(name, folded);
  return out;
}

/** The folds set here that the core doesn't say yet (`core`, headingFolds of its own): the rest go. */
export function settleFolds(set: Readonly<Record<string, boolean>>, core: ReadonlyMap<string, boolean>): Record<string, boolean> {
  return Object.fromEntries(Object.entries(set).filter(([name, folded]) => core.get(name) !== folded));
}

/** How much a project needs you: its tasks that wait on you and its lessons waiting, as its row's
 *  badge counts them (0070 review R2). */
export const needsOf = (r: { needs: number; lessons: number }): number => r.needs + r.lessons;

/** A heading's counts (0082): how much its projects need you (`needsOf`, as their rows' badges),
 *  how many projects it holds, and how many of their tasks are being worked on. */
export type HeadingCounts = { needs: number; projects: number; working: number };
export function headingCounts(rows: { needs: number; lessons: number; activity: { working: number } }[]): HeadingCounts {
  return { needs: rows.reduce((n, r) => n + needsOf(r), 0), projects: rows.length, working: rows.reduce((n, r) => n + r.activity.working, 0) };
}

/** A heading's counts in words, the same open and folded (0082): what needs you first, so at the
 *  sidebar's narrowest it's never the part cut; what's 0 left out, but never the projects. */
export function countsText(c: HeadingCounts, sep = " · "): string {
  return [
    c.needs > 0 && tn("shell", "heading.needYou", { count: c.needs }),
    tn("shell", "heading.projects", { count: c.projects }),
    c.working > 0 && workingWords(c.working),
  ].filter(Boolean).join(sep);
}

/** The folded heading a move opens (0070, review R4): the one a project was moved under, so it
 *  stays in sight; null when it moved under an open one, or nobody changed heading. The core opens
 *  it too (arrange.rs), for every window. */
export function unfoldFor(plan: Plan, folds: ReadonlyMap<string, boolean>): string | null {
  return plan.moved && folds.get(plan.moved.category) === true ? plan.moved.category : null;
}

/** A fold's bookkeeping (0070, review R5), as `track` is a move's: shown at once, kept while the
 *  core keeps it, taken back if the core refuses (which is passed on), unless a later fold of the
 *  same heading replaced it. `set` is the component's functional state setter. */
export function trackFold(name: string, folded: boolean, set: (f: (s: Record<string, boolean>) => Record<string, boolean>) => void, send: () => Promise<void>): Promise<void> {
  set((s) => ({ ...s, [name]: folded }));
  return send().catch((e: unknown) => {
    set((s) => (s[name] === folded ? Object.fromEntries(Object.entries(s).filter(([n]) => n !== name)) : s));
    throw e;
  });
}

/** A move's unfold (`unfoldFor`'s heading, or null) while the move is saved (`saving`, `track`'s):
 *  shown at once, kept when the core keeps the move (it unfolds there too), and put back as it was
 *  if the core refuses, which is passed on, unless the heading was folded again meanwhile (review R11). */
export function trackUnfold(name: string | null, set: (f: (s: Record<string, boolean>) => Record<string, boolean>) => void, saving: Promise<void>): Promise<void> {
  if (name === null) return saving;
  // What this window had set for it before (none: the core's fold shows again).
  let before: boolean | undefined;
  set((s) => {
    before = s[name];
    return { ...s, [name]: false };
  });
  return saving.catch((e: unknown) => {
    set((s) => {
      if (s[name] !== false) return s;
      const rest = Object.fromEntries(Object.entries(s).filter(([n]) => n !== name));
      return before === undefined ? rest : { ...rest, [name]: before };
    });
    throw e;
  });
}

/** The folds set here after a read of the queues (`core`, the core's own folds): `settleFolds`,
 *  the same object when nothing went, so the read doesn't render again for nothing. */
export function readFolds(set: Record<string, boolean>, core: ReadonlyMap<string, boolean>): Record<string, boolean> {
  const kept = settleFolds(set, core);
  return Object.keys(kept).length === Object.keys(set).length ? set : kept;
}

// The window's entry points (mu-88): each handler's body, out of its component so a test drives it
// with fake setters and a save that resolves or refuses. The components call these and nothing else.

/** A component's functional state setter, as React's is called. */
export type Setter<T> = (f: (s: T) => T) => void;
/** Folds set here, by heading (0070). */
export type Folds = Record<string, boolean>;
/** What a screen reader hears (the polite status line) and the refusal shown (role="alert"). */
export type Voice = { say: (text: string) => void; problem: (text: string) => void };

/** Settings › Projects' ↑ and ↓ for the project `key` (0059 R3/R5): each plans from the layout
 *  shown (the pending move's, else the core's), so a second click before the queues read back goes
 *  on from the first, and is tracked (`track`) as it's saved. Null where it can't go. */
export function placeSteps(pending: Pending | null, core: Layout, key: string, named: (key: string) => string, set: Setter<Pending | null>, save: (plan: Plan) => Promise<void>): { up: (() => Promise<void>) | null; down: (() => Promise<void>) | null } {
  const layout = shownLayout(pending, core);
  const step = (by: -1 | 1) => {
    const plan = planMove(layout, { kind: "step-project", key, by }, named);
    return plan ? () => track(plan, set, () => save(plan)) : null;
  };
  return { up: step(-1), down: step(1) };
}

/** The sidebar's fold or unfold of a heading (0070 R4/R5): said, shown at once (`trackFold`), and a
 *  refusal put back and shown in the problem line. Never rejects. */
export function foldHeading(name: string, folded: boolean, ui: Voice & { setFolds: Setter<Folds> }, send: () => Promise<void>): Promise<void> {
  ui.say(tn("shell", folded ? "move.folded" : "move.unfolded", { name }));
  return trackFold(name, folded, ui.setFolds, send).catch((e: unknown) => ui.problem(tn("shell", folded ? "move.cantFold" : "move.cantUnfold", { name, error: String(e) })));
}

/** The sidebar after a read of the queues (0070 R4/R5): the pending move settles against the
 *  layout they give (`core`, `settle`), and so do the folds set here against theirs (`readFolds`). */
export function readBack(core: Layout, coreFolds: ReadonlyMap<string, boolean>, setPending: Setter<Pending | null>, setFolds: Setter<Folds>): void {
  setPending((p) => settle(p, core));
  setFolds((f) => readFolds(f, coreFolds));
}

/** The sidebar's move (0057 R5, 0070): said, shown at once (`track`), the folded heading it moves a
 *  project under opened at once and taken back if the core refuses (`trackUnfold`), the new heading
 *  it filled no longer waiting (`setFresh`), and a refusal shown. Never rejects. */
export function runMove(plan: Plan, ui: Voice & { folds: ReadonlyMap<string, boolean>; setPending: Setter<Pending | null>; setFolds: Setter<Folds>; setFresh: Setter<string[]> }, send: () => Promise<void>): Promise<void> {
  const opened = unfoldFor(plan, ui.folds);
  ui.say(opened ? tn("shell", "move.movedAndUnfolded", { said: plan.said, name: opened }) : plan.said);
  ui.problem("");
  return trackUnfold(opened, ui.setFolds, track(plan, ui.setPending, send)).then(
    () => {
      const made = plan.moved?.category;
      if (made !== undefined) ui.setFresh((f) => f.filter((x) => x !== made));
    },
    (e: unknown) => ui.problem(tn("shell", "move.cantKeep", { error: String(e) })),
  );
}
