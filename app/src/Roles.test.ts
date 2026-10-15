// Role slots (0112): a stamp's `slot coder-2` shows as the role's name now on cards and in the
// pane, renaming a role in Settings › Agents › Roles renames it there, and old stamps read as before.
import rolesRs from "../src-tauri/src/roles.rs?raw";
import { createElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import CardStatus from "./CardStatus";
import { GraphView } from "./GraphView";
import { applyQuery, dayContext, EMPTY_QUERY, GROUPS } from "./filters";
import { askerShown, heldByOther, holder, nameOf, sameHolder, shownHolder, shownLabel, slotLabel, splitHolder, whoLine } from "./lib";
import AnswerBox from "./AnswerBox";
import Comments from "./Comments";
import { Checkins } from "./TaskPanel";
import { CardHolderRibbon, NoteHolderRibbon } from "./HolderRibbon";
import { WorkView } from "./Work";
import { releasedBy, trackMove } from "./paneData";
import { queryWords, taskMatches } from "./search";
import { sortTasks } from "./viewData";
import { StatusLine } from "./PaneHeader";
import Recent from "./Recent";
import { builtBy } from "./recentData";
import { RolesView } from "./Roles";
import { NextStep } from "./Sidebar";
import StageContent from "./StageContent";
import type { TaskActions } from "./useTaskActions";
import { TableView } from "./Views";
import { BUILT_IN_TITLES, PERMISSIONS, permissionWord, ROLE_FIELDS, roleLine, roleTitle, slotWords } from "./rolesData";
import { setLocale } from "./i18n/runtime";
import { tn, unmarked } from "./i18n";
import { RoleNamesContext } from "./settingsData";
import type { Question, Role, SessionRow, Task, Work as WorkData } from "./types";

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0030", title: "T", kind: "task", priority: "normal", status: "doing", stage: "building", blocked: false, stageHistory: [],
  folder: "doing", path: "doing/0030-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "building", ...over,
});

const KAVEH = "Kaveh-2 (main · session 7b5e8856 · slot coder-2 · cc-hg)";
const NAMES = { coder: "Kaveh", tester: "Kia" };
const RENAMED = { coder: "کاوه", tester: "Kia" };

const role = (over: Partial<Role> = {}): Role => ({
  id: "coder", name: "Kaveh", title: "Coder", stage: "building", prompt: "muster instructions coder", tools: ["Read"], permission: "worktree",
  models: ["sonnet", "opus"], maxTurns: 300, maxBudgetUsd: 20, contextCap: 150000, workers: 0, slots: 3, moves: ["ready>building", "building>testing"],
  providers: ["claude", "codex"], writes: [], ...over,
});

const inNames = (names: Record<string, string>, el: ReactNode) => renderToStaticMarkup(createElement(RoleNamesContext.Provider, { value: names }, el));
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/\s+/g, " ").trim();

