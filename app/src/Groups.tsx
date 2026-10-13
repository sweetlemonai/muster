import { Fragment, type ReactNode } from "react";
import { type BoardSort, columns, key, STAGES } from "./lib";
import { estimateLeft, type Group, GROUP_BYS, type GroupBy, levelLabel, progressText, shownStageClass, shownStageLabel } from "./levels";
import type { Task } from "./types";
import EffortGroups, { useAcceptResults } from "./EffortGroups";

/** Group by: None · Feature · Goal, on screen above the board and the table, the current one lit. */
export function GroupByChips({ value, onChange }: { value: GroupBy; onChange: (g: GroupBy) => void }) {
  return (
    <span className="group-by" role="group" aria-label="Group by">
      <span className="meta">Group by</span>
      {GROUP_BYS.map((g) => (
        <button key={g.id} className={value === g.id ? "chip on" : "chip"} aria-pressed={value === g.id} onClick={() => onChange(g.id)}>
          {g.label}
        </button>
      ))}
    </span>
  );
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
  const h = group.head;
  if (!h) {
    return (
      <div className="band-head">
        <span className="band-title">{by === "goal" ? "No goal" : "No feature"}</span>
        <span className="meta">{group.items.length} {group.items.length === 1 ? "item" : "items"}</span>
      </div>
    );
  }
  const left = estimateLeft(h.progress);
  return (
    <div className="band-head">
      <button className={`band-title link${active ? " active" : ""}`} onClick={() => onOpen(h)} title={`Open ${levelLabel(h.level).toLowerCase()} ${h.id}`}>
        {multi && <span className="q">{h.queue}</span>} <span className="level">{levelLabel(h.level)}</span> <span className="id">{h.id}</span> {h.title || "(no title)"}
      </button>
      <span className={`status-pill s-${shownStageClass(h)}`}>{shownStageLabel(h)}</span>
      {h.blocked && <span className="status-pill blocked">blocked</span>}
      <span className="meta">{progressText(h.progress)}</span>
      {left && <span className="meta">{left}</span>}
      {h.due && <span className="meta">due {h.due}</span>}
    </div>
  );
}

/** The board grouped: one band per feature or goal, the stage columns inside each, each column
 *  in the board's sort (task 0032). Goals and features are the bands, never cards in the columns. */
export function GroupedBoard({ groups, by, sort, multi, openKey, onOpen, renderCard, onChanged }: {
  groups: Group[];
  by: "feature" | "goal";
  sort: BoardSort;
  multi: boolean;
  openKey: string | null;
  onOpen: (t: Task) => void;
  renderCard: (t: Task) => ReactNode;
  /** With it, each band's Your check column is grouped by effort, with Accept all (0043). */
  onChanged?: () => void;
}) {
  const results = useAcceptResults();
  return (
    <main className="bands">
      {groups.length === 0 && <p className="empty">No tasks match.</p>}
      {groups.map((g) => {
        const cols = columns(g.items, sort);
        // The band's Your check keeps its place while Accept all's result line has something to say.
        const place = `band:${g.head ? key(g.head) : "none"}`;
        const kept = (id: string) => id === "your-check" && !!onChanged && results.has(place);
        return (
          <section key={g.head ? key(g.head) : "none"} className="band">
            <BandHead group={g} by={by} multi={multi} active={!!g.head && openKey === key(g.head)} onOpen={onOpen} />
            {g.items.length === 0 ? (
              <p className="none">No tasks yet</p>
            ) : (
              <div className="board band-board" style={{ gridTemplateColumns: STAGES.map((s) => ((cols[s.id] ?? []).length || kept(s.id) ? "minmax(118px, 1fr)" : "44px")).join(" ") }}>
                {STAGES.map((c) => {
                  const list = cols[c.id] ?? [];
                  return (
                    <section key={c.id} className={`column col-${c.id}${list.length || kept(c.id) ? "" : " empty"}`}>
                      <h2 title={c.role ? `${c.label}: ${c.role}` : c.label}>
                        {c.label} <small>{list.length}</small>
                      </h2>
                      <div className="cards">
                        {c.id === "your-check" && onChanged && (list.length > 0 || kept(c.id))
                          ? <EffortGroups tasks={list} place={place} renderCard={renderCard} onChanged={onChanged} />
                          : list.map((t) => (
                              <Fragment key={key(t)}>{renderCard(t)}</Fragment>
                            ))}
                      </div>
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
