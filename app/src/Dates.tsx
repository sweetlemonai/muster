// Stored dates as people read them (mu-151): in the calendar and digits Settings › Language
// picked, each in a `<time>` that carries what's stored, for VoiceOver and for copying.
import { fmt, Trans } from "./i18n";

/** A goal's due date: "due October 31, 2026", «سررسید ۹ آبان ۱۴۰۵». */
export function DueDate({ iso }: { iso: string }) {
  return <Trans ns="common" k="common:due" params={{ date: fmt.isoDay(iso) }} components={{ time: <time dateTime={iso} /> }} />;
}

const STAMP = /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/;
const DAY = /^\d{4}-\d{2}-\d{2}$/;

/** A stored stamp (`2026-10-14 20:22`) or day (`2026-10-14`) as a `<time datetime>` takes it
 *  (`2026-10-14T20:22`); undefined for anything else, so no `<time>` claims a date it hasn't. */
export const stampDateTime = (at: string): string | undefined => (STAMP.test(at) ? at.replace(" ", "T") : DAY.test(at) ? at : undefined);

/** A tool stamp (`2026-10-14 20:22`, local time) as people read it: «۲۲ مهر ۱۴۰۵ ۲۰:۲۲», and as
 *  stored in Latin digits and Gregorian (fmt.stamp); without `year`, the year left out. Anything
 *  that isn't a stamp shows as written. Isolated (`dir="auto"`, review R3): its day, month, year
 *  and clock keep their order in a line that starts with a Latin name, «Tester_51 · ۲۲ مهر ۱۴۰۵». */
export function Stamp({ at, year = false, className }: { at: string; year?: boolean; className?: string }) {
  if (!STAMP.test(at)) return <span className={className}>{at}</span>;
  return <time className={className} dateTime={stampDateTime(at)} dir="auto">{fmt.stamp(at, year)}</time>;
}
