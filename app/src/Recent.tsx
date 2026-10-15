import { type ReactNode, useContext, useEffect, useMemo, useState } from "react";
import { api } from "./api";
import { stampDateTime } from "./Dates";
import { fmt, tl, tn } from "./i18n";
import { renderTags } from "./i18n/react";
import { fmtHours, key, type RoleNames, shortTokens } from "./lib";
import { taskKey } from "./taskKey";
import { builtBy, byKind, dayGroups, type DayGroup, type Finish, type FinishEvent, type FinishKind, finishes, KIND_FILTERS, type KindFilter, whenText } from "./recentData";
import { RoleNamesContext } from "./settingsData";
import type { Task, Work } from "./types";
import ProjectName, { TaskKey } from "./ProjectName";
import { Pill } from "./ui";

/** A task's time and tokens, the Work section's totals (0025), or why they couldn't be read.
 *  `minutes` is the agent time from the stage log, null when it was never in Building. */
export type Cost = { minutes: number | null; tokens: number; read: number } | { error: string };

/** The cost as the Work section compares it with the estimates (mu-172): the agent time and the
 *  counted tokens (input, output and cache writes) of the task's own sessions. */
export function costOf(w: Work): Cost {
  return { minutes: w.totals.agentMinutes, tokens: w.totals.countedTokens, read: w.totals.read };
}

/** The time and tokens line: "12 min · 450k tokens", in hours past an hour ("10 h", never days),
 *  the tokens alone with no agent time, or why there's none. Undefined is still reading. */
