import { Fragment, type ReactNode, type Ref, useMemo, useRef } from "react";
import { loadOrder, nextOrder, type OrderMemo, projectRows, type SidebarOrder, sumActivity } from "./activity";
import { projectCount } from "./Backlog";
import { needsYouCount, stalledProjectsWords, workingInWords, workingWords } from "./human";
import { fmt, useT } from "./i18n";
import { lessonsWaiting } from "./Lessons";
import { inProgress } from "./lib";
import { rowWords } from "./ProjectList";
import { sidebarGroups } from "./projectOrder";
import { headingName } from "./SidebarProjects";
import type { Activity, Note, QueueInfo, Task } from "./types";
import { Pill } from "./ui";
import { cx } from "./ui/cx";

/** The sidebar's element id, which its Hide and the rail's Show control (aria-controls). */
export const SIDEBAR_ID = "sidebar";

/** The sidebar hidden (mu-167): a rail in its place, so what needs the owner never folds. Show at
 *  the top, then All and every project in the sidebar's order, each its colour dot, its working
 *  dot and its needs-you badge, whole; a category heading is a thin divider, and every project is
 *  there whatever its heading's fold. Each is a button that picks the project, named, and titled,
 *  with its counts in the sidebar's words (`rowWords`). The same in every mode. It reads only what
 *  the sidebar gets; `order` is the sidebar's kept order. */
export default function ProjectRail({ queues, tasks, notes, activity, now, current, onPick, onShow, showRef, order = loadOrder() }: {
  queues: QueueInfo[];
  tasks: Task[];
  notes: Note[];
  activity: Activity[];
  now: number;
  current: string;
  onPick: (queue: string) => void;
  onShow: () => void;
  showRef?: Ref<HTMLButtonElement>;
  order?: SidebarOrder;
}) {
  const t = useT("shell");
  const rows = useMemo(() => projectRows(queues, tasks, activity, now), [queues, tasks, activity, now]);
  // The sidebar's order, held between polls the same way, so a dot doesn't jump as minutes tick.
  const memo = useRef<OrderMemo | null>(null);
  memo.current = nextOrder(memo.current, rows, order);
  const byName = new Map(rows.map((r) => [r.name, r]));
  const groups = sidebarGroups(memo.current.names.flatMap((n) => byName.get(n) ?? []), []);
  const byQueue = new Map(queues.map((q) => [q.name, q]));
  const sums = useMemo(() => sumActivity(rows.map((r) => r.activity)), [rows]);
  const allSaid = rowWords({
    name: t("sidebar.all"),
    needs: needsYouCount(tasks) + lessonsWaiting(queues, "all"),
    doing: tasks.filter(inProgress).length,
    paused: tasks.filter((x) => x.paused).length,
    total: tasks.length + notes.length,
    running: false,
    working: sums.working > 0 ? workingInWords(sums.working, sums.projects) : "",
    stalled: sums.stalled > 0 ? stalledProjectsWords(sums.stalled) : "",
  });
  return (
    <div className="rail">
      <Pill ref={showRef} aria-expanded={false} aria-controls={SIDEBAR_ID} title={t("toggles.sidebar.showTitle")} type="button" className="side-toggle" onClick={onShow}>
        {t("toggles.sidebar.show")}
      </Pill>
      <nav className="rail-list" aria-label={t("rail.label")}>
        <RailItem said={allSaid} on={current === "all"} needs={needsYouCount(tasks) + lessonsWaiting(queues, "all")} working={sums.working > 0} onClick={() => onPick("all")}>
          <span className="rail-all">{t("sidebar.all")}</span>
        </RailItem>
        {groups.map((g) => (
          <Fragment key={g.name}>
            <hr className="rail-divider" aria-label={headingName(g.name)} title={headingName(g.name)} />
            {g.rows.map((r) => {
              const needs = needsYouCount(r.tasks) + r.lessons;
              const said = rowWords({
                name: r.label,
                needs,
                doing: r.tasks.filter(inProgress).length,
                paused: r.tasks.filter((x) => x.paused).length,
                total: projectCount(byQueue.get(r.name)?.counts ?? {}, notes, r.name),
                running: byQueue.get(r.name)?.running === true,
                working: r.activity.working > 0 ? workingWords(r.activity.working) : "",
                stalled: r.activity.stalled ? t("sidebar.stalled") : "",
              });
              return (
                <RailItem key={r.name} said={said} on={current === r.name} needs={needs} working={r.activity.working > 0} onClick={() => onPick(r.name)}>
                  <i className="side-swatch-dot" data-colour={r.colour || undefined} aria-hidden="true" />
                </RailItem>
              );
            })}
          </Fragment>
        ))}
      </nav>
    </div>
  );
}

/** One place in the rail: its mark (All's word or a project's dot), the working dot, and the
 *  needs-you badge under them, whole. Its name and tooltip are `said`; what shows is for the eye. */
function RailItem({ said, on, needs, working, onClick, children }: { said: string; on: boolean; needs: number; working: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button type="button" className={cx("rail-item", on && "on")} aria-current={on ? "true" : undefined} aria-label={said} title={said} onClick={onClick}>
      <span className="rail-marks">
        {children}
        {working && <i className="dot rail-working" aria-hidden="true" />}
      </span>
      {needs > 0 && <span className="count warn rail-needs" aria-hidden="true">{fmt.num(needs)}</span>}
    </button>
  );
}
