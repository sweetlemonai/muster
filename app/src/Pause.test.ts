import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { isWorking, projectActivity } from "./activity";
import Card from "./Card";
import { applyQuery, dayContext, GROUPS, groupValues } from "./filters";
import { leaseRunOut, quietCheckin } from "./lib";
import { StatusLine } from "./PaneHeader";
import { GraphView } from "./GraphView";
import { readyToStart } from "./graph";
import { needsYou, needsYouCount } from "./human";
import { cardPauseNote, pausable, PausedChip, pausedText } from "./Pause";
import StageTrack from "./StageTrack";
import { TableView } from "./Views";
import WaitingList from "./WaitingList";
import { ProjectItem } from "./ProjectList";
import type { Task } from "./types";
import { pausedMsg } from "./useTaskActions";

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0042", title: "T", kind: "code", priority: "normal", status: "doing", stage: "building", blocked: false, stageHistory: [],
  folder: "doing", path: "doing/0042-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "building", ...over,
});

/** The wall clock here reading `s`, as the state's `now` (seconds since 1970). */
const at = (s: string) => {
  const [y, mo, d, h, mi] = s.split(/[- :]/).map(Number);
  return new Date(y, mo - 1, d, h, mi).getTime() / 1000;
};
const NOW = at("2026-10-13 12:00");
const A = "wt-a (wt-0045 · session 7b5e8856 · cc-hg)";
/** In Building since 08:00, quiet since 09:00, paused at 10:00 with a reason. */
const paused = { stage: "building", claimedBy: A, stageSince: "2026-10-13 08:00", heartbeat: "2026-10-13 09:00", paused: "2026-10-13 10:00", pausedWhy: "the design changes first", modified: NOW };
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/\s+/g, " ").trim();
const buttons = (html: string) => [...html.matchAll(/<button([^>]*)>([^<]*)<\/button>/g)].map((m) => `${m[2]}${m[1].includes("disabled") ? " (disabled)" : ""}`);
const card = (over: Partial<Task>) => renderToStaticMarkup(createElement(Card, { t: task(over), now: NOW, multi: false, parent: "", onOpen: () => {}, active: false, words: [] }));
const classOf = (html: string) => /^<button class="([^"]*)"/.exec(html)?.[1] ?? "";

describe("what a paused task says (0045)", () => {
  it("when and why, from the core's stamp", () => {
    expect(pausedText(task(paused), NOW)).toBe("2 h ago · the design changes first");
    expect(pausedText(task({ ...paused, pausedWhy: "" }), NOW)).toBe("2 h ago");
    expect(pausedText(task({}), NOW)).toBe("");
  });
  it("can be paused from Backlog through Testing, not in Your check or Done, nor twice", () => {
    for (const stage of ["backlog", "shaping", "ready", "building", "testing"]) expect(pausable(task({ stage })), stage).toBe(true);
    for (const stage of ["your-check", "done"]) expect(pausable(task({ stage })), stage).toBe(false);
    expect(pausable(task(paused))).toBe(false);
  });
  it("the chip is the neutral paused look with the pause glyph, never a stage colour or red", () => {
    const html = renderToStaticMarkup(createElement(PausedChip, { t: task(paused), now: NOW }));
    expect(html).toContain('class="paused-chip"');
    expect(text(html)).toBe("⏸︎ Paused 2 h ago · the design changes first");
    expect(html).not.toMatch(/blocked|s-building|warn/);
    expect(renderToStaticMarkup(createElement(PausedChip, { t: task({}), now: NOW }))).toBe("");
    // Review u8: the why is in the tooltip too, since the line may be cut.
    expect(html).toContain('title="Paused since 2026-10-13 10:00: the design changes first. Nobody claims or passes it until it&#x27;s resumed"');
  });
});

