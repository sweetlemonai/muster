import { claimant, fmtMinutes, fmtTokens, stageLabel, stamp, stampMinutes } from "./lib";
import { estimateLeft, isWork, levelLabel, progressText, shownStageClass, shownStageLabel } from "./levels";
import Markdown from "./Markdown";
import OwnerChecklist from "./OwnerChecklist";
import { criteriaSection, fmtWall, timeInWork } from "./paneData";
import type { Task } from "./types";

/** The task pane's left column: what this stage needs. Building and Testing: the work so far.
 *  Your check: the owner's checklist and the history. Backlog, Shaping and Ready: the criteria and
 *  the estimate. Done (and a stage the window doesn't know): the history. A goal or feature: its
 *  roll-up. `body` is the write-up as loaded (null while it loads). */
export default function StageContent({ task, nowSecs, body, childTasks, busy, onTick }: {
  task: Task;
  nowSecs: number;
  body: string | null;
  childTasks?: Task[];
  busy: boolean;
  onTick: (which: string, undo: boolean) => void;
}) {
  if (!isWork(task)) return <Rollup task={task} childTasks={childTasks} />;
  switch (task.stage) {
    case "building":
    case "testing":
      return <WorkSoFar task={task} nowSecs={nowSecs} />;
    case "your-check":
      return (
        <>
          <div className="stage-block" data-part="checklist">
            <span className="lbl">Your checklist</span>
            {task.ownerCriteria.length > 0 ? (
              <OwnerChecklist task={task} busy={busy} onTick={onTick} />
            ) : (
              <p className="meta small">None of the criteria are yours to verify. Accept it, or send it back.</p>
            )}
          </div>
          <History task={task} />
        </>
      );
    case "backlog":
    case "shaping":
    case "ready":
      return (
        <>
          <Criteria body={body} />
          <Estimates task={task} />
        </>
      );
    default:
      return <History task={task} />;
  }
}

/** Building and Testing: the stage history, time spent against the estimate, and sessions (0025). */
function WorkSoFar({ task, nowSecs }: { task: Task; nowSecs: number }) {
  const spent = timeInWork(task.stageHistory, task.stage, stampMinutes(stamp(new Date(nowSecs * 1000))));
  // Time spent is wall-clock (24-hour days); the estimates are agent time and test time (8-hour
  // days). Each is named for what it is, and nothing pretends to compare them.
  const est = [
    task.estMinutes > 0 && `${fmtMinutes(task.estMinutes)} of agent time`,
    task.estTestMinutes > 0 && `${fmtMinutes(task.estTestMinutes)} of test time`,
  ].filter(Boolean).join(" and ");
  return (
    <div className="stage-block" data-part="work">
      <span className="lbl">Work so far</span>
      <p className="wline">
        <span className="lbl">Time</span>
        <span className="small">
          {spent === null ? "not timed: no move into Building in the log" : `${fmtWall(spent)} in Building and Testing`}
          {est ? ` · estimate ${est}` : " · no time estimate"}
        </span>
      </p>
      {task.estTokenCount > 0 && (
        <p className="wline">
          <span className="lbl">Tokens</span>
          <span className="small">{fmtTokens(task.estTokenCount)} estimated · used: with 0025</span>
        </p>
      )}
      <p className="meta small">Sessions: coming with 0025</p>
      <HistoryList task={task} />
    </div>
  );
}

/** The stage moves from the log, newest first: when, from → to, who. */
function History({ task }: { task: Task }) {
  return (
    <div className="stage-block" data-part="history">
      <span className="lbl">History <span className="meta">from the log, newest first</span></span>
      <HistoryList task={task} />
    </div>
  );
}

function HistoryList({ task }: { task: Task }) {
  if (task.stageHistory.length === 0) return <p className="meta small">No stage moves in the log yet.</p>;
  return (
    <ol className="history">
      {[...task.stageHistory].reverse().map((s, i) => (
        <li key={`${s.at}-${i}`}>
          <span className="h-when">{s.at.slice(5)}</span>
          <span>
            <b>{stageLabel(s.from)} → {stageLabel(s.to)}</b> <span className="meta">· {claimant(s.who)}</span>
          </span>
        </li>
      ))}
    </ol>
  );
}

/** Backlog, Shaping and Ready: the acceptance criteria from the write-up. */
function Criteria({ body }: { body: string | null }) {
  const section = body === null ? null : criteriaSection(body);
  return (
    <div className="stage-block" data-part="criteria">
      <span className="lbl">Criteria</span>
      {body === null ? <p className="meta small">Loading…</p> : section ? <Markdown text={section} /> : <p className="meta small">No acceptance criteria written yet. Edit adds them.</p>}
    </div>
  );
}

/** The estimate: agent time plus test time (as the board's totals count it), and tokens. */
function Estimates({ task }: { task: Task }) {
  const parts = [fmtMinutes(task.estMinutes + task.estTestMinutes), fmtTokens(task.estTokenCount)].filter(Boolean).join(" · ");
  return (
    <div className="stage-block" data-part="estimates">
      <span className="lbl">Estimate</span>
      <p className="small">{parts ? `≈ ${parts}` : <span className="meta">Not estimated yet: Est, on the right.</span>}</p>
    </div>
  );
}

/** A goal's or feature's roll-up: its tasks' progress, the estimate left, a goal's due date, and
 *  its children with the stage each shows. */
function Rollup({ task, childTasks }: { task: Task; childTasks?: Task[] }) {
  const byId = new Map((childTasks ?? []).map((c) => [c.id, c]));
  const left = estimateLeft(task.progress);
  return (
    <section className="rollup-panel" aria-label="Roll-up" data-part="rollup">
      <h2>{levelLabel(task.level)} · {progressText(task.progress) || "no tasks yet"}</h2>
      {left && <p className="meta">{left}</p>}
      {task.due && <p className="meta">due {task.due}</p>}
      {task.children.length > 0 && (
        <ul className="children">
          {task.children.map((id) => {
            const c = byId.get(id);
            return (
              <li key={id}>
                {c ? <><span className="id">{id}</span> {c.title} <span className={`status-pill s-${shownStageClass(c)}`}>{shownStageLabel(c)}</span></> : id}
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
