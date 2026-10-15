// The languages Muster speaks (task mu-146, ported from Drover's task 0007,
// decision 0021). Adding one is a folder of files under ./locales/<code>/ and
// an entry here (and in the core's i18n.rs, which reads the same files for
// the menu bar).

export const LOCALES = ["en", "fa"] as const;
export type Locale = (typeof LOCALES)[number];

/** For development: English with every letter accented and padded, so
 *  text that skipped translation, or got cut off, shows at a glance. */
export const PSEUDO = "pseudo";
export type AnyLocale = Locale | typeof PSEUDO;

/** Each language by its own name, as Settings lists it: never translated,
 *  so a person who can't read the language shown can still find theirs. */
export const NATIVE_NAMES: Record<Locale, string> = {
  en: "English",
  fa: "فارسی",
};

/** Written right to left: the whole layout mirrors. */
const RTL: ReadonlySet<string> = new Set(["fa"]);

/**
 * The languages offered in Settings: those whose files are complete
 * (locales.test.ts holds every one of them to every English key).
 */
export const READY: readonly Locale[] = ["en", "fa"];

export const isLocale = (x: unknown): x is Locale => typeof x === "string" && (LOCALES as readonly string[]).includes(x);
export const isAnyLocale = (x: unknown): x is AnyLocale => isLocale(x) || (import.meta.env.DEV && x === PSEUDO);

export const dirOf = (l: AnyLocale): "rtl" | "ltr" => (RTL.has(l) ? "rtl" : "ltr");

/** The BCP 47 tag `Intl` gets (the pseudo-locale formats as English). */
export const intlTag = (l: AnyLocale): string => (l === PSEUDO ? "en" : l);