describe("a paused card on the board", () => {
  it("stays in its stage's look, with the Paused chip, not red, not quiet, and Resume under it", () => {
    const html = card(paused);
    const cls = classOf(html).split(" ");
    expect(cls).not.toContain("blocked");
    expect(cls).not.toContain("needs-you");
    expect(text(html)).toContain("Paused 2 h ago · the design changes first");
    // Three hours without a check-in, but paused: not quiet, so no flag and no Ask for a check-in.
    expect(text(html)).not.toContain("no check-in");
    expect(buttons(html)).toEqual(["Resume"]);
  });
  it("offers Pause on a card from Backlog through Testing, beside the card, and nothing after", () => {
    for (const stage of ["backlog", "ready", "building", "testing"]) expect(buttons(card({ stage })), stage).toEqual(["Pause"]);
    for (const stage of ["your-check", "done"]) expect(buttons(card({ stage })), stage).toEqual([]);
    // Review u7: Pause comes first, in the same place whether the card is quiet or not; a quiet
    // card adds its check-in ask after it, in the bordered tab, and otherwise the strip is bare.
    const quiet = card({ stage: "building", claimedBy: A, stageSince: "2026-10-13 08:00", heartbeat: "2026-10-13 11:00", modified: NOW });
    expect(buttons(quiet)).toEqual(["Pause", "Ask for a check-in"]);
    expect(quiet.match(/data-part="card-ping"/g)).toHaveLength(1);
    expect(quiet).toContain('<div class="card-ping" data-part="card-ping">');
    expect(card({ stage: "ready" })).toContain('<div class="card-ping bare" data-part="card-ping">');
  });
  it("starts the strip's Pause afresh when the task is paused or resumed, so an old note goes (review 2)", () => {
    // React remounts a child whose key changes, which clears what it remembered.
    const pauseOf = (t: Task) => {
      const strip = (Card({ t, now: NOW, multi: false, parent: "", onOpen: () => {}, active: false, words: [] }) as ReactElement<{ children: ReactElement[] }>).props.children[1] as ReactElement<{ children: ReactElement }>;
      return strip.props.children;
    };
    expect(pauseOf(task(paused)).key).toBe("2026-10-13 10:00");
    expect(pauseOf(task({ stage: "ready" })).key).toBe("");
  });
  it("says why its session couldn't be told, in the strip, until the next poll (review u6)", () => {
    expect(cardPauseNote({ path: "", session: "", unreached: "this claim has no session to reach; ask its session to re-claim" })).toBe(
      "its session can't be told: this claim has no session to reach; ask its session to re-claim",
    );
    expect(cardPauseNote({ path: "", session: "7b5e8856", unreached: "" })).toBe("");
    expect(cardPauseNote({ path: "", session: "", unreached: "" })).toBe("");
  });
  it("a paused claim isn't quiet and its lease doesn't run out", () => {
    const t = task({ ...paused, heartbeat: "2026-10-12 08:00" });
    expect(quietCheckin(t, NOW)).toBeNull();
    expect(leaseRunOut(t, NOW, 60)).toBe(false);
    expect(leaseRunOut(task({ ...t, paused: "" }), NOW, 60)).toBe(true);
  });
});

describe("the pane's status line", () => {
  const line = (over: Partial<Task>) =>
    renderToStaticMarkup(createElement(StatusLine, { task: task(over), now: NOW, pause: { busy: false, onPause: async () => true, onResume: () => {} } }));
  it("shows Paused with when and why beside the stage, and Resume", () => {
    const html = line(paused);
    expect(text(html)).toContain("Building ⏸︎ Paused 2 h ago · the design changes first");
    expect(buttons(html)).toEqual(["Resume"]);
  });
  it("offers Pause with an optional why, in place, no dialog", () => {
    const html = line({ stage: "ready" });
    expect(buttons(html)).toEqual(["Pause"]);
    expect(html).toContain('placeholder="why (optional)"');
    expect(buttons(line({ stage: "your-check" }))).toEqual([]);
  });
});

describe("what the pane says after a pause or resume", () => {
  it("whether its session was told, or why it couldn't be", () => {
    expect(pausedMsg({ path: "", session: "7b5e8856", unreached: "" }, true)).toBe(
      "Paused: it keeps its place and claim, and nobody works it until you resume it. Session 7b5e8856 is asked to stop at a safe point after its next tool call.",
    );
    expect(pausedMsg({ path: "", session: "", unreached: "this claim has no session to reach; ask its session to re-claim" }, false)).toBe(
      "Resumed: it carries on as it was. Its session can't be told: this claim has no session to reach; ask its session to re-claim.",
    );
    expect(pausedMsg({ path: "", session: "", unreached: "" }, false)).toBe("Resumed: it carries on as it was.");
  });
});

