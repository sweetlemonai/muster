import { invoke } from "@tauri-apps/api/core";
import { createElement, type ReactElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api, type HelperInfo } from "./api";
import { HELP_TOPICS } from "./helpTopics";
import { tn } from "./i18n";
import { headings } from "./manual";
import inTheApp from "../../docs/manual/04-in-the-app.md?raw";
import appSource from "./App.tsx?raw";
import { byPlace, CategoryCell, ColourCell, GateCell, GateRow, mineLayout, NameCell, PlaceCell, PrefixCell, PROJECT_COLUMNS } from "./Projects";
import { AccountPicker } from "./Helper";
import helperSource from "./Helper.tsx?raw";
import Settings, { AboutView, AccountAdd, AccountsRow, AccountsView, AgentsView, addAccount, ProfileEditor, profileKey, removeAccount, saveAccounts, saveAgentsSetting, saveProfile, SpanSetting, withModel } from "./Settings";
import { type AccountsInfo, type AccountView, profileForm, type ProfileSave } from "./accountsData";
import settingsSource from "./Settings.tsx?raw";
import { accountsEdit, DEFAULT_MACHINE, machineOf, SETTINGS_GROUPS, type SettingsInfo, spanInput, spanText } from "./settingsData";
import css from "./styles.css?raw";
import type { Project } from "./types";

// The core, at the process boundary: set_setting is answered without a window.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));

const decode = (s: string) => s.replace(/&#x27;/g, "'").replace(/&quot;/g, '"').replace(/&amp;/g, "&");
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/&quot;/g, '"').replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/\s+/g, " ").trim();

/** One CSS rule's body. */
const rule = (sel: string) => new RegExp(`(?:^|\\n)${sel.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")} \\{([^}]*)\\}`).exec(css)?.[1] ?? "";

const INFO: SettingsInfo = {
  settings: { leaseMinutes: 720, liveMinutes: 5, claudeAccounts: ["mg", "hg"] },
  leaseBounds: [60, 10080],
  liveBounds: [1, 60],
  version: "0.1.0",
  home: "/Users/someone",
  configDir: "/Users/someone/Library/Application Support/ai.sweetlemon.muster",
  base: "/Users/someone/playground",
  transcripts: ["/Users/someone/.claude-mg", "/Users/someone/.claude-hg"],
  transcriptsUnder: "/Users/someone/playground",
};
const HELPER: HelperInfo = {
  claude: "/usr/local/bin/claude",
  accounts: [
    { name: ".claude-mg", loggedIn: true, plan: "max" },
    { name: ".claude-hg", loggedIn: false, plan: "" },
  ],
  account: ".claude-mg",
  model: "sonnet",
  models: ["sonnet", "haiku", "opus"],
  running: false,
  lines: [],
};
const QUEUES = [
  { name: "muster", path: "/Users/someone/playground/muster/tasks", counts: {} },
  { name: "drover", path: "/Users/someone/playground/drover/tasks", counts: {} },
];

const settings = (group: (typeof SETTINGS_GROUPS)[number]["id"]) =>
  renderToStaticMarkup(createElement(Settings, { group, onGroup: () => {}, queues: QUEUES, onChanged: () => {}, mode: "adhd", onMode: () => {} }));

