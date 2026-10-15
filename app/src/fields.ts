import { fmt, isolate, latinDigits, tl, type Calendar } from "./i18n";
import { epochDayToJalali, gregorianToEpochDay, parseJalali } from "./jalali";
import type { Task } from "./types";

/** A task's type, the core's list (ops::KINDS, task 0017) in its order: the only ones offered.
 *  Old files may still say an older type (`LEGACY_KINDS`; the core reads task as code). */
export const KINDS = ["code", "bug", "chore", "tech-debt", "spike", "design", "docs", "research"] as const;

/** The older types the queues still hold (mu-194): said in the language shown, never offered
 *  when creating or editing a task. */
export const LEGACY_KINDS = ["task", "feature", "idea"] as const;

type KnownKind = (typeof KINDS)[number] | (typeof LEGACY_KINDS)[number];
const KNOWN: readonly string[] = [...KINDS, ...LEGACY_KINDS];
const isKnownKind = (v: string): v is KnownKind => KNOWN.includes(v);

/** A type as the screen says it: a core or older type in the language shown, any other as written. */
export const kindLabel = (v: string): string => (isKnownKind(v) ? tl(`kind.${v}`) : v);

/** The estimates in one line, as the card shows them: "agent 2h · test 30m · 400k", only the parts
 *  set (the same as the core's ops::estimate_text). The words in the language shown, each figure in
 *  its digits: in Persian «ایجنت ۲ ساعت · تست ۳۰ دقیقه · ۴۰۰ هزار» (mu-151, i18n/format.ts `duration`
 *  and `tokens`). */
export function estimateText(t: Pick<Task, "estTime" | "estTestTime" | "estTokens">): string {
  return [
    t.estTime && tl("estimatePart.agent", { time: fmt.duration(t.estTime) }),
    t.estTestTime && tl("estimatePart.test", { time: fmt.duration(t.estTestTime) }),
    t.estTokens && isolate(fmt.tokens(t.estTokens)),
  ]
    .filter(Boolean)
    .join(" · ");
}

/** The fields that hold a number or a date (`set_field`'s names): typed in any digits, stored in
 *  Latin ones (mu-151), so a Persian ۲ is the 2 the core checks and keeps. */
const NUMBER_FIELDS: ReadonlySet<string> = new Set(["time", "est-time", "test-time", "est-test-time", "tokens", "est-tokens", "due"]);

/** What a field stores of what was typed: a number or date field's digits in Latin, any other
 *  field (a title, tags, a summary) as typed. */
export const storedValue = (field: string, value: string): string => (NUMBER_FIELDS.has(field) ? latinDigits(value) : value);

/** A new task's estimates and due date as stored, whatever digits were typed. */
export function storedEstimates<T extends { estTime: string; estTestTime: string; estTokens: string; due?: string }>(t: T): T {
  return { ...t, estTime: latinDigits(t.estTime), estTestTime: latinDigits(t.estTestTime), estTokens: latinDigits(t.estTokens), ...(t.due === undefined ? {} : { due: latinDigits(t.due) }) };
}

// A typed day, year first, one separator throughout (1405/07-22 is a typo, not a date). Checked
// after latinDigits, so \d (0–9 only, in JavaScript) covers the three scripts.
const TYPED_DAY = /^(\d{4})([/-])(\d{1,2})\2(\d{1,2})$/;
/** The first year read as Jalali: 1300 is 1921. No due date is set before it. */
const FIRST_JALALI_YEAR = 1300;
/** The first year read as Gregorian: Jalali 1700 is 2321, so a year from here on is one typed in
 *  Gregorian, out of habit or from a file, and never a Jalali one (mu-151, review R1). */
const FIRST_GREGORIAN_YEAR = 1700;

/** What a goal's due field stores of what was typed (mu-151): its Gregorian ISO day. The year says
 *  the calendar, whichever one is picked: 1300–1699 is Jalali (۱۴۰۵/۰۷/۲۲, 1405-7-22), 1700 on
 *  Gregorian (2026-10-14, 2026/10/14), so a date typed in the other calendar out of habit is the
 *  day meant, never one six centuries off. Any digits. Empty clears it. Refused with why: a date
 *  not written that way, a year before 1300, or a day the calendar doesn't have. */
