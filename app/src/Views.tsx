import { useMemo, useState } from "react";
import { estimateText } from "./fields";
import BlockWhy from "./BlockWhy";
import { blockedRed, blockLead, isBlocked } from "./blockWords";
import { ago, holder, key, stageLabel, stageOf } from "./lib";
import { PAUSE_GLYPH } from "./Pause";
import type { Task } from "./types";
import { Badge, Pill, StatusPill } from "./ui";
import {
  axis, barTitle, layout, nowAt, parseScale, SCALES, segmentTitle, parseTableSort, sortTasks, TABLE_SORT_KEY, spans, timeWindow, wallMinutes, windowLabel,
  type Placed, type Scale, type SortKey, type Tick,
} from "./viewData";
import { BandHead } from "./Groups";
import { estimateLeft, type Group, type GroupBy, isWork, levelLabel, progressText, shownStageClass, shownStageLabel } from "./levels";
import { Hl, HlId } from "./FilterBar";
import { isHuman } from "./human";
import YouRibbon from "./YouRibbon";
import ProjectName, { TaskKey } from "./ProjectName";

const COLS: { key: SortKey; label: string }[] = [
  { key: "queue", label: "Project" },
  { key: "id", label: "Id" },
  { key: "title", label: "Title" },
  { key: "kind", label: "Type" },
  { key: "priority", label: "Priority" },
  { key: "stage", label: "Stage" },
  { key: "who", label: "On it / waiting on" },
  { key: "estimate", label: "Estimate" },
  { key: "captured", label: "Captured" },
  { key: "changed", label: "Changed" },
];

/** Every task as a row: sortable by any column, everything at a glance. Grouped (by feature or
 *  goal), each band's heading row is the feature or goal, and its rows are sorted inside it. */
export function TableView({ tasks, now, multi, openKey, onOpen, groups, by: groupBy, words }: {
  tasks: Task[];
  now: number;
  multi: boolean;
  openKey: string | null;
  onOpen: (t: Task) => void;
  groups?: Group[];
  by?: GroupBy;
  /** The search's words, marked in ids, titles and tags (task 0032). */
  words: string[];
}) {
  // The last sort is kept across restarts (owner, 2026-10-13).
  const [sort, setSort] = useState(() => {
    try {
      return parseTableSort(localStorage.getItem(TABLE_SORT_KEY));
    } catch {
      return parseTableSort(null); // no storage (off, or a test run): the default
    }
  });
  const { by, desc } = sort;
  const sortBy = (next: { by: SortKey; desc: boolean }) => {
    setSort(next);
    try {
      localStorage.setItem(TABLE_SORT_KEY, JSON.stringify(next));
    } catch {
      /* storage full or off: the sort still applies for now */
    }
  };
  const rows = useMemo(() => sortTasks(tasks, by, desc), [tasks, by, desc]);
  const cols = multi ? COLS : COLS.filter((c) => c.key !== "queue");
  const row = (t: Task) => <Row key={key(t)} t={t} now={now} multi={multi} active={openKey === key(t)} onOpen={onOpen} words={words} />;
  return (
    <main className="table-view">
      <table className="tasks-table">
        <thead>
          <tr>
            {cols.map((c) => (
              <th key={c.key} aria-sort={by === c.key ? (desc ? "descending" : "ascending") : "none"}>
                <button className="sort" onClick={() => sortBy(by === c.key ? { by, desc: !desc } : { by: c.key, desc: false })}>
                  {c.label}
                  {by === c.key ? (desc ? " ↓" : " ↑") : ""}
                </button>
              </th>
            ))}
            <th className="plain">Tags</th>
          </tr>
        </thead>
        {groups && groupBy && groupBy !== "none"
          ? groups.map((g) => (
              <tbody key={g.head ? key(g.head) : "none"} className="band-rows">
                <tr className="group-row">
                  <td colSpan={cols.length + 1}>
                    <BandHead group={g} by={groupBy} multi={multi} active={!!g.head && openKey === key(g.head)} onOpen={onOpen} />
                  </td>
                </tr>
                {sortTasks(g.items, by, desc).map(row)}
              </tbody>
            ))
          : <tbody>{rows.map(row)}</tbody>}
      </table>
      {rows.length === 0 && <p className="empty">No tasks match.</p>}
    </main>
  );
}

/** One row of the table. A goal or feature says so, with its progress, the stage its tasks give
 *  it and the estimate left of its open tasks. */
