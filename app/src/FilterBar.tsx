import { Fragment, useState, type ReactNode, type RefObject } from "react";
import {
  activeItems, deleteView, GROUPS, groupValues, hiddenLine, isActive, removeFilter, sameQuery, saveView, toggleFilter, type DayContext, type Query, type SavedView,
} from "./filters";
import { BOARD_SORTS } from "./lib";
import { useLook } from "./modes";
import { searchBoxKey } from "./keys";
import { highlights, idHit } from "./search";
import { taskKey } from "./taskKey";
import type { Task } from "./types";

/** Text with the search's words marked. */
export function Hl({ text, words }: { text: string; words: string[] }) {
  const marks = highlights(text, words);
  if (!marks.length) return <>{text}</>;
  const out: ReactNode[] = [];
  let at = 0;
  for (const [s, e] of marks) {
    if (s > at) out.push(text.slice(at, s));
    out.push(<mark key={s}>{text.slice(s, e)}</mark>);
    at = e;
  }
  if (at < text.length) out.push(text.slice(at));
  return <>{out}</>;
}

/** A task's key (0033), marked when the search names it ("19" or "mu-19" for mu-19). */
export function HlId({ t, words }: { t: Pick<Task, "id" | "prefix">; words: string[] }) {
  const k = taskKey(t);
  return idHit(t, words) ? <mark>{k}</mark> : <>{k}</>;
}

type Props = {
  /** Every task in the project, filtered or not: the chips offer the values these have. */
  tasks: Task[];
  query: Query;
  onQuery: (q: Query) => void;
  ctx: DayContext;
  /** The board sorts by the Sort chips; the Table by its columns, Timeline by date. */
  sortable: boolean;
  views: SavedView[];
  onViews: (views: SavedView[]) => void;
  shown: number;
  total: number;
  /** Tasks and notes the search and filters hide. */
  hidden: number;
  estimate: string;
  /** How the view is laid out (Group by on the Board, Table and Graph), at the right before the
   *  count. */
  arrange?: ReactNode;
  searchRef?: RefObject<HTMLInputElement | null>;
  /** The view's "?", last in the top row (task 0035). */
  help?: ReactNode;
  /** Focus (0038) hides the bar until ⌘F brings it `out`; `onSummon` is ⌘F's action, for the
   *  button left in its place, and `onHide` puts it away again. */
  out?: boolean;
  onSummon?: () => void;
  onHide?: () => void;
};

/** The bar above Board, Table, Timeline and Graph: search, saved views, every filter group as
 *  chips, the board's sort, and one line that reads what's active and how much it hides. In ADHD
 *  mode nothing in it folds away (docs/DESIGN.md); Standard folds the groups and Sort behind one
 *  Filters chip that opens them in place, and Focus leaves only a button until ⌘F, a search or a
 *  filter brings the bar back (0038). The active line shows whenever the bar does. */
