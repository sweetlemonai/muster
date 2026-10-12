import type { Activity, Task } from "./types";

export const COLUMNS = [
  { status: "inbox", label: "Inbox" },
  { status: "todo", label: "To do" },
  { status: "doing", label: "Doing" },
  { status: "blocked", label: "Blocked" },
  { status: "done", label: "Done" },
] as const;

const PRIORITY: Record<string, number> = { high: 0, P0: 0, P1: 0, normal: 1, P2: 1, low: 2, P3: 2 };

/** Tasks per column. Waiting-for-owner first, then priority, then id. Unknown statuses go to Inbox. */
export function columns(tasks: Task[]): Record<string, Task[]> {
  const out: Record<string, Task[]> = Object.fromEntries(COLUMNS.map((c) => [c.status, [] as Task[]]));
  for (const t of tasks) (out[t.status] ?? out.inbox).push(t);
  for (const [status, list] of Object.entries(out)) {
    list.sort((a, b) =>
      status === "done"
        ? b.modified - a.modified
        : Number(b.waitingForOwner) - Number(a.waitingForOwner) ||
          (PRIORITY[a.priority] ?? 1) - (PRIORITY[b.priority] ?? 1) ||
          a.queue.localeCompare(b.queue) ||
          a.id.localeCompare(b.id),
    );
  }
  return out;
}

/** "now", "5 min", "3 h", "2 d". */
export function ago(now: number, at: number): string {
  const s = Math.max(0, now - at);
  if (s < 60) return "now";
  if (s < 3600) return `${Math.floor(s / 60)} min`;
  if (s < 86400) return `${Math.floor(s / 3600)} h`;
  return `${Math.floor(s / 86400)} d`;
}

/** Local time as the file contract writes it: YYYY-MM-DD HH:MM. */
export function stamp(d: Date): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

/** Who holds a claim, without the session id: "drover/main (session 3fba2222)" → "drover/main". */
export function claimant(claimedBy: string): string {
  // "drover/main (session 3fba2222)", "worktree-i18n (.claude/worktrees/i18n), session 41f7fe18" and
  // "capture (drover/main, session 349bc9c3)" all come out as the name before the first ( or ,.
  return claimedBy.split(/\s*[(,]/)[0].trim() || claimedBy.trim();
}

/** Recent changes: commits, plus tasks whose files changed, newest first. */
export function feed(activity: Activity[], tasks: Task[], limit = 60): Activity[] {
  const touched: Activity[] = tasks.map((t) => ({
    queue: t.queue,
    at: t.modified,
    what: `${t.id} ${t.title} · ${t.status}${t.logTail.length ? ` · ${t.logTail[t.logTail.length - 1]}` : ""}`,
    source: "file",
  }));
  return [...activity, ...touched].sort((a, b) => b.at - a.at).slice(0, limit);
}

export const key = (t: Pick<Task, "queue" | "id">) => `${t.queue}/${t.id}`;

/** Files from a paste or drop, as [name, data URL]. Pasted images without a name get one. */
export async function readFiles(list: FileList | File[], stampText: string): Promise<[string, string][]> {
  const files = Array.from(list);
  return Promise.all(
    files.map(
      (f, i) =>
        new Promise<[string, string]>((resolve, reject) => {
          const r = new FileReader();
          r.onload = () => resolve([fileName(f, i, stampText), String(r.result)]);
          r.onerror = () => reject(r.error);
          r.readAsDataURL(f);
        }),
    ),
  );
}

export function fileName(f: { name: string; type: string }, i: number, stampText: string): string {
  if (f.name && f.name !== "image.png") return f.name;
  const ext = f.type.split("/")[1]?.replace("jpeg", "jpg") || "bin";
  return `pasted-${stampText.replace(/[ :]/g, "-")}${i ? `-${i + 1}` : ""}.${ext}`;
}

/** "14 h", "2 d 3 h" (8-hour agent days, as the core counts them), "45 min". */
export function fmtMinutes(m: number): string {
  if (m <= 0) return "";
  if (m < 60) return `${m} min`;
  const h = Math.round(m / 6) / 10;
  if (h < 8) return `${h} h`;
  const d = Math.floor(h / 8), rest = Math.round((h - d * 8) * 10) / 10;
  return rest ? `${d} d ${rest} h` : `${d} d`;
}
/** "3.2M tokens", "450k tokens". */
export function fmtTokens(n: number): string {
  if (n <= 0) return "";
  if (n >= 1e6) return `${(Math.round(n / 1e5) / 10).toString()}M tokens`;
  if (n >= 1e3) return `${Math.round(n / 1e3)}k tokens`;
  return `${n} tokens`;
}
/** Totals of what's shown: "≈ 14 h · 3.2M tokens (9 of 24 estimated)". */
export function estimateTotal(tasks: { estMinutes: number; estTokenCount: number; status: string }[]): string {
  const open = tasks.filter((t) => t.status !== "done");
  const est = open.filter((t) => t.estMinutes || t.estTokenCount);
  if (!est.length) return "";
  const m = est.reduce((a, t) => a + t.estMinutes, 0), k = est.reduce((a, t) => a + t.estTokenCount, 0);
  const parts = [fmtMinutes(m), fmtTokens(k)].filter(Boolean).join(" · ");
  return `≈ ${parts} left${est.length < open.length ? ` (${est.length} of ${open.length} open tasks estimated)` : ""}`;
}
