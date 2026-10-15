import { useEffect, useRef, useState } from "react";
import { api } from "./api";
import { useT } from "./i18n";
import { stamp } from "./lib";
import { escOwned } from "./ownEscape";
import type { Task } from "./types";
import { Button } from "./ui";
import { editKey, isDirty, savedText, unsavedText } from "./writeUp";

/** The task's write-up as text, in place of its rendering: ⌘S saves, Esc cancels. `opened` is the
 *  write-up Edit opened; the save sends it, so the core refuses it if the file changed since. A
 *  refusal shows here and the text stays, so nothing typed is lost. `onDirty` tells the window
 *  whether there are unsaved changes (so it won't switch panes); `nudge` goes up each time it
 *  refused to, and the editor then says why. */
export default function EditBody({ task, opened, onClose, onSaved, onDirty = () => {}, nudge = 0 }: {
  task: Task;
  opened: string;
  onClose: () => void;
  onSaved: (msg: string) => void;
  onDirty?: (dirty: boolean) => void;
  nudge?: number;
}) {
  const t = useT("pane");
  const [text, setText] = useState(opened);
  const [armed, setArmed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [nudgeAtOpen] = useState(nudge);
  const area = useRef<HTMLTextAreaElement>(null);
  const dirty = isDirty(opened, text);
  const refusedToLeave = nudge !== nudgeAtOpen;

  // The window only lets the pane go while nothing here is unsaved.
  useEffect(() => {
    onDirty(dirty);
    return () => onDirty(false);
  }, [dirty, onDirty]);

  // A refused switch brings the editor back into view.
  useEffect(() => {
    if (refusedToLeave) area.current?.focus();
  }, [nudge, refusedToLeave]);

  // With unsaved changes, Esc anywhere in the window is the editor's: the first arms, the second
  // throws the changes away. Caught before the window's own Esc, which would close the pane. Not
  // one pressed in Release's confirm (mu-184), which is the confirm's own.
  useEffect(() => {
    if (!dirty) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || escOwned(e.target)) return;
      e.stopPropagation();
      const act = editKey({ key: e.key, metaKey: e.metaKey, ctrlKey: e.ctrlKey, altKey: e.altKey, isComposing: e.isComposing }, true, armed);
      if (act === null) return;
      e.preventDefault();
      if (act === "cancel") onClose();
      else setArmed(true);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [dirty, armed, onClose]);

  const save = async () => {
    if (busy) return;
    setBusy(true);
    setError("");
    try {
      const path = await api.editBody(task.queue, task.id, text, opened, stamp(new Date()));
      onSaved(savedText(path));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="write-up-edit">
      <p className="meta small">{t("pane.editor.hint")}</p>
      {refusedToLeave && dirty && <p className="err small">{unsavedText()}</p>}
      <textarea
        ref={area}
        autoFocus
        value={text}
        aria-label={t("pane.editor.label")}
        onChange={(e) => {
          setText(e.target.value);
          setArmed(false);
        }}
        onKeyDown={(e) => {
          // Esc here is the editor's, never the window's (which would close the pane). With
          // unsaved changes the window-wide listener above has it first.
          if (e.key === "Escape") e.stopPropagation();
          const act = editKey({ key: e.key, metaKey: e.metaKey, ctrlKey: e.ctrlKey, altKey: e.altKey, isComposing: e.nativeEvent.isComposing }, dirty, armed);
          if (act === null) return;
          e.preventDefault();
          if (act === "save") void save();
          else if (act === "cancel") onClose();
          else setArmed(true);
        }}
      />
      <div className="edit-bar">
        <Button type="button" variant="primary" disabled={busy} onClick={() => void save()}>{busy ? t("pane.editor.saving") : t("common:save")}</Button>
        <Button type="button" disabled={busy} onClick={() => (dirty && !armed ? setArmed(true) : onClose())}>{t("common:cancel")}</Button>
        {armed && <span className="meta">{t("pane.editor.again")}</span>}
        {error && <span className="err">{error}</span>}
      </div>
    </div>
  );
}
