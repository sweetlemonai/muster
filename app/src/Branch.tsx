import { tn, useT } from "./i18n";
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
    <span className="card-branch ltr" title={t.worktree ? tn("board", "branch.worktree", { path: t.worktree }) : undefined}>
      {line}
    </span>
  );
}

/** The pane's part for a started task: its branch and worktree path; after the merge, the merge
 *  commit and the report. Nothing for a task that was never started in a worktree. */
export function BranchPane({ task }: { task: Task }) {
  const tr = useT("pane");
  if (task.branch) {
    return (
      <div className="branch-pane" data-part="worktree">
        <div className="kv">
          <span className="lbl">{tr("side.branch.branch")}</span>
          <code className="ltr">{task.branch}</code>
        </div>
        <div className="kv stack">
          <span className="lbl">{tr("side.branch.worktree")}</span>
          {task.worktree ? <code className="path ltr">{task.worktree}</code> : <span className="meta">{tr("side.branch.notRecorded")}</span>}
        </div>
      </div>
    );
  }
  if (task.mergeReport) {
    return (
      <div className="branch-pane" data-part="merged">
        <div className="kv">
          <span className="lbl">{tr("side.branch.merged")}</span>
          {task.commit ? <code className="ltr">{task.commit.slice(0, 7)}</code> : <span className="meta">{tr("side.branch.noCommit")}</span>}
        </div>
        <p className="meta small merge-report">{task.mergeReport}</p>
      </div>
    );
  }
  return null;
}
