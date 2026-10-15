// Settings › Projects' Run line (0077): each project's run command under its gate, read-only,
// with Run beside it when there is one (0079).
import { isValidElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { fixDone, GateRow, type HistoryFix, HistoryCell, historyLine, newestOnly, ProjectGateRow, RunCell, runFix } from "./Projects";
import { Button } from "./ui";
import type { HistoryState, Project } from "./types";

const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&quot;/g, '"').replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/\s+/g, " ").trim();
const row = (gate: string, run: string, name = "muster") =>
  renderToStaticMarkup(
    <table>
      <tbody>
        <GateRow name={name} gate={gate} run={run} hidden={false} />
      </tbody>
    </table>,
  );

describe("the Run line", () => {
  it("shows the run command under the gate, with its label, in the gate line's look, and Run beside it", () => {
    const html = row("bin/gate", "bin/run --fresh");
    expect(text(html)).toBe("Gate bin/gate Run bin/run --fresh Run");
    // Its own line after the gate's, both in the one row that spans the table.
    expect(html).toContain('<div><b class="gate-label">Gate</b> <code class="path">bin/gate</code></div><div><b class="gate-label">Run</b> <code class="path ltr">bin/run --fresh</code> <span class="run-controls">');
    // The button names the project (0079).
    expect(html).toMatch(/<button[^>]*aria-label="Run muster"[^>]*>Run<\/button>/);
  });

  it("says how to set one when there's none, beside the gate's own hint", () => {
    expect(text(row("", ""))).toBe('Gate none: merges wait for one. muster projects set muster gate "<command>" Run none: say what starts it, to try it. muster projects set muster run "<command>"');
    expect(text(row("bin/gate", ""))).toBe('Gate bin/gate Run none: say what starts it, to try it. muster projects set muster run "<command>"');
  });

  it("is filled from the project the core listed: its own run command, or the hint", () => {
    const p: Project = { key: "/q", name: "muster", path: "/q", source: "found", group: "", hidden: false, ok: true, gate: "bin/gate", run: "bin/run" };
    const of = (q: Project) => text(renderToStaticMarkup(<table><tbody><ProjectGateRow p={q} /></tbody></table>));
    expect(of(p)).toBe("Gate bin/gate Run bin/run Run");
    expect(of({ ...p, run: undefined })).toBe('Gate bin/gate Run none: say what starts it, to try it. muster projects set muster run "<command>"');
    expect(renderToStaticMarkup(<table><tbody><ProjectGateRow p={{ ...p, hidden: true }} /></tbody></table>)).toContain('<tr class="gate-row dim">');
  });

  it("keeps the command left to right, so right-to-left text around it can't reorder it", () => {
    expect(renderToStaticMarkup(<RunCell name="muster" run="swift run --product نگار" />)).toBe('<code class="path ltr">swift run --product نگار</code>');
    expect(renderToStaticMarkup(<RunCell name="negar" run="" />)).toContain('<code class="ltr">muster projects set negar run &quot;&lt;command&gt;&quot;</code>');
  });
});

