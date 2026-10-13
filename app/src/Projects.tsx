import { useCallback, useEffect, useState } from "react";
import { api } from "./api";
import { queueTotal } from "./lib";
import { HelpButton } from "./HelpButton";
import type { Project, QueueInfo } from "./types";

export default function Projects({ queues, onChanged }: { queues: QueueInfo[]; onChanged: () => void }) {
  const [list, setList] = useState<Project[] | null>(null);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [name, setName] = useState("");
  // The project whose prefix (0033) is being changed, and what's typed.
  const [prefixOf, setPrefixOf] = useState<string | null>(null);
  const [prefix, setPrefix] = useState("");

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
    <main className="list projects">
      <div className="projects-bar">
        <button onClick={() => void act(api.createProject, "Queue created and added.")}>New queue…</button>
        <button onClick={() => void act(api.addProject, "Queue added.")}>Add existing queue…</button>
        {msg && <span className={msg.ok ? "ok" : "err"}>{msg.text}</span>}
        <HelpButton topic="projects" />
      </div>
      <p className="meta">
        Muster finds every <code>~/playground/*/tasks</code> queue on its own. Add queues kept anywhere else, or start one for a
        project: pick the project's folder (the one that holds its repo, not the repo itself) and Muster creates its{" "}
        <code>tasks/</code> folder with the rules, the tools and its own git history. Removing or hiding a project never deletes files.
      </p>
      {list === null && <p className="none">Loading…</p>}
      <table className="projects-table">
        <thead>
          <tr><th>Project</th><th title="What its tasks' keys start with: mu-12">Prefix</th><th>Folder</th><th>Tasks</th><th></th></tr>
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
              <td className="prefix">
                {prefixOf === p.key ? (
                  <form
                    onSubmit={(e) => {
                      e.preventDefault();
                      setPrefixOf(null);
                      const next = prefix.trim();
                      void act(() => api.setProjectPrefix(p.key, next), `Prefix set: its tasks are ${next}-1, ${next}-2…`);
                    }}
                  >
                    <input autoFocus value={prefix} onChange={(e) => setPrefix(e.target.value)} maxLength={4} size={5} aria-label={`${p.name}'s prefix`} placeholder="2–4 letters" />
                  </form>
                ) : (
                  <code>{p.prefix}</code>
                )}
              </td>
              <td className="path" title={p.path}>{p.path}</td>
              <td>{p.hidden || !p.ok ? "—" : total(p) ?? "…"}</td>
              <td className="actions">
                <button onClick={() => { setEditing(p.key); setName(p.name); }}>Rename</button>
                {p.ok && <button onClick={() => { setPrefixOf(p.key); setPrefix(p.prefix); }} title="What its tasks' keys start with (mu-12): 2 to 4 lowercase letters, no other project's">Prefix</button>}
                <button onClick={() => void act(() => api.hideProject(p.key, !p.hidden))}>{p.hidden ? "Show" : "Hide"}</button>
                {p.source === "added" && <button onClick={() => void act(() => api.removeProject(p.key))}>Remove</button>}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </main>
  );
}
