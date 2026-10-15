// The Sessions view (0098): grouping as Claude Code's agents view does, the header count, empty
// groups, the Older chip, All versus one project, the keys and the rows' names, Focus's chips, and
// the recap's sections, chain and Transcript button.
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";
import { fastest } from "./fastest";
import { setLocale } from "./i18n/runtime";
import css from "./styles.css?raw";
import { Mode, type DisplayMode } from "./modes";
import { RecapView } from "./SessionRecap";
import { SessionsView } from "./Sessions";
import { Labels } from "./labels";
import { chainLine, groupSessions, headerCount, historyKind, nextKey, OLDER_SECS, rowAge, rowLabel, sessionKey, WORDS } from "./sessionsData";
import type { SessionItem, SessionRecap, SessionsList } from "./types";

const NOW = 1_800_000_000;
const noop = () => {};
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/&quot;/g, '"').replace(/&amp;/g, "&").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/\s+/g, " ").trim();

let n = 0;
const row = (over: Partial<SessionItem> = {}): SessionItem => ({
  account: "mg",
  id: `0000${String(n++).padStart(4, "0")}`,
  name: "Asghar_6",
  group: "completed",
  word: "done",
  message: "Sidebar heading fix committed",
  project: "muster",
  started: NOW - 3600,
  at: NOW - 660,
  ...over,
});
const list = (rows: SessionItem[], over: Partial<SessionsList> = {}): SessionsList => ({ rows, reading: false, problems: [], accounts: ["cc-mg", "cc-hg"], ...over });

const view = (l: SessionsList | null, more: { all?: boolean; selected?: string | null; mode?: DisplayMode; error?: string } = {}) =>
  renderToStaticMarkup(
    <Mode.Provider value={more.mode ?? "adhd"}>
      <SessionsView list={l} error={more.error ?? ""} now={NOW} all={more.all ?? true} selected={more.selected ?? null} onOpen={noop} />
    </Mode.Provider>,
  );

/** The groups' sections, by their class, as text. */
const section = (html: string, id: string) => {
  const m = new RegExp(`<section class="session-group sg-${id}"[\\s\\S]*?</section>`).exec(html);
  return m ? m[0] : "";
};

// The five states the agents view shows, as the core groups them (agents::ListState).
const FIVE = () => [
  row({ name: "Asker", group: "needs-input", word: "blocked", message: "approve Bash: git push", at: NOW - 60 }),
  row({ name: "Builder", group: "working", word: "working", message: "Running the gate", at: NOW - 120 }),
  row({ name: "Done", group: "completed", word: "done", at: NOW - 600 }),
  row({ name: "Stopped", group: "completed", word: "stopped", at: NOW - 900 }),
  row({ name: "Failed", group: "completed", word: "failed", at: NOW - 1200 }),
];

