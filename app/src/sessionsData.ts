// The Sessions view (0098): the sessions of the Claude accounts Muster reads, grouped as Claude
// Code's agents view groups them, and what each row and the recap say. Pure, so the tests check the
// list the owner sees. Its words are board.json's "sessions" and "recap" (the Work section's where
// they're the same); WORDS reads them each time it's asked, in the language shown, so whoever
// shows them renders with useT("board") and follows a switch.
import { fmt, isolate, tl, tn } from "./i18n";
import { agoText } from "./lib";
import type { SessionGroup, SessionItem, SessionRecap } from "./types";

/** Every word the view and the recap show, apart from the session's own name and messages. */
export const WORDS = {
  get view() { return tn("board", "sessions.view"); },
  get reading() { return tn("board", "sessions.reading"); },
  get none() { return tn("board", "sessions.none"); },
  get other() { return tn("board", "sessions.other"); },
  older: (n: number) => tn("board", "sessions.older", { count: n }),
  get olderTitle() { return tn("board", "sessions.olderTitle"); },
  get fewer() { return tn("board", "sessions.fewer"); },
  get fewerTitle() { return tn("board", "sessions.fewerTitle"); },
  /** A Focus chip's tooltip; no count while the first read runs (null). */
  open: (label: string, n: number | null) => (n === null ? tn("board", "sessions.open", { label }) : tn("board", "sessions.openCount", { label, count: n })),
  get noAccounts() { return tn("board", "sessions.noAccounts"); },
  /** Each account isolated, so a list of Latin names keeps its order right to left. */
  accounts: (names: string[]) => tn("board", "sessions.accounts", { names: fmt.list(names.map(isolate)) }),
  get transcript() { return tn("board", "recap.transcript"); },
  get recap() { return tn("board", "recap.recap"); },
  get close() { return tn("board", "work.close"); },
  get readingRecap() { return tn("board", "recap.readingRecap"); },
  get readingTranscript() { return tn("board", "work.readingTranscript"); },
  get noTasks() { return tn("board", "recap.noTasks"); },
  get noHistory() { return tn("board", "recap.noHistory"); },
  get noProgress() { return tn("board", "recap.noProgress"); },
  get guess() { return tn("board", "recap.guess"); },
  get holds() { return tn("board", "work.holdsIt"); },
  /** The chain's accessible name: "Handoff chain: Asghar_4 → Asghar_5". */
  chainLabel: (chain: string) => tn("board", "recap.chainLabel", { chain }),
};

/** A group's words, by the key its words have in board.json. */
type GroupKey = "needsInput" | "working" | "completed";

/** One group: its heading (`label`), the word a row's state reads as (`word`), and `core`, the
 *  word the core gives a row whose state is just its group's, which the recap doesn't repeat. */
function group(id: SessionGroup, key: GroupKey, core: string) {
  return {
    id,
    key,
    core,
    get label() { return tn("board", `sessions.group.${key}`); },
    get word() { return tn("board", `sessions.state.${key}`); },
  };
}

/** The three groups, in their fixed order, with the words a row's state reads as. */
export const SESSION_GROUPS: readonly { readonly id: SessionGroup; readonly key: GroupKey; readonly core: string; readonly label: string; readonly word: string }[] = [
  // i18n-ignore: the core's words for a state, compared, never shown
  group("needs-input", "needsInput", "needs input"),
  group("working", "working", "working"),
  group("completed", "completed", "completed"),
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
export const headerCount = (g: Grouped) => SESSION_GROUPS.map((s) => tn("board", `sessions.count.${s.key}`, { count: groupCount(g[s.id]) })).join(" · ");

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
  return [sessionName(r), place, group, rowAge(now, r.at), r.message].filter(Boolean).join(tl("listSep"));
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

/** The handoff chain as one line: "Asghar_4 → Asghar_5 → Asghar_6", each by its name or its id
 *  (the arrow points the way the language reads). */
export const chainLine = (chain: SessionRecap["chain"]) => chain.map((l) => isolate(sessionName(l))).join(tn("board", "sessions.chainArrow"));

/** A history line's kind, by the core's name for it: its key in board.json's recap.kind. */
const HISTORY_KINDS: Record<string, "claim" | "checkIn" | "pass" | "return" | "stage" | "takeover" | "note" | "handoff"> = {
  claim: "claim",
  "check-in": "checkIn",
  pass: "pass",
  return: "return",
  stage: "stage",
  takeover: "takeover",
  note: "note",
  "handoff-from": "handoff",
  "handoff-to": "handoff",
};
/** A history line's kind, in words; one the window doesn't know as the core named it. */
export const historyKind = (kind: string) => {
  const key = HISTORY_KINDS[kind];
  return key ? tn("board", `recap.kind.${key}`) : kind;
};

/** The project a row says under All: its name, or Other. */
export const projectOf = (r: { project: string }) => r.project || WORDS.other;
