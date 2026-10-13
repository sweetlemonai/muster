import { useEffect, useState } from "react";
import { api } from "./api";
import { BranchPane } from "./Branch";
import { KINDS } from "./fields";
import { claimant, earlierStages, heldByOther, isStage, LEASE_SECS, leaseRunOut, passLabel, stageLabel } from "./lib";
import { isHuman } from "./human";
import { isWork } from "./levels";
import Links from "./Links";
import { ownerOpen } from "./OwnerChecklist";
import TagEditor from "./Tags";
import type { Task } from "./types";
import type { TaskActions } from "./useTaskActions";

const IMAGE = /\.(png|jpe?g|gif|webp)$/i;

/** The task pane's right column: the next step as the one primary button, then Send back and
 *  Block or Unblock; under a rule, priority, type and who does it (the current value; click to choose),
 *  a started task's branch and worktree or its merge (0023), the estimate, tags, files and the
 *  task's links (0017). Every control is always in the same place. */
export default function Sidebar({ task, nowSecs, a }: { task: Task; nowSecs: number; a: TaskActions }) {
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
              onPick={(v) => a.setField("for", v, v === "human" ? "It's yours to do now: on the Human board." : "It's the agents' now: on the AI board.")}
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
        <Files task={task} onAttach={a.attachPick} />
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
  // A claim nobody has heard from past the lease: the owner may give it back to Ready.
  const stale = !shapedOnly && leaseRunOut(task, nowSecs);
  const showPass = pass !== null && !holder && (!shapedOnly || task.stage === "backlog" || task.stage === "shaping");
  const yoursOpen = !human && task.stage === "your-check" && ownerOpen(task) > 0;
  return (
    <div className="next-step" role="group" aria-label="Next step" data-part="next-step">
      <span className="lbl">Next step</span>
      {showPass && (
        <button
          type="button"
          className="primary"
          disabled={task.blocked || a.busy || yoursOpen}
          title={yoursOpen ? "Tick yours in the checklist first" : undefined}
          onClick={a.pass}
        >
          {pass}
        </button>
      )}
      {stale && (
        <button
          type="button"
          className={showPass ? "release" : "primary"}
          disabled={a.busy}
          title="Back to Ready, with the claim released and logged as reclaimed"
          onClick={a.release}
        >
          Release
        </button>
      )}
      {back.length > 0 && <button type="button" disabled={a.busy} onClick={() => a.open("back")}>Send back</button>}
      {task.blocked ? (
        <button type="button" disabled={a.busy} onClick={a.unblock}>Unblock</button>
      ) : (
        <button type="button" disabled={a.busy} onClick={() => a.open("block")}>Block</button>
      )}
      {holder && (
        <p className="meta small">
          {holder} holds it in {stageLabel(task.stage)}, so only they can pass it on. To move it yourself, click a stage on the track.
        </p>
      )}
      {stale && (
        <p className="meta small">
          {claimant(task.claimedBy)} hasn't been heard from for {LEASE_SECS / 3600} h. Release puts it back in Ready, logged as reclaimed.
        </p>
      )}
      {showPass && task.blocked && <p className="meta small">Blocked on {task.waitingOn || "something it doesn't name"}. Unblock it to pass it on.</p>}
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
    <span
      className="seg"
      role="group"
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
        <button key={o} type="button" className={o === value ? "on" : undefined} aria-pressed={o === value} autoFocus={o === value} onClick={() => onPick(o)}>{o}</button>
      ))}
    </span>
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

/** The task's files: images as small thumbnails that open large (← → between them, Esc to
 *  close), the others by name, and Attach. Paste or drop files anywhere on the pane too. */
function Files({ task, onAttach }: { task: Task; onAttach: () => void }) {
  const [images, setImages] = useState<Record<string, string>>({});
  const [big, setBig] = useState<number | null>(null);
  const names = task.attachments.filter((n) => IMAGE.test(n));
  const others = task.attachments.filter((n) => !IMAGE.test(n));

  useEffect(() => {
    let live = true;
    setImages({});
    for (const n of names) {
      api.image(task.queue, task.id, n).then(
        (url) => live && setImages((m) => ({ ...m, [n]: url })),
        () => {},
      );
    }
    return () => {
      live = false;
    };
    // `names` comes from task.attachments, so their joined list is the dependency.
  }, [task.queue, task.id, task.attachments.join("|")]);

  useEffect(() => {
    if (big === null) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "ArrowRight") setBig((i) => (i === null ? i : (i + 1) % names.length));
      if (e.key === "ArrowLeft") setBig((i) => (i === null ? i : (i - 1 + names.length) % names.length));
      if (e.key === "Escape") {
        e.stopPropagation();
        setBig(null);
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [big, names.length]);

  return (
    <div className="kv stack files" aria-label="Files">
      <span className="lbl">Files <small>{task.attachments.length}</small></span>
      <span className="file-list">
        {names.map((n, i) => (
          <button key={n} type="button" className="mini-thumb" onClick={() => setBig(i)} title={n}>
            {images[n] ? <img src={images[n]} alt="" /> : <span className="mini-img" />}
            <span className="name">{n}</span>
          </button>
        ))}
        {others.map((n) => <span key={n} className="other-file" title={n}>{n}</span>)}
        <button type="button" className="attach" onClick={onAttach}>+ Attach</button>
      </span>
      {big !== null && names[big] && (
        <div className="lightbox" onClick={() => setBig(null)} role="dialog" aria-label={names[big]}>
          {images[names[big]] && <img src={images[names[big]]} alt={names[big]} />}
          <p>{names[big]} · {big + 1} of {names.length} · ← → to move · Esc to close</p>
        </div>
      )}
    </div>
  );
}
