// The shell in Persian (mu-146, AC3): the sidebar, the toolbar's mode chip, the filter bar and
// every Settings group, drawn with the Persian files loaded, show no English UI text: every word
// in Latin letters left is a product name, a key, user data (a project's name, a path, a command
// in <code>) or the language chips' own names. English is checked the same way, so the check
// itself sees text: it finds English words there.
import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import type { HelperInfo } from "./api";
import { AccountMeterView } from "./AccountMeter";
import { type AccountView, profileForm, type WindowView } from "./accountsData";
import { ChoiceChips, ModeChip } from "./ModeViews";
import FilterBar from "./FilterBar";
import { dayContext, EMPTY_QUERY, type Query } from "./filters";
import { GroupByChips } from "./Groups";
import { HelpButton } from "./HelpButton";
import { FOR_FILTERS } from "./human";
import { tn } from "./i18n";
import { setLocale } from "./i18n/runtime";
import { estimateTotal } from "./lib";
import { DisplayModes } from "./Display";
import { NotificationsView } from "./Notifications";
import ProjectList, { type ProjectEdits } from "./ProjectList";
import Projects, { CategoryCell, ColourCell, GateRow, HistoryCell, NameCell, PlaceCell, PrefixCell } from "./Projects";
import { RolesView } from "./Roles";
import { LimitCell } from "./CheckLimit";
import Settings, { AboutView, AccountAdd, AccountsView, AgentsView, ProfileEditor } from "./Settings";
import { SETTINGS_GROUPS, type SettingsInfo } from "./settingsData";
import type { HistoryState, Project, QueueInfo, Role, Task } from "./types";

const secs = (h: number, m: number) => new Date(2026, 9, 13, h, m).getTime() / 1000;
const NOW = secs(14, 0);

const task = (over: Partial<Task>): Task => ({
  queue: "alpha", id: "0001", title: "T", kind: "bug", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [],
  folder: "todo", path: "todo/0001-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "2026-10-13", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: NOW - 600, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "ready", search: "", ...over,
});

// One of everything the sidebar says: working, stalled with a quiet claim (its check-in button),
// waiting on the owner, paused, running, over its Your check limit, a heading and Other.
const TASKS: Task[] = [
  task({ id: "0001", stage: "building", live: true, estMinutes: 150, estTokenCount: 450_000 }),
  task({ id: "0002", queue: "zeta", stage: "building", claimedBy: "gholi_18 (main · session abcd1234 · cc-mg)", heartbeat: "2026-10-13 11:00", modified: NOW - 7200 }),
  task({ id: "0003", queue: "zeta", stage: "your-check", waitingForOwner: true, priority: "high", openQuestions: 1 }),
  task({ id: "0004", stage: "ready", paused: "2026-10-13 12:00" }),
  task({ id: "0005", stage: "done", doneOn: "2026-10-12" }),
];
const QUEUES: QueueInfo[] = [
  { name: "alpha", path: "/p/alpha/tasks", counts: { building: 1, ready: 1, done: 1 }, category: "Clients", colour: "lime", running: true },
  { name: "zeta", path: "/p/zeta/tasks", counts: { building: 1, "your-check": 1 }, yourCheck: 3, yourCheckLimit: 2, lessonsWaiting: 1 },
];
const EDITS: ProjectEdits = { arrange: async () => {}, renameCategory: async () => {}, colour: async () => {}, fold: async () => {} };

const ROLE: Role = { id: "builder", name: "gholi", title: "builder", stage: "building", prompt: "", tools: [], permission: "acceptEdits", models: ["opus"], maxTurns: 200, maxBudgetUsd: 20, contextCap: 150000, workers: 1, slots: 2, moves: ["building>testing"], providers: ["claude"], writes: [] };
const HISTORY: HistoryState = { kept: true, started: true, lastCommit: NOW - 120, pending: 0, pendingError: "", missing: [], changed: [] };
const HISTORIES: HistoryState[] = [HISTORY, { ...HISTORY, pending: 2, pendingError: "boom" }, { ...HISTORY, missing: ["a"] }, { ...HISTORY, changed: ["a", "b"] }, { ...HISTORY, started: false, lastCommit: null }];

