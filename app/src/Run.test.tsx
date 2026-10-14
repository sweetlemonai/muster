// Run from the app (0079): what the window holds of a run (merged from `run_state` and the `run`
// events), and the Run line, the Run block, Settings' Run and the sidebar's chip in each state.
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import css from "./styles.css?raw";
import { ProjectItem } from "./ProjectList";
import { RunBlock, RunButtons, RunLine, RunStatusLive } from "./Run";
import { fmtRunTime, KEEP_CHARS, KEEP_LINES, keyed, merged, received, shortFolder, shownLines, SHOWN_LINES, statusWords, WAITING_EVENTS, type RunInbox } from "./runData";
import StageContent, { AcceptingBlock } from "./StageContent";
import type { RunSnap, RunView, Task } from "./types";

const snap = (over: Partial<RunSnap> = {}): RunSnap => ({
  project: "muster", key: "/q/muster/.muster", id: 1, command: "bin/run", folder: "/Users/someone/playground/muster/main",
  status: { kind: "running" }, since: 1000, ended: null, first: 0, lines: [], ...over,
});
const view = (over: Partial<RunView> = {}): RunView => ({
  project: "muster", key: "/q/muster/.muster", command: "bin/run", folder: "/Users/someone/playground/muster/main", refused: "", run: null, ...over,
});
const n = (from: number, count: number) => Array.from({ length: count }, (_, i) => `line ${from + i}`);
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/&quot;/g, '"').replace(/&amp;/g, "&").replace(/\s+/g, " ").trim();
const noop = () => {};

const line = (info: RunView | null, run: RunSnap | null = null, more: { busy?: boolean; error?: string } = {}) =>
  renderToStaticMarkup(<RunLine project="muster" info={info} run={run} busy={more.busy ?? false} error={more.error ?? ""} onRun={noop} />);
const block = (run: RunSnap, all = false, nowSecs = 1080) =>
  renderToStaticMarkup(<RunBlock project="muster" run={run} nowSecs={nowSecs} busy={false} all={all} onAll={noop} onRun={noop} onStop={noop} />);

describe("which run events a project's Run takes", () => {
  it("takes its own by the key run_state gave, never by the name, which may have passed on", () => {
    const own = snap({ project: "x", key: "/q/x", lines: ["mine"] });
    // The run started as x-2 and still sends that name; another project is x-2 now.
    const renamed = snap({ project: "x-2", key: "/q/x", first: 1, lines: ["still mine"] });
    const other = snap({ project: "x", key: "/q/other", lines: ["not mine"] });
    let inbox: RunInbox = { key: "/q/x", waiting: [] };
    expect(received(inbox, own).take).toBe(own);
    expect(received(inbox, renamed).take).toBe(renamed);
    expect(received(inbox, other).take).toBeNull();
    // The new x-2, keyed by its own folder, doesn't take the old run's events that carry its name.
    expect(received({ key: "/q/new-x-2", waiting: [] }, renamed).take).toBeNull();
    // Before run_state answers, events wait; its answer takes only the ones of its key, in order.
    inbox = { key: null, waiting: [] };
    for (const e of [own, other, renamed]) {
      const r = received(inbox, e);
      expect(r.take).toBeNull();
      inbox = r.inbox;
    }
    const k = keyed(inbox, "/q/x");
    expect(k.take).toEqual([own, renamed]);
    expect(k.inbox).toEqual({ key: "/q/x", waiting: [] });
    expect(merged(merged(null, k.take[0]), k.take[1]).lines).toEqual(["mine", "still mine"]);
  });

  it("keeps only the last events while it waits for the key", () => {
    let inbox: RunInbox = { key: null, waiting: [] };
    for (let i = 0; i < WAITING_EVENTS + 10; i++) inbox = received(inbox, snap({ first: i, lines: [`l${i}`] })).inbox;
    expect(inbox.waiting.length).toBe(WAITING_EVENTS);
    expect(inbox.waiting[0].first).toBe(10);
    expect(keyed(inbox, "/q/elsewhere").take).toEqual([]);
  });
});

