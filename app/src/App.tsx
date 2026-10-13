import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "./api";
import { mayLeave } from "./writeUp";
import { ago, columns, estimateTotal, feed, key, STAGES } from "./lib";
import { keyIndex } from "./taskKey";
import type { Note, State, Task } from "./types";
import { backlogColumn, backlogNotes, hiddenCount, NoteCard, noteKey, QuickAdd } from "./Backlog";
import ProjectList from "./ProjectList";
import NewTask from "./NewTask";
import Card from "./Card";
import { GraphView } from "./GraphView";
import { TableView, TimelineView } from "./Views";
import { GroupByChips, GroupedBoard } from "./Groups";
import { childrenOf, GROUP_KEY, type GroupBy, groupItems, indexTasks, isWork, parentLine, parseGroupBy } from "./levels";
import { visionWarnings } from "./Vision";
import FilterBar from "./FilterBar";
import { applyQuery, dayContext, EMPTY_QUERY, loadViews, notesFor, storeViews, type Query, type SavedView } from "./filters";
import { indexStamp, needsIndex, queryWords, withSearch, type IndexStamp, type SearchIndex } from "./search";
import { focusSearchBox, searchView, windowKey } from "./keys";
import WaitingList from "./WaitingList";
import EffortGroups, { useAcceptResults } from "./EffortGroups";
import type { NamedView } from "./yourCheck";
import { byWho, FOR_FILTERS, FOR_KEY, type ForFilter, needsYouCount, parseForFilter, waitingForYou, whoFiltersOn, whoShows } from "./human";
import { HelpButton, OpenHelp } from "./HelpButton";
import { HELP_TOPICS, type HelpTopic, type ManualAt } from "./helpTopics";
import { Machine, machineOf, type SettingsGroup } from "./settingsData";
import logo from "./logo.png";

// The task panel and its Markdown renderer load on first open, in their own chunk.
const TaskPanel = lazy(() => import("./TaskPanel"));
const NotePanel = lazy(() => import("./NotePanel"));
const Helper = lazy(() => import("./Helper"));
const HelpView = lazy(() => import("./Help"));
const SettingsView = lazy(() => import("./Settings"));
type View = "board" | "table" | "timeline" | "graph" | "helper" | "waiting" | "activity" | "problems" | "help" | "settings";
// One docked pane beside the content, never over it (the owner wants everything in view).
type Pane = { kind: "task"; key: string } | { kind: "note"; key: string } | { kind: "new" } | null;
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

