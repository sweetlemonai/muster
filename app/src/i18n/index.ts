// Muster's translation layer, ported from Drover's (Drover task 0007,
// decision 0021, commit a990fc8; mu-146 here). Components use
// `const t = useT("ns")` and `t("key", { count, name })`; dates and numbers
// go through `fmt`. The text lives in ./locales/<language>/<ns>.json.
export { useLocale, useT, Trans, type TFn } from "./react";
export * as fmt from "./format";
export { followLocale, startLocale, LOCALE_CHANGED } from "./boot";
export { getLocale } from "./runtime";
export { isolate, sentences, tl, tn, unmarked } from "./lib";
export { withText } from "./prefetch";
export { dirOf, isLocale, LOCALES, NATIVE_NAMES, PSEUDO, READY, type AnyLocale, type Locale } from "./locales";
export type { Key, Ns, TKey } from "./types";