describe("Settings (0036)", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("lists its groups in a fixed order, the open one marked, with room for later groups", () => {
    expect(SETTINGS_GROUPS.map((g) => g.label)).toEqual(["Appearance", "Display", "Projects", "Agents", "Accounts", "Notifications", "About"]);
    const html = settings("agents");
    const list = /<nav class="settings-groups"[^>]*>([\s\S]*?)<\/nav>/.exec(html)?.[1] ?? "";
    expect([...list.matchAll(/<button[^>]*>([^<]*)<small>/g)].map((m) => m[1])).toEqual(["Appearance", "Display", "Projects", "Agents", "Accounts", "Notifications", "About"]);
    expect(list.match(/aria-current="page"/g)).toHaveLength(1);
    expect(list).toMatch(/aria-current="page"[^>]*>Agents</);
    // Each says what it holds, so nothing is behind a click to find out.
    expect(text(list)).toContain("Agents The helper, the claim lease, LIVE, roles");
  });

  it("heads each group with its name, what it holds and a ? into the manual", () => {
    vi.stubGlobal("window", { matchMedia: () => ({ matches: true }) });
    vi.stubGlobal("localStorage", { getItem: () => null, setItem: () => {} });
    for (const g of SETTINGS_GROUPS) {
      const head = /<header class="settings-head">([\s\S]*?)<\/header>/.exec(settings(g.id))?.[1] ?? "";
      expect(text(head)).toContain(`${g.label} ? ${g.holds}`);
      expect(head).toContain(`aria-label="Help: ${HELP_TOPICS[g.topic].label}"`);
    }
  });

  it("holds everything the Appearance pane had: theme, accent, text size, cards, reset", () => {
    vi.stubGlobal("window", { matchMedia: () => ({ matches: true }) });
    vi.stubGlobal("localStorage", { getItem: () => null, setItem: () => {} });
    const html = text(settings("appearance"));
    for (const words of ["Match the Mac's light or dark", "Theme", "Accent", "Text size", "Cards", "Colour shows", "Back to Muster's violet"]) expect(html).toContain(words);
  });

  it("holds the Projects tab's list with New queue and Add existing", () => {
    const html = text(settings("projects"));
    expect(html).toContain("New queue…");
    expect(html).toContain("Add existing queue…");
    expect(html).toContain("Removing or hiding a project never deletes files.");
  });

  it("shows each project's gate (0023), and how to set one when there's none, without a second place to edit it", () => {
    const gate = (g: string) => text(renderToStaticMarkup(createElement(GateCell, { name: "muster", gate: g })));
    expect(gate("bin/gate full")).toBe("bin/gate full");
    expect(gate("")).toBe('none: merges wait for one. muster projects set muster gate "<command>"');
    // On a line of its own under the project's row, so no column of it crushes the folder (0061).
    const row = (g: string, hidden = false) => renderToStaticMarkup(createElement("table", null, createElement("tbody", null, createElement(GateRow, { name: "muster", gate: g, run: "bin/run", hidden }))));
    expect(text(row("bin/gate full"))).toBe("Gate bin/gate full Run bin/run Run"); // the last: Run's button (0079)
    expect(row("bin/gate")).toContain('<tr class="gate-row"><td colSpan="9">');
    // Across every column the table has, the Colour and Mine order ones too (0057).
    expect(settings("projects").match(/<th[ >]/g)).toHaveLength(PROJECT_COLUMNS);
    expect(row("bin/gate", true)).toContain('<tr class="gate-row dim">');
    expect(text(settings("projects"))).not.toMatch(/\bGate\b/);
  });

  it("shows each project's prefix (0033) and sets it in place, saying why when it's refused", () => {
    const cell = (over: Partial<Parameters<typeof PrefixCell>[0]> = {}) =>
      renderToStaticMarkup(
        createElement(PrefixCell, { name: "muster", prefix: "mu", editing: false, value: "", error: "", canSet: true, onEdit: () => {}, onChange: () => {}, onSave: () => {}, onCancel: () => {}, ...over }),
      );
    expect(text(cell())).toBe("mu Set");
    // Set is the ui Button, with a gap between it and the prefix (0061).
    expect(cell()).toMatch(/<button type="button" title="Set muster&#x27;s prefix" class="ui-button">Set<\/button>/);
    expect(rule(".prefix")).toMatch(/display: inline-flex;[^]*gap: 6px;/);
    expect(cell()).toContain('title="Set muster&#x27;s prefix"');
    expect(text(cell({ canSet: false }))).toBe("mu");
    const open = cell({ editing: true, value: "mus" });
    expect(open).toContain('value="mus"');
    expect(open).toContain('maxLength="4"');
    expect(open).toContain('aria-label="muster&#x27;s prefix"');
    expect(text(open)).toBe("Save Cancel");
    expect(open).toContain('<button type="submit" class="ui-button">Save</button>');
    // 4 characters wide, not the table's 220px, and the buttons style like the other rows' (0061).
    expect(rule(".projects-table .prefix input")).toMatch(/width: 4ch;/);
    expect(rule(".projects-bar button, .actions button, .prefix button, .limit-edit button")).toContain("border: 1px solid var(--line)");
    expect(open).not.toContain('role="alert"');
    // The core's refusal shows under the field, which stays open: no overlay.
    const refused = cell({ editing: true, value: "br", error: "braindump has br: a prefix names one project (see `muster projects`)" });
    expect(refused).toContain('<p class="err small" role="alert">braindump has br');
    expect(refused).toContain('value="br"');
    expect(text(settings("projects"))).toContain("Prefix");
  });

  it("sets each project's label in place (0044), the real name small under it, empty showing the name", () => {
    const p: Project = { key: "/p/muster/tasks", name: "muster", path: "/p/muster/tasks", source: "found", group: "", hidden: false, ok: true, label: "Muster" };
    const cell = (over: Partial<Project> = {}) => renderToStaticMarkup(createElement(NameCell, { p: { ...p, ...over }, onSave: async () => {} }));
    const html = cell();
    expect(html).toContain('value="Muster"');
    expect(html).toContain('maxLength="40"');
    expect(html).toContain('placeholder="muster"');
    expect(html).toContain('aria-label="Label for muster"');
    expect(text(html)).toBe("muster · found in ~/playground");
    // No label: the field is empty and shows the name; hidden and broken say so, as before.
    const bare = cell({ label: "", hidden: true, ok: false, source: "added" });
    expect(bare).toContain('value=""');
    expect(text(bare)).toBe("muster · added · hidden · not a queue any more");
    // Rename is gone: the label is the name the window shows, and commands keep the real one.
    expect(text(settings("projects"))).not.toContain("Rename");
    expect(text(settings("projects"))).toContain("Category");
  });

  it("sets each project's category in place (0044), empty meaning its group or Other", () => {
    const p: Project = { key: "/p/colors/tasks", name: "colors", path: "/p/colors/tasks", source: "found", group: "persianUtils", hidden: false, ok: true, category: "" };
    const cell = (over: Partial<Project> = {}) => renderToStaticMarkup(createElement(CategoryCell, { p: { ...p, ...over }, categories: ["Apps", "Other"], onSave: async () => {} }));
    const html = cell();
    expect(html).toContain('value=""');
    expect(html).toContain('placeholder="persianUtils"');
    expect(html).toContain('maxLength="24"');
    expect(html).toContain('aria-label="Category for colors"');
    // The chips to pick from lay out under it only while it has focus: none at rest.
    expect(html).not.toContain("inline-options");
    expect(cell({ group: "" })).toContain('placeholder="Other"');
    // A group with a display name (0070) shows that as where it goes.
    expect(cell({ groupName: "DashAfzar" })).toContain('placeholder="DashAfzar"');
    expect(cell({ category: "Tools" })).toContain('value="Tools"');
  });

  it("sets each project's colour (0057) from the palette, laid out in place, the current one pressed", () => {
    const p: Project = { key: "/p/muster/tasks", name: "muster", path: "/p/muster/tasks", source: "found", group: "", hidden: false, ok: true, label: "Muster", colour: "mint" };
    const html = renderToStaticMarkup(createElement(ColourCell, { p, onSave: async () => {} }));
    expect(html).toMatch(/^<div class="proj-swatches" role="group" aria-label="Colour for Muster">/);
    expect(html.match(/aria-pressed="true"[^>]*aria-label="([^"]+)"/)?.[1]).toBe("Mint");
    // A place for choosing: always laid out, nothing takes the focus.
    expect(html).not.toContain("autofocus");
    expect(renderToStaticMarkup(createElement(ColourCell, { p: { ...p, colour: "", label: "" }, onSave: async () => {} }))).toContain('aria-label="Colour for muster"');
    expect(text(settings("projects"))).toContain("Colour");
  });

  it("moves each project in the sidebar's Mine order (0057), as ⌥↑ ⌥↓ do there, the rows in that order", () => {
    const q = (name: string, category: string, rank: number | null, categoryRank: number | null) => ({ name, path: `/p/${name}/tasks`, counts: {}, label: name, category, rank, categoryRank });
    const queues = [q("muster", "Apps", 1, 1), q("zeta", "Tools", 0, 0), q("alpha", "Apps", 0, 1), q("beta", "Other", null, null)];
    const layout = mineLayout(queues);
    expect(layout).toEqual([
      { name: "Tools", keys: ["/p/zeta/tasks"] },
      { name: "Apps", keys: ["/p/alpha/tasks", "/p/muster/tasks"] },
      { name: "Other", keys: ["/p/beta/tasks"] },
    ]);
    const project = (name: string, over: Partial<Project> = {}): Project => ({ key: `/p/${name}/tasks`, name, path: `/p/${name}/tasks`, source: "found", group: "", hidden: false, ok: true, ...over });
    // Shown ones in that order, then a hidden one, as the core listed it.
    const list = [project("alpha"), project("old", { hidden: true }), project("beta"), project("muster"), project("zeta")];
    expect(byPlace(list, layout).map((p) => p.name)).toEqual(["zeta", "alpha", "muster", "beta", "old"]);
    const cell = (canUp: boolean, canDown: boolean) => renderToStaticMarkup(createElement(PlaceCell, { name: "Muster", canUp, canDown, onUp: () => {}, onDown: () => {} }));
    expect(cell(true, false)).toBe(
      '<span class="place-cell"><button aria-label="Move Muster up in the sidebar&#x27;s Mine order" title="Up one place in the sidebar&#x27;s Mine order" class="ui-button">↑</button><button disabled="" aria-label="Move Muster down in the sidebar&#x27;s Mine order" title="Down one place in the sidebar&#x27;s Mine order" class="ui-button">↓</button></span>',
    );
    expect(text(settings("projects"))).toContain("Mine order");
  });

  it("starts the helper's and the core's settings as loading, not as made-up values", () => {
    expect(text(settings("agents"))).toContain("Loading…");
    expect(text(settings("about"))).toContain("Loading…");
  });
});

