import type { ClipboardEvent } from "react";
import { copiedPath, homePath } from "./lib";

/** A path with `~` for the home folder (owner, 2026-10-13): the real path is its tooltip, and
 *  copying it, or part of it, copies the real path. */
export function HomePath({ path, home, className }: { path: string; home: string; className?: string }) {
  const shown = homePath(path, home);
  const onCopy = (e: ClipboardEvent<HTMLSpanElement>) => {
    const sel = window.getSelection();
    if (!sel || sel.rangeCount === 0) return;
    const r = sel.getRangeAt(0);
    const node = e.currentTarget.firstChild;
    const inside = node !== null && r.startContainer === node && r.endContainer === node;
    const text = copiedPath(shown, path, inside ? { text: sel.toString(), start: r.startOffset, end: r.endOffset } : { text: sel.toString() });
    if (text === null) return;
    e.preventDefault();
    e.clipboardData.setData("text/plain", text);
  };
  return (
    <span className={className} title={path} onCopy={onCopy}>
      {shown}
    </span>
  );
}
