import { useContext, useEffect, useState } from "react";
import { blockSentence } from "./blockWords";
import { BranchPane } from "./Branch";
import { KINDS } from "./fields";
import { claimant, earlierStages, heldByOther, isStage, leaseRunOut, passLabel, stageLabel } from "./lib";
import { Machine, spanText } from "./settingsData";
import { isHuman } from "./human";
import { isWork } from "./levels";
import Links from "./Links";
import { ownerOpen } from "./OwnerChecklist";
import { IMAGE } from "./paneData";
import { ImageThumb, toggled } from "./StageContent";
import { toBuilder } from "./StageForms";
import TagEditor from "./Tags";
import { Button, Segment, Segmented } from "./ui";
import type { Task } from "./types";
import type { TaskActions } from "./useTaskActions";

/** The task pane's right column: the next step as the one primary button, then Send back and
 *  Block or Unblock; under a rule, priority, type and who does it (the current value; click to choose),
 *  a started task's branch and worktree or its merge (0023), the estimate, tags, files and the
 *  task's links (0017). Every control is always in the same place. */
export default function Sidebar({ task, nowSecs, a, images, files }: {
  task: Task;
  nowSecs: number;
  a: TaskActions;
  images: Record<string, string>;
  /** Which of Files' images are open, flipping one, and the row under the columns that shows them. */
  files: { open: ReadonlySet<string>; onToggle: (name: string) => void; openId: string };
}) {
  return (
    <aside className="tp-side" aria-label="Actions and settings">
      <NextStep task={task} nowSecs={nowSecs} a={a} />
      <div className="side-settings" data-part="settings">
        <div className="kv">
          <span className="lbl">Priority</span>
          <ValueChip label="Priority" value={task.priority || "normal"} options={["high", "normal", "low"]} onPick={(v) => a.setField("priority", v, "Priority set.")} />
        </div>
        <div className="kv">
          <span className="lbl">Type</span>
          {/* The core's types (0017); an old file's idea still shows, as the one pressed. */}
          <ValueChip label="Type" value={task.kind || "no type"} options={KINDS} onPick={(v) => a.setField("type", v, "Type set.")} />
        </div>
        {/* Who does it (0022): a task is the owner's own (human) or an agent's (ai). Goals and
            features are their tasks' sum, nobody's to do. */}
        {isWork(task) && (
          <div className="kv">
            <span className="lbl">For</span>
            <ValueChip
              label="For"
              value={isHuman(task) ? "human" : "ai"}
              options={["human", "ai"]}
              onPick={(v) => a.setField("for", v, v === "human" ? "It's yours to do now: hatched on the board, with the You ribbon." : "It's the agents' now.")}
            />
          </div>
        )}
        {/* A task started in its own worktree (0023): where; after the merge, the commit and report. */}
        <BranchPane task={task} />
        {/* Three fields (agent time, test time, tokens) need the column's width: label above. */}
        <div className="kv stack">
          <span className="lbl">Est</span>
          <Estimate task={task} onSave={a.setField} />
        </div>
        <div className="kv stack">
          <span className="lbl">Tags</span>
          <TagEditor tags={task.tags} onChange={(t) => a.setField("tags", t.join(","), "Tags saved.")} />
        </div>
        <FilesView task={task} images={images} open={files.open} onToggle={files.onToggle} onAttach={a.attachPick} openId={files.openId} />
        <Links task={task} />
      </div>
    </aside>
  );
}

/** The next step for this stage, and the two ways off it. A goal or feature is passed on only
 *  while it's shaped (it never goes into or through Building) and has no claim to release. The
 *  owner's own task (0022) is Start in To do and Done in Doing. */
