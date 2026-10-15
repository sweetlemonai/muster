import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { stamp } from "./lib";
import type { SessionRow, Work } from "./types";
import { WorkView } from "./Work";

const tokens = (input: number, output: number, cacheRead: number, cacheWrite: number) => ({ input, output, cacheRead, cacheWrite });
const row = (over: Partial<SessionRow>): SessionRow => ({
  id: "7b5e8856", who: "muster/wt-0025", stages: ["building"], holds: false, found: "read", tokens: tokens(15, 3000, 50000, 5000),
  start: 1_791_835_200, end: 1_791_837_000, minutes: 30, live: false, skippedLines: 0, skippedFiles: 0, sharedWith: 0, ...over,
});
// The core's answer for a task two sessions worked on (work.rs, two_sessions_each_with_stage_times_and_tokens_and_the_totals).
const two: Work = {
  rows: [
    row({}),
    row({ id: "4d517d9d", who: "qa", stages: ["testing"], tokens: tokens(1, 500, 10000, 1000), start: 1_791_845_400, end: 1_791_846_000, minutes: 10, live: true }),
  ],
  totals: { sessions: 2, read: 2, live: 1, shared: 0, tokens: tokens(16, 3500, 60000, 6000), totalTokens: 69516, minutes: 40 },
  estimate: { minutes: 120, tokens: 100000, minutesPct: 33, tokensPct: 70 },
  guess: { pct: 90, basis: "2 of 4 criteria ticked, in Your check" },
};

type Props = Parameters<typeof WorkView>[0];
function view(over: Partial<Props>) {
  const html = renderToStaticMarkup(createElement(WorkView, { work: null, error: "", viewing: null, transcript: null, onView: () => {}, ...over }));
  const text = (h: string) => h.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/\s+/g, " ").trim();
  const rows = [...html.matchAll(/<tr[^>]*>([\s\S]*?)<\/tr>/g)].slice(1).map((m) => [...m[1].matchAll(/<td[^>]*>([\s\S]*?)<\/td>/g)].map((c) => text(c[1])));
  const buttons = [...html.matchAll(/<button([^>]*)>([^<]*)<\/button>/g)].map((m) => `${m[2]}${m[1].includes("disabled") ? " (disabled)" : ""}${m[1].includes('aria-pressed="true"') ? " (on)" : ""}`);
  return { html, text: text(html), rows, buttons };
}

describe("the Work section", () => {
  it("says it's reading while the core reads the transcripts, and shows the core's refusal", () => {
    expect(view({}).text).toBe("Work Reading the sessions' transcripts…");
    expect(view({ error: "no queue named \"x\"" }).text).toContain("no queue named");
  });

  it("lists each session with its stage, start, end, minutes and tokens", () => {
    const v = view({ work: two });
    expect(v.rows).toEqual([
      ["7b5e8856 muster/wt-0025", "Building", stamp(new Date(1_791_835_200_000)), stamp(new Date(1_791_837_000_000)), "30 min", "58k", "View"],
      ["4d517d9d qa", "Testing", stamp(new Date(1_791_845_400_000)), "live", "10 min", "12k", "View"],
    ]);
    expect(v.html).toContain('<tr class="work-live">');
    expect(v.text).toContain("Work 2 sessions · 1 live");
  });

  it("totals against the estimates, and the progress guess labelled as one", () => {
    const v = view({ work: two });
    expect(v.text).toContain("Time 40 min of 2 h (33%) Tokens 70k of 100k (70%)");
    expect(v.text).toContain("input 16 · output 3.5k · cache read 60k · cache write 6k");
    expect(v.text).toContain("About 90% done (a guess from 2 of 4 criteria ticked, in Your check)");
    const none = view({ work: { ...two, estimate: { minutes: 0, tokens: 0, minutesPct: null, tokensPct: null } } });
    expect(none.text).toContain("Time 40 min Tokens 70k input");
  });

  it("a session not on this Mac keeps its row, says so, and can't be viewed", () => {
    const gone = row({ id: "0123abcd", who: "wt-b", stages: [], holds: true, found: "missing", tokens: tokens(0, 0, 0, 0), start: null, end: null, minutes: 0 });
    const big = row({ id: "aaaabbbb", found: "too-big", live: true });
    const v = view({ work: { ...two, rows: [two.rows[0], gone, big], totals: { ...two.totals, sessions: 3, read: 1 } } });
    expect(v.rows[1]).toEqual(["0123abcd wt-b · holds it", "—", "not on this Mac", "View"]);
    expect(v.rows[2]).toEqual(["aaaabbbb muster/wt-0025", "Building", "transcript over 128 MB, not read · live", "View"]);
    expect(v.buttons).toEqual(["View", "View (disabled)", "View (disabled)"]);
    expect(v.text).toContain("2 of 3 sessions not read on this Mac");
  });

  it("a session not stamped with an account Muster reads keeps its row, says why, and can't be viewed (0099)", () => {
    const unread = row({ id: "11111111", who: "wt-c", found: "not-read", tokens: tokens(0, 0, 0, 0), start: null, end: null, minutes: 0 });
    const v = view({ work: { ...two, rows: [two.rows[0], unread], totals: { ...two.totals, read: 1 } } });
    expect(v.rows[1]).toEqual(["11111111 wt-c", "Building", "not read: not stamped with a Claude account Muster reads", "View"]);
    expect(v.text).not.toContain("not on this Mac");
    expect(v.buttons).toEqual(["View", "View (disabled)"]);
  });

  it("with no sessions yet, says so and still guesses from the stage", () => {
    const v = view({ work: { rows: [], totals: { sessions: 0, read: 0, live: 0, shared: 0, tokens: tokens(0, 0, 0, 0), totalTokens: 0, minutes: 0 }, estimate: two.estimate, guess: { pct: 0, basis: "not started (Ready)" } } });
    expect(v.text).toBe("Work No session has worked on it yet: none has claimed it or written to its log with a session id. About 0% done (a guess from not started (Ready))");
  });
});

