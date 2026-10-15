// LTR islands (mu-149 item 7, AC4): task keys, branch names, paths, commands and estimates as typed
// read left to right inside فارسی text, `mu-12` and never `12-mu`. One class does it, `.ltr`,
// and every place that shows one of them carries it. The screenshots show the result; these hold
// the class on each render site and the rule behind it.
import MarkdownIt from "markdown-it";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { BranchCardLine, BranchPane } from "./Branch";
import Card from "./Card";
import { HomePath } from "./HomePath";
import { taskKeyLinks } from "./keyLinks";
import PaneHeader from "./PaneHeader";
import { TaskKey } from "./ProjectName";
import { RunCell } from "./Projects";
import { keyIndex } from "./taskKey";
import TaskPanel from "./TaskPanel";
import type { Task } from "./types";
import WaitingList from "./WaitingList";
import css from "./styles.css?raw";

const task = (over: Partial<Task>): Task => ({
  queue: "muster", id: "0012", prefix: "mu", title: "T", kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "todo/0012-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", search: "", ...over,
});
const html = (node: Parameters<typeof renderToStaticMarkup>[0]) => renderToStaticMarkup(node);
/** The class lists of every element in `markup` whose text is exactly `text`. */
const classesOf = (markup: string, text: string) =>
  [...markup.matchAll(/<(\w+)[^>]*?class="([^"]*)"[^>]*>([^<]*)<\/\1>/g)].filter((m) => m[3] === text).map((m) => m[2].split(" "));

describe("the .ltr rule", () => {
  it("is one rule for the class and for code, kbd and pre: left to right, isolated from the text around", () => {
    expect(css).toMatch(/\n\.ltr, code, kbd, pre \{ direction: ltr; unicode-bidi: isolate; \}/);
    // A block or cell island still lines up with the window's start, not its own.
    expect(css).toMatch(/\n\.ltr \{ text-align: match-parent; \}/);
    // Nothing else sets a direction on its own, so the class is the one way in.
    expect(css.match(/direction: ltr/g)).toHaveLength(1);
  });
});

const sources = import.meta.glob(["./**/*.tsx", "./**/*.ts", "!./**/*.test.*"], { query: "?raw", import: "default", eager: true }) as Record<string, string>;

/** The classes an element carries beside `ltr`, from every component (HomePath adds it itself). */
function besideLtr(): Set<string> {
  const lists: string[] = [];
  for (const text of Object.values(sources)) {
    for (const m of text.matchAll(/className="([^"]*)"|attrSet\("class", "([^"]*)"\)/g)) lists.push(m[1] ?? m[2]);
    for (const m of text.matchAll(/<HomePath[^>]*?className="([^"]*)"/g)) lists.push(`${m[1]} ltr`);
  }
  return new Set(lists.map((l) => l.split(" ")).filter((l) => l.includes("ltr")).flat().filter((c) => c !== "ltr"));
}

/** Logical declarations that would resolve against the island's own direction instead of the
 *  window's: a side margin, padding, border or offset, a start or end alignment, a start or end
 *  corner. A symmetric one (`margin-inline: -3px`) reads the same either way. */
function sided(prop: string, value: string): boolean {
  if (/^border-(start|end)-(start|end)-radius$/.test(prop)) return true;
  if (prop === "text-align") return /^(start|end)$/.test(value);
  if (/^(margin|padding|border|inset|scroll-margin|scroll-padding)-inline-(start|end)/.test(prop)) return true;
  if (/^(margin|padding|inset|scroll-margin|scroll-padding)-inline$/.test(prop)) {
    const [a, b = a] = value.split(/\s+/);
    return a !== b;
  }
  return false;
}

