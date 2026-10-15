import type { MarkdownIt } from "markdown-it";

/** The blocks that read in their own text's direction. */
const AUTO = new Set(["paragraph_open", "heading_open", "list_item_open", "blockquote_open", "th_open", "td_open"]);

/**
 * Each block of rendered text in its own direction (mu-148): `dir="auto"` on every paragraph,
 * heading, list item, quote and table cell, so a Persian paragraph reads right to left inside an
 * English body and the other way round; code blocks `dir="ltr"`, since code always reads left to
 * right. The renderer sets it, never the text: with `html: false` nothing in the text becomes an
 * attribute.
 */
export function blockDir(md: MarkdownIt): void {
  md.core.ruler.push("block_dir", (state) => {
    for (const t of state.tokens) if (AUTO.has(t.type)) t.attrSet("dir", "auto");
  });
  for (const rule of ["fence", "code_block"] as const) {
    const render = md.renderer.rules[rule];
    if (!render) continue;
    md.renderer.rules[rule] = (tokens, idx, options, env, self) => render(tokens, idx, options, env, self).replace(/^<pre>/, '<pre dir="ltr">');
  }
}
