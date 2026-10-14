import type { ComponentProps } from "react";
import { cx } from "./cx";

/** One button of a `Segmented`; `active` is the one that's on (and pressed). */
export function Segment({ active, className, ...props }: ComponentProps<"button"> & { active?: boolean }) {
  return <button aria-pressed={!!active} {...props} className={cx("ui-segment", active && "ui-segment--active", className)} />;
}
