import MarkdownIt from "markdown-it";
import { createElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { NoteCard } from "./Backlog";
import Card from "./Card";
import { taskKeyLinks } from "./keyLinks";
import { Labels } from "./labels";
import PaneHeader from "./PaneHeader";
import ProjectName from "./ProjectName";
import { coloursOf, ProjectColours } from "./projectColours";
import Recent from "./Recent";
import { keyIndex } from "./taskKey";
import type { Note, Task } from "./types";
import Helper from "./Helper";
import { TableView } from "./Views";
import WaitingList from "./WaitingList";

// A project's label (0044) everywhere a task, note or line names its project; the real name, the
// one commands take, as the tooltip.

const task = (over: Partial<Task>): Task => ({
  queue: "muster", id: "0012", prefix: "mu", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "todo/0012-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", search: "", ...over,
});
const note = (over: Partial<Note>): Note => ({ queue: "muster", file: "n.md", title: "A note", text: "A note", truncated: false, added: 0, heldBy: "", heldSince: "", heartbeat: "", ...over });

const LABELS = new Map([["muster", "Muster — the queue"]]);
const html = (node: ReactNode, labels: ReadonlyMap<string, string> = LABELS) => renderToStaticMarkup(createElement(Labels.Provider, { value: labels }, node));
const NAMED = '<span class="q" title="muster">Muster — the queue</span>';

describe("a project's name where a task names it (0044)", () => {
  it("is its label, the real name as the tooltip; just the name without a label", () => {
    expect(html(createElement(ProjectName, { name: "muster" }))).toBe(NAMED);
    expect(html(createElement(ProjectName, { name: "drover" }))).toBe('<span class="q">drover</span>');
    expect(renderToStaticMarkup(createElement(ProjectName, { name: "muster" }))).toBe('<span class="q">muster</span>');
  });
  it("on a card in All projects, with the key's tooltip naming the project", () => {
    const h = html(createElement(Card, { t: task({}), now: 0, multi: true, parent: "", onOpen: () => {}, active: false, words: [] }));
    expect(h).toContain(NAMED);
    expect(h).toContain('<span class="id" title="Muster — the queue · mu-12">mu-12</span>');
  });
  it("in Waiting for you", () => {
    const h = html(createElement(WaitingList, { tasks: [task({ stage: "ready", waitingForOwner: true, blockedKind: "you", blocked: true })], multi: true, openKey: null, onOpen: () => {}, onChanged: () => {}, now: 0 }));
    expect(h).toContain(NAMED);
    expect(h).not.toContain(">muster<");
  });
  it("on a backlog note, in the table and in the task pane", () => {
    expect(html(createElement(NoteCard, { n: note({}), now: 0, multi: true, active: false, onOpen: () => {}, words: [] }))).toContain(NAMED);
    const table = html(createElement(TableView, { tasks: [task({})], now: 0, multi: true, openKey: null, onOpen: () => {}, words: [] }));
    expect(table).toContain('<td class="q" title="muster">Muster — the queue</td>');
    expect(table).toContain('title="Muster — the queue · mu-12"');
    const pane = html(createElement(PaneHeader, { task: task({}), now: 0, onClose: () => {}, onRename: () => {} }));
    expect(pane).toContain('<b class="qname" title="muster">Muster — the queue</b>');
  });
});

describe("the Helper beside its project picker (0044, review)", () => {
  it("says which project's queue it files in by the label", () => {
    const h = html(createElement(Helper, { queues: ["muster", "drover"], initial: "muster", onChanged: () => {} }));
    expect(h).toContain("It files them in Muster — the queue&#x27;s queue");
    expect(h).toContain('<option value="muster" selected="">Muster — the queue</option>');
  });
});

describe("a task key in a task's text (0044)", () => {
  const md = new MarkdownIt({ html: false, linkify: true, breaks: false }).use(taskKeyLinks);
  const keys = keyIndex([{ name: "muster", prefix: "mu" }, { name: "drover", prefix: "dr" }]);
  it("names its project by the label in its tooltip", () => {
    expect(md.render("mu-12 and dr-3", { keys, labels: LABELS })).toBe(
      '<p><a href="#task/muster/0012" class="task-key" title="Muster — the queue · mu-12">mu-12</a> and <a href="#task/drover/0003" class="task-key" title="drover · dr-3">dr-3</a></p>\n',
    );
  });
});

describe("a project's colour where it's named (0057)", () => {
  const COLOURS = new Map([["muster", "lime"]]);
  const both = (node: ReactNode, colours: ReadonlyMap<string, string> = COLOURS) =>
    html(createElement(ProjectColours.Provider, { value: colours }, node));
  const DOT = '<i class="proj-dot" data-colour="lime" aria-hidden="true"></i>';
  const NOW = new Date(2026, 9, 13, 14, 0).getTime() / 1000;
  it("is a dot before the label, and nothing for a project without one or without a provider", () => {
    expect(both(createElement(ProjectName, { name: "muster" }))).toBe(`<span class="q" title="muster">${DOT}Muster — the queue</span>`);
    expect(both(createElement(ProjectName, { name: "drover" }))).toBe('<span class="q">drover</span>');
    expect(html(createElement(ProjectName, { name: "muster" }))).toBe(NAMED);
  });
  it("on a card in All projects, in Waiting for you, in Recently finished, the table and the task pane", () => {
    expect(both(createElement(Card, { t: task({}), now: 0, multi: true, parent: "", onOpen: () => {}, active: false, words: [] }))).toContain(DOT);
    expect(both(createElement(WaitingList, { tasks: [task({ stage: "ready", waitingForOwner: true, blockedKind: "you", blocked: true })], multi: true, openKey: null, onOpen: () => {}, onChanged: () => {}, now: 0 }))).toContain(DOT);
    const merged = task({ stage: "your-check", stageHistory: [{ at: "2026-10-13 13:56", who: "muster/main (merge)", from: "testing", to: "your-check" }] });
    const recent = both(createElement(Recent, { tasks: [merged], now: NOW, kind: "all", onKind: () => {}, multi: true, openKey: null, onOpen: () => {}, onFold: () => {} }));
    expect(recent).toContain(`${DOT}Muster — the queue`);
    expect(both(createElement(TableView, { tasks: [task({})], now: 0, multi: true, openKey: null, onOpen: () => {}, words: [] }))).toContain(`<td class="q" title="muster">${DOT}Muster — the queue</td>`);
    expect(both(createElement(PaneHeader, { task: task({}), now: 0, onClose: () => {}, onRename: () => {} }))).toContain(`<b class="qname" title="muster">${DOT}Muster — the queue</b>`);
    expect(both(createElement(NoteCard, { n: note({}), now: 0, multi: true, active: false, onOpen: () => {}, words: [] }))).toContain(DOT);
  });
  it("comes only from a colour the palette has, by each queue's name", () => {
    const colours = coloursOf([{ name: "muster", colour: "lime" }, { name: "drover", colour: "red" }, { name: "germ", colour: "" }, { name: "old" }]);
    expect([...colours]).toEqual([["muster", "lime"]]);
    expect(both(createElement(ProjectName, { name: "drover" }), colours)).toBe('<span class="q">drover</span>');
  });
});
