import { Fragment, useState } from "react";
import { api } from "./api";
import BlockWhy from "./BlockWhy";
import { BLOCK_LOOKS, blockWords } from "./blockWords";
import EffortGroups from "./EffortGroups";
import { humanColumn, isHuman } from "./human";
import { key, stamp } from "./lib";
import LessonsWaiting, { lessonsWaiting } from "./Lessons";
import { taskKey } from "./taskKey";
import { isWork } from "./levels";
import { PausedChip } from "./Pause";
import PickOne from "./PickOne";
import type { QueueInfo, Task } from "./types";
import YouRibbon from "./YouRibbon";
import { nextStep } from "./yourCheck";
import { AnswerBox } from "./TaskPanel";
import ProjectName from "./ProjectName";

/** Waiting for you: every task waiting on the owner, in the order given. The agents' tasks in
 *  Your check come first, by effort (0043) with each one's next step in full and Accept all in
 *  Just accept; then, answers first, then actions, then permissions (0060), questions escalated to
 *  the owner, blocks on them, sessions waiting for input
 *  in Claude Code (0048) and their own tasks. Above them all, the lessons from reviews waiting for
 *  the owner's yes (0027) in `queue` ("all": every one of `queues`). */
export default function WaitingList({ tasks, multi, openKey, onOpen, onChanged, now, queues = [], queue = "all" }: {
  tasks: Task[];
  multi: boolean;
  openKey: string | null;
  onOpen: (t: Task) => void;
  onChanged: () => void;
  /** The state's clock, seconds since 1970 (how long a paused one has been paused). */
  now: number;
  queues?: QueueInfo[];
  queue?: string;
}) {
  // An agent's task in Your check; a goal or feature put there by hand is listed with the rest.
  const inCheck = (t: Task) => !isHuman(t) && isWork(t) && t.stage === "your-check";
  const check = tasks.filter(inCheck);
  // Answers, then actions, then permissions (0060), each kept in the order given.
  const rest = tasks.filter((t) => !inCheck(t)).sort((a, b) => waitRank(a) - waitRank(b));
  const item = (t: Task) => <WaitingItem key={key(t)} t={t} now={now} multi={multi} active={openKey === key(t)} onOpen={onOpen} onChanged={onChanged} />;
  return (
    <main className="list waiting-list">
      <LessonsWaiting queues={queues} queue={queue} onChanged={onChanged} />
      {tasks.length === 0 && lessonsWaiting(queues, queue) === 0 && <p className="empty">Nothing is waiting for you.</p>}
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

/** Where a task waiting on the owner sits among the questions, blocks and their own tasks (0060):
 *  what needs their answer first, then their action, then a permission, then the rest. */
export function waitRank(t: Task): number {
  const { look } = blockWords(t);
  if (look === "answer" || t.questions.some((q) => !q.ticked && q.escalated)) return 0;
  return look === "action" ? 1 : look === "permission" ? 2 : 3;
}

/** One task waiting on the owner: what it is, that it's paused (0045), each thing it waits on them
 *  for (a pick-one question with its options, answered here in one click, 0049; a block on them
 *  with its button, 0060), and Open. */
function WaitingItem({ t, now, multi, active, onOpen, onChanged }: { t: Task; now: number; multi: boolean; active: boolean; onOpen: (t: Task) => void; onChanged: () => void }) {
  const [answering, setAnswering] = useState(false);
  const blocking = blockWords(t);
  // The question a block waits on for the owner's answer is said once, in the block's line.
  const asked = t.questions.filter((q) => !q.ticked && q.escalated);
  const shown = blocking.look === "answer" ? asked.filter((q) => q !== blocking.asked) : asked;
  // A question or a block as the next step is shown above in its own words.
  const next = t.stage === "your-check" && !isHuman(t) && isWork(t) ? nextStep(t) : null;
  const step = next && (next.kind === "criterion" || next.kind === "accept") ? next : null;
  return (
    <article className={active ? "waiting active" : "waiting"}>
      <header>
        <button className="link" onClick={() => onOpen(t)}>
          {isHuman(t) && <YouRibbon />} {multi && <ProjectName name={t.queue} />} {taskKey(t)} · {t.title}
        </button>
      </header>
      {t.paused && <p><PausedChip t={t} now={now} /></p>}
      {isHuman(t) && <p>Yours to do: {humanColumn(t.stage) === "doing" ? "in Doing. Mark it Done when it is." : "in To do. Start it when you do."}</p>}
      {shown.map((q) => (
        <Fragment key={q.n ?? q.text}>
          <p><b>Q{q.n}</b> {q.choices ? q.choices.question : q.text}</p>
          {q.choices && <PickOne task={t} q={q} choices={q.choices} onChanged={onChanged} />}
        </Fragment>
      ))}
      <BlockLine t={t} answering={answering} onAnswer={() => setAnswering(true)} onChanged={onChanged} />
      {t.sessionState === "needs-input" && <p>Waiting for input in Claude Code: answer it there.</p>}
      {step && <p className="next-step-line">{`${step.full}${step.more ? ` · ${step.more} more` : ""}`}</p>}
      <button onClick={() => onOpen(t)}>{asked.length > 0 || t.blockedKind === "you" ? "Open to answer" : "Open"}</button>
    </article>
  );
}

/** A block the owner acts on, in its words (0060), with its button: Answer opens the answer box
 *  right here, under the line (`answering`); an action or a permission is unblocked once done.
 *  Nothing for a block the owner can't act on (other tasks, something outside, a question capture
 *  still has). */
export function BlockLine({ t, answering, onAnswer, onChanged }: { t: Task; answering: boolean; onAnswer: () => void; onChanged: () => void }) {
  const { look, asked } = blockWords(t);
  if (!BLOCK_LOOKS[look].owner) return null;
  return (
    <>
      <p><BlockWhy t={t} onAnswer={answering ? undefined : onAnswer} /></p>
      {look === "answer" && answering && asked && <AnswerBox task={t} q={asked} onChanged={onChanged} />}
      {(look === "action" || look === "permission") && <UnblockNow t={t} label={look === "action" ? "Done, unblock" : "Granted, unblock"} onChanged={onChanged} />}
    </>
  );
}

/** What Done, unblock does: unblocks the task through `send` (the core's `unblock_task`) as of
 *  `at`, and says what came of it; the core's refusal is the text. */
export async function unblockNow(send: (queue: string, id: string, stamp: string) => Promise<string>, t: Pick<Task, "queue" | "id">, at: Date): Promise<{ ok: boolean; text: string }> {
  try {
    await send(t.queue, t.id, stamp(at));
    return { ok: true, text: "Unblocked." };
  } catch (e) {
    return { ok: false, text: String(e) };
  }
}

function UnblockNow({ t, label, onChanged }: { t: Task; label: string; onChanged: () => void }) {
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{ ok: boolean; text: string } | null>(null);
  const run = async () => {
    setBusy(true);
    const r = await unblockNow(api.unblockTask, t, new Date());
    setBusy(false);
    setResult(r);
    if (r.ok) onChanged();
  };
  return (
    <>
      <button type="button" className="block-unblock" disabled={busy || result?.ok === true} onClick={() => void run()}>{busy ? "Unblocking…" : label}</button>
      {result && <span className={result.ok ? "ok" : "err"}> {result.text}</span>}
    </>
  );
}
