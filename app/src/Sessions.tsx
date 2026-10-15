import { fmt } from "./i18n";
import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from "react";
import { api } from "./api";
import { HelpButton } from "./HelpButton";
import { useT } from "./i18n";
import { labelOf, Labels } from "./labels";
import { useLook } from "./modes";
import ProjectName from "./ProjectName";
import { groupCount, groupSessions, headerCount, nextKey, projectOf, rowAge, rowLabel, SESSION_GROUPS, sessionKey, sessionName, WORDS, type GroupRows } from "./sessionsData";
import type { SessionGroup, SessionItem, SessionsList } from "./types";
import { Badge, Pill } from "./ui";

/** How often the open view asks the core. The core reads `claude agents` at most every 15 s and
 *  hands back its latest list in between, so this only keeps the ages and a landed read fresh. */
const POLL_MS = 3000;

/** The list for one project (`project`, its name) or all (null), asked for every few seconds
 *  while the view is open and the window visible, and never otherwise. An answer for a project the
 *  view no longer shows is dropped. */
function useSessionsList(project: string | null): { list: SessionsList | null; error: string } {
  const [got, setGot] = useState<{ project: string | null; list: SessionsList } | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    let live = true;
    let timer = 0;
    const tick = () => {
      window.clearTimeout(timer);
      if (document.hidden) return;
      api.sessionsList(project).then(
        (list) => {
          if (!live) return;
          setGot({ project, list });
          setError("");
        },
        (e: unknown) => live && setError(String(e)),
      );
      timer = window.setTimeout(tick, POLL_MS);
    };
    tick();
    document.addEventListener("visibilitychange", tick);
    return () => {
      live = false;
      window.clearTimeout(timer);
      document.removeEventListener("visibilitychange", tick);
    };
  }, [project]);
  return { list: got && got.project === project ? got.list : null, error };
}

/** The Sessions view (0098), live: `selected` is the row whose recap the pane shows. */
export default function Sessions({ project, now, selected, onOpen }: { project: string | null; now: number; selected: string | null; onOpen: (r: SessionItem) => void }) {
  const { list, error } = useSessionsList(project);
  return <SessionsView list={list} error={error} now={now} all={project === null} selected={selected} onOpen={onOpen} />;
}

/** The rows on screen in `root`, in order, as their keys. */
const rowsIn = (root: HTMLElement) => Array.from(root.querySelectorAll<HTMLElement>("[data-session]"));

/** The view as drawn from what the core said: the header count, then Needs input, Working and
 *  Completed, always in that order and always on screen, an empty one saying "none". */
export function SessionsView({ list, error, now, all, selected, onOpen }: {
  list: SessionsList | null;
  error: string;
  now: number;
  /** Under All: each row says its project. */
  all: boolean;
  selected: string | null;
  onOpen: (r: SessionItem) => void;
}) {
  // Its words (sessionsData's WORDS) are the board's: loaded, and drawn again on a language switch.
  useT("board");
  const how = useLook("session-groups");
  const root = useRef<HTMLElement>(null);
  const grouped = groupSessions(list?.rows ?? [], now);
  const reading = !list || (list.reading && list.rows.length === 0);

  // Escape closes the recap (the window's key): focus goes back to the row it was opened from.
  const was = useRef(selected);
  useEffect(() => {
    const before = was.current;
    was.current = selected;
    if (before === null || selected !== null || !root.current) return;
    rowsIn(root.current).find((el) => el.dataset.session === before)?.focus();
  }, [selected]);

  // The arrows, Home and End move between the rows on screen; Enter opens one (it's a button).
  const onKeyDown = useCallback((e: KeyboardEvent<HTMLElement>) => {
    const el = e.target as HTMLElement;
    if (!root.current || el.dataset.session === undefined) return;
    const rows = rowsIn(root.current);
    const to = nextKey(rows.map((r) => r.dataset.session ?? ""), el.dataset.session, e.key);
    if (to === null) return;
    e.preventDefault();
    rows.find((r) => r.dataset.session === to)?.focus();
  }, []);

  return (
    <main className="list sessions" ref={root} onKeyDown={onKeyDown}>
      <div className="sessions-head">
        <h1>{WORDS.view}</h1>
        <span className="sessions-count" dir="auto" aria-live="polite">{reading ? WORDS.reading : headerCount(grouped)}</span>
        <HelpButton topic="sessions" />
      </div>
      {list && <p className="meta small">{list.accounts.length ? WORDS.accounts(list.accounts) : WORDS.noAccounts}</p>}
      {error && <p className="err small" role="alert">{error}</p>}
      {list?.problems.map((p) => <p key={p} className="err small">{p}</p>)}
      {SESSION_GROUPS.map((g) => (
        <SessionGroupList
          key={g.id}
          id={g.id}
          label={g.label}
          rows={grouped[g.id]}
          chip={how === "needs-input" && g.id !== "needs-input"}
          reading={reading}
          now={now}
          all={all}
          selected={selected}
          onOpen={onOpen}
        />
      ))}
    </main>
  );
}

