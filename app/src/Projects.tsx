import { Fragment, useCallback, useEffect, useMemo, useState } from "react";
import { orderProjects, projectRows } from "./activity";
import { api } from "./api";
import { LimitCell } from "./CheckLimit";
import { HomePath } from "./HomePath";
import InlineField from "./InlineField";
import { CATEGORY_MAX, LABEL_MAX, OTHER, otherCategories } from "./labels";
import { queueTotal } from "./lib";
import { type Layout, layoutOf, type Pending, planMove, settle, shownLayout, sidebarGroups, track } from "./projectOrder";
import { Swatches } from "./ProjectSwatches";
import type { Project, QueueInfo } from "./types";
import { Button } from "./ui";

/** Settings' Projects group (0036): every project with its label and category (0044), its colour
 *  and its place in the sidebar's Mine order (0057), prefix (0033), folder and task count, New
 *  queue and Add existing; the shown ones in that order. A folder under `home` shows as `~/…`. */
export default function Projects({ queues, home, onChanged }: { queues: QueueInfo[]; home: string; onChanged: () => void }) {
  const [list, setList] = useState<Project[] | null>(null);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
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
      setMsg({ ok: true, text: `${p.label || p.name}'s tasks are ${next}-1, ${next}-2… now.` });
      await load();
      onChanged();
    } catch (e) {
      setPrefixError(String(e));
    }
  };
  // A label or category saved as it's typed: the window shows it at once, and the save settles
  // once the list has it, so the field shows what it sent until then (InlineField).
  const saved = () => {
    onChanged();
    return load();
  };
  const total = (p: Project) => {
    const q = queues.find((x) => x.path === p.path);
    return q ? queueTotal(q.counts) : null;
  };
  // The sidebar's Mine order (0057), which ↑ ↓ here change as ⌥↑ ⌥↓ do there. A move shows at
  // once and the next one plans from it, until the queues read back take over (projectOrder.settle),
  // so a second click before the refresh never undoes the first (review R3).
  const core = useMemo(() => mineLayout(queues), [queues]);
  const [pending, setPending] = useState<Pending | null>(null);
  useEffect(() => setPending((p) => settle(p, core)), [core]);
  const layout = shownLayout(pending, core);
  const place = (p: Project) => {
    if (!layout.some((g) => g.keys.includes(p.key))) return "—";
    const name = p.label || p.name;
    const named = (k: string) => list?.find((x) => x.key === k)?.label || k;
    const step = (by: -1 | 1) => planMove(layout, { kind: "step-project", key: p.key, by }, named);
    const [up, down] = [step(-1), step(1)];
    const go = (plan: typeof up) => {
      if (!plan) return;
      track(plan, setPending, () => api.arrangeProjects(plan.order, plan.categories, plan.moved)).then(
        () => {
          onChanged();
          return load();
        },
        (e: unknown) => setMsg({ ok: false, text: String(e) }),
      );
    };
    return <PlaceCell name={name} canUp={up !== null} canDown={down !== null} onUp={() => go(up)} onDown={() => go(down)} />;
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
        <Button onClick={() => void act(api.createProject, "Queue created and added.")}>New queue…</Button>
        <Button onClick={() => void act(api.addProject, "Queue added.")}>Add existing queue…</Button>
        {msg && <span className={msg.ok ? "ok" : "err"}>{msg.text}</span>}
      </div>
      <p className="meta">
        Muster finds the queues in <code>~/playground</code> on its own: a repo's <code>&lt;checkout&gt;/.muster</code>{" "}
        (<code>~/playground/&lt;checkout&gt;</code> or <code>~/playground/&lt;project&gt;/&lt;checkout&gt;</code>) and the older{" "}
        <code>&lt;project&gt;/tasks</code>, one level deeper too (<code>~/playground/&lt;group&gt;/&lt;project&gt;/tasks</code>). Add
        queues kept anywhere else, or start one: New queue picks the project's repo (its main checkout or any worktree of
        it) and makes a local <code>.muster/</code> in the main checkout, as <code>muster init</code> does. A folder outside
        any repo still gets the older <code>tasks/</code>. Removing or hiding a project never deletes files.
      </p>
      {list === null && <p className="none">Loading…</p>}
      <table className="projects-table">
        <thead>
          <tr><th title="What the window calls it; the name under it is what commands take">Project</th><th title="The sidebar heading it goes under">Category</th><th title="The dot by its name, in the sidebar and wherever it's named">Colour</th><th title="Its place in the sidebar's Mine order: ↑ ↓ here, or drag it or ⌥↑ ⌥↓ there">Mine order</th><th title="What its tasks' keys start with: mu in mu-12">Prefix</th><th>Folder</th><th>Tasks</th><th>Your check</th><th></th></tr>
        </thead>
        <tbody>
          {list && byPlace(list, layout).map((p) => (
            <Fragment key={p.key}>
              <tr className={p.hidden ? "dim" : ""}>
                <td><NameCell p={p} onSave={(v) => api.setProjectLabel(p.key, v).then(saved)} /></td>
                <td><CategoryCell p={p} categories={otherCategories(queues, p.path)} onSave={(v) => api.setProjectCategory(p.key, v).then(saved)} /></td>
                <td><ColourCell p={p} onSave={(c) => act(() => api.setProjectColour(p.key, c))} /></td>
                <td>{place(p)}</td>
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
                <td className="actions">
                  <Button onClick={() => void act(() => api.hideProject(p.key, !p.hidden))}>{p.hidden ? "Show" : "Hide"}</Button>
                  {p.source === "added" && <Button onClick={() => void act(() => api.removeProject(p.key))}>Remove</Button>}
                </td>
              </tr>
              <GateRow name={p.name} gate={p.gate ?? ""} hidden={p.hidden} />
            </Fragment>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/** A project's label (0044), what the window calls it, set in place and saved as it's typed; empty
 *  shows its name. Under it, small, its real name (what commands take), where it came from, and
 *  whether it's hidden or no longer a queue. */
export function NameCell({ p, onSave }: { p: Project; onSave: (label: string) => Promise<unknown> }) {
  return (
    <>
      <InlineField value={p.label ?? ""} onSave={onSave} max={LABEL_MAX} placeholder={p.name} label={`Label for ${p.name}`} />
      <div className="meta">
        <code>{p.name}</code> · {p.source === "found" ? "found in ~/playground" : "added"}
        {p.hidden && " · hidden"}
        {!p.ok && <span className="err"> · not a queue any more</span>}
      </div>
    </>
  );
}

/** A project's category (0044), the sidebar heading it goes under, set in place and saved as it's
 *  typed: a new one, or one of `categories` (the other shown projects'), laid out as chips under the
 *  field while it has focus. Empty puts it under its group's heading (a queue one level deeper; the
 *  group's display name when it has one, 0070),
 *  else Other, which the field shows greyed. */
export function CategoryCell({ p, categories, onSave }: { p: Project; categories: string[]; onSave: (category: string) => Promise<unknown> }) {
  return <InlineField value={p.category ?? ""} onSave={onSave} max={CATEGORY_MAX} placeholder={p.groupName || p.group || OTHER} label={`Category for ${p.name}`} options={categories} />;
}

/** A project's colour (0057), the dot by its name: the palette laid out, the current one pressed
 *  (Settings is a place for choosing, so it stays laid out). */
export function ColourCell({ p, onSave }: { p: Project; onSave: (colour: string) => Promise<unknown> }) {
  const now = p.colour ?? "";
  return <Swatches name={p.label || p.name} value={now} onPick={(c) => c !== now && void onSave(c)} />;
}

/** A project's place in the sidebar's Mine order (0057): up or down one, as ⌥↑ ⌥↓ move it there
 *  (past its heading's edge it goes under the next heading); off at the top and the bottom. */
export function PlaceCell({ name, canUp, canDown, onUp, onDown }: { name: string; canUp: boolean; canDown: boolean; onUp: () => void; onDown: () => void }) {
  return (
    <span className="place-cell">
      <Button disabled={!canUp} aria-label={`Move ${name} up in the sidebar's Mine order`} title="Up one place in the sidebar's Mine order" onClick={onUp}>↑</Button>
      <Button disabled={!canDown} aria-label={`Move ${name} down in the sidebar's Mine order`} title="Down one place in the sidebar's Mine order" onClick={onDown}>↓</Button>
    </span>
  );
}

/** The sidebar's Mine order (0057) of the shown queues: their headings in the owner's order, and
 *  each heading's projects by their places, the ones never placed after them A–Z. */
export function mineLayout(queues: QueueInfo[]): Layout {
  return layoutOf(sidebarGroups(orderProjects(projectRows(queues, [], [], 0), "mine"), []));
}

/** `list` with the shown projects in `layout`'s order, then the others (hidden, or no longer a
 *  queue) as the core listed them. */
export function byPlace(list: Project[], layout: Layout): Project[] {
  const at = new Map(layout.flatMap((g) => g.keys).map((k, i) => [k, i]));
  const placed = list.filter((p) => at.has(p.key)).sort((a, b) => (at.get(a.key) ?? 0) - (at.get(b.key) ?? 0));
  return [...placed, ...list.filter((p) => !at.has(p.key))];
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
        <code>{prefix}</code> {canSet && <Button type="button" onClick={onEdit} title={`Set ${name}'s prefix`}>Set</Button>}
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
        aria-label={`${name}'s prefix`}
        title="2 to 4 lowercase letters"
      />{" "}
      <Button type="submit">Save</Button> <Button type="button" onClick={onCancel}>Cancel</Button>
      {error && <p className="err small" role="alert">{error}</p>}
    </form>
  );
}

/** The Projects table's columns, which a project's gate row spans (0061; Colour and Mine order, 0057). */
export const PROJECT_COLUMNS = 9;

/** A project's gate on a line under its row, with room to wrap like text; a column of it left the
 *  folder and the gate a word or a few letters wide. */
export function GateRow({ name, gate, hidden }: { name: string; gate: string; hidden: boolean }) {
  return (
    <tr className={hidden ? "gate-row dim" : "gate-row"}>
      <td colSpan={PROJECT_COLUMNS}>
        <b className="gate-label">Gate</b> <GateCell name={name} gate={gate} />
      </td>
    </tr>
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
