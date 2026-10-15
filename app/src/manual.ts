// The manual: its pages, bundled at build time from docs/manual (the same files GitHub shows),
// the generated Commands and Shortcuts pages, links between them, and search. Loaded with the
// Help tab, never in the window's first chunk.
import startHere from "../../docs/manual/01-start-here.md?raw";
import concepts from "../../docs/manual/02-concepts.md?raw";
import tutorials from "../../docs/manual/03-tutorials.md?raw";
import inTheApp from "../../docs/manual/04-in-the-app.md?raw";
import forAgents from "../../docs/manual/07-for-agents.md?raw";
import faq from "../../docs/manual/08-faq.md?raw";
import { type Heading, headings, linksIn, slugify } from "./anchors";
import { tn, type TKey } from "./i18n";
import { shortcutsPage } from "./shortcutsPage";

export { headings, linksIn, slugify };

export type PageInfo = { file: string; title: string };

/** A page whose title is help.json's `pages.<file>` (mu-148), read in the language shown each time
 *  it's asked, so the Help tab's list follows a switch. The page's own text stays as written. */
const titled = (file: string): PageInfo => ({
  file,
  get title() {
    return tn("help", `pages.${file.replace(/\.md$/, "")}` as TKey<"help">);
  },
});

/** The pages, in reading order. */
export const PAGES: PageInfo[] = ["01-start-here.md", "02-concepts.md", "03-tutorials.md", "04-in-the-app.md", "05-commands.md", "06-shortcuts.md", "07-for-agents.md", "08-faq.md"].map(titled);

const WRITTEN: Record<string, string> = {
  "01-start-here.md": startHere,
  "02-concepts.md": concepts,
  "03-tutorials.md": tutorials,
  "04-in-the-app.md": inTheApp,
  "07-for-agents.md": forAgents,
  "08-faq.md": faq,
};

/** A page with its Markdown (null while the core's help is loading, for Commands) and its
 *  sections: the `##` headings the sidebar lists. */
export type Page = PageInfo & { text: string | null; sections: Heading[] };

/** Every page. `commands` is the core's Commands page (help.rs), null until it has loaded. */
export function manualPages(commands: string | null): Page[] {
  const made: Record<string, string | null> = { "05-commands.md": commands, "06-shortcuts.md": shortcutsPage() };
  return PAGES.map((p) => {
    const text = p.file in made ? made[p.file] : (WRITTEN[p.file] ?? null);
    return { ...p, text, sections: text === null ? [] : headings(text).filter((h) => h.level === 2) };
  });
}

/** The pages before and after one, for ◀ ▶. */
export function neighbours(file: string): { prev: PageInfo | null; next: PageInfo | null } {
  const i = PAGES.findIndex((p) => p.file === file);
  return { prev: i > 0 ? PAGES[i - 1] : null, next: i >= 0 && i < PAGES.length - 1 ? PAGES[i + 1] : null };
}

/** Where a link in a manual page leads: a page of the manual (and a section, "" for its top),
 *  another file in the repo, a web page (https only), or nowhere it may go. */
export type Target =
  | { kind: "page"; file: string; slug: string }
  | { kind: "repo"; path: string; slug: string }
  | { kind: "web"; url: string }
  | { kind: "bad"; href: string };

const MANUAL_DIR = ["docs", "manual"];

/** Resolves `href` from the page `from`, as GitHub would from docs/manual/. */
export function resolve(from: string, href: string): Target {
  const bad: Target = { kind: "bad", href };
  if (/^https:\/\/\S+$/i.test(href)) return { kind: "web", url: href };
  // Any other scheme, an absolute path, or nothing at all.
  if (!href || /^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith("/")) return bad;
  const hash = href.indexOf("#");
  const path = hash < 0 ? href : href.slice(0, hash);
  let slug = hash < 0 ? "" : href.slice(hash + 1);
  try {
    slug = decodeURIComponent(slug);
  } catch {
    return bad;
  }
  if (!path) return { kind: "page", file: from, slug };
  const parts = [...MANUAL_DIR];
  for (const seg of path.split("/")) {
    if (seg === "" || seg === ".") continue;
    if (seg === "..") {
      if (!parts.length) return bad;
      parts.pop();
    } else parts.push(seg);
  }
  if (!parts.length) return bad;
  const full = parts.join("/");
  const page = PAGES.find((p) => `${MANUAL_DIR.join("/")}/${p.file}` === full);
  return page ? { kind: "page", file: page.file, slug } : { kind: "repo", path: full, slug };
}

/** A search result: the page, the section it's in (slug "" for the page's opening), and the words
 *  around the first match. */
export type Hit = { file: string; title: string; slug: string; section: string; snippet: string };

/** Markdown as plain words for matching: no escapes, code marks, emphasis or heading marks. */
const plain = (md: string) =>
  md
    .replace(/\\([!-/:-@[-`{-~])/g, "$1")
    .replace(/[`*]/g, "")
    .replace(/^#+\s+/gm, "")
    .replace(/\s+/g, " ")
    .trim();

/** The words around the first of `words` in `text`. */
function snippet(text: string, word: string): string {
  const at = text.toLowerCase().indexOf(word);
  const from = Math.max(0, at - 50);
  const to = Math.min(text.length, at + word.length + 70);
  return `${from > 0 ? "…" : ""}${text.slice(from, to)}${to < text.length ? "…" : ""}`;
}

/** Every section of every page holding all the words of `query`, in any case, in reading order,
 *  at most `limit`. A simple in-memory match: the whole manual is a few hundred KB at most. */
export function search(pages: Page[], query: string, limit = 30): Hit[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return [];
  const hits: Hit[] = [];
  for (const p of pages) {
    if (p.text === null) continue;
    const lines = p.text.split("\n");
    const marks = headings(p.text).filter((h) => h.level > 1);
    const parts = [{ slug: "", section: p.title, from: 0 }, ...marks.map((h) => ({ slug: h.slug, section: h.title, from: h.line }))];
    for (let i = 0; i < parts.length && hits.length < limit; i++) {
      const text = plain(lines.slice(parts[i].from, parts[i + 1]?.from ?? lines.length).join("\n"));
      const lower = text.toLowerCase();
      if (words.every((w) => lower.includes(w))) {
        hits.push({ file: p.file, title: p.title, slug: parts[i].slug, section: parts[i].section, snippet: snippet(text, words[0]) });
      }
    }
    if (hits.length >= limit) break;
  }
  return hits;
}
