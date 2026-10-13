import { useMemo, useState } from "react";
import { HUMAN_COLUMNS, humanBoard, laneLines, needsYouCount, type HumanColumn } from "./human";
import { key, priorityName, stageLabel, stageOf } from "./lib";
import type { Task } from "./types";

/** Done shows its newest few; the rest one click away, as the AI board's Done does. */
const DONE_SHOWN = 5;

/** The Human board (0022): compact, above the AI board on the Board view, and always there (the
 *  owner has ADHD: out of sight is out of mind). "From the agents" first: agents' tasks waiting on
 *  the owner (Your check, questions escalated to them, blocked on them), each opening its task in
 *  the pane without moving it. Then the owner's own tasks: To do · Doing · Done. An empty column
 *  is one line. The count is `needsYouCount`, the number in the sidebar and the header too. */
export default function HumanBoard({ tasks, multi, openKey, onOpen }: { tasks: Task[]; multi: boolean; openKey: string | null; onOpen: (t: Task) => void }) {
  const b = useMemo(() => humanBoard(tasks), [tasks]);
  const needs = needsYouCount(tasks);
  const [allDone, setAllDone] = useState(false);
  const done = allDone ? b.done : b.done.slice(0, DONE_SHOWN);
  return (
    <section className="human-board" aria-label="Human board">
      <div className="hb-head">
        <h2>Human board</h2>
        <span>{needs === 0 ? "nothing needs you" : `${needs} need${needs === 1 ? "s" : ""} you`}</span>
        <span>· what only you do, and what the agents wait on you for</span>
      </div>
      <div className="hb-cols">
        <div className="hb-col lane" role="group" aria-label="From the agents">
          <h3>From the agents <small>{b.lane.length}</small></h3>
          {b.lane.length === 0 && <p className="none">Nothing waits on you</p>}
          {b.lane.map((t) => (
            <button key={key(t)} type="button" className={`hb-item lane-item s-${stageOf(t)}${openKey === key(t) ? " active" : ""}`} onClick={() => onOpen(t)}>
              <Line t={t} multi={multi} stage />
              {laneLines(t).map((l) => <span key={l} className="hb-why" title={l}>{l}</span>)}
            </button>
          ))}
        </div>
        {HUMAN_COLUMNS.map((c) => {
          const list = c.id === "done" ? done : b[c.id];
          const total = b[c.id].length;
          return (
            <div key={c.id} className={`hb-col col-${c.id}`} role="group" aria-label={c.label}>
              <h3>{c.label} <small>{total}</small></h3>
              {total === 0 && <p className="none">{EMPTY[c.id]}</p>}
              {list.map((t) => (
                <button key={key(t)} type="button" className={`hb-item s-${stageOf(t)}${openKey === key(t) ? " active" : ""}`} onClick={() => onOpen(t)}>
                  <Line t={t} multi={multi} />
                  {t.blocked && <span className="hb-why">blocked: {t.waitingOn || "something it doesn't name"}</span>}
                </button>
              ))}
              {c.id === "done" && total > DONE_SHOWN && (
                <button type="button" className="more" onClick={() => setAllDone((v) => !v)}>
                  {allDone ? "Show fewer" : `Show all ${total}`}
                </button>
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
}

const EMPTY: Record<HumanColumn, string> = { todo: "Nothing to do", doing: "Nothing started", done: "Nothing done yet" };

/** One line: project (when several), id, title, the stage an agent's task is in, high priority. */
function Line({ t, multi, stage = false }: { t: Task; multi: boolean; stage?: boolean }) {
  return (
    <span className="hb-line">
      {multi && <span className="q">{t.queue}</span>}
      <span className="id">{t.id}</span>
      <span className="t" title={t.title}>{t.title || "(no title)"}</span>
      {stage && <span className={`status-pill s-${stageOf(t)}`}>{stageLabel(t.stage)}</span>}
      {priorityName(t.priority) === "high" && <span className="pri pri-high">high</span>}
    </span>
  );
}
