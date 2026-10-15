// Message keys, typed from the English files: a key that doesn't exist
// fails the type check. Type-only imports: no file is bundled by these.
import type board from "./locales/en/board.json";
import type cli from "./locales/en/cli.json";
import type common from "./locales/en/common.json";
import type core from "./locales/en/core.json";
import type help from "./locales/en/help.json";
import type native from "./locales/en/native.json";
import type pane from "./locales/en/pane.json";
import type shell from "./locales/en/shell.json";

/** One entry per file in ./locales/en/ (locales.test.ts checks they match).
 *  A namespace is the text of one area; each area task edits only its own. */
export interface Resources {
  /** Words every area uses; always loaded, and what `tl` reads. */
  common: typeof common;
  /** The window's frame: sidebar, toolbar, filter bar, Settings (mu-146; loaded at start). */
  shell: typeof shell;
  /** The board, its views, the backlog and sessions (mu-147). */
  board: typeof board;
  /** The task pane and everything opened from it: forms, comments, review, vision, and the Helper (mu-148). */
  pane: typeof pane;
  /** The Help pane's chrome, the manual's page titles and the generated Shortcuts page (mu-148; the manual's pages are mu-154's). */
  help: typeof help;
  /** The core's text: the menu bar, notifications (i18n.rs). */
  native: typeof native;
  /** The command line's text (mu-152). */
  cli: typeof cli;
  /** The core's other messages (mu-153). */
  core: typeof core;
}

export type Ns = keyof Resources;

type PluralSuffix = "zero" | "one" | "two" | "few" | "many" | "other";
/** `files_one`, `files_other` → `files`. */
type Base<K extends string> = K extends `${infer B}_${PluralSuffix}` ? B : K;

/** Every leaf's dotted path, plural forms folded into one key. */
type Leaves<T, P extends string = ""> = {
  [K in keyof T & string]: T[K] extends string ? `${P}${Base<K>}` : Leaves<T[K], `${P}${K}.`>;
}[keyof T & string];

export type Key<N extends Ns> = Leaves<Resources[N]>;

/** What a namespace's `t` takes: its own keys, and the shared ones as
 *  `common:key`. */
export type TKey<N extends Ns> = Key<N> | `common:${Key<"common">}`;
