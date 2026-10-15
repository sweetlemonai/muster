import { useEffect, useState } from "react";
import { api } from "./api";
import { useT } from "./i18n";
import { oneAtATime, stamp } from "./lib";
import Markdown from "./Markdown";
import { copyNow } from "./NotePanel";
import type { Task } from "./types";
import { Button } from "./ui";
import { conflictTitles, visionSection, visionStatus } from "./Vision";

/** The task pane's Vision section (0029): where the check stands, Resolve for each open conflict
 *  (how it was settled, in one line, in place), Copy vision brief, and the section the core wrote,
 *  as sanitized Markdown. Always shown, so a task that was never checked says so. `body` is the
 *  task's text, null while it loads; the Resolve buttons come from the task, so they don't wait. */
export default function VisionPanel({ task, body, onChanged }: { task: Task; body: string | null; onChanged: () => void }) {
  const t = useT("pane");
  const checked = (task.visionOpen ?? null) !== null;
  const open = task.visionIds ?? [];
  const titles = body === null ? {} : conflictTitles(body);
  const section = checked && body !== null ? visionSection(body) : null;
  const [which, setWhich] = useState<string | null>(null);
  const [how, setHow] = useState("");
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [once] = useState(oneAtATime);
  // The brief, loaded when the pane opens and when the task changes, so Copy needs no IPC.
  const [brief, setBrief] = useState<{ text: string | null; error: string }>({ text: null, error: "" });
  useEffect(() => {
    let live = true;
    setBrief({ text: null, error: "" });
    api.visionBrief(task.queue, task.id).then(
      (text) => live && setBrief({ text, error: "" }),
      (e) => live && setBrief({ text: null, error: String(e) }),
    );
    return () => {
      live = false;
    };
  }, [task.queue, task.id, task.modified]);

  // The form belongs to a conflict that's still open: once it's resolved, here or elsewhere, it's gone.
  const resolving = which !== null && open.includes(which) ? which : null;
  const resolve = () => {
    const conflict = resolving;
    const text = how;
    if (!conflict || !text.trim()) return;
    void once(async () => {
      setBusy(true);
      setMsg(null);
      try {
        const left = await api.resolveVision(task.queue, task.id, conflict, text, stamp(new Date()));
        setMsg({ ok: true, text: left === 0 ? t("vision.resolvedNone", { id: conflict }) : t("vision.resolvedLeft", { id: conflict, count: left }) });
        setWhich(null);
        setHow("");
        onChanged();
        return true;
      } catch (e) {
        setMsg({ ok: false, text: String(e) });
        return false;
      } finally {
        setBusy(false);
      }
    });
  };
  // Not through `once`: its awaits would come before the clipboard write.
  const copy = () =>
    copyNow(brief.text, navigator.clipboard, t("vision.brief")).then(
      () => setMsg({ ok: true, text: t("vision.copied") }),
      (e) => setMsg({ ok: false, text: String(e) }),
    );

  return (
    <section className={`vision${open.length ? " has-open" : ""}`} aria-label={t("vision.aria")} data-part="vision">
      <h2>{t("vision.heading")} <small>{visionStatus(task)}</small></h2>
      <div className="control-row">
        {open.map((id) => (
          <Button
            key={id}
            type="button"
            className={resolving === id ? "on" : undefined}
            disabled={busy}
            title={titles[id]}
            onClick={() => {
              setWhich(id);
              setHow("");
              setMsg(null);
            }}
          >
            {t("vision.resolve", { id })}
          </Button>
        ))}
        <Button type="button" disabled={brief.text === null} title={brief.text === null && !brief.error ? t("vision.loadingBrief") : undefined} onClick={() => void copy()}>
          {t("vision.copy")}
        </Button>
      </div>
      {brief.error && <p className="err small">{t("vision.cantLoad", { error: brief.error })}</p>}
      {resolving && (
        <form
          className="control-row"
          onSubmit={(e) => {
            e.preventDefault();
            resolve();
          }}
        >
          <input autoFocus value={how} onChange={(e) => setHow(e.target.value)} placeholder={t("vision.howPlaceholder", { id: resolving })} aria-label={t("vision.howLabel", { id: resolving })} maxLength={500} />
          <Button type="submit" disabled={busy || !how.trim()}>{t("vision.resolve", { id: resolving })}</Button>
          <Button type="button" onClick={() => setWhich(null)}>{t("common:cancel")}</Button>
        </form>
      )}
      {msg && <p className={msg.ok ? "ok small" : "err small"}>{msg.text}</p>}
      {!checked && <p className="meta small">{t("vision.notCheckedNote")}</p>}
      {checked && body !== null && section === null && <p className="err small">{t("vision.gone")}</p>}
      {section !== null && <Markdown text={section} />}
    </section>
  );
}
