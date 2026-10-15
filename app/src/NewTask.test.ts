import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import NewTask, { parentRequest } from "./NewTask";

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

describe("New task's Parent (0033)", () => {
  it("takes the parent's key or number, and sends its id", () => {
    for (const typed of ["mu-30", "MU-30", "30", "0030", " mu-0030 "]) expect(parentRequest(typed, "mu"), typed).toEqual({ ok: true, id: "0030" });
    expect(parentRequest("", "mu")).toEqual({ ok: true, id: "" });
    expect(parentRequest("  ", "mu")).toEqual({ ok: true, id: "" });
  });
  it("reads a key or number typed in Persian or Arabic-Indic digits as its Latin form (mu-217)", () => {
    for (const typed of ["mu-۳۰", "MU-۰۰۳۰", "۳۰", "٣٠", "mu-3۰", " ۰۰۳۰ "]) expect(parentRequest(typed, "mu"), typed).toEqual({ ok: true, id: "0030" });
    expect(parentRequest("br-۳۰", "mu")).toEqual({ ok: false, why: "br-۳۰ is another project's task: a parent is in this one (mu-30 or 30)" });
    expect(parentRequest("br-٣٠", "mu")).toEqual({ ok: false, why: "br-٣٠ is another project's task: a parent is in this one (mu-30 or 30)" });
    for (const typed of ["mu-۳۰۰۰۰", "mu-۳٫۰", "مو-۳۰"]) expect(parentRequest(typed, "mu"), typed).toEqual({ ok: false, why: "A parent is a feature or goal of this project: mu-30 or 30" });
  });
  it("says why when it's another project's task or not a task at all", () => {
    expect(parentRequest("br-30", "mu")).toEqual({ ok: false, why: "br-30 is another project's task: a parent is in this one (mu-30 or 30)" });
    expect(parentRequest("soon", "mu")).toEqual({ ok: false, why: "A parent is a feature or goal of this project: mu-30 or 30" });
  });
  it("shows the project's key as the example, and the reason inline, never an overlay", () => {
    const html = renderToStaticMarkup(createElement(NewTask, { queues: ["muster"], initial: "muster", prefixes: { muster: "mu" }, onClose: () => {}, onCreated: () => {} }));
    expect(html).toContain('placeholder="mu-30"');
    expect(html).not.toContain('inputMode="numeric"');
    expect(html).not.toContain('role="alert"');
  });
});
