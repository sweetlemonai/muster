import { Fragment, useContext, useState } from "react";
import { HelpButton } from "./HelpButton";
import { ago, builtTested, holderOf, isStage, shownHolder, splitHolder, stageHolders, stageLabel, stageOf, STAGES, startedAgo, type RoleNames } from "./lib";
import { focusAnswer } from "./answerFocus";
import BlockWhy from "./BlockWhy";
import { taskKey } from "./taskKey";
import { isLive, waitingOnYou } from "./paneData";
import { sessionWords } from "./sessionState";
import { Machine, RoleNamesContext } from "./settingsData";
import { isWork, levelLabel, shownStageClass, shownStageLabel } from "./levels";
import { isHuman } from "./human";
import { PanePause, PausedChip } from "./Pause";
import type { Task } from "./types";
import { Button, StatusPill } from "./ui";
import YouRibbon from "./YouRibbon";
import ProjectName from "./ProjectName";

/** The top of the task pane: where it is and when it changed, the title (click to rename it in
 *  place), and the status line. */
export default function PaneHeader({ task, now, onClose, onRename, onOpenTask, pause }: { task: Task; now: number; onClose: () => void; onRename: (title: string) => void; onOpenTask?: (id: string) => void; pause?: PauseProps }) {
  const [title, setTitle] = useState<string | null>(null);
  const since = ago(now, task.modified);
  return (
    <>
      <div className="topline">
        <span className="meta">
          {isHuman(task) && <><YouRibbon />{" "}</>}
          <ProjectName name={task.queue} as="b" className="qname" /> · changed {since === "now" ? "just now" : `${since} ago`}
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
      <StatusLine task={task} now={now} onOpenTask={onOpenTask} pause={pause} />
    </>
  );
}

/** The pane's Pause and Resume (0045), run through the pane's actions. */
type PauseProps = { busy: boolean; onPause: (why: string) => Promise<boolean>; onResume: () => void };

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
      <Button type="submit" disabled={!value.trim()}>Save</Button>
      <Button type="button" onClick={onCancel}>Cancel</Button>
    </form>
  );
}

/** "you" for the owner, else who as written, session and all (the pane shows the full id; cards
 *  the short one). */
export const person = (who: string, names: RoleNames = {}) =>
  splitHolder(who).label === "owner" ? (who === "owner" ? "you" : `you, ${who}`) : shownHolder(who, names);

/** Stage, Paused with when and why and the Pause or Resume beside it (0045), why it's blocked (the
 *  kind in bold, each task it's blocked by a link, 0024), who's on it
 *  (in full, session and all; "Your turn" in Your check, 0043), whether it's live and since when,
 *  what Claude Code says its session is doing (0048),
 *  who held the stages before (0030), who built and who tested it (0047), its role ("your own task" for a human task, 0022), and what's waiting on the owner
 *  (gold) or that nothing is. A goal or feature shows the stage its children give it, and its
 *  level. */
export function StatusLine({ task, now, onOpenTask, pause }: { task: Task; now: number; onOpenTask?: (id: string) => void; pause?: PauseProps }) {
  const work = isWork(task);
  const known = isStage(task.stage);
  const pill = work
    ? <StatusPill status={stageOf(task)}>{known ? stageLabel(task.stage) : `${task.stage || "no stage"} (unknown)`}</StatusPill>
    : <StatusPill status={shownStageClass(task)}>{shownStageLabel(task)}</StatusPill>;
  // The owner's own task (0022) is theirs in every stage, whatever the stage's usual role.
  const role = work ? (known ? (isHuman(task) ? "your own task" : STAGES.find((s) => s.id === task.stage)?.role) : "") : levelLabel(task.level);
  const who = holderOf(task);
  // Your check is the owner's turn, not a claim on it (0043).
  const yourTurn = task.stage === "your-check" && splitHolder(who).label === "owner";
  const started = startedAgo(task, now);
  const earlier = stageHolders(task).filter((h) => h.stage !== task.stage);
  // A role slot by its role's name now (0112).
  const names = useContext(RoleNamesContext);
  const name = (w: string) => person(w, names);
  const built = builtTested(task, name);
  const live = isLive(task, now, useContext(Machine).liveMinutes * 60);
  const waiting = waitingOnYou(task);
  const session = sessionWords(task);
  return (
    <div className="status-line" data-part="status">
      {pill}
      <PausedChip t={task} now={now} />
      {pause && work && <PanePause task={task} busy={pause.busy} onPause={pause.onPause} onResume={pause.onResume} />}
      <BlockWhy t={task} onOpenTask={onOpenTask} onAnswer={(q) => { if (q.n !== null) focusAnswer(task, q.n); }} />
      {(who || started || earlier.length > 0 || built) && (
        <span className="holder">
          {who && (
            <>
              <span className={live ? "pulse" : "pulse idle"} aria-hidden="true" />
              {live ? <b className="live-badge">LIVE</b> : <span className="meta">quiet {ago(now, task.modified)} ·</span>}
            </>
          )}
          <span className="holders">
            {who && (yourTurn ? <b className="your-turn">Your turn</b> : <>on it: <bdi>{name(who)}</bdi></>)}
            {started && <>{who ? " · " : ""}{started}</>}
            {earlier.map((h, i) => (
              <Fragment key={h.stage}>{who || started || i > 0 ? " · " : ""}{stageLabel(h.stage)}: <bdi>{name(h.who)}</bdi></Fragment>
            ))}
          </span>
          {built && <span className="built-tested">{built}</span>}
        </span>
      )}
      {session && <span className={`session-state ${task.sessionState}`}>{session}</span>}
      {role && <span className="meta">{role}</span>}
      {waiting.length > 0 ? <span className="needs">Waiting on you: {waiting.join(" · ")}</span> : <span className="meta">nothing waiting on you</span>}
    </div>
  );
}
