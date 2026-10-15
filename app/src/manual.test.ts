import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import helpRs from "../src-tauri/src/help.rs?raw";
import commandsFile from "../../docs/manual/05-commands.md?raw";
import { ManualView } from "./Help";
import { HELP_TOPICS } from "./helpTopics";
import { headings, linksIn, manualPages, neighbours, PAGES, resolve, search, slugify } from "./manual";
import Markdown from "./Markdown";
import { SHORTCUTS } from "./shortcuts";

// Every Markdown file in the repo's docs and commands, by its path from the repo's root, so links
// out of the manual can be checked too.
const repoDocs = Object.fromEntries(
  Object.entries(import.meta.glob<string>(["../../docs/**/*.md", "../../commands/*.md", "../../README.md"], { query: "?raw", import: "default", eager: true }))
    .map(([path, text]) => [path.replace(/^\.\.\/\.\.\//, ""), text]),
);

// The manual as the app shows it once the core's help has loaded: the Commands page is the text
// the core generates, which a Rust test keeps equal to docs/manual/05-commands.md.
const pages = manualPages(commandsFile);
const page = (file: string) => pages.find((p) => p.file === file)!;

/** The commands help.rs documents: each `name: "…"` in its COMMANDS list. */
const helpCommands = [...helpRs.slice(helpRs.indexOf("pub const COMMANDS"), helpRs.indexOf("pub const COMMON")).matchAll(/^\s+name: "([a-z-]+)",$/gm)].map((m) => m[1]);

const view = (over: Partial<Parameters<typeof ManualView>[0]> = {}) =>
  renderToStaticMarkup(
    createElement(ManualView, { pages, at: { file: "02-concepts.md", slug: "", n: 0 }, query: "", onQuery: () => {}, onGo: () => {}, ...over }),
  );

describe("the manual's pages", () => {
  it("are the eight pages, in order, each starting with its title", () => {
    expect(PAGES.map((p) => p.title)).toEqual(["Start here", "Concepts", "Tutorials", "In the app", "Commands", "Shortcuts", "For agents", "FAQ and troubleshooting"]);
    for (const p of pages) {
      expect(p.text, p.file).not.toBeNull();
      expect(p.text!.split("\n")[0], p.file).toBe(`# ${p.title}`);
      expect(p.sections.length, `${p.file} has sections`).toBeGreaterThan(1);
    }
  });

  it("keep the Commands page empty until the core's help has loaded", () => {
    const loading = manualPages(null);
    expect(loading.find((p) => p.file === "05-commands.md")).toMatchObject({ title: "Commands", text: null, sections: [] });
    expect(loading.filter((p) => p.text !== null)).toHaveLength(7);
  });

  it("have no link to a page, a section or a file that doesn't exist", () => {
    const broken: string[] = [];
    for (const p of pages) {
      for (const href of linksIn(p.text!)) {
        const t = resolve(p.file, href);
        if (t.kind === "bad") broken.push(`${p.file}: ${href}`);
        else if (t.kind === "page") {
          const to = page(t.file);
          if (!to) broken.push(`${p.file}: ${href} (no such page)`);
          else if (t.slug && !to.sections.concat(headings(to.text!)).some((h) => h.slug === t.slug)) broken.push(`${p.file}: ${href} (no such section)`);
        } else if (t.kind === "repo") {
          const text = repoDocs[t.path];
          if (text === undefined) broken.push(`${p.file}: ${href} (no file ${t.path})`);
          else if (t.slug && !headings(text).some((h) => h.slug === t.slug)) broken.push(`${p.file}: ${href} (no such section)`);
        }
      }
    }
    expect(broken).toEqual([]);
  });

  it("link to each other: every page is linked from another one", () => {
    for (const p of PAGES.slice(1)) {
      const from = pages.filter((q) => q.file !== p.file && linksIn(q.text!).some((h) => { const t = resolve(q.file, h); return t.kind === "page" && t.file === p.file; }));
      expect(from.length, p.file).toBeGreaterThan(0);
    }
  });
});

describe("links", () => {
  it("resolve to a page and section, a repo file, the web, or nothing", () => {
    expect(resolve("01-start-here.md", "02-concepts.md#stages-and-roles")).toEqual({ kind: "page", file: "02-concepts.md", slug: "stages-and-roles" });
    expect(resolve("02-concepts.md", "#blocked")).toEqual({ kind: "page", file: "02-concepts.md", slug: "blocked" });
    expect(resolve("02-concepts.md", "./03-tutorials.md")).toEqual({ kind: "page", file: "03-tutorials.md", slug: "" });
    expect(resolve("07-for-agents.md", "../PROTOCOL.md#roles")).toEqual({ kind: "repo", path: "docs/PROTOCOL.md", slug: "roles" });
    expect(resolve("02-concepts.md", "../decisions/0003-muster-in-the-repo-backlog-and-stages.md")).toEqual({ kind: "repo", path: "docs/decisions/0003-muster-in-the-repo-backlog-and-stages.md", slug: "" });
    expect(resolve("08-faq.md", "https://example.com/x")).toEqual({ kind: "web", url: "https://example.com/x" });
    for (const bad of ["http://example.com", "javascript:alert(1)", "/etc/passwd", "../../../outside.md", "", "mailto:a@b.c"]) {
      expect(resolve("01-start-here.md", bad).kind, bad).toBe("bad");
    }
  });
  it("make GitHub's slugs", () => {
    expect(slugify("Stages and roles")).toBe("stages-and-roles");
    expect(slugify("Claims, leases and check-ins")).toBe("claims-leases-and-check-ins");
    expect(slugify("muster new")).toBe("muster-new");
    expect(slugify("Agents doing the work (coding sessions)")).toBe("agents-doing-the-work-coding-sessions");
    expect(slugify("Your check: try it, tick, accept")).toBe("your-check-try-it-tick-accept");
    expect(slugify("What's a queue?")).toBe("whats-a-queue");
  });
  it("number a repeated heading's slug as GitHub does", () => {
    expect(headings("# A\n\n## Examples\n\n## Examples\n\n```\n## not a heading\n```\n").map((h) => h.slug)).toEqual(["a", "examples", "examples-1"]);
  });
});

describe("the Commands page", () => {
  it("has every command in help.rs, each under its own heading", () => {
    expect(helpCommands.length).toBeGreaterThan(30);
    const slugs = headings(page("05-commands.md").text!).map((h) => h.slug);
    const missing = helpCommands.filter((c) => !slugs.includes(`muster-${c}`));
    expect(missing).toEqual([]);
  });
  it("is grouped as the spec says", () => {
    expect(page("05-commands.md").sections.map((s) => s.title)).toEqual([
      "Tasks", "Stages", "Claims", "Review", "Team", "Questions", "Backlog", "Levels", "Projects and the queue", "Accounts", "On every command", "Inside a Claude Code session",
    ]);
  });
});

describe("the Shortcuts page", () => {
  it("has every shortcut in the table, with its keys", () => {
    const text = page("06-shortcuts.md").text!;
    for (const s of SHORTCUTS) {
      expect(text, s.id).toContain(s.keys);
      expect(text, s.id).toContain(s.what);
    }
  });
});

describe("search", () => {
  it("finds a word on a page, and says which section", () => {
    const hits = search(pages, "lease");
    expect(hits.some((h) => h.file === "02-concepts.md")).toBe(true);
    const hit = hits.find((h) => h.file === "02-concepts.md")!;
    expect(hit.snippet.toLowerCase()).toContain("lease");
    expect(page("02-concepts.md").sections.concat(headings(page("02-concepts.md").text!)).some((s) => s.slug === hit.slug)).toBe(true);
  });
  it("needs every word, in any case, across all pages, the generated ones too", () => {
    expect(search(pages, "HEARTBEAT reclaim").length).toBeGreaterThan(0);
    expect(search(pages, "⌘N").some((h) => h.file === "06-shortcuts.md")).toBe(true);
    expect(search(pages, "muster checkin").some((h) => h.file === "05-commands.md")).toBe(true);
    expect(search(pages, "zebra-quokka-nothing")).toEqual([]);
    expect(search(pages, "   ")).toEqual([]);
  });
  it("keeps the list short", () => {
    expect(search(pages, "the", 12)).toHaveLength(12);
  });
});

describe("the Help tab", () => {
  it("lists every page and its sections in the sidebar, the current one highlighted", () => {
    const html = view();
    for (const p of pages) {
      expect(html, p.file).toContain(`>${p.title}</button>`);
      for (const s of p.sections) expect(html, `${p.file} ${s.title}`).toContain(`data-section="${p.file}#${s.slug}"`);
    }
    expect(html).toMatch(/<button[^>]*aria-current="page"[^>]*>Concepts<\/button>/);
  });
  it("has ◀ ▶ to the previous and next page, and none past the ends", () => {
    expect(neighbours("02-concepts.md")).toEqual({ prev: PAGES[0], next: PAGES[2] });
    expect(neighbours("01-start-here.md").prev).toBeNull();
    expect(neighbours("08-faq.md").next).toBeNull();
    const html = view();
    expect(html).toContain("◀ Start here");
    expect(html).toContain("Tutorials ▶");
  });
  it("renders the page's headings as anchors for the sidebar and links", () => {
    const html = view();
    for (const s of page("02-concepts.md").sections) expect(html).toContain(`data-anchor="${s.slug}"`);
  });
  it("shows search results beside the pages, which stay listed", () => {
    const html = view({ query: "lease" });
    expect(html).toContain('class="manual-hits"');
    expect(html).toContain("Concepts ›");
    expect(html).toContain(">Start here</button>");
    expect(view({ query: "zebra-quokka-nothing" })).toContain("Nothing found");
  });
  it("says the Commands page is loading, not empty, before the core answers", () => {
    const html = view({ pages: manualPages(null), at: { file: "05-commands.md", slug: "", n: 0 } });
    expect(html).toContain("Loading the commands…");
  });
});

describe("context help", () => {
  it("opens a section that exists, for every pane and view", () => {
    for (const [topic, at] of Object.entries(HELP_TOPICS)) {
      const p = page(at.file);
      expect(p, topic).toBeDefined();
      expect(headings(p.text!).some((h) => h.slug === at.slug), `${topic}: ${at.file}#${at.slug}`).toBe(true);
    }
  });
});

describe("Markdown for the manual", () => {
  it("anchors headings only when asked, and keeps HTML as text", () => {
    const md = "# Title\n\n## Two words\n\n<script>x</script>";
    const plain = renderToStaticMarkup(createElement(Markdown, { text: md }));
    const anchored = renderToStaticMarkup(createElement(Markdown, { text: md, anchors: true }));
    expect(plain).not.toContain("data-anchor");
    expect(anchored).toContain('<h2 data-anchor="two-words">Two words</h2>');
    expect(anchored).not.toContain("<script>");
  });
  it("doesn't turn a file name into a web link", () => {
    expect(renderToStaticMarkup(createElement(Markdown, { text: "See INDEX.md and README.md.", anchors: true }))).not.toContain("<a ");
  });
});
