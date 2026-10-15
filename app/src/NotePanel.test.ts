import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import NotePanel, { copyNow } from "./NotePanel";
import type { Note } from "./types";

const note = (over: Partial<Note>): Note => ({ queue: "muster", file: "2026-10-12-1405-x.md", title: "Board flickers", text: "Board flickers\n\n**on resize**", truncated: false, added: 0, heldBy: "", heldSince: "", heartbeat: "", ...over });
const pane = (over: Partial<Note>) =>
  renderToStaticMarkup(createElement(NotePanel, { note: note(over), now: 3600, onClose: () => {}, onChanged: () => {}, onFiled: () => {} }));

describe("the note pane", () => {
  it("shows the note's text as sanitized Markdown, and its three actions", () => {
    const html = pane({});
    const row = /<section class="controls">\s*<div class="control-row">([\s\S]*?)<\/div>/.exec(html)?.[1] ?? "";
    expect([...row.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1])).toEqual(["File as task", "Drop", "Copy shaping brief"]);
    expect(html).toContain("<h1>Board flickers</h1>");
    expect(html).toContain("<strong>on resize</strong>");
    expect(html).toContain("added 1 h ago");
    const raw = pane({ text: "<img src=x onerror=alert(1)>" });
    expect(raw).not.toContain("<img");
    expect(raw).toContain("&lt;img");
  });

  it("writes the clipboard in the click itself, with the brief already loaded", async () => {
    const calls: string[] = [];
    const clipboard = { writeText: (t: string) => { calls.push(t); return Promise.resolve(); } };
    const done = copyNow("the brief", clipboard);
    // Written before anything is awaited: WebKit refuses a clipboard write after an await.
    expect(calls).toEqual(["the brief"]);
    await expect(done).resolves.toBeUndefined();
    await expect(copyNow("x", undefined)).rejects.toThrow("clipboard isn't available");
    await expect(copyNow(null, clipboard)).rejects.toThrow("still loading");
    expect(calls).toEqual(["the brief"]);
    // Until the brief is loaded, its button is off.
    const html = pane({});
    expect(html).toMatch(/<button type="button" disabled=""[^>]*>Copy shaping brief<\/button>/);
  });

  it("says when a note is empty or cut, and leaves its front matter out", () => {
    expect(pane({ title: "", text: "" })).toContain("(empty note)");
    expect(pane({ title: "", text: "" })).toContain("This note is empty.");
    expect(pane({ truncated: true })).toContain("Shown up to 20 KB");
    expect(pane({ text: "---\ntype: bug\n---\n# Heading" })).not.toContain("type: bug");
  });
});
