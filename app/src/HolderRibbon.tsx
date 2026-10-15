import { useContext, type ReactNode } from "react";
import { isHuman } from "./human";
import { agoText, leaseRunOut, minutesSince, nameOf, shownHolder, splitHolder } from "./lib";
import { useLook } from "./modes";
import { ribbonStateWords, sessionWords } from "./sessionState";
import { Machine, RoleNamesContext, spanText } from "./settingsData";
import type { Note, Task } from "./types";
import YouRibbon from "./YouRibbon";

// The holder ribbon (0097): who holds a task, or is shaping a note, in the card's top-right
// corner, the You ribbon's spot (0022). The name is an identity: neutral, never a stage colour.
// After it, what Claude Code says the holder's session is doing (mu-171, 0048), in words, the
// state alone in its colour. The card's who line keeps the rest (session, since when).

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

/** Claude Code's word on the holder's session wins over the live dot (mu-171): while it waits for
 *  input or has ended, the card shows no dot, wherever the dot sits (the token ribbon, mu-172). */
export function stateHidesDot(t: Pick<Task, "stage" | "sessionState" | "paused">): boolean {
  if (ribbonStateWords(t, false) === "") return false;
  return t.sessionState === "needs-input" || t.sessionState === "ended";
}

const isOwner = (who: string) => splitHolder(who).label === "owner";

/** The one ribbon a card wears, never two: You on the owner's own task and on one the owner
 *  holds, the holder's on one a session holds, none otherwise. */
export function cardRibbon(t: Pick<Task, "stage" | "claimedBy" | "heldBy" | "forWho">): "you" | "holder" | null {
  const who = ribbonHolder(t);
  if (isHuman(t) || (who && isOwner(who))) return "you";
  return who ? "holder" : null;
}

/** The card's ribbon says its holder's session waits for input (mu-171), in the needs-you colour,
 *  so the card's own "needs you" badge would say it twice. */
export const ribbonWaits = (t: Pick<Task, "stage" | "claimedBy" | "heldBy" | "forWho" | "sessionState" | "paused">) =>
  cardRibbon(t) === "holder" && t.sessionState === "needs-input" && ribbonStateWords(t, false) !== "";

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

/** A holder's name in two, for a narrow ribbon (mu-171 R1): `tail`, the number it ends in, its last
 *  three digits at most, is what tells two sessions of one role apart (asghar_16, asghar_13), so
 *  CSS cuts `head`, never the tail: "as…16". A name without a number, or that is only one, has no
 *  tail and is cut at its end as before. Any script's digits count (قلی_۳۱). */
export function nameEnds(name: string): { head: string; tail: string } {
  const m = /\p{Nd}+$/u.exec(name);
  if (!m || m.index === 0) return { head: name, tail: "" };
  const tail = [...m[0]].slice(-3).join("");
  return { head: name.slice(0, name.length - tail.length), tail };
}

/** The name as the ribbon shows it: whole where it fits, else its head ends in … and its number
 *  stays (styles.css `.holder-name.tail-N`, the tail's length in digits). `dir="auto"` keeps a
 *  right-to-left name's head and number in its own reading order. The two parts are flex items,
 *  which a screen reader may read apart, so it hears the name whole instead. */
function HolderName({ name }: { name: string }) {
  const { head, tail } = nameEnds(name);
  if (!tail) return <span className="holder-name" dir="auto">{name}</span>;
  return (
    <span className={`holder-name tail-${tail.length}`} dir="auto">
      <span className="sr-only">{name}</span>
      <span aria-hidden="true" className="holder-head">{head}</span>
      <span aria-hidden="true" className="holder-tail">{tail}</span>
    </span>
  );
}

const quietTip = (lease: number) => `. Quiet: no heartbeat for over ${spanText(lease)}, its lease ran out`;

/** The ribbon itself: `spoken` before the name for a screen reader, `shown` and a "·" before it
 *  on screen, the short name cut by CSS with the whole label in the tooltip, and "quiet" in words.
 *  Where it's narrow, `shown` gives way first, the "·" stays, and the name keeps a few characters
 *  (styles.css). `children` follow the name (a card's session state, its live dot). Not a
 *  control: the card is the button, and the keyboard order stays the card's. */
