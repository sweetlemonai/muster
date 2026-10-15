import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "./api";
import type { ManualAt } from "./helpTopics";
import { manualPages, neighbours, type Page, type PageInfo, resolve, search } from "./manual";
import Markdown from "./Markdown";
import { useT } from "./i18n";
import { Pill } from "./ui";

/** The Help tab: the manual (docs/manual), with the Commands page from the core's help (help.rs),
 *  the same list `muster help` prints. Where it's open lives in the window (`at`, `onGo`), so a
 *  "?" elsewhere can open a section and switching tabs keeps the place. */
export default function Help({ at, onGo }: { at: ManualAt; onGo: (file: string, slug: string) => void }) {
  const t = useT("help");
  const [commands, setCommands] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [query, setQuery] = useState("");
  useEffect(() => {
    let live = true;
    api.help().then(
      (h) => {
        if (!live) return;
        if (typeof h.commandsPage === "string") setCommands(h.commandsPage);
        else setError(t("manual.noCommandsPage"));
      },
      (e) => live && setError(String(e)),
    );
    return () => {
      live = false;
    };
    // The error's words are read when it happens; the effect runs once.
  }, []);
  // `t` changes with the language: the pages' titles are read again (manual.ts).
  const pages = useMemo(() => manualPages(commands), [commands, t]);
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
  const t = useT("help");
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
    const to = resolve(page.file, href);
    if (to.kind === "page") go(to.file, to.slug);
    else if (to.kind === "repo") setNote(t("manual.repo", { path: to.path }));
    else if (to.kind === "web") setNote(t("manual.web", { url: to.url }));
    else setNote(t("manual.bad"));
  };
  const copy = (e: React.MouseEvent) => {
    const pre = (e.target as HTMLElement).closest("pre");
    if (!pre) return;
    const text = pre.textContent ?? "";
    navigator.clipboard?.writeText(text).then(
      () => setNote(t("manual.copied", { text })),
      () => setNote(t("manual.noClipboard")),
    );
  };
  const { prev, next } = neighbours(page.file);

  return (
    <main className="manual">
      <nav className="manual-side" aria-label={t("manual.nav")}>
        <input
          type="search"
          className="manual-search"
          value={query}
          onChange={(e) => onQuery(e.target.value)}
          placeholder={t("manual.search")}
          aria-label={t("manual.search")}
        />
        {hits !== null && (
          <div className="manual-hits" role="region" aria-label={t("manual.results")}>
            {hits.length === 0 ? (
              <p className="none">{t("manual.nothing", { query: query.trim() })}</p>
            ) : (
              <ul>
                {/* manual.hit keeps › in every language: it's Bidi_Mirrored, so right to left it
                    already draws as ‹, unlike ◀ ▶ in PageSteps, which a locale turns round itself. */}
                {hits.map((h) => (
                  <li key={`${h.file}#${h.slug}`}>
                    <button type="button" className="hit" onClick={() => go(h.file, h.slug)}>
                      <span className="meta">{t("manual.hit", { title: h.title })}</span> {h.section}
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
          <p className="err">{t("manual.commandsError", { error })}</p>
        ) : (
          <p className="none">{t("manual.loadingCommands")}</p>
        )}
        <PageSteps prev={prev} next={next} onGo={go} />
      </article>
    </main>
  );
}

/** ◀ the page before · the page after ▶, at the top and the bottom of a page. */
function PageSteps({ prev, next, onGo }: { prev: PageInfo | null; next: PageInfo | null; onGo: (file: string, slug: string) => void }) {
  const t = useT("help");
  // Right to left the arrows turn round: back points right, where the page starts.
  return (
    <div className="manual-steps" role="group" aria-label={t("manual.steps")}>
      <Pill type="button" disabled={!prev} onClick={() => prev && onGo(prev.file, "")}>{prev ? t("manual.prev", { title: prev.title }) : t("manual.prevNone")}</Pill>
      <Pill type="button" disabled={!next} onClick={() => next && onGo(next.file, "")}>{next ? t("manual.next", { title: next.title }) : t("manual.nextNone")}</Pill>
    </div>
  );
}
