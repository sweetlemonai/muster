import { useContext, useEffect, useState, type ReactNode } from "react";
import { fmtMinutes, fmtTokens, nameOf, stageLabel, stamp, stampMinutes } from "./lib";
import { estimateLeft, isWork, levelLabel, progressText, shownStageClass, shownStageLabel } from "./levels";
import { DueDate, Stamp } from "./Dates";
import { tn, unmarked, useT, type TKey } from "./i18n";
import { renderTags } from "./i18n/react";
import Markdown from "./Markdown";
import { siblingKey, taskKey } from "./taskKey";
import OwnerChecklist from "./OwnerChecklist";
import { useRunParts } from "./Run";
import { criteriaSection, fmtWall, IMAGE, timeInWork } from "./paneData";
import type { Criterion, Task } from "./types";
import { StatusPill } from "./ui";
import { acceptWords, criterionWords, type NamedView } from "./yourCheck";
import { RoleNamesContext } from "./settingsData";

/** The task pane's left column: what this stage needs. Building and Testing: the work so far.
 *  Your check: what you're accepting (mu-78) with its Run (mu-79), the owner's checklist and the history. Backlog, Shaping and Ready: the criteria and
 *  the estimate. Done (and a stage the window doesn't know): the history. A goal or feature: its
 *  roll-up. `body` is the write-up as loaded (null while it loads). */
export default function StageContent({ task, nowSecs, body, childTasks, busy, onTick, onView, images = {} }: {
  task: Task;
  nowSecs: number;
  body: string | null;
  childTasks?: Task[];
  busy: boolean;
  onTick: (which: string, undo: boolean) => void;
  /** Opens a view an owner criterion names (0043). */
  onView?: (v: NamedView) => void;
  /** The task's images as data URLs, by file name, loaded once by the pane (TaskPanel). */
  images?: Record<string, string>;
}) {
  const t = useT("pane");
  if (!isWork(task)) return <Rollup task={task} childTasks={childTasks} />;
  switch (task.stage) {
    case "building":
    case "testing":
      return <WorkSoFar task={task} nowSecs={nowSecs} />;
    case "your-check":
      return (
        <>
          <WhatYoureAccepting task={task} images={images} nowSecs={nowSecs} />
          <div className="stage-block" data-part="checklist">
            <span className="lbl">{t("stage.checklist")}</span>
            {task.ownerCriteria.length > 0 ? (
              <OwnerChecklist task={task} busy={busy} onTick={onTick} onView={onView} />
            ) : (
              <p className="meta small">{t("stage.noOwner")}</p>
            )}
          </div>
          <History task={task} />
        </>
      );
    case "backlog":
    case "shaping":
    case "ready":
      return (
        <>
          <Criteria body={body} />
          <Estimates task={task} />
        </>
      );
    default:
      return <History task={task} />;
  }
}

/** Which thumbnails are open (proof ones by `<criterion n>/<file>`, Files' by file name): `key`
 *  opened, or shut again. */
export function toggled(open: ReadonlySet<string>, key: string): Set<string> {
  const next = new Set(open);
  if (!next.delete(key)) next.add(key);
  return next;
}

/** A screenshot as a small thumbnail button that opens it large in the pane's flow and shuts it on
 *  a second click: no overlay. Its file name names the button and stays under the image; a
 *  placeholder until the image has loaded. A proof's opens right there, at the main column's width
 *  (mu-78). Files' (mu-91) sits in the narrow side column, so it stays small, marked open, and
 *  `controls` names the full-width row below the columns that holds the large image. */
export function ImageThumb({ name, url, open, onToggle, controls, id }: { name: string; url: string | undefined; open: boolean; onToggle: () => void; controls?: string; id?: string }) {
  const big = open && controls === undefined;
  return (
    <button type="button" id={id} className={big ? "mini-thumb proof-thumb open" : open ? "mini-thumb proof-thumb on" : "mini-thumb proof-thumb"} aria-expanded={open} aria-controls={controls} aria-label={name} title={name} onClick={onToggle}>
      {url ? <img className={big ? "proof-big" : undefined} src={url} alt={big ? name : ""} /> : <span className="mini-img" />}
      <span className="name" dir="auto">{name}</span>
    </button>
  );
}

/** Your check's "What you're accepting" (mu-78): which thumbnails are open, shut again for
 *  another task. The screenshots come from the pane's one map (`images`), which Files reads too.
 *  Its Run line, and the Run block under it once the project has a run (mu-79). */
function WhatYoureAccepting({ task, images, nowSecs }: { task: Task; images: Record<string, string>; nowSecs: number }) {
  const [open, setOpen] = useState<ReadonlySet<string>>(() => new Set());
  useEffect(() => setOpen(new Set()), [task.queue, task.id]);
  const run = useRunParts(task.queue, nowSecs);
  return (
    <>
      <AcceptingBlock task={task} urls={images} open={open} onToggle={(k) => setOpen((o) => toggled(o, k))} runLine={run.line} />
      {run.block}
    </>
  );
}

