import { Fragment, useState, type ReactNode, type RefObject } from "react";
import {
  activeItems, deleteView, GROUPS, groupValues, hiddenLine, isActive, removeFilter, sameQuery, saveView, toggleFilter, type DayContext, type Query, type SavedView,
} from "./filters";
import { fmt, useT } from "./i18n";
import { BOARD_SORTS } from "./lib";
import { useLook } from "./modes";
import { searchBoxKey } from "./keys";
import { highlights, idHit } from "./search";
import { taskKey } from "./taskKey";
import type { Task } from "./types";
import { Pill } from "./ui";

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
  const t = useT("shell");
  const [naming, setNaming] = useState<string | null>(null);
  const [unfolded, setUnfolded] = useState(false);
  const how = useLook("filters");
  const items = activeItems(query);
  const active = isActive(query);
  // Focus never hides the bar while it hides tasks, so a filter can't hide them unannounced.
  const forced = active || hidden > 0;
  if (how === "on-key" && !out && !forced) {
    return (
      <div className="filter-bar summon" aria-label={t("filter.bar")}>
        <Pill type="button" onClick={onSummon}>{t("filter.bar")} <small>⌘F</small></Pill>
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
    <div className="filter-bar" aria-label={t("filter.bar")}>
      <div className="filters">
        <input
          type="search"
          className="search"
          ref={searchRef}
          value={query.search}
          placeholder={t("filter.placeholder")}
          aria-label={t("filter.searchAria")}
          spellCheck={false}
          onChange={(e) => set({ search: e.target.value })}
          onKeyDown={(e) => searchBoxKey(e, query.search, () => set({ search: "" }))}
        />
        <span className="fgroup" role="group" aria-label={t("filter.savedViews")}>
          <span className="meta sep">{t("filter.views")}</span>
          {views.map((v) => {
            const on = sameQuery(v, query);
            return (
              <span key={v.name} className="view-item">
                <Pill on={on} className="view" aria-pressed={on} onClick={() => onQuery({ search: v.search, filters: v.filters, sort: v.sort })}>{v.name}</Pill>
                <button className="chip-x" aria-label={t("filter.deleteView", { name: v.name })} title={t("filter.deleteViewTitle")} onClick={() => onViews(deleteView(views, v.name))}>✕</button>
              </span>
            );
          })}
          {naming === null ? (
            <Pill className="add" onClick={() => setNaming("")} title={t("filter.saveViewTitle")}>{t("filter.saveView")}</Pill>
          ) : (
            <span className="view-name">
              <input
                autoFocus
                value={naming}
                maxLength={40}
                placeholder={t("filter.namePlaceholder")}
                aria-label={t("filter.nameAria")}
                onChange={(e) => setNaming(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") save();
                  if (e.key === "Escape") {
                    e.stopPropagation();
                    setNaming(null);
                  }
                }}
              />
              <Pill on onClick={save} disabled={!naming.trim()}>{t("common:save")}</Pill>
              <Pill onClick={() => setNaming(null)}>{t("common:cancel")}</Pill>
            </span>
          )}
        </span>
        {how === "folded" && (
          <Pill type="button" on={unfolded} className="filters-fold" aria-expanded={unfolded} onClick={() => setUnfolded((u) => !u)} title={t("filter.filtersTitle")}>
            {t("filter.filters")}{filterCount > 0 && <> <b className="count">{fmt.num(filterCount)}</b></>}
          </Pill>
        )}
        {/* Group by pushes itself right (`.group-by`); without it, the count does. */}
        {arrange}
        <span className={arrange ? "meta" : "meta right"}>
          {estimate && <span className="est-total">{estimate} · </span>}
          {shown === total ? t("filter.tasks", { count: total }) : t("filter.shownOf", { shown, count: total })}
        </span>
        {how === "on-key" && !forced && (
          <Pill type="button" onClick={onHide} title={t("filter.hideTitle")}>{t("common:hide")}</Pill>
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
                  <Pill key={v} on={on} className={on && g.id === "priority" ? `pchip-${v}` : undefined} aria-pressed={on} onClick={() => set({ filters: toggleFilter(query.filters, g.id, v) })}>
                    {g.chip(v)}
                  </Pill>
                );
              })}
            </span>
          );
        })}
        {sortable && (
          <span className="fgroup" role="group" aria-label={t("filter.sortAria")}>
            <span className="meta sep">{t("filter.sort")}</span>
            {BOARD_SORTS.map((s) => (
              <Pill key={s.id} on={query.sort === s.id} aria-pressed={query.sort === s.id} title={s.title} onClick={() => set({ sort: s.id })}>
                {s.label}
              </Pill>
            ))}
          </span>
        )}
      </div>}
      <div className="active-line" aria-label={t("filter.active")}>
        {active ? (
          <>
            {items.map((it, i) => (
              <Fragment key={`${it.group}:${it.value}`}>
                {i > 0 && <span className="meta"> · </span>}
                <span className="active-item">
                  {it.label}
                  <button
                    className="chip-x"
                    aria-label={t("filter.remove", { label: it.label })}
                    onClick={() => set(it.group === "search" ? { search: "" } : { filters: removeFilter(query.filters, it.group, it.value) })}
                  >
                    ✕
                  </button>
                </span>
              </Fragment>
            ))}
            <span className={hidden > 0 ? "hidden-count some" : "hidden-count"}>{hiddenLine(hidden, true)}</span>
            <Pill onClick={() => set({ search: "", filters: {} })}>{t("filter.clearAll")}</Pill>
          </>
        ) : (
          <span className="meta">{t("filter.none")}</span>
        )}
      </div>
    </div>
  );
}