describe("finding paused tasks", () => {
  const ctx = dayContext(NOW);
  const tasks = [task({ ...paused, id: "0001" }), task({ id: "0002" })];
  it("the Flags filter has a Paused chip when a task is paused, and it keeps only those", () => {
    const flags = GROUPS.find((g) => g.id === "state")!;
    expect(groupValues(flags, tasks, [], ctx)).toContain("paused");
    expect(flags.chip("paused")).toBe("paused");
    expect(groupValues(flags, [task({})], [], ctx)).not.toContain("paused");
    expect(applyQuery(tasks, { search: "", filters: { state: ["paused"] }, sort: "priority" }, ctx).map((t) => t.id)).toEqual(["0001"]);
  });
  it("the sidebar row counts a project's paused tasks", () => {
    const row = (list: Task[]) => renderToStaticMarkup(createElement(ProjectItem, { name: "demo", total: list.length, tasks: list, on: false, onClick: () => {}, children: null }));
    const html = row(tasks);
    expect(html).toContain('<span class="side-paused" title="1 paused">⏸︎ 1</span>');
    expect(row([task({})])).not.toContain("side-paused");
  });
  it("a project whose only Building work is paused isn't stalled", () => {
    const list = [task({ ...paused, modified: NOW - 4 * 3600 })];
    expect(projectActivity(list, 0, NOW).stalled).toBe(false);
    expect(projectActivity([{ ...list[0], paused: "" }], 0, NOW).stalled).toBe(true);
  });
});

describe("paused everywhere a task shows (review)", () => {
  it("the owner's own paused task isn't waiting on them; an agent's that is says it's paused", () => {
    const own = task({ ...paused, forWho: "human", claimedBy: "owner" });
    expect(needsYou(own)).toBe(false);
    expect(needsYouCount([own, task({ forWho: "human", stage: "ready" })])).toBe(1);
    const asked = task({ ...paused, stage: "testing", waitingForOwner: true, questions: [{ n: 1, ticked: false, asker: "qa", text: "Which port?", escalated: true, ownerAnswered: false }], openQuestions: 1 });
    expect(needsYou(asked)).toBe(true);
    const html = renderToStaticMarkup(createElement(WaitingList, { tasks: [asked], multi: false, openKey: null, onOpen: () => {}, onChanged: () => {}, now: NOW }));
    expect(text(html)).toContain("Paused 2 h ago · the design changes first");
  });
  it("the graph doesn't call it ready to start, and says paused", () => {
    const ready = task({ stage: "ready", paused: "2026-10-13 10:00" });
    expect(readyToStart(ready)).toBe(false);
    expect(readyToStart(task({ stage: "ready" }))).toBe(true);
    const html = renderToStaticMarkup(createElement(GraphView, { tasks: [ready], all: [ready], multi: false, openKey: null, onOpen: () => {} }));
    expect(html).toContain('<span class="ui-badge ui-badge--paused">⏸︎ paused</span>');
    expect(html).not.toContain('class="g-ready"');
  });
  it("the table has a grey Paused pill beside the stage", () => {
    const html = renderToStaticMarkup(createElement(TableView, { tasks: [task(paused)], now: NOW, multi: false, openKey: null, onOpen: () => {}, words: [] }));
    expect(html).toContain('<span class="ui-status-pill ui-status-pill--building">Building</span><span class="ui-status-pill ui-status-pill--paused">⏸︎ Paused</span>');
  });
  it("a fresh heartbeat on a paused task isn't anyone working", () => {
    const fresh = task({ ...paused, heartbeat: "2026-10-13 11:55" });
    expect(isWorking(fresh, NOW)).toBe(false);
    expect(isWorking({ ...fresh, paused: "" }, NOW)).toBe(true);
  });
  it("the pane's stage track is off while it's paused, with why", () => {
    const html = renderToStaticMarkup(createElement(StageTrack, { stage: "building", busy: false, onPick: () => {}, paused: "2026-10-13 10:00" }));
    const off = (h: string) => [...h.matchAll(/<button([^>]*)>/g)].map((m) => m[1].includes("disabled"));
    expect(off(html)).toEqual(Array(7).fill(true));
    expect(text(html)).toContain("paused: resume it to move it");
    const live = renderToStaticMarkup(createElement(StageTrack, { stage: "building", busy: false, onPick: () => {} }));
    expect(off(live)).toEqual(Array(7).fill(false));
  });
});
