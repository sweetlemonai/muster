import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "./api";
import { mayLeave } from "./writeUp";
import { ago, columns, estimateTotal, feed, key, STAGES } from "./lib";
import { estimateText } from "./fields";
import type { State, Task } from "./types";
import { backlogColumn, backlogNotes, NoteCard, noteKey, QuickAdd } from "./Backlog";
import Projects from "./Projects";
import ProjectList from "./ProjectList";
import NewTask from "./NewTask";
import Appearance from "./Appearance";
import { CardBand, cardClass, LiveDot } from "./CardLook";
import CardStatus from "./CardStatus";
import { GraphView } from "./GraphView";
import { TableView, TimelineView } from "./Views";
import { GroupByChips, GroupedBoard } from "./Groups";
import { childrenOf, GROUP_KEY, type GroupBy, groupItems, indexTasks, isWork, parentLine, parseGroupBy } from "./levels";
import { visionWarnings } from "./Vision";
import FilterBar, { Hl, HlId } from "./FilterBar";
import { applyQuery, dayContext, EMPTY_QUERY, loadViews, notesFor, storeViews, type Query, type SavedView } from "./filters";
import { indexStamp, needsIndex, queryWords, withSearch, type IndexStamp, type SearchIndex } from "./search";
import { focusSearchBox, searchView, windowKey } from "./keys";
import HumanBoard from "./HumanBoard";
import { byWho, FOR_FILTERS, FOR_KEY, type ForFilter, humanBoard, humanColumn, isHuman, needsYouCount, parseForFilter, waitsOnYou } from "./human";
import YouRibbon from "./YouRibbon";
import { HelpButton, OpenHelp } from "./HelpButton";
import { HELP_TOPICS, type HelpTopic, type ManualAt } from "./helpTopics";
import logo from "./logo.png";

// The task panel and its Markdown renderer load on first open, in their own chunk.
const TaskPanel = lazy(() => import("./TaskPanel"));
const NotePanel = lazy(() => import("./NotePanel"));
const Helper = lazy(() => import("./Helper"));
const HelpView = lazy(() => import("./Help"));
type View = "board" | "table" | "timeline" | "graph" | "helper" | "waiting" | "activity" | "problems" | "projects" | "help";
// One docked pane beside the content, never over it (the owner wants everything in view).
type Pane = { kind: "task"; key: string } | { kind: "note"; key: string } | { kind: "new" } | { kind: "appearance" } | null;
const POLL_MS = 2000;
const PANE_KEY = "muster.paneWidth";

// Poll the core while the window is visible; stop when it's hidden.
function useQueues() {
  const [state, setState] = useState<State | null>(null);
  const [error, setError] = useState("");
  const [at, setAt] = useState(0);
  const busy = useRef(false);
  const refresh = useCallback(async () => {
    if (busy.current) return;
    busy.current = true;
    try {
      setState(await api.state());
      setError("");
      setAt(Date.now());
    } catch (e) {
      setError(String(e));
    } finally {
      busy.current = false;
    }
  }, []);
  useEffect(() => {
    let timer = 0;
    const tick = () => {
      window.clearTimeout(timer);
      if (document.hidden) return;
      void refresh();
      timer = window.setTimeout(tick, POLL_MS);
    };
    tick();
    document.addEventListener("visibilitychange", tick);
    return () => {
      window.clearTimeout(timer);
      document.removeEventListener("visibilitychange", tick);
    };
  }, [refresh]);
  return { state, error, at, refresh };
}

/** The search text of the tasks in view, from the core: asked for only while the search has
 *  words, and again only when a task in view is new or changed (search.ts `needsIndex`). Dropped
 *  when the search empties; an answer that comes after a newer question is ignored. */
