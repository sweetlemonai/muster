import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";
import { setLocale } from "./i18n/runtime";
import { shortTokens } from "./lib";
import type { SessionRow, StageRow, StageSession, Tokens, Work } from "./types";
import css from "./styles.css?raw";
import { WorkView } from "./Work";

const tokens = (input: number, output: number, cacheRead: number, cacheWrite: number) => ({ input, output, cacheRead, cacheWrite });
const NONE = tokens(0, 0, 0, 0);
const counted = (t: Tokens) => t.input + t.output + t.cacheWrite;
const all = (t: Tokens) => t.input + t.output + t.cacheRead + t.cacheWrite;
const add = (...ts: Tokens[]) => ts.reduce((a, t) => tokens(a.input + t.input, a.output + t.output, a.cacheRead + t.cacheRead, a.cacheWrite + t.cacheWrite), NONE);
const row = (over: Partial<SessionRow>): SessionRow => ({
  id: "7b5e8856", who: "muster/wt-0025", stages: ["building"], holds: false, found: "read", tokens: tokens(15, 3000, 50000, 5000),
  start: 1_791_835_200, end: 1_791_837_000, minutes: 30, live: false, skippedLines: 0, skippedFiles: 0, sharedWith: 0, ...over,
});
const inStage = (id: string, t: Tokens, minutes: number, shared = false): StageSession => ({ id, tokens: t, countedTokens: counted(t), minutes, shared });
/** A stage row as the core sums it: its own sessions' tokens, the shared ones apart. */
function stage(name: StageRow["stage"], minutes: number, sessions: StageSession[]): StageRow {
  const own = add(...sessions.filter((s) => !s.shared).map((s) => s.tokens));
  return { stage: name, minutes, tokens: own, countedTokens: counted(own), sharedTokens: add(...sessions.filter((s) => s.shared).map((s) => s.tokens)), sessions };
}

// Hand-made view inputs, shaped like the core's answer (work.rs) but with figures of their own: a
// task two sessions worked on, modelled on two_sessions_each_with_stage_times_and_tokens_and_the_totals.
// Since mu-172 the tokens against the estimate are the counted ones (input, output and cache
// writes: 16 + 3,500 + 6,000), and the time is the agent time: Building + Testing.
const two: Work = {
  rows: [
    row({}),
    row({ id: "4d517d9d", who: "qa", stages: ["testing"], tokens: tokens(1, 500, 10000, 1000), start: 1_791_845_400, end: 1_791_846_000, minutes: 10, live: true }),
  ],
  stages: [stage("building", 30, [inStage("7b5e8856", tokens(15, 3000, 50000, 5000), 30)]), stage("testing", 10, [inStage("4d517d9d", tokens(1, 500, 10000, 1000), 10)])],
  totals: { sessions: 2, read: 2, live: 1, shared: 0, tokens: tokens(16, 3500, 60000, 6000), totalTokens: 69516, minutes: 40, countedTokens: 9516, sharedTokens: NONE, sharedRead: 0, agentMinutes: 40 },
  estimate: { minutes: 120, tokens: 100000, minutesPct: 33, tokensPct: 10 },
  guess: { pct: 90, basis: "2 of 4 criteria ticked, in Your check" },
};

// R1, modelled on the core's STAGED fixture (work.rs) with figures of its own: Shaping 09:00 → Ready 09:30 → Building 10:00 → Testing 12:00 →
// Your check 13:00, blocked 11:00–11:30. S (own) has turns in Shaping and Building, B (own) in
// Building and Testing, L (another task's claim names it) in Testing.
const S1 = tokens(1000, 20_000, 300_000, 100_000);
const S2 = tokens(2000, 200_000, 5_000_000, 400_000);
const B1 = tokens(1000, 300_000, 20_000_000, 500_000);
const B2 = tokens(500, 100_000, 15_000_000, 200_000);
const L = tokens(8000, 250_000, 124_000_000, 2_000_000);
const own = add(S1, S2, B1, B2);
const r1: Work = {
  rows: [
    row({ id: "5e55e55e", who: "shapour", stages: ["shaping", "building"], tokens: add(S1, S2) }),
    row({ id: "b0b0b0b0", who: "asghar_33", stages: ["building", "testing"], holds: true, tokens: add(B1, B2) }),
    row({ id: "1e1e1e1e", who: "muster-lead", stages: [], tokens: L, sharedWith: 1 }),
  ],
  stages: [
    stage("shaping", 30, [inStage("5e55e55e", S1, 15)]),
    stage("building", 90, [inStage("5e55e55e", S2, 100), inStage("b0b0b0b0", B1, 110)]),
    stage("testing", 60, [inStage("b0b0b0b0", B2, 50), inStage("1e1e1e1e", L, 45, true)]),
    stage("your-check", 45, []),
  ],
  totals: { sessions: 3, read: 3, live: 0, shared: 1, tokens: own, totalTokens: all(own), minutes: 0, countedTokens: counted(own), sharedTokens: L, sharedRead: 1, agentMinutes: 150 },
  estimate: { minutes: 480, tokens: 1_500_000, minutesPct: 31, tokensPct: 122 },
  guess: { pct: 90, basis: "in Your check" },
};

