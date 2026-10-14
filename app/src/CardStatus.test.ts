import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import CardStatus from "./CardStatus";
import type { Task } from "./types";
import { Machine } from "./settingsData";
import css from "./styles.css?raw";

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0030", title: "T", kind: "task", priority: "normal", status: "doing", stage: "building", blocked: false, stageHistory: [],
  folder: "doing", path: "doing/0030-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "building", ...over,
});

/** The wall clock here reading `s`, as the state's `now` (seconds since 1970): a fake clock. */
const at = (s: string) => {
  const [y, mo, d, h, mi] = s.split(/[- :]/).map(Number);
  return new Date(y, mo - 1, d, h, mi).getTime() / 1000;
};
const NOW = at("2026-10-12 14:05");
const A = "wt-a (wt-0030 · session 7b5e8856 · cc-hg)";

/** The card's lines as [class, text]. */
function lines(over: Partial<Task>, now = NOW): [string, string][] {
  const html = renderToStaticMarkup(createElement(CardStatus, { t: task(over), now }));
  return [...html.matchAll(/<span class="([^"]+)"[^>]*>([^<]*)<\/span>/g)].map((m) => [m[1], m[2].replace(/&#x27;/g, "'")]);
}

describe("the card's who and progress lines", () => {
  const building = { stage: "building", claimedBy: A, stageSince: "2026-10-12 12:00", heartbeat: "2026-10-12 13:53", modified: NOW };
  it("in Building: who, since when, and the latest check-in with its percentage and age", () => {
    const checkins = [{ at: "2026-10-12 13:53", who: A, pct: 60, text: "gate green, writing tests" }];
    expect(lines({ ...building, checkins })).toEqual([
      ["card-who", "wt-a · wt-0030 · 7b5e8856 · started 2 h ago"],
      ["card-checkin", "60% · gate green, writing tests · 12 min ago"],
    ]);
  });
  it("flags a session quiet past 30 min in the attention colour, and not at 30", () => {
    expect(lines({ ...building, heartbeat: "2026-10-12 13:20" })).toEqual([
      ["card-who", "wt-a · wt-0030 · 7b5e8856 · started 2 h ago"],
      ["card-quiet", "no check-in for 45 min"],
    ]);
    expect(lines({ ...building, heartbeat: "2026-10-12 13:35" }).map(([c]) => c)).toEqual(["card-who"]);
    // The clock moves on, the same card goes quiet.
    expect(lines({ ...building, heartbeat: "2026-10-12 13:35" }, NOW + 60).at(-1)).toEqual(["card-quiet", "no check-in for 31 min"]);
    // Past the lease too: it says so, as the Release button does.
    expect(lines({ ...building, heartbeat: "2026-10-12 01:00" }).at(-1)).toEqual(["card-quiet", "quiet 12 h+ · no check-in for 13 h"]);
    // Blocked is waiting, not quiet; the owner's own claim only shows the lease.
    expect(lines({ ...building, heartbeat: "2026-10-12 10:00", blocked: true, waitingOn: "owner" }).map(([c]) => c)).toEqual(["card-who"]);
    expect(lines({ ...building, claimedBy: "owner", heartbeat: "2026-10-12 01:00" })).toEqual([
      ["card-who", "you · started 2 h ago"],
      ["card-quiet", "quiet 12 h+"],
    ]);
  });
  it("judges the lease by this machine's settings (0036), and names it", () => {
    const withLease = (leaseMinutes: number, over: Partial<Task>) => {
      const card = createElement(CardStatus, { t: task(over), now: NOW });
      const html = renderToStaticMarkup(createElement(Machine.Provider, { value: { leaseMinutes, liveMinutes: 5 } }, card));
      return /<span class="card-quiet">([^<]*)<\/span>/.exec(html)?.[1] ?? "";
    };
    // 1 h 15 min quiet: past a 1-hour lease, not the default.
    const quiet = { ...building, claimedBy: "owner", heartbeat: "2026-10-12 12:50" };
    expect(withLease(60, quiet)).toBe("quiet 1 h+");
    expect(withLease(90, quiet)).toBe("");
    expect(withLease(720, quiet)).toBe("");
    expect(withLease(70, { ...quiet })).toBe("quiet 1 h 10 min+");
  });
  it("in Testing and Your check: the holder from held-by, and Testing's start", () => {
    expect(lines({ stage: "testing", heldBy: "wt-0030 (session 7b5e8856)", stageSince: "2026-10-12 13:00" })).toEqual([
      ["card-who", "wt-0030 · 7b5e8856 · started 1 h ago"],
    ]);
    expect(lines({ stage: "your-check", heldBy: "wt-0030 (session 7b5e8856)" })[0]).toEqual(["card-who", "wt-0030 · 7b5e8856"]);
  });
  it("in Testing, held by its builder: needs a tester, on a quiet line of its own (0047)", () => {
    expect(lines({ stage: "testing", heldBy: A, builtBy: A, gatedBy: A, stageSince: "2026-10-12 13:00" })).toEqual([
      ["card-who", "wt-a · wt-0030 · 7b5e8856 · started 1 h ago"],
      ["card-tester", "needs a tester"],
    ]);
    // Gated into Testing by a session that never claimed it: a builder all the same.
    expect(lines({ stage: "testing", heldBy: "wt-b", builtBy: A, gatedBy: "wt-b", stageSince: "2026-10-12 13:00" }).at(-1)).toEqual(["card-tester", "needs a tester"]);
    expect(lines({ stage: "testing", heldBy: "owner", builtBy: A, stageSince: "2026-10-12 13:00" })).toEqual([["card-who", "you · started 1 h ago"]]);
  });
  it("in Your check: who built it and who tested it, under the next step (0047)", () => {
    expect(lines({ stage: "your-check", heldBy: "owner", builtBy: A, testedBy: "qa-0030 (session 99999999)" })).toEqual([
      ["card-who your-turn", "Your turn"],
      ["card-next", "Nothing to try: Accept"],
      ["card-built", "built by wt-a · tested by qa-0030"],
    ]);
  });
  it("never cuts who built it, who tested it or needs a tester: those lines wrap (0047)", () => {
    const rule = (sel: string) => [...css.replace(/\/\*[\s\S]*?\*\//g, "").matchAll(/([^{}]+)\{([^}]*)\}/g)].filter((m) => m[1].split(",").map((s) => s.trim()).includes(sel)).map((m) => m[2]).join(";");
    for (const sel of [".card-built", ".card-tester"]) {
      expect(rule(sel), sel).toContain("color: var(--muted)");
      expect(rule(sel), sel).not.toMatch(/nowrap|ellipsis/);
      expect(rule(sel), sel).toContain("overflow-wrap: anywhere");
    }
  });
  it("in Your check: Your turn, then the next step (0043)", () => {
    expect(lines({ stage: "your-check", heldBy: "owner", stageSince: "2026-10-12 13:00" })).toEqual([
      ["card-who your-turn", "Your turn"],
      ["card-next", "Nothing to try: Accept"],
    ]);
    // Nobody named is the owner's too.
    expect(lines({ stage: "your-check" })[0]).toEqual(["card-who your-turn", "Your turn"]);
    const crit = (n: number, ticked: boolean, words: string) => ({ n, label: `AC${n}`, text: `AC${n}: ${words} — verify: owner`, ticked, owner: true });
    expect(lines({ stage: "your-check", ownerCriteria: [crit(1, true, "x"), crit(2, false, "the owner opens INDEX.md and finds it useful."), crit(3, false, "y")] })).toEqual([
      ["card-who your-turn", "Your turn"],
      ["card-next", "Try: open INDEX.md and find it useful · 1 more"],
    ]);
    // A long one is cut on the card, and whole in its tooltip.
    const long = `the owner looks at ${"every part of the sidebar ".repeat(6)}`;
    const html = renderToStaticMarkup(createElement(CardStatus, { t: task({ stage: "your-check", ownerCriteria: [crit(1, false, long)] }), now: NOW }));
    expect(html).toMatch(/<span class="card-next" title="Try: look at (every part of the sidebar ){5}every part of the sidebar">Try: look at every part[^<]*…<\/span>/);
  });
  it("a task sent back from Testing says to fix it first, and the holder's other task says it waits behind (mu-66)", () => {
    const base = { ...building, prefix: "mu", id: "0059" };
    expect(lines({ ...base, sentBack: true })).toEqual([
      ["card-who", "wt-a · wt-0030 · 7b5e8856 · started 2 h ago"],
      ["card-first", "fix mu-59 first (sent back by the review)"],
    ]);
    expect(lines({ ...base, id: "0061", behind: "0059" })).toEqual([
      ["card-who", "wt-a · wt-0030 · 7b5e8856 · started 2 h ago"],
      ["card-behind", "waiting behind mu-59 (sent back by the review)"],
    ]);
    // A send-back that is blocked is waiting on something else, not first; the plain card is unchanged.
    expect(lines({ ...base, sentBack: true, blocked: true, waitingOn: "owner" }).map(([c]) => c)).toEqual(["card-who"]);
    expect(lines({ ...base }).map(([c]) => c)).toEqual(["card-who"]);
    // Every look keeps them: they say which to do first.
    for (const look of ["brief", "status"] as const) {
      const html = renderToStaticMarkup(createElement(CardStatus, { t: task({ ...base, sentBack: true }), now: NOW, lines: look }));
      expect(html, look).toContain("fix mu-59 first (sent back by the review)");
    }
  });
  it("in Ready says nobody is on it yet, and Backlog, Shaping and Done say nothing", () => {
    expect(lines({ stage: "ready" })).toEqual([["card-who nobody", "nobody on it"]]);
    expect(lines({ stage: "ready", claimedBy: "wt-b" })).toEqual([["card-who", "wt-b"]]);
    for (const stage of ["backlog", "shaping", "done"]) expect(lines({ stage, claimedBy: A, heldBy: A })).toEqual([]);
  });
  it("adds open vision conflicts (0029) as one more line, at any stage, and nothing without them", () => {
    expect(lines({ stage: "shaping", visionOpen: 2, visionIds: ["V1", "V2"] })).toEqual([["card-vision", "⚠ 2 vision"]]);
    expect(lines({ ...building, visionOpen: 1, visionIds: ["V3"] })).toEqual([
      ["card-who", "wt-a · wt-0030 · 7b5e8856 · started 2 h ago"],
      ["card-vision", "⚠ 1 vision"],
    ]);
    for (const visionOpen of [0, null, undefined]) expect(lines({ stage: "shaping", visionOpen })).toEqual([]);
    const html = renderToStaticMarkup(createElement(CardStatus, { t: task({ stage: "shaping", visionOpen: 2 }), now: NOW }));
    expect(html).toContain("can&#x27;t pass from Shaping to Ready");
  });
});
