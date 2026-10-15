import { useContext, useId, useMemo, useState } from "react";
import BlockWhy from "./BlockWhy";
import { blockedRed, blockSentence, isBlocked } from "./blockWords";
import { downstream, edgeLit, type Graph, graphMarks, hiddenDeps, isLoopEdge, layoutGraph, readyToStart, upstream } from "./graph";
import { GRAPH_RESERVE, graphLegend, Legend } from "./Legend";
import { isHuman } from "./human";
import { holder, key, stageLabel, stageOf } from "./lib";
import { PAUSE_GLYPH } from "./Pause";
import { siblingKey, taskKey } from "./taskKey";
import type { Task } from "./types";
import { Badge, Pill } from "./ui";
import YouRibbon from "./YouRibbon";
import { labelOf, Labels } from "./labels";
import ProjectName from "./ProjectName";
import { RoleNamesContext } from "./settingsData";

const W = 232; // node width: id, stage, priority and needs you fit one line
const H = 100; // node height: room for the top line to wrap onto a second
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
  // Each queue's layout, here so the legend names what every queue draws.
  const graphs = useMemo(() => new Map([...groups.shown].map(([q, ts]) => [q, layoutGraph(ts)])), [groups.shown]);
  const legend = useMemo(() => graphLegend(graphMarks([...graphs.values()])), [graphs]);
  return (
    <main className="graph-view">
      {/* The bar and the legend stay in view: only the graphs below them scroll (mu-173 R2). */}
      <div className="graph-strip">
        <div className="graph-bar">
          <Pill type="button" on={hideDone} aria-pressed={hideDone} onClick={() => setHideDone(!hideDone)}>
            Hide done
          </Pill>
          <span className="meta">Arrows point from a task to what waits on it.</span>
        </div>
        <Legend items={legend} reserve={GRAPH_RESERVE} />
      </div>
      <div className="graph-scroll">
        {queues.map((q) => (
          <QueueGraph
            key={q}
            name={multi ? q : ""}
            g={graphs.get(q) ?? EMPTY}
            tasks={groups.shown.get(q) ?? NONE}
            all={groups.every.get(q) ?? NONE}
            openKey={openKey}
            onOpen={onOpen}
          />
        ))}
        {queues.length === 0 && <p className="empty">No tasks to show.</p>}
      </div>
    </main>
  );
}

const NONE: Task[] = [];
const EMPTY: Graph = layoutGraph(NONE);

/** The state line every task shows, as a node or a chip: stage, ready, paused, blocked, who holds
 *  it. A space between the parts, so a chip reads "Done high", not "Donehigh". */
function Status({ t, ready }: { t: Task; ready: boolean }) {
  const who = holder(t, useContext(RoleNamesContext));
  const parts = [
    <span key="stage" className="g-stage">{stageLabel(t.stage)}</span>,
    t.priority === "high" && <span key="pri" className="pri pri-high">high</span>,
    ready && <span key="ready" className="g-ready">ready</span>,
    t.paused && <Badge key="paused" variant="paused">{`${PAUSE_GLYPH} paused`}</Badge>,
    blockedRed(t) ? <Badge key="blocked" variant="blocked">blocked</Badge> : isBlocked(t) && <BlockWhy key="why" t={t} short />,
    t.waitingForOwner && <Badge key="needs" variant="warn">needs you</Badge>,
    who && <span key="who" className="g-who">{who}</span>,
  ].filter((p) => p !== false && p !== "");
  return <>{parts.flatMap((p, i) => (i === 0 ? [p] : [" ", p]))}</>;
}

function QueueGraph({ name, g, tasks, all, openKey, onOpen }: { name: string; g: Graph; tasks: Task[]; all: Task[]; openKey: string | null; onOpen: (t: Task) => void }) {
  const labels = useContext(Labels);
  const uid = useId().replace(/[^a-zA-Z0-9_-]/g, "");
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
    <section className="graph-queue" aria-label={name ? `${labelOf(labels, name)} dependencies` : "Dependencies"}>
      {name && <h2><ProjectName name={name} className="" /></h2>}
      {g.nodes.length === 0 ? (
        <p className="meta">
          {all.some((t) => t.depends.some((d) => d !== t.id))
            ? "Nothing linked among the tasks shown: what they wait on is done (hidden) or filtered out."
            : <>No task here depends on another yet. Set one with <code>muster set &lt;id&gt; depends {taskKey(tasks[0] ?? all[0])}</code>.</>}
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
              const back = isLoopEdge(g, e); // closes a loop: route it below
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
            const ready = readyToStart(t);
            return (
              <button
                key={n.id}
                type="button"
                className={`g-node s-${stageOf(t)}${isHuman(t) ? " human" : ""}${blockedRed(t) ? " blocked" : ""}${ready ? " ready" : ""}${role(n.id)}`}
                style={{ left: p.x, top: p.y, width: W, height: H }}
                onClick={() => onOpen(t)}
                title={`${taskKey(t)} ${t.title}\n${stageLabel(t.stage)}${t.blockedKind ? `\n${blockSentence(t)}` : ""}${t.depends.length ? `\nwaits on ${t.depends.map((d) => siblingKey(t, d)).join(", ")}` : ""}`}
                aria-pressed={n.id === selected}
              >
                <span className="g-top">
                  <span className="id">{taskKey(t)}</span>
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
            <Pill key={t.id} type="button" on={key(t) === openKey} className={`g-loose s-${stageOf(t)}${isHuman(t) ? " human" : ""}`} onClick={() => onOpen(t)} title={t.title}>
              <span className="id">{taskKey(t)}</span>{isHuman(t) && <YouRibbon />} {t.title.length > 34 ? `${t.title.slice(0, 33)}…` : t.title} <Status t={t} ready={readyToStart(t)} />
            </Pill>
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
      {deps.length > 0 && <>waits on {deps.map((d) => (hidden.includes(d) ? `${siblingKey(t, d)} (not shown)` : siblingKey(t, d))).join(", ")}</>}
      {deps.length > 0 && neededBy > 0 && " · "}
      {neededBy > 0 && <>{neededBy} wait{neededBy === 1 ? "s" : ""} on it</>}
    </span>
  );
}
