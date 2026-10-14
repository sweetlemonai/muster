import { useContext, useState } from "react";
import { api } from "./api";
import { Hl } from "./FilterBar";
import { ago, oneAtATime, queueTotal, stamp } from "./lib";
import type { Note, Task } from "./types";
import ProjectName from "./ProjectName";
import { labelOf, Labels } from "./labels";
import { Badge, Pill } from "./ui";

/** A note's title on the board; an empty note says so. */
export const noteTitle = (n: Pick<Note, "title">) => n.title.trim() || "(empty note)";

/** Which pane shows a note: its queue and file name. */
export const noteKey = (n: Pick<Note, "queue" | "file">) => `${n.queue}/${n.file}`;

/** The notes the Backlog column shows for `queue` ("all" for every project): newest first, then
 *  by project and file name, as the core sorts them. */
export function backlogNotes(notes: Note[], queue: string): Note[] {
  return notes
    .filter((n) => queue === "all" || n.queue === queue)
    .sort((a, b) => b.added - a.added || a.queue.localeCompare(b.queue) || a.file.localeCompare(b.file));
}

export type BacklogItem = { kind: "note"; note: Note } | { kind: "task"; task: Task };

/** The Backlog column: the notes first, then the numbered backlog tasks (as `columns` sorts them). */
export function backlogColumn(notes: Note[], tasks: Task[]): BacklogItem[] {
  return [...notes.map((note) => ({ kind: "note" as const, note })), ...tasks.map((task) => ({ kind: "task" as const, task }))];
}

/** The filter bar's hidden count: the tasks the filters (and the Who chips) hide, plus on the board
 *  the notes they hide. The board lists the notes with Group by on too, in its Backlog band (0046);
 *  the Table, Timeline and Graph don't list them, so they don't count them. */
export function hiddenCount(view: string, tasks: { all: number; shown: number }, notes: { all: number; shown: number }): number {
  return tasks.all - tasks.shown + (view === "board" ? notes.all - notes.shown : 0);
}

/** A project's count in the sidebar: its tasks in every stage, plus its backlog notes. */
export function projectCount(counts: Record<string, number>, notes: Note[], queue: string): number {
  return queueTotal(counts) + notes.filter((n) => n.queue === queue).length;
}

/** File as task, prefilled from the note: its first line as the title, its text as the body. */
export function fileDraft(n: Pick<Note, "title" | "text">) {
  return { title: n.title, kind: "code", priority: "normal", body: n.text };
}

/** A note on the board: quieter than a task's card (no id, no stage colour), with its title, "note"
 *  and how long ago it was added. The search's `words` are marked in the title, as on task cards. */
export function NoteCard({ n, now, multi, active, onOpen, words }: { n: Note; now: number; multi: boolean; active: boolean; onOpen: () => void; words: string[] }) {
  return (
    <button className={`note-card${active ? " active" : ""}`} onClick={onOpen} title={n.file}>
      <span className="title">{n.title.trim() ? <Hl text={noteTitle(n)} words={words} /> : noteTitle(n)}</span>
      <span className="note-meta">
        {multi && <ProjectName name={n.queue} />}
        <Badge variant="note">note</Badge>
        <span className="age">{ago(now, n.added)}</span>
      </span>
    </button>
  );
}

/** What a key does in the quick add field: Enter adds, Esc clears, anything else types. Enter while
 *  an input method is composing picks a candidate, so it does nothing here. The field keeps the
 *  keys it acts on: the board's shortcuts (Esc closes the pane) never see them. */
export function quickAddKey(key: string, composing: boolean): "add" | "clear" | null {
  if (composing) return null;
  if (key === "Enter") return "add";
  if (key === "Escape") return "clear";
  return null;
}

/** The one-line field at the top of the Backlog column. Enter adds the line as a note and empties
 *  the field, which keeps the focus; Esc empties it. Keys typed here stay here: the board's own
 *  shortcuts (Esc closes the pane) don't see them. With more than one project in view, a chip shows
 *  where the note goes; clicking it lays the projects out in place. */
export function QuickAdd({ queues, onAdded }: { queues: string[]; onAdded: () => void }) {
  const [text, setText] = useState("");
  const [target, setTarget] = useState("");
  const labels = useContext(Labels);
  const [picking, setPicking] = useState(false);
  const [error, setError] = useState("");
  const [once] = useState(oneAtATime);
  const queue = queues.includes(target) ? target : queues[0] ?? "";
  const add = () =>
    void once(async () => {
      const line = text;
      if (!line.trim() || !queue) return false;
      try {
        await api.addBacklogNote(queue, line, stamp(new Date()));
        // Only what was sent is cleared: anything typed meanwhile stays.
        setText((t) => (t === line ? "" : t));
        setError("");
        onAdded();
        return true;
      } catch (e) {
        setError(String(e));
        return false;
      }
    });
  return (
    <div className="quick-add">
      <input
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder="Add to the backlog…"
        aria-label={queue ? `Add to the backlog of ${queue}` : "Add to the backlog"}
        disabled={!queue}
        onKeyDown={(e) => {
          const act = quickAddKey(e.key, e.nativeEvent.isComposing);
          if (!act) return;
          e.preventDefault();
          e.stopPropagation();
          if (act === "add") {
            add();
          } else {
            setText("");
            setError("");
          }
        }}
      />
      {queues.length > 1 && (
        <div className="quick-add-to" role="group" aria-label="Which project">
          {picking ? (
            queues.map((q) => (
              <Pill key={q} type="button" on={q === queue} aria-pressed={q === queue} onClick={() => { setTarget(q); setPicking(false); }}>
                {labelOf(labels, q)}
              </Pill>
            ))
          ) : (
            <Pill type="button" title="The project the note goes to; click to change it" onClick={() => setPicking(true)}>
              {`to ${labelOf(labels, queue)}`}
            </Pill>
          )}
        </div>
      )}
      {error && <p className="err small" role="alert">{error}</p>}
    </div>
  );
}
