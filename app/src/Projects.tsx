import { Fragment, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { lastLabel, orderProjects, projectRows } from "./activity";
import { api } from "./api";
import { LimitCell } from "./CheckLimit";
import { HomePath } from "./HomePath";
import { tl, tn, Trans, useT } from "./i18n";
import InlineField from "./InlineField";
import { CATEGORY_MAX, LABEL_MAX, OTHER, otherCategories } from "./labels";
import { queueTotal } from "./lib";
import { type Layout, layoutOf, type Pending, placeSteps, settle, shownLayout, sidebarGroups } from "./projectOrder";
import { Swatches } from "./ProjectSwatches";
import { headingName } from "./SidebarProjects";
import { RunControls } from "./Run";
import type { HistoryState, Project, QueueInfo } from "./types";
import { Button } from "./ui";

/** Settings' Projects group (0036): every project with its label and category (0044), its colour
 *  and its place in the sidebar's Mine order (0057), prefix (0033), folder and task count, New
 *  queue and Add existing; the shown ones in that order. A folder under `home` shows as `~/…`. */
export default function Projects({ queues, home, onChanged }: { queues: QueueInfo[]; home: string; onChanged: () => void }) {
  const [list, setList] = useState<Project[] | null>(null);
  // Each local queue's history (0109), by key: read after the list, so the table never waits on it.
  const [histories, setHistories] = useState<Record<string, HistoryState>>({});
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  // The project whose prefix is being set, what's typed, and why the core refused it.
  const [prefixOf, setPrefixOf] = useState<string | null>(null);
  const [prefix, setPrefix] = useState("");
  const [prefixError, setPrefixError] = useState("");

  // The histories are read again while the pane is open, so the line keeps up with agents'
  // writes; a reply that comes after a newer one is dropped.
  const historyReplies = useRef(newestOnly());
  const loadHistories = useCallback(() => {
    const n = historyReplies.current.ask();
    // A history that can't be read shows no line; the rest of the table is unaffected.
    api.projectHistories().then(
      (l) => historyReplies.current.take(n) && setHistories(Object.fromEntries(l.map((h) => [h.key, h.history]))),
      () => historyReplies.current.take(n) && setHistories({}),
    );
  }, []);
  const load = useCallback(async () => {
    try {
      setList(await api.projects());
    } catch (e) {
      setMsg({ ok: false, text: String(e) });
      return;
    }
    loadHistories();
  }, [loadHistories]);
  useEffect(() => {
    void load();
  }, [load]);
  useEffect(() => {
    const t = setInterval(loadHistories, HISTORY_EVERY_MS);
    return () => clearInterval(t);
  }, [loadHistories]);

  const act = async <T,>(what: () => Promise<T>, done?: string | ((r: T) => string)) => {
    try {
      const r = await what();
      if (r !== null && done) setMsg({ ok: true, text: typeof done === "string" ? done : done(r) });
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
      setMsg({ ok: true, text: tn("shell", "projects.prefixSaved", { name: p.label || p.name, first: `${next}-1`, second: `${next}-2` }) });
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
    const { up, down } = placeSteps(pending, core, p.key, named, setPending, (plan) => api.arrangeProjects(plan.order, plan.categories, plan.moved));
    const go = (step: typeof up) => {
      if (!step) return;
      step().then(
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

  const t = useT("shell");
  return (
    <div className="projects">
      <div className="projects-bar">
        <Button onClick={() => void act(api.createProject, t("projects.created"))}>{t("projects.newQueue")}</Button>
        <Button onClick={() => void act(api.addProject, t("projects.added"))}>{t("projects.addExisting")}</Button>
        {msg && <span className={msg.ok ? "ok" : "err"}>{msg.text}</span>}
      </div>
      <p className="meta">
        <Trans
          ns="shell"
          k="projects.finds"
          params={{
            base: "~/playground",
            repo: "<checkout>/.muster",
            under: "~/playground/<checkout>",
            deeper: "~/playground/<project>/<checkout>",
            older: "<project>/tasks",
            olderDeeper: "~/playground/<group>/<project>/tasks",
            local: ".muster/",
            init: "muster init",
            tasks: "tasks/",
          }}
          components={{ code: <code /> }}
        />
      </p>
      {list === null && <p className="none">{t("common:loading")}</p>}
      <table className="projects-table">
        <thead>
          <tr><th title={t("projects.col.projectTitle")}>{t("projects.col.project")}</th><th title={t("projects.col.categoryTitle")}>{t("projects.col.category")}</th><th title={t("projects.col.colourTitle")}>{t("projects.col.colour")}</th><th title={t("projects.col.mineTitle")}>{t("projects.col.mine")}</th><th title={t("projects.col.prefixTitle")}>{t("projects.col.prefix")}</th><th>{t("projects.col.folder")}</th><th>{t("projects.col.tasks")}</th><th>{t("common:stage.your-check")}</th><th></th></tr>
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
                  <Button onClick={() => void act(() => api.hideProject(p.key, !p.hidden))}>{p.hidden ? t("common:show") : t("common:hide")}</Button>
                  {p.source === "added" && <Button onClick={() => void act(() => api.removeProject(p.key))}>{t("common:remove")}</Button>}
                </td>
              </tr>
              <ProjectGateRow
                p={p}
                history={histories[p.key]}
                onFix={(fix) => void act(() => runFix(fix, p.key), (n) => fixDone(fix, p.label || p.name, n))}
              />
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
  const t = useT("shell");
  return (
    <>
      <InlineField value={p.label ?? ""} onSave={onSave} max={LABEL_MAX} placeholder={p.name} label={t("projects.labelFor", { name: p.name })} />
      <div className="meta">
        <code>{p.name}</code>
        {p.source === "found" ? t("projects.found", { base: "~/playground" }) : t("projects.addedSource")}
        {p.hidden && t("projects.hidden")}
        {!p.ok && <span className="err">{t("projects.notQueue")}</span>}
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
  return <InlineField value={p.category ?? ""} onSave={onSave} max={CATEGORY_MAX} placeholder={p.groupName || p.group || headingName(OTHER)} label={tn("shell", "projects.categoryFor", { name: p.name })} options={categories} />;
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
      <Button disabled={!canUp} aria-label={tn("shell", "projects.moveUp", { name })} title={tn("shell", "projects.upTitle")} onClick={onUp}>↑</Button>
      <Button disabled={!canDown} aria-label={tn("shell", "projects.moveDown", { name })} title={tn("shell", "projects.downTitle")} onClick={onDown}>↓</Button>
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
        <code>{prefix}</code> {canSet && <Button type="button" onClick={onEdit} title={tn("shell", "projects.setPrefix", { name })}>{tl("set")}</Button>}
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
        aria-label={tn("shell", "projects.prefixOf", { name })}
        title={tn("shell", "projects.prefixRule")}
      />{" "}
      <Button type="submit">{tl("save")}</Button> <Button type="button" onClick={onCancel}>{tl("cancel")}</Button>
      {error && <p className="err small" role="alert">{error}</p>}
    </form>
  );
}

/** The Projects table's columns, which a project's gate row spans (0061; Colour and Mine order, 0057). */
export const PROJECT_COLUMNS = 9;

/** A project's gate on a line under its row, with room to wrap like text; a column of it left the
 *  folder and the gate a word or a few letters wide. Its run command (0077) is the line under it,
 *  with Run, or Stop while it runs (0079). */
export function GateRow({ name, gate, run, hidden, history, now, onFix }: { name: string; gate: string; run: string; hidden: boolean; history?: HistoryState; now?: number; onFix?: (fix: HistoryFix) => void }) {
  return (
    <tr className={hidden ? "gate-row dim" : "gate-row"}>
      <td colSpan={PROJECT_COLUMNS}>
        <div><b className="gate-label">{tn("shell", "projects.gate")}</b> <GateCell name={name} gate={gate} /></div>
        <div><b className="gate-label">{tn("shell", "projects.run")}</b> <RunCell name={name} run={run} />{run && <> <RunControls project={name} /></>}</div>
        {history?.kept && (
          <div><b className="gate-label">{tn("shell", "projects.history")}</b> <HistoryCell h={history} now={now ?? Date.now() / 1000} name={name} onFix={onFix} /></div>
        )}
      </td>
    </tr>
  );
}

/** The gate row of the project `p` as the core listed it: "" for a gate or run command it doesn't
 *  send; a History line when its queue keeps one (0109). */
export function ProjectGateRow({ p, history, onFix }: { p: Project; history?: HistoryState; onFix?: (fix: HistoryFix) => void }) {
  return <GateRow name={p.name} gate={p.gate ?? ""} run={p.run ?? ""} hidden={p.hidden} history={history} onFix={onFix} />;
}

/** How often Settings › Projects reads the histories again while it's open. */
export const HISTORY_EVERY_MS = 30_000;

/** Numbers requests and keeps only the newest reply: `ask()` before a request, `take(n)` when its
 *  reply comes, true when nothing newer has been shown yet. */
export function newestOnly(): { ask: () => number; take: (n: number) => boolean } {
  let asked = 0;
  let shown = 0;
  return {
    ask: () => ++asked,
    take: (n) => {
      if (n <= shown) return false;
      shown = n;
      return true;
    },
  };
}

/** What fixes a history in one click: `muster restore --all`, `muster history commit`, or
 *  `muster history removed --all` (files deleted on purpose). */
export type HistoryFix = "restore" | "commit" | "removed";

/** The core call behind each fix, for the project `key`; resolves to how many files it did. */
export function runFix(fix: HistoryFix, key: string, a: Pick<typeof api, "restoreHistory" | "commitHistory" | "recordRemoved"> = api): Promise<number> {
  if (fix === "restore") return a.restoreHistory(key);
  if (fix === "commit") return a.commitHistory(key);
  return a.recordRemoved(key);
}

/** What a fix of the project `name` did, from how many files it did (`n`). */
export function fixDone(fix: HistoryFix, name: string, n: number): string {
  if (n === 0) return tn("shell", `projects.fixed.${fix}None`, { name });
  return tn("shell", `projects.fixed.${fix}`, { name, files: tn("shell", "projects.files", { count: n }) });
}

/** A local queue's history (0109) in words: what's wrong first, with the command that fixes it,
 *  in the warning colour and saying so in words; else when it last committed, quietly. `fixes`
 *  are the buttons, the usual one first. `now` is seconds since 1970. */
export function historyLine(h: HistoryState, now: number): { text: string; command: string; warn: boolean; fixes: HistoryFix[] } {
  if (h.pending > 0) {
    const text = tn("shell", "projects.historyLine.pending", { count: h.pending });
    return { text: h.pendingError ? tn("shell", "projects.historyLine.withError", { text, error: h.pendingError }) : text, command: "muster history commit", warn: true, fixes: ["commit"] };
  }
  if (h.missing.length > 0)
    return { text: tn("shell", "projects.historyLine.missing", { count: h.missing.length }), command: "muster restore --all", warn: true, fixes: ["restore", "removed"] };
  if (!h.started) return { text: tn("shell", "projects.historyLine.notStarted"), command: "", warn: false, fixes: [] };
  const at = h.lastCommit === null ? null : Math.max(0, Math.floor((now - h.lastCommit) / 60));
  const committed = tn("shell", "projects.historyLine.committed", { when: lastLabel(at, now) });
  if (h.changed.length > 0)
    return { text: tn("shell", "projects.historyLine.changed", { committed, count: h.changed.length }), command: "muster history commit", warn: false, fixes: ["commit"] };
  return { text: committed, command: "", warn: false, fixes: [] };
}

/** Each fix's button: its words, and what it does to the project `name` for a screen reader. */
const fixLabel = (f: HistoryFix) => tn("shell", `projects.fix.${f}`);
const fixAria = (f: HistoryFix, name: string) => tn("shell", `projects.fix.${f}Aria`, { name });

/** The History line's value: the words, then the command when there's one to run, and a button
 *  for each fix (Restore, Commit, Keep deleted) for the project `name` when the window can. */
export function HistoryCell({ h, now, name, onFix }: { h: HistoryState; now: number; name: string; onFix?: (fix: HistoryFix) => void }) {
  const l = historyLine(h, now);
  return (
    <>
      <span className={l.warn ? "history-warn" : "meta"}>
        {l.text}
        {l.command && <>: <code>{l.command}</code></>}
      </span>
      {onFix &&
        l.fixes.map((f) => (
          <Fragment key={f}>
            {" "}
            <Button aria-label={fixAria(f, name)} onClick={() => onFix(f)}>{fixLabel(f)}</Button>
          </Fragment>
        ))}
    </>
  );
}

/** A project's gate command (0023), shown as it is: a merge runs it in the task's worktree, and
 *  never runs without one. Set with `muster projects set`, the one place that checks it. */
export function GateCell({ name, gate }: { name: string; gate: string }) {
  if (gate) return <code className="path">{gate}</code>;
  const command = `muster projects set ${name} gate "<command>"`; // i18n-ignore: a command, typed as it is
  return (
    <span className="meta">
      <Trans ns="shell" k="projects.noGate" params={{ command }} components={{ code: <code /> }} />
    </span>
  );
}

/** A project's run command (0077), shown as it is: what the app's Run starts in the main checkout
 *  to try the project. Left to right on its own (`dir="ltr"`), so a right-to-left word in or
 *  beside it can't reorder it. Set with `muster projects set`, as the gate is. */
export function RunCell({ name, run }: { name: string; run: string }) {
  if (run) return <code className="path" dir="ltr">{run}</code>;
  const command = `muster projects set ${name} run "<command>"`; // i18n-ignore: a command, typed as it is
  return (
    <span className="meta">
      <Trans ns="shell" k="projects.noRun" params={{ command }} components={{ code: <code dir="ltr" /> }} />
    </span>
  );
}
