import type { MarkdownIt, Token } from "markdown-it";
import { keyTitle, labelOf } from "./labels";
import { keysIn, taskHref } from "./taskKey";

/** What the window renders task text with: each project's prefix to its queue's name, and the
 *  projects' labels by name (0044), for each link's tooltip. */
export type KeyEnv = { keys?: Map<string, string>; labels?: ReadonlyMap<string, string> };

/** Task keys in rendered text link to their task (0033): `mu-12` written anywhere in a task's
 *  text becomes a link to `taskHref`, for each prefix in `env.keys` (prefix → queue, taskKey.ts
 *  `keyIndex`). Text already in a link, and code, stay as they are. Without `env.keys`, nothing
 *  changes. With `env.labels`, each link's tooltip names its project by its label (0044). The
 *  window opens the task when the link is clicked (Markdown's `onLink`). */
export function taskKeyLinks(md: MarkdownIt): void {
  md.core.ruler.push("task_keys", (state) => {
    // markdown-it types `env` as any: it's what Markdown passes to `render`, a KeyEnv.
    const keys = (state.env as KeyEnv | undefined)?.keys;
    const labels = (state.env as KeyEnv | undefined)?.labels;
    if (!keys?.size) return;
    const text = (content: string) => {
      const t = new state.Token("text", "", 0);
      t.content = content;
      return t;
    };
    for (const block of state.tokens) {
      if (block.type !== "inline" || !block.children) continue;
      const out: Token[] = [];
      let inLink = 0;
      for (const t of block.children) {
        if (t.type === "link_open") inLink++;
        if (t.type === "link_close") inLink--;
        const found = t.type === "text" && inLink === 0 ? keysIn(t.content, keys) : [];
        if (!found.length) {
          out.push(t);
          continue;
        }
        let at = 0;
        for (const k of found) {
          if (k.start > at) out.push(text(t.content.slice(at, k.start)));
          const open = new state.Token("link_open", "a", 1);
          open.attrSet("href", taskHref(k.queue, k.id));
          open.attrSet("class", "task-key");
          if (labels) open.attrSet("title", keyTitle(labelOf(labels, k.queue), t.content.slice(k.start, k.end)));
          out.push(open, text(t.content.slice(k.start, k.end)), new state.Token("link_close", "a", -1));
          at = k.end;
        }
        if (at < t.content.length) out.push(text(t.content.slice(at)));
      }
      block.children = out;
    }
  });
}
