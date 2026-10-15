// Settings › Agents › Roles (0112): the words and fields for each role's profile, as roles.toml
// holds it. No React views here; Roles.tsx has those.
import { tl, tn } from "./i18n";
import { isStage, stageLabel } from "./lib";
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

/** The core's eleven roles, each with the title `roles.rs` gives it by default. Only compared,
 *  never shown: a role still titled so is said by shell.json `roles.title.<id>` (mu-195). Kept
 *  here, not read from the English file, because the window loads only the language shown. */
// i18n-ignore: roles.rs's default titles, compared with roles.toml's, never shown
export const BUILT_IN_TITLES = { shaper: "Shaper", architect: "Architect", designer: "Designer", coder: "Coder", tester: "QA tester", security: "Security reviewer", performance: "Performance reviewer", accessibility: "Accessibility reviewer", release: "Release and merge", lead: "Lead", researcher: "Researcher" } as const;

type BuiltIn = keyof typeof BUILT_IN_TITLES;

const isBuiltIn = (id: string): id is BuiltIn => Object.hasOwn(BUILT_IN_TITLES, id);

/** A role's title in the language shown: a built-in role still titled as by default reads in that
 *  language; a title the owner wrote in roles.toml, or an unknown role's, shows as written. */
export function roleTitle(r: Role): string {
  if (!isBuiltIn(r.id) || r.title !== BUILT_IN_TITLES[r.id]) return r.title;
  return tn("shell", `roles.title.${r.id}`);
}

/** "1 slot", "3 slots". */
export const slotWords = (n: number) => tn("shell", "roles.slots", { count: n });

/** The core's permission profiles (roles.rs `PERMISSIONS`), each said by shell.json
 *  `roles.permission.<id>` (mu-195). */
// i18n-ignore: roles.rs's permission ids, compared, never shown
export const PERMISSIONS = ["read-only", "shaping", "research", "worktree", "release", "lead"] as const;

type Permission = (typeof PERMISSIONS)[number];

const isPermission = (p: string): p is Permission => (PERMISSIONS as readonly string[]).includes(p);

/** A permission profile in the language shown; one this window doesn't know, as written. */
export const permissionWord = (p: string) => (isPermission(p) ? tn("shell", `roles.permission.${p}`) : p);

/** A stage in a role's profile, as the board names it; "any", and one this window doesn't know,
 *  as written. */
const stageWord = (stage: string) => (stage === "any" ? tn("shell", "roles.anyStage") : isStage(stage) ? stageLabel(stage) : stage);

/** A role's fixed parts in one line: its title, stage, permission and the stage moves it may make,
 *  all in the language shown (mu-195). */
export function roleLine(r: Role): string {
  const move = (m: string) => {
    const [from = "", to = ""] = m.split(">");
    return tn("shell", "roles.move", { from: stageWord(from), to: stageWord(to) });
  };
  const moves = r.moves.length ? r.moves.map(move).join(tl("listSep")) : tn("shell", "roles.noMoves");
  return `${roleTitle(r)} · ${stageWord(r.stage)} · ${permissionWord(r.permission)} · ${moves}`;
}
