// Your check says what to do (0043), the parts without React: each task's next step, how much
// it asks of the owner (its effort), the views a criterion names, and Accept all. Tested in
// yourCheck.test.ts.
import { blockSentence, isBlocked } from "./blockWords";
import { tl, tn, type TKey } from "./i18n";
import { key } from "./lib";
import { keyOf } from "./taskKey";
import { isWork } from "./levels";
import type { Criterion, Task } from "./types";

/** How much a task in Your check asks of the owner: nothing to try, one look in the app, or a
 *  session or terminal to run something in. */
export type Effort = "accept" | "look" | "session";
export const EFFORTS: { readonly id: Effort; readonly label: string }[] = (["accept", "look", "session"] as const).map((id) => ({
  id,
  // In the language shown, each time it's read (board.json `effort`).
  get label() {
    return tn("board", `effort.${id}`);
  },
}));

/** A criterion's own say, `— effort: look` or `— effort: session`, anywhere in it. */
const EFFORT_TAG = /\s*[—–-]*\s*\beffort:\s*(look|session)\b/i;
/** The words that mean running something outside the app. */
const SESSION_WORDS = /\b(sessions?|terminals?|commands?)\b/i;

/** The criterion without its `— effort: …` override. */
export const stripEffort = (text: string) => text.replace(EFFORT_TAG, "");

/** A criterion's words without its "— verify: …" tail, which says who checks it (here, you),
 *  and without its effort override. */
export function criterionWords(text: string): string {
  const words = stripEffort(text);
  // The last "verify:" (case-insensitive), found in the text itself so the cut lands where it is.
  const m = /verify:(?![\s\S]*verify:)/i.exec(words);
  return (m ? words.slice(0, m.index) : words).replace(/[\s—–-]+$/, "").trim();
}

/** One criterion's effort: its override when it has one, else a session when it mentions a
 *  session, a terminal or a command, else one look. */
export function criterionEffort(text: string): "look" | "session" {
  const said = EFFORT_TAG.exec(text)?.[1]?.toLowerCase();
  if (said === "look" || said === "session") return said;
  return SESSION_WORDS.test(text) ? "session" : "look";
}

/** What a task in Your check needs for its next step and effort: its criteria (the owner's, and
 *  all of them for what's proven), questions and block. */
export type StepTask = Pick<Task, "ownerCriteria" | "questions" | "blockedKind" | "blockDetail" | "blockedBy" | "criteria">;

const openOwner = (t: Pick<Task, "ownerCriteria">) => t.ownerCriteria.filter((c) => !c.ticked);
/** Open questions escalated to the owner: theirs to answer, like a criterion is theirs to try. */
const askedOwner = (t: Pick<Task, "questions">) => t.questions.filter((q) => !q.ticked && q.escalated);
/** Blocked on the owner or by a permission: a person has to act (as the core's needs_owner). */
const blockedOnOwner = (t: Pick<Task, "blockedKind">) => t.blockedKind === "you" || t.blockedKind === "permission";
/** A criterion as Your check names it: its label, else its place ("criterion 3"). */
const critName = (c: Pick<Criterion, "label" | "n">) => c.label || tn("board", "check.criterion", { n: c.n });
/** Criteria named one after another: "AC2, criterion 3". */
const critNames = (cs: Pick<Criterion, "label" | "n">[]) => cs.map(critName).join(tl("listSep"));
/** An agent's criterion not ticked: nothing proves it (mu-78). */
const unproven = (t: Pick<Task, "criteria">) => (t.criteria ?? []).filter((c) => !c.owner && !c.ticked);
/** Every criterion ticked, and each of the agents' with its proof written: proven, not only
 *  ticked. The owner's own, ticked, is their check and needs none. */
const allProven = (t: Pick<Task, "criteria">) => (t.criteria ?? []).every((c) => c.ticked && (c.owner || !!c.proof));

