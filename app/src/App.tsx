import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "./api";
import { Trans, useT } from "./i18n";
import { Button, Pill, Splitter } from "./ui";
import { mayLeave } from "./writeUp";
import { ago, columns, estimateTotal, feed, key, STAGES } from "./lib";
import { keyIndex } from "./taskKey";
import { Labels, labelsOf } from "./labels";
import type { Note, State, Task } from "./types";
import { backlogColumn, backlogNotes, hiddenCount, NoteCard, noteKey, QuickAdd } from "./Backlog";
import ProjectList, { projectEdits } from "./ProjectList";
import { coloursOf, ProjectColours } from "./projectColours";
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
import { PANE_KEY, PANE_LEGACY_KEY, SIDEBAR_KEY, useSplitWidth } from "./splitter";
import { ZOOM_EVENT } from "./zoom";
import EffortGroups, { useAcceptResults } from "./EffortGroups";
import { CheckCount, ColumnLine, OverLine } from "./CheckLimit";
import { overLimit } from "./yourCheckLimit";
import type { NamedView } from "./yourCheck";
import { byWho, FOR_FILTERS, FOR_KEY, type ForFilter, needsYouCount, parseForFilter, waitingForYou, whoFiltersOn, whoShows } from "./human";
import { HelpButton, OpenHelp } from "./HelpButton";
import { HELP_TOPICS, type HelpTopic, type ManualAt } from "./helpTopics";
import { Machine, machineOf, RoleNamesContext, type SettingsGroup } from "./settingsData";
import { bandFold, columnClass, loadMode, loadPicks, plainBoard, look, Mode, pickColumn, pickMode, RAIL_ID, storeMode, storePicks, toggleBand, toggleFocus, type BoardLooks, type ColumnFolds, type ColumnPicks, type ModePicks, type ModeState } from "./modes";
import { blockedCount, ChoiceChips, ColumnHead, ModeChip } from "./ModeViews";
import ColumnCards from "./ColumnCards";
import ColumnRow, { placeCount, type PlaceCounts, type RowChip, rowChipsOf } from "./ColumnRow";
import { PausedColumn } from "./PausedLane";
import { sessionKey } from "./sessionsData";
import { type KindFilter, loadKind, storeKind, weekCount } from "./recentData";
import OldPausesOffer from "./OldPauses";
import FeatureGrid from "./FeatureGrid";
import { gridRows, headerLine, type Layout, LAYOUTS, loadLayout, storeLayout } from "./featureGridData";
import logoDark from "./logo-dark.svg";
import logoLight from "./logo-light.svg";

// The task panel and its Markdown renderer load on first open, in their own chunk.
const TaskPanel = lazy(() => import("./TaskPanel"));
const NotePanel = lazy(() => import("./NotePanel"));
const Helper = lazy(() => import("./Helper"));
const HelpView = lazy(() => import("./Help"));
const SettingsView = lazy(() => import("./Settings"));
// The Sessions view and its recap (0098) load on first open too.
const SessionsView = lazy(() => import("./Sessions"));
const SessionRecap = lazy(() => import("./SessionRecap"));
type View = "board" | "table" | "timeline" | "graph" | "sessions" | "helper" | "waiting" | "activity" | "problems" | "help" | "settings";
// One docked pane beside the content, never over it (the owner wants everything in view).
type Pane = { kind: "task"; key: string } | { kind: "note"; key: string } | { kind: "new" } | { kind: "session"; key: string; account: string; id: string } | null;
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

/** The board's layout (mu-143), Columns or Feature grid, per project ("All" is a project here), on
 *  this Mac beside the project's saved views. */
