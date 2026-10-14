import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { OldPause } from "./api";
import { OldPausesNotice } from "./OldPauses";
import { failedLine, oldPauseLine, offerShown } from "./oldPauseData";

// The one-time offer on load (mu-59): pauses recorded before Pause existed (0045) are blocks whose
// reason says paused; the board lists them once and offers to make them real pauses.

const old = (over: Partial<OldPause> = {}): OldPause => ({
  queue: "drover", id: "0022", key: "dr-22", title: "Pets", stage: "building", reason: "", since: "2026-10-10 14:00", path: "doing/0022-pets.md", ...over,
});
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&quot;/g, '"').replace(/\s+/g, " ").trim();
const notice = (over: Partial<Parameters<typeof OldPausesNotice>[0]> = {}) =>
  renderToStaticMarkup(createElement(OldPausesNotice, { list: [old(), old({ queue: "muster", id: "0007", key: "mu-7", title: "", reason: "design first", since: "2026-10-09 09:30" })], busy: false, error: "", onFix: () => {}, onLater: () => {}, ...over }));

describe("whether the board offers to convert old pauses", () => {
  it("only when it hasn't been offered and there are some", () => {
    expect(offerShown(false, [old()])).toBe(true);
    expect(offerShown(true, [old()])).toBe(false);
    expect(offerShown(false, [])).toBe(false);
    // Not yet asked (null), or a core that doesn't say (undefined): nothing yet.
    expect(offerShown(false, null)).toBe(false);
    expect(offerShown(undefined, [old()])).toBe(false);
  });
  it("names each by key and title, how long it's been blocked as paused, and the reason it keeps", () => {
    expect(oldPauseLine(old())).toBe("dr-22 · Pets · blocked as paused since 2026-10-10 14:00");
    expect(oldPauseLine(old({ reason: "design first" }))).toBe("dr-22 · Pets · blocked as paused since 2026-10-10 14:00 · design first");
    expect(oldPauseLine(old({ title: "  " }))).toBe("dr-22 · (no title) · blocked as paused since 2026-10-10 14:00");
    expect(oldPauseLine(old({ since: "" }))).toBe("dr-22 · Pets · blocked as paused");
  });
  it("says which it couldn't convert, and how to try again; nothing when all went", () => {
    expect(failedLine(["dr-22 (drover): its holder holds 0031 in Building"])).toBe(
      "Couldn't make these pauses: dr-22 (drover): its holder holds 0031 in Building. muster check --fix-pauses tries again.",
    );
    expect(failedLine(["a", "b"])).toBe("Couldn't make these pauses: a; b. muster check --fix-pauses tries again.");
    expect(failedLine([])).toBe("");
  });
});

describe("the offer on the board", () => {
  it("lists every one before it changes anything, inline, with both choices and what they do", () => {
    const html = notice();
    expect(html).toMatch(/^<section class="old-pauses" aria-label="Old pauses">/);
    expect(html).not.toMatch(/modal|dialog|popover/);
    const t = text(html);
    expect(t).toContain("2 tasks were paused before Pause existed");
    expect(t).toContain("dr-22 · Pets · blocked as paused since 2026-10-10 14:00");
    expect(t).toContain("mu-7 · (no title) · blocked as paused since 2026-10-09 09:30 · design first");
    expect([...html.matchAll(/<button[^>]*class="([^"]*)"[^>]*>([^<]*)<\/button>/g)].map((m) => `${m[2]}: ${m[1]}`)).toEqual(["Make them pauses: ui-button ui-button--primary", "Not now: ui-button"]);
    expect(t).toContain("muster check --fix-pauses");
  });
  it("says one in the singular", () => {
    expect(text(notice({ list: [old()] }))).toContain("1 task was paused before Pause existed");
  });
  it("turns its buttons off while it works, and says what went wrong, keeping the list", () => {
    const busy = notice({ busy: true });
    expect([...busy.matchAll(/<button([^>]*)>/g)].every((m) => m[1].includes("disabled"))).toBe(true);
    const failed = notice({ error: "the queue is locked" });
    expect(failed).toContain('<p class="err small" role="alert">the queue is locked</p>');
    expect(text(failed)).toContain("dr-22 · Pets");
  });
});
