import { useContext, useState } from "react";
import { api, type Paused, type Returned } from "./api";
import { isStage, nextStage, oneAtATime, readFiles, stageLabel, stamp } from "./lib";
import { isHuman } from "./human";
import { sentences, tn } from "./i18n";
import { releaseWords, trackMove, unreachedWords } from "./paneData";
import { RoleNamesContext } from "./settingsData";
import type { Task } from "./types";

export type StageForm = "gate" | "back" | "block";
/** A move on the track that can be undone: from `to` back to `back`. */
export type Undo = { back: string; to: string };
/** The pane's last change: what came of it, and for a move on the track, its undo. */
export type Msg = { ok: boolean; text: string; undo?: Undo };

/** The task pane's changes, shared by the track, the sidebar, the stage forms and the note bar:
 *  every one through the core's rules, its refusal shown as is. Stage actions run one at a time
 *  and their controls are off (`busy`) while one is in flight. */
export function useTaskActions(task: Task, onChanged: () => void) {
  const [msg, setMsg] = useState<Msg | null>(null);
  // The roles' names now (0112), for the claim a track move releases.
  const names = useContext(RoleNamesContext);
  // One stage form open at a time: the gate line (Building → Testing), send back, or block. A
  // form belongs to the stage it was opened in: once the task moves, here or elsewhere, it's gone.
  const [opened, setOpened] = useState<{ kind: StageForm; stage: string } | null>(null);
  const form = opened?.stage === task.stage ? opened.kind : null;
  const [once] = useState(oneAtATime);
  const [busy, setBusy] = useState(false);

  /** Runs one change; the core's refusal is shown as is. True when it went through. `done` may
   *  read what the core returned. */
  const run = async <T,>(what: () => Promise<T>, done: string | ((r: T) => string), undo?: Undo) => {
    try {
      const r = await what();
      setMsg({ ok: true, text: typeof done === "string" ? done : done(r), undo });
      onChanged();
      return true;
    } catch (e) {
      setMsg({ ok: false, text: String(e) });
      return false;
    }
  };
  const stageRun = <T,>(what: () => Promise<T>, done: string | ((r: T) => string), undo?: Undo) =>
    once(async () => {
      setBusy(true);
      try {
        return await run(what, done, undo);
      } finally {
        setBusy(false);
      }
    });
  const now = () => stamp(new Date());
  // A stage form closes only once the core took the change; its text goes with it.
  const submit = <T,>(what: () => Promise<T>, done: string | ((r: T) => string)) =>
    void stageRun(what, done).then((ok) => {
      if (ok) setOpened(null);
    });
  // Every pass says the stage it was clicked in, so the core refuses it if the task moved on.
  const passFrom = (gateLine: string, done: string, summary = "") => submit(() => api.passTask(task.queue, task.id, { from: task.stage, gate: gateLine, commit: "", summary }, now()), done);

  return {
    msg,
    busy,
    form,
    open: (kind: StageForm) => setOpened({ kind, stage: task.stage }),
    close: () => setOpened(null),
    run,
    passFrom,
    /** The next step: out of Building it asks for the gate line first. The owner's own task
     *  (0022) needs none: Start puts it in Doing, Done finishes it. */
    pass: () => {
      const human = isHuman(task);
      if (task.stage === "building" && !human) {
        setOpened({ kind: "gate", stage: task.stage });
        return;
      }
      const next = nextStage(task.stage, human);
      const done = human && next?.id === "building"
        ? tn("pane", "pane.did.started")
        : human && next?.id === "done"
          ? tn("pane", "pane.did.done")
          : task.stage === "your-check" ? tn("pane", "pane.did.accepted") : next ? tn("pane", "pane.did.passed", { stage: next.label }) : tn("pane", "pane.did.passedNext");
      passFrom("", done);
    },
    sendBack: (why: string, to: string) => submit(() => api.returnTask(task.queue, task.id, why, to, now()), sentBackMsg),
    block: (kind: string, detail: string) =>
      submit(() => api.blockTask(task.queue, task.id, kind, detail, now()), tn("pane", kind === "tasks" ? "pane.did.blockedTasks" : "pane.did.blocked")),
    unblock: () => void stageRun(() => api.unblockTask(task.queue, task.id, now()), tn("pane", "pane.did.unblocked")),
    release: () => void stageRun(() => api.reclaimTask(task.queue, task.id, now()), releaseWords(task.stage).done),
    /** Release a claim no session can be reached on (mu-184): true once the core took it. */
    releaseUnreached: () => stageRun(() => api.releaseUnreached(task.queue, task.id, task.stage, now()), unreachedWords(task.stage).done),
    /** Pause (0045), with an optional why: true once the core took it. */
    pause: (why: string) => stageRun(() => api.pauseTask(task.queue, task.id, why, now()), (p) => pausedMsg(p, true)),
    resume: () => void stageRun(() => api.resumeTask(task.queue, task.id, now()), (p) => pausedMsg(p, false)),
    /** The owner's override from the track: no confirmation, an Undo in the message instead
     *  (see trackMove). It says the stage it was clicked in, so a stale window is refused. */
    move: (to: string) => {
      const { done, undo } = trackMove(task, to, names);
      void stageRun(() => api.setStage(task.queue, task.id, to, isStage(task.stage) ? task.stage : null, now()), done, undo);
    },
    /** Undo a move: back where it was, refused if it has moved on from where the move put it. */
    undo: (u: Undo) => void stageRun(() => api.setStage(task.queue, task.id, u.back, u.to, now()), tn("pane", "pane.did.putBack", { stage: stageLabel(u.back) })),
    tick: (which: string, undo: boolean) => void stageRun(() => api.tickCriterion(task.queue, task.id, which, undo, now()), tn("pane", undo ? "pane.did.unticked" : "pane.did.ticked", { which })),
    setField: (field: string, value: string, done: string) => void run(() => api.setField(task.queue, task.id, field, value, now()), done),
    note: (text: string) => void run(() => api.addNote(task.queue, task.id, text, now()), tn("pane", "pane.did.noted")),
    attachPick: () => void run(() => api.attachPick(task.queue, task.id, now()), tn("pane", "pane.did.attached")),
    /** Files pasted or dropped; reading one that fails shows like a refusal. */
    attachFiles: (list: FileList | File[]) => {
      if (!list.length) return;
      void run(
        async () => api.attachData(task.queue, task.id, await readFiles(list, now()), now()),
        tn("pane", "pane.did.attachedN", { count: list.length }),
      );
    },
  };
}

export type TaskActions = ReturnType<typeof useTaskActions>;

/** What the pane says after Send back, from what the core did: a QA send-back from Your check
 *  (mu-101) recorded the reason as an owner finding, first in line; any other return didn't. And
 *  whether the builder's session was told, when it was meant to be. */
export function sentBackMsg(r: Returned): string {
  const head = r.finding ? tn("pane", "pane.sentBack.finding", { finding: r.finding }) : tn("pane", "pane.sentBack.to", { stage: stageLabel(r.to) });
  if (r.notTold) return sentences([head, tn("pane", "pane.sentBack.notTold", { why: r.notTold })]);
  return r.told ? sentences([head, tn("pane", "pane.sentBack.told", { session: r.told })]) : head;
}

/** What the pane says after a pause or resume: what it means, and whether its session was told. */
export function pausedMsg(p: Paused, paused: boolean): string {
  const head = tn("pane", paused ? "pane.paused.head" : "pane.paused.resumed");
  if (p.session) return sentences([head, tn("pane", paused ? "pane.paused.askedStop" : "pane.paused.askedOn", { session: p.session })]);
  return p.unreached ? sentences([head, tn("pane", "pane.paused.unreached", { why: p.unreached })]) : head;
}
