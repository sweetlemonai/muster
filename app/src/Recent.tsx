import { useEffect, useMemo, useState } from "react";
import { api } from "./api";
import { fmtMinutes, key, shortTokens } from "./lib";
import { builtBy, byKind, dayGroups, type DayGroup, type Finish, finishes, KIND_FILTERS, type KindFilter, loadKind, storeKind, whenText } from "./recentData";
import type { Task } from "./types";

/** A task's time and tokens, the Work section's totals (0025), or why they couldn't be read. */
export type Cost = { minutes: number; tokens: number; read: number } | { error: string };

/** The time and tokens line: "12 min · 450k tokens", or why there's none. Undefined is still reading. */
export function costText(c: Cost | undefined): string {
  if (!c) return "time and tokens…";
  if ("error" in c) return "time and tokens unknown";
  if (c.read === 0) return "no transcripts on this Mac";
  return `${fmtMinutes(c.minutes) || "0 min"} · ${shortTokens(c.tokens)} tokens`;
}

/** Costs read so far, by "queue/id", with the file's modified time they were read at. The core
 *  caches the transcripts too; this only saves asking again for a row that didn't change. Bounded:
 *  the oldest entry goes past COSTS_CAP. */
const COSTS_CAP = 200;
const costCache = new Map<string, { modified: number; cost: Cost }>();

/** The time and tokens of `tasks` (the rows in view), one task at a time so a long list doesn't
 *  ask the core for every transcript at once. Asked again when a task's file changes; a late
 *  answer after the rows changed is dropped. */
function useCosts(tasks: Task[]): Map<string, Cost> {
  const [costs, setCosts] = useState<Map<string, Cost>>(() => new Map());
  const wanted = tasks.map((t) => `${key(t)}@${t.modified}`).join(" ");
  useEffect(() => {
    let live = true;
    const show = () => setCosts(new Map([...costCache].map(([k, v]) => [k, v.cost])));
    void (async () => {
      show();
      for (const t of tasks) {
        const k = key(t);
        if (costCache.get(k)?.modified === t.modified) continue;
        let cost: Cost;
        try {
          const w = await api.taskSessions(t.queue, t.id);
          cost = { minutes: w.totals.minutes, tokens: w.totals.totalTokens, read: w.totals.read };
        } catch (e) {
          cost = { error: String(e) };
        }
        costCache.delete(k);
        costCache.set(k, { modified: t.modified, cost });
        if (costCache.size > COSTS_CAP) costCache.delete(costCache.keys().next().value ?? k);
        if (!live) return;
        show();
      }
    })();
    return () => {
      live = false;
    };
    // `wanted` names exactly the tasks and versions `tasks` holds, which is new every poll.
  }, [wanted]);
  return costs;
}

/** Recently finished (0051), docked beside the board: what landed on main and what the owner
 *  accepted in the project in view (every project with All), newest first, grouped by day, from
 *  the stage log. The kind chips are remembered; the window starts at 7 days and Show older adds
 *  a week. `now` is the state's clock, seconds since 1970. */
export default function Recent({ tasks, now, multi, openKey, onOpen }: {
  tasks: Task[];
  now: number;
  multi: boolean;
  openKey: string | null;
  onOpen: (t: Task) => void;
}) {
  const [kind, setKind] = useState<KindFilter>(() => loadKind());
  const [weeks, setWeeks] = useState(1);
  const pick = (k: KindFilter) => {
    setKind(k);
    storeKind(k);
  };
  const all = useMemo(() => finishes(tasks), [tasks]);
  const { groups, older } = useMemo(() => dayGroups(byKind(all, kind), now, weeks), [all, kind, now, weeks]);
  const shown = useMemo(() => [...new Map(groups.flatMap((g) => g.rows).map((r) => [key(r.task), r.task])).values()], [groups]);
  const costs = useCosts(shown);
  return <RecentList groups={groups} older={older} kind={kind} onKind={pick} weeks={weeks} onOlder={() => setWeeks((w) => w + 1)} now={now} multi={multi} openKey={openKey} onOpen={onOpen} costs={costs} />;
}

const NOTHING: Record<KindFilter, string> = { all: "merged or accepted", merged: "merged", accepted: "accepted" };

/** The list as drawn: the head with its window and the kind chips, the day groups, Show older. */
export function RecentList({ groups, older, kind, onKind, weeks, onOlder, now, multi, openKey, onOpen, costs }: {
  groups: DayGroup[];
  older: number;
  kind: KindFilter;
  onKind: (k: KindFilter) => void;
  weeks: number;
  onOlder: () => void;
  now: number;
  multi: boolean;
  openKey: string | null;
  onOpen: (t: Task) => void;
  costs: Map<string, Cost>;
}) {
  const days = weeks * 7;
  return (
    <aside className="recent" aria-label="Recently finished">
      <header className="recent-head">
        <h2>Recently finished <small>last {days} days</small></h2>
        <span className="recent-kinds" role="group" aria-label="Show what finished">
          {KIND_FILTERS.map((k) => (
            <button key={k.id} type="button" className={kind === k.id ? "chip on" : "chip"} aria-pressed={kind === k.id} title={k.title} onClick={() => onKind(k.id)}>
              {k.label}
            </button>
          ))}
        </span>
      </header>
      <div className="recent-list">
        {groups.length === 0 && <p className="none">Nothing {NOTHING[kind]} in the last {days} days.</p>}
        {groups.map((g) => (
          <section key={g.day} className="recent-day" aria-label={g.label}>
            <h3 className="recent-day-head">{g.label} <small>{g.rows.length}</small></h3>
            {g.rows.map((r) => (
              <RecentRow key={`${key(r.task)}:${r.kind}`} r={r} now={now} multi={multi} active={openKey === key(r.task)} onOpen={onOpen} cost={costs.get(key(r.task))} />
            ))}
          </section>
        ))}
        {older > 0 && (
          <button type="button" className="more" onClick={onOlder} title="Add the week before">
            Show older · {older} more
          </button>
        )}
      </div>
    </aside>
  );
}

/** One row: the project (with All), id and title, what happened and when; under them who built
 *  it and its time and tokens. The whole row opens the task in the pane. */
function RecentRow({ r, now, multi, active, onOpen, cost }: { r: Finish; now: number; multi: boolean; active: boolean; onOpen: (t: Task) => void; cost: Cost | undefined }) {
  const t = r.task;
  const by = builtBy(t);
  return (
    <button type="button" className={active ? "recent-row active" : "recent-row"} onClick={() => onOpen(t)}>
      <span className="recent-top">
        {multi && <span className="q">{t.queue}</span>}
        <span className="id">{t.id}</span>
        <span className="recent-title">{t.title || "(no title)"}</span>
        <span className="recent-kind-when">
          <span className={`recent-kind ${r.kind}`}>{r.kind}</span>
          <span className="recent-when" title={r.at}>{whenText(r.at, now)}</span>
        </span>
      </span>
      <span className="recent-meta">{[by ? `built by ${by}` : "", costText(cost)].filter(Boolean).join(" · ")}</span>
    </button>
  );
}
