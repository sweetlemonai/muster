import { useEffect, useRef, useState } from "react";
import { type FieldEvent, fieldShows, fieldStep, idleField } from "./fieldSteps";
import { tl } from "./i18n";
import { pressed } from "./shortcuts";
import { Pill } from "./ui";

/** How long typing rests before a field saves what's typed. */
export const SAVE_AFTER_MS = 400;

/** A one-line field that's always there and saves as you type (0044): after a short rest, and at
 *  once on ↩ or on leaving it. Esc puts back what it held when you came in, and saves that. Left,
 *  it shows what it sent until that save settles (`onSave` resolves once what it saved is loaded
 *  back), so the old value never flashes. `options` lay out under it as chips while it has focus,
 *  and a click on one picks it. What the core refuses shows under it. The timing is fieldSteps.ts. */
export default function InlineField({ value, onSave, max, placeholder, label, options = [] }: {
  /** What's saved; the field shows it whenever it doesn't have focus and no save is out. */
  value: string;
  onSave: (v: string) => Promise<unknown>;
  max: number;
  placeholder: string;
  /** Its accessible name. */
  label: string;
  options?: string[];
}) {
  const [field, setField] = useState(() => idleField(value));
  const [error, setError] = useState("");
  // The state the next event steps from (events can come before a render), the rest timer, and
  // whether the field is still on the page.
  const now = useRef(field);
  const timer = useRef(0);
  const mounted = useRef(true);
  const save = useRef(onSave);
  save.current = onSave;
  const run = (e: FieldEvent, input?: HTMLInputElement) => {
    const s = fieldStep(now.current, e);
    now.current = s.field;
    if (mounted.current) setField(s.field);
    if (s.schedule || !s.field.waiting) window.clearTimeout(timer.current);
    if (s.schedule) timer.current = window.setTimeout(() => run({ kind: "rest" }), SAVE_AFTER_MS);
    if (s.save !== null) {
      const text = s.save;
      save.current(text).then(
        () => mounted.current && setError(""),
        (err: unknown) => mounted.current && setError(String(err)),
      ).finally(() => {
        if (mounted.current) run({ kind: "saved", text });
      });
    }
    if (s.blur) input?.blur();
  };
  // Leaving the page while a save waits sends it, so nothing typed is lost.
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      run({ kind: "unmount" });
    };
    // Once, on mount and unmount: the step reads only refs.
  }, []);
  const shows = fieldShows(field, value);
  return (
    <span className="inline-field">
      <input
        value={shows}
        maxLength={max}
        placeholder={placeholder}
        aria-label={label}
        onFocus={() => run({ kind: "arrive", value })}
        onChange={(e) => run({ kind: "type", text: e.target.value })}
        onKeyDown={(e) => {
          if (pressed(e, "label-save")) {
            e.preventDefault();
            run({ kind: "enter" }, e.currentTarget);
          }
          if (pressed(e, "label-revert")) {
            // Esc here puts the field back, not the pane away.
            e.stopPropagation();
            run({ kind: "escape" }, e.currentTarget);
          }
        }}
        onBlur={() => run({ kind: "blur" })}
      />
      {field.focused && options.length > 0 && (
        <span className="inline-options" role="group" aria-label={tl("pickFor", { label })}>
          {options.map((o) => (
            // Picked without taking focus from the field, so the chips stay until it's left.
            <Pill key={o} type="button" on={o === shows} aria-pressed={o === shows} onMouseDown={(e) => e.preventDefault()} onClick={() => run({ kind: "pick", text: o })}>
              {o}
            </Pill>
          ))}
        </span>
      )}
      {error && <span className="err small" role="alert">{error}</span>}
    </span>
  );
}
