import type { OldPause } from "./api";

// The one-time offer to convert old pauses (mu-59), the parts without React. Before Pause (0045),
// a pause was a block whose reason said so ("owner (paused)"); the core finds those in every
// queue (`muster check --fix-pauses` does the same) and the board lists them once.

/** Whether the board shows the offer: not offered before, and the core found some. `offered` is
 *  undefined from a core that doesn't say, and `list` null until the core has answered. */
export function offerShown(offered: boolean | undefined, list: OldPause[] | null): boolean {
  return offered === false && list !== null && list.length > 0;
}

/** "dr-22 · Pets · blocked as paused since 2026-10-10 14:00 · design first": the reason is what
 *  the pause keeps as its why (the core's, without the word paused), when there's one. */
export function oldPauseLine(p: OldPause): string {
  return `${p.key} · ${p.title.trim() || "(no title)"} · blocked as paused${p.since ? ` since ${p.since}` : ""}${p.reason ? ` · ${p.reason}` : ""}`;
}

/** What the board says when some couldn't be converted (each "<key> (<queue>): <why>"); "" when
 *  all were. */
export function failedLine(failed: string[]): string {
  return failed.length ? `Couldn't make these pauses: ${failed.join("; ")}. muster check --fix-pauses tries again.` : "";
}
