import { fmt, isolate, tl, tn } from "./i18n";
import { keyOf, taskKey } from "./taskKey";
import type { Activity, Checkin, Question, Task } from "./types";

/** The stages a task moves through, in order, with who works each by default. Mirrors the core's
 *  STAGES, stage_label and stage_role (queue.rs); its tests pin the same order. The core leaves
 *  Ready's role empty because nobody holds it; here it says so. The words are read in the language
 *  shown each time (common.json, mu-146), so they follow a switch. */
export const STAGES = [
  { id: "backlog", get label() { return tl("stage.backlog"); }, get role() { return tl("role.backlog"); } },
  { id: "shaping", get label() { return tl("stage.shaping"); }, get role() { return tl("role.shaping"); } },
  { id: "ready", get label() { return tl("stage.ready"); }, get role() { return tl("role.ready"); } },
  { id: "building", get label() { return tl("stage.building"); }, get role() { return tl("role.building"); } },
  { id: "testing", get label() { return tl("stage.testing"); }, get role() { return tl("role.testing"); } },
  { id: "your-check", get label() { return tl("stage.your-check"); }, get role() { return tl("role.your-check"); } },
  { id: "done", get label() { return tl("stage.done"); }, role: "" },
] as const;
export type Stage = (typeof STAGES)[number];
export type StageId = Stage["id"];

const stageIndex = (stage: string) => STAGES.findIndex((s) => s.id === stage);

/** The task's stage, or Backlog for a value this window doesn't know. */
export function stageOf(t: Pick<Task, "stage">): StageId {
  return STAGES[Math.max(0, stageIndex(t.stage))].id;
}
export const isStage = (stage: string) => stageIndex(stage) >= 0;
/** Position in STAGES; an unknown stage ranks as Backlog, where the board shows it. */
export const stageRank = (stage: string) => Math.max(0, stageIndex(stage));
/** "Your check"; an unknown stage reads as Backlog, like the board shows it. */
export const stageLabel = (stage: string) => STAGES[stageRank(stage)].label;

/** The stage `pass` leads to, or null from Done (or an unknown stage). The owner's own task
 *  (`human`, 0022) has no Testing or Your check: from Building on, it goes to Done, and before
 *  Building the owner's Start takes it to Building (the core's pass_change, as the owner). */
export function nextStage(stage: string, human = false): Stage | null {
  const i = stageIndex(stage);
  if (i < 0) return null;
  if (human && i >= stageIndex("building") && i < STAGES.length - 1) return STAGES[STAGES.length - 1];
  if (human && i < stageIndex("building")) return STAGES[stageIndex("building")];
  return STAGES[i + 1] ?? null;
}
/** The pass button's words: "Pass to Testing", and "Accept" in Your check; for the owner's own
 *  task, Start (To do → Doing) and Done. */
export function passLabel(stage: string, human = false): string | null {
  const next = nextStage(stage, human);
  if (!next) return null;
  if (human && next.id === "building") return tn("board", "words.pass.start");
  if (human && next.id === "done") return tn("board", "words.pass.done");
  return stage === "your-check" ? tn("board", "words.pass.accept") : tn("board", "words.pass.to", { stage: next.label });
}
/** Where `return` can send a task: every earlier stage, the nearest (the default) first. */
export function earlierStages(stage: string): Stage[] {
  return STAGES.slice(0, Math.max(0, stageIndex(stage))).reverse();
}
/** Being worked: in Building or Testing, and not paused (mu-59: a paused one waits in the Paused lane). */
export const inProgress = (t: Pick<Task, "stage" | "paused">) => (t.stage === "building" || t.stage === "testing") && !t.paused;

/** Who's on a task in its stage, in full (with the session): the claim in Shaping (0097), Ready
 *  and Building, `held-by` in Testing and Your check (the owner's when it names nobody), nobody
 *  elsewhere. The core's Task::holder; a leftover `claimed-by` on another stage shows nothing. */
export function holderOf(t: Pick<Task, "stage" | "claimedBy" | "heldBy">): string {
  switch (t.stage) {
    case "shaping":
    case "ready":
    case "building":
      return t.claimedBy.trim();
    case "testing":
      return t.heldBy.trim();
    case "your-check":
      return t.heldBy.trim() || "owner";
    default:
      return "";
  }
}

/** Who holds a task, without the session id (a role slot by its role's name now, 0112). */
export function holder(t: Pick<Task, "stage" | "claimedBy" | "heldBy">, names: RoleNames): string {
  const who = holderOf(t);
  return who ? nameOf(who, names) : "";
}

/** Who holds a task as the Table and the Graph show it: `holder`, the owner as "you" (as the
 *  card's who line says it). */
export function holderShown(t: Pick<Task, "stage" | "claimedBy" | "heldBy">, names: RoleNames): string {
  const who = holderOf(t);
  return who ? shortWho(who, names) : "";
}

/** Who else holds a task in Ready or Building, the stages that are claims; "" when nobody or the
 *  owner does. The core refuses the owner's pass while someone else holds it. */