function useSearchIndex(queue: string, tasks: Task[], searching: boolean): { index: SearchIndex | null; error: string } {
  const [index, setIndex] = useState<SearchIndex | null>(null);
  const [error, setError] = useState("");
  const asked = useRef<IndexStamp | null>(null);
  const latest = useRef(0);
  useEffect(() => {
    if (!searching) {
      latest.current++;
      asked.current = null;
      setIndex(null);
      setError("");
      return;
    }
    if (!needsIndex(searching, tasks, asked.current)) return;
    asked.current = indexStamp(tasks);
    const n = ++latest.current;
    api.searchIndex(queue === "all" ? null : queue).then(
      (got) => {
        if (n !== latest.current) return;
        setIndex(got);
        setError("");
      },
      (e: unknown) => {
        if (n !== latest.current) return;
        // Asked again at the next poll.
        asked.current = null;
        setError(String(e));
      },
    );
  }, [queue, tasks, searching]);
  return { index, error };
}

function usePaneWidth(): [number, (w: number) => void] {
  const [w, setW] = useState(() => Number(localStorage.getItem(PANE_KEY)) || 560);
  const set = useCallback((v: number) => {
    const c = Math.round(Math.min(Math.max(v, 360), Math.max(420, window.innerWidth - 420)));
    setW(c);
    localStorage.setItem(PANE_KEY, String(c));
  }, []);
  return [w, set];
}

/** Group by for the board and the table, remembered between launches. */
function useGroupBy(): [GroupBy, (g: GroupBy) => void] {
  const [g, setG] = useState<GroupBy>(() => parseGroupBy(localStorage.getItem(GROUP_KEY)));
  const set = useCallback((v: GroupBy) => {
    setG(v);
    localStorage.setItem(GROUP_KEY, v);
  }, []);
  return [g, set];
}

/** The project's saved views ("All" is a project here), from local storage. */
function useSavedViews(project: string): [SavedView[], (views: SavedView[]) => void] {
  const [stored, setStored] = useState(() => ({ project, views: loadViews(localStorage, project) }));
  if (stored.project !== project) setStored({ project, views: loadViews(localStorage, project) });
  const set = useCallback(
    (views: SavedView[]) => {
      storeViews(localStorage, project, views);
      setStored({ project, views });
    },
    [project],
  );
  return [stored.project === project ? stored.views : [], set];
}

/** Table's and Timeline's Human · AI · All filter (0022), remembered between launches. */
function useForFilter(): [ForFilter, (f: ForFilter) => void] {
  const [f, setF] = useState<ForFilter>(() => parseForFilter(localStorage.getItem(FOR_KEY)));
  const set = useCallback((v: ForFilter) => {
    setF(v);
    localStorage.setItem(FOR_KEY, v);
  }, []);
  return [f, set];
}

