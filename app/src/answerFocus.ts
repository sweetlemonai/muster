import type { Task } from "./types";

/** The id of question `n`'s answer box, so a block waiting on it (0060) can take the owner there. */
export const answerId = (t: Pick<Task, "queue" | "id">, n: number) => `answer-${t.queue}-${t.id}-q${n}`;

/** Takes the owner to question `n`'s answer box in the pane, a block's Answer (0060): scrolled into
 *  view and focused, ready to type. Nothing when the box isn't on screen. */
export function focusAnswer(t: Pick<Task, "queue" | "id">, n: number, doc: Pick<Document, "getElementById"> = document) {
  const box = doc.getElementById(answerId(t, n));
  box?.scrollIntoView({ block: "nearest" });
  box?.focus();
}
