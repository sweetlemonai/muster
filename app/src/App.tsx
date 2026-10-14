import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "./api";
import { Button, Pill, Splitter } from "./ui";
import { mayLeave } from "./writeUp";
import { ago, columns, estimateTotal, feed, key, STAGES } from "./lib";
import { keyIndex } from "./taskKey";
import { Labels, labelsOf } from "./labels";
import type { Note, State, Task } from "./types";
import { backlogColumn, backlogNotes, hiddenCount, NoteCard, noteKey, QuickAdd } from "./Backlog";
import ProjectList from "./ProjectList";
import ProjectName from "./ProjectName";
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
import { lessonsWaiting } from "./Lessons";
import Recent from "./Recent";
import { clampPane, clampSidebar, layoutWidth, PANE_DEFAULT, paneRange, railBeside, SIDEBAR, SIDEBAR_NARROW, sidebarRange } from "./paneSize";
import { PANE_KEY, SIDEBAR_KEY, useSplitWidth } from "./splitter";
import { ZOOM_EVENT } from "./zoom";
import EffortGroups, { useAcceptResults } from "./EffortGroups";
import { CheckCount, ColumnLine, OverLine } from "./CheckLimit";
import { overLimit } from "./yourCheckLimit";
import type { NamedView } from "./yourCheck";
import { byWho, FOR_FILTERS, FOR_KEY, type ForFilter, needsYouCount, parseForFilter, waitingForYou, whoFiltersOn, whoShows } from "./human";
import { HelpButton, OpenHelp } from "./HelpButton";
import { HELP_TOPICS, type HelpTopic, type ManualAt } from "./helpTopics";
import { Machine, machineOf, type SettingsGroup } from "./settingsData";
import { columnFold, loadMode, look, Mode, pickMode, storeMode, toggleFocus, type ColumnFolds, type ModeState } from "./modes";
import { blockedCount, ChoiceChips, ColumnHead, ModeChip } from "./ModeViews";
import ColumnCards from "./ColumnCards";
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

/** The window's width in the page's pixels, under the ⌘+ zoom (paneSize.ts `layoutWidth`). */
const pageWidth = () => layoutWidth(window.innerWidth, document.documentElement.style.getPropertyValue("zoom"));

/** The window's width in the page's pixels, kept current: the window resizing and the Text size
 *  changing both draw the sidebar and the pane again, fitted to what's left. */
