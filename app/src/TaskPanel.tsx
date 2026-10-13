import { useCallback, useEffect, useState } from "react";
import { api } from "./api";
import Comments from "./Comments";
import EditBody from "./EditBody";
import { splitLog, withoutComments } from "./fields";
import { stageLabel, stamp } from "./lib";
import Markdown from "./Markdown";
import PaneHeader from "./PaneHeader";
import { isFileDrag, releasedBy, undoFor } from "./paneData";
import Sidebar from "./Sidebar";
import StageContent from "./StageContent";
import StageForms from "./StageForms";
import StageTrack from "./StageTrack";
import type { Question, Task } from "./types";
import { useTaskActions } from "./useTaskActions";
import { anyDirty, withPart } from "./writeUp";

/** The task pane (0021, the owner's design D2): title and status first, the stage track, then
 *  two columns (what this stage needs; the next step and the task's settings), the stage forms
 *  under both, the path; then open questions, the write-up, the comment thread (0017) and the
 *  Log and, at the bottom, the note bar. */
export default function TaskPanel({ task, now, onClose, onChanged, onDirty, nudge, childTasks, onOpenTask }: {
  task: Task;
  now: number;
  onClose: () => void;
  onChanged: () => void;
  /** `onDirty` and `nudge` pass between the window and the parts with typed text, the write-up
   *  editor (see EditBody) and the comment box: the pane is unsaved while either is. */
  onDirty?: (dirty: boolean) => void;
  nudge?: number;
  /** A goal's or feature's children, as far as the window has them. */
  childTasks?: Task[];
  /** Open another task of this queue in the pane, by id (the tasks a block names, 0024). */
  onOpenTask?: (id: string) => void;
}) {
  const [dirtyParts, setDirtyParts] = useState<Record<string, boolean>>({});
  const editDirty = useCallback((d: boolean) => setDirtyParts((p) => withPart(p, "edit", d)), []);
  const commentDirty = useCallback((d: boolean) => setDirtyParts((p) => withPart(p, "comment", d)), []);
  const unsaved = anyDirty(dirtyParts);
  useEffect(() => {
    onDirty?.(unsaved);
  }, [unsaved, onDirty]);
  useEffect(() => () => onDirty?.(false), [onDirty]);
  const [body, setBody] = useState<string | null>(null);
  const [raw, setRaw] = useState(false);
  const [error, setError] = useState("");
  // The write-up Edit opened, while the editor is open; what the last edit came to.
  const [editing, setEditing] = useState<string | null>(null);
  const [editMsg, setEditMsg] = useState<{ ok: boolean; text: string } | null>(null);
  // Bumped after a save, so the text reloads even within the second the file last changed.
  const [saves, setSaves] = useState(0);
  const [over, setOver] = useState(false);
  const a = useTaskActions(task, onChanged);

  // Reload the text when the file changes (its modified time moves).
  useEffect(() => {
    let live = true;
    api.taskBody(task.queue, task.id).then(
      (b) => live && setBody(b),
      (e) => live && setError(String(e)),
    );
    return () => {
      live = false;
    };
  }, [task.queue, task.id, task.modified, saves]);

  const startEdit = () => {
    setEditMsg(null);
    api.editableBody(task.queue, task.id).then(setEditing, (e) => setEditMsg({ ok: false, text: String(e) }));
  };
  // Files pasted on the header or the note bar, or dropped anywhere on the pane, are attached.
  const onPaste = (e: React.ClipboardEvent) => {
    if (e.clipboardData.files.length) {
      e.preventDefault();
      a.attachFiles(e.clipboardData.files);
    }
  };

  // The comments show as their own thread, between the write-up and the Log, so they're left out
  // of the rendering.
  const [shown, log] = splitLog(body ? withoutComments(body.replace(/^---\n[\s\S]*?\n---\n/, "")) : "");
  const open = task.questions.filter((q) => !q.ticked && q.n !== null);
  const undo = undoFor(a.msg, task.stage);

  return (
    <div
      className={`panel task-pane${over ? " drop" : ""}`}
      aria-label={`Task ${task.id}`}
      // Only file drags are the pane's: text dragged into the editor or an answer box drops there.
      onDragOver={(e) => {
        if (!isFileDrag(e.dataTransfer.types)) return;
        e.preventDefault();
        setOver(true);
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(e) => {
        if (!isFileDrag(e.dataTransfer.types)) return;
        e.preventDefault();
        setOver(false);
        a.attachFiles(e.dataTransfer.files);
      }}
    >
      <header className="pane-top" onPaste={onPaste}>
        <PaneHeader task={task} now={now} onClose={onClose} onRename={(t) => a.setField("title", t, "Renamed.")} onOpenTask={onOpenTask} />
        <StageTrack stage={task.stage} busy={a.busy} onPick={a.move} releases={(to) => releasedBy(task, to)} />
        <div className="tp-cols">
          <div className="tp-main">
            <StageContent task={task} nowSecs={now} body={body === null ? null : shown} childTasks={childTasks} busy={a.busy} onTick={a.tick} />
          </div>
          <Sidebar task={task} nowSecs={now} a={a} />
        </div>
        <StageForms task={task} a={a} />
        {a.msg && (
          <p className={a.msg.ok ? "ok small pane-msg" : "err small pane-msg"} role="status">
            {a.msg.text}
            {undo && (
              <button type="button" className="undo" disabled={a.busy} onClick={() => a.undo(undo)}>
                Undo: back to {stageLabel(undo.back)}
              </button>
            )}
          </p>
        )}
        <div className="pathline" data-part="path" title={task.path}>
          {task.commit ? `commit ${task.commit}` : "commit none"} · {task.path}
        </div>
      </header>

      {open.length > 0 && (
        <section className="answers" data-part="questions">
          <h2>Open questions</h2>
          {open.map((q) => (
            <AnswerBox key={q.n} task={task} q={q} onChanged={onChanged} />
          ))}
        </section>
      )}

      <section className="body" data-part="body">
        {editing !== null ? (
          <EditBody
            task={task}
            opened={editing}
            onDirty={editDirty}
            nudge={nudge}
            onClose={() => setEditing(null)}
            onSaved={(text) => {
              setEditing(null);
              setEditMsg({ ok: true, text });
              setSaves((n) => n + 1);
              onChanged();
            }}
          />
        ) : (
          <>
            <div className="body-bar">
              <button className={raw ? "tab" : "tab on"} onClick={() => setRaw(false)}>Rendered</button>
              <button className={raw ? "tab on" : "tab"} onClick={() => setRaw(true)}>Raw</button>
              <button className="tab edit" onClick={startEdit} title="Edit the description, To do and criteria here (⌘S saves, Esc cancels)">Edit</button>
            </div>
            {editMsg && <p className={editMsg.ok ? "ok small" : "err small"}>{editMsg.text}</p>}
            {error && <p className="err">{error}</p>}
            {body === null && !error && <p className="none">Loading…</p>}
            {body !== null && (raw ? <pre className="raw">{body}</pre> : <Markdown text={shown} />)}
          </>
        )}
      </section>

      <Comments task={task} onChanged={onChanged} onDirty={commentDirty} nudge={nudge} />
      {log && !raw && editing === null && (
        <section className="log">
          <Markdown text={log} />
        </section>
      )}

      {task.checkins.length > 0 && <Checkins task={task} />}

      <NoteBar onNote={a.note} onPaste={onPaste} />
    </div>
  );
}

/** A note to the log, in a bar fixed at the bottom of the pane. */
function NoteBar({ onNote, onPaste }: { onNote: (text: string) => void; onPaste: (e: React.ClipboardEvent) => void }) {
  const [note, setNote] = useState("");
  return (
    <form
      className="note-bar"
      data-part="note-bar"
      onPaste={onPaste}
      onSubmit={(e) => {
        e.preventDefault();
        const n = note;
        setNote("");
        onNote(n);
      }}
    >
      <input value={note} onChange={(e) => setNote(e.target.value)} placeholder="Add a note to the log… (or paste / drop files here)" aria-label="Note to the log" maxLength={2000} />
      <button type="submit" disabled={!note.trim()}>Add to log</button>
    </form>
  );
}

/** Every check-in, in order: when, how far (the session's own estimate), what, who (0030). */
function Checkins({ task }: { task: Task }) {
  return (
    <section className="checkins">
      <h2>Check-ins <small>{task.checkins.length}</small></h2>
      <p className="meta small">The percentages are each session's own estimate.</p>
      <ol>
        {task.checkins.map((c, i) => (
          <li key={i}>
            <span className="when">{c.at}</span> {c.pct !== null && <b>{c.pct}%</b>} {c.text} <span className="meta">· {c.who}</span>
          </li>
        ))}
      </ol>
    </section>
  );
}

function AnswerBox({ task, q, onChanged }: { task: Task; q: Question; onChanged: () => void }) {
  const [text, setText] = useState("");
  const [state, setState] = useState<"idle" | "saving" | "done" | "error">("idle");
  const [msg, setMsg] = useState("");
  const send = async () => {
    if (!text.trim() || q.n === null) return;
    setState("saving");
    try {
      const asker = await api.answer(task.queue, task.id, q.n, text, stamp(new Date()));
      setState("done");
      setMsg(asker ? "Answered. The asking agent gets it after its next step." : "Answered, and written in the task. No asking session on record, so it waits there.");
      setText("");
      onChanged();
    } catch (e) {
      setState("error");
      setMsg(String(e));
    }
  };
  return (
    <div className={`answer${q.escalated ? " needs-you" : ""}`}>
      <p>
        <b>Q{q.n}</b> <span className="asker">{q.asker}</span>
        {q.escalated && <span className="badge warn">for you</span>}
      </p>
      <Markdown text={q.text} />
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder="Your answer (⌘↩ to send)"
        rows={3}
        maxLength={4000}
        onKeyDown={(e) => {
          if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) void send();
        }}
      />
      <div className="answer-bar">
        <button disabled={!text.trim() || state === "saving"} onClick={() => void send()}>
          {state === "saving" ? "Sending…" : "Answer"}
        </button>
        {msg && <span className={state === "error" ? "err" : "ok"}>{msg}</span>}
      </div>
    </div>
  );
}
