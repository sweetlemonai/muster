import MarkdownIt from "markdown-it";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { taskKeyLinks } from "./keyLinks";
import Comments from "./Comments";
import { keyIndex, taskOfHref } from "./taskKey";
import type { Task } from "./types";

// A key written in a task's text links to that task (0033), as the task pane renders it.

const md = new MarkdownIt({ html: false, linkify: true, breaks: false }).use(taskKeyLinks);
const keys = keyIndex([{ name: "muster", prefix: "mu" }, { name: "brain dump", prefix: "br" }]);
const render = (text: string) => md.render(text, { keys });

describe("keys in task text", () => {
  it("link to their task, in any project the window has", () => {
    expect(render("Needs mu-12 first, then **BR-7**.")).toBe(
      '<p>Needs <a href="#task/muster/0012" class="task-key">mu-12</a> first, then <strong><a href="#task/brain%20dump/0007" class="task-key">BR-7</a></strong>.</p>\n',
    );
    expect(taskOfHref("#task/brain%20dump/0007")).toEqual({ queue: "brain dump", id: "0007" });
  });
  it("leave code, links, paths and unknown prefixes alone", () => {
    expect(render("`mu-12` and [mu-12](https://example.com) and docs/mu-12.md and zz-3 and utf-8")).toBe(
      '<p><code>mu-12</code> and <a href="https://example.com">mu-12</a> and docs/mu-12.md and zz-3 and utf-8</p>\n',
    );
    expect(render("```\nmu-12\n```")).toBe("<pre><code>mu-12\n</code></pre>\n");
  });
  it("change nothing without the projects' prefixes", () => {
    expect(md.render("mu-12")).toBe("<p>mu-12</p>\n");
    expect(md.render("mu-12", { keys: new Map() })).toBe("<p>mu-12</p>\n");
  });
  it("still escape what's around them", () => {
    expect(render("<b>mu-1</b>")).toBe('<p>&lt;b&gt;<a href="#task/muster/0001" class="task-key">mu-1</a>&lt;/b&gt;</p>\n');
  });
});

describe("keys in comments (0033)", () => {
  const task: Task = {
    queue: "muster", id: "0013", title: "T", kind: "code", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
    folder: "todo", path: "todo/0013-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
    openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [],
    comments: [{ who: "owner", at: "2026-10-13 09:00", text: "Same as mu-12." }], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: "ready",
  };
  it("link there too, when the pane gives the projects' prefixes", () => {
    const html = renderToStaticMarkup(createElement(Comments, { task, onChanged: () => {}, keys }));
    // Its tooltip names the project (0044): by its name, with no label set.
    expect(html).toContain('<a href="#task/muster/0012" class="task-key" title="muster · mu-12">mu-12</a>');
    expect(renderToStaticMarkup(createElement(Comments, { task, onChanged: () => {} }))).not.toContain("task-key");
  });
});
