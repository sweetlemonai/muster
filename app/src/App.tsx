import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "./api";
import { mayLeave } from "./writeUp";
import { ago, columns, estimateTotal, feed, holder, inProgress, key, leaseRunOut, STAGES } from "./lib";
import type { State, Task } from "./types";
import { backlogColumn, backlogNotes, NoteCard, noteKey, projectCount, QuickAdd } from "./Backlog";
import Projects from "./Projects";
import NewTask from "./NewTask";
import Appearance from "./Appearance";
import { CardBand, cardClass } from "./CardLook";
import { TableView, TimelineView } from "./Views";
import { GroupByChips, GroupedBoard } from "./Groups";
import { childrenOf, GROUP_KEY, type GroupBy, groupItems, indexTasks, isWork, parentLine, parseGroupBy } from "./levels";
import logo from "./logo.png";

// The task panel and its Markdown renderer load on first open, in their own chunk.
const TaskPanel = lazy(() => import("./TaskPanel"));
const NotePanel = lazy(() => import("./NotePanel"));
const Helper = lazy(() => import("./Helper"));
const HelpView = lazy(() => import("./Help"));
type View = "board" | "table" | "timeline" | "helper" | "waiting" | "activity" | "problems" | "projects" | "help";
// One docked pane beside the content, never over it (the owner wants everything in view).
type Pane = { kind: "task"; key: string } | { kind: "note"; key: string } | { kind: "new" } | { kind: "appearance" } | null;
const POLL_MS = 2000;
const PRIORITIES = ["high", "normal", "low"] as const;
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

