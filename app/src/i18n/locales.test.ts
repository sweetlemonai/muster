// The translation files hold together: every language has the same keys
// as English (the finished ones all of them, the others no stray ones),
// the same placeholders and tags, and every plural form its language needs
// (Drover's test, mu-146: here English and Persian).
import { describe, expect, it } from "vitest";
import { LOCALES, READY, intlTag, type Locale } from "./locales";
import type { Messages } from "./runtime";
import typesSource from "./types.ts?raw";

const all = import.meta.glob<Messages>("./locales/*/*.json", { import: "default", eager: true });

const byLocale = new Map<string, Map<string, Messages>>();
for (const [path, m] of Object.entries(all)) {
  const [, locale, ns] = /^\.\/locales\/([^/]+)\/([^/]+)\.json$/.exec(path) ?? [];
  if (!byLocale.has(locale)) byLocale.set(locale, new Map());
  byLocale.get(locale)?.set(ns, m);
}
const en = byLocale.get("en") ?? new Map<string, Messages>();

const SUFFIX = /_(zero|one|two|few|many|other)$/;

/** Every leaf as "a.b.c" → text. */
function leaves(m: Messages, prefix = ""): Map<string, string> {
  const out = new Map<string, string>();
  for (const [k, v] of Object.entries(m)) {
    if (typeof v === "string") out.set(prefix + k, v);
    else for (const [kk, vv] of leaves(v, `${prefix}${k}.`)) out.set(kk, vv);
  }
  return out;
}

/** Keys with plural forms folded: base → the forms present. */
function plurals(keys: Iterable<string>): Map<string, Set<string>> {
  const out = new Map<string, Set<string>>();
  for (const k of keys) {
    const m = SUFFIX.exec(k);
    if (!m) continue;
    const base = k.slice(0, -m[0].length);
    if (!out.has(base)) out.set(base, new Set());
    out.get(base)?.add(m[1]);
  }
  return out;
}

const placeholders = (s: string) => [...s.matchAll(/\{\{\s*([\w.]+)\s*\}\}/g)].map((m) => m[1]).sort();
const tags = (s: string) => [...s.matchAll(/<\/?(\w+)\s*\/?>/g)].map((m) => m[0].replace(/\s/g, "")).sort();

/** The text a plural key's forms must carry the same placeholders as:
 *  English's `_other` (its `_one` may say "a" instead of {{count}}). */
function englishFor(ns: string, key: string): string | undefined {
  const e = leaves(en.get(ns) ?? {});
  const m = SUFFIX.exec(key);
  return m ? (e.get(key.replace(SUFFIX, "_other")) ?? e.get(key)) : e.get(key);
}

describe("translation files", () => {
  it("has a folder per language, and nothing else", () => {
    expect([...byLocale.keys()].sort()).toEqual([...LOCALES].sort());
  });

  it("gives every language the core's file (i18n.rs compiles them all in)", () => {
    for (const l of LOCALES) expect(byLocale.get(l)?.has("native"), l).toBe(true);
  });

  it("has a file per area, each area task editing only its own (mu-146)", () => {
    expect([...en.keys()].sort()).toEqual(["board", "cli", "common", "core", "help", "native", "pane", "shell"]);
    // No text for agents: what they read stays English, outside this layer.
    for (const files of byLocale.values()) expect(files.has("agent")).toBe(false);
  });

  it("types every English file (types.ts's Resources)", () => {
    const typed = [...typesSource.matchAll(/^\s+(\w+): typeof \w+;$/gm)].map((m) => m[1]).sort();
    expect(typed).toEqual([...en.keys()].sort());
  });

  it("keeps English's plural keys whole: one and other, with {{count}} in other", () => {
    for (const [ns, m] of en) {
      const keys = leaves(m);
      for (const [base, forms] of plurals(keys.keys())) {
        expect([...forms].sort(), `${ns}:${base}`).toEqual(["one", "other"]);
        expect(keys.get(`${base}_other`), `${ns}:${base}_other`).toContain("{{count}}");
      }
    }
  });

  for (const locale of LOCALES.filter((l) => l !== "en")) {
    describe(locale, () => {
      const files = byLocale.get(locale) ?? new Map<string, Messages>();
      // VITE_I18N_COMPLETE=fa,ar holds languages being finished to every key too.
      const ready = READY.includes(locale as Locale) || (import.meta.env.VITE_I18N_COMPLETE ?? "").split(",").includes(locale);
      const needed = new Intl.PluralRules(intlTag(locale)).resolvedOptions().pluralCategories as string[];

      it("has no file or key English doesn't", () => {
        for (const [ns, m] of files) {
          expect(en.has(ns), `${locale}/${ns}.json`).toBe(true);
          const enBases = new Set([...leaves(en.get(ns) ?? {}).keys()].map((k) => k.replace(SUFFIX, "")));
          for (const k of leaves(m).keys()) expect(enBases.has(k.replace(SUFFIX, "")), `${locale} ${ns}:${k}`).toBe(true);
        }
      });

      it("keeps every string's placeholders and tags", () => {
        for (const [ns, m] of files) {
          for (const [k, text] of leaves(m)) {
            const source = englishFor(ns, k);
            if (source === undefined) continue;
            const have = new Set(placeholders(text));
            const want = new Set(placeholders(source));
            // A "zero", "one" or "two" form may spell its number out.
            const spelled = /^(zero|one|two)$/.test(SUFFIX.exec(k)?.[1] ?? "");
            expect([...have].filter((p) => !want.has(p)), `${locale} ${ns}:${k} adds a placeholder`).toEqual([]);
            expect([...want].filter((p) => !have.has(p) && !(spelled && p === "count")), `${locale} ${ns}:${k} drops a placeholder`).toEqual([]);
            expect(tags(text), `${locale} ${ns}:${k}`).toEqual(tags(source));
          }
        }
      });

      it("has every plural form the language needs", () => {
        for (const [ns, m] of files) {
          const keys = [...leaves(m).keys()];
          const enPlurals = plurals(leaves(en.get(ns) ?? {}).keys());
          for (const base of enPlurals.keys()) {
            const forms = keys.filter((k) => k.replace(SUFFIX, "") === base && SUFFIX.test(k)).map((k) => SUFFIX.exec(k)?.[1]);
            if (forms.length === 0 && !ready) continue;
            expect(forms.sort(), `${locale} ${ns}:${base}`).toEqual(expect.arrayContaining(needed));
          }
        }
      });

      if (ready) {
        it("has every English key (it's offered in Settings)", () => {
          for (const [ns, m] of en) {
            const mine = new Set([...leaves(files.get(ns) ?? {}).keys()].map((k) => k.replace(SUFFIX, "")));
            for (const k of leaves(m).keys()) expect(mine.has(k.replace(SUFFIX, "")), `${locale} ${ns}:${k}`).toBe(true);
          }
        });
      }
    });
  }
});
