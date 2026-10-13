import type { Activity, Checkin, Task } from "./types";

/** The stages a task moves through, in order, with who works each by default. Mirrors the core's
 *  STAGES, stage_label and stage_role (queue.rs); its tests pin the same order. The core leaves
 *  Ready's role empty because nobody holds it; here it says so. */
export const STAGES = [
  { id: "backlog", label: "Backlog", role: "anyone" },
  { id: "shaping", label: "Shaping", role: "product + tech + UI agents" },
  { id: "ready", label: "Ready", role: "claimable" },
  { id: "building", label: "Building", role: "code agent" },
  { id: "testing", label: "Testing", role: "QA agent" },
  { id: "your-check", label: "Your check", role: "owner" },
  { id: "done", label: "Done", role: "" },
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

/** The stage `pass` leads to, or null from Done (or an unknown stage). */
export function nextStage(stage: string): Stage | null {
  const i = stageIndex(stage);
  return i < 0 ? null : STAGES[i + 1] ?? null;
}
/** The pass button's words: "Pass to Testing", and "Accept" in Your check. */
export function passLabel(stage: string): string | null {
  const next = nextStage(stage);
  if (!next) return null;
  return stage === "your-check" ? "Accept" : `Pass to ${next.label}`;
}
/** Where `return` can send a task: every earlier stage, the nearest (the default) first. */
export function earlierStages(stage: string): Stage[] {
  return STAGES.slice(0, Math.max(0, stageIndex(stage))).reverse();
}
export const inProgress = (t: Pick<Task, "stage">) => t.stage === "building" || t.stage === "testing";

/** Who's on a task in its stage, in full (with the session): the claim in Ready and Building,
 *  `held-by` in Testing and Your check (the owner's when it names nobody), nobody elsewhere. The
 *  core's Task::holder; a leftover `claimed-by` on another stage shows nothing. */
export function holderOf(t: Pick<Task, "stage" | "claimedBy" | "heldBy">): string {
  switch (t.stage) {
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

/** Who holds a task, without the session id. */
export function holder(t: Pick<Task, "stage" | "claimedBy" | "heldBy">): string {
  const who = holderOf(t);
  return who ? claimant(who) : "";
}

/** Who else holds a task in Ready or Building, the stages that are claims; "" when nobody or the
 *  owner does. The core refuses the owner's pass while someone else holds it. */
export function heldByOther(t: Pick<Task, "stage" | "claimedBy">): string {
  if (t.stage !== "ready" && t.stage !== "building") return "";
  const who = t.claimedBy.trim();
  return !who || splitHolder(who).label === "owner" ? "" : claimant(who);
}

/** A holder as the core stamps it (session.rs `stamped`): `label (main-29 · wt-0030 · session
 *  7b5e8856 · cc-hg)`, the name and checkout first, the account last. Anything else is all label. */
export type HolderParts = { label: string; parts: string[]; id: string; account: string };
const STAMP_PART = /^[A-Za-z0-9._-]{1,40}$/;
export function splitHolder(who: string): HolderParts {
  const none = { label: who, parts: [], id: "", account: "" };
  const at = who.lastIndexOf(" (");
  if (at <= 0 || !who.endsWith(")")) return none;
  const out: HolderParts = { label: who.slice(0, at), parts: [], id: "", account: "" };
  for (const part of who.slice(at + 2, -1).split(" · ")) {
    if (part.startsWith("session ")) {
      const id = /^session ([0-9a-f]{8})$/.exec(part)?.[1];
      if (out.id || !id) return none;
      out.id = id;
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

/** The card's who line (0030): who's on it, short: "wt-a · main-29 · wt-0030 · 7b5e8856", the
 *  owner as "you", "" for nobody. A session typed by hand in an older claim still shows. */
export function whoLine(t: Pick<Task, "stage" | "claimedBy" | "heldBy">): string {
  const who = holderOf(t);
  if (!who) return "";
  const { label, parts, id } = splitHolder(who);
  const name = claimant(label);
  const session = id || (/\bsession ([0-9a-f]{8})\b/.exec(label)?.[1] ?? "");
  const shown = [name === "owner" ? "you" : name, ...parts, session].filter(Boolean);
  return [...new Set(shown)].join(" · ");
}

/** "45 min", "2 h", "3 d": a span of whole minutes. */
function span(m: number): string {
  if (m < 60) return `${m} min`;
  if (m < 1440) return `${Math.floor(m / 60)} h`;
  return `${Math.floor(m / 1440)} d`;
}
/** "12 min ago", or "just now" under a minute. */
const agoText = (m: number) => (m < 1 ? "just now" : `${span(m)} ago`);

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
  return m === null ? "" : `started ${agoText(m)}`;
}

/** The latest check-in of the current Building stage (the core's Task::current_checkin). */
export function currentCheckin(t: Pick<Task, "stage" | "stageSince" | "checkins">): Checkin | null {
  if (t.stage !== "building") return null;
  return [...t.checkins].reverse().find((c) => c.at >= t.stageSince) ?? null;
}

/** A check-in in one line: "60% · gate green, writing tests · 12 min ago". */
export function checkinLine(c: Checkin, now: number): string {
  const m = minutesSince(c.at, now);
  return [c.pct === null ? "" : `${c.pct}%`, c.text, m === null ? "" : agoText(m)].filter(Boolean).join(" · ");
}

/** A working session checks in at least this often (docs/PROTOCOL.md). */
export const CHECKIN_MINUTES = 30;

/** Minutes a session holding a Building task has been quiet, when that's past CHECKIN_MINUTES:
 *  since its last check-in or heartbeat (a check-in is one). Null when it isn't, when it's
 *  blocked (waiting, by tasks too), held by the owner (who doesn't check in), or held by nobody. */
export function quietCheckin(t: Pick<Task, "stage" | "blocked" | "blockedKind" | "claimedBy" | "heartbeat" | "modified">, now: number): number | null {
  const who = t.claimedBy.trim();
  if (t.stage !== "building" || t.blocked || t.blockedKind || !who || splitHolder(who).label === "owner") return null;
  const m = quietMinutes(t, now);
  return m > CHECKIN_MINUTES ? m : null;
}

/** "no check-in for 45 min". */
export const quietText = (m: number) => `no check-in for ${span(m)}`;

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

/** A claim whose lease ran out, exactly what the core's reclaim takes: held, in Building, not
 *  blocked (that's waiting, by tasks too), and quiet for longer than the lease. */
export function leaseRunOut(t: Pick<Task, "stage" | "blocked" | "blockedKind" | "claimedBy" | "heartbeat" | "modified">, now: number): boolean {
  return t.stage === "building" && !t.blocked && !t.blockedKind && t.claimedBy.trim() !== "" && quietMinutes(t, now) > LEASE_SECS / 60;
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

/** Tasks per stage. Blocked tasks stay in their stage; an unknown stage goes to Backlog.
 *  Waiting-for-owner first, then priority, then id; Done newest first. */
export function columns(tasks: Task[]): Record<string, Task[]> {
  const out: Record<string, Task[]> = Object.fromEntries(STAGES.map((s) => [s.id, [] as Task[]]));
  for (const t of tasks) out[stageOf(t)].push(t);
  for (const [stage, list] of Object.entries(out)) {
    list.sort((a, b) =>
      stage === "done"
        ? b.modified - a.modified
        : Number(b.waitingForOwner) - Number(a.waitingForOwner) ||
          (PRIORITY[a.priority] ?? 1) - (PRIORITY[b.priority] ?? 1) ||
          a.queue.localeCompare(b.queue) ||
          a.id.localeCompare(b.id),
    );
  }
  return out;
}

/** "now", "5 min", "3 h", "2 d". */
export function ago(now: number, at: number): string {
  const s = Math.max(0, now - at);
  if (s < 60) return "now";
  if (s < 3600) return `${Math.floor(s / 60)} min`;
  if (s < 86400) return `${Math.floor(s / 3600)} h`;
  return `${Math.floor(s / 86400)} d`;
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

/** Recent changes: commits, plus tasks whose files changed, newest first. */
export function feed(activity: Activity[], tasks: Task[], limit = 60): Activity[] {
  const touched: Activity[] = tasks.map((t) => ({
    queue: t.queue,
    at: t.modified,
    what: `${t.id} ${t.title} · ${stageLabel(t.stage)}${t.blocked ? `, blocked on ${t.waitingOn || "something"}` : ""}${t.logTail.length ? ` · ${t.logTail[t.logTail.length - 1]}` : ""}`,
    source: "file",
  }));
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
  return `pasted-${stampText.replace(/[ :]/g, "-")}${i ? `-${i + 1}` : ""}.${ext}`;
}

/** "14 h", "2 d 3 h" (8-hour agent days, as the core counts them), "45 min". */
export function fmtMinutes(m: number): string {
  if (m <= 0) return "";
  if (m < 60) return `${m} min`;
  const h = Math.round(m / 6) / 10;
  if (h < 8) return `${h} h`;
  const d = Math.floor(h / 8), rest = Math.round((h - d * 8) * 10) / 10;
  return rest ? `${d} d ${rest} h` : `${d} d`;
}
/** "3.2M tokens", "450k tokens". */
export function fmtTokens(n: number): string {
  if (n <= 0) return "";
  if (n >= 1e6) return `${(Math.round(n / 1e5) / 10).toString()}M tokens`;
  if (n >= 1e3) return `${Math.round(n / 1e3)}k tokens`;
  return `${n} tokens`;
}
/** Totals of what's shown: "≈ 14 h · 3.2M tokens (9 of 24 estimated)". Time is agent time plus
 *  test time. */
export function estimateTotal(tasks: { estMinutes: number; estTestMinutes: number; estTokenCount: number; stage: string }[]): string {
  const open = tasks.filter((t) => t.stage !== "done");
  const est = open.filter((t) => t.estMinutes || t.estTestMinutes || t.estTokenCount);
  if (!est.length) return "";
  const m = est.reduce((a, t) => a + t.estMinutes + t.estTestMinutes, 0), k = est.reduce((a, t) => a + t.estTokenCount, 0);
  const parts = [fmtMinutes(m), fmtTokens(k)].filter(Boolean).join(" · ");
  return `≈ ${parts} left${est.length < open.length ? ` (${est.length} of ${open.length} open tasks estimated)` : ""}`;
}
