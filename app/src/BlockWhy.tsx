import { Fragment } from "react";
import { blockedByText, blockWords } from "./blockWords";
import type { Task } from "./types";

/** Why a task is blocked, in the red pill, the kind in bold (0024). In full, each task it's
 *  blocked by is a link that opens it, when `onOpenTask` is given; `short` is the card's form
 *  (ids only, a long detail cut), plain text since the card itself is a button. `pill={false}`
 *  gives the words alone, for a table cell. Nothing when nothing blocks it. */
export default function BlockWhy({ t, short = false, pill = true, onOpenTask }: {
  t: Pick<Task, "blockedKind" | "blockDetail" | "blockedBy">;
  short?: boolean;
  pill?: boolean;
  onOpenTask?: (id: string) => void;
}) {
  if (!t.blockedKind) return null;
  const { lead, text, tasks } = blockWords(t, short);
  const links = !short && onOpenTask && tasks.length > 0;
  const look = !pill ? "block-why" : short ? "badge blocked block-why" : "status-pill blocked block-why";
  return (
    <span className={look}>
      <b>{lead}</b>
      {links ? (
        <>
          {": "}
          {tasks.map((b, i) => (
            <Fragment key={b.id}>
              {i > 0 && ", "}
              <button type="button" className="link" title={`Open ${b.id}`} onClick={() => onOpenTask(b.id)}>{b.id}</button>
              {blockedByText(b).slice(b.id.length)}
            </Fragment>
          ))}
        </>
      ) : (
        text && `: ${text}`
      )}
    </span>
  );
}
