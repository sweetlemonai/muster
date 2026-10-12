import { useMemo, useState } from "react";
import { ago, claimant, key } from "./lib";
import type { Task } from "./types";
import { bars, dayLabel, sortTasks, type SortKey } from "./viewData";

const COLS: { key: SortKey; label: string }[] = [
  { key: "queue", label: "Project" },
  { key: "id", label: "Id" },
  { key: "title", label: "Title" },
  { key: "kind", label: "Type" },
  { key: "priority", label: "Priority" },
  { key: "status", label: "Status" },
  { key: "who", label: "On it / waiting on" },
  { key: "estimate", label: "Estimate" },
  { key: "captured", label: "Captured" },
  { key: "changed", label: "Changed" },
];

/** Every task as a row: sortable by any column, everything at a glance. */
export function TableView({ tasks, now, multi, openKey, onOpen }: { tasks: Task[]; now: number; multi: boolean; openKey: string | null; onOpen: (t: Task) => void }) {
  const [by, setBy] = useState<SortKey>("status");
  const [desc, setDesc] = useState(false);
  const rows = useMemo(() => sortTasks(tasks, by, desc), [tasks, by, desc]);
  const cols = multi ? COLS : COLS.filter((c) => c.key !== "queue");
  return (
    <main className="table-view">
      <table className="tasks-table">
        <thead>
          <tr>
            {cols.map((c) => (
              <th key={c.key} aria-sort={by === c.key ? (desc ? "descending" : "ascending") : "none"}>
                <button onClick={() => (by === c.key ? setDesc(!desc) : (setBy(c.key), setDesc(false)))}>
                  {c.label}
                  {by === c.key ? (desc ? " ↓" : " ↑") : ""}
                </button>
              </th>
            ))}
            <th className="plain">Tags</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((t) => (
            <tr key={key(t)} className={`${openKey === key(t) ? "active " : ""}st-${t.status}`} onClick={() => onOpen(t)}>
              {multi && <td className="q">{t.queue}</td>}
              <td className="id">{t.id}</td>
              <td className="t-title">
                {t.waitingForOwner && <span className="badge warn">needs you</span>} {t.title}
                {t.openQuestions > 0 && <span className="badge">{t.openQuestions} open Q</span>}
              </td>
              <td className={`kind ${t.kind}`}>{t.kind}</td>
              <td><span className={`pri pri-${t.priority || "normal"}`}>{t.priority || "normal"}</span></td>
              <td><span className={`status-pill s-${t.status}`}>{t.status}</span></td>
              <td className="meta">{t.status === "doing" ? claimant(t.claimedBy) : t.status === "blocked" ? t.waitingOn : ""}</td>
              <td className="meta est-cell">{[t.estTime, t.estTokens].filter(Boolean).join(" · ")}</td>
              <td className="meta">{t.captured}</td>
              <td className="meta">{ago(now, t.modified)}</td>
              <td>{t.tags.map((g) => <span key={g} className="tag">#{g}</span>)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {rows.length === 0 && <p className="empty">No tasks match.</p>}
    </main>
  );
}

/** Each task as a bar from capture to done (or today); the darker part is from the claim on. */
export function TimelineView({ tasks, now, multi, openKey, onOpen }: { tasks: Task[]; now: number; multi: boolean; openKey: string | null; onOpen: (t: Task) => void }) {
  const today = Math.floor(now / 86400);
  const { bars: list, from, to } = useMemo(() => bars(tasks, today), [tasks, today]);
  const days = to - from + 1;
  const dayW = Math.max(28, Math.min(90, Math.floor(900 / days)));
  const ticks = Array.from({ length: days }, (_, i) => from + i);
  return (
    <main className="timeline">
      <div className="tl-grid" style={{ ["--day" as string]: `${dayW}px`, ["--days" as string]: String(days) }}>
        <div className="tl-head">
          <span className="tl-label meta">{list.length} tasks</span>
          <span className="tl-track">
            {ticks.map((d) => (
              <span key={d} className={d === today ? "tl-tick today" : "tl-tick"} style={{ left: (d - from) * dayW }}>{dayLabel(d)}</span>
            ))}
          </span>
        </div>
        {list.map((b) => (
          <button key={key(b.task)} className={openKey === key(b.task) ? "tl-row active" : "tl-row"} onClick={() => onOpen(b.task)}>
            <span className="tl-label">
              {multi && <span className="q">{b.task.queue}</span>} <span className="id">{b.task.id}</span> {b.task.title}
            </span>
            <span className="tl-track">
              <span className="tl-today" style={{ left: (today - from) * dayW + dayW / 2 }} />
              <span
                className={`tl-bar s-${b.task.status}${b.open ? " open" : ""}${b.task.waitingForOwner ? " needs-you" : ""}`}
                style={{ left: (b.start - from) * dayW + 3, width: (b.end - b.start + 1) * dayW - 6 }}
                title={`${b.task.status} · captured ${b.task.captured}${b.task.claimedOn ? ` · started ${b.task.claimedOn}` : ""}${b.task.doneOn ? ` · done ${b.task.doneOn}` : ""}`}
              >
                {b.work !== null && <span className="tl-work" style={{ left: (b.work - b.start) * dayW, right: 0 }} />}
                <span className="tl-bar-text">{b.task.status}{b.task.priority === "high" ? " · high" : ""}</span>
              </span>
            </span>
          </button>
        ))}
      </div>
      {list.length === 0 && <p className="empty">No tasks match.</p>}
      <p className="meta tl-legend">
        A bar runs from the day a task was captured to the day it was done, or to today while it's open. The darker part starts
        the day an agent claimed it. Dates come from the task files and their logs.
      </p>
    </main>
  );
}
