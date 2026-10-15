import { Fragment, type ReactNode, useContext, useEffect, useRef, useState, useSyncExternalStore } from "react";
import { api } from "./api";
import { fmt, useT } from "./i18n";
import { key, oneAtATime, stamp } from "./lib";
import { pressed } from "./shortcuts";
import { taskKey } from "./taskKey";
import type { Task } from "./types";
import { Button } from "./ui";
import { type AcceptOutcome, acceptAllRun, byEffort, EFFORTS, justAccept, staleResult } from "./yourCheck";
import { labelOf, Labels } from "./labels";

// Accept all's result lines, by place ("board", a band, "waiting"), outside any component: the
// column it was clicked in can empty and unmount, and the line must still say what came of it.
let results: ReadonlyMap<string, AcceptOutcome> = new Map();
const listeners = new Set<() => void>();

/** Sets (or, with null, clears) a place's result line. */
export function putAcceptResult(place: string, r: AcceptOutcome | null): void {
  const next = new Map(results);
  if (r) next.set(place, r);
  else next.delete(place);
  results = next;
  for (const l of listeners) l();
}
const subscribe = (l: () => void) => {
  listeners.add(l);
  return () => {
    listeners.delete(l);
  };
};
const snapshot = () => results;

/** Every place's Accept all result line; a place keeps its Your check column open while it has one. */
export const useAcceptResults = (): ReadonlyMap<string, AcceptOutcome> => useSyncExternalStore(subscribe, snapshot, snapshot);

/** Tasks in Your check by what they ask of the owner (0043): Just accept, One look, Needs a
 *  session, always in that order and each with its count, so nothing moves when one empties.
 *  Just accept has Accept all, and `place` keeps its result line. `renderCard` draws each task as
 *  its place does (a board card, a Waiting for you item); each group keeps the order it's given.
 *  With no tasks left, only the place's result line shows, until the next change. */
export default function EffortGroups({ tasks, place, renderCard, onChanged }: { tasks: Task[]; place: string; renderCard: (t: Task) => ReactNode; onChanged: () => void }) {
  const t = useT("board");
  const result = useAcceptResults().get(place);
  useEffect(() => {
    if (result && staleResult(result, tasks)) putAcceptResult(place, null);
  }, [result, tasks, place]);
  const groups = byEffort(tasks);
  const accept = <AcceptAll tasks={tasks} ready={justAccept(groups.accept)} place={place} result={result ?? null} onChanged={onChanged} />;
  if (tasks.length === 0) return result ? accept : null;
  return (
    <>
      {EFFORTS.map((e) => {
        const list = groups[e.id];
        // The group's name in the language shown, by its id.
        const label = t(`effort.${e.id}`);
        return (
          <div key={e.id} className={`effort-group effort-${e.id}${list.length ? "" : " effort-empty"}`} role="group" aria-label={label}>
            <h3 className="effort-head">{label} <small>{fmt.num(list.length)}</small></h3>
            {e.id === "accept" && accept}
            {list.map((x) => <Fragment key={key(x)}>{renderCard(x)}</Fragment>)}
          </div>
        );
      })}
    </>
  );
}

/** Esc while Accept all's confirm is open, from a capture listener on the window: it's the
 *  confirm's, so it never reaches the window's own Esc (which closes the pane). It cancels,
 *  except while the accepting runs. */
export function confirmEscape(e: { key: string; metaKey: boolean; ctrlKey: boolean; stopPropagation: () => void; preventDefault: () => void }, busy: boolean, cancel: () => void): void {
  if (!pressed(e, "accept-all-cancel")) return;
  e.stopPropagation();
  e.preventDefault();
  if (!busy) cancel();
}

/** Accept all, with its confirm in place: the first click freezes the tasks shown (`ready`) and
 *  names them, with the focus on Cancel; the second accepts exactly those (no dialog,
 *  docs/DESIGN.md). The core logs each. Cancel, Esc or the end puts the focus back on the button. */
