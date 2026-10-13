import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import appSource from "./App.tsx?raw";
import projectsSource from "./Projects.tsx?raw";
import projectListSource from "./ProjectList.tsx?raw";
import { CheckCount, LimitCell, OverLine, SideLimit } from "./CheckLimit";
import { Mode, type DisplayMode } from "./modes";
import { ColumnHead } from "./ModeViews";
import { ProjectItem } from "./ProjectList";
import type { QueueInfo } from "./types";
import { overLimit } from "./yourCheckLimit";

const q = (name: string, yourCheck: number, yourCheckLimit: number | null): QueueInfo => ({ name, path: `/p/${name}/tasks`, counts: {}, yourCheck, yourCheckLimit });
const html = (el: Parameters<typeof renderToStaticMarkup>[0]) => renderToStaticMarkup(el);
const text = (h: string) => h.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/\s+/g, " ").trim();

describe("the Your check header over its limit (0050)", () => {
  it("one project over: N of L in orange, and the line under the header", () => {
    const over = overLimit([q("muster", 7, 5)], "muster");
    const count = html(createElement(CheckCount, { count: 7, over, queue: "muster" }));
    expect(count).toBe('<small class="over-limit" title="7 tasks wait in Your check, over your limit of 5 (Settings › Projects)">7 of 5</small>');
    const line = html(createElement(OverLine, { over, queue: "muster" }));
    expect(line).toBe('<p class="limit-line over-limit" role="status">Over your limit of 5: check these before more start.</p>');
  });

  it("within the limit: the plain count, and the line's place kept empty so nothing jumps", () => {
    const over = overLimit([q("muster", 5, 5)], "muster");
    expect(html(createElement(CheckCount, { count: 5, over, queue: "muster" }))).toBe("<small>5</small>");
    expect(html(createElement(OverLine, { over, queue: "muster" }))).toBe('<p class="limit-line" role="status"></p>');
    // Off: never over, however many wait.
    const off = overLimit([q("muster", 30, null)], "muster");
    expect(html(createElement(CheckCount, { count: 30, over: off, queue: "muster" }))).toBe("<small>30</small>");
  });

  it("All projects: each project on its own, the line names the ones over", () => {
    const over = overLimit([q("muster", 7, 5), q("drover", 2, 5), q("germ", 4, 3)], "all");
    const count = html(createElement(CheckCount, { count: 13, over, queue: "all" }));
    expect(count).toContain('class="over-limit"');
    expect(text(count)).toBe("13");
    expect(text(html(createElement(OverLine, { over, queue: "all" })))).toBe("Over your limit in muster (7 of 5) and germ (4 of 3): check these before more start.");
  });

  it("the board puts both in the Your check column, keeps it open while over, and says it above Group by's bands", () => {
    expect(appSource).toContain("countNode={c.id === \"your-check\" ? <CheckCount count={count} over={overCheck} queue={queue} /> : undefined}");
    expect(appSource).toMatch(/c\.id === "your-check" && <OverLine over=\{overCheck\} queue=\{queue\} \/>/);
    expect(appSource).toContain('(s.id === "your-check" && (boardResult || overCheck.length > 0))');
    expect(appSource).toMatch(/groupBy !== "none" && overCheck\.length > 0 && <OverLine over=\{overCheck\} queue=\{queue\} \/>/);
  });
});

describe("the limit in every display mode (0050 with 0038)", () => {
  const MODES: DisplayMode[] = ["adhd", "standard", "focus"];
  it("the sidebar row says it in every mode, the narrow Focus sidebar too", () => {
    for (const m of MODES) {
      const h = html(createElement(Mode.Provider, { value: m }, createElement(ProjectItem, { name: "muster", total: 9, tasks: [], on: false, onClick: () => {}, alert: createElement(SideLimit, { q: q("muster", 7, 5) }), children: createElement("i", null, "activity") })));
      expect(h, m).toContain('<span class="side-limit over-limit">7 in Your check, limit 5</span>');
    }
  });
  it("a column's head says N of L open, fixed or folded to its strip", () => {
    const node = createElement(CheckCount, { count: 7, over: overLimit([q("muster", 7, 5)], "muster"), queue: "muster" });
    for (const fold of ["fixed", "opened", "folded"] as const) {
      const h = html(createElement(ColumnHead, { stage: { id: "your-check", label: "Your check", role: "owner" }, count: 7, countNode: node, needs: 7, blocked: 0, fold, onFold: () => {} }));
      expect(h, fold).toContain(">7 of 5</small>");
    }
    // Without one, the plain count as before.
    expect(html(createElement(ColumnHead, { stage: { id: "ready", label: "Ready", role: "" }, count: 3, needs: 0, blocked: 0, fold: "fixed", onFold: () => {} }))).toContain("Ready <small>3</small>");
  });
});

describe("the sidebar row over its limit (0050)", () => {
  it("says how many wait and the limit, in orange; nothing within it", () => {
    expect(html(createElement(SideLimit, { q: q("muster", 7, 5) }))).toBe('<span class="side-limit over-limit">7 in Your check, limit 5</span>');
    expect(html(createElement(SideLimit, { q: q("muster", 5, 5) }))).toBe("");
    expect(html(createElement(SideLimit, { q: q("muster", 9, null) }))).toBe("");
    // In every mode: the row's alert, which the narrow sidebar (Focus) keeps.
    expect(projectListSource).toContain("alert={<SideLimit q={queues.find((q) => q.name === r.name)} />}");
    expect(projectListSource).toMatch(/\{full && children\}\n\s*\{alert\}/);
  });
});

describe("Settings › Projects' Your check limit (0050)", () => {
  it("shows each limit with its count, the Off chip and a number field", () => {
    const h = html(createElement(LimitCell, { q: q("muster", 7, 5), onSet: () => {} }));
    expect(h).toContain('aria-label="Your check limit for muster"');
    expect(h).toContain('<span class="over-limit">limit 5 · 7 waiting</span>');
    expect(h).toMatch(/<button type="button" class="chip" aria-pressed="false">Off<\/button>/);
    expect(h).toMatch(/<input type="number" min="1" max="50" aria-label="Limit for muster, 1 to 50" value="5"\/>/);
    // Nothing to set until the number changes.
    expect(h).toMatch(/<button type="submit" disabled="">Set<\/button>/);
  });

  it("within the limit the count is quiet; off, the chip is pressed and the field offers the default", () => {
    expect(html(createElement(LimitCell, { q: q("muster", 2, 5), onSet: () => {} }))).toContain('<span class="meta">limit 5 · 2 waiting</span>');
    const off = html(createElement(LimitCell, { q: q("muster", 9, null), onSet: () => {} }));
    expect(off).toContain('<span class="meta">off · 9 waiting</span>');
    expect(off).toMatch(/class="chip on" aria-pressed="true">Off</);
    expect(off).toContain('value="5"');
    expect(off).toMatch(/<button type="submit">Set<\/button>/);
  });

  it("is a column of the projects table, applied live: saved by the core, then the board read again", () => {
    expect(projectsSource).toContain("<th>Your check</th>");
    expect(projectsSource).toMatch(/<LimitCell key=\{`\$\{p\.key\}-\$\{q\.yourCheckLimit\}`\} q=\{q\} onSet=\{\(v\) => void act\(\(\) => api\.setYourCheckLimit\(q\.name, v\)\)\} \/>/);
  });
});
