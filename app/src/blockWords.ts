import { isStage, stageLabel } from "./lib";
import { keyOf, parseKey } from "./taskKey";
import type { BlockedBy, Task } from "./types";

// Why a task is blocked, in words (task 0024). The core's block.rs says the same words: its
// `sentence` and this file's `blockSentence` are tested on the same cases, so the card, the pane,
// the table and the timeline read like `muster list` and INDEX.md.

/** The kinds, in the order the Block form shows them, with what each asks for. */
export const BLOCK_KINDS = [
  { id: "tasks", label: "tasks", need: "Tasks by key or number, like 14, 16" },
  { id: "you", label: "you", need: "What you have to do or decide" },
  { id: "permission", label: "permission", need: "What access it lacks, and where to grant it" },
  { id: "question", label: "question", need: "Pick the open question it waits on" },
  { id: "outside", label: "outside", need: "What it waits on outside" },
] as const;
export type BlockKind = (typeof BLOCK_KINDS)[number]["id"];

/** The most of a detail the card's short form shows, in characters (block.rs SHORT). */
const SHORT = 60;

type Block = Pick<Task, "blockedKind" | "blockDetail" | "blockedBy" | "prefix">;

/** The words a block's sentence starts with, bold on screen. */
export function blockLead(kind: string): string {
  switch (kind) {
    case "tasks": return "Blocked by tasks";
    case "you": return "Blocked on you";
    case "permission": return "Blocked by permission";
    case "question": return "Blocked by a question";
    case "outside": return "Blocked by outside";
    default: return "Blocked";
  }
}

/** `text` cut to SHORT characters (code points, as the core counts them), with "…" when longer. */
function cut(text: string): string {
  const chars = Array.from(text);
  return chars.length <= SHORT ? text : `${chars.slice(0, SHORT).join("").trimEnd()}…`;
}

/** One task as the full sentence names it, by its key with its project's `prefix` (0033): "mu-14
 *  (Building, codex)", "mu-16 (Ready)", "mu-99". */
export function blockedByText(b: BlockedBy, prefix?: string): string {
  const k = keyOf(prefix, b.id);
  if (!isStage(b.stage)) return k;
  return `${k} (${stageLabel(b.stage)}${b.holder ? `, ${b.holder}` : ""})`;
}

/** The lead, and what follows it (block.rs `words`); `tasks` are the tasks the text names, for
 *  links. The short form, the card's, names tasks by id only and cuts a detail at 60 characters. */
export function blockWords(t: Block, short = false): { lead: string; text: string; tasks: BlockedBy[] } {
  const kind = t.blockedKind ?? "";
  const detail = (t.blockDetail ?? "").trim();
  const by = t.blockedBy ?? [];
  const shown = short ? cut(detail) : detail;
  const lead = blockLead(kind);
  if (kind === "tasks" && by.length > 0) {
    return { lead, text: by.map((b) => (short ? keyOf(t.prefix, b.id) : blockedByText(b, t.prefix))).join(", "), tasks: by };
  }
  if (kind === "question" && !short) return { lead, text: shown ? `${shown}, waiting on capture` : "waiting on capture", tasks: [] };
  return { lead, text: shown, tasks: [] };
}

/** The sentence, kind first ("Blocked on you: set the API key"); "" when nothing blocks it. */
export function blockSentence(t: Block, short = false): string {
  if (!t.blockedKind) return "";
  const { lead, text } = blockWords(t, short);
  return text ? `${lead}: ${text}` : lead;
}

/** Blocked, by the flag or by tasks it depends on that aren't merged yet (the core's `blockedKind`). */
export const isBlocked = (t: Pick<Task, "blocked" | "blockedKind">) => t.blocked || !!t.blockedKind;

/** What the Block form sends for a kind and what was typed or picked, or why it can't yet. Tasks
 *  are typed as keys of the task's own project (`prefix`, 0033) or numbers, and sent as ids. */
export function blockRequest(kind: BlockKind, detail: string, prefix?: string): { ok: true; kind: BlockKind; detail: string } | { ok: false; why: string } {
  const d = detail.trim();
  const need = BLOCK_KINDS.find((k) => k.id === kind)?.need ?? "";
  if (kind !== "tasks") return d !== "" ? { ok: true, kind, detail: d } : { ok: false, why: need };
  const typed = d.split(/[\s,]+/).filter(Boolean).map(parseKey);
  const ids = typed.flatMap((k) => (k && (k.prefix === null || k.prefix === prefix) ? [k.id] : []));
  return ids.length > 0 && ids.length === typed.length ? { ok: true, kind, detail: ids.join(", ") } : { ok: false, why: need };
}
