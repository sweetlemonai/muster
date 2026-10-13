import { useContext } from "react";
import { BranchCardLine } from "./Branch";
import { checkinLine, currentCheckin, holderOf, leaseRunOut, quietCheckin, quietText, splitHolder, startedAgo, whoLine } from "./lib";
import { askedText } from "./reach";
import { Machine, spanText } from "./settingsData";
import type { Task } from "./types";
import { VisionBadge } from "./Vision";
import { nextStep } from "./yourCheck";

/** Stages where someone can be on a task, so the card says who. */
const HELD = new Set(["ready", "building", "testing"]);

/** A card's who and progress lines (0030), quiet unless something's wrong: who's on it and since
 *  when, the latest check-in, and a session gone quiet in the attention colour. In Your check,
 *  "Your turn" and the next step instead (0043). A task started in its own worktree (0023) adds
 *  its branch and the worktree's folder. Then, at any stage, its open vision conflicts (0029).
 *  `now` is the state's clock, seconds since 1970. */
export default function CardStatus({ t, now }: { t: Task; now: number }) {
  return (
    <>
      {HELD.has(t.stage) && <HeldLines t={t} now={now} />}
      {t.stage === "your-check" && <YourTurn t={t} />}
      <BranchCardLine t={t} />
      <VisionBadge t={t} />
    </>
  );
}

/** Your check (0043): whose turn it is (yours, unless the file names someone else) and the one
 *  thing to try, cut to fit with the whole of it in the tooltip, or "Nothing to try: Accept". */
function YourTurn({ t }: { t: Task }) {
  const yours = splitHolder(holderOf(t)).label === "owner";
  const step = nextStep(t);
  return (
    <>
      {yours ? <span className="card-who your-turn">Your turn</span> : <span className="card-who">{whoLine(t)}</span>}
      <span className="card-next" title={step.full}>{`${step.text}${step.more ? ` · ${step.more} more` : ""}`}</span>
    </>
  );
}

function HeldLines({ t, now }: { t: Task; now: number }) {
  const who = whoLine(t);
  const started = startedAgo(t, now);
  const checkin = currentCheckin(t);
  // Asked for a check-in (0039): it says when in the check-in's place until one comes.
  const asked = askedText(t, now);
  const quiet = quietCheckin(t, now);
  // This machine's lease (Settings, 0036), as the core's reclaim judges it.
  const lease = useContext(Machine).leaseMinutes;
  const stale = leaseRunOut(t, now, lease);
  const flag = [stale ? `quiet ${spanText(lease)}+` : "", quiet === null ? "" : quietText(quiet)].filter(Boolean).join(" · ");
  return (
    <>
      <span className={who ? "card-who" : "card-who nobody"}>{[who || "nobody on it", started].filter(Boolean).join(" · ")}</span>
      {asked ? <span className="card-asked">{asked}</span> : checkin && <span className="card-checkin">{checkinLine(checkin, now)}</span>}
      {flag && <span className="card-quiet">{flag}</span>}
    </>
  );
}
