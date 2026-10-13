import MarkdownIt, { type Token } from "markdown-it";

// Headings as places to go: the manual's sidebar, search and links between sections use each
// heading's slug, made the way GitHub makes them, so the same links work on GitHub and in the app.

/** `line` is where it is in the document, counting from 0. */
export type Heading = { level: number; title: string; slug: string; line: number };

/** GitHub's slug for a heading: lowercase, punctuation out (letters, digits, `_` and `-` stay),
 *  every space a hyphen. "Claims, leases and check-ins" → "claims-leases-and-check-ins". */
export function slugify(title: string): string {
  return title.trim().toLowerCase().replace(/[^\p{L}\p{N}\s_-]/gu, "").replace(/\s/g, "-");
}

/** The words of a heading's inline token, as GitHub reads them: text and code, not the markup. */
function headingText(inline: Token | undefined): string {
  return (inline?.children ?? []).map((c) => (c.type === "text" || c.type === "code_inline" ? c.content : "")).join("");
}

/** Every heading in a parsed document with its slug, a repeated one numbered (`examples-1`). */
function headingsOf(tokens: Token[]): { token: Token; heading: Heading }[] {
  const seen = new Map<string, number>();
  const out: { token: Token; heading: Heading }[] = [];
  tokens.forEach((t, i) => {
    if (t.type !== "heading_open") return;
    const title = headingText(tokens[i + 1]);
    const base = slugify(title);
    const n = seen.get(base) ?? 0;
    seen.set(base, n + 1);
    out.push({ token: t, heading: { level: Number(t.tag.slice(1)), title, slug: n ? `${base}-${n}` : base, line: t.map?.[0] ?? 0 } });
  });
  return out;
}

/** The manual's Markdown: task text's rules (raw HTML stays text), each heading carrying its slug
 *  as `data-anchor`, and no web links made from bare words like INDEX.md (GitHub makes none). */
export const manualMd = new MarkdownIt({ html: false, linkify: true, breaks: false });
manualMd.linkify.set({ fuzzyLink: false });
manualMd.core.ruler.push("anchors", (state) => {
  for (const { token, heading } of headingsOf(state.tokens)) token.attrSet("data-anchor", heading.slug);
});

/** A Markdown document's headings, in order. */
export function headings(md: string): Heading[] {
  return headingsOf(manualMd.parse(md, {})).map((h) => h.heading);
}

/** Every link's address in a Markdown document, images' included. */
export function linksIn(md: string): string[] {
  const out: string[] = [];
  const walk = (tokens: Token[]) => {
    for (const t of tokens) {
      if (t.type === "link_open") out.push(String(t.attrGet("href") ?? ""));
      if (t.type === "image") out.push(String(t.attrGet("src") ?? ""));
      if (t.children) walk(t.children);
    }
  };
  walk(manualMd.parse(md, {}));
  return out;
}
