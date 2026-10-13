import { useContext } from "react";
import { priorityName, stageLabel } from "./lib";
import { Machine } from "./settingsData";
import { CARD_LOOKS, type CardColour, type CardLook } from "./themes";
import type { Task } from "./types";
import YouRibbon from "./YouRibbon";

/** A board card's classes: its priority (for colour by priority), and the states that take over
 *  its colour in every look: blocked (by the flag or by tasks it depends on, 0024), needs you; the
 *  selected one; and `human` for the owner's own task (0022), hatched in gold (styles.css
 *  `.card.human`) with the You ribbon in every look. */
export function cardClass(t: Pick<Task, "priority" | "blocked" | "blockedKind" | "waitingForOwner"> & { forWho?: string }, active: boolean): string {
  return `card p-${priorityName(t.priority)}${active ? " active" : ""}${t.waitingForOwner ? " needs-you" : ""}${t.blocked || t.blockedKind ? " blocked" : ""}${t.forWho === "human" ? " human" : ""}`;
}

/** The band the Top bar look shows (0026): what the card's colour means, by stage or by priority.
 *  Hidden in the other looks; the column, badges and priority chip say the same in words. */
export function CardBand({ t }: { t: Pick<Task, "stage" | "blocked" | "blockedKind" | "waitingForOwner" | "priority"> }) {
  const flag = t.blocked || t.blockedKind ? "Blocked" : t.waitingForOwner ? "Needs you" : "";
  return (
    <span className="card-band" aria-hidden="true">
      <span className="by-stage">{flag || stageLabel(t.stage)}</span>
      <span className="by-pri">{flag || `${priorityName(t.priority)} priority`}</span>
    </span>
  );
}

/** The card's live dot (0025): one of the task's sessions changed its transcript within the LIVE
 *  window (5 minutes, or this machine's from Settings, 0036), so someone is working on it now.
 *  Nothing otherwise. */
export function LiveDot({ t }: { t: Pick<Task, "live"> }) {
  const live = useContext(Machine).liveMinutes;
  if (!t.live) return null;
  return <i className="dot live-dot" role="img" aria-label="live" title={`A session on it is working now: its transcript changed in the last ${live} minute${live === 1 ? "" : "s"}`} />;
}

type Sample = { stage: string; title: string; priority: string; blocked?: boolean; needsYou?: boolean; human?: boolean };
/** What each preview shows: a plain card, one waiting on the owner, a blocked one, and one of the
 *  owner's own with its You ribbon (0022). */
export const SAMPLES: Sample[] = [
  { stage: "building", title: "Board flickers on resize", priority: "high" },
  { stage: "your-check", title: "Try the new header", priority: "normal", needsYou: true },
  { stage: "ready", title: "Graph view", priority: "low", blocked: true },
  { stage: "ready", title: "Buy the domain", priority: "normal", human: true },
];

/** A small, live preview of a card look: the same classes as the board, so it can't drift. */
function Preview({ look, colour }: { look: CardLook; colour: CardColour }) {
  return (
    <span className={`look-preview look-${look}${colour === "priority" ? " look-by-priority" : ""}`}>
      {SAMPLES.map((s) => (
        <span key={s.title} className={`${cardClass({ priority: s.priority, blocked: !!s.blocked, waitingForOwner: !!s.needsYou, forWho: s.human ? "human" : "ai" }, false)} s-${s.stage}`}>
          {s.human && <YouRibbon />}
          <CardBand t={{ stage: s.stage, blocked: !!s.blocked, waitingForOwner: !!s.needsYou, priority: s.priority }} />
          <span className="card-top"><span className="id">{s.title}</span></span>
        </span>
      ))}
    </span>
  );
}

/** Appearance's Cards section: every look with its preview, and what the colour means. */
export function CardLookPicker({ look, colour, onLook, onColour }: { look: CardLook; colour: CardColour; onLook: (l: CardLook) => void; onColour: (c: CardColour) => void }) {
  return (
    <>
      <div className="looks">
        {CARD_LOOKS.map((l) => (
          <button key={l.id} type="button" className={look === l.id ? "look on" : "look"} onClick={() => onLook(l.id)} title={l.hint} aria-pressed={look === l.id}>
            <Preview look={l.id} colour={colour} />
            <span className="swatch-name">{l.name}</span>
          </button>
        ))}
      </div>
      <div className="look-colour" role="group" aria-label="Card colour shows">
        <span className="meta">Colour shows</span>
        {(["stage", "priority"] as const).map((c) => (
          <button key={c} type="button" className={colour === c ? "chip on" : "chip"} onClick={() => onColour(c)} aria-pressed={colour === c}>
            {c === "stage" ? "Stage" : "Priority"}
          </button>
        ))}
      </div>
    </>
  );
}