/** A task's effort: Needs a session when an open owner criterion does; One look for any other
 *  owner work (an open criterion, a question escalated to them, a block on them) and for an
 *  agent's criterion nobody proved (mu-78: look before accepting it); else Just accept. */
export function taskEffort(t: StepTask): Effort {
  const open = openOwner(t);
  if (open.some((c) => criterionEffort(c.text) === "session")) return "session";
  return open.length > 0 || askedOwner(t).length > 0 || blockedOnOwner(t) || unproven(t).length > 0 ? "look" : "accept";
}

/** The tasks by effort, each group in the order given. */
export function byEffort(tasks: Task[]): Record<Effort, Task[]> {
  const out: Record<Effort, Task[]> = { accept: [], look: [], session: [] };
  for (const t of tasks) out[taskEffort(t)].push(t);
  return out;
}

/** What Accept all accepts: tasks (not goals or features, which a single Accept never offers) in
 *  Your check with nothing open of the owner's, no question for them, every agent's criterion
 *  ticked, and not blocked. The core refuses the same (lib.rs `accept_all_in`). */
export const justAccept = (tasks: Task[]) =>
  tasks.filter((t) => t.stage === "your-check" && isWork(t) && !isBlocked(t) && taskEffort(t) === "accept");

/** How long the next step may be on a card; the pane shows it whole. */
export const CARD_STEP_CHARS = 90;

/** `text` in at most `max` characters, cut on a word with "…" when it's longer. */
export function cutText(text: string, max: number): string {
  if (text.length <= max) return text;
  const head = text.slice(0, max - 1);
  const space = head.lastIndexOf(" ");
  return `${(space > max / 2 ? head.slice(0, space) : head).trimEnd()}…`;
}

const NOT_VERBS = new Set(["its", "this", "is", "was", "his", "has", "as", "us", "thus", "plus", "always", "yes", "less"]);
/** "groups" → "group", "tries" → "try", "pushes" → "push": a verb the owner does, as an
 *  instruction. Null for a word that doesn't look like one. */
function base(word: string): string | null {
  const w = word.toLowerCase();
  if (word !== w || NOT_VERBS.has(w) || !/^[a-z]+s$/.test(w) || w.endsWith("ss")) return null;
  if (w.endsWith("ies") && w.length > 4) return `${w.slice(0, -3)}y`;
  if (/(sh|ch|x|z|o)es$/.test(w)) return w.slice(0, -2);
  return w.slice(0, -1);
}

/** A criterion's words as the next thing to do: "the owner groups the board and checks it"
 *  becomes "Try: group the board and check it"; any other sentence is something to check. */
export function tryText(words: string): string {
  const w = words.trim().replace(/[.\s]+$/, "");
  const m = /^(?:the\s+)?owner\s+(\S+)(.*)$/i.exec(w);
  const first = m ? base(m[1]) : null;
  if (!m || first === null) return tn("board", "check.step.check", { words: w || tn("board", "check.step.noWords") });
  const rest = m[2].replace(/\band (\S+)/g, (all, v: string) => {
    const b = base(v);
    // i18n-ignore: the criterion's own English words, with its verb as an instruction
    return b === null ? all : `and ${b}`;
  });
  return tn("board", "check.step.try", { words: `${first}${rest}` });
}

/** A next step: what it is, the words cut to fit a card, the words whole, and how many more
 *  things of the owner's are open after it. */
export type NextStep = { kind: "question" | "block" | "criterion" | "accept"; text: string; full: string; more: number };

const step = (kind: NextStep["kind"], full: string, more: number): NextStep => ({ kind, text: cutText(full, CARD_STEP_CHARS), full, more });


/** The one thing to do next for a task in Your check: answer a question escalated to the owner
 *  ("Answer Q2: …", its first line), unblock a block on them ("Unblock: …"), try the first open
 *  owner criterion as an instruction, or, with none of these, accept it (mu-78): "All 4 proven:
 *  Accept" ("ticked" when one of the agents' has no proof written), or what isn't proven ("Not proven: AC2 · …") instead of offering Accept, and
 *  "Nothing to try: Accept" when the core sent no criteria. */
