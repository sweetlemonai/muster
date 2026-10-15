import { useEffect, useState } from "react";
import { api } from "./api";
import { tl, tn, useT } from "./i18n";
import { oneAtATime, stamp } from "./lib";
import type { Lesson, QueueInfo } from "./types";
import { Button } from "./ui";

// Lessons from reviews (0027): a kind of mistake the review of record kept finding becomes a rule
// for the next builders, and one it kept getting wrong something reviewers don't flag. They
// become instructions to agents only with the owner's yes, given in one batch here, at the top of
// Waiting for you. Their text is a reviewer's: shown as plain text, never as Markdown.

/** The most accepted lessons a queue keeps, as the core caps them: past it the oldest expire. */
const LESSONS_CAP = 40;

/** How many lessons wait for the owner in `queue` ("all": every queue): part of the needs-you count. */
export function lessonsWaiting(queues: Pick<QueueInfo, "name" | "lessonsWaiting">[], queue: string): number {
  return queues.filter((q) => queue === "all" || q.name === queue).reduce((n, q) => n + (q.lessonsWaiting ?? 0), 0);
}

/** What Accept all accepts: the lessons on screen, by id. Never "every proposal": one proposed
 *  after the list was read would become an instruction unseen (the core refuses an id that's no
 *  longer a proposal). */
export function acceptAll(shown: Pick<Lesson, "id">[]): string[] {
  return shown.map((l) => l.id);
}

/** "for builders · app/src-tauri/src/ · from 0027 R1, 0031 R2". */
export function lessonMeta(l: Pick<Lesson, "kind" | "for" | "areas" | "from">): string {
  return [
    l.kind,
    tn("board", l.for === "reviewers" ? "lessons.forReviewers" : "lessons.forBuilders"),
    l.areas.length ? l.areas.join(tl("listSep")) : "",
    l.from.length ? tn("board", "lessons.from", { list: l.from.join(tl("listSep")) }) : "",
  ]
    .filter(Boolean)
    .join(" · ");
}

/** What the core did with a decision: "Accepted L1, L2; rejected L3; L4 expired to stay under the
 *  cap of 40." */
export function decidedText(done: { accepted: string[]; rejected: string[]; expired: string[] }): string {
  const ids = (list: string[]) => list.join(tl("listSep"));
  const said = [
    done.accepted.length ? tn("board", "lessons.accepted", { ids: ids(done.accepted) }) : "",
    done.rejected.length ? tn("board", "lessons.rejected", { ids: ids(done.rejected) }) : "",
    done.expired.length ? tn("board", "lessons.expired", { ids: ids(done.expired), cap: LESSONS_CAP }) : "",
  ].filter(Boolean);
  return tn("board", "lessons.done", { said: said.join(tl("clauseSep")) });
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
  const t = useT("board");
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
  const decide = (accept: string[], reject: string[], reason: string) =>
    void once(async () => {
      setBusy(true);
      setMsg(null);
      try {
        const done = await api.decideLessons(queue, accept, false, reject, reason, stamp(new Date()));
        setMsg({ ok: true, text: decidedText(done) });
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
    <section className="lessons-waiting" aria-label={t("lessons.aria", { label })}>
      <h3>
        {multi ? t("lessons.titleIn", { label }) : t("lessons.title")} <small className="count warn">{count}</small>
      </h3>
      <p className="meta">{t("lessons.intro")}</p>
      {list.error && <p className="err small">{t("lessons.cantRead", { error: list.error })}</p>}
      {list.lessons === null && !list.error && <p className="none">{t("common:loading")}</p>}
      <ul>
        {lessons.map((l) => (
          <li key={l.id}>
            <span className="lesson-rule">
              {l.id} · <span dir="auto">{l.rule}</span>
            </span>
            {l.bad && <div className="meta">{t("lessons.bad", { text: l.bad })}</div>}
            {l.good && <div className="meta">{t("lessons.good", { text: l.good })}</div>}
            <div className="meta">{lessonMeta(l)}</div>
            {rejecting === l.id ? (
              <form
                className="control-row"
                onSubmit={(e) => {
                  e.preventDefault();
                  if (why.trim()) decide([], [l.id], why);
                }}
              >
                <input autoFocus dir="auto" value={why} onChange={(e) => setWhy(e.target.value)} placeholder={t("lessons.why")} aria-label={t("lessons.whyAria", { id: l.id })} maxLength={500} />
                <Button type="submit" disabled={busy || !why.trim()}>{t("lessons.reject", { id: l.id })}</Button>
                <Button type="button" onClick={() => setRejecting(null)}>{t("common:cancel")}</Button>
              </form>
            ) : (
              <div className="control-row">
                <Button type="button" disabled={busy} onClick={() => setRejecting(l.id)}>{t("lessons.rejectOpen", { id: l.id })}</Button>
              </div>
            )}
          </li>
        ))}
      </ul>
      <div className="control-row">
        <Button type="button" variant="primary" disabled={busy || lessons.length === 0} onClick={() => decide(acceptAll(lessons), [], "")}>
          {t("lessons.acceptAll", { count: lessons.length })}
        </Button>
      </div>
      {msg && <p className={msg.ok ? "ok small" : "err small"}>{msg.text}</p>}
    </section>
  );
}
