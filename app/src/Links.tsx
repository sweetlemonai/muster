import { useState } from "react";
import { api } from "./api";
import type { Link, Task } from "./types";

/** How a link reads: its title, else its address without https://. */
export const linkLabel = (l: Link) => l.title || l.url.replace(/^https:\/\//i, "");

/** The task's external links (muster link adds them), in the pane's sidebar under its files. A
 *  click asks the core to open one in the browser; the core opens only an address the task has,
 *  and only https, so a hand-written link that isn't https shows without a way to open it.
 *  Nothing when there are none. */
export default function Links({ task }: { task: Task }) {
  const [error, setError] = useState("");
  if (task.links.length === 0) return null;
  const open = (url: string) => {
    setError("");
    api.openLink(task.queue, task.id, url).catch((e) => setError(String(e)));
  };
  return (
    <div className="kv stack links" aria-label="Links">
      <span className="lbl">Links <small>{task.links.length}</small></span>
      <ul className="link-list">
        {task.links.map((l, i) => (
          <li key={`${i}-${l.url}`}>
            {/^https:\/\//i.test(l.url) ? (
              <button type="button" className="link" title={l.url} onClick={() => open(l.url)}>
                {linkLabel(l)}
              </button>
            ) : (
              <span className="meta">
                {l.title ? `${l.title} ` : ""}
                {l.url} (not https, so it won&apos;t open)
              </span>
            )}
          </li>
        ))}
      </ul>
      {error && <p className="err small">{error}</p>}
    </div>
  );
}
