// Settings (0036): its groups in the order they show, this machine's settings as the core sends
// them (the claim lease and the LIVE window, saved in the app's settings.json, which `muster`
// reads too, whether Mac notifications are on, 0052, the menu bar icon, 0085, and the Claude accounts Muster reads, 0099), and the words for them. No React views here; Settings.tsx has those.
import { createContext } from "react";
import type { HelpTopic } from "./helpTopics";
import { LEASE_SECS } from "./lib";

export type SettingsGroup = "appearance" | "display" | "projects" | "agents" | "notifications" | "about";

/** Settings' groups, in the order the list shows them, each with what it holds. Help isn't one:
 *  the manual is the header's Help tab, and only there (owner, 2026-10-13). */
export const SETTINGS_GROUPS: { id: SettingsGroup; label: string; holds: string; topic: HelpTopic }[] = [
  { id: "appearance", label: "Appearance", holds: "Theme, accent, text size, cards", topic: "appearance" },
  { id: "display", label: "Display", holds: "ADHD, Standard or Focus: how much shows", topic: "display" },
  { id: "projects", label: "Projects", holds: "The queues Muster shows", topic: "projects" },
  { id: "agents", label: "Agents", holds: "The helper, the claim lease, LIVE, roles", topic: "agents" },
  { id: "notifications", label: "Notifications", holds: "A Mac notification when something new waits for you", topic: "notifications" },
  { id: "about", label: "About", holds: "Version, settings, what it reads", topic: "about" },
];

/** This machine's settings, in minutes, as `state` and `settings_info` carry them. */
export type MachineSettings = { leaseMinutes: number; liveMinutes: number };
/** The defaults: the core's lease (ops::LEASE) and LIVE window (transcripts::LIVE_SECS, paneData's LIVE_SECS). */
export const DEFAULT_MACHINE: MachineSettings = { leaseMinutes: LEASE_SECS / 60, liveMinutes: 5 };

/** The settings the board judges by: the window provides the ones from the last poll; the
 *  defaults before the first. */
export const Machine = createContext<MachineSettings>(DEFAULT_MACHINE);

/** Each role's name by its id (0112), from the last poll: what a slot in a stamp (`slot coder-2`)
 *  shows as on cards and in the pane. Empty before the first poll and when roles.toml is refused. */
export const RoleNamesContext = createContext<Readonly<Record<string, string>>>({});

/** The state's settings, or the defaults when the core sent none (an older core). */
export function machineOf(s: { settings?: Partial<MachineSettings> } | null | undefined): MachineSettings {
  const ok = (n: unknown): n is number => typeof n === "number" && Number.isInteger(n) && n > 0;
  const lease = s?.settings?.leaseMinutes;
  const live = s?.settings?.liveMinutes;
  return {
    leaseMinutes: ok(lease) ? lease : DEFAULT_MACHINE.leaseMinutes,
    liveMinutes: ok(live) ? live : DEFAULT_MACHINE.liveMinutes,
  };
}

/** "12 h", "1 h 30 min", "1 d 6 h", "5 min": a span of minutes, as the core's span_text says it. */
export function spanText(minutes: number): string {
  const m = Math.max(0, Math.round(minutes));
  const parts = [
    [Math.floor(m / 1440), "d"],
    [Math.floor((m % 1440) / 60), "h"],
    [m % 60, "min"],
  ] as const;
  const shown = parts.filter(([n]) => n > 0).map(([n, u]) => `${n} ${u}`);
  return shown.length ? shown.join(" ") : "0 min";
}

/** A span as it's typed (30m, 2h, 1d), for the field: the largest whole unit. */
export function spanInput(minutes: number): string {
  if (minutes > 0 && minutes % 1440 === 0) return `${minutes / 1440}d`;
  if (minutes > 0 && minutes % 60 === 0) return `${minutes / 60}h`;
  return `${minutes}m`;
}

/** The words of Agents' Claude accounts row (0099). A label is an account's folder after
 *  `~/.claude-` (`mg` is `~/.claude-mg`), shown as its `cc-mg` command. */
