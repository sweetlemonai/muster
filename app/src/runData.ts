// Run from the app (0079): what the window holds of a project's run, put together from the core's
// `run_state` and its `run` events, and how the Run block words it.
import type { RunSnap } from "./types";

/** The most lines the window keeps of a run, as the core's ring does. */
export const KEEP_LINES = 2000;
/** The most characters the window keeps of a run's lines, as the core's ring keeps 256 KB. */
export const KEEP_CHARS = 256 * 1024;
/** How many of the last lines the Run block shows until All output is open. */
export const SHOWN_LINES = 12;

/** What the window holds after `push` (a `run_state` snap or a `run` event) came in on `held`.
 *  An older run's late event changes nothing; a newer run replaces the one held. Of the same run,
 *  the lines it doesn't hold yet are added after its own (when the push starts past them, the
 *  ring rolled on: it starts again from the push), a push from before them (`run_state` answered
 *  after an event) puts its earlier lines in front, and once it has ended, a push that still says
 *  running (a `run_state` read before the end, answered after) doesn't bring it back. */
export function merged(held: RunSnap | null, push: RunSnap): RunSnap {
  if (held && push.id < held.id) return held;
  if (!held || push.id > held.id) return bounded({ ...push, lines: [...push.lines] });
  if (push.first < held.first) {
    // A run_state answered after an event: it holds the lines from before the event's too.
    const end = push.first + push.lines.length;
    if (end < held.first) return held;
    return bounded({ ...held, first: push.first, lines: [...push.lines, ...held.lines.slice(end - held.first)] });
  }
  const next = held.first + held.lines.length;
  const gap = push.first > next;
  const lines = gap ? [...push.lines] : [...held.lines, ...push.lines.slice(next - push.first)];
  const ended = held.status.kind !== "running" && push.status.kind === "running";
  return bounded({
    ...push,
    status: ended ? held.status : push.status,
    ended: ended ? held.ended : push.ended,
    first: gap ? push.first : held.first,
    lines,
  });
}

/** The most `run` events the window keeps while it doesn't know the project's key yet. */
export const WAITING_EVENTS = 50;

/** Which `run` events a project's Run takes: its own, matched on the key `run_state` gave
 *  (`key`), never on the name, which may have passed to another project since the run started.
 *  Until the key is known, events wait in `waiting` (the last `WAITING_EVENTS`); `run_state`'s
 *  answer then takes the ones of its key (`keyed`). */
export type RunInbox = { key: string | null; waiting: RunSnap[] };

/** `inbox` after the event `push` came in, and the event to merge, if it's this project's. */
export function received(inbox: RunInbox, push: RunSnap): { inbox: RunInbox; take: RunSnap | null } {
  if (inbox.key !== null) return { inbox, take: push.key === inbox.key ? push : null };
  return { inbox: { key: null, waiting: [...inbox.waiting, push].slice(-WAITING_EVENTS) }, take: null };
}

/** `inbox` once `run_state` said the project's key, and the waiting events that are its own. */
export function keyed(inbox: RunInbox, key: string): { inbox: RunInbox; take: RunSnap[] } {
  return { inbox: { key, waiting: [] }, take: inbox.waiting.filter((e) => e.key === key) };
}

/** `s` with its oldest lines dropped past `KEEP_LINES` or `KEEP_CHARS`. */
function bounded(s: RunSnap): RunSnap {
  let chars = s.lines.reduce((n, l) => n + l.length + 1, 0);
  let drop = 0;
  while (drop < s.lines.length && (s.lines.length - drop > KEEP_LINES || chars > KEEP_CHARS)) {
    chars -= s.lines[drop].length + 1;
    drop++;
  }
  return drop ? { ...s, first: s.first + drop, lines: s.lines.slice(drop) } : s;
}

/** A run's time: "12s", "1m 20s", "1h 5m". */
export function fmtRunTime(secs: number): string {
  const s = Math.max(0, Math.floor(secs));
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.floor(s / 60)}m ${s % 60}s`;
  return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
}

/** The checkout as the Run block names it: its last two folders ("muster/main"). */
export function shortFolder(path: string): string {
  return path.split("/").filter(Boolean).slice(-2).join("/");
}

/** Where `run` stands, in words that change only when its state does (what a screen reader is
 *  told): "Running bin/run in muster/main", "Exited 0 after 2m 10s", "Ended by a signal after
 *  4s", "Stopped after 1m 2s". The running time, which ticks, isn't in it. */
export function statusWords(run: RunSnap): string {
  const took = run.ended === null ? "" : ` after ${fmtRunTime(run.ended - run.since)}`;
  switch (run.status.kind) {
    case "running":
      return `Running ${run.command} in ${shortFolder(run.folder)}`;
    case "exited":
      return run.status.code === null ? `Ended by a signal${took}` : `Exited ${run.status.code}${took}`;
    case "stopped":
      return `Stopped${took}`;
  }
}

/** The lines the Run block shows: the last `SHOWN_LINES`, or every one held with `all`; each with
 *  where it is among the run's lines, which keys it. */
export function shownLines(run: RunSnap, all: boolean): { n: number; text: string }[] {
  const from = all ? 0 : Math.max(0, run.lines.length - SHOWN_LINES);
  return run.lines.slice(from).map((text, i) => ({ n: run.first + from + i, text }));
}
