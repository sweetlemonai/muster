import { useEffect, useState } from "react";
import { api } from "./api";
import { ago, readFiles, stamp } from "./lib";
import Markdown from "./Markdown";
import TagEditor from "./Tags";
import type { Question, Task } from "./types";

const IMAGE = /\.(png|jpe?g|gif|webp)$/i;

export default function TaskPanel({ task, now, onClose, onChanged }: { task: Task; now: number; onClose: () => void; onChanged: () => void }) {
  const [body, setBody] = useState<string | null>(null);
  const [raw, setRaw] = useState(false);
  const [error, setError] = useState("");

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
  }, [task.queue, task.id, task.modified]);

  const shown = body ? body.replace(/^---\n[\s\S]*?\n---\n/, "") : "";
  const open = task.questions.filter((q) => !q.ticked && q.n !== null);

  return (
    <div className="panel" aria-label={`Task ${task.id}`}>
      <header>
        <div>
          <div className="meta">
            {task.queue} · {task.id} · {task.kind} · {task.priority} · <b>{task.status}</b> · changed {ago(now, task.modified)} ago
          </div>
          <h1>{task.title}</h1>
          <div className="meta">
            {task.claimedBy && <>on it: {task.claimedBy} · </>}
            {task.waitingOn && <>waiting on: {task.waitingOn} · </>}
            {task.commit && <>commit {task.commit} · </>}
            <span title={task.path}>{task.path}</span>
          </div>
        </div>
        <button className="close" onClick={onClose} aria-label="Close">×</button>
      </header>

      <Controls task={task} onChanged={onChanged} />

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
        <div className="body-bar">
          <button className={raw ? "tab" : "tab on"} onClick={() => setRaw(false)}>Rendered</button>
          <button className={raw ? "tab on" : "tab"} onClick={() => setRaw(true)}>Raw</button>
        </div>
        {error && <p className="err">{error}</p>}
        {body === null && !error && <p className="none">Loading…</p>}
        {body !== null && (raw ? <pre className="raw">{body}</pre> : <Markdown text={shown} />)}
      </section>
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

const STATUSES = ["inbox", "todo", "blocked", "done"];

/** The owner's changes: status, priority, type, title, a note, files. All through the core's rules. */
function Controls({ task, onChanged }: { task: Task; onChanged: () => void }) {
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [blocking, setBlocking] = useState(false);
  const [waitingOn, setWaitingOn] = useState("");
  const [note, setNote] = useState("");
  const [title, setTitle] = useState<string | null>(null);
  const [over, setOver] = useState(false);

  const run = async (what: () => Promise<unknown>, done: string) => {
    try {
      await what();
      setMsg({ ok: true, text: done });
      onChanged();
    } catch (e) {
      setMsg({ ok: false, text: String(e) });
    }
  };
  const move = (status: string) => {
    if (status === "blocked") {
      setBlocking(true);
      return;
    }
    void run(() => api.moveTask(task.queue, task.id, status, "", stamp(new Date())), `Moved to ${status}.`);
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
          Status
          {task.status === "doing" ? (
            <select value="doing" onChange={(e) => move(e.target.value)} title="Claimed by an agent. Moving it releases the claim.">
              <option value="doing" disabled>doing (claimed)</option>
              {STATUSES.map((s) => <option key={s} value={s}>{s}</option>)}
            </select>
          ) : (
            <select value={task.status} onChange={(e) => move(e.target.value)}>
              {STATUSES.map((s) => <option key={s} value={s}>{s}</option>)}
            </select>
          )}
        </label>
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
      {blocking && (
        <form
          className="control-row"
          onSubmit={(e) => {
            e.preventDefault();
            setBlocking(false);
            void run(() => api.moveTask(task.queue, task.id, "blocked", waitingOn, stamp(new Date())), "Blocked.");
          }}
        >
          <input autoFocus value={waitingOn} onChange={(e) => setWaitingOn(e.target.value)} placeholder="Waiting on: a task id (0012), owner, or capture" maxLength={200} />
          <button type="submit" disabled={!waitingOn.trim()}>Block</button>
          <button type="button" onClick={() => setBlocking(false)}>Cancel</button>
        </form>
      )}
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
function Estimate({ task, run }: { task: Task; run: (what: () => Promise<unknown>, done: string) => Promise<void> }) {
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
