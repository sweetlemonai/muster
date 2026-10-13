// The manual's Shortcuts page (docs/manual/06-shortcuts.md), made from the shortcut table
// (shortcuts.ts) and the /mu routes. Apart from the table, so the window's first chunk carries only
// what the key handlers need.
import { AREAS, SHORTCUTS } from "./shortcuts";

/** The /mu routes (commands/mu.md, task 0003), in its order: `/muster` takes the same. */
export const SLASH: { route: string; who: "owner" | "agent" | "system"; what: string }[] = [
  { route: "/mu", who: "owner", what: "What's waiting for you, then the queue in one line" },
  { route: "/mu <id>", who: "owner", what: "One task in plain words" },
  { route: "/mu waiting", who: "owner", what: "Only what's waiting for you" },
  { route: "/mu add <text>", who: "owner", what: "Put a one-liner in the backlog" },
  { route: "/mu answer <id> Q<n> <text>", who: "owner", what: "Answer a question; it reaches the agent that asked" },
  { route: "/mu <anything else>", who: "owner", what: "Captured as a backlog one-liner, never run as a command" },
  { route: "/mu ask <id> <question>", who: "agent", what: "Ask without stopping; the answer is pushed back" },
  { route: "/mu claim <id>", who: "agent", what: "Claim a Ready task" },
  { route: "/mu next", who: "agent", what: "Claim the top Ready task you may take" },
  { route: "/mu heartbeat <id>", who: "agent", what: "Keep a claim alive" },
  { route: "/mu note <id> <text>", who: "agent", what: "A line in the task's log" },
  { route: "/mu pass <id> …", who: "agent", what: "Hand the task on to the next stage" },
  { route: "/mu return <id> <why>", who: "agent", what: "Send it back a stage, with the reason" },
  { route: "/mu block <id> <on what>", who: "agent", what: "Mark it blocked on a task, the owner or capture" },
  { route: "/mu unblock <id>", who: "agent", what: "Clear the blocked flag" },
  { route: "/mu list [--stage <s>]", who: "system", what: "Tasks by stage" },
  { route: "/mu check", who: "system", what: "Problems in the queue" },
  { route: "/mu help [<command>]", who: "system", what: "The routes, then muster's own help" },
];

/** A table cell's text: `<` and `>` stay text on GitHub, and a `|` can't end the cell. */
const cell = (s: string) => s.replace(/[<>|]/g, (c) => `\\${c}`);

/** The manual's Shortcuts page (docs/manual/06-shortcuts.md), made from the tables above. */
export function shortcutsPage(): string {
  const cmdOnly = SHORTCUTS.filter((s) => s.mod === "cmd").map((s) => s.keys);
  const list = (xs: string[]) => (xs.length < 2 ? xs.join("") : `${xs.slice(0, -1).join(", ")} and ${xs[xs.length - 1]}`);
  const ctrl = cmdOnly.length
    ? `Ctrl works too wherever ⌘ is shown, except ${list(cmdOnly)}, which ${cmdOnly.length === 1 ? "takes" : "take"} ⌘ only.`
    : "Ctrl works too wherever ⌘ is shown.";
  const out = [
    "# Shortcuts",
    "",
    `Every keyboard shortcut in the app, and the slash commands for Claude Code. This page is made from the app's own table of shortcuts, and a test checks every key the app reacts to against that table, so a shortcut can't exist without being listed here. ⌘ is the Command key. ${ctrl} ↩ is Return.`,
    "",
  ];
  for (const area of AREAS) {
    out.push(`## ${area}`, "", "| Keys | What it does | Where |", "| --- | --- | --- |");
    for (const s of SHORTCUTS.filter((x) => x.area === area)) out.push(`| ${s.keys} | ${s.what} | ${s.where} |`);
    out.push("");
  }
  out.push(
    "## Slash commands in Claude Code",
    "",
    "`/mu` (or the long form, `/muster`) runs one `muster` command from inside a Claude Code session. The first word picks what it does, and anything that isn't a route is captured as a backlog one-liner, never guessed. Routes for the owner answer in plain words; routes for agents and the system print what `muster` printed.",
    "",
    "The router is built (task 0003) but not installed yet: it goes into the personal layer once the owner says yes. Until then, type `! muster …` in a session, as [For agents](07-for-agents.md#in-a-claude-code-session) shows.",
    "",
    "| Route | For | What it does |",
    "| --- | --- | --- |",
  );
  for (const r of SLASH) out.push(`| \`${r.route}\` | ${r.who} | ${cell(r.what)} |`);
  out.push("");
  return out.join("\n");
}
