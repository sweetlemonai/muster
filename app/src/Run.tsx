import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState, type ReactElement } from "react";
import { api } from "./api";
import { keyed, merged, received, fmtRunTime, shownLines, statusWords, SHOWN_LINES, type RunInbox } from "./runData";
import type { RunSnap, RunView } from "./types";
import { Button } from "./ui";

/** A project's Run (0079): what the core knows of it (`info`, null while it loads), its latest
 *  run as the window holds it (`run`: `run_state`'s lines, then the `run` events' new ones), and
 *  Run and Stop, each saying what the core refused in `error`. The project is named by its name
 *  only; the core looks up its command and checkout. Events are taken by the project's key, which
 *  `run_state` gives (`RunInbox`). */
export function useRun(project: string): {
  info: RunView | null;
  run: RunSnap | null;
  busy: boolean;
  error: string;
  start: () => void;
  stop: () => void;
} {
  const [info, setInfo] = useState<RunView | null>(null);
  const [run, setRun] = useState<RunSnap | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const inbox = useRef<RunInbox>({ key: null, waiting: [] });

  const take = useCallback((v: RunView) => {
    setInfo(v);
    const r = v.run;
    const k = keyed(inbox.current, v.key);
    inbox.current = k.inbox;
    setRun((held) => [...k.take, ...(r ? [r] : [])].reduce<RunSnap | null>((h, push) => merged(h, push), held));
  }, []);

  useEffect(() => {
    let live = true;
    setInfo(null);
    setRun(null);
    setError("");
    inbox.current = { key: null, waiting: [] };
    // Listening first, then reading, so no event falls between the two; `merged` takes the
    // lines both have once.
    const off = listen<RunSnap>("run", (e) => {
      if (!live) return;
      const r = received(inbox.current, e.payload);
      inbox.current = r.inbox;
      const push = r.take;
      if (push) setRun((held) => merged(held, push));
    });
    void off.then(() =>
      api.runState(project).then(
        (v) => live && take(v),
        (e) => live && setError(String(e)),
      ),
    );
    return () => {
      live = false;
      void off.then((f) => f());
    };
  }, [project, take]);

  const act = useCallback(
    (call: (p: string) => Promise<RunView>) => {
      setBusy(true);
      setError("");
      call(project).then(
        (v) => {
          take(v);
          setBusy(false);
        },
        (e) => {
          setError(String(e));
          setBusy(false);
        },
      );
    },
    [project, take],
  );

  return { info, run, busy, error, start: () => act(api.runStart), stop: () => act(api.runStop) };
}

const going = (run: RunSnap | null) => run?.status.kind === "running";

/** The Run line of What you're accepting (0079): Run (Restart while it runs), named for the
 *  project, with the command beside it as code, so the owner sees what runs; or how to set a run
 *  command when there's none. Why the core can't run it, or refused a click, under it. */
export function RunLine({ project, info, run, busy, error, onRun }: {
  project: string;
  info: RunView | null;
  run: RunSnap | null;
  busy: boolean;
  error: string;
  onRun: () => void;
}) {
  if (!info) return <span className="small meta">{error || "…"}</span>;
  if (!info.command) {
    return (
      <span className="small meta">
        No run command: <code dir="ltr">muster projects set {project} run …</code>
      </span>
    );
  }
  const word = going(run) ? "Restart" : "Run";
  return (
    <span className="small run-line-cmd">
      <Button type="button" aria-label={`${word} ${project}`} disabled={busy || info.refused !== ""} onClick={onRun}>{word}</Button>{" "}
      <code dir="ltr">{info.command}</code>
      {info.refused && <span className="err small"> {info.refused}</span>}
      {!info.refused && error && <span className="err small" role="alert"> {error}</span>}
    </span>
  );
}

/** The Run block (0079), under What you're accepting while the project has a run: where it stands
 *  (screen readers hear it from `RunStatusLive`; the running time ticks beside it), Stop and Restart while it runs or Run again after, and its last `SHOWN_LINES` lines in
 *  a labelled log that isn't live, which All output opens in place to every line kept. Lines are
 *  plain text, each set right to left or left to right by its own first letters. */
