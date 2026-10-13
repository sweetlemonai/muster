import { useState } from "react";
import { api } from "./api";
import { stamp } from "./lib";
import Markdown from "./Markdown";
import type { Task } from "./types";

/** The task's comment thread under the description, oldest first, then a box to add one (⌘↩
 *  sends), in the answer box's look. Anyone may comment at any stage, while someone else holds the
 *  task too. The core cleans the text and refuses a line that would read as a stage change; its
 *  refusal shows as is and the text stays in the box. */
export default function Comments({ task, onChanged }: { task: Task; onChanged: () => void }) {
  const [text, setText] = useState("");
  const [state, setState] = useState<"idle" | "saving" | "done" | "error">("idle");
  const [msg, setMsg] = useState("");
  const send = async () => {
    if (!text.trim() || state === "saving") return;
    setState("saving");
    try {
      await api.addComment(task.queue, task.id, text, stamp(new Date()));
      setState("done");
      setMsg("Comment added.");
      setText("");
      onChanged();
    } catch (e) {
      setState("error");
      setMsg(String(e));
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
          <Markdown text={c.text} />
        </div>
      ))}
      <div className="answer comment-box">
        <textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="Add a comment (⌘↩ to send)"
          aria-label="Your comment"
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
          <button type="button" disabled={!text.trim() || state === "saving"} onClick={() => void send()}>
            {state === "saving" ? "Sending…" : "Comment"}
          </button>
          {msg && <span className={state === "error" ? "err" : "ok"}>{msg}</span>}
        </div>
      </div>
    </section>
  );
}
