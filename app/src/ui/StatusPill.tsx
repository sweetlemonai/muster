import type { ComponentProps } from "react";
import type { StageId } from "../lib";
import { cx } from "./cx";

/** Muster's states: a stage, or the blocked and paused flags. */
export type PillStatus = StageId | "blocked" | "paused";

/** A small label in the colour of a task's stage (or blocked's red, or paused's outline). */
export function StatusPill({ status, className, ...props }: ComponentProps<"span"> & { status: PillStatus }) {
  return <span {...props} className={cx("ui-status-pill", `ui-status-pill--${status}`, className)} />;
}