describe("what the window holds of a run", () => {
  it("adds each event's new lines once, whatever the order run_state and the events came in", () => {
    let held = merged(null, snap({ lines: n(0, 3) }));
    // An event that overlaps what run_state gave, then one past it.
    held = merged(held, snap({ first: 2, lines: n(2, 3) }));
    held = merged(held, snap({ first: 5, lines: n(5, 1) }));
    expect(held.lines).toEqual(n(0, 6));
    expect(held.first).toBe(0);
    // run_state answered late with fewer lines than the events brought: nothing is lost or doubled.
    held = merged(held, snap({ first: 0, lines: n(0, 4) }));
    expect(held.lines).toEqual(n(0, 6));
    // The ring rolled past what's held (the window missed lines): it starts again from the push.
    held = merged(held, snap({ first: 50, lines: n(50, 2) }));
    expect([held.first, held.lines]).toEqual([50, n(50, 2)]);
    // An event landed before run_state answered: the answer's earlier lines go in front.
    const early = merged(merged(null, snap({ first: 500, lines: n(500, 20) })), snap({ first: 0, lines: n(0, 530) }));
    expect([early.first, early.lines]).toEqual([0, n(0, 530)]);
    const later = merged(merged(null, snap({ first: 500, lines: n(500, 20) })), snap({ first: 0, lines: n(0, 510) }));
    expect([later.first, later.lines]).toEqual([0, n(0, 520)]);
  });

  it("keeps an ended run ended, ignores an older run's late event, and takes a restart's run whole", () => {
    const stopped = merged(snap({ lines: n(0, 2) }), snap({ status: { kind: "stopped" }, ended: 1100, first: 2, lines: ["bye"] }));
    expect(stopped.status).toEqual({ kind: "stopped" });
    // A run_state read before the end, answered after it.
    const late = merged(stopped, snap({ lines: n(0, 2) }));
    expect([late.status, late.ended, late.lines]).toEqual([{ kind: "stopped" }, 1100, [...n(0, 2), "bye"]]);
    // Restart: the new run (id 2) replaces it; the old run's last event, sent after, changes nothing.
    const fresh = merged(late, snap({ id: 2, since: 1200, lines: ["building"] }));
    expect([fresh.id, fresh.lines, fresh.status]).toEqual([2, ["building"], { kind: "running" }]);
    expect(merged(fresh, snap({ id: 1, status: { kind: "stopped" }, first: 3, lines: ["old"] }))).toBe(fresh);
  });

  it("keeps at most the ring's bound of lines and characters, the oldest going first", () => {
    let held = merged(null, snap({ lines: n(0, KEEP_LINES) }));
    held = merged(held, snap({ first: KEEP_LINES, lines: n(KEEP_LINES, 10) }));
    expect(held.lines.length).toBe(KEEP_LINES);
    expect([held.first, held.lines[0], held.lines.at(-1)]).toEqual([10, "line 10", `line ${KEEP_LINES + 9}`]);
    const long = "x".repeat(4096);
    const big = merged(null, snap({ lines: Array.from({ length: 100 }, () => long) }));
    expect(big.lines.reduce((c, l) => c + l.length + 1, 0)).toBeLessThanOrEqual(KEEP_CHARS);
    expect(big.first + big.lines.length).toBe(100);
  });

  it("words the state and the time as the block shows them", () => {
    expect(fmtRunTime(0)).toBe("0s");
    expect(fmtRunTime(59.9)).toBe("59s");
    expect(fmtRunTime(80)).toBe("1m 20s");
    expect(fmtRunTime(3900)).toBe("1h 5m");
    expect(fmtRunTime(-5)).toBe("0s");
    expect(shortFolder("/Users/someone/playground/muster/main/")).toBe("muster/main");
    expect(statusWords(snap())).toBe("Running bin/run in muster/main");
    expect(statusWords(snap({ status: { kind: "exited", code: 0 }, ended: 1130 }))).toBe("Exited 0 after 2m 10s");
    expect(statusWords(snap({ status: { kind: "exited", code: 101 }, ended: 1004 }))).toBe("Exited 101 after 4s");
    expect(statusWords(snap({ status: { kind: "exited", code: null }, ended: 1004 }))).toBe("Ended by a signal after 4s");
    expect(statusWords(snap({ status: { kind: "stopped" }, ended: 1062 }))).toBe("Stopped after 1m 2s");
  });

  it("shows the last lines, or every one held, each keyed by where it is in the run", () => {
    const run = snap({ first: 100, lines: n(100, 20) });
    expect(shownLines(run, false).map((l) => l.n)).toEqual(Array.from({ length: SHOWN_LINES }, (_, i) => 108 + i));
    expect(shownLines(run, false)[0].text).toBe("line 108");
    expect(shownLines(run, true).length).toBe(20);
    expect(shownLines(snap({ lines: ["one"] }), false)).toEqual([{ n: 0, text: "one" }]);
  });
});

