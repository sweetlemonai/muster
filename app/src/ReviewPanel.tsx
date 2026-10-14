import { useEffect, useState } from "react";
import { api } from "./api";
import { stamp } from "./lib";
import Markdown from "./Markdown";
import { copyNow } from "./NotePanel";
import { reviewSection, reviewStatus } from "./Review";
import type { Task } from "./types";
import { Button } from "./ui";

/** The task pane's Review section (0027): where the review of record stands, Copy review brief
 *  for a session to run the next round (a started task only), and the section the core wrote, as
 *  sanitized Markdown: every finding with its state (open, fixed, disputed, confirmed,
 *  false-positive, wont-fix), the builder's and the reviewer's words on it, and each round. Always
 *  shown, so a task that wasn't reviewed says so. `body` is the task's text, null while it loads. */
export default function ReviewPanel({ task, body }: { task: Task; body: string | null }) {
  const reviewed = (task.reviewRound ?? null) !== null;
  const section = reviewed && body !== null ? reviewSection(body) : null;
  const started = Boolean(task.branch);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  // The brief, loaded when the pane opens and when the task changes, so Copy needs no IPC. It
  // reads the branch with git, so only for a started task.
  const [brief, setBrief] = useState<{ text: string | null; error: string }>({ text: null, error: "" });
  useEffect(() => {
    if (!started) return;
    let live = true;
    setBrief({ text: null, error: "" });
    api.reviewBrief(task.queue, task.id, stamp(new Date())).then(
      (text) => live && setBrief({ text, error: "" }),
      (e) => live && setBrief({ text: null, error: String(e) }),
    );
    return () => {
      live = false;
    };
  }, [task.queue, task.id, task.modified, started]);
  const copy = () =>
    copyNow(brief.text, navigator.clipboard, "The review brief").then(
      () => setMsg({ ok: true, text: "Copied the review brief. Paste it into a session that didn't build the task to run the next round." }),
      (e) => setMsg({ ok: false, text: String(e) }),
    );
  const open = (task.reviewOpen ?? 0) > 0;
  return (
    <section className={`review${open ? " has-open" : ""}`} aria-label="Review of record" data-part="review">
      <h2>Review <small>{reviewStatus(task)}</small></h2>
      {started && (
        <div className="control-row">
          <Button type="button" disabled={brief.text === null} title={brief.text === null && !brief.error ? "Loading the brief…" : undefined} onClick={() => void copy()}>
            Copy review brief
          </Button>
        </div>
      )}
      {brief.error && <p className="err small">Couldn't load the review brief: {brief.error}</p>}
      {msg && <p className={msg.ok ? "ok small" : "err small"}>{msg.text}</p>}
      {!reviewed && (
        <p className="meta small">
          {started
            ? "Not reviewed yet. In Testing a session that didn't build it runs the owner's review engine on the branch (muster review); the merge waits for a clean review."
            : "Reviewed only when it's started on its own branch (muster start)."}
        </p>
      )}
      {reviewed && body !== null && section === null && <p className="err small">Its Review section is gone from the text. Record a new round.</p>}
      {section !== null && <Markdown text={section} />}
    </section>
  );
}
