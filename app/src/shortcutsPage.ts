// The manual's Shortcuts page (docs/manual/06-shortcuts.md), made from the shortcut table
// (shortcuts.ts) and the /mu routes, in the language shown (help.json, mu-148). Apart from the
// table, so the window's first chunk carries only what the key handlers need.
import { dirOf, getLocale, tn, type TKey } from "./i18n";
import { anchorFor } from "./anchors";
import { type Area, AREAS, SHORTCUTS } from "./shortcuts";

/** The /mu routes (commands/mu.md, task 0003), in its order: `/muster` takes the same. `id` names
 *  the route's words in help.json (`slash.what.<id>`). */
export const SLASH: { id: SlashId; route: string; who: "owner" | "agent" | "system" }[] = [
  { id: "status", route: "/mu", who: "owner" },
  { id: "show", route: "/mu <id>", who: "owner" },
  { id: "waiting", route: "/mu waiting", who: "owner" },
  { id: "add", route: "/mu add <text>", who: "owner" },
  { id: "answer", route: "/mu answer <id> Q<n> <text>", who: "owner" },
  { id: "capture", route: "/mu <anything else>", who: "owner" },
  { id: "ask", route: "/mu ask <id> <question>", who: "agent" },
  { id: "claim", route: "/mu claim <id>", who: "agent" },
  { id: "next", route: "/mu next", who: "agent" },
  { id: "heartbeat", route: "/mu heartbeat <id>", who: "agent" },
  { id: "note", route: "/mu note <id> <text>", who: "agent" },
  { id: "pass", route: "/mu pass <id> …", who: "agent" }, // i18n-ignore: a command
  { id: "return", route: "/mu return <id> <why>", who: "agent" },
  { id: "block", route: "/mu block <id> <on what>", who: "agent" },
  { id: "unblock", route: "/mu unblock <id>", who: "agent" },
  { id: "list", route: "/mu list [--stage <s>]", who: "system" },
  { id: "check", route: "/mu check", who: "system" },
  { id: "help", route: "/mu help [<command>]", who: "system" },
];
type SlashId = "status" | "show" | "waiting" | "add" | "answer" | "capture" | "ask" | "claim" | "next" | "heartbeat" | "note" | "pass" | "return" | "block" | "unblock" | "list" | "check" | "help";

/** What a route does, in the language shown. */
export const slashWhat = (id: SlashId) => tn("help", `slash.what.${id}`);

/** The shortcuts whose keys column has words in it ("Esc, twice"): help.json writes it. */
const WORDED = new Set(["discard-text", "tag-add", "grid-ends", "grid-enter"]);

/** Each section's anchor: its English heading's slug, in every language, so a link from an English
 *  page (07-for-agents.md's #slash-commands-in-claude-code) and a remembered section still land. */
export const SECTION_IDS: Record<Area | "slash", string> = {
  anywhere: "anywhere",
  search: "search-and-views",
  writing: "writing-and-sending",
  pane: "the-task-pane",
  board: "the-board",
  projects: "projects",
  slash: "slash-commands-in-claude-code",
};

/** A table cell's text: `<` and `>` stay text on GitHub, and a `|` can't end the cell. */
const cell = (s: string) => s.replace(/[<>|]/g, (c) => `\\${c}`);

/** Left to right in a right-to-left page (an LRI…PDI isolate): keys keep their order, «⌘N» never
 *  reads «N⌘» (docs/i18n/glossary-fa.md). */
const ltr = (s: string) => (dirOf(getLocale()) === "rtl" ? `⁦${s}⁩` : s);

/** "a, b and c" through help.json's joins (Intl.ListFormat would add English's serial comma, which
 *  the page has never had). */
function list(xs: string[]): string {
  if (xs.length < 2) return xs.join("");
  const head = xs.slice(0, -1).reduce((a, b) => tn("help", "page.listMore", { a, b }));
  return tn("help", "page.listLast", { a: head, b: xs[xs.length - 1] });
}

/** The manual's Shortcuts page (docs/manual/06-shortcuts.md), made from the tables above. */
export function shortcutsPage(): string {
  const t = (key: TKey<"help">, params?: Record<string, string>) => tn("help", key, params);
  // A chord that already shows ⌃ (⌃⌘S, mu-167) says Ctrl itself: it isn't one of the "⌘ only" ones.
  const cmdOnly = SHORTCUTS.filter((s) => s.mod === "cmd" && !s.keys.includes("⌃")).map((s) => ltr(s.keys));
  const ctrl = cmdOnly.length ? t(cmdOnly.length === 1 ? "page.ctrlExceptOne" : "page.ctrlExceptMany", { list: list(cmdOnly) }) : t("page.ctrl");
  const option = SHORTCUTS.some((s) => s.mod === "alt") ? t("page.option") : "";
  const out = [`# ${tn("help", "pages.06-shortcuts")}`, "", t("page.intro", { ctrl, option }), ""];
  const head = `| ${t("page.keys")} | ${t("page.what")} | ${t("page.where")} |`;
  for (const area of AREAS) {
    out.push(`## ${anchorFor(t(`areas.${area}`), SECTION_IDS[area])}`, "", head, "| --- | --- | --- |");
    for (const s of SHORTCUTS.filter((x) => x.area === area)) {
      // help.json isolates a worded cell's keys itself ("⁦Esc⁩، دو بار"); a second isolate around
      // the whole cell would read its Persian words left to right.
      const keys = WORDED.has(s.id) ? t(`shortcuts.${s.id}.keys` as TKey<"help">) : ltr(s.keys);
      out.push(`| ${keys} | ${s.what} | ${s.where} |`);
    }
    out.push("");
  }
  out.push(`## ${anchorFor(t("page.slashTitle"), SECTION_IDS.slash)}`, "", t("page.slashIntro"), "", t("page.slashRouter"), "", `| ${t("page.route")} | ${t("page.for")} | ${t("page.what")} |`, "| --- | --- | --- |");
  for (const r of SLASH) out.push(`| \`${r.route}\` | ${t(`slash.who.${r.who}`)} | ${cell(slashWhat(r.id))} |`);
  out.push("");
  return out.join("\n");
}
