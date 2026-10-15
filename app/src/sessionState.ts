import { tn } from "./i18n";
import type { Task } from "./types";

// Claude Code's word on the session holding a task (0048): the core reads `claude agents --json`
// (agents.rs) and gives each task its holder's state. Only a state it could read and match says
// anything; unknown leaves the card's heartbeat lines and live dot exactly as they were.

/** What Claude Code says the holder's session is doing; absent reads as unknown. */
export type SessionState = "working" | "needs-input" | "ended" | "unknown";

/** The words for a session waiting on the owner in Claude Code (a permission prompt, a question). */
export const WAITING_IN_CLAUDE = "waiting for input in Claude Code";

/** Why a check-in isn't asked for while the session waits for input: it couldn't read it yet. */
export const ANSWER_FIRST = `${WAITING_IN_CLAUDE}: answer it there first`;

/** The card's and the pane's words for the state: "working", "waiting for input in Claude Code",
 *  "session ended, task still in Building"; "" for unknown, and for ended outside Building (the
 *  core never sends that: in Testing the holder is the builder that passed it, agents.rs). */
export function sessionWords(t: Pick<Task, "stage" | "sessionState">): string {
  switch (t.sessionState) {
    case "working":
      return "working";
    case "needs-input":
      return WAITING_IN_CLAUDE;
    case "ended":
      return t.stage === "building" ? "session ended, task still in Building" : "";
    default:
      return "";
  }
}

/** The holder ribbon's words for the state (mu-171), full and `short` for a narrow card: "working",
 *  "waiting for input" or "needs you", "session ended" or "ended"; "" exactly where `sessionWords`
 *  says nothing, and while the task is paused (0045: its session was told to stop). The ribbon
 *  reads `sessionWords` out, so a screen reader always hears the full words. */
export function ribbonStateWords(t: Pick<Task, "stage" | "sessionState" | "paused">, short: boolean): string {
  if (t.paused || !sessionWords(t)) return "";
  switch (t.sessionState) {
    case "working":
      return "working";
    case "needs-input":
      return short ? "needs you" : "waiting for input";
    default:
      return short ? "ended" : "session ended";
  }
}

/** Settings > Agents' line (0036): whether the states are read, from the core's summary. */
export const sessionStatesLine = (summary: string) => tn("shell", "agents.sessionStates", { summary });
