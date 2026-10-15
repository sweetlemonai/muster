import { useEffect, useState } from "react";
import { api } from "./api";
import { fileDraft, noteTitle } from "./Backlog";
import { ago, oneAtATime, stamp } from "./lib";
import { HelpButton } from "./HelpButton";
import { tl, tn, useT } from "./i18n";
import Markdown from "./Markdown";
import type { Note } from "./types";

import { KINDS, kindLabel } from "./fields";
import ProjectName from "./ProjectName";
import { Button, Pill } from "./ui";
const PRIORITIES = ["high", "normal", "low"] as const;

/** Put `text` on the clipboard now, in the click that asked: WebKit refuses a write made after an
 *  await (the IPC for the brief), so the brief is loaded before the click and nothing is awaited
 *  before `writeText` is called. `what` names the brief in the refusal, in the language shown
 *  (the shaping brief's when not given). */
export function copyNow(text: string | null, clipboard: Pick<Clipboard, "writeText"> | undefined, what = tn("board", "note.shapingBrief")): Promise<void> {
  if (text === null) return Promise.reject(new Error(tn("board", "note.stillLoading", { what })));
  if (!clipboard) return Promise.reject(new Error(tn("board", "note.noClipboard")));
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
  const t = useT("board");
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
      () => setMsg({ ok: true, text: tn("board", "note.copied") }),
      (e) => setMsg({ ok: false, text: String(e) }),
    );
  const shown = note.text.replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n/, "");

  return (
    <div className="panel note-panel" aria-label={t("note.aria", { file: note.file })}>
      <header>
        <div>
          <div className="meta">
            <ProjectName name={note.queue} className="" /> · {now - note.added < 60 ? t("note.metaNow") : t("note.meta", { ago: ago(now, note.added) })}
          </div>
          <div className="title-row">
            {/* The note's own title keeps its own direction; "(empty note)" is the window's. */}
            <h1>{note.title.trim() ? <span dir="auto">{noteTitle(note)}</span> : noteTitle(note)}</h1>
            <HelpButton topic="note-pane" />
          </div>
          <div className="meta">
            <span title={note.file} dir="auto">{note.file}</span>
          </div>
        </div>
        <button className="close" onClick={onClose} aria-label={t("note.close")}>×</button>
      </header>

      <section className="controls">
        <div className="control-row">
          <Button type="button" variant="primary" className={form === "file" ? "on" : undefined} disabled={busy} onClick={() => setForm("file")}>{t("note.file")}</Button>
          <Button type="button" className={form === "drop" ? "on" : undefined} disabled={busy} onClick={() => setForm("drop")}>{t("note.drop")}</Button>
          <Button type="button" disabled={busy || brief.text === null} title={brief.text === null && !brief.error ? t("note.loadingBrief") : undefined} onClick={() => void copy()}>{t("note.copy")}</Button>
        </div>
        {brief.error && <p className="err small">{t("note.briefFailed", { error: brief.error })}</p>}
        {form === "file" && (
          <form
            className="note-file"
            onSubmit={(e) => {
              e.preventDefault();
              file();
            }}
          >
            <input autoFocus dir="auto" value={draft.title} onChange={(e) => setDraft((d) => ({ ...d, title: e.target.value }))} placeholder={t("note.titlePlaceholder")} aria-label={t("note.titleAria")} maxLength={200} />
            <div className="chip-row">
              <span className="meta">{t("note.type")}</span>
              {KINDS.map((k) => (
                <Pill key={k} type="button" on={draft.kind === k} aria-pressed={draft.kind === k} onClick={() => setDraft((d) => ({ ...d, kind: k }))}>{kindLabel(k)}</Pill>
              ))}
              <span className="meta sep">{t("note.priority")}</span>
              {PRIORITIES.map((p) => (
                <Pill key={p} type="button" on={draft.priority === p} className={draft.priority === p ? `pchip-${p}` : undefined} aria-pressed={draft.priority === p} onClick={() => setDraft((d) => ({ ...d, priority: p }))}>{tl(`priority.${p}`)}</Pill>
              ))}
            </div>
            <textarea
              dir="auto"
              value={draft.body}
              onChange={(e) => setDraft((d) => ({ ...d, body: e.target.value }))}
              placeholder={t("note.bodyPlaceholder")}
              aria-label={t("note.bodyAria")}
              rows={10}
              onKeyDown={(e) => {
                if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                  e.preventDefault();
                  file();
                }
              }}
            />
            <p className="meta small">{t("note.filedIn")}</p>
            <div className="control-row">
              <Button type="submit" variant="primary" disabled={busy || !draft.title.trim()}>{busy ? t("note.filing") : t("note.save")}</Button>
              <Button type="button" onClick={() => setForm(null)}>{t("common:cancel")}</Button>
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
            <input autoFocus dir="auto" value={why} onChange={(e) => setWhy(e.target.value)} placeholder={t("note.whyPlaceholder")} aria-label={t("note.whyAria")} maxLength={500} />
            <Button type="submit" disabled={busy || !why.trim()}>{t("note.drop")}</Button>
            <Button type="button" onClick={() => setForm(null)}>{t("common:cancel")}</Button>
          </form>
        )}
        {msg && <p className={msg.ok ? "ok small" : "err small"}>{msg.text}</p>}
      </section>

      <section className="body">
        {shown.trim() ? <Markdown text={shown} /> : <p className="none">{t("note.empty")}</p>}
        {note.truncated && <p className="meta small">{t("note.truncated")}</p>}
      </section>
    </div>
  );
}
