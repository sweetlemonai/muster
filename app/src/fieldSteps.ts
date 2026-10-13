// A field saved as you type (0044, InlineField.tsx), as steps: each event gives the field's next
// state and what to do (save a text, schedule the rest timer, leave the field). Pure, so the
// timing of saves, Esc and leaving the page is tested without a DOM (fieldSteps.test.ts).

export type Field = {
  /** It has focus: it shows `text`. */
  focused: boolean;
  /** What's typed. */
  text: string;
  /** What it held when focus came in, which Esc puts back. */
  before: string;
  /** The last text saved (or held when focus came in): the same again isn't saved twice. */
  sent: string;
  /** The text whose save hasn't settled yet, shown after the field is left until it does, so the
   *  old value never flashes in between; null when none is out. */
  saving: string | null;
  /** The rest timer is running: a save waits for typing to pause. */
  waiting: boolean;
};

export type FieldEvent =
  | { kind: "arrive"; value: string }
  | { kind: "type"; text: string }
  /** Typing paused: the rest timer fired. */
  | { kind: "rest" }
  | { kind: "enter" }
  | { kind: "escape" }
  | { kind: "blur" }
  /** A chip under the field picked. */
  | { kind: "pick"; text: string }
  /** The save of `text` settled, saved or refused. */
  | { kind: "saved"; text: string }
  | { kind: "unmount" };

/** The field's next state, the text to save now (null for none), whether to (re)start the rest
 *  timer, and whether to leave the field. */
export type FieldStep = { field: Field; save: string | null; schedule: boolean; blur: boolean };

export const idleField = (value: string): Field => ({ focused: false, text: value, before: value, sent: value, saving: null, waiting: false });

/** What the field shows: what's typed while it has focus, then what was sent until that save
 *  settles, then its saved value. */
export const fieldShows = (f: Field, value: string): string => (f.focused ? f.text : f.saving ?? value);

const step = (field: Field, more: Partial<Omit<FieldStep, "field">> = {}): FieldStep => ({ field, save: null, schedule: false, blur: false, ...more });

/** Save `text` unless it was the last one sent; either way the rest timer stops. */
function send(f: Field, text: string, more: Partial<Omit<FieldStep, "field">> = {}): FieldStep {
  const stopped = { ...f, text, waiting: false };
  if (text === f.sent) return step(stopped, more);
  return step({ ...stopped, sent: text, saving: text }, { ...more, save: text });
}

export function fieldStep(f: Field, e: FieldEvent): FieldStep {
  switch (e.kind) {
    case "arrive": {
      // Back in while a save is out: carry on from what it shows.
      const now = fieldShows(f, e.value);
      return step({ ...f, focused: true, text: now, before: now, sent: now, waiting: false });
    }
    case "type":
      return step({ ...f, text: e.text, waiting: true }, { schedule: true });
    case "rest":
      return f.waiting ? send(f, f.text) : step(f);
    case "enter":
      return step(f, { blur: true });
    case "escape":
      return send(f, f.before, { blur: true });
    case "blur": {
      const s = send(f, f.text);
      return { ...s, field: { ...s.field, focused: false } };
    }
    case "pick":
      return send(f, e.text);
    case "saved":
      return step(f.saving === e.text ? { ...f, saving: null } : f);
    case "unmount":
      return f.waiting ? send(f, f.text) : step(f);
  }
}
