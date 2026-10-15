import { useState } from "react";
import { api, type Paused } from "./api";
import { isolate, tn, useT } from "./i18n";
import { agoText, minutesSince, stageLabel, stamp } from "./lib";
import type { Task } from "./types";
import { Button } from "./ui";

// Pausing a task (0045): "not now, keep it as it is". The core decides (pause.rs): a paused task
// keeps its stage and claim but leaves the working columns for the Paused lane (0059) until
// Resume puts it back; nobody claims or passes it, and its session is asked to stop. It isn't blocked, so it's never red: a calm grey chip with the pause glyph
// (docs/DESIGN.md: neutral, not a stage colour).

/** The stages a task can be paused in (pause::PAUSABLE): Backlog through Testing. */
const PAUSABLE = new Set(["backlog", "shaping", "ready", "building", "testing"]);
/** The pause glyph, drawn as text (not the emoji). */
export const PAUSE_GLYPH = "⏸︎";

/** Can it be paused now: in Backlog through Testing, and not already. */
export const pausable = (t: Pick<Task, "stage" | "paused">) => PAUSABLE.has(t.stage) && !t.paused;

/** "2 h ago · the design changes first": when it was paused, and why when it says; "" when it
 *  isn't paused. */
export function pausedText(t: Pick<Task, "paused" | "pausedWhy">, now: number): string {
  if (!t.paused) return "";
  const m = minutesSince(t.paused, now);
  const when = m === null ? tn("board", "pause.since", { at: t.paused }) : agoText(m);
  return withWhy(when, t.pausedWhy);
}

/** "2 h ago · the design changes first": the why after the rest, as written (isolated); the rest
 *  alone without one. */
const withWhy = (text: string, why: string | undefined) => (why ? `${text} · ${isolate(why)}` : text);

/** "was Building · paused 2 h ago · the design changes first": what a card in the Paused lane
 *  (mu-59) says, where it was, since when and why; "" when it isn't paused. */
export function wasText(t: Pick<Task, "stage" | "paused" | "pausedWhy">, now: number): string {
  if (!t.paused) return "";
  const m = minutesSince(t.paused, now);
  const stage = stageLabel(t.stage);
  const was = m === null ? tn("board", "pause.wasSince", { stage, at: t.paused }) : tn("board", "pause.was", { stage, ago: agoText(m) });
  return withWhy(was, t.pausedWhy);
}

/** The Paused chip and when and why, in the pane; nothing for a task that isn't. With `was`, a
 *  card in the Paused lane: the glyph and where it was (`wasText`), since the lane says Paused. */
export function PausedChip({ t, now, was = false }: { t: Task; now: number; was?: boolean }) {
  const tr = useT("board");
  if (!t.paused) return null;
  const title = t.pausedWhy ? tr("pause.titleWhy", { at: t.paused, why: t.pausedWhy }) : tr("pause.title", { at: t.paused });
  return (
    <span className="card-paused" title={title}>
      <span className="paused-chip">{was ? PAUSE_GLYPH : `${PAUSE_GLYPH} ${tr("pause.paused")}`}</span> {was ? wasText(t, now) : pausedText(t, now)}
    </span>
  );
}

/** What the card's strip says after a pause or resume: only that its session couldn't be told,
 *  with why; "" when it was, or nobody works on it. */
export function cardPauseNote(p: Paused): string {
  return p.session || !p.unreached ? "" : tn("board", "pause.unreached", { why: p.unreached });
}

/** Pause or Resume under a card, one click: Pause from Backlog through Testing, Resume on a paused
 *  one, nothing otherwise. It goes first in the card's strip (Ping's CardPing), beside the card,
 *  since the card is itself a button. What came of it stays beside it until the task is paused or
 *  resumed again, from here or anywhere (the card keys it on `paused`). */
export function CardPause({ t }: { t: Task }) {
  const tr = useT("board");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [note, setNote] = useState("");
  const paused = Boolean(t.paused);
  if (!paused && !pausable(t)) return null;
  const go = async () => {
    setBusy(true);
    setError("");
    setNote("");
    try {
      const p = paused ? await api.resumeTask(t.queue, t.id, stamp(new Date())) : await api.pauseTask(t.queue, t.id, "", stamp(new Date()));
      setNote(cardPauseNote(p));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <Button
        type="button"
        disabled={busy}
        title={paused ? tr("pause.resumeTitle") : tr("pause.pauseTitle")}
        onClick={() => void go()}
      >
        {paused ? tr("pause.resume") : tr("pause.pause")}
      </Button>
      {note && <span className="why">{note}</span>}
      {error && <span className="err">{error}</span>}
    </>
  );
}

/** The pane header's Pause (with an optional why, typed in place) or Resume. */
export function PanePause({ task, busy, onPause, onResume }: { task: Task; busy: boolean; onPause: (why: string) => Promise<boolean>; onResume: () => void }) {
  const tr = useT("board");
  const [why, setWhy] = useState("");
  if (task.paused) {
    return (
      <Button type="button" className="pause-btn" disabled={busy} title={tr("pause.resumeTitle")} onClick={onResume}>
        {tr("pause.resume")}
      </Button>
    );
  }
  if (!pausable(task)) return null;
  return (
    <form
      className="pane-pause"
      onSubmit={(e) => {
        e.preventDefault();
        // The why stays in the box when the core refuses it.
        void onPause(why).then((ok) => {
          if (ok) setWhy("");
        });
      }}
    >
      <Button type="submit" className="pause-btn" disabled={busy} title={tr("pause.pauseTitle")}>
        {tr("pause.pause")}
      </Button>
      <input value={why} onChange={(e) => setWhy(e.target.value)} placeholder={tr("pause.why")} aria-label={tr("pause.whyLabel")} dir="auto" />
    </form>
  );
}