describe("Agents", () => {
  const agents = (over: Partial<Parameters<typeof AgentsView>[0]> = {}) =>
    renderToStaticMarkup(createElement(AgentsView, { info: INFO, helper: HELPER, onSet: async () => null, onModel: () => {}, onHelper: () => {}, ...over }));

  it("shows the helper's account, with every account's state, and its model with the choices", () => {
    const html = agents();
    expect(text(html)).toContain("~/.claude-mg · in use logged in · max");
    expect(text(html)).toContain("~/.claude-hg not logged in");
    const models = /<div[^>]*aria-label="Helper model"[^>]*>([\s\S]*?)<\/div>/.exec(html)?.[1] ?? "";
    expect([...models.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1])).toEqual(["Sonnet", "Haiku", "Opus"]);
    expect(models).toMatch(/aria-pressed="true" class="ui-pill ui-pill--on">Sonnet</);
    expect(text(html)).toContain("Sonnet unless you pick another");
  });

  it("shows the lease and the LIVE window: the value, the default, the bounds and what each changes", () => {
    const t = text(agents());
    expect(t).toContain("Claim lease 12 h default");
    expect(t).toContain("From 1 h to 7 d");
    expect(t).toContain("muster reclaim");
    expect(t).toContain("LIVE window 5 min default");
    expect(t).toContain("From 1 min to 1 h");
    expect(t).toContain("Saved on this Mac, in ~/Library/Application Support/ai.sweetlemon.muster/settings.json");
    // A changed value says so and offers the way back.
    const changedHtml = agents({ info: { ...INFO, settings: { leaseMinutes: 120, liveMinutes: 10, claudeAccounts: ["mg", "hg"] } } });
    const changed = text(changedHtml);
    expect(changed).toContain("Claim lease 2 h A claim in Building");
    expect(changed).toContain("Back to 12 h");
    expect(changed).toContain("LIVE window 10 min How recent");
    expect(changed).toContain("Back to 5 min");
    // The accounts row's muted line names ~/.claude, "the default account" (0099): that one only.
    const never = /<p class="meta account-never">Never ~\/\.claude, the default account\.<\/p>/;
    expect(changedHtml).toMatch(never);
    expect(text(changedHtml.replace(never, ""))).not.toContain(" default ");
    // At the default, the way back is there but off: nothing moves when a value changes.
    expect(agents()).toMatch(/<button type="button" disabled="" class="ui-button">Back to 12 h<\/button>/);
  });

  it("says under Claims whether Claude Code's session states are read (0048), or why not", () => {
    expect(text(agents({ info: { ...INFO, sessionStates: "on (2 accounts)" } }))).toContain("LIVE window 5 min default");
    expect(agents({ info: { ...INFO, sessionStates: "on (2 accounts)" } })).toContain('<p class="meta">Claude Code&#x27;s session states: on (2 accounts)</p>');
    const off = text(agents({ info: { ...INFO, sessionStates: "not available: claude isn't installed" } }));
    expect(off).toContain("Claude Code's session states: not available: claude isn't installed");
    // In the Claims section, above where it's saved.
    expect(off.indexOf("Claude Code's session states")).toBeLessThan(off.indexOf("Saved on this Mac"));
    expect(off.indexOf("Claude Code's session states")).toBeLessThan(off.indexOf("The helper"));
  });

  it("puts Claims first, so the helper's slow account check can't push them down (docs/DESIGN.md)", () => {
    const h2s = (html: string) => [...html.matchAll(/<h2>([^<]*)<\/h2>/g)].map((m) => m[1]);
    expect(h2s(agents({ helper: null }))).toEqual(["Claims", "The helper"]);
    expect(h2s(agents())).toEqual(["Claims", "The helper", "Which Claude account should the helper use?"]);
    // Claims render the same while the accounts load and after.
    const claims = (html: string) => /<section><h2>Claims<\/h2>[\s\S]*?<\/section>/.exec(html)?.[0];
    expect(claims(agents({ helper: null }))).toBe(claims(agents()));
  });

  it("lights the picked model as soon as the core saved it, without asking for the accounts again", () => {
    expect(withModel(HELPER, "opus")).toEqual({ ...HELPER, model: "opus" });
    expect(withModel(null, "opus")).toBeNull();
    const html = agents({ helper: withModel(HELPER, "haiku") });
    expect(html).toMatch(/aria-pressed="true" class="ui-pill ui-pill--on">Haiku</);
  });

  it("without the claude command says so, and still shows the lease and LIVE", () => {
    const t = text(agents({ helper: { ...HELPER, claude: null } }));
    expect(t).toContain("Couldn't find the claude command");
    expect(t).toContain("Claim lease 12 h");
  });

  it("shows a field for each span with the current value typed out, and its bounds", () => {
    const html = renderToStaticMarkup(
      createElement(SpanSetting, { label: "Claim lease", minutes: 90, def: 720, bounds: [60, 10080], what: "x", onSet: async () => null }),
    );
    expect(html).toContain('value="90m"');
    expect(html).toContain('aria-label="Claim lease, as 30m, 2h or 1d"');
    expect(text(html)).toContain("Claim lease 1 h 30 min");
  });
});

