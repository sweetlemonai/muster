import type { OldPause } from "./api";
import { tl, tn } from "./i18n";

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
  const params = { key: p.key, title: p.title.trim() || tn("board", "oldPauses.noTitle"), since: p.since, reason: p.reason };
  if (p.since) return tn("board", p.reason ? "oldPauses.lineSinceReason" : "oldPauses.lineSince", params);
  return tn("board", p.reason ? "oldPauses.lineReason" : "oldPauses.line", params);
}

/** What Make them pauses converts: exactly the old pauses the offer listed, by queue and id, so
 *  one found since isn't changed without being shown first. */
export function fixItems(list: OldPause[]): { queue: string; id: string }[] {
  return list.map((p) => ({ queue: p.queue, id: p.id }));
}

/** What the board says when some couldn't be converted (each "<key> (<queue>): <why>"); "" when
 *  all were. */
export function failedLine(failed: string[]): string {
  // i18n-ignore: a command, the same in every language
  const command = "muster check --fix-pauses";
  return failed.length ? tn("board", "oldPauses.failed", { list: failed.join(tl("clauseSep")), command }) : "";
}
