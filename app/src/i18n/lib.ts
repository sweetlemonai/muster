// Text from helpers outside components: `tl("key")` reads the shared
// "common" namespace in the current language (Drover has a "lib" namespace
// for this; Muster's namespaces are by area, so a helper of one area, like
// filters.ts for the shell, uses `tn` with its own). The window loads
// "common" before it renders. Call these while rendering a component that
// uses useT, so the text follows a language switch; never keep the result
// in a module constant.
import { cleanValue, FSI, getLocale, isolates, PDI, translate, type Params } from "./runtime";
import type { Ns, TKey } from "./types";

export function tl(key: TKey<"common">, params?: Params): string {
  return translate(getLocale(), "common", key, params);
}

/**
 * Any namespace's text in the language shown now, outside rendering: for
 * a click handler's message. The namespace must already be loaded in the
 * window (`common`, or the window's own, `shell`); a lazy screen's own
 * text belongs in its `useT`.
 */
export function tn<N extends Ns>(ns: N, key: TKey<N>, params?: Params): string {
  return translate(getLocale(), ns, key, params);
}

/** A value (a path, a name, an email) put into text outside a translation,
 *  isolated in a right-to-left language as a filled-in placeholder is
 *  (`runtime.isolates`; Drover's task 0020), so `~/.ssh` keeps its tilde in place.
 *  It goes in clean, in every language, as a placeholder's value does (`runtime.cleanValue`). */
export function isolate(value: string): string {
  const clean = cleanValue(value);
  return clean && isolates(getLocale(), "common") ? `${FSI}${clean}${PDI}` : clean;
}

const ISOLATES = /[\u2066-\u2069]/g;

/** A translation that becomes data, not text on screen (a file name, a
 *  name that's kept), without the isolation marks a right-to-left language
 *  puts in (Drover's task 0020). */
export function unmarked(text: string): string {
  return text.replace(ISOLATES, "");
}

/** Whole sentences one after another, through `common:sentences`: most
 *  languages put a space between them, Japanese and Chinese don't. Empty
 *  ones are left out. */
export function sentences(parts: readonly string[]): string {
  const [first = "", ...rest] = parts.filter(Boolean);
  return rest.reduce((a, b) => tn("common", "sentences", { a, b }), first);
}
