import { fmt } from "./i18n";
import { useContext, useEffect, useRef, useState, type KeyboardEvent } from "react";
import BlockWhy from "./BlockWhy";
import { noteTitle } from "./Backlog";
import { RowCue } from "./ColumnRow";
import { Hl } from "./FilterBar";
import {
  type ActiveCell,
  activeCell,
  alertLine,
  cardStatus,
  type Cell,
  CELL_CAP,
  cellShown,
  checkFull,
  checkHead,
  columnChips,
  dimmed,
  doneTodayWords,
  GRID_STAGES,
  type GridItem,
  type GridRow,
  moveCell,
  stageWho,
  troubleOf,
  viewSpan,
} from "./featureGridData";
import { needsYou } from "./human";
import { useT } from "./i18n";
import { key, type RoleNames } from "./lib";
import { GROUP_BYS, type GroupBy, progressText } from "./levels";
import { PAUSE_GLYPH } from "./Pause";
import { type ChipSpan, inView, overflowCue } from "./rowCue";
import { RoleNamesContext } from "./settingsData";
import { pressed } from "./shortcuts";
import ProjectName from "./ProjectName";
import { taskKey } from "./taskKey";
import type { Note, QueueInfo, Task } from "./types";
import { Badge, Pill, Segment, Segmented } from "./ui";

// The Feature grid (mu-143), the board's second layout: features (or goals) down the side, the
// stages across with who works each, small cards with one status, Done as today's count. The
// model is featureGridData.ts; this draws it as an ARIA grid, so a screen reader names a card's
// row and column ("Orchestration, Building"), and the arrow keys move between cells.

/** The grid's rows by: the board's own Group by, its words too (levels.ts), feature first. */
const ROWS_BY = (["feature", "goal", "none"] as const).map((id) => GROUP_BYS.find((g) => g.id === id)!);
/** The columns a row has: its stages, then Done. */
const COLS = GRID_STAGES.length + 1;

export type FeatureGridProps = {
  rows: GridRow[];
  /** The project shown, or "all", with every project's info for the Your check limit. */
  queue: string;
  queues: QueueInfo[];
  /** What the grid's title says: the project's label, or "All projects". */
  title: string;
  header: { needs: string; rest: string };
  /** Whether something waits for the owner: the header's first part is in the needs-you colour. */
  needs: number;
  by: GroupBy;
  onBy: (g: GroupBy) => void;
  multi: boolean;
  /** Only what needs me: dims every card that doesn't wait on the owner. */
  mine: boolean;
  onMine: (on: boolean) => void;
  now: number;
  words: string[];
  openKey: string | null;
  openNoteKey: string | null;
  onOpen: (t: Task) => void;
  onOpenNote: (n: Note) => void;
};

