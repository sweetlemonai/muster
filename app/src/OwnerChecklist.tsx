import type { Task } from "./types";

/** A criterion's words without its "— verify: …" tail, which says who checks it (here, you). */
export function criterionWords(text: string): string {
  // The last "verify:" (case-insensitive), found in the text itself so the cut lands where it is.
  const m = /verify:(?![\s\S]*verify:)/i.exec(text);
  return (m ? text.slice(0, m.index) : text).replace(/[\s—–-]+$/, "").trim();
}

/** How many of the owner's criteria are still open. */
export const ownerOpen = (t: Pick<Task, "ownerCriteria">) => t.ownerCriteria.filter((c) => !c.ticked).length;

/** Your check's list of the criteria only you verify (0028): tick each as you try it, then
 *  Accept. Each tick is written by the core and logged. */
export default function OwnerChecklist({ task, busy, onTick }: { task: Task; busy: boolean; onTick: (which: string, undo: boolean) => void }) {
  if (task.stage !== "your-check" || task.ownerCriteria.length === 0) return null;
  const open = ownerOpen(task);
  return (
    <div className="owner-checklist" role="group" aria-label="Yours to verify">
      <p className="meta small">{open === 0 ? "All yours are ticked. Accept to finish." : `Yours to verify: ${open} left, then Accept.`}</p>
      {task.ownerCriteria.map((c) => {
        const which = c.label || String(c.n);
        return (
          <button
            key={c.n}
            type="button"
            className={c.ticked ? "criterion ticked" : "criterion"}
            aria-pressed={c.ticked}
            disabled={busy}
            onClick={() => onTick(which, c.ticked)}
            title={c.ticked ? "Untick" : "Tick: you've checked this"}
          >
            <span className="box" aria-hidden="true">{c.ticked ? "✓" : ""}</span>
            {c.label && <b>{c.label}</b>} {criterionWords(c.text.replace(/^AC\d+:\s*/, ""))}
          </button>
        );
      })}
    </div>
  );
}
