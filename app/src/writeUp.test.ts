import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import EditBody from "./EditBody";
import { editKey, isDirty, mayLeave, savedText, UNSAVED } from "./writeUp";
import type { Task } from "./types";

const key = (k: string, mods: Partial<{ metaKey: boolean; ctrlKey: boolean; altKey: boolean; isComposing: boolean }> = {}) => ({
  key: k, metaKey: false, ctrlKey: false, altKey: false, isComposing: false, ...mods,
});

describe("the write-up editor's dirty state", () => {
  it("is dirty only when the text differs from what Edit opened", () => {
    expect(isDirty("# T\n\nOld.", "# T\n\nOld.")).toBe(false);
    expect(isDirty("# T\n\nOld.", "# T\n\nOld!")).toBe(true);
    expect(isDirty("# T", "# T\n")).toBe(true);
    expect(isDirty("کار 🚀", "کار 🚀")).toBe(false);
  });
});

describe("the write-up editor's keys", () => {
  it("⌘S or Ctrl+S saves, whatever the case of the letter", () => {
    expect(editKey(key("s", { metaKey: true }), false, false)).toBe("save");
    expect(editKey(key("S", { metaKey: true }), true, false)).toBe("save");
    expect(editKey(key("s", { ctrlKey: true }), true, false)).toBe("save");
    expect(editKey(key("s"), true, false)).toBeNull();
    expect(editKey(key("s", { metaKey: true, altKey: true }), true, false)).toBeNull();
  });
  it("Esc cancels at once when nothing was typed", () => {
    expect(editKey(key("Escape"), false, false)).toBe("cancel");
  });
  it("Esc with unsaved changes asks for a second Esc first", () => {
    expect(editKey(key("Escape"), true, false)).toBe("arm");
    expect(editKey(key("Escape"), true, true)).toBe("cancel");
  });
  it("leaves other keys, and keys inside an input method's composition, alone", () => {
    expect(editKey(key("Enter", { metaKey: true }), true, false)).toBeNull();
    expect(editKey(key("a"), true, true)).toBeNull();
    expect(editKey(key("Escape", { isComposing: true }), true, true)).toBeNull();
    expect(editKey(key("s", { metaKey: true, isComposing: true }), true, false)).toBeNull();
  });
});

describe("leaving a pane with unsaved changes", () => {
  const t7 = { kind: "task", key: "demo/0007" };
  it("is refused for another card, a new task, settings or closing", () => {
    for (const to of [{ kind: "task", key: "demo/0008" }, { kind: "task", key: "other/0007" }, { kind: "new" }, { kind: "appearance" }, null]) {
      expect(mayLeave(true, t7, to)).toBe(false);
    }
  });
  it("is fine when nothing is unsaved, or the pane stays the same card", () => {
    expect(mayLeave(false, t7, { kind: "task", key: "demo/0008" })).toBe(true);
    expect(mayLeave(false, t7, null)).toBe(true);
    expect(mayLeave(true, t7, { kind: "task", key: "demo/0007" })).toBe(true);
    expect(mayLeave(false, null, { kind: "new" })).toBe(true);
  });
  it("says what to do in the editor", () => {
    expect(UNSAVED).toBe("Unsaved changes: Save, or press Esc twice to discard");
  });
});

describe("after a save", () => {
  it("says whether anything changed", () => {
    expect(savedText("todo/0007-t.md")).toBe("Saved. The edit is in the log.");
    expect(savedText(null)).toBe("No change: the text is the same as the file's.");
  });
});

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0007", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "todo/0007-t.md", claimedBy: "", heartbeat: "", waitingOn: "", commit: "", captured: "", questions: [],
  openQuestions: 0, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTokens: "", estMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", attachments: [], logTail: [], modified: 0,
  level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", ...over,
});

describe("the editor in the pane", () => {
  it("opens on the write-up in a textarea, with Save and Cancel, in place", () => {
    const html = renderToStaticMarkup(createElement(EditBody, { task: task({}), opened: "# T\n\nOld <b>text</b>.", onClose: () => {}, onSaved: () => {} }));
    expect(html).toContain("# T\n\nOld &lt;b&gt;text&lt;/b&gt;.</textarea>");
    expect([...html.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1])).toEqual(["Save", "Cancel"]);
    expect(html).toContain("⌘S saves · Esc cancels");
    expect(html).not.toContain("modal");
  });
});
