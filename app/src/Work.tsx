import { useEffect, useState } from "react";
import { api } from "./api";
import { fmtMinutes, ofEstimate, shortTokens, stageLabel, stamp } from "./lib";
import Markdown from "./Markdown";
import type { SessionRow, Task, Tokens, Work as WorkData } from "./types";
import { Button } from "./ui";

/** A session's transcript as the core folded it, or why it couldn't. */
type Transcript = { id: string; md?: string; error?: string };

/** While one of the task's sessions is live its numbers move without the task file changing, so
 *  the section reads them again this often. */
const LIVE_REFRESH_MS = 60_000;

const total = (t: Tokens) => t.input + t.output + t.cacheRead + t.cacheWrite;
const minutes = (m: number) => fmtMinutes(m) || "0 min";
const when = (secs: number | null) => (secs === null ? "—" : stamp(new Date(secs * 1000)));

/** Why a session's row has no numbers, by its `found`. */
const NOT_READ: Record<Exclude<SessionRow["found"], "read">, string> = {
  missing: "not on this Mac",
  "too-big": "transcript over 128 MB, not read",
  "not-read": "not read: not stamped with a Claude account Muster reads",
};

/** The task pane's Work section (0025): who worked on the task, what it cost against the
 *  estimates, a progress guess, and each session's transcript on request. Reads through the core,
 *  which reads the transcripts on this Mac and sends nothing anywhere. */
export default function Work({ task }: { task: Task }) {
  const [work, setWork] = useState<WorkData | null>(null);
  const [error, setError] = useState("");
  const [viewing, setViewing] = useState<string | null>(null);
  const [transcript, setTranscript] = useState<Transcript | null>(null);
  const [tick, setTick] = useState(0);
  const anyLive = (work?.totals.live ?? 0) > 0;

  // Read again when the task file changes, when its card's live flag flips, and every minute
  // while a session is live.
  useEffect(() => {
    let live = true;
    api.taskSessions(task.queue, task.id).then(
      (w) => {
        if (!live) return;
        setWork(w);
        setError("");
      },
      (e) => live && setError(String(e)),
    );
    return () => {
      live = false;
    };
  }, [task.queue, task.id, task.modified, task.live, tick]);

  useEffect(() => {
    if (!anyLive) return;
    const timer = window.setInterval(() => setTick((n) => n + 1), LIVE_REFRESH_MS);
    return () => window.clearInterval(timer);
  }, [anyLive]);

  useEffect(() => {
    if (!viewing) return;
    let live = true;
    api.sessionTranscript(task.queue, task.id, viewing).then(
      (md) => live && setTranscript({ id: viewing, md }),
      (e) => live && setTranscript({ id: viewing, error: String(e) }),
    );
    return () => {
      live = false;
    };
  }, [task.queue, task.id, viewing]);

  return <WorkView work={work} error={error} viewing={viewing} transcript={transcript} onView={setViewing} />;
}

/** The section as drawn from what the core said; `viewing` is the session whose transcript is
 *  open below the rows. */
export function WorkView({ work, error, viewing, transcript, onView }: {
  work: WorkData | null;
  error: string;
  viewing: string | null;
  transcript: Transcript | null;
  onView: (session: string | null) => void;
}) {
  const t = work?.totals;
  const est = work?.estimate;
  return (
    <section className="work" aria-label="Work">
      <h2>
        Work{t && t.sessions > 0 && <small> {t.sessions} session{t.sessions === 1 ? "" : "s"}{t.live > 0 ? ` · ${t.live} live` : ""}</small>}
      </h2>
      {error && <p className="err small">{error}</p>}
      {!work && !error && <p className="none">Reading the sessions' transcripts…</p>}
      {work && t && est && work.rows.length === 0 && (
        <p className="none">No session has worked on it yet: none has claimed it or written to its log with a session id.</p>
      )}
      {work && t && est && work.rows.length > 0 && (
        <>
          <p className="work-totals">
            <span>Time {ofEstimate(minutes(t.minutes), est.minutes ? minutes(est.minutes) : "", est.minutesPct)}</span>
            <span>Tokens {ofEstimate(shortTokens(t.totalTokens), est.tokens ? shortTokens(est.tokens) : "", est.tokensPct)}</span>
          </p>
          <p className="meta small">
            input {shortTokens(t.tokens.input)} · output {shortTokens(t.tokens.output)} · cache read {shortTokens(t.tokens.cacheRead)} · cache write {shortTokens(t.tokens.cacheWrite)}
            {t.read < t.sessions && ` · ${t.sessions - t.read} of ${t.sessions} sessions not read on this Mac`}
          </p>
          {t.shared > 0 && (
            <p className="meta small work-shared">
              Includes {t.shared === 1 ? "a session" : `${t.shared} sessions`} that also worked on other tasks, counted in full here, so this task's own cost is lower.
            </p>
          )}
          <table className="work-rows">
            <thead>
              <tr>
                <th>Session</th>
                <th>Stage</th>
                <th>Started</th>
                <th>Ended</th>
                <th>Time</th>
                <th>Tokens</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {work.rows.map((r) => (
                <SessionLine key={r.id} r={r} on={viewing === r.id} onView={onView} />
              ))}
            </tbody>
          </table>
        </>
      )}
      {work && (
        <p className="work-guess">
          About {work.guess.pct}% done <span className="meta">(a guess from {work.guess.basis})</span>
        </p>
      )}
      {viewing && (
        <div className="transcript" aria-label={`Transcript of session ${viewing}`}>
          <div className="transcript-bar">
            <span className="transcript-title">Session {viewing}</span>
            <span className="meta">read on this Mac, not sent anywhere</span>
            <Button type="button" onClick={() => onView(null)}>Close</Button>
          </div>
          {transcript?.id !== viewing ? (
            <p className="none">Reading the transcript…</p>
          ) : transcript.error !== undefined ? (
            <p className="err small">{transcript.error}</p>
          ) : (
            <Markdown text={transcript.md ?? ""} />
          )}
        </div>
      )}
    </section>
  );
}

function SessionLine({ r, on, onView }: { r: SessionRow; on: boolean; onView: (session: string | null) => void }) {
  const live = r.live ? <span className="live-now"><i className="dot" />live</span> : null;
  return (
    <tr className={r.live ? "work-live" : undefined}>
      <td>
        <code>{r.id}</code> <span className="meta">{r.who}{r.holds ? " · holds it" : ""}{r.sharedWith > 0 ? ` · also on ${r.sharedWith} other task${r.sharedWith === 1 ? "" : "s"}` : ""}</span>
      </td>
      <td>{r.stages.map(stageLabel).join(", ") || "—"}</td>
      {r.found === "read" ? (
        <>
          <td>{when(r.start)}</td>
          <td>{live ?? when(r.end)}</td>
          <td>{minutes(r.minutes)}</td>
          <td>{shortTokens(total(r.tokens))}</td>
        </>
      ) : (
        <td colSpan={4} className="meta">
          {NOT_READ[r.found]}
          {r.live && " · live"}
        </td>
      )}
      <td>
        <Button type="button" disabled={r.found !== "read"} aria-pressed={on} onClick={() => onView(on ? null : r.id)}>View</Button>
      </td>
    </tr>
  );
}
