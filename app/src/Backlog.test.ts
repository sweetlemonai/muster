import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { backlogColumn, backlogNotes, fileDraft, NoteCard, noteTitle, projectCount, QuickAdd, quickAddKey } from "./Backlog";
import { columns } from "./lib";
import type { Note, Task } from "./types";

const note = (over: Partial<Note>): Note => ({ queue: "muster", file: "n.md", title: "A note", text: "A note", truncated: false, added: 0, ...over });
const task = (over: Partial<Task>): Task => ({
  queue: "muster", id: "0001", title: "T", kind: "task", priority: "normal", status: "inbox", stage: "backlog", blocked: false, stageHistory: [],
  folder: "inbox", path: "inbox/0001-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], commit: "", captured: "", capturedAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0,
  level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "backlog", search: "", ...over,
});

describe("backlog notes on the board", () => {
  it("falls back to (empty note) for a note with no line", () => {
    expect(noteTitle(note({ title: "Fix it" }))).toBe("Fix it");
    expect(noteTitle(note({ title: "" }))).toBe("(empty note)");
    expect(noteTitle(note({ title: "   " }))).toBe("(empty note)");
  });

  it("shows a project's notes, or every project's in All, newest first", () => {
    const notes = [
      note({ queue: "muster", file: "old.md", added: 10 }),
      note({ queue: "drover", file: "b.md", added: 30 }),
      note({ queue: "muster", file: "new.md", added: 30 }),
      note({ queue: "drover", file: "a.md", added: 30 }),
    ];
    const files = (q: string) => backlogNotes(notes, q).map((n) => `${n.queue}/${n.file}`);
    expect(files("muster")).toEqual(["muster/new.md", "muster/old.md"]);
    expect(files("all")).toEqual(["drover/a.md", "drover/b.md", "muster/new.md", "muster/old.md"]);
    expect(files("braindump")).toEqual([]);
    expect(notes.map((n) => n.file)).toEqual(["old.md", "b.md", "new.md", "a.md"]);
  });

  it("puts notes above the numbered backlog tasks in the Backlog column", () => {
    const cols = columns([task({ id: "0002", priority: "low" }), task({ id: "0003", priority: "high" }), task({ id: "0004", stage: "ready" })]);
    const items = backlogColumn(backlogNotes([note({ file: "x.md", added: 1 }), note({ file: "y.md", added: 2 })], "all"), cols.backlog);
    expect(items.map((i) => (i.kind === "note" ? i.note.file : i.task.id))).toEqual(["y.md", "x.md", "0003", "0002"]);
    expect(backlogColumn([], [])).toEqual([]);
  });

  it("counts a project's notes with its tasks in the sidebar", () => {
    const notes = [note({ queue: "muster" }), note({ queue: "muster", file: "m.md" }), note({ queue: "drover" })];
    expect(projectCount({ backlog: 1, ready: 2, blocked: 1 }, notes, "muster")).toBe(5);
    expect(projectCount({}, notes, "drover")).toBe(1);
    expect(projectCount({ done: 4 }, notes, "braindump")).toBe(4);
  });

  it("prefills File as task from the note", () => {
    expect(fileDraft(note({ title: "Board flickers", text: "Board flickers\n\nmore" }))).toEqual({
      title: "Board flickers", kind: "code", priority: "normal", body: "Board flickers\n\nmore",
    });
    expect(fileDraft(note({ title: "", text: "" })).title).toBe("");
  });

  it("draws a note as a quiet card: its title, note, and how long ago", () => {
    const html = renderToStaticMarkup(createElement(NoteCard, { n: note({ title: "", added: 100 }), now: 7300, multi: true, active: false, onOpen: () => {}, words: [] }));
    expect(html).toContain('class="note-card"');
    expect(html).toContain("(empty note)");
    expect(html).toContain(">note<");
    expect(html).toContain(">2 h<");
    expect(html).toContain(">muster<");
    const one = renderToStaticMarkup(createElement(NoteCard, { n: note({}), now: 0, multi: false, active: true, onOpen: () => {}, words: [] }));
    expect(one).toContain('class="note-card active"');
    expect(one).not.toContain('class="q"');
  });

  it("marks the words the search matched in a note's title, as task cards do", () => {
    const card = (title: string, words: string[]) => renderToStaticMarkup(createElement(NoteCard, { n: note({ title }), now: 0, multi: false, active: false, onOpen: () => {}, words }));
    expect(card("Board flickers on Résumé", ["flick", "resume"])).toContain('<span class="title">Board <mark>flick</mark>ers on <mark>Résumé</mark></span>');
    expect(card("Board flickers", [])).toContain('<span class="title">Board flickers</span>');
    expect(card("", ["empty"])).toContain('<span class="title">(empty note)</span>');
  });

  it("adds on Enter and clears on Esc, but not while an input method is composing", () => {
    expect(quickAddKey("Enter", false)).toBe("add");
    expect(quickAddKey("Escape", false)).toBe("clear");
    expect(quickAddKey("Enter", true)).toBeNull();
    expect(quickAddKey("Escape", true)).toBeNull();
    expect(quickAddKey("n", false)).toBeNull();
    expect(quickAddKey("Tab", false)).toBeNull();
  });

  it("has a quick add field, and a project chip only when there's a choice", () => {
    const one = renderToStaticMarkup(createElement(QuickAdd, { queues: ["muster"], onAdded: () => {} }));
    expect(one).toContain('placeholder="Add to the backlog…"');
    expect(one).not.toContain("to muster");
    const all = renderToStaticMarkup(createElement(QuickAdd, { queues: ["drover", "muster"], onAdded: () => {} }));
    expect(all).toContain("to drover");
  });
});
