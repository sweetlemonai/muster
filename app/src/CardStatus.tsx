import { checkinLine, currentCheckin, leaseRunOut, LEASE_SECS, quietCheckin, quietText, startedAgo, whoLine } from "./lib";
import type { Task } from "./types";
import { VisionBadge } from "./Vision";

/** Stages where someone can be on a task, so the card says who. */
const HELD = new Set(["ready", "building", "testing", "your-check"]);

/** A card's who and progress lines (0030), quiet unless something's wrong: who's on it and since
 *  when, the latest check-in, and a session gone quiet in the attention colour. Then, at any
 *  stage, its open vision conflicts (0029). `now` is the state's clock, seconds since 1970. */
export default function CardStatus({ t, now }: { t: Task; now: number }) {
  return (
    <>
      {HELD.has(t.stage) && <HeldLines t={t} now={now} />}
      <VisionBadge t={t} />
    </>
  );
}

function HeldLines({ t, now }: { t: Task; now: number }) {
  const who = whoLine(t);
  const started = startedAgo(t, now);
  const checkin = currentCheckin(t);
  const quiet = quietCheckin(t, now);
  const stale = leaseRunOut(t, now);
  const flag = [stale ? `quiet ${LEASE_SECS / 3600} h+` : "", quiet === null ? "" : quietText(quiet)].filter(Boolean).join(" · ");
  return (
    <>
      <span className={who ? "card-who" : "card-who nobody"}>{[who || "nobody on it", started].filter(Boolean).join(" · ")}</span>
      {checkin && <span className="card-checkin">{checkinLine(checkin, now)}</span>}
      {flag && <span className="card-quiet">{flag}</span>}
    </>
  );
}
