import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { isWorking, projectActivity } from "./activity";
import { cardLines, look, Mode, MODES } from "./modes";
import CardStatus from "./CardStatus";
import { needsYouCount, waitingForYou } from "./human";
import { waitingOnYou } from "./paneData";
import { StatusLine } from "./PaneHeader";
import { CardPing, QuietPing } from "./Ping";
import { cantAsk } from "./reach";
import Card from "./Card";
import { ANSWER_FIRST, ribbonStateWords, sessionStatesLine, sessionWords } from "./sessionState";
import css from "./styles.css?raw";
import type { Task } from "./types";
import WaitingList from "./WaitingList";

// Claude Code's session states on the card, the pane, the sidebar and the ping (0048). The core
// reads and matches them (agents.rs); these check what the window makes of each.

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0048", title: "T", kind: "task", priority: "normal", status: "doing", stage: "building", blocked: false, stageHistory: [],
  folder: "doing", path: "doing/0048-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "building", ...over,
});

/** The wall clock here reading `s`, as the state's `now` (seconds since 1970): a fake clock. */
const at = (s: string) => {
  const [y, mo, d, h, mi] = s.split(/[- :]/).map(Number);
  return new Date(y, mo - 1, d, h, mi).getTime() / 1000;
};
const NOW = at("2026-10-13 14:05");
const A = "wt-a (wt-0048 · session 7b5e8856 · cc-hg)";
/** In Building since noon, last heard from at 13:20: quiet for 45 minutes. */
const quiet = { stage: "building", claimedBy: A, stageSince: "2026-10-13 12:00", heartbeat: "2026-10-13 13:20", modified: NOW };
/** Waiting for input, as the core sends it: the state, and waiting on the owner. */
const waiting = { ...quiet, sessionState: "needs-input" as const, waitingForOwner: true };

function lines(over: Partial<Task>): [string, string][] {
  const html = renderToStaticMarkup(createElement(CardStatus, { t: task(over), now: NOW }));
  return [...html.matchAll(/<span class="([^"]+)"[^>]*>(?:<i[^>]*><\/i>)?([^<]*)<\/span>/g)].map((m) => [m[1], m[2]]);
}

describe("the words for each state", () => {
  it("working, waiting for input, ended in Building, and nothing for unknown", () => {
    expect(sessionWords(task({ sessionState: "working" }))).toBe("working");
    expect(sessionWords(task({ sessionState: "needs-input" }))).toBe("waiting for input in Claude Code");
    expect(sessionWords(task({ sessionState: "ended" }))).toBe("session ended, task still in Building");
    // In Testing the holder is the builder that passed it: its session ending is the normal course.
    expect(sessionWords(task({ stage: "testing", sessionState: "ended" }))).toBe("");
    expect(sessionWords(task({ sessionState: "unknown" }))).toBe("");
    expect(sessionWords(task({}))).toBe("");
  });
  it("Settings > Agents says whether they're read, as the core sums it up", () => {
    expect(sessionStatesLine("on (2 accounts)")).toBe("Claude Code's session states: on (2 accounts)");
    expect(sessionStatesLine("not available: claude isn't installed")).toBe("Claude Code's session states: not available: claude isn't installed");
  });
});

