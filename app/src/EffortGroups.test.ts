import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import EffortGroups, { AcceptAllView, confirmEscape, putAcceptResult } from "./EffortGroups";
import { windowKey } from "./keys";
import css from "./styles.css?raw";
import { GroupedBoard } from "./Groups";
import WaitingList from "./WaitingList";
import type { Criterion, Question, Task } from "./types";

const crit = (n: number, ticked: boolean, words: string): Criterion => ({ n, label: `AC${n}`, text: `AC${n}: ${words} — verify: owner`, ticked, owner: true });
const task = (id: string, over: Partial<Task> = {}): Task => ({
  queue: "muster", id, title: `Task ${id}`, kind: "code", priority: "normal", status: "doing", stage: "your-check", blocked: false, stageHistory: [],
  folder: "doing", path: `doing/${id}-t.md`, claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: true, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "your-check", ...over,
});
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/\s+/g, " ").trim();

const CHECK = [
  task("0016", { ownerCriteria: [crit(7, false, "the owner groups the board by Feature and checks a goal's progress.")] }),
  task("0003", { ownerCriteria: [crit(1, false, "every row routes to a working command.")] }),
  task("0014"),
  task("0021", { ownerCriteria: [crit(2, true, "looked")] }),
  task("0024", { blocked: true, waitingOn: "owner: pick", blockedKind: "you", blockDetail: "pick" }),
];

