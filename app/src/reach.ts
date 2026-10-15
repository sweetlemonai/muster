import { agoText, currentCheckin, holderOf, minutesSince, quietCheckin, splitHolder, stageLabel } from "./lib";
import { ANSWER_FIRST } from "./sessionState";
import type { Task } from "./types";

// Reaching the session working on a task (0039): who can be reached, and whether a check-in was
// asked for. The core decides (ping.rs); these say the same in the window, so a button that would
// be refused is shown disabled with the core's reason, never hidden.

/** A message's length as the core counts it (ping::clean_message): characters (code points, not
 *  UTF-16 units), after line and paragraph separators become line breaks, control characters
 *  other than line breaks go, and the ends are trimmed. */
export function messageLength(text: string): number {
  const out = [...text.replace(/[\u2028\u2029]/g, "\n")].filter((c) => c === "\n" || !/\p{Cc}/u.test(c)).join("");
  return [...out.trim()].length;
}

/** Why a holder can't be reached: its claim names no session, and what the owner can do about it
 *  (mu-184): Release it, or a session under its label takes it over. `{label}` stands for the
 *  holder's label (noSession). Word for word ping::NO_SESSION; a test in ping.rs reads this line. */
export const NO_SESSION = "no session to reach: Release it, or let a new session under {label} take it over";
/** NO_SESSION for a hold in Testing: only Release, as nobody takes a hold over there. Word for word
 *  ping::NO_SESSION_HELD; a test in ping.rs reads this line. */
export const NO_SESSION_HELD = "no session to reach: Release it";
/** NO_SESSION for a holder's label (ping::no_session). */
export const noSession = (label: string) => NO_SESSION.replace("{label}", () => label.trim());
/** Why Testing has no check-in to ask for: `muster checkin` is Building's. */
export const CHECKIN_IN_TESTING = "check-ins are for Building; send a message instead";
/** The longest message (ping::MAX_MESSAGE). */
export const MAX_MESSAGE = 500;

/** Why the session on a task can't be reached, or "" when it can (ping::reach): only a holder in
 *  Building or Testing, not the owner, whose claim has a session stamp. */
export function unreachable(t: Pick<Task, "stage" | "claimedBy" | "heldBy">): string {
  if (t.stage !== "building" && t.stage !== "testing") return `${stageLabel(t.stage)} has no session working on it`;
  const who = holderOf(t);
  if (!who) return "nobody holds it";
  const { label, id } = splitHolder(who);
  if (label === "owner") return "you hold it yourself";
  if (id) return "";
  return t.stage === "testing" ? NO_SESSION_HELD : noSession(label);
}

/** Why a check-in can't be asked for, or "" when it can: reachable, in Building, and not waiting
 *  for input in Claude Code (0048), where it couldn't read the request until that's answered. */
export function cantAsk(t: Pick<Task, "stage" | "claimedBy" | "heldBy" | "sessionState">): string {
  return unreachable(t) || (t.sessionState === "needs-input" ? ANSWER_FIRST : "") || (t.stage === "testing" ? CHECKIN_IN_TESTING : "");
}

/** "asked 2 min ago" while a check-in asked for in this Building stage hasn't come; a check-in
 *  in the same minute or later answers it. "" otherwise. */
export function askedText(t: Pick<Task, "stage" | "stageSince" | "checkins" | "checkinAsked">, now: number): string {
  const asked = t.checkinAsked ?? "";
  if (t.stage !== "building" || !asked || asked < t.stageSince) return "";
  const c = currentCheckin(t);
  if (c && c.at >= asked) return "";
  const m = minutesSince(asked, now);
  return m === null ? "" : `asked ${agoText(m)}`;
}

/** A project's quiet claims (the card's "no check-in for N min"), and the ones a check-in request
 *  can reach. */
export function quietToAsk(tasks: Task[], now: number): { quiet: Task[]; reachable: Task[] } {
  const quiet = tasks.filter((t) => quietCheckin(t, now) !== null);
  return { quiet, reachable: quiet.filter((t) => cantAsk(t) === "") };
}
