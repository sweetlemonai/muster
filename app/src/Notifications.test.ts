import { createElement, type ReactElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { HELP_TOPICS } from "./helpTopics";
import { NotificationsView } from "./Notifications";
import { SETTINGS_GROUPS, type SettingsInfo } from "./settingsData";

const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/&quot;/g, '"').replace(/\s+/g, " ").trim();

const INFO: SettingsInfo = {
  settings: { leaseMinutes: 720, liveMinutes: 5, notify: true, claudeAccounts: ["mg", "hg"] },
  leaseBounds: [60, 10080],
  liveBounds: [1, 60],
  version: "0.1.0",
  home: "/Users/someone",
  configDir: "/Users/someone/Library/Application Support/ai.sweetlemon.muster",
  base: "/Users/someone/playground",
  transcripts: [],
  transcriptsUnder: "/Users/someone/playground",
};

const view = (over: Partial<Parameters<typeof NotificationsView>[0]> = {}) =>
  renderToStaticMarkup(createElement(NotificationsView, { info: INFO, onSet: () => {}, ...over }));

/** The view's buttons by their words, each with its click. */
function buttons(node: ReactNode): { words: string; click: () => void }[] {
  if (!node || typeof node !== "object") return [];
  if (Array.isArray(node)) return node.flatMap(buttons);
  const el = node as ReactElement<{ children?: ReactNode; onClick?: () => void }>;
  if (typeof el.type === "function") return buttons((el.type as (p: unknown) => ReactNode)(el.props));
  const own = el.type === "button" ? [{ words: String(el.props.children), click: () => el.props.onClick?.() }] : [];
  return [...own, ...buttons(el.props.children)];
}

describe("Notifications (0052)", () => {
  it("is a Settings group of its own, before About, with its section in the manual", () => {
    expect(SETTINGS_GROUPS.map((g) => g.id)).toEqual(["appearance", "display", "projects", "agents", "notifications", "about"]);
    expect(SETTINGS_GROUPS.find((g) => g.id === "notifications")).toMatchObject({ label: "Notifications", topic: "notifications" });
    expect(HELP_TOPICS.notifications).toMatchObject({ file: "04-in-the-app.md", slug: "notifications" });
  });

  it("shows On and Off with the one in force lit, on unless turned off", () => {
    const chips = (html: string) => /<div[^>]*aria-label="Mac notifications"[^>]*>([\s\S]*?)<\/div>/.exec(html)?.[1] ?? "";
    expect(chips(view())).toMatch(/aria-pressed="true" class="ui-pill ui-pill--on">On</);
    expect(chips(view())).toMatch(/aria-pressed="false" class="ui-pill">Off</);
    const off = view({ info: { ...INFO, settings: { ...INFO.settings, notify: false } } });
    expect(chips(off)).toMatch(/aria-pressed="true" class="ui-pill ui-pill--on">Off</);
    // A core from before notifications sends none: on, its default.
    const older = view({ info: { ...INFO, settings: { leaseMinutes: 720, liveMinutes: 5, claudeAccounts: ["mg", "hg"] } } });
    expect(chips(older)).toMatch(/aria-pressed="true" class="ui-pill ui-pill--on">On</);
  });

  it("turns them off and on through the core", () => {
    const onSet = vi.fn();
    const b = buttons(NotificationsView({ info: INFO, onSet }));
    b.find((x) => x.words === "Off")!.click();
    b.find((x) => x.words === "On")!.click();
    expect(onSet.mock.calls).toEqual([
      ["notify", false],
      ["notify", true],
    ]);
  });

  it("says what notifies, when it doesn't, who asks for leave, and that nothing leaves the Mac", () => {
    const t = text(view());
    for (const what of ["Your check", "a question escalated to you", "blocked on you or by a permission", "waits for your input", "your own", "lessons to approve"]) {
      expect(t).toContain(what);
    }
    expect(t).toContain("Several at once come as one");
    expect(t).toContain("never twice for the same thing");
    expect(t).toContain("not for what was already waiting when Muster started");
    expect(t).toContain("While Muster's window is in front, what it shows (its project, or All) counts as seen");
    expect(t).toContain("news from another project waits until you switch away");
    expect(t).toContain("macOS asks whether Muster may notify the first time it has something to tell you");
    expect(t).toContain("System Settings › Notifications");
    expect(t).toContain("Nothing leaves this Mac");
    expect(t).toContain("Both are saved on this Mac, in ~/Library/Application Support/ai.sweetlemon.muster/settings.json");
  });

  it("shows the menu bar icon's On and Off (0085), on unless turned off, apart from notifications", () => {
    const chips = (html: string) => /<div[^>]*aria-label="Menu bar icon"[^>]*>([\s\S]*?)<\/div>/.exec(html)?.[1] ?? "";
    expect(view()).toContain("<h2>Menu bar icon</h2>");
    expect(chips(view())).toMatch(/aria-pressed="true" class="ui-pill ui-pill--on">On</);
    const off = view({ info: { ...INFO, settings: { ...INFO.settings, menuBar: false } } });
    expect(chips(off)).toMatch(/aria-pressed="true" class="ui-pill ui-pill--on">Off</);
    // Notifications off leaves the icon on, and the other way round.
    const notifyOff = view({ info: { ...INFO, settings: { ...INFO.settings, notify: false } } });
    expect(chips(notifyOff)).toMatch(/aria-pressed="true" class="ui-pill ui-pill--on">On</);
    // A core from before the icon sends no menuBar: on, its default.
    const older = view({ info: { ...INFO, settings: { leaseMinutes: 720, liveMinutes: 5, notify: true, claudeAccounts: ["mg", "hg"] } } });
    expect(chips(older)).toMatch(/aria-pressed="true" class="ui-pill ui-pill--on">On</);
  });

  it("turns the menu bar icon off and on through the core, by its own name", () => {
    const onSet = vi.fn();
    const b = buttons(NotificationsView({ info: INFO, onSet })).filter((x) => x.words === "Off");
    expect(b).toHaveLength(2);
    b[1].click();
    expect(onSet.mock.calls).toEqual([["menu-bar", false]]);
  });

  it("says what the menu bar icon shows, what clicking does, and that it names a count, never a task", () => {
    const t = text(view());
    expect(t).toContain("plain while nothing waits for you and with its antenna tips filled in when something does");
    expect(t).toContain("the same count as the Dock badge, from the same reads");
    expect(t).toContain("say the count in words, never a task's text");
    expect(t).toContain("Click it to bring Muster's window forward");
    expect(t).toContain("Right-click or Control-click it for Show Muster, the count and Quit Muster");
  });

  it("shows why a change was refused", () => {
    expect(view({ error: "disk full" })).toContain('<p class="err" role="alert">disk full</p>');
    expect(view()).not.toContain('class="err"');
  });
});
