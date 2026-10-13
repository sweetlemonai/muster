import { useEffect, useRef, useState } from "react";
import { pressed } from "./shortcuts";

/** How long typing rests before a field saves what's typed. */
export const SAVE_AFTER_MS = 400;

/** A one-line field that's always there and saves as you type (0044): after a short rest, and at
 *  once on ↩ or on leaving it. Esc puts back what it held when you came in, and saves that.
 *  `options` lay out under it as chips while it has focus, and a click on one picks it. What the
 *  core refuses shows under it. */
export default function InlineField({ value, onSave, max, placeholder, label, options = [] }: {
  /** What's saved; the field shows it whenever it doesn't have focus. */
  value: string;
  onSave: (v: string) => Promise<unknown>;
  max: number;
  placeholder: string;
  /** Its accessible name. */
  label: string;
  options?: string[];
}) {
  // What's typed while the field has focus; null shows the saved value.
  const [draft, setDraft] = useState<string | null>(null);
  const [error, setError] = useState("");
  const timer = useRef(0);
  // The latest text, the last one sent, and the one the field held when focus came in.
  const latest = useRef(value);
  const sent = useRef(value);
  const before = useRef(value);
  const save = useRef(onSave);
  save.current = onSave;
  const send = (v: string) => {
    window.clearTimeout(timer.current);
    timer.current = 0;
    if (v === sent.current) return;
    sent.current = v;
    save.current(v).then(
      () => setError(""),
      (e: unknown) => setError(String(e)),
    );
  };
  // Leaving the page while a save waits sends it, so nothing typed is lost.
  useEffect(
    () => () => {
      if (timer.current) {
        window.clearTimeout(timer.current);
        if (latest.current !== sent.current) void save.current(latest.current).catch(() => undefined);
      }
    },
    [],
  );
  const pick = (v: string) => {
    latest.current = v;
    setDraft(v);
    send(v);
  };
  return (
    <span className="inline-field">
      <input
        value={draft ?? value}
        maxLength={max}
        placeholder={placeholder}
        aria-label={label}
        onFocus={() => {
          before.current = value;
          sent.current = value;
          latest.current = value;
          setDraft(value);
        }}
        onChange={(e) => {
          const v = e.target.value;
          latest.current = v;
          setDraft(v);
          window.clearTimeout(timer.current);
          timer.current = window.setTimeout(() => send(v), SAVE_AFTER_MS);
        }}
        onKeyDown={(e) => {
          if (pressed(e, "label-save")) {
            e.preventDefault();
            e.currentTarget.blur();
          }
          if (pressed(e, "label-revert")) {
            // Esc here puts the field back, not the pane away.
            e.stopPropagation();
            pick(before.current);
            e.currentTarget.blur();
          }
        }}
        onBlur={() => {
          send(latest.current);
          setDraft(null);
        }}
      />
      {draft !== null && options.length > 0 && (
        <span className="inline-options" role="group" aria-label={`Pick for ${label}`}>
          {options.map((o) => (
            // Picked without taking focus from the field, so the chips stay until it's left.
            <button key={o} type="button" className={o === draft ? "chip on" : "chip"} aria-pressed={o === draft} onMouseDown={(e) => e.preventDefault()} onClick={() => pick(o)}>
              {o}
            </button>
          ))}
        </span>
      )}
      {error && <span className="err small" role="alert">{error}</span>}
    </span>
  );
}