export function heldByOther(t: Pick<Task, "stage" | "claimedBy">, names: RoleNames): string {
  if (t.stage !== "ready" && t.stage !== "building") return "";
  const who = t.claimedBy.trim();
  return !who || splitHolder(who).label === "owner" ? "" : nameOf(who, names);
}

/** A holder as the core stamps it (session.rs `stamped`): `label (main-29 · wt-0030 · session
 *  7b5e8856 · cc-hg)`, the name and checkout first, the account last; a teammate's (0090) names its
 *  agent right after the session (`session 7b5e8856 · agent a1b2c3d4`). Anything else is all label. */
export type HolderParts = { label: string; parts: string[]; id: string; agent?: string; slot?: string; account: string };
/** A role slot as a stamp stores it (0112, roles.rs `Slot::parse`): the role's id and a number 1 to 20. */
const SLOT = /^slot ([a-z]{1,20}-(?:[1-9]|1[0-9]|20))$/;
const STAMP_PART = /^[A-Za-z0-9._-]{1,40}$/;
export function splitHolder(who: string): HolderParts {
  const none = { label: who, parts: [], id: "", account: "" };
  const at = who.lastIndexOf(" (");
  if (at <= 0 || !who.endsWith(")")) return none;
  const out: HolderParts = { label: who.slice(0, at), parts: [], id: "", account: "" };
  let afterSession = false;
  for (const part of who.slice(at + 2, -1).split(" · ")) {
    const justAfterSession = afterSession;
    afterSession = false;
    if (part.startsWith("session ")) {
      const id = /^session ([0-9a-f]{8})$/.exec(part)?.[1];
      if (out.id || !id) return none;
      out.id = id;
      afterSession = true;
    } else if (part.startsWith("agent ") && out.id) {
      const agent = /^agent ([0-9a-f]{8})$/.exec(part)?.[1];
      if (!justAfterSession || !agent) return none;
      out.agent = agent;
    } else if (part.startsWith("slot ") && out.id) {
      // Once, after the session and agent, before the account (0112).
      const slot = SLOT.exec(part)?.[1];
      if (out.slot || out.account || !slot) return none;
      out.slot = slot;
    } else if (!out.id) {
      if (!STAMP_PART.test(part) || out.parts.length === 2) return none;
      out.parts.push(part);
    } else {
      const account = /^cc-([a-z0-9]{1,12})$/.exec(part)?.[1];
      if (out.account || !account) return none;
      out.account = account;
    }
  }
  return out.id ? out : none;
}

/** Is `a` the same holder as `b`? The core's session::same_holder: the same text, or the same
 *  label (or one role slot, 0112, whatever the labels say) with sessions that don't differ (one
 *  without a session matches any) and the same agent (0090: none on both is the lead; a teammate
 *  is never its lead, nor another teammate). */
export function sameHolder(a: string, b: string): boolean {
  if (a === b) return true;
  const x = splitHolder(a);
  const y = splitHolder(b);
  const oneSlot = !!x.slot && x.slot === y.slot;
  return (x.label === y.label || oneSlot) && (!x.id || !y.id || (x.id === y.id && (x.agent ?? "") === (y.agent ?? "")));
}

/** Each role's name by its id (0112), as the state carries it from roles.toml. */
export type RoleNames = Readonly<Record<string, string>>;

/** A holder's label as views show it (0112): a stamp with a role slot (`slot coder-2`) shows the
 *  role's name now and the number (`Kaveh-2`, or `کاوه-2` after a rename), so renaming a role
 *  renames it everywhere; any other holder, and a slot whose role has no name, its label as
 *  written. */
export function shownLabel(who: string, names: RoleNames): string {
  const { label, slot } = splitHolder(who);
  return slotLabel(label, slot, names);
}

/** A label that acts as role slot `slot` (0112, `coder-2`) by its role's name now (`کاوه-2`); the
 *  label as written without a slot, or for a role with no name. */
export function slotLabel(label: string, slot: string | undefined, names: RoleNames): string {
  if (!slot) return label;
  const at = slot.lastIndexOf("-");
  const id = slot.slice(0, at);
  // Only the roles' own keys: a slot `constructor-1` isn't Object's.
  const name = Object.hasOwn(names, id) ? names[id] : "";
  return name ? `${name}-${slot.slice(at + 1)}` : label;
}

/** A question's asker part (`Kaveh-2, session 7b5e8856, 2026-10-14 20:00`) with its label by the
 *  slot's name now, when the core found the slot it acts as (queue.rs `asker_slot`). */
export function askerShown(q: Pick<Question, "asker" | "slot">, names: RoleNames): string {
  const at = q.asker.indexOf(", ");
  const label = at < 0 ? q.asker : q.asker.slice(0, at);
  return slotLabel(label, q.slot, names) + q.asker.slice(label.length);
}

/** A holder's name, without the session: a role slot by its role's name now (0112), anyone else
 *  by the name before any brackets. */
