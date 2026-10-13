import { memo, useMemo } from "react";
import MarkdownIt from "markdown-it";
import { manualMd } from "./anchors";

// Task text is untrusted: raw HTML stays text (html: false) and markdown-it refuses unsafe
// link schemes. Links never navigate the window; their address shows as a tooltip.
const md = new MarkdownIt({ html: false, linkify: true, breaks: false });

/** `anchors` renders the manual's way (anchors.ts): headings carry their slug. `onLink` gets the
 *  address of a clicked link, which still never navigates the window. */
function Markdown({ text, anchors = false, onLink }: { text: string; anchors?: boolean; onLink?: (href: string) => void }) {
  const html = useMemo(() => (anchors ? manualMd : md).render(text), [text, anchors]);
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
