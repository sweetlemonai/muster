import { type DragEvent, type ReactNode, useRef, useState } from "react";
import { CATEGORY_MAX, OTHER } from "./labels";
import { ColourDot, Swatches } from "./ProjectSwatches";
import { pressed } from "./shortcuts";
import { Pill } from "./ui";
import { cx } from "./ui/cx";

// The sidebar's arrangement in place (task 0057): a project's row and a category heading that can
// be dragged and moved with ⌥↑/⌥↓, a heading renamed in place, a new heading made at the bottom,
// and a project's colour picked from its dot. What a move does is projectOrder.ts; ProjectList
// holds the state and asks the core to keep it.

/** Where a drop lands on what's under the pointer: before or after a row or heading, or into a
 *  heading (a project dropped on it). */
export type Side = "before" | "after" | "into";

/** Which half of the element the pointer is over. */
export const half = (e: DragEvent<HTMLElement>): "before" | "after" => {
  const r = e.currentTarget.getBoundingClientRect();
  return e.clientY < r.top + r.height / 2 ? "before" : "after";
};

type Keys = { key: string; metaKey: boolean; ctrlKey: boolean; altKey: boolean };
/** ⌥↑ is a step up (-1), ⌥↓ down (1); anything else none. */
export const stepOf = (e: Keys): -1 | 1 | null => (pressed(e, "move-up") ? -1 : pressed(e, "move-down") ? 1 : null);

/** What a screen reader hears of a row or heading that moves. */
export const MOVE_KEYS = "Alt+ArrowUp Alt+ArrowDown";

/** A heading as it's kept: control characters out, trimmed, at most 24 characters, "other" in any
 *  case the Other heading (as the core's arrange::heading). */
export function cleanHeading(name: string): string {
  const h = [...name.replace(/\p{Cc}/gu, "").trim()].slice(0, CATEGORY_MAX).join("").trimEnd();
  return h.toLowerCase() === OTHER.toLowerCase() ? OTHER : h;
}

/** The drag handlers a row or heading takes; without them (no way to save) it doesn't move. */
export type DragProps = {
  /** This one is being dragged. */
  dragging: boolean;
  /** Where something dragged over it would land, if it would. */
  drop: Side | null;
  onDragStart: () => void;
  onDragEnd: () => void;
  /** It takes what's being dragged (projectOrder.accepts). One that doesn't lets the drag and the
   *  drop go on to the group around it, which may take them (review R1). */
  accepts: boolean;
  /** What it takes is dragged over it, on this half. */
  onOver: (side: "before" | "after") => void;
  onDrop: (side: "before" | "after") => void;
  onStep: (by: -1 | 1) => void;
};

/** The drag-and-drop and ⌥↑/⌥↓ attributes of a row or heading: draggable with its name as the
 *  text a drop elsewhere gets, a drop target while something it takes is dragged over it. */
function dragAttrs(d: DragProps | undefined, text: string) {
  if (!d) return {};
  return {
    draggable: true,
    onDragStart: (e: DragEvent<HTMLElement>) => {
      e.stopPropagation();
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData("text/plain", text);
      d.onDragStart();
    },
    onDragEnd: d.onDragEnd,
    onDragOver: (e: DragEvent<HTMLElement>) => {
      if (!d.accepts) return;
      e.preventDefault();
      e.stopPropagation();
      e.dataTransfer.dropEffect = "move";
      d.onOver(half(e));
    },
    // As the dragover: a drop it doesn't take goes on to the group around it.
    onDrop: (e: DragEvent<HTMLElement>) => {
      if (!d.accepts) return;
      e.preventDefault();
      e.stopPropagation();
      d.onDrop(half(e));
    },
    onKeyDown: (e: React.KeyboardEvent<HTMLElement>) => {
      const by = stepOf(e);
      if (by === null) return;
      e.preventDefault();
      d.onStep(by);
    },
  };
}

/** A project's row in the sidebar (0057): its colour dot at the left edge (a click lays its
 *  colours out under the row, in place; `onFold` folds them, `by` Esc or tabbing away), then the
 *  row itself (`children`, ProjectItem); dragged by anywhere on it, and ⌥↑/⌥↓ on it moves it. */
export function SideRow({ rowKey, label, colour, swatches, onDot, onColour, onFold, drag, children }: {
  rowKey: string;
  label: string;
  colour: string;
  /** Its colours are laid out. */
  swatches: boolean;
  /** Without these, the dot only shows the colour. */
  onDot?: () => void;
  onColour?: (id: string) => void;
  onFold?: (by: "key" | "away") => void;
  drag?: DragProps;
  children: ReactNode;
}) {
  return (
    <div className={cx("side-row", drag?.dragging && "dragging", drag?.drop && `drop-${drag.drop}`)} data-key={rowKey} {...dragAttrs(drag, label)}>
      <span className="side-row-line">
        <ColourDot name={label} colour={colour} open={swatches} onClick={onDot} />
        {children}
      </span>
      {swatches && onColour && onFold && <Swatches name={label} value={colour} onPick={onColour} onClose={onFold} />}
    </div>
  );
}

