import { useState } from "react";
import { api } from "./api";
import { stamp } from "./lib";
import type { Task } from "./types";
import { editKey, isDirty, savedText } from "./writeUp";

/** The task's write-up as text, in place of its rendering: ⌘S saves, Esc cancels. `opened` is the
 *  write-up Edit opened; the save sends it, so the core refuses it if the file changed since. A
 *  refusal shows here and the text stays, so nothing typed is lost. */
export default function EditBody({ task, opened, onClose, onSaved }: { task: Task; opened: string; onClose: () => void; onSaved: (msg: string) => void }) {
  const [text, setText] = useState(opened);
  const [armed, setArmed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const dirty = isDirty(opened, text);

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
      <p className="meta small">Editing the write-up · ⌘S saves · Esc cancels</p>
      <textarea
        autoFocus
        value={text}
        aria-label="The write-up, in Markdown"
        onChange={(e) => {
          setText(e.target.value);
          setArmed(false);
        }}
        onKeyDown={(e) => {
          // Esc here is the editor's, never the window's (which would close the pane).
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
        <button type="button" className="primary" disabled={busy} onClick={() => void save()}>{busy ? "Saving…" : "Save"}</button>
        <button type="button" disabled={busy} onClick={onClose}>Cancel</button>
        {armed && <span className="meta">Esc again to throw your changes away</span>}
        {error && <span className="err">{error}</span>}
      </div>
    </div>
  );
}
