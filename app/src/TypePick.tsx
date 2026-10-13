import { useState } from "react";
import { KINDS } from "./fields";

/** A task's type as one chip (DESIGN.md: show the value, not every value). Clicking it lays the
 *  other types out inline, in place, never in a popover; picking one hands it to `onPick` and folds
 *  them away again, as does a second click on the chip. */
export default function TypePick({ value, onPick }: { value: string; onPick: (kind: string) => void }) {
  const [open, setOpen] = useState(false);
  return (
    <span className="type-pick" role="group" aria-label="Type">
      <span className="meta">Type</span>
      <button type="button" className="chip on" aria-expanded={open} title={open ? "Keep this type" : "Change the type"} onClick={() => setOpen((o) => !o)}>
        {value || "no type"}
      </button>
      {open &&
        KINDS.filter((k) => k !== value).map((k) => (
          <button
            key={k}
            type="button"
            className="chip"
            onClick={() => {
              setOpen(false);
              onPick(k);
            }}
          >
            {k}
          </button>
        ))}
    </span>
  );
}