export function nameOf(who: string, names: RoleNames): string {
  return splitHolder(who).slot ? shownLabel(who, names) : claimant(who);
}

/** A holder in full with its label as views show it (`shownLabel`), the stamp after it as is. */
export function shownHolder(who: string, names: RoleNames): string {
  const { label } = splitHolder(who);
  return shownLabel(who, names) + who.slice(label.length);
}

/** What a card says about the order a holder works their tasks in (mu-66): a task sent back to
 *  its builder comes first, and the holder's other tasks in Building wait behind it. Sent back
 *  from Your check (mu-101), it says who sent it ("by you" for the owner) and, in `why`, the
 *  reason: "Sent back by you: <why>", with `by` and `reason` apart so the card can isolate a
 *  right-to-left reason from the English around it. Nothing for a blocked one, which isn't held; the core's
 *  `ops::sent_back_first` says the same for `next`. */
export function sendBackLine(t: Pick<Task, "prefix" | "id" | "blocked" | "sentBack" | "sentBackBy" | "sentBackWhy" | "behind" | "behindBy">): { kind: "first" | "behind"; text: string; why: string; by: string; reason: string } | null {
  if (t.blocked) return null;
  const by = t.sentBack ? sentBackSender(t) : "";
  const reason = by ? (t.sentBackWhy ?? "") : "";
  const why = by ? tn("board", "words.sentBack.why", { by, reason }) : "";
  const sentBy = { why, by, reason };
  if (t.sentBack && !t.behind) return { kind: "first", text: tn("board", "words.sentBack.first", { key: taskKey(t), by: by || tn("board", "words.sentBack.review") }), ...sentBy };
  if (t.behind) {
    const aheadBy = (t.behindBy ?? "").trim();
    const ahead = !aheadBy ? tn("board", "words.sentBack.review") : splitHolder(aheadBy).label === "owner" ? tl("holder.you") : aheadBy;
    return { kind: "behind", text: tn("board", "words.sentBack.behind", { key: keyOf(t.prefix, t.behind), by: ahead }), ...sentBy };
  }
  return null;
}

/** Who sent a task back from Your check, as its card says it: "you" for the owner, else their
 *  label; "" for the review's send-back from Testing (session.rs is_owner). */
const sentBackSender = (t: Pick<Task, "sentBackBy" | "sentBackWhy">): string => {
  const by = (t.sentBackBy ?? "").trim();
  if (!by || !t.sentBackWhy) return "";
  return splitHolder(by).label === "owner" ? tl("holder.you") : by;
};

/** An agent's task in Testing still held by one of its builders (0047, `builders`): a session
 *  that didn't build it has to test it and pass it on (or the owner). */
export function needsTester(t: Pick<Task, "stage" | "heldBy" | "builders" | "builtBy" | "gatedBy" | "forWho">): boolean {
  const held = t.heldBy.trim();
  return t.stage === "testing" && t.forWho !== "human" && held !== "" && builders(t).some((b) => sameHolder(held, b));
}

/** Who built it, as the core's ops::tester_rule counts them: the takeover chain since the claim
 *  (0090: a successor isn't a new tester), whoever claimed it last, and whoever last passed it
 *  into Testing with the gate's line, oldest first. Every one is kept, as the core checks each:
 *  a bare label matches any stamp, so two holders that match each other can match different
 *  holders. */
function builders(t: Pick<Task, "builders" | "builtBy" | "gatedBy">): string[] {
  return [...(t.builders ?? []), t.builtBy ?? "", t.gatedBy ?? ""].map((b) => b.trim()).filter(Boolean);
}

/** `builders`, each holder named once (`sameHolder`), the first of them kept. */
function builderNames(t: Pick<Task, "builders" | "builtBy" | "gatedBy">): string[] {
  const out: string[] = [];
  for (const b of builders(t)) if (!out.some((o) => sameHolder(o, b))) out.push(b);
  return out;
}

/** A holder in a few words: "you" for the owner, else the name before any brackets (a role slot
 *  by its role's name now, 0112). */
export const shortWho = (who: string, names: RoleNames) =>
  splitHolder(who).label === "owner" ? tl("holder.you") : nameOf(who, names);

/** Who built an agent's task and who tested it (0047), from Testing on: "built by wt-a · tested
 *  by qa-0042" (Your check and Done), "built by wt-a · needs a tester" (Testing, still the
 *  builder's); "built by wt-a and wt-b" when one claimed it and another took the claim over or
 *  gated it in, "built by wt-a, wt-b and wt-c" for three. "" before Testing, for the owner's own
 *  task, or with no builder on record. `name` shows each holder (short by default; the pane names
 *  them in full). */
