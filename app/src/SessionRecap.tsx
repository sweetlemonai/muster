import { useEffect, useState } from "react";
import { api } from "./api";
import { Stamp } from "./Dates";
import { HelpButton } from "./HelpButton";
import { fmt, Trans, tl, useT, type TFn } from "./i18n";
import { fmtMinutes, shortTokens, stageLabel, stamp } from "./lib";
import Markdown from "./Markdown";
import { useLook } from "./modes";
import ProjectName from "./ProjectName";
import { chainLine, historyKind, projectOf, rowAge, SESSION_GROUPS, sessionName, WORDS } from "./sessionsData";
import type { SessionRecap as Recap, SessionRow, Tokens } from "./types";
import { Button } from "./ui";

/** While the recap is open its session may still be working, so it's read again this often. */
const REFRESH_MS = 15_000;
/** The core reads no transcript bigger than this (as the Work section says). */
const MAX_TRANSCRIPT_MB = 128;

const minutes = (m: number) => fmtMinutes(m) || tl("time.min", { count: 0 });
/** A time as a tool stamp (`2026-10-14 20:22`), "—" when it isn't known. */
const stampOf = (secs: number | null) => (secs === null ? "—" : stamp(new Date(secs * 1000)));
/** The same, as people read it (mu-151): «۲۲ مهر ۱۴۰۵ ۲۰:۲۲» in Persian, as stored in English. */
const when = (secs: number | null) => fmt.stamp(stampOf(secs), true);
/** Why a session has no numbers, by its `found`. */
function notRead(t: TFn<"board">, found: Exclude<SessionRow["found"], "read">): string {
  if (found === "missing") return t("work.missing");
  if (found === "too-big") return t("work.tooBig", { mb: MAX_TRANSCRIPT_MB });
  return t("work.notStamped");
}
/** "1.2M of 2M (60%)": an actual against its estimate, with the core's share; the actual alone
 *  when there's no estimate. The share's digits are the language's, as `shortTokens`' are. */
const ofEstimate = (t: TFn<"board">, actual: string, est: string, pct: number | null) => (est && pct !== null ? t("recap.ofEstimate", { actual, est, pct }) : actual);
/** " + 300k cache reads, not counted" (mu-172, as the Work section says it): cache reads are shown
 *  beside the counted tokens and never held against the estimate; "" when there were none. */
const cacheReads = (t: TFn<"board">, tk: Tokens) => (tk.cacheRead > 0 ? ` ${t("work.cacheReads", { amount: shortTokens(tk.cacheRead) })}` : "");
const breakdown = (t: TFn<"board">, tk: Tokens) =>
  t("work.breakdown", { input: shortTokens(tk.input), output: shortTokens(tk.output), cacheRead: shortTokens(tk.cacheRead), cacheWrite: shortTokens(tk.cacheWrite) });

/** A session's transcript as the core folded it, or why it couldn't. */
type Transcript = { md?: string; error?: string };

/** The recap of one session (0098), in the docked pane: who it is, the tasks it worked on, its
 *  history and handoff chain, its time, tokens and progress. Read through the core, which reads
 *  only the accounts Muster reads and sends nothing anywhere. The window keys it by the session,
 *  so another session starts afresh, its transcript closed and the recap leading. */
export default function SessionRecap({ account, id, now, onClose, onOpenTask }: {
  account: string;
  id: string;
  now: number;
  onClose: () => void;
  onOpenTask: (queue: string, id: string) => void;
}) {
  const [got, setGot] = useState<{ id: string; recap: Recap } | null>(null);
  const [error, setError] = useState("");
  const [showing, setShowing] = useState(false);
  const [transcript, setTranscript] = useState<Transcript | null>(null);
  const [tick, setTick] = useState(0);
  const at = `${account}/${id}`;

  useEffect(() => {
    let live = true;
    api.sessionRecap(account, id).then(
      (recap) => {
        if (!live) return;
        setGot({ id: at, recap });
        setError("");
      },
      (e: unknown) => live && setError(String(e)),
    );
    return () => {
      live = false;
    };
  }, [account, id, at, tick]);

  useEffect(() => {
    const timer = window.setInterval(() => !document.hidden && setTick((n) => n + 1), REFRESH_MS);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    if (!showing) return;
    let live = true;
    api.sessionMarkdown(account, id).then(
      (md) => live && setTranscript({ md }),
      (e: unknown) => live && setTranscript({ error: String(e) }),
    );
    return () => {
      live = false;
    };
  }, [account, id, showing]);

  return (
    <RecapView
      recap={got?.id === at ? got.recap : null}
      error={error}
      now={now}
      showing={showing}
      transcript={transcript}
      onTranscript={() => setShowing((s) => !s)}
      onClose={onClose}
      onOpenTask={onOpenTask}
    />
  );
}

