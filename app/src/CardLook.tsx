import { useContext } from "react";
import { tl, tn, useT } from "./i18n";
import { priorityName, stageLabel } from "./lib";
import { Machine } from "./settingsData";
import { CARD_LOOKS, type CardColour, type CardLook } from "./themes";
import type { Task } from "./types";
import { Pill } from "./ui";
import YouRibbon from "./YouRibbon";

/** A board card's classes: its priority (for colour by priority), and the states that take over
 *  its colour in every look: blocked (not when only by tasks, which clear by themselves, 0060), needs you; the
 *  selected one; and `human` for the owner's own task (0022), hatched in gold (styles.css
 *  `.card.human`) with the You ribbon in every look. */
export function cardClass(t: Pick<Task, "priority" | "blocked" | "blockedKind" | "waitingForOwner"> & { forWho?: string }, active: boolean): string {
  // i18n-ignore: class names, not text
  return `card p-${priorityName(t.priority)}${active ? " active" : ""}${t.waitingForOwner ? " needs-you" : ""}${t.blocked || (t.blockedKind && t.blockedKind !== "tasks") ? " blocked" : ""}${t.forWho === "human" ? " human" : ""}`;
}

/** The band the Top bar look shows (0026): what the card's colour means, by stage or by priority.
 *  Hidden in the other looks; the column, badges and priority chip say the same in words. */
export function CardBand({ t }: { t: Pick<Task, "stage" | "blocked" | "blockedKind" | "waitingForOwner" | "priority"> }) {
  const flag = t.blocked || (t.blockedKind && t.blockedKind !== "tasks") ? tl("flag.blocked") : t.waitingForOwner ? tl("flag.needsYou") : "";
  return (
    <span className="card-band" aria-hidden="true">
      <span className="by-stage">{flag || stageLabel(t.stage)}</span>
      <span className="by-pri">{flag || tl("priority.word", { priority: tl(`priority.${priorityName(t.priority)}`) })}</span>
    </span>
  );
}

/** The card's live dot (0025): one of the task's sessions changed its transcript within the LIVE
 *  window (5 minutes, or this machine's from Settings, 0036), so someone is working on it now.
 *  Nothing otherwise. `silent` where the words beside it already say so (the token ribbon's
 *  "being used now", mu-172): a shape only, not read out a second time. Never animated. */
export function LiveDot({ t, silent = false }: { t: Pick<Task, "live">; silent?: boolean }) {
  const live = useContext(Machine).liveMinutes;
  const tr = useT("board");
  if (!t.live) return null;
  const title = tr("card.liveTitle", { count: live });
  if (silent) return <i className="dot live-dot" aria-hidden="true" title={title} />;
  return <i className="dot live-dot" role="img" aria-label={tr("card.live")} title={title} />;
}

type Sample = { stage: string; title: string; priority: string; blocked?: boolean; needsYou?: boolean; human?: boolean };
/** What each preview shows: a plain card, one waiting on the owner, a blocked one, and one of the
 *  owner's own with its You ribbon (0022). */
export const SAMPLES: Sample[] = [
  { stage: "building", get title() { return tn("shell", "appearance.sample.flicker"); }, priority: "high" },
  { stage: "your-check", get title() { return tn("shell", "appearance.sample.header"); }, priority: "normal", needsYou: true },
  { stage: "ready", get title() { return tn("shell", "appearance.sample.graph"); }, priority: "low", blocked: true },
  { stage: "ready", get title() { return tn("shell", "appearance.sample.domain"); }, priority: "normal", human: true },
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
      <div className="look-colour" role="group" aria-label={tn("shell", "appearance.colourShowsAria")}>
        <span className="meta">{tn("shell", "appearance.colourShows")}</span>
        {(["stage", "priority"] as const).map((c) => (
          <Pill key={c} type="button" on={colour === c} onClick={() => onColour(c)} aria-pressed={colour === c}>
            {tn("shell", `appearance.colourBy.${c}`)}
          </Pill>
        ))}
      </div>
    </>
  );
}
