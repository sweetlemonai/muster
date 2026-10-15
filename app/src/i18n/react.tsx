// The layer's React side: useT gives a component its namespace's `t` and
// re-renders it when the language changes; <Trans> puts elements (code,
// links, bold) inside a translated sentence.
import { cloneElement, Fragment, isValidElement, useMemo, useSyncExternalStore, type ReactElement, type ReactNode } from "react";
import type { AnyLocale } from "./locales";
import { getLocale, getTextVersion, interpolate, isLoaded, isolates, load, subscribe, template, translate, type Params } from "./runtime";
import type { Ns, TKey } from "./types";

export type TFn<N extends Ns> = (key: TKey<N>, params?: Params) => string;

/** The current language; re-renders on a switch. */
export function useLocale(): AnyLocale {
  return useSyncExternalStore(subscribe, getLocale, getLocale);
}

/**
 * `t` for one namespace (plus `common:` keys). A window has its first
 * screen's namespaces before it renders, and a lazy screen brings its own
 * with its code (`withText`), so they're in. One that isn't yet starts
 * loading, the part shows English meanwhile (always loaded first), and it
 * renders again when the file is in. No suspending: no Suspense boundary
 * needed, and no blank frame.
 */
export function useT<N extends Ns>(ns: N): TFn<N> {
  const text = useSyncExternalStore(subscribe, getTextVersion, getTextVersion);
  const locale = getLocale();
  // Idempotent: one fetch per file, whatever renders ask.
  if (!isLoaded(locale, ns)) void load(locale, ns).catch(() => {});
  // A new function whenever the language or its files change, so memoized
  // children see it.
  return useMemo(() => translator(ns, text), [ns, text]);
}

/** `t` for the language and files `text` names (getTextVersion). */
function translator<N extends Ns>(ns: N, text: string): TFn<N> {
  const locale = text.slice(0, text.indexOf("#")) as AnyLocale;
  return (key, params) => translate(locale, ns, key, params);
}

const TAG = /<(\w+)>([\s\S]*?)<\/\1>|<(\w+)\s*\/>/g;

/**
 * A sentence with elements inside, i18next's named-tag style:
 * `"Run <code>{{cmd}}</code> first"` with `components={{ code: <code /> }}`.
 * Each tag's text becomes the element's child; a `<br/>`-style tag renders
 * its element as is. Tags are found before placeholders are filled, so a
 * value (a branch name, a path) is always plain text.
 *
 * Right to left, a value outside the tags goes in a `<bdi>` (Drover's task 0020): an
 * element, not marks, so copying it copies the value as it is. Inside a tag
 * it's left as it is: `<code>` isolates itself (styles.css).
 */
export function Trans<N extends Ns>({ ns, k, params, components }: { ns: N; k: TKey<N>; params?: Params; components: Record<string, ReactElement> }): ReactNode {
  useT(ns);
  const locale = getLocale();
  const rtl = isolates(locale, ns);
  return renderTags(template(locale, ns, k, params), components, (s, inTag) => (rtl && !inTag ? isolated(locale, s, params) : interpolate(locale, s, params)));
}

/** A `{{name}}` placeholder, kept by `split`. */
const SLOT = /(\{\{\s*[\w.]+\s*\}\})/;

/** `text` filled in, each string value in its own `<bdi>`. */
function isolated(locale: AnyLocale, text: string, params?: Params): ReactNode[] {
  return text.split(SLOT).map((part, i) => {
    if (i % 2 === 0) return part;
    const v = params?.[part.slice(2, -2).trim()];
    return typeof v === "string" && v !== "" ? <bdi key={i}>{v}</bdi> : interpolate(locale, part, params);
  });
}

export function renderTags(text: string, components: Record<string, ReactElement>, fill: (s: string, inTag: boolean) => ReactNode = (s) => s): ReactNode {
  const out: ReactNode[] = [];
  let last = 0;
  let i = 0;
  for (const m of text.matchAll(TAG)) {
    if (m.index > last) out.push(fill(text.slice(last, m.index), false));
    const name = m[1] ?? m[3];
    const el = components[name];
    const inner = m[1] !== undefined ? fill(m[2], true) : undefined;
    if (el && isValidElement(el)) out.push(cloneElement(el, { key: i++ }, ...(inner === undefined ? [] : [inner])));
    else out.push(inner ?? "");
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push(fill(text.slice(last), false));
  return <Fragment>{out}</Fragment>;
}