export function builtTested(t: Pick<Task, "stage" | "heldBy" | "builders" | "builtBy" | "gatedBy" | "testedBy" | "forWho">, name: (who: string) => string): string {
  const built = [...new Set(builderNames(t).map(name))];
  if (built.length === 0 || t.forWho === "human" || stageRank(t.stage) < stageRank("testing")) return "";
  const tested = (t.testedBy ?? "").trim();
  const second = t.stage === "testing" ? (needsTester(t) ? tn("board", "words.built.needsTester") : "") : tested ? tn("board", "words.built.testedBy", { name: name(tested) }) : "";
  // Each name of a list isolated in a right-to-left language, so the list keeps its order.
  const names = built.length > 1 ? tn("board", "words.built.and", { rest: built.slice(0, -1).map(isolate).join(tl("listSep")), last: built[built.length - 1] }) : built[0];
  return [tn("board", "words.built.by", { names }), second].filter(Boolean).join(" · ");
}

/** The card's who line (0030): who's on it, short: "wt-a · main-29 · wt-0030 · 7b5e8856", the
 *  owner as "you", "" for nobody. A session typed by hand in an older claim still shows. */
export function whoLine(t: Pick<Task, "stage" | "claimedBy" | "heldBy">, names: RoleNames): string {
  const who = holderOf(t);
  if (!who) return "";
  const { label, parts, id, slot } = splitHolder(who);
  // A role slot shows its role's name now (0112); any other label, its name before brackets.
  const name = slot ? shownLabel(who, names) : claimant(label);
  const session = id || (/\bsession ([0-9a-f]{8})\b/.exec(label)?.[1] ?? "");
  const shown = [name === "owner" ? tl("holder.you") : name, ...parts, session].filter(Boolean);
  return [...new Set(shown)].join(" · ");
}

/** "45 min", "2 h", "3 d": a span of whole minutes. */
function span(m: number): string {
  if (m < 60) return tl("time.min", { count: m });
  if (m < 1440) return tl("time.h", { count: Math.floor(m / 60) });
  return tl("time.d", { count: Math.floor(m / 1440) });
}
/** "12 min ago", "2 h ago", "3 d ago", or "just now" under a minute. */
export function agoText(m: number): string {
  if (m < 1) return tl("time.justNow");
  if (m < 60) return tl("time.minAgo", { count: m });
  if (m < 1440) return tl("time.hAgo", { count: Math.floor(m / 60) });
  return tl("time.dAgo", { count: Math.floor(m / 1440) });
}

/** Wall-clock minutes from a tool stamp to `now` (seconds since 1970), as quietMinutes counts
 *  them; null when `at` isn't a stamp. */
export function minutesSince(at: string, now: number): number | null {
  const from = stampMinutes(at);
  const wall = stampMinutes(stamp(new Date(now * 1000)));
  return from === null || wall === null ? null : Math.max(0, wall - from);
}

/** "started 2 h ago" for a task in Building or Testing, from the stage line that began it. */
export function startedAgo(t: Pick<Task, "stage" | "stageSince">, now: number): string {
  if (t.stage !== "building" && t.stage !== "testing") return "";
  const m = minutesSince(t.stageSince, now);
  return m === null ? "" : tn("board", "words.startedAgo", { ago: agoText(m) });
}

/** The latest check-in of the current Building stage (the core's Task::current_checkin). */
export function currentCheckin(t: Pick<Task, "stage" | "stageSince" | "checkins">): Checkin | null {
  if (t.stage !== "building") return null;
  return [...t.checkins].reverse().find((c) => c.at >= t.stageSince) ?? null;
}

/** A check-in in one line: "60% · gate green, writing tests · 12 min ago". */
export function checkinLine(c: Checkin, now: number): string {
  const m = minutesSince(c.at, now);
  return [c.pct === null ? "" : tn("board", "words.pct", { pct: c.pct }), isolate(c.text), m === null ? "" : agoText(m)].filter(Boolean).join(" · ");
}

/** A working session checks in at least this often (docs/PROTOCOL.md). */
export const CHECKIN_MINUTES = 30;

/** Minutes a session holding a Building task has been quiet, when that's past CHECKIN_MINUTES:
 *  since its last check-in or heartbeat (a check-in is one). Null when it isn't, when it's
 *  blocked (waiting, by tasks too), held by the owner (who doesn't check in), or held by nobody. */
export function quietCheckin(t: Pick<Task, "stage" | "blocked" | "blockedKind" | "claimedBy" | "heartbeat" | "modified" | "paused">, now: number): number | null {
  const who = t.claimedBy.trim();
  // Paused (0045): stopping is what it was asked to do.
  if (t.stage !== "building" || t.blocked || t.blockedKind || t.paused || !who || splitHolder(who).label === "owner") return null;
  const m = quietMinutes(t, now);
  return m > CHECKIN_MINUTES ? m : null;
}

/** "no check-in for 45 min". */
export const quietText = (m: number) => tn("board", "words.quiet", { span: span(m) });

/** Who held each stage from Building on, as far as the task has got, in full: the current
 *  stage's holder live, Building's earlier claimer and Testing's earlier holder from the stage
 *  lines that began them, Your check the owner's. */
