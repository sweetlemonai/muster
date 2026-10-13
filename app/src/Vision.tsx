import type { Task } from "./types";

// The vision check (0029): a task checked against the project's one picture. The core writes a
// "## Vision" section above the task's Log and counts its open conflicts (`visionOpen`); these
// read them for the card, the Problems list and the task pane. No Markdown here: the board loads
// this, and the renderer stays in the pane's own chunk (VisionPanel).

const HEAD = "## Vision";
const CONFLICTS = "### Conflicts";

/** The text as lines, each with its offset and its line break. */
function lines(text: string): { at: number; line: string }[] {
  const out: { at: number; line: string }[] = [];
  let at = 0;
  while (at < text.length) {
    const nl = text.indexOf("\n", at);
    const end = nl < 0 ? text.length : nl + 1;
    out.push({ at, line: text.slice(at, end) });
    at = end;
  }
  return out;
}

/** The task's Vision section as [start, end) offsets: from its "## Vision" line to the next
 *  "## " heading. The last one above the last "## Log", as vision.rs section_span reads it. */
export function visionSpan(text: string): [number, number] | null {
  const heads = lines(text).filter((l) => l.line.startsWith("## ")).map((l) => ({ at: l.at, head: l.line.trimEnd() }));
  let upto = heads.length;
  for (let i = heads.length - 1; i >= 0; i--) {
    if (heads[i].head === "## Log") {
      upto = i;
      break;
    }
  }
  for (let i = upto - 1; i >= 0; i--) {
    if (heads[i].head === HEAD) return [heads[i].at, heads[i + 1]?.at ?? text.length];
  }
  return null;
}

/** The section's text without its heading (the pane has its own), or null when there's none. */
export function visionSection(text: string): string | null {
  const span = visionSpan(text);
  if (!span) return null;
  const section = text.slice(span[0], span[1]);
  return section.slice(section.indexOf("\n") + 1 || section.length).replace(/^\n+/, "");
}

/** The text without the tool's Vision section, for the description below the pane's own. A task
 *  never checked has none: a "## Vision" there is the description's. */
export function withoutVision(text: string, checked: boolean): string {
  const span = checked ? visionSpan(text) : null;
  return span ? text.slice(0, span[0]) + text.slice(span[1]) : text;
}

/** The conflicts' titles by id (V1 → "Two records…"), from the section's Conflicts list only:
 *  the item lines the core writes, `- [ ] **V1: <title>**` or `- [x] …`. */
export function conflictTitles(text: string): Record<string, string> {
  const span = visionSpan(text);
  if (!span) return {};
  const out: Record<string, string> = {};
  let inside = false;
  for (const { line } of lines(text.slice(span[0], span[1]))) {
    const l = line.replace(/\r?\n$/, "");
    if (l.startsWith("### ")) {
      inside = l.trimEnd() === CONFLICTS;
      continue;
    }
    const m = inside ? /^- \[[ x]\] \*\*(V\d+): (.*)\*\*\s*$/.exec(l) : null;
    if (m) out[m[1]] = m[2];
  }
  return out;
}

/** The card's badge: "⚠ 2 vision" while conflicts are open; null otherwise (quiet). */
export function visionBadge(t: Pick<Task, "visionOpen">): string | null {
  const n = t.visionOpen ?? 0;
  return n > 0 ? `⚠ ${n} vision` : null;
}

export function VisionBadge({ t }: { t: Pick<Task, "visionOpen"> }) {
  const text = visionBadge(t);
  return text ? <span className="badge vision" title="Open vision conflicts: it can't pass from Shaping to Ready until they're resolved">{text}</span> : null;
}

/** Where the check stands, in a few words. */
export function visionStatus(t: Pick<Task, "visionOpen">): string {
  const n = t.visionOpen;
  if (n === null || n === undefined) return "not checked";
  if (n === 0) return "no open conflicts";
  return `${n} open conflict${n === 1 ? "" : "s"}`;
}

/** Problems' "Vision" lines: Shaping tasks with no check yet (warnings, as `muster check` lists
 *  them), by project and id. */
export function visionWarnings(tasks: Task[]): string[] {
  return tasks
    .filter((t) => t.stage === "shaping" && (t.visionOpen ?? null) === null)
    .sort((a, b) => a.queue.localeCompare(b.queue) || a.id.localeCompare(b.id))
    .map((t) => `${t.queue}: ${t.id} ${t.title} is in Shaping with no vision check yet (muster vision ${t.id})`);
}