export default function App() {
  const { state, error, at, refresh } = useQueues();
  const [queue, setQueue] = useState("all");
  const [view, setView] = useState<View>("board");
  const [pane, setPane] = useState<Pane>(null);
  const [showAllDone, setShowAllDone] = useState(false);
  const [query, setQuery] = useState<Query>(EMPTY_QUERY);
  const [views, setViews] = useSavedViews(queue);
  const searchRef = useRef<HTMLInputElement>(null);
  const [focusSearch, setFocusSearch] = useState(0);
  const [width, setWidth] = usePaneWidth();
  const [groupBy, setGroupBy] = useGroupBy();
  const [who, setWho] = useForFilter();
  // Where the manual is open in the Help tab; a "?" opens it at its topic, and the pane stays.
  const [helpAt, setHelpAt] = useState<ManualAt>({ file: "01-start-here.md", slug: "", n: 0 });
  const helpGo = useCallback((file: string, slug: string) => setHelpAt((a) => ({ file, slug, n: a.n + 1 })), []);
  const openHelp = useCallback(
    (topic: HelpTopic) => {
      setView("help");
      helpGo(HELP_TOPICS[topic].file, HELP_TOPICS[topic].slug);
    },
    [helpGo],
  );

  useEffect(() => {
    if (queue !== "all" && state && !state.queues.some((q) => q.name === queue)) setQueue("all");
  }, [state, queue]);

  const now = state?.now ?? Math.floor(Date.now() / 1000);
  const ctx = useMemo(() => dayContext(now), [now]);
  const inQueue = useMemo(() => (state?.tasks ?? []).filter((t) => queue === "all" || t.queue === queue), [state, queue]);
  const words = useMemo(() => queryWords(query.search), [query.search]);
  const searching = words.length > 0;
  const { index: searchIndex, error: indexError } = useSearchIndex(queue, inQueue, searching);
  const searchable = useMemo(() => (searching && searchIndex ? withSearch(inQueue, searchIndex) : inQueue), [inQueue, searchIndex, searching]);
  const tasks = useMemo(() => applyQuery(searchable, query, ctx), [searchable, query, ctx]);
  const problems = useMemo(() => (state?.problems ?? []).filter((p) => queue === "all" || p.startsWith(`${queue}:`)), [state, queue]);
  // Not problems: Shaping tasks with no vision check yet (0029), listed under them.
  const unchecked = useMemo(() => visionWarnings(inQueue), [inQueue]);
  // What needs the owner (0022): one count everywhere, and the Waiting tab lists the same tasks,
  // From the agents first, then the owner's own To do and Doing.
  const needs = needsYouCount(inQueue);
  const waiting = useMemo(() => {
    const b = humanBoard(inQueue);
    return [...b.lane, ...b.todo, ...b.doing];
  }, [inQueue]);
  // Goals and features aren't cards in the stage columns: they're the bands, and table rows. The
  // AI board is the agents' tasks; the owner's own are on the Human board above it.
  const work = useMemo(() => tasks.filter(isWork), [tasks]);
  const aiWork = useMemo(() => byWho(work, "ai"), [work]);
  // Each column in the Sort chips' order, in the plain board and inside every Group by band.
  const cols = useMemo(() => columns(aiWork, query.sort), [aiWork, query.sort]);
  const index = useMemo(() => indexTasks(state?.tasks ?? []), [state]);
  const boardGroups = useMemo(() => (groupBy === "none" ? [] : groupItems(aiWork, inQueue, groupBy, tasks)), [groupBy, aiWork, inQueue, tasks]);
  // Table and Timeline show both kinds, each human task with its You ribbon; Human · AI · All picks.
  const viewTasks = useMemo(() => byWho(tasks, who), [tasks, who]);
  const tableGroups = useMemo(() => (groupBy === "none" ? undefined : groupItems(viewTasks, inQueue, groupBy)), [groupBy, viewTasks, inQueue]);
  const whoFilters = view === "table" || view === "timeline";
  const shownCount = whoFilters ? viewTasks.length : tasks.length;
  const openKey = pane?.kind === "task" ? pane.key : null;
  const openTask = openKey ? (state?.tasks ?? []).find((t) => key(t) === openKey) ?? null : null;
  const allNotes = state?.backlog ?? [];
  const queueNotes = useMemo(() => backlogNotes(state?.backlog ?? [], queue), [state, queue]);
  const notes = useMemo(() => notesFor(queueNotes, query), [queueNotes, query]);
  // Notes show on the ungrouped board only (Group by bands hold tasks), so only it counts them
  // as hidden. Table and Timeline also count the tasks their Who chips hide.
  const notesShown = view === "board" && groupBy === "none";
  const hidden = inQueue.length - shownCount + (notesShown ? queueNotes.length - notes.length : 0);
  const openNoteKey = pane?.kind === "note" ? pane.key : null;
  const openNote = openNoteKey ? allNotes.find((n) => noteKey(n) === openNoteKey) ?? null : null;
  const showTask = (q: string, id: string) => {
    setView("board");
    void refresh().then(() => setPane({ kind: "task", key: `${q}/${id}` }));
  };
  const multi = queue === "all" && (state?.queues.length ?? 0) > 1;
  // Unsaved changes in a task's write-up editor keep the pane where it is: switching would unmount
  // the editor and lose the typing. A refused switch bumps `nudge`, and the editor says why.
  const unsaved = useRef(false);
  const [nudge, setNudge] = useState(0);
  const onDirty = useCallback((dirty: boolean) => {
    unsaved.current = dirty;
  }, []);
  const go = useCallback(
    (next: Pane) => {
      if (!mayLeave(unsaved.current, pane, next)) {
        setNudge((n) => n + 1);
        return;
      }
      setPane(next);
    },
    [pane],
  );
  const open = (t: Task) => go({ kind: "task", key: key(t) });

  useEffect(() => {
    const onKey = (e: KeyboardEvent) =>
      windowKey(e, {
        closePane: () => go(null),
        newTask: () => go({ kind: "new" }),
        // ⌘F: the search box, in the project view that's showing (Board, Table, Timeline or
        // Graph), on the board when another tab is.
        search: () => {
          setView(searchView);
          setFocusSearch((n) => n + 1);
        },
      });
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [go]);

  // After the view that holds the search box has rendered.
  useEffect(() => {
    if (focusSearch) focusSearchBox(searchRef.current);
  }, [focusSearch]);

  return (
    <OpenHelp.Provider value={openHelp}>
    <div className="app shell">
      <aside className="sidebar" aria-label="Projects">
        <div className="brand"><img src={logo} alt="" width={22} height={22} /> Muster</div>
        <ProjectList queues={state?.queues ?? []} tasks={state?.tasks ?? []} notes={allNotes} activity={state?.activity ?? []} now={now} current={queue} onPick={setQueue} />
        <button className="side-add" onClick={() => setView("projects")} title="New or existing project queue">+ Project</button>
      </aside>
      <div className="main-col">
      <header className="top">
        <button className="new" onClick={() => go({ kind: "new" })} disabled={!state?.queues.length} title="New task (⌘N)">+ New task</button>
        <nav className="views" aria-label="Views">
          <span className="view-group" role="group" aria-label="Project views">
            <button className={view === "board" ? "tab on" : "tab"} onClick={() => setView("board")}>Board</button>
            <button className={view === "table" ? "tab on" : "tab"} onClick={() => setView("table")}>Table</button>
            <button className={view === "timeline" ? "tab on" : "tab"} onClick={() => setView("timeline")}>Timeline</button>
            <button className={view === "graph" ? "tab on" : "tab"} onClick={() => setView("graph")} title="What waits on what">Graph</button>
          </span>
          <button className={view === "helper" ? "tab on" : "tab"} onClick={() => setView("helper")}>Helper</button>
          <button className={view === "waiting" ? "tab on" : "tab"} onClick={() => setView("waiting")} title="Your own tasks and what the agents wait on you for: the Human board's count">
            Waiting for you{needs ? <b className="count warn">{needs}</b> : null}
          </button>
          <button className={view === "activity" ? "tab on" : "tab"} onClick={() => setView("activity")}>Activity</button>
          <button className={view === "problems" ? "tab on" : "tab"} onClick={() => setView("problems")}>
            Problems{problems.length ? <b className="count">{problems.length}</b> : null}
            {unchecked.length ? <small className="count quiet" title="Shaping tasks with no vision check yet">{unchecked.length}</small> : null}
          </button>
          <button className={view === "projects" ? "tab on" : "tab"} onClick={() => setView("projects")}>Projects</button>
          <button className={view === "help" ? "tab on" : "tab"} onClick={() => setView("help")}>Help</button>
        </nav>
        <button className={pane?.kind === "appearance" ? "tab on" : "tab"} onClick={() => go(pane?.kind === "appearance" ? null : { kind: "appearance" })} title="Theme and accent">
          Appearance
        </button>
        <span className="live" title={error || "Updates every 2 seconds while the window is visible"}>
          {error ? <span className="err">Can't read the queues</span> : at ? <><i className="dot" /> live</> : "loading…"}
        </span>
      </header>

      <div className="workspace">
        <div className="content">
          {state && state.queues.length === 0 && (
            <p className="empty">No queues yet. Muster looks for <code>~/playground/*/tasks</code> folders, or add one under Projects.</p>
          )}

          {(view === "board" || view === "table" || view === "timeline" || view === "graph") && (
            <>
              <FilterBar
                tasks={inQueue}
                query={query}
                onQuery={setQuery}
                ctx={ctx}
                sortable={view === "board"}
                views={views}
                onViews={setViews}
                shown={shownCount}
                total={inQueue.length}
                hidden={hidden}
                estimate={estimateTotal(work)}
                arrange={
                  <>
                    {whoFilters && (
                      <span className="for-filter" role="group" aria-label="Who does it">
                        <span className="meta sep">Who</span>
                        {FOR_FILTERS.map((f) => (
                          <button key={f.id} className={who === f.id ? "chip on" : "chip"} aria-pressed={who === f.id} onClick={() => setWho(f.id)}>
                            {f.label}
                          </button>
                        ))}
                      </span>
                    )}
                    {view !== "timeline" && <GroupByChips value={groupBy} onChange={setGroupBy} />}
                  </>
                }
                searchRef={searchRef}
                help={<HelpButton topic={view} />}
              />
              {indexError && <p className="err small" role="alert">Search can't read the tasks' text: {indexError}</p>}
              {view === "table" && <TableView tasks={viewTasks} now={now} multi={multi} openKey={openKey} onOpen={open} groups={tableGroups} by={groupBy} words={words} />}
              {view === "graph" && <GraphView tasks={tasks} all={inQueue} multi={multi} openKey={openKey} onOpen={open} />}
              {view === "timeline" && <TimelineView tasks={viewTasks} now={now} multi={multi} openKey={openKey} onOpen={open} words={words} />}
              {/* The Human board sits above the AI board, both in view (docs/DESIGN.md). It shows
                  everything that needs the owner, whatever the filters, so nothing waiting is hidden. */}
              {view === "board" && <HumanBoard tasks={inQueue} multi={multi} openKey={openKey} onOpen={open} />}
              {view === "board" && <h2 className="board-label">AI board</h2>}
              {view === "board" && groupBy !== "none" && (
                <GroupedBoard
                  groups={boardGroups}
                  by={groupBy}
                  sort={query.sort}
                  multi={multi}
                  openKey={openKey}
                  onOpen={open}
                  renderCard={(t) => <Card t={t} now={now} multi={multi} parent={parentLine(t, index)} waitsOn={waitsOnYou(t, index)} onOpen={() => open(t)} active={openKey === key(t)} words={words} />}
                />
              )}
              {/* Backlog never folds: its quick add is always there. */}
              {view === "board" && groupBy === "none" && <main className="board" style={{ gridTemplateColumns: STAGES.map((s) => (s.id === "backlog" || (cols[s.id] ?? []).length ? "minmax(118px, 1fr)" : "44px")).join(" ") }}>
                {STAGES.map((c) => {
                  const list = cols[c.id] ?? [];
                  const shown = c.id === "done" && !showAllDone ? list.slice(0, 8) : list;
                  const backlog = c.id === "backlog";
                  const count = list.length + (backlog ? notes.length : 0);
                  return (
                    <section key={c.id} className={`column col-${c.id}${count || backlog ? "" : " empty"}`}>
                      <h2 title={c.role ? `${c.label}: ${c.role}` : c.label}>
                        {c.label} <small>{count}</small>
                        {c.role && <span className="role">{c.role}</span>}
                      </h2>
                      {backlog && state && state.queues.length > 0 && (
                        <QuickAdd queues={queue === "all" ? state.queues.map((q) => q.name) : [queue]} onAdded={() => void refresh()} />
                      )}
                      <div className="cards">
                        {backlog
                          ? backlogColumn(notes, list).map((item) =>
                              item.kind === "note" ? (
                                <NoteCard key={`note:${noteKey(item.note)}`} n={item.note} now={now} multi={multi} onOpen={() => setPane({ kind: "note", key: noteKey(item.note) })} active={openNoteKey === noteKey(item.note)} words={words} />
                              ) : (
                                <Card key={key(item.task)} t={item.task} now={now} multi={multi} parent={parentLine(item.task, index)} waitsOn={waitsOnYou(item.task, index)} onOpen={() => open(item.task)} active={openKey === key(item.task)} words={words} />
                              ),
                            )
                          : shown.map((t) => (
                              <Card key={key(t)} t={t} now={now} multi={multi} parent={parentLine(t, index)} waitsOn={waitsOnYou(t, index)} onOpen={() => open(t)} active={openKey === key(t)} words={words} />
                            ))}
                        {c.id === "done" && list.length > 8 && (
                          <button className="more" onClick={() => setShowAllDone((v) => !v)}>
                            {showAllDone ? "Show fewer" : `Show all ${list.length}`}
                          </button>
                        )}
                        {count === 0 && <p className="none">Nothing here</p>}
                      </div>
                    </section>
                  );
                })}
              </main>}
            </>
          )}

          {view === "waiting" && (
            <main className="list">
              {waiting.length === 0 && <p className="empty">Nothing is waiting for you.</p>}
              {waiting.map((t) => (
                <article key={key(t)} className={openKey === key(t) ? "waiting active" : "waiting"}>
                  <header>
                    <button className="link" onClick={() => open(t)}>
                      {isHuman(t) && <YouRibbon />} {multi && <span className="q">{t.queue}</span>} {t.id} · {t.title}
                    </button>
                  </header>
                  {isHuman(t) && <p>Yours to do: {humanColumn(t.stage) === "doing" ? "in Doing. Mark it Done when it is." : "in To do. Start it when you do."}</p>}
                  {t.questions.filter((q) => !q.ticked && q.escalated).map((q) => (
                    <p key={q.n ?? q.text}><b>Q{q.n}</b> {q.text}</p>
                  ))}
                  {t.blocked && t.waitingOn.toLowerCase().startsWith("owner") && <p>Blocked on you: {t.waitingOn}</p>}
                  {t.stage === "your-check" && <p>In Your check: accept it, or send it back with what's wrong.</p>}
                  <button onClick={() => open(t)}>{isHuman(t) ? "Open" : "Open to answer"}</button>
                </article>
              ))}
            </main>
          )}

          {view === "activity" && (
            <main className="list">
              <ul className="feed">
                {feed(state?.activity.filter((a) => queue === "all" || a.queue === queue) ?? [], inQueue).map((a, i) => (
                  <li key={i}>
                    <span className="when">{ago(now, a.at)}</span>
                    {multi && <span className="q">{a.queue}</span>}
                    <span className={`src ${a.source}`}>{a.source === "commit" ? "commit" : "changed"}</span>
                    <span className="what">{a.what}</span>
                  </li>
                ))}
              </ul>
            </main>
          )}

          {view === "problems" && (
            <main className="list">
              {problems.length === 0 ? <p className="empty">No problems found in the queues.</p> : <ul className="problems">{problems.map((p) => <li key={p}>{p}</li>)}</ul>}
              {unchecked.length > 0 && <ul className="problems warnings">{unchecked.map((w) => <li key={w}><b>Vision</b> {w}</li>)}</ul>}
            </main>
          )}

          {view === "helper" && state && state.queues.length > 0 && (
            <Suspense fallback={<p className="none">Loading…</p>}>
              <Helper queues={state.queues.map((q) => q.name)} initial={queue !== "all" ? queue : state.queues[0].name} onChanged={refresh} />
            </Suspense>
          )}

          {view === "help" && (
            <Suspense fallback={<p className="none">Loading…</p>}>
              <HelpView at={helpAt} onGo={helpGo} />
            </Suspense>
          )}

          {view === "projects" && <Projects queues={state?.queues ?? []} onChanged={refresh} />}
        </div>

        {pane && (
          <>
            <Splitter width={width} onWidth={setWidth} />
            <aside className="pane" style={{ width }} aria-label="Details">
              {pane.kind === "task" && openTask && (
                <Suspense fallback={<p className="none">Loading…</p>}>
                  <TaskPanel key={openKey} task={openTask} now={now} onClose={() => go(null)} onChanged={refresh} onDirty={onDirty} nudge={nudge} childTasks={childrenOf(openTask, index)} />
                </Suspense>
              )}
              {pane.kind === "task" && !openTask && <p className="none">That task is gone.</p>}
              {pane.kind === "note" && openNote && (
                <Suspense fallback={<p className="none">Loading…</p>}>
                  <NotePanel key={openNoteKey} note={openNote} now={now} onClose={() => setPane(null)} onChanged={refresh} onFiled={showTask} />
                </Suspense>
              )}
              {pane.kind === "note" && !openNote && <p className="none">That note is gone: filed as a task, or dropped.</p>}
              {pane.kind === "new" && state && state.queues.length > 0 && (
                <NewTask
                  queues={state.queues.map((q) => q.name)}
                  initial={queue !== "all" ? queue : state.queues[0].name}
                  onClose={() => setPane(null)}
                  onCreated={showTask}
                />
              )}
              {pane.kind === "appearance" && <Appearance onClose={() => setPane(null)} />}
            </aside>
          </>
        )}
      </div>
      </div>
    </div>
    </OpenHelp.Provider>
  );
}

/** The drag handle between the content and the pane. */
function Splitter({ width, onWidth }: { width: number; onWidth: (w: number) => void }) {
  return (
    <div
      className="splitter"
      role="separator"
      aria-orientation="vertical"
      aria-valuenow={width}
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "ArrowLeft") onWidth(width + 40);
        if (e.key === "ArrowRight") onWidth(width - 40);
      }}
      onMouseDown={(e) => {
        e.preventDefault();
        const startX = e.clientX;
        const start = width;
        const move = (ev: MouseEvent) => onWidth(start + (startX - ev.clientX));
        const up = () => {
          window.removeEventListener("mousemove", move);
          window.removeEventListener("mouseup", up);
        };
        window.addEventListener("mousemove", move);
        window.addEventListener("mouseup", up);
      }}
      title="Drag to resize"
    />
  );
}

