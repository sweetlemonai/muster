// Accounts and their use (task 0113): the core's accounts as `accounts_info` sends them, and the
// words and numbers of the header's meter and Settings › Accounts. The core reads the readings
// (Claude Code's status line and a run's rate_limit_event) and says what's stale; nothing here is
// guessed. No React views here; AccountMeter.tsx and Settings.tsx have those. The words are
// shell.json's `meter` and `profiles` (mu-146's layer), read in the language shown each time.
import { fmt, tn } from "./i18n";
import { ago } from "./lib";

/** One window's latest reading. `usedPercentage` may pass 100 (over the cap): clamp only to
 *  draw it. `resetsAt` in unix seconds; `stale` when it's at or before the core's now. `headroom`
 *  is what's left above the reserve, negative when into it. */
export type WindowView = { usedPercentage: number; resetsAt: number; stale: boolean; headroom: number };
export type Provider = "claude" | "codex";
/** An account's profile and its latest reading. `label` is the folder's (`mg` for
 *  `~/.claude-mg`), `name` how it's shown (`cc-mg`). */
export type AccountView = {
  provider: Provider;
  label: string;
  name: string;
  plan: string;
  roles: string[];
  models: string[];
  concurrency: number;
  reserve: number;
  paused: boolean;
  projects: string[];
  fiveHour: WindowView | null;
  sevenDay: WindowView | null;
  readingAt: number | null;
  source: "stream" | "statusline" | null;
};
export type AccountsInfo = { accounts: AccountView[]; now: number };
/** What `account_save` takes: the fields Settings › Accounts edits. */
export type ProfileSave = { label: string; plan: string; roles: string[]; models: string[]; concurrency: number; reserve: number; paused: boolean };

/** The two windows, in the order the bars stack, each with its short name in the language shown. */
export const WINDOWS = [
  {
    key: "fiveHour",
    get short() {
      return tn("shell", "meter.window.fiveHour");
    },
  },
  {
    key: "sevenDay",
    get short() {
      return tn("shell", "meter.window.sevenDay");
    },
  },
] as const;

/** The bounds the core keeps (accounts.rs): reserve percent, concurrency, the plan's length. */
export const RESERVE_MAX = 90;
export const CONCURRENCY: [number, number] = [1, 8];
export const PLAN_MAX = 80;

const clamp = (n: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, n));

/** The percent a bar shows: whole, 0 to 100, so an over-cap reading is a full bar, never wider. */
export const shownPercent = (used: number) => (Number.isFinite(used) ? Math.round(clamp(used, 0, 100)) : 0);

/** Where the reserve mark sits along the bar: where the reserve starts, 100 minus it. */
export const reserveAt = (reserve: number) => clamp(100 - reserve, 0, 100);

/** Whether the use has passed into the owner's reserve. */
export const intoReserve = (used: number, reserve: number) => used > 100 - reserve;

// The month's short name is the one Activity uses (shell.json `recent.date.month`).
const MONTHS = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"] as const;
/** Two digits, in the language's own digits. */
const pad2 = (n: number) => fmt.number(n, { minimumIntegerDigits: 2, useGrouping: false });