/** One group: its heading with its count, and its rows, or "none". Completed's older rows are one
 *  chip that opens them in place. `chip`: Focus shows the whole group as a chip with its count,
 *  which opens it in place too. */
function SessionGroupList({ id, label, rows, chip, reading, now, all, selected, onOpen }: {
  id: SessionGroup;
  label: string;
  rows: GroupRows;
  chip: boolean;
  reading: boolean;
  now: number;
  all: boolean;
  selected: string | null;
  onOpen: (r: SessionItem) => void;
}) {
  const [opened, setOpened] = useState(false);
  const [olderOpen, setOlderOpen] = useState(false);
  const count = groupCount(rows);
  const head = `sessions-${id}`;
  const folded = chip && !opened;
  const row = (r: SessionItem) => <SessionRow key={sessionKey(r)} r={r} now={now} all={all} on={selected === sessionKey(r)} onOpen={onOpen} />;
  return (
    <section className={`session-group sg-${id}`} aria-labelledby={head}>
      <h2 id={head}>
        {label} {!reading && (id === "needs-input" && count > 0 ? <Badge variant="warn">{fmt.num(count)}</Badge> : <small>{fmt.num(count)}</small>)}
        {chip && opened && (
          <Pill type="button" className="session-fold" aria-expanded={true} onClick={() => setOpened(false)}>{WORDS.fewer}</Pill>
        )}
      </h2>
      {folded ? (
        <Pill type="button" className="session-chip" aria-expanded={false} title={WORDS.open(label, reading ? null : count)} onClick={() => setOpened(true)}>
          {label} {!reading && <small>{fmt.num(count)}</small>}
        </Pill>
      ) : count === 0 ? (
        !reading && <p className="none">{WORDS.none}</p>
      ) : (
        <>
          {rows.rows.length > 0 && <ul className="session-rows" aria-labelledby={head}>{rows.rows.map(row)}</ul>}
          {rows.older.length > 0 &&
            (olderOpen ? (
              <>
                <ul className="session-rows older" aria-label={WORDS.older(rows.older.length)}>{rows.older.map(row)}</ul>
                <Pill type="button" className="session-chip" aria-expanded={true} title={WORDS.fewerTitle} onClick={() => setOlderOpen(false)}>{WORDS.fewer}</Pill>
              </>
            ) : (
              <Pill type="button" className="session-chip" aria-expanded={false} title={WORDS.olderTitle} onClick={() => setOlderOpen(true)}>{WORDS.older(rows.older.length)}</Pill>
            ))}
        </>
      )}
    </section>
  );
}

/** One session on one line: its name, under All its project, its last message or what it waits
 *  for, and its age at the inline end. Names and messages are the owner's own text, plain and
 *  direction-free (`dir="auto"`). */
export function SessionRow({ r, now, all, on, onOpen }: { r: SessionItem; now: number; all: boolean; on: boolean; onOpen: (r: SessionItem) => void }) {
  return (
    <li>
      <Labels.Consumer>
        {(labels) => (
          // Under All the name says the project too, as the row shows it: its label, or Other.
          <button type="button" className={on ? `session-row ${r.group} on` : `session-row ${r.group}`} data-session={sessionKey(r)} aria-label={rowLabel(r, now, all ? (r.project ? labelOf(labels, r.project) : projectOf(r)) : "")} aria-pressed={on} onClick={() => onOpen(r)}>
            <span className="session-name" dir="auto">{sessionName(r)}</span>
            {all && (r.project ? <ProjectName name={r.project} /> : <span className="q">{projectOf(r)}</span>)}
            <span className="session-msg" dir="auto">{r.message}</span>
            <span className="session-age" dir="auto">{rowAge(now, r.at)}</span>
          </button>
        )}
      </Labels.Consumer>
    </li>
  );
}
