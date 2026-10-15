// Project labels and categories (task 0044): what the window calls a project ("Muster", "Drover —
// agents") and the sidebar heading it goes under ("Apps", "Tools"). The core keeps them in
// projects.json by queue folder (labels.rs) and sends each queue's with the state
// (`QueueInfo.label` and `category`, already falling back to the name and to the group or Other).
// The queue's name stays what commands, folders and the window's own bookkeeping use.
import { createContext, useContext } from "react";
import { fmt } from "./i18n";
import type { QueueInfo } from "./types";

/** The longest label and category, in characters, as the core's labels::MAX_LABEL and MAX_CATEGORY. */
export const LABEL_MAX = 40;
export const CATEGORY_MAX = 24;
/** The heading of the projects with no category, last in the sidebar. */
export const OTHER = "Other";

/** Each queue's name to what the window calls it; a queue without a label (an older core) isn't in it. */
export function labelsOf(queues: Pick<QueueInfo, "name" | "label">[]): Map<string, string> {
  const out = new Map<string, string>();
  for (const q of queues) if (q.label && q.label !== q.name) out.set(q.name, q.label);
  return out;
}

/** What the window calls the project named `name`: its label, else the name. */
export const labelOf = (labels: ReadonlyMap<string, string>, name: string): string => labels.get(name) ?? name;

/** The labels of the projects the window shows, by name (`labelsOf`). Without a provider, every
 *  project is called by its name. */
export const Labels = createContext<ReadonlyMap<string, string>>(new Map());

/** What the window calls the project named `name`. */
export function useLabel(name: string): string {
  return labelOf(useContext(Labels), name);
}

/** A task key's tooltip: the project it belongs to, by its label ("Muster · mu-12"), since the
 *  prefix isn't the name. */
export const keyTitle = (label: string, key: string) => `${label} · ${key}`;

// Code-unit order after folding case, as the sidebar's A–Z order, so it's the same on every machine.
const cmp = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);
// Other last, the rest in the language's order (Persian order in فارسی, mu-151), then by code point.
const byHeading = (a: string, b: string) => Number(a === OTHER) - Number(b === OTHER) || fmt.compare(a, b) || cmp(a.toLowerCase(), b.toLowerCase()) || cmp(a, b);

/** One heading in the sidebar: the category and its rows. */
export type CategoryGroup<T> = { name: string; rows: T[] };

/** `rows` under their category headings: A–Z, Other last, and an empty category is Other. Each
 *  heading keeps its rows in the order given (the order chips'). */
export function byCategory<T extends { category: string }>(rows: T[]): CategoryGroup<T>[] {
  const groups = new Map<string, T[]>();
  for (const r of rows) {
    const name = r.category.trim() || OTHER;
    const list = groups.get(name);
    if (list) list.push(r);
    else groups.set(name, [r]);
  }
  return [...groups.keys()].sort(byHeading).map((name) => ({ name, rows: groups.get(name) ?? [] }));
}

/** Every shown project's category, A–Z, Other last. */
export function categoriesOf(queues: Pick<QueueInfo, "category">[]): string[] {
  return [...new Set(queues.map((q) => q.category?.trim() || OTHER))].sort(byHeading);
}

/** The categories a project's field offers in Settings › Projects: the other shown projects', so
 *  what's being typed (saved as it's typed) never becomes a chip and the chips don't move. */
export function otherCategories(queues: Pick<QueueInfo, "path" | "category">[], path: string): string[] {
  return categoriesOf(queues.filter((q) => q.path !== path));
}
