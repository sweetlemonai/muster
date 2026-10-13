import { Fragment, useContext, useState } from "react";
import { HelpButton } from "./HelpButton";
import { ago, holderOf, isStage, splitHolder, stageHolders, stageLabel, stageOf, STAGES, startedAgo } from "./lib";
import BlockWhy from "./BlockWhy";
import { taskKey } from "./taskKey";
import { isLive, waitingOnYou } from "./paneData";
import { Machine } from "./settingsData";
import { isWork, levelLabel, shownStageClass, shownStageLabel } from "./levels";
import { isHuman } from "./human";
import type { Task } from "./types";
import YouRibbon from "./YouRibbon";

/** The top of the task pane: where it is and when it changed, the title (click to rename it in
 *  place), and the status line. */
export default function PaneHeader({ task, now, onClose, onRename, onOpenTask }: { task: Task; now: number; onClose: () => void; onRename: (title: string) => void; onOpenTask?: (id: string) => void }) {
  const [title, setTitle] = useState<string | null>(null);
  const since = ago(now, task.modified);
  return (
    <>
      <div className="topline">
        <span className="meta">
          {isHuman(task) && <><YouRibbon />{" "}</>}
          <b className="qname">{task.queue}</b> · changed {since === "now" ? "just now" : `${since} ago`}
        </span>
        <button type="button" className="close" onClick={onClose} aria-label="Close">×</button>
      </div>
      {title === null ? (
        <h1 className="pane-title" data-part="title">
          <span className="tid">{taskKey(task)}</span>{" "}
          <button type="button" className="title-btn" title="Click to rename" onClick={() => setTitle(task.title)}>{task.title}</button>{" "}
          <HelpButton topic="task-pane" />
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
      <StatusLine task={task} now={now} onOpenTask={onOpenTask} />
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

/** "you" for the owner, else who as written, session and all (the pane shows the full id; cards
 *  the short one). */
const person = (who: string) => (splitHolder(who).label === "owner" ? (who === "owner" ? "you" : `you, ${who}`) : who);

/** Stage, why it's blocked (the kind in bold, each task it's blocked by a link, 0024), who's on it
 *  (in full, session and all; "Your turn" in Your check, 0043), whether it's live and since when,
 *  who held the stages before (0030), its role ("your own task" for a human task, 0022), and what's waiting on the owner
 *  (gold) or that nothing is. A goal or feature shows the stage its children give it, and its
 *  level. */
export function StatusLine({ task, now, onOpenTask }: { task: Task; now: number; onOpenTask?: (id: string) => void }) {
  const work = isWork(task);
  const known = isStage(task.stage);
  const pill = work
    ? <span className={`status-pill s-${stageOf(task)}`}>{known ? stageLabel(task.stage) : `${task.stage || "no stage"} (unknown)`}</span>
    : <span className={`status-pill s-${shownStageClass(task)}`}>{shownStageLabel(task)}</span>;
  // The owner's own task (0022) is theirs in every stage, whatever the stage's usual role.
  const role = work ? (known ? (isHuman(task) ? "your own task" : STAGES.find((s) => s.id === task.stage)?.role) : "") : levelLabel(task.level);
  const who = holderOf(task);
  // Your check is the owner's turn, not a claim on it (0043).
  const yourTurn = task.stage === "your-check" && splitHolder(who).label === "owner";
  const started = startedAgo(task, now);
  const earlier = stageHolders(task).filter((h) => h.stage !== task.stage);
  const live = isLive(task, now, useContext(Machine).liveMinutes * 60);
  const waiting = waitingOnYou(task);
  return (
    <div className="status-line" data-part="status">
      {pill}
      <BlockWhy t={task} onOpenTask={onOpenTask} />
      {(who || started || earlier.length > 0) && (
        <span className="holder">
          {who && (
            <>
              <span className={live ? "pulse" : "pulse idle"} aria-hidden="true" />
              {live ? <b className="live-badge">LIVE</b> : <span className="meta">quiet {ago(now, task.modified)} ·</span>}
            </>
          )}
          <span className="holders">
            {who && (yourTurn ? <b className="your-turn">Your turn</b> : <>on it: {person(who)}</>)}
            {started && <>{who ? " · " : ""}{started}</>}
            {earlier.map((h, i) => (
              <Fragment key={h.stage}>{who || started || i > 0 ? " · " : ""}{stageLabel(h.stage)}: {person(h.who)}</Fragment>
            ))}
          </span>
        </span>
      )}
      {role && <span className="meta">{role}</span>}
      {waiting.length > 0 ? <span className="needs">Waiting on you: {waiting.join(" · ")}</span> : <span className="meta">nothing waiting on you</span>}
    </div>
  );
}