export function stageHolders(t: Pick<Task, "stage" | "claimedBy" | "heldBy" | "stageHistory">): { stage: StageId; who: string }[] {
  const from: Record<string, string> = { building: "ready", testing: "building" };
  const out: { stage: StageId; who: string }[] = [];
  for (const s of ["building", "testing", "your-check"] as const) {
    const reached = stageRank(t.stage) >= stageRank(s) || t.stageHistory.some((h) => h.to === s);
    if (!reached) continue;
    const who =
      t.stage === s
        ? holderOf(t)
        : s === "your-check"
          ? "owner"
          : ([...t.stageHistory].reverse().find((h) => h.to === s && h.from === from[s])?.who ?? "");
    if (who) out.push({ stage: s, who });
  }
  return out;
}

/** The core's default lease (ops::LEASE): a claim nobody has heard from for longer may be released. */
export const LEASE_SECS = 12 * 3600;

/** A tool stamp ("2026-10-12 14:05", local time) as wall-clock minutes: minutes since
 *  1970-01-01 00:00 as if every day had 24 hours, with no time zone. Null for anything else, a
 *  date that doesn't exist included. The same count as the core's ops::stamp_minutes. */
export function stampMinutes(s: string): number | null {
  const m = /^(\d{4})-(\d{2})-(\d{2}) (\d{2}):(\d{2})$/.exec(s);
  if (!m) return null;
  const [y, mo, d, h, mi] = m.slice(1).map(Number);
  if (h > 23 || mi > 59) return null;
  const ms = Date.UTC(y, mo - 1, d, h, mi);
  const at = new Date(ms);
  if (at.getUTCFullYear() !== y || at.getUTCMonth() !== mo - 1 || at.getUTCDate() !== d) return null;
  return ms / 60000;
}

/** Whole minutes a claim has been quiet (ops::quiet_minutes): from its heartbeat to the wall
 *  clock now, both as stamps, when it has one; else since its file changed. `now` is seconds
 *  since 1970. Counting wall-clock minutes like the core keeps the two agreeing across a clock
 *  change, so Release shows exactly when the core would take it. */
export function quietMinutes(t: Pick<Task, "heartbeat" | "modified">, now: number): number {
  const beat = stampMinutes(t.heartbeat);
  const wall = stampMinutes(stamp(new Date(now * 1000)));
  if (beat !== null && wall !== null) return Math.max(0, wall - beat);
  return Math.floor(Math.max(0, now - t.modified) / 60);
}

/** A claim whose lease ran out, exactly what the core's reclaim takes: held, in Building or
 *  Shaping (0097: Shaping stays, unheld), not blocked (that's waiting, by tasks too), not the
 *  owner's own task (`for: human`, which has no lease), and quiet for longer than the lease:
 *  `leaseMinutes`, this machine's from Settings (0036). */
export function leaseRunOut(t: Pick<Task, "stage" | "blocked" | "blockedKind" | "claimedBy" | "heartbeat" | "modified" | "forWho" | "paused">, now: number, leaseMinutes = LEASE_SECS / 60): boolean {
  // A paused claim's lease doesn't run (0045), as the core's reclaim judges it.
  return (t.stage === "building" || t.stage === "shaping") && !t.blocked && !t.blockedKind && !t.paused && t.forWho !== "human" && t.claimedBy.trim() !== "" && quietMinutes(t, now) > leaseMinutes;
}

/** Runs one action at a time: a call made while one is in flight is dropped (false), so a
 *  double click can't pass a task twice. */
export function oneAtATime() {
  let busy = false;
  return async (action: () => Promise<boolean>): Promise<boolean> => {
    if (busy) return false;
    busy = true;
    try {
      return await action();
    } finally {
      busy = false;
    }
  };
}

/** A queue's task count: per-stage counts only, since "blocked" counts tasks already in a stage. */
export function queueTotal(counts: Record<string, number>): number {
  return Object.entries(counts).reduce((n, [k, v]) => (k === "blocked" ? n : n + v), 0);
}

const PRIORITY: Record<string, number> = { high: 0, P0: 0, P1: 0, normal: 1, P2: 1, low: 2, P3: 2 };

/** A priority as high, normal or low, reading P0–P3 the way the board sorts them; anything else is normal. */
export function priorityName(p: string): "high" | "normal" | "low" {
  return (["high", "normal", "low"] as const)[PRIORITY[p] ?? 1];
}

/** The board column a task sits in: its stage (Backlog for one this window doesn't know). The
 *  owner's own task (`for: human`, 0022) shares the agents' columns without their stages: To do
 *  in Ready (a stage before Building too), Doing in Building (Testing and Your check too, which it
 *  never takes), Done in Done. As human.ts `humanColumn` names them. */
export function boardColumn(t: Pick<Task, "stage" | "forWho">): StageId {
  if (t.forWho !== "human") return stageOf(t);
  if (t.stage === "done") return "done";
  return stageRank(t.stage) >= stageRank("building") ? "building" : "ready";
}

