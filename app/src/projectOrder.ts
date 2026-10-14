// The sidebar's own arrangement (task 0057): the headings and the projects under them as the
// owner puts them, by dragging or with ⌥↑/⌥↓, in the sidebar or in Settings › Projects. Pure
// functions of what's shown, so a move is worked out (and tested) without a page. The core keeps
// what a move says in projects.json (arrange.rs) and sends each queue's place back with the state
// (`QueueInfo.rank`, `categoryRank`).
import type { Moved } from "./api";
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
    return plan(l, null, `${move.name} moved to ${to + 1} of ${l.length}`);
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
  return plan(out, moved, `${label(move.key)} moved to ${now.i + 1} of ${g.keys.length} in ${g.name}`);
}

function plan(layout: Layout, moved: Moved | null, said: string): Plan {
  const kept = layout.filter((g) => g.keys.length > 0);
  return { layout, order: kept.flatMap((g) => g.keys), categories: kept.map((g) => g.name), moved, chip: "mine", said };
}
