import { useState, type ReactNode } from "react";
import { api } from "./api";
import { isHuman } from "./human";
import { sentences, tn, useT } from "./i18n";
import { holderOf, quietCheckin, splitHolder, stamp } from "./lib";
import { taskKey } from "./taskKey";
import { askedText, cantAsk, MAX_MESSAGE, messageLength, quietToAsk, unreachable } from "./reach";
import { answerFirst } from "./sessionState";
import type { Task } from "./types";
import { Button } from "./ui";

// Reaching the session working on a task (0039): ask it for a check-in, or send it a message.
// Each goes to the holder's session mailbox through the core (ping.rs), and the answer hook hands
// it to that session after its next tool call. A button the core would refuse is shown disabled
// with the reason, never hidden (docs/DESIGN.md).

type Result = { ok: boolean; text: string } | null;

/** What a check-in request asks the session to run, and how often after (minutes). */
// i18n-ignore: a command, the same in every language
const CHECKIN_COMMAND = "muster checkin";
const CHECKIN_EVERY_MIN = 30;

/** "Ask for a check-in" under a quiet card (the "no check-in for N min" flag), beside the card
 *  rather than inside it, since the card is itself a button. The card's own controls for the
 *  strip (`children`: Pause, 0045) come first, so they never move; without a quiet card the strip
 *  is bare (no border or fill), and nothing at all without either. */
export function CardPing({ t, now, children }: { t: Task; now: number; children?: ReactNode }) {
  const tr = useT("board");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const quiet = quietCheckin(t, now) !== null;
  if (!quiet && !children) return null;
  const why = cantAsk(t);
  const ask = async () => {
    setBusy(true);
    setError("");
    try {
      await api.askCheckin(t.queue, t.id, stamp(new Date()));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className={quiet ? "card-ping" : "card-ping bare"} data-part="card-ping">
      {children}
      {quiet && (
        <>
          <Button
            type="button"
            disabled={why !== "" || busy}
            title={why || tr("ping.cardTitle")}
            onClick={() => void ask()}
          >
            {tr("ping.ask")}
          </Button>
          {why && <span className="why">{why}</span>}
          {error && <span className="err">{error}</span>}
        </>
      )}
    </div>
  );
}

/** The task pane's strip for the session working on the task, in Building and Testing (an
 *  agent's task, not one the owner holds): Ask for a check-in, when it was asked, and a one-line
 *  message box. */
export function SessionPing({ task, now, onChanged }: { task: Task; now: number; onChanged: () => void }) {
  const t = useT("board");
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<Result>(null);
  const working = (task.stage === "building" || task.stage === "testing") && !isHuman(task) && splitHolder(holderOf(task)).label !== "owner";
  if (!working) return null;
  const why = unreachable(task);
  const askWhy = cantAsk(task);
  const asked = askedText(task, now);
  const session = splitHolder(holderOf(task)).id;
  const length = messageLength(text);
  const sendable = length > 0 && length <= MAX_MESSAGE;
  const run = async (what: () => Promise<unknown>, done: string) => {
    setBusy(true);
    setResult(null);
    try {
      await what();
      setResult({ ok: true, text: done });
      onChanged();
      return true;
    } catch (e) {
      setResult({ ok: false, text: String(e) });
      return false;
    } finally {
      setBusy(false);
    }
  };
  const when = () => stamp(new Date());
  const send = async () => {
    const message = text;
    if (await run(() => api.messageSession(task.queue, task.id, message, when()), t("ping.sent"))) setText("");
  };
  const note = why || (askWhy ? t("ping.reachesNoAsk", { session, why: askWhy }) : t("ping.reaches", { session }));
  return (
    <div className="session-ping" data-part="ping" role="group" aria-label={t("ping.stripAria")}>
      <span className="lbl">{t("ping.session")}</span>
      <Button
        type="button"
        disabled={askWhy !== "" || busy}
        title={askWhy || t("ping.askTitle", { command: CHECKIN_COMMAND, count: CHECKIN_EVERY_MIN })}
        onClick={() => void run(() => api.askCheckin(task.queue, task.id, when()), t("ping.asked"))}
      >
        {t("ping.ask")}
      </Button>
      {asked && <span className="meta">{asked}</span>}
      <form
        className="ping-msg"
        onSubmit={(e) => {
          e.preventDefault();
          if (sendable) void send();
        }}
      >
        {/* No maxLength: it counts UTF-16 units, not characters, and would cut a paste without a
            word. The count says how long it is, as the core counts it. */}
        <input
          value={text}
          onChange={(e) => setText(e.target.value)}
          dir="auto"
          placeholder={t("ping.message")}
          aria-label={t("ping.message")}
          disabled={why !== "" || busy}
        />
        <MessageCount text={text} />
        <Button type="submit" disabled={why !== "" || busy || !sendable}>{t("ping.send")}</Button>
      </form>
      <p className="meta small">{note}</p>
      {result && <p className={result.ok ? "ok small" : "err small"} role="status">{result.text}</p>}
    </div>
  );
}

/** "120/500": the message's length as the core counts it, in the error colour past the cap, with
 *  why Send is off. */
export function MessageCount({ text }: { text: string }) {
  const t = useT("board");
  const n = messageLength(text);
  return n > MAX_MESSAGE ? (
    <span className="ping-count err" title={t("ping.tooLong", { max: MAX_MESSAGE })}>{`${n}/${MAX_MESSAGE}`}</span>
  ) : (
    <span className="ping-count">{`${n}/${MAX_MESSAGE}`}</span>
  );
}

/** Under a stalled project in the sidebar: ask every quiet claim it can reach for a check-in, one
 *  after another. Disabled, with why, when none can be reached. */
export function QuietPing({ tasks, now }: { tasks: Task[]; now: number }) {
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<Result>(null);
  // Why a claim can't be asked (reach.ts) is in the board's words, wherever this shows.
  useT("board");
  const { quiet, reachable } = quietToAsk(tasks, now);
  if (quiet.length === 0) return null;
  // Waiting for input in Claude Code (0048): reachable, but not until that's answered there.
  const answer = answerFirst();
  const waiting = quiet.filter((t) => cantAsk(t) === answer).length;
  const unreached = quiet.length - reachable.length - waiting;
  const why = [
    unreached > 0 ? tn("shell", "ping.unreached", { count: unreached }) : "",
    waiting > 0 ? tn("shell", "ping.waiting", { count: waiting }) : "",
  ]
    .filter(Boolean)
    .join(" · ");
  const askAll = async () => {
    setBusy(true);
    setResult(null);
    const failed: string[] = [];
    for (const t of reachable) {
      try {
        await api.askCheckin(t.queue, t.id, stamp(new Date()));
      } catch (e) {
        failed.push(`${taskKey(t)}: ${String(e)}`);
      }
    }
    const asked = reachable.length - failed.length;
    setResult({ ok: failed.length === 0, text: sentences([tn("shell", "ping.asked", { count: asked }), ...failed]) });
    setBusy(false);
  };
  const label = reachable.length ? tn("shell", "ping.askAllCount", { count: reachable.length }) : tn("shell", "ping.askAll");
  return (
    <div className="side-ping">
      <Button type="button" disabled={reachable.length === 0 || busy} onClick={() => void askAll()}>{label}</Button>
      {reachable.length === 0 && waiting === 0 ? (
        <span className="why">{tn("shell", "ping.noneReach")}</span>
      ) : (
        why && <span className="why">{why}</span>
      )}
      {result && <span className={result.ok ? "ok" : "err"} role="status">{result.text}</span>}
    </div>
  );
}
