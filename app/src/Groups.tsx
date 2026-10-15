import { Fragment, type ReactNode } from "react";
import { tl, useT } from "./i18n";
import { backlogColumn, noteKey } from "./Backlog";
import { boardColumn, type BoardSort, columns, key, STAGES } from "./lib";
import { taskKey } from "./taskKey";
import { estimateLeft, type Group, GROUP_BYS, type GroupBy, levelLabel, progressText, shownStageClass, shownStageLabel } from "./levels";
import type { Note, Task } from "./types";
import EffortGroups, { useAcceptResults } from "./EffortGroups";
import ColumnCards from "./ColumnCards";
import { DueDate } from "./Dates";
import { needsYouCount } from "./human";
import { blockedCount, ChoiceChips, ColumnHead, FoldCounts, placeWords } from "./ModeViews";
import { columnClass, columnState, columnTrack, type ColumnFolds } from "./modes";
import { BandCount } from "./CheckLimit";
import { StatusPill } from "./ui";
import type { Over } from "./yourCheckLimit";
import ProjectName from "./ProjectName";
import { PausedBand } from "./PausedLane";

/** Group by: None · Feature · Goal, on screen above the board and the table, the current one lit. */
export function GroupByChips({ value, onChange }: { value: GroupBy; onChange: (g: GroupBy) => void }) {
  return <ChoiceChips label={tl("groupBy.label")} className="group-by" aria={tl("groupBy.label")} value={value} options={GROUP_BYS} onPick={onChange} />;
}

/** A band's heading: the feature or goal (click to open it), the stage its tasks give it, its
 *  progress, the estimate left and a goal's due date. The last band is "No feature" or "No goal". */
export function BandHead({ group, by, multi, active, onOpen }: {
  group: Group;
  by: "feature" | "goal";
  multi: boolean;
  active: boolean;
  onOpen: (t: Task) => void;
}) {
  const t = useT("board");
  const h = group.head;
  if (!h) {
    return (
      <div className="band-head">
        <span className="band-title">{by === "goal" ? t("groups.noGoal") : t("groups.noFeature")}</span>
        <span className="meta">{t("modes.items", { count: group.items.length })}</span>
      </div>
    );
  }
  const left = estimateLeft(h.progress);
  const open = h.level === "goal" ? "groups.openGoal" : h.level === "feature" ? "groups.openFeature" : "groups.openTask";
  return (
    <div className="band-head">
      <button className={`band-title link${active ? " active" : ""}`} onClick={() => onOpen(h)} title={t(open, { key: taskKey(h) })}>
        {multi && <ProjectName name={h.queue} />} <span className="level">{levelLabel(h.level)}</span> <span className="id ltr">{taskKey(h)}</span> {h.title ? <bdi dir="auto">{h.title}</bdi> : t("groups.noTitle")}
      </button>
      <StatusPill status={shownStageClass(h)}>{shownStageLabel(h)}</StatusPill>
      {h.blocked && <StatusPill status="blocked">{t("groups.blocked")}</StatusPill>}
      <span className="meta">{progressText(h.progress)}</span>
      {left && <span className="meta">{left}</span>}
      {h.due && <span className="meta"><DueDate iso={h.due} /></span>}
    </div>
  );
}

/** What the grouped board's Backlog band shows besides its tasks (0046). */
export type BacklogBandParts = {
  /** The notes in view, newest first (`backlogNotes`), after the search and the filters. */
  notes: Note[];
  renderNote: (n: Note) => ReactNode;
  /** The quick add, as at the top of the plain board's Backlog column. */
  quickAdd: ReactNode;
};

/** The first band (0046): the quick add, the notes, then the Backlog tasks with no feature or goal,
 *  as the plain board's Backlog column shows them. Notes have no parent, so no other band holds
 *  them. Drawn with nothing in it too, so the quick add never disappears; Focus (0038) folds it to
 *  its head, a button with its counts, one click from open. */
function BacklogBand({ by, parts, tasks, renderCard, fold }: { by: "feature" | "goal"; parts: BacklogBandParts; tasks: Task[]; renderCard: (t: Task) => ReactNode; fold?: ColumnFolds }) {
  const t = useT("board");
  const items = backlogColumn(parts.notes, tasks);
  const title = by === "goal" ? t("groups.backlogGoal") : t("groups.backlogFeature");
  const count = t("modes.items", { count: items.length });
  // Focus (0038) folds it as it folds the plain board's Backlog column: its head is the button.
  const folded = fold?.of("backlog") ?? "fixed";
  const needs = needsYouCount(tasks);
  const blocked = blockedCount(tasks);
  return (
    <section className="band backlog-band">
      {folded === "fixed" ? (
        <div className="band-head">
          <span className="band-title">{title}</span>
          <span className="meta">{count}</span>
        </div>
      ) : (
        <button type="button" className="band-fold" aria-expanded={folded === "opened"} aria-label={placeWords(title, count, needs, blocked)} onClick={() => fold?.toggle("backlog")} title={folded === "folded" ? t("groups.openBacklog") : t("groups.foldBacklog")}>
          {title} <small>{count}</small>
          <FoldCounts needs={needs} blocked={blocked} />
        </button>
      )}
      {folded !== "folded" && <div className="board band-board backlog-board">
        <section className="column col-backlog">
          {parts.quickAdd}
          <ColumnCards>
            {items.map((item) =>
              item.kind === "note"
                ? <Fragment key={`note:${noteKey(item.note)}`}>{parts.renderNote(item.note)}</Fragment>
                : <Fragment key={key(item.task)}>{renderCard(item.task)}</Fragment>,
            )}
            {items.length === 0 && <p className="none">{t("groups.nothing")}</p>}
          </ColumnCards>
        </section>
      </div>}
    </section>
  );
}