const INFO: SettingsInfo = {
  settings: { leaseMinutes: 720, liveMinutes: 90, notify: true, menuBar: false, claudeAccounts: ["mg", "hg"] },
  leaseBounds: [60, 10080],
  liveBounds: [1, 1440],
  version: "0.1.0",
  home: "/Users/someone",
  configDir: "/Users/someone/cfg",
  base: "/Users/someone/playground",
  transcripts: ["/Users/someone/.claude-mg/projects"],
  transcriptsUnder: "/Users/someone/playground",
  sessionStates: "on",
};
const HELPER: HelperInfo = {
  claude: "/usr/local/bin/claude",
  accounts: [
    { name: ".claude", loggedIn: true, plan: "max" },
    { name: ".claude-mg", loggedIn: false, plan: "" },
    { name: ".claude-x", loggedIn: null, plan: "" },
  ],
  account: ".claude",
  model: "sonnet",
  models: ["sonnet", "haiku", "opus"],
  running: false,
  lines: [],
};
const project = (over: Partial<Project>): Project => ({ key: "/p/alpha/tasks", name: "alpha", path: "/p/alpha/tasks", source: "found", group: "", hidden: false, ok: true, gate: "", run: "", prefix: "al", label: "", category: "", colour: "", groupName: "", ...over });
const PROJECTS = [project({}), project({ key: "/x/zeta/tasks", name: "zeta", path: "/x/zeta/tasks", source: "added", hidden: true }), project({ key: "/x/gone", name: "gone", path: "/x/gone", source: "added", ok: false })];

const QUERY: Query = { search: "flicker", filters: { stage: ["building"], priority: ["high"], type: ["bug"], state: ["needs-you"], captured: ["week"], holder: ["gholi_18"] }, sort: "priority" };

