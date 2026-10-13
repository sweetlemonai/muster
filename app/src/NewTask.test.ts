import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import NewTask from "./NewTask";

describe("New task's Human / AI chip pair (0022)", () => {
  it("is a chip pair, AI pressed until you pick Human", () => {
    const html = renderToStaticMarkup(createElement(NewTask, { queues: ["muster"], initial: "muster", onClose: () => {}, onCreated: () => {} }));
    const group = /role="group" aria-label="Who does it">([\s\S]*?)<\/div>/.exec(html)?.[1] ?? "";
    expect([...group.matchAll(/aria-pressed="(\w+)"[^>]*>(\w+)<\/button>/g)].map((m) => `${m[2]} ${m[1]}`)).toEqual(["Human false", "AI true"]);
    expect(html).not.toContain("<select aria-label=\"Who");
  });
  it("says where each kind shows: on the one board, the owner's own hatched with the You ribbon", () => {
    const html = renderToStaticMarkup(createElement(NewTask, { queues: ["muster"], initial: "muster", onClose: () => {}, onCreated: () => {} }));
    expect(html).not.toMatch(/Human board|AI board/);
  });
});
