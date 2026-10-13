// Text size for the whole window, Help included (owner, 2026-10-13: "option to zoom in the help
// page and all pages"). CSS zoom on the page root: no window permission needed. The size is one of
// a few steps, remembered, and changed with ⌘+ ⌘− ⌘0 (the shortcut table) or in Appearance.
import { pressed } from "./shortcuts";

export const ZOOMS = [0.9, 1, 1.15, 1.3, 1.5] as const;
export const ZOOM_KEY = "muster.zoom";
export const ZOOM_EVENT = "muster:zoom";

/** A saved size read back: the nearest step, 100% for anything else. */
export function parseZoom(raw: string | null): number {
  const n = Number(raw);
  if (raw === null || raw === "" || !Number.isFinite(n) || n <= 0) return 1;
  return ZOOMS.reduce((best, z) => (Math.abs(z - n) < Math.abs(best - n) ? z : best), 1 as number);
}

/** The next size up (1), down (-1) or back to 100% (0), staying within the steps. */
export function stepZoom(z: number, dir: 1 | -1 | 0): number {
  if (dir === 0) return 1;
  const i = ZOOMS.indexOf(parseZoom(String(z)) as (typeof ZOOMS)[number]);
  return ZOOMS[Math.min(ZOOMS.length - 1, Math.max(0, i + dir))];
}

/** Which way a key event changes the size, or null when it isn't a zoom key. */
export function zoomDirection(e: { key: string; metaKey: boolean; ctrlKey: boolean }): 1 | -1 | 0 | null {
  if (pressed(e, "zoom-in")) return 1;
  if (pressed(e, "zoom-out")) return -1;
  if (pressed(e, "zoom-reset")) return 0;
  return null;
}

export function loadZoom(read: () => string | null = () => localStorage.getItem(ZOOM_KEY)): number {
  try {
    return parseZoom(read());
  } catch {
    return 1; // no storage (off, or a test run)
  }
}

/** Sets the size on the page, remembers it, and tells the Appearance pane. */
export function setZoom(z: number, root: { style: { setProperty: (k: string, v: string) => void } } = document.documentElement): number {
  const size = parseZoom(String(z));
  root.style.setProperty("zoom", String(size));
  try {
    localStorage.setItem(ZOOM_KEY, String(size));
  } catch {
    /* storage off: the size holds for now */
  }
  if (typeof window !== "undefined") window.dispatchEvent(new CustomEvent(ZOOM_EVENT, { detail: size }));
  return size;
}

/** The window's zoom keys: ⌘+ ⌘− ⌘0. Installed once at start. */
export function installZoomKeys(): void {
  setZoom(loadZoom());
  window.addEventListener("keydown", (e) => {
    const dir = zoomDirection(e);
    if (dir === null) return;
    e.preventDefault();
    setZoom(stepZoom(loadZoom(), dir));
  });
}