describe("the ribbon's words for each state (mu-171)", () => {
  it("full: working, waiting for input, session ended in Building only; nothing for unknown or paused", () => {
    expect(ribbonStateWords(task({ sessionState: "working" }), false)).toBe("working");
    expect(ribbonStateWords(task({ sessionState: "needs-input" }), false)).toBe("waiting for input");
    expect(ribbonStateWords(task({ sessionState: "ended" }), false)).toBe("session ended");
    expect(ribbonStateWords(task({ stage: "testing", sessionState: "ended" }), false)).toBe("");
    expect(ribbonStateWords(task({ stage: "shaping", sessionState: "ended" }), false)).toBe("");
    expect(ribbonStateWords(task({ sessionState: "unknown" }), false)).toBe("");
    expect(ribbonStateWords(task({}), false)).toBe("");
    // Paused (0045): its session was told to stop, so whatever was read says nothing.
    expect(ribbonStateWords(task({ sessionState: "working", paused: "2026-10-13 10:00" }), false)).toBe("");
  });
  it("short, for a narrow card: working, needs you, ended", () => {
    expect(ribbonStateWords(task({ sessionState: "working" }), true)).toBe("working");
    expect(ribbonStateWords(task({ sessionState: "needs-input" }), true)).toBe("needs you");
    expect(ribbonStateWords(task({ sessionState: "ended" }), true)).toBe("ended");
    expect(ribbonStateWords(task({ stage: "testing", sessionState: "ended" }), true)).toBe("");
    expect(ribbonStateWords(task({ sessionState: "unknown" }), true)).toBe("");
  });
  it("says something exactly where the pane's words do, so the two can't drift", () => {
    for (const stage of ["shaping", "building", "testing"])
      for (const sessionState of ["working", "needs-input", "ended", "unknown"] as const) {
        const t = task({ stage, sessionState });
        expect(ribbonStateWords(t, false) !== "", `${stage} ${sessionState}`).toBe(sessionWords(t) !== "");
      }
  });
});

