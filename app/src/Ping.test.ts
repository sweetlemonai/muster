import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import CardStatus from "./CardStatus";
import { CardPing, MessageCount, QuietPing, SessionPing } from "./Ping";
import { askedText, cantAsk, MAX_MESSAGE, messageLength, NO_SESSION, quietToAsk, unreachable } from "./reach";
import type { Task } from "./types";

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0022", title: "T", kind: "code", priority: "normal", status: "doing", stage: "building", blocked: false, stageHistory: [],
  folder: "doing", path: "doing/0022-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "building", ...over,
});

/** The wall clock here reading `s`, as the state's `now` (seconds since 1970). */
const at = (s: string) => {
  const [y, mo, d, h, mi] = s.split(/[- :]/).map(Number);
  return new Date(y, mo - 1, d, h, mi).getTime() / 1000;
};
const NOW = at("2026-10-13 10:40");
const A = "wt-a (wt-0030 · session 7b5e8856 · cc-hg)";
/** In Building since 09:00, quiet since 10:00: 40 minutes. */
const quiet = { stage: "building", claimedBy: A, stageSince: "2026-10-13 09:00", heartbeat: "2026-10-13 10:00", modified: NOW };
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/\s+/g, " ").trim();
const buttons = (html: string) => [...html.matchAll(/<button([^>]*)>([^<]*)<\/button>/g)].map((m) => `${m[2]}${m[1].includes("disabled") ? " (disabled)" : ""}`);

describe("who a ping can reach (the core's ping::reach)", () => {
  it("only a holder in Building or Testing whose claim has a session stamp", () => {
    expect(unreachable(task({ claimedBy: A }))).toBe("");
    expect(unreachable(task({ stage: "testing", heldBy: A }))).toBe("");
    // Before stamps, or a session typed by hand: no session to reach.
    expect(unreachable(task({ claimedBy: "wt-a" }))).toBe(NO_SESSION);
    expect(unreachable(task({ claimedBy: "capture (drover/main, session 349bc9c3)" }))).toBe(NO_SESSION);
    expect(NO_SESSION).toBe("this claim has no session to reach; ask its session to re-claim");
    expect(unreachable(task({ claimedBy: "" }))).toBe("nobody holds it");
    expect(unreachable(task({ claimedBy: "owner (session 7b5e8856)" }))).toBe("you hold it yourself");
    expect(unreachable(task({ stage: "ready", claimedBy: A }))).toBe("Ready has no session working on it");
    // Testing's holder is held-by: a claim left over from Building isn't it.
    expect(unreachable(task({ stage: "testing", claimedBy: A }))).toBe("nobody holds it");
  });
  it("a check-in only in Building; in Testing, a message", () => {
    expect(cantAsk(task({ claimedBy: A }))).toBe("");
    expect(cantAsk(task({ stage: "testing", heldBy: A }))).toBe("check-ins are for Building; send a message instead");
    expect(cantAsk(task({ claimedBy: "wt-a" }))).toBe(NO_SESSION);
  });
});

describe("asked for a check-in", () => {
  it("says when, until a check-in of this Building stage arrives", () => {
    expect(askedText(task({ ...quiet, checkinAsked: "2026-10-13 10:38" }), NOW)).toBe("asked 2 min ago");
    expect(askedText(task({ ...quiet, checkinAsked: "2026-10-13 10:40" }), NOW)).toBe("asked just now");
    const checkins = [{ at: "2026-10-13 10:39", who: A, pct: 50, text: "halfway" }];
    expect(askedText(task({ ...quiet, checkinAsked: "2026-10-13 10:38", checkins }), NOW)).toBe("");
    // A check-in from before the ask doesn't answer it; one in the same minute does.
    expect(askedText(task({ ...quiet, checkinAsked: "2026-10-13 10:38", checkins: [{ ...checkins[0], at: "2026-10-13 10:30" }] }), NOW)).toBe("asked 2 min ago");
    expect(askedText(task({ ...quiet, checkinAsked: "2026-10-13 10:39", checkins }), NOW)).toBe("");
  });
  it("is nothing when never asked, asked in an earlier stage, or out of Building", () => {
    expect(askedText(task(quiet), NOW)).toBe("");
    expect(askedText(task({ ...quiet, checkinAsked: "" }), NOW)).toBe("");
    expect(askedText(task({ ...quiet, checkinAsked: "2026-10-13 08:00" }), NOW)).toBe("");
    expect(askedText(task({ ...quiet, stage: "testing", checkinAsked: "2026-10-13 10:38" }), NOW)).toBe("");
  });
  it("replaces the check-in line on the card, and the quiet flag stays", () => {
    const checkins = [{ at: "2026-10-13 10:00", who: A, pct: 50, text: "halfway" }];
    const html = renderToStaticMarkup(createElement(CardStatus, { t: task({ ...quiet, checkins, checkinAsked: "2026-10-13 10:38" }), now: NOW }));
    expect(text(html)).toBe("wt-a · wt-0030 · 7b5e8856 · started 1 h ago asked 2 min ago no check-in for 40 min");
    expect(html).toContain('<span class="card-asked">asked 2 min ago</span>');
    expect(html).not.toContain("halfway");
  });
});