function Row({ t, now, multi, active, onOpen, words }: { t: Task; now: number; multi: boolean; active: boolean; onOpen: (t: Task) => void; words: string[] }) {
  const parentLevel = !isWork(t);
  const own = estimateText(t);
  return (
    <tr className={`${active ? "active " : ""}s-${parentLevel ? shownStageClass(t) : stageOf(t)}${parentLevel ? " level-item" : ""}`} onClick={() => onOpen(t)}>
      {multi && <ProjectName name={t.queue} as="td" />}
      <TaskKey t={t} as="td"><HlId t={t} words={words} /></TaskKey>
      <td className="t-title">
        {isHuman(t) && <YouRibbon />}
        {parentLevel && <Badge variant="level">{levelLabel(t.level)}</Badge>}
        {t.waitingForOwner && <Badge variant="warn">needs you</Badge>} <Hl text={t.title} words={words} />
        {parentLevel && <span className="meta rollup">{progressText(t.progress)}</span>}
        {t.openQuestions > 0 && <Badge>{t.openQuestions} open Q</Badge>}
      </td>
      <td className={`kind ${t.kind}`}>{t.kind}</td>
      <td><span className={`pri pri-${t.priority || "normal"}`}>{t.priority || "normal"}</span></td>
      <td>
        {parentLevel ? <StatusPill status={shownStageClass(t)}>{shownStageLabel(t)}</StatusPill> : <StatusPill status={stageOf(t)}>{stageLabel(t.stage)}</StatusPill>}
        {t.paused && <StatusPill status="paused">{`${PAUSE_GLYPH} Paused`}</StatusPill>}
        {blockedRed(t) && <StatusPill status="blocked">blocked</StatusPill>}
      </td>
      <td className="meta">{isBlocked(t) ? <BlockWhy t={t} short pill={false} /> : holder(t)}</td>
      <td className="meta est-cell">{own || (parentLevel ? estimateLeft(t.progress) : "")}</td>
      <td className="meta">{t.captured}</td>
      <td className="meta">{ago(now, t.modified)}</td>
      <td>{t.tags.map((g) => <span key={g} className="tag">#<Hl text={g} words={words} /></span>)}</td>
    </tr>
  );
}

const SCALE_KEY = "muster.timelineScale";
const CURRENT: Record<Scale, string> = { day: "Now", week: "This week", month: "This month" };
/** Pixels per slot: an hour at Day, a quarter day at Week, a day at Month. */
const SLOT_PX: Record<Scale, number> = { day: 40, week: 32, month: 30 };
const pct = (x: number) => `${(x * 100).toFixed(3)}%`;

/** Each task as a bar in a day, week or month: waiting, then each stage it held, with a now line. */
export function TimelineView({ tasks, now, multi, openKey, onOpen, words }: { tasks: Task[]; now: number; multi: boolean; openKey: string | null; onOpen: (t: Task) => void; words: string[] }) {
  const [scale, setScale] = useState<Scale>(() => parseScale(localStorage.getItem(SCALE_KEY)));
  const [offset, setOffset] = useState(0);
  // Wall-clock minutes (viewData's unit); changes once a minute, not on every poll.
  const at = wallMinutes(new Date(now * 1000));
  const list = useMemo(() => spans(tasks, at), [tasks, at]);
  const win = useMemo(() => timeWindow(scale, at, offset), [scale, at, offset]);
  const rows = useMemo(() => layout(list, win, scale), [list, win, scale]);
  const ticks = useMemo(() => axis(scale, win, at), [scale, win, at]);
  const pick = (s: Scale) => {
    setScale(s);
    setOffset(0);
    localStorage.setItem(SCALE_KEY, s);
  };
  return (
    <main className="timeline">
      <TimelineControls scale={scale} offset={offset} label={windowLabel(scale, win)} onScale={pick} onStep={(d) => setOffset((o) => o + d)} onToday={() => setOffset(0)} />
      <div className="tl-scroll">
        <TimelineChart scale={scale} rows={rows} ticks={ticks} now={nowAt(win, at)} multi={multi} openKey={openKey} onOpen={onOpen} words={words} />
        {rows.length === 0 && <p className="empty">{tasks.length ? `No work ${scale === "day" ? "in these 24 hours" : `in this ${scale}`}.` : "No tasks match."}</p>}
        {(list.notStarted ?? 0) > 0 && (
          <p className="meta tl-not-started">{list.notStarted} {list.notStarted === 1 ? "task hasn't" : "tasks haven't"} started yet (or the log has no times for them), so they aren't drawn.</p>
        )}
        <p className="meta tl-legend">
          Each bar is a task's work: it starts when the task was claimed and ends when the work finished (it reached Your check or
          Done), one segment per stage in that stage's colour, from the times muster logged. The thin dotted tail after it is the
          time it has waited for your check. Tasks still being worked run to the now line. Red hatching is time blocked, grey
          hatching time paused (not counted as work). Day is the
          last 24 hours; Week shows quarter days, Month days.
        </p>
      </div>
    </main>
  );
}

/** The scale chips (all three always visible) and the ◀ current ▶ stepper, with the window's name. */
export function TimelineControls({ scale, offset, label, onScale, onStep, onToday }: {
  scale: Scale; offset: number; label: string; onScale: (s: Scale) => void; onStep: (by: number) => void; onToday: () => void;
}) {
  return (
    <div className="tl-controls" role="toolbar" aria-label="Timeline scale">
      <span className="meta">Scale</span>
      {SCALES.map((s) => (
        <Pill key={s.id} on={s.id === scale} aria-pressed={s.id === scale} onClick={() => onScale(s.id)}>
          {s.label}
        </Pill>
      ))}
      <span className="tl-nav">
        <Pill onClick={() => onStep(-1)} title={`Previous ${scale}`} aria-label={`Previous ${scale}`}>◀</Pill>
        <Pill onClick={onToday} disabled={offset === 0}>{CURRENT[scale]}</Pill>
        <Pill onClick={() => onStep(1)} title={`Next ${scale}`} aria-label={`Next ${scale}`}>▶</Pill>
        <span className="tl-window">{label}</span>
      </span>
    </div>
  );
}

/** The axis, one row per placed bar (label left, bar with its segments), and the now line. */
export function TimelineChart({ scale, rows, ticks, now, multi, openKey, onOpen, words = [] }: {
  scale: Scale; rows: Placed[]; ticks: Tick[]; now: number | null; multi: boolean; openKey: string | null; onOpen: (t: Task) => void;
  /** The search's words, marked in each row's id and title (task 0032). */
  words?: string[];
}) {
  const slotPx = SLOT_PX[scale];
  const slots = ticks.length * (scale === "week" ? 4 : 1);
  const style = { ["--track" as string]: `${slots * slotPx}px`, ["--major" as string]: `${scale === "week" ? slotPx * 4 : slotPx}px`, ["--minor" as string]: `${slotPx}px` };
  return (
    <div className={`tl-grid tl-${scale}`} style={style}>
      <div className="tl-head">
        <span className="tl-label meta">{rows.length} {rows.length === 1 ? "task" : "tasks"}</span>
        <span className="tl-track">
          {ticks.map((k) => (
            <span key={k.at} className={k.current ? "tl-tick now" : "tl-tick"} style={{ left: pct(k.left), width: pct(k.width) }}>{k.label}</span>
          ))}
        </span>
      </div>
      {rows.map((p) => {
        const t = p.span.task;
        return (
          <button key={key(t)} className={openKey === key(t) ? "tl-row active" : "tl-row"} onClick={() => onOpen(t)}>
            <span className="tl-label">
              {isHuman(t) && <YouRibbon />}
              {multi && <ProjectName name={t.queue} />} <TaskKey t={t}><HlId t={t} words={words} /></TaskKey> <Hl text={t.title} words={words} />
            </span>
            <span className="tl-track">
              <span
                className={`tl-bar s-${stageOf(t)}${p.span.open ? " open" : ""}${blockedRed(t) ? " blocked" : ""}${t.waitingForOwner ? " needs-you" : ""}${p.clipStart ? " clip-start" : ""}${p.clipEnd ? " clip-end" : ""}`}
                style={{ left: pct(p.left), width: pct(p.width) }}
                title={barTitle(t, p.span.timed)}
              >
                {p.segments.map((g) => (
                  <span
                    key={g.seg.from}
                    className={`tl-seg ${g.seg.stage === "waiting" ? "waiting" : `s-${g.seg.stage}`}${g.seg.fuzzy ? " fuzzy" : ""}`}
                    style={{ left: pct(g.left), width: pct(g.width) }}
                    title={segmentTitle(g.seg)}
                  />
                ))}
                {p.blocks.map((b) => (
                  <span key={`b${b.left}`} className="tl-block" style={{ left: pct(b.left), width: pct(b.width) }} title={blockLead(b.kind)} />
                ))}
                {p.pauses.map((g) => (
                  <span key={`p${g.left}`} className="tl-pause" style={{ left: pct(g.left), width: pct(g.width) }} />
                ))}
                <span className="tl-bar-text">{stageLabel(t.stage)}{t.paused ? " · paused" : ""}{isBlocked(t) ? " · blocked" : ""}{t.priority === "high" ? " · high" : ""}</span>
              </span>
            </span>
          </button>
        );
      })}
      {now !== null && (
        <span className="tl-now-layer" aria-hidden="true">
          <span className="tl-now" style={{ left: pct(now) }} />
        </span>
      )}
    </div>
  );
}
