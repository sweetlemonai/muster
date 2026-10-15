import { Fragment, type ReactNode } from "react";
import ColumnCards from "./ColumnCards";
import { needsYouCount } from "./human";
import { tn, useT } from "./i18n";
import { key } from "./lib";
import { blockedCount, ColumnHead, FoldCounts, placeWords } from "./ModeViews";
import { columnClass, type ColumnFolds } from "./modes";
import { PAUSE_GLYPH } from "./Pause";
import type { Task } from "./types";

// The Paused lane (mu-59): paused tasks wait at the board's left, beside Backlog, out of the
// working columns. It's only a view: a paused task keeps its stage and claim (lib.ts `columns`
// puts it here), so Resume puts it back in its column. Always there with its count: open while
// it holds tasks, else a chip in the row above the board (mu-84), where Focus (0038) folds it too.

/** The lane as a board column's head reads it, in the language shown when it's read. */
export const PAUSED_LANE = {
  id: "paused",
  get label() {
    return `${PAUSE_GLYPH} ${tn("board", "pause.paused")}`;
  },
  get role() {
    return tn("board", "lane.role");
  },
};

/** The plain board's Paused lane, open: a column like the stages', first on the board, with the
 *  Fold that sends it to the row (modes.ts `plainBoard` says when it's open). */
export function PausedColumn({ tasks, onHide, renderCard }: { tasks: Task[]; onHide: () => void; renderCard: (t: Task) => ReactNode }) {
  const t = useT("board");
  return (
    <section className={columnClass("paused", "open")} data-col="paused">
      <ColumnHead stage={PAUSED_LANE} count={tasks.length} needs={needsYouCount(tasks)} blocked={blockedCount(tasks)} fold="fixed" onFold={onHide} onHide={onHide}>
        <span className="role">{PAUSED_LANE.role}</span>
      </ColumnHead>
      <ColumnCards>
        {tasks.map((t) => (
          <Fragment key={key(t)}>{renderCard(t)}</Fragment>
        ))}
        {tasks.length === 0 && <p className="none">{t("lane.none")}</p>}
      </ColumnCards>
    </section>
  );
}

/** The grouped board's Paused band, first, as the lane is on the plain board: every paused task
 *  in view, whichever feature or goal it's under (its card says which), side by side. With nothing
 *  paused it's its head alone. Focus folds it to its head, a button with its counts. */
export function PausedBand({ tasks, renderCard, fold }: { tasks: Task[]; renderCard: (t: Task) => ReactNode; fold?: ColumnFolds }) {
  const t = useT("board");
  const count = t("lane.tasks", { count: tasks.length });
  const folded = fold?.of(PAUSED_LANE.id) ?? "fixed";
  const needs = needsYouCount(tasks);
  const blocked = blockedCount(tasks);
  return (
    <section className={`band paused-band${tasks.length ? "" : " paused-none"}`}>
      {folded === "fixed" ? (
        <div className="band-head" title={PAUSED_LANE.role}>
          <span className="band-title">{PAUSED_LANE.label}</span>
          <span className="meta">{count}</span>
        </div>
      ) : (
        <button type="button" className="band-fold" aria-expanded={folded === "opened"} aria-label={placeWords(PAUSED_LANE.label, count, needs, blocked)} onClick={() => fold?.toggle(PAUSED_LANE.id)} title={folded === "folded" ? t("lane.openHere") : t("lane.foldHead")}>
          {PAUSED_LANE.label} <small>{count}</small>
          <FoldCounts needs={needs} blocked={blocked} />
        </button>
      )}
      {folded !== "folded" && tasks.length > 0 && (
        <div className="board band-board paused-board">
          <section className="column col-paused">
            <ColumnCards>
              {tasks.map((t) => (
                <Fragment key={key(t)}>{renderCard(t)}</Fragment>
              ))}
            </ColumnCards>
          </section>
        </div>
      )}
    </section>
  );
}