/** The recap as drawn from what the core said. Its sections are in a fixed order; the transcript,
 *  when asked for, comes after them, so the recap is always what shows first. */
export function RecapView({ recap, error, now, showing, transcript, onTranscript, onClose, onOpenTask }: {
  recap: Recap | null;
  error: string;
  now: number;
  showing: boolean;
  transcript: Transcript | null;
  onTranscript: () => void;
  onClose: () => void;
  onOpenTask: (queue: string, id: string) => void;
}) {
  const t = useT("board");
  const who = recap?.who;
  return (
    <div className="pane-body session-recap">
      <div className="pane-head recap-strip">
        <h1 dir="auto">{who ? sessionName(who) : WORDS.recap}</h1>
        <HelpButton topic="sessions" />
        <Button type="button" aria-pressed={showing} disabled={!recap} onClick={onTranscript}>{WORDS.transcript}</Button>
        <Button type="button" onClick={onClose}>{WORDS.close}</Button>
      </div>
      {error && <p className="err small recap-err" role="alert">{error}</p>}
      {!recap && !error && <p className="none">{WORDS.readingRecap}</p>}
      {recap && who && (
        <>
          {recap.chain.length > 1 && (
            <p className="recap-chain" aria-label={WORDS.chainLabel(chainLine(recap.chain))}>
              {recap.chain.map((l, i) => (
                <span key={l.id}>
                  {i > 0 && <span className="meta" aria-hidden="true">{t("sessions.chainArrow")}</span>}
                  {l.current ? <b dir="auto">{sessionName(l)}</b> : <span dir="auto">{sessionName(l)}</span>}
                </span>
              ))}
            </p>
          )}
          <WhoSection recap={recap} now={now} />
          <TasksSection tasks={recap.tasks} onOpenTask={onOpenTask} />
          <HistorySection history={recap.history} />
          <TimeSection recap={recap} />
          <TokensSection recap={recap} />
          <ProgressSection progress={recap.progress} />
        </>
      )}
      {showing && (
        <section className="transcript" aria-label={t("work.transcriptOf", { id: who?.id ?? "" })}>
          <div className="transcript-bar">
            <span className="transcript-title">{t("work.session", { id: who?.id ?? "" })}</span>
            <span className="meta">{t("work.localOnly")}</span>
            <Button type="button" onClick={onTranscript}>{WORDS.close}</Button>
          </div>
          {!transcript ? (
            <p className="none">{WORDS.readingTranscript}</p>
          ) : transcript.error !== undefined ? (
            <p className="err small">{transcript.error}</p>
          ) : (
            <Markdown text={transcript.md ?? ""} />
          )}
        </section>
      )}
    </div>
  );
}

function WhoSection({ recap, now }: { recap: Recap; now: number }) {
  const t = useT("board");
  const w = recap.who;
  const group = SESSION_GROUPS.find((g) => g.id === w.group);
  const word = group?.word ?? w.group;
  // The core's own word for the state (blocked, stopped…) beside the group's, unless it's the same.
  const state = w.word && w.word !== group?.core ? t("recap.stateWord", { state: word, word: w.word }) : word;
  const age = rowAge(now, w.at);
  return (
    <section className="recap-who" aria-labelledby="recap-who">
      <h2 id="recap-who">{t("recap.who")}</h2>
      <dl className="recap-facts">
        <dt>{t("recap.name")}</dt>
        <dd dir="auto">{w.name || "—"}</dd>
        <dt>{t("recap.session")}</dt>
        <dd><code>{w.id}</code></dd>
        <dt>{t("recap.account")}</dt>
        {/* i18n-ignore: the account's launcher command (cc-mg), a name */}
        <dd>cc-{w.account}</dd>
        <dt>{t("recap.project")}</dt>
        <dd>{w.project ? <ProjectName name={w.project} /> : projectOf(w)}{w.checkout ? <span className="meta"> · <bdi>{w.checkout}</bdi></span> : null}</dd>
        <dt>{t("recap.state")}</dt>
        <dd className={w.group === "needs-input" ? "recap-state needs-input" : "recap-state"}>{state}{age ? <span className="meta"> · {age}</span> : null}</dd>
      </dl>
    </section>
  );
}

