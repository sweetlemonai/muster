import { Fragment } from "react";
import { BLOCK_LOOKS, blockedByText, blockWords } from "./blockWords";
import { keyOf } from "./taskKey";
import type { Question, Task } from "./types";
import { Badge, Button, StatusPill } from "./ui";

/** Why a task is blocked, the lead in bold (0024), in its kind's look (0060): what the owner acts
 *  on in the needs-you orange, the rest neutral, each with its red mark. In full, each task it
 *  waits on is a link that opens it, when `onOpenTask` is given, and a question it can answer has
 *  an Answer button, when `onAnswer` is given; `short` is the card's form (keys only, a long detail
 *  cut), plain text since the card itself is a button. `pill={false}` gives the words alone, for a
 *  table cell. Nothing when nothing blocks it. */
export default function BlockWhy({ t, short = false, pill = true, onOpenTask, onAnswer }: {
  t: Pick<Task, "blockedKind" | "blockDetail" | "blockedBy" | "prefix"> & Partial<Pick<Task, "questions">>;
  short?: boolean;
  pill?: boolean;
  onOpenTask?: (id: string) => void;
  onAnswer?: (q: Question) => void;
}) {
  if (!t.blockedKind) return null;
  const { look, lead, text, tasks, asked } = blockWords(t, short);
  const links = !short && onOpenTask && tasks.length > 0;
  const open = !short && asked && !asked.ticked && asked.n !== null ? asked : null;
  const answer = open && onAnswer ? () => onAnswer(open) : null;
  const sep = look === "tasks" && tasks.length > 0 ? " " : ": ";
  const looks = `${BLOCK_LOOKS[look].owner ? "for-you" : "not-you"} look-${look}`;
  const words = (
    <>
      <b>{lead}</b>
      {links ? (
        <>
          {sep}
          {tasks.map((b, i) => (
            <Fragment key={b.id}>
              {i > 0 && ", "}
              <button type="button" className="link" title={`Open ${keyOf(t.prefix, b.id)}`} onClick={() => onOpenTask(b.id)}>{keyOf(t.prefix, b.id)}</button>
              {blockedByText(b, t.prefix).slice(keyOf(t.prefix, b.id).length)}
            </Fragment>
          ))}
        </>
      ) : (
        text && `${sep}${text}`
      )}
      {answer && (
        <>
          {" "}
          <Button type="button" className="block-answer" onClick={answer}>Answer</Button>
        </>
      )}
    </>
  );
  if (!pill) return <span className={`block-why plain ${looks}`}>{words}</span>;
  // The look's colours (styles.css .block-why.for-you / .not-you) take over the atom's own.
  return short ? <Badge className={`block-why ${looks}`}>{words}</Badge> : <StatusPill status="blocked" className={`block-why ${looks}`}>{words}</StatusPill>;
}