describe("the Run line in What you're accepting", () => {
  it("shows Run named for the project with the command beside it as code", () => {
    const html = line(view());
    expect(html).toMatch(/<button[^>]*aria-label="Run muster"[^>]*>Run<\/button>/);
    expect(html).toContain('<code dir="ltr">bin/run</code>');
    expect(html).not.toContain("disabled");
    // While it runs, the same button restarts it.
    expect(line(view(), snap())).toMatch(/aria-label="Restart muster"[^>]*>Restart<\/button>/);
  });

  it("says how to set a run command when there's none, with no button", () => {
    const html = line(view({ command: "", refused: "muster has no run command" }));
    expect(text(html)).toBe("No run command: muster projects set muster run …");
    expect(html).not.toContain("<button");
  });

  it("turns Run off with why when the core can't run it, and says what a click was refused", () => {
    const html = line(view({ folder: "", refused: "no checkout to run in: /p/tasks is an old-layout queue" }));
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Run<\/button>/);
    expect(html).toContain("> no checkout to run in");
    expect(html).not.toContain('role="alert"');
    expect(line(view(), null, { error: "couldn't start \"bin/run\"" })).toContain("couldn&#x27;t start");
    expect(line(view(), null, { busy: true })).toMatch(/disabled=""[^>]*>Run<\/button>/);
    // Loading: a quiet mark, or why it couldn't load.
    expect(text(line(null))).toBe("…");
    expect(text(line(null, null, { error: "no project named \"x\"" }))).toBe('no project named "x"');
  });

  it("sits in the block as its own Try line, and only when given", () => {
    const task = { queue: "muster", id: "0079", prefix: "mu", title: "Run", stage: "your-check", criteria: [], summary: "Run from the app" } as unknown as Task;
    const html = renderToStaticMarkup(<AcceptingBlock task={task} urls={{}} open={new Set()} onToggle={noop} runLine={<span>RUNLINE</span>} />);
    expect(text(html)).toContain("Try RUNLINE");
    expect(renderToStaticMarkup(<AcceptingBlock task={task} urls={{}} open={new Set()} onToggle={noop} />)).not.toContain("Try");
    // The pane draws it in Your check, loading until run_state answers, and no Run block yet.
    const pane = renderToStaticMarkup(<StageContent task={{ ...task, ownerCriteria: [], stageHistory: [] } as Task} nowSecs={0} body={null} busy={false} onTick={noop} />);
    expect(text(pane)).toContain("Try …");
    expect(pane).not.toContain('data-part="run"');
  });
});

