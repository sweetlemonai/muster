import { useState } from "react";
import { ago, holder, isStage, stageLabel, stageOf, STAGES } from "./lib";
import { isLive, waitingOnYou } from "./paneData";
import { isWork, levelLabel, shownStageClass, shownStageLabel } from "./levels";
import type { Task } from "./types";

/** The top of the task pane: where it is and when it changed, the title (click to rename it in
 *  place), and the status line. */
export default function PaneHeader({ task, now, onClose, onRename }: { task: Task; now: number; onClose: () => void; onRename: (title: string) => void }) {
  const [title, setTitle] = useState<string | null>(null);
  const since = ago(now, task.modified);
  return (
    <>
      <div className="topline">
        <span className="meta">
          <b className="qname">{task.queue}</b> · changed {since === "now" ? "just now" : `${since} ago`}
        </span>
        <button type="button" className="close" onClick={onClose} aria-label="Close">×</button>
      </div>
      {title === null ? (
        <h1 className="pane-title" data-part="title">
          <span className="tid">{task.id}</span>{" "}
          <button type="button" className="title-btn" title="Click to rename" onClick={() => setTitle(task.title)}>{task.title}</button>
        </h1>
      ) : (
        <TitleEdit
          value={title}
          onChange={setTitle}
          onCancel={() => setTitle(null)}
          onSave={() => {
            setTitle(null);
            onRename(title);
          }}
        />
      )}
      <StatusLine task={task} now={now} />
    </>
  );
}

/** The title as a text box, in its own place: Enter or Save saves, Esc or Cancel keeps the old one. */
export function TitleEdit({ value, onChange, onSave, onCancel }: { value: string; onChange: (v: string) => void; onSave: () => void; onCancel: () => void }) {
  return (
    <form
      className="control-row title-edit"
      data-part="title"
      onSubmit={(e) => {
        e.preventDefault();
        if (value.trim()) onSave();
      }}
    >
      <input
        autoFocus
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          // Esc here leaves the rename, not the pane.
          if (e.key === "Escape") {
            e.stopPropagation();
            onCancel();
          }
        }}
        maxLength={200}
        aria-label="Title"
      />
      <button type="submit" disabled={!value.trim()}>Save</button>
      <button type="button" onClick={onCancel}>Cancel</button>
    </form>
  );
}

/** Stage, blocked and what it waits on, who's on it and whether it's live, its role, and what's
 *  waiting on the owner (gold) or that nothing is. A goal or feature shows the stage its children
 *  give it, and its level. */
export function StatusLine({ task, now }: { task: Task; now: number }) {
  const work = isWork(task);
  const known = isStage(task.stage);
  const pill = work
    ? <span className={`status-pill s-${stageOf(task)}`}>{known ? stageLabel(task.stage) : `${task.stage || "no stage"} (unknown)`}</span>
    : <span className={`status-pill s-${shownStageClass(task)}`}>{shownStageLabel(task)}</span>;
  const role = work ? (known ? STAGES.find((s) => s.id === task.stage)?.role : "") : levelLabel(task.level);
  const who = holder(task);
  const live = isLive(task, now);
  const waiting = waitingOnYou(task);
  return (
    <div className="status-line" data-part="status">
      {pill}
      {task.blocked && <span className="status-pill blocked">blocked{task.waitingOn ? `: ${task.waitingOn}` : ""}</span>}
      {who && (
        <span className="holder">
          <span className={live ? "pulse" : "pulse idle"} aria-hidden="true" />
          on it: {who}
          {live ? <b className="live-badge">LIVE</b> : <span className="meta">· quiet {ago(now, task.modified)}</span>}
        </span>
      )}
      {role && <span className="meta">{role}</span>}
      {waiting.length > 0 ? <span className="needs">Waiting on you: {waiting.join(" · ")}</span> : <span className="meta">nothing waiting on you</span>}
    </div>
  );
}