// Settings › Projects' History line (0109): a local queue's own history, quietly beside the gate.
describe("the History line", () => {
  const now = 1_760_000_000;
  const h: HistoryState = { kept: true, started: true, lastCommit: now - 120, pending: 0, pendingError: "", missing: [], changed: [] };
  const line = (x: HistoryState) =>
    text(renderToStaticMarkup(<table><tbody><GateRow name="muster" gate="bin/gate" run="" hidden={false} history={x} now={now} /></tbody></table>));

  it("says when the history last committed, in the muted words, after the gate and run lines", () => {
    expect(historyLine(h, now)).toEqual({ text: "committed 2 min ago", command: "", warn: false, fixes: [] });
    expect(line(h)).toMatch(/Run none: .* History committed 2 min ago$/);
    const html = renderToStaticMarkup(<table><tbody><GateRow name="m" gate="g" run="" hidden={false} history={h} now={now} /></tbody></table>);
    expect(html).toContain('<div><b class="gate-label">History</b> <span class="meta">committed 2 min ago</span></div>');
    expect(historyLine({ ...h, lastCommit: now }, now).text).toBe("committed now");
  });

  it("puts writes not committed and missing files first, as warnings in words, with the command", () => {
    const pending = historyLine({ ...h, pending: 2, pendingError: "index.lock exists", missing: ["a"] }, now);
    expect(pending).toEqual({ text: "warning: 2 writes not committed (index.lock exists)", command: "muster history commit", warn: true, fixes: ["commit"] });
    expect(historyLine({ ...h, pending: 1 }, now).text).toBe("warning: 1 write not committed");
    expect(historyLine({ ...h, missing: ["tasks/0001-a.md"] }, now)).toEqual({ text: "warning: 1 file missing since the last commit", command: "muster restore --all", warn: true, fixes: ["restore", "removed"] });
    const html = renderToStaticMarkup(<table><tbody><GateRow name="m" gate="g" run="" hidden={false} history={{ ...h, missing: ["a", "b"] }} now={now} /></tbody></table>);
    expect(html).toContain('<span class="history-warn">warning: 2 files missing since the last commit: <code>muster restore --all</code></span>');
  });

  it("names files changed outside muster quietly, and a history not started yet", () => {
    expect(historyLine({ ...h, changed: ["a", "b", "c"] }, now)).toEqual({ text: "committed 2 min ago; 3 files changed outside muster since", command: "muster history commit", warn: false, fixes: ["commit"] });
    expect(historyLine({ ...h, started: false, lastCommit: null }, now)).toEqual({ text: "not started yet: the queue's next write starts it", command: "", warn: false, fixes: [] });
  });

  it("offers the fix in one click, named for the project, only where there's one and the window can run it", () => {
    const missing = { ...h, missing: ["a"] };
    const none = () => undefined;
    const html = (x: HistoryState, onFix?: (f: HistoryFix) => void) =>
      renderToStaticMarkup(<table><tbody><GateRow name="muster" gate="g" run="" hidden={false} history={x} now={now} onFix={onFix} /></tbody></table>);
    expect(html(missing, none)).toMatch(/<button[^>]*aria-label="Restore muster&#x27;s queue files"[^>]*>Restore<\/button> <button[^>]*aria-label="Record muster&#x27;s missing queue files as deleted"[^>]*>Keep deleted<\/button>/);
    expect(html({ ...h, pending: 1 }, none)).toMatch(/<button[^>]*aria-label="Commit muster&#x27;s queue files"[^>]*>Commit<\/button>/);
    expect(html({ ...h, changed: ["a"] }, none)).toContain(">Commit</button>");
    expect(html(h, none)).not.toContain("<button");
    expect(html(missing)).not.toContain("<button");
  });

  it("runs the fix each button names", () => {
    // Each button's click, read from the cell's elements (no DOM here): what it hands to onFix.
    const clicks = (x: HistoryState) => {
      const got: HistoryFix[] = [];
      const buttons: { label: string; click: () => void }[] = [];
      const walk = (n: unknown): void => {
        if (Array.isArray(n)) return n.forEach(walk);
        if (!isValidElement(n)) return;
        const props = n.props as { children?: unknown; onClick?: () => void };
        if (n.type === Button && props.onClick) buttons.push({ label: String(props.children), click: props.onClick });
        walk(props.children);
      };
      walk(HistoryCell({ h: x, now, name: "muster", onFix: (f) => got.push(f) }));
      return buttons.map((b) => {
        b.click();
        return `${b.label}→${got.at(-1)}`;
      });
    };
    expect(clicks({ ...h, missing: ["a"] })).toEqual(["Restore→restore", "Keep deleted→removed"]);
    expect(clicks({ ...h, pending: 1 })).toEqual(["Commit→commit"]);
    expect(clicks({ ...h, changed: ["a"] })).toEqual(["Commit→commit"]);

    // And each fix calls its own core command, for the project's key.
    const called: string[] = [];
    const fake = {
      restoreHistory: (k: string) => (called.push(`restore ${k}`), Promise.resolve(1)),
      commitHistory: (k: string) => (called.push(`commit ${k}`), Promise.resolve(2)),
      recordRemoved: (k: string) => (called.push(`removed ${k}`), Promise.resolve(3)),
    };
    void runFix("restore", "/q", fake);
    void runFix("commit", "/q", fake);
    void runFix("removed", "/q", fake);
    expect(called).toEqual(["restore /q", "commit /q", "removed /q"]);
  });

  it("says what the fix did, from how many files it did", () => {
    expect(fixDone("restore", "Muster", 0)).toBe("Muster: nothing was missing; its files match its history.");
    expect(fixDone("restore", "Muster", 1)).toBe("Muster: 1 file restored from its history.");
    expect(fixDone("restore", "Muster", 3)).toBe("Muster: 3 files restored from its history.");
    expect(fixDone("commit", "Muster", 0)).toBe("Muster: nothing to commit; its history has every file.");
    expect(fixDone("commit", "Muster", 2)).toBe("Muster: 2 files committed to its history.");
    expect(fixDone("removed", "Muster", 0)).toBe("Muster: nothing was missing.");
    expect(fixDone("removed", "Muster", 1)).toBe("Muster: 1 file recorded as deleted; older commits keep them (muster restore <path>).");
  });

  it("keeps the newest reply when replies come out of order", () => {
    const r = newestOnly();
    const first = r.ask();
    const second = r.ask();
    expect(r.take(second)).toBe(true);
    expect(r.take(first)).toBe(false);
    const third = r.ask();
    expect(r.take(third)).toBe(true);
    expect(r.take(third)).toBe(false);
  });

  it("isn't there for a queue that keeps none", () => {
    expect(line({ ...h, kept: false })).not.toContain("History");
    const p: Project = { key: "/q", name: "muster", path: "/q", source: "found", group: "", hidden: false, ok: true, gate: "g", run: "" };
    expect(text(renderToStaticMarkup(<table><tbody><ProjectGateRow p={p} /></tbody></table>))).not.toContain("History");
    expect(text(renderToStaticMarkup(<table><tbody><ProjectGateRow p={p} history={h} /></tbody></table>))).toContain("History committed");
  });
});