/** The board grouped: one band per feature or goal, the stage columns inside each, each column
 *  in the board's sort (task 0032). Goals and features are the bands, never cards in the columns.
 *  With `backlog`, the Backlog band comes first and takes the Backlog tasks of the last band
 *  (No feature or No goal), which then holds the rest. The Paused band (mu-59) goes before it and
 *  takes every band's paused tasks, as the Paused lane does on the plain board. */
export function GroupedBoard({ groups, by, sort, multi, openKey, onOpen, renderCard, onChanged, backlog, fold, limit }: {
  groups: Group[];
  by: "feature" | "goal";
  sort: BoardSort;
  multi: boolean;
  openKey: string | null;
  onOpen: (t: Task) => void;
  renderCard: (t: Task) => ReactNode;
  /** With it, each band's Your check column is grouped by effort, with Accept all (0043). */
  onChanged?: () => void;
  backlog?: BacklogBandParts;
  /** The columns the display mode folds (Focus, 0038), the same in every band as on the board. */
  fold?: ColumnFolds;
  /** The projects over their Your check limit (0050): a band's Your check count turns orange
   *  when it holds their tasks. */
  limit?: { over: Over[]; queue: string };
}) {
  const t = useT("board");
  const results = useAcceptResults();
  const paused = columns(groups.flatMap((g) => g.items), sort).paused;
  const loose = backlog ? groups.find((g) => !g.head) : undefined;
  const looseBacklog = loose ? columns(loose.items, sort).backlog : [];
  const rest = loose ? loose.items.filter((x) => !x.paused && boardColumn(x) !== "backlog") : [];
  const bands = loose ? groups.flatMap((g) => (g !== loose ? [g] : rest.length ? [{ head: null, items: rest }] : [])) : groups;
  return (
    <main className="bands">
      <PausedBand tasks={paused} renderCard={renderCard} fold={fold} />
      {backlog && <BacklogBand by={by} parts={backlog} tasks={looseBacklog} renderCard={renderCard} fold={fold} />}
      {bands.length === 0 && looseBacklog.length === 0 && paused.length === 0 && <p className="empty">{t("groups.noMatch")}</p>}
      {bands.map((g) => {
        const cols = columns(g.items, sort);
        // The band's Your check keeps its place while Accept all's result line has something to say.
        const place = `band:${g.head ? key(g.head) : "none"}`;
        const kept = (id: string) => id === "your-check" && !!onChanged && results.has(place);
        return (
          <section key={g.head ? key(g.head) : "none"} className="band">
            <BandHead group={g} by={by} multi={multi} active={!!g.head && openKey === key(g.head)} onOpen={onOpen} />
            {g.items.length === 0 ? (
              <p className="none">{t("groups.noTasks")}</p>
            ) : g.items.every((x) => x.paused) ? (
              <p className="none">{t("groups.allPaused")}</p>
            ) : (
              <div className="board band-board" style={{ gridTemplateColumns: STAGES.map((s) => columnTrack(columnState(fold?.of(s.id) ?? "fixed", (cols[s.id] ?? []).length, kept(s.id)))).join(" ") }}>
                {STAGES.map((c) => {
                  const list = cols[c.id] ?? [];
                  const folded = fold?.of(c.id) ?? "fixed";
                  const state = columnState(folded, list.length, kept(c.id));
                  return (
                    <section key={c.id} className={columnClass(c.id, state)}>
                      <ColumnHead stage={c} count={list.length} countNode={c.id === "your-check" && limit && list.length > 0 ? <BandCount tasks={list} over={limit.over} queue={limit.queue} /> : undefined} needs={needsYouCount(list)} blocked={blockedCount(list)} fold={folded} onFold={() => fold?.toggle(c.id)} empty={state === "empty"} />
                      <ColumnCards>
                        {c.id === "your-check" && onChanged && (list.length > 0 || kept(c.id))
                          ? <EffortGroups tasks={list} place={place} renderCard={renderCard} onChanged={onChanged} />
                          : list.map((x) => (
                              <Fragment key={key(x)}>{renderCard(x)}</Fragment>
                            ))}
                      </ColumnCards>
                    </section>
                  );
                })}
              </div>
            )}
          </section>
        );
      })}
    </main>
  );
}