function useLayout(project: string): [Layout, (l: Layout) => void] {
  const [stored, setStored] = useState(() => ({ project, layout: loadLayout(localStorage, project) }));
  if (stored.project !== project) setStored({ project, layout: loadLayout(localStorage, project) });
  const set = useCallback(
    (layout: Layout) => {
      storeLayout(localStorage, project, layout);
      setStored({ project, layout });
    },
    [project],
  );
  return [stored.project === project ? stored.layout : loadLayout(localStorage, project), set];
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

/** The columns the owner opened or folded, per display mode, remembered on this Mac (mu-84). */
function useColumnPicks(): [ColumnPicks, (f: (p: ColumnPicks) => ColumnPicks) => void] {
  const [p, setP] = useState<ColumnPicks>(() => loadPicks(localStorage));
  useEffect(() => storePicks(localStorage, p), [p]);
  return [p, setP];
}

const NO_PICKS: ModePicks = {};

export default function App() {
  const t = useT("shell");
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
  const [layout, setLayout] = useLayout(queue);
  // The Feature grid (mu-143) in place of the columns, on the board only.
  const grid = view === "board" && layout === "grid";
  // The grid's Only what needs me, for this window's life: it dims, never hides.
  const [mine, setMine] = useState(false);
  const [who, setWho] = useForFilter();
  const [modes, setModes] = useDisplayMode();
  // The sidebar's and the pane's widths (0058), the owner's or the default, fitted to the window
  // each render (and each resize or Text size change), so a narrower window still leaves the board
  // its room. On the board, Recently finished (0051) goes under the board when the row can't hold
  // it beside (paneSize.ts).
  const win = usePageWidth();
  const sideSplit = useSplitWidth(SIDEBAR_KEY);
  const paneSplit = useSplitWidth(PANE_KEY, PANE_LEGACY_KEY);
  const sidebarLook = look("sidebar", modes.mode);
  const sidebarWidth = clampSidebar(sideSplit.width ?? (sidebarLook === "narrow" ? SIDEBAR_NARROW : SIDEBAR), win);
  const paneWidth = clampPane(paneSplit.width ?? PANE_DEFAULT, win, sidebarWidth);
  const railSide = railBeside(win, pane ? paneWidth : 0, sidebarWidth);
  // The filter bar ⌘F brought out in Focus, until the mode changes.
  const [barOut, setBarOut] = useState(false);
  const changeMode = useCallback(
    (f: (s: ModeState) => ModeState) => {
      setModes(f);
      setBarOut(false);
    },
    [setModes],
  );
  // Which board columns are open (mu-84): the mode's folds, and the owner's picks in this mode,
  // kept on this Mac. The grouped board's bands fold what the mode folds and what the owner hid.
  const [picks, setPicks] = useColumnPicks();
  const modePicks = picks[modes.mode] ?? NO_PICKS;
  const boardLooks: BoardLooks = { columns: look("columns", modes.mode), rail: look("rail", modes.mode) };
  const columnFoldOf: ColumnFolds = {
    of: (id) => bandFold(boardLooks, modePicks, id),
    toggle: (id) => setPicks((p) => toggleBand(p, modes.mode, boardLooks, id)),
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
  // The roles' names (0112), from the last poll; kept while they read the same, so cards
  // re-render on a rename only.
  const roleNamesKey = JSON.stringify(state?.roleNames ?? {});
  const roleNames = useMemo(() => JSON.parse(roleNamesKey) as Record<string, string>, [roleNamesKey]);

  useEffect(() => {
    if (queue !== "all" && state && !state.queues.some((q) => q.name === queue)) setQueue("all");
  }, [state, queue]);

  const now = state?.now ?? Math.floor(Date.now() / 1000);
  const ctx = useMemo(() => dayContext(now, roleNames), [now, roleNames]);
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
  const gridList = useMemo(
    () => (grid ? gridRows({ work: boardWork, all: inQueue, notes, by: groupBy, today: ctx.today, sort: query.sort, shown: viewTasks }) : []),
    [grid, boardWork, inQueue, notes, groupBy, ctx.today, query.sort, viewTasks],
  );
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
  // And each one's colour (0057), the dot wherever it's named.
  const colourText = (state?.queues ?? []).map((q) => `${q.name}\n${q.colour ?? ""}`).join("\n");
  const colours = useMemo(() => coloursOf(state?.queues ?? []), [colourText]);
  useEffect(() => {
    api.showProjectTitle(queue).catch((e: unknown) => console.warn("Can't set the window's title:", e));
  }, [queue, labelText]);
  const allNotes = state?.backlog ?? [];
  // Notes show on the board, grouped or not (Group by draws them in its Backlog band, 0046), so
  // only it counts them as hidden. The Board, Table and Timeline also count the tasks their Who
  // chips hide.
  const hidden = hiddenCount(view, { all: inQueue.length, shown: shownCount }, { all: queueNotes.length, shown: notes.length });
  const openNoteKey = pane?.kind === "note" ? pane.key : null;
  // The session whose recap the pane shows (0098), by the Sessions view's row key.
  const openSession = pane?.kind === "session" ? pane.key : null;
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
  // Each place of the plain board open, or a chip in the row above it (mu-84). Backlog keeps its
  // quick add; Your check stays open while Accept all's result line has something to say (0043),
  // or to say it's over its limit (0050). Recently finished counts what its list shows: its first
  // week, of the kind its chips pick (remembered on this Mac), so its chip and the list agree.
  const [recentKind, setRecentKind] = useState<KindFilter>(() => loadKind());
  const pickRecentKind = (k: KindFilter) => {
    setRecentKind(k);
    storeKind(k);
  };
  const finishedWeek = useMemo(() => weekCount(inQueue, now, recentKind), [inQueue, now, recentKind]);
  const placeCounts: PlaceCounts = { cols, notes: notes.length, finished: finishedWeek };
  const plain = plainBoard(boardLooks, modePicks, (id) => placeCount(id, placeCounts), (id) => id === "backlog" || (id === "your-check" && (boardResult || overCheck.length > 0)));
  const setColumnOpen = (id: string, isOpen: boolean) => setPicks((p) => pickColumn(p, modes.mode, id, isOpen, plain.auto(id)));
  // The row: every folded place in board order; with Group by, only Recently finished.
  const rowChips: RowChip[] = rowChipsOf(plain.row, groupBy !== "none", placeCounts);
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
    <RoleNamesContext.Provider value={roleNames}>
    <Mode.Provider value={modes.mode}>
    <Labels.Provider value={labels}>
    <ProjectColours.Provider value={colours}>
    <div className="app shell">
      <aside className="sidebar" aria-label={t("app.sidebar")} data-sidebar={sidebarLook} style={{ width: sidebarWidth }}>
        <div className="brand"><img className="logo-dark" src={logoDark} alt="" width={22} height={22} /><img className="logo-light" src={logoLight} alt="" width={22} height={22} /> Muster</div>
        <ProjectList queues={state?.queues ?? []} tasks={state?.tasks ?? []} notes={allNotes} activity={state?.activity ?? []} now={now} current={queue} onPick={setQueue} onLabel={(k, label) => api.setProjectLabel(k, label).then(refresh)} edit={projectEdits(refresh)} />
        <button className="side-add" onClick={() => openSettings("projects")} title={t("app.addProjectTitle")}>{t("app.addProject")}</button>
      </aside>
      <Splitter {...sidebarRange(win)} width={sidebarWidth} side="left" label={t("app.sidebarWidth")} onWidth={(w) => sideSplit.set(clampSidebar(w, pageWidth()))} onDrag={(w) => sideSplit.drag(clampSidebar(w, pageWidth()))} onDragEnd={sideSplit.settle} onReset={sideSplit.reset} />
      <div className="main-col">
      <header className="top">
        <Button variant="primary" className="new" onClick={() => go({ kind: "new" })} disabled={!state?.queues.length} title={t("app.newTaskTitle")}>{t("app.newTask")}</Button>
        <nav className="views" aria-label={t("app.views")}>
          <span className="view-group" role="group" aria-label={t("app.projectViews")}>
            <Pill on={view === "board"} onClick={() => setView("board")}>{t("app.view.board")}</Pill>
            <Pill on={view === "table"} onClick={() => setView("table")}>{t("app.view.table")}</Pill>
            <Pill on={view === "timeline"} onClick={() => setView("timeline")}>{t("app.view.timeline")}</Pill>
            <Pill on={view === "graph"} onClick={() => setView("graph")} title={t("app.view.graphTitle")}>{t("app.view.graph")}</Pill>
            <Pill on={view === "sessions"} onClick={() => setView("sessions")} title={t("app.view.sessionsTitle")}>{t("app.view.sessions")}</Pill>
          </span>
          <Pill on={view === "helper"} onClick={() => setView("helper")}>{t("app.view.helper")}</Pill>
          <Pill on={view === "waiting"} onClick={() => setView("waiting")} title={t("app.view.waitingTitle")}>
            {t("app.view.waiting")}{needs ? <b className="count warn">{needs}</b> : null}
          </Pill>
          <Pill on={view === "activity"} onClick={() => setView("activity")}>{t("app.view.activity")}</Pill>
          <Pill on={view === "problems"} onClick={() => setView("problems")}>
            {t("app.view.problems")}{problems.length ? <b className="count">{problems.length}</b> : null}
            {unchecked.length ? <small className="count quiet" title={t("app.view.uncheckedTitle")}>{unchecked.length}</small> : null}
          </Pill>
          <Pill on={view === "help"} onClick={() => setView("help")}>{t("app.view.help")}</Pill>
          <Pill on={view === "settings"} onClick={() => setView("settings")} title={t("app.view.settingsTitle")}>{t("app.view.settings")}</Pill>
        </nav>
        <ModeChip mode={modes.mode} onOpen={() => openSettings("display")} />
        <span className="live" title={error || t("app.liveTitle")}>
          {error ? <span className="err">{t("app.cantRead")}</span> : at ? <><i className="dot" /> {t("app.live")}</> : t("app.loading")}
        </span>
      </header>

      <div className="workspace">
        <div className="content">
          {state && state.queues.length === 0 && (
            <p className="empty"><Trans ns="shell" k="app.noQueues" params={{ path: "~/playground/<project>/tasks" }} components={{ code: <code /> }} /></p>
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
                    {whoFilters && <ChoiceChips label={t("app.who")} className="for-filter" aria={t("app.whoAria")} metaClass="meta sep" value={who} options={FOR_FILTERS} onPick={setWho} />}
                    {/* The grid has the same choice above it, as its rows (Feature, Goal, None). */}
                    {view !== "timeline" && !grid && <GroupByChips value={groupBy} onChange={setGroupBy} />}
                    {view === "board" && <ChoiceChips label={t("app.layout")} className="layout-pick" aria={t("app.layoutAria")} value={layout} options={LAYOUTS} onPick={setLayout} />}
                  </>
                }
                searchRef={searchRef}
                help={<HelpButton topic={view} />}
                out={barOut}
                onSummon={summonSearch}
                onHide={() => setBarOut(false)}
              />
              {indexError && <p className="err small" role="alert">{t("app.searchError", { error: indexError })}</p>}
              {view === "table" && <TableView tasks={viewTasks} now={now} multi={multi} openKey={openKey} onOpen={open} groups={tableGroups} by={groupBy} words={words} />}
              {view === "graph" && <GraphView tasks={tasks} all={inQueue} multi={multi} openKey={openKey} onOpen={open} />}
              {view === "timeline" && <TimelineView tasks={viewTasks} now={now} multi={multi} openKey={openKey} onOpen={open} words={words} />}
              {/* Every folded or empty column is a chip in one row under the filter bar, in board order (mu-84). A new
                  project or mode starts the row afresh, scrolled to its start (QA_4). */}
              {/* In the Feature grid, as under Group by, the row holds only Recently finished when it's folded. */}
              {view === "board" && <ColumnRow key={`${queue}/${modes.mode}`} chips={grid ? rowChipsOf(plain.row, true, placeCounts) : rowChips} grouped={grid || groupBy !== "none"} onOpen={(id) => setColumnOpen(id, true)} />}
              {/* Recently finished (0051) is docked at the board's right edge, grouped or not; under it when the row is too narrow for both. */}
              {/* The grid says a full Your check in its own alert line, over it. */}
              {view === "board" && !grid && <OverLine over={overCheck} queue={queue} />}
              {view === "board" && <OldPausesOffer offered={state?.settings?.oldPausesOffered} onDone={() => void refresh()} />}
              {view === "board" && <div className={railSide ? "board-row" : "board-row stacked"}>
              {grid && (
                <FeatureGrid
                  key={queue}
                  rows={gridList}
                  queue={queue}
                  queues={state?.queues ?? []}
                  title={queue === "all" ? t("app.allProjects") : (state?.queues.find((q) => q.name === queue)?.label || queue)}
                  header={headerLine(inQueue.filter(isWork), needs, ctx.today)}
                  needs={needs}
                  by={groupBy}
                  onBy={setGroupBy}
                  multi={multi}
                  mine={mine}
                  onMine={setMine}
                  now={now}
                  words={words}
                  openKey={openKey}
                  openNoteKey={openNoteKey}
                  onOpen={open}
                  onOpenNote={(n) => go({ kind: "note", key: noteKey(n) })}
                />
              )}
              {!grid && groupBy !== "none" && (
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
              {/* Only the open columns, each with its Fold; the rest are chips in the row above. Backlog
                  stays open for its quick add unless Focus (0038) folds it or the owner does. The
                  Paused lane (mu-59) comes first, beside Backlog. An open column is at most
                  OPEN_COLUMN_MAX wide, and the room left over stays at the right (mu-83). */}
              {!grid && groupBy === "none" && <main className="board" style={{ gridTemplateColumns: plain.tracks }}>
                {plain.open.length === 0 && <p className="none">{t("app.allFolded")}</p>}
                {plain.isOpen("paused") && <PausedColumn tasks={cols.paused} onHide={() => setColumnOpen("paused", false)} renderCard={card} />}
                {STAGES.filter((c) => plain.isOpen(c.id)).map((c) => {
                  const list = cols[c.id] ?? [];
                  const shown = c.id === "done" && !showAllDone ? list.slice(0, 8) : list;
                  const backlog = c.id === "backlog";
                  const count = placeCount(c.id, placeCounts);
                  // Your check stays open while Accept all's result line has something to say (0043).
                  const kept = c.id === "your-check" && boardResult;
                  const hide = () => setColumnOpen(c.id, false);
                  return (
                    <section key={c.id} className={columnClass(c.id, "open")}>
                      <ColumnHead stage={c} count={count} countNode={c.id === "your-check" ? <CheckCount count={count} over={overCheck} queue={queue} /> : undefined} needs={needsYouCount(list)} blocked={blockedCount(list)} fold="fixed" onFold={hide} onHide={hide}>
                        {c.role && <span className="role">{c.role}</span>}
                      </ColumnHead>
                      {c.id === "your-check" && <ColumnLine over={overCheck} queue={queue} />}
                      {backlog && quickAdd}
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
                            {showAllDone ? t("app.showFewer") : t("app.showAll", { count: list.length })}
                          </button>
                        )}
                        {count === 0 && !kept && <p className="none">{t("app.nothingHere")}</p>}
                      </ColumnCards>
                    </section>
                  );
                })}
              </main>}
              {plain.isOpen(RAIL_ID) && <Recent key={`${queue}/${modes.mode}`} tasks={inQueue} now={now} kind={recentKind} onKind={pickRecentKind} multi={multi} openKey={openKey} onOpen={open} onFold={() => setColumnOpen(RAIL_ID, false)} />}
              </div>}
            </>
          )}

          {view === "sessions" && (
            <Suspense fallback={<p className="none">{t("common:loading")}</p>}>
              <SessionsView project={queue === "all" ? null : queue} now={now} selected={openSession} onOpen={(r) => go({ kind: "session", key: sessionKey(r), account: r.account, id: r.id })} />
            </Suspense>
          )}

          {view === "waiting" && <WaitingList tasks={waiting} multi={multi} openKey={openKey} onOpen={open} onChanged={() => void refresh()} now={now} queues={state?.queues ?? []} queue={queue} />}

          {view === "activity" && (
            <main className="list">
              <ul className="feed">
                {feed(state?.activity.filter((a) => queue === "all" || a.queue === queue) ?? [], inQueue).map((a, i) => (
                  <li key={i}>
                    <span className="when">{ago(now, a.at)}</span>
                    {multi && <ProjectName name={a.queue} />}
                    <span className={`src ${a.source}`}>{a.source === "commit" ? t("app.source.commit") : t("app.source.changed")}</span>
                    <span className="what">{a.what}</span>
                  </li>
                ))}
              </ul>
            </main>
          )}

          {view === "problems" && (
            <main className="list">
              {problems.length === 0 ? <p className="empty">{t("app.noProblems")}</p> : <ul className="problems">{problems.map((p) => <li key={p}>{p}</li>)}</ul>}
              {unchecked.length > 0 && <ul className="problems warnings">{unchecked.map((w) => <li key={w}><b>{t("app.vision")}</b> {w}</li>)}</ul>}
            </main>
          )}

          {view === "helper" && state && state.queues.length > 0 && (
            <Suspense fallback={<p className="none">{t("common:loading")}</p>}>
              <Helper queues={state.queues.map((q) => q.name)} initial={queue !== "all" ? queue : state.queues[0].name} onChanged={refresh} />
            </Suspense>
          )}

          {view === "help" && (
            <Suspense fallback={<p className="none">{t("common:loading")}</p>}>
              <HelpView at={helpAt} onGo={helpGo} />
            </Suspense>
          )}

          {view === "settings" && (
            <Suspense fallback={<p className="none">{t("common:loading")}</p>}>
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
            <Splitter {...paneRange(win, sidebarWidth)} width={paneWidth} side="right" label={t("app.detailsWidth")} onWidth={(w) => paneSplit.set(clampPane(w, pageWidth(), sidebarWidth))} onDrag={(w) => paneSplit.drag(clampPane(w, pageWidth(), sidebarWidth))} onDragEnd={paneSplit.settle} onReset={paneSplit.reset} />
            <aside className="pane" style={{ width: paneWidth }} aria-label={t("app.details")}>
              {pane.kind === "task" && openTask && (
                <Suspense fallback={<p className="none">{t("common:loading")}</p>}>
                  <TaskPanel key={openKey} task={openTask} now={now} onClose={() => go(null)} onChanged={refresh} onDirty={onDirty} nudge={nudge} childTasks={childrenOf(openTask, index)} onOpenTask={(id) => go({ kind: "task", key: key({ queue: openTask.queue, id }) })} onOpenView={openView} keys={keys} onOpenAt={(queue, id) => go({ kind: "task", key: key({ queue, id }) })} />
                </Suspense>
              )}
              {pane.kind === "task" && !openTask && <p className="none">{t("app.taskGone")}</p>}
              {pane.kind === "note" && openNote && (
                <Suspense fallback={<p className="none">{t("common:loading")}</p>}>
                  <NotePanel key={openNoteKey} note={openNote} now={now} onClose={() => setPane(null)} onChanged={refresh} onFiled={showTask} />
                </Suspense>
              )}
              {pane.kind === "note" && !openNote && <p className="none">{t("app.noteGone")}</p>}
              {pane.kind === "session" && (
                <Suspense fallback={<p className="none">{t("common:loading")}</p>}>
                  <SessionRecap key={pane.key} account={pane.account} id={pane.id} now={now} onClose={() => go(null)} onOpenTask={(q, id) => go({ kind: "task", key: key({ queue: q, id }) })} />
                </Suspense>
              )}
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
    </ProjectColours.Provider>
    </Labels.Provider>
    </Mode.Provider>
    </RoleNamesContext.Provider>
    </Machine.Provider>
    </OpenHelp.Provider>
  );
}
