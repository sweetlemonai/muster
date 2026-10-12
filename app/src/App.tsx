import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "./api";
import { ago, claimant, columns, COLUMNS, feed, key } from "./lib";
import type { State, Task } from "./types";

// The task panel and its Markdown renderer load on first open, in their own chunk.
const TaskPanel = lazy(() => import("./TaskPanel"));
type View = "board" | "waiting" | "activity" | "problems";
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

export default function App() {
  const { state, error, at, refresh } = useQueues();
  const [queue, setQueue] = useState("all");
  const [view, setView] = useState<View>("board");
  const [open, setOpen] = useState<string | null>(null);
  const [showAllDone, setShowAllDone] = useState(false);

  const tasks = useMemo(
    () => (state?.tasks ?? []).filter((t) => queue === "all" || t.queue === queue),
    [state, queue],
  );
  const problems = useMemo(
    () => (state?.problems ?? []).filter((p) => queue === "all" || p.startsWith(`${queue}:`)),
    [state, queue],
  );
  const waiting = tasks.filter((t) => t.waitingForOwner);
  const cols = useMemo(() => columns(tasks), [tasks]);
  const openTask = open ? (state?.tasks ?? []).find((t) => key(t) === open) ?? null : null;
  const now = state?.now ?? Math.floor(Date.now() / 1000);
  const multi = queue === "all" && (state?.queues.length ?? 0) > 1;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(null);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="app">
      <header className="top">
        <span className="brand">Muster</span>
        <nav className="queues" aria-label="Queues">
          <button className={queue === "all" ? "chip on" : "chip"} onClick={() => setQueue("all")}>
            All <small>{state?.tasks.length ?? ""}</small>
          </button>
          {state?.queues.map((q) => (
            <button key={q.name} className={queue === q.name ? "chip on" : "chip"} onClick={() => setQueue(q.name)} title={q.path}>
              {q.name} <small>{Object.values(q.counts).reduce((a, b) => a + b, 0)}</small>
            </button>
          ))}
        </nav>
        <nav className="views" aria-label="Views">
          <button className={view === "board" ? "tab on" : "tab"} onClick={() => setView("board")}>Board</button>
          <button className={view === "waiting" ? "tab on" : "tab"} onClick={() => setView("waiting")}>
            Waiting for you{waiting.length ? <b className="count warn">{waiting.length}</b> : null}
          </button>
          <button className={view === "activity" ? "tab on" : "tab"} onClick={() => setView("activity")}>Activity</button>
          <button className={view === "problems" ? "tab on" : "tab"} onClick={() => setView("problems")}>
            Problems{problems.length ? <b className="count">{problems.length}</b> : null}
          </button>
        </nav>
        <span className="live" title={error || "Updates every 2 seconds while the window is visible"}>
          {error ? <span className="err">Can't read the queues</span> : at ? <><i className="dot" /> live</> : "loading…"}
        </span>
      </header>

      {state && state.queues.length === 0 && (
        <p className="empty">No queues found. Muster looks for <code>~/playground/*/tasks</code> folders with a README and a <code>todo/</code> folder.</p>
      )}

      {view === "board" && (
        <main className="board">
          {COLUMNS.map((c) => {
            const list = cols[c.status] ?? [];
            const shown = c.status === "done" && !showAllDone ? list.slice(0, 8) : list;
            return (
              <section key={c.status} className={`column col-${c.status}`}>
                <h2>
                  {c.label} <small>{list.length}</small>
                </h2>
                <div className="cards">
                  {shown.map((t) => (
                    <Card key={key(t)} t={t} now={now} multi={multi} onOpen={() => setOpen(key(t))} active={open === key(t)} />
                  ))}
                  {c.status === "done" && list.length > 8 && (
                    <button className="more" onClick={() => setShowAllDone((v) => !v)}>
                      {showAllDone ? "Show fewer" : `Show all ${list.length}`}
                    </button>
                  )}
                  {list.length === 0 && <p className="none">Nothing here</p>}
                </div>
              </section>
            );
          })}
        </main>
      )}

      {view === "waiting" && (
        <main className="list">
          {waiting.length === 0 && <p className="empty">Nothing is waiting for you.</p>}
          {waiting.map((t) => (
            <article key={key(t)} className="waiting">
              <header>
                <button className="link" onClick={() => setOpen(key(t))}>
                  {multi && <span className="q">{t.queue}</span>} {t.id} · {t.title}
                </button>
              </header>
              {t.questions.filter((q) => !q.ticked && q.escalated).map((q) => (
                <p key={q.n ?? q.text}><b>Q{q.n}</b> {q.text}</p>
              ))}
              {t.status === "blocked" && t.waitingOn.toLowerCase().startsWith("owner") && <p>Blocked on you: {t.waitingOn}</p>}
              <button onClick={() => setOpen(key(t))}>Open to answer</button>
            </article>
          ))}
        </main>
      )}

      {view === "activity" && (
        <main className="list">
          <ul className="feed">
            {feed(state?.activity.filter((a) => queue === "all" || a.queue === queue) ?? [], tasks).map((a, i) => (
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
          {problems.length === 0 ? <p className="empty">No problems found in the queues.</p> : (
            <ul className="problems">{problems.map((p) => <li key={p}>{p}</li>)}</ul>
          )}
        </main>
      )}

      {openTask && (
        <Suspense fallback={<aside className="panel"><p className="none">Loading…</p></aside>}>
          <TaskPanel task={openTask} now={now} onClose={() => setOpen(null)} onChanged={refresh} />
        </Suspense>
      )}
    </div>
  );
}

function Card({ t, now, multi, onOpen, active }: { t: Task; now: number; multi: boolean; onOpen: () => void; active: boolean }) {
  const stale = t.status === "doing" && now - t.modified > 12 * 3600;
  return (
    <button className={`card${active ? " active" : ""}${t.waitingForOwner ? " needs-you" : ""}`} onClick={onOpen}>
      <span className="card-top">
        {multi && <span className="q">{t.queue}</span>}
        <span className="id">{t.id}</span>
        <span className={`kind ${t.kind}`}>{t.kind}</span>
        {t.priority === "high" && <span className="pri">high</span>}
      </span>
      <span className="title">{t.title || "(no title)"}</span>
      <span className="card-bottom">
        {t.waitingForOwner && <span className="badge warn">needs you</span>}
        {t.openQuestions > 0 && <span className="badge">{t.openQuestions} open Q</span>}
        {t.attachments.length > 0 && <span className="badge">{t.attachments.length} file{t.attachments.length === 1 ? "" : "s"}</span>}
        {t.status === "doing" && t.claimedBy && <span className={stale ? "badge warn" : "badge who"}>{claimant(t.claimedBy)}{stale ? " · quiet 12 h+" : ""}</span>}
        {t.status === "blocked" && t.waitingOn && <span className="badge">on {t.waitingOn}</span>}
        <span className="age">{ago(now, t.modified)}</span>
      </span>
    </button>
  );
}