export default function FeatureGrid(p: FeatureGridProps) {
  const t = useT("board");
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(() => new Set());
  // Role slots go by their role's name now (0112), in the who lines and on the cards.
  const roles = useContext(RoleNamesContext);
  const [active, setActive] = useState<ActiveCell>({ row: "", c: 0 });
  const grid = useRef<HTMLDivElement>(null);
  const frame = useRef<HTMLDivElement>(null);
  const at = activeCell(p.rows, active);
  const rowId = (r: number) => p.rows[r]?.id ?? "";
  // The columns wholly in view, or null when the whole grid fits; and the column the cue's last
  // click went to, as on the board's row of folded columns (rowCue.ts).
  const [inSight, setInSight] = useState<string[] | null>(null);
  const [from, setFrom] = useState<string | null>(null);
  useEffect(() => {
    if (!frame.current || !grid.current) return;
    return watchGrid(frame.current, grid.current, (v) => setInSight((prev) => (sameIds(prev, v) ? prev : v)));
  }, []);
  const chips = columnChips(p.rows);
  const cue = inSight ? overflowCue(chips, inSight, from) : null;
  const full = checkFull(p.queues, p.queue);
  const alert = alertLine(full, p.queue);
  const check = checkHead(p.queues, p.queue);
  const doneToday = p.rows.reduce((n, r) => n + r.doneToday, 0);
  const toggle = (id: string) =>
    setExpanded((s) => {
      const next = new Set(s);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  const cellAt = (c: Cell) => grid.current?.querySelector<HTMLElement>(`[data-cell="${c.r}:${c.c}"]`) ?? null;
  const go = (c: Cell) => {
    setActive({ row: rowId(c.r), c: c.c });
    cellAt(c)?.focus();
  };
  /** The cue's click: the column into view just after the feature names, and the Tab stop's row's
   *  cell in it focused, so the focus never stays on a cue that's gone. */
  const showColumn = (id: string) => {
    setFrom(id);
    const c = chips.findIndex((x) => x.id === id);
    const cell = cellAt({ r: at.r, c });
    const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    (cell ?? grid.current?.querySelector<HTMLElement>(`[data-col="${id}"]`))?.scrollIntoView({ inline: "start", block: "nearest", behavior: still ? "auto" : "smooth" });
    if (cell) {
      setActive({ row: rowId(at.r), c });
      cell.focus({ preventScroll: true });
    }
  };
  const headAt = (r: number) => grid.current?.querySelector<HTMLElement>(`[data-rowhead="${r}"] button`) ?? null;
  // The grid is one tab stop (the active cell): the cards, "+N more" and the feature names are
  // reached with the keys, never with Tab.
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement;
    const rtl = grid.current ? getComputedStyle(grid.current).direction === "rtl" : false;
    const forward = rtl ? "ArrowLeft" : "ArrowRight";
    const back = rtl ? "ArrowRight" : "ArrowLeft";
    const head = target.closest<HTMLElement>("[data-rowhead]");
    if (head) {
      // A feature's name: → back to its row's first cell, ↑ ↓ to the feature above or below.
      const r = Number(head.dataset.rowhead);
      if (!pressed(e, "grid-move")) return;
      e.preventDefault();
      if (e.key === forward) go({ r, c: 0 });
      else if (e.key !== back) {
        const to = moveCell({ r, c: 0 }, e.key, { rows: p.rows.length, cols: COLS }, rtl);
        if (to) (headAt(to.r) ?? cellAt({ r: to.r, c: 0 }))?.focus();
      }
      return;
    }
    const cell = target.closest<HTMLElement>("[data-cell]");
    if (!cell) return;
    if (target !== cell) {
      // Inside a cell: Esc goes back to the cell, ↑ ↓ step through its cards and "+N more", and
      // ↩ opens a card (they're buttons).
      if (pressed(e, "grid-leave")) {
        e.preventDefault();
        e.stopPropagation();
        cell.focus();
      } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
        if (!pressed(e, "grid-move")) return;
        e.preventDefault();
        const items = [...cell.querySelectorAll<HTMLElement>("button")];
        const i = items.indexOf(target) + (e.key === "ArrowDown" ? 1 : -1);
        items[Math.max(0, Math.min(items.length - 1, i))]?.focus();
      }
      return;
    }
    const [r, c] = (cell.dataset.cell ?? "0:0").split(":").map(Number);
    if (pressed(e, "grid-enter")) {
      // Enter goes into the cell: its first card, or its "+N more".
      const first = cell.querySelector<HTMLElement>("button");
      if (first) {
        e.preventDefault();
        first.focus();
      }
      return;
    }
    if (!pressed(e, "grid-move") && !pressed(e, "grid-ends")) return;
    // ← from Backlog (→ in right-to-left) goes to the feature's name, which opens it.
    const name = c === 0 && e.key === back ? headAt(r) : null;
    if (name) {
      e.preventDefault();
      name.focus();
      return;
    }
    const next = moveCell({ r, c }, e.key, { rows: p.rows.length, cols: COLS }, rtl);
    if (!next) return;
    e.preventDefault();
    go(next);
  };

  return (
    <section className="fg" aria-label={t("grid.label")}>
      <div className="fg-top">
        <h2 className="fg-title">{p.title}</h2>
        <p className="fg-summary">
          <b className={p.needs ? "fg-needs" : undefined}>{p.header.needs}</b>
          {t("common:listSep")}
          {p.header.rest}
        </p>
        <span className="fg-tools">
          <Segmented aria-label={t("grid.rowsBy")} className="fg-by">
            {ROWS_BY.map((o) => (
              <Segment key={o.id} type="button" active={p.by === o.id} onClick={() => p.onBy(o.id)}>
                {o.label}
              </Segment>
            ))}
          </Segmented>
          <Pill type="button" className="fg-mine-pick" on={p.mine} aria-pressed={p.mine} onClick={() => p.onMine(!p.mine)} title={t("grid.mineTitle")}>
            {t("grid.mine")}
          </Pill>
        </span>
      </div>
      {/* The alert is always there, empty while Your check has room, so the grid never moves when it
          comes. At its end, over the grid's end edge, the cue for the columns past the edge (review
          R1): macOS hides the scrollbar, and Your check is often the one out of sight. */}
      <div className="fg-over">
        <p className={alert ? "fg-alert fg-alert-on" : "fg-alert"} role="status">
          {alert}
        </p>
        {cue && <RowCue cue={cue} onShow={() => showColumn(cue.target)} />}
      </div>
      <div className="fg-frame" ref={frame}>
        <div className="fg-grid" role="grid" aria-label={t("grid.labelOf", { title: p.title })} aria-rowcount={p.rows.length + 1} aria-colcount={COLS + 1} ref={grid} onKeyDown={onKey}>
          <div className="fg-row fg-headrow" role="row">
            <div className="fg-corner" role="columnheader">{p.by === "none" ? "" : ROWS_BY.find((o) => o.id === p.by)?.label}</div>
            {GRID_STAGES.map((s) => {
              const count = p.rows.reduce((n, r) => n + r.cells[s.id].length, 0);
              const sub = s.id === "your-check" ? check.text : stageWho(s.id, p.rows.flatMap((r) => r.cells[s.id]), roles);
              const warn = s.id === "your-check" && check.full;
              return (
                <div key={s.id} role="columnheader" className={`fg-colhead col-${s.id}${warn ? " fg-full" : ""}`} data-col={s.id} title={`${s.label}: ${sub}`}>
                  <b>{s.label}</b> <span className="fg-n">{fmt.num(count)}</span>
                  <small dir="auto">{sub}</small>
                </div>
              );
            })}
            <div role="columnheader" className="fg-colhead col-done" data-col="done" aria-label={t("grid.doneTodayLabel", { n: doneToday })}>
              <b>{t("common:stage.done")}</b> <span className="fg-n">{t("grid.todayCount", { n: doneToday })}</span>
            </div>
          </div>
          {p.rows.map((row, r) => (
            <div key={row.id} className="fg-row" role="row">
              <RowHead r={r} row={row} multi={p.multi} active={!!row.head && p.openKey === key(row.head)} onOpen={p.onOpen} />
              {GRID_STAGES.map((s, c) => {
                const id = `${row.id}:${s.id}`;
                const open = expanded.has(id);
                const { shown, more } = cellShown(row.cells[s.id], open);
                return (
                  <div
                    key={s.id}
                    role="gridcell"
                    className="fg-cell"
                    data-cell={`${r}:${c}`}
                    tabIndex={at.r === r && at.c === c ? 0 : -1}
                    onFocus={(e) => {
                      if (e.target === e.currentTarget) setActive({ row: row.id, c });
                    }}
                  >
                    {shown.map((item) => (
                      <GridCard key={itemKey(item)} item={item} {...p} dim={dimmed(item, p.mine)} roles={roles} />
                    ))}
                    {/* One button, "+N more" or "Show fewer", so the focus stays on it either way. */}
                    {row.cells[s.id].length > CELL_CAP && (
                      <button
                        type="button"
                        className="fg-more"
                        tabIndex={-1}
                        aria-expanded={open}
                        aria-label={open ? t("grid.fewerIn", { row: row.title, stage: s.label }) : t("grid.moreIn", { count: more, row: row.title, stage: s.label })}
                        onClick={() => toggle(id)}
                      >
                        {open ? t("grid.fewer") : t("cue.more", { count: more })}
                      </button>
                    )}
                  </div>
                );
              })}
              <div
                role="gridcell"
                className="fg-cell fg-donecell"
                data-cell={`${r}:${COLS - 1}`}
                tabIndex={at.r === r && at.c === COLS - 1 ? 0 : -1}
                aria-label={doneTodayWords(row.doneToday)}
                onFocus={(e) => {
                  if (e.target === e.currentTarget) setActive({ row: row.id, c: COLS - 1 });
                }}
              >
                {row.doneToday > 0 ? row.doneToday : ""}
              </div>
            </div>
          ))}
        </div>
        {p.rows.length === 0 && <p className="none">{t("groups.noMatch")}</p>}
      </div>
    </section>
  );
}

