import { memo, useMemo } from "react";
import MarkdownIt from "markdown-it";

// Task text is untrusted: raw HTML stays text (html: false) and markdown-it refuses unsafe
// link schemes. Links never navigate the window; their address shows as a tooltip.
const md = new MarkdownIt({ html: false, linkify: true, breaks: false });

function Markdown({ text }: { text: string }) {
  const html = useMemo(() => md.render(text), [text]);
  return (
    <div
      className="markdown"
      // Safe: html:false escapes any HTML in `text`.
      dangerouslySetInnerHTML={{ __html: html }}
      onClick={(e) => {
        if ((e.target as HTMLElement).closest("a")) e.preventDefault();
      }}
    />
  );
}

export default memo(Markdown);