/** Each rule that gives a class used with `.ltr` a sided declaration, as "selector  prop: value". */
function sidedOnIslands(classes: Set<string>): string[] {
  const code = css.replace(/\/\*[\s\S]*?\*\//g, "");
  const out: string[] = [];
  for (const m of code.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    // The element a selector styles is its last compound: `.a .b:hover` styles a `.b`.
    const styled = m[1].split(",").map((s) => s.trim().split(/[\s>+~]+/).pop() ?? "");
    const hit = [...classes].find((c) => styled.some((s) => new RegExp(`\\.${c}(?![\\w-])`).test(s)));
    if (!hit) continue;
    for (const d of m[2].split(";")) {
      const at = d.indexOf(":");
      if (at < 0) continue;
      const [prop, value] = [d.slice(0, at).trim(), d.slice(at + 1).trim()];
      if (sided(prop, value)) out.push(`${m[1].trim()}  ${prop}: ${value}`);
    }
  }
  return out;
}

describe("an island never carries a side of its own", () => {
  // `.ltr` sets direction: ltr on its element, so a logical side on that same element resolves
  // left to right: in فارسی a key's `margin-inline-start: auto` pushed it to the wrong end, and a
  // first cell's start border landed on its inner side. Such an element keeps its classes and
  // wraps its text in an inner `.ltr`.
  it("finds a side given to a class used with .ltr, and passes a symmetric one", () => {
    expect(sided("margin-inline-start", "auto")).toBe(true);
    expect(sided("text-align", "end")).toBe(true);
    expect(sided("padding-inline", "2px 6px")).toBe(true);
    expect(sided("margin-inline", "-3px")).toBe(false);
    expect(sided("text-align", "center")).toBe(false);
    expect([...besideLtr()]).toEqual(expect.arrayContaining(["id", "path"]));
  });

  it("has no rule that gives one", () => {
    expect(sidedOnIslands(besideLtr())).toEqual([]);
  });

  it("puts a table cell's key in an inner island, so the cell's start border stays on the window's start", () => {
    expect(css).toMatch(/\n\.tasks-table td:first-child \{ border-inline-start:/);
    expect(html(createElement(TaskKey, { t: task({}), as: "td", children: "mu-12" }))).toBe('<td class="id" title="muster · mu-12"><span class="ltr">mu-12</span></td>');
    // No component puts .ltr on a cell itself.
    for (const [file, text] of Object.entries(sources)) expect(text, file).not.toMatch(/<t[dh][^>]*className="[^"]*\bltr\b/);
  });
});

describe("a task key, wherever it shows", () => {
  it("on a card, in a row and in a table cell (TaskKey)", () => {
    expect(html(createElement(TaskKey, { t: task({}), children: "mu-12" }))).toBe('<span class="id ltr" title="muster · mu-12">mu-12</span>');
    const card = html(createElement(Card, { t: task({}), now: 0, multi: false, parent: "", onOpen: () => {}, active: false, words: [] }));
    expect(classesOf(card, "mu-12")).toEqual([["id", "ltr"]]);
  });

  it("in the task pane's title, in Waiting for you and as a link in a task's text", () => {
    const pane = html(createElement(PaneHeader, { task: task({}), now: 0, onClose: () => {}, onRename: () => {} }));
    expect(classesOf(pane, "mu-12")).toEqual([["tid", "ltr"]]);
    const waiting = html(createElement(WaitingList, { tasks: [task({ waitingForOwner: true, blockedKind: "you", blocked: true })], multi: false, openKey: null, onOpen: () => {}, onChanged: () => {}, now: 0 }));
    expect(classesOf(waiting, "mu-12")).toEqual([["ltr"]]);
    const md = new MarkdownIt().use(taskKeyLinks);
    expect(md.render("see mu-12", { keys: keyIndex([{ name: "muster", prefix: "mu" }]) })).toContain('<a href="#task/muster/0012" class="task-key ltr">mu-12</a>');
  });
});

describe("a branch, a path and a command", () => {
  const started = task({ stage: "building", branch: "task/mu-12-rtl", worktree: "/Users/someone/playground/muster/worktrees/main/mu-12-rtl" });

  it("the card's branch line and the pane's branch, worktree and merge commit", () => {
    expect(html(createElement(BranchCardLine, { t: started }))).toMatch(/^<span class="card-branch ltr" /);
    const pane = html(createElement(BranchPane, { task: started }));
    expect(classesOf(pane, "task/mu-12-rtl")).toEqual([["ltr"]]);
    expect(classesOf(pane, started.worktree ?? "")).toEqual([["path", "ltr"]]);
    const merged = html(createElement(BranchPane, { task: task({ stage: "done", commit: "a8e1c42573", mergeReport: "merged" }) }));
    expect(classesOf(merged, "a8e1c42")).toEqual([["ltr"]]);
  });

  it("a path with ~ for home, with or without a class of its own", () => {
    expect(html(createElement(HomePath, { path: "/Users/someone/x", home: "/Users/someone", className: "path" }))).toBe('<span class="path ltr" title="/Users/someone/x">~/x</span>');
    expect(html(createElement(HomePath, { path: "/Users/someone/x", home: "/Users/someone" }))).toBe('<span class="ltr" title="/Users/someone/x">~/x</span>');
  });

  it("a project's run command, set or not", () => {
    expect(html(createElement(RunCell, { name: "negar", run: "swift run --product نگار" }))).toBe('<code class="path ltr">swift run --product نگار</code>');
    expect(html(createElement(RunCell, { name: "negar", run: "" }))).toContain('<code class="ltr">muster projects set negar run &quot;&lt;command&gt;&quot;</code>');
  });
});

describe("an estimate as it's typed", () => {
  it("the pane's agent time, test time and tokens fields: 2h, 30m and 500k", () => {
    const pane = html(createElement(TaskPanel, { task: task({ estTime: "2h", estTestTime: "30m", estTokens: "500k" }), now: 0, onClose: () => {}, onChanged: () => {} }));
    const pair = /<span class="est-pair">([\s\S]*?)<\/span>/.exec(pane)?.[1] ?? "";
    const inputs = [...pair.matchAll(/<input[^>]*>/g)].map((m) => m[0]);
    expect(inputs.map((i) => /value="([^"]*)"/.exec(i)?.[1])).toEqual(["2h", "30m", "500k"]);
    for (const i of inputs) expect(i).toContain('class="ltr"');
  });
});