export default function FilterBar({ tasks, query, onQuery, ctx, sortable, views, onViews, shown, total, hidden, estimate, arrange, searchRef, help, out = false, onSummon, onHide }: Props) {
  const [naming, setNaming] = useState<string | null>(null);
  const [unfolded, setUnfolded] = useState(false);
  const how = useLook("filters");
  const items = activeItems(query);
  const active = isActive(query);
  // Focus never hides the bar while it hides tasks, so a filter can't hide them unannounced.
  const forced = active || hidden > 0;
  if (how === "on-key" && !out && !forced) {
    return (
      <div className="filter-bar summon" aria-label="Search and filters">
        <button type="button" className="chip" onClick={onSummon}>Search and filters <small>⌘F</small></button>
        {help}
      </div>
    );
  }
  const filterCount = items.filter((it) => it.group !== "search").length;
  const set = (over: Partial<Query>) => onQuery({ ...query, ...over });
  const save = () => {
    if (naming?.trim()) onViews(saveView(views, { ...query, name: naming }));
    setNaming(null);
  };
  return (
    <div className="filter-bar" aria-label="Search and filters">
      <div className="filters">
        <input
          type="search"
          className="search"
          ref={searchRef}
          value={query.search}
          placeholder="Search id, title, text, tags, holder  ⌘F"
          aria-label="Search tasks"
          spellCheck={false}
          onChange={(e) => set({ search: e.target.value })}
          onKeyDown={(e) => searchBoxKey(e, query.search, () => set({ search: "" }))}
        />
        <span className="fgroup" role="group" aria-label="Saved views">
          <span className="meta sep">Views</span>
          {views.map((v) => {
            const on = sameQuery(v, query);
            return (
              <span key={v.name} className="view-item">
                <button className={on ? "chip view on" : "chip view"} aria-pressed={on} onClick={() => onQuery({ search: v.search, filters: v.filters, sort: v.sort })}>{v.name}</button>
                <button className="chip-x" aria-label={`Delete view ${v.name}`} title="Delete this saved view" onClick={() => onViews(deleteView(views, v.name))}>✕</button>
              </span>
            );
          })}
          {naming === null ? (
            <button className="chip add" onClick={() => setNaming("")} title="Save the search, filters and sort under a name">+ Save view</button>
          ) : (
            <span className="view-name">
              <input
                autoFocus
                value={naming}
                maxLength={40}
                placeholder="Name, e.g. My checks"
                aria-label="Name for this view"
                onChange={(e) => setNaming(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") save();
                  if (e.key === "Escape") {
                    e.stopPropagation();
                    setNaming(null);
                  }
                }}
              />
              <button className="chip on" onClick={save} disabled={!naming.trim()}>Save</button>
              <button className="chip" onClick={() => setNaming(null)}>Cancel</button>
            </span>
          )}
        </span>
        {how === "folded" && (
          <button type="button" className={unfolded ? "chip filters-fold on" : "chip filters-fold"} aria-expanded={unfolded} onClick={() => setUnfolded((u) => !u)} title="The filter groups and Sort, under the bar">
            Filters{filterCount > 0 && <> <b className="count">{filterCount}</b></>}
          </button>
        )}
        {/* Group by pushes itself right (`.group-by`); without it, the count does. */}
        {arrange}
        <span className={arrange ? "meta" : "meta right"}>
          {estimate && <span className="est-total">{estimate} · </span>}
          {shown === total ? `${total} tasks` : `${shown} of ${total} tasks`}
        </span>
        {how === "on-key" && !forced && (
          <button type="button" className="chip" onClick={onHide} title="Put the bar away again: ⌘F brings it back">Hide</button>
        )}
        {help}
      </div>
      {(how !== "folded" || unfolded) && <div className="filters groups">
        {GROUPS.map((g) => {
          const selected = query.filters[g.id] ?? [];
          const values = groupValues(g, tasks, selected, ctx);
          if (!values.length) return null;
          return (
            <span key={g.id} className="fgroup" role="group" aria-label={g.label}>
              <span className="meta">{g.label}</span>
              {values.map((v) => {
                const on = selected.includes(v);
                return (
                  <button key={v} className={on ? `chip on${g.id === "priority" ? ` pchip-${v}` : ""}` : "chip"} aria-pressed={on} onClick={() => set({ filters: toggleFilter(query.filters, g.id, v) })}>
                    {g.chip(v)}
                  </button>
                );
              })}
            </span>
          );
        })}
        {sortable && (
          <span className="fgroup" role="group" aria-label="Sort each column">
            <span className="meta sep">Sort</span>
            {BOARD_SORTS.map((s) => (
              <button key={s.id} className={query.sort === s.id ? "chip on" : "chip"} aria-pressed={query.sort === s.id} title={s.title} onClick={() => set({ sort: s.id })}>
                {s.label}
              </button>
            ))}
          </span>
        )}
      </div>}
      <div className="active-line" aria-label="Active filters">
        {active ? (
          <>
            {items.map((it, i) => (
              <Fragment key={`${it.group}:${it.value}`}>
                {i > 0 && <span className="meta"> · </span>}
                <span className="active-item">
                  {it.label}
                  <button
                    className="chip-x"
                    aria-label={`Remove ${it.label}`}
                    onClick={() => set(it.group === "search" ? { search: "" } : { filters: removeFilter(query.filters, it.group, it.value) })}
                  >
                    ✕
                  </button>
                </span>
              </Fragment>
            ))}
            <span className={hidden > 0 ? "hidden-count some" : "hidden-count"}>{hiddenLine(hidden, true)}</span>
            <button className="chip" onClick={() => set({ search: "", filters: {} })}>Clear all</button>
          </>
        ) : (
          <span className="meta">No filters: every task shows</span>
        )}
      </div>
    </div>
  );
}
