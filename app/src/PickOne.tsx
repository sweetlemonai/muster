import { useState } from "react";
import { api } from "./api";
import { stamp } from "./lib";
import type { Choices, Question, Task } from "./types";

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
  const [result, setResult] = useState<{ ok: boolean; text: string } | null>(null);
  const send = async (i: number) => {
    const [queue, id, n, pick, words] = pickArgs(task, q, i, extra);
    if (n === null || busy) return;
    setBusy(true);
    setResult(null);
    try {
      const asker = await api.answerPick(queue, id, n, pick, words, stamp(new Date()));
      setResult({ ok: true, text: answeredText(asker) });
      onAnswered?.();
      onChanged();
    } catch (e) {
      setResult({ ok: false, text: String(e) });
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="pick-one" data-part="pick-one" role="group" aria-label={`Options for Q${q.n}`}>
      {choices.options.map((o, i) => (
        <button
          key={i}
          type="button"
          className={`chip pick-chip${choices.pick === i ? " mine" : ""}`}
          disabled={busy || q.n === null}
          title={`Answer Q${q.n} with (${letter(i)})${extra.trim() ? ", followed by the words in the box" : ""}`}
          onClick={() => void send(i)}
        >
          <b>({letter(i)})</b> {o}
          {choices.pick === i && <small className="pick-tag">pick</small>}
        </button>
      ))}
      {choices.noAnswer !== null && <span className="pick-fallback">with no answer: {letter(choices.noAnswer)}</span>}
      {result && <span className={result.ok ? "ok" : "err"} role="status">{result.text}</span>}
    </div>
  );
}
