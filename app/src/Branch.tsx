import type { Task } from "./types";

/** The last folder of a worktree's path: what the card names it by. */
export function folderName(path: string): string {
  return path.split("/").filter(Boolean).pop() ?? "";
}

/** A started task's card line (0023): its branch and its worktree's folder, or "" for a task
 *  that wasn't started in a worktree. */
export function branchLine(t: Task): string {
  if (!t.branch) return "";
  const folder = folderName(t.worktree ?? "");
  return folder ? `${t.branch} · ${folder}` : t.branch;
}

/** The card's quiet line for a started task; nothing for any other. */
export function BranchCardLine({ t }: { t: Task }) {
  const line = branchLine(t);
  if (!line) return null;
  return (
    <span className="card-branch" title={t.worktree ? `Worktree: ${t.worktree}` : undefined}>
      {line}
    </span>
  );
}

/** The pane's part for a started task: its branch and worktree path; after the merge, the merge
 *  commit and the report. Nothing for a task that was never started in a worktree. */
export function BranchPane({ task }: { task: Task }) {
  if (task.branch) {
    return (
      <div className="branch-pane" data-part="worktree">
        <div className="kv">
          <span className="lbl">Branch</span>
          <code>{task.branch}</code>
        </div>
        <div className="kv stack">
          <span className="lbl">Worktree</span>
          <code className="path">{task.worktree ? task.worktree : "not recorded"}</code>
        </div>
      </div>
    );
  }
  if (task.mergeReport) {
    return (
      <div className="branch-pane" data-part="merged">
        <div className="kv">
          <span className="lbl">Merged</span>
          <code>{task.commit ? task.commit.slice(0, 7) : "no commit recorded"}</code>
        </div>
        <p className="meta small merge-report">{task.mergeReport}</p>
      </div>
    );
  }
  return null;
}
