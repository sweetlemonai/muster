import BlockWhy from "./BlockWhy";
import { CardBand, cardClass, LiveDot } from "./CardLook";
import CardStatus from "./CardStatus";
import { estimateText } from "./fields";
import { Hl, HlId } from "./FilterBar";
import { isHuman } from "./human";
import { ago } from "./lib";
import { CardPing } from "./Ping";
import type { Task } from "./types";
import YouRibbon from "./YouRibbon";

type CardProps = { t: Task; now: number; multi: boolean; parent: string; onOpen: () => void; active: boolean; words: string[] };

/** A board card, and under a quiet one its Ask for a check-in (0039): beside the card, not in it,
 *  since the card is itself a button. */
export default function Card(props: CardProps) {
  return (
    <>
      <CardButton {...props} />
      <CardPing t={props.t} now={props.now} />
    </>
  );
}

/** The card itself. The owner's own task (0022) is hatched in the human gold and carries the You
 *  ribbon, in the same columns as the agents' tasks. Why it's blocked is 0024's one sentence (by
 *  tasks too: an agent's task waiting on the owner's unfinished one says "Blocked by tasks: 0041");
 *  one waiting on the owner (Your check, an escalated question, blocked on you or by a permission)
 *  is a needs-you card. */
function CardButton({ t, now, multi, parent, onOpen, active, words }: CardProps) {
  const priority = t.priority || "normal";
  return (
    <button className={cardClass(t, active)} onClick={onOpen}>
      {isHuman(t) && <YouRibbon />}
      <CardBand t={t} />
      <span className="card-top">
        {multi && <span className="q">{t.queue}</span>}
        <span className="id"><HlId t={t} words={words} /></span>
        <LiveDot t={t} />
        <span className={`kind ${t.kind}`}>{t.kind}</span>
        <span className={`pri pri-${priority}`} title={`Priority: ${priority}`}>{priority}</span>
      </span>
      <span className="title">{t.title ? <Hl text={t.title} words={words} /> : "(no title)"}</span>
      {parent && <span className="parent-line">{parent}</span>}
      {t.tags.length > 0 && (
        <span className="tags">
          {t.tags.map((g) => <span key={g} className="tag">#<Hl text={g} words={words} /></span>)}
        </span>
      )}
      <CardStatus t={t} now={now} />
      <span className="card-bottom">
        <BlockWhy t={t} short />
        {t.waitingForOwner && <span className="badge warn">needs you</span>}
        {t.openQuestions > 0 && <span className="badge">{t.openQuestions} open Q</span>}
        {t.attachments.length > 0 && <span className="badge">{t.attachments.length} file{t.attachments.length === 1 ? "" : "s"}</span>}
        {estimateText(t) && <span className="badge est" title="Estimated agent time, test time and tokens">{estimateText(t)}</span>}
        <span className="age">{ago(now, t.modified)}</span>
      </span>
    </button>
  );
}