/** A category heading (0044) as it moves (0057): focusable, dragged with its projects, a drop
 *  target for a project (into it) or a heading (before or after it); a double click or ↩ renames
 *  it in place. A new heading (`fresh`) has no project yet: it says how to give it one, and
 *  Remove takes it away. */
export function HeadingFrame({ name, count, fresh = false, drag, onRename, onRemove, children }: {
  name: string;
  /** Its projects, for what a screen reader hears. */
  count: number;
  fresh?: boolean;
  drag?: DragProps;
  onRename?: () => void;
  onRemove?: () => void;
  children: ReactNode;
}) {
  const said = fresh ? "new, no projects yet" : `${count} ${count === 1 ? "project" : "projects"}`;
  const attrs = dragAttrs(drag, name);
  return (
    <div
      className={cx("side-cat", fresh && "fresh", drag?.dragging && "dragging", drag?.drop && `drop-${drag.drop}`)}
      data-heading={name}
      tabIndex={drag || onRename ? 0 : undefined}
      role={drag || onRename ? "button" : undefined}
      aria-keyshortcuts={drag ? MOVE_KEYS : undefined}
      aria-label={`${name}, category heading, ${said}${onRename ? ": ↩ renames it" : ""}${drag ? ", ⌥↑ ⌥↓ move it" : ""}`}
      title={onRename ? `Double-click or ↩ to rename${drag ? "; drag it, or ⌥↑ ⌥↓, to move it with its projects" : ""}` : undefined}
      {...attrs}
      onKeyDown={(e) => {
        if (onRename && pressed(e, "heading-rename") && e.target === e.currentTarget) {
          e.preventDefault();
          onRename();
          return;
        }
        attrs.onKeyDown?.(e);
      }}
      onDoubleClick={onRename}
    >
      {children}
      {fresh && (
        <span className="side-cat-fresh">
          empty: drag a project here, or ⌥↓ the last one above
          {onRemove && <Pill type="button" className="side-cat-remove" onClick={onRemove} title="Take this new heading away">Remove</Pill>}
        </span>
      )}
    </div>
  );
}

/** A heading renamed in place (0057), in its place: the field with its name, the keys under it.
 *  ↩ or leaving the field saves (a name another heading has puts the two together), Esc keeps
 *  it; a refusal shows under the field, which stays open. */
export function HeadingEdit({ name, onSave, onDone }: { name: string; onSave: (to: string) => Promise<void>; onDone: () => void }) {
  const [error, setError] = useState("");
  // Saved or left once: ↩ blurs the field too, and Esc's blur saves nothing.
  const settled = useRef(false);
  const save = (typed: string) => {
    if (settled.current) return;
    settled.current = true;
    const to = cleanHeading(typed);
    if (to === name || to === "") {
      onDone();
      return;
    }
    onSave(to).then(onDone, (e: unknown) => {
      settled.current = false;
      setError(String(e));
    });
  };
  return (
    <div className="side-cat side-editing">
      <input
        className="side-label-input"
        autoFocus
        defaultValue={name}
        maxLength={CATEGORY_MAX}
        aria-label={`Name of the heading ${name}`}
        placeholder={name}
        onKeyDown={(e) => {
          if (pressed(e, "label-save")) save(e.currentTarget.value);
          if (pressed(e, "label-revert")) {
            e.stopPropagation();
            settled.current = true;
            onDone();
          }
        }}
        onBlur={(e) => save(e.currentTarget.value)}
      />
      <small className="meta">↩ renames, Esc keeps it · another heading's name joins the two</small>
      {error && <small className="err" role="alert">{error}</small>}
    </div>
  );
}

/** "+ Category" at the bottom of the projects (0057): a click opens a field in its place; ↩ (or
 *  leaving it with a name) makes the heading, empty until a project is dragged or moved onto it.
 *  `onAdd` says why a name can't be one ("" when it was made), which shows under the field. */
export function NewCategory({ onAdd, open: start = false }: { onAdd: (name: string) => string; open?: boolean }) {
  const [open, setOpen] = useState(start);
  const [error, setError] = useState("");
  // Made or left once: the field going away blurs it too.
  const settled = useRef(false);
  if (!open) {
    return (
      <Pill
        type="button"
        className="side-new-cat"
        onClick={() => {
          settled.current = false;
          setOpen(true);
        }}
        title="A new heading: then drag a project onto it"
      >
        + Category
      </Pill>
    );
  }
  const close = () => {
    settled.current = true;
    setOpen(false);
    setError("");
  };
  const add = (typed: string) => {
    if (settled.current) return;
    if (cleanHeading(typed) === "") return close();
    const why = onAdd(typed);
    if (why) setError(why);
    else close();
  };
  return (
    <div className="side-new-cat side-editing">
      <input
        className="side-label-input"
        autoFocus
        maxLength={CATEGORY_MAX}
        aria-label="New category"
        placeholder="New category"
        onKeyDown={(e) => {
          if (pressed(e, "label-save")) add(e.currentTarget.value);
          if (pressed(e, "label-revert")) {
            e.stopPropagation();
            close();
          }
        }}
        onBlur={(e) => add(e.currentTarget.value)}
      />
      <small className="meta">↩ makes it, Esc doesn't · then drag a project onto it</small>
      {error && <small className="err" role="alert">{error}</small>}
    </div>
  );
}