function Ribbon({ who, spoken, shown, tip, quiet, state = "", children }: { who: string; spoken: string; shown?: string; tip: string; quiet: boolean; state?: string; children?: ReactNode }) {
  const names = useContext(RoleNamesContext);
  const cls = ["holder-ribbon", state && `stated ${state}`, quiet && "quiet"].filter(Boolean).join(" ");
  return (
    <span className={cls} title={tip}>
      <span className="sr-only">{`${spoken} `}</span>
      {shown && <span aria-hidden="true" className="holder-prefix">{shown}</span>}
      {shown && <span aria-hidden="true" className="holder-sep"> · </span>}
      <HolderName name={nameOf(who, names)} />
      {children}
      {quiet && <span className="holder-quiet"> · quiet</span>}
    </span>
  );
}

/** What Claude Code says the holder's session is doing (mu-171), after the name: the full words,
 *  and the short ones a narrow card shows instead (styles.css picks by the ribbon's room), both
 *  hidden from a screen reader, which hears `sessionWords`' full words after the name. Waiting
 *  for input is a needs-you block running to the card's edge, which separates it from the name,
 *  so it has no "·". */
function RibbonState({ t }: { t: Task }) {
  const full = ribbonStateWords(t, false);
  const short = ribbonStateWords(t, true);
  if (!full) return null;
  const state = t.sessionState ?? "unknown";
  return (
    <>
      <span className="sr-only">{`, ${sessionWords(t)}`}</span>
      {state !== "needs-input" && <span aria-hidden="true" className="holder-sep"> · </span>}
      <span aria-hidden="true" className={`holder-state ${state}`}>
        {full === short ? full : <><span className="holder-full">{full}</span><span className="holder-short">{short}</span></>}
      </span>
    </>
  );
}

/** A held card's ribbon (`cardRibbon` says "holder"): the name, its session's state (mu-171), and
 *  quiet once the lease runs out, judged as the card's flag judges it. Waiting for input is the owner's turn, so
 *  quiet goes to the tooltip there (the card's flag line says it too). The ribbon sits in its
 *  room, the strip above the card's top row less 8px at the start, which picks the full or short
 *  words by its width. Every mode shows it.
 *  The live dot is the token ribbon's, by the number it moves (mu-172). */
export function CardHolderRibbon({ t, now }: { t: Task; now: number }) {
  const on = useLook("holder-ribbon");
  const lease = useContext(Machine).leaseMinutes;
  const names = useContext(RoleNamesContext);
  const who = ribbonHolder(t);
  if (on !== "show" || !who) return null;
  const stated = ribbonStateWords(t, false) !== "";
  const state = stated ? (t.sessionState ?? "") : "";
  const quiet = leaseRunOut(t, now, lease);
  const tip = `Held by ${shownHolder(who, names)}${stated ? `, ${sessionWords(t)}` : ""}${quiet ? quietTip(lease) : ""}`;
  return (
    <span className="holder-room">
      <Ribbon who={who} spoken="held by" tip={tip} quiet={quiet && state !== "needs-input"} state={state}>
        <RibbonState t={t} />
      </Ribbon>
    </span>
  );
}

/** A backlog note's ribbon while a session shapes it: "shaping · Shapour"; the gold You while the
 *  owner does, as on a card the owner holds. Either says quiet once the lease ran out: reclaim
 *  ends the owner's claim on a note too. */
export function NoteHolderRibbon({ n, now }: { n: Note; now: number }) {
  const on = useLook("holder-ribbon");
  const lease = useContext(Machine).leaseMinutes;
  const names = useContext(RoleNamesContext);
  const who = n.heldBy.trim();
  if (on !== "show" || !who) return null;
  const taken = minutesSince(n.heldSince, now);
  const since = taken === null ? "" : `, taken ${agoText(taken)}`;
  const quiet = noteQuiet(n, now, lease);
  const tip = `${since}${quiet ? quietTip(lease) : ""}`;
  if (isOwner(who)) return <YouRibbon title={`You're shaping it${tip}`} spoken="being shaped by" quiet={quiet} />;
  return <Ribbon who={who} spoken="being shaped by" shown="shaping" tip={`Being shaped by ${shownHolder(who, names)}${tip}`} quiet={quiet} />;
}