export function costText(c: Cost | undefined): string {
  if (!c) return tn("shell", "recent.costReading");
  if ("error" in c) return tn("shell", "recent.costUnknown");
  if (c.read === 0) return tn("shell", "recent.noTranscripts");
  const tokens = tl("tokensShort", { amount: shortTokens(c.tokens) });
  return c.minutes === null ? tokens : tn("shell", "recent.cost", { time: fmtHours(c.minutes), tokens });
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
          cost = costOf(w);
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
 *  the stage log. The kind chips are remembered (App keeps the pick); the window starts at 7 days and Show older adds
 *  a week. Drawn only while open: folded (by Focus, by Fold, or with nothing in its week), it's a
 *  chip at the end of the row above the board (mu-84) and reads no costs. `now` is the state's
 *  clock, seconds since 1970. */
export default function Recent({ tasks, now, kind, onKind, multi, openKey, onOpen, onFold }: {
  tasks: Task[];
  now: number;
  /** The kind chips' pick: App keeps it, so the row's chip counts what this list shows. */
  kind: KindFilter;
  onKind: (k: KindFilter) => void;
  multi: boolean;
  openKey: string | null;
  onOpen: (t: Task) => void;
  /** Sends it to the row above the board. */
  onFold: () => void;
}) {
  const [weeks, setWeeks] = useState(1);
  const all = useMemo(() => finishes(tasks), [tasks]);
  const { groups, older } = useMemo(() => dayGroups(byKind(all, kind), now, weeks), [all, kind, now, weeks]);
  const shown = useMemo(() => [...new Map(groups.flatMap((g) => g.rows).map((r) => [key(r.task), r.task])).values()], [groups]);
  const costs = useCosts(shown);
  const names = useContext(RoleNamesContext);
  return (
    <RecentList
      groups={groups}
      older={older}
      kind={kind}
      onKind={onKind}
      weeks={weeks}
      onOlder={() => setWeeks((w) => w + 1)}
      now={now}
      multi={multi}
      openKey={openKey}
      onOpen={onOpen}
      costs={costs}
      onFold={onFold}
      names={names}
    />
  );
}


/** The list as drawn: the head with its window and the kind chips, the day groups, Show older. */
export function RecentList({ groups, older, kind, onKind, weeks, onOlder, now, multi, openKey, onOpen, costs, onFold, names = {} }: {
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
  /** Sends it to the row above the board (mu-84). */
  onFold: () => void;
  /** Each role's name by its id (0112), for a builder's slot. */
  names?: RoleNames;
}) {
  const days = weeks * 7;
  return (
    <aside className="recent" aria-label={tn("shell", "recent.title")}>
      <header className="recent-head">
        <h2>
          {tn("shell", "recent.title")} <small>{tn("shell", "recent.lastDays", { count: days })}</small>
          <Pill type="button" className="rail-unfold" aria-expanded={true} aria-label={tn("shell", "recent.foldTitle")} title={tn("shell", "recent.foldTitle")} onClick={onFold}>
            {tl("fold")}
          </Pill>
        </h2>
        <span className="recent-kinds" role="group" aria-label={tn("shell", "recent.kindsAria")}>
          {KIND_FILTERS.map((k) => (
            <Pill key={k.id} type="button" on={kind === k.id} aria-pressed={kind === k.id} title={k.title} onClick={() => onKind(k.id)}>
              {k.label}
            </Pill>
          ))}
        </span>
      </header>
      <div className="recent-list">
        {groups.length === 0 && <p className="none">{tn("shell", `recent.nothing.${kind}`, { count: days })}</p>}
        {groups.map((g) => (
          <section key={g.day} className="recent-day" aria-label={g.label}>
            <h3 className="recent-day-head">{g.label} <small>{fmt.num(g.rows.length)}</small></h3>
            {g.rows.map((r) => (
              <RecentRow key={`${key(r.task)}:${r.kind}`} r={r} now={now} multi={multi} active={openKey === key(r.task)} onOpen={onOpen} cost={costs.get(key(r.task))} names={names} />
            ))}
          </section>
        ))}
        {older > 0 && (
          <button type="button" className="more" onClick={onOlder} title={tn("shell", "recent.olderTitle")}>
            {tn("shell", "recent.older", { count: older })}
          </button>
        )}
      </div>
    </aside>
  );
}

/** Under All, a task merged and accepted says both, oldest first, as one sentence each way round
 *  (mu-201): "merged yesterday 18:20 · accepted 4 h ago". */
const BOTH = { merged: "recent.events.mergedThenAccepted", accepted: "recent.events.acceptedThenMerged" } as const;

/** What happened and when: each kind word in its colour, each time a <time> with its stamp.
 *  renderTags, not <Trans>: the row is drawn without hooks (its test walks it as plain calls). */
function events(list: FinishEvent[], now: number): ReactNode {
  const kind = (k: FinishKind) => <span className={`recent-kind ${k}`} />;
  const time = (at: string) => <time className="recent-when" title={at} dateTime={stampDateTime(at)} />;
  const [first, second] = list;
  if (!second) {
    return (
      <>
        <span className={`recent-kind ${first.kind}`}>{tn("shell", `recent.finish.${first.kind}`)}</span>
        <time className="recent-when" title={first.at} dateTime={stampDateTime(first.at)}>{whenText(first.at, now)}</time>
      </>
    );
  }
  return renderTags(tn("shell", BOTH[first.kind], { first: whenText(first.at, now), second: whenText(second.at, now) }), {
    merged: kind("merged"),
    accepted: kind("accepted"),
    first: time(first.at),
    second: time(second.at),
  });
}

/** One row: the project (with All), id and title, what happened and when; under them who built
 *  it and its time and tokens. The whole row opens the task in the pane. */
function RecentRow({ r, now, multi, active, onOpen, cost, names }: { r: Finish; now: number; multi: boolean; active: boolean; onOpen: (t: Task) => void; cost: Cost | undefined; names: RoleNames }) {
  const task = r.task;
  const by = builtBy(task, names);
  return (
    <button type="button" className={active ? "recent-row active" : "recent-row"} onClick={() => onOpen(task)}>
      <span className="recent-top">
        {multi && <ProjectName name={task.queue} />}
        <TaskKey t={task}>{taskKey(task)}</TaskKey>
        <span className="recent-title">{task.title || tn("shell", "recent.noTitle")}</span>
        <span className="recent-kind-when">{events(r.events, now)}</span>
      </span>
      <span className="recent-meta">{[by ? tn("shell", "recent.builtBy", { who: by }) : "", costText(cost)].filter(Boolean).join(" · ")}</span>
    </button>
  );
}
