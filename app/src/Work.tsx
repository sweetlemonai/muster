import { useContext, useEffect, useState } from "react";
import { api } from "./api";
import { useT, type TFn } from "./i18n";
import { fmtHours, shortTokens, slotLabel, stageLabel } from "./lib";
import Markdown from "./Markdown";
import { RoleNamesContext } from "./settingsData";
import type { SessionRow, Task, Tokens, Work as WorkData, WorkStage } from "./types";
import { Button } from "./ui";

/** A session's transcript as the core folded it, or why it couldn't. */
type Transcript = { id: string; md?: string; error?: string };

/** While one of the task's sessions is live its numbers move without the task file changing, so
 *  the section reads them again this often. */
const LIVE_REFRESH_MS = 60_000;

/** The transcript size the core won't read past, in MB (transcripts.rs MAX_TRANSCRIPT_BYTES). */
const MAX_TRANSCRIPT_MB = 128;

/** What `est-tokens` counts (mu-172): input, output and cache writes, never cache reads. */
const counted = (t: Tokens) => t.input + t.output + t.cacheWrite;
const SEP = " · ";

/** The session whose transcript may open: never one Muster won't read (decision 0012), even if a
 *  stale `viewing` still names it (mu-162). The core refuses its transcript too. */
const openable = (work: WorkData | null, viewing: string | null) =>
  viewing && work?.rows.find((r) => r.id === viewing)?.found !== "not-read" ? viewing : null;

/** Why a session's row has no numbers: its transcript wasn't read, or nothing in it fell in a stage. */
function noNumbers(t: TFn<"board">, found: SessionRow["found"]): string {
  if (found === "missing") return t("work.missing");
  if (found === "too-big") return t("work.tooBig", { mb: MAX_TRANSCRIPT_MB });
  if (found === "not-read") return t("work.notStamped");
  return t("work.noTokens");
}

/** A stage's name: the board's for the four, "Other (Backlog, Ready, Done)" for the rest. */
const stageName = (t: TFn<"board">, s: WorkStage) => (s === "other" ? t("work.stageOther") : stageLabel(s));

/** The total line (mu-172 round 2): "Total: 1.8M of 1.5M tokens (120%) · 2.5 h of 8 h (31%)", the
 *  counted tokens and the agent time (Building + Testing) against the estimates, in hours (D4); a
 *  dash before the task was ever in Building, the actual alone without an estimate. */
function totalLine(t: TFn<"board">, totals: WorkData["totals"], est: WorkData["estimate"]): string {
  const tokens = est.tokens > 0 && est.tokensPct !== null
    ? t("work.totalTokens", { actual: shortTokens(totals.countedTokens), est: shortTokens(est.tokens), pct: est.tokensPct })
    : t("work.totalTokensAlone", { actual: shortTokens(totals.countedTokens) });
  const estTime = est.minutes > 0 ? fmtHours(est.minutes) : "";
  const agent = totals.agentMinutes;
  const time = agent === null
    ? (estTime ? t("work.totalTimeNoneOf", { est: estTime }) : "—")
    : estTime && est.minutesPct !== null ? t("work.totalTime", { actual: fmtHours(agent), est: estTime, pct: est.minutesPct }) : fmtHours(agent);
  return t("work.total", { tokens, time });
}

/** The tokens cell: the counted tokens, then cache reads and a stage's shared sessions apart, so
 *  no number in the table mixes what's counted with what isn't (F3). */
function TokensCell({ tokens, shared }: { tokens: Tokens; shared?: Tokens }) {
  const tr = useT("board");
  const none = counted(tokens) === 0 && tokens.cacheRead === 0 && (!shared || counted(shared) === 0);
  if (none) return <td>—</td>;
  const apart = [
    tokens.cacheRead > 0 ? tr("work.cacheReadsShort", { amount: shortTokens(tokens.cacheRead) }) : "",
    shared && counted(shared) > 0 ? tr("work.sharedApart", { amount: shortTokens(counted(shared)) }) : "",
  ].filter(Boolean);
  return (
    <td>
      {shortTokens(counted(tokens))}
      {apart.length > 0 && <span className="meta"> {apart.join(SEP)}</span>}
    </td>
  );
}

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
  const open = openable(work, viewing);

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
    if (!open) return;
    let live = true;
    api.sessionTranscript(task.queue, task.id, open).then(
      (md) => live && setTranscript({ id: open, md }),
      (e) => live && setTranscript({ id: open, error: String(e) }),
    );
    return () => {
      live = false;
    };
  }, [task.queue, task.id, open]);

  return <WorkView work={work} error={error} viewing={viewing} transcript={transcript} onView={setViewing} />;
}