export function RunBlock({ project, run, nowSecs, busy, all, onAll, onRun, onStop }: {
  project: string;
  run: RunSnap;
  nowSecs: number;
  busy: boolean;
  all: boolean;
  onAll: () => void;
  onRun: () => void;
  onStop: () => void;
}) {
  const running = going(run);
  return (
    <div className="stage-block run-block" data-part="run" role="group" aria-label={`Run of ${project}`}>
      <span className="lbl">Run</span>
      <p className="wline run-status">
        <span className="small">{statusWords(run)}</span>
        {running && <span className="small meta"> · {fmtRunTime(nowSecs - run.since)}</span>}
      </p>
      <p className="wline">
        {running ? (
          <>
            <Button type="button" aria-label={`Stop ${project}`} disabled={busy} onClick={onStop}>Stop</Button>{" "}
            <Button type="button" aria-label={`Restart ${project}`} disabled={busy} onClick={onRun}>Restart</Button>
          </>
        ) : (
          <Button type="button" aria-label={`Run again ${project}`} disabled={busy} onClick={onRun}>Run again</Button>
        )}
      </p>
      <div className={all ? "run-output all" : "run-output"} role="log" aria-live="off" aria-label={`${project}'s run output`}>
        {run.lines.length === 0 ? (
          <span className="meta">No output yet.</span>
        ) : (
          shownLines(run, all).map((l) => (
            <div key={l.n} className="run-out-line">{l.text || "\u00a0"}</div>
          ))
        )}
      </div>
      {run.lines.length > SHOWN_LINES && (
        <Button type="button" aria-expanded={all} onClick={onAll}>
          {all ? `Last ${SHOWN_LINES} lines` : `All output (${run.lines.length} lines)`}
        </Button>
      )}
    </div>
  );
}

/** Where the run stands, for screen readers: a polite live region there from the start, before
 *  any run, so the first "Running …" is announced too, and changing with the state only. */
export function RunStatusLive({ run }: { run: RunSnap | null }) {
  return <span className="sr-only" aria-live="polite">{run ? statusWords(run) : ""}</span>;
}

/** Seconds since 1970, ticking each second while `on`; `fallback` when not. */
export function useTick(on: boolean, fallback: number): number {
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));
  useEffect(() => {
    if (!on) return;
    setNow(Math.floor(Date.now() / 1000));
    const t = window.setInterval(() => setNow(Math.floor(Date.now() / 1000)), 1000);
    return () => window.clearInterval(t);
  }, [on]);
  return on ? now : fallback;
}

/** Your check's Run (0079): the line in What you're accepting (`line`) and the block under it
 *  (`block`, null before the project's first run), sharing one `useRun`. All output shuts again
 *  for another project. */
export function useRunParts(project: string, nowSecs: number): { line: ReactElement; block: ReactElement | null } {
  const r = useRun(project);
  const [all, setAll] = useState(false);
  useEffect(() => setAll(false), [project]);
  const now = useTick(going(r.run), nowSecs);
  return {
    line: (
      <>
        <RunLine project={project} info={r.info} run={r.run} busy={r.busy} error={r.error} onRun={r.start} />
        <RunStatusLive run={r.run} />
      </>
    ),
    block: r.run && (
      <RunBlock project={project} run={r.run} nowSecs={now} busy={r.busy} all={all} onAll={() => setAll((a) => !a)} onRun={r.start} onStop={r.stop} />
    ),
  };
}

/** Run in Settings › Projects (0079), on the project's Run line: Run, or while it runs "running"
 *  with Stop and Restart; its output shows in the pane of a task in Your check. */
export function RunControls({ project }: { project: string }) {
  const r = useRun(project);
  return <RunButtons project={project} run={r.run} busy={r.busy} refused={r.info?.refused ?? ""} error={r.error} onRun={r.start} onStop={r.stop} />;
}

/** RunControls drawn: `refused` is why the core can't run it (Run is off), `error` what it
 *  refused of a click (Run stays on, to try again). */
export function RunButtons({ project, run, busy, refused, error, onRun, onStop }: {
  project: string;
  run: RunSnap | null;
  busy: boolean;
  refused: string;
  error: string;
  onRun: () => void;
  onStop: () => void;
}) {
  return (
    <span className="run-controls">
      {going(run) ? (
        <>
          <span className="run-chip">running</span>{" "}
          <Button type="button" aria-label={`Stop ${project}`} disabled={busy} onClick={onStop}>Stop</Button>{" "}
          <Button type="button" aria-label={`Restart ${project}`} disabled={busy} onClick={onRun}>Restart</Button>
        </>
      ) : (
        <Button type="button" aria-label={`Run ${project}`} disabled={busy || refused !== ""} onClick={onRun}>Run</Button>
      )}
      {refused && <span className="err small"> {refused}</span>}
      {!refused && error && <span className="err small" role="alert"> {error}</span>}
    </span>
  );
}
