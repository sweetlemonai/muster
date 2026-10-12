import { useEffect, useRef, useState } from "react";
import { api } from "./api";
import { readFiles, stamp } from "./lib";
import TagEditor from "./Tags";

/** New task, in the docked pane beside the board: title, type, priority, tags, description, files. */
export default function NewTask({ queues, initial, onClose, onCreated }: {
  queues: string[];
  initial: string;
  onClose: () => void;
  onCreated: (queue: string, id: string) => void;
}) {
  const [queue, setQueue] = useState(initial);
  const [title, setTitle] = useState("");
  const [kind, setKind] = useState("task");
  const [priority, setPriority] = useState("normal");
  const [body, setBody] = useState("");
  const [tags, setTags] = useState<string[]>([]);
  const [estTime, setEstTime] = useState("");
  const [estTokens, setEstTokens] = useState("");
  const [files, setFiles] = useState<[string, string][]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const titleRef = useRef<HTMLInputElement>(null);

  useEffect(() => titleRef.current?.focus(), []);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
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
      const c = await api.createTask(queue, { title, kind, priority, body, tags, estTime, estTokens }, files, stamp(new Date()));
      onCreated(queue, c.id);
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  return (
    <div className="pane-body">
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
            Type
            <select value={kind} onChange={(e) => setKind(e.target.value)}>
              <option value="task">Task</option>
              <option value="bug">Bug</option>
              <option value="idea">Idea</option>
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
          <label>
            Tokens
            <input value={estTokens} onChange={(e) => setEstTokens(e.target.value)} placeholder="500k" size={6} maxLength={10} />
          </label>
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