describe("the list's groups and counts (AC1)", () => {
  it("groups the five states as the agents view does, with the header count", () => {
    const g = groupSessions(FIVE(), NOW);
    expect(g["needs-input"].rows.map((r) => r.name)).toEqual(["Asker"]);
    expect(g.working.rows.map((r) => r.name)).toEqual(["Builder"]);
    expect(g.completed.rows.map((r) => r.name)).toEqual(["Done", "Stopped", "Failed"]);
    expect(headerCount(g)).toBe("1 need input · 1 working · 3 completed");
    const html = view(list(FIVE()));
    expect(text(html)).toContain("1 need input · 1 working · 3 completed");
    expect(text(section(html, "needs-input"))).toContain("Asker");
    expect(text(section(html, "working"))).toContain("Builder");
    expect(text(section(html, "completed"))).toMatch(/Done.*Stopped.*Failed/);
  });

  it("keeps the three groups in fixed order, an empty one saying none", () => {
    const html = view(list([row({ group: "working", name: "Only" })]));
    const heads = [...html.matchAll(/<h2 id="sessions-([a-z-]+)">/g)].map((m) => m[1]);
    expect(heads).toEqual(["needs-input", "working", "completed"]);
    expect(text(section(html, "needs-input"))).toContain("none");
    expect(text(section(html, "completed"))).toContain("none");
    expect(text(section(html, "working"))).not.toContain("none");
    expect(text(html)).toContain("0 need input · 1 working · 0 completed");
  });

  it("orders each group newest first, a row with no time last", () => {
    const g = groupSessions([row({ name: "old", group: "working", at: NOW - 500 }), row({ name: "none", group: "working", at: null }), row({ name: "new", group: "working", at: NOW - 5 })], NOW);
    expect(g.working.rows.map((r) => r.name)).toEqual(["new", "old", "none"]);
  });

  it("gives each row its name, message and age, and under All its project, Other for none", () => {
    const html = view(list([row({ name: "Gholi_22", message: "Core done", project: "muster" }), row({ name: "Loose", project: "" })]));
    expect(text(html)).toContain("Gholi_22 muster Core done 11 min ago");
    expect(text(html)).toContain("Loose Other");
    // Per project: no project name on the rows.
    const one = view(list([row({ name: "Gholi_22", message: "Core done" })]), { all: false });
    expect(text(one)).toContain("Gholi_22 Core done 11 min ago");
    expect(one).not.toContain('class="q"');
  });

  it("shows a session with no name by its id", () => {
    const html = view(list([row({ name: "", id: "7b5e8856" })]));
    expect(text(html)).toContain("7b5e8856");
  });

  it("says Reading sessions… in place until the first read lands, with the groups still there", () => {
    for (const l of [null, list([], { reading: true })]) {
      const html = view(l);
      expect(text(html)).toContain(WORDS.reading);
      expect(html.match(/<h2 id="sessions-/g)).toHaveLength(3);
      expect(text(html)).not.toContain("none");
      // No spinner: nothing moves.
      expect(html).not.toMatch(/spinner|animation/);
      // In Focus the chips hide their counts too, as the headings do: no "Working 0".
      const focus = view(l, { mode: "focus" });
      for (const id of ["working", "completed"]) {
        expect(section(focus, id)).toMatch(/<button[^>]*title="Open (Working|Completed) here"[^>]*>(Working|Completed) <\/button>/);
        expect(text(section(focus, id))).not.toMatch(/\d/);
      }
    }
  });

  it("says which accounts it read, and an account it couldn't, and the core's error", () => {
    const html = view(list([], { problems: ["cc-hg: claude isn't installed"] }), { error: "boom" });
    expect(text(html)).toContain("Read from cc-mg and cc-hg on this Mac, sent nowhere.");
    expect(text(html)).toContain("cc-hg: claude isn't installed");
    expect(text(html)).toContain("boom");
    expect(text(view(list([], { accounts: [] })))).toContain(WORDS.noAccounts);
  });
});

describe("the Older chip and the modes (AC5)", () => {
  it("folds Completed older than a day into one labelled chip, in place", () => {
    const rows = [
      row({ name: "recent", at: NOW - 3600 }),
      row({ name: "edge", at: NOW - OLDER_SECS }),
      row({ name: "old1", at: NOW - OLDER_SECS - 1 }),
      row({ name: "old2", at: NOW - 5 * OLDER_SECS }),
      row({ name: "never", at: null }),
      // Only Completed folds: an old Working row stays a row.
      row({ name: "busy", group: "working", at: NOW - 9 * OLDER_SECS }),
    ];
    const g = groupSessions(rows, NOW);
    expect(g.completed.rows.map((r) => r.name)).toEqual(["recent", "edge"]);
    expect(g.completed.older.map((r) => r.name)).toEqual(["old1", "old2", "never"]);
    expect(g.working.rows.map((r) => r.name)).toEqual(["busy"]);
    const html = section(view(list(rows)), "completed");
    expect(html).toMatch(/<button[^>]*aria-expanded="false"[^>]*>Older: 3<\/button>/);
    expect(text(html)).not.toContain("old1");
    // The count counts the older ones too.
    expect(headerCount(g)).toBe("0 need input · 1 working · 5 completed");
  });

  it("in Focus, shows Needs input's rows and Working and Completed as chips with counts", () => {
    const html = view(list(FIVE()), { mode: "focus" });
    expect(text(section(html, "needs-input"))).toContain("Asker");
    for (const [id, label, count] of [["working", "Working", 1], ["completed", "Completed", 3]] as const) {
      const s = section(html, id);
      expect(s).toMatch(new RegExp(`<button[^>]*aria-expanded="false"[^>]*title="Open ${label} here \\(${count}\\)"`));
      expect(s).not.toContain("data-session");
    }
  });

  it("in ADHD and Standard, shows every group's rows", () => {
    for (const mode of ["adhd", "standard"] as const) expect(view(list(FIVE()), { mode }).match(/data-session=/g)).toHaveLength(5);
  });
});

describe("keys, names and plain text (AC6, AC7)", () => {
  it("moves with the arrows, Home and End, stopping at the ends", () => {
    const keys = ["a", "b", "c"];
    expect(nextKey(keys, "a", "ArrowDown")).toBe("b");
    expect(nextKey(keys, "c", "ArrowDown")).toBe("c");
    expect(nextKey(keys, "a", "ArrowUp")).toBe("a");
    expect(nextKey(keys, "b", "ArrowUp")).toBe("a");
    expect(nextKey(keys, "b", "Home")).toBe("a");
    expect(nextKey(keys, "a", "End")).toBe("c");
    expect(nextKey(keys, "gone", "ArrowDown")).toBe("a");
    expect(nextKey(keys, "a", "Enter")).toBeNull();
    expect(nextKey([], "a", "ArrowDown")).toBeNull();
  });

  it("makes each row a button named by name, state in words, age and message", () => {
    const r = row({ name: "Asghar_6", group: "completed", at: NOW - 660, message: "Sidebar heading fix committed" });
    expect(rowLabel(r, NOW)).toBe("Asghar_6, completed, 11 min ago, Sidebar heading fix committed");
    expect(rowLabel(row({ name: "Q", group: "needs-input", at: null, message: "" }), NOW)).toBe("Q, needs input");
    // One project's view: no project in the name; it's the view's.
    const html = view(list([r]), { all: false });
    expect(html).toMatch(/class="session-row completed"[^>]*aria-label="Asghar_6, completed, 11 min ago, Sidebar heading fix committed"/);
    // Under All the name says the project as the row shows it: its label, or Other.
    const all = renderToStaticMarkup(
      <Labels.Provider value={new Map([["muster", "Muster app"]])}>
        <SessionsView list={list([r, row({ name: "Loose", project: "", at: NOW - 660 })])} error="" now={NOW} all={true} selected={null} onOpen={noop} />
      </Labels.Provider>,
    );
    expect(all).toMatch(/aria-label="Asghar_6, Muster app, completed, 11 min ago, Sidebar heading fix committed"/);
    expect(all).toMatch(/aria-label="Loose, Other, completed, 11 min ago, Sidebar heading fix committed"/);
    // Each group is a list under its heading.
    expect(html).toMatch(/<ul class="session-rows" aria-labelledby="sessions-completed">/);
  });

  it("marks the selected row, and carries the needs-you class only on Needs input", () => {
    const a = row({ group: "needs-input" });
    const html = view(list([a, row()]), { selected: sessionKey(a) });
    expect(html).toMatch(/class="session-row needs-input on"[^>]*aria-pressed="true"/);
    expect(html.match(/aria-pressed="true"/g)).toHaveLength(1);
  });

  it("puts dir=auto on names and messages, which may be Persian", () => {
    const html = view(list([row({ name: "گلی", message: "کار تمام شد" })]));
    expect(html).toContain('<span class="session-name" dir="auto">گلی</span>');
    expect(html).toContain('<span class="session-msg" dir="auto">کار تمام شد</span>');
  });

  it("shows hostile names and messages as plain text, never markup", () => {
    const html = view(list([row({ name: "<img src=x onerror=alert(1)>", message: "**bold** <script>x()</script> [a](javascript:x)" })]));
    expect(html).not.toContain("<img");
    expect(html).not.toContain("<script");
    expect(html).not.toContain("<strong");
    expect(html).not.toContain('href="javascript');
    expect(text(html)).toContain("<img src=x onerror=alert(1)>");
  });

  it("lays the row out with logical properties, so RTL mirrors it", () => {
    const rules = css.slice(css.indexOf("/* The Sessions view (0098)"));
    expect(rules).toContain("border-inline-start");
    expect(rules).toContain("margin-inline-start: auto");
    expect(rules).not.toMatch(/(?:margin|padding|border)-(?:left|right)\b|text-align: (?:left|right)/);
    // Colours only through the tokens.
    expect(rules).not.toMatch(/#[0-9a-fA-F]{3,8}\b|rgb\(/);
  });

  it("says ages with the app's formatter", () => {
    expect(rowAge(NOW, NOW - 30)).toBe("just now");
    expect(rowAge(NOW, NOW - 3 * 3600)).toBe("3 h ago");
    expect(rowAge(NOW, null)).toBe("");
  });
});

describe("speed (AC8)", () => {
  it("renders a list of 60 sessions in under 50 ms (the fastest render, mu-220)", () => {
    const rows = Array.from({ length: 60 }, (_, i) => row({ name: `S_${i}`, group: (["needs-input", "working", "completed"] as const)[i % 3], at: NOW - i * 4000 }));
    const { ms, out: html } = fastest(() => view(list(rows)));
    expect(ms).toBeLessThan(50);
    expect(html.match(/data-session=/g)?.length).toBeGreaterThan(30);
  });
});

const recap = (over: Partial<SessionRecap> = {}): SessionRecap => ({
  who: { account: "mg", id: "aaaa0005", name: "Asghar_5", project: "muster", checkout: "wt-0098", group: "completed", word: "stopped", at: NOW - 660 },
  tasks: [{ queue: "muster", id: "0098", key: "mu-98", title: "Sessions view", stage: "testing", holds: true }],
  history: [
    { at: NOW - 7200, key: "", kind: "handoff-from", text: "took over from Asghar_4 (aaaa0004) by a handoff", pct: null },
    { at: NOW - 7100, key: "mu-98", kind: "takeover", text: "took over the claim from asghar_4", pct: null },
    { at: NOW - 3600, key: "mu-98", kind: "check-in", text: "core done", pct: 40 },
    { at: NOW - 700, key: "mu-98", kind: "pass", text: "passed it Building → Testing", pct: null },
    { at: NOW - 600, key: "", kind: "handoff-to", text: "handed off to Asghar_6 (aaaa0006)", pct: null },
  ],
  chain: [
    { id: "aaaa0004", name: "Asghar_4", current: false },
    { id: "aaaa0005", name: "Asghar_5", current: true },
    { id: "aaaa0006", name: "", current: false },
  ],
  cost: {
    found: "read", start: NOW - 7200, end: NOW - 660, minutes: 109, tokens: { input: 1000, output: 2000, cacheRead: 300_000, cacheWrite: 50_000 }, totalTokens: 353_000, countedTokens: 53_000,
    chainMinutes: 300, chainTokens: { input: 3000, output: 6000, cacheRead: 900_000, cacheWrite: 150_000 }, chainTotalTokens: 1_059_000, chainCountedTokens: 159_000, chainRead: 2, chainSessions: 3,
    // mu-172: the core's shares are of the counted tokens (53k and 159k of 1.5M).
    estTokens: 1_500_000, estPct: 4, chainEstPct: 11,
  },
  progress: [{ key: "mu-98", pct: 40, text: "core done", guess: false }],
  ...over,
});
const recapView = (r: SessionRecap | null, more: { mode?: DisplayMode; showing?: boolean; transcript?: { md?: string; error?: string } | null; error?: string } = {}) =>
  renderToStaticMarkup(
    <Mode.Provider value={more.mode ?? "adhd"}>
      <RecapView recap={r} error={more.error ?? ""} now={NOW} showing={more.showing ?? false} transcript={more.transcript ?? null} onTranscript={noop} onClose={noop} onOpenTask={noop} />
    </Mode.Provider>,
  );

describe("the recap (AC3, AC4)", () => {
  afterEach(async () => {
    await setLocale("en");
  });

  it("has its sections in a fixed order, under a strip with Transcript and Close", () => {
    const html = recapView(recap());
    const order = [...html.matchAll(/<section class="recap-([a-z]+)"/g)].map((m) => m[1]);
    expect(order).toEqual(["who", "tasks", "history", "time", "tokens", "progress"]);
    expect(html).toMatch(/<div class="pane-head recap-strip">[\s\S]*?Transcript<\/button>[\s\S]*?Close<\/button><\/div>/);
    // The recap leads: no transcript until asked for.
    expect(html).not.toContain('class="transcript"');
  });

  it("says who: name, id, account, project and checkout, state with age", () => {
    const t = text(recapView(recap()));
    expect(t).toContain("Name Asghar_5 Session aaaa0005 Account cc-mg Project muster · wt-0098 State completed (stopped) · 11 min ago");
  });

  it("lists the tasks it worked on as links, with stage and whether it holds it", () => {
    const html = recapView(recap());
    expect(html).toMatch(/class="recap-task"[^>]*>.*mu-98.*Sessions view<\/span><\/button>/);
    expect(text(html)).toContain("mu-98 Sessions view · Testing · holds it");
    expect(text(recapView(recap({ tasks: [] })))).toContain(WORDS.noTasks);
  });

  it("shows the history in time order with check-in percentages and handoffs", () => {
    const t = text(recapView(recap()));
    expect(t).toMatch(/handoff took over from Asghar_4.*takeover mu-98 took over the claim.*check-in mu-98 40% core done.*pass mu-98 passed it Building → Testing.*handoff handed off to Asghar_6/);
    expect(historyKind("handoff-to")).toBe("handoff");
    expect(historyKind("odd")).toBe("odd");
  });

  it("folds the history to a header with its count in Standard and Focus", () => {
    for (const mode of ["standard", "focus"] as const) {
      const html = recapView(recap(), { mode });
      expect(html).toMatch(/class="recap-fold" aria-expanded="false">History <small>5<\/small><\/button>/);
      expect(html).not.toContain("recap-events");
    }
    expect(recapView(recap())).toContain("recap-events");
  });

  it("draws the chain as one line, this session bold, a nameless one by its id", () => {
    const r = recap();
    expect(chainLine(r.chain)).toBe("Asghar_4 → Asghar_5 → aaaa0006");
    const html = recapView(r);
    expect(html).toContain('aria-label="Handoff chain: Asghar_4 → Asghar_5 → aaaa0006"');
    expect(html).toContain('<b dir="auto">Asghar_5</b>');
    // No chain line for a session on its own.
    expect(recapView(recap({ chain: [{ id: "aaaa0005", name: "Asghar_5", current: true }] }))).not.toContain("recap-chain");
  });

  it("gives time and tokens, the chain's sums beside them, and the share of est-tokens", () => {
    const t = text(recapView(recap()));
    expect(t).toMatch(/Started \d{4}-\d\d-\d\d \d\d:\d\d · last active \d{4}-\d\d-\d\d \d\d:\d\d · 1\.8 h · the chain of 3: 5 h/);
    // mu-172: the counted tokens against the estimate, cache reads beside them and never counted.
    expect(t).toContain("53k of 1.5M (4%) + 300k cache reads, not counted");
    expect(t).not.toContain("353k");
    expect(t).toContain("input 1k · output 2k · cache read 300k · cache write 50k");
    expect(t).toContain("The chain of 3: 159k of 1.5M (11%) + 900k cache reads, not counted · 1 of 3 not read on this Mac");
    expect(t).not.toContain("1.1M");
  });

  it("gives the share in the same digits as the token figures beside it (review R6)", async () => {
    await setLocale("fa");
    // The figures follow the language, the share too (its words are Persian since mu-147).
    expect(text(recapView(recap())).replace(/[\u2066-\u2069]/g, "")).toContain("۵۳k از ۱٫۵M (۴٪)");
  });

  it("says no cache reads when there were none, and the counted tokens alone without an estimate", () => {
    const base = recap().cost;
    const t = text(recapView(recap({ cost: { ...base, tokens: { ...base.tokens, cacheRead: 0 }, estTokens: 0, estPct: null, chainSessions: 1 } })));
    expect(t).toContain("Tokens 53k input 1k");
    expect(t).not.toContain("cache reads, not counted");
  });

  it("says why there are no numbers when the transcript wasn't read", () => {
    const t = text(recapView(recap({ cost: { ...recap().cost, found: "not-read", chainSessions: 1 } })));
    expect(t).toContain("not read: not stamped with a Claude account Muster reads");
    expect(t).not.toContain("Started");
  });

  it("shows progress per task, a guess labelled as one", () => {
    expect(text(recapView(recap()))).toContain("mu-98 40% core done");
    const guess = text(recapView(recap({ progress: [{ key: "mu-98", pct: 60, text: "2 of 3 criteria ticked", guess: true }] })));
    expect(guess).toContain("mu-98 60% 2 of 3 criteria ticked (a guess)");
  });

  it("opens the transcript after the recap, never in its place", () => {
    const html = recapView(recap(), { showing: true, transcript: { md: "# Hello" } });
    expect(html.indexOf("recap-progress")).toBeLessThan(html.indexOf('class="transcript"'));
    expect(html).toContain("<h1 dir=\"auto\">Hello</h1>");
    expect(html).toMatch(/aria-pressed="true"[^>]*>Transcript/);
    expect(text(recapView(recap(), { showing: true }))).toContain(WORDS.readingTranscript);
    expect(text(recapView(recap(), { showing: true, transcript: { error: "gone" } }))).toContain("gone");
  });

  it("says it's reading, or the core's error, before a recap lands", () => {
    expect(text(recapView(null))).toContain(WORDS.readingRecap);
    expect(recapView(null)).toMatch(/disabled=""[^>]*>Transcript/);
    expect(text(recapView(null, { error: "session aaaa0005 isn't in cc-mg's list now" }))).toContain("isn't in cc-mg's list now");
  });

  it("shows a hostile name and history text as plain text", () => {
    const r = recap();
    r.who.name = "<b>x</b>";
    r.history[2].text = "<script>x()</script>";
    const html = recapView(r);
    expect(html).not.toContain("<script");
    expect(html).not.toContain("<b>x</b>");
  });
});