/** The Who chips (All · You · Agents, 0022) of the Board, Table and Timeline, remembered between
 *  launches. */
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
  // Settings (0036): the group it shows; the sidebar's + Project and Your check's view buttons open it at theirs.
  const [settingsAt, setSettingsAt] = useState<SettingsGroup>("appearance");
  const openSettings = (g: SettingsGroup) => {
    setSettingsAt(g);
    setView("settings");
  };
  // This machine's lease and LIVE window, from the last poll, for the cards and the task pane.
  const { leaseMinutes, liveMinutes } = machineOf(state);
  const machine = useMemo(() => ({ leaseMinutes, liveMinutes }), [leaseMinutes, liveMinutes]);

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
  // what the agents wait on the owner for first, then the owner's own To do and Doing.
  const needs = needsYouCount(inQueue);
  const waiting = useMemo(() => waitingForYou(inQueue), [inQueue]);
  // The Board, Table and Timeline show both kinds, the owner's own tasks hatched with the You
  // ribbon; the Who chips (All · You · Agents) pick.
  // Backlog notes show under every Who: they're nobody's yet.
  const queueNotes = useMemo(() => backlogNotes(state?.backlog ?? [], queue), [state, queue]);
  const { tasks: viewTasks, notes } = useMemo(() => whoShows(tasks, notesFor(queueNotes, query), who), [tasks, queueNotes, query, who]);
  // Goals and features aren't cards in the stage columns: they're the bands, and table rows.
  const work = useMemo(() => tasks.filter(isWork), [tasks]);
  const boardWork = useMemo(() => byWho(work, who), [work, who]);
  // Each column in the Sort chips' order, in the plain board and inside every Group by band.
  const cols = useMemo(() => columns(boardWork, query.sort), [boardWork, query.sort]);
  const index = useMemo(() => indexTasks(state?.tasks ?? []), [state]);
  const boardGroups = useMemo(() => (groupBy === "none" ? [] : groupItems(boardWork, inQueue, groupBy, viewTasks)), [groupBy, boardWork, inQueue, viewTasks]);
  const tableGroups = useMemo(() => (groupBy === "none" ? undefined : groupItems(viewTasks, inQueue, groupBy)), [groupBy, viewTasks, inQueue]);
  const whoFilters = whoFiltersOn(view);
  const shownCount = whoFilters ? viewTasks.length : tasks.length;
  const openKey = pane?.kind === "task" ? pane.key : null;
  const openTask = openKey ? (state?.tasks ?? []).find((t) => key(t) === openKey) ?? null : null;
  // Each project's prefix to its queue (0033), for keys in task text: a new map only when a prefix
  // or a name changes (what `prefixes` sums up), so the 2 s poll doesn't render the text again.
  const prefixes = (state?.queues ?? []).map((q) => `${q.prefix ?? ""} ${q.name}`).join("\n");
  const keys = useMemo(() => keyIndex(state?.queues ?? []), [prefixes]);
  const allNotes = state?.backlog ?? [];
  // Notes show on the board, grouped or not (Group by draws them in its Backlog band, 0046), so
  // only it counts them as hidden. The Board, Table and Timeline also count the tasks their Who
  // chips hide.
  const hidden = hiddenCount(view, { all: inQueue.length, shown: shownCount }, { all: queueNotes.length, shown: notes.length });
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
  const boardResult = useAcceptResults().has("board");
  const card = (t: Task) => (
    <Card key={key(t)} t={t} now={now} multi={multi} parent={parentLine(t, index)} onOpen={() => open(t)} active={openKey === key(t)} words={words} />
  );
  const noteCard = (n: Note) => (
    <NoteCard key={`note:${noteKey(n)}`} n={n} now={now} multi={multi} onOpen={() => setPane({ kind: "note", key: noteKey(n) })} active={openNoteKey === noteKey(n)} words={words} />
  );
  // The Backlog's one-line capture, on the plain board and in the grouped board's Backlog band.
  const quickAdd = state && state.queues.length > 0 ? <QuickAdd queues={queue === "all" ? state.queues.map((q) => q.name) : [queue]} onAdded={() => void refresh()} /> : null;
  // A view an owner criterion names, from its button in Your check (0043). Each is a tab, and the
  // task stays open beside it; Appearance and Projects are groups of Settings (0036).
  const openView = (v: NamedView) => {
    if (v === "appearance" || v === "projects") openSettings(v);
    else setView(v);
  };

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
    <Machine.Provider value={machine}>
    <div className="app shell">
      <aside className="sidebar" aria-label="Projects">
        <div className="brand"><img src={logo} alt="" width={22} height={22} /> Muster</div>
        <ProjectList queues={state?.queues ?? []} tasks={state?.tasks ?? []} notes={allNotes} activity={state?.activity ?? []} now={now} current={queue} onPick={setQueue} />
        <button className="side-add" onClick={() => openSettings("projects")} title="New or existing project queue, in Settings">+ Project</button>
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
          <button className={view === "waiting" ? "tab on" : "tab"} onClick={() => setView("waiting")} title="Your own tasks to do and doing, and what the agents wait on you for: the needs-you count">
            Waiting for you{needs ? <b className="count warn">{needs}</b> : null}
          </button>
          <button className={view === "activity" ? "tab on" : "tab"} onClick={() => setView("activity")}>Activity</button>
          <button className={view === "problems" ? "tab on" : "tab"} onClick={() => setView("problems")}>
            Problems{problems.length ? <b className="count">{problems.length}</b> : null}
            {unchecked.length ? <small className="count quiet" title="Shaping tasks with no vision check yet">{unchecked.length}</small> : null}
          </button>
          <button className={view === "help" ? "tab on" : "tab"} onClick={() => setView("help")}>Help</button>
          <button className={view === "settings" ? "tab on" : "tab"} onClick={() => setView("settings")} title="Appearance, projects, agents and about">Settings</button>
        </nav>
        <span className="live" title={error || "Updates every 2 seconds while the window is visible"}>
          {error ? <span className="err">Can't read the queues</span> : at ? <><i className="dot" /> live</> : "loading…"}
        </span>
      </header>

      <div className="workspace">
        <div className="content">
          {state && state.queues.length === 0 && (
            <p className="empty">No queues yet. Muster looks for <code>~/playground/&lt;project&gt;/tasks</code> folders, and one level deeper, or add one in Settings, under Projects.</p>
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
              {view === "board" && groupBy !== "none" && (
                <GroupedBoard
                  groups={boardGroups}
                  by={groupBy}
                  sort={query.sort}
                  multi={multi}
                  openKey={openKey}
                  onOpen={open}
                  renderCard={card}
                  onChanged={() => void refresh()}
                  backlog={quickAdd ? { notes, renderNote: noteCard, quickAdd } : undefined}
                />
              )}
              {/* Backlog never folds: its quick add is always there. */}
              {view === "board" && groupBy === "none" && <main className="board" style={{ gridTemplateColumns: STAGES.map((s) => (s.id === "backlog" || (cols[s.id] ?? []).length || (s.id === "your-check" && boardResult) ? "minmax(118px, 1fr)" : "44px")).join(" ") }}>
                {STAGES.map((c) => {
                  const list = cols[c.id] ?? [];
                  const shown = c.id === "done" && !showAllDone ? list.slice(0, 8) : list;
                  const backlog = c.id === "backlog";
                  const count = list.length + (backlog ? notes.length : 0);
                  // Your check stays open while Accept all's result line has something to say (0043).
                  const kept = c.id === "your-check" && boardResult;
                  return (
                    <section key={c.id} className={`column col-${c.id}${count || backlog || kept ? "" : " empty"}`}>
                      <h2 title={c.role ? `${c.label}: ${c.role}` : c.label}>
                        {c.label} <small>{count}</small>
                        {c.role && <span className="role">{c.role}</span>}
                      </h2>
                      {backlog && quickAdd}
                      <div className="cards">
                        {backlog
                          ? backlogColumn(notes, list).map((item) =>
                              item.kind === "note" ? (
                                noteCard(item.note)
                              ) : (
                                <Card key={key(item.task)} t={item.task} now={now} multi={multi} parent={parentLine(item.task, index)} onOpen={() => open(item.task)} active={openKey === key(item.task)} words={words} />
                              ),
                            )
                          : c.id === "your-check" && (shown.length > 0 || kept)
                            ? <EffortGroups tasks={shown} place="board" renderCard={card} onChanged={() => void refresh()} />
                            : shown.map(card)}
                        {c.id === "done" && list.length > 8 && (
                          <button className="more" onClick={() => setShowAllDone((v) => !v)}>
                            {showAllDone ? "Show fewer" : `Show all ${list.length}`}
                          </button>
                        )}
                        {count === 0 && !kept && <p className="none">Nothing here</p>}
                      </div>
                    </section>
                  );
                })}
              </main>}
            </>
          )}

          {view === "waiting" && <WaitingList tasks={waiting} multi={multi} openKey={openKey} onOpen={open} onChanged={() => void refresh()} />}

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

          {view === "settings" && (
            <Suspense fallback={<p className="none">Loading…</p>}>
              <SettingsView
                group={settingsAt}
                onGroup={setSettingsAt}
                queues={state?.queues ?? []}
                onChanged={refresh}
              />
            </Suspense>
          )}
        </div>

        {pane && (
          <>
            <Splitter width={width} onWidth={setWidth} />
            <aside className="pane" style={{ width }} aria-label="Details">
              {pane.kind === "task" && openTask && (
                <Suspense fallback={<p className="none">Loading…</p>}>
                  <TaskPanel key={openKey} task={openTask} now={now} onClose={() => go(null)} onChanged={refresh} onDirty={onDirty} nudge={nudge} childTasks={childrenOf(openTask, index)} onOpenTask={(id) => go({ kind: "task", key: key({ queue: openTask.queue, id }) })} onOpenView={openView} keys={keys} onOpenAt={(queue, id) => go({ kind: "task", key: key({ queue, id }) })} />
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
            </aside>
          </>
        )}
      </div>
      </div>
    </div>
    </Machine.Provider>
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