describe("a slot in a holder's stamp", () => {
  it("parses after the session and agent, once, before the account", () => {
    expect(splitHolder(KAVEH)).toEqual({ label: "Kaveh-2", parts: ["main"], id: "7b5e8856", slot: "coder-2", account: "hg" });
    expect(splitHolder("Sohrab-1 (session 7b5e8856 · agent a1b2c3d4 · slot security-1)").slot).toBe("security-1");
    for (const who of [
      "x (slot coder-2 · session 7b5e8856)",
      "x (session 7b5e8856 · cc-hg · slot coder-2)",
      "x (session 7b5e8856 · slot coder-2 · slot coder-2)",
      "x (session 7b5e8856 · slot coder-02)",
      "x (session 7b5e8856 · slot coder-21)",
      "x (session 7b5e8856 · slot Coder-2)",
    ]) {
      expect(splitHolder(who), who).toEqual({ label: who, parts: [], id: "", account: "" });
    }
  });
  it("shows the role's name now, and an old stamp, or a role with no name, as written", () => {
    expect(shownLabel(KAVEH, NAMES)).toBe("Kaveh-2");
    expect(shownLabel(KAVEH, RENAMED)).toBe("کاوه-2");
    expect(shownHolder(KAVEH, RENAMED)).toBe("کاوه-2 (main · session 7b5e8856 · slot coder-2 · cc-hg)");
    expect(shownLabel(KAVEH, {})).toBe("Kaveh-2");
    const old = "wt-a (main · session 7b5e8856 · cc-hg)";
    expect(shownLabel(old, RENAMED)).toBe("wt-a");
    expect(shownHolder(old, RENAMED)).toBe(old);
    expect(shownHolder("Kaveh-2", RENAMED)).toBe("Kaveh-2");
  });
  it("looks a role up only among the roles, never on the object's prototype", () => {
    expect(shownLabel("Kia-1 (session 7b5e8856 · slot constructor-1)", NAMES)).toBe("Kia-1");
    expect(shownLabel("Kia-1 (session 7b5e8856 · slot constructor-1)", {})).toBe("Kia-1");
    expect(nameOf("x (session 7b5e8856 · slot valueof-1)", NAMES)).toBe("x");
  });
  it("is one holder in one session whatever its label says", () => {
    expect(sameHolder(KAVEH, "coder-2 (session 7b5e8856 · slot coder-2)")).toBe(true);
    expect(sameHolder(KAVEH, "coder-2 (session 11111111 · slot coder-2)")).toBe(false);
    expect(sameHolder(KAVEH, "Kaveh-3 (session 7b5e8856 · slot coder-3)")).toBe(false);
    expect(sameHolder("wt-a (session 7b5e8856)", "wt-b (session 7b5e8856)")).toBe(false);
  });
});

describe("the card and the pane name a slot by its role", () => {
  const building = { stage: "building", claimedBy: KAVEH };
  it("the card's who line says the slot's name where the label shows", () => {
    expect(whoLine(task(building), NAMES)).toBe("Kaveh-2 · main · 7b5e8856");
    expect(whoLine(task({ stage: "building", claimedBy: "wt-a (main · session 7b5e8856 · cc-hg)" }), RENAMED)).toBe("wt-a · main · 7b5e8856");
    const card = (names: Record<string, string>) => inNames(names, createElement(CardStatus, { t: task(building), now: 0 }));
    expect(card(NAMES)).toContain('<span class="card-who" dir="auto">Kaveh-2 · main · 7b5e8856');
    // Renamed in Settings: the next poll's names, the same card.
    expect(card(RENAMED)).toContain('<span class="card-who" dir="auto">کاوه-2 · main · 7b5e8856');
  });
  it("the pane's on it and built by lines say it too", () => {
    const pane = (t: Task, names: Record<string, string>) => text(inNames(names, createElement(StatusLine, { task: t, now: 0 })));
    expect(pane(task(building), RENAMED)).toContain("on it: کاوه-2 (main · session 7b5e8856 · slot coder-2 · cc-hg)");
    const tested = task({ stage: "your-check", heldBy: "", builtBy: KAVEH, gatedBy: KAVEH, testedBy: "Kia-1 (session 0f0f0f0f · slot tester-1)" });
    expect(pane(tested, RENAMED)).toContain("built by کاوه-2 (main · session 7b5e8856 · slot coder-2 · cc-hg) · tested by Kia-1 (session 0f0f0f0f · slot tester-1)");
  });
});

