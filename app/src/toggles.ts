// The shared toggle (mu-167): the sidebar, Recently finished and the filter bar each shown or
// hidden. Each mode's table row (modes.ts) gives the default; the owner's pick per mode is kept on
// this Mac only where it differs, as the board's columns are (mu-84), so switching modes resets
// nothing. The View menu (app_menu.rs) sends a `view-toggle` event with the region, the one path
// for its key (⌃⌘S), and is told back what's shown so its words stay true (`viewMenu`).
// The sidebar is wired here; Recently finished and the filters are the follow-up's (mu-169).
import { tn } from "./i18n";
import { look, MODES, type DisplayMode } from "./modes";

export const REGIONS = ["sidebar", "recent", "filters"] as const;
export type Region = (typeof REGIONS)[number];

/** The regions the window toggles today; the menu's other items are disabled until theirs is. */
const WIRED: ReadonlySet<Region> = new Set(["sidebar"]);

export type TogglePick = "shown" | "hidden";
export type TogglePicks = Partial<Record<DisplayMode, Partial<Record<Region, TogglePick>>>>;
export const TOGGLES_KEY = "muster.toggles";

/** Whether a region shows in a mode when the owner hasn't picked. */
export function regionDefault(region: Region, mode: DisplayMode): boolean {
  if (region === "sidebar") return look("sidebar-shown", mode) === "shown";
  if (region === "recent") return look("rail", mode) === "open";
  return look("filters", mode) === "open";
}

export function isShown(picks: TogglePicks, mode: DisplayMode, region: Region): boolean {
  const pick = picks[mode]?.[region];
  return pick ? pick === "shown" : regionDefault(region, mode);
}

/** The picks after the owner shows (`show` true) or hides a region in a mode, kept only where that
 *  differs from the mode's default. */
export function pickToggle(picks: TogglePicks, mode: DisplayMode, region: Region, show: boolean): TogglePicks {
  const mine = { ...picks[mode] };
  if (show === regionDefault(region, mode)) delete mine[region];
  else mine[region] = show ? "shown" : "hidden";
  const out = { ...picks };
  if (Object.keys(mine).length) out[mode] = mine;
  else delete out[mode];
  return out;
}

export function toggleRegion(picks: TogglePicks, mode: DisplayMode, region: Region): TogglePicks {
  return pickToggle(picks, mode, region, !isShown(picks, mode, region));
}

/** A `view-toggle` event's payload as a region, or null for anything else. */
export function regionOf(payload: unknown): Region | null {
  return REGIONS.find((r) => r === payload) ?? null;
}

/** The picks after a `view-toggle` event: its region toggled once when the window wires it, the
 *  same picks otherwise. */
export function toggled(picks: TogglePicks, mode: DisplayMode, payload: unknown): TogglePicks {
  const region = regionOf(payload);
  return region && WIRED.has(region) ? toggleRegion(picks, mode, region) : picks;
}

/** One toggle as App runs it: the new picks, what the live region says, and where the focus must
 *  land after the render: the sidebar's state it was for when the focus was inside what goes
 *  (`focusWasInside`, asked before), else null. Null for an event that changes nothing. */
export function flip(picks: TogglePicks, mode: DisplayMode, payload: unknown, focusWasInside: boolean): { picks: TogglePicks; said: string; refocus: boolean | null } | null {
  const next = toggled(picks, mode, payload);
  if (next === picks) return null;
  const shown = isShown(next, mode, "sidebar");
  return { picks: next, said: toggleWords("sidebar", shown), refocus: focusWasInside ? shown : null };
}

/** What the View menu is told: each region shown now (`set_view_menu`, three booleans). */
export function viewMenu(picks: TogglePicks, mode: DisplayMode): Record<Region, boolean> {
  return { sidebar: isShown(picks, mode, "sidebar"), recent: isShown(picks, mode, "recent"), filters: isShown(picks, mode, "filters") };
}

/** The kept picks: none for a missing or broken value, only known modes, regions and picks, and
 *  none that says what the mode does anyway. */
export function loadToggles(store: Pick<Storage, "getItem">): TogglePicks {
  let raw: unknown;
  try {
    raw = JSON.parse(store.getItem(TOGGLES_KEY) ?? "{}");
  } catch {
    return {};
  }
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return {};
  let out: TogglePicks = {};
  for (const { id } of MODES) {
    const m: unknown = (raw as Record<string, unknown>)[id];
    if (!m || typeof m !== "object" || Array.isArray(m)) continue;
    for (const [k, v] of Object.entries(m)) {
      const region = regionOf(k);
      if (region && (v === "shown" || v === "hidden")) out = pickToggle(out, id, region, v === "shown");
    }
  }
  return out;
}

export function storeToggles(store: Pick<Storage, "setItem">, picks: TogglePicks): void {
  try {
    store.setItem(TOGGLES_KEY, JSON.stringify(picks));
  } catch {
    // Storage full or off: the pick holds for this window's life.
  }
}

/** What the live region says after a toggle: "Sidebar hidden", "Sidebar shown". */
export function toggleWords(region: Region, shown: boolean): string {
  const [hidden, showing] = WORDS[region];
  return tn("shell", shown ? showing : hidden);
}
const WORDS = {
  sidebar: ["toggles.sidebar.hidden", "toggles.sidebar.shown"],
  recent: ["toggles.recent.hidden", "toggles.recent.shown"],
  filters: ["toggles.filters.hidden", "toggles.filters.shown"],
} as const satisfies Record<Region, readonly [string, string]>;

/** Whether the keyboard focus (`active`, document.activeElement) is inside `region`: asked before
 *  a toggle, while the part that goes is still there. */
export function focusInside(active: unknown, region: { contains: (n: Node | null) => boolean } | null): boolean {
  return Boolean(region && active && region.contains(active as Node));
}

/** After the toggle has rendered: the focus to `target` (the rail's Show, or the sidebar's Hide)
 *  when it was inside what went, so it's never left on the page's body. */
export function moveFocus(wasInside: boolean, target: { focus: () => void } | null): boolean {
  if (!wasInside || !target) return false;
  target.focus();
  return true;
}
