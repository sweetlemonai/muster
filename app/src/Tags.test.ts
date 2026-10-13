import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import TagEditor, { MAX_TAGS, SUGGESTED_TAGS, cleanTag, tagSuggestions } from "./Tags";

/** The editor as first drawn: the chips it shows, the completions it offers, and its input. */
function editor(tags: string[]) {
  const html = renderToStaticMarkup(createElement(TagEditor, { tags, onChange: () => {} }));
  const chips = [...html.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1]);
  const options = [...html.matchAll(/<option value="([^"]*)"/g)].map((m) => m[1]);
  const input = /<input[^>]*>/.exec(html)?.[0] ?? "";
  return { chips, options, input };
}

describe("tag editor", () => {
  it("shows only the task's own tags, not every suggestion", () => {
    expect(editor([]).chips).toEqual([]);
    expect(editor(["code", "my-own"]).chips).toEqual(["#code ×", "#my-own ×"]);
  });
  it("offers the suggestions the task doesn't have yet as completions", () => {
    expect(editor([]).options).toEqual(SUGGESTED_TAGS);
    const { options } = editor(["code", "perf"]);
    expect(options).not.toContain("code");
    expect(options).not.toContain("perf");
    expect(options).toHaveLength(SUGGESTED_TAGS.length - 2);
  });
  it("ties the input to its completions", () => {
    const html = renderToStaticMarkup(createElement(TagEditor, { tags: [], onChange: () => {} }));
    const list = /<input[^>]*list="([^"]+)"/.exec(html)?.[1];
    expect(list).toBeTruthy();
    expect(html).toContain(`<datalist id="${list}">`);
  });
  it("names each chip as a remove action", () => {
    const html = renderToStaticMarkup(createElement(TagEditor, { tags: ["docs"], onChange: () => {} }));
    expect(html).toContain('aria-label="Remove tag docs"');
  });
  it("stops taking tags at the cap", () => {
    const full = Array.from({ length: MAX_TAGS }, (_, i) => `t${i}`);
    expect(editor(full).input).toContain("disabled");
    expect(editor(full).input).toContain(`${MAX_TAGS} tags max`);
    expect(editor(full.slice(1)).input).not.toContain("disabled");
  });
});

describe("tagSuggestions", () => {
  it("keeps the suggested order and drops tags already on the task", () => {
    expect(tagSuggestions(["spike", "unknown"])).toEqual(SUGGESTED_TAGS.filter((t) => t !== "spike"));
    expect(tagSuggestions(SUGGESTED_TAGS)).toEqual([]);
  });
});

describe("cleanTag", () => {
  it("lowercases, joins words with dashes and drops other characters", () => {
    expect(cleanTag("  Tech Debt! ")).toBe("tech-debt");
    expect(cleanTag("--a_b--")).toBe("a-b");
    expect(cleanTag("#")).toBe("");
    expect(cleanTag("x".repeat(40))).toHaveLength(24);
  });
});