function NextStep({ task, nowSecs, a }: { task: Task; nowSecs: number; a: TaskActions }) {
  const human = isHuman(task);
  const pass = passLabel(task.stage, human);
  const back = earlierStages(task.stage);
  const holder = heldByOther(task);
  const shapedOnly = !isWork(task);
  // A claim nobody has heard from past the lease (this machine's, Settings 0036): the owner may give it back to Ready.
  const lease = useContext(Machine).leaseMinutes;
  const stale = !shapedOnly && leaseRunOut(task, nowSecs, lease);
  const showPass = pass !== null && !holder && (!shapedOnly || task.stage === "backlog" || task.stage === "shaping");
  const yoursOpen = !human && task.stage === "your-check" && ownerOpen(task) > 0;
  return (
    <div className="next-step" role="group" aria-label="Next step" data-part="next-step">
      <span className="lbl">Next step</span>
      {showPass && (
        <Button
          type="button"
          variant="primary"
          disabled={task.blocked || Boolean(task.paused) || a.busy || yoursOpen}
          title={task.paused ? "Paused: resume it first" : yoursOpen ? "Tick yours in the checklist first" : undefined}
          onClick={a.pass}
        >
          {pass}
        </Button>
      )}
      {stale && (
        <Button
          type="button"
          variant={showPass ? "default" : "primary"}
          className={showPass ? "release" : undefined}
          disabled={a.busy}
          title="Back to Ready, with the claim released and logged as reclaimed"
          onClick={a.release}
        >
          Release
        </Button>
      )}
      {back.length > 0 && <Button type="button" disabled={a.busy} onClick={() => a.open("back")}>{toBuilder(task.stage, human) ? "Send back to builder" : "Send back"}</Button>}
      {task.blocked ? (
        <Button type="button" disabled={a.busy} onClick={a.unblock}>Unblock</Button>
      ) : (
        <Button type="button" disabled={a.busy} onClick={() => a.open("block")}>Block</Button>
      )}
      {holder && (
        <p className="meta small">
          {holder} holds it in {stageLabel(task.stage)}, so only they can pass it on. To move it yourself, click a stage on the track.
        </p>
      )}
      {stale && (
        <p className="meta small">
          {claimant(task.claimedBy)} hasn't been heard from for {spanText(lease)}. Release puts it back in Ready, logged as reclaimed.
        </p>
      )}
      {showPass && task.blocked && <p className="meta small">{blockSentence(task) || "Blocked on something it doesn't name"}. Unblock it to pass it on.</p>}
      {showPass && task.paused && <p className="meta small">Paused: resume it (beside its stage, above) to pass it on.</p>}
      {!showPass && !stale && !holder && <p className="meta small">{nothingNext(task)}</p>}
    </div>
  );
}

/** Why there's no next step to click. */
function nothingNext(t: Task): string {
  if (!isStage(t.stage)) return "A stage this window doesn't know: click one on the track.";
  if (t.stage === "done") return "Done. Send it back to reopen it.";
  return "Its tasks move it on.";
}

/** A field's current value as one chip. Clicking it lays the options out in its place (no
 *  popover); picking one, Esc, or tabbing away closes them. Picking the current value changes nothing. */
export function ValueChip({ label, value, options, onPick }: { label: string; value: string; options: readonly string[]; onPick: (v: string) => void }) {
  const [open, setOpen] = useState(false);
  const pick = (v: string) => {
    setOpen(false);
    if (v !== value) onPick(v);
  };
  return <ValueChipView label={label} value={value} options={options} open={open} onOpen={() => setOpen(true)} onClose={() => setOpen(false)} onPick={pick} />;
}

export function ValueChipView({ label, value, options, open, onOpen, onClose, onPick }: {
  label: string; value: string; options: readonly string[]; open: boolean; onOpen: () => void; onClose: () => void; onPick: (v: string) => void;
}) {
  if (!open) {
    return <button type="button" className="vchip" aria-label={`${label}: ${value}, click to change`} onClick={onOpen}>{value}</button>;
  }
  // A value the options don't name (P1 from an older tool) still shows, as the one pressed.
  const shown = options.includes(value) ? options : [value, ...options];
  return (
    <Segmented
      aria-label={label}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          onClose();
        }
      }}
      onBlur={(e) => {
        // Tabbing away closes it. A click elsewhere leaves it open: WebKit doesn't focus a clicked
        // button, so a click on an option arrives with no new focus, and must still land.
        if (e.relatedTarget && !e.currentTarget.contains(e.relatedTarget)) onClose();
      }}
    >
      {shown.map((o) => (
        <Segment key={o} type="button" active={o === value} autoFocus={o === value} onClick={() => onPick(o)}>{o}</Segment>
      ))}
    </Segmented>
  );
}

/** Estimated agent time, test time (the QA agent plus the owner's check) and tokens, saved when a
 *  field loses focus or on Enter. */
function Estimate({ task, onSave }: { task: Task; onSave: (field: string, value: string, done: string) => void }) {
  const [time, setTime] = useState(task.estTime);
  const [testTime, setTestTime] = useState(task.estTestTime);
  const [tokens, setTokens] = useState(task.estTokens);
  useEffect(() => setTime(task.estTime), [task.estTime]);
  useEffect(() => setTestTime(task.estTestTime), [task.estTestTime]);
  useEffect(() => setTokens(task.estTokens), [task.estTokens]);
  const saved = { time: "Time estimate saved.", "test-time": "Test time estimate saved.", tokens: "Token estimate saved." };
  const save = (field: "time" | "test-time" | "tokens", value: string, was: string) => {
    if (value.trim() === was) return;
    onSave(field, value, saved[field]);
  };
  const keys = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") (e.target as HTMLInputElement).blur();
  };
  return (
    <span className="est-pair">
      <input value={time} onChange={(e) => setTime(e.target.value)} onBlur={() => save("time", time, task.estTime)} onKeyDown={keys} placeholder="2h" size={4} maxLength={10} aria-label="Agent time" title="Agent time (45m, 2h, 1.5d)" />
      <input value={testTime} onChange={(e) => setTestTime(e.target.value)} onBlur={() => save("test-time", testTime, task.estTestTime)} onKeyDown={keys} placeholder="30m" size={4} maxLength={10} aria-label="Test time" title="Test time, the QA agent plus your check (30m, 1h)" />
      <input value={tokens} onChange={(e) => setTokens(e.target.value)} onBlur={() => save("tokens", tokens, task.estTokens)} onKeyDown={keys} placeholder="500k" size={4} maxLength={10} aria-label="Tokens" title="Tokens (150k, 1.2M)" />
    </span>
  );
}

