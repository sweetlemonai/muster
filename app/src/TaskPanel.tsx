import { useEffect, useState } from "react";
import { api } from "./api";
import EditBody from "./EditBody";
import { ago, claimant, earlierStages, heldByOther, isStage, LEASE_SECS, leaseRunOut, nextStage, oneAtATime, passLabel, readFiles, stamp, STAGES, stageLabel, stageOf } from "./lib";
import Markdown from "./Markdown";
import OwnerChecklist, { ownerOpen } from "./OwnerChecklist";
import TagEditor from "./Tags";
import type { Question, Task } from "./types";

const IMAGE = /\.(png|jpe?g|gif|webp)$/i;

/** `onDirty` and `nudge` pass between the window and the write-up editor (see EditBody). */
export default function TaskPanel({ task, now, onClose, onChanged, onDirty, nudge }: { task: Task; now: number; onClose: () => void; onChanged: () => void; onDirty?: (dirty: boolean) => void; nudge?: number }) {
  const [body, setBody] = useState<string | null>(null);
  const [raw, setRaw] = useState(false);
  const [error, setError] = useState("");
  // The write-up Edit opened, while the editor is open; what the last edit came to.
  const [editing, setEditing] = useState<string | null>(null);
  const [editMsg, setEditMsg] = useState<{ ok: boolean; text: string } | null>(null);
  // Bumped after a save, so the text reloads even within the second the file last changed.
  const [saves, setSaves] = useState(0);

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

  const shown = body ? body.replace(/^---\n[\s\S]*?\n---\n/, "") : "";
  const open = task.questions.filter((q) => !q.ticked && q.n !== null);

  return (
    <div className="panel" aria-label={`Task ${task.id}`}>
      <header>
        <div>
          <div className="meta">
            {task.queue} · {task.id} · {task.kind} · {task.priority} · changed {ago(now, task.modified)} ago
          </div>
          <h1>{task.title}</h1>
          <StageLine task={task} />
          <div className="meta">
            {task.claimedBy && <>on it: {task.claimedBy} · </>}
            {task.waitingOn && <>waiting on: {task.waitingOn} · </>}
            {task.commit && <>commit {task.commit} · </>}
            <span title={task.path}>{task.path}</span>
          </div>
        </div>
        <button className="close" onClick={onClose} aria-label="Close">×</button>
      </header>

      <Controls task={task} nowSecs={now} onChanged={onChanged} />

      {open.length > 0 && (
        <section className="answers">
          <h2>Open questions</h2>
          {open.map((q) => (
            <AnswerBox key={q.n} task={task} q={q} onChanged={onChanged} />
          ))}
        </section>
      )}

      {task.attachments.length > 0 && <Attachments task={task} />}

      <section className="body">
        {editing !== null ? (
          <EditBody
            task={task}
            opened={editing}
            onDirty={onDirty}
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
    </div>
  );
}

/** The stage, who works it, and the blocked flag. */
function StageLine({ task }: { task: Task }) {
  const known = isStage(task.stage);
  const role = known ? STAGES.find((s) => s.id === task.stage)?.role : "";
  return (
    <div className="stage-line">
      <span className={`status-pill s-${stageOf(task)}`}>{known ? stageLabel(task.stage) : `${task.stage || "no stage"} (unknown)`}</span>
      {role && <span className="meta">{role}</span>}
      {task.blocked && <span className="status-pill blocked">blocked{task.waitingOn ? `: ${task.waitingOn}` : ""}</span>}
    </div>
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

function Attachments({ task }: { task: Task }) {
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
    <section className="attachments">
      <h2>Files <small>{task.attachments.length}</small></h2>
      <div className="thumbs">
        {names.map((n, i) => (
          <button key={n} className="thumb" onClick={() => setBig(i)} title={n}>
            {images[n] ? <img src={images[n]} alt={n} /> : <span className="none">…</span>}
            <span className="name">{n}</span>
          </button>
        ))}
      </div>
      {others.length > 0 && <p className="meta">Other files: {others.join(", ")}</p>}
      {big !== null && names[big] && (
        <div className="lightbox" onClick={() => setBig(null)} role="dialog" aria-label={names[big]}>
          {images[names[big]] && <img src={images[names[big]]} alt={names[big]} />}
          <p>{names[big]} · {big + 1} of {names.length} · ← → to move · Esc to close</p>
        </div>
      )}
    </section>
  );
}

/** The owner's changes: stage, priority, type, title, a note, files. All through the core's rules.
 *  `nowSecs` is the state's clock, for the claim's lease. */
function Controls({ task, nowSecs, onChanged }: { task: Task; nowSecs: number; onChanged: () => void }) {
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  // One stage form open at a time: the gate line (Building → Testing), send back, or block. A
  // form belongs to the stage it was opened in: once the task moves, here or elsewhere, it's gone.
  const [opened, setForm] = useState<{ kind: "gate" | "back" | "block"; stage: string } | null>(null);
  const form = opened?.stage === task.stage ? opened.kind : null;
  const open = (kind: "gate" | "back" | "block") => setForm({ kind, stage: task.stage });
  // Stage actions run one at a time, and their controls are off while one is in flight.
  const [once] = useState(oneAtATime);
  const [busy, setBusy] = useState(false);
  const [gate, setGate] = useState("");
  const [why, setWhy] = useState("");
  const [backTo, setBackTo] = useState("");
  const [waitingOn, setWaitingOn] = useState("");
  const [note, setNote] = useState("");
  const [title, setTitle] = useState<string | null>(null);
  const [over, setOver] = useState(false);

  /** Runs one change; the core's refusal is shown as is. True when it went through. */
  const run = async (what: () => Promise<unknown>, done: string) => {
    try {
      await what();
      setMsg({ ok: true, text: done });
      onChanged();
      return true;
    } catch (e) {
      setMsg({ ok: false, text: String(e) });
      return false;
    }
  };
  const stageRun = (what: () => Promise<unknown>, done: string) =>
    once(async () => {
      setBusy(true);
      try {
        return await run(what, done);
      } finally {
        setBusy(false);
      }
    });
  // A stage form closes (and forgets its text) only once the core took the change.
  const submit = (what: () => Promise<unknown>, done: string) =>
    void stageRun(what, done).then((ok) => {
      if (!ok) return;
      setForm(null);
      setGate("");
      setWhy("");
      setWaitingOn("");
    });
  const now = () => stamp(new Date());
  const known = isStage(task.stage);
  const pass = passLabel(task.stage);
  const back = earlierStages(task.stage);
  // Where Send back goes: the choice, while it's still an earlier stage; else one back.
  const backTarget = back.some((s) => s.id === backTo) ? backTo : back[0]?.id ?? "";
  const holder = heldByOther(task);
  // A claim nobody has heard from past the lease: the owner may give it back to Ready.
  const stale = leaseRunOut(task, nowSecs);
  const showPass = pass !== null && !holder;
  // Every pass says the stage it was clicked in, so the core refuses it if the task moved on.
  const passFrom = (gateLine: string, done: string) => submit(() => api.passTask(task.queue, task.id, task.stage, gateLine, "", now()), done);
  const passNow = () => {
    if (task.stage === "building") {
      open("gate");
      return;
    }
    passFrom("", task.stage === "your-check" ? "Accepted: it's done." : `Passed to ${nextStage(task.stage)?.label ?? "the next stage"}.`);
  };
  const files = async (list: FileList | File[] | null) => {
    if (!list || !list.length) return;
    const read = await readFiles(list, stamp(new Date()));
    void run(() => api.attachData(task.queue, task.id, read, stamp(new Date())), `Attached ${read.length} file${read.length === 1 ? "" : "s"}.`);
  };

  return (
    <section
      className={`controls${over ? " drop" : ""}`}
      onDragOver={(e) => {
        e.preventDefault();
        setOver(true);
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(e) => {
        e.preventDefault();
        setOver(false);
        void files(e.dataTransfer.files);
      }}
      onPaste={(e) => {
        if (e.clipboardData.files.length) {
          e.preventDefault();
          void files(e.clipboardData.files);
        }
      }}
    >
      <div className="control-row">
        <label>
          Stage
          <select
            value={known ? task.stage : ""}
            disabled={busy}
            title="Your override: put it in any stage. Leaving Building releases the claim."
            onChange={(e) => {
              const to = e.target.value;
              void stageRun(() => api.setStage(task.queue, task.id, to, now()), `Put in ${stageLabel(to)}.`);
            }}
          >
            {!known && <option value="" disabled>{task.stage || "no stage"} (unknown)</option>}
            {STAGES.map((s) => <option key={s.id} value={s.id}>{s.label}</option>)}
          </select>
        </label>
        {showPass && (
          <button
            type="button"
            className="primary"
            disabled={task.blocked || busy || (task.stage === "your-check" && ownerOpen(task) > 0)}
            title={task.stage === "your-check" && ownerOpen(task) > 0 ? "Tick yours below first" : undefined}
            onClick={passNow}
          >
            {pass}
          </button>
        )}
        {stale && (
          <button
            type="button"
            className={showPass ? undefined : "primary"}
            disabled={busy}
            title="Back to Ready, with the claim released and logged as reclaimed"
            onClick={() => void stageRun(() => api.reclaimTask(task.queue, task.id, now()), "Released: back in Ready.")}
          >
            Release
          </button>
        )}
        {back.length > 0 && (
          <button type="button" disabled={busy} onClick={() => { setBackTo(""); open("back"); }}>Send back</button>
        )}
        {task.blocked ? (
          <button type="button" disabled={busy} onClick={() => void stageRun(() => api.unblockTask(task.queue, task.id, now()), "Unblocked.")}>Unblock</button>
        ) : (
          <button type="button" disabled={busy} onClick={() => open("block")}>Block</button>
        )}
      </div>
      <OwnerChecklist
        task={task}
        busy={busy}
        onTick={(which, undo) => void stageRun(() => api.tickCriterion(task.queue, task.id, which, undo, now()), `${undo ? "Unticked" : "Ticked"} ${which}.`)}
      />
      {holder && (
        <p className="meta small">
          {holder} holds it in {stageLabel(task.stage)}, so only they can pass it on. To move it yourself, pick a stage above.
        </p>
      )}
      {stale && (
        <p className="meta small">
          {claimant(task.claimedBy)} hasn't been heard from for {LEASE_SECS / 3600} h. Release puts it back in Ready, logged as reclaimed.
        </p>
      )}
      {pass && !holder && task.blocked && <p className="meta small">Blocked on {task.waitingOn || "something it doesn't name"}. Unblock it to pass it on.</p>}
      {form === "gate" && task.stage === "building" && (
        <form
          className="control-row"
          onSubmit={(e) => {
            e.preventDefault();
            if (gate.trim()) passFrom(gate, "Passed to Testing.");
          }}
        >
          <input autoFocus value={gate} onChange={(e) => setGate(e.target.value)} placeholder="The gate's result line (gate full: green …)" aria-label="The gate's result line" maxLength={500} />
          <button type="submit" disabled={busy || !gate.trim()}>Pass to Testing</button>
          <button type="button" onClick={() => setForm(null)}>Cancel</button>
        </form>
      )}
      {form === "back" && backTarget && (
        <form
          className="control-row"
          onSubmit={(e) => {
            e.preventDefault();
            if (why.trim()) submit(() => api.returnTask(task.queue, task.id, why, backTarget, now()), `Sent back to ${stageLabel(backTarget)}.`);
          }}
        >
          <label>
            Back to
            <select value={backTarget} onChange={(e) => setBackTo(e.target.value)}>
              {back.map((s) => <option key={s.id} value={s.id}>{s.label}</option>)}
            </select>
          </label>
          <input autoFocus value={why} onChange={(e) => setWhy(e.target.value)} placeholder="Why: what's wrong or missing" aria-label="Why it goes back" maxLength={500} />
          <button type="submit" disabled={busy || !why.trim()}>Send back</button>
          <button type="button" onClick={() => setForm(null)}>Cancel</button>
        </form>
      )}
      {form === "block" && !task.blocked && (
        <form
          className="control-row"
          onSubmit={(e) => {
            e.preventDefault();
            if (waitingOn.trim()) submit(() => api.blockTask(task.queue, task.id, waitingOn, now()), "Blocked. It keeps its stage.");
          }}
        >
          <input autoFocus value={waitingOn} onChange={(e) => setWaitingOn(e.target.value)} placeholder="Waiting on: a task id (0012), owner, or capture, then a few words" aria-label="What it's waiting on" maxLength={200} />
          <button type="submit" disabled={busy || !waitingOn.trim()}>Block</button>
          <button type="button" onClick={() => setForm(null)}>Cancel</button>
        </form>
      )}
      <div className="control-row">
        <label>
          Priority
          <select value={task.priority || "normal"} onChange={(e) => void run(() => api.setField(task.queue, task.id, "priority", e.target.value, stamp(new Date())), "Priority set.")}>
            <option value="high">high</option>
            <option value="normal">normal</option>
            <option value="low">low</option>
          </select>
        </label>
        <label>
          Type
          <select value={task.kind || "task"} onChange={(e) => void run(() => api.setField(task.queue, task.id, "type", e.target.value, stamp(new Date())), "Type set.")}>
            <option value="task">task</option>
            <option value="bug">bug</option>
            <option value="idea">idea</option>
          </select>
        </label>
        <Estimate task={task} run={run} />
        <button type="button" onClick={() => setTitle(task.title)}>Rename</button>
        <button type="button" onClick={() => void run(() => api.attachPick(task.queue, task.id, stamp(new Date())), "Files attached.")}>Attach…</button>
      </div>
      <TagEditor tags={task.tags} onChange={(t) => void run(() => api.setField(task.queue, task.id, "tags", t.join(","), stamp(new Date())), "Tags saved.")} />
      {title !== null && (
        <form
          className="control-row"
          onSubmit={(e) => {
            e.preventDefault();
            const t = title;
            setTitle(null);
            void run(() => api.setField(task.queue, task.id, "title", t, stamp(new Date())), "Renamed.");
          }}
        >
          <input autoFocus value={title} onChange={(e) => setTitle(e.target.value)} maxLength={200} />
          <button type="submit" disabled={!title.trim()}>Save</button>
          <button type="button" onClick={() => setTitle(null)}>Cancel</button>
        </form>
      )}
      <form
        className="control-row"
        onSubmit={(e) => {
          e.preventDefault();
          const n = note;
          setNote("");
          void run(() => api.addNote(task.queue, task.id, n, stamp(new Date())), "Note added to the log.");
        }}
      >
        <input value={note} onChange={(e) => setNote(e.target.value)} placeholder="Add a note to the log… (or paste / drop files here)" maxLength={2000} />
        <button type="submit" disabled={!note.trim()}>Add</button>
      </form>
      {msg && <p className={msg.ok ? "ok small" : "err small"}>{msg.text}</p>}
    </section>
  );
}

/** Estimated agent time and tokens, saved when a field loses focus or on Enter. */
function Estimate({ task, run }: { task: Task; run: (what: () => Promise<unknown>, done: string) => Promise<boolean> }) {
  const [time, setTime] = useState(task.estTime);
  const [tokens, setTokens] = useState(task.estTokens);
  useEffect(() => setTime(task.estTime), [task.estTime]);
  useEffect(() => setTokens(task.estTokens), [task.estTokens]);
  const save = (field: "time" | "tokens", value: string, was: string) => {
    if (value.trim() === was) return;
    void run(() => api.setField(task.queue, task.id, field, value, stamp(new Date())), field === "time" ? "Time estimate saved." : "Token estimate saved.");
  };
  const keys = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") (e.target as HTMLInputElement).blur();
  };
  return (
    <>
      <label>
        Agent time
        <input value={time} onChange={(e) => setTime(e.target.value)} onBlur={() => save("time", time, task.estTime)} onKeyDown={keys} placeholder="2h" size={5} maxLength={10} />
      </label>
      <label>
        Tokens
        <input value={tokens} onChange={(e) => setTokens(e.target.value)} onBlur={() => save("tokens", tokens, task.estTokens)} onKeyDown={keys} placeholder="500k" size={5} maxLength={10} />
      </label>
    </>
  );
}
