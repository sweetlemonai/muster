import { Fragment, useMemo, useRef, useState, type ReactNode } from "react";
import { lastLabel, loadOrder, nextOrder, type OrderMemo, type ProjectRow, projectRows, SIDEBAR_ORDERS, storeOrder, sumActivity, type ProjectActivity, type SidebarOrder, type Sums } from "./activity";
import { projectCount } from "./Backlog";
import { SideLimit } from "./CheckLimit";
import { needsYouCount } from "./human";
import { byCategory, LABEL_MAX } from "./labels";
import { inProgress } from "./lib";
import { useLook } from "./modes";
import { PAUSE_GLYPH } from "./Pause";
import { QuietPing } from "./Ping";
import { pressed } from "./shortcuts";
import type { Activity, Note, QueueInfo, Task } from "./types";

/** The sidebar's projects (0037): All first, then the order chips and one row per queue under its
 *  category's heading (0044), each called by its label and saying whether it's being worked on
 *  now, or stalled, and when it last moved. `now` is the state's clock, seconds since 1970.
 *  `onLabel` saves a label set by double-clicking a row's name (by the project's key, its folder);
 *  without it, names can't be edited here. */
export default function ProjectList({ queues, tasks, notes, activity, now, current, onPick, onLabel }: {
  queues: QueueInfo[];
  tasks: Task[];
  notes: Note[];
  activity: Activity[];
  now: number;
  current: string;
  onPick: (queue: string) => void;
  onLabel?: (key: string, label: string) => Promise<void>;
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
  const totalOf = (r: ProjectRow) => projectCount(counts.get(r.name) ?? {}, notes, r.name);
  // The project whose label is being set in place, by its name.
  const [editing, setEditing] = useState<string | null>(null);
  // Focus (0038) narrows the sidebar to the names, the working dot, stalled and the needs-you and paused counts;
  // a stalled project keeps its Ask all quiet sessions under it.
  const full = useLook("sidebar") === "full";
  return (
    <nav className="projects-nav">
      <ProjectItem name="All" total={tasks.length + notes.length} tasks={tasks} on={current === "all"} onClick={() => onPick("all")} working={sums.working > 0} stalled={sums.stalled > 0 ? `${sums.stalled} stalled` : ""}>
        <SumsLine s={sums} now={now} />
      </ProjectItem>
      {full && <div className="side-label">Projects</div>}
      {full && <OrderChips value={order} onChange={pick} />}
      {byCategory(shown).map((g) => (
        <div key={g.name} className="side-group" role="group" aria-label={g.name}>
          <CategoryHead name={g.name} rows={g.rows} total={g.rows.reduce((n, r) => n + totalOf(r), 0)} />
          {g.rows.map((r) => {
            const q = queues.find((x) => x.name === r.name);
            if (editing === r.name && onLabel && q) {
              return <SideLabelEdit key={r.name} name={r.name} label={r.label} onSave={(v) => onLabel(q.path, labelToSave(v, r.name))} onDone={() => setEditing(null)} />;
            }
            return (
              <Fragment key={r.name}>
                <ProjectItem name={r.label} title={r.title} total={totalOf(r)} tasks={r.tasks} on={current === r.name} onClick={() => onPick(r.name)} onRename={onLabel ? () => setEditing(r.name) : undefined} working={r.activity.working > 0} stalled={r.activity.stalled ? "stalled" : ""} alert={<SideLimit q={q} />}>
                  <ActivityLine a={r.activity} now={now} />
                </ProjectItem>
                {/* A stalled project offers to ask its quiet sessions for a check-in (0039), under its
                    row, since the row is itself a button. */}
                {r.activity.stalled && <QuietPing tasks={r.tasks} now={now} />}
              </Fragment>
            );
          })}
        </div>
      ))}
    </nav>
  );
}

/** One project in the sidebar: its name (its label, 0044) and count, what's in progress, what's
 *  paused (0045) and what needs you (the needs-you count: your own tasks to do and doing, and what
 *  the agents wait on you for), and under them its activity line. Narrowed (Focus, 0038), the
 *  name, the working dot, "stalled" in the attention colour (the activity line's word, or "2
 *  stalled" on All), the needs-you count and the paused count. `alert` shows in every mode, under
 *  them: over its Your check limit (0050), how many wait and the limit. With `onRename`, a double
 *  click on the name sets its label. */
export function ProjectItem({ name, title, total, tasks, on, onClick, onRename, working = false, stalled = "", alert, children }: { name: string; title?: string; total: number; tasks: Task[]; on: boolean; onClick: () => void; onRename?: () => void; working?: boolean; stalled?: string; alert?: ReactNode; children: ReactNode }) {
  const doing = tasks.filter(inProgress).length;
  const needs = needsYouCount(tasks);
  const full = useLook("sidebar") === "full";
  const paused = tasks.filter((t) => t.paused).length;
  return (
    <button className={on ? "side-item on" : "side-item"} onClick={onClick} title={title}>
      <span className="side-line">
        {!full && working && <i className="dot side-dot" title={`Being worked on now: ${WORKING_HINT}`} />}
        <span className="side-name" title={onRename ? "Double-click to change the label" : undefined} onDoubleClick={onRename}>{name}</span>
        {!full && stalled && <span className="side-stalled" title={`Stalled: ${STALLED_HINT}`}>{stalled}</span>}
        <span className="side-badges">
          {needs > 0 && <span className="count warn" title={`${needs} need you`}>{needs}</span>}
          {full && doing > 0 && <span className="side-doing" title={`${doing} in progress (Building or Testing)`}>{doing}</span>}
          {paused > 0 && <span className="side-paused" title={`${paused} paused`}>{`${PAUSE_GLYPH} ${paused}`}</span>}
          {full && <small>{total}</small>}
        </span>
      </span>
      {full && children}
      {alert}
    </button>
  );
}

/** A category's heading (0044), always shown, above its projects: its name, what needs you in it,
 *  and, but in Focus's narrow sidebar, how many of its tasks are being worked on and its tasks. */
function CategoryHead({ name, rows, total }: { name: string; rows: ProjectRow[]; total: number }) {
  const full = useLook("sidebar") === "full";
  const needs = rows.reduce((n, r) => n + r.needs, 0);
  const working = rows.reduce((n, r) => n + r.activity.working, 0);
  const projects = `${rows.length} ${rows.length === 1 ? "project" : "projects"}`;
  return (
    <div className="side-cat">
      <span className="side-cat-name">{name}</span>
      <span className="side-badges">
        {needs > 0 && <span className="count warn" title={`${needs} need${needs === 1 ? "s" : ""} you in ${name}`}>{needs}</span>}
        {full && working > 0 && (
          <span className="side-working" title={`${working} ${working === 1 ? "task" : "tasks"} being worked on now in ${name}: ${WORKING_HINT}`}>
            <i className="dot" aria-hidden="true" />
            {working} working
          </span>
        )}
        {full && <small title={`${total} ${total === 1 ? "task" : "tasks"} in ${projects}`}>{total}</small>}
      </span>
    </div>
  );
}

/** What a label typed in place saves: the project's own name typed back is no label at all. */
export const labelToSave = (typed: string, name: string) => (typed.trim() === name ? "" : typed.trim());

/** A project's label set in place (0044), in its row's place: the field, with the real name and
 *  the keys under it. ↩ or leaving the field saves, Esc keeps the label as it was; a refusal shows
 *  under the field, which stays open. */
export function SideLabelEdit({ name, label, onSave, onDone }: { name: string; label: string; onSave: (label: string) => Promise<void>; onDone: () => void }) {
  const [error, setError] = useState("");
  // Saved or left once: ↩ blurs the field too, and Esc's blur saves nothing.
  const settled = useRef(false);
  const save = (v: string) => {
    if (settled.current) return;
    settled.current = true;
    if (v.trim() === label) {
      onDone();
      return;
    }
    onSave(v).then(onDone, (e: unknown) => {
      settled.current = false;
      setError(String(e));
    });
  };
  return (
    <div className="side-item on side-editing">
      <input
        className="side-label-input"
        autoFocus
        defaultValue={label}
        maxLength={LABEL_MAX}
        aria-label={`Label for ${name}`}
        placeholder={name}
        onKeyDown={(e) => {
          if (pressed(e, "label-save")) save(e.currentTarget.value);
          if (pressed(e, "label-revert")) {
            // Esc here leaves the label as it was, not the pane.
            e.stopPropagation();
            settled.current = true;
            onDone();
          }
        }}
        onBlur={(e) => save(e.currentTarget.value)}
      />
      <small className="meta">{name} · ↩ saves, Esc keeps it</small>
      {error && <small className="err" role="alert">{error}</small>}
    </div>
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
