import { useId, useMemo, useState } from "react";
import { downstream, edgeLit, type Graph, hiddenDeps, layoutGraph, readyToStart, upstream } from "./graph";
import { isHuman } from "./human";
import { holder, key, stageLabel, stageOf } from "./lib";
import type { Task } from "./types";
import YouRibbon from "./YouRibbon";

const W = 196; // node width
const H = 76; // node height
const GX = 56; // gap between ranks (room for the arrows)
const GY = 12; // gap between rows

/** What depends on what, per queue (task 0019). The selected task lights up what it waits on
 *  and what waits on it; the rest dims. Opens the task in the docked pane on click. */
export function GraphView({ tasks, all, multi, openKey, onOpen }: { tasks: Task[]; all: Task[]; multi: boolean; openKey: string | null; onOpen: (t: Task) => void }) {
  const [hideDone, setHideDone] = useState(false);
  // Per-queue lists, rebuilt only when the tasks or the switch change, so each queue's layout
  // is memoised across renders (a splitter drag, opening a task).
  const groups = useMemo(() => {
    const shown = new Map<string, Task[]>();
    const every = new Map<string, Task[]>();
    for (const t of tasks) if (!(hideDone && stageOf(t) === "done")) shown.set(t.queue, [...(shown.get(t.queue) ?? []), t]);
    for (const t of all) every.set(t.queue, [...(every.get(t.queue) ?? []), t]);
    return { shown, every };
  }, [tasks, all, hideDone]);
  const queues = useMemo(() => [...new Set(tasks.map((t) => t.queue))].sort(), [tasks]);
  return (
    <main className="graph-view">
      <div className="graph-bar">
        <button type="button" className={hideDone ? "chip on" : "chip"} aria-pressed={hideDone} onClick={() => setHideDone(!hideDone)}>
          Hide done
        </button>
        <span className="meta">
          Arrows point from a task to what waits on it. <span className="g-legend ready">Ready to start</span> has nothing left to wait on.
        </span>
      </div>
      {queues.map((q) => (
        <QueueGraph
          key={q}
          name={multi ? q : ""}
          tasks={groups.shown.get(q) ?? NONE}
          all={groups.every.get(q) ?? NONE}
          openKey={openKey}
          onOpen={onOpen}
        />
      ))}
      {queues.length === 0 && <p className="empty">No tasks to show.</p>}
    </main>
  );
}

const NONE: Task[] = [];

/** The state line every task shows, as a node or a chip: stage, ready, blocked, who holds it. */
function Status({ t, ready }: { t: Task; ready: boolean }) {
  const who = holder(t);
  return (
    <>
      <span className="g-stage">{stageLabel(t.stage)}</span>
      {t.priority === "high" && <span className="pri pri-high">high</span>}
      {ready && <span className="g-ready">ready</span>}
      {t.blocked && <span className="badge blocked">blocked</span>}
      {t.waitingForOwner && <span className="badge warn">needs you</span>}
      {who && <span className="g-who">{who}</span>}
    </>
  );
}