/** A reset time in local time: "14:20" on `now`'s day, "15 Oct 00:30" on another. */
export function resetText(resetsAt: number, now: number): string {
  const d = new Date(resetsAt * 1000);
  const today = new Date(now * 1000);
  const clock = `${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
  const sameDay = d.getFullYear() === today.getFullYear() && d.getMonth() === today.getMonth() && d.getDate() === today.getDate();
  return sameDay ? clock : tn("shell", "meter.date", { day: d.getDate(), month: tn("shell", `recent.date.month.${MONTHS[d.getMonth()]}`), clock });
}

/** When a window resets: "resets 14:20", or, for a stale one whose reset has passed, "was due to
 *  reset 19:32", so a past time never reads as the next one. */
const resetWords = (w: WindowView, now: number) => tn("shell", w.stale ? "meter.wasDue" : "meter.resets", { time: resetText(w.resetsAt, now) });
/** ", over the limit" after a use past 100%, else nothing. */
const overWords = (w: WindowView) => (w.usedPercentage > 100 ? tn("shell", "meter.over") : "");
/** A window's use: "42% used", then over the limit. */
const usedWords = (w: WindowView) => tn("shell", "meter.used", { used: shownPercent(w.usedPercentage) }) + overWords(w);

/** A bar's accessible name: "cc-mg 5 h: 42% used, resets 14:20; reserve 20%", then ", stale"
 *  and ", into the reserve" when they apply, in words since the bar only hatches or colours. */
export function barLabel(name: string, short: string, w: WindowView | null, reserve: number, now: number): string {
  if (!w) return tn("shell", "meter.barNone", { name, short, reserve });
  const tail = (w.stale ? tn("shell", "meter.tailStale") : "") + (intoReserve(w.usedPercentage, reserve) ? tn("shell", "meter.tailReserve") : "");
  return tn("shell", "meter.bar", { name, short, used: usedWords(w), reset: resetWords(w, now), reserve }) + tail;
}

/** The words beside an account's bars: the 5 h window's next reset, or stale, or no reading;
 *  then a stale weekly window and paused. `stale` when any of it is stale. */
export function rowStatus(a: AccountView, now: number): { text: string; stale: boolean } {
  const five = a.fiveHour;
  const week = a.sevenDay;
  const parts: string[] = [];
  if (!five && !week) parts.push(tn("shell", "meter.noReading"));
  else if (!five) parts.push(tn("shell", "meter.noFiveHour"));
  else parts.push(five.stale ? tn("shell", "meter.stale") : tn("shell", "meter.resets", { time: resetText(five.resetsAt, now) }));
  if (week?.stale && !five?.stale) parts.push(tn("shell", "meter.weekStale"));
  if (a.paused) parts.push(tn("shell", "meter.paused"));
  return { text: parts.join(" · "), stale: Boolean(five?.stale || week?.stale) };
}

/** What the meter row says, a line each: both bars, paused, and what a click does. Its tooltip
 *  shows them as lines. */
export function rowLines(a: AccountView, now: number): string[] {
  const bars = WINDOWS.map((w) => barLabel(a.name, w.short, a[w.key], a.reserve, now));
  return [...bars, ...(a.paused ? [tn("shell", "meter.pausedLine")] : []), tn("shell", "meter.open")];
}

/** The meter row's lines as one accessible name, one sentence each. */
export const rowName = (lines: readonly string[]) => lines.join(tn("shell", "meter.lineSep"));

/** The meter row's accessible name: its lines as sentences. */
export const rowLabel = (a: AccountView, now: number) => rowName(rowLines(a, now));

/** The headroom above the reserve, or how far the use is into it. */
export const headroomText = (h: number) => (h < 0 ? tn("shell", "meter.intoReserve", { n: Math.round(-h) }) : tn("shell", "meter.toSpare", { n: Math.round(h) }));

/** One window as Settings › Accounts says it: "5 h: 42% used, resets 14:20, 38% to spare above
 *  the reserve", over the limit and stale in words. */
export function windowLine(short: string, w: WindowView | null, now: number): string {
  if (!w) return tn("shell", "meter.windowNone", { short });
  return tn("shell", "meter.windowLine", { short, used: usedWords(w), reset: resetWords(w, now), headroom: headroomText(w.headroom) }) + (w.stale ? tn("shell", "meter.tailStale") : "");
}

/** When the latest reading came and from where: under a minute is just now. */
export function readingText(a: AccountView, now: number): string {
  if (a.readingAt === null || a.source === null) return tn("shell", "meter.readNone");
  const from = tn("shell", `meter.source.${a.source}`);
  return now - a.readingAt < 60 ? tn("shell", "meter.readJustNow", { from }) : tn("shell", "meter.readAgo", { age: ago(now, a.readingAt), from });
}

/** Seconds until the next live window resets (and turns stale), when the window asks the core
 *  again though nothing on disk changed; null when no live window is left. */
export function nextAsk(accounts: AccountView[], now: number): number | null {
  const resets = accounts.flatMap((a) => WINDOWS.map((w) => a[w.key])).filter((w): w is WindowView => w !== null && !w.stale && w.resetsAt > now);
  return resets.length ? Math.min(...resets.map((w) => w.resetsAt)) - now + 1 : null;
}

/** A list field as typed ("builder, tester"): split by commas, as the core does (a name may hold
 *  a space: "sonnet 4.5"), each trimmed, once. */
export const parseList = (text: string) => [...new Set(text.split(",").map((s) => s.trim()).filter(Boolean))];

/** Settings › Accounts' fields as typed. */
export type ProfileForm = { plan: string; roles: string; models: string; concurrency: string; reserve: string; paused: boolean };

/** The fields of an account as the form starts them. */
export const profileForm = (a: AccountView): ProfileForm => ({
  plan: a.plan,
  roles: a.roles.join(", "),
  models: a.models.join(", "),
  concurrency: String(a.concurrency),
  reserve: String(a.reserve),
  paused: a.paused,
});

const wholeIn = (text: string, lo: number, hi: number): number | null => {
  const t = text.trim();
  if (!/^\d+$/.test(t)) return null;
  const n = Number(t);
  return n >= lo && n <= hi ? n : null;
};

/** The profile to save from the form, or why not, in the bounds the core keeps. */
export function profileEdit(label: string, f: ProfileForm): { profile: ProfileSave } | { error: string } {
  const plan = f.plan.trim();
  if ([...plan].length > PLAN_MAX) return { error: tn("shell", "profiles.planMax", { max: PLAN_MAX }) };
  const reserve = wholeIn(f.reserve, 0, RESERVE_MAX);
  if (reserve === null) return { error: tn("shell", "profiles.reserveBounds", { max: RESERVE_MAX }) };
  const concurrency = wholeIn(f.concurrency, CONCURRENCY[0], CONCURRENCY[1]);
  if (concurrency === null) return { error: tn("shell", "profiles.concurrencyBounds", { min: CONCURRENCY[0], max: CONCURRENCY[1] }) };
  return { profile: { label, plan, roles: parseList(f.roles), models: parseList(f.models), concurrency, reserve, paused: f.paused } };
}

/** The label to add, typed as `mg` or `cc-mg`, or why not. The core refuses the rest with why:
 *  ~/.claude, a Claude label off the allowlist, a folder that isn't there, one already added. */
export function addEdit(typed: string): { label: string } | { error: string } {
  const t = typed.trim();
  const label = t.startsWith("cc-") ? t.slice(3).trim() : t;
  return label === "" ? { error: tn("shell", "accounts.empty") } : { label };
}
