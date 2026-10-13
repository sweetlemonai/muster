import { Fragment } from "react";
import BlockWhy from "./BlockWhy";
import EffortGroups from "./EffortGroups";
import { humanColumn, isHuman } from "./human";
import { key } from "./lib";
import { taskKey } from "./taskKey";
import { isWork } from "./levels";
import { PausedChip } from "./Pause";
import PickOne from "./PickOne";
import type { Task } from "./types";
import YouRibbon from "./YouRibbon";
import { nextStep } from "./yourCheck";

/** Waiting for you: every task waiting on the owner, in the order given. The agents' tasks in
 *  Your check come first, by effort (0043) with each one's next step in full and Accept all in
 *  Just accept; then questions escalated to the owner, blocks on them and their own tasks. */
export default function WaitingList({ tasks, multi, openKey, onOpen, onChanged, now }: {
  tasks: Task[];
  multi: boolean;
  openKey: string | null;
  onOpen: (t: Task) => void;
  onChanged: () => void;
  /** The state's clock, seconds since 1970 (how long a paused one has been paused). */
  now: number;
}) {
  // An agent's task in Your check; a goal or feature put there by hand is listed with the rest.
  const inCheck = (t: Task) => !isHuman(t) && isWork(t) && t.stage === "your-check";
  const check = tasks.filter(inCheck);
  const rest = tasks.filter((t) => !inCheck(t));
  const item = (t: Task) => <WaitingItem key={key(t)} t={t} now={now} multi={multi} active={openKey === key(t)} onOpen={onOpen} onChanged={onChanged} />;
  return (
    <main className="list waiting-list">
      {tasks.length === 0 && <p className="empty">Nothing is waiting for you.</p>}
      {/* With none left in Your check, it still shows what Accept all came to, until the next change. */}
      <EffortGroups tasks={check} place="waiting" renderCard={item} onChanged={onChanged} />
      {rest.length > 0 && (
        <div className="effort-group effort-other" role="group" aria-label="Questions, blocks and your own tasks">
          <h3 className="effort-head">Questions, blocks and your own tasks <small>{rest.length}</small></h3>
          {rest.map(item)}
        </div>
      )}
    </main>
  );
}

/** One task waiting on the owner: what it is, that it's paused (0045), each thing it waits on them
 *  for (a pick-one question with its options, answered here in one click, 0049), and Open. */
function WaitingItem({ t, now, multi, active, onOpen, onChanged }: { t: Task; now: number; multi: boolean; active: boolean; onOpen: (t: Task) => void; onChanged: () => void }) {
  const asked = t.questions.filter((q) => !q.ticked && q.escalated);
  // A question or a block as the next step is shown above in its own words.
  const next = t.stage === "your-check" && !isHuman(t) && isWork(t) ? nextStep(t) : null;
  const step = next && (next.kind === "criterion" || next.kind === "accept") ? next : null;
  return (
    <article className={active ? "waiting active" : "waiting"}>
      <header>
        <button className="link" onClick={() => onOpen(t)}>
          {isHuman(t) && <YouRibbon />} {multi && <span className="q">{t.queue}</span>} {taskKey(t)} · {t.title}
        </button>
      </header>
      {t.paused && <p><PausedChip t={t} now={now} /></p>}
      {isHuman(t) && <p>Yours to do: {humanColumn(t.stage) === "doing" ? "in Doing. Mark it Done when it is." : "in To do. Start it when you do."}</p>}
      {asked.map((q) => (
        <Fragment key={q.n ?? q.text}>
          <p><b>Q{q.n}</b> {q.choices ? q.choices.question : q.text}</p>
          {q.choices && <PickOne task={t} q={q} choices={q.choices} onChanged={onChanged} />}
        </Fragment>
      ))}
      {(t.blockedKind === "you" || t.blockedKind === "permission") && <p><BlockWhy t={t} /></p>}
      {step && <p className="next-step-line">{`${step.full}${step.more ? ` · ${step.more} more` : ""}`}</p>}
      <button onClick={() => onOpen(t)}>{asked.length > 0 || t.blockedKind === "you" ? "Open to answer" : "Open"}</button>
    </article>
  );
}
