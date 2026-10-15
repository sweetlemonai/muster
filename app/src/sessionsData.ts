// The Sessions view (0098): the sessions of the Claude accounts Muster reads, grouped as Claude
// Code's agents view groups them, and what each row and the recap say. Pure, so the tests check the
// list the owner sees. The app has no strings table of its own; this view's words are in WORDS,
// one place, so a translation finds them together.
import { agoText } from "./lib";
import type { SessionGroup, SessionItem, SessionRecap } from "./types";

/** Every word the view and the recap show, apart from the session's own name and messages. */
export const WORDS = {
  view: "Sessions",
  reading: "Reading sessions…",
  none: "none",
  other: "Other",
  older: (n: number) => `Older: ${n}`,
  olderTitle: "Completed more than a day ago: click to show them here",
  fewer: "Fewer",
  fewerTitle: "Fold the older ones back into their chip",
  /** A Focus chip's tooltip; no count while the first read runs (null). */
  open: (label: string, n: number | null) => (n === null ? `Open ${label} here` : `Open ${label} here (${n})`),
  noAccounts: "No Claude account to read: Settings › Agents lists which Muster reads.",
  accounts: (names: string[]) => `Read from ${names.join(" and ")} on this Mac, sent nowhere.`,
  transcript: "Transcript",
  recap: "Recap",
  close: "Close",
  readingRecap: "Reading the session's recap…",
  readingTranscript: "Reading the transcript…",
  noTasks: "No task names this session in its log or holders.",
  noHistory: "Nothing in the tasks' logs names this session.",
  noProgress: "It worked on no task, so there's no progress to show.",
  guess: "a guess",
  holds: "holds it",
  chainLabel: "Handoff chain",
} as const;

/** The three groups, in their fixed order, with the words a row's state reads as. */
export const SESSION_GROUPS: readonly { id: SessionGroup; label: string; word: string; count: string }[] = [
  { id: "needs-input", label: "Needs input", word: "needs input", count: "need input" },
  { id: "working", label: "Working", word: "working", count: "working" },
  { id: "completed", label: "Completed", word: "completed", count: "completed" },
];

/** A Completed session older than this, from its last activity, folds into the Older chip. */
export const OLDER_SECS = 24 * 3600;

/** A row's key, unique across the accounts. */
export const sessionKey = (r: { account: string; id: string }) => `${r.account}/${r.id}`;

/** What a session is called: its name, else its short id. */
export const sessionName = (r: { name: string; id: string }) => r.name || r.id;

/** Newest activity first; a session with no time last; then by name, so equal times keep a fixed order. */
function newestFirst(a: SessionItem, b: SessionItem): number {
  const d = (b.at ?? -1) - (a.at ?? -1);
  return d !== 0 ? d : sessionName(a).localeCompare(sessionName(b));
}

/** One group of the list: the rows shown, and the Completed ones older than a day (`older`, empty
 *  for the other groups), which fold into one chip. */
export type GroupRows = { rows: SessionItem[]; older: SessionItem[] };
export type Grouped = Record<SessionGroup, GroupRows>;

/** The rows by group, newest first. A Completed row with no time counts as older. */
export function groupSessions(rows: readonly SessionItem[], now: number): Grouped {
  const out: Grouped = { "needs-input": { rows: [], older: [] }, working: { rows: [], older: [] }, completed: { rows: [], older: [] } };
  for (const r of [...rows].sort(newestFirst)) {
    const old = r.group === "completed" && (r.at === null || now - r.at > OLDER_SECS);
    (old ? out[r.group].older : out[r.group].rows).push(r);
  }
  return out;
}

/** How many in each group, the older ones included. */
export const groupCount = (g: GroupRows) => g.rows.length + g.older.length;

/** The header's count: "8 need input · 0 working · 37 completed". */
export const headerCount = (g: Grouped) => SESSION_GROUPS.map((s) => `${groupCount(g[s.id])} ${s.count}`).join(" · ");

/** Whole minutes since `at`, null without a time. */
const minutesAgo = (now: number, at: number | null) => (at === null ? null : Math.max(0, Math.floor((now - at) / 60)));

/** A row's age at its inline end: "12 min ago", "" without a time. */
export function rowAge(now: number, at: number | null): string {
  const m = minutesAgo(now, at);
  return m === null ? "" : agoText(m);
}

/** A row's accessible name: "Asghar_6, completed, 11 min ago, Sidebar heading fix committed". The
 *  state is in words, never colour alone. Under All, `place` is the project as the row shows it
 *  (its label, or Other), after the name: "Asghar_6, Muster, completed, …". */
export function rowLabel(r: SessionItem, now: number, place = ""): string {
  const group = SESSION_GROUPS.find((s) => s.id === r.group)?.word ?? r.group;
  return [sessionName(r), place, group, rowAge(now, r.at), r.message].filter(Boolean).join(", ");
}

/** The key the arrows move to among `keys` (the rows on screen, in order): Down and Up one row,
 *  stopping at the ends; Home and End the first and last. Null when there's nowhere to go. */
export function nextKey(keys: readonly string[], current: string, key: string): string | null {
  if (keys.length === 0) return null;
  const at = keys.indexOf(current);
  switch (key) {
    case "ArrowDown":
      return at < 0 ? keys[0] : keys[Math.min(at + 1, keys.length - 1)];
    case "ArrowUp":
      return at < 0 ? keys[keys.length - 1] : keys[Math.max(at - 1, 0)];
    case "Home":
      return keys[0];
    case "End":
      return keys[keys.length - 1];
    default:
      return null;
  }
}

/** The handoff chain as one line: "Asghar_4 → Asghar_5 → Asghar_6", each by its name or its id. */
export const chainLine = (chain: SessionRecap["chain"]) => chain.map(sessionName).join(" → ");

/** A history line's kind, in words. */
export const HISTORY_KINDS: Record<string, string> = {
  claim: "claim",
  "check-in": "check-in",
  pass: "pass",
  return: "return",
  stage: "stage",
  takeover: "takeover",
  note: "note",
  "handoff-from": "handoff",
  "handoff-to": "handoff",
};
export const historyKind = (kind: string) => HISTORY_KINDS[kind] ?? kind;

/** The project a row says under All: its name, or Other. */
export const projectOf = (r: { project: string }) => r.project || WORDS.other;
