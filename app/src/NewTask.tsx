import { useEffect, useRef, useState } from "react";
import { api } from "./api";
import { KINDS } from "./fields";
import { paneEscape } from "./keys";
import { readFiles, stamp } from "./lib";
import TagEditor from "./Tags";
import { LEVELS } from "./levels";

/** Who does a new task (0022), as a chip pair: an agent (the default) or you. */
export const FOR_CHOICES = [
  { id: "human", label: "Human", hint: "Yours to do: on the Human board, and agents never take it" },
  { id: "ai", label: "AI", hint: "An agent's: on the AI board" },
] as const;

/** New task, in the docked pane beside the board: who does it, title, type, priority, tags,
 *  description, files. */
export default function NewTask({ queues, initial, onClose, onCreated }: {
  queues: string[];
  initial: string;
  onClose: () => void;
  onCreated: (queue: string, id: string) => void;
}) {
  const [queue, setQueue] = useState(initial);
  const [title, setTitle] = useState("");
  const [kind, setKind] = useState<string>("code");
  const [priority, setPriority] = useState("normal");
  const [body, setBody] = useState("");
  const [tags, setTags] = useState<string[]>([]);
  const [estTime, setEstTime] = useState("");
  const [estTestTime, setEstTestTime] = useState("");
  const [estTokens, setEstTokens] = useState("");
  const [level, setLevel] = useState<string>("task");
  // Who does it (0022): an agent, or you (a human task: only a task, never a goal or feature).
  const [forWho, setForWho] = useState<"ai" | "human">("ai");
  const [parent, setParent] = useState("");
  const [files, setFiles] = useState<[string, string][]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const titleRef = useRef<HTMLInputElement>(null);
  const paneRef = useRef<HTMLDivElement>(null);

  useEffect(() => titleRef.current?.focus(), []);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => paneEscape(e, paneRef.current, onClose);
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);

  const add = async (list: FileList | File[] | null) => {
    if (!list || list.length === 0) return;
    try {
      const read = await readFiles(list, stamp(new Date()));
      setFiles((f) => [...f, ...read]);
    } catch (e) {
      setError(String(e));
    }
  };
  const submit = async () => {
    if (!title.trim() || !queue) return;
    setBusy(true);
    setError("");
    try {
      // A goal has no parent; the core refuses one, so none is sent.
      const link = { level, parent: level === "goal" ? "" : parent.trim(), due: "" };
      const c = await api.createTask(queue, { title, kind, priority, body, tags, estTime, estTestTime, estTokens, ...link, forWho: level === "task" ? forWho : "ai" }, files, stamp(new Date()));
      onCreated(queue, c.id);
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  return (
    <div className="pane-body" ref={paneRef}>
      <form
        className="pane-form"
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
        onPaste={(e) => {
          if (e.clipboardData.files.length) {
            e.preventDefault();
            void add(e.clipboardData.files);
          }
        }}
        onDragOver={(e) => e.preventDefault()}
        onDrop={(e) => {
          e.preventDefault();
          void add(e.dataTransfer.files);
        }}
      >
        <header className="pane-head">
          <h1>New task</h1>
          <button type="button" className="close" onClick={onClose} aria-label="Close">×</button>
        </header>
        <div className="row">
          <label>
            Project
            <select value={queue} onChange={(e) => setQueue(e.target.value)}>
              {queues.map((q) => <option key={q}>{q}</option>)}
            </select>
          </label>
          <label>
            Priority
            <select value={priority} onChange={(e) => setPriority(e.target.value)}>
              <option value="high">High</option>
              <option value="normal">Normal</option>
              <option value="low">Low</option>
            </select>
          </label>
          <label>
            Agent time
            <input value={estTime} onChange={(e) => setEstTime(e.target.value)} placeholder="2h" size={6} maxLength={10} />
          </label>
          <label title="The QA agent plus your check">
            Test time
            <input value={estTestTime} onChange={(e) => setEstTestTime(e.target.value)} placeholder="30m" size={6} maxLength={10} />
          </label>
          <label>
            Tokens
            <input value={estTokens} onChange={(e) => setEstTokens(e.target.value)} placeholder="500k" size={6} maxLength={10} />
          </label>
        </div>
        <div className="chip-row" role="group" aria-label="Who does it">
          <span className="meta">For</span>
          {FOR_CHOICES.map((f) => (
            <button
              key={f.id}
              type="button"
              className={forWho === f.id ? "chip on" : "chip"}
              aria-pressed={forWho === f.id}
              title={f.hint}
              onClick={() => {
                setForWho(f.id);
                if (f.id === "human") setLevel("task");
              }}
            >
              {f.label}
            </button>
          ))}
        </div>
        <div className="row level-pick" role="group" aria-label="Level">
          <span className="meta">Level</span>
          {LEVELS.map((l) => (
            <button
              key={l.id}
              type="button"
              className={level === l.id ? "chip on" : "chip"}
              aria-pressed={level === l.id}
              disabled={forWho === "human" && l.id !== "task"}
              title={forWho === "human" && l.id !== "task" ? "Your own task is a task: a goal or feature is its tasks' sum" : undefined}
              onClick={() => setLevel(l.id)}
            >
              {l.label}
            </button>
          ))}
          <label className="parent-field" title={level === "goal" ? "A goal has no parent" : "The id of the feature or goal it sits under"}>
            Parent
            <input
              value={level === "goal" ? "" : parent}
              onChange={(e) => setParent(e.target.value.replace(/\D/g, "").slice(0, 4))}
              placeholder={level === "goal" ? "none" : "0030"}
              size={5}
              inputMode="numeric"
              disabled={level === "goal"}
            />
          </label>
        </div>
        <div className="chip-row" role="group" aria-label="Type">
          <span className="meta">Type</span>
          {KINDS.map((k) => (
            <button key={k} type="button" className={kind === k ? "chip on" : "chip"} aria-pressed={kind === k} onClick={() => setKind(k)}>{k}</button>
          ))}
        </div>
        <input ref={titleRef} className="title-input" value={title} onChange={(e) => setTitle(e.target.value)} placeholder="What's wrong or wanted, in one line" maxLength={200} />
        <TagEditor tags={tags} onChange={setTags} />
        <textarea
          value={body}
          onChange={(e) => setBody(e.target.value)}
          placeholder="Details in Markdown: steps to reproduce, where in the app, what done looks like. Paste or drop screenshots anywhere here."
          rows={10}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) void submit();
          }}
        />
        {files.length > 0 && (
          <div className="pending">
            {files.map(([name, url], i) => (
              <span key={name + i} className="pending-file">
                {url.startsWith("data:image/") ? <img src={url} alt="" /> : null}
                {name}
                <button type="button" onClick={() => setFiles((f) => f.filter((_, j) => j !== i))} aria-label={`Remove ${name}`}>×</button>
              </span>
            ))}
          </div>
        )}
        <div className="modal-bar">
          <label className="file-pick">
            Add files…
            <input type="file" multiple onChange={(e) => void add(e.target.files)} />
          </label>
          {error && <span className="err">{error}</span>}
          <span className="spacer" />
          <button type="button" onClick={onClose}>Cancel</button>
          <button type="submit" className="primary" disabled={!title.trim() || busy}>{busy ? "Creating…" : "Create (⌘↩)"}</button>
        </div>
      </form>
    </div>
  );
}
