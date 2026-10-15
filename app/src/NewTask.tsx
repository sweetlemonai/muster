import { useContext, useEffect, useRef, useState } from "react";
import { api } from "./api";
import { KINDS, kindLabel } from "./fields";
import { paneEscape } from "./keys";
import { readFiles, stamp } from "./lib";
import TagEditor from "./Tags";
import { LEVELS } from "./levels";
import { HelpButton } from "./HelpButton";
import { latinDigits, tn, useT } from "./i18n";
import { pressed } from "./shortcuts";
import { parseKey } from "./taskKey";
import { labelOf, Labels } from "./labels";
import { Button, Pill } from "./ui";

/** Who does a new task (0022), as a chip pair: an agent (the default) or you; named in the
 *  language shown. */
const forChoice = <I extends "human" | "ai">(id: I) => ({
  id,
  get label() { return tn("board", `newTask.${id}`); },
  get hint() { return tn("board", `newTask.${id}Hint`); },
});
export const FOR_CHOICES = [forChoice("human"), forChoice("ai")] as const;

/** The Parent field (0033): its key (`mu-30`, `MU-30`) or number (`30`, `0030`) as the id the core
 *  takes; "" for none. Another project's key, or something that isn't a task, says why, in the
 *  language shown. */
export function parentRequest(typed: string, prefix: string | undefined): { ok: true; id: string } | { ok: false; why: string } {
  const t = typed.trim();
  if (!t) return { ok: true, id: "" };
  // The key and the number as they're typed: Latin, whatever the language.
  const example = prefix ? tn("board", "newTask.example", { key: `${prefix}-30`, number: "30" }) : "30";
  // Typed in Persian or Arabic-Indic digits (the fa keyboard's), it's the same key (mu-217).
  const k = parseKey(latinDigits(t));
  if (!k) return { ok: false, why: tn("board", "newTask.parentNotTask", { example }) };
  if (k.prefix !== null && prefix && k.prefix !== prefix) {
    return { ok: false, why: tn("board", "newTask.parentElsewhere", { typed: t, example }) };
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
  const t = useT("board");
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
          <h1>{t("newTask.title")} <HelpButton topic="new-task" /></h1>
          <button type="button" className="close" onClick={onClose} aria-label={t("newTask.close")}>×</button>
        </header>
        <div className="row">
          <label>
            {t("newTask.project")}
            <select value={queue} onChange={(e) => setQueue(e.target.value)}>
              {queues.map((q) => <option key={q} value={q}>{labelOf(labels, q)}</option>)}
            </select>
          </label>
          <label>
            {t("newTask.priority")}
            <select value={priority} onChange={(e) => setPriority(e.target.value)}>
              <option value="high">{t("newTask.priorityValue.high")}</option>
              <option value="normal">{t("newTask.priorityValue.normal")}</option>
              <option value="low">{t("newTask.priorityValue.low")}</option>
            </select>
          </label>
          <label title={t("common:estimateField.agentTime")}>
            {t("newTask.agentTime")}
            {/* i18n-ignore: an estimate as it's typed */}
            <input value={estTime} onChange={(e) => setEstTime(e.target.value)} placeholder="2h" size={6} maxLength={10} className="ltr" />
          </label>
          <label title={t("common:estimateField.testTime")}>
            {t("newTask.testTime")}
            {/* i18n-ignore: an estimate as it's typed */}
            <input value={estTestTime} onChange={(e) => setEstTestTime(e.target.value)} placeholder="30m" size={6} maxLength={10} className="ltr" />
          </label>
          <label>
            {t("newTask.tokens")}
            {/* i18n-ignore: an estimate as it's typed */}
            <input value={estTokens} onChange={(e) => setEstTokens(e.target.value)} placeholder="500k" size={6} maxLength={10} className="ltr" />
          </label>
        </div>
        <div className="chip-row" role="group" aria-label={t("newTask.whoAria")}>
          <span className="meta">{t("newTask.for")}</span>
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
        <div className="row level-pick" role="group" aria-label={t("newTask.level")}>
          <span className="meta">{t("newTask.level")}</span>
          {LEVELS.map((l) => (
            <Pill
              key={l.id}
              type="button"
              on={level === l.id}
              aria-pressed={level === l.id}
              disabled={forWho === "human" && l.id !== "task"}
              title={forWho === "human" && l.id !== "task" ? t("newTask.humanIsTask") : undefined}
              onClick={() => setLevel(l.id)}
            >
              {l.label}
            </Pill>
          ))}
          <label className="parent-field" title={level === "goal" ? t("newTask.goalNoParent") : t("newTask.parentHint")}>
            {t("newTask.parent")}
            <input
              value={level === "goal" ? "" : parent}
              onChange={(e) => setParent(e.target.value.slice(0, 12))}
              placeholder={level === "goal" ? t("newTask.parentNone") : prefixes[queue] ? `${prefixes[queue]}-30` : "30"}
              size={7}
              disabled={level === "goal"}
            />
          </label>
          {!parentAsked.ok && <span className="err small" role="alert">{parentAsked.why}</span>}
        </div>
        <div className="chip-row" role="group" aria-label={t("newTask.type")}>
          <span className="meta">{t("newTask.type")}</span>
          {KINDS.map((k) => (
            <Pill key={k} type="button" on={kind === k} aria-pressed={kind === k} onClick={() => setKind(k)}>{kindLabel(k)}</Pill>
          ))}
        </div>
        <input ref={titleRef} className="title-input" dir="auto" value={title} onChange={(e) => setTitle(e.target.value)} placeholder={t("newTask.titlePlaceholder")} maxLength={200} />
        <TagEditor tags={tags} onChange={setTags} />
        <textarea
          dir="auto"
          value={body}
          onChange={(e) => setBody(e.target.value)}
          placeholder={t("newTask.bodyPlaceholder")}
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
                <span dir="auto">{name}</span>
                <button type="button" className="remove" onClick={() => setFiles((f) => f.filter((_, j) => j !== i))} aria-label={t("newTask.removeFile", { name })}>×</button>
              </span>
            ))}
          </div>
        )}
        <div className="modal-bar">
          <label className="file-pick">
            {t("newTask.addFiles")}
            <input type="file" multiple onChange={(e) => void add(e.target.files)} />
          </label>
          {error && <span className="err">{error}</span>}
          <span className="spacer" />
          <Button type="button" onClick={onClose}>{t("common:cancel")}</Button>
          <Button type="submit" variant="primary" disabled={!title.trim() || busy}>{busy ? t("newTask.creating") : t("newTask.create")}</Button>
        </div>
      </form>
    </div>
  );
}