const itemKey = (i: GridItem) => (i.kind === "task" ? key(i.task) : `note:${i.note.queue}/${i.note.file}`);

const sameIds = (a: string[] | null, b: string[] | null) => a === b || (!!a && !!b && a.length === b.length && a.every((id, i) => id === b[i]));

/** Watches the grid's frame and calls `onChange` with the columns wholly in view beside the feature
 *  names, or null when the whole grid fits: on resize (the frame, the grid, the header row) and on
 *  scroll, each burst measured once on the next frame. It keeps the header row's height in
 *  --fg-head-h too, the frame's scroll-padding-top. Returns the function that stops it. */
function watchGrid(frame: HTMLElement, grid: HTMLElement, onChange: (visible: string[] | null) => void): () => void {
  let raf = 0;
  const head = grid.querySelector<HTMLElement>(".fg-headrow");
  const measure = () => {
    raf = 0;
    if (head) frame.style.setProperty("--fg-head-h", `${head.getBoundingClientRect().height}px`);
    if (grid.scrollWidth <= frame.clientWidth + 1) return onChange(null);
    const left = frame.getBoundingClientRect().left + frame.clientLeft;
    const corner = grid.querySelector(".fg-corner")?.getBoundingClientRect() ?? { left, right: left };
    const { start, end } = viewSpan({ left, right: left + frame.clientWidth }, corner);
    const spans: ChipSpan[] = Array.from(grid.querySelectorAll<HTMLElement>("[data-col]"), (h) => {
      const r = h.getBoundingClientRect();
      return { id: h.dataset.col ?? "", start: r.left, end: r.right };
    });
    onChange(inView(spans, start, end));
  };
  const schedule = () => {
    if (!raf) raf = requestAnimationFrame(measure);
  };
  const sizes = new ResizeObserver(schedule);
  sizes.observe(frame);
  sizes.observe(grid);
  if (head) sizes.observe(head);
  frame.addEventListener("scroll", schedule, { passive: true });
  schedule();
  return () => {
    frame.removeEventListener("scroll", schedule);
    sizes.disconnect();
    if (raf) cancelAnimationFrame(raf);
    raf = 0;
  };
}

