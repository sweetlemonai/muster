import { Fragment, useMemo, useRef, useState, type ReactNode } from "react";
import { lastLabel, loadOrder, nextOrder, type OrderMemo, projectRows, SIDEBAR_ORDERS, storeOrder, sumActivity, type ProjectActivity, type SidebarOrder, type Sums } from "./activity";
import { projectCount } from "./Backlog";
import { needsYouCount } from "./human";
import { inProgress } from "./lib";
import { QuietPing } from "./Ping";
import type { Activity, Note, QueueInfo, Task } from "./types";

/** The sidebar's projects (0037): All first, then the order chips and one row per queue, each
 *  saying whether it's being worked on now, or stalled, and when it last moved. `now` is the
 *  state's clock, seconds since 1970. */
export default function ProjectList({ queues, tasks, notes, activity, now, current, onPick }: {
  queues: QueueInfo[];
  tasks: Task[];
  notes: Note[];
  activity: Activity[];
  now: number;
  current: string;
  onPick: (queue: string) => void;
}) {
  const [order, setOrder] = useState<SidebarOrder>(() => loadOrder());
  const pick = (o: SidebarOrder) => {
    setOrder(o);
    storeOrder(o);
  };
  const rows = useMemo(() => projectRows(queues, tasks, activity, now), [queues, tasks, activity, now]);
  // The order holds between polls unless what defines it changed (nextOrder), so rows don't jump.
  const memo = useRef<OrderMemo | null>(null);
  memo.current = nextOrder(memo.current, rows, order);
  const byName = new Map(rows.map((r) => [r.name, r]));
  const shown = memo.current.names.flatMap((n) => byName.get(n) ?? []);
  const sums = useMemo(() => sumActivity(rows.map((r) => r.activity)), [rows]);
  const counts = useMemo(() => new Map(queues.map((q) => [q.name, q.counts])), [queues]);
  return (
    <nav className="projects-nav">
      <ProjectItem name="All" total={tasks.length + notes.length} tasks={tasks} on={current === "all"} onClick={() => onPick("all")}>
        <SumsLine s={sums} now={now} />
      </ProjectItem>
      <div className="side-label">Projects</div>
      <OrderChips value={order} onChange={pick} />
      {shown.map((r) => (
        <Fragment key={r.name}>
          <ProjectItem name={r.name} title={r.title} total={projectCount(counts.get(r.name) ?? {}, notes, r.name)} tasks={r.tasks} on={current === r.name} onClick={() => onPick(r.name)}>
            <ActivityLine a={r.activity} now={now} />
          </ProjectItem>
          {/* A stalled project offers to ask its quiet sessions for a check-in (0039), under its
              row, since the row is itself a button. */}
          {r.activity.stalled && <QuietPing tasks={r.tasks} now={now} />}
        </Fragment>
      ))}
    </nav>
  );
}

/** One project in the sidebar: its name and count, what's in progress and what needs you (the
 *  needs-you count: your own tasks to do and doing, and what the agents wait on you for), and
 *  under them its activity line. */
export function ProjectItem({ name, title, total, tasks, on, onClick, children }: { name: string; title?: string; total: number; tasks: Task[]; on: boolean; onClick: () => void; children: ReactNode }) {
  const doing = tasks.filter(inProgress).length;
  const needs = needsYouCount(tasks);
  return (
    <button className={on ? "side-item on" : "side-item"} onClick={onClick} title={title}>
      <span className="side-line">
        <span className="side-name">{name}</span>
        <span className="side-badges">
          {needs > 0 && <span className="count warn" title={`${needs} need you`}>{needs}</span>}
          {doing > 0 && <span className="side-doing" title={`${doing} in progress (Building or Testing)`}>{doing}</span>}
          <small>{total}</small>
        </span>
      </span>
      {children}
    </button>
  );
}

const WORKING_HINT = "a live session, or a check-in or heartbeat in the last 30 minutes";
const STALLED_HINT = "work in Building, but nothing for over 30 minutes: no check-in, heartbeat, stage change, commit or file change";

/** A project's activity, quietly: the Building-colour dot and "2 working", or "stalled" in the
 *  attention colour, then when it last moved. The line is always there, so rows don't jump. */
export function ActivityLine({ a, now }: { a: ProjectActivity; now: number }) {
  return (
    <span className="side-activity">
      <span className="side-state">
        {a.working > 0 ? (
          <span className="side-working" title={`${a.working} ${a.working === 1 ? "task" : "tasks"} being worked on now: ${WORKING_HINT}`}>
            <i className="dot" aria-hidden="true" />
            {a.working} working
          </span>
        ) : a.stalled ? (
          <span className="side-stalled" title={`Stalled: ${STALLED_HINT}`}>stalled</span>
        ) : null}
      </span>
      <When m={a.last} now={now} />
    </span>
  );
}

/** The All projects row's sums: projects being worked on, projects stalled, the newest activity. */
export function SumsLine({ s, now }: { s: Sums; now: number }) {
  const projects = (n: number) => `${n} ${n === 1 ? "project" : "projects"}`;
  return (
    <span className="side-activity">
      <span className="side-state">
        {s.working > 0 && (
          <span className="side-working" title={`${projects(s.working)} being worked on now: ${WORKING_HINT}`}>
            <i className="dot" aria-hidden="true" />
            {s.working} working
          </span>
        )}
        {s.working > 0 && s.stalled > 0 && " · "}
        {s.stalled > 0 && <span className="side-stalled" title={`${projects(s.stalled)} stalled: ${STALLED_HINT}`}>{s.stalled} stalled</span>}
      </span>
      <When m={s.last} now={now} />
    </span>
  );
}

function When({ m, now }: { m: number | null; now: number }) {
  return <span className="side-when" title={m === null ? "Nothing says when this was last worked on" : `Last activity: ${lastLabel(m, now)}`}>{lastLabel(m, now)}</span>;
}

/** The sidebar's order, as chips: always on screen, the current one pressed. */
export function OrderChips({ value, onChange }: { value: SidebarOrder; onChange: (o: SidebarOrder) => void }) {
  return (
    <span className="side-order" role="group" aria-label="Order projects">
      {SIDEBAR_ORDERS.map((o) => (
        <button key={o.id} type="button" className={value === o.id ? "chip on" : "chip"} aria-pressed={value === o.id} title={o.title} onClick={() => onChange(o.id)}>
          {o.label}
        </button>
      ))}
    </span>
  );
}
