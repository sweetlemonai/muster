import { stageLabel } from "./lib";
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
 *  "session ended, task still in Building"; "" for unknown. */
export function sessionWords(t: Pick<Task, "stage" | "sessionState">): string {
  switch (t.sessionState) {
    case "working":
      return "working";
    case "needs-input":
      return WAITING_IN_CLAUDE;
    case "ended":
      return `session ended, task still in ${stageLabel(t.stage)}`;
    default:
      return "";
  }
}

/** Claude Code said something about this task's session (working, waiting or ended). */
export const sessionKnown = (t: Pick<Task, "sessionState">) => t.sessionState === "working" || t.sessionState === "needs-input" || t.sessionState === "ended";

/** Settings > Agents' line (0036): whether the states are read, from the core's summary. */
export const sessionStatesLine = (summary: string) => `Claude Code's session states: ${summary}`;
