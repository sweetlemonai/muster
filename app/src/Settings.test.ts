import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { HelperInfo } from "./api";
import { HELP_TOPICS } from "./helpTopics";
import { headings } from "./manual";
import inTheApp from "../../docs/manual/04-in-the-app.md?raw";
import appSource from "./App.tsx?raw";
import { GateCell } from "./Projects";
import Settings, { AboutView, AgentsView, SpanSetting, withModel } from "./Settings";
import { DEFAULT_MACHINE, machineOf, SETTINGS_GROUPS, type SettingsInfo, spanInput, spanText } from "./settingsData";

const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/&quot;/g, '"').replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/\s+/g, " ").trim();

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
  renderToStaticMarkup(createElement(Settings, { group, onGroup: () => {}, queues: QUEUES, onChanged: () => {} }));

describe("Settings (0036)", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("lists its groups in a fixed order, the open one marked, with room for later groups", () => {
    expect(SETTINGS_GROUPS.map((g) => g.label)).toEqual(["Appearance", "Projects", "Agents", "About"]);
    const html = settings("agents");
    const list = /<nav class="settings-groups"[^>]*>([\s\S]*?)<\/nav>/.exec(html)?.[1] ?? "";
    expect([...list.matchAll(/<button[^>]*>([^<]*)<small>/g)].map((m) => m[1])).toEqual(["Appearance", "Projects", "Agents", "About"]);
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
    expect(text(settings("projects"))).toContain("Gate");
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
    expect(models).toMatch(/class="chip on" aria-pressed="true">Sonnet</);
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
    expect(agents()).toMatch(/<button type="button" disabled="">Back to 12 h<\/button>/);
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
    expect(html).toMatch(/class="chip on" aria-pressed="true">Haiku</);
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

describe("About", () => {
  it("shows the version, where the settings are kept, and every folder it reads", () => {
    const t = text(renderToStaticMarkup(createElement(AboutView, { info: INFO, queues: QUEUES })));
    expect(t).toContain("Muster 0.1.0");
    expect(t).toContain("~/Library/Application Support/ai.sweetlemon.muster projects.json");
    expect(t).toContain("settings.json");
    expect(t).toContain("helper/");
    expect(t).toContain("Queues found on their own: ~/playground /*/tasks");
    expect(t).toContain("muster: ~/playground/muster/tasks");
    expect(t).toContain("drover: ~/playground/drover/tasks");
    expect(t).toContain("Session transcripts: ~/.claude/projects");
    expect(t).toContain("Session transcripts: ~/.claude-mg/projects");
    expect(t).toContain("nothing is sent anywhere");
  });
  it("shows paths under home with ~, never the user's name; the real path is the tooltip", () => {
    const html = renderToStaticMarkup(createElement(AboutView, { info: INFO, queues: QUEUES }));
    expect(text(html)).not.toContain("/Users/someone");
    const titles = [...html.matchAll(/title="([^"]*)"/g)].map((m) => m[1]);
    for (const p of [INFO.configDir, INFO.base, ...QUEUES.map((q) => q.path), ...INFO.transcripts]) expect(titles).toContain(p);
  });
  it("leaves a path outside home as it is", () => {
    const t = text(renderToStaticMarkup(createElement(AboutView, { info: { ...INFO, transcripts: ["/Volumes/Work/.claude/projects"] }, queues: [] })));
    expect(t).toContain("Session transcripts: /Volumes/Work/.claude/projects");
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
    expect(header).toContain(`onClick={() => setView("help")}>Help</button>`);
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
    expect(header).toContain(">Settings</button>");
    expect(header).not.toMatch(/Appearance\s*<\/button>|<Appearance\w*/);
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