/** What you're accepting, drawn: what was built (the builder's one-line summary, as plain text,
 *  or the title and how to add one), what Accept does (`acceptWords`), and each criterion in one
 *  line with its proof in words and the screenshots it names as thumbnails under it, each opening
 *  large in place (`open`, keyed `<n>/<file>`) and shrinking on a second click. Then the merge
 *  line from the log. Every line's words are isolated (`dir="auto"`) for right-to-left text.
 *  `runLine` is the Run line (mu-79): Run with the project's command, to try it. */
export function AcceptingBlock({ task, urls, open, onToggle, runLine }: {
  task: Task;
  urls: Record<string, string>;
  open: ReadonlySet<string>;
  onToggle: (key: string) => void;
  runLine?: ReactNode;
}) {
  // tn, not useT, and renderTags, not <Trans>: drawn without hooks (its test walks it as plain
  // calls); StageContent's useT renders it again on a language switch.
  const t = (k: TKey<"pane">) => tn("pane", k);
  const criteria = task.criteria ?? [];
  const accept = acceptWords(task);
  return (
    <div className="stage-block accepting" data-part="accepting" role="group" aria-label={t("stage.accepting")}>
      <span className="lbl">{t("stage.accepting")}</span>
      <p className="wline">
        <span className="lbl">{t("stage.built")}</span>
        <span className="small">
          {task.summary ? (
            <span dir="auto">{task.summary}</span>
          ) : (
            <>
              <span dir="auto">{task.title}</span>{" "}
              <span className="meta">
                {/* i18n-ignore: the command is what is typed, Latin, in its own <code>, without isolation marks. */}
                {renderTags(tn("pane", "stage.noSummary", { cmd: `muster set ${taskKey(task)} summary` }), { code: <code /> }, (s, inTag) => (inTag ? unmarked(s) : s))}
              </span>
            </>
          )}
        </span>
      </p>
      <p className="wline">
        <span className="lbl">{t("stage.accept")}</span>
        <span className={`small accept-${accept.kind}`}>{accept.text}</span>
      </p>
      {runLine && (
        <p className="wline">
          <span className="lbl">{t("stage.try")}</span>
          {runLine}
        </p>
      )}
      <p className="wline">
        <span className="lbl">{t("stage.proof")}</span>
        {criteria.length === 0 && <span className="small meta">{t("stage.noCriteria")}</span>}
      </p>
      {criteria.length > 0 && (
        <ul className="proof-list">
          {criteria.map((c) => (
            <ProofLine key={c.n} c={c} urls={urls} open={open} onToggle={onToggle} />
          ))}
        </ul>
      )}
      {task.mergeReport && <p className="meta small merge-line" dir="auto">{task.mergeReport}</p>}
    </div>
  );
}

/** How a criterion stands, in words: proven (with its proof), checked by the owner, ticked with
 *  no proof, the owner's still to check, or not proven. */
function standing(c: Criterion): string {
  if (c.ticked) return tn("pane", c.owner ? "stage.standing.checkedByYou" : c.proof ? "stage.standing.proven" : "stage.standing.tickedNoProof");
  return tn("pane", c.owner ? "stage.standing.yours" : "stage.standing.notProven");
}

function ProofLine({ c, urls, open, onToggle }: { c: Criterion; urls: Record<string, string>; open: ReadonlySet<string>; onToggle: (key: string) => void }) {
  const files = c.proofFiles ?? [];
  const words = criterionWords(c.text.replace(/^AC\d+:\s*/, ""));
  return (
    <li className={c.ticked ? "proof-line ticked" : "proof-line"}>
      <b className="proof-label">{c.label || tn("pane", "stage.criterion", { n: String(c.n) })}</b> · <span dir="auto">{words}</span> · <span className="proof-state">{standing(c)}</span>
      {c.ticked && !c.owner && c.proof && <> <span className="proof-text" dir="auto">{c.proof}</span></>}
      {files.length > 0 && (
        <span className="file-list proof-files">
          {files.map((n) => {
            const k = `${c.n}/${n}`;
            if (!IMAGE.test(n)) return <span key={n} className="other-file" title={n}>{n}</span>;
            return <ImageThumb key={n} name={n} url={urls[n]} open={open.has(k)} onToggle={() => onToggle(k)} />;
          })}
        </span>
      )}
    </li>
  );
}

