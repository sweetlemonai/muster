// Context help: where each view's and pane's "?" opens the manual. Kept apart from the manual
// itself, so the window's first chunk doesn't carry its pages. A test checks every section exists.

export type HelpTopic = "board" | "table" | "timeline" | "graph" | "helper" | "projects" | "task-pane" | "note-pane" | "new-task" | "appearance" | "display" | "agents" | "about";

/** A place in the manual: a page's file and a heading's slug ("" for the page's top). */
export type HelpAt = { file: string; slug: string };

export const HELP_TOPICS: Record<HelpTopic, HelpAt & { label: string }> = {
  board: { file: "04-in-the-app.md", slug: "the-board", label: "the board" },
  table: { file: "04-in-the-app.md", slug: "the-table", label: "the table" },
  timeline: { file: "04-in-the-app.md", slug: "the-timeline", label: "the timeline" },
  graph: { file: "04-in-the-app.md", slug: "the-graph", label: "the graph" },
  helper: { file: "04-in-the-app.md", slug: "the-helper", label: "the Helper" },
  projects: { file: "04-in-the-app.md", slug: "projects", label: "Projects" },
  "task-pane": { file: "04-in-the-app.md", slug: "the-task-pane", label: "the task pane" },
  "note-pane": { file: "04-in-the-app.md", slug: "a-backlog-note", label: "a backlog note" },
  "new-task": { file: "04-in-the-app.md", slug: "new-task", label: "New task" },
  appearance: { file: "04-in-the-app.md", slug: "appearance", label: "Appearance" },
  display: { file: "04-in-the-app.md", slug: "display", label: "Display" },
  agents: { file: "04-in-the-app.md", slug: "agents", label: "Agents" },
  about: { file: "04-in-the-app.md", slug: "about", label: "About" },
};

/** Where the Help tab is open; `n` goes up on every ask, so asking for the same place again
 *  still scrolls there. */
export type ManualAt = HelpAt & { n: number };
