import { useState } from "react";
import { api, type Paused } from "./api";
import { isStage, nextStage, oneAtATime, readFiles, stageLabel, stamp } from "./lib";
import { isHuman } from "./human";
import { trackMove } from "./paneData";
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
  const submit = (what: () => Promise<unknown>, done: string) =>
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
        ? "Started: it's in Doing."
        : human && next?.id === "done"
          ? "Done."
          : task.stage === "your-check" ? "Accepted: it's done." : `Passed to ${next?.label ?? "the next stage"}.`;
      passFrom("", done);
    },
    sendBack: (why: string, to: string) => submit(() => api.returnTask(task.queue, task.id, why, to, now()), `Sent back to ${stageLabel(to)}.`),
    block: (kind: string, detail: string) =>
      submit(() => api.blockTask(task.queue, task.id, kind, detail, now()), kind === "tasks" ? "Blocked by those tasks: added to its depends." : "Blocked. It keeps its stage."),
    unblock: () => void stageRun(() => api.unblockTask(task.queue, task.id, now()), "Unblocked."),
    release: () => void stageRun(() => api.reclaimTask(task.queue, task.id, now()), "Released: back in Ready."),
    /** Pause (0045), with an optional why: true once the core took it. */
    pause: (why: string) => stageRun(() => api.pauseTask(task.queue, task.id, why, now()), (p) => pausedMsg(p, true)),
    resume: () => void stageRun(() => api.resumeTask(task.queue, task.id, now()), (p) => pausedMsg(p, false)),
    /** The owner's override from the track: no confirmation, an Undo in the message instead
     *  (see trackMove). It says the stage it was clicked in, so a stale window is refused. */
    move: (to: string) => {
      const { done, undo } = trackMove(task, to);
      void stageRun(() => api.setStage(task.queue, task.id, to, isStage(task.stage) ? task.stage : null, now()), done, undo);
    },
    /** Undo a move: back where it was, refused if it has moved on from where the move put it. */
    undo: (u: Undo) => void stageRun(() => api.setStage(task.queue, task.id, u.back, u.to, now()), `Put back in ${stageLabel(u.back)}.`),
    tick: (which: string, undo: boolean) => void stageRun(() => api.tickCriterion(task.queue, task.id, which, undo, now()), `${undo ? "Unticked" : "Ticked"} ${which}.`),
    setField: (field: string, value: string, done: string) => void run(() => api.setField(task.queue, task.id, field, value, now()), done),
    note: (text: string) => void run(() => api.addNote(task.queue, task.id, text, now()), "Note added to the log."),
    attachPick: () => void run(() => api.attachPick(task.queue, task.id, now()), "Files attached."),
    /** Files pasted or dropped; reading one that fails shows like a refusal. */
    attachFiles: (list: FileList | File[]) => {
      if (!list.length) return;
      void run(
        async () => api.attachData(task.queue, task.id, await readFiles(list, now()), now()),
        `Attached ${list.length} file${list.length === 1 ? "" : "s"}.`,
      );
    },
  };
}

export type TaskActions = ReturnType<typeof useTaskActions>;

/** What the pane says after a pause or resume: what it means, and whether its session was told. */
export function pausedMsg(p: Paused, paused: boolean): string {
  const head = paused ? "Paused: it's in the Paused lane with its stage and claim, and nobody works it until you resume it." : "Resumed: it's back in its column and carries on as it was.";
  if (p.session) return `${head} Session ${p.session} is asked to ${paused ? "stop at a safe point" : "carry on"} after its next tool call.`;
  return p.unreached ? `${head} Its session can't be told: ${p.unreached}.` : head;
}