/** Tasks per column, each column in the board's sort (priority unless the Sort chips say
 *  otherwise). Blocked tasks stay in their stage; an unknown stage goes to Backlog; the owner's
 *  own tasks go where `boardColumn` puts them. A paused one (mu-59) goes to `paused`, the
 *  Paused lane beside Backlog, and to no stage's column: it keeps its stage, so resumed, it's
 *  back in its column. */
export function columns(tasks: Task[], by: BoardSort = "priority"): Record<string, Task[]> {
  const out: Record<string, Task[]> = Object.fromEntries([["paused", [] as Task[]], ...STAGES.map((s) => [s.id, [] as Task[]])]);
  for (const t of sortBoard(tasks, by)) out[t.paused ? "paused" : boardColumn(t)].push(t);
  return out;
}

// The board's orders (task 0032). The core sorts `muster list --sort` the same way (search.rs),
// and a shared fixture (search-cases.json) holds the two to the same order.

export type BoardSort = "priority" | "newest" | "oldest" | "changed" | "estimate" | "id";
/** The board's orders within each column, as chips. The Table sorts by its columns instead. */
export const BOARD_SORTS: { readonly id: BoardSort; readonly label: string; readonly title: string }[] = (["priority", "newest", "oldest", "changed", "estimate", "id"] as const).map((id) => ({
  id,
  get label() {
    return tl(`sort.${id}.label`);
  },
  get title() {
    return tl(`sort.${id}.title`);
  },
}));
export const isBoardSort = (s: unknown): s is BoardSort => BOARD_SORTS.some((x) => x.id === s);

const rank = (t: Task) => PRIORITY[t.priority] ?? 1;
// Plain code-unit order, like Rust's string order, so the CLI and the app sort alike.
const cmp = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);
const byPlace = (a: Task, b: Task) => cmp(a.queue, b.queue) || cmp(a.id, b.id);
const estimated = (t: Task) => t.estMinutes > 0 || t.estTokenCount > 0;
/** When a task finished, to compare: its move into Done to the minute, else the day the log
 *  knows (which sorts before that day's times), else "" (last). The core's search.rs `finished`. */
const finished = (t: Task) => t.doneAt || t.doneOn;

const COMPARE: Record<BoardSort, (a: Task, b: Task) => number> = {
  // Open work: needs you, then priority. Done (where priority no longer matters): latest finished
  // first (0051), then latest changed.
  priority: (a, b) => {
    const da = a.stage === "done", db = b.stage === "done";
    if (da !== db) return da ? 1 : -1;
    if (da) return cmp(finished(b), finished(a)) || b.modified - a.modified || byPlace(a, b);
    return Number(b.waitingForOwner) - Number(a.waitingForOwner) || rank(a) - rank(b) || byPlace(a, b);
  },
  // By the day it was captured; the same day, the higher id is the newer. No date goes last.
  newest: (a, b) => Number(!a.captured) - Number(!b.captured) || cmp(b.captured, a.captured) || cmp(b.id, a.id) || cmp(a.queue, b.queue),
  oldest: (a, b) => Number(!a.captured) - Number(!b.captured) || cmp(a.captured, b.captured) || byPlace(a, b),
  changed: (a, b) => b.modified - a.modified || byPlace(a, b),
  estimate: (a, b) => Number(!estimated(a)) - Number(!estimated(b)) || a.estMinutes - b.estMinutes || a.estTokenCount - b.estTokenCount || byPlace(a, b),
  id: byPlace,
};

/** A sorted copy. */
export function sortBoard(tasks: Task[], by: BoardSort): Task[] {
  return [...tasks].sort(COMPARE[by]);
}

/** "now", "5 min", "3 h", "2 d". */
export function ago(now: number, at: number): string {
  const s = Math.max(0, now - at);
  if (s < 60) return tl("time.now");
  if (s < 3600) return tl("time.min", { count: Math.floor(s / 60) });
  if (s < 86400) return tl("time.h", { count: Math.floor(s / 3600) });
  return tl("time.d", { count: Math.floor(s / 86400) });
}

/** Local time as the file contract writes it: YYYY-MM-DD HH:MM. */
export function stamp(d: Date): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

