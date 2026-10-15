// mu-151, AC3: number and date inputs take Persian and Arabic-Indic digits and store Latin ones; a
// Jalali due date stores its Gregorian ISO day; a day the calendar doesn't have is refused with
// why. The core is mocked at the IPC boundary, where the window hands it what's stored.
import { isValidElement, type ReactElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { api } from "./api";
import { profileEdit } from "./accountsData";
import { dueCommit, dueSave, dueStored, dueTyped, storedEstimates, storedValue } from "./fields";
import { setLocale, setNumbering } from "./i18n/runtime";
import { DueField, DueFieldView } from "./Sidebar";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));
const invoked = vi.mocked(invoke);

beforeEach(() => invoked.mockClear());
afterEach(async () => {
  setNumbering({});
  await setLocale("en");
});

describe("number fields store Latin digits", () => {
  it("estimates, tokens and due; text fields as typed", () => {
    expect(storedValue("time", "۲h")).toBe("2h");
    expect(storedValue("test-time", "٣٠m")).toBe("30m");
    expect(storedValue("tokens", "۱.۲M")).toBe("1.2M");
    expect(storedValue("est-time", "۱.۵d")).toBe("1.5d");
    expect(storedValue("due", "۲۰۲۶-۱۰-۱۴")).toBe("2026-10-14");
    expect(storedValue("title", "فاز ۲")).toBe("فاز ۲");
    expect(storedValue("tags", "۲۰۲۶")).toBe("۲۰۲۶");
  });

  it("the Persian keyboard's decimal separator is a point", () => {
    expect(storedValue("time", "۱٫۵h")).toBe("1.5h");
    expect(storedValue("tokens", "١٫٢M")).toBe("1.2M");
  });

  it("an account's reserve and concurrency, and a role's numbers, in any digits", async () => {
    const form = { plan: "", roles: "", models: "", concurrency: "۲", reserve: "۲۰", paused: false };
    expect(profileEdit("mg", form)).toEqual({ profile: expect.objectContaining({ reserve: 20, concurrency: 2 }) });
    await api.setRole("builder", "max_turns", "۳۰");
    expect(invoked).toHaveBeenLastCalledWith("set_role", { id: "builder", field: "max_turns", value: "30" });
    await api.setRole("builder", "name", "سازنده ۲");
    expect(invoked).toHaveBeenLastCalledWith("set_role", { id: "builder", field: "name", value: "سازنده ۲" });
  });

  it("a new task's estimates", () => {
    const t = storedEstimates({ title: "کار ۳", estTime: "۲h", estTestTime: "۳۰m", estTokens: "۴۰۰k", due: "" });
    expect(t).toEqual({ title: "کار ۳", estTime: "2h", estTestTime: "30m", estTokens: "400k", due: "" });
  });

  it("reach the core in Latin digits, so a Persian digit can't get past its number checks", async () => {
    await api.setField("demo", "0007", "time", "۲h", "2026-10-14 10:00");
    expect(invoked).toHaveBeenLastCalledWith("set_field", { queue: "demo", id: "0007", field: "time", value: "2h", stamp: "2026-10-14 10:00" });
    await api.setField("demo", "0007", "title", "فاز ۲", "2026-10-14 10:00");
    expect(invoked).toHaveBeenLastCalledWith("set_field", expect.objectContaining({ field: "title", value: "فاز ۲" }));
    await api.setYourCheckLimit("demo", "۷");
    expect(invoked).toHaveBeenLastCalledWith("set_your_check_limit", { key: "demo", value: "7" });
    await api.setProjectPrefix("demo", "mu۲");
    expect(invoked).toHaveBeenLastCalledWith("set_project_prefix", { key: "demo", prefix: "mu2" });
    await api.setSetting("lease", "۱۲h");
    expect(invoked).toHaveBeenLastCalledWith("set_setting", { name: "lease", value: "12h" });
    await api.createTask("demo", { title: "T", kind: "code", priority: "normal", body: "", tags: [], estTime: "٢h", estTestTime: "", estTokens: "۵۰۰k", level: "task", parent: "", due: "", forWho: "ai" }, [], "s");
    expect(invoked).toHaveBeenLastCalledWith("create_task", expect.objectContaining({ task: expect.objectContaining({ estTime: "2h", estTokens: "500k" }) }));
  });
});

