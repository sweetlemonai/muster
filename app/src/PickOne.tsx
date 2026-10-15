import { useState } from "react";
import { api } from "./api";
import { stamp } from "./lib";
import type { Choices, Question, Task } from "./types";
import { Pill } from "./ui";

// Pick-one questions (0049): an agent asks with options, and the owner answers with one click.
// The core writes the chosen option's text as the answer and mails it to the asking session.

/** An option's letter: a for the first. */
export const letter = (i: number) => "abcdef"[i] ?? "?";

/** What a click on option `i` sends to the core (`answer_pick`): the task, the question, the
 *  option's letter and the extra words, if any. */
export function pickArgs(task: Task, q: Question, i: number, extra: string) {
  return [task.queue, task.id, q.n, letter(i), extra.trim()] as const;
}

/** The line under an answer once it's written. */
export function answeredText(asker: string | null): string {
  return asker
    ? "Answered. The asking agent gets it after its next step."
    : "Answered, and written in the task. No asking session on record, so it waits there.";
}

type Result = { ok: boolean; text: string } | null;
type Send = (queue: string, id: string, n: number, pick: string, extra: string, stamp: string) => Promise<string | null>;

/** What a click on option `i` does: sends the pick through `send` (the core's `answer_pick`), as
 *  of `at`, and says what came of it. The core's refusal (a pick already answered) is the text. */
export async function sendPick(send: Send, task: Task, q: Question, i: number, extra: string, at: Date): Promise<{ ok: boolean; text: string }> {
  const [queue, id, n, pick, words] = pickArgs(task, q, i, extra);
  if (n === null) return { ok: false, text: "This question has no number, so it can't be answered here." };
  try {
    return { ok: true, text: answeredText(await send(queue, id, n, pick, words, stamp(at))) };
  } catch (e) {
    return { ok: false, text: String(e) };
  }
}

/** Are the chips off? While a pick is on its way, and for good once one is answered (the list
 *  may not have refreshed yet, and a second click must not send a second answer); a refusal
 *  leaves them on to try another. */
export function chipsOff(s: { busy: boolean; result: Result }, n: number | null): boolean {
  return s.busy || s.result?.ok === true || n === null;
}

/** A pick-one question's options as chips, every one on screen (a place for choosing,
 *  docs/DESIGN.md), the asker's pick marked "pick", then what it does with no answer. One click
 *  answers with that option, plus `extra` (the answer box's words) when there are any. */
export default function PickOne({ task, q, choices, extra = "", onAnswered, onChanged }: {
  task: Task;
  q: Question;
  choices: Choices;
  extra?: string;
  onAnswered?: () => void;
  onChanged: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<Result>(null);
  const off = chipsOff({ busy, result }, q.n);
  const send = async (i: number) => {
    if (off) return;
    setBusy(true);
    setResult(null);
    const r = await sendPick(api.answerPick, task, q, i, extra, new Date());
    setResult(r);
    setBusy(false);
    if (r.ok) {
      onAnswered?.();
      onChanged();
    }
  };
  return (
    <div className="pick-one" data-part="pick-one" role="group" aria-label={`Options for Q${q.n}`}>
      {choices.options.map((o, i) => (
        <Pill
          key={i}
          type="button"
          className={`pick-chip${choices.pick === i ? " mine" : ""}`}
          disabled={off}
          title={`Answer Q${q.n} with (${letter(i)})${extra.trim() ? ", followed by the words in the box" : ""}`}
          onClick={() => void send(i)}
        >
          <b>({letter(i)})</b> {o}
          {choices.pick === i && <small className="pick-tag">pick</small>}
        </Pill>
      ))}
      {choices.noAnswer !== null && <span className="pick-fallback">going with ({letter(choices.noAnswer)}) unless you answer</span>}
      {result && <span className={result.ok ? "ok" : "err"} role="status">{result.text}</span>}
    </div>
  );
}
