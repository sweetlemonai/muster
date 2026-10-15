import { useContext, useEffect, useRef, useState } from "react";
import { api } from "./api";
import { KINDS } from "./fields";
import { paneEscape } from "./keys";
import { readFiles, stamp } from "./lib";
import TagEditor from "./Tags";
import { LEVELS } from "./levels";
import { HelpButton } from "./HelpButton";
import { useT } from "./i18n";
import { pressed } from "./shortcuts";
import { parseKey } from "./taskKey";
import { labelOf, Labels } from "./labels";
import { Button, Pill } from "./ui";

/** Who does a new task (0022), as a chip pair: an agent (the default) or you. */
export const FOR_CHOICES = [
  { id: "human", label: "Human", hint: "Yours to do: on the board hatched, with the You ribbon, and agents never take it" },
  { id: "ai", label: "AI", hint: "An agent's: on the board as usual" },
] as const;

/** The Parent field (0033): its key (`mu-30`, `MU-30`) or number (`30`, `0030`) as the id the core
 *  takes; "" for none. Another project's key, or something that isn't a task, says why. */
export function parentRequest(typed: string, prefix: string | undefined): { ok: true; id: string } | { ok: false; why: string } {
  const t = typed.trim();
  if (!t) return { ok: true, id: "" };
  const example = prefix ? `${prefix}-30 or 30` : "30";
  const k = parseKey(t);
  if (!k) return { ok: false, why: `A parent is a feature or goal of this project: ${example}` };
  if (k.prefix !== null && prefix && k.prefix !== prefix) {
    return { ok: false, why: `${t} is another project's task: a parent is in this one (${example})` };
  }
  return { ok: true, id: k.id };
}

/** New task, in the docked pane beside the board: who does it, title, type, priority, tags,
 *  description, files. */
export default function NewTask({ queues, initial, prefixes = {}, onClose, onCreated }: {
  queues: string[];
  initial: string;
  /** Each queue's prefix (0033), for the Parent field's keys. */
  prefixes?: Record<string, string>;
  onClose: () => void;
  onCreated: (queue: string, id: string) => void;
}) {
  const t = useT("common");
  const [queue, setQueue] = useState(initial);
  const labels = useContext(Labels);
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
  // A goal has no parent; the core refuses one, so none is sent.
  const parentAsked = parentRequest(level === "goal" ? "" : parent, prefixes[queue]);
  const submit = async () => {
    if (!title.trim() || !queue) return;
    if (!parentAsked.ok) {
      setError(parentAsked.why);
      return;
    }
    setBusy(true);
    setError("");
    try {
      const link = { level, parent: parentAsked.id, due: "" };
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
          <h1>New task <HelpButton topic="new-task" /></h1>
          <button type="button" className="close" onClick={onClose} aria-label="Close">×</button>
        </header>
        <div className="row">
          <label>
            Project
            <select value={queue} onChange={(e) => setQueue(e.target.value)}>
              {queues.map((q) => <option key={q} value={q}>{labelOf(labels, q)}</option>)}
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
          <label title={t("estimateField.agentTime")}>
            Agent time
            <input value={estTime} onChange={(e) => setEstTime(e.target.value)} placeholder="2h" size={6} maxLength={10} />
          </label>
          <label title={t("estimateField.testTime")}>
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
            <Pill
              key={f.id}
              type="button"
              on={forWho === f.id}
              aria-pressed={forWho === f.id}
              title={f.hint}
              onClick={() => {
                setForWho(f.id);
                if (f.id === "human") setLevel("task");
              }}
            >
              {f.label}
            </Pill>
          ))}
        </div>
        <div className="row level-pick" role="group" aria-label="Level">
          <span className="meta">Level</span>
          {LEVELS.map((l) => (
            <Pill
              key={l.id}
              type="button"
              on={level === l.id}
              aria-pressed={level === l.id}
              disabled={forWho === "human" && l.id !== "task"}
              title={forWho === "human" && l.id !== "task" ? "Your own task is a task: a goal or feature is its tasks' sum" : undefined}
              onClick={() => setLevel(l.id)}
            >
              {l.label}
            </Pill>
          ))}
          <label className="parent-field" title={level === "goal" ? "A goal has no parent" : "The feature or goal it sits under, by its key or number"}>
            Parent
            <input
              value={level === "goal" ? "" : parent}
              onChange={(e) => setParent(e.target.value.slice(0, 12))}
              placeholder={level === "goal" ? "none" : prefixes[queue] ? `${prefixes[queue]}-30` : "30"}
              size={7}
              disabled={level === "goal"}
            />
          </label>
          {!parentAsked.ok && <span className="err small" role="alert">{parentAsked.why}</span>}
        </div>
        <div className="chip-row" role="group" aria-label="Type">
          <span className="meta">Type</span>
          {KINDS.map((k) => (
            <Pill key={k} type="button" on={kind === k} aria-pressed={kind === k} onClick={() => setKind(k)}>{k}</Pill>
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
            if (pressed(e, "send")) void submit();
          }}
        />
        {files.length > 0 && (
          <div className="pending">
            {files.map(([name, url], i) => (
              <span key={name + i} className="pending-file">
                {url.startsWith("data:image/") ? <img src={url} alt="" /> : null}
                {name}
                <button type="button" className="remove" onClick={() => setFiles((f) => f.filter((_, j) => j !== i))} aria-label={`Remove ${name}`}>×</button>
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
          <Button type="button" onClick={onClose}>Cancel</Button>
          <Button type="submit" variant="primary" disabled={!title.trim() || busy}>{busy ? "Creating…" : "Create (⌘↩)"}</Button>
        </div>
      </form>
    </div>
  );
}