describe("a goal's due date", () => {
  it("in Jalali: a Jalali date in any digits, stored as its Gregorian ISO day", () => {
    expect(dueStored("۱۴۰۵/۰۷/۲۲", "jalali")).toEqual({ iso: "2026-10-14" });
    expect(dueStored(" 1405-7-22 ", "jalali")).toEqual({ iso: "2026-10-14" });
    expect(dueStored("١٤٠٤/١/١", "jalali")).toEqual({ iso: "2025-03-21" });
    expect(dueStored("", "jalali")).toEqual({ iso: "" });
  });

  it("refuses a Jalali day that doesn't exist, or a date not written as one, with why", () => {
    expect(dueStored("1404/12/30", "jalali")).toEqual({ error: "1404/12/30 isn't a day in the Jalali calendar." });
    expect(dueStored("۱۴۰۵/۰۷/۳۱", "jalali")).toEqual({ error: "1405/07/31 isn't a day in the Jalali calendar." });
    expect(dueStored("1405/13/01", "jalali")).toEqual({ error: "1405/13/01 isn't a day in the Jalali calendar." });
    expect(dueStored("tomorrow", "jalali")).toEqual({ error: "Write the date like 1405/07/22." });
  });

  it("in Gregorian: the ISO day, in any digits; an impossible one refused", () => {
    expect(dueStored("۲۰۲۶-۱۰-۱۴", "gregorian")).toEqual({ iso: "2026-10-14" });
    expect(dueStored("2026-2-3", "gregorian")).toEqual({ iso: "2026-02-03" });
    expect(dueStored("2026-02-30", "gregorian")).toEqual({ error: "2026-02-30 isn't a day in the calendar." });
    expect(dueStored("14/10/2026", "gregorian")).toEqual({ error: "Write the date like 2026-10-14." });
  });

  it("the refusal in Persian", async () => {
    await setLocale("fa");
    expect(dueStored("1404/12/30", "jalali")).toEqual({ error: "⁨1404/12/30⁩ روزی در تقویم خورشیدی نیست." });
  });

  it("shows the stored day to edit in the calendar picked, and one that's no day as written", () => {
    expect(dueTyped("2026-10-14", "jalali")).toBe("1405/07/22");
    expect(dueTyped("2026-10-14", "gregorian")).toBe("2026-10-14");
    expect(dueTyped("", "jalali")).toBe("");
    // A hand-edited file: shown so it can be fixed, never a crash while the pane draws.
    expect(dueTyped("2026-02-30", "jalali")).toBe("2026-02-30");
    expect(dueTyped("2026-13-01", "jalali")).toBe("2026-13-01");
    expect(renderToStaticMarkup(<DueField task={{ due: "2026-02-30" }} onSave={() => {}} />)).toContain('value="2026-02-30"');
  });

  it("on ↩ or leaving the field: saves the ISO day typed, says why not, or does nothing", () => {
    expect(dueSave("۱۴۰۵/۰۷/۲۲", "jalali", "")).toEqual({ save: "2026-10-14" });
    expect(dueSave("1405/07/22", "jalali", "2026-10-14")).toBeNull();
    expect(dueSave("", "jalali", "2026-10-14")).toEqual({ save: "" });
    expect(dueSave("1404/12/30", "jalali", "")).toEqual({ error: "1404/12/30 isn't a day in the Jalali calendar." });
    expect(dueSave("2026-10-31", "gregorian", "2026-10-14")).toEqual({ save: "2026-10-31" });
  });

  it("R1: leaving the field with nothing typed saves nothing, even a hand-edited day that's no day", () => {
    // `due: 2026-02-30` is shown as written; read as Jalali it would be a day in 2647.
    expect(dueSave(dueTyped("2026-02-30", "jalali"), "jalali", "2026-02-30")).toBeNull();
    expect(dueSave(dueTyped("2026-02-30", "gregorian"), "gregorian", "2026-02-30")).toBeNull();
    expect(dueSave(dueTyped("2026-10-14", "jalali"), "jalali", "2026-10-14")).toBeNull();
    expect(dueSave("۱۴۰۵/۰۷/۲۲", "jalali", "2026-10-14")).toBeNull();
  });

  it("R1: a Gregorian date typed out of habit in Jalali is read as the Gregorian day it is", () => {
    expect(dueStored("2026-10-14", "jalali")).toEqual({ iso: "2026-10-14" });
    expect(dueStored("۲۰۲۶/۱۰/۱۴", "jalali")).toEqual({ iso: "2026-10-14" });
    expect(dueSave("2026-10-31", "jalali", "2026-10-14")).toEqual({ save: "2026-10-31" });
    expect(dueStored("2026-02-30", "jalali")).toEqual({ error: "2026-02-30 isn't a day in the calendar." });
    // And a Jalali one typed with Gregorian picked is read as Jalali, never as the year 1405.
    expect(dueStored("1405-07-22", "gregorian")).toEqual({ iso: "2026-10-14" });
    // The seam: 1699 is a Jalali year (2320), 1700 a Gregorian one.
    expect(dueStored("1699/01/01", "jalali")).toEqual({ iso: "2320-03-21" });
    expect(dueStored("1700-01-01", "jalali")).toEqual({ iso: "1700-01-01" });
  });

  it("R1: a year no due date has is refused, in either calendar", async () => {
    expect(dueStored("1299/12/29", "jalali")).toEqual({ error: "1299/12/29 is no year for a due date: write it like 1405/07/22." });
    expect(dueStored("0026-10-14", "gregorian")).toEqual({ error: "0026-10-14 is no year for a due date: write it like 2026-10-14." });
    expect(dueStored("1300/01/01", "jalali")).toEqual({ iso: "1921-03-21" });
    await setLocale("fa");
    expect(dueStored("1299/12/29", "jalali")).toEqual({ error: "⁨1299/12/29⁩ سالی برای سررسید نیست: آن را مانند ⁨1405/07/22⁩ بنویسید." });
  });

  it("the field: in فارسی, the Jalali date with the ISO day it saves beside it", async () => {
    await setLocale("fa");
    const html = renderToStaticMarkup(<DueField task={{ due: "2026-10-14" }} onSave={() => {}} />);
    expect(html).toContain('value="1405/07/22"');
    expect(html).toContain('dir="ltr"');
    expect(html).toContain('<time dateTime="2026-10-14">');
    expect(html).toContain("2026-10-14");
  });

  it("the field: in English, the ISO day alone", () => {
    const html = renderToStaticMarkup(<DueField task={{ due: "2026-10-14" }} onSave={() => {}} />);
    expect(html).toContain('value="2026-10-14"');
    expect(html).not.toContain("<time");
  });
});

