import { describe, expect, it } from "vitest";
import { type Field, type FieldEvent, fieldShows, fieldStep, idleField } from "./fieldSteps";

// A field saved as you type (0044, InlineField.tsx): what each event saves, schedules and shows.

/** Runs events from `f`, collecting what each saved. */
function run(f: Field, events: FieldEvent[]): { field: Field; saves: string[]; blurs: number; schedules: number } {
  const saves: string[] = [];
  let blurs = 0;
  let schedules = 0;
  for (const e of events) {
    const s = fieldStep(f, e);
    f = s.field;
    if (s.save !== null) saves.push(s.save);
    if (s.blur) blurs++;
    if (s.schedule) schedules++;
  }
  return { field: f, saves, blurs, schedules };
}

describe("a field saved as you type", () => {
  it("saves once typing rests, not on every key, and leaving it saves nothing again", () => {
    const r = run(idleField("muster"), [{ kind: "arrive", value: "muster" }, { kind: "type", text: "M" }, { kind: "type", text: "Mu" }, { kind: "rest" }, { kind: "blur" }]);
    expect(r.saves).toEqual(["Mu"]);
    expect(r.schedules).toBe(2);
  });
  it("saves at once on leaving it, and a rest timer that fires after does nothing", () => {
    const r = run(idleField(""), [{ kind: "arrive", value: "" }, { kind: "type", text: "Apps" }, { kind: "blur" }, { kind: "rest" }]);
    expect(r.saves).toEqual(["Apps"]);
  });
  it("↩ leaves the field, which saves", () => {
    const enter = fieldStep(fieldStep(fieldStep(idleField(""), { kind: "arrive", value: "" }).field, { kind: "type", text: "Tools" }).field, { kind: "enter" });
    expect(enter.blur).toBe(true);
    expect(enter.save).toBeNull();
    expect(fieldStep(enter.field, { kind: "blur" }).save).toBe("Tools");
  });
  it("Esc before anything was saved puts it back and saves nothing", () => {
    const r = run(idleField("Muster"), [{ kind: "arrive", value: "Muster" }, { kind: "type", text: "Mx" }, { kind: "escape" }, { kind: "blur" }, { kind: "rest" }]);
    expect(r.saves).toEqual([]);
    expect(r.blurs).toBe(1);
    expect(fieldShows(r.field, "Muster")).toBe("Muster");
  });
  it("Esc after a save puts back what it held when focus came in, and saves that", () => {
    const r = run(idleField("Muster"), [{ kind: "arrive", value: "Muster" }, { kind: "type", text: "Mx" }, { kind: "rest" }, { kind: "type", text: "Mxy" }, { kind: "escape" }, { kind: "blur" }]);
    expect(r.saves).toEqual(["Mx", "Muster"]);
  });
  it("a chip picked saves at once", () => {
    const r = run(idleField(""), [{ kind: "arrive", value: "" }, { kind: "type", text: "Ap" }, { kind: "pick", text: "Apps" }, { kind: "rest" }, { kind: "blur" }]);
    expect(r.saves).toEqual(["Apps"]);
  });
  it("leaving the page while a save waits sends it; with none waiting, nothing", () => {
    expect(run(idleField(""), [{ kind: "arrive", value: "" }, { kind: "type", text: "Apps" }, { kind: "unmount" }]).saves).toEqual(["Apps"]);
    expect(run(idleField(""), [{ kind: "arrive", value: "" }, { kind: "type", text: "Apps" }, { kind: "rest" }, { kind: "unmount" }]).saves).toEqual(["Apps"]);
    expect(run(idleField("x"), [{ kind: "unmount" }]).saves).toEqual([]);
  });
});

describe("what the field shows", () => {
  it("what's typed while it has focus", () => {
    const f = run(idleField("muster"), [{ kind: "arrive", value: "muster" }, { kind: "type", text: "Mu" }]).field;
    expect(fieldShows(f, "muster")).toBe("Mu");
  });
  it("after leaving it, what was sent until that save settles, never the old value in between", () => {
    const left = run(idleField("muster"), [{ kind: "arrive", value: "muster" }, { kind: "type", text: "Muster" }, { kind: "blur" }]).field;
    expect(fieldShows(left, "muster")).toBe("Muster");
    // An older save settling doesn't end the wait for the newer one.
    expect(fieldShows(fieldStep(left, { kind: "saved", text: "Mus" }).field, "muster")).toBe("Muster");
    const settled = fieldStep(left, { kind: "saved", text: "Muster" }).field;
    expect(fieldShows(settled, "Muster")).toBe("Muster");
    // Refused: the value as it is.
    expect(fieldShows(settled, "muster")).toBe("muster");
  });
  it("its value at rest, as the value changes", () => {
    expect(fieldShows(idleField("a"), "b")).toBe("b");
  });
});