export const ACCOUNTS_WORDS = {
  group: "Claude accounts Muster reads",
  label: "Claude accounts Muster reads:",
  none: "none: Muster reads no Claude sessions",
  never: "Never ~/.claude, the default account.",
  add: "Add an account",
  chip: (l: string) => `cc-${l}`,
  remove: (l: string) => `Remove cc-${l} from the accounts Muster reads`,
  empty: "Type an account's label, like mg for cc-mg.",
  defaultAccount: "Never ~/.claude: it's the default account, which Muster doesn't read.",
  // As the core says it (settings.rs parse_accounts), the typed text cut at 24 characters.
  invalid: (typed: string) => `${JSON.stringify([...typed].slice(0, 24).join(""))} is not a valid account label: 1 to 12 lowercase letters and digits, like mg`,
  twice: (l: string) => `cc-${l} is already read.`,
  // About's folders read. With none, why: an empty list, or `listed` labels none of which has its
  // ~/.claude-<label> folder (accounts.rs resolve leaves those out).
  transcripts: "Session transcripts:",
  transcriptsNone: (listed: number) =>
    listed === 0 ? "Session transcripts: none, no Claude account is on the list in Agents" : "Session transcripts: none, no account on the list in Agents has a ~/.claude-<label> folder",
  transcriptsUnder: "In them, only projects under",
  // The helper's picker with no account, the same two ways; `listed` is null when the list
  // couldn't be read.
  helperNone: (listed: number | null) =>
    listed === null
      ? "No Claude account Muster may use was found. Check its list in Settings, Agents."
      : listed === 0
        ? "No Claude account is on Muster's list of accounts it may use. Add one in Settings, Agents."
        : "No account on Muster's list of accounts it may use has a ~/.claude-<label> folder. Check the list in Settings, Agents.",
};

/** A label as the core takes it (accounts.rs is_label): 1 to 12 lowercase letters and digits. */
const ACCOUNT_LABEL = /^[a-z0-9]{1,12}$/;

/** A change to the accounts list: add what was typed, or remove a label. The value to save
 *  (`set_setting("claude-accounts", value)`, the labels by spaces, empty for none), or why not. */
export function accountsEdit(labels: string[], edit: { add: string } | { remove: string }): { value: string } | { error: string } {
  if ("remove" in edit) return { value: labels.filter((l) => l !== edit.remove).join(" ") };
  const trimmed = edit.add.trim();
  const label = trimmed.startsWith("cc-") ? trimmed.slice(3).trim() : trimmed;
  if (label === "") return { error: ACCOUNTS_WORDS.empty };
  if (/^(?:~\/)?\.claude\/?$/.test(label)) return { error: ACCOUNTS_WORDS.defaultAccount };
  if (!ACCOUNT_LABEL.test(label)) return { error: ACCOUNTS_WORDS.invalid(label) };
  if (labels.includes(label)) return { error: ACCOUNTS_WORDS.twice(label) };
  return { value: [...labels, label].join(" ") };
}

/** The core's answer for Settings: the settings with their bounds, and what About shows. */
export type SettingsInfo = {
  /** `notify`: Mac notifications on (0052); `menuBar`: the menu bar icon on (0085). A core from
   *  before them sends none, which reads as on. `claudeAccounts`: the labels of the Claude
   *  accounts Muster reads (0099), `["mg", "hg"]` unless changed; never `.claude`. */
  settings: MachineSettings & { notify?: boolean; menuBar?: boolean; claudeAccounts: string[] };
  leaseBounds: [number, number];
  liveBounds: [number, number];
  version: string;
  /** The home folder: paths under it show as `~/…` (`homePath`). */
  home: string;
  configDir: string;
  base: string;
  /** The Claude account folders the session transcripts are read from: the allowed ones (0099),
   *  never `~/.claude`. */
  transcripts: string[];
  /** In those, only the projects under this folder are read (0099): `~/playground`. */
  transcriptsUnder: string;
  /** Whether Claude Code's session states are read (0048): "on (2 accounts)", or "not
   *  available: <why>" (sessionState.ts `sessionStatesLine`). */
  sessionStates?: string;
};
