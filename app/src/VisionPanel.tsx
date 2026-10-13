import { useEffect, useState } from "react";
import { api } from "./api";
import { oneAtATime, stamp } from "./lib";
import Markdown from "./Markdown";
import { copyNow } from "./NotePanel";
import type { Task } from "./types";
import { conflictTitles, visionSection, visionStatus } from "./Vision";

/** The task pane's Vision section (0029): where the check stands, Resolve for each open conflict
 *  (how it was settled, in one line, in place), Copy vision brief, and the section the core wrote,
 *  as sanitized Markdown. Always shown, so a task that was never checked says so. `body` is the
 *  task's text, null while it loads; the Resolve buttons come from the task, so they don't wait. */
export default function VisionPanel({ task, body, onChanged }: { task: Task; body: string | null; onChanged: () => void }) {
  const checked = (task.visionOpen ?? null) !== null;
  const open = task.visionIds ?? [];
  const titles = body === null ? {} : conflictTitles(body);
  const section = checked && body !== null ? visionSection(body) : null;
  const [which, setWhich] = useState<string | null>(null);
  const [how, setHow] = useState("");
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [once] = useState(oneAtATime);
  // The brief, loaded when the pane opens and when the task changes, so Copy needs no IPC.
  const [brief, setBrief] = useState<{ text: string | null; error: string }>({ text: null, error: "" });
  useEffect(() => {
    let live = true;
    setBrief({ text: null, error: "" });
    api.visionBrief(task.queue, task.id).then(
      (text) => live && setBrief({ text, error: "" }),
      (e) => live && setBrief({ text: null, error: String(e) }),
    );
    return () => {
      live = false;
    };
  }, [task.queue, task.id, task.modified]);

  // The form belongs to a conflict that's still open: once it's resolved, here or elsewhere, it's gone.
  const resolving = which !== null && open.includes(which) ? which : null;
  const resolve = () => {
    const conflict = resolving;
    const text = how;
    if (!conflict || !text.trim()) return;
    void once(async () => {
      setBusy(true);
      setMsg(null);
      try {
        const left = await api.resolveVision(task.queue, task.id, conflict, text, stamp(new Date()));
        setMsg({ ok: true, text: `Resolved ${conflict}. ${left === 0 ? "No conflict is open, so it can pass to Ready." : `${left} still open.`}` });
        setWhich(null);
        setHow("");
        onChanged();
        return true;
      } catch (e) {
        setMsg({ ok: false, text: String(e) });
        return false;
      } finally {
        setBusy(false);
      }
    });
  };
  // Not through `once`: its awaits would come before the clipboard write.
  const copy = () =>
    copyNow(brief.text, navigator.clipboard, "The vision brief").then(
      () => setMsg({ ok: true, text: "Copied the vision brief. Paste it into a session to check the task against the picture." }),
      (e) => setMsg({ ok: false, text: String(e) }),
    );

  return (
    <section className={`vision${open.length ? " has-open" : ""}`} aria-label="Vision check" data-part="vision">
      <h2>Vision <small>{visionStatus(task)}</small></h2>
      <div className="control-row">
        {open.map((id) => (
          <button
            key={id}
            type="button"
            className={resolving === id ? "on" : undefined}
            disabled={busy}
            title={titles[id]}
            onClick={() => {
              setWhich(id);
              setHow("");
              setMsg(null);
            }}
          >
            {`Resolve ${id}`}
          </button>
        ))}
        <button type="button" disabled={brief.text === null} title={brief.text === null && !brief.error ? "Loading the brief…" : undefined} onClick={() => void copy()}>
          Copy vision brief
        </button>
      </div>
      {brief.error && <p className="err small">Couldn't load the vision brief: {brief.error}</p>}
      {resolving && (
        <form
          className="control-row"
          onSubmit={(e) => {
            e.preventDefault();
            resolve();
          }}
        >
          <input autoFocus value={how} onChange={(e) => setHow(e.target.value)} placeholder={`How ${resolving} was settled, in one line`} aria-label={`How ${resolving} was settled`} maxLength={500} />
          <button type="submit" disabled={busy || !how.trim()}>{`Resolve ${resolving}`}</button>
          <button type="button" onClick={() => setWhich(null)}>Cancel</button>
        </form>
      )}
      {msg && <p className={msg.ok ? "ok small" : "err small"}>{msg.text}</p>}
      {!checked && <p className="meta small">Not checked against the project's picture yet. Copy the brief for a session to check it; until it's checked, nothing holds it back.</p>}
      {checked && body !== null && section === null && <p className="err small">Its Vision section is gone from the text. Record a new check.</p>}
      {section !== null && <Markdown text={section} />}
    </section>
  );
}