afterEach(async () => {
  await setLocale("en");
});

type Props = Parameters<typeof WorkView>[0];
const strip = (h: string) => h.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/[\u2066-\u2069]/g, "").replace(/\s+/g, " ").trim();
function view(over: Partial<Props>) {
  const html = renderToStaticMarkup(createElement(WorkView, { work: null, error: "", viewing: null, transcript: null, onView: () => {}, ...over }));
  // Every body and foot row of the table, as its cells' text.
  const rows = [...html.matchAll(/<tr[^>]*>([\s\S]*?)<\/tr>/g)].slice(1).map((m) => [...m[1].matchAll(/<t([hd])[^>]*>([\s\S]*?)<\/t\1>/g)].map((c) => strip(c[2])));
  const buttons = [...html.matchAll(/<button([^>]*)>([^<]*)<\/button>/g)].map((m) => `${m[2]}${m[1].includes("disabled") ? " (disabled)" : ""}${m[1].includes('aria-pressed="true"') ? " (on)" : ""}`);
  return { html, text: strip(html), rows, buttons, total: rows[rows.length - 1]?.[0] ?? "" };
}

describe("the Work section as a table by stage (mu-172 round 2)", () => {
  it("R1: a row per stage with its counted tokens, cache reads and time, its sessions under it, then the total", () => {
    const v = view({ work: r1 });
    expect(v.rows).toEqual([
      ["Shaping", "121k + 300k cache reads", "30 min", ""],
      ["5e55e55e shapour", "121k + 300k cache reads", "15 min", "View"],
      // S's turns split by their time: Shaping's above, Building's here.
      ["Building", "1.4M + 25M cache reads", "1.5 h", ""],
      ["5e55e55e shapour", "602k + 5M cache reads", "1.7 h", "View"],
      ["b0b0b0b0 asghar_33 · holds it", "801k + 20M cache reads", "1.8 h", "View"],
      // The shared session is listed with what it spent here, and kept out of the row's count.
      ["Testing", "301k + 15M cache reads · + 2.3M in shared sessions, not counted", "1 h", ""],
      ["b0b0b0b0 asghar_33 · holds it", "301k + 15M cache reads", "50 min", "View"],
      ["1e1e1e1e muster-lead · shared with 1 other task, not counted", "2.3M + 124M cache reads", "45 min", "View"],
      // Your check is the owner's wait: on its own row, never in the total.
      ["Your check", "—", "45 min, your wait", ""],
      ["Total: 1.8M of 1.5M tokens (122%) · 2.5 h of 8 h (31%) + 40.3M cache reads, not counted · + 2.3M in shared sessions, not counted · time is agent time (Building + Testing); Your check is your wait, not in it"],
    ]);
    // The total is the own rows summed: what the card's ribbon says.
    expect(r1.stages.reduce((n, s) => n + s.countedTokens, 0)).toBe(r1.totals.countedTokens);
  });

  it("names each stage as its group's row header, so a screen reader says a session's stage with its row (review R2)", () => {
    const ready = tokens(10, 5000, 90_000, 1000);
    const { html } = view({ work: { ...r1, rows: [...r1.rows, row({ id: "0d0d0d0d", who: "gholi", stages: [], found: "missing" })], stages: [...r1.stages, stage("other", 0, [inStage("5e55e55e", ready, 3)])] } });
    const heads = [...html.matchAll(/<th scope="rowgroup">([^<]*)<\/th>/g)].map((m) => m[1]);
    expect(heads).toEqual(["Shaping", "Building", "Testing", "Your check", "Other (Backlog, Ready, Done)", "No tokens by stage"]);
    // Each stage is its own row group; a session's row is in its stage's.
    expect(html.match(/<tbody class="work-stage">/g)).toHaveLength(heads.length);
    // The column headers say they head columns, the stage's included.
    const thead = /<thead>([\s\S]*?)<\/thead>/.exec(html)?.[1] ?? "";
    expect(thead.match(/<th scope="col"/g)).toHaveLength(4);
    expect(thead).not.toMatch(/<th(?! scope="col")/);
  });

  it("R2: what fell outside the four stages is its own row", () => {
    const ready = tokens(10, 5000, 90_000, 1000);
    const v = view({ work: { ...r1, stages: [...r1.stages, stage("other", 0, [inStage("5e55e55e", ready, 3)])] } });
    expect(v.rows.slice(-3, -1)).toEqual([
      ["Other (Backlog, Ready, Done)", "6k + 90k cache reads", "0 min", ""],
      ["5e55e55e shapour", "6k + 90k cache reads", "3 min", "View"],
    ]);
  });

  it("times in hours, never days; a dash before any agent time; the actual alone without an estimate", () => {
    const long = view({ work: { ...two, stages: [stage("building", 1590, [inStage("7b5e8856", tokens(15, 3000, 50000, 5000), 1590)])], totals: { ...two.totals, agentMinutes: 1590 }, estimate: { ...two.estimate, minutes: 480, minutesPct: 331 } } });
    expect(long.rows[0]).toEqual(["Building", "8k + 50k cache reads", "26.5 h", ""]);
    expect(long.total).toMatch(/^Total: 9.5k of 100k tokens \(10%\) · 26.5 h of 8 h \(331%\)/);
    expect(long.text).not.toMatch(/\bd\b/);
    const notYet = view({ work: { ...two, totals: { ...two.totals, agentMinutes: null }, estimate: { ...two.estimate, minutesPct: null } } });
    expect(notYet.total).toMatch(/^Total: 9.5k of 100k tokens \(10%\) · — of 2 h /);
    const none = view({ work: { ...two, totals: { ...two.totals, agentMinutes: null }, estimate: { minutes: 0, tokens: 0, minutesPct: null, tokensPct: null } } });
    expect(none.total).toMatch(/^Total: 9.5k tokens · — /);
    const noCache = view({ work: { ...two, totals: { ...two.totals, tokens: tokens(16, 3500, 0, 6000) } } });
    expect(noCache.total).toBe("Total: 9.5k of 100k tokens (10%) · 40 min of 2 h (33%) time is agent time (Building + Testing); Your check is your wait, not in it");
  });

  it("shows counted tokens on every session line, cache reads apart, never an all-kinds number", () => {
    const v = view({ work: r1 });
    const kinds = [S1, S2, B1, B2, L, add(S1, S2), add(B1, B2), add(S2, B1)].map((t) => shortTokens(all(t)));
    expect(kinds).toContain("5.6M");
    for (const r of v.rows) for (const cell of r) for (const n of kinds) expect(cell.split(/[ ,]/), `${cell} has ${n}`).not.toContain(n);
  });

  it("lists a live session with its mark, and the counts in the header", () => {
    const v = view({ work: two });
    expect(v.rows[3]).toEqual(["4d517d9d qa live", "1.5k + 10k cache reads", "10 min", "View"]);
    expect(v.html).toContain('<tr class="work-session work-live">');
    expect(v.html).not.toMatch(/<tr class="live">/);
    expect(v.text).toContain("Work 2 sessions · 1 live");
  });

  it("keeps the input, output and cache breakdown and the progress guess labelled as one", () => {
    const v = view({ work: r1 });
    expect(v.text).toContain("input 4.5k · output 620k · cache read 40.3M · cache write 1.2M");
    expect(v.text).toContain("About 90% done (a guess from in Your check)");
    expect(v.text).toContain("Not counted: 1 session shared with other tasks, 2.3M tokens. Its share of this task can't be told apart from its transcript.");
  });

  it("the owner's fixture: no more 168.1M of 1.5M, no more 1 d", () => {
    // What the section said before, from the same sessions: every kind of every session.
    const before = r1.rows.reduce((n, r) => n + all(r.tokens), 0);
    expect(Math.round((before / 1_500_000) * 100)).toBeGreaterThan(10_000);
    const v = view({ work: r1 });
    for (const old of ["168.1M", "11204", "1 d", "counted in full"]) expect(v.text).not.toContain(old);
  });
});