describe("the Run block", () => {
  it("running: the state in a polite live region, the time beside it, Stop and Restart, the last 12 lines", () => {
    const html = block(snap({ lines: n(0, 20) }));
    expect(html).toContain('<span class="small">Running bin/run in muster/main</span><span class="small meta"> · 1m 20s</span>');
    expect(html).not.toContain("aria-live=\"polite\"");
    expect(html).toMatch(/aria-label="Stop muster"[^>]*>Stop<\/button>/);
    expect(html).toMatch(/aria-label="Restart muster"[^>]*>Restart<\/button>/);
    const shown = [...html.matchAll(/<div class="run-out-line">([^<]*)<\/div>/g)].map((m) => m[1]);
    expect(shown).toEqual(n(8, 12));
    expect(html).toMatch(/<button[^>]*aria-expanded="false"[^>]*>All output \(20 lines\)<\/button>/);
  });

  it("All output opens every line kept in place, and shuts again", () => {
    const html = block(snap({ lines: n(0, 20) }), true);
    expect([...html.matchAll(/class="run-out-line"/g)].length).toBe(20);
    expect(html).toContain('class="run-output all"');
    expect(html).toMatch(/aria-expanded="true"[^>]*>Last 12 lines<\/button>/);
    // Twelve lines or fewer: nothing to open.
    expect(block(snap({ lines: n(0, 12) }))).not.toContain("aria-expanded");
  });

  it("exited and stopped: how it ended and when, Run again, no ticking time", () => {
    const exited = block(snap({ status: { kind: "exited", code: 0 }, ended: 1130, lines: ["done"] }), false, 99999);
    expect(exited).toContain(">Exited 0 after 2m 10s</span>");
    expect(exited).not.toContain("meta\"> ·");
    expect(exited).toMatch(/aria-label="Run again muster"[^>]*>Run again<\/button>/);
    expect(exited).not.toContain("Stop muster");
    const stopped = block(snap({ status: { kind: "stopped" }, ended: 1062, lines: [] }));
    expect(stopped).toContain(">Stopped after 1m 2s</span>");
    // Screen readers hear the state from a polite region there before any run, state words only.
    expect(renderToStaticMarkup(<RunStatusLive run={null} />)).toBe('<span class="sr-only" aria-live="polite"></span>');
    expect(renderToStaticMarkup(<RunStatusLive run={snap()} />)).toBe('<span class="sr-only" aria-live="polite">Running bin/run in muster/main</span>');
    expect(text(stopped)).toContain("No output yet.");
  });

  it("is a labelled log that isn't live, of plain text, each line set by its own direction", () => {
    const persian = "خطا: پوشه‌ی /Users/someone/پروژه پیدا نشد";
    const html = block(snap({ lines: ["Compiling muster v0.1.0", persian, "<b>not markup</b>", ""] }));
    expect(html).toContain('role="log" aria-live="off" aria-label="muster&#x27;s run output"');
    expect(html).toContain(`<div class="run-out-line">${persian}</div>`);
    expect(html).toContain("&lt;b&gt;not markup&lt;/b&gt;");
    expect(html).not.toContain("<b>not markup");
    expect(html).toContain('<div class="run-out-line">\u00a0</div>');
    expect(html).toContain('role="group" aria-label="Run of muster"');
    const rule = (sel: string) => css.match(new RegExp(`\\n${sel.replace(".", "\\.")} \\{([^}]*)\\}`))?.[1] ?? "";
    expect(rule(".run-out-line")).toContain("unicode-bidi: plaintext");
    expect(rule(".run-output")).toContain("monospace");
  });
});

describe("Run in Settings and the sidebar's chip", () => {
  it("Settings: Run, or while it runs a running chip with Stop and Restart; refused turns Run off", () => {
    const of = (run: RunSnap | null, refused = "", error = "") =>
      renderToStaticMarkup(<RunButtons project="muster" run={run} busy={false} refused={refused} error={error} onRun={noop} onStop={noop} />);
    expect(of(null)).toMatch(/aria-label="Run muster"[^>]*>Run<\/button>/);
    const going = of(snap());
    expect(going).toContain('<span class="run-chip">running</span>');
    expect(going).toMatch(/aria-label="Stop muster"[^>]*>Stop<\/button>/);
    expect(going).toMatch(/aria-label="Restart muster"[^>]*>Restart<\/button>/);
    expect(of(snap({ status: { kind: "stopped" }, ended: 1001 }))).not.toContain("run-chip");
    expect(of(null, "no checkout to run in")).toMatch(/disabled=""[^>]*>Run<\/button>/);
    // What a click was refused leaves Run on, to try again.
    expect(of(null, "", "couldn't start")).not.toContain("disabled");
  });

  it("the sidebar row says running in words while a run goes, and doesn't move", () => {
    const row = (running: boolean) => renderToStaticMarkup(createElement(ProjectItem, { name: "muster", total: 3, tasks: [], on: false, onClick: noop, running, children: null }));
    expect(row(true)).toMatch(/<span class="run-chip" title="[^"]+">running<\/span>/);
    expect(row(false)).not.toContain("run-chip");
    // Inside the row's button, so a click on it selects the project.
    expect(row(true).indexOf("run-chip")).toBeGreaterThan(row(true).indexOf("<button"));
    const chip = css.match(/\n\.run-chip \{([^}]*)\}/)?.[1] ?? "";
    expect(chip).toContain("animation: none");
    expect(chip).toContain("color: var(--text)");
  });
});