describe("every other view names a slot by its role now too (R2)", () => {
  const OLD = "wt-a (main · session 7b5e8856 · cc-hg)";
  const claimed = task({ stage: "building", claimedBy: KAVEH, heartbeat: "2026-10-01 09:00", stageHistory: [{ at: "2026-10-01 09:00", who: KAVEH, from: "ready", to: "building" }] });
  const late = new Date(2026, 9, 14, 12, 0).getTime() / 1000;
  it("the holder, held-by-another and built-by words", () => {
    expect(nameOf(KAVEH, RENAMED)).toBe("کاوه-2");
    expect(nameOf(OLD, RENAMED)).toBe("wt-a");
    expect(holder(claimed, RENAMED)).toBe("کاوه-2");
    expect(holder(claimed, {})).toBe("Kaveh-2");
    expect(holder(task({ stage: "testing", heldBy: KAVEH }), RENAMED)).toBe("کاوه-2");
    expect(heldByOther(claimed, RENAMED)).toBe("کاوه-2");
    expect(heldByOther(task({ stage: "building", claimedBy: OLD }), RENAMED)).toBe("wt-a");
    expect(builtBy(claimed, RENAMED)).toBe("کاوه-2 · main · 7b5e8856");
    expect(builtBy(task({ stage: "building", claimedBy: OLD, stageHistory: [{ at: "2026-10-01 09:00", who: OLD, from: "ready", to: "building" }] }), RENAMED)).toBe("wt-a · main · 7b5e8856");
  });
  it("the pane's History", () => {
    const html = text(inNames(RENAMED, createElement(StageContent, { task: claimed, nowSecs: 0, body: null, busy: false, onTick: () => {} })));
    expect(html).toContain("Ready → Building · کاوه-2");
    expect(html).not.toContain("Kaveh");
  });
  it("the Next step's held and stale notes", () => {
    const a = { busy: false, pass: () => {}, release: () => {}, open: () => {}, unblock: () => {} } as unknown as TaskActions;
    const html = text(inNames(RENAMED, createElement(NextStep, { task: claimed, nowSecs: late, a })));
    expect(html).toContain("کاوه-2 holds it in Building");
    expect(html).toContain("کاوه-2 hasn't been heard from");
    expect(html).not.toContain("Kaveh");
  });
  it("the table and the graph", () => {
    const table = inNames(RENAMED, createElement(TableView, { tasks: [claimed], now: 0, multi: false, openKey: null, onOpen: () => {}, words: [] }));
    expect(text(table)).toContain("کاوه-2");
    expect(table).not.toContain("Kaveh");
    const graph = inNames(RENAMED, createElement(GraphView, { tasks: [claimed], all: [claimed], multi: false, openKey: null, onOpen: () => {} }));
    expect(text(graph)).toContain("کاوه-2");
    expect(graph).not.toContain("Kaveh");
  });
  it("Recently finished's built by", () => {
    const done = task({
      stage: "your-check", heldBy: "owner",
      stageHistory: [{ at: "2026-10-13 09:00", who: KAVEH, from: "ready", to: "building" }, { at: "2026-10-13 10:00", who: KAVEH, from: "building", to: "testing" }, { at: "2026-10-13 13:00", who: "qa", from: "testing", to: "your-check" }],
    });
    const now = new Date(2026, 9, 13, 14, 0).getTime() / 1000;
    const html = inNames(RENAMED, createElement(Recent, { tasks: [done], now, kind: "all", onKind: () => {}, multi: false, openKey: null, onOpen: () => {}, onFold: () => {} }));
    expect(text(html)).toContain("کاوه-2 · main · 7b5e8856");
    expect(html).not.toContain("Kaveh");
  });
});