/** A row's head: the feature or goal (a click opens it beside the board), its progress bar and
 *  "3 of 11 done"; Other and All tasks are words. */
function RowHead({ r, row, multi, active, onOpen }: { r: number; row: GridRow; multi: boolean; active: boolean; onOpen: (t: Task) => void }) {
  const t = useT("board");
  const h = row.head;
  const pct = row.progress.total ? Math.round((row.progress.done / row.progress.total) * 100) : 0;
  return (
    <div className="fg-rowhead" role="rowheader" data-rowhead={r}>
      {/* The title in an isolate of its own (review R3): in All projects the project's name comes
          first, and the title's direction is the title's, not the name's. */}
      {h ? (
        <button type="button" className={`fg-rowtitle link${active ? " fg-active" : ""}`} tabIndex={-1} onClick={() => onOpen(h)} title={t("grid.open", { key: taskKey(h), title: row.title })}>
          {multi && (
            <>
              <ProjectName name={h.queue} />{" "}
            </>
          )}
          <bdi dir="auto">{row.title}</bdi>
        </button>
      ) : (
        <span className="fg-rowtitle">{row.title}</span>
      )}
      <span className="fg-bar" aria-hidden="true">
        <i style={{ width: `${pct}%` }} />
      </span>
      <small>{progressText(row.progress)}</small>
    </div>
  );
}

type CardProps = FeatureGridProps & { item: GridItem; dim: boolean; roles: RoleNames };

/** A small card: the title, any trouble on its own line under it, then one status and the key. A
 *  card waiting on you has the needs-you edge; the one open in the pane is selected. */
function GridCard({ item, dim, roles, ...p }: CardProps) {
  const tr = useT("board");
  if (item.kind === "note") {
    const n = item.note;
    const on = p.openNoteKey === `${n.queue}/${n.file}`;
    return (
      <button type="button" className={`fg-card fg-note${on ? " fg-active" : ""}${dim ? " fg-dim" : ""}`} tabIndex={-1} onClick={() => p.onOpenNote(n)}>
        <span className="fg-t" dir="auto" title={noteTitle(n)}><Hl text={noteTitle(n)} words={p.words} /></span>
        <span className="fg-meta">
          <Badge variant="note">{tr("grid.note")}</Badge>
          {p.multi && <ProjectName name={n.queue} />}
        </span>
      </button>
    );
  }
  const t = item.task;
  const status = cardStatus(t, roles);
  const trouble = troubleOf(t, p.now);
  // The needs-you edge on every card that waits on you, a block on you included.
  const cls = ["fg-card", needsYou(t) && "fg-you", p.openKey === key(t) && "fg-active", dim && "fg-dim"].filter(Boolean).join(" ");
  return (
    <button type="button" className={cls} tabIndex={-1} onClick={() => p.onOpen(t)}>
      <span className="fg-t" dir="auto" title={t.title || undefined}>{t.title ? <Hl text={t.title} words={p.words} /> : tr("groups.noTitle")}</span>
      {trouble && <span className="fg-trouble" title={trouble}>{trouble}</span>}
      {/* Why it waits can name several tasks: its own line, so the key keeps its place. */}
      {status.kind === "blocked" && <span className="fg-wait"><BlockWhy t={t} short /></span>}
      <span className="fg-meta">
        {status.kind !== "blocked" && <StatusLine status={status} />}
        {p.multi && <ProjectName name={t.queue} />}
        <span className="fg-key">{taskKey(t)}</span>
      </span>
    </button>
  );
}

function StatusLine({ status }: { status: ReturnType<typeof cardStatus> }) {
  const t = useT("board");
  switch (status.kind) {
    case "you":
      return <Badge variant="warn">{status.text}</Badge>;
    case "paused":
      return <Badge variant="paused">{PAUSE_GLYPH} {status.text}</Badge>;
    case "who":
      return (
        <span className="fg-who" title={status.live ? t("grid.workingNow", { who: status.text }) : status.text}>
          <i className={status.live ? "dot fg-dot" : "fg-idle"} aria-hidden="true" />
          {status.text}
        </span>
      );
    default:
      return null;
  }
}
