import BlockWhy from "./BlockWhy";
import { CardBand, cardClass, LiveDot } from "./CardLook";
import CardStatus from "./CardStatus";
import { estimateText } from "./fields";
import { Hl, HlId } from "./FilterBar";
import { isHuman } from "./human";
import { ago } from "./lib";
import { cardLines, useLook } from "./modes";
import { CardPause, pausable, PausedChip } from "./Pause";
import { CardPing } from "./Ping";
import type { Task } from "./types";
import { Badge } from "./ui";
import YouRibbon from "./YouRibbon";
import ProjectName, { TaskKey } from "./ProjectName";

type CardProps = { t: Task; now: number; multi: boolean; parent: string; onOpen: () => void; active: boolean; words: string[] };

/** A board card, and under it, in one strip, a quiet one's Ask for a check-in (0039) and Pause or
 *  Resume (0045): beside the card, not in it, since the card is itself a button. */
export default function Card(props: CardProps) {
  const t = props.t;
  return (
    <>
      <CardButton {...props} />
      {/* Keyed on the pause, so a pause or resume from anywhere starts it afresh, without an old note. */}
      <CardPing t={t} now={props.now}>{(t.paused || pausable(t)) && <CardPause key={t.paused ?? ""} t={t} />}</CardPing>
    </>
  );
}

/** The card itself. The owner's own task (0022) is hatched in the human gold and carries the You
 *  ribbon, in the same columns as the agents' tasks. Why it's blocked is 0024's one sentence (by
 *  tasks too: an agent's task waiting on the owner's unfinished one says "After 0041");
 *  one waiting on the owner (Your check, an escalated question, blocked on you or by a permission)
 *  is a needs-you card. */
function CardButton({ t, now, multi, parent, onOpen, active, words }: CardProps) {
  const priority = t.priority || "normal";
  // The display mode (0038): every line, Standard's brief card, or Focus's title and status line.
  const lines = cardLines(useLook("card-lines"), active);
  const all = lines === "all";
  return (
    <button className={cardClass(t, active)} onClick={onOpen}>
      {isHuman(t) && <YouRibbon />}
      <CardBand t={t} />
      <span className="card-top">
        {multi && <ProjectName name={t.queue} />}
        <TaskKey t={t}><HlId t={t} words={words} /></TaskKey>
        <LiveDot t={t} />
        {lines !== "status" && <span className={`kind ${t.kind}`}>{t.kind}</span>}
        {lines !== "status" && <span className={`pri pri-${priority}`} title={`Priority: ${priority}`}>{priority}</span>}
      </span>
      <span className="title">{t.title ? <Hl text={t.title} words={words} /> : "(no title)"}</span>
      {all && parent && <span className="parent-line">{parent}</span>}
      {/* Paused (0045) shows in every mode: a paused task must never look worked on. Its card is in
          the Paused lane (mu-59), so it says where it was. */}
      <PausedChip t={t} now={now} was />
      {all && t.tags.length > 0 && (
        <span className="tags">
          {t.tags.map((g) => <span key={g} className="tag">#<Hl text={g} words={words} /></span>)}
        </span>
      )}
      <CardStatus t={t} now={now} lines={lines} />
      <span className="card-bottom">
        <BlockWhy t={t} short />
        {t.waitingForOwner && <Badge variant="warn">needs you</Badge>}
        {lines !== "status" && t.openQuestions > 0 && <Badge>{t.openQuestions} open Q</Badge>}
        {all && t.attachments.length > 0 && <Badge>{t.attachments.length} file{t.attachments.length === 1 ? "" : "s"}</Badge>}
        {all && estimateText(t) && <Badge variant="est" title="Estimated agent time, test time and tokens">{estimateText(t)}</Badge>}
        {lines !== "status" && <span className="age">{ago(now, t.modified)}</span>}
      </span>
    </button>
  );
}
