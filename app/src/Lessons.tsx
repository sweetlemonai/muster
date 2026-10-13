import { useEffect, useState } from "react";
import { api } from "./api";
import { oneAtATime, stamp } from "./lib";
import type { Lesson, QueueInfo } from "./types";

// Lessons from reviews (0027): a kind of mistake the review of record kept finding becomes a rule
// for the next builders, and one it kept getting wrong something reviewers don't flag. They
// become instructions to agents only with the owner's yes, given in one batch here, at the top of
// Waiting for you. Their text is a reviewer's: shown as plain text, never as Markdown.

/** How many lessons wait for the owner in `queue` ("all": every queue): part of the needs-you count. */
export function lessonsWaiting(queues: Pick<QueueInfo, "name" | "lessonsWaiting">[], queue: string): number {
  return queues.filter((q) => queue === "all" || q.name === queue).reduce((n, q) => n + (q.lessonsWaiting ?? 0), 0);
}

/** "for builders · app/src-tauri/src/ · from 0027 R1, 0031 R2". */
export function lessonMeta(l: Pick<Lesson, "kind" | "for" | "areas" | "from">): string {
  return [
    l.kind,
    l.for === "reviewers" ? "for reviewers: don't flag" : "for builders",
    l.areas.length ? l.areas.join(", ") : "",
    l.from.length ? `from ${l.from.join(", ")}` : "",
  ]
    .filter(Boolean)
    .join(" · ");
}

/** Waiting for you's Lessons block: each queue with proposals, every one in full, Accept all for
 *  the batch, and Reject with why for any that shouldn't become a rule. Nothing when none wait. */
export default function LessonsWaiting({ queues, queue, onChanged }: { queues: QueueInfo[]; queue: string; onChanged: () => void }) {
  const shown = queues.filter((q) => (queue === "all" || q.name === queue) && (q.lessonsWaiting ?? 0) > 0);
  if (shown.length === 0) return null;
  return (
    <>
      {shown.map((q) => (
        <QueueLessons key={q.name} queue={q.name} label={q.label ?? q.name} count={q.lessonsWaiting ?? 0} multi={queue === "all"} onChanged={onChanged} />
      ))}
    </>
  );
}

/** One queue's proposals: `queue` is its name, for the core; `label` what the window calls it (0044). */
function QueueLessons({ queue, label, count, multi, onChanged }: { queue: string; label: string; count: number; multi: boolean; onChanged: () => void }) {
  const [list, setList] = useState<{ lessons: Lesson[] | null; error: string }>({ lessons: null, error: "" });
  const [rejecting, setRejecting] = useState<string | null>(null);
  const [why, setWhy] = useState("");
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [once] = useState(oneAtATime);
  // Read again whenever the count the state carries changes.
  useEffect(() => {
    let live = true;
    api.lessons(queue, stamp(new Date())).then(
      (l) => live && setList({ lessons: l.proposed, error: "" }),
      (e) => live && setList({ lessons: null, error: String(e) }),
    );
    return () => {
      live = false;
    };
  }, [queue, count]);
  const decide = (accept: boolean, reject: string[], reason: string) =>
    void once(async () => {
      setBusy(true);
      setMsg(null);
      try {
        const done = await api.decideLessons(queue, [], accept, reject, reason, stamp(new Date()));
        const said = [
          done.accepted.length ? `Accepted ${done.accepted.join(", ")}` : "",
          done.rejected.length ? `rejected ${done.rejected.join(", ")}` : "",
          done.expired.length ? `${done.expired.join(", ")} expired to stay under the cap of 40` : "",
        ].filter(Boolean);
        setMsg({ ok: true, text: `${said.join("; ")}.` });
        setRejecting(null);
        setWhy("");
        onChanged();
        return true;
      } catch (e) {
        setMsg({ ok: false, text: String(e) });
        return false;
      } finally {
        setBusy(false);
      }
    });
  const lessons = list.lessons ?? [];
  return (
    <section className="lessons-waiting" aria-label={`Lessons to approve in ${label}`}>
      <h3>
        Lessons to approve{multi ? ` in ${label}` : ""} <small className="count warn">{count}</small>
      </h3>
      <p className="meta">Kinds of mistake the reviews kept finding. Accepted ones go into the next agents' briefs for these folders; until then they're only proposals.</p>
      {list.error && <p className="err small">Couldn't read the lessons: {list.error}</p>}
      {list.lessons === null && !list.error && <p className="none">Loading…</p>}
      <ul>
        {lessons.map((l) => (
          <li key={l.id}>
            <span className="lesson-rule">
              {l.id} · {l.rule}
            </span>
            {l.bad && <div className="meta">Bad: {l.bad}</div>}
            {l.good && <div className="meta">Good: {l.good}</div>}
            <div className="meta">{lessonMeta(l)}</div>
            {rejecting === l.id ? (
              <form
                className="control-row"
                onSubmit={(e) => {
                  e.preventDefault();
                  if (why.trim()) decide(false, [l.id], why);
                }}
              >
                <input autoFocus value={why} onChange={(e) => setWhy(e.target.value)} placeholder="Why it shouldn't become a rule, in one line" aria-label={`Why ${l.id} is rejected`} maxLength={500} />
                <button type="submit" disabled={busy || !why.trim()}>{`Reject ${l.id}`}</button>
                <button type="button" onClick={() => setRejecting(null)}>Cancel</button>
              </form>
            ) : (
              <div className="control-row">
                <button type="button" disabled={busy} onClick={() => setRejecting(l.id)}>{`Reject ${l.id}…`}</button>
              </div>
            )}
          </li>
        ))}
      </ul>
      <div className="control-row">
        <button type="button" className="primary" disabled={busy || lessons.length === 0} onClick={() => decide(true, [], "")}>
          {`Accept all ${lessons.length}`}
        </button>
      </div>
      {msg && <p className={msg.ok ? "ok small" : "err small"}>{msg.text}</p>}
    </section>
  );
}
