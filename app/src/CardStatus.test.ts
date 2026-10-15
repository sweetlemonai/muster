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
  // A line's text, a reason isolated in <bdi> included.
  return [...html.matchAll(/<span class="([^"]+)"[^>]*>((?:[^<]|<\/?bdi>)*)<\/span>/g)].map((m) => [m[1], m[2].replace(/<\/?bdi>/g, "").replace(/&#x27;/g, "'")]);
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
  it("a task sent back from Your check says who sent it and why, first in line (mu-101)", () => {
    const base = { ...building, prefix: "mu", id: "0082", sentBack: true };
    expect(lines({ ...base, sentBackBy: "owner", sentBackWhy: "the icon is bad" })).toEqual([
      ["card-who", "wt-a · wt-0030 · 7b5e8856 · started 2 h ago"],
      ["card-first", "fix mu-82 first (sent back by you)"],
      ["card-first card-why", "Sent back by you: the icon is bad"],
    ]);
    // An agent relaying it is named; behind another send-back, it still says why.
    expect(lines({ ...base, sentBackBy: "lead", sentBackWhy: "AC2 fails", behind: "0059" }).slice(1)).toEqual([
      ["card-behind", "waiting behind mu-59 (sent back by the review)"],
      ["card-first card-why", "Sent back by lead: AC2 fails"],
    ]);
    // The holder's other task names who sent the first one back.
    expect(lines({ ...building, prefix: "mu", id: "0085", behind: "0082", behindBy: "owner" }).slice(1)).toEqual([
      ["card-behind", "waiting behind mu-82 (sent back by you)"],
    ]);
    expect(lines({ ...building, prefix: "mu", id: "0085", behind: "0082", behindBy: "lead" }).slice(1)).toEqual([
      ["card-behind", "waiting behind mu-82 (sent back by lead)"],
    ]);
    // A Persian reason reads right to left, isolated from the English before it, so its full stop
    // stays at its end; a long one is cut with the whole of it in the tooltip.
    const html = renderToStaticMarkup(createElement(CardStatus, { t: task({ ...base, sentBackBy: "owner", sentBackWhy: "آیکون بد است." }), now: NOW }));
    expect(html).toContain('<span class="card-first card-why" title="Sent back by you: آیکون بد است.">Sent back by you: <bdi>آیکون بد است.</bdi></span>');
    // Blocked, it isn't first: neither line.
    expect(lines({ ...base, sentBackBy: "owner", sentBackWhy: "x", blocked: true, waitingOn: "owner" }).map(([c]) => c)).toEqual(["card-who"]);
    // Every look keeps both.
    for (const look of ["brief", "status"] as const) {
      const out = renderToStaticMarkup(createElement(CardStatus, { t: task({ ...base, sentBackBy: "owner", sentBackWhy: "x" }), now: NOW, lines: look }));
      expect(out, look).toContain("fix mu-82 first (sent back by you)");
      expect(out, look).toContain("Sent back by you: <bdi>x</bdi>");
    }
    // The why line shares the first-in-line token: no colour of its own.
    expect(css).not.toMatch(/\.card-why[^{]*\{[^}]*color/);
  });
  it("in Ready says nobody is on it yet, and Backlog, Done and an unheld Shaping say nothing", () => {
    expect(lines({ stage: "ready" })).toEqual([["card-who nobody", "nobody on it"]]);
    expect(lines({ stage: "ready", claimedBy: "wt-b" })).toEqual([["card-who", "wt-b"]]);
    for (const stage of ["backlog", "done"]) expect(lines({ stage, claimedBy: A, heldBy: A })).toEqual([]);
    expect(lines({ stage: "shaping", heldBy: A })).toEqual([]);
  });
  it("in Shaping, held (0097): who and the lease, as Building's claim says them", () => {
    const shaping = { stage: "shaping", claimedBy: A, heartbeat: "2026-10-12 13:50", modified: NOW };
    expect(lines(shaping)).toEqual([["card-who", "wt-a · wt-0030 · 7b5e8856"]]);
    expect(lines({ ...shaping, heartbeat: "2026-10-12 01:00" })).toEqual([
      ["card-who", "wt-a · wt-0030 · 7b5e8856"],
      ["card-quiet", "quiet 12 h+"],
    ]);
    // Beside the ribbon that names the shaper, only the rest.
    const html = renderToStaticMarkup(createElement(CardStatus, { t: task(shaping), now: NOW, named: true }));
    expect(html).toBe('<span class="card-who">wt-0030 · 7b5e8856</span>');
  });
  it("beside a ribbon that names the holder (0097), keeps the rest of the who line and the flags", () => {
    const named = (over: Partial<Task>, look: "all" | "brief" | "status" = "all") => {
      const html = renderToStaticMarkup(createElement(CardStatus, { t: task(over), now: NOW, lines: look, named: true }));
      return [...html.matchAll(/<span class="([^"]+)"[^>]*>([^<]*)<\/span>/g)].map((m) => [m[1], m[2]]);
    };
    expect(named({ stage: "building", claimedBy: A, stageSince: "2026-10-12 12:00", heartbeat: "2026-10-12 01:00" })).toEqual([
      ["card-who", "wt-0030 · 7b5e8856 · started 2 h ago"],
      ["card-quiet", "quiet 12 h+ · no check-in for 13 h"],
    ]);
    expect(named({ stage: "testing", heldBy: "owner", stageSince: "2026-10-12 13:00" }, "status")).toEqual([["card-who", "started 1 h ago"]]);
    // Nothing left to say: no line, never "nobody on it".
    expect(named({ stage: "testing", heldBy: "qa" })).toEqual([]);
    expect(named({ stage: "testing", heldBy: A, builtBy: A })).toEqual([
      ["card-who", "wt-0030 · 7b5e8856"],
      ["card-tester", "needs a tester"],
    ]);
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

describe("the card in Your check says what you're accepting (mu-78)", () => {
  const proven = (n: number) => ({ n, label: `AC${n}`, text: `AC${n}: works — verify: command`, ticked: true, owner: false, proof: "ok", proofFiles: [] });
  const summary = "A chat example the reader steps through with Next or the keyboard; works offline; under 1 KB";
  it("says All N proven: Accept, with the summary on the next line, whole in its tooltip", () => {
    const over = { stage: "your-check", heldBy: "owner", criteria: [1, 2, 3, 4].map(proven), summary };
    expect(lines(over)).toEqual([
      ["card-who your-turn", "Your turn"],
      ["card-next", "All 4 proven: Accept"],
      ["card-summary", summary],
    ]);
    const html = renderToStaticMarkup(createElement(CardStatus, { t: task(over), now: NOW }));
    expect(html).toContain(`<span class="card-summary" dir="auto" title="${summary}">`);
    // One line, cut to the card's width; the tooltip has all of it.
    const rule = [...css.matchAll(/\.card-summary[^{]*\{([^}]*)\}/g)].map((m) => m[1]).join(";");
    expect(rule).toMatch(/white-space: nowrap/);
    expect(rule).toMatch(/text-overflow: ellipsis/);
    expect(rule).toMatch(/overflow: hidden/);
  });
  it("keeps the next step when something of yours is open, the summary under it", () => {
    const open = { n: 2, label: "AC2", text: "AC2: the owner opens the board. — verify: owner", ticked: false, owner: true };
    expect(lines({ stage: "your-check", heldBy: "owner", criteria: [proven(1), open], ownerCriteria: [open], summary: "Built it" })).toEqual([
      ["card-who your-turn", "Your turn"],
      ["card-next", "Try: open the board"],
      ["card-summary", "Built it"],
    ]);
  });
  it("adds no line without a summary, and only in Your check", () => {
    expect(lines({ stage: "your-check", heldBy: "owner", criteria: [proven(1)] }).map((l) => l[0])).toEqual(["card-who your-turn", "card-next"]);
    expect(lines({ stage: "testing", heldBy: "qa", summary: "Built it" }).map((l) => l[0])).toEqual(["card-who"]);
  });
  it("shows the summary as text: markup in it stays literal", () => {
    const html = renderToStaticMarkup(createElement(CardStatus, { t: task({ stage: "your-check", summary: "a <b>bold</b> claim" }), now: NOW }));
    expect(html).toContain("a &lt;b&gt;bold&lt;/b&gt; claim");
  });
});
