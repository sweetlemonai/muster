import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { CardBand, CardLookPicker, cardClass, LiveDot, SAMPLES } from "./CardLook";
import { Machine } from "./settingsData";

const band = (t: { stage: string; blocked: boolean; waitingForOwner: boolean; priority: string }) =>
  renderToStaticMarkup(createElement(CardBand, { t }));
const text = (html: string, cls: string) => new RegExp(`<span class="${cls}">([^<]*)</span>`).exec(html)?.[1];

describe("cardClass", () => {
  it("carries the priority, and blocked, needs you and selected as classes", () => {
    expect(cardClass({ priority: "high", blocked: false, waitingForOwner: false }, false)).toBe("card p-high");
    expect(cardClass({ priority: "", blocked: true, waitingForOwner: true }, true)).toBe("card p-normal active needs-you blocked");
  });
  it("is blocked by the tasks it depends on too, with no flag (0024)", () => {
    expect(cardClass({ priority: "low", blocked: false, blockedKind: "tasks", waitingForOwner: false }, false)).toBe("card p-low blocked");
    expect(cardClass({ priority: "low", blocked: false, blockedKind: "", waitingForOwner: false }, false)).toBe("card p-low");
    const html = renderToStaticMarkup(createElement(CardBand, { t: { stage: "ready", blocked: false, blockedKind: "tasks", waitingForOwner: false, priority: "" } }));
    expect(text(html, "by-stage")).toBe("Blocked");
  });
  it("reads P0–P3 as the board sorts them, and anything unknown as normal", () => {
    const pri = (priority: string) => cardClass({ priority, blocked: false, waitingForOwner: false }, false);
    expect([pri("P0"), pri("P1"), pri("P2"), pri("P3")]).toEqual(["card p-high", "card p-high", "card p-normal", "card p-low"]);
    expect(pri("urgent")).toBe("card p-normal");
  });
  it("marks the owner's own task, and only theirs (0022)", () => {
    expect(cardClass({ priority: "high", blocked: false, waitingForOwner: false, forWho: "human" }, false)).toBe("card p-high human");
    expect(cardClass({ priority: "high", blocked: false, waitingForOwner: false, forWho: "ai" }, false)).toBe("card p-high");
  });
});

describe("the Top bar band", () => {
  it("names the stage, or the priority when colour shows priority", () => {
    const html = band({ stage: "your-check", blocked: false, waitingForOwner: false, priority: "high" });
    expect(text(html, "by-stage")).toBe("Your check");
    expect(text(html, "by-pri")).toBe("high priority");
    expect(html).toContain('aria-hidden="true"');
  });
  it("names the priority the way the colour reads it", () => {
    expect(text(band({ stage: "ready", blocked: false, waitingForOwner: false, priority: "P0" }), "by-pri")).toBe("high priority");
    expect(text(band({ stage: "ready", blocked: false, waitingForOwner: false, priority: "" }), "by-pri")).toBe("normal priority");
  });
  it("says blocked or needs you, whatever the colour shows, blocked first", () => {
    const both = band({ stage: "building", blocked: true, waitingForOwner: true, priority: "low" });
    expect([text(both, "by-stage"), text(both, "by-pri")]).toEqual(["Blocked", "Blocked"]);
    const needs = band({ stage: "your-check", blocked: false, waitingForOwner: true, priority: "" });
    expect([text(needs, "by-stage"), text(needs, "by-pri")]).toEqual(["Needs you", "Needs you"]);
  });
});

describe("Appearance's Cards picker", () => {
  const picker = (look: "stripe" | "tint" | "top" | "dot" | "outline", colour: "stage" | "priority") =>
    renderToStaticMarkup(createElement(CardLookPicker, { look, colour, onLook: () => {}, onColour: () => {} }));
  it("shows every look with a preview, the current one pressed", () => {
    const html = picker("tint", "stage");
    for (const l of ["stripe", "tint", "top", "dot", "outline"]) expect(html).toContain(`look-preview look-${l}"`);
    expect(html.match(/class="look on"/g)).toHaveLength(1);
    expect(html).toMatch(/class="look on"[^>]*aria-pressed="true"[^>]*>.*?look-tint/);
  });
  it("previews a plain card, one that needs you and a blocked one, in each look", () => {
    const html = picker("stripe", "stage");
    expect(html.match(/card p-[a-z]+[^"]* s-/g)).toHaveLength(5 * SAMPLES.length);
    expect(html.match(/ needs-you s-your-check/g)).toHaveLength(5);
    expect(html.match(/ blocked s-ready/g)).toHaveLength(5);
  });
  it("previews the owner's own task with its You ribbon in every look (0022)", () => {
    const html = picker("tint", "stage");
    expect(html.match(/ human s-ready/g)).toHaveLength(5);
    expect(html.match(/<span class="you-ribbon"[^>]*>You<\/span>/g)).toHaveLength(5);
  });
  it("previews follow what the colour shows", () => {
    expect(picker("dot", "priority").match(/look-by-priority/g)).toHaveLength(5);
    expect(picker("dot", "stage")).not.toContain("look-by-priority");
  });
  it("offers stage and priority, the current one pressed", () => {
    const html = picker("stripe", "priority");
    expect(html).toMatch(/class="chip on" aria-pressed="true">Priority</);
    expect(html).toMatch(/class="chip" aria-pressed="false">Stage</);
  });
});

describe("the live dot (0025)", () => {
  it("shows while one of the task's sessions is live, and nothing otherwise", () => {
    expect(renderToStaticMarkup(createElement(LiveDot, { t: { live: true } }))).toBe(
      '<i class="dot live-dot" role="img" aria-label="live" title="A session on it is working now: its transcript changed in the last 5 minutes"></i>',
    );
    expect(renderToStaticMarkup(createElement(LiveDot, { t: { live: false } }))).toBe("");
  });
  it("names this machine's LIVE window from Settings (0036)", () => {
    const html = renderToStaticMarkup(createElement(Machine.Provider, { value: { leaseMinutes: 720, liveMinutes: 15 } }, createElement(LiveDot, { t: { live: true } })));
    expect(html).toContain('title="A session on it is working now: its transcript changed in the last 15 minutes"');
  });
});
