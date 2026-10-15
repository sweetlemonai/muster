import { useContext } from "react";
import { BranchCardLine } from "./Branch";
import { Trans, useT } from "./i18n";
import { builtTested, checkinLine, currentCheckin, holderOf, leaseRunOut, needsTester, sendBackLine, quietCheckin, quietText, shortWho, splitHolder, startedAgo, whoLine } from "./lib";
import { askedText } from "./reach";
import { finishText } from "./recentData";
import { useLook, type CardLines } from "./modes";
import { sessionWords } from "./sessionState";
import { Machine, RoleNamesContext, spanText } from "./settingsData";
import type { Task } from "./types";
import { ReviewBadge } from "./Review";
import { VisionBadge } from "./Vision";
import { nextStep } from "./yourCheck";

/** Stages where someone can be on a task, so the card says who: Shaping (0097) only while held,
 *  since most Shaping tasks have no shaper on them. */
const HELD = new Set(["ready", "building", "testing"]);
const held = (t: Task) => HELD.has(t.stage) || (t.stage === "shaping" && t.claimedBy.trim() !== "");

/** A card's who and progress lines (0030), quiet unless something's wrong: who's on it and since
 *  when (what Claude Code says its session is doing, 0048, is on the holder ribbon, mu-171),
 *  the latest check-in, and a session gone quiet in the attention colour. In Your check,
 *  "Your turn" and the next step instead (0043), then who built and who tested it; in Testing,
 *  "needs a tester" on its own line while a builder still holds it (0047); in Done, when it was
 *  done (0051). A task started in its own worktree (0023) adds its branch and the worktree's
 *  folder. Then, at any stage, its open vision conflicts (0029), and in Building and Testing where
 *  its review of record stands (0027).
 *  `lines` is the display mode's (0038): Standard's brief card keeps only what says something is
 *  wrong or waits on you; Focus keeps who's on it, or Your turn, and the alarms (needs a tester
 *  among them: a builder holding Testing isn't work going on). The Done line is
 *  the Done card's status line, read from the modes' table on its own.
 *  `named`: the card's ribbon names the holder (0097), so the who line keeps only the rest.
 *  `now` is the state's clock, seconds since 1970. */
export default function CardStatus({ t, now, lines = "all", named = false }: { t: Task; now: number; lines?: CardLines; named?: boolean }) {
  const doneLine = useLook("done-line");
  return (
    <>
      {held(t) && <HeldLines t={t} now={now} lines={lines} named={named} />}
      {t.stage === "your-check" && <YourTurn t={t} lines={lines} />}
      {doneLine === "show" && <DoneLine t={t} now={now} />}
      {lines === "all" && <BranchCardLine t={t} />}
      {lines !== "status" && <VisionBadge t={t} />}
      {lines !== "status" && <ReviewBadge t={t} />}
    </>
  );
}

/** In Done (0051): when it was done, "done 4 min ago", as Recently finished says it; the stamp
 *  in the tooltip. Nothing when the log doesn't know. */
function DoneLine({ t, now }: { t: Task; now: number }) {
  const tr = useT("board");
  const text = finishText(t, now);
  return text ? <span className="card-done" title={tr("card.doneTitle", { at: t.doneAt || t.doneOn })}>{text}</span> : null;
}

/** Your check (0043): whose turn it is (yours, unless the file names someone else) and the one
 *  thing to try, cut to fit with the whole of it in the tooltip, or "All 4 proven: Accept"; then
 *  what was built (mu-78), the builder's one line cut to the card's width, whole in the tooltip. */
function YourTurn({ t, lines }: { t: Task; lines: CardLines }) {
  const tr = useT("board");
  const yours = splitHolder(holderOf(t)).label === "owner";
  const step = nextStep(t);
  // Role slots by their role's name now (0112).
  const names = useContext(RoleNamesContext);
  const built = builtTested(t, (w) => shortWho(w, names));
  return (
    <>
      {yours ? <span className="card-who your-turn">{tr("card.yourTurn")}</span> : <span className="card-who" dir="auto">{whoLine(t, names)}</span>}
      {lines !== "status" && <span className="card-next" title={step.full}>{step.more ? tr("card.nextMore", { step: step.text, count: step.more }) : step.text}</span>}
      {lines !== "status" && t.summary && <span className="card-summary" dir="auto" title={t.summary}>{t.summary}</span>}
      {lines === "all" && built && <span className="card-built">{built}</span>}
    </>
  );
}

/** With `named`, the who line leaves out the name the ribbon says ("wt-0097 · a82f2534 · started
 *  2 h ago"), and a held card with nothing more to say has no who line, never "nobody on it". */
function HeldLines({ t, now, lines, named }: { t: Task; now: number; lines: CardLines; named: boolean }) {
  const tr = useT("board");
  const line = whoLine(t, useContext(RoleNamesContext));
  const who = named ? line.split(" · ").slice(1).join(" · ") : line;
  const started = startedAgo(t, now);
  const whoText = [who || (named ? "" : tr("card.nobody")), started].filter(Boolean).join(" · ");
  const checkin = currentCheckin(t);
  // Asked for a check-in (0039): it says when in the check-in's place until one comes.
  const asked = askedText(t, now);
  const quiet = quietCheckin(t, now);
  // This machine's lease (Settings, 0036), as the core's reclaim judges it.
  const lease = useContext(Machine).leaseMinutes;
  const stale = leaseRunOut(t, now, lease);
  const flag = [stale ? tr("card.quietLease", { span: spanText(lease) }) : "", quiet === null ? "" : quietText(quiet)].filter(Boolean).join(" · ");
  // What Claude Code says the session is doing (0048) is on the holder ribbon (mu-171); when it
  // says something, the flag stays, quiet.
  const session = sessionWords(t);
  const order = sendBackLine(t);
  return (
    <>
      {lines !== "brief" && whoText !== "" && <span className={who || named ? "card-who" : "card-who nobody"} dir="auto">{whoText}</span>}
      {needsTester(t) && <span className="card-tester">{tr("words.built.needsTester")}</span>}
      {order && <span className={order.kind === "first" ? "card-first" : "card-behind"}>{order.text}</span>}
      {order?.why && (
        <span className="card-first card-why" title={order.why}>
          <Trans ns="board" k="card.sentBack" params={{ by: order.by, reason: order.reason }} components={{ reason: <bdi /> }} />
        </span>
      )}
      {lines === "all" && (asked ? <span className="card-asked">{asked}</span> : checkin && <span className="card-checkin">{checkinLine(checkin, now)}</span>)}
      {flag && <span className={session ? "card-quiet calm" : "card-quiet"}>{flag}</span>}
    </>
  );
}
