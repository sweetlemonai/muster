// Settings (0036): its groups in the order they show, this machine's settings as the core sends
// them (the claim lease and the LIVE window, saved in the app's settings.json, which `muster`
// reads too, and whether Mac notifications are on, 0052), and the words for them. No React views here; Settings.tsx has those.
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
  { id: "agents", label: "Agents", holds: "The helper, the claim lease, LIVE", topic: "agents" },
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

/** The core's answer for Settings: the settings with their bounds, and what About shows. */
export type SettingsInfo = {
  /** `notify`: Mac notifications on (0052); a core from before them sends none, which reads as on. */
  settings: MachineSettings & { notify?: boolean };
  leaseBounds: [number, number];
  liveBounds: [number, number];
  version: string;
  /** The home folder: paths under it show as `~/…` (`homePath`). */
  home: string;
  configDir: string;
  base: string;
  transcripts: string[];
  /** Whether Claude Code's session states are read (0048): "on (2 accounts)", or "not
   *  available: <why>" (sessionState.ts `sessionStatesLine`). */
  sessionStates?: string;
};
