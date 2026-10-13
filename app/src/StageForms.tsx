import { useState } from "react";
import { earlierStages } from "./lib";
import type { Task } from "./types";
import type { TaskActions } from "./useTaskActions";

/** The open stage form, full width under both columns: the gate line out of Building, Send back
 *  with a reason, or Block with what it waits on. Its text stays while the core refuses it and
 *  goes once it's taken (the form closes). */
export default function StageForms({ task, a }: { task: Task; a: TaskActions }) {
  if (a.form === "gate" && task.stage === "building") return <GateForm a={a} />;
  if (a.form === "back" && earlierStages(task.stage).length > 0) return <BackForm task={task} a={a} />;
  if (a.form === "block" && !task.blocked) return <BlockForm a={a} />;
  return null;
}

function GateForm({ a }: { a: TaskActions }) {
  const [gate, setGate] = useState("");
  return (
    <form
      className="control-row stage-form"
      data-part="form"
      onSubmit={(e) => {
        e.preventDefault();
        if (gate.trim()) a.passFrom(gate, "Passed to Testing.");
      }}
    >
      <input autoFocus value={gate} onChange={(e) => setGate(e.target.value)} placeholder="The gate's result line (gate full: green …)" aria-label="The gate's result line" maxLength={500} />
      <button type="submit" disabled={a.busy || !gate.trim()}>Pass to Testing</button>
      <button type="button" onClick={a.close}>Cancel</button>
    </form>
  );
}

function BackForm({ task, a }: { task: Task; a: TaskActions }) {
  const back = earlierStages(task.stage);
  const [backTo, setBackTo] = useState("");
  const [why, setWhy] = useState("");
  // Where it goes: the choice, while it's still an earlier stage; else one back.
  const target = back.some((s) => s.id === backTo) ? backTo : back[0].id;
  return (
    <form
      className="control-row stage-form"
      data-part="form"
      onSubmit={(e) => {
        e.preventDefault();
        if (why.trim()) a.sendBack(why, target);
      }}
    >
      <label>
        Back to
        <select value={target} onChange={(e) => setBackTo(e.target.value)}>
          {back.map((s) => <option key={s.id} value={s.id}>{s.label}</option>)}
        </select>
      </label>
      <input autoFocus value={why} onChange={(e) => setWhy(e.target.value)} placeholder="Why: what's wrong or missing" aria-label="Why it goes back" maxLength={500} />
      <button type="submit" disabled={a.busy || !why.trim()}>Send back</button>
      <button type="button" onClick={a.close}>Cancel</button>
    </form>
  );
}

function BlockForm({ a }: { a: TaskActions }) {
  const [waitingOn, setWaitingOn] = useState("");
  return (
    <form
      className="control-row stage-form"
      data-part="form"
      onSubmit={(e) => {
        e.preventDefault();
        if (waitingOn.trim()) a.block(waitingOn);
      }}
    >
      <input autoFocus value={waitingOn} onChange={(e) => setWaitingOn(e.target.value)} placeholder="Waiting on: a task id (0012), owner, or capture, then a few words" aria-label="What it's waiting on" maxLength={200} />
      <button type="submit" disabled={a.busy || !waitingOn.trim()}>Block</button>
      <button type="button" onClick={a.close}>Cancel</button>
    </form>
  );
}
