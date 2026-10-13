import { useCallback, useEffect, useState } from "react";
import { api } from "./api";
import { LimitCell } from "./CheckLimit";
import { HomePath } from "./HomePath";
import { queueTotal } from "./lib";
import type { Project, QueueInfo } from "./types";

/** Settings' Projects group (0036): every project with its prefix (0033), folder and task count,
 *  New queue and Add existing. A folder under `home` shows as `~/…`. */
export default function Projects({ queues, home, onChanged }: { queues: QueueInfo[]; home: string; onChanged: () => void }) {
  const [list, setList] = useState<Project[] | null>(null);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [name, setName] = useState("");
  // The project whose prefix is being set, what's typed, and why the core refused it.
  const [prefixOf, setPrefixOf] = useState<string | null>(null);
  const [prefix, setPrefix] = useState("");
  const [prefixError, setPrefixError] = useState("");

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
  const savePrefix = async (p: Project) => {
    const next = prefix.trim();
    try {
      await api.setProjectPrefix(p.key, next);
      setPrefixOf(null);
      setPrefixError("");
      setMsg({ ok: true, text: `${p.name}'s tasks are ${next}-1, ${next}-2… now.` });
      await load();
      onChanged();
    } catch (e) {
      setPrefixError(String(e));
    }
  };
  const total = (p: Project) => {
    const q = queues.find((x) => x.path === p.path);
    return q ? queueTotal(q.counts) : null;
  };
  // The Your check limit (0050) of a project the board reads; a hidden one has none in use.
  const limit = (p: Project) => {
    const q = !p.hidden && p.ok ? queues.find((x) => x.path === p.path) : undefined;
    if (!q) return "—";
    return <LimitCell key={`${p.key}-${q.yourCheckLimit}`} q={q} onSet={(v) => void act(() => api.setYourCheckLimit(p.key, v))} />;
  };

  return (
    <div className="projects">
      <div className="projects-bar">
        <button onClick={() => void act(api.createProject, "Queue created and added.")}>New queue…</button>
        <button onClick={() => void act(api.addProject, "Queue added.")}>Add existing queue…</button>
        {msg && <span className={msg.ok ? "ok" : "err"}>{msg.text}</span>}
      </div>
      <p className="meta">
        Muster finds the queues at <code>~/playground/&lt;project&gt;/tasks</code> on its own, and one level deeper
        (<code>~/playground/&lt;group&gt;/&lt;project&gt;/tasks</code>). Add queues kept anywhere else, or start one for a
        project: pick the project's folder (the one that holds its repo, not the repo itself) and Muster creates its{" "}
        <code>tasks/</code> folder with the rules, the tools and its own git history. Removing or hiding a project never deletes files.
      </p>
      {list === null && <p className="none">Loading…</p>}
      <table className="projects-table">
        <thead>
          <tr><th>Project</th><th title="What its tasks' keys start with: mu in mu-12">Prefix</th><th>Folder</th><th>Tasks</th><th>Your check</th><th>Gate</th><th></th></tr>
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
              <td>
                <PrefixCell
                  name={p.name}
                  prefix={p.prefix ?? ""}
                  editing={prefixOf === p.key}
                  value={prefix}
                  error={prefixOf === p.key ? prefixError : ""}
                  canSet={p.ok}
                  onEdit={() => { setPrefixOf(p.key); setPrefix(p.prefix ?? ""); setPrefixError(""); }}
                  onChange={setPrefix}
                  onSave={() => void savePrefix(p)}
                  onCancel={() => { setPrefixOf(null); setPrefixError(""); }}
                />
              </td>
              <td className="path"><HomePath path={p.path} home={home} /></td>
              <td>{p.hidden || !p.ok ? "—" : total(p) ?? "…"}</td>
              <td>{limit(p)}</td>
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

/** A project's prefix (0033), what its tasks' keys start with, set in place: Set opens a field
 *  for 2 to 4 lowercase letters, Enter or Save saves, Cancel leaves it. The core refuses a bad one
 *  or another project's, and why shows under the field, which stays open. */
export function PrefixCell({ name, prefix, editing, value, error, canSet, onEdit, onChange, onSave, onCancel }: {
  name: string;
  prefix: string;
  editing: boolean;
  value: string;
  error: string;
  /** The folder is still a queue, so there's a README to store it in. */
  canSet: boolean;
  onEdit: () => void;
  onChange: (v: string) => void;
  onSave: () => void;
  onCancel: () => void;
}) {
  if (!editing) {
    return (
      <span className="prefix">
        <code>{prefix}</code> {canSet && <button type="button" className="link" onClick={onEdit} title={`Set ${name}'s prefix`}>Set</button>}
      </span>
    );
  }
  return (
    <form
      className="prefix"
      onSubmit={(e) => {
        e.preventDefault();
        onSave();
      }}
    >
      <input
        autoFocus
        value={value}
        onChange={(e) => onChange(e.target.value)}
        maxLength={4}
        size={5}
        aria-label={`${name}'s prefix`}
        placeholder="2–4 letters"
      />{" "}
      <button type="submit">Save</button> <button type="button" onClick={onCancel}>Cancel</button>
      {error && <p className="err small" role="alert">{error}</p>}
    </form>
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
