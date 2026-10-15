// Every block of rendered task text reads in its own direction (mu-148): the renderer puts
// dir="auto" on each paragraph, heading, list item, quote and table cell, so a Persian paragraph
// reads right to left inside an English body and the other way round; code stays left to right.
// The attribute comes from the renderer only: the text can't set it, and its HTML stays text.
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import Markdown from "./Markdown";

const html = (text: string, anchors = false) => renderToStaticMarkup(createElement(Markdown, { text, anchors }));

const MIXED = [
  "این تسک جستجو را سریع‌تر می‌کند.",
  "",
  "This paragraph is English.",
  "",
  "- یک مورد فارسی",
  "- An English item",
  "",
].join("\n");

describe("block direction", () => {
  for (const anchors of [false, true]) {
    const which = anchors ? "the manual's renderer" : "the task renderer";
    it(`${which}: a Persian paragraph, an English one and a mixed list each get dir="auto" (AC3)`, () => {
      const out = html(MIXED, anchors);
      expect(out).toContain('<p dir="auto">این تسک جستجو را سریع‌تر می‌کند.</p>');
      expect(out).toContain('<p dir="auto">This paragraph is English.</p>');
      expect(out).toContain('<li dir="auto">یک مورد فارسی</li>');
      expect(out).toContain('<li dir="auto">An English item</li>');
    });

    it(`${which}: headings, quotes and table cells too; code blocks stay left to right`, () => {
      const out = html("## عنوان\n\n> نقل قول\n\n| a | ب |\n| --- | --- |\n| x | ی |\n\n```\nmuster show mu-1\n```\n\n    indented code\n", anchors);
      expect(out).toMatch(/<h2[^>]* dir="auto"|<h2 dir="auto"/);
      expect(out).toContain('<blockquote dir="auto">');
      expect(out).toContain('<th dir="auto">a</th>');
      expect(out).toContain('<td dir="auto">ی</td>');
      expect(out).toContain('<pre dir="ltr"><code>muster show mu-1');
      expect(out).toContain('<pre dir="ltr"><code>indented code');
    });
  }

  it("keeps the manual's anchors beside the direction", () => {
    expect(html("## Two words\n", true)).toContain('<h2 data-anchor="two-words" dir="auto">Two words</h2>');
  });

  it("never takes a direction from the text: its HTML stays text, and nothing else sets dir", () => {
    const out = html('<p dir="rtl">x</p>\n\nhello {dir=rtl}\n\n<div dir="ltr">y</div>');
    expect(out).not.toContain('dir="rtl"');
    expect(out).toContain("&lt;p dir=&quot;rtl&quot;&gt;x&lt;/p&gt;");
    expect(out).toContain('<p dir="auto">hello {dir=rtl}</p>');
    expect(out.match(/ dir="/g)).toHaveLength(3);
  });
});