export function dueStored(typed: string, calendar: Calendar): { iso: string } | { error: string } {
  const text = latinDigits(typed).trim();
  if (text === "") return { iso: "" };
  const example = calendar === "jalali" ? "1405/07/22" : "2026-10-14";
  const m = TYPED_DAY.exec(text);
  if (!m) return { error: tl("dueField.format", { example }) };
  const [y, mo, d] = [Number(m[1]), Number(m[3]), Number(m[4])];
  if (y < FIRST_JALALI_YEAR) return { error: tl("dueField.noYear", { date: text, example }) };
  if (y < FIRST_GREGORIAN_YEAR) {
    const r = parseJalali(text);
    return "iso" in r ? r : { error: tl("dueField.noDay", { date: text }) };
  }
  try {
    gregorianToEpochDay(y, mo, d);
  } catch {
    return { error: tl("dueField.noGregorianDay", { date: text }) };
  }
  return { iso: `${m[1]}-${String(mo).padStart(2, "0")}-${String(d).padStart(2, "0")}` };
}

/** A stored due day as the field shows it to edit: `1405/07/22` in Jalali, as stored otherwise,
 *  and as stored when it's no day (a hand-edited `2026-02-30`), so it can be seen and fixed.
 *  Latin digits: it's what's typed over. */
export function dueTyped(iso: string, calendar: Calendar): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!m || calendar !== "jalali") return iso;
  try {
    const j = epochDayToJalali(gregorianToEpochDay(Number(m[1]), Number(m[2]), Number(m[3])));
    return `${j.year}/${String(j.month).padStart(2, "0")}/${String(j.day).padStart(2, "0")}`;
  } catch {
    return iso;
  }
}

/** What the due field does on ↩ or on leaving it, from what's typed and what's saved: save the
 *  ISO day it stands for, say why not, or nothing when it's what's saved already. Text left as
 *  the field showed it is never read again (review R1): a hand-edited day that's no day stays as
 *  the file has it until it's typed over. */
export function dueSave(typed: string, calendar: Calendar, saved: string): { save: string } | { error: string } | null {
  if (latinDigits(typed).trim() === dueTyped(saved, calendar)) return null;
  const r = dueStored(typed, calendar);
  if ("error" in r) return r;
  return r.iso === saved ? null : { save: r.iso };
}

/** The due field's commit: `dueSave`'s step carried out, the day handed to `onSave` with the
 *  line it says when saved; returns the refusal to show, or "" when there's none. */
export function dueCommit(typed: string, calendar: Calendar, saved: string, onSave: (field: string, value: string, done: string) => void, done: string): string {
  const step = dueSave(typed, calendar, saved);
  if (step && "save" in step) onSave("due", step.save, done);
  return step && "error" in step ? step.error : "";
}

const heading = (line: string) => line.replace(/\r?\n$/, "").trimEnd();
const linesOf = (text: string) => text.match(/[^\n]*\n|[^\n]+$/g) ?? [];

/** Index of the last `## Log` heading line (the core's queue::log_heading), or the line count. */
function lastLog(lines: string[]): number {
  let log = lines.length;
  lines.forEach((l, i) => {
    if (heading(l) === "## Log") log = i;
  });
  return log;
}

/** The text split at the Log (the last `## Log` heading): [what's above it, the Log and after]. */
export function splitLog(text: string): [string, string] {
  const lines = linesOf(text);
  const log = lastLog(lines);
  return [lines.slice(0, log).join(""), lines.slice(log).join("")];
}

/** The task's text without the tool's Comments section, which the pane shows as a thread instead.
 *  The same section as the core's queue::comments_span: the last `## Comments` heading above the
 *  last `## Log` (anywhere, without a Log), up to the next `## ` line. */
export function withoutComments(text: string): string {
  const lines = linesOf(text);
  const log = lastLog(lines);
  let head = -1;
  for (let i = 0; i < log; i++) if (heading(lines[i]) === "## Comments") head = i;
  if (head < 0) return text;
  let end = head + 1;
  while (end < lines.length && !lines[end].startsWith("## ")) end++;
  return [...lines.slice(0, head), ...lines.slice(end)].join("");
}
