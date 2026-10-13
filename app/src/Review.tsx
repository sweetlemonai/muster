import type { Task } from "./types";

// The review of record (0027): a started task's branch reviewed in Testing by the owner's engine,
// its builder fixing or disputing each finding, up to 3 rounds. The core writes a "## Review"
// section under the write-up (above its Comments and Log) and sums it up in `review:` (the round,
// what's open for the builder, what's answered for the next round); these read them for the card
// and the task pane. No Markdown here: the board loads this, and the renderer stays in the pane's
// own chunk (ReviewPanel).

const HEAD = "## Review";
/** Rounds before a review still open goes to the owner, as review.rs MAX_ROUNDS. */
export const MAX_ROUNDS = 3;

type Reviewed = Pick<Task, "reviewRound" | "reviewOpen" | "reviewAnswered" | "stage" | "branch">;

/** The task's Review section as [start, end) offsets: from its "## Review" line to the next "## "
 *  heading. The last one above the last "## Log", as review.rs section_span reads it. */
export function reviewSpan(text: string): [number, number] | null {
  const heads: { at: number; head: string }[] = [];
  let at = 0;
  while (at < text.length) {
    const nl = text.indexOf("\n", at);
    const end = nl < 0 ? text.length : nl + 1;
    const line = text.slice(at, end);
    if (line.startsWith("## ")) heads.push({ at, head: line.trimEnd() });
    at = end;
  }
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
export function reviewSection(text: string): string | null {
  const span = reviewSpan(text);
  if (!span) return null;
  const section = text.slice(span[0], span[1]);
  return section.slice(section.indexOf("\n") + 1 || section.length).replace(/^\n+/, "");
}

/** The text without the tool's Review section, for the description. A task with no review round
 *  has none: a "## Review" there is the description's. */
export function withoutReview(text: string, reviewed: boolean): string {
  const span = reviewed ? reviewSpan(text) : null;
  return span ? text.slice(0, span[0]) + text.slice(span[1]) : text;
}

const reviewed = (t: Pick<Task, "reviewRound">) => (t.reviewRound ?? null) !== null;

/** The card's line, in Building and Testing: "review 1/3 · 2 to fix", "review 2/3 · 1 to
 *  re-check", "review clean · round 2", or "not reviewed yet" for a started task in Testing.
 *  Null elsewhere (after the merge it's history, in the pane). */
export function reviewBadge(t: Reviewed): string | null {
  if (t.stage !== "building" && t.stage !== "testing") return null;
  if (!reviewed(t)) return t.stage === "testing" && t.branch ? "not reviewed yet" : null;
  const round = `review ${t.reviewRound}/${MAX_ROUNDS}`;
  const open = t.reviewOpen ?? 0;
  const answered = t.reviewAnswered ?? 0;
  if (open > 0) return `${round} · ${open} to fix`;
  if (answered > 0) return `${round} · ${answered} to re-check`;
  return `review clean · round ${t.reviewRound}`;
}

/** The badge as one of the card's status lines (CardStatus): quiet, since it's the agents' work;
 *  a review that goes to the owner shows as blocked on you. */
export function ReviewBadge({ t }: { t: Reviewed }) {
  const text = reviewBadge(t);
  if (!text) return null;
  const open = (t.reviewOpen ?? 0) > 0;
  const title = open
    ? "Findings for the builder to fix or dispute (muster review --fixed / --dispute); the merge waits for a clean review"
    : "The review of record (muster review): the merge waits for it to be clean at the branch's tip";
  return (
    <span className={open ? "card-review open" : "card-review"} title={title}>
      {text}
    </span>
  );
}

/** Where the review stands, in a few words, for the pane's header. */
export function reviewStatus(t: Reviewed): string {
  if (!reviewed(t)) return t.branch ? "not reviewed yet" : "not started on its own branch";
  const open = t.reviewOpen ?? 0;
  const answered = t.reviewAnswered ?? 0;
  if (open === 0 && answered === 0) return `clean after round ${t.reviewRound}`;
  return `round ${t.reviewRound} of ${MAX_ROUNDS}: ${open} open, ${answered} answered`;
}
