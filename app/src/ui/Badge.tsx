import type { ComponentProps } from "react";
import { cx } from "./cx";

/** What a badge says beyond a plain count: needs you, blocked, paused, a backlog note, an
 *  estimate, a level. */
export type BadgeVariant = "warn" | "blocked" | "paused" | "note" | "est" | "level";

/** A small label on a card or row (a count, "needs you", an estimate). */
export function Badge({ variant, className, ...props }: ComponentProps<"span"> & { variant?: BadgeVariant }) {
  return <span {...props} className={cx("ui-badge", variant && `ui-badge--${variant}`, className)} />;
}