/** Everything the shell draws, in the language loaded now. */
function shell(): string {
  const parts: ReactElement[] = [
    createElement(ProjectList, { queues: QUEUES, tasks: TASKS, notes: [], activity: [], now: NOW, current: "all", onPick: () => {}, onLabel: async () => {}, edit: EDITS }),
    createElement(ModeChip, { mode: "adhd", onOpen: () => {} }),
    createElement(FilterBar, {
      tasks: TASKS, query: QUERY, onQuery: () => {}, ctx: dayContext(NOW, {}), sortable: true, views: [{ ...EMPTY_QUERY, name: "Mine" }], onViews: () => {}, shown: 2, total: 5, hidden: 3, estimate: estimateTotal(TASKS),
      arrange: createElement("span", null, createElement(ChoiceChips, { label: tn("shell", "app.who"), className: "for-filter", aria: tn("shell", "app.whoAria"), value: "all", options: FOR_FILTERS, onPick: () => {} }), createElement(GroupByChips, { value: "none", onChange: () => {} })),
      help: createElement(HelpButton, { topic: "board" }),
    }),
    createElement(FilterBar, { tasks: TASKS, query: EMPTY_QUERY, onQuery: () => {}, ctx: dayContext(NOW, {}), sortable: true, views: [], onViews: () => {}, shown: 5, total: 5, hidden: 0, estimate: "" }),
    ...SETTINGS_GROUPS.map((g) => createElement(Settings, { key: g.id, group: g.id, onGroup: () => {}, queues: QUEUES, onChanged: () => {}, mode: "adhd", onMode: () => {} })),
    createElement(DisplayModes, { mode: "focus", onMode: () => {} }),
    createElement(Projects, { queues: QUEUES, home: INFO.home, onChanged: () => {} }),
    ...PROJECTS.map((p) => createElement("div", { key: p.key }, createElement(NameCell, { p, onSave: async () => {} }), createElement(CategoryCell, { p, categories: ["Clients"], onSave: async () => {} }), createElement(ColourCell, { p, onSave: async () => {} }))),
    createElement(PlaceCell, { name: "alpha", canUp: true, canDown: false, onUp: () => {}, onDown: () => {} }),
    createElement(PrefixCell, { name: "alpha", prefix: "al", editing: false, value: "", error: "", canSet: true, onEdit: () => {}, onChange: () => {}, onSave: () => {}, onCancel: () => {} }),
    createElement(PrefixCell, { name: "alpha", prefix: "al", editing: true, value: "al", error: "", canSet: true, onEdit: () => {}, onChange: () => {}, onSave: () => {}, onCancel: () => {} }),
    createElement("table", null, createElement("tbody", null, createElement(GateRow, { name: "alpha", gate: "", run: "", hidden: false }), createElement(GateRow, { name: "alpha", gate: "bin/gate", run: "pnpm dev", hidden: false }))),
    createElement(LimitCell, { q: QUEUES[1], onSet: () => {} }),
    createElement(LimitCell, { q: { ...QUEUES[0], yourCheck: 0, yourCheckLimit: null }, onSet: () => {} }),
    createElement(AgentsView, { info: INFO, helper: HELPER, onHelper: () => {}, onModel: () => {}, onSet: async () => null }),
    createElement(AgentsView, { info: INFO, helper: { ...HELPER, claude: null }, onHelper: () => {}, onModel: () => {}, onSet: async () => null }),
    createElement(AgentsView, { info: INFO, helper: null, onHelper: () => {}, onModel: () => {}, onSet: async () => null }),
    createElement(AboutView, { info: INFO, projects: PROJECTS }),
    createElement(AboutView, { info: INFO, projects: [] }),
    createElement(AboutView, { info: INFO, projects: null, error: "boom" }),
    createElement(NotificationsView, { info: INFO, onSet: () => {} }),
    // Main's additions to the shell (rebased onto mu-109, mu-112): Roles and a queue's history.
    createElement(RolesView, { roles: [ROLE], open: "builder", onOpen: () => {}, onSave: async () => null }),
    createElement(RolesView, { roles: null, open: "", onOpen: () => {}, onSave: async () => null }),
    ...HISTORIES.map((h, i) => createElement(HistoryCell, { key: i, h, now: NOW, name: "alpha", onFix: () => {} })),
    // mu-113's header meter and Settings › Accounts, every state: live, stale, over the limit and
    // into the reserve, no reading, paused, none, unreadable, loading; Remove's ask.
    ...ACCOUNT_INFOS.map((info, i) => createElement(AccountMeterView, { key: i, info, error: "", onOpen: () => {} })),
    createElement(AccountMeterView, { info: null, error: "boom", onOpen: () => {} }),
    ...ACCOUNT_INFOS.map((info, i) => createElement(AccountsView, { key: i, info, error: "", calls: ACCOUNT_CALLS })),
    createElement(AccountsView, { info: null, error: "boom", calls: ACCOUNT_CALLS }),
    createElement(AccountsView, { info: null, error: "", calls: ACCOUNT_CALLS }),
    createElement(() => ProfileEditor({ a: ACCOUNTS[0], form: profileForm(ACCOUNTS[0]), error: "", busy: false, removing: true, onForm: () => {}, onSave: () => {}, onRemoving: () => {}, onRemove: () => {} })),
    createElement(AccountAdd, { typed: "", provider: "codex", error: "", busy: false, onTyped: () => {}, onProvider: () => {}, onAdd: () => {} }),
  ];
  return parts.map((p) => renderToStaticMarkup(p)).join("\n");
}

const win = (over: Partial<WindowView> = {}): WindowView => ({ usedPercentage: 42, resetsAt: secs(16, 20), stale: false, headroom: 38, ...over });
const ACCOUNTS: AccountView[] = [
  { provider: "claude", label: "mg", name: "cc-mg", plan: "", roles: [], models: [], concurrency: 2, reserve: 20, paused: false, projects: ["alpha"], fiveHour: win(), sevenDay: win({ resetsAt: NOW + 3 * 86400 }), readingAt: NOW - 600, source: "statusline" },
  { provider: "claude", label: "hg", name: "cc-hg", plan: "", roles: [], models: [], concurrency: 2, reserve: 20, paused: true, projects: [], fiveHour: win({ usedPercentage: 104, headroom: -24, stale: true, resetsAt: secs(9, 0) }), sevenDay: win({ stale: true, resetsAt: NOW - 86400 }), readingAt: NOW - 30, source: "stream" },
  { provider: "claude", label: "mg", name: "cc-mg", plan: "", roles: [], models: [], concurrency: 2, reserve: 20, paused: false, projects: [], fiveHour: null, sevenDay: win({ stale: true }), readingAt: NOW - 30, source: "stream" },
  { provider: "codex", label: "mg", name: "codex-mg", plan: "", roles: [], models: [], concurrency: 2, reserve: 20, paused: false, projects: [], fiveHour: null, sevenDay: null, readingAt: null, source: null },
];
const ACCOUNT_INFOS = [{ accounts: ACCOUNTS, now: NOW }, { accounts: [], now: NOW }];
const ACCOUNT_CALLS = { add: async () => {}, remove: async () => {}, save: async () => {}, after: () => {} };

