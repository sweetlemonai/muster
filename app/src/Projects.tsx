import { useCallback, useEffect, useState } from "react";
import { api } from "./api";
import { queueTotal } from "./lib";
import type { Project, QueueInfo } from "./types";

/** Settings' Projects group (0036): every project with its folder and task count, New queue and Add existing. */
export default function Projects({ queues, onChanged }: { queues: QueueInfo[]; onChanged: () => void }) {
  const [list, setList] = useState<Project[] | null>(null);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [name, setName] = useState("");

  const load = useCallback(async () => {
    try {
      setList(await api.projects());
    } catch (e) {
      setMsg({ ok: false, text: String(e) });
    }
  }, []);
  useEffect(() => {
    void load();
  }, [load]);

  const act = async (what: () => Promise<unknown>, done?: string) => {
    try {
      const r = await what();
      if (r !== null && done) setMsg({ ok: true, text: done });
      await load();
      onChanged();
    } catch (e) {
      setMsg({ ok: false, text: String(e) });
    }
  };
  const total = (p: Project) => {
    const q = queues.find((x) => x.path === p.path);
    return q ? queueTotal(q.counts) : null;
  };

  return (
    <div className="projects">
      <div className="projects-bar">
        <button onClick={() => void act(api.createProject, "Queue created and added.")}>New queue…</button>
        <button onClick={() => void act(api.addProject, "Queue added.")}>Add existing queue…</button>
        {msg && <span className={msg.ok ? "ok" : "err"}>{msg.text}</span>}
      </div>
      <p className="meta">
        Muster finds every <code>~/playground/*/tasks</code> queue on its own. Add queues kept anywhere else, or start one for a
        project: pick the project's folder (the one that holds its repo, not the repo itself) and Muster creates its{" "}
        <code>tasks/</code> folder with the rules, the tools and its own git history. Removing or hiding a project never deletes files.
      </p>
      {list === null && <p className="none">Loading…</p>}
      <table className="projects-table">
        <thead>
          <tr><th>Project</th><th>Folder</th><th>Tasks</th><th>Gate</th><th></th></tr>
        </thead>
        <tbody>
          {list?.map((p) => (
            <tr key={p.key} className={p.hidden ? "dim" : ""}>
              <td>
                {editing === p.key ? (
                  <form
                    onSubmit={(e) => {
                      e.preventDefault();
                      setEditing(null);
                      void act(() => api.renameProject(p.key, name));
                    }}
                  >
                    <input autoFocus value={name} onChange={(e) => setName(e.target.value)} maxLength={60} placeholder="Name (empty = folder's name)" />
                  </form>
                ) : (
                  <b>{p.name}</b>
                )}
                <div className="meta">
                  {p.source === "found" ? "found in ~/playground" : "added"}
                  {p.hidden && " · hidden"}
                  {!p.ok && <span className="err"> · not a queue any more</span>}
                </div>
              </td>
              <td className="path" title={p.path}>{p.path}</td>
              <td>{p.hidden || !p.ok ? "—" : total(p) ?? "…"}</td>
              <td><GateCell name={p.name} gate={p.gate ?? ""} /></td>
              <td className="actions">
                <button onClick={() => { setEditing(p.key); setName(p.name); }}>Rename</button>
                <button onClick={() => void act(() => api.hideProject(p.key, !p.hidden))}>{p.hidden ? "Show" : "Hide"}</button>
                {p.source === "added" && <button onClick={() => void act(() => api.removeProject(p.key))}>Remove</button>}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/** A project's gate command (0023), shown as it is: a merge runs it in the task's worktree, and
 *  never runs without one. Set with `muster projects set`, the one place that checks it. */
export function GateCell({ name, gate }: { name: string; gate: string }) {
  if (gate) return <code className="path">{gate}</code>;
  return (
    <span className="meta">
      none: merges wait for one. <code>muster projects set {name} gate "&lt;command&gt;"</code>
    </span>
  );
}