/** The task's files: images as small thumbnails, the others by name, and Attach. Paste or drop
 *  files anywhere on the pane too. The images come loaded from the pane (`images`, by file name),
 *  shared with Your check's proof. A click opens an image large in the pane's full-width row under
 *  the columns (FilesOpen, mu-91: this column is too narrow to read one in), several at once; a
 *  second click shuts it. The pane holds which are open (`open`, by file name). */
export function FilesView({ task, images, open, onToggle, onAttach, openId }: {
  task: Task;
  images: Record<string, string>;
  open: ReadonlySet<string>;
  onToggle: (name: string) => void;
  onAttach: () => void;
  /** The id of the row that shows the open images. */
  openId: string;
}) {
  const names = task.attachments.filter((n) => IMAGE.test(n));
  const others = task.attachments.filter((n) => !IMAGE.test(n));
  return (
    <div className="kv stack files" aria-label="Files">
      <span className="lbl">Files <small>{task.attachments.length}</small></span>
      <span className="file-list">
        {names.map((n, i) => <ImageThumb key={n} id={thumbId(openId, i)} name={n} url={images[n]} open={open.has(n)} onToggle={() => onToggle(n)} controls={openId} />)}
        {others.map((n) => <span key={n} className="other-file" title={n}>{n}</span>)}
        <Button type="button" className="attach" onClick={onAttach}>+ Attach</Button>
      </span>
    </div>
  );
}

/** The id of Files' i-th image thumbnail, from the open row's id. */
export function thumbId(openId: string, i: number): string {
  return `${openId}-thumb-${i}`;
}

function focusById(id: string) {
  document.getElementById(id)?.focus();
}

/** The id of Files' i-th image as it shows open, in the row under the columns. */
export function bigId(openId: string, i: number): string {
  return `${openId}-big-${i}`;
}

/** What toggling a Files image does (mu-91): flips it in `open`, and one that opens is brought
 *  into view (`show`, given its figure's id) once it's drawn, since the row can sit below the fold. */
export function fileToggler(task: Pick<Task, "attachments">, openId: string, open: ReadonlySet<string>, setOpen: (f: (o: ReadonlySet<string>) => Set<string>) => void, show: (id: string) => void = showSoon) {
  const names = task.attachments.filter((n) => IMAGE.test(n));
  return (n: string) => {
    setOpen((o) => toggled(o, n));
    if (!open.has(n) && names.includes(n)) show(bigId(openId, names.indexOf(n)));
  };
}

/** Scrolls the element with this id into view after the next render: as little as shows it, and
 *  not smoothly when the system asks for less motion. Again once its image has loaded, since it
 *  only then has its height. */
export function showSoon(id: string, win: Pick<Window, "requestAnimationFrame" | "matchMedia" | "document"> = window) {
  win.requestAnimationFrame(() => {
    const el = win.document.getElementById(id);
    if (!el) return;
    const still = win.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const go = () => el.scrollIntoView({ block: "nearest", behavior: still ? "auto" : "smooth" });
    go();
    const img = el.querySelector("img");
    if (img && !img.complete) img.addEventListener("load", go, { once: true });
  });
}

/** Files' open images (mu-91), each at the pane's full width in a row under the two columns, in
 *  the task's order, with its name under it; clicking one shuts it, as its thumbnail does. Nothing
 *  while none is open: the row stays, hidden, so the thumbnails' `aria-controls` always resolve; a
 *  task with no image has no row. Shutting one from here puts the focus on its thumbnail, since
 *  the button that had it is gone. */
export function FilesOpen({ task, images, open, onToggle, id, focus = focusById }: {
  task: Task;
  images: Record<string, string>;
  open: ReadonlySet<string>;
  onToggle: (name: string) => void;
  id: string;
  /** Puts the focus on the element with this id (a test passes its own). */
  focus?: (id: string) => void;
}) {
  const names = task.attachments.filter((n) => IMAGE.test(n));
  if (names.length === 0) return null;
  const shown = names.filter((n) => open.has(n));
  const shut = (n: string) => {
    onToggle(n);
    focus(thumbId(id, names.indexOf(n)));
  };
  return (
    <div className="files-open" id={id} data-part="files-open" aria-label="Open files" hidden={shown.length === 0}>
      {shown.map((n) => (
        <figure key={n} id={bigId(id, names.indexOf(n))} className="file-big">
          <button type="button" className="file-big-btn" aria-label={`Shut ${n}`} title={`Shut ${n}`} onClick={() => shut(n)}>
            {images[n] ? <img src={images[n]} alt={n} /> : <span className="mini-img" />}
          </button>
          <figcaption dir="auto">{n}</figcaption>
        </figure>
      ))}
    </div>
  );
}
