import { useState } from "react";
import { BLOCK_KINDS, type BlockKind, blockRequest } from "./blockWords";
import { isHuman } from "./human";
import { tn, useT } from "./i18n";
import { earlierStages, passLabel, stageLabel } from "./lib";
import { isWork } from "./levels";
import type { Question, Task } from "./types";
import { Button, Pill } from "./ui";
import type { TaskActions } from "./useTaskActions";

/** The open stage form, full width under both columns: the gate line and the summary out of
 *  Building, Send back with a reason, or Block with what it waits on. Its text stays while the
 *  core refuses it and goes once it's taken (the form closes). */
export default function StageForms({ task, a }: { task: Task; a: TaskActions }) {
  if (a.form === "gate" && task.stage === "building") return <GateForm task={task} a={a} />;
  if (a.form === "back" && earlierStages(task.stage).length > 0) return <BackForm task={task} a={a} />;
  if (a.form === "block" && !task.blocked) return <BlockForm task={task} a={a} />;
  return null;
}

/** Out of Building: the gate's result line and what was built in one plain line (mu-78), which
 *  leads the owner's check. The summary starts as the task's own; a goal or feature needs none. */
function GateForm({ task, a }: { task: Task; a: TaskActions }) {
  const [gate, setGate] = useState("");
  const [summary, setSummary] = useState(task.summary ?? "");
  const needsSummary = isWork(task);
  return (
    <GateFormView
      gate={gate}
      summary={summary}
      needsSummary={needsSummary}
      busy={a.busy}
      onGate={setGate}
      onSummary={setSummary}
      onSubmit={() => {
        if (gate.trim() && (summary.trim() || !needsSummary)) a.passFrom(gate, tn("pane", "pane.did.passed", { stage: stageLabel("testing") }), summary.trim());
      }}
      onCancel={a.close}
    />
  );
}

/** The gate form, drawn: Pass to Testing waits for the gate line, and for a task the summary. */
export function GateFormView({ gate, summary, needsSummary, busy, onGate, onSummary, onSubmit, onCancel }: {
  gate: string;
  summary: string;
  needsSummary: boolean;
  busy: boolean;
  onGate: (v: string) => void;
  onSummary: (v: string) => void;
  onSubmit: () => void;
  onCancel: () => void;
}) {
  const t = useT("pane");
  const ready = gate.trim() !== "" && (summary.trim() !== "" || !needsSummary);
  return (
    <form
      className="control-row stage-form"
      data-part="form"
      onSubmit={(e) => {
        e.preventDefault();
        onSubmit();
      }}
    >
      <input autoFocus value={gate} onChange={(e) => onGate(e.target.value)} placeholder={t("pane.gate.placeholder")} aria-label={t("pane.gate.label")} maxLength={500} />
      <input value={summary} onChange={(e) => onSummary(e.target.value)} placeholder={t("pane.gate.summaryPlaceholder")} aria-label={t("pane.gate.summaryLabel")} maxLength={160} dir="auto" />
      <Button type="submit" disabled={busy || !ready}>{passLabel("building")}</Button>
      <Button type="button" onClick={onCancel}>{t("common:cancel")}</Button>
    </form>
  );
}

/** Your check's verdict (mu-101): an agent's task there goes back to its builder, the owner's QA
 *  send-back. The owner's own task has no builder. */
export const toBuilder = (stage: string, human: boolean) => stage === "your-check" && !human;

/** Where Send back goes: the choice, while it's still an earlier stage; else the default, which
 *  is Building from Your check (the builder) and one stage back anywhere else. "" when it can't
 *  go back. */
export function backTarget(stage: string, human: boolean, choice: string): string {
  const back = earlierStages(stage);
  if (back.some((s) => s.id === choice)) return choice;
  return toBuilder(stage, human) ? "building" : (back[0]?.id ?? "");
}

/** ⌘↩ (Ctrl+↩) sends a stage form from its text box, as it does the comment box; not while an
 *  input method is composing. */
export const submitKey = (e: { key: string; metaKey: boolean; ctrlKey: boolean; isComposing: boolean }) =>
  e.key === "Enter" && (e.metaKey || e.ctrlKey) && !e.isComposing;

function BackForm({ task, a }: { task: Task; a: TaskActions }) {
  const [choice, setChoice] = useState("");
  const [why, setWhy] = useState("");
  const human = isHuman(task);
  return (
    <BackFormView
      stage={task.stage}
      human={human}
      choice={choice}
      why={why}
      busy={a.busy}
      onChoice={setChoice}
      onWhy={setWhy}
      onSubmit={() => {
        if (why.trim()) a.sendBack(why, backTarget(task.stage, human, choice));
      }}
      onCancel={a.close}
    />
  );
}

/** The Send back form, drawn: Back to (every earlier stage, the default picked) and why. To the
 *  builder from Your check it asks what's wrong, which the builder gets as a finding. */
