// Settings › Agents › Roles (0112): the words and fields for each role's profile, as roles.toml
// holds it. No React views here; Roles.tsx has those.
import type { Role, RoleField } from "./types";

/** The fields Roles edits, in the order they show: each with its label, what it changes, and its
 *  value as the field holds it (a list comma-separated, as the core reads it back). */
export const ROLE_FIELDS: { field: RoleField; label: string; what: string; value: (r: Role) => string; size: number }[] = [
  { field: "name", label: "Name", what: "What the board, the pane, the log and muster team call it. Letters only, any script.", value: (r) => r.name, size: 14 },
  { field: "models", label: "Models", what: "Preferred models, in order: opus, sonnet, haiku, fable, or a model id.", value: (r) => r.models.join(", "), size: 24 },
  { field: "providers", label: "Providers", what: "Who may serve it: claude, codex.", value: (r) => r.providers.join(", "), size: 14 },
  { field: "slots", label: "Slots", what: "How many may run at once, 1 to 20; --role hands out the lowest free one.", value: (r) => String(r.slots), size: 4 },
  { field: "max_turns", label: "Turn cap", what: "Turns a run may take, 1 to 1000.", value: (r) => String(r.maxTurns), size: 6 },
  { field: "max_budget_usd", label: "Budget cap", what: "Dollars a run may spend, 0 to 1000.", value: (r) => String(r.maxBudgetUsd), size: 6 },
  { field: "context_cap", label: "Context cap", what: "Tokens of context before it hands off, 10000 to 1000000.", value: (r) => String(r.contextCap), size: 9 },
];

/** "1 slot", "3 slots". */
export const slotWords = (n: number) => `${n} ${n === 1 ? "slot" : "slots"}`;

/** A role's fixed parts in one line: its job, stage, permission and the stage moves it may make. */
export function roleLine(r: Role): string {
  const stage = r.stage === "any" ? "any stage" : r.stage.replace("-", " ");
  const moves = r.moves.length ? r.moves.map((m) => m.replace(">", " → ")).join(", ") : "no stage moves";
  return `${r.title} · ${stage} · ${r.permission} · ${moves}`;
}
