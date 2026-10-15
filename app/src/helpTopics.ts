// Context help: where each view's and pane's "?" opens the manual. Kept apart from the manual
// itself, so the window's first chunk doesn't carry its pages. A test checks every section exists.
import { tl } from "./i18n";

export type HelpTopic = "board" | "table" | "timeline" | "graph" | "helper" | "projects" | "task-pane" | "note-pane" | "new-task" | "appearance" | "display" | "agents" | "notifications" | "about";

/** A place in the manual: a page's file and a heading's slug ("" for the page's top). */
export type HelpAt = { file: string; slug: string };

/** A section of the app's page, with what its "?" calls it (common.json, read in the language
 *  shown each time it's asked, mu-146). */
const inApp = (topic: HelpTopic, slug: string): HelpAt & { readonly label: string } => ({
  file: "04-in-the-app.md",
  slug,
  get label() {
    return tl(`help.topic.${topic}`);
  },
});

export const HELP_TOPICS: Record<HelpTopic, HelpAt & { readonly label: string }> = {
  board: inApp("board", "the-board"),
  table: inApp("table", "the-table"),
  timeline: inApp("timeline", "the-timeline"),
  graph: inApp("graph", "the-graph"),
  helper: inApp("helper", "the-helper"),
  projects: inApp("projects", "projects"),
  "task-pane": inApp("task-pane", "the-task-pane"),
  "note-pane": inApp("note-pane", "a-backlog-note"),
  "new-task": inApp("new-task", "new-task"),
  appearance: inApp("appearance", "appearance"),
  display: inApp("display", "display"),
  agents: inApp("agents", "agents"),
  notifications: inApp("notifications", "notifications"),
  about: inApp("about", "about"),
};

/** Where the Help tab is open; `n` goes up on every ask, so asking for the same place again
 *  still scrolls there. */
export type ManualAt = HelpAt & { n: number };
