import type { Task } from "./types";

/** A task's type, the core's list (ops::KINDS, task 0017) in its order. Old files may still say
 *  idea (the core reads task as code); those show as they are. */
export const KINDS = ["code", "bug", "chore", "tech-debt", "spike", "design", "docs", "research"] as const;

/** The estimates in one line, as the card shows them: "agent 2h · test 30m · 400k", only the parts
 *  set (the same as the core's ops::estimate_text). */
export function estimateText(t: Pick<Task, "estTime" | "estTestTime" | "estTokens">): string {
  return [t.estTime && `agent ${t.estTime}`, t.estTestTime && `test ${t.estTestTime}`, t.estTokens].filter(Boolean).join(" · ");
}

const heading = (line: string) => line.replace(/\r?\n$/, "").trimEnd();
const linesOf = (text: string) => text.match(/[^\n]*\n|[^\n]+$/g) ?? [];

/** Index of the last `## Log` heading line (the core's queue::log_heading), or the line count. */
function lastLog(lines: string[]): number {
  let log = lines.length;
  lines.forEach((l, i) => {
    if (heading(l) === "## Log") log = i;
  });
  return log;
}

/** The text split at the Log (the last `## Log` heading): [what's above it, the Log and after]. */
export function splitLog(text: string): [string, string] {
  const lines = linesOf(text);
  const log = lastLog(lines);
  return [lines.slice(0, log).join(""), lines.slice(log).join("")];
}

/** The task's text without the tool's Comments section, which the pane shows as a thread instead.
 *  The same section as the core's queue::comments_span: the last `## Comments` heading above the
 *  last `## Log` (anywhere, without a Log), up to the next `## ` line. */
export function withoutComments(text: string): string {
  const lines = linesOf(text);
  const log = lastLog(lines);
  let head = -1;
  for (let i = 0; i < log; i++) if (heading(lines[i]) === "## Comments") head = i;
  if (head < 0) return text;
  let end = head + 1;
  while (end < lines.length && !lines[end].startsWith("## ")) end++;
  return [...lines.slice(0, head), ...lines.slice(end)].join("");
}