describe("Settings › Agents › Roles", () => {
  const roles = [role(), role({ id: "tester", name: "Kia", title: "QA tester", stage: "testing", slots: 1, moves: [] })];
  const view = (open: string, rs: Role[] | null = roles, error = "") =>
    renderToStaticMarkup(createElement(RolesView, { roles: rs, error, open, onOpen: () => {}, onSave: async () => null }));
  it("lists each role with its name and slot count, nothing open", () => {
    const html = view("");
    expect(text(html)).toContain("Kaveh Coder · 3 slots Kia QA tester · 1 slot");
    expect(html).toContain('<span class="meta"><bdi>Coder</bdi> · 3 slots</span>');
    expect(html).not.toContain("--role coder");
    expect(html).toContain('<button type="button" class="role-row" aria-expanded="false"><b dir="auto">Kaveh</b>');
    expect(html).toContain('<b dir="auto">Kaveh</b>');
    expect(html).not.toContain("role-fields");
  });
  it("opens one role's fields inline under its row, each a labelled box with Save", () => {
    const html = view("coder");
    expect(html).toContain('aria-expanded="true" aria-controls="role-coder"');
    expect(html).toContain('<div class="role-fields" id="role-coder" role="group" aria-label="Kaveh&#x27;s profile">');
    expect(text(html)).toContain("Coder · Building · worktree · Ready → Building, Building → Testing");
    expect(html).toContain('<p class="meta">Claim with <code class="ltr">--role coder</code></p>');
    for (const f of ROLE_FIELDS) expect(text(html)).toContain(f.label);
    expect(html).toContain('value="sonnet, opus"');
    expect(html).toContain('<label><b>Name</b> <input size="14" maxLength="400" dir="auto" value="Kaveh"/></label>');
    // Kia's stay closed.
    expect(html).not.toContain('id="role-tester"');
  });
  it("shows a renamed role by its new name, and loading and a refused file", () => {
    expect(view("", [role({ name: "کاوه" })])).toContain('<b dir="auto">کاوه</b>');
    expect(text(view("", null))).toContain("Loading…");
    const refused = view("", null, "roles.toml line 2: unknown key \"colour\" in [coder]");
    expect(refused).toContain('<p class="err" role="alert">roles.toml line 2: unknown key &quot;colour&quot; in [coder]</p>');
    expect(refused).not.toContain("Loading");
  });
  it("words: slots and the role's line", () => {
    expect(slotWords(1)).toBe("1 slot");
    expect(slotWords(3)).toBe("3 slots");
    expect(roleLine(role({ stage: "any", moves: [], permission: "lead", title: "Lead" }))).toBe("Lead · any stage · lead · no stage moves");
    expect(roleLine(role({ stage: "your-check" }))).toContain(" · Your check · ");
    // A stage or permission this window doesn't know shows as written, not as Backlog.
    expect(roleLine(role({ stage: "triage", permission: "admin", moves: ["triage>done"] }))).toBe("Coder · triage · admin · triage → Done");
  });
});