describe("the card (mu-171: the state is on the ribbon, not a line)", () => {
  it("has no session line; the check-in flag stays, quiet, while a state is known", () => {
    expect(lines({ ...quiet, sessionState: "working" })).toEqual([
      ["card-who", "wt-a · wt-0048 · 7b5e8856 · started 2 h ago"],
      ["card-quiet calm", "no check-in for 45 min"],
    ]);
    expect(lines(waiting)).toEqual([
      ["card-who", "wt-a · wt-0048 · 7b5e8856 · started 2 h ago"],
      ["card-quiet calm", "no check-in for 45 min"],
    ]);
    expect(lines({ ...quiet, sessionState: "ended" }).map(([c]) => c)).toEqual(["card-who", "card-quiet calm"]);
    for (const sessionState of ["working", "needs-input", "ended", "unknown"] as const) {
      const html = renderToStaticMarkup(createElement(CardStatus, { t: task({ ...quiet, sessionState }), now: NOW }));
      expect(html, sessionState).not.toContain("card-session");
    }
  });
  it("with no state from Claude Code reads exactly as before", () => {
    const before = lines(quiet);
    expect(before).toEqual([
      ["card-who", "wt-a · wt-0048 · 7b5e8856 · started 2 h ago"],
      ["card-quiet", "no check-in for 45 min"],
    ]);
    expect(lines({ ...quiet, sessionState: "unknown" })).toEqual(before);
  });
  it("leaves no .card-session rule in the styles; the pane's .session-state stays", () => {
    expect(css).not.toContain(".card-session");
    expect(css).toMatch(/\.session-state\.needs-input[^{]*\{[^}]*background: var\(--st-needs\); color: var\(--on-status\)/);
  });
});

describe("in every display mode (0038)", () => {
  it("the state shows on the ribbon, beside 0047's needs a tester and 0051's Done line", () => {
    for (const { id: mode } of MODES) {
      for (const active of [false, true]) {
        const card = (over: Partial<Task>) => {
          const el = createElement(Card, { t: task(over), now: NOW, multi: false, parent: "", onOpen: () => {}, active, words: [] });
          return renderToStaticMarkup(createElement(Mode.Provider, { value: mode }, el));
        };
        const status = (over: Partial<Task>) => {
          const el = createElement(CardStatus, { t: task(over), now: NOW, lines: cardLines(look("card-lines", mode), active) });
          return renderToStaticMarkup(createElement(Mode.Provider, { value: mode }, el));
        };
        const where = `${mode}${active ? ", selected" : ""}`;
        expect(card(waiting), where).toContain('<span aria-hidden="true" class="holder-state needs-input">');
        expect(card(waiting), where).toContain(">waiting for input</span>");
        expect(card({ ...quiet, sessionState: "working" }), where).toContain('<span aria-hidden="true" class="holder-state working">');
        expect(card(waiting), where).not.toContain("card-session");
        expect(status({ stage: "testing", heldBy: A, builtBy: A, gatedBy: A, stageSince: "2026-10-13 13:00" }), where).toContain(">needs a tester</span>");
        expect(status({ stage: "done", doneAt: "2026-10-13 14:01" }), where).toContain("done 4 min ago");
      }
    }
  });
});

describe("the task pane's who line", () => {
  const status = (over: Partial<Task>) => renderToStaticMarkup(createElement(StatusLine, { task: task(over), now: NOW }));
  it("adds the state after who's on it, and waiting for input is waiting on you", () => {
    expect(status({ ...quiet, sessionState: "working" })).toContain('<span class="session-state working">working</span>');
    const w = status(waiting);
    expect(w).toContain('<span class="session-state needs-input">waiting for input in Claude Code</span>');
    expect(w).toContain("Waiting on you: input in Claude Code");
    expect(status({ ...quiet, sessionState: "ended" })).toContain('<span class="session-state ended">session ended, task still in Building</span>');
    expect(status(quiet)).not.toContain("session-state");
    expect(waitingOnYou(task(waiting))).toEqual(["input in Claude Code"]);
    expect(waitingOnYou(task({ ...quiet, sessionState: "working" }))).toEqual([]);
  });
});

describe("what needs you", () => {
  it("counts a session waiting for input, lists it in Waiting for you, and says why there", () => {
    const tasks = [task(waiting), task({ ...quiet, id: "0049", sessionState: "working" })];
    expect(needsYouCount(tasks)).toBe(1);
    expect(waitingForYou(tasks).map((t) => t.id)).toEqual(["0048"]);
    const html = renderToStaticMarkup(createElement(WaitingList, { tasks: waitingForYou(tasks), multi: false, openKey: null, onOpen: () => {}, onChanged: () => {}, now: NOW }));
    expect(html).toContain("Waiting for input in Claude Code: answer it there.");
  });
});

describe("the sidebar's working now", () => {
  it("takes Claude Code's word where it has one: working is working, waiting or ended isn't", () => {
    // Quiet for 45 minutes and no live transcript: not working by heartbeats.
    expect(isWorking(task(quiet), NOW)).toBe(false);
    expect(isWorking(task({ ...quiet, sessionState: "working" }), NOW)).toBe(true);
    // A transcript that changed a minute ago, but the session waits on a prompt or has ended.
    expect(isWorking(task({ ...quiet, live: true }), NOW)).toBe(true);
    expect(isWorking(task({ ...quiet, live: true, sessionState: "needs-input" }), NOW)).toBe(false);
    expect(isWorking(task({ ...quiet, live: true, sessionState: "ended" }), NOW)).toBe(false);
    expect(isWorking(task({ ...quiet, sessionState: "working", stage: "done" }), NOW)).toBe(false);
    expect(projectActivity([task({ ...quiet, sessionState: "working" })], 0, NOW)).toEqual({ working: 1, stalled: false, last: 0 });
  });
});

describe("Ask for a check-in while the session waits for input", () => {
  it("is refused in the window with why: answer it in Claude Code first", () => {
    expect(ANSWER_FIRST).toBe("waiting for input in Claude Code: answer it there first");
    expect(cantAsk(task(waiting))).toBe(ANSWER_FIRST);
    expect(cantAsk(task({ ...quiet, sessionState: "working" }))).toBe("");
    const card = renderToStaticMarkup(createElement(CardPing, { t: task(waiting), now: NOW }));
    expect(card).toContain("disabled");
    expect(card).toContain(`<span class="why">${ANSWER_FIRST}</span>`);
  });
  it("the sidebar's Ask all leaves it out and says so", () => {
    const html = renderToStaticMarkup(createElement(QuietPing, { tasks: [task(waiting), task({ ...quiet, id: "0049" })], now: NOW }));
    expect(html).toContain("Ask all quiet sessions for a check-in (1)");
    expect(html).toContain("1 waits for input in Claude Code: answer it there first");
    const only = renderToStaticMarkup(createElement(QuietPing, { tasks: [task(waiting)], now: NOW }));
    expect(only).toContain("1 waits for input in Claude Code: answer it there first");
    expect(only).not.toContain("none of its quiet claims has a session to reach");
  });
});
