import { priorityName, stageLabel } from "./lib";
import { CARD_LOOKS, type CardColour, type CardLook } from "./themes";
import type { Task } from "./types";

/** A board card's classes: its priority (for colour by priority), and the states that take over
 *  its colour in every look: blocked (by the flag or by tasks it depends on, 0024), needs you; plus
 *  the selected one. */
export function cardClass(t: Pick<Task, "priority" | "blocked" | "blockedKind" | "waitingForOwner">, active: boolean): string {
  return `card p-${priorityName(t.priority)}${active ? " active" : ""}${t.waitingForOwner ? " needs-you" : ""}${t.blocked || t.blockedKind ? " blocked" : ""}`;
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

type Sample = { stage: string; title: string; priority: string; blocked?: boolean; needsYou?: boolean };
/** What each preview shows: a plain card, one waiting on the owner, and a blocked one. */
export const SAMPLES: Sample[] = [
  { stage: "building", title: "Board flickers on resize", priority: "high" },
  { stage: "your-check", title: "Try the new header", priority: "normal", needsYou: true },
  { stage: "ready", title: "Graph view", priority: "low", blocked: true },
];

/** A small, live preview of a card look: the same classes as the board, so it can't drift. */
function Preview({ look, colour }: { look: CardLook; colour: CardColour }) {
  return (
    <span className={`look-preview look-${look}${colour === "priority" ? " look-by-priority" : ""}`}>
      {SAMPLES.map((s) => (
        <span key={s.title} className={`${cardClass({ priority: s.priority, blocked: !!s.blocked, waitingForOwner: !!s.needsYou }, false)} s-${s.stage}`}>
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