function TasksSection({ tasks, onOpenTask }: { tasks: Recap["tasks"]; onOpenTask: (queue: string, id: string) => void }) {
  const tr = useT("board");
  return (
    <section className="recap-tasks" aria-labelledby="recap-tasks">
      <h2 id="recap-tasks">{tr("recap.tasks")} <small>{fmt.num(tasks.length)}</small></h2>
      {tasks.length === 0 ? (
        <p className="none">{WORDS.noTasks}</p>
      ) : (
        <ul>
          {tasks.map((t) => (
            <li key={`${t.queue}/${t.id}`}>
              <button type="button" className="recap-task" onClick={() => onOpenTask(t.queue, t.id)} title={tr("recap.openTask", { key: t.key })}>
                <bdi className="id">{t.key}</bdi> <span dir="auto">{t.title}</span>
              </button>
              <span className="meta"> · {stageLabel(t.stage)}{t.holds ? ` · ${WORDS.holds}` : ""}</span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

/** The history: open in ADHD, a header with its count in Standard and Focus (modes.ts). */
function HistorySection({ history }: { history: Recap["history"] }) {
  const t = useT("board");
  const how = useLook("session-history");
  const [open, setOpen] = useState(false);
  const shown = how === "open" || open;
  return (
    <section className="recap-history" aria-labelledby="recap-history">
      {how === "open" ? (
        <h2 id="recap-history">{t("recap.history")} <small>{fmt.num(history.length)}</small></h2>
      ) : (
        <h2 id="recap-history">
          <button type="button" className="recap-fold" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
            {t("recap.history")} <small>{fmt.num(history.length)}</small>
          </button>
        </h2>
      )}
      {shown &&
        (history.length === 0 ? (
          <p className="none">{WORDS.noHistory}</p>
        ) : (
          <ol className="recap-events">
            {history.map((e, i) => (
              <li key={i} className={`recap-event ev-${e.kind}`}>
                <Stamp className="when" at={stampOf(e.at)} year />
                <span className="kind">{historyKind(e.kind)}</span>
                {e.key && <bdi className="id">{e.key}</bdi>}
                {e.pct !== null && <b>{t("recap.pct", { pct: e.pct })}</b>}
                <span className="what" dir="auto">{e.text}</span>
              </li>
            ))}
          </ol>
        ))}
    </section>
  );
}

function TimeSection({ recap }: { recap: Recap }) {
  const t = useT("board");
  const c = recap.cost;
  const chained = c.chainSessions > 1;
  return (
    <section className="recap-time" aria-labelledby="recap-time">
      <h2 id="recap-time">{t("recap.time")}</h2>
      {c.found === "read" ? (
        <p className="small">
          <Trans ns="board" k="recap.started" params={{ start: when(c.start), end: when(c.end), minutes: minutes(c.minutes) }} components={{ b: <b /> }} />
          {chained && <span className="meta"> · {t("recap.chainTime", { count: c.chainSessions, minutes: minutes(c.chainMinutes) })}</span>}
        </p>
      ) : (
        <p className="meta small">{notRead(t, c.found)}</p>
      )}
    </section>
  );
}

function TokensSection({ recap }: { recap: Recap }) {
  const t = useT("board");
  const c = recap.cost;
  const est = c.estTokens ? shortTokens(c.estTokens) : "";
  const chained = c.chainSessions > 1;
  const unread = c.chainSessions - c.chainRead;
  return (
    <section className="recap-tokens" aria-labelledby="recap-tokens">
      <h2 id="recap-tokens">{t("recap.tokens")}</h2>
      {c.found === "read" ? (
        <>
          <p className="small">
            <b>{ofEstimate(t, shortTokens(c.countedTokens), est, c.estPct)}</b>
            {cacheReads(t, c.tokens)}
          </p>
          <p className="meta small">{breakdown(t, c.tokens)}</p>
        </>
      ) : (
        <p className="meta small">{notRead(t, c.found)}</p>
      )}
      {chained && (
        <p className="meta small">
          {t("recap.chainTokens", { count: c.chainSessions, tokens: ofEstimate(t, shortTokens(c.chainCountedTokens), est, c.chainEstPct) })}
          {cacheReads(t, c.chainTokens)}
          {unread > 0 && ` · ${t("recap.chainUnread", { unread, count: c.chainSessions })}`}
        </p>
      )}
    </section>
  );
}

function ProgressSection({ progress }: { progress: Recap["progress"] }) {
  const t = useT("board");
  return (
    <section className="recap-progress" aria-labelledby="recap-progress">
      <h2 id="recap-progress">{t("recap.progress")}</h2>
      {progress.length === 0 ? (
        <p className="none">{WORDS.noProgress}</p>
      ) : (
        <ul>
          {progress.map((p) => (
            <li key={p.key}>
              <bdi className="id">{p.key}</bdi> <b>{t("recap.pct", { pct: p.pct })}</b> <span dir="auto">{p.text}</span>
              {p.guess && <span className="meta"> {WORDS.guess}</span>}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
