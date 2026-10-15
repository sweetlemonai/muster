// Settings › Agents › Roles (0112): the words and fields for each role's profile, as roles.toml
// holds it. No React views here; Roles.tsx has those.
import { tl, tn } from "./i18n";
import type { Role, RoleField } from "./types";

/** The fields Roles edits, in the order they show: each with its label, what it changes (both in
 *  the language shown, shell.json `roles.field`, mu-146), and its value as the field holds it (a
 *  list comma-separated, as the core reads it back). */
type FieldSpec = { readonly field: RoleField; readonly label: string; readonly what: string; value: (r: Role) => string; size: number };

const fieldOf = (field: RoleField, value: (r: Role) => string, size: number): FieldSpec => ({
  field,
  value,
  size,
  get label() {
    return tn("shell", `roles.field.${field}.label`);
  },
  get what() {
    return tn("shell", `roles.field.${field}.what`);
  },
});

export const ROLE_FIELDS: FieldSpec[] = [
  fieldOf("name", (r) => r.name, 14),
  fieldOf("models", (r) => r.models.join(", "), 24),
  fieldOf("providers", (r) => r.providers.join(", "), 14),
  fieldOf("slots", (r) => String(r.slots), 4),
  fieldOf("max_turns", (r) => String(r.maxTurns), 6),
  fieldOf("max_budget_usd", (r) => String(r.maxBudgetUsd), 6),
  fieldOf("context_cap", (r) => String(r.contextCap), 9),
];

/** "1 slot", "3 slots". */
export const slotWords = (n: number) => tn("shell", "roles.slots", { count: n });

/** A role's fixed parts in one line: its job, stage, permission and the stage moves it may make.
 *  The stage and moves are the protocol's ids, as roles.toml has them. */
export function roleLine(r: Role): string {
  const stage = r.stage === "any" ? tn("shell", "roles.anyStage") : r.stage.replace("-", " ");
  const moves = r.moves.length ? r.moves.map((m) => m.replace(">", " → ")).join(tl("listSep")) : tn("shell", "roles.noMoves");
  return `${r.title} · ${stage} · ${r.permission} · ${moves}`;
}
