import type { ReactNode } from "react";
import { GRAPH_MARKS, type GraphMark } from "./graph";
import { type TFn, useT } from "./i18n";
import { TIMELINE_MARKS, type TimelineMark } from "./viewData";
import YouRibbon from "./YouRibbon";

// The swatch row under the Timeline's scale and the Graph's bar (mu-173): each colour and mark the
// view draws, with its name. The marks come from legendMarks and graphMarks; every swatch is built
// from the view's own classes, by mark, never from task data. The names are board:legend.<mark>.

export type LegendMark = TimelineMark | GraphMark;
export type LegendItem = { id: LegendMark; swatch: ReactNode };

const list = (items: readonly LegendItem[], t: TFn<"board">) =>
  items.map((i) => (
    <li key={i.id} className="legend-item">
      <span className="legend-swatch" aria-hidden="true">{i.swatch}</span>
      {t(`legend.${i.id}`)}
    </li>
  ));

/** The swatches drawn, each followed by its name, wrapping onto more lines when the width is short.
 *  `reserve` is every item the view can draw: a hidden copy of it shares the row's grid cell, so the
 *  row is always as tall as the full set wraps to at this width, and stepping to a window with more
 *  or fewer marks never moves the chart below (R4). The copy is hidden from sight, screen readers
 *  and the mouse, and holds nothing focusable. */
export function Legend({ items, reserve }: { items: readonly LegendItem[]; reserve: readonly LegendItem[] }) {
  const t = useT("board");
  return (
    <div className="legend" role="group" aria-label={t("legend.label")}>
      <ul className="legend-list">{list(items, t)}</ul>
      <ul className="legend-list legend-reserve" aria-hidden="true">
        {list(reserve, t)}
      </ul>
    </div>
  );
}

/** A bar holding one segment, as the timeline draws a stage (Your check's is its thin tail). */
const stageBar = (s: "backlog" | "shaping" | "ready" | "building" | "testing" | "your-check") => (
  <span className={`tl-bar s-${s}`}>
    <span className={`tl-seg s-${s}`} />
  </span>
);

function timelineSwatch(m: TimelineMark): ReactNode {
  switch (m) {
    case "backlog":
    case "shaping":
    case "ready":
    case "building":
    case "testing":
    case "your-check":
      return stageBar(m);
    case "done":
      return <span className="tl-bar s-done" />;
    case "waiting":
      return (
        <span className="tl-bar lg-neutral">
          <span className="tl-seg waiting" />
        </span>
      );
    case "fuzzy":
      return (
        <span className="tl-bar lg-neutral">
          <span className="tl-seg lg-neutral fuzzy" />
        </span>
      );
    case "blocked":
      return (
        <span className="tl-bar lg-neutral blocked">
          <span className="tl-block" />
        </span>
      );
    case "paused":
      return (
        <span className="tl-bar lg-neutral">
          <span className="tl-pause" />
        </span>
      );
    case "now":
      return <span className="tl-now" />;
  }
}

function graphSwatch(m: GraphMark): ReactNode {
  switch (m) {
    case "backlog":
    case "shaping":
    case "ready":
    case "building":
    case "testing":
    case "your-check":
    case "done":
      return <span className={`g-node s-${m}`} />;
    case "ready-to-start":
      return <span className="g-node ready" />;
    case "blocked":
      return <span className="g-node blocked" />;
    case "human":
      return <YouRibbon />;
    case "loop":
      return (
        <svg className="lg-edge" viewBox="0 0 24 12">
          <path className="g-edge loop" d="M1,6 L23,6" />
        </svg>
      );
  }
}

export const timelineLegend = (marks: readonly TimelineMark[]): LegendItem[] => marks.map((m) => ({ id: m, swatch: timelineSwatch(m) }));
export const graphLegend = (marks: readonly GraphMark[]): LegendItem[] => marks.map((m) => ({ id: m, swatch: graphSwatch(m) }));
/** Every item each view can draw: the rows' reserved height (R4). */
export const TIMELINE_RESERVE = timelineLegend(TIMELINE_MARKS);
export const GRAPH_RESERVE = graphLegend(GRAPH_MARKS);