type Props = { children?: ReactNode; value?: string; role?: string; onChange?: (e: { target: { value: string } }) => void; onBlur?: () => void; onKeyDown?: (e: { key: string; currentTarget: { blur: () => void } }) => void };

/** The view's elements as plain objects (no DOM here), function components called through. */
function elements(node: ReactNode): ReactElement<Props>[] {
  if (Array.isArray(node)) return node.flatMap(elements);
  if (!isValidElement<Props>(node)) return [];
  if (typeof node.type === "function") return elements((node.type as (p: Props) => ReactNode)(node.props));
  return [node, ...elements(node.props.children)];
}

const textOf = (node: ReactNode): string =>
  typeof node === "string" || typeof node === "number" ? String(node) : Array.isArray(node) ? node.map(textOf).join("") : isValidElement<Props>(node) ? textOf(node.props.children) : "";

/** The due field as DueField wires it (R2): what's typed and the error held here, the view's own
 *  input driven with the events it gets, its commit through `dueCommit`. */
function dueField(saved: string, calendar: "jalali" | "gregorian") {
  const onSave = vi.fn();
  let typed = dueTyped(saved, calendar);
  let error = "";
  const tree = () =>
    elements(
      DueFieldView({
        typed,
        error,
        calendar,
        onType: (v) => (typed = v),
        onCommit: () => (error = dueCommit(typed, calendar, saved, onSave, "Due date saved.")),
      }),
    );
  const input = () => tree().find((e) => e.type === "input")!;
  return {
    onSave,
    type: (v: string) => input().props.onChange?.({ target: { value: v } }),
    blur: () => input().props.onBlur?.(),
    enter: () => input().props.onKeyDown?.({ key: "Enter", currentTarget: { blur: () => input().props.onBlur?.() } }),
    shown: () => input().props.value,
    alert: () => {
      const a = tree().find((e) => e.props.role === "alert");
      return a ? textOf(a.props.children) : null;
    },
  };
}

describe("the due field, typed into and left (R2)", () => {
  it("↩ on a Jalali date in Persian digits saves its Gregorian ISO day", () => {
    const f = dueField("", "jalali");
    f.type("۱۴۰۵/۰۷/۲۲");
    f.enter();
    expect(f.onSave.mock.calls).toEqual([["due", "2026-10-14", "Due date saved."]]);
    expect(f.alert()).toBeNull();
  });

  it("a day the calendar doesn't have: nothing saved, why in the alert", () => {
    const f = dueField("2026-10-14", "jalali");
    f.type("1404/12/30");
    f.blur();
    expect(f.onSave).not.toHaveBeenCalled();
    expect(f.alert()).toBe("1404/12/30 isn't a day in the Jalali calendar.");
    // Fixed and left again: saved, and the alert goes.
    f.type("1404/12/29");
    f.blur();
    expect(f.onSave.mock.calls).toEqual([["due", "2026-03-20", "Due date saved."]]);
    expect(f.alert()).toBeNull();
  });

  it("left with nothing typed: nothing saved, a hand-edited no-day included", () => {
    const f = dueField("2026-02-30", "jalali");
    expect(f.shown()).toBe("2026-02-30");
    f.blur();
    f.enter();
    expect(f.onSave).not.toHaveBeenCalled();
    expect(f.alert()).toBeNull();
    const g = dueField("2026-10-14", "jalali");
    expect(g.shown()).toBe("1405/07/22");
    g.blur();
    expect(g.onSave).not.toHaveBeenCalled();
  });

  it("emptied: the due date cleared", () => {
    const f = dueField("2026-10-14", "gregorian");
    f.type("");
    f.blur();
    expect(f.onSave.mock.calls).toEqual([["due", "", "Due date saved."]]);
  });

  it("the ISO day it saves shows beside what's typed, in a <time>, whenever they differ", () => {
    const html = (typed: string, calendar: "jalali" | "gregorian") => renderToStaticMarkup(<DueFieldView typed={typed} error="" calendar={calendar} onType={() => {}} onCommit={() => {}} />);
    expect(html("1405/07/22", "jalali")).toContain('<time dateTime="2026-10-14">saved as 2026-10-14</time>');
    expect(html("1405-07-22", "gregorian")).toContain('<time dateTime="2026-10-14">');
    expect(html("2026-10-14", "jalali")).not.toContain("<time");
    expect(html("2026-10-14", "gregorian")).not.toContain("<time");
  });
});
