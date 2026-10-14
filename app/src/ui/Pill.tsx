import type { ComponentProps } from "react";
import { cx } from "./cx";

/** A rounded chip: a choice in a row of them (filters, kinds, scales), or a quiet action beside
 *  them. `on` marks the chosen one. */
export function Pill({ on, className, ...props }: ComponentProps<"button"> & { on?: boolean }) {
  return <button {...props} className={cx("ui-pill", on && "ui-pill--on", className)} />;
}