describe("sessions with no tokens in any stage", () => {
  it("keep their row under the stages, say why, and can't be viewed when not read", () => {
    const gone = row({ id: "0123abcd", who: "wt-b", stages: [], holds: true, found: "missing", tokens: NONE, start: null, end: null, minutes: 0 });
    const big = row({ id: "aaaabbbb", found: "too-big", live: true });
    const unread = row({ id: "11111111", who: "wt-c", found: "not-read", tokens: NONE, start: null, end: null, minutes: 0 });
    const quiet = row({ id: "22222222", who: "wt-d", tokens: NONE });
    const v = view({ work: { ...two, rows: [...two.rows, gone, big, unread, quiet], totals: { ...two.totals, sessions: 6, read: 3 } } });
    expect(v.rows.slice(4, -1)).toEqual([
      ["No tokens by stage", "", "", ""],
      ["0123abcd wt-b · holds it", "not on this Mac", "View"],
      ["aaaabbbb muster/wt-0025 live", "transcript over 128 MB, not read", "View"],
      // Not read by policy (decision 0012): no View at all, its last cell kept empty (mu-162).
      ["11111111 wt-c", "not read: not stamped with a Claude account Muster reads", ""],
      ["22222222 wt-d", "no tokens read", "View"],
    ]);
    expect(v.buttons).toEqual(["View", "View", "View (disabled)", "View (disabled)", "View"]);
    expect(v.text).toContain("3 of 6 sessions not read on this Mac");
  });

  it("a session Muster won't read has no View, even when it's the one being viewed (mu-162)", () => {
    const gone = row({ id: "0123abcd", who: "wt-b", stages: [], found: "missing", tokens: NONE, start: null, end: null, minutes: 0 });
    const unread = row({ id: "11111111", who: "wt-c", stages: [], found: "not-read", tokens: NONE, start: null, end: null, minutes: 0 });
    const work = { ...two, rows: [...two.rows, gone, unread], totals: { ...two.totals, sessions: 4, read: 2 } };
    const v = view({ work });
    // Read rows keep View, the missing one its disabled View, and the not-read one has none.
    expect(v.buttons).toEqual(["View", "View", "View (disabled)"]);
    // Its row still ends in the action column's cell, empty, like the missing row's with its
    // button: the columns line up and nothing moves.
    const unreadRow = /<tr[^>]*>(?:(?!<\/tr>)[\s\S])*?11111111[\s\S]*?<\/tr>/.exec(v.html)?.[0] ?? "";
    const goneRow = /<tr[^>]*>(?:(?!<\/tr>)[\s\S])*?0123abcd[\s\S]*?<\/tr>/.exec(v.html)?.[0] ?? "";
    expect(unreadRow).toMatch(/<td><\/td><\/tr>$/);
    expect(unreadRow).not.toContain("<button");
    const cells = (h: string) => [...h.matchAll(/<td(?: colSpan="(\d)")?[^>]*>/gi)].reduce((n, m) => n + Number(m[1] ?? 1), 0);
    expect(cells(unreadRow)).toBe(4);
    expect(cells(goneRow)).toBe(4);
    // A stale `viewing` on it opens nothing: no transcript pane, no Close, no pressed View.
    const stale = view({ work, viewing: "11111111", transcript: { id: "11111111", error: "not read" } });
    expect(stale.buttons).toEqual(["View", "View", "View (disabled)"]);
    expect(stale.html).not.toContain('class="transcript"');
    // A missing session's pane still opens and says why it can't be read: that difference stays.
    const missing = view({ work, viewing: "0123abcd", transcript: { id: "0123abcd", error: "not on this Mac" } });
    expect(missing.buttons).toEqual(["View", "View", "View (disabled) (on)", "Close"]);
    expect(missing.text).toContain("Close not on this Mac");
  });

  it("with no sessions yet, says so and still guesses from the stage", () => {
    const totals = { sessions: 0, read: 0, live: 0, shared: 0, tokens: NONE, totalTokens: 0, minutes: 0, countedTokens: 0, sharedTokens: NONE, sharedRead: 0, agentMinutes: null };
    const v = view({ work: { rows: [], stages: [], totals, estimate: two.estimate, guess: { pct: 0, basis: "not started (Ready)" } } });
    expect(v.text).toBe("Work No session has worked on it yet: none has claimed it or written to its log with a session id. About 0% done (a guess from not started (Ready))");
  });

  it("says it's reading while the core reads the transcripts, and shows the core's refusal", () => {
    expect(view({}).text).toBe("Work Reading the sessions' transcripts…");
    expect(view({ error: "no queue named \"x\"" }).text).toContain("no queue named");
  });
});

