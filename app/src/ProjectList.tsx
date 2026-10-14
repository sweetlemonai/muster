import { Fragment, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { lastLabel, loadOrder, nextOrder, type OrderMemo, type ProjectRow, projectRows, SIDEBAR_ORDERS, storeOrder, sumActivity, type ProjectActivity, type SidebarOrder, type Sums } from "./activity";
import { projectCount } from "./Backlog";
import { SideLimit } from "./CheckLimit";
import { needsYouCount } from "./human";
import { api, type Moved } from "./api";
import { LABEL_MAX } from "./labels";
import { lessonsWaiting } from "./Lessons";
import { inProgress } from "./lib";
import { useLook } from "./modes";
import { PAUSE_GLYPH } from "./Pause";
import { QuietPing } from "./Ping";
import { accepts, dropMove, type DropTarget, grouped, headingFolds, layoutOf, peeks, settleFolds, type Move, type Pending, type Plan, planMove, settle, sidebarGroups, track, withFresh } from "./projectOrder";
import { ColourDot } from "./ProjectSwatches";
import { pressed } from "./shortcuts";
import { cleanHeading, type DragProps, half, HeadingEdit, HeadingFrame, MOVE_KEYS, NewCategory, SideRow } from "./SidebarProjects";
import type { Activity, Note, QueueInfo, Task } from "./types";
import { Pill } from "./ui";
import { cx } from "./ui/cx";

/** What the sidebar asks the core to keep when the owner arranges it in place (0057): the order
 *  after a move (projects by key, headings, the project that went under another heading), a
 *  heading renamed, and a project's colour (by key, "" for none). Each settles once the window
 *  has read the queues again. */
export type ProjectEdits = {
  arrange: (order: string[], categories: string[], moved: Moved | null) => Promise<void>;
  renameCategory: (from: string, to: string) => Promise<void>;
  colour: (key: string, colour: string) => Promise<void>;
  /** A heading folded or unfolded (0070), by its name. */
  fold: (heading: string, folded: boolean) => Promise<void>;
};

/** The edits through the core, each settling once `after` (the window's read of the queues) has. */
export const projectEdits = (after: () => Promise<void>): ProjectEdits => ({
  arrange: (order, categories, moved) => api.arrangeProjects(order, categories, moved).then(after),
  renameCategory: (from, to) => api.renameCategory(from, to).then(after),
  colour: (key, colour) => api.setProjectColour(key, colour).then(after),
  fold: (heading, folded) => api.foldCategory(heading, folded).then(after),
});

/** What's being dragged, and where it would land. */
type Drag = { kind: "project"; key: string } | { kind: "heading"; name: string };
type Over = { kind: "row"; key: string; side: "before" | "after" } | { kind: "heading"; name: string; side: "before" | "after" | "into" };
/** What gets the focus back after a move or a pick, which puts rows in new places. */
type Focus = { kind: "row" | "dot"; key: string } | { kind: "heading"; name: string };

/** The sidebar's projects (0037): All first, then the order chips and one row per queue under its
 *  category's heading (0044), each called by its label and saying whether it's being worked on
 *  now, or stalled, and when it last moved. `now` is the state's clock, seconds since 1970.
 *  `onLabel` saves a label set by double-clicking a row's name (by the project's key, its folder);
 *  without it, names can't be edited here. With `edit` (0057) the owner arranges it in place:
 *  rows and headings dragged or moved with ⌥↑/⌥↓ (the first move picks Mine), a project dropped
 *  on another heading goes under it, + Category makes a heading, a double click renames one, and
 *  a project's dot picks its colour. */
export default function ProjectList({ queues, tasks, notes, activity, now, current, onPick, onLabel, edit }: {
  queues: QueueInfo[];
  tasks: Task[];
  notes: Note[];
  activity: Activity[];
  now: number;
  current: string;
  onPick: (queue: string) => void;
  onLabel?: (key: string, label: string) => Promise<void>;
  edit?: ProjectEdits;
}) {
  const [order, setOrder] = useState<SidebarOrder>(() => loadOrder());
  const pick = (o: SidebarOrder) => {
    setOrder(o);
    storeOrder(o);
  };
  const rows = useMemo(() => projectRows(queues, tasks, activity, now), [queues, tasks, activity, now]);
  // The order holds between polls unless what defines it changed (nextOrder), so rows don't jump.
  const memo = useRef<OrderMemo | null>(null);
  // The project whose label is being set in place, by its name: while it is, the order holds, so
  // the field never moves under the caret (which would leave it, and save half a label).
  const [editing, setEditing] = useState<string | null>(null);
  // Arranging in place (0057): what's dragged and where it would land; the layout a move showed
  // until the queues read back say the same; the new headings with no project yet; the heading
  // being renamed; the row whose colours are laid out; what a screen reader hears; a refusal.
  const [drag, setDrag] = useState<Drag | null>(null);
  const [over, setOver] = useState<Over | null>(null);
  const [pending, setPending] = useState<Pending | null>(null);
  const [fresh, setFresh] = useState<string[]>([]);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [swatch, setSwatch] = useState<string | null>(null);
  const [said, setSaid] = useState("");
  const [problem, setProblem] = useState("");
  const [focus, setFocus] = useState<Focus | null>(null);
  // Folds set here (0070), shown at once, until the core says the same.
  const [foldSet, setFoldSet] = useState<Record<string, boolean>>({});
  const nav = useRef<HTMLElement>(null);
  // While a row is dragged the order holds too, so nothing moves under the pointer.
  memo.current = nextOrder(memo.current, rows, order, editing !== null || drag !== null);
  const byName = new Map(rows.map((r) => [r.name, r]));
  const shown = memo.current.names.flatMap((n) => byName.get(n) ?? []);
  const fromCore = sidebarGroups(shown, fresh);
  const groups = pending ? withFresh(grouped(shown, pending.layout), fresh) : fromCore;
  const layout = layoutOf(groups);
  // A move shows at once; once it's saved, the queues read back take over (projectOrder.settle).
  const core = useRef(layout);
  core.current = layoutOf(fromCore);
  useEffect(() => setPending((p) => settle(p, core.current)), [queues]);
  const folds = headingFolds(rows, foldSet);
  const coreFolds = useRef(folds);
  coreFolds.current = headingFolds(rows, {});
  useEffect(() => setFoldSet((f) => {
    const kept = settleFolds(f, coreFolds.current);
    return Object.keys(kept).length === Object.keys(f).length ? f : kept;
  }), [queues]);
  /** Fold or unfold a heading: at once, then kept by the core; a refusal puts it back. */
  const fold = (name: string, folded: boolean) => {
    if (!edit) return;
    setFoldSet((f) => ({ ...f, [name]: folded }));
    setSaid(`${name} ${folded ? "folded" : "unfolded"}`);
    edit.fold(name, folded).catch((e: unknown) => {
      setFoldSet((f) => Object.fromEntries(Object.entries(f).filter(([n]) => n !== name)));
      setProblem(`Couldn't ${folded ? "fold" : "unfold"} ${name}: ${String(e)}`);
    });
  };
  useEffect(() => {
    if (!focus) return;
    const sel =
      focus.kind === "heading"
        ? `.side-cat[data-heading="${CSS.escape(focus.name)}"]`
        : `.side-row[data-key="${CSS.escape(focus.key)}"] ${focus.kind === "dot" ? ".side-swatch-dot" : ".side-item"}`;
    nav.current?.querySelector<HTMLElement>(sel)?.focus();
  }, [focus]);
  const labelOfKey = (key: string) => rows.find((r) => r.key === key)?.label ?? key;
  /** A move: shown at once, Mine picked (the first move chooses it), kept by the core. */
  const run = (p: Plan | null, then: Focus | null) => {
    if (!p || !edit) return;
    pick(p.chip);
    setSaid(p.said);
    setProblem("");
    if (then) setFocus({ ...then });
    // A project moved under a folded heading unfolds it, so the row stays in sight (0070).
    if (p.moved && folds.get(p.moved.category)) fold(p.moved.category, false);
    track(p, setPending, () => edit.arrange(p.order, p.categories, p.moved)).then(
      () => {
        if (p.moved) {
          const made = p.moved.category;
          setFresh((f) => f.filter((x) => x !== made));
        }
      },
      (e: unknown) => {
        setProblem(`Couldn't keep that order: ${String(e)}`);
      },
    );
  };
  const move = (m: Move, then: Focus | null = null) => run(planMove(layout, m, labelOfKey), then);
  const hover = (o: Over | null) => setOver((was) => (JSON.stringify(was) === JSON.stringify(o) ? was : o));
  const endDrag = () => {
    setDrag(null);
    setOver(null);
  };
  /** A drop on `t` (the innermost target that takes it: one that doesn't lets it bubble on): its move. */
  const dropOn = (t: DropTarget, side: "before" | "after") => {
    const m = dropMove(layout, drag, t, side);
    if (m) move(m);
    endDrag();
  };
  /** Where the drop line goes for what's dragged over `t`: none over the dragged one itself. */
  const overOf = (t: DropTarget, side: "before" | "after"): Over | null => {
    if (!drag) return null;
    if (t.kind === "row") return drag.kind === "project" && drag.key !== t.key ? { kind: "row", key: t.key, side } : null;
    if (drag.kind === "project") return { kind: "heading", name: t.name, side: "into" };
    return drag.name === t.name ? null : { kind: "heading", name: t.name, side };
  };
  const targetDrag = (t: DropTarget, own: Drag, onStep: (by: -1 | 1) => void): Omit<DragProps, "dragging" | "drop"> => ({
    onDragStart: () => {
      setSwatch(null);
      setDrag(own);
    },
    onDragEnd: endDrag,
    accepts: accepts(drag, t),
    onOver: (side) => hover(overOf(t, side)),
    onDrop: (side) => dropOn(t, side),
    onStep,
  });
  const rowDrag = (r: ProjectRow): DragProps => ({
    dragging: drag?.kind === "project" && drag.key === r.key,
    drop: over?.kind === "row" && over.key === r.key ? over.side : null,
    ...targetDrag({ kind: "row", key: r.key }, { kind: "project", key: r.key }, (by) => move({ kind: "step-project", key: r.key, by }, { kind: "row", key: r.key })),
  });
  const headingDrag = (name: string): DragProps => ({
    dragging: drag?.kind === "heading" && drag.name === name,
    drop: over?.kind === "heading" && over.name === name && over.side === "into" ? "into" : null,
    ...targetDrag({ kind: "heading", name }, { kind: "heading", name }, (by) => move({ kind: "step-heading", name, by }, { kind: "heading", name })),
  });
  /** A heading renamed: a new one (no project yet) here, any other by the core. */
  const rename = (from: string, to: string): Promise<void> => {
    if (fresh.includes(from)) {
      setFresh((f) => (groups.some((g) => g.name === to) ? f.filter((x) => x !== from) : f.map((x) => (x === from ? to : x))));
      return Promise.resolve();
    }
    if (!edit) return Promise.resolve();
    return edit.renameCategory(from, to).then(() => {
      setSaid(`${from} renamed ${to}`);
      setFocus({ kind: "heading", name: to });
    });
  };
  const addHeading = (typed: string): string => {
    const name = cleanHeading(typed);
    if (groups.some((g) => g.name === name)) return `${name} is a heading already: drag a project onto it`;
    setFresh((f) => [...f, name]);
    setSaid(`New heading ${name}: drag a project onto it, or ⌥↓ the last one above`);
    setFocus({ kind: "heading", name });
    return "";
  };
  const pickColour = (r: ProjectRow, id: string) => {
    setSwatch(null);
    setFocus({ kind: "dot", key: r.key });
    if (!edit || id === r.colour) return;
    edit.colour(r.key, id).catch((e: unknown) => setProblem(`Couldn't set ${r.label}'s colour: ${String(e)}`));
  };
  const sums = useMemo(() => sumActivity(rows.map((r) => r.activity)), [rows]);
  const counts = useMemo(() => new Map(queues.map((q) => [q.name, q.counts])), [queues]);
  const totalOf = (r: ProjectRow) => projectCount(counts.get(r.name) ?? {}, notes, r.name);
  // Focus (0038) narrows the sidebar to the names, the working dot, stalled and the needs-you and paused counts;
  // a stalled project keeps its Ask all quiet sessions under it.
  const full = useLook("sidebar") === "full";
  return (
    <nav
      className="projects-nav"
      ref={nav}
      // Over nothing that takes the drag (a gap, + Category, past the last group), or off the
      // sidebar: no drop line (review R5).
      onDragOver={drag ? (e) => !e.isDefaultPrevented() && hover(null) : undefined}
      onDragLeave={drag ? (e) => !(e.relatedTarget instanceof Node && e.currentTarget.contains(e.relatedTarget)) && hover(null) : undefined}
    >
      <ProjectItem name="All" total={tasks.length + notes.length} tasks={tasks} lessons={lessonsWaiting(queues, "all")} on={current === "all"} onClick={() => onPick("all")} working={sums.working > 0} stalled={sums.stalled > 0 ? `${sums.stalled} stalled` : ""}>
        <SumsLine s={sums} now={now} />
      </ProjectItem>
      {full && <div className="side-label">Projects</div>}
      {full && <OrderChips value={order} onChange={pick} />}
      {groups.map((g) => {
        const isFresh = g.rows.length === 0;
        const isFolded = Boolean(edit) && !isFresh && folds.get(g.name) === true;
        // A heading dragged over another heading's group lands before or after the whole group.
        const groupDrop = over?.kind === "heading" && over.name === g.name && over.side !== "into" ? `drop-${over.side}` : "";
        return (
          <div
            key={g.name}
            className={cx("side-group", groupDrop)}
            role="group"
            aria-label={g.name}
            onDragOver={
              edit
                ? (e) => {
                    const t = { kind: "group", name: g.name } as const;
                    if (!accepts(drag, t)) return;
                    e.preventDefault();
                    e.dataTransfer.dropEffect = "move";
                    hover(overOf(t, half(e)));
                  }
                : undefined
            }
            onDrop={
              edit
                ? (e) => {
                    const t = { kind: "group", name: g.name } as const;
                    if (!accepts(drag, t)) return;
                    e.preventDefault();
                    dropOn(t, half(e));
                  }
                : undefined
            }
          >
            {renaming === g.name && edit ? (
              <HeadingEdit name={g.name} onSave={(to) => rename(g.name, to)} onDone={() => setRenaming(null)} />
            ) : (
              <CategoryHead
                name={g.name}
                rows={g.rows}
                total={g.rows.reduce((n, r) => n + totalOf(r), 0)}
                fresh={isFresh}
                drag={edit && !isFresh ? headingDrag(g.name) : undefined}
                onRename={edit ? () => setRenaming(g.name) : undefined}
                onRemove={isFresh ? () => setFresh((f) => f.filter((x) => x !== g.name)) : undefined}
                into={isFresh && edit && drag?.kind === "project" ? headingDrag(g.name) : undefined}
                folded={isFolded}
                onFold={edit && !isFresh ? (v) => fold(g.name, v) : undefined}
              />
            )}
            {isFolded && <Peeks heading={g.name} rows={peeks(g.rows, current)} current={current} onPick={onPick} />}
            {!isFolded && g.rows.map((r) => {
              const q = queues.find((x) => x.name === r.name);
              if (editing === r.name && onLabel && q) {
                return <SideLabelEdit key={r.name} name={r.name} label={r.label} onSave={(v) => onLabel(q.path, labelToSave(v, r.name))} onDone={() => setEditing(null)} />;
              }
              return (
                <Fragment key={r.name}>
                  <SideRow
                    rowKey={r.key}
                    label={r.label}
                    colour={r.colour}
                    swatches={swatch === r.key}
                    onDot={edit ? () => setSwatch((s) => (s === r.key ? null : r.key)) : undefined}
                    onColour={edit ? (id) => pickColour(r, id) : undefined}
                    onFold={(by) => {
                      setSwatch(null);
                      // Esc gives the focus back to the dot; tabbing away leaves it where it went.
                      if (by === "key") setFocus({ kind: "dot", key: r.key });
                    }}
                    drag={edit ? rowDrag(r) : undefined}
                  >
                    <ProjectItem name={r.label} title={r.title} total={totalOf(r)} tasks={r.tasks} lessons={lessonsWaiting(queues, r.name)} on={current === r.name} onClick={() => onPick(r.name)} onRename={onLabel ? () => setEditing(r.name) : undefined} keys={edit ? MOVE_KEYS : undefined} working={r.activity.working > 0} stalled={r.activity.stalled ? "stalled" : ""} alert={<SideLimit q={q} />}>
                      <ActivityLine a={r.activity} now={now} />
                    </ProjectItem>
                  </SideRow>
                  {/* A stalled project offers to ask its quiet sessions for a check-in (0039), under its
                      row, since the row is itself a button. */}
                  {r.activity.stalled && <QuietPing tasks={r.tasks} now={now} />}
                </Fragment>
              );
            })}
          </div>
        );
      })}
      {full && edit && <NewCategory onAdd={addHeading} />}
      {problem && <p className="side-problem err" role="alert">{problem}</p>}
      <p className="sr-only" role="status" aria-live="polite">{said}</p>
    </nav>
  );
}

/** One project in the sidebar: its name (its label, 0044) and count, what's in progress, what's
 *  paused (0045) and what needs you (the needs-you count: your own tasks to do and doing, what the
 *  agents wait on you for, and the lessons from reviews waiting for your yes, 0027), and under
 *  them its activity line. Narrowed (Focus, 0038), the name, the working dot, "stalled" in the attention colour (the activity line's word, or "2
 *  stalled" on All), the needs-you count and the paused count. `alert` shows in every mode, under
 *  them: over its Your check limit (0050), how many wait and the limit. With `onRename`, a double
 *  click on the name sets its label. `keys` are the keys that move it (0057), for a screen reader. */
export function ProjectItem({ name, title, total, tasks, lessons = 0, on, onClick, onRename, keys, working = false, stalled = "", alert, children }: { name: string; title?: string; total: number; tasks: Task[]; lessons?: number; on: boolean; onClick: () => void; onRename?: () => void; keys?: string; working?: boolean; stalled?: string; alert?: ReactNode; children: ReactNode }) {
  const doing = tasks.filter(inProgress).length;
  const needs = needsYouCount(tasks) + lessons;
  const full = useLook("sidebar") === "full";
  const paused = tasks.filter((t) => t.paused).length;
  return (
    <button className={on ? "side-item on" : "side-item"} onClick={onClick} title={title} aria-keyshortcuts={keys}>
      <span className="side-line">
        {!full && working && <i className="dot side-dot" title={`Being worked on now: ${WORKING_HINT}`} />}
        <span className="side-name" title={onRename ? (title ? `${title}\n${RENAME_HINT}` : RENAME_HINT) : undefined} onDoubleClick={onRename}>{name}</span>
        {!full && stalled && <span className="side-stalled" title={`Stalled: ${STALLED_HINT}`}>{stalled}</span>}
        <span className="side-badges">
          {needs > 0 && <span className="count warn" title={`${needs} need you`}>{needs}</span>}
          {full && doing > 0 && <span className="side-doing" title={`${doing} in progress (Building or Testing)`}>{doing}</span>}
          {paused > 0 && <span className="side-paused" title={`${paused} paused`}>{`${PAUSE_GLYPH} ${paused}`}</span>}
          {full && <small>{total}</small>}
        </span>
      </span>
      {full && children}
      {alert}
    </button>
  );
}

/** A category's heading (0044), always shown, above its projects: its name, what needs you in it,
 *  and, but in Focus's narrow sidebar, how many of its tasks are being worked on and its tasks.
 *  Arranged in place (0057): `drag` moves it, `onRename` renames it, and a new one (`fresh`) takes
 *  a dragged project (`into`) or Remove. */
function CategoryHead({ name, rows, total, fresh = false, drag, onRename, onRemove, into, folded = false, onFold }: {
  name: string;
  rows: ProjectRow[];
  total: number;
  fresh?: boolean;
  drag?: DragProps;
  onRename?: () => void;
  onRemove?: () => void;
  into?: DragProps;
  /** Folded (0070): it says how many projects it holds, and what's being worked on in every mode. */
  folded?: boolean;
  onFold?: (folded: boolean) => void;
}) {
  const full = useLook("sidebar") === "full";
  const needs = rows.reduce((n, r) => n + r.needs, 0);
  const working = rows.reduce((n, r) => n + r.activity.working, 0);
  const projects = `${rows.length} ${rows.length === 1 ? "project" : "projects"}`;
  return (
    <HeadingFrame name={name} count={rows.length} fresh={fresh} folded={folded} drag={drag ?? into} onRename={onRename} onRemove={onRemove} onFold={onFold}>
      <span className="side-cat-name">{name}</span>
      <span className="side-badges">
        {folded && <small className="side-cat-projects" title={`${projects} in ${name}, folded`}>{projects}</small>}
        {needs > 0 && <span className="count warn" title={`${needs} need${needs === 1 ? "s" : ""} you in ${name}`}>{needs}</span>}
        {(full || folded) && working > 0 && (
          <span className="side-working" title={`${working} ${working === 1 ? "task" : "tasks"} being worked on now in ${name}: ${WORKING_HINT}`}>
            <i className="dot" aria-hidden="true" />
            {working} working
          </span>
        )}
        {full && !fresh && <small title={`${total} ${total === 1 ? "task" : "tasks"} in ${projects}`}>{total}</small>}
      </span>
    </HeadingFrame>
  );
}

/** What a folded heading still shows (0070): the selected project and the ones that need you, as
 *  chips under it, each with its colour and what needs you; a click picks it. */
function Peeks({ heading, rows, current, onPick }: { heading: string; rows: ProjectRow[]; current: string; onPick: (queue: string) => void }) {
  if (rows.length === 0) return null;
  return (
    <div className="side-peeks" role="group" aria-label={`Shown from folded ${heading}`}>
      {rows.map((r) => (
        <Pill key={r.name} type="button" on={current === r.name} aria-pressed={current === r.name} className="side-peek" title={`${r.title}: in ${heading}, folded`} onClick={() => onPick(r.name)}>
          <ColourDot name={r.label} colour={r.colour} />
          {r.label}
          {r.needs > 0 && (
            <>
              {" "}
              <span className="count warn" title={`${r.needs} need${r.needs === 1 ? "s" : ""} you`}>{r.needs}</span>
            </>
          )}
        </Pill>
      ))}
    </div>
  );
}

/** What a label typed in place saves: the project's own name typed back is no label at all. */
export const labelToSave = (typed: string, name: string) => (typed.trim() === name ? "" : typed.trim());

/** A project's label set in place (0044), in its row's place: the field, with the real name and
 *  the keys under it. ↩ or leaving the field saves, Esc keeps the label as it was; a refusal shows
 *  under the field, which stays open. */
export function SideLabelEdit({ name, label, onSave, onDone }: { name: string; label: string; onSave: (label: string) => Promise<void>; onDone: () => void }) {
  const [error, setError] = useState("");
  // Saved or left once: ↩ blurs the field too, and Esc's blur saves nothing.
  const settled = useRef(false);
  const save = (v: string) => {
    if (settled.current) return;
    settled.current = true;
    if (v.trim() === label) {
      onDone();
      return;
    }
    onSave(v).then(onDone, (e: unknown) => {
      settled.current = false;
      setError(String(e));
    });
  };
  return (
    <div className="side-item on side-editing">
      <input
        className="side-label-input"
        autoFocus
        defaultValue={label}
        maxLength={LABEL_MAX}
        aria-label={`Label for ${name}`}
        placeholder={name}
        onKeyDown={(e) => {
          if (pressed(e, "label-save")) save(e.currentTarget.value);
          if (pressed(e, "label-revert")) {
            // Esc here leaves the label as it was, not the pane.
            e.stopPropagation();
            settled.current = true;
            onDone();
          }
        }}
        onBlur={(e) => save(e.currentTarget.value)}
      />
      <small className="meta">{name} · ↩ saves, Esc keeps it</small>
      {error && <small className="err" role="alert">{error}</small>}
    </div>
  );
}

const RENAME_HINT = "Double-click to change the label";
const WORKING_HINT = "a live session, or a check-in or heartbeat in the last 30 minutes";
const STALLED_HINT = "work in Building, but nothing for over 30 minutes: no check-in, heartbeat, stage change, commit or file change";

/** A project's activity, quietly: the Building-colour dot and "2 working", or "stalled" in the
 *  attention colour, then when it last moved. The line is always there, so rows don't jump. */
export function ActivityLine({ a, now }: { a: ProjectActivity; now: number }) {
  return (
    <span className="side-activity">
      <span className="side-state">
        {a.working > 0 ? (
          <span className="side-working" title={`${a.working} ${a.working === 1 ? "task" : "tasks"} being worked on now: ${WORKING_HINT}`}>
            <i className="dot" aria-hidden="true" />
            {a.working} working
          </span>
        ) : a.stalled ? (
          <span className="side-stalled" title={`Stalled: ${STALLED_HINT}`}>stalled</span>
        ) : null}
      </span>
      <When m={a.last} now={now} />
    </span>
  );
}

/** The All projects row's sums: projects being worked on, projects stalled, the newest activity. */
export function SumsLine({ s, now }: { s: Sums; now: number }) {
  const projects = (n: number) => `${n} ${n === 1 ? "project" : "projects"}`;
  return (
    <span className="side-activity">
      <span className="side-state">
        {s.working > 0 && (
          <span className="side-working" title={`${projects(s.working)} being worked on now: ${WORKING_HINT}`}>
            <i className="dot" aria-hidden="true" />
            {s.working} working
          </span>
        )}
        {s.working > 0 && s.stalled > 0 && " · "}
        {s.stalled > 0 && <span className="side-stalled" title={`${projects(s.stalled)} stalled: ${STALLED_HINT}`}>{s.stalled} stalled</span>}
      </span>
      <When m={s.last} now={now} />
    </span>
  );
}

function When({ m, now }: { m: number | null; now: number }) {
  return <span className="side-when" title={m === null ? "Nothing says when this was last worked on" : `Last activity: ${lastLabel(m, now)}`}>{lastLabel(m, now)}</span>;
}

/** The sidebar's order, as chips: always on screen, the current one pressed. */
export function OrderChips({ value, onChange }: { value: SidebarOrder; onChange: (o: SidebarOrder) => void }) {
  return (
    <span className="side-order" role="group" aria-label="Order projects">
      {SIDEBAR_ORDERS.map((o) => (
        <Pill key={o.id} type="button" on={value === o.id} aria-pressed={value === o.id} title={o.title} onClick={() => onChange(o.id)}>
          {o.label}
        </Pill>
      ))}
    </span>
  );
}