function usePageWidth(): number {
  const [w, setW] = useState(pageWidth);
  useEffect(() => {
    const update = () => setW(pageWidth());
    window.addEventListener("resize", update);
    window.addEventListener(ZOOM_EVENT, update);
    return () => {
      window.removeEventListener("resize", update);
      window.removeEventListener(ZOOM_EVENT, update);
    };
  }, []);
  return w;
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

/** The display mode (0038) and the one ⌘. goes back to, remembered between launches. */
function useDisplayMode(): [ModeState, (f: (s: ModeState) => ModeState) => void] {
  const [s, setS] = useState<ModeState>(() => loadMode(localStorage));
  useEffect(() => storeMode(localStorage, s), [s]);
  return [s, setS];
}

const NO_COLUMNS: ReadonlySet<string> = new Set();

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
  const [groupBy, setGroupBy] = useGroupBy();
  const [who, setWho] = useForFilter();
  const [modes, setModes] = useDisplayMode();
  // The sidebar's and the pane's widths (0058), the owner's or the default, fitted to the window
  // each render (and each resize or Text size change), so a narrower window still leaves the board
  // its room. On the board, Recently finished (0051) goes under the board when the row can't hold
  // it beside (paneSize.ts).
  const win = usePageWidth();
  const [sideAsked, setSideAsked, resetSide] = useSplitWidth(SIDEBAR_KEY);
  const [paneAsked, setPaneAsked, resetPane] = useSplitWidth(PANE_KEY);
  const sidebarLook = look("sidebar", modes.mode);
  const sidebarWidth = clampSidebar(sideAsked ?? (sidebarLook === "narrow" ? SIDEBAR_NARROW : SIDEBAR), win);
  const paneWidth = clampPane(paneAsked ?? PANE_DEFAULT, win, sidebarWidth);
  const railSide = railBeside(win, pane ? paneWidth : 0, sidebarWidth);
  // The columns Focus folds that a click opened, and the filter bar ⌘F brought out in Focus: both
  // until the mode changes.
  const [opened, setOpened] = useState<ReadonlySet<string>>(NO_COLUMNS);
  const [barOut, setBarOut] = useState(false);
  const changeMode = useCallback(
    (f: (s: ModeState) => ModeState) => {
      setModes(f);
      setOpened(NO_COLUMNS);
      setBarOut(false);
    },
    [setModes],
  );
  const columnFoldOf: ColumnFolds = {
    of: (id) => columnFold(look("columns", modes.mode), id, opened),
    toggle: (id) =>
      setOpened((o) => {
        const n = new Set(o);
        if (!n.delete(id)) n.add(id);
        return n;
      }),
  };
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
  // Lessons from reviews waiting for the owner's yes (0027) count too, and lead the Waiting list.
  const needs = needsYouCount(inQueue) + lessonsWaiting(state?.queues ?? [], queue);
  const waiting = useMemo(() => waitingForYou(inQueue), [inQueue]);
  // The projects shown with more in Your check than their limit (0050): the board says so.
  const overCheck = useMemo(() => overLimit(state?.queues ?? [], queue), [state, queue]);
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
  // Each project's label (0044) by its name, a new map only when one changes, for everywhere a
  // project is named; the window's title says the one shown.
  const labelText = (state?.queues ?? []).map((q) => `${q.name}\n${q.label ?? ""}`).join("\n");
  const labels = useMemo(() => labelsOf(state?.queues ?? []), [labelText]);
  useEffect(() => {
    api.showProjectTitle(queue).catch((e: unknown) => console.warn("Can't set the window's title:", e));
  }, [queue, labelText]);
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

  // ⌘F: the search box, in the project view that's showing (Board, Table, Timeline or Graph), on
  // the board when another tab is; in Focus it brings the filter bar out first.
  const summonSearch = useCallback(() => {
    setView(searchView);
    setBarOut(true);
    setFocusSearch((n) => n + 1);
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) =>
      windowKey(e, {
        closePane: () => go(null),
        newTask: () => go({ kind: "new" }),
        search: summonSearch,
        focusMode: () => changeMode(toggleFocus),
      });
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [go, summonSearch, changeMode]);

  // After the view that holds the search box has rendered.
  useEffect(() => {
    if (focusSearch) focusSearchBox(searchRef.current);
  }, [focusSearch]);

  return (
    <OpenHelp.Provider value={openHelp}>
    <Machine.Provider value={machine}>
    <Mode.Provider value={modes.mode}>
    <Labels.Provider value={labels}>
    <div className="app shell">
      <aside className="sidebar" aria-label="Projects" data-sidebar={sidebarLook} style={{ width: sidebarWidth }}>
        <div className="brand"><img src={logo} alt="" width={22} height={22} /> Muster</div>
        <ProjectList queues={state?.queues ?? []} tasks={state?.tasks ?? []} notes={allNotes} activity={state?.activity ?? []} now={now} current={queue} onPick={setQueue} onLabel={(k, label) => api.setProjectLabel(k, label).then(refresh)} />
        <button className="side-add" onClick={() => openSettings("projects")} title="New or existing project queue, in Settings">+ Project</button>
      </aside>
      <Splitter {...sidebarRange(win)} width={sidebarWidth} side="left" label="Projects width" onWidth={(w) => setSideAsked(clampSidebar(w, pageWidth()))} onReset={resetSide} />
      <div className="main-col">
      <header className="top">
        <Button variant="primary" className="new" onClick={() => go({ kind: "new" })} disabled={!state?.queues.length} title="New task (⌘N)">+ New task</Button>
        <nav className="views" aria-label="Views">
          <span className="view-group" role="group" aria-label="Project views">
            <Pill on={view === "board"} onClick={() => setView("board")}>Board</Pill>
            <Pill on={view === "table"} onClick={() => setView("table")}>Table</Pill>
            <Pill on={view === "timeline"} onClick={() => setView("timeline")}>Timeline</Pill>
            <Pill on={view === "graph"} onClick={() => setView("graph")} title="What waits on what">Graph</Pill>
          </span>
          <Pill on={view === "helper"} onClick={() => setView("helper")}>Helper</Pill>
          <Pill on={view === "waiting"} onClick={() => setView("waiting")} title="Your own tasks to do and doing, and what the agents wait on you for: the needs-you count">
            Waiting for you{needs ? <b className="count warn">{needs}</b> : null}
          </Pill>
          <Pill on={view === "activity"} onClick={() => setView("activity")}>Activity</Pill>
          <Pill on={view === "problems"} onClick={() => setView("problems")}>
            Problems{problems.length ? <b className="count">{problems.length}</b> : null}
            {unchecked.length ? <small className="count quiet" title="Shaping tasks with no vision check yet">{unchecked.length}</small> : null}
          </Pill>
          <Pill on={view === "help"} onClick={() => setView("help")}>Help</Pill>
          <Pill on={view === "settings"} onClick={() => setView("settings")} title="Appearance, projects, agents and about">Settings</Pill>
        </nav>
        <ModeChip mode={modes.mode} onOpen={() => openSettings("display")} />
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
                    {whoFilters && <ChoiceChips label="Who" className="for-filter" aria="Who does it" metaClass="meta sep" value={who} options={FOR_FILTERS} onPick={setWho} />}
                    {view !== "timeline" && <GroupByChips value={groupBy} onChange={setGroupBy} />}
                  </>
                }
                searchRef={searchRef}
                help={<HelpButton topic={view} />}
                out={barOut}
                onSummon={summonSearch}
                onHide={() => setBarOut(false)}
              />
              {indexError && <p className="err small" role="alert">Search can't read the tasks' text: {indexError}</p>}
              {view === "table" && <TableView tasks={viewTasks} now={now} multi={multi} openKey={openKey} onOpen={open} groups={tableGroups} by={groupBy} words={words} />}
              {view === "graph" && <GraphView tasks={tasks} all={inQueue} multi={multi} openKey={openKey} onOpen={open} />}
              {view === "timeline" && <TimelineView tasks={viewTasks} now={now} multi={multi} openKey={openKey} onOpen={open} words={words} />}
              {/* Recently finished (0051) is docked at the board's right edge, grouped or not; under it when the row is too narrow for both. */}
              {view === "board" && <OverLine over={overCheck} queue={queue} />}
              {view === "board" && <div className={railSide ? "board-row" : "board-row stacked"}>
              {groupBy !== "none" && (
                <GroupedBoard
                  groups={boardGroups}
                  by={groupBy}
                  sort={query.sort}
                  multi={multi}
                  openKey={openKey}
                  onOpen={open}
                  renderCard={card}
                  onChanged={() => void refresh()}
                  limit={{ over: overCheck, queue }}
                  backlog={quickAdd ? { notes, renderNote: noteCard, quickAdd } : undefined}
                  fold={columnFoldOf}
                />
              )}
              {/* Backlog never folds, its quick add is always there, except in Focus (0038), which folds
                  every column but Building, Testing and Your check to a strip that opens with a click. */}
              {groupBy === "none" && <main className="board" style={{ gridTemplateColumns: STAGES.map((s) => ((s.id === "backlog" || (cols[s.id] ?? []).length || (s.id === "your-check" && (boardResult || overCheck.length > 0))) && columnFoldOf.of(s.id) !== "folded" ? "minmax(118px, 1fr)" : "44px")).join(" ") }}>
                {STAGES.map((c) => {
                  const list = cols[c.id] ?? [];
                  const shown = c.id === "done" && !showAllDone ? list.slice(0, 8) : list;
                  const backlog = c.id === "backlog";
                  const count = list.length + (backlog ? notes.length : 0);
                  // Your check stays open while Accept all's result line has something to say (0043).
                  const kept = c.id === "your-check" && boardResult;
                  const fold = columnFoldOf.of(c.id);
                  // Over its limit (0050), Your check stays open to say so.
                  const over = c.id === "your-check" && overCheck.length > 0;
                  return (
                    <section key={c.id} className={`column col-${c.id}${fold === "folded" ? " folded" : count || backlog || kept || over ? "" : " empty"}`}>
                      <ColumnHead stage={c} count={count} countNode={c.id === "your-check" ? <CheckCount count={count} over={overCheck} queue={queue} /> : undefined} needs={needsYouCount(list)} blocked={blockedCount(list)} fold={fold} onFold={() => columnFoldOf.toggle(c.id)}>
                        {c.role && <span className="role">{c.role}</span>}
                      </ColumnHead>
                      {c.id === "your-check" && <ColumnLine over={overCheck} queue={queue} />}
                      {backlog && fold !== "folded" && quickAdd}
                      <ColumnCards>
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
                      </ColumnCards>
                    </section>
                  );
                })}
              </main>}
              <Recent key={`${queue}/${modes.mode}`} tasks={inQueue} now={now} multi={multi} openKey={openKey} onOpen={open} />
              </div>}
            </>
          )}

          {view === "waiting" && <WaitingList tasks={waiting} multi={multi} openKey={openKey} onOpen={open} onChanged={() => void refresh()} now={now} queues={state?.queues ?? []} queue={queue} />}

          {view === "activity" && (
            <main className="list">
              <ul className="feed">
                {feed(state?.activity.filter((a) => queue === "all" || a.queue === queue) ?? [], inQueue).map((a, i) => (
                  <li key={i}>
                    <span className="when">{ago(now, a.at)}</span>
                    {multi && <ProjectName name={a.queue} />}
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
                mode={modes.mode}
                onMode={(m) => changeMode((s) => pickMode(s, m))}
              />
            </Suspense>
          )}
        </div>

        {pane && (
          <>
            <Splitter {...paneRange(win, sidebarWidth)} width={paneWidth} side="right" label="Details width" onWidth={(w) => setPaneAsked(clampPane(w, pageWidth(), sidebarWidth))} onReset={resetPane} />
            <aside className="pane" style={{ width: paneWidth }} aria-label="Details">
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
                  prefixes={Object.fromEntries(state.queues.map((q) => [q.name, q.prefix ?? ""]))}
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
    </Labels.Provider>
    </Mode.Provider>
    </Machine.Provider>
    </OpenHelp.Provider>
  );
}