describe("the Claude accounts Muster reads (0099)", () => {
  const row = (over: Partial<Parameters<typeof AccountsRow>[0]> = {}) => ({ labels: ["mg", "hg"], typed: "", error: "", onTyped: () => {}, onAdd: () => {}, onRemove: () => {}, ...over });
  const html = (over: Partial<Parameters<typeof AccountsRow>[0]> = {}) => renderToStaticMarkup(createElement(AccountsRow, row(over)));

  /** The row's buttons by their aria-label, each with its click. */
  function buttons(node: ReactNode): { label: string; click: () => void }[] {
    if (!node || typeof node !== "object") return [];
    if (Array.isArray(node)) return node.flatMap(buttons);
    const el = node as ReactElement<{ children?: ReactNode; onClick?: () => void; "aria-label"?: string }>;
    if (typeof el.type === "function") return buttons((el.type as (p: unknown) => ReactNode)(el.props));
    const own = el.type === "button" ? [{ label: el.props["aria-label"] ?? "", click: () => el.props.onClick?.() }] : [];
    return [...own, ...buttons(el.props.children)];
  }
  /** The row's form, whose submit (Enter in the field) adds. */
  function form(node: ReactNode): ReactElement<{ onSubmit: (e: { preventDefault: () => void }) => void }> | null {
    if (!node || typeof node !== "object") return null;
    if (Array.isArray(node)) return node.map(form).find((f) => f) ?? null;
    const el = node as ReactElement<{ children?: ReactNode; onSubmit?: () => void }>;
    if (el.type === "form") return el as ReactElement<{ onSubmit: (e: { preventDefault: () => void }) => void }>;
    return form(el.props.children);
  }

  it("shows each account as a chip, cc-mg and cc-hg by default, each with its ✕, the add field at the end and what's never read", () => {
    const agents = renderToStaticMarkup(createElement(AgentsView, { info: { ...INFO, sessionStates: "on (2 accounts)" }, helper: null, onSet: async () => null, onModel: () => {}, onHelper: () => {} }));
    // A row of Agents' first section, under the session states it governs and above where it's saved.
    const section = /<section><h2>Claims<\/h2>[\s\S]*?<\/section>/.exec(agents)?.[0] ?? "";
    expect(text(section)).toContain(
      "Claude Code's session states: on (2 accounts) Claude accounts Muster reads: cc-mg ✕ cc-hg ✕ Add an account Never ~/.claude, the default account. Saved on this Mac",
    );
    expect(section).toContain('<div class="setting" role="group" aria-label="Claude accounts Muster reads">');
    expect(section).toContain('aria-label="Remove cc-mg from the accounts Muster reads"');
    expect(section).toContain('aria-label="Remove cc-hg from the accounts Muster reads"');
    // A visible label on the field, edited in place: no popover, no overlay.
    expect(section).toMatch(/<label class="account-add"><span class="meta">Add an account<\/span> ?<input/);
    expect(section).not.toMatch(/popover|dialog/);
    // The ✕ buttons come first, then the field: the tab order is the reading order.
    const own = section.slice(section.indexOf('aria-label="Claude accounts Muster reads"'));
    expect(own.indexOf("Remove cc-hg")).toBeGreaterThan(0);
    expect(own.indexOf("Remove cc-hg")).toBeLessThan(own.indexOf("<input"));
    expect(section).toContain('<p class="meta account-never">Never ~/.claude, the default account.</p>');
  });

  it("removing one saves the rest", async () => {
    const removed: string[] = [];
    const tree = AccountsRow(row({ onRemove: (l) => removed.push(l) }));
    const x = buttons(tree).find((b) => b.label === "Remove cc-hg from the accounts Muster reads");
    x?.click();
    expect(removed).toEqual(["hg"]);
    expect(accountsEdit(["mg", "hg"], { remove: "hg" })).toEqual({ value: "mg" });
    expect(accountsEdit(["mg"], { remove: "mg" })).toEqual({ value: "" });
    // The value goes to the core as set_setting("claude-accounts", …).
    await api.setSetting("claude-accounts", "mg");
    expect(invoke).toHaveBeenCalledWith("set_setting", { name: "claude-accounts", value: "mg" });
  });

  it("Enter in the field adds what's typed, trimmed and without a leading cc-", () => {
    let added = 0;
    const f = form(AccountsRow(row({ typed: "cc-ih", onAdd: () => added++ })));
    let prevented = false;
    f?.props.onSubmit({ preventDefault: () => (prevented = true) });
    expect(added).toBe(1);
    expect(prevented).toBe(true);
    expect(accountsEdit(["mg", "hg"], { add: "ih" })).toEqual({ value: "mg hg ih" });
    expect(accountsEdit(["mg", "hg"], { add: "  cc-ih " })).toEqual({ value: "mg hg ih" });
    expect(accountsEdit([], { add: "cc-mg" })).toEqual({ value: "mg" });
    expect(accountsEdit(["mg"], { add: "work2" })).toEqual({ value: "mg work2" });
  });

  it("refuses an empty, invalid or repeated label, and ~/.claude, saying why where a screen reader hears it", () => {
    const refused = (typed: string, list = ["mg", "hg"]) => {
      const r = accountsEdit(list, { add: typed });
      return "error" in r ? r.error : `saved ${r.value}`;
    };
    expect(refused("")).toBe("Type an account's label, like mg for cc-mg.");
    expect(refused("  cc- ")).toBe("Type an account's label, like mg for cc-mg.");
    expect(refused(".claude")).toBe("Never ~/.claude: it's the default account, which Muster doesn't read.");
    expect(refused("~/.claude")).toBe("Never ~/.claude: it's the default account, which Muster doesn't read.");
    expect(refused("~/.claude/")).toBe("Never ~/.claude: it's the default account, which Muster doesn't read.");
    for (const bad of ["MG", "m g", "../x", ".claude-mg", "abcdefghijklm", "mg!", "نگار"]) {
      expect(refused(bad)).toBe(`"${bad}" is not a valid account label: 1 to 12 lowercase letters and digits, like mg`);
    }
    // A long paste is cut in the message, as the core cuts it.
    expect(refused("x".repeat(40))).toBe(`"${"x".repeat(24)}" is not a valid account label: 1 to 12 lowercase letters and digits, like mg`);
    expect(refused("abcdefghijkl")).toBe("saved mg hg abcdefghijkl");
    expect(refused("hg")).toBe("cc-hg is already read.");
    expect(refused("cc-mg")).toBe("cc-mg is already read.");
    // The message, the client's or the core's, shows in a live region that's always there.
    expect(html()).toContain('<p class="err account-error" aria-live="polite"></p>');
    expect(html({ error: "cc-hg is already read." })).toContain('<p class="err account-error" aria-live="polite">cc-hg is already read.</p>');
    expect(html({ error: "settings.json: permission denied" })).toContain('aria-live="polite">settings.json: permission denied</p>');
  });

  it("warns when the list is empty: Muster reads no Claude sessions", () => {
    const empty = html({ labels: [] });
    expect(text(empty)).toBe("Claude accounts Muster reads: none: Muster reads no Claude sessions Add an account Never ~/.claude, the default account.");
    expect(empty).toContain('<span class="accounts-none">none: Muster reads no Claude sessions</span>');
    expect(rule(".accounts-none")).toContain("color: var(--warn)");
    expect(html()).not.toContain("accounts-none");
  });

  /** The first element in the tree that `pick` takes, without running function components. */
  function element<P>(node: ReactNode, pick: (el: ReactElement<P>) => boolean): ReactElement<P> | null {
    if (!node || typeof node !== "object") return null;
    if (Array.isArray(node)) return node.map((n) => element(n, pick)).find((e) => e) ?? null;
    const el = node as ReactElement<P & { children?: ReactNode }>;
    return pick(el) ? el : element(el.props.children, pick);
  }

  it("saves the list through Agents as claude-accounts: a removal, an add without its cc-, and the refusals (review R4)", async () => {
    const sent: [string, string][] = [];
    const tree = AgentsView({
      info: INFO,
      helper: null,
      onModel: () => {},
      onHelper: () => {},
      onSet: async (name, value) => {
        sent.push([name, value]);
        return value.includes("ih") ? "settings.json: permission denied" : null;
      },
    });
    type Row = { labels: string[]; onSet: (v: string) => Promise<string | null> };
    const accounts = element<Row>(tree, (el) => Array.isArray((el.props as Partial<Row>).labels));
    expect(accounts?.props.labels).toEqual(["mg", "hg"]);
    const onSet = accounts?.props.onSet ?? (async () => "no accounts row");
    // A removal saves the rest.
    expect(await saveAccounts(["mg", "hg"], { remove: "hg" }, onSet)).toBeNull();
    expect(sent).toEqual([["claude-accounts", "mg"]]);
    // An add, trimmed and without its cc-; the core's refusal comes back to show.
    expect(await saveAccounts(["mg", "hg"], { add: "  cc-ih " }, onSet)).toBe("settings.json: permission denied");
    expect(sent).toEqual([["claude-accounts", "mg"], ["claude-accounts", "mg hg ih"]]);
    expect(await saveAccounts([], { add: "cc-mg" }, onSet)).toBeNull();
    expect(sent.at(-1)).toEqual(["claude-accounts", "mg"]);
    // The client's refusal never reaches the core.
    expect(await saveAccounts(["mg"], { add: "mg" }, onSet)).toBe("cc-mg is already read.");
    expect(await saveAccounts(["mg"], { add: "~/.claude" }, onSet)).toBe("Never ~/.claude: it's the default account, which Muster doesn't read.");
    expect(sent).toHaveLength(3);
    // The row saves through it, and clears the field only after an add was saved.
    expect(settingsSource).toContain("const refused = await saveAccounts(labels, edit, onSet);");
    expect(settingsSource).toContain('if (refused === null && "add" in edit) setTyped("");');
  });

  it("asks for the helper's accounts again once the list is saved, so its picker on the same page follows (review R2)", async () => {
    const calls: string[] = [];
    const to = {
      set: async (name: string, value: string) => {
        calls.push(`set ${name} ${value}`);
        return INFO;
      },
      onInfo: () => calls.push("info"),
      onChanged: () => calls.push("changed"),
      onHelper: () => calls.push("helper"),
    };
    expect(await saveAgentsSetting("claude-accounts", "mg", to)).toBeNull();
    expect(calls).toEqual(["set claude-accounts mg", "info", "changed", "helper"]);
    // The lease and LIVE don't change the accounts: no slow check again.
    calls.length = 0;
    expect(await saveAgentsSetting("lease", "2h", to)).toBeNull();
    expect(await saveAgentsSetting("live", "10m", to)).toBeNull();
    expect(calls).toEqual(["set lease 2h", "info", "changed", "set live 10m", "info", "changed"]);
    // Refused: the core's words come back, and nothing is asked again.
    calls.length = 0;
    const refusing = { ...to, set: () => Promise.reject(new Error("settings.json: permission denied")) };
    expect(await saveAgentsSetting("claude-accounts", "mg", refusing)).toBe("Error: settings.json: permission denied");
    expect(calls).toEqual([]);
    // Agents saves through it, its onHelper asking helper_info again.
    expect(settingsSource).toContain("saveAgentsSetting(name, value, { set: api.setSetting, onInfo, onChanged, onHelper: () => setAsked((n) => n + 1) })");
  });

  it("the helper's picker says why it has no account: an empty list, or no folder for the ones on it (review R7)", () => {
    const picker = (listed: number | null) => text(renderToStaticMarkup(createElement(AccountPicker, { info: { ...HELPER, accounts: [], account: "" }, listed, onPicked: () => {}, onRefresh: () => {} })));
    expect(picker(0)).toContain("No Claude account is on Muster's list of accounts it may use. Add one in Settings, Agents.");
    expect(picker(2)).toContain("No account on Muster's list of accounts it may use has a ~/.claude-<label> folder. Check the list in Settings, Agents.");
    expect(picker(2)).not.toContain("is on Muster's list");
    expect(picker(null)).toContain("No Claude account Muster may use was found. Check its list in Settings, Agents.");
    // With an account, none of them.
    const one = text(renderToStaticMarkup(createElement(AccountPicker, { info: HELPER, listed: 2, onPicked: () => {}, onRefresh: () => {} })));
    expect(one).not.toMatch(/No (?:Claude )?account/);
    // Agents counts its own list; the Helper tab asks Settings for it.
    const agents = (claudeAccounts: string[]) => text(renderToStaticMarkup(createElement(AgentsView, { info: { ...INFO, settings: { ...INFO.settings, claudeAccounts } }, helper: { ...HELPER, accounts: [], account: "" }, onSet: async () => null, onModel: () => {}, onHelper: () => {} })));
    expect(agents(["mg"])).toContain("has a ~/.claude-<label> folder");
    expect(agents([])).toContain("No Claude account is on Muster's list");
    expect(helperSource).toContain("listed={listed}");
    expect(helperSource).toMatch(/api\.settingsInfo\(\)\.then\(\s*\(s\) => live && setListed\(s\.settings\.claudeAccounts\.length\)/);
  });

  it("styles the row with logical properties and the theme's tokens only", () => {
    const rules = [".accounts-row", ".account-chip", ".account-x", ".account-x:hover, .account-x:focus-visible", ".accounts-none", ".account-add", ".account-add input", ".account-error"].map(rule);
    for (const r of rules) expect(r).not.toBe("");
    const all = rules.join(";");
    expect(all).not.toMatch(/(?:margin|padding|border)-(?:left|right|top|bottom)|(?<![-\w])(?:width|height):|text-align: (?:left|right)/);
    for (const v of all.match(/var\(--[\w-]+\)/g) ?? []) expect(["var(--text)", "var(--muted)", "var(--line)", "var(--hover)", "var(--warn)"]).toContain(v);
  });
});

const project = (name: string, path: string, source: Project["source"], more: Partial<Project> = {}): Project => ({
  key: path,
  name,
  path,
  source,
  group: "",
  hidden: false,
  ok: true,
  gate: "",
  ...more,
});
const PROJECTS: Project[] = [
  project("drover", "/Users/someone/playground/drover/tasks", "found"),
  project("colors", "/Users/someone/playground/persianUtils/colors/tasks", "found", { group: "persianUtils" }),
  project("old", "/Users/someone/playground/old/tasks", "found", { hidden: true }),
  project("notes", "/Volumes/Work/notes/tasks", "added"),
  project("gone", "/Users/someone/elsewhere/tasks", "added", { ok: false }),
];
const about = (projects: Project[] | null, error = "") => text(renderToStaticMarkup(createElement(AboutView, { info: INFO, projects, error })));

describe("About", () => {
  it("shows the version, where the settings are kept, and every folder it reads", () => {
    const t = about(PROJECTS);
    expect(t).toContain("Muster 0.1.0");
    expect(t).toContain("~/Library/Application Support/ai.sweetlemon.muster projects.json");
    expect(t).toContain("settings.json");
    expect(t).toContain("helper/");
    // The allowed account folders (0099), and only the projects under ~/playground in them.
    expect(t).toContain("Session transcripts: ~/.claude-mg Session transcripts: ~/.claude-hg In them, only projects under ~/playground");
    expect(t).not.toMatch(/~\/\.claude\b(?!-)/);
    expect(t).toContain("settings.json : the Claude accounts Muster reads, the claim lease");
    expect(t).toContain("nothing is sent anywhere");
  });
  it("says when no account's sessions are read", () => {
    const none = { ...INFO, settings: { ...INFO.settings, claudeAccounts: [] }, transcripts: [] };
    const t = text(renderToStaticMarkup(createElement(AboutView, { info: none, projects: [] })));
    expect(t).toContain("Session transcripts: none, no Claude account is on the list in Agents");
    expect(t).not.toContain("only projects under");
  });
  it("with accounts on the list but none of their folders there, says that, not that the list is empty (review R7)", () => {
    const t = text(renderToStaticMarkup(createElement(AboutView, { info: { ...INFO, transcripts: [] }, projects: [] })));
    expect(t).toContain("Session transcripts: none, no account on the list in Agents has a ~/.claude-<label> folder");
    expect(t).not.toContain("no Claude account is on the list");
    expect(t).not.toContain("only projects under");
  });
  it("shows paths under home with ~, never the user's name; the real path is the tooltip", () => {
    const html = renderToStaticMarkup(createElement(AboutView, { info: INFO, projects: PROJECTS }));
    expect(text(html)).not.toContain("/Users/someone");
    const titles = [...html.matchAll(/title="([^"]*)"/g)].map((m) => m[1]);
    for (const p of [INFO.configDir, ...PROJECTS.map((q) => q.path), ...INFO.transcripts, INFO.transcriptsUnder]) expect(titles).toContain(p);
  });
  it("names the bundled font and its licence, and where the licence is (mu-150)", () => {
    const t = about(PROJECTS);
    expect(t).toContain("Fonts Persian text is drawn in Vazirmatn by Saber Rastikerdar, under the SIL Open Font License 1.1");
    expect(t).toContain("Muster.app/Contents/Resources/fonts/Vazirmatn-OFL.txt");
  });
  it("leaves a path outside home as it is", () => {
    const t = text(renderToStaticMarkup(createElement(AboutView, { info: { ...INFO, transcripts: ["/Volumes/Work/.claude-mg"], transcriptsUnder: "/Volumes/Work/playground" }, projects: [] })));
    expect(t).toContain("Session transcripts: /Volumes/Work/.claude-mg In them, only projects under /Volumes/Work/playground");
  });

  it("says which queues it found on its own and which you added, with home paths as ~ (owner, 2026-10-13)", () => {
    const t = about(PROJECTS);
    expect(t).toContain("Found on their own (~/playground/<project>/tasks and one level deeper)");
    expect(t).not.toContain("*/tasks");
    const found = t.slice(t.indexOf("Found on their own"), t.indexOf("Added by you"));
    const added = t.slice(t.indexOf("Added by you"), t.indexOf("Session transcripts"));
    expect(found).toContain("drover: ~/playground/drover/tasks");
    expect(found).toContain("colors: ~/playground/persianUtils/colors/tasks");
    expect(found).toContain("old: ~/playground/old/tasks (hidden, not read)");
    expect(found).not.toContain("notes");
    expect(added).toContain("notes: /Volumes/Work/notes/tasks");
    expect(added).toContain("gone: ~/elsewhere/tasks (not a queue any more)");
    expect(added).not.toContain("drover");
    expect(t).not.toContain("/Users/someone/playground/drover");
  });

  it("says so plainly when you added none, found none, or the list isn't there", () => {
    const onlyFound = about(PROJECTS.filter((p) => p.source === "found"));
    expect(onlyFound).toContain("Added by you: none. Add a queue kept anywhere else in Projects.");
    const onlyAdded = about(PROJECTS.filter((p) => p.source === "added"));
    expect(onlyAdded).toContain("~/playground/<project>/tasks and one level deeper): none");
    expect(about(null)).toContain("Loading the projects…");
    expect(about(null, "boom")).toContain("Can't read the projects: boom");
  });
});

describe("the manual and Settings", () => {
  it("the manual describes every group of Settings", () => {
    const slugs = headings(inTheApp).map((h) => h.slug);
    for (const s of ["settings", "appearance", "projects", "agents", "accounts", "about"]) expect(slugs).toContain(s);
  });
  // settings.json holds the language once one is picked (mu-146), and the digits and calendar
  // (mu-151): About and the manual say so.
  it("the manual has a Language section, and it and About list the language in settings.json", () => {
    const slugs = headings(inTheApp).map((h) => h.slug);
    expect(slugs).toContain("language");
    expect(slugs.indexOf("language")).toBeLessThan(slugs.indexOf("appearance"));
    expect(inTheApp).toMatch(/`settings\.json` \([^)]*the language, digits and calendar, once you pick them[^)]*\)/);
    expect(about(PROJECTS)).toMatch(/settings\.json : [^;]*the language, digits and calendar, once you pick them/);
    // And the Language section says what Digits and Calendar do (mu-151, review R7).
    const language = inTheApp.slice(inTheApp.indexOf("### Language"), inTheApp.indexOf("### Appearance"));
    expect(language).toMatch(/\*\*Digits\*\*[^\n]*\*\*Calendar\*\*/);
  });
});

