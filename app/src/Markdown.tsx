import { memo, useContext, useMemo } from "react";
import MarkdownIt from "markdown-it";
import { manualMd } from "./anchors";
import { taskKeyLinks } from "./keyLinks";
import { Labels } from "./labels";

// Task text is untrusted: raw HTML stays text (html: false) and markdown-it refuses unsafe
// link schemes. Links never navigate the window; their address shows as a tooltip. A task's key
// in the text links to the task (0033) when `keys` is given.
const md = new MarkdownIt({ html: false, linkify: true, breaks: false }).use(taskKeyLinks);

/** `anchors` renders the manual's way (anchors.ts): headings carry their slug. `onLink` gets the
 *  address of a clicked link, which still never navigates the window. `keys` (each project's
 *  prefix to its queue, taskKey.ts `keyIndex`) makes task keys links (keyLinks.ts), each with its
 *  project's label in its tooltip (0044). */
function Markdown({ text, anchors = false, onLink, keys }: { text: string; anchors?: boolean; onLink?: (href: string) => void; keys?: Map<string, string> }) {
  const labels = useContext(Labels);
  const html = useMemo(() => (anchors ? manualMd.render(text) : md.render(text, { keys, labels })), [text, anchors, keys, labels]);
  return (
    <div
      className="markdown"
      // Safe: html:false escapes any HTML in `text`.
      dangerouslySetInnerHTML={{ __html: html }}
      onClick={(e) => {
        const a = (e.target as HTMLElement).closest("a");
        if (!a) return;
        e.preventDefault();
        onLink?.(a.getAttribute("href") ?? "");
      }}
    />
  );
}

export default memo(Markdown);
