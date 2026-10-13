import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "./api";
import type { ManualAt } from "./helpTopics";
import { manualPages, neighbours, type Page, type PageInfo, resolve, search } from "./manual";
import Markdown from "./Markdown";

/** The Help tab: the manual (docs/manual), with the Commands page from the core's help (help.rs),
 *  the same list `muster help` prints. Where it's open lives in the window (`at`, `onGo`), so a
 *  "?" elsewhere can open a section and switching tabs keeps the place. */
export default function Help({ at, onGo }: { at: ManualAt; onGo: (file: string, slug: string) => void }) {
  const [commands, setCommands] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [query, setQuery] = useState("");
  useEffect(() => {
    let live = true;
    api.help().then(
      (h) => {
        if (!live) return;
        if (typeof h.commandsPage === "string") setCommands(h.commandsPage);
        else setError("the core sent no Commands page");
      },
      (e) => live && setError(String(e)),
    );
    return () => {
      live = false;
    };
  }, []);
  const pages = useMemo(() => manualPages(commands), [commands]);
  return <ManualView pages={pages} at={at} query={query} onQuery={setQuery} onGo={onGo} error={error} />;
}

/** The manual, docked in the tab: the sidebar (search, its results, then every page with its
 *  sections, the current one lit) beside the page, with ◀ ▶ to the pages before and after.
 *  Links between pages and sections move here; nothing opens over it. Click a command line to copy it. */
export function ManualView({ pages, at, query, onQuery, onGo, error = "" }: {
  pages: Page[];
  at: ManualAt;
  query: string;
  onQuery: (q: string) => void;
  onGo: (file: string, slug: string) => void;
  error?: string;
}) {
  const page = pages.find((p) => p.file === at.file) ?? pages[0];
  const hits = useMemo(() => (query.trim() ? search(pages, query) : null), [pages, query]);
  const [note, setNote] = useState("");
  const article = useRef<HTMLElement>(null);
  const loaded = page.text !== null;

  // Go to the section asked for, or the page's top; again on every ask (`n`), and once the
  // Commands page arrives.
  useEffect(() => {
    const el = article.current;
    if (!el) return;
    const target = at.slug ? el.querySelector(`[data-anchor="${CSS.escape(at.slug)}"]`) : null;
    if (target) target.scrollIntoView({ block: "start" });
    else el.scrollTop = 0;
  }, [at.file, at.slug, at.n, loaded]);

  // Moving clears the last note (a copy, a link that doesn't open here).
  const go = (file: string, slug: string) => {
    setNote("");
    onGo(file, slug);
  };
  const follow = (href: string) => {
    const t = resolve(page.file, href);
    if (t.kind === "page") go(t.file, t.slug);
    else if (t.kind === "repo") setNote(`That's ${t.path} in the Muster repo, which the app doesn't open: read it on GitHub or in your editor.`);
    else if (t.kind === "web") setNote(`Web links don't open from the manual: ${t.url}`);
    else setNote("That link doesn't lead anywhere.");
  };
  const copy = (e: React.MouseEvent) => {
    const pre = (e.target as HTMLElement).closest("pre");
    if (!pre) return;
    const text = pre.textContent ?? "";
    navigator.clipboard?.writeText(text).then(
      () => setNote(`Copied: ${text}`),
      () => setNote("The clipboard isn't available here."),
    );
  };
  const { prev, next } = neighbours(page.file);

  return (
    <main className="manual">
      <nav className="manual-side" aria-label="Manual">
        <input
          type="search"
          className="manual-search"
          value={query}
          onChange={(e) => onQuery(e.target.value)}
          placeholder="Search the manual"
          aria-label="Search the manual"
        />
        {hits !== null && (
          <div className="manual-hits" role="region" aria-label="Search results">
            {hits.length === 0 ? (
              <p className="none">Nothing found for “{query.trim()}”.</p>
            ) : (
              <ul>
                {hits.map((h) => (
                  <li key={`${h.file}#${h.slug}`}>
                    <button type="button" onClick={() => go(h.file, h.slug)}>
                      <span className="meta">{`${h.title} ›`}</span> {h.section}
                      <span className="snippet">{h.snippet}</span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}
        <ol className="manual-pages">
          {pages.map((p) => {
            const on = p.file === page.file;
            return (
              <li key={p.file}>
                <button type="button" className={on ? "page on" : "page"} aria-current={on ? "page" : undefined} onClick={() => go(p.file, "")}>{p.title}</button>
                {p.sections.length > 0 && (
                  <ul>
                    {p.sections.map((s) => (
                      <li key={s.slug}>
                        <button
                          type="button"
                          className={on && at.slug === s.slug ? "section on" : "section"}
                          data-section={`${p.file}#${s.slug}`}
                          onClick={() => go(p.file, s.slug)}
                        >
                          {s.title}
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
              </li>
            );
          })}
        </ol>
      </nav>
      <article className="manual-page" ref={article} onClick={copy}>
        <PageSteps prev={prev} next={next} onGo={go} />
        {note && <p className="meta manual-note" role="status">{note}</p>}
        {page.text !== null ? (
          <Markdown text={page.text} anchors onLink={follow} />
        ) : error ? (
          <p className="err">Couldn't load the commands: {error}</p>
        ) : (
          <p className="none">Loading the commands…</p>
        )}
        <PageSteps prev={prev} next={next} onGo={go} />
      </article>
    </main>
  );
}

/** ◀ the page before · the page after ▶, at the top and the bottom of a page. */
function PageSteps({ prev, next, onGo }: { prev: PageInfo | null; next: PageInfo | null; onGo: (file: string, slug: string) => void }) {
  return (
    <div className="manual-steps">
      <button type="button" className="chip" disabled={!prev} onClick={() => prev && onGo(prev.file, "")}>{prev ? `◀ ${prev.title}` : "◀"}</button>
      <button type="button" className="chip" disabled={!next} onClick={() => next && onGo(next.file, "")}>{next ? `${next.title} ▶` : "▶"}</button>
    </div>
  );
}
