import type { Task } from "./types";
import { Button } from "./ui";
import { criterionEffort, criterionWords, EFFORTS, namedViews, type NamedView, VIEW_NAMES } from "./yourCheck";

export { criterionWords };

/** How many of the owner's criteria are still open. */
export const ownerOpen = (t: Pick<Task, "ownerCriteria">) => t.ownerCriteria.filter((c) => !c.ticked).length;

/** Your check's list of the criteria only you verify (0028): the open ones first (0043), each
 *  with its effort, Tick, and a button for each view it names (when `onView` can open one); then
 *  the ticked ones with Untick. Each tick is written by the core and logged. */
export default function OwnerChecklist({ task, busy, onTick, onView }: {
  task: Task;
  busy: boolean;
  onTick: (which: string, undo: boolean) => void;
  onView?: (v: NamedView) => void;
}) {
  if (task.stage !== "your-check" || task.ownerCriteria.length === 0) return null;
  const open = ownerOpen(task);
  const shown = [...task.ownerCriteria.filter((c) => !c.ticked), ...task.ownerCriteria.filter((c) => c.ticked)];
  return (
    <div className="owner-checklist" role="group" aria-label="Yours to verify">
      <p className="meta small">{open === 0 ? "All yours are ticked. Accept to finish." : `Yours to verify: ${open} left, then Accept.`}</p>
      <ul className="criteria-list">
        {shown.map((c) => {
          const which = c.label || String(c.n);
          const views = c.ticked || !onView ? [] : namedViews(criterionWords(c.text));
          return (
            <li key={c.n} className={c.ticked ? "criterion ticked" : "criterion"}>
              <span className="box" aria-hidden="true">{c.ticked ? "✓" : ""}</span>
              <span className="words">{c.label && <b>{c.label}</b>} {criterionWords(c.text.replace(/^AC\d+:\s*/, ""))}</span>
              {!c.ticked && <span className="effort meta">{EFFORTS.find((e) => e.id === criterionEffort(c.text))?.label}</span>}
              <span className="crit-actions">
                <Button
                  type="button"
                  className="tick"
                  aria-pressed={c.ticked}
                  disabled={busy}
                  onClick={() => onTick(which, c.ticked)}
                  title={c.ticked ? `Untick ${which}` : `Tick ${which}: you've checked it`}
                >
                  {c.ticked ? "Untick" : "Tick"}
                </Button>
                {views.map((v) => (
                  <Button key={v} type="button" className="view-btn" onClick={() => onView?.(v)}>
                    Open {VIEW_NAMES.find((n) => n.id === v)?.label}
                  </Button>
                ))}
              </span>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
