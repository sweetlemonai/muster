import { useContext, type ReactNode } from "react";
import { LiveDot } from "./CardLook";
import { isHuman } from "./human";
import { agoText, claimant, leaseRunOut, minutesSince, splitHolder } from "./lib";
import { useLook } from "./modes";
import { Machine, spanText } from "./settingsData";
import type { Note, Task } from "./types";
import YouRibbon from "./YouRibbon";

// The holder ribbon (0097): who holds a task, or is shaping a note, in the card's top-right
// corner, the You ribbon's spot (0022). An identity, not a state: neutral, never a stage colour.
// It names the holder; the card's who line keeps the rest (session, since when, its state).

/** The stages a card wears its holder's ribbon in: where a session holds it while it works. */
const RIBBON_STAGES = new Set(["shaping", "building", "testing"]);

/** A stage a ribbon can come to: the card keeps its room in the top row (styles.css
 *  `.card.holdable`), so a claim arriving with the next poll moves nothing. Not the owner's own
 *  card, which keeps the You ribbon's room. */
export const ribbonStage = (t: Pick<Task, "stage" | "forWho">) => RIBBON_STAGES.has(t.stage) && !isHuman(t);

/** Who holds a card's task for its ribbon, in full: the claim in Shaping and Building, `held-by`
 *  in Testing; "" for nobody, and in every other stage. */
export function ribbonHolder(t: Pick<Task, "stage" | "claimedBy" | "heldBy">): string {
  if (!RIBBON_STAGES.has(t.stage)) return "";
  return (t.stage === "testing" ? t.heldBy : t.claimedBy).trim();
}

const isOwner = (who: string) => splitHolder(who).label === "owner";

/** The one ribbon a card wears, never two: You on the owner's own task and on one the owner
 *  holds, the holder's on one a session holds, none otherwise. */
export function cardRibbon(t: Pick<Task, "stage" | "claimedBy" | "heldBy" | "forWho">): "you" | "holder" | null {
  const who = ribbonHolder(t);
  if (isHuman(t) || (who && isOwner(who))) return "you";
  return who ? "holder" : null;
}

/** The card's ribbon says who holds it, so its who line leaves the name out. The owner's own task
 *  wears You for being theirs, so a name in its claim isn't on the ribbon. */
export function ribbonNames(t: Pick<Task, "stage" | "claimedBy" | "heldBy" | "forWho">): boolean {
  const who = ribbonHolder(t);
  return who !== "" && (isOwner(who) || !isHuman(t));
}

/** A held note gone quiet: its heartbeat is older than the lease (this machine's, Settings 0036),
 *  as reclaim judges a claim. No heartbeat to judge by is not quiet. */
export function noteQuiet(n: Pick<Note, "heldBy" | "heartbeat">, now: number, leaseMinutes: number): boolean {
  if (!n.heldBy.trim()) return false;
  const m = minutesSince(n.heartbeat, now);
  return m !== null && m > leaseMinutes;
}

const quietTip = (lease: number) => `. Quiet: no heartbeat for over ${spanText(lease)}, its lease ran out`;

/** The ribbon itself: `spoken` before the name for a screen reader, `shown` and a "·" before it
 *  on screen, the short name cut by CSS with the whole label in the tooltip, and "quiet" in words.
 *  Where it's narrow, `shown` gives way first, the "·" stays, and the name keeps a few characters
 *  (styles.css). Not a
 *  control: the card is the button, and the keyboard order stays the card's. */
function Ribbon({ who, spoken, shown, tip, quiet, children }: { who: string; spoken: string; shown?: string; tip: string; quiet: boolean; children?: ReactNode }) {
  return (
    <span className={quiet ? "holder-ribbon quiet" : "holder-ribbon"} title={tip}>
      <span className="sr-only">{`${spoken} `}</span>
      {shown && <span aria-hidden="true" className="holder-prefix">{shown}</span>}
      {shown && <span aria-hidden="true" className="holder-sep"> · </span>}
      <span className="holder-name" dir="auto">{claimant(who)}</span>
      {children}
      {quiet && <span className="holder-quiet"> · quiet</span>}
    </span>
  );
}

/** A held card's ribbon (`cardRibbon` says "holder"): the name, the live dot while a session on it
 *  works (0025, moved here from the top row), and quiet once the lease runs out, judged as the
 *  card's flag judges it. Every mode shows it. */
export function CardHolderRibbon({ t, now }: { t: Task; now: number }) {
  const on = useLook("holder-ribbon");
  const lease = useContext(Machine).leaseMinutes;
  const who = ribbonHolder(t);
  if (on !== "show" || !who) return null;
  const quiet = leaseRunOut(t, now, lease);
  return (
    <Ribbon who={who} spoken="held by" tip={`Held by ${who}${quiet ? quietTip(lease) : ""}`} quiet={quiet}>
      <LiveDot t={t} />
    </Ribbon>
  );
}

/** A backlog note's ribbon while a session shapes it: "shaping · Shapour"; the gold You while the
 *  owner does, as on a card the owner holds. Either says quiet once the lease ran out: reclaim
 *  ends the owner's claim on a note too. */
export function NoteHolderRibbon({ n, now }: { n: Note; now: number }) {
  const on = useLook("holder-ribbon");
  const lease = useContext(Machine).leaseMinutes;
  const who = n.heldBy.trim();
  if (on !== "show" || !who) return null;
  const taken = minutesSince(n.heldSince, now);
  const since = taken === null ? "" : `, taken ${agoText(taken)}`;
  const quiet = noteQuiet(n, now, lease);
  const tip = `${since}${quiet ? quietTip(lease) : ""}`;
  if (isOwner(who)) return <YouRibbon title={`You're shaping it${tip}`} spoken="being shaped by" quiet={quiet} />;
  return <Ribbon who={who} spoken="being shaped by" shown="shaping" tip={`Being shaped by ${who}${tip}`} quiet={quiet} />;
}