describe("the transcript view", () => {
  it("opens in the section, below the table, with its session's View pressed wherever it's listed", () => {
    const v = view({ work: r1, viewing: "5e55e55e", transcript: null });
    expect(v.buttons).toEqual(["View (on)", "View (on)", "View", "View", "View", "Close"]);
    expect(v.text).toContain("Session 5e55e55e read on this Mac, not sent anywhere Close Reading the transcript…");
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

describe("sessions shared with other tasks (review; mu-172)", () => {
  it("say so where they're listed, and the totals say what they used, kept out", () => {
    expect(view({ work: { ...r1, rows: r1.rows.map((r) => (r.id === "1e1e1e1e" ? { ...r, sharedWith: 3 } : r)) } }).text).toContain("muster-lead · shared with 3 other tasks, not counted");
    const shared: Work = { ...two, totals: { ...two.totals, shared: 2, sharedRead: 1, sharedTokens: tokens(15, 3000, 50000, 5000) } };
    expect(view({ work: shared }).text).toContain("Not counted: 2 sessions shared with other tasks, 8k tokens. Their share");
    // None of them read: no tokens to tell, and nothing to add to the total.
    const unread = view({ work: { ...shared, totals: { ...shared.totals, shared: 1, sharedRead: 0, sharedTokens: NONE } } });
    expect(unread.text).toContain("Not counted: 1 session shared with other tasks. Its share of this task can't be told apart from its transcript.");
    expect(unread.total).not.toContain("shared sessions");
    expect(view({ work: two }).text).not.toContain("shared");
  });
});

describe("the Work table right to left (review round 3)", () => {
  it("aligns by the text's start and end, never left or right, so Persian headings sit over their columns", () => {
    const rules = [...css.matchAll(/\n([^{}\n]*\.(?:work|transcript)[-\w]*[^{}\n]*)\{([^}]*)\}/g)].map((m) => [m[1].trim(), m[2]] as const);
    expect(rules.length).toBeGreaterThan(5);
    expect(rules.filter(([, body]) => /text-align: (left|right)|\b(margin|padding)-(left|right)|(^|[\s;])(left|right):/.test(body)).map(([sel]) => sel)).toEqual([]);
    expect(rules.find(([sel]) => sel === ".work-rows th")?.[1]).toContain("text-align: start;");
    expect(rules.find(([sel]) => sel === ".work-rows td:last-child")?.[1]).toContain("text-align: end;");
  });
});

describe("the Work section in Persian (mu-172)", () => {
  it("says it all in Persian, with Persian digits, the figures isolated so they never flip", async () => {
    await setLocale("fa");
    const v = view({ work: r1, viewing: "5e55e55e", transcript: null });
    expect(v.rows[2]).toEqual(["در حال ساخت", "۱٫۴M + ۲۵M خواندن از کش", "۱٫۵ ساعت", ""]);
    expect(v.rows[8]).toEqual(["بررسی شما", "—", "۴۵ دقیقه، انتظار شما", ""]);
    expect(v.total).toMatch(/^مجموع: ۱٫۸M از ۱٫۵M توکن \(۱۲۲٪\) · ۲٫۵ ساعت از ۸ ساعت \(۳۱٪\)/);
    // Each figure filled into a Persian sentence keeps its own left-to-right run.
    expect(v.html).toContain("\u2068۱٫۸M\u2069 از \u2068۱٫۵M\u2069");
    // Every word left in Latin letters is data: session ids, who, the core's guess basis (mu-153).
    const left = v.text.replace(/\b(5e55e55e|b0b0b0b0|1e1e1e1e|shapour|asghar_33|muster-lead|in Your check)\b/g, "").match(/[a-z]{2,}/gi) ?? [];
    expect(left).toEqual([]);
    expect(view({}).text).toBe("کار در حال خواندن رونوشت‌های نشست‌ها…");
  });
});
