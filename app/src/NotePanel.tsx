import { useEffect, useState } from "react";
import { api } from "./api";
import { fileDraft, noteTitle } from "./Backlog";
import { ago, oneAtATime, stamp } from "./lib";
import { HelpButton } from "./HelpButton";
import Markdown from "./Markdown";
import type { Note } from "./types";

import { KINDS } from "./fields";
import ProjectName from "./ProjectName";
const PRIORITIES = ["high", "normal", "low"] as const;

/** Put `text` on the clipboard now, in the click that asked: WebKit refuses a write made after an
 *  await (the IPC for the brief), so the brief is loaded before the click and nothing is awaited
 *  before `writeText` is called. `what` names the brief in the refusal. */
export function copyNow(text: string | null, clipboard: Pick<Clipboard, "writeText"> | undefined, what = "The shaping brief"): Promise<void> {
  if (text === null) return Promise.reject(new Error(`${what} is still loading.`));
  if (!clipboard) return Promise.reject(new Error("The clipboard isn't available here."));
  return clipboard.writeText(text);
}

/** A backlog note in the docked pane: its text, and what can be done with it. File as task makes a
 *  task in Shaping (the note goes into its files), Drop sets it aside with the reason, and Copy
 *  shaping brief puts the brief a shaping session starts from on the clipboard. Each opens in
 *  place under its buttons; the core's refusal shows as is. */
export default function NotePanel({ note, now, onClose, onChanged, onFiled }: {
  note: Note;
  now: number;
  onClose: () => void;
  onChanged: () => void;
  onFiled: (queue: string, id: string) => void;
}) {
  const [form, setForm] = useState<"file" | "drop" | null>(null);
  const [draft, setDraft] = useState(() => fileDraft(note));
  const [why, setWhy] = useState("");
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [once] = useState(oneAtATime);
  // The shaping brief, loaded when the pane opens (and when the note changes), so Copy needs no IPC.
  const [brief, setBrief] = useState<{ text: string | null; error: string }>({ text: null, error: "" });
  useEffect(() => {
    let live = true;
    setBrief({ text: null, error: "" });
    api.shapeNote(note.queue, note.file).then(
      (text) => live && setBrief({ text, error: "" }),
      (e) => live && setBrief({ text: null, error: String(e) }),
    );
    return () => {
      live = false;
    };
  }, [note.queue, note.file, note.added]);

  /** One action at a time; its refusal shows under the buttons. */
  const run = (what: () => Promise<void>) =>
    void once(async () => {
      setBusy(true);
      setMsg(null);
      try {
        await what();
        return true;
      } catch (e) {
        setMsg({ ok: false, text: String(e) });
        return false;
      } finally {
        setBusy(false);
      }
    });
  const file = () => {
    if (!draft.title.trim()) return;
    run(async () => {
      const f = await api.fileNote(note.queue, note.file, { ...draft, tags: [], estTime: "", estTestTime: "", estTokens: "" }, stamp(new Date()));
      onFiled(note.queue, f.id);
    });
  };
  const drop = () => {
    if (!why.trim()) return;
    run(async () => {
      await api.dropNote(note.queue, note.file, why, stamp(new Date()));
      onChanged();
      onClose();
    });
  };
  // Not through `run`: its awaits would come before the write.
  const copy = () =>
    copyNow(brief.text, navigator.clipboard).then(
      () => setMsg({ ok: true, text: "Copied the shaping brief. Paste it into a session to shape the note." }),
      (e) => setMsg({ ok: false, text: String(e) }),
    );
  const shown = note.text.replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n/, "");

  return (
    <div className="panel note-panel" aria-label={`Backlog note ${note.file}`}>
      <header>
        <div>
          <div className="meta">
            <ProjectName name={note.queue} className="" /> · backlog note · added {ago(now, note.added)} ago
          </div>
          <div className="title-row">
            <h1>{noteTitle(note)}</h1>
            <HelpButton topic="note-pane" />
          </div>
          <div className="meta">
            <span title={note.file}>{note.file}</span>
          </div>
        </div>
        <button className="close" onClick={onClose} aria-label="Close">×</button>
      </header>

      <section className="controls">
        <div className="control-row">
          <button type="button" className={form === "file" ? "primary on" : "primary"} disabled={busy} onClick={() => setForm("file")}>File as task</button>
          <button type="button" className={form === "drop" ? "on" : undefined} disabled={busy} onClick={() => setForm("drop")}>Drop</button>
          <button type="button" disabled={busy || brief.text === null} title={brief.text === null && !brief.error ? "Loading the brief…" : undefined} onClick={() => void copy()}>Copy shaping brief</button>
        </div>
        {brief.error && <p className="err small">Couldn't load the shaping brief: {brief.error}</p>}
        {form === "file" && (
          <form
            className="note-file"
            onSubmit={(e) => {
              e.preventDefault();
              file();
            }}
          >
            <input autoFocus value={draft.title} onChange={(e) => setDraft((d) => ({ ...d, title: e.target.value }))} placeholder="The task's title, in one line" aria-label="The task's title" maxLength={200} />
            <div className="chip-row">
              <span className="meta">Type</span>
              {KINDS.map((k) => (
                <button key={k} type="button" className={draft.kind === k ? "chip on" : "chip"} aria-pressed={draft.kind === k} onClick={() => setDraft((d) => ({ ...d, kind: k }))}>{k}</button>
              ))}
              <span className="meta sep">Priority</span>
              {PRIORITIES.map((p) => (
                <button key={p} type="button" className={draft.priority === p ? `chip on pchip-${p}` : "chip"} aria-pressed={draft.priority === p} onClick={() => setDraft((d) => ({ ...d, priority: p }))}>{p}</button>
              ))}
            </div>
            <textarea
              value={draft.body}
              onChange={(e) => setDraft((d) => ({ ...d, body: e.target.value }))}
              placeholder="What the owner said, the details, and ## Acceptance criteria"
              aria-label="The task's body, in Markdown"
              rows={10}
              onKeyDown={(e) => {
                if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                  e.preventDefault();
                  file();
                }
              }}
            />
            <p className="meta small">It's filed in Shaping, with the note in its files. Pass it to Ready once it has acceptance criteria.</p>
            <div className="control-row">
              <button type="submit" className="primary" disabled={busy || !draft.title.trim()}>{busy ? "Filing…" : "Save (⌘↩)"}</button>
              <button type="button" onClick={() => setForm(null)}>Cancel</button>
            </div>
          </form>
        )}
        {form === "drop" && (
          <form
            className="control-row"
            onSubmit={(e) => {
              e.preventDefault();
              drop();
            }}
          >
            <input autoFocus value={why} onChange={(e) => setWhy(e.target.value)} placeholder="Why: a duplicate of 0012, not needed, done already…" aria-label="Why it's dropped" maxLength={500} />
            <button type="submit" disabled={busy || !why.trim()}>Drop</button>
            <button type="button" onClick={() => setForm(null)}>Cancel</button>
          </form>
        )}
        {msg && <p className={msg.ok ? "ok small" : "err small"}>{msg.text}</p>}
      </section>

      <section className="body">
        {shown.trim() ? <Markdown text={shown} /> : <p className="none">This note is empty.</p>}
        {note.truncated && <p className="meta small">Shown up to 20 KB. The whole note goes into the task's files when it's filed, and into the shaping brief.</p>}
      </section>
    </div>
  );
}
