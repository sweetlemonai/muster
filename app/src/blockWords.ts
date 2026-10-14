import { isStage, stageLabel } from "./lib";
import { keyOf, parseKey } from "./taskKey";
import type { BlockedBy, Question, Task } from "./types";

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

type Block = Pick<Task, "blockedKind" | "blockDetail" | "blockedBy" | "prefix"> & Partial<Pick<Task, "questions">>;

/** How each block looks (0060): the words it starts with, and whether the owner acts on it. Those
 *  (your answer, your action, a permission) take the needs-you orange with the red block mark; the
 *  rest clear without the owner (a question capture still has, other tasks, something outside) and
 *  are neutral with a small red mark. The one table for the card, the pane, the table, the
 *  timeline and Waiting for you; the core's block.rs `look` and `lead` say the same. */
export const BLOCK_LOOKS = {
  answer: { lead: "Your answer", owner: true },
  action: { lead: "Your action", owner: true },
  permission: { lead: "Permission", owner: true },
  question: { lead: "Question", owner: false },
  tasks: { lead: "After", owner: false },
  outside: { lead: "Outside", owner: false },
  other: { lead: "Blocked", owner: false },
} as const;
export type BlockLook = keyof typeof BLOCK_LOOKS;

/** A block's look, from its kind and, for a question, the question it waits on: an open one
 *  escalated to the owner is theirs to answer. */
export function blockLook(kind: string, asked?: Question): BlockLook {
  switch (kind) {
    case "question": return asked?.escalated && !asked.ticked ? "answer" : "question";
    case "you": return "action";
    case "permission":
    case "tasks":
    case "outside": return kind;
    default: return "other";
  }
}

/** The question a question block waits on, by the number in its detail ("Q2"; block.rs `asked`). */
export function askedOf(t: Block): Question | undefined {
  const n = t.blockedKind === "question" ? /^[Qq](\d{1,6})$/.exec((t.blockDetail ?? "").trim())?.[1] : undefined;
  return n === undefined ? undefined : t.questions?.find((q) => q.n === Number(n));
}

/** The words a block of `kind` starts with, bold on screen (a question's, without the question). */
export const blockLead = (kind: string): string => BLOCK_LOOKS[blockLook(kind)].lead;

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

/** The look, its lead, and what follows it (block.rs `words`): the tasks it waits on (`tasks`, for
 *  links), a question's number and first line ("Q2 which colour?", and "waiting on capture" while
 *  capture has it; `asked` is that question), or the detail. The short form, the card's, names
 *  tasks by key only and cuts a detail at 60 characters. */
export function blockWords(t: Block, short = false): { look: BlockLook; lead: string; text: string; tasks: BlockedBy[]; asked: Question | undefined } {
  const asked = askedOf(t);
  const look = blockLook(t.blockedKind ?? "", asked);
  const lead = BLOCK_LOOKS[look].lead;
  const by = t.blockedBy ?? [];
  const first = asked?.text.split("\n")[0].trim() ?? "";
  const detail = [(t.blockDetail ?? "").trim(), first].filter(Boolean).join(" ");
  const shown = short ? cut(detail) : detail;
  if (look === "tasks" && by.length > 0) {
    return { look, lead, text: by.map((b) => (short ? keyOf(t.prefix, b.id) : blockedByText(b, t.prefix))).join(", "), tasks: by, asked };
  }
  if (look === "question" && !short) return { look, lead, text: shown ? `${shown}, waiting on capture` : "waiting on capture", tasks: [], asked };
  return { look, lead, text: shown, tasks: [], asked };
}

/** The sentence, saying plainly what it waits for: "After mu-14 (Building, codex)", "Your action:
 *  set the API key", "Your answer: Q2 which colour?"; "" when nothing blocks it. */
export function blockSentence(t: Block, short = false): string {
  if (!t.blockedKind) return "";
  const { look, lead, text, tasks } = blockWords(t, short);
  if (look === "tasks") return tasks.length > 0 ? `${lead} ${text}` : `${lead} other tasks${text ? `: ${text}` : ""}`;
  return text ? `${lead}: ${text}` : lead;
}

/** Blocked, by the flag or by tasks it depends on that aren't merged yet (the core's `blockedKind`). */
export const isBlocked = (t: Pick<Task, "blocked" | "blockedKind">) => t.blocked || !!t.blockedKind;

/** Blocked in red (0060): by the flag, or by any kind but tasks. A wait on other tasks alone clears
 *  by itself, so it is neutral (CardLook's cardClass checks the same). */
export const blockedRed = (t: Pick<Task, "blocked" | "blockedKind">) => t.blocked || (!!t.blockedKind && t.blockedKind !== "tasks");

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