describe("Help lives only in the header (owner, 2026-10-13)", () => {
  it("Settings has no Help group", () => {
    expect(SETTINGS_GROUPS.map((g) => g.id)).not.toContain("help");
    const list = /<nav class="settings-groups"[^>]*>([\s\S]*?)<\/nav>/.exec(settings("about"))?.[1] ?? "";
    expect(text(list)).not.toMatch(/\bHelp\b/);
  });
  it("the header's Help tab still opens the manual, as the ? buttons do", () => {
    const header = /<header className="top">([\s\S]*?)<\/header>/.exec(appSource)?.[1] ?? "";
    expect(header).toContain(`onClick={() => setView("help")}>{t("app.view.help")}</Pill>`);
    expect(tn("shell", "app.view.help")).toBe("Help");
    expect(appSource).toMatch(/\{view === "help" && \(\s*<Suspense fallback=\{<p className="none">\{t\("common:loading"\)\}<\/p>\}>\s*<HelpView /);
    expect(appSource).toMatch(/const openHelp = useCallback\(\s*\(topic: HelpTopic\) => \{\s*setView\("help"\);/);
  });
  it("the manual's Settings section has no Help group: Help is the header's tab", () => {
    expect(headings(inTheApp).filter((h) => h.title === "Help")).toHaveLength(1);
  });
});

describe("Appearance lives only in Settings (owner, 2026-10-13)", () => {
  it("the header has no Appearance button, and Settings still has the Appearance group", () => {
    vi.stubGlobal("window", { matchMedia: () => ({ matches: true }) });
    vi.stubGlobal("localStorage", { getItem: () => null, setItem: () => {} });
    const header = /<header className="top">([\s\S]*?)<\/header>/.exec(appSource)?.[1] ?? "";
    expect(header).toContain('>{t("app.view.settings")}</Pill>');
    expect(tn("shell", "app.view.settings")).toBe("Settings");
    expect(header).not.toMatch(/Appearance\s*<\/(button|Pill)>|<Appearance\w*|app\.view\.appearance/);
    expect(SETTINGS_GROUPS.some((g) => g.id === "appearance" && g.label === "Appearance")).toBe(true);
    expect(text(settings("appearance"))).toContain("Appearance ? Theme, accent, text size, cards");
    vi.unstubAllGlobals();
  });
  it("the manual doesn't send you to the header for it", () => {
    expect(inTheApp).not.toMatch(/\*\*Appearance\*\* (beside it|, which opens)/);
  });
});

describe("settings data", () => {
  it("says spans the way the core does, and types them back", () => {
    expect([spanText(720), spanText(90), spanText(1800), spanText(5), spanText(0)]).toEqual(["12 h", "1 h 30 min", "1 d 6 h", "5 min", "0 min"]);
    expect([spanInput(720), spanInput(1440), spanInput(90), spanInput(5)]).toEqual(["12h", "1d", "90m", "5m"]);
  });
  it("reads the state's settings, the defaults when missing or odd", () => {
    expect(machineOf({ settings: { leaseMinutes: 60, liveMinutes: 2 } })).toEqual({ leaseMinutes: 60, liveMinutes: 2 });
    expect(machineOf(null)).toEqual(DEFAULT_MACHINE);
    expect(machineOf({})).toEqual(DEFAULT_MACHINE);
    expect(machineOf({ settings: { leaseMinutes: -1, liveMinutes: 1.5 } })).toEqual(DEFAULT_MACHINE);
    expect(DEFAULT_MACHINE).toEqual({ leaseMinutes: 720, liveMinutes: 5 });
  });
});

describe("Settings › Accounts (0113)", () => {
  const at = (y: number, mo: number, d: number, h: number, mi: number) => new Date(y, mo - 1, d, h, mi).getTime() / 1000;
  const NOW = at(2026, 10, 14, 11, 5);
  const MG: AccountView = {
    provider: "claude", label: "mg", name: "cc-mg", plan: "Max 5x", roles: ["builder", "tester"], models: ["opus"], concurrency: 2, reserve: 20, paused: false, projects: [],
    fiveHour: { usedPercentage: 42, resetsAt: at(2026, 10, 14, 14, 20), stale: false, headroom: 38 },
    sevenDay: { usedPercentage: 85, resetsAt: at(2026, 10, 18, 9, 0), stale: false, headroom: -5 },
    readingAt: NOW - 180, source: "statusline",
  };
  const HG: AccountView = { ...MG, label: "hg", name: "cc-hg", plan: "", roles: [], models: [], fiveHour: null, sevenDay: null, readingAt: null, source: null, projects: ["drover"] };
  const calls = { add: async () => {}, remove: async () => {}, save: async () => {}, after: () => {} };
  const view = (info: AccountsInfo | null, error = "") => renderToStaticMarkup(createElement(AccountsView, { info, error, calls }));

  it("is a group of Settings after Agents, its ? opening its section of the manual", () => {
    expect(SETTINGS_GROUPS.find((g) => g.id === "accounts")).toEqual({ id: "accounts", label: "Accounts", holds: "Each account's use, reserve and limits", topic: "accounts" });
    expect(HELP_TOPICS.accounts).toEqual({ file: "04-in-the-app.md", slug: "accounts", label: "Accounts" });
    expect(text(settings("accounts"))).toContain("Accounts ? Each account's use, reserve and limits");
  });

  it("shows each account's reading and its fields inline, and the one quiet line on balancing", () => {
    const html = view({ accounts: [MG, HG], now: NOW });
    const t = text(html);
    expect(t).toContain("No automatic balancing across accounts (decision 0013).");
    expect(html).toContain('<p class="meta">No automatic balancing across accounts (decision 0013).</p>');
    expect(t).toContain("cc-mg Claude · Max 5x");
    expect(t).toContain("5 h: 42% used, resets 14:20, 38% to spare above the reserve");
    expect(t).toContain("7 d: 85% used, resets 18 Oct 09:00, 5% into the reserve");
    expect(t).toContain("Read 3 min ago, from the status line.");
    // Every field in place, with its value: no overlay, nothing behind a click.
    for (const [label, value] of [["Plan", "Max 5x"], ["Reserve %", "20"], ["Concurrency", "2"], ["Roles", "builder, tester"], ["Models", "opus"]]) {
      expect(decode(html)).toMatch(new RegExp(`<label class="profile-field"><span class="meta">${label.replace("%", "\\%")}</span> <input[^>]* value="${value}"`));
    }
    expect(html).toMatch(/<label class="profile-field"><input type="checkbox"\/> Paused<\/label>/);
    expect(html).toContain('aria-label="cc-mg&#x27;s profile"');
    expect(html).toMatch(/<button type="button" class="ui-button">Remove…<\/button>/);
    // An account with no reading says so, and names the projects pinned to it.
    expect(t).toContain("cc-hg Claude 5 h: no reading 7 d: no reading No reading yet.");
    expect(t).toContain("Pinned to it: drover");
    expect(html).not.toMatch(/role="dialog"|aria-modal/);
  });

  it("adds an account by label and provider, its refusal shown in place", () => {
    const t = text(view({ accounts: [MG], now: NOW }));
    expect(t).toContain("Add an account Label Claude Codex Add");
    const add = (error: string) => renderToStaticMarkup(createElement(AccountAdd, { typed: "x", provider: "claude", error, busy: false, onTyped: () => {}, onProvider: () => {}, onAdd: () => {} }));
    expect(add("")).toMatch(/<button type="button" aria-pressed="true" class="ui-pill ui-pill--on">Claude<\/button>/);
    const refused = add("cc-x isn't on the allowlist of Claude accounts (Settings): add it there first");
    expect(refused).toContain('<p class="err profile-error" aria-live="polite">cc-x isn&#x27;t on the allowlist of Claude accounts (Settings): add it there first</p>');
    // Add, Save and Remove look like Settings' other buttons, never the system's (QA).
    expect(add("")).toMatch(/<button type="submit" class="ui-button">Add<\/button>/);
    expect(rule(".settings-sections section > button, .setting-edit button, .profile-form .ui-button")).toBe(" border: 1px solid var(--line); background: var(--bg); border-radius: 6px; padding: 4px 12px; ");
  });

  /** The ref the button saying `label` is given, in a tree of plain elements and hook-free
   *  components. */
  function refOf(node: ReactNode, label: string): unknown {
    if (!node || typeof node !== "object") return undefined;
    if (Array.isArray(node)) return node.map((n: ReactNode) => refOf(n, label)).find((r) => r !== undefined);
    const el = node as ReactElement<{ children?: ReactNode; ref?: unknown }>;
    if (typeof el.type === "function") return refOf((el.type as (p: unknown) => ReactNode)(el.props), label);
    if (el.type === "button" && el.props.children === label) return el.props.ref ?? null;
    return refOf(el.props.children, label);
  }

  it("keeps the keyboard's place through Remove's ask: Keep, back to Remove…, then the add form (review R6)", () => {
    const removeRef = { current: null };
    const keepRef = { current: null };
    const editor = (removing: boolean) =>
      ProfileEditor({ a: MG, form: profileForm(MG), error: "", busy: false, removing, removeRef, keepRef, onForm: () => {}, onSave: () => {}, onRemoving: () => {}, onRemove: () => {} });
    expect(refOf(editor(false), "Remove…")).toBe(removeRef);
    expect(refOf(editor(true), "Keep")).toBe(keepRef);
    // Profile moves the focus once the ask opens or closes, and a removal hands it to the add form.
    expect(settingsSource).toMatch(/useEffect\(\(\) => \{\s*if \(removing\) keepButton\.current\?\.focus\(\);\s*else if \(backToRemove\.current\) \{\s*backToRemove\.current = false;\s*removeButton\.current\?\.focus\(\);/);
    expect(settingsSource).toContain("backToRemove.current = !asking;");
    expect(settingsSource).toContain("if (refused === null) onRemoved();");
    expect(settingsSource).toContain("onRemoved={() => addField.current?.focus()}");
    expect(settingsSource).toContain("<AddAccount calls={calls} field={addField} />");
    expect(settingsSource).toContain("<input ref={field} value={typed}");
  });

  it("names the account by provider and label when it saves or removes, so cc-mg and codex-mg stay apart (review R5)", () => {
    expect(settingsSource).toContain("saveProfile(a.name, form, calls)");
    expect(settingsSource).toContain("removeAccount(a.name, calls)");
  });

  it("says when there are no accounts, while they load, and when they can't be read", () => {
    expect(text(view({ accounts: [], now: NOW }))).toContain("No accounts yet. Add one below.");
    expect(text(view(null))).toContain("Loading…");
    expect(view(null, "accounts.json: permission denied")).toContain('<p class="err" role="alert">Can\'t read the accounts: accounts.json: permission denied</p>'.replace("'", "&#x27;"));
  });

  it("saves a profile through the core, checking its bounds first, and shows the core's refusal as is", async () => {
    const sent: ProfileSave[] = [];
    let after = 0;
    const to = { ...calls, save: async (p: ProfileSave) => void sent.push(p), after: () => void after++ };
    const form = profileForm(MG);
    expect(await saveProfile("mg", { ...form, reserve: "25" }, to)).toBeNull();
    expect(sent).toEqual([{ label: "mg", plan: "Max 5x", roles: ["builder", "tester"], models: ["opus"], concurrency: 2, reserve: 25, paused: false }]);
    expect(after).toBe(1);
    // The client's refusal never reaches the core.
    expect(await saveProfile("mg", { ...form, reserve: "95" }, to)).toBe("Reserve is a whole percent from 0 to 90.");
    expect(sent).toHaveLength(1);
    const refusing = { ...to, save: () => Promise.reject("accounts.json: permission denied") };
    expect(await saveProfile("mg", form, refusing)).toBe("accounts.json: permission denied");
    expect(after).toBe(1);
  });

  it("adds and removes through the core, with its refusals: the company account, one off the allowlist", async () => {
    const sent: string[] = [];
    let after = 0;
    const to = {
      ...calls,
      add: async (label: string, provider: string) => {
        if (label === "claude") throw "~/.claude is the company account: Muster never adds it";
        sent.push(`add ${label} ${provider}`);
      },
      remove: async (label: string) => void sent.push(`remove ${label}`),
      after: () => void after++,
    };
    expect(await addAccount(" cc-hg ", "claude", to)).toBeNull();
    expect(await addAccount("work", "codex", to)).toBeNull();
    expect(await addAccount("claude", "claude", to)).toBe("~/.claude is the company account: Muster never adds it");
    expect(await addAccount("  ", "claude", to)).toBe("Type an account's label, like mg for cc-mg.");
    expect(await removeAccount("hg", to)).toBeNull();
    expect(sent).toEqual(["add hg claude", "add work codex", "remove hg"]);
    expect(after).toBe(3);
  });

  it("keeps a profile's typing when only a reading changes, and starts afresh when the profile does", () => {
    expect(profileKey(MG)).toBe(profileKey({ ...MG, fiveHour: null, readingAt: NOW }));
    expect(profileKey(MG)).not.toBe(profileKey({ ...MG, reserve: 25 }));
  });

  it("About lists the accounts' files", () => {
    const t = about(PROJECTS);
    expect(t).toContain("accounts.json : each account's profile: plan, roles, models, concurrency, reserve, paused");
    expect(t).toContain("usage/ : each account's readings of its 5-hour and weekly use, the last 7 days");
  });

  it("the manual describes it", () => {
    expect(headings(inTheApp).map((h) => h.slug)).toContain("accounts");
    expect(inTheApp).toContain("No automatic balancing across accounts");
  });
});
