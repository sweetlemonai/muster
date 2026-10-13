import { useState, type ReactNode } from "react";
import { modeLabel, useLook, type DisplayMode } from "./modes";

// The small views the display modes (0038) share: the header's mode chip, a board column's head
// (a strip that opens when Focus folds it), a task pane section folded to its header, and a row
// of choices shown whole or as its value. Each folds in place: nothing opens over anything.

/** The header's mode: its name, as a chip that opens Settings › Display. */
export function ModeChip({ mode, onOpen }: { mode: DisplayMode; onOpen: () => void }) {
  return (
    <button type="button" className="chip mode-chip" onClick={onOpen} title="Display mode: change it in Settings, Display. ⌘. turns Focus on and off">
      {modeLabel(mode)}
    </button>
  );
}

/** A board column's head. A column the mode folds is a button: its name, count and needs-you
 *  count on a thin strip, and a click opens the column there (or folds it back). */
export function ColumnHead({ stage, count, needs, fold, onFold, children }: {
  stage: { id: string; label: string; role: string };
  count: number;
  /** How many in the column wait on the owner: said on the strip, since its cards don't show. */
  needs: number;
  fold: "fixed" | "folded" | "opened";
  onFold: () => void;
  /** After the count in an open column's head (the board's role line). */
  children?: ReactNode;
}) {
  const title = stage.role ? `${stage.label}: ${stage.role}` : stage.label;
  if (fold === "fixed") {
    return (
      <h2 title={title}>
        {stage.label} <small>{count}</small>
        {children}
      </h2>
    );
  }
  const folded = fold === "folded";
  return (
    <h2 title={title}>
      <button type="button" className="col-fold" aria-expanded={!folded} onClick={onFold} title={folded ? `Open ${stage.label}: Focus folds it` : `Fold ${stage.label} back to its strip, as Focus has it`}>
        {stage.label} <small>{count}</small>
        {needs > 0 && <b className="count warn" title={`${needs} need you`}>{needs}</b>}
      </button>
    </h2>
  );
}

/** A task pane section that Standard and Focus fold to its header, with a word on what's in it.
 *  The header opens it in place; ADHD shows the section as it is. */
export function Fold({ label, summary, children }: { label: string; summary?: string; children?: ReactNode }) {
  const how = useLook("pane-sections");
  const [open, setOpen] = useState(false);
  if (how === "open") return <>{children}</>;
  return (
    <div className={open ? "fold open" : "fold"}>
      <button type="button" className="fold-head" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        {label} {summary && <small>{summary}</small>}
      </button>
      {open && children}
    </div>
  );
}

/** A row of choices (Who, Group by): every choice as a chip, or the current one alone, which
 *  lays the others out in its place when clicked; picking one folds them back. */
export function ChoiceChips<T extends string>({ label, className, aria, metaClass = "meta", value, options, onPick }: {
  label: string;
  className: string;
  aria: string;
  metaClass?: string;
  value: T;
  options: readonly { id: T; label: string }[];
  onPick: (v: T) => void;
}) {
  const how = useLook("choices");
  const [open, setOpen] = useState(false);
  const current = options.find((o) => o.id === value) ?? options[0];
  const pick = (v: T) => {
    setOpen(false);
    if (v !== value) onPick(v);
  };
  return (
    <span className={className} role="group" aria-label={aria}>
      <span className={metaClass}>{label}</span>
      {how === "value" && !open ? (
        <button className="chip on" aria-label={`${label}: ${current.label}, click to change`} onClick={() => setOpen(true)}>
          {current.label}
        </button>
      ) : (
        options.map((o) => (
          <button key={o.id} className={value === o.id ? "chip on" : "chip"} aria-pressed={value === o.id} onClick={() => pick(o.id)}>
            {o.label}
          </button>
        ))
      )}
    </span>
  );
}
