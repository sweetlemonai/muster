import { useEffect, useId, useRef, useState } from "react";
import { api, type Commented } from "./api";
import { stamp } from "./lib";
import Markdown from "./Markdown";
import type { Task } from "./types";
import { Button } from "./ui";
import { commentNotice, editKey } from "./writeUp";

/** The task's comment thread under the description, oldest first, then a box to add one (⌘↩
 *  sends), in the answer box's look. Anyone may comment at any stage, while someone else holds the
 *  task too. The core cleans the text and refuses a line that would read as a stage change; its
 *  refusal shows as is and the text stays in the box. Typed text is guarded like the write-up
 *  editor's: `onDirty` tells the pane, so the window won't close it or switch cards (`nudge` goes
 *  up each time it refused to); the first Esc only arms, and the second throws the text away.
 *  The owner's comment that starts with @accept, @sendback, @block or @ask runs that first
 *  (mu-101, the core's `fields::comment`), and the box's status says what it did; one line under
 *  the box says so. */
export default function Comments({ task, onChanged, onDirty = () => {}, nudge = 0, keys, onLink }: {
  task: Task;
  onChanged: () => void;
  onDirty?: (dirty: boolean) => void;
  nudge?: number;
  /** Task keys in a comment link to their task (0033): each project's prefix to its queue, and
   *  what a clicked link does. */
  keys?: Map<string, string>;
  onLink?: (href: string) => void;
}) {
  const [text, setText] = useState("");
  const [state, setState] = useState<"idle" | "saving" | "done" | "error">("idle");
  const [msg, setMsg] = useState("");
  const [armed, setArmed] = useState(false);
  // The refusals already answered: a refused switch from before this text doesn't count.
  const [seenNudge, setSeenNudge] = useState(nudge);
  const area = useRef<HTMLTextAreaElement>(null);
  const hintId = useId();
  const dirty = text !== "";
  const refused = nudge !== seenNudge;
  const notice = commentNotice(dirty, armed, refused);

  const clear = () => {
    setText("");
    setArmed(false);
    setSeenNudge(nudge);
  };

  // The window only lets the pane go while nothing here is unsent.
  useEffect(() => {
    onDirty(dirty);
    return () => onDirty(false);
  }, [dirty, onDirty]);

  // A refused switch brings the box back into view.
  useEffect(() => {
    if (refused && dirty) area.current?.focus();
  }, [nudge, refused, dirty]);

  // With typed text, Esc anywhere in the window is the box's: the first arms, the second throws
  // the text away. Caught before the window's own Esc, which would close the pane.
  useEffect(() => {
    if (!dirty) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      const act = editKey({ key: e.key, metaKey: e.metaKey, ctrlKey: e.ctrlKey, altKey: e.altKey, isComposing: e.isComposing }, true, armed);
      if (act === null) return;
      e.stopPropagation();
      e.preventDefault();
      if (act === "cancel") clear();
      else setArmed(true);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
    // `clear` reads `nudge`, so the listener is renewed when it changes.
  }, [dirty, armed, nudge]);

  const send = async () => {
    if (!text.trim() || state === "saving") return;
    setState("saving");
    try {
      const r = await api.addComment(task.queue, task.id, text, stamp(new Date()));
      // Its action ran even when the comment then couldn't be saved: the box empties all the
      // same, so sending it again doesn't run the action twice.
      setState(r.unsaved ? "error" : "done");
      setMsg(commentDone(r));
      clear();
      onChanged();
    } catch (e) {
      // Refused: the text stays in the box. The board is read again all the same, in case a
      // refusal came after a change (a send-back whose finding couldn't be written).
      setState("error");
      setMsg(String(e));
      onChanged();
    }
  };
  return (
    <section className="comments" aria-label="Comments">
      <h2>Comments <small>{task.comments.length}</small></h2>
      {task.comments.length === 0 && <p className="none">No comments yet.</p>}
      {task.comments.map((c, i) => (
        <div key={`${i}-${c.at}-${c.who}`} className="comment">
          <p className="meta">
            {c.who} · {c.at}
          </p>
          <Markdown text={c.text} keys={keys} onLink={onLink} />
        </div>
      ))}
      <div className="answer comment-box">
        <textarea
          ref={area}
          value={text}
          onChange={(e) => {
            setText(e.target.value);
            setArmed(false);
          }}
          placeholder="Add a comment (⌘↩ to send)"
          aria-label="Your comment"
          aria-describedby={hintId}
          dir="auto"
          rows={3}
          maxLength={4000}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
              e.preventDefault();
              void send();
            }
          }}
        />
        <div className="answer-bar">
          <Button type="button" disabled={!text.trim() || state === "saving"} onClick={() => void send()}>
            {state === "saving" ? "Sending…" : "Comment"}
          </Button>
          {notice && <span className="err">{notice}</span>}
          {!notice && msg && <span className={state === "error" ? "err" : "ok"} role="status">{msg}</span>}
        </div>
        <p id={hintId} className="meta small comment-hint">Start with @accept, @sendback &lt;why&gt;, @block &lt;why&gt; or @ask &lt;question&gt; to act.</p>
      </div>
    </section>
  );
}

/** What the box says once the core took a comment: added, and what its @action did; or what the
 *  action did and why its comment couldn't be saved. */
export function commentDone(r: Commented): string {
  if (r.unsaved) return `Done: ${r.did}. The comment couldn't be saved (${r.unsaved}), so it isn't in the thread; don't send it again.`;
  return r.did ? `Comment added: ${r.did}.` : "Comment added.";
}
