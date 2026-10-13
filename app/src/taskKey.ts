import type { QueueInfo, Task } from "./types";

// Task keys (task 0033): a task is shown as `mu-12`, its project's prefix and its number without
// the leading zeros, while its file, `depends:`, `parent:` and the ids the core sends keep `0012`.
// Every place that shows a task calls these, and nothing else formats one. The core's half is
// key.rs; key-cases.json holds both to the same answers. The prefix comes from the core with each
// task and queue (`prefix`).

const DIGITS = /^[0-9]+$/;
const TYPED_PREFIX = /^[A-Za-z]{2,4}$/;

/** `mu-12` for task `0012` of the project with the prefix `mu`. Without a prefix, or for an id
 *  that isn't digits, the id as it is. */
export function keyOf(prefix: string | undefined, id: string): string {
  if (!prefix || !DIGITS.test(id)) return id;
  const n = id.replace(/^0+/, "");
  return `${prefix}-${n === "" ? "0" : n}`;
}

/** A task's key: `mu-12`. */
export const taskKey = (t: Pick<Task, "prefix" | "id">) => keyOf(t.prefix, t.id);

/** The key of task `id` of the same project as `t` (what it depends on, its parent, its children). */
export const siblingKey = (t: Pick<Task, "prefix">, id: string) => keyOf(t.prefix, id);

/** What's typed for a task: its prefix (lowercased; null without one) and its four-digit id. */
export type Typed = { prefix: string | null; id: string };

/** `mu-12`, `MU-12`, `mu-0012`, `12` or `0012` (spaces around it aside); null for anything else. */
export function parseKey(input: string): Typed | null {
  const s = input.trim();
  const dash = s.indexOf("-");
  // Checked as typed, ASCII letters only, before lowercasing: toLowerCase turns some other
  // letters into ASCII ones (the Kelvin sign into k), which the core's to_ascii_lowercase doesn't.
  const typed = dash < 0 ? null : s.slice(0, dash);
  const number = dash < 0 ? s : s.slice(dash + 1);
  if (!number || number.length > 4 || !DIGITS.test(number)) return null;
  if (typed !== null && !TYPED_PREFIX.test(typed)) return null;
  return { prefix: typed === null ? null : typed.toLowerCase(), id: number.padStart(4, "0") };
}

/** Each project's prefix to its queue's name, for finding the task a key names. */
export function keyIndex(queues: Pick<QueueInfo, "name" | "prefix">[]): Map<string, string> {
  const out = new Map<string, string>();
  for (const q of queues) if (q.prefix && !out.has(q.prefix)) out.set(q.prefix, q.name);
  return out;
}

/** Where the keys of known projects are in a text, as [start, end) with what they name: `mu-12`
 *  or `MU-12` standing alone (not inside a longer word, a path or a number). */
export function keysIn(text: string, prefixes: Map<string, string>): { start: number; end: number; queue: string; id: string }[] {
  const out: { start: number; end: number; queue: string; id: string }[] = [];
  if (!prefixes.size) return out;
  for (const m of text.matchAll(/(?<![\p{L}\p{N}_\-/.])([A-Za-z]{2,4})-([0-9]{1,4})(?![\p{L}\p{N}_]|-[\p{L}\p{N}]|\.[\p{L}\p{N}])/gu)) {
    const queue = prefixes.get(m[1].toLowerCase());
    if (queue === undefined) continue;
    out.push({ start: m.index, end: m.index + m[0].length, queue, id: m[2].padStart(4, "0") });
  }
  return out;
}

/** A link to a task, as the window writes it in rendered text: `#task/<queue>/<id>`. */
export const taskHref = (queue: string, id: string) => `#task/${encodeURIComponent(queue)}/${id}`;

/** The task a `taskHref` names, or null for any other address. */
export function taskOfHref(href: string): { queue: string; id: string } | null {
  const m = /^#task\/([^/]+)\/([0-9]{4})$/.exec(href);
  if (!m) return null;
  try {
    return { queue: decodeURIComponent(m[1]), id: m[2] };
  } catch {
    return null;
  }
}