/** Product and model names, macOS's own names, keys and the language chips' names: the same in
 *  every language (docs/i18n/glossary-fa.md: names stay Latin; worktree stays Latin). */
const KEPT = new Set(["Muster", "Claude", "Code", "Codex", "macOS", "Anthropic", "Sonnet", "Haiku", "Opus", "ADHD", "WCAG", "AA", "Drover", "Braindump", "Dock", "Control", "Shift", "Esc", "English", "worktree", "SIL", "Open", "Font", "License"]);
/** The fixture's own data: project names and a category, the saved view, a holder, a plan, the
 *  prefix and a task key (mu-12), the branch main, commands (muster reclaim), an error, and the
 *  core's summary of session states ("on": the core's own text, mu-153's), and the Claude accounts
 *  read, as their commands (cc-mg) and labels (mg); a role's slot name (Kaveh), its roles.toml
 *  values (title, stage, permission, moves, models, providers) and the commands muster team and
 *  --role; the role names a profile's Roles field shows as its example (builder, tester). The
 *  font's licence is a name. */
const DATA = new Set(["alpha", "zeta", "gone", "Clients", "Mine", "flicker", "gholi", "max", "al", "boom", "mu", "main", "muster", "reclaim", "on", "cc-mg", "cc-hg", "mg", "Kaveh", "builder", "tester", "building", "testing", "acceptEdits", "opus", "sonnet", "haiku", "fable", "claude", "codex", "codex-mg", "team", "role"]);

/** The Latin words a person sees or hears: text and the text attributes, without code, paths
 *  (~/… and /…), keys like ⌘N and the fixture's data. */
function latinWords(html: string): string[] {
  const attrs = [...html.matchAll(/\s(?:title|aria-label|placeholder|alt)="([^"]*)"/g)].map((m) => m[1]);
  const body = html.replace(/<code[^>]*>[\s\S]*?<\/code>/g, " ").replace(/<[^>]+>/g, " ");
  const text = [body, ...attrs]
    .join(" ")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&[a-z#0-9]+;/gi, " ")
    .replace(/[~/][^\s,،)]*/g, " ")
    .replace(/\b\d+(\.\d+)?[mhdkM]\b/g, " ")
    .replace(/⌘\S*/g, " ");
  const words = (text.match(/[A-Za-z][A-Za-z'’-]*/g) ?? []).map((w) => w.replace(/[-'’]+$/, ""));
  return [...new Set(words)].filter((w) => w.length > 1 && !KEPT.has(w) && !DATA.has(w));
}

beforeAll(() => {
  // Appearance reads the Mac's light or dark and the window's saved choice.
  vi.stubGlobal("window", { matchMedia: () => ({ matches: true }) });
  vi.stubGlobal("localStorage", { getItem: () => null, setItem: () => {} });
});
afterEach(async () => {
  await setLocale("en");
});
afterAll(() => {
  vi.unstubAllGlobals();
});

describe("the shell in Persian (AC3)", () => {
  it("draws English in English: the check sees the shell's words", () => {
    const words = latinWords(shell());
    for (const w of ["Projects", "Settings", "Appearance", "Search", "Notifications", "stalled", "working", "Stage", "Lime"]) expect(words).toContain(w);
  });

  it("shows no English UI text in the sidebar, the toolbar, the filter bar or any Settings group", async () => {
    await setLocale("fa");
    const html = shell();
    expect(latinWords(html)).toEqual([]);
    // And it is Persian, with the stage, priority and type values said in it.
    for (const w of ["پروژه‌ها", "تنظیمات", "در حال ساخت", "بالا", "باگ", "راکد", "در حال کار", "لیمویی", "زبان"]) expect(html).toContain(w);
  });
});
