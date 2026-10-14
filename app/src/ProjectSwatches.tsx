import { colourName } from "./projectColours";
import { pressed } from "./shortcuts";
import { PROJECT_COLOURS } from "./themes";

/** The dot at the left of a project's sidebar row (0057): its colour, or a hollow ring for none.
 *  With `onClick` it's the button that lays the colours out under the row; without, only the dot. */
export function ColourDot({ name, colour, open = false, onClick }: { name: string; colour: string; open?: boolean; onClick?: () => void }) {
  const said = colourName(colour) || "no colour";
  if (!onClick) return <i className="side-swatch-dot" data-colour={colour || undefined} title={`Colour: ${said}`} aria-hidden="true" />;
  return (
    <button
      type="button"
      className="side-swatch-dot"
      data-colour={colour || undefined}
      aria-expanded={open}
      aria-label={`Colour for ${name}: ${said}. Click to choose`}
      title={`Colour: ${said}. Click to choose`}
      onClick={onClick}
    />
  );
}

/** A project's colours laid out in a row, in place (never a popover): None, then the palette, the
 *  current one pressed. With `onClose` (the sidebar's) Esc (`by` "key") or tabbing away ("away")
 *  folds it back; without (Settings, a place for choosing) it stays laid out. */
export function Swatches({ name, value, onPick, onClose }: { name: string; value: string; onPick: (id: string) => void; onClose?: (by: "key" | "away") => void }) {
  const options = [{ id: "", name: "None" }, ...PROJECT_COLOURS];
  return (
    <div
      className="swatches"
      role="group"
      aria-label={`Colour for ${name}`}
      onKeyDown={(e) => {
        if (onClose && pressed(e, "colour-close")) {
          // Esc here folds the colours, not the pane.
          e.stopPropagation();
          onClose("key");
        }
      }}
      onBlur={(e) => {
        // Tabbing away folds it. A click elsewhere leaves it: WebKit doesn't focus a clicked
        // button, so a click on a colour arrives with no new focus, and must still land.
        if (onClose && e.relatedTarget && !e.currentTarget.contains(e.relatedTarget)) onClose("away");
      }}
    >
      {options.map((c) => (
        <button
          key={c.id || "none"}
          type="button"
          className="swatch"
          data-colour={c.id || undefined}
          aria-pressed={c.id === value}
          aria-label={c.name}
          title={c.name}
          autoFocus={onClose !== undefined && c.id === value}
          onClick={() => onPick(c.id)}
        />
      ))}
    </div>
  );
}