export function nextStep(t: StepTask): NextStep {
  const open = openOwner(t);
  const asked = askedOwner(t);
  if (asked.length > 0) {
    const first = asked[0].text.split("\n").find((l) => l.trim())?.trim() ?? "";
    return step("question", tn("board", "check.step.answer", { n: asked[0].n ?? "?", text: first }), asked.length - 1 + open.length);
  }
  if (blockedOnOwner(t)) return step("block", tn("board", "check.step.unblock", { detail: (t.blockDetail ?? "").trim() || blockSentence(t) }), open.length);
  if (open.length === 0) {
    const all = t.criteria ?? [];
    const notYet = unproven(t);
    if (all.length === 0) return step("accept", tn("board", "check.step.nothingToTry"), 0);
    if (notYet.length > 0) return step("accept", tn("board", "check.step.notProven", { list: critNames(notYet) }), 0);
    return step("accept", tn("board", allProven(t) ? "check.step.allProven" : "check.step.allTicked", { count: all.length }), 0);
  }
  return step("criterion", tryText(criterionWords(open[0].text.replace(/^AC\d+:\s*/, ""))), open.length - 1);
}

/** What Accept does, for the pane's "What you're accepting" (mu-78): blocked (the flag: the core
 *  refuses a pass then; a wait on other tasks alone doesn't hold Accept), something not proven
 *  (said instead of offering Accept), the owner's own criteria still open (named), or every
 *  criterion proven ("ticked" when one of the agents' has no proof written). "none" when there
 *  are no criteria to prove it. */
export type AcceptWords = { kind: "blocked" | "unproven" | "yours" | "proven" | "ticked" | "none"; text: string };

export function acceptWords(t: StepTask & Pick<Task, "blocked">): AcceptWords {
  if (t.blocked) return { kind: "blocked", text: tn("board", "check.accept.blocked", { why: blockSentence(t) || tn("board", "check.accept.waitsOnSomething") }) };
  const all = t.criteria ?? [];
  if (all.length === 0) return { kind: "none", text: tn("board", "check.accept.none") };
  const notYet = unproven(t);
  if (notYet.length > 0) return { kind: "unproven", text: tn("board", "check.accept.unproven", { list: critNames(notYet) }) };
  const open = openOwner(t);
  if (open.length > 0) return { kind: "yours", text: tn("board", "check.accept.yours", { count: open.length, list: critNames(open) }) };
  if (!allProven(t)) return { kind: "ticked", text: tn("board", "check.accept.ticked") };
  return { kind: "proven", text: tn("board", "check.accept.proven") };
}

/** The app's views a criterion can name, in the app's order. "board", "table", "timeline",
 *  "graph" and "appearance" count in any case; the rest are everyday words, so only their name
 *  with its capital counts ("Help", not "helps"). */
export type NamedView = "board" | "table" | "timeline" | "graph" | "helper" | "waiting" | "activity" | "problems" | "projects" | "help" | "settings" | "appearance";
/** A view the criteria name: what finds it in a criterion's (English) words, and its name as the
 *  window says it (shell.json, the header's and Settings' own words), read each time. */
const view = (id: NamedView, find: RegExp, name: TKey<"shell">) => ({
  id,
  find,
  get label() {
    return tn("shell", name);
  },
});
export const VIEW_NAMES: { readonly id: NamedView; readonly label: string; readonly find: RegExp }[] = [
  view("board", /\bboard\b/i, "app.view.board"),
  view("table", /\btable\b/i, "app.view.table"),
  view("timeline", /\btimeline\b/i, "app.view.timeline"),
  view("graph", /\bgraph\b/i, "app.view.graph"),
  view("helper", /\bHelper\b/, "app.view.helper"),
  view("waiting", /\bWaiting for you\b/, "app.view.waiting"),
  view("activity", /\bActivity\b/, "app.view.activity"),
  view("problems", /\bProblems\b/, "app.view.problems"),
  view("projects", /\bProjects\b/, "sidebar.projects"),
  view("help", /\bHelp\b/, "app.view.help"),
  view("settings", /\bSettings\b/, "app.view.settings"),
  view("appearance", /\bappearance\b/i, "settings.groups.appearance.label"),
];
export const namedViews = (text: string): NamedView[] => VIEW_NAMES.filter((v) => v.find.test(text)).map((v) => v.id);

