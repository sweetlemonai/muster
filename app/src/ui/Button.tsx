import type { ComponentProps } from "react";
import { cx } from "./cx";

/** Drover's names; Muster draws only these two. */
export type ButtonVariant = "default" | "primary";

export type ButtonProps = ComponentProps<"button"> & { variant?: ButtonVariant };

/** A bordered button; `primary` is a place's one next step. Native props (and `ref`) pass
 *  through, and no type is forced. Its size and quiet look come from where it sits (styles.css). */
export function Button({ variant = "default", className, ...props }: ButtonProps) {
  return <button {...props} className={cx("ui-button", variant !== "default" && `ui-button--${variant}`, className)} />;
}
