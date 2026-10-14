import { useCallback, useEffect, useState } from "react";
import AnswerBox from "./AnswerBox";
import { answerId } from "./answerFocus";
import { api } from "./api";
import Comments from "./Comments";
import EditBody from "./EditBody";
import { splitLog, withoutComments } from "./fields";
import { isHuman } from "./human";
import { stageLabel } from "./lib";
import Markdown from "./Markdown";
import { Fold } from "./ModeViews";
import { taskKey, taskOfHref } from "./taskKey";
import PaneHeader from "./PaneHeader";
import { isFileDrag, logSummary, releasedBy, undoFor } from "./paneData";
import { SessionPing } from "./Ping";
import { reviewStatus, withoutReview } from "./Review";
import ReviewPanel from "./ReviewPanel";
import Sidebar from "./Sidebar";
import StageContent from "./StageContent";
import StageForms from "./StageForms";
import StageTrack from "./StageTrack";
import type { Task } from "./types";
import { Button, Pill } from "./ui";
import { useTaskActions } from "./useTaskActions";
import { visionStatus, withoutVision } from "./Vision";
import VisionPanel from "./VisionPanel";
import { anyDirty, withPart } from "./writeUp";
import Work from "./Work";
import type { NamedView } from "./yourCheck";

/** The task pane (0021, the owner's design D2): title and status first, the stage track, then
 *  two columns (what this stage needs; the next step and the task's settings), the stage forms
 *  under both, the path; then open questions, the write-up, the vision check (0029), the review of
 *  record (0027), the comment thread (0017) and the Log and, at the bottom, the note bar. */
export default function TaskPanel({ task, now, onClose, onChanged, onDirty, nudge, childTasks, onOpenTask, onOpenView, keys, onOpenAt }: {
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
  /** Open a view an owner criterion names, from its button in Your check (0043). */
  onOpenView?: (v: NamedView) => void;
  /** Each project's prefix to its queue (0033): a key written in the text links to its task, which
   *  `onOpenAt` opens, in whichever project it is. */
  keys?: Map<string, string>;
  onOpenAt?: (queue: string, id: string) => void;
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
  const openLink = (href: string) => {
    const at = taskOfHref(href);
    if (at) onOpenAt?.(at.queue, at.id);
  };

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

  // The comments show as their own thread, between the write-up and the Log, and the Vision and
  // Review sections in their own places under the write-up (VisionPanel, ReviewPanel), so they're
  // left out of the rendering.
  const checked = (task.visionOpen ?? null) !== null;
  const reviewed = (task.reviewRound ?? null) !== null;
  const [shown, log] = splitLog(body ? withoutComments(withoutReview(withoutVision(body.replace(/^---\n[\s\S]*?\n---\n/, ""), checked), reviewed)) : "");
  const open = task.questions.filter((q) => !q.ticked && q.n !== null);
  const undo = undoFor(a.msg, task.stage);

  return (
    <div
      className={`panel task-pane${over ? " drop" : ""}`}
      aria-label={`Task ${taskKey(task)}`}
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
        <PaneHeader
          task={task}
          now={now}
          onClose={onClose}
          onRename={(t) => a.setField("title", t, "Renamed.")}
          onOpenTask={onOpenTask}
          pause={{ busy: a.busy, onPause: a.pause, onResume: a.resume }}
        />
        <StageTrack stage={task.stage} busy={a.busy} onPick={a.move} releases={(to) => releasedBy(task, to)} human={isHuman(task)} paused={task.paused ?? ""} />
        <div className="tp-cols">
          <div className="tp-main">
            <StageContent task={task} nowSecs={now} body={body === null ? null : shown} childTasks={childTasks} busy={a.busy} onTick={a.tick} onView={onOpenView} />
          </div>
          <Sidebar task={task} nowSecs={now} a={a} />
        </div>
        <StageForms task={task} a={a} />
        <SessionPing task={task} now={now} onChanged={onChanged} />
        {a.msg && (
          <p className={a.msg.ok ? "ok small pane-msg" : "err small pane-msg"} role="status">
            {a.msg.text}
            {undo && (
              <Button type="button" className="undo" disabled={a.busy} onClick={() => a.undo(undo)}>
                Undo: back to {stageLabel(undo.back)}
              </Button>
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
            <AnswerBox key={q.n} task={task} q={q} onChanged={onChanged} keys={keys} onLink={openLink} boxId={q.n === null ? undefined : answerId(task, q.n)} />
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
              <Pill on={!raw} onClick={() => setRaw(false)}>Rendered</Pill>
              <Pill on={raw} onClick={() => setRaw(true)}>Raw</Pill>
              <Pill className="edit" onClick={startEdit} title="Edit the description, To do and criteria here (⌘S saves, Esc cancels)">Edit</Pill>
            </div>
            {editMsg && <p className={editMsg.ok ? "ok small" : "err small"}>{editMsg.text}</p>}
            {error && <p className="err">{error}</p>}
            {body === null && !error && <p className="none">Loading…</p>}
            {body !== null && (raw ? <pre className="raw">{body}</pre> : <Markdown text={shown} keys={keys} onLink={openLink} />)}
          </>
        )}
      </section>

      {/* Standard and Focus fold these to a header each (0038); ADHD shows them as they are. */}
      <Fold label="Vision" summary={visionStatus(task)}>
        <VisionPanel task={task} body={body} onChanged={onChanged} />
      </Fold>
      <Fold label="Review" summary={reviewStatus(task)}>
        <ReviewPanel task={task} body={body} />
      </Fold>
      <Fold label="Work" summary="who worked on it, time and tokens">
        <Work task={task} />
      </Fold>

      <Comments task={task} onChanged={onChanged} onDirty={commentDirty} nudge={nudge} keys={keys} onLink={openLink} />
      {((log && !raw && editing === null) || task.checkins.length > 0) && (
        <Fold label="Log and check-ins" summary={logSummary(log && !raw && editing === null ? log : "", task.checkins.length)}>
          {log && !raw && editing === null && (
            <section className="log">
              <Markdown text={log} keys={keys} onLink={openLink} />
            </section>
          )}

          {task.checkins.length > 0 && <Checkins task={task} />}
        </Fold>
      )}

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
      <Button type="submit" disabled={!note.trim()}>Add to log</Button>
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