export function BackFormView({ stage, human, choice, why, busy, onChoice, onWhy, onSubmit, onCancel }: {
  stage: string;
  human: boolean;
  choice: string;
  why: string;
  busy: boolean;
  onChoice: (v: string) => void;
  onWhy: (v: string) => void;
  onSubmit: () => void;
  onCancel: () => void;
}) {
  const t = useT("pane");
  const target = backTarget(stage, human, choice);
  const verdict = toBuilder(stage, human) && target === "building";
  return (
    <form
      className="control-row stage-form"
      data-part="form"
      onSubmit={(e) => {
        e.preventDefault();
        onSubmit();
      }}
    >
      <label>
        {t("pane.back.to")}
        <select value={target} onChange={(e) => onChoice(e.target.value)}>
          {earlierStages(stage).map((s) => <option key={s.id} value={s.id}>{s.label}</option>)}
        </select>
      </label>
      <input
        autoFocus
        value={why}
        onChange={(e) => onWhy(e.target.value)}
        onKeyDown={(e) => {
          if (!submitKey({ key: e.key, metaKey: e.metaKey, ctrlKey: e.ctrlKey, isComposing: e.nativeEvent.isComposing })) return;
          e.preventDefault();
          if (!busy) onSubmit();
        }}
        placeholder={verdict ? t("pane.back.wrongPlaceholder") : t("pane.back.whyPlaceholder")}
        aria-label={verdict ? t("pane.back.wrongLabel") : t("pane.back.whyLabel")}
        maxLength={500}
        dir="auto"
      />
      <Button type="submit" disabled={busy || !why.trim()}>{verdict ? t("pane.back.toBuilder") : t("pane.back.send")}</Button>
      <Button type="button" onClick={onCancel}>{t("common:cancel")}</Button>
    </form>
  );
}

/** Block: why, on a row of chips, then the detail that kind needs (0024). What's typed for each
 *  kind is kept while the owner tries the others. */
function BlockForm({ task, a }: { task: Task; a: TaskActions }) {
  const [kind, setKind] = useState<BlockKind | null>(null);
  const [details, setDetails] = useState<Partial<Record<BlockKind, string>>>({});
  const detail = kind ? (details[kind] ?? "") : "";
  return (
    <BlockFormView
      kind={kind}
      detail={detail}
      questions={task.questions}
      prefix={task.prefix}
      busy={a.busy}
      onKind={setKind}
      onDetail={(d) => {
        if (kind) setDetails((all) => ({ ...all, [kind]: d }));
      }}
      onSubmit={() => {
        const r = kind ? blockRequest(kind, detail, task.prefix) : null;
        if (r?.ok) a.block(r.kind, r.detail);
      }}
      onCancel={a.close}
    />
  );
}

/** The Block form as drawn: the five kinds always on screen (no dropdown), the chosen one
 *  pressed, and under it, in the same place, the detail: task ids (they go into its depends), the
 *  task's open questions as chips, or a line of words. Block waits for what the kind needs. */
export function BlockFormView({ kind, detail, questions, prefix, busy, onKind, onDetail, onSubmit, onCancel }: {
  kind: BlockKind | null;
  detail: string;
  questions: Question[];
  /** The task's project's prefix (0033): its keys are what the tasks field takes. */
  prefix?: string;
  busy: boolean;
  onKind: (k: BlockKind) => void;
  onDetail: (d: string) => void;
  onSubmit: () => void;
  onCancel: () => void;
}) {
  const t = useT("pane");
  const ready = kind !== null && blockRequest(kind, detail, prefix).ok;
  const need = BLOCK_KINDS.find((k) => k.id === kind)?.need ?? "";
  const open = questions.filter((q) => !q.ticked && q.n !== null);
  return (
    <form
      className="stage-form block-form"
      data-part="form"
      onSubmit={(e) => {
        e.preventDefault();
        if (ready) onSubmit();
      }}
    >
      <div className="chip-row" role="group" aria-label={t("pane.block.by")}>
        <span className="lbl">{t("pane.block.by")}</span>
        {BLOCK_KINDS.map((k) => (
          <Pill key={k.id} type="button" on={k.id === kind} aria-pressed={k.id === kind} onClick={() => onKind(k.id)}>
            {k.label}
          </Pill>
        ))}
      </div>
      <div className="control-row">
        {kind === null && <span className="meta small">{t("pane.block.pick")}</span>}
        {kind === "question" && (open.length > 0 ? (
          <div className="chip-row" role="group" aria-label={t("pane.block.questionAria")}>
            {open.map((q) => {
              const id = `Q${q.n}`;
              return (
                <Pill key={id} type="button" on={detail === id} aria-pressed={detail === id} title={q.text} onClick={() => onDetail(id)}>
                  {id} {q.text.split("\n")[0].slice(0, 60)}
                </Pill>
              );
            })}
          </div>
        ) : (
          <span className="meta small">{t("pane.block.noQuestion")}</span>
        ))}
        {kind !== null && kind !== "question" && (
          <input
            key={kind}
            autoFocus
            value={detail}
            onChange={(e) => onDetail(e.target.value)}
            placeholder={need}
            aria-label={need}
            maxLength={200}
          />
        )}
        <Button type="submit" disabled={busy || !ready}>{t("pane.block.block")}</Button>
        <Button type="button" onClick={onCancel}>{t("common:cancel")}</Button>
      </div>
      {kind === "tasks" && <p className="meta small">{t("pane.block.depends")}</p>}
    </form>
  );
}