describe("the Your check column by effort (0043)", () => {
  const groups = (tasks: Task[]) =>
    renderToStaticMarkup(createElement(EffortGroups, { tasks, place: "test", renderCard: (t: Task) => createElement("i", { className: "card" }, t.id), onChanged: () => {} }));
  it("shows Just accept, One look and Needs a session in that order, each with its count and cards", () => {
    const html = groups(CHECK);
    const shown = [...html.matchAll(/<div class="effort-group[^"]*" role="group" aria-label="([^"]+)">([\s\S]*?)<\/div>/g)].map(
      (m) => `${m[1]}: ${[...m[2].matchAll(/<i class="card">(\d+)<\/i>/g)].map((c) => c[1]).join(" ")}`,
    );
    expect(shown).toEqual(["Just accept: 0014 0021", "One look: 0016 0024", "Needs a session: 0003"]);
    expect([...html.matchAll(/<h3 class="effort-head">([^<]*) <small>(\d+)<\/small>/g)].map((m) => `${m[1]} ${m[2]}`)).toEqual(["Just accept 2", "One look 2", "Needs a session 1"]);
  });
  it("puts Accept all in Just accept, with its count", () => {
    expect(groups(CHECK)).toMatch(/aria-label="Just accept">.*<button type="button" class="accept-all">Accept all 2<\/button>/);
    // Nothing to accept: no button.
    expect(groups([CHECK[0]])).not.toContain("accept-all");
  });
  it("keeps an empty group's heading, quietly, so nothing moves", () => {
    expect(groups([CHECK[2]])).toContain('<div class="effort-group effort-look empty" role="group" aria-label="One look"><h3 class="effort-head">One look <small>0</small></h3></div>');
  });
});

describe("the grouped board's Your check columns (0043)", () => {
  const band = (onChanged?: () => void) =>
    renderToStaticMarkup(
      createElement(GroupedBoard, {
        groups: [{ head: null, items: [CHECK[0], CHECK[2], task("0009", { stage: "ready", waitingForOwner: false })] }], by: "feature", sort: "priority", multi: false, openKey: null, onOpen: () => {},
        renderCard: (t: Task) => createElement("i", { className: "card" }, t.id), onChanged,
      }),
    );
  it("group by effort inside each band, with Accept all for the band's own", () => {
    const col = /<section class="column col-your-check">([\s\S]*?)<\/section>/.exec(band(() => {}))?.[1] ?? "";
    expect(text(col)).toBe("Your check 2 Just accept 1 Accept all 1 0014 One look 1 0016 Needs a session 0");
    // The other columns are as they were.
    expect(/<section class="column col-ready">([\s\S]*?)<\/section>/.exec(band(() => {}))?.[1]).not.toContain("effort");
  });
});

describe("Accept all's confirm, in place (0043)", () => {
  const view = (over: Partial<Parameters<typeof AcceptAllView>[0]>) =>
    renderToStaticMarkup(createElement(AcceptAllView, { count: 2, asked: null, busy: false, msg: null, onAsk: () => {}, onConfirm: () => {}, onCancel: () => {}, ...over }));
  it("first shows the count", () => {
    expect(view({})).toBe('<span class="accept-all-bar"><button type="button" class="accept-all">Accept all 2</button></span>');
  });
  it("then asks, in its own place, naming each task, with the second click and Cancel", () => {
    const html = view({ asked: ["0014", "0021"] });
    expect(text(html)).toBe("Accept 2: 0014, 0021? Yes, accept 2 Cancel");
    expect(html).toContain('role="group" aria-label="Confirm Accept all"');
    expect(html).toMatch(/<button type="button" class="primary">Yes, accept 2<\/button>/);
    expect(html).not.toContain("dialog");
  });
  it("is off while it runs, then says what came of it", () => {
    expect(view({ asked: ["0014"], busy: true })).toMatch(/<button type="button" class="primary" disabled="">Accepting…<\/button><button type="button" disabled="">Cancel<\/button>/);
    expect(view({ count: 0, msg: { ok: true, text: "Accepted 2: they're done." } })).toBe('<span class="accept-all-bar"><span class="ok small" role="status">Accepted 2: they&#x27;re done.</span></span>');
    expect(view({ count: 1, msg: { ok: false, text: "Accepted none. Not accepted: muster 0014: no." } })).toContain('<span class="err small" role="status">');
    expect(view({ count: 0 })).toBe("");
  });
});

describe("Waiting for you by effort (0043)", () => {
  const q = (n: number, text: string): Question => ({ n, ticked: false, asker: "wt-a", text, escalated: true, ownerAnswered: false });
  const list = (tasks: Task[]) => renderToStaticMarkup(createElement(WaitingList, { tasks, multi: false, openKey: null, onOpen: () => {}, onChanged: () => {}, now: 0 }));
  it("groups Your check by effort, with each one's next step in full, then the rest", () => {
    const html = list([
      ...CHECK.slice(0, 3),
      task("0051", { stage: "building", claimedBy: "wt-a", questions: [q(2, "Child webview or a window?")], openQuestions: 1 }),
      task("0041", { forWho: "human", stage: "ready", waitingForOwner: false, title: "Buy the domain" }),
    ]);
    const heads = [...html.matchAll(/<h3 class="effort-head">([^<]*) <small>(\d+)<\/small>/g)].map((m) => `${m[1]} ${m[2]}`);
    expect(heads).toEqual(["Just accept 1", "One look 1", "Needs a session 1", "Questions, blocks and your own tasks 2"]);
    const items = [...html.matchAll(/<article[^>]*>([\s\S]*?)<\/article>/g)].map((m) => text(m[1]));
    expect(items).toEqual([
      "0014 · Task 0014 Nothing to try: Accept Open",
      "0016 · Task 0016 Try: group the board by Feature and check a goal's progress Open",
      "0003 · Task 0003 Check: every row routes to a working command Open",
      "0051 · Task 0051 Q2 Child webview or a window? Open to answer",
      "You 0041 · Buy the domain Yours to do: in To do. Start it when you do. Open",
    ]);
    expect(html).toContain('<button type="button" class="accept-all">Accept all 1</button>');
  });
  it("says when nothing is waiting, and leaves out the effort groups with no Your check", () => {
    expect(text(list([]))).toBe("Nothing is waiting for you.");
    const html = list([task("0051", { stage: "building", questions: [q(2, "Which?")], openQuestions: 1 })]);
    expect(html).not.toContain("Just accept");
    expect(html).toContain("Questions, blocks and your own tasks");
  });
  it("leaves a goal or feature in Your check out of the effort groups and Accept all (review, round 1)", () => {
    const html = list([task("0001", { level: "goal", title: "Ship 1.0" }), task("0014")]);
    expect(html).toContain('<button type="button" class="accept-all">Accept all 1</button>');
    const other = /aria-label="Questions, blocks and your own tasks">([\s\S]*)<\/div>/.exec(html)?.[1] ?? "";
    expect(text(other)).toContain("0001 · Ship 1.0");
  });
  it("puts a Your check task with a question for the owner in One look, the question once, and not in Accept all", () => {
    const html = list([task("0030", { questions: [q(1, "Which colour?")], openQuestions: 1 })]);
    expect(html).not.toContain("accept-all");
    const items = [...html.matchAll(/<article[^>]*>([\s\S]*?)<\/article>/g)].map((m) => text(m[1]));
    expect(items).toEqual(["0030 · Task 0030 Q1 Which colour? Open to answer"]);
    expect(html).toMatch(/aria-label="One look"><h3 class="effort-head">One look <small>1<\/small>/);
  });
});

describe("Accept all's confirm and the keyboard (review, round 1)", () => {
  /** A key pressed while the confirm is open, as the browser runs it: the confirm's capture
   *  listener on the window first, then the window's own shortcuts unless it stopped the key. */
  function press(key: string, busy: boolean) {
    const log: string[] = [];
    let stopped = false;
    const e = { key, metaKey: false, ctrlKey: false, stopPropagation: () => (stopped = true), preventDefault: () => {} };
    confirmEscape(e, busy, () => log.push("cancel"));
    if (!stopped) windowKey(e, { closePane: () => log.push("close pane"), newTask: () => log.push("new task"), search: () => log.push("search"), focusMode: () => log.push("focus") });
    return { log, stopped };
  }
  it("Esc cancels the confirm and the pane stays open", () => {
    expect(press("Escape", false)).toEqual({ log: ["cancel"], stopped: true });
  });
  it("Esc while it runs does nothing, and still doesn't close the pane", () => {
    expect(press("Escape", true)).toEqual({ log: [], stopped: true });
  });
  it("other keys go on as usual", () => {
    expect(press("Enter", false)).toEqual({ log: [], stopped: false });
  });
});

describe("Accept all's result line outlives its column (review, round 1)", () => {
  const render = (tasks: Task[], place: string) =>
    renderToStaticMarkup(createElement(EffortGroups, { tasks, place, renderCard: (t: Task) => createElement("i", null, t.id), onChanged: () => {} }));
  it("shows the place's result with no tasks left, and nothing when there's none", () => {
    putAcceptResult("emptied", { ok: true, text: "Accepted 2: they're done.", refused: [], known: ["muster/0014"], at: 1 });
    expect(render([], "emptied")).toBe('<span class="accept-all-bar"><span class="ok small" role="status">Accepted 2: they&#x27;re done.</span></span>');
    expect(render([], "never")).toBe("");
    putAcceptResult("emptied", null);
    expect(render([], "emptied")).toBe("");
  });
  it("shows it in Just accept while tasks remain, one place's not another's", () => {
    putAcceptResult("here", { ok: false, text: "Accepted none. Not accepted: muster 0014: it is blocked.", refused: ["muster/0014"], known: ["muster/0014"], at: 1 });
    expect(render([task("0014")], "here")).toContain('<span class="err small" role="status">Accepted none. Not accepted: muster 0014: it is blocked.</span>');
    expect(render([task("0014")], "there")).not.toContain("role=\"status\"");
    putAcceptResult("here", null);
  });
});

describe("the colours (review, round 1)", () => {
  it("draws Your turn as a filled chip, like the needs-you badge, for contrast", () => {
    const rule = /\.card-who\.your-turn, \.holders \.your-turn \{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(rule).toContain("background: var(--st-needs)");
    expect(rule).toContain("color: var(--on-status)");
  });
  it("doesn't light a checklist row on hover: only its buttons are clickable", () => {
    expect(css).not.toContain(".criterion:hover");
  });
});
