import { useState, type ReactNode } from "react";
import { api } from "./api";
import { isHuman } from "./human";
import { holderOf, quietCheckin, splitHolder, stamp } from "./lib";
import { taskKey } from "./taskKey";
import { askedText, cantAsk, MAX_MESSAGE, messageLength, quietToAsk, unreachable } from "./reach";
import type { Task } from "./types";

// Reaching the session working on a task (0039): ask it for a check-in, or send it a message.
// Each goes to the holder's session mailbox through the core (ping.rs), and the answer hook hands
// it to that session after its next tool call. A button the core would refuse is shown disabled
// with the reason, never hidden (docs/DESIGN.md).

type Result = { ok: boolean; text: string } | null;

/** "Ask for a check-in" under a quiet card (the "no check-in for N min" flag), beside the card
 *  rather than inside it, since the card is itself a button. The card's own controls for the
 *  strip (`children`: Pause, 0045) come first, so they never move; without a quiet card the strip
 *  is bare (no border or fill), and nothing at all without either. */
export function CardPing({ t, now, children }: { t: Task; now: number; children?: ReactNode }) {
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
          <button
            type="button"
            disabled={why !== "" || busy}
            title={why || "Writes to its session's mailbox: it gets the request after its next tool call, and the card says when you asked until it checks in"}
            onClick={() => void ask()}
          >
            Ask for a check-in
          </button>
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
    if (await run(() => api.messageSession(task.queue, task.id, message, when()),"Sent. It arrives after the session's next tool call.")) setText("");
  };
  const note = why || (askWhy ? `It reaches session ${session} after its next tool call, quoted as your note. Ask for a check-in: ${askWhy}.` : `It reaches session ${session} after its next tool call, quoted as your note.`);
  return (
    <div className="session-ping" data-part="ping" role="group" aria-label="Reach the session working on this">
      <span className="lbl">Session</span>
      <button
        type="button"
        disabled={askWhy !== "" || busy}
        title={askWhy || "Asks it to run muster checkin now, and every 30 minutes"}
        onClick={() => void run(() => api.askCheckin(task.queue, task.id, when()), "Asked. The card says so until it checks in.")}
      >
        Ask for a check-in
      </button>
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
          placeholder="Message the session working on this"
          aria-label="Message the session working on this"
          disabled={why !== "" || busy}
        />
        <MessageCount text={text} />
        <button type="submit" disabled={why !== "" || busy || !sendable}>Send</button>
      </form>
      <p className="meta small">{note}</p>
      {result && <p className={result.ok ? "ok small" : "err small"} role="status">{result.text}</p>}
    </div>
  );
}

/** "120/500": the message's length as the core counts it, in the error colour past the cap, with
 *  why Send is off. */
export function MessageCount({ text }: { text: string }) {
  const n = messageLength(text);
  return n > MAX_MESSAGE ? (
    <span className="ping-count err" title={`At most ${MAX_MESSAGE} characters: shorten it to send`}>{`${n}/${MAX_MESSAGE}`}</span>
  ) : (
    <span className="ping-count">{`${n}/${MAX_MESSAGE}`}</span>
  );
}

/** Under a stalled project in the sidebar: ask every quiet claim it can reach for a check-in, one
 *  after another. Disabled, with why, when none can be reached. */
export function QuietPing({ tasks, now }: { tasks: Task[]; now: number }) {
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<Result>(null);
  const { quiet, reachable } = quietToAsk(tasks, now);
  if (quiet.length === 0) return null;
  const unreached = quiet.length - reachable.length;
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
    setResult({ ok: failed.length === 0, text: [`Asked ${asked} ${asked === 1 ? "session" : "sessions"}.`, ...failed].join(" ") });
    setBusy(false);
  };
  const label = reachable.length ? `Ask all quiet sessions for a check-in (${reachable.length})` : "Ask all quiet sessions for a check-in";
  return (
    <div className="side-ping">
      <button type="button" disabled={reachable.length === 0 || busy} onClick={() => void askAll()}>{label}</button>
      {reachable.length === 0 ? (
        <span className="why">none of its quiet claims has a session to reach</span>
      ) : (
        unreached > 0 && <span className="why">{`${unreached} of them ${unreached === 1 ? "has" : "have"} no session to reach`}</span>
      )}
      {result && <span className={result.ok ? "ok" : "err"} role="status">{result.text}</span>}
    </div>
  );
}
