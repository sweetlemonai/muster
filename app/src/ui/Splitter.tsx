import type { KeyboardEvent, PointerEvent } from "react";
import { tl } from "../i18n";
import { dragWidth, type SplitSide, splitKey } from "../splitter";
import { cx } from "./cx";

export type SplitterProps = {
  /** The pane's width now, and the narrowest and widest it can be (a separator's value, min and max). */
  width: number;
  min: number;
  max: number;
  /** Which side of the splitter the pane is on. */
  side: SplitSide;
  /** What it resizes, for screen readers: "Projects" */
  label: string;
  /** The width a key asks for, not clamped: the caller fits it to the window and keeps it. */
  onWidth: (w: number) => void;
  /** The width at each move of a drag, and the drag's end (release or cancel): the caller needn't
   *  keep it before the end. */
  onDrag: (w: number) => void;
  onDragEnd: () => void;
  /** Double-click: back to the default width. */
  onReset: () => void;
  className?: string;
};

/** How many client px a layout px is: the Text size zoom, whichever way the engine reports pointer
 *  positions (the box and the pointer are both in client px). */
function scaleOf(el: HTMLElement): number {
  const layout = el.offsetWidth;
  const scale = layout > 0 ? el.getBoundingClientRect().width / layout : 1;
  return Number.isFinite(scale) && scale > 0 ? scale : 1;
}

/** The drag handle between a side pane and the content, the same for the projects sidebar and the
 *  task pane: drag it, double-click for the default width, or Tab to it and press ← → (Shift for
 *  bigger steps). The pointer is captured, so the drag holds over anything and the pane follows the
 *  edge without jumping. Always in the same place, its edge lit on hover, focus and drag. */
export function Splitter({ width, min, max, side, label, onWidth, onDrag, onDragEnd, onReset, className }: SplitterProps) {
  const onKeyDown = (e: KeyboardEvent<HTMLElement>) => {
    const next = splitKey(e.key, e.shiftKey, side, width);
    if (next === null) return;
    e.preventDefault();
    onWidth(next);
  };
  const onPointerDown = (e: PointerEvent<HTMLElement>) => {
    if (e.button !== 0) return;
    e.preventDefault();
    const el = e.currentTarget;
    const id = e.pointerId;
    const startX = e.clientX;
    const scale = scaleOf(el);
    el.setPointerCapture(id);
    el.setAttribute("data-dragging", "");
    const move = (ev: globalThis.PointerEvent) => onDrag(dragWidth(width, startX, ev.clientX, side, scale));
    const end = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", end);
      el.removeEventListener("pointercancel", end);
      el.removeEventListener("lostpointercapture", end);
      el.removeAttribute("data-dragging");
      if (el.hasPointerCapture(id)) el.releasePointerCapture(id);
      onDragEnd();
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", end);
    el.addEventListener("pointercancel", end);
    el.addEventListener("lostpointercapture", end);
  };
  return (
    <div
      className={cx("ui-splitter", className)}
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      aria-valuenow={width}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      title={tl("splitter")}
      onKeyDown={onKeyDown}
      onDoubleClick={onReset}
      onPointerDown={onPointerDown}
    />
  );
}