/** A board card. `waitsOn`: the owner's own tasks it depends on that aren't done (0022), shown as
 *  blocked by them until they are. */
function Card({ t, now, multi, parent, waitsOn = [], onOpen, active, words }: { t: Task; now: number; multi: boolean; parent: string; waitsOn?: string[]; onOpen: () => void; active: boolean; words: string[] }) {
  const priority = t.priority || "normal";
  return (
    <button className={cardClass(t, active)} onClick={onOpen}>
      {isHuman(t) && <YouRibbon />}
      <CardBand t={t} />
      <span className="card-top">
        {multi && <span className="q">{t.queue}</span>}
        <span className="id"><HlId id={t.id} words={words} /></span>
        <LiveDot t={t} />
        <span className={`kind ${t.kind}`}>{t.kind}</span>
        <span className={`pri pri-${priority}`} title={`Priority: ${priority}`}>{priority}</span>
      </span>
      <span className="title">{t.title ? <Hl text={t.title} words={words} /> : "(no title)"}</span>
      {parent && <span className="parent-line">{parent}</span>}
      {t.tags.length > 0 && (
        <span className="tags">
          {t.tags.map((g) => <span key={g} className="tag">#<Hl text={g} words={words} /></span>)}
        </span>
      )}
      <CardStatus t={t} now={now} />
      <span className="card-bottom">
        {t.blocked && <span className="badge blocked">blocked{t.waitingOn ? `: ${t.waitingOn}` : ""}</span>}
        {waitsOn.length > 0 && <span className="badge waits-you" title="It can't start until your task is done">blocked by yours: {waitsOn.join(", ")}</span>}
        {t.waitingForOwner && <span className="badge warn">needs you</span>}
        {t.openQuestions > 0 && <span className="badge">{t.openQuestions} open Q</span>}
        {t.attachments.length > 0 && <span className="badge">{t.attachments.length} file{t.attachments.length === 1 ? "" : "s"}</span>}
        {estimateText(t) && <span className="badge est" title="Estimated agent time, test time and tokens">{estimateText(t)}</span>}
        <span className="age">{ago(now, t.modified)}</span>
      </span>
    </button>
  );
}
