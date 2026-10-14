import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { HelperInfo } from "./api";
import { HELP_TOPICS } from "./helpTopics";
import { headings } from "./manual";
import inTheApp from "../../docs/manual/04-in-the-app.md?raw";
import appSource from "./App.tsx?raw";
import { byPlace, CategoryCell, ColourCell, GateCell, GateRow, mineLayout, NameCell, PlaceCell, PrefixCell, PROJECT_COLUMNS } from "./Projects";
import Settings, { AboutView, AgentsView, SpanSetting, withModel } from "./Settings";
import { DEFAULT_MACHINE, machineOf, SETTINGS_GROUPS, type SettingsInfo, spanInput, spanText } from "./settingsData";
import css from "./styles.css?raw";
import type { Project } from "./types";

const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/&quot;/g, '"').replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/\s+/g, " ").trim();

/** One CSS rule's body. */
const rule = (sel: string) => new RegExp(`(?:^|\\n)${sel.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")} \\{([^}]*)\\}`).exec(css)?.[1] ?? "";

const INFO: SettingsInfo = {
  settings: { leaseMinutes: 720, liveMinutes: 5 },
  leaseBounds: [60, 10080],
  liveBounds: [1, 60],
  version: "0.1.0",
  home: "/Users/someone",
  configDir: "/Users/someone/Library/Application Support/ai.sweetlemon.muster",
  base: "/Users/someone/playground",
  transcripts: ["/Users/someone/.claude/projects", "/Users/someone/.claude-mg/projects"],
};
const HELPER: HelperInfo = {
  claude: "/usr/local/bin/claude",
  accounts: [
    { name: ".claude", loggedIn: true, plan: "max" },
    { name: ".claude-mg", loggedIn: false, plan: "" },
  ],
  account: ".claude",
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
    expect(SETTINGS_GROUPS.map((g) => g.label)).toEqual(["Appearance", "Display", "Projects", "Agents", "Notifications", "About"]);
    const html = settings("agents");
    const list = /<nav class="settings-groups"[^>]*>([\s\S]*?)<\/nav>/.exec(html)?.[1] ?? "";
    expect([...list.matchAll(/<button[^>]*>([^<]*)<small>/g)].map((m) => m[1])).toEqual(["Appearance", "Display", "Projects", "Agents", "Notifications", "About"]);
    expect(list.match(/aria-current="page"/g)).toHaveLength(1);
    expect(list).toMatch(/aria-current="page"[^>]*>Agents</);
    // Each says what it holds, so nothing is behind a click to find out.
    expect(text(list)).toContain("Agents The helper, the claim lease, LIVE");
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
    expect(text(row("bin/gate full"))).toBe("Gate bin/gate full Run bin/run");
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
    expect(text(html)).toContain("~/.claude (default) · in use logged in · max");
    expect(text(html)).toContain("~/.claude-mg not logged in");
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
    const changed = text(agents({ info: { ...INFO, settings: { leaseMinutes: 120, liveMinutes: 10 } } }));
    expect(changed).toContain("Claim lease 2 h A claim in Building");
    expect(changed).toContain("Back to 12 h");
    expect(changed).toContain("LIVE window 10 min How recent");
    expect(changed).toContain("Back to 5 min");
    expect(changed).not.toContain(" default ");
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
    expect(t).toContain("Session transcripts: ~/.claude/projects");
    expect(t).toContain("Session transcripts: ~/.claude-mg/projects");
    expect(t).toContain("nothing is sent anywhere");
  });
  it("shows paths under home with ~, never the user's name; the real path is the tooltip", () => {
    const html = renderToStaticMarkup(createElement(AboutView, { info: INFO, projects: PROJECTS }));
    expect(text(html)).not.toContain("/Users/someone");
    const titles = [...html.matchAll(/title="([^"]*)"/g)].map((m) => m[1]);
    for (const p of [INFO.configDir, ...PROJECTS.map((q) => q.path), ...INFO.transcripts]) expect(titles).toContain(p);
  });
  it("leaves a path outside home as it is", () => {
    const t = text(renderToStaticMarkup(createElement(AboutView, { info: { ...INFO, transcripts: ["/Volumes/Work/.claude/projects"] }, projects: [] })));
    expect(t).toContain("Session transcripts: /Volumes/Work/.claude/projects");
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
    for (const s of ["settings", "appearance", "projects", "agents", "about"]) expect(slugs).toContain(s);
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
    expect(header).toContain(`onClick={() => setView("help")}>Help</Pill>`);
    expect(appSource).toMatch(/\{view === "help" && \(\s*<Suspense fallback=\{[^}]*\}>\s*<HelpView /);
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
    expect(header).toContain(">Settings</Pill>");
    expect(header).not.toMatch(/Appearance\s*<\/(button|Pill)>|<Appearance\w*/);
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
