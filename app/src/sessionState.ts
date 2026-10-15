import { tl, tn } from "./i18n";
import type { Task } from "./types";

// Claude Code's word on the session holding a task (0048): the core reads `claude agents --json`
// (agents.rs) and gives each task its holder's state. Only a state it could read and match says
// anything; unknown leaves the card's heartbeat lines and live dot exactly as they were.

/** What Claude Code says the holder's session is doing; absent reads as unknown. */
export type SessionState = "working" | "needs-input" | "ended" | "unknown";

/** Why a check-in isn't asked for while the session waits for input: it couldn't read it yet.
 *  "waiting for input in Claude Code: answer it there first", in the language shown. In common.json,
 *  as the sidebar's Ask all (reach.ts `cantAsk`) counts by it. */
export const answerFirst = () => tl("reach.answerFirst");

/** Whether the state says anything: working, waiting for input, or ended while in Building (the
 *  core never sends ended elsewhere: in Testing the holder is the builder that passed it,
 *  agents.rs). Decided by the state, not by its words, which may still be loading. */
const stated = (t: Pick<Task, "stage" | "sessionState">) =>
  t.sessionState === "working" || t.sessionState === "needs-input" || (t.sessionState === "ended" && t.stage === "building");

/** The card's and the pane's words for the state: "working", "waiting for input in Claude Code"
 *  (a permission prompt, a question), "session ended, task still in Building"; "" for unknown, and
 *  for ended outside Building. */
export function sessionWords(t: Pick<Task, "stage" | "sessionState">): string {
  if (!stated(t)) return "";
  switch (t.sessionState) {
    case "working":
      return tn("board", "words.session.working");
    case "needs-input":
      return tn("board", "words.session.waitingInClaude");
    default:
      return tn("board", "words.session.ended");
  }
}

/** The holder ribbon's words for the state (mu-171), full and `short` for a narrow card: "working",
 *  "waiting for input" or "needs you", "session ended" or "ended"; "" exactly where `sessionWords`
 *  says nothing, and while the task is paused (0045: its session was told to stop). The ribbon
 *  reads `sessionWords` out, so a screen reader always hears the full words. */
export function ribbonStateWords(t: Pick<Task, "stage" | "sessionState" | "paused">, short: boolean): string {
  if (t.paused || !stated(t)) return "";
  switch (t.sessionState) {
    case "working":
      return tn("board", "words.session.working");
    case "needs-input":
      return tn("board", short ? "words.session.ribbon.needsYou" : "words.session.ribbon.waiting");
    default:
      return tn("board", short ? "words.session.ribbon.endedShort" : "words.session.ribbon.ended");
  }
}

/** Settings > Agents' line (0036): whether the states are read, from the core's summary. */
export const sessionStatesLine = (summary: string) => tn("shell", "agents.sessionStates", { summary });