/** Building and Testing: the stage history, time spent against the estimate, and sessions (0025). */
function WorkSoFar({ task, nowSecs }: { task: Task; nowSecs: number }) {
  // Paused time (0045) isn't time spent on it.
  const spent = timeInWork(task.stageHistory, task.stage, stampMinutes(stamp(new Date(nowSecs * 1000))), task.pauseHistory);
  // Time spent is wall-clock (24-hour days); the estimates are agent time and test time (8-hour
  // days). Each is named for what it is, and nothing pretends to compare them.
  const t = useT("pane");
  const parts = [
    task.estMinutes > 0 ? t("stage.work.agentPart", { time: fmtMinutes(task.estMinutes) }) : "",
    task.estTestMinutes > 0 ? t("stage.work.testPart", { time: fmtMinutes(task.estTestMinutes) }) : "",
  ].filter(Boolean);
  const est = parts.length === 2 ? t("stage.work.both", { a: parts[0], b: parts[1] }) : (parts[0] ?? "");
  const spentText = spent === null ? t("stage.work.notTimed") : t("stage.work.spent", { time: fmtWall(spent) });
  return (
    <div className="stage-block" data-part="work">
      <span className="lbl">{t("stage.work.title")}</span>
      <p className="wline">
        <span className="lbl">{t("stage.work.time")}</span>
        <span className="small">{est ? t("stage.work.withEstimate", { spent: spentText, est }) : t("stage.work.noEstimate", { spent: spentText })}</span>
      </p>
      {task.estTokenCount > 0 && (
        <p className="wline">
          <span className="lbl">{t("stage.work.tokens")}</span>
          <span className="small">{t("stage.work.tokensLine", { tokens: fmtTokens(task.estTokenCount) })}</span>
        </p>
      )}
      <p className="meta small">{t("stage.work.sessions")}</p>
      <HistoryList task={task} />
    </div>
  );
}

/** The stage moves from the log, newest first: when, from → to, who. */
function History({ task }: { task: Task }) {
  const t = useT("pane");
  return (
    <div className="stage-block" data-part="history">
      <span className="lbl">{t("stage.history.title")} <span className="meta">{t("stage.history.sub")}</span></span>
      <HistoryList task={task} />
    </div>
  );
}

function HistoryList({ task }: { task: Task }) {
  const t = useT("pane");
  const names = useContext(RoleNamesContext);
  if (task.stageHistory.length === 0) return <p className="meta small">{t("stage.history.none")}</p>;
  return (
    <ol className="history">
      {[...task.stageHistory].reverse().map((s, i) => (
        <li key={`${s.at}-${i}`}>
          <Stamp className="h-when" at={s.at} />
          <span>
            <b>{t("stage.history.move", { from: stageLabel(s.from), to: stageLabel(s.to) })}</b> <span className="meta" dir="auto">· {nameOf(s.who, names)}</span>
          </span>
        </li>
      ))}
    </ol>
  );
}

/** Backlog, Shaping and Ready: the acceptance criteria from the write-up. */
function Criteria({ body }: { body: string | null }) {
  const t = useT("pane");
  const section = body === null ? null : criteriaSection(body);
  return (
    <div className="stage-block" data-part="criteria">
      <span className="lbl">{t("stage.criteria")}</span>
      {body === null ? <p className="meta small">{t("common:loading")}</p> : section ? <Markdown text={section} /> : <p className="meta small">{t("stage.noCriteriaYet")}</p>}
    </div>
  );
}

/** The estimate: agent time plus test time (as the board's totals count it), and tokens. */
function Estimates({ task }: { task: Task }) {
  const t = useT("pane");
  const parts = [fmtMinutes(task.estMinutes + task.estTestMinutes), fmtTokens(task.estTokenCount)].filter(Boolean).join(" · ");
  return (
    <div className="stage-block" data-part="estimates">
      <span className="lbl">{t("stage.estimate")}</span>
      <p className="small">{parts ? t("stage.approx", { parts }) : <span className="meta">{t("stage.notEstimated")}</span>}</p>
    </div>
  );
}

/** A goal's or feature's roll-up: its tasks' progress, the estimate left, a goal's due date, and
 *  its children with the stage each shows. */
function Rollup({ task, childTasks }: { task: Task; childTasks?: Task[] }) {
  const byId = new Map((childTasks ?? []).map((c) => [c.id, c]));
  const t = useT("pane");
  const left = estimateLeft(task.progress);
  return (
    <section className="rollup-panel" aria-label={t("stage.rollup")} data-part="rollup">
      <h2>{levelLabel(task.level)} · {progressText(task.progress) || t("stage.noTasks")}</h2>
      {left && <p className="meta">{left}</p>}
      {task.due && <p className="meta"><DueDate iso={task.due} /></p>}
      {task.children.length > 0 && (
        <ul className="children">
          {task.children.map((id) => {
            const c = byId.get(id);
            return (
              <li key={id}>
                {c ? <><span className="id">{siblingKey(task, id)}</span> {c.title} <StatusPill status={shownStageClass(c)}>{shownStageLabel(c)}</StatusPill></> : siblingKey(task, id)}
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