/** What the core says to Accept all for one project: the ids it accepted, and each it refused
 *  with why (`[id, why]`). */
export type AcceptResult = { accepted: string[]; refused: [string, string][] };

/** A refusal from the core as the app says it: without the leading id the line already names,
 *  and without command-line hints ("muster tick …"), which aren't how the app does it. */
export function refusalText(id: string, why: string): string {
  let s = why.trim().replace(new RegExp(`^${id}:\\s*`), "").replace(new RegExp(`^${id}\\s+`), "it ");
  const tick = /\bmuster tick\b/.test(s);
  if (tick) s = s.replace(/;[\s\S]*$/, "");
  s = s.replace(/\s*\(muster [^)]*\)/g, "");
  return tick ? tn("board", "check.result.tickFirst", { why: s }) : s;
}

/** Accept all: the tasks shown, one call to the core per project with exactly their ids, in the
 *  order shown. A project the core can't reach is reported, and the rest still go. `refused`
 *  is the keys (`queue/id`) of the tasks that weren't accepted. */
export async function acceptAllRun(tasks: Task[], accept: (queue: string, ids: string[]) => Promise<AcceptResult>): Promise<{ ok: boolean; text: string; refused: string[] }> {
  const byQueue = new Map<string, string[]>();
  for (const t of tasks) byQueue.set(t.queue, [...(byQueue.get(t.queue) ?? []), t.id]);
  let accepted = 0;
  const notes: string[] = [];
  const refused: string[] = [];
  for (const [queue, ids] of byQueue) {
    // Each refused task named by its key (0033), its project's prefix from the tasks shown.
    const prefix = tasks.find((t) => t.queue === queue)?.prefix;
    try {
      const r = await accept(queue, ids);
      accepted += r.accepted.length;
      for (const [id, why] of r.refused) {
        notes.push(tn("board", "check.result.refused", { project: queue, key: keyOf(prefix, id), why: refusalText(id, why) }));
        refused.push(key({ queue, id }));
      }
    } catch (e) {
      notes.push(tn("board", "check.result.unreachable", { project: queue, error: String(e) }));
      refused.push(...ids.map((id) => key({ queue, id })));
    }
  }
  if (notes.length === 0) return { ok: true, text: tn("board", "check.result.done", { count: accepted }), refused };
  const said = notes.join(tl("clauseSep"));
  return { ok: false, text: accepted === 0 ? tn("board", "check.result.noneAccepted", { notes: said }) : tn("board", "check.result.notAll", { count: accepted, notes: said }), refused };
}

/** Accept all's result line, kept by its place so it outlives the column it was clicked in:
 *  the words, the tasks it refused (`queue/id`), every task that place showed, and when (seconds
 *  since 1970, as a task's `modified`). */
export type AcceptOutcome = { ok: boolean; text: string; refused: string[]; known: string[]; at: number };

/** The result line has had its say: a task new to Your check in that place (the next change),
 *  or a task it refused has changed since (a criterion ticked, a question answered, unblocked:
 *  the problem may be fixed). The tasks it accepted leaving is what it reported, so that keeps
 *  it. `current` is the place's Your check tasks now. */
export function staleResult(r: AcceptOutcome, current: Task[]): boolean {
  const known = new Set(r.known);
  const refused = new Set(r.refused);
  return current.some((t) => !known.has(key(t)) || (refused.has(key(t)) && t.modified > r.at));
}