describe("the card's Ask for a check-in", () => {
  const card = (over: Partial<Task>) => renderToStaticMarkup(createElement(CardPing, { t: task(over), now: NOW }));
  it("is on a quiet card whose holder can be reached", () => {
    const html = card(quiet);
    expect(buttons(html)).toEqual(["Ask for a check-in"]);
    expect(html).toContain('data-part="card-ping"');
  });
  it("without a session stamp it's shown disabled, with the reason, never hidden", () => {
    const html = card({ ...quiet, claimedBy: "wt-a" });
    expect(buttons(html)).toEqual(["Ask for a check-in (disabled)"]);
    expect(text(html)).toContain(NO_SESSION);
  });
  it("isn't on a card that isn't quiet", () => {
    expect(card({ ...quiet, heartbeat: "2026-10-13 10:30" })).toBe("");
    expect(card({ ...quiet, blocked: true, waitingOn: "owner" })).toBe("");
    expect(card({ ...quiet, stage: "ready" })).toBe("");
  });
});

describe("the task pane's session strip", () => {
  const strip = (over: Partial<Task>) => renderToStaticMarkup(createElement(SessionPing, { task: task(over), now: NOW, onChanged: () => {} }));
  it("in Building: ask for a check-in, and a one-line message box capped at 500", () => {
    const html = strip({ ...quiet, checkinAsked: "2026-10-13 10:38" });
    expect(buttons(html)).toEqual(["Ask for a check-in", "Send (disabled)"]);
    expect(html).toContain('placeholder="Message the session working on this"');
    // No maxLength: it counts UTF-16 units and would cut a paste silently. A live count instead.
    expect(html).not.toContain("maxLength");
    expect(html).toContain('<span class="ping-count">0/500</span>');
    expect(text(html)).toContain("asked 2 min ago");
    expect(text(html)).toContain("It reaches session 7b5e8856 after its next tool call, quoted as your note.");
  });
  it("without a session stamp both are disabled with the reason", () => {
    const html = strip({ ...quiet, claimedBy: "wt-a" });
    expect(buttons(html)).toEqual(["Ask for a check-in (disabled)", "Send (disabled)"]);
    expect(html).toMatch(/<input[^>]*disabled/);
    expect(text(html)).toContain(NO_SESSION);
  });
  it("in Testing: a message reaches the holder; a check-in says why it can't", () => {
    const html = strip({ stage: "testing", heldBy: A });
    expect(buttons(html)).toEqual(["Ask for a check-in (disabled)", "Send (disabled)"]);
    expect(html).not.toMatch(/<input[^>]*disabled/);
    expect(text(html)).toContain("check-ins are for Building; send a message instead");
  });
  it("is only where a session works: Building and Testing, an agent's task the owner doesn't hold", () => {
    for (const stage of ["backlog", "shaping", "ready", "your-check", "done"]) expect(strip({ ...quiet, stage })).toBe("");
    expect(strip({ ...quiet, forWho: "human", claimedBy: "owner" })).toBe("");
    expect(strip({ ...quiet, claimedBy: "owner (session 7b5e8856)" })).toBe("");
    // Nobody on it in Building (a claim released by hand): shown, with why it's off.
    expect(text(strip({ ...quiet, claimedBy: "" }))).toContain("nobody holds it");
  });
});

describe("the message's length, counted as the core counts it (characters, not UTF-16 units)", () => {
  it("counts code points of what goes out: control characters out, trimmed", () => {
    expect(messageLength("")).toBe(0);
    expect(messageLength("  hi\u0007 ")).toBe(2);
    expect(messageLength("😀".repeat(MAX_MESSAGE))).toBe(500);
    expect("😀".repeat(MAX_MESSAGE).length).toBe(1000);
    expect(messageLength("é".repeat(MAX_MESSAGE) + "x")).toBe(501);
    // A line or paragraph separator goes out as a line break: one character.
    expect(messageLength("a\u2028b")).toBe(3);
  });
  it("shows N/500, in the error colour past it, and Send says why it's off", () => {
    const count = (text: string) => renderToStaticMarkup(createElement(MessageCount, { text }));
    expect(count("😀".repeat(MAX_MESSAGE))).toBe('<span class="ping-count">500/500</span>');
    expect(count("😀".repeat(MAX_MESSAGE + 1))).toBe('<span class="ping-count err" title="At most 500 characters: shorten it to send">501/500</span>');
  });
});

describe("Ask all quiet sessions for a check-in (a stalled project)", () => {
  const quietA = task({ ...quiet, id: "0022" });
  const quietB = task({ ...quiet, id: "0024", claimedBy: "wt-b (session aaaa1111)" });
  const unstamped = task({ ...quiet, id: "0025", claimedBy: "wt-c" });
  const live = task({ ...quiet, id: "0026", heartbeat: "2026-10-13 10:35" });
  it("asks each quiet claim it can reach", () => {
    const q = quietToAsk([quietA, quietB, unstamped, live, task({ stage: "done" })], NOW);
    expect(q.quiet.map((t) => t.id)).toEqual(["0022", "0024", "0025"]);
    expect(q.reachable.map((t) => t.id)).toEqual(["0022", "0024"]);
  });
  it("the button says how many, and is disabled with the reason when none can be reached", () => {
    const ok = renderToStaticMarkup(createElement(QuietPing, { tasks: [quietA, quietB, unstamped], now: NOW }));
    expect(buttons(ok)).toEqual(["Ask all quiet sessions for a check-in (2)"]);
    expect(text(ok)).toContain("1 of them has no session to reach");
    const none = renderToStaticMarkup(createElement(QuietPing, { tasks: [unstamped], now: NOW }));
    expect(buttons(none)).toEqual(["Ask all quiet sessions for a check-in (disabled)"]);
    expect(text(none)).toContain("none of its quiet claims has a session to reach");
  });
});
