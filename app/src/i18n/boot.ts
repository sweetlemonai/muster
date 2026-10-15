// The window's language at start, and after. The core decides which
// language is in effect (the setting, or macOS's); a copy kept in
// localStorage lets the window load that language's text before its first
// render, so nothing shows in English first.
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { LocaleInfo } from "../types";
import { isAnyLocale, type AnyLocale } from "./locales";
import { applyToDocument, getLocale, loadAll, setLocale } from "./runtime";
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

/**
 * Load the last language's text for `namespaces` (plus the shared ones),
 * then follow the core: its language now, and every change after. Resolves
 * once the text is in, so the window can render. Called once, before the
 * first render (main.tsx).
 */
export async function startLocale(namespaces: readonly Ns[]): Promise<void> {
  const saved = readMirror();
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
