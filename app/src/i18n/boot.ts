// The window's language at start, and after. The core decides which
// language is in effect (the setting, or macOS's); a copy kept in
// localStorage lets the window load that language's text before its first
// render, so nothing shows in English first.
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { LocaleInfo } from "../types";
import { isAnyLocale, type AnyLocale } from "./locales";
import { readNumberingMirror, writeNumberingMirror } from "./numbering";
import { applyToDocument, getLocale, loadAll, setLocale, setNumbering } from "./runtime";
import type { Ns } from "./types";

const MIRROR = "muster.locale";
/** The core's event to the window when the language changes. */
export const LOCALE_CHANGED = "locale-changed";

function readMirror(): AnyLocale {
  try {
    const v = localStorage.getItem(MIRROR);
    return isAnyLocale(v) ? v : "en";
  } catch {
    return "en";
  }
}

function writeMirror(l: AnyLocale) {
  try {
    localStorage.setItem(MIRROR, l);
  } catch {
    // Without storage the next start shows English until the core answers.
  }
}

/** Switch to what the core says is in effect (ignored unless it's one we know). */
async function take(effective: unknown) {
  if (!isAnyLocale(effective)) return;
  try {
    await setLocale(effective);
    writeMirror(getLocale());
  } catch {
    // Its files didn't load: the window stays in the language it shows.
  }
}

/** Settings' Language: the window switches to what the core answered with,
 *  without waiting for its event. Both arrive; the second does nothing. */
export const followLocale = take;

/** When Settings last saved Digits or Calendar (a `performance.now()` time), so a poll asked for
 *  before that can't put the old values back (its settings were read before the save). */
let savedAt = -Infinity;

/** The digits and calendar the core's settings say (mu-151; null or missing follows the
 *  language): from each state poll (`askedAt`, when the poll was asked for) and from Settings'
 *  save at once (no `askedAt`). A poll asked for before the last save is passed over. Kept in
 *  localStorage for the next start's first render. */
export function followNumbering(settings: { digits?: unknown; calendar?: unknown } | null | undefined, askedAt?: number) {
  if (!settings) return;
  if (askedAt === undefined) savedAt = performance.now();
  else if (askedAt < savedAt) return;
  if (setNumbering({ digits: settings.digits, calendar: settings.calendar })) writeNumberingMirror();
}

/**
 * Load the last language's text for `namespaces` (plus the shared ones),
 * then follow the core: its language now, and every change after. Resolves
 * once the text is in, so the window can render. Called once, before the
 * first render (main.tsx).
 */
export async function startLocale(namespaces: readonly Ns[]): Promise<void> {
  const saved = readMirror();
  readNumberingMirror();
  try {
    await loadAll(saved, ["common", ...namespaces]);
    await setLocale(saved);
  } catch {
    // A file that won't load: English, which ships with the app.
    await loadAll("en", ["common", ...namespaces]);
  }
  applyToDocument(getLocale());
  void listen<{ effective?: unknown }>(LOCALE_CHANGED, (e) => {
    void take(e.payload?.effective);
  }).catch(() => {});
  // invoke, not api.ts: the layer stays without the whole API, as in Drover.
  void invoke<LocaleInfo>("locale_info")
    .then((info) => take(info.effective))
    .catch(() => {});
}