/** Who holds a claim, without the session id: "drover/main (session 3fba2222)" → "drover/main". */
export function claimant(claimedBy: string): string {
  // "drover/main (session 3fba2222)", "worktree-i18n (.claude/worktrees/i18n), session 41f7fe18" and
  // "capture (drover/main, session 349bc9c3)" all come out as the name before the first ( or ,.
  return claimedBy.split(/\s*[(,]/)[0].trim() || claimedBy.trim();
}

/** What a blocked task waits on, in a few words: its `waiting_on`, else the tasks it's blocked
 *  by (a block by tasks has no flag, 0024); "" when nothing blocks it. */
function blockedOn(t: Pick<Task, "blocked" | "blockedKind" | "waitingOn" | "blockedBy" | "prefix">): string {
  if (!t.blocked && !t.blockedKind) return "";
  return t.waitingOn || (t.blockedBy ?? []).map((b) => keyOf(t.prefix, b.id)).join(tl("listSep")) || tl("feed.something");
}

/** Recent changes: commits, plus tasks whose files changed, newest first. A task's line: its key,
 *  title and stage, what blocks it, and its log's last line. */
export function feed(activity: Activity[], tasks: Task[], limit = 60): Activity[] {
  const touched: Activity[] = tasks.map((t) => {
    const on = blockedOn(t);
    const log = t.logTail.length ? t.logTail[t.logTail.length - 1] : "";
    const params = { key: taskKey(t), title: t.title, stage: stageLabel(t.stage), on, log };
    return {
      queue: t.queue,
      at: t.modified,
      what: tl(on ? (log ? "feed.blockedLog" : "feed.blocked") : log ? "feed.taskLog" : "feed.task", params),
      source: "file",
    };
  });
  return [...activity, ...touched].sort((a, b) => b.at - a.at).slice(0, limit);
}

export const key = (t: Pick<Task, "queue" | "id">) => `${t.queue}/${t.id}`;

/** Files from a paste or drop, as [name, data URL]. Pasted images without a name get one. */
export async function readFiles(list: FileList | File[], stampText: string): Promise<[string, string][]> {
  const files = Array.from(list);
  return Promise.all(
    files.map(
      (f, i) =>
        new Promise<[string, string]>((resolve, reject) => {
          const r = new FileReader();
          r.onload = () => resolve([fileName(f, i, stampText), String(r.result)]);
          r.onerror = () => reject(r.error);
          r.readAsDataURL(f);
        }),
    ),
  );
}

export function fileName(f: { name: string; type: string }, i: number, stampText: string): string {
  if (f.name && f.name !== "image.png") return f.name;
  const ext = f.type.split("/")[1]?.replace("jpeg", "jpg") || "bin";
  return `pasted-${stampText.replace(/[ :]/g, "-")}${i ? `-${i + 1}` : ""}.${ext}`; // i18n-ignore: a file name, not text on screen
}

/** "14 h", "2 d 3 h" (8-hour agent days, as the core counts them), "45 min". */
export function fmtMinutes(m: number): string {
  if (m <= 0) return "";
  if (m < 60) return tl("time.min", { count: m });
  const h = Math.round(m / 6) / 10;
  if (h < 8) return tl("time.h", { count: h });
  const d = Math.floor(h / 8), rest = Math.round((h - d * 8) * 10) / 10;
  return rest ? tl("time.join", { a: tl("time.d", { count: d }), b: tl("time.h", { count: rest }) }) : tl("time.d", { count: d });
}
/** "3.2M tokens", "450k tokens". */
export function fmtTokens(n: number): string {
  if (n <= 0) return "";
  // The digits are the ones picked (mu-151); the k and M stay as they are.
  if (n >= 1e6) return tl("tokensShort", { amount: `${fmt.number(Math.round(n / 1e5) / 10, { useGrouping: false })}M` });
  if (n >= 1e3) return tl("tokensShort", { amount: `${fmt.number(Math.round(n / 1e3), { useGrouping: false })}k` });
  return tl("tokens", { count: n });
}
/** Totals of what's shown: "≈ 14 h · 3.2M tokens (9 of 24 estimated)". Time is agent time plus
 *  test time. */
export function estimateTotal(tasks: { estMinutes: number; estTestMinutes: number; estTokenCount: number; stage: string }[]): string {
  const open = tasks.filter((t) => t.stage !== "done");
  const est = open.filter((t) => t.estMinutes || t.estTestMinutes || t.estTokenCount);
  if (!est.length) return "";
  const m = est.reduce((a, t) => a + t.estMinutes + t.estTestMinutes, 0), k = est.reduce((a, t) => a + t.estTokenCount, 0);
  const parts = [fmtMinutes(m), fmtTokens(k)].filter(Boolean).join(" · ");
  return est.length < open.length ? tl("estimateSome", { parts, est: est.length, open: open.length }) : tl("estimate", { parts });
}
/** A token count, short and without the word: "1.2M", "70k", "3.5k", "900". The digits are the
 *  language's (۱٫۲M in Persian, mu-172); the k and M stay as they are. */
export function shortTokens(n: number): string {
  const v = Math.max(0, Math.round(n));
  const one = (x: number) => fmt.number(Math.round(x * 10) / 10, { useGrouping: false });
  if (v >= 999_500) return `${one(v / 1e6)}M`;
  if (v >= 10_000) return `${one(Math.round(v / 1e3))}k`;
  if (v >= 1000) return `${one(v / 1e3)}k`;
  return one(v);
}

/** A time in hours, never days (mu-172, D4): "45 min" under an hour, else "8 h", "26.5 h". Both
 *  sides of the Work section's Time line, its rows and Recently finished use it, so a measured
 *  time and its estimate read in the same unit: an estimate of `1d` is 8 agent-hours. */
export function fmtHours(m: number): string {
  const v = Math.max(0, Math.round(m));
  if (v < 60) return tl("time.min", { count: v });
  return tl("time.h", { count: Math.round(v / 6) / 10 });
}

/** Left-to-right isolate and its pop: each of the token ribbon's figures keeps its own order right
 *  to left. */
const LRI = "\u2066";
const PDI = "\u2069";
const ltr = (s: string) => `${LRI}${s}${PDI}`;

type TokenFields = Pick<Task, "tokensUsed" | "tokensShared" | "estTokenCount" | "live">;

/** Its counted tokens are past its estimate (both known; the estimate itself isn't over). */
export const tokensOver = (t: Pick<Task, "tokensUsed" | "estTokenCount">) => t.tokensUsed != null && t.estTokenCount > 0 && t.tokensUsed > t.estTokenCount;

/** What a card's token ribbon says (mu-172), in its parts, as the card lays them out:
 *  - `figures`: "210k / 600k" (Persian «۲۱۰k از ۶۰۰k»: a slash beside the decimal ٫ reads as
 *    another slash), each figure in a left-to-right isolate; never cut on the card.
 *  - `over`: the word past the estimate ("" when it isn't); never cut either.
 *  - `note`: the short words for what's missing ("· no est", "est · not read"); they give way on a
 *    narrow card.
 *  - `shared`: "· shared" when a shared session was left out; gives way first.
 *  - `spoken`: what a screen reader says instead, whole ("210k of 600k tokens used, over the
 *    estimate, a session shared with other tasks not counted, being used now"), `tip` the exact
 *    counts. "being used now" goes by `live`, which the caller gives as the dot does.
 *  Tokens are what `est-tokens` counts: input, output and cache writes. Read while rendering a
 *  component that uses `useT`, so the words follow a language switch; they are in `common`, there
 *  before the first paint. */
export function tokenRibbonText(t: TokenFields): { figures: string; over: string; note: string; shared: string; spoken: string; tip: string } {
  const used = t.tokensUsed ?? null;
  const est = t.estTokenCount > 0 ? t.estTokenCount : 0;
  const over = tokensOver(t) ? tl("tokenRibbon.over") : "";
  let figures: string, note: string, spoken: string, tip: string;
  if (used !== null && est) {
    figures = tl("tokenRibbon.pair", { used: ltr(shortTokens(used)), est: ltr(shortTokens(est)) });
    note = "";
    spoken = tl("tokenRibbon.spoken", { used: shortTokens(used), est: shortTokens(est) });
    tip = tl("tokenRibbon.tip", { used, est, pct: Math.round((used / est) * 100) });
  } else if (used !== null) {
    figures = ltr(shortTokens(used));
    note = tl("tokenRibbon.noEstimate");
    spoken = tl("tokenRibbon.spokenNoEstimate", { used: shortTokens(used) });
    tip = tl("tokenRibbon.tipNoEstimate", { used });
  } else if (est) {
    figures = ltr(shortTokens(est));
    note = tl("tokenRibbon.notRead");
    spoken = tl("tokenRibbon.spokenNotRead", { est: shortTokens(est) });
    tip = tl("tokenRibbon.tipNotRead", { est });
  } else {
    figures = "";
    note = tl("tokenRibbon.nothing");
    spoken = tl("tokenRibbon.spokenNothing");
    tip = tl("tokenRibbon.tipNothing");
  }
  const shared = t.tokensShared ? tl("tokenRibbon.shared") : "";
  if (over) spoken += tl("tokenRibbon.spokenOver");
  if (shared) spoken += tl("tokenRibbon.spokenShared");
  if (t.live) spoken += tl("tokenRibbon.spokenLive");
  if (shared) tip += tl("tokenRibbon.tipShared");
  return { figures, over, note, shared, spoken, tip };
}


/** A path as the window shows it (owner, 2026-10-13): under the home folder, `~/…`, so the
 *  user's name stays off the screen. Only for display: the real path stays in the tooltip and is
 *  what's copied (`copiedPath`). `home` with or without a trailing slash; empty or "/" shortens
 *  nothing. */
export function homePath(p: string, home: string): string {
  const h = home.replace(/\/+$/, "");
  if (!h) return p;
  if (p === h) return "~";
  return p.startsWith(`${h}/`) ? `~${p.slice(h.length)}` : p;
}

/** What copying a path shown by `homePath` puts on the clipboard: the real path's characters, with
 *  the home folder in place of a selected `~`. `start` and `end` are the selection's offsets in
 *  `shown` when both ends are inside it; without them, only a selection of the whole path (a
 *  triple click) is taken. Null leaves the copy as the browser makes it. */
export function copiedPath(shown: string, full: string, sel: { text: string; start?: number; end?: number }): string | null {
  if (shown === full) return null;
  if (sel.start === undefined || sel.end === undefined) return sel.text.trim() === shown ? full : null;
  if (sel.end <= sel.start) return "";
  const extra = full.length - shown.length;
  return full.slice(sel.start === 0 ? 0 : sel.start + extra, sel.end + extra);
}