function QueueGraph({ name, tasks, all, openKey, onOpen }: { name: string; tasks: Task[]; all: Task[]; openKey: string | null; onOpen: (t: Task) => void }) {
  const g: Graph = useMemo(() => layoutGraph(tasks), [tasks]);
  const uid = useId().replace(/[^a-zA-Z0-9_-]/g, "");
  const byId = useMemo(() => new Map(all.map((t) => [t.id, t])), [all]);
  const selected = g.nodes.find((n) => key(n.task) === openKey)?.id ?? null;
  const up = useMemo(() => (selected ? upstream(g, selected) : new Set<string>()), [g, selected]);
  const down = useMemo(() => (selected ? downstream(g, selected) : new Set<string>()), [g, selected]);
  const at = new Map(g.nodes.map((n) => [n.id, { x: n.rank * (W + GX), y: n.order * (H + GY) }]));
  const width = Math.max(0, g.ranks * (W + GX) - GX);
  const height = Math.max(0, g.rows * (H + GY) - GY);
  const role = (id: string) => (id === selected ? " sel" : up.has(id) ? " up" : down.has(id) ? " down" : selected ? " dim" : "");
  const shown = useMemo(() => new Set(tasks.map((t) => t.id)), [tasks]);
  const neededBy = useMemo(() => {
    const n = new Map<string, number>();
    for (const e of g.edges) n.set(e.from, (n.get(e.from) ?? 0) + 1);
    return n;
  }, [g]);

  return (
    <section className="graph-queue" aria-label={name ? `${name} dependencies` : "Dependencies"}>
      {name && <h2>{name}</h2>}
      {g.nodes.length === 0 ? (
        <p className="meta">
          {all.some((t) => t.depends.some((d) => d !== t.id))
            ? "Nothing linked among the tasks shown: what they wait on is done (hidden) or filtered out."
            : <>No task here depends on another yet. Set one with <code>muster set &lt;id&gt; depends 0014</code>.</>}
        </p>
      ) : (
        <div className="graph-canvas" style={{ width, height }}>
          <svg width={width} height={height} aria-hidden="true">
            <defs>
              {(["", "-on"] as const).map((v) => (
                <marker key={v} id={`g-arrow${v}-${uid}`} viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto">
                  <path d="M0,0 L8,4 L0,8 z" className={`g-head${v}`} />
                </marker>
              ))}
            </defs>
            {g.edges.map((e) => {
              const a = at.get(e.from)!, b = at.get(e.to)!;
              const x1 = a.x + W, y1 = a.y + H / 2, x2 = b.x - 2, y2 = b.y + H / 2;
              const back = x2 <= x1; // closes a loop: route it below
              const d = back
                ? `M${x1},${y1} C${x1 + 40},${y1 + H} ${x2 - 40},${y2 + H} ${x2},${y2}`
                : `M${x1},${y1} C${(x1 + x2) / 2},${y1} ${(x1 + x2) / 2},${y2} ${x2},${y2}`;
              const on = edgeLit(e, selected, up, down);
              return <path key={`${e.from}-${e.to}`} d={d} className={`g-edge${on ? " on" : selected ? " dim" : ""}${back ? " loop" : ""}`} markerEnd={`url(#g-arrow${on ? "-on" : ""}-${uid})`} />;
            })}
          </svg>
          {g.nodes.map((n) => {
            const p = at.get(n.id)!;
            const t = n.task;
            const ready = readyToStart(t, byId);
            return (
              <button
                key={n.id}
                type="button"
                className={`g-node s-${stageOf(t)}${isHuman(t) ? " human" : ""}${t.blocked ? " blocked" : ""}${ready ? " ready" : ""}${role(n.id)}`}
                style={{ left: p.x, top: p.y, width: W, height: H }}
                onClick={() => onOpen(t)}
                title={`${t.id} ${t.title}\n${stageLabel(t.stage)}${t.blocked ? `, blocked on ${t.waitingOn || "something"}` : ""}${t.depends.length ? `\nwaits on ${t.depends.join(", ")}` : ""}`}
                aria-pressed={n.id === selected}
              >
                <span className="g-top">
                  <span className="id">{t.id}</span>
                  {isHuman(t) && <YouRibbon />}
                  {up.has(n.id) && <span className="g-rel">needed first</span>}
                  {down.has(n.id) && <span className="g-rel">waits for it</span>}
                  <Status t={t} ready={ready} />
                </span>
                <span className="g-title">{t.title || "(no title)"}</span>
                <DepsLine t={t} hidden={hiddenDeps(t, shown)} neededBy={neededBy.get(n.id) ?? 0} />
              </button>
            );
          })}
        </div>
      )}
      {g.loose.length > 0 && (
        <div className="graph-loose">
          <span className="meta">Not linked to any task ({g.loose.length})</span>
          {g.loose.map((t) => (
            <button key={t.id} type="button" className={`chip g-loose s-${stageOf(t)}${isHuman(t) ? " human" : ""}${key(t) === openKey ? " on" : ""}`} onClick={() => onOpen(t)} title={t.title}>
              <span className="id">{t.id}</span>{isHuman(t) && <YouRibbon />} {t.title.length > 34 ? `${t.title.slice(0, 33)}…` : t.title} <Status t={t} ready={readyToStart(t, byId)} />
            </button>
          ))}
        </div>
      )}
    </section>
  );
}

/** In words, what the arrows say: what it waits on (naming any not drawn here) and how many wait on it. */
function DepsLine({ t, hidden, neededBy }: { t: Task; hidden: string[]; neededBy: number }) {
  const deps = t.depends.filter((d) => d !== t.id);
  if (!deps.length && !neededBy) return null;
  return (
    <span className="g-deps">
      {deps.length > 0 && <>waits on {deps.map((d) => (hidden.includes(d) ? `${d} (not shown)` : d)).join(", ")}</>}
      {deps.length > 0 && neededBy > 0 && " · "}
      {neededBy > 0 && <>{neededBy} wait{neededBy === 1 ? "s" : ""} on it</>}
    </span>
  );
}