describe("a role's title in the language shown (mu-195)", () => {
  afterEach(async () => {
    await setLocale("en");
  });
  /** The glossary's words (docs/i18n/glossary-fa.md, Roles). */
  const PERSIAN: Record<string, string> = {
    shaper: "شکل‌دهنده", architect: "معمار", designer: "طراح", coder: "برنامه‌نویس", tester: "تستر",
    security: "بازبین امنیت", performance: "بازبین کارایی", accessibility: "بازبین دسترس‌پذیری",
    release: "انتشار و ادغام", lead: "سرپرست", researcher: "پژوهشگر",
  };
  /** The core's defaults, id → title, as roles.rs writes them: `role("coder", "Kaveh", "Coder", …)`. */
  const DEFAULTS: Record<string, string> = Object.fromEntries([...rolesRs.matchAll(/role\(\s*"([a-z]+)",\s*"[^"]*",\s*"([^"]*)",/g)].map((m) => [m[1], m[2]]));
  const builtIns = Object.entries(DEFAULTS).map(([id, title]) => role({ id, title }));
  const list = (roles: Role[], open = "") => renderToStaticMarkup(createElement(RolesView, { roles, open, onOpen: () => {}, onSave: async () => null }));

  it("knows the core's eleven roles by their default titles, as roles.rs and the English file say them", () => {
    expect(Object.keys(DEFAULTS)).toHaveLength(11);
    expect(BUILT_IN_TITLES).toEqual(DEFAULTS);
    expect(Object.keys(PERSIAN).sort()).toEqual(Object.keys(DEFAULTS).sort());
    for (const [id, title] of Object.entries(DEFAULTS)) expect(tn("shell", `roles.title.${id as keyof typeof BUILT_IN_TITLES}`), id).toBe(title);
  });
  it("says a built-in role's title in English, and in Persian from the glossary", async () => {
    for (const r of builtIns) expect(roleTitle(r)).toBe(r.title);
    await setLocale("fa");
    for (const r of builtIns) expect(roleTitle(r), r.id).toBe(PERSIAN[r.id]);
  });
  it("shows a title the owner wrote in roles.toml, or an unknown role's, as written in every language", async () => {
    await setLocale("fa");
    expect(roleTitle(role({ title: "Rust coder" }))).toBe("Rust coder");
    expect(roleTitle(role({ title: "کدنویس" }))).toBe("کدنویس");
    expect(roleTitle(role({ id: "cook", title: "Cook" }))).toBe("Cook");
    // Looked up among the roles only, never on the object's prototype.
    expect(roleTitle(role({ id: "constructor", title: "Builder" }))).toBe("Builder");
    expect(roleTitle(role({ id: "toString", title: "Shaper" }))).toBe("Shaper");
    // Another built-in's default title under this id is the owner's own words too.
    expect(roleTitle(role({ id: "coder", title: "Shaper" }))).toBe("Shaper");
  });
  it("reads every Roles row in Persian: no role id, the id only in the opened profile's --role hint, left to right", async () => {
    await setLocale("fa");
    const html = list(builtIns);
    const rows = text(html.slice(html.indexOf("<ul")));
    for (const r of builtIns) expect(rows).toContain(PERSIAN[r.id]);
    // The fixture names every role Kaveh; nothing else in a row is Latin.
    expect(rows.replace(/\bKaveh\b/g, "")).not.toMatch(/[A-Za-z]/);
    expect(html).toContain(`<span class="meta"><bdi>${PERSIAN.coder}</bdi> · ۳ جایگاه</span>`);
    expect(html).not.toMatch(/--role [a-z]/);
    const open = list(builtIns, "researcher");
    expect(open).toContain(`<code class="ltr">--role researcher</code>`);
    expect(open.match(/--role [a-z]+/g)).toEqual(["--role researcher"]);
    expect(text(open)).toContain("برداشتن با --role researcher");
  });
  it("names the row's button by name, title and slots in the language shown", async () => {
    await setLocale("fa");
    const html = list([role()]);
    expect(text(html.slice(html.indexOf("<button"), html.indexOf("</button>")))).toBe("Kaveh برنامه‌نویس · ۳ جایگاه");
  });
  it("says the opened profile's line in the language shown: title, stage, permission and moves", async () => {
    await setLocale("fa");
    expect(unmarked(roleLine(role()))).toBe("برنامه‌نویس · در حال ساخت · worktree · آماده ← در حال ساخت، در حال ساخت ← در حال تست");
    expect(roleLine(role({ stage: "any", moves: [], permission: "lead", title: "Lead", id: "lead" }))).toBe("سرپرست · هر مرحله · سرپرستی · بی جابه‌جایی مرحله");
    expect(roleLine(role({ stage: "testing", permission: "read-only", moves: [] }))).toContain(" · در حال تست · فقط‌خواندنی · ");
    // The profile as drawn, opened: the line, then the --role hint.
    const open = text(list([role()], "coder"));
    expect(unmarked(open)).toContain("برنامه‌نویس · در حال ساخت · worktree · آماده ← در حال ساخت");
  });
  it("has a word in English and Persian for each of the core's permission profiles", async () => {
    const core = /pub const PERMISSIONS: &\[&str\] = &\[([^\]]*)\]/.exec(rolesRs)?.[1] ?? "";
    expect([...core.matchAll(/"([a-z-]+)"/g)].map((m) => m[1])).toEqual([...PERMISSIONS]);
    const fa: Record<string, string> = { "read-only": "فقط‌خواندنی", shaping: "شکل‌دهی", research: "پژوهش", worktree: "worktree", release: "انتشار", lead: "سرپرستی" };
    for (const p of PERMISSIONS) expect(permissionWord(p)).toBe(p);
    await setLocale("fa");
    for (const p of PERMISSIONS) expect(permissionWord(p), p).toBe(fa[p]);
    expect(permissionWord("admin")).toBe("admin");
    expect(permissionWord("constructor")).toBe("constructor");
  });
  it("says the Persian words with only the Persian files loaded, as the fa window does (R1)", async () => {
    // A fresh copy of the layer with nothing loaded: testSetup.ts preloads every English file, the
    // window shown in Persian never loads one (fa is READY), so a lookup that leans on the English
    // file at run time passes above and fails here.
    vi.resetModules();
    const runtime = await import("./i18n/runtime");
    const { readFromDisk } = await import("./i18n/testReader");
    const asked: string[] = [];
    runtime.setReader((l, ns) => {
      asked.push(`${l}/${ns}`);
      return l === "fa" ? readFromDisk(l, ns) : Promise.reject(new Error(`the fa window never reads ${l}/${ns}`));
    });
    await runtime.setLocale("fa");
    await runtime.load("fa", "shell");
    const fresh = await import("./rolesData");
    for (const r of builtIns) expect(fresh.roleTitle(r), r.id).toBe(PERSIAN[r.id]);
    expect(unmarked(fresh.roleLine(role()))).toBe("برنامه‌نویس · در حال ساخت · worktree · آماده ← در حال ساخت، در حال ساخت ← در حال تست");
    expect(asked).toEqual(["fa/common", "fa/shell"]);
  });
});

describe("filters, search, the track and the table sort read a slot by its role now too (R13)", () => {
  const claimed = task({ stage: "building", claimedBy: KAVEH });
  const ctx = (names: Record<string, string>) => dayContext(0, names);
  it("the Held by chips and their filter", () => {
    const held = GROUPS.find((g) => g.id === "holder")!;
    expect(held.present([claimed], ctx(RENAMED))).toEqual(["کاوه-2"]);
    expect(held.has(claimed, "کاوه-2", ctx(RENAMED))).toBe(true);
    expect(held.has(claimed, "Kaveh-2", ctx(RENAMED))).toBe(false);
    expect(applyQuery([claimed], { ...EMPTY_QUERY, filters: { holder: ["کاوه-2"] } }, ctx(RENAMED))).toEqual([claimed]);
  });
  it("search finds the holder by the name it shows", () => {
    expect(taskMatches(claimed, queryWords("کاوه"), RENAMED)).toBe(true);
    expect(taskMatches(claimed, queryWords("kaveh"), RENAMED)).toBe(false);
    expect(applyQuery([claimed], { ...EMPTY_QUERY, search: "کاوه" }, ctx(RENAMED))).toEqual([claimed]);
  });
  it("the stage track's title and the move's toast", () => {
    expect(releasedBy(claimed, "testing", RENAMED)).toBe("کاوه-2");
    expect(trackMove(claimed, "testing", RENAMED).done).toBe("Put in Testing. کاوه-2's claim on Building was released.");
  });
  it("the table's who sort", () => {
    const lale = task({ id: "0031", stage: "building", claimedBy: "Lale (main · session 0f0f0f0f)" });
    const ids = (names: Record<string, string>) => sortTasks([lale, claimed], "who", false, names).map((t) => t.id);
    expect(ids(NAMES)).toEqual(["0030", "0031"]);
    expect(ids({ coder: "Zand" })).toEqual(["0031", "0030"]);
  });
});

describe("check-ins, comments, Work rows and question askers name a slot by its role now too (R18)", () => {
  it("a slot's label by its role's name, anything else as written", () => {
    expect(slotLabel("Kaveh-2", "coder-2", RENAMED)).toBe("کاوه-2");
    expect(slotLabel("Kaveh-2", "coder-2", {})).toBe("Kaveh-2");
    expect(slotLabel("akbar", undefined, RENAMED)).toBe("akbar");
    expect(slotLabel("Kia-1", "constructor-1", RENAMED)).toBe("Kia-1");
  });
  it("the pane's check-ins", () => {
    const t = task({ checkins: [{ at: "2026-10-14 20:00", who: KAVEH, pct: 40, text: "on it" }, { at: "2026-10-14 20:10", who: "wt-a (main · session 1c6af0d9)", pct: null, text: "x" }] });
    const html = inNames(RENAMED, createElement(Checkins, { task: t }));
    expect(text(html)).toContain("· کاوه-2 (main · session 7b5e8856 · slot coder-2 · cc-hg)");
    expect(text(html)).toContain("· wt-a (main · session 1c6af0d9)");
    expect(html).not.toContain("Kaveh");
  });
  it("the comments' headers", () => {
    const t = task({ comments: [{ who: KAVEH, at: "2026-10-14 20:00", text: "Looks right" }] });
    const html = inNames(RENAMED, createElement(Comments, { task: t, onChanged: () => {} }));
    expect(text(html)).toContain("کاوه-2 (main · session 7b5e8856 · slot coder-2 · cc-hg) · 2026-10-14 20:00");
    expect(html).not.toContain("Kaveh");
  });
  it("the Work rows", () => {
    const row = (over: Partial<SessionRow>): SessionRow => ({
      id: "7b5e8856", who: "Kaveh-2", stages: ["building"], holds: true, found: "not-read", tokens: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
      start: null, end: null, minutes: 0, live: false, skippedLines: 0, skippedFiles: 0, sharedWith: 0, ...over,
    });
    const work = {
      rows: [row({ slot: "coder-2" }), row({ id: "1c6af0d9", who: "wt-a", holds: false })],
      stages: [],
      totals: { sessions: 2, read: 0, live: 0, shared: 0, tokens: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 }, totalTokens: 0, minutes: 0, countedTokens: 0, sharedTokens: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 }, sharedRead: 0, agentMinutes: null },
      estimate: { minutes: 0, tokens: 0, minutesPct: null, tokensPct: null },
      guess: { pct: 50, basis: "the stage" },
    } satisfies WorkData;
    const html = inNames(RENAMED, createElement(WorkView, { work, error: "", viewing: null, transcript: null, onView: () => {} }));
    expect(text(html)).toContain("کاوه-2 · holds it");
    expect(text(html)).toContain("wt-a");
    expect(html).not.toContain("Kaveh");
  });
  it("the holder ribbon on a card and on a note", () => {
    const ribbon = text(inNames(RENAMED, createElement(CardHolderRibbon, { t: task({ stage: "building", claimedBy: KAVEH }), now: 0 })));
    expect(ribbon).toContain("کاوه-2");
    expect(ribbon).not.toContain("Kaveh");
    const tip = inNames(RENAMED, createElement(CardHolderRibbon, { t: task({ stage: "building", claimedBy: KAVEH }), now: 0 }));
    expect(tip).toContain('title="Held by کاوه-2 (main · session 7b5e8856 · slot coder-2 · cc-hg)"');
    const note = { queue: "demo", file: "n.md", line: "x", heldBy: "Shapour-1 (session 7b5e8856 · slot shaper-1)", heartbeat: "", heldSince: "" } as unknown as Parameters<typeof NoteHolderRibbon>[0]["n"];
    const shaped = inNames({ shaper: "شاپور" }, createElement(NoteHolderRibbon, { n: note, now: 0 }));
    expect(text(shaped)).toContain("شاپور-1");
    expect(shaped).not.toContain("Shapour");
  });
  it("a question's asker", () => {
    const q = (over: Partial<Question>): Question => ({ n: 1, ticked: false, asker: "Kaveh-2, session 7b5e8856, 2026-10-14 20:00", text: "Which port?", escalated: false, ownerAnswered: false, ...over });
    expect(askerShown(q({ slot: "coder-2" }), RENAMED)).toBe("کاوه-2, session 7b5e8856, 2026-10-14 20:00");
    expect(askerShown(q({}), RENAMED)).toBe("Kaveh-2, session 7b5e8856, 2026-10-14 20:00");
    const html = inNames(RENAMED, createElement(AnswerBox, { task: task({}), q: q({ slot: "coder-2" }), onChanged: () => {} }));
    expect(text(html)).toContain("Q1 کاوه-2, session 7b5e8856, 2026-10-14 20:00");
    expect(html).not.toContain("Kaveh");
  });
});