export default function App() {
  const { state, error, at, refresh } = useQueues();
  const [queue, setQueue] = useState("all");
  const [view, setView] = useState<View>("board");
  const [pane, setPane] = useState<Pane>(null);
  const [showAllDone, setShowAllDone] = useState(false);
  const [prio, setPrio] = useState<string[]>([]);
  const [tagFilter, setTagFilter] = useState<string[]>([]);
  const [width, setWidth] = usePaneWidth();
  const [groupBy, setGroupBy] = useGroupBy();

  useEffect(() => {
    if (queue !== "all" && state && !state.queues.some((q) => q.name === queue)) setQueue("all");
  }, [state, queue]);

  const inQueue = useMemo(() => (state?.tasks ?? []).filter((t) => queue === "all" || t.queue === queue), [state, queue]);
  const tasks = useMemo(
    () =>
      inQueue.filter(
        (t) => (prio.length === 0 || prio.includes(t.priority || "normal")) && (tagFilter.length === 0 || tagFilter.some((g) => t.tags.includes(g))),
      ),
    [inQueue, prio, tagFilter],
  );
  const allTags = useMemo(() => [...new Set(inQueue.flatMap((t) => t.tags))].sort(), [inQueue]);
  const problems = useMemo(() => (state?.problems ?? []).filter((p) => queue === "all" || p.startsWith(`${queue}:`)), [state, queue]);
  const waiting = inQueue.filter((t) => t.waitingForOwner);
  // Goals and features aren't cards in the stage columns: they're the bands, and table rows.
  const work = useMemo(() => tasks.filter(isWork), [tasks]);
  const cols = useMemo(() => columns(work), [work]);
  const index = useMemo(() => indexTasks(state?.tasks ?? []), [state]);
  const boardGroups = useMemo(() => (groupBy === "none" ? [] : groupItems(work, inQueue, groupBy, tasks)), [groupBy, work, inQueue, tasks]);
  const tableGroups = useMemo(() => (groupBy === "none" ? undefined : groupItems(tasks, inQueue, groupBy)), [groupBy, tasks, inQueue]);
  const openKey = pane?.kind === "task" ? pane.key : null;
  const openTask = openKey ? (state?.tasks ?? []).find((t) => key(t) === openKey) ?? null : null;
  const allNotes = state?.backlog ?? [];
  const notes = useMemo(() => backlogNotes(state?.backlog ?? [], queue), [state, queue]);
  const openNoteKey = pane?.kind === "note" ? pane.key : null;
  const openNote = openNoteKey ? allNotes.find((n) => noteKey(n) === openNoteKey) ?? null : null;
  const showTask = (q: string, id: string) => {
    setView("board");
    void refresh().then(() => setPane({ kind: "task", key: `${q}/${id}` }));
  };
  const now = state?.now ?? Math.floor(Date.now() / 1000);
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
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") go(null);
      if (e.key === "n" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        go({ kind: "new" });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [go]);

  const toggle = (list: string[], set: (v: string[]) => void, v: string) => set(list.includes(v) ? list.filter((x) => x !== v) : [...list, v]);

  return (
    <div className="app shell">
      <aside className="sidebar" aria-label="Projects">
        <div className="brand"><img src={logo} alt="" width={22} height={22} /> Muster</div>
        <nav className="projects-nav">
          <ProjectItem name="All" total={(state?.tasks.length ?? 0) + allNotes.length} tasks={state?.tasks ?? []} on={queue === "all"} onClick={() => setQueue("all")} />
          <div className="side-label">Projects</div>
          {state?.queues.map((q) => (
            <ProjectItem
              key={q.name}
              name={q.name}
              title={q.path}
              total={projectCount(q.counts, allNotes, q.name)}
              tasks={(state?.tasks ?? []).filter((t) => t.queue === q.name)}
              on={queue === q.name}
              onClick={() => setQueue(q.name)}
            />
          ))}
        </nav>
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
          </span>
          <button className={view === "helper" ? "tab on" : "tab"} onClick={() => setView("helper")}>Helper</button>
          <button className={view === "waiting" ? "tab on" : "tab"} onClick={() => setView("waiting")}>
            Waiting for you{waiting.length ? <b className="count warn">{waiting.length}</b> : null}
          </button>
          <button className={view === "activity" ? "tab on" : "tab"} onClick={() => setView("activity")}>Activity</button>
          <button className={view === "problems" ? "tab on" : "tab"} onClick={() => setView("problems")}>
            Problems{problems.length ? <b className="count">{problems.length}</b> : null}
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

          {(view === "board" || view === "table" || view === "timeline") && (
            <>
              <div className="filters" aria-label="Filters">
                <span className="meta">Priority</span>
                {PRIORITIES.map((p) => (
                  <button key={p} className={prio.includes(p) ? `chip on pchip-${p}` : "chip"} onClick={() => toggle(prio, setPrio, p)}>
                    {p}
                  </button>
                ))}
                {allTags.length > 0 && <span className="meta sep">Tags</span>}
                {allTags.map((g) => (
                  <button key={g} className={tagFilter.includes(g) ? "chip on" : "chip"} onClick={() => toggle(tagFilter, setTagFilter, g)}>
                    #{g}
                  </button>
                ))}
                {(prio.length > 0 || tagFilter.length > 0) && (
                  <button className="chip" onClick={() => { setPrio([]); setTagFilter([]); }}>Clear</button>
                )}
                {view !== "timeline" && <GroupByChips value={groupBy} onChange={setGroupBy} />}
                <span className={view !== "timeline" ? "meta" : "meta right"}>
                  {estimateTotal(work) && <span className="est-total">{estimateTotal(work)} · </span>}
                  {tasks.length === inQueue.length ? `${tasks.length} tasks` : `${tasks.length} of ${inQueue.length} tasks`}
                </span>
              </div>
              {view === "table" && <TableView tasks={tasks} now={now} multi={multi} openKey={openKey} onOpen={open} groups={tableGroups} by={groupBy} />}
              {view === "timeline" && <TimelineView tasks={tasks} now={now} multi={multi} openKey={openKey} onOpen={open} />}
              {view === "board" && groupBy !== "none" && (
                <GroupedBoard
                  groups={boardGroups}
                  by={groupBy}
                  multi={multi}
                  openKey={openKey}
                  onOpen={open}
                  renderCard={(t) => <Card t={t} now={now} multi={multi} parent={parentLine(t, index)} onOpen={() => open(t)} active={openKey === key(t)} />}
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
                                <NoteCard key={`note:${noteKey(item.note)}`} n={item.note} now={now} multi={multi} onOpen={() => setPane({ kind: "note", key: noteKey(item.note) })} active={openNoteKey === noteKey(item.note)} />
                              ) : (
                                <Card key={key(item.task)} t={item.task} now={now} multi={multi} parent={parentLine(item.task, index)} onOpen={() => open(item.task)} active={openKey === key(item.task)} />
                              ),
                            )
                          : shown.map((t) => (
                              <Card key={key(t)} t={t} now={now} multi={multi} parent={parentLine(t, index)} onOpen={() => open(t)} active={openKey === key(t)} />
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
                      {multi && <span className="q">{t.queue}</span>} {t.id} · {t.title}
                    </button>
                  </header>
                  {t.questions.filter((q) => !q.ticked && q.escalated).map((q) => (
                    <p key={q.n ?? q.text}><b>Q{q.n}</b> {q.text}</p>
                  ))}
                  {t.blocked && t.waitingOn.toLowerCase().startsWith("owner") && <p>Blocked on you: {t.waitingOn}</p>}
                  {t.stage === "your-check" && <p>In Your check: accept it, or send it back with what's wrong.</p>}
                  <button onClick={() => open(t)}>Open to answer</button>
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
            </main>
          )}

          {view === "helper" && state && state.queues.length > 0 && (
            <Suspense fallback={<p className="none">Loading…</p>}>
              <Helper queues={state.queues.map((q) => q.name)} initial={queue !== "all" ? queue : state.queues[0].name} onChanged={refresh} />
            </Suspense>
          )}

          {view === "help" && (
            <Suspense fallback={<p className="none">Loading…</p>}>
              <HelpView />
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
  );
}

/** One project in the sidebar: its name and count, what's in progress and what needs you. */
function ProjectItem({ name, title, total, tasks, on, onClick }: { name: string; title?: string; total: number; tasks: Task[]; on: boolean; onClick: () => void }) {
  const doing = tasks.filter(inProgress).length;
  const needs = tasks.filter((t) => t.waitingForOwner).length;
  return (
    <button className={on ? "side-item on" : "side-item"} onClick={onClick} title={title}>
      <span className="side-name">{name}</span>
      <span className="side-badges">
        {needs > 0 && <span className="count warn" title={`${needs} need you`}>{needs}</span>}
        {doing > 0 && <span className="side-doing" title={`${doing} in progress (Building or Testing)`}>{doing}</span>}
        <small>{total}</small>
      </span>
    </button>
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

function Card({ t, now, multi, parent, onOpen, active }: { t: Task; now: number; multi: boolean; parent: string; onOpen: () => void; active: boolean }) {
  const stale = leaseRunOut(t, now);
  const who = holder(t);
  const priority = t.priority || "normal";
  return (
    <button className={cardClass(t, active)} onClick={onOpen}>
      <CardBand t={t} />
      <span className="card-top">
        {multi && <span className="q">{t.queue}</span>}
        <span className="id">{t.id}</span>
        <span className={`kind ${t.kind}`}>{t.kind}</span>
        <span className={`pri pri-${priority}`} title={`Priority: ${priority}`}>{priority}</span>
      </span>
      <span className="title">{t.title || "(no title)"}</span>
      {parent && <span className="parent-line">{parent}</span>}
      {t.tags.length > 0 && (
        <span className="tags">
          {t.tags.map((g) => <span key={g} className="tag">#{g}</span>)}
        </span>
      )}
      <span className="card-bottom">
        {t.blocked && <span className="badge blocked">blocked{t.waitingOn ? `: ${t.waitingOn}` : ""}</span>}
        {t.waitingForOwner && <span className="badge warn">needs you</span>}
        {t.openQuestions > 0 && <span className="badge">{t.openQuestions} open Q</span>}
        {t.attachments.length > 0 && <span className="badge">{t.attachments.length} file{t.attachments.length === 1 ? "" : "s"}</span>}
        {(t.estTime || t.estTokens) && <span className="badge est" title="Estimated agent time and tokens">~{[t.estTime, t.estTokens].filter(Boolean).join(" · ")}</span>}
        {who && <span className={stale ? "badge stale" : "badge who"}>{who}{stale ? " · quiet 12 h+" : ""}</span>}
        <span className="age">{ago(now, t.modified)}</span>
      </span>
    </button>
  );
}
