import { Fragment } from "react";
import { blockedByText, blockWords } from "./blockWords";
import { keyOf } from "./taskKey";
import type { Task } from "./types";
import { Badge, StatusPill } from "./ui";

/** Why a task is blocked, in the red pill, the kind in bold (0024). In full, each task it's
 *  blocked by is a link that opens it, when `onOpenTask` is given; `short` is the card's form
 *  (keys only, a long detail cut), plain text since the card itself is a button. `pill={false}`
 *  gives the words alone, for a table cell. Nothing when nothing blocks it. */
export default function BlockWhy({ t, short = false, pill = true, onOpenTask }: {
  t: Pick<Task, "blockedKind" | "blockDetail" | "blockedBy" | "prefix">;
  short?: boolean;
  pill?: boolean;
  onOpenTask?: (id: string) => void;
}) {
  if (!t.blockedKind) return null;
  const { lead, text, tasks } = blockWords(t, short);
  const links = !short && onOpenTask && tasks.length > 0;
  const words = (
    <>
      <b>{lead}</b>
      {links ? (
        <>
          {": "}
          {tasks.map((b, i) => (
            <Fragment key={b.id}>
              {i > 0 && ", "}
              <button type="button" className="link" title={`Open ${keyOf(t.prefix, b.id)}`} onClick={() => onOpenTask(b.id)}>{keyOf(t.prefix, b.id)}</button>
              {blockedByText(b, t.prefix).slice(keyOf(t.prefix, b.id).length)}
            </Fragment>
          ))}
        </>
      ) : (
        text && `: ${text}`
      )}
    </>
  );
  if (!pill) return <span className="block-why">{words}</span>;
  return short ? <Badge variant="blocked" className="block-why">{words}</Badge> : <StatusPill status="blocked" className="block-why">{words}</StatusPill>;
}
