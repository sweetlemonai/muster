import { LiveDot } from "./CardLook";
import { ribbonHolder, stateHidesDot } from "./HolderRibbon";
import { useT } from "./i18n";
import { tokenRibbonText, tokensOver } from "./lib";
import { tokenRibbonShows, useLook } from "./modes";
import type { Task } from "./types";

/** The token ribbon (mu-172): the tokens a task's own sessions used against its estimate, at the
 *  card's bottom corner on the side the text ends, on a card someone holds in Shaping, Building or
 *  Testing. Neutral like the holder ribbon: what the work cost, not a stage. "over" is a word, not
 *  a colour, and the live dot sits here by the number it moves (D5), so a card has one dot. Its
 *  room is kept on every card a holder can come to (`.card.holdable`), held or not, so a first
 *  count or a claim arriving on a poll moves nothing: on a card nobody holds yet the room stays
 *  empty but for the live dot. The mode decides only whether it fills it (`modes.ts`). Not a
 *  control: the card is the button. The parts are hidden from a screen reader, which reads the
 *  words instead ("210k of 600k tokens used, over the estimate"). Its words are `common`'s, there
 *  before the first paint.
 *
 *  On a narrow card a part gives way whole or not at all (styles.css): the parts are laid out in
 *  the order they must stay, the dot, the figures, over and the note, and "shared" last, in one
 *  wrapping line one line high, so a part that doesn't fit wraps out of sight. The "·" before over
 *  stays on every card. The live dot doesn't show while Claude Code says the holder's session waits
 *  for input or has ended (mu-171, `stateHidesDot`), nor do the words say "being used now", nor
 *  does Standard or Focus show the ribbon for it. The spaces between the parts are for copied
 *  text only; the parts' padding spaces them. */
export default function TokenRibbon({ t, active }: { t: Task; active: boolean }) {
  // Renders again when the language changes.
  useT("common");
  const how = useLook("token-ribbon");
  // Being spent now: live, unless Claude Code says the session waits for input or has ended. The
  // dot, the spoken words and the busy check all go by this one answer.
  const live = t.live && !stateHidesDot(t);
  if (!tokenRibbonShows(how, live || tokensOver(t), active)) return null;
  if (!ribbonHolder(t)) {
    return live ? (
      <span className="token-ribbon token-dot">
        <span className="token-live token-end">
          <LiveDot t={t} />
        </span>
      </span>
    ) : null;
  }
  const w = tokenRibbonText({ ...t, live });
  // Nothing was read: "600k est · not read" is the whole message, so the note is glued to the
  // figures as one part that never wraps away, and the dot comes after it, the part that gives
  // way first (the words still say "being used now").
  const unread = t.tokensUsed == null;
  // The last part that never gives way ends the ribbon with its padding (shared has its own).
  const end = w.note ? "note" : w.over ? "over" : "figures";
  const cls = (part: string) => `token-${part}${part === end ? " token-end" : ""}`;
  const dot = live && (
    <span className={unread ? "token-live token-end" : "token-live"}>
      <LiveDot t={t} silent />
    </span>
  );
  return (
    <span className="token-ribbon" title={w.tip}>
      {!unread && dot}
      {unread ? (
        <span aria-hidden="true" className="token-figures token-note token-end">{[w.figures, w.note].filter(Boolean).join(" ")}</span>
      ) : (
        <>
          {w.figures && <span aria-hidden="true" className={cls("figures")}>{w.figures}</span>}
          {w.over && (
            <>
              {" "}
              <span aria-hidden="true" className="token-sep">·</span>{" "}
              <b aria-hidden="true" className={cls("over")}>{w.over}</b>
            </>
          )}
          {w.note && (
            <>
              {" "}
              <span aria-hidden="true" className={cls("note")}>{w.note}</span>
            </>
          )}
        </>
      )}
      {unread && dot}
      {w.shared && (
        <>
          {" "}
          <span aria-hidden="true" className="token-shared">{w.shared}</span>
        </>
      )}
      <span className="sr-only">{` ${w.spoken}`}</span>
    </span>
  );
}
