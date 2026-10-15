import { useId } from "react";
import { useT } from "./i18n";
import type { Task } from "./types";
import { Button } from "./ui";
import { criterionEffort, criterionWords, EFFORTS, namedViews, type NamedView, VIEW_NAMES } from "./yourCheck";

export { criterionWords };

/** How many of the owner's criteria are still open. */
export const ownerOpen = (t: Pick<Task, "ownerCriteria">) => t.ownerCriteria.filter((c) => !c.ticked).length;

/** Your check's list of the criteria only you verify (0028): the open ones first (0043), each
 *  with its effort, Tick, and a button for each view it names (when `onView` can open one); then
 *  the ticked ones with Untick. Each tick is written by the core and logged. Each Tick names its
 *  criterion's words for a screen reader (aria-describedby), so the box reads its criterion while
 *  its own name stays Tick. */
export default function OwnerChecklist({ task, busy, onTick, onView }: {
  task: Task;
  busy: boolean;
  onTick: (which: string, undo: boolean) => void;
  onView?: (v: NamedView) => void;
}) {
  const t = useT("pane");
  const id = useId();
  if (task.stage !== "your-check" || task.ownerCriteria.length === 0) return null;
  const open = ownerOpen(task);
  const shown = [...task.ownerCriteria.filter((c) => !c.ticked), ...task.ownerCriteria.filter((c) => c.ticked)];
  return (
    <div className="owner-checklist" role="group" aria-label={t("check.aria")}>
      <p className="meta small">{open === 0 ? t("check.allTicked") : t("check.left", { count: open })}</p>
      <ul className="criteria-list">
        {shown.map((c) => {
          const which = c.label || String(c.n);
          const views = c.ticked || !onView ? [] : namedViews(criterionWords(c.text));
          const words = `${id}-${c.n}`;
          return (
            <li key={c.n} className={c.ticked ? "criterion ticked" : "criterion"}>
              <span className="box" aria-hidden="true">{c.ticked ? "✓" : ""}</span>
              {/* The words take their own direction, never the Latin label's: a Persian criterion reads right to left. */}
              <span className="words" id={words}>{c.label && <b>{c.label}</b>} <span dir="auto">{criterionWords(c.text.replace(/^AC\d+:\s*/, ""))}</span></span>
              {!c.ticked && <span className="effort meta">{EFFORTS.find((e) => e.id === criterionEffort(c.text))?.label}</span>}
              <span className="crit-actions">
                <Button
                  type="button"
                  className="tick"
                  aria-pressed={c.ticked}
                  aria-describedby={words}
                  disabled={busy}
                  onClick={() => onTick(which, c.ticked)}
                  title={c.ticked ? t("check.untickTitle", { which }) : t("check.tickTitle", { which })}
                >
                  {c.ticked ? t("check.untick") : t("check.tick")}
                </Button>
                {views.map((v) => (
                  <Button key={v} type="button" className="view-btn" onClick={() => onView?.(v)}>
                    {t("check.openView", { view: VIEW_NAMES.find((n) => n.id === v)?.label ?? v })}
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
