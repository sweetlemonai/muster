// The task pane's write-up editor, the parts without React: dirty state, keys, messages.

/** Unsaved changes: the text differs from what Edit opened. */
export const isDirty = (opened: string, text: string) => text !== opened;

export type EditAction = "save" | "cancel" | "arm" | null;
type Key = { key: string; metaKey: boolean; ctrlKey: boolean; altKey: boolean; isComposing: boolean };

/** What a key in the editor does. ⌘S (Ctrl+S) saves. Esc cancels; with unsaved changes the first
 *  Esc only arms it (`armed`), so a stray key never throws typing away. Keys inside an input
 *  method's composition belong to it. */
export function editKey(e: Key, dirty: boolean, armed: boolean): EditAction {
  if (e.isComposing) return null;
  if (e.key.toLowerCase() === "s" && (e.metaKey || e.ctrlKey) && !e.altKey) return "save";
  if (e.key === "Escape") return !dirty || armed ? "cancel" : "arm";
  return null;
}

/** The pane's line after a save: the new path, or null when the text was the file's already. */
export const savedText = (path: string | null) =>
  path === null ? "No change: the text is the same as the file's." : "Saved. The edit is in the log.";
