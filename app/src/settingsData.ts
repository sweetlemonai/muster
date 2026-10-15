// Settings (0036): its groups in the order they show, this machine's settings as the core sends
// them (the claim lease and the LIVE window, saved in the app's settings.json, which `muster`
// reads too, whether Mac notifications are on, 0052, the menu bar icon, 0085, and the Claude accounts Muster reads, 0099), and the words for them. No React views here; Settings.tsx has those.
import { createContext } from "react";
import type { HelpTopic } from "./helpTopics";
import { tl, tn } from "./i18n";
import { LEASE_SECS } from "./lib";
import type { LocaleInfo } from "./types";

export type SettingsGroup = "appearance" | "display" | "projects" | "agents" | "notifications" | "about";

/** Settings' groups, in the order the list shows them, each with what it holds. Help isn't one:
 *  the manual is the header's Help tab, and only there (owner, 2026-10-13). */
export const SETTINGS_GROUPS: { readonly id: SettingsGroup; readonly label: string; readonly holds: string; readonly topic: HelpTopic }[] = (["appearance", "display", "projects", "agents", "notifications", "about"] as const).map((id) => ({
  id,
  topic: id,
  // In the language shown, each time they're read (shell.json, mu-146).
  get label() {
    return tn("shell", `settings.groups.${id}.label`);
  },
  get holds() {
    return tn("shell", `settings.groups.${id}.holds`);
  },
}));

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
    [Math.floor(m / 1440), "time.d"],
    [Math.floor((m % 1440) / 60), "time.h"],
    [m % 60, "time.min"],
  ] as const;
  const shown = parts.filter(([n]) => n > 0).map(([n, u]) => tl(u, { count: n }));
  return shown.length ? shown.reduce((a, b) => tl("time.join", { a, b })) : tl("time.min", { count: 0 });
}

/** A span as it's typed (30m, 2h, 1d), for the field: the largest whole unit. */
export function spanInput(minutes: number): string {
  if (minutes > 0 && minutes % 1440 === 0) return `${minutes / 1440}d`;
  if (minutes > 0 && minutes % 60 === 0) return `${minutes / 60}h`;
  return `${minutes}m`;
}

/** The words of Agents' Claude accounts row (0099), in the language shown each time they're read
 *  (shell.json `accounts`, mu-146). A label is an account's folder after `~/.claude-` (`mg` is
 *  `~/.claude-mg`), shown as its `cc-mg` command. */
export const ACCOUNTS_WORDS = {
  get group() {
    return tn("shell", "accounts.group");
  },
  get label() {
    return tn("shell", "accounts.label");
  },
  get none() {
    return tn("shell", "accounts.none");
  },
  get never() {
    return tn("shell", "accounts.never");
  },
  get add() {
    return tn("shell", "accounts.add");
  },
  chip: (l: string) => `cc-${l}`,
  remove: (l: string) => tn("shell", "accounts.remove", { chip: `cc-${l}` }),
  get empty() {
    return tn("shell", "accounts.empty");
  },
  get defaultAccount() {
    return tn("shell", "accounts.defaultAccount");
  },
  // As the core says it (settings.rs parse_accounts), the typed text cut at 24 characters.
  invalid: (typed: string) => tn("shell", "accounts.invalid", { typed: JSON.stringify([...typed].slice(0, 24).join("")) }),
  twice: (l: string) => tn("shell", "accounts.twice", { chip: `cc-${l}` }),
  // About's folders read. With none, why: an empty list, or `listed` labels none of which has its
  // ~/.claude-<label> folder (accounts.rs resolve leaves those out).
  transcriptsNone: (listed: number) => tn("shell", listed === 0 ? "accounts.transcriptsNoneListed" : "accounts.transcriptsNoneFolder"),
  // The helper's picker with no account, the same two ways; `listed` is null when the list
  // couldn't be read.
  helperNone: (listed: number | null) => tn("shell", listed === null ? "accounts.helperNoneUnread" : listed === 0 ? "accounts.helperNoneListed" : "accounts.helperNoneFolder"),
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
   *  accounts Muster reads (0099), `["mg", "hg"]` unless changed; never `.claude`. `lang`: the
   *  language picked in Settings (mu-146); none while it follows macOS. */
  settings: MachineSettings & { notify?: boolean; menuBar?: boolean; claudeAccounts: string[]; lang?: string };
  /** The language setting and the one in effect (mu-146); an older core sends none. */
  locale?: LocaleInfo;
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
