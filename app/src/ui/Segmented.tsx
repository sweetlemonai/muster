import type { ComponentProps } from "react";
import { cx } from "./cx";

/** A row of joined toggle buttons, one of them on: a `group` of `Segment`s. Give it an
 *  `aria-label`. (Drover's has sizes; Muster draws one, the task pane's.) */
export function Segmented({ className, ...props }: ComponentProps<"div">) {
  return <div role="group" {...props} className={cx("ui-segmented", className)} />;
}
