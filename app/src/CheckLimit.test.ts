import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import appSource from "./App.tsx?raw";
import css from "./styles.css?raw";
import projectsSource from "./Projects.tsx?raw";
import projectListSource from "./ProjectList.tsx?raw";
import { CheckCount, ColumnLine, LimitCell, OverLine, SideLimit } from "./CheckLimit";
import { GroupedBoard } from "./Groups";
import { groupItems, isWork } from "./levels";
import { Mode, type DisplayMode } from "./modes";
import { ColumnHead } from "./ModeViews";
import { ProjectItem } from "./ProjectList";
import type { QueueInfo, Task } from "./types";
import { overLimit } from "./yourCheckLimit";

const q = (name: string, yourCheck: number, yourCheckLimit: number | null): QueueInfo => ({ name, path: `/p/${name}/tasks`, counts: {}, yourCheck, yourCheckLimit });
const html = (el: Parameters<typeof renderToStaticMarkup>[0]) => renderToStaticMarkup(el);
const text = (h: string) => h.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/\s+/g, " ").trim();
/** One CSS rule's body. */
const rule = (sel: string) => new RegExp(`(?:^|\\n)${sel.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")} \\{([^}]*)\\}`).exec(css)?.[1] ?? "";

describe("the Your check header over its limit (0050)", () => {
  it("one project over: N of L in orange, a short line in the column, the sentence above the board", () => {
    const over = overLimit([q("muster", 7, 5)], "muster");
    const count = html(createElement(CheckCount, { count: 7, over, queue: "muster" }));
    expect(count).toBe('<small class="over-limit" title="7 tasks wait in Your check, over your limit of 5 (Settings › Projects)">7 of 5</small>');
    expect(html(createElement(ColumnLine, { over, queue: "muster" }))).toBe('<p class="limit-line over-limit">Over your limit of 5</p>');
    expect(html(createElement(OverLine, { over, queue: "muster" }))).toBe('<p class="limit-board over-limit" role="status">Over your limit of 5: check these before more start.</p>');
  });

  it("within the limit: the plain count, and both lines' places kept empty so nothing jumps", () => {
    const over = overLimit([q("muster", 5, 5)], "muster");
    expect(html(createElement(CheckCount, { count: 5, over, queue: "muster" }))).toBe("<small>5</small>");
    expect(html(createElement(ColumnLine, { over, queue: "muster" }))).toBe('<p class="limit-line"></p>');
    expect(html(createElement(OverLine, { over, queue: "muster" }))).toBe('<p class="limit-board" role="status"></p>');
    // Off: never over, however many wait.
    const off = overLimit([q("muster", 30, null)], "muster");
    expect(html(createElement(CheckCount, { count: 30, over: off, queue: "muster" }))).toBe("<small>30</small>");
  });

  it("All projects: each project on its own; the column says how many, the line above the board names them", () => {
    const over = overLimit([q("muster", 7, 5), q("drover", 2, 5), q("germ", 4, 3)], "all");
    const count = html(createElement(CheckCount, { count: 13, over, queue: "all" }));
    expect(count).toContain('class="over-limit"');
    expect(text(count)).toBe("13");
    expect(text(html(createElement(ColumnLine, { over, queue: "all" })))).toBe("2 projects over limit");
    expect(text(html(createElement(OverLine, { over, queue: "all" })))).toBe("Over your limit in muster (7 of 5) and germ (4 of 3): check these before more start.");
  });

  it("the places are reserved: two lines in the column, one above the board, in the needs-you text colour", () => {
    expect(rule(".limit-line")).toContain("min-height: calc(2 * 1.35em)");
    expect(rule(".limit-board")).toContain("min-height: 1.35em");
    expect(rule(".over-limit")).toContain("color: var(--needs-text)");
  });

  it("the board: the line above it always (plain or grouped), the count and the short line in Your check, kept open while over", () => {
    expect(appSource).toContain('{view === "board" && <OverLine over={overCheck} queue={queue} />}');
    expect(appSource).toContain("countNode={c.id === \"your-check\" ? <CheckCount count={count} over={overCheck} queue={queue} /> : undefined}");
    expect(appSource).toContain('{c.id === "your-check" && <ColumnLine over={overCheck} queue={queue} />}');
    expect(appSource).toContain('(s.id === "your-check" && (boardResult || overCheck.length > 0))');
    expect(appSource).toContain("limit={{ over: overCheck, queue }}");
  });

  it("with Group by, each band's Your check header says N of L too", () => {
    const t = (id: string): Task => ({
      queue: "muster", id, title: "T", kind: "task", priority: "normal", status: "doing", stage: "your-check", blocked: false, stageHistory: [], folder: "doing", path: `doing/${id}-t.md`,
      claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [],
      questions: [], openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: true, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0,
      claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: "your-check", search: "",
    });
    const items = [t("0001"), t("0002")];
    const band = (limit?: { over: ReturnType<typeof overLimit>; queue: string }) =>
      html(createElement(GroupedBoard, { groups: groupItems(items.filter(isWork), items, "feature"), by: "feature", sort: "priority", multi: false, openKey: null, onOpen: () => {}, renderCard: () => null, limit }));
    const head = (h: string) => /<section class="column col-your-check"><h2[^>]*>(.*?)<\/h2>/.exec(h)?.[1] ?? "";
    expect(head(band({ over: overLimit([q("muster", 7, 5)], "muster"), queue: "muster" }))).toContain('<small class="over-limit" title="7 tasks wait in Your check, over your limit of 5 (Settings › Projects)">7 of 5</small>');
    expect(head(band({ over: [], queue: "muster" }))).toContain("<small>2</small>");
    expect(head(band())).toContain("<small>2</small>");
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

  it("is a column of the projects table, kept by the project's folder (its key), applied live", () => {
    expect(projectsSource).toContain("<th>Your check</th>");
    expect(projectsSource).toMatch(/<LimitCell key=\{`\$\{p\.key\}-\$\{q\.yourCheckLimit\}`\} q=\{q\} onSet=\{\(v\) => void act\(\(\) => api\.setYourCheckLimit\(p\.key, v\)\)\} \/>/);
  });
});
