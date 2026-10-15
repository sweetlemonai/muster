// The task pane's write-up editor, the parts without React: dirty state, keys, messages.
import { tn } from "./i18n";

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

/** What the editor says when the window refused to leave it (in the language shown now, mu-148). */
export const unsavedText = () => tn("pane", "pane.editor.unsaved");

/** What the comment box says, with typed text, after a first Esc or a refused switch. */
export const unsentText = () => tn("pane", "pane.editor.unsent");

/** The comment box's notice: only while it holds text, once Esc armed the discard or the window
 *  refused to leave it. */
export const commentNotice = (dirty: boolean, armed: boolean, refused: boolean): string | null =>
  dirty && (armed || refused) ? unsentText() : null;

/** The task pane's unsaved parts (the write-up editor, the comment box), with one part's state
 *  set. The same object when nothing changed, so setting it again doesn't re-render. */
export function withPart(parts: Record<string, boolean>, part: string, dirty: boolean): Record<string, boolean> {
  return (parts[part] ?? false) === dirty ? parts : { ...parts, [part]: dirty };
}

/** Whether any part of the pane has unsaved text: then the window keeps the pane where it is. */
export const anyDirty = (parts: Record<string, boolean>) => Object.values(parts).some(Boolean);

type PaneLike = { kind: string; key?: string } | null;

/** May the docked pane change from `from` to `to`? Not while the write-up editor has unsaved
 *  changes (the pane would unmount and take the typing with it), unless it stays on the same card. */
export function mayLeave(unsaved: boolean, from: PaneLike, to: PaneLike): boolean {
  return !unsaved || (from?.kind === to?.kind && from?.key === to?.key);
}

/** The pane's line after a save: the new path, or null when the text was the file's already. */
export const savedText = (path: string | null) =>
  tn("pane", path === null ? "pane.editor.noChange" : "pane.editor.saved");