/** The section as drawn from what the core said; `viewing` is the session whose transcript is
 *  open below the table. A table by stage (mu-172 round 2, the owner: "break down of tokens per
 *  stage and time per stage and sessions each then total"): each stage the task was in with its
 *  counted tokens, cache reads apart, and its time; the sessions that spent in it under it; a
 *  session no stage has tokens of after them, with why; then the total, against the estimates.
 *  Tokens are what `est-tokens` counts; a session another task names too is listed where it spent
 *  but never counted, and the total says what it spent so nothing vanishes. The time against the
 *  estimate is agent time (Building + Testing); Your check's is the owner's wait, on its row only.
 *  Each stage is a row group headed by its name (`scope="rowgroup"`), so a screen reader says a
 *  session's stage with its row. */
export function WorkView({ work, error, viewing, transcript, onView }: {
  work: WorkData | null;
  error: string;
  viewing: string | null;
  transcript: Transcript | null;
  onView: (session: string | null) => void;
}) {
  const tr = useT("board");
  const t = work?.totals;
  const est = work?.estimate;
  const byId = new Map((work?.rows ?? []).map((r) => [r.id, r]));
  const staged = new Set((work?.stages ?? []).flatMap((s) => s.sessions.map((x) => x.id)));
  const unstaged = (work?.rows ?? []).filter((r) => !staged.has(r.id));
  const open = openable(work, viewing);
  const view = (id: string) => ({ on: open === id, onView });
  return (
    <section className="work" aria-label={tr("work.title")}>
      <h2>
        {tr("work.title")}
        {t && t.sessions > 0 && <small> {[tr("work.sessions", { count: t.sessions }), t.live > 0 ? tr("work.live", { count: t.live }) : ""].filter(Boolean).join(SEP)}</small>}
      </h2>
      {error && <p className="err small">{error}</p>}
      {!work && !error && <p className="none">{tr("work.reading")}</p>}
      {work && t && est && work.rows.length === 0 && <p className="none">{tr("work.noSessions")}</p>}
      {work && t && est && work.rows.length > 0 && (
        <>
          <table className="work-rows work-stages">
            <thead>
              <tr>
                <th scope="col">{tr("work.head.stage")}</th>
                <th scope="col">{tr("work.head.tokens")}</th>
                <th scope="col">{tr("work.head.time")}</th>
                <th scope="col" />
              </tr>
            </thead>
            {work.stages.map((s) => (
              <tbody key={s.stage} className="work-stage">
                <tr className="work-stage-row">
                  <th scope="rowgroup">{stageName(tr, s.stage)}</th>
                  <TokensCell tokens={s.tokens} shared={s.sharedTokens} />
                  <td>{s.stage === "your-check" ? tr("work.ownerWait", { time: fmtHours(s.minutes) }) : fmtHours(s.minutes)}</td>
                  <td />
                </tr>
                {s.sessions.map((x) => {
                  const r = byId.get(x.id);
                  return (
                    <tr key={x.id} className={r?.live ? "work-session work-live" : "work-session"}>
                      <WhoCell id={x.id} r={r} shared={x.shared} />
                      <TokensCell tokens={x.tokens} />
                      <td>{fmtHours(x.minutes)}</td>
                      <ViewCell id={x.id} found={r?.found ?? "read"} {...view(x.id)} />
                    </tr>
                  );
                })}
              </tbody>
            ))}
            {unstaged.length > 0 && (
              <tbody className="work-stage">
                <tr className="work-stage-row">
                  <th scope="rowgroup">{tr("work.noStage")}</th>
                  <td />
                  <td />
                  <td />
                </tr>
                {unstaged.map((r) => (
                  <tr key={r.id} className={r.live ? "work-session work-live" : "work-session"}>
                    <WhoCell id={r.id} r={r} shared={r.sharedWith > 0} />
                    <td colSpan={2} className="meta">{noNumbers(tr, r.found)}</td>
                    <ViewCell id={r.id} found={r.found} {...view(r.id)} />
                  </tr>
                ))}
              </tbody>
            )}
            <tfoot>
              <tr className="work-total">
                <td colSpan={4}>
                  <strong>{totalLine(tr, t, est)}</strong>{" "}
                  <span className="meta">
                    {[
                      t.tokens.cacheRead > 0 ? tr("work.cacheReads", { amount: shortTokens(t.tokens.cacheRead) }) : "",
                      counted(t.sharedTokens) > 0 ? tr("work.sharedApart", { amount: shortTokens(counted(t.sharedTokens)) }) : "",
                      tr("work.agentTime"),
                    ]
                      .filter(Boolean)
                      .join(SEP)}
                  </span>
                </td>
              </tr>
            </tfoot>
          </table>
          <p className="meta small">
            {[
              tr("work.breakdown", { input: shortTokens(t.tokens.input), output: shortTokens(t.tokens.output), cacheRead: shortTokens(t.tokens.cacheRead), cacheWrite: shortTokens(t.tokens.cacheWrite) }),
              t.read < t.sessions ? tr("work.unread", { unread: t.sessions - t.read, count: t.sessions }) : "",
            ]
              .filter(Boolean)
              .join(SEP)}
          </p>
          {t.shared > 0 && (
            <p className="meta small work-shared">
              {t.sharedRead > 0 ? tr("work.shared", { count: t.shared, tokens: shortTokens(counted(t.sharedTokens)) }) : tr("work.sharedUnread", { count: t.shared })}
            </p>
          )}
        </>
      )}
      {work && (
        <p className="work-guess">
          {tr("work.guess", { pct: work.guess.pct })} <span className="meta">{tr("work.guessBasis", { basis: work.guess.basis })}</span>
        </p>
      )}
      {open && (
        <div className="transcript" aria-label={tr("work.transcriptOf", { id: open })}>
          <div className="transcript-bar">
            <span className="transcript-title">{tr("work.session", { id: open })}</span>
            <span className="meta">{tr("work.localOnly")}</span>
            <Button type="button" onClick={() => onView(null)}>{tr("work.close")}</Button>
          </div>
          {transcript?.id !== open ? (
            <p className="none">{tr("work.readingTranscript")}</p>
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

/** A session's id, who it acted as, holds it, and shared or live, as its row in `rows` says. */
function WhoCell({ id, r, shared }: { id: string; r: SessionRow | undefined; shared: boolean }) {
  const tr = useT("board");
  const names = useContext(RoleNamesContext);
  const who = [
    r ? slotLabel(r.who, r.slot, names) : "",
    r?.holds ? tr("work.holdsIt") : "",
    shared ? tr("work.sharedSession", { count: Math.max(1, r?.sharedWith ?? 1) }) : "",
  ].filter(Boolean).join(SEP);
  return (
    <td>
      <code>{id}</code> <span className="meta" dir="auto">{who}</span>
      {r?.live && <span className="live-now"><i className="dot" />{tr("work.liveWord")}</span>}
    </td>
  );
}

/** A session's View. A transcript that exists but can't be read here (missing, too big) keeps a
 *  disabled one; a session Muster won't read (decision 0012) gets none, only the empty cell that
 *  keeps the columns in line (mu-162). */
function ViewCell({ id, found, on, onView }: { id: string; found: SessionRow["found"]; on: boolean; onView: (session: string | null) => void }) {
  const tr = useT("board");
  if (found === "not-read") return <td />;
  return (
    <td>
      <Button type="button" disabled={found !== "read"} aria-pressed={on} onClick={() => onView(on ? null : id)}>{tr("work.view")}</Button>
    </td>
  );
}

