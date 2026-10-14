import { useState } from "react";
import { api, type Paused } from "./api";
import { agoText, minutesSince, stageLabel, stamp } from "./lib";
import type { Task } from "./types";
import { Button } from "./ui";

// Pausing a task (0045): "not now, keep it as it is". The core decides (pause.rs): a paused task
// keeps its stage, claim and place on the board, nobody claims or passes it, and its session is
// asked to stop. It isn't blocked, so it's never red: a calm grey chip with the pause glyph
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
  const when = m === null ? `since ${t.paused}` : agoText(m);
  return t.pausedWhy ? `${when} · ${t.pausedWhy}` : when;
}

/** "was Building · paused 2 h ago · the design changes first": what a card in the Paused lane
 *  (mu-59) says, where it was, since when and why; "" when it isn't paused. */
export function wasText(t: Pick<Task, "stage" | "paused" | "pausedWhy">, now: number): string {
  return t.paused ? `was ${stageLabel(t.stage)} · paused ${pausedText(t, now)}` : "";
}

/** The Paused chip and when and why, in the pane; nothing for a task that isn't. With `was`, a
 *  card in the Paused lane: the glyph and where it was (`wasText`), since the lane says Paused. */
export function PausedChip({ t, now, was = false }: { t: Task; now: number; was?: boolean }) {
  if (!t.paused) return null;
  return (
    <span className="card-paused" title={`Paused since ${t.paused}${t.pausedWhy ? `: ${t.pausedWhy}` : ""}. Nobody claims or passes it until it's resumed`}>
      <span className="paused-chip">{was ? PAUSE_GLYPH : `${PAUSE_GLYPH} Paused`}</span> {was ? wasText(t, now) : pausedText(t, now)}
    </span>
  );
}

/** What the card's strip says after a pause or resume: only that its session couldn't be told,
 *  with why; "" when it was, or nobody works on it. */
export function cardPauseNote(p: Paused): string {
  return p.session || !p.unreached ? "" : `its session can't be told: ${p.unreached}`;
}

/** Pause or Resume under a card, one click: Pause from Backlog through Testing, Resume on a paused
 *  one, nothing otherwise. It goes first in the card's strip (Ping's CardPing), beside the card,
 *  since the card is itself a button. What came of it stays beside it until the task is paused or
 *  resumed again, from here or anywhere (the card keys it on `paused`). */
export function CardPause({ t }: { t: Task }) {
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
        title={paused ? "Carry on as it was; its session is told to carry on" : "Not now: it keeps its place and claim, nobody works it, and its session is asked to stop"}
        onClick={() => void go()}
      >
        {paused ? "Resume" : "Pause"}
      </Button>
      {note && <span className="why">{note}</span>}
      {error && <span className="err">{error}</span>}
    </>
  );
}

/** The pane header's Pause (with an optional why, typed in place) or Resume. */
export function PanePause({ task, busy, onPause, onResume }: { task: Task; busy: boolean; onPause: (why: string) => Promise<boolean>; onResume: () => void }) {
  const [why, setWhy] = useState("");
  if (task.paused) {
    return (
      <Button type="button" className="pause-btn" disabled={busy} title="Carry on as it was; its session is told to carry on" onClick={onResume}>
        Resume
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
      <Button type="submit" className="pause-btn" disabled={busy} title="Not now: it keeps its place and claim, nobody works it, and its session is asked to stop">
        Pause
      </Button>
      <input value={why} onChange={(e) => setWhy(e.target.value)} placeholder="why (optional)" aria-label="Why pause it (optional)" />
    </form>
  );
}