describe("the transcript view", () => {
  it("opens in the section, below the rows, with its row's View pressed", () => {
    const v = view({ work: two, viewing: "4d517d9d", transcript: null });
    expect(v.buttons).toEqual(["View", "View (on)", "Close"]);
    expect(v.text).toContain("Session 4d517d9d read on this Mac, not sent anywhere Close Reading the transcript…");
    // An answer for another session is still loading for this one.
    expect(view({ work: two, viewing: "4d517d9d", transcript: { id: "7b5e8856", md: "x" } }).text).toContain("Reading the transcript…");
  });

  it("shows the transcript as Markdown, with its text as written: no HTML, links or images from it", () => {
    // What the core sends (transcripts.rs escapes every ASCII punctuation character), plus raw HTML
    // in case anything ever reaches the view unescaped.
    const md = "**Session 7b5e8856**, folded.\n\n- **You:** \\<b\\>hi\\<\\/b\\> \\!\\[x\\]\\(https\\:\\/\\/e\\.example\\/i\\.png\\)\n- **Tool:** Bash · List files\n- <script>alert(1)</script> <img src=x onerror=alert(1)>\n";
    const v = view({ work: two, viewing: "7b5e8856", transcript: { id: "7b5e8856", md } });
    const pane = /<div class="transcript"[\s\S]*$/.exec(v.html)?.[0] ?? "";
    expect(pane).toContain('<div class="markdown">');
    expect(pane).toContain("<li><strong>You:</strong> &lt;b&gt;hi&lt;/b&gt; ![x](https://e.example/i.png)</li>");
    expect(pane).toContain("<li><strong>Tool:</strong> Bash · List files</li>");
    expect(pane).not.toMatch(/<(img|a|script|b)[\s>]/);
  });

  it("shows why a transcript can't be read", () => {
    const v = view({ work: two, viewing: "7b5e8856", transcript: { id: "7b5e8856", error: "not on this Mac" } });
    expect(v.text).toContain("Close not on this Mac");
  });
});

describe("sessions shared with other tasks (review)", () => {
  it("says so on the row and the totals, so a shared cost isn't read as this task's", () => {
    const shared: Work = { ...two, rows: [row({ sharedWith: 3 }), two.rows[1]], totals: { ...two.totals, shared: 1 } };
    const v = view({ work: shared });
    expect(v.text).toContain("muster/wt-0025 · also on 3 other tasks");
    expect(v.text).toContain("Includes a session that also worked on other tasks, counted in full here");
    expect(view({ work: two }).text).not.toContain("also worked on other tasks");
  });
  it("marks a live row with its own class, not the header's right-aligned one", () => {
    const html = view({ work: two }).html;
    expect(html).toContain('<tr class="work-live">');
    expect(html).not.toMatch(/<tr class="live">/);
  });
});