function AcceptAll({ tasks, ready, place, result, onChanged }: { tasks: Task[]; ready: Task[]; place: string; result: AcceptOutcome | null; onChanged: () => void }) {
  const [asked, setAsked] = useState<Task[] | null>(null);
  const [busy, setBusy] = useState(false);
  // A double click on "Yes" runs it once (the second would only report each as moved on).
  const [once] = useState(oneAtATime);
  const shown = useRef(tasks);
  shown.current = tasks;
  const button = useRef<HTMLButtonElement>(null);
  const cancelButton = useRef<HTMLButtonElement>(null);
  const backToButton = useRef(false);
  const open = asked !== null;

  useEffect(() => {
    if (open) cancelButton.current?.focus();
    else if (backToButton.current) {
      backToButton.current = false;
      button.current?.focus();
    }
  }, [open]);

  const close = () => {
    backToButton.current = true;
    setAsked(null);
  };
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => confirmEscape(e, busy, close);
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [open, busy]);

  const confirm = (list: Task[]) =>
    once(async () => {
      setBusy(true);
      const at = stamp(new Date());
      try {
        const r = await acceptAllRun(list, (queue, ids) => api.acceptAll(queue, ids, at));
        // Every task the place showed, at the click and now: only one new since is the next change.
        const known = [...new Set([...list, ...shown.current].map(key))];
        putAcceptResult(place, { ...r, known, at: Math.floor(Date.now() / 1000) });
      } finally {
        setBusy(false);
        close();
        onChanged();
      }
      return true;
    });
  const several = new Set((asked ?? ready).map((t) => t.queue)).size > 1;
  const labels = useContext(Labels);
  return (
    <AcceptAllView
      count={ready.length}
      asked={asked && asked.map((t) => (several ? `${labelOf(labels, t.queue)} ${taskKey(t)}` : taskKey(t)))}
      busy={busy}
      msg={result}
      onAsk={() => {
        putAcceptResult(place, null);
        setAsked(ready);
      }}
      onConfirm={() => {
        if (asked) void confirm(asked);
      }}
      onCancel={close}
      buttonRef={button}
      cancelRef={cancelButton}
    />
  );
}

/** Accept all as drawn: the button with its count; once asked, "Accept 2: 0014, 0021?" with
 *  the second click and Cancel in the same place; then what came of it. */
export function AcceptAllView({ count, asked, busy, msg, onAsk, onConfirm, onCancel, buttonRef, cancelRef }: {
  count: number;
  /** The tasks the first click froze, as shown; null before it. */
  asked: string[] | null;
  busy: boolean;
  msg: { ok: boolean; text: string } | null;
  onAsk: () => void;
  onConfirm: () => void;
  onCancel: () => void;
  buttonRef?: React.Ref<HTMLButtonElement>;
  cancelRef?: React.Ref<HTMLButtonElement>;
}) {
  const t = useT("board");
  if (count === 0 && !asked && !msg) return null;
  return (
    <span className="accept-all-bar">
      {asked ? (
        <span className="accept-confirm" role="group" aria-label={t("effort.confirmLabel")}>
          {t("effort.confirm", { count: asked.length, list: asked.join(t("common:listSep")) })}{" "}
          <Button type="button" variant="primary" disabled={busy} onClick={onConfirm}>{busy ? t("effort.accepting") : t("effort.yes", { count: asked.length })}</Button>
          <Button type="button" disabled={busy} onClick={onCancel} ref={cancelRef}>{t("common:cancel")}</Button>
        </span>
      ) : (
        count > 0 && <Button type="button" className="accept-all" onClick={onAsk} ref={buttonRef}>{t("effort.all", { count })}</Button>
      )}
      {msg && <span className={msg.ok ? "ok small" : "err small"} role="status">{msg.text}</span>}
    </span>
  );
}
