import { useContext, useEffect, useId, useRef, useState } from "react";
import { blockSentence } from "./blockWords";
import { calendarIn, fmt, latinDigits, sentences, tl, tn, useLocale, useT, type Calendar } from "./i18n";
import { BranchPane } from "./Branch";
import { dueCommit, dueStored, dueTyped, kindLabel, KINDS } from "./fields";
import { earlierStages, heldByOther, isStage, leaseRunOut, nameOf, passLabel, stageLabel } from "./lib";
import { Machine, RoleNamesContext, spanText } from "./settingsData";
import { isHuman } from "./human";
import { isWork } from "./levels";
import Links from "./Links";
import { ownerOpen } from "./OwnerChecklist";
import { IMAGE, releaseWords, unreachedHolder, unreachedWords } from "./paneData";
import { pressed } from "./shortcuts";
import { ImageThumb, toggled } from "./StageContent";
import { toBuilder } from "./StageForms";
import TagEditor from "./Tags";
import { Button, Segment, Segmented } from "./ui";
import type { Task } from "./types";
import type { TaskActions } from "./useTaskActions";

/** The task pane's right column: the next step as the one primary button, then Send back and
 *  Block or Unblock; under a rule, priority, type and who does it (the current value; click to choose),
 *  a started task's branch and worktree or its merge (0023), the estimate, tags, files and the
 *  task's links (0017). Every control is always in the same place. */
export default function Sidebar({ task, nowSecs, a, images, files }: {
  task: Task;
  nowSecs: number;
  a: TaskActions;
  images: Record<string, string>;
  /** Which of Files' images are open, flipping one, and the row under the columns that shows them. */
  files: { open: ReadonlySet<string>; onToggle: (name: string) => void; openId: string };
}) {
  const t = useT("pane");
  return (
    <aside className="tp-side" aria-label={t("side.aria")}>
      <NextStep task={task} nowSecs={nowSecs} a={a} />
      <div className="side-settings" data-part="settings">
        <div className="kv">
          <span className="lbl">{t("side.priority")}</span>
          <ValueChip label={t("side.priority")} value={task.priority || "normal"} options={["high", "normal", "low"]} shown={priorityShown} onPick={(v) => a.setField("priority", v, t("side.saved.priority"))} />
        </div>
        <div className="kv">
          <span className="lbl">{t("side.type")}</span>
          {/* The core's types (0017); an old file's idea still shows, as the one pressed. No type is
              the empty value, shown in words. */}
          <ValueChip label={t("side.type")} value={task.kind} options={KINDS} shown={typeShown} onPick={(v) => a.setField("type", v, t("side.saved.type"))} />
        </div>
        {/* Who does it (0022): a task is the owner's own (human) or an agent's (ai). Goals and
            features are their tasks' sum, nobody's to do. */}
        {isWork(task) && (
          <div className="kv">
            <span className="lbl">{t("side.for")}</span>
            <ValueChip
              label={t("side.for")}
              value={isHuman(task) ? "human" : "ai"}
              options={["human", "ai"]}
              shown={forShown}
              onPick={(v) => a.setField("for", v, v === "human" ? t("side.saved.forHuman") : t("side.saved.forAi"))}
            />
          </div>
        )}
        {/* A task started in its own worktree (0023): where; after the merge, the commit and report. */}
        <BranchPane task={task} />
        {/* Three fields (agent time, test time, tokens) need the column's width: label above. */}
        <div className="kv stack">
          <span className="lbl">{t("side.est")}</span>
          <Estimate task={task} onSave={a.setField} />
        </div>
        {task.level === "goal" && (
          <div className="kv stack">
            <span className="lbl">{tn("common", "dueField.label")}</span>
            <DueField task={task} onSave={a.setField} />
          </div>
        )}
        <div className="kv stack">
          <span className="lbl">{t("side.tags")}</span>
          <TagEditor tags={task.tags} onChange={(tags) => a.setField("tags", tags.join(","), t("side.saved.tags"))} />
        </div>
        <FilesView task={task} images={images} open={files.open} onToggle={files.onToggle} onAttach={a.attachPick} openId={files.openId} />
        <Links task={task} />
      </div>
    </aside>
  );
}

/** The next step for this stage, and the two ways off it. A goal or feature is passed on only
 *  while it's shaped (it never goes into or through Building) and has no claim to release. The
 *  owner's own task (0022) is Start in To do and Done in Doing. */
export function NextStep({ task, nowSecs, a }: { task: Task; nowSecs: number; a: TaskActions }) {
  const human = isHuman(task);
  const pass = passLabel(task.stage, human);
  const back = earlierStages(task.stage);
  const names = useContext(RoleNamesContext);
  const holder = heldByOther(task, names);
  const shapedOnly = !isWork(task);
  // A claim nobody has heard from past the lease (this machine's, Settings 0036): the owner may give
  // it back to Ready, or in Shaping let go of it there (0097).
  const lease = useContext(Machine).leaseMinutes;
  // A claim (or Testing's hold) no session can be reached on (mu-184): its own Release, whatever
  // the lease, so the lease's Release isn't shown beside it.
  const unreached = isWork(task) ? unreachedHolder(task) : "";
  const stale = !shapedOnly && !unreached && leaseRunOut(task, nowSecs, lease);
  const showPass = pass !== null && !holder && (!shapedOnly || task.stage === "backlog" || task.stage === "shaping");
  const yoursOpen = !human && task.stage === "your-check" && ownerOpen(task) > 0;
  const t = useT("pane");
  // The words also go in a title: a long translation truncates in its half of the column, never wraps.
  const backWords = toBuilder(task.stage, human) ? t("side.next.sendBackBuilder") : t("side.next.sendBack");
  return (
    <div className="next-step" role="group" aria-label={t("side.next.aria")} data-part="next-step" tabIndex={-1}>
      <span className="lbl">{t("side.next.label")}</span>
      {showPass && (
        <Button
          type="button"
          variant="primary"
          disabled={task.blocked || Boolean(task.paused) || a.busy || yoursOpen}
          title={task.paused ? t("side.next.pausedTitle") : yoursOpen ? t("side.next.tickFirst") : undefined}
          onClick={a.pass}
        >
          {pass}
        </Button>
      )}
      {stale && (
        <Button
          type="button"
          variant={showPass ? "default" : "primary"}
          className={showPass ? "release" : undefined}
          disabled={a.busy}
          title={releaseWords(task.stage).title}
          onClick={a.release}
        >
          {t("side.next.release")}
        </Button>
      )}
      {unreached && <ReleaseUnreached task={task} who={unreached} a={a} />}
      {back.length > 0 && <Button type="button" disabled={a.busy} title={backWords} onClick={() => a.open("back")}>{backWords}</Button>}
      {task.blocked ? (
        <Button type="button" disabled={a.busy} title={t("side.next.unblock")} onClick={a.unblock}>{t("side.next.unblock")}</Button>
      ) : (
        <Button type="button" disabled={a.busy} title={t("side.next.block")} onClick={() => a.open("block")}>{t("side.next.block")}</Button>
      )}
      {holder && <p className="meta small">{t("side.next.holder", { holder, stage: stageLabel(task.stage) })}</p>}
      {stale && <p className="meta small">{sentences([t("side.next.stale", { who: nameOf(task.claimedBy, names), span: spanText(lease) }), releaseWords(task.stage).does])}</p>}
      {unreached && (
        <p className="meta small">
          {unreachedWords(task.stage).note}
          {task.paused ? ` ${unreachedWords(task.stage).paused}` : ""}
        </p>
      )}
      {showPass && task.blocked && <p className="meta small">{blockSentence(task) ? t("side.next.blocked", { why: blockSentence(task) }) : t("side.next.blockedUnnamed")}</p>}
      {showPass && task.paused && <p className="meta small">{t("side.next.paused")}</p>}
      {!showPass && !stale && !holder && <p className="meta small">{nothingNext(task)}</p>}
    </div>
  );
}

/** Esc while Release's confirm is open, from a capture listener on the window. One pressed in
 *  the confirm (`mine`: the focus is in its group, or nowhere, as while its buttons are off) is
 *  the confirm's, so it never reaches the window's own Esc (which closes the pane); it cancels,
 *  except while the release runs. Any other Esc (a rename, a field's choices) goes on as usual. */
export function releaseEscape(e: { key: string; metaKey: boolean; ctrlKey: boolean; stopPropagation: () => void; preventDefault: () => void }, mine: boolean, busy: boolean, cancel: () => void): void {
  if (!mine || !pressed(e, "release-cancel")) return;
  e.stopPropagation();
  e.preventDefault();
  if (!busy) cancel();
}

/** Release's confirm on the window while it's open: a capture listener runs `releaseEscape` for
 *  each key, `inGroup` telling whether the key was pressed in the confirm. Returns what takes it
 *  off again. Apart from the component so real key events can drive it in a test (review R5). */
export function watchReleaseEscape(win: Pick<EventTarget, "addEventListener" | "removeEventListener">, inGroup: (target: EventTarget | null) => boolean, busy: () => boolean, cancel: () => void): () => void {
  const onKey = (e: Event) => releaseEscape(e as KeyboardEvent, inGroup(e.target), busy(), cancel);
  win.addEventListener("keydown", onKey, { capture: true });
  return () => win.removeEventListener("keydown", onKey, { capture: true });
}

/** One of `unreachedWords`' sentences with the holder's name for `{{who}}`, as given. */
export const whoIn = (sentence: string, who: string) => sentence.replace("{{who}}", () => who);

/** The pane's Next step group around `el`, which takes the focus once a release went through. */
const nextStepOf = (el: Element | null) => el?.closest<HTMLElement>('[data-part="next-step"]') ?? null;

/** Release on a claim no session can be reached on (mu-184), with its confirm in place (no
 *  dialog, docs/DESIGN.md): the first click asks, with the focus on the confirm's Release, so
 *  Enter confirms; Cancel or Esc puts the focus back on Release. Once the release goes through,
 *  Release goes with the claim, so the focus moves to the Next step group (review R6); a refusal
 *  puts it back on Release. While it runs, its buttons are off and the group holds the focus, so
 *  an Esc still lands in it. The confirm belongs to the holder and stage it asked about: once
 *  either changes, it's gone. The core refuses for anyone but the owner and for a holder that has
 *  a session by then, and its answer shows in the pane's status. */
function ReleaseUnreached({ task, who, a }: { task: Task; who: string; a: TaskActions }) {
  const names = useContext(RoleNamesContext);
  const key = `${task.stage}|${who}`;
  const [asked, setAsked] = useState<string | null>(null);
  const open = asked === key;
  const button = useRef<HTMLButtonElement>(null);
  const group = useRef<HTMLSpanElement>(null);
  const askId = useId();
  const backTo = useRef<"button" | null>(null);
  useEffect(() => {
    if (open) return;
    const back = backTo.current;
    backTo.current = null;
    if (back === "button") button.current?.focus();
  }, [open]);
  const close = () => {
    backTo.current = "button";
    setAsked(null);
  };
  const busy = useRef(a.busy);
  busy.current = a.busy;
  useEffect(() => {
    if (open && a.busy) group.current?.focus();
  }, [open, a.busy]);
  useEffect(() => {
    if (!open) return;
    return watchReleaseEscape(window, (t) => !t || t === document.body || (t instanceof Node && Boolean(group.current?.contains(t))), () => busy.current, close);
  }, [open]);
  const confirm = async () => {
    const ok = await a.releaseUnreached();
    if (ok) nextStepOf(group.current)?.focus();
    else close();
  };
  return (
    <ReleaseUnreachedView
      stage={task.stage}
      who={nameOf(who, names)}
      asked={open}
      busy={a.busy}
      paused={Boolean(task.paused)}
      onAsk={() => setAsked(key)}
      onConfirm={() => void confirm()}
      onCancel={close}
      buttonRef={button}
      groupRef={group}
      askId={askId}
    />
  );
}

/** Release as drawn (mu-184): the button, named for whose claim it releases; once asked, the
 *  question with the holder's name isolated (<bdi>, so a Persian label reads right) and Release
 *  and Cancel in the same place. */
export function ReleaseUnreachedView({ stage, who, asked, busy, paused, onAsk, onConfirm, onCancel, buttonRef, groupRef, askId = "release-ask" }: {
  stage: string;
  /** The holder's name as the pane shows it. */
  who: string;
  asked: boolean;
  busy: boolean;
  paused: boolean;
  onAsk: () => void;
  onConfirm: () => void;
  onCancel: () => void;
  buttonRef?: React.Ref<HTMLButtonElement>;
  groupRef?: React.Ref<HTMLSpanElement>;
  /** The question's id, for the confirm's aria-describedby (one per pane: useId). */
  askId?: string;
}) {
  const w = unreachedWords(stage);
  const label = whoIn(w.label, who);
  const [before, after] = w.ask.split("{{who}}");
  if (!asked) {
    return (
      <Button type="button" className="release" aria-label={label} title={paused ? w.paused : undefined} disabled={busy || paused} onClick={onAsk} ref={buttonRef}>
        {w.button}
      </Button>
    );
  }
  return (
    <span className="release-confirm" role="group" aria-label={w.group} data-own-esc="" tabIndex={-1} ref={groupRef}>
      <span className="release-ask" id={askId}>{before}<bdi>{who}</bdi>{after}</span>
      <Button type="button" variant="primary" aria-label={label} aria-describedby={askId} disabled={busy} onClick={onConfirm} autoFocus>
        {busy ? w.busy : w.button}
      </Button>
      <Button type="button" disabled={busy} onClick={onCancel}>{w.cancel}</Button>
    </span>
  );
}

/** Why there's no next step to click. */
function nothingNext(t: Task): string {
  if (!isStage(t.stage)) return tn("pane", "side.next.unknownStage");
  if (t.stage === "done") return tn("pane", "side.next.done");
  return tn("pane", "side.next.children");
}

/** A field's current value as one chip. Clicking it lays the options out in its place (no
 *  popover); picking one, Esc, or tabbing away closes them. Picking the current value changes nothing. */
export function ValueChip({ label, value, options, shown, onPick }: { label: string; value: string; options: readonly string[]; shown?: (v: string) => string; onPick: (v: string) => void }) {
  const [open, setOpen] = useState(false);
  const pick = (v: string) => {
    setOpen(false);
    if (v !== value) onPick(v);
  };
  return <ValueChipView label={label} value={value} options={options} shown={shown} open={open} onOpen={() => setOpen(true)} onClose={() => setOpen(false)} onPick={pick} />;
}

/** Priority's values in words (common.json); one the window doesn't know (P1) as written. */
export const priorityShown = (v: string) => (v === "high" || v === "normal" || v === "low" ? tl(`priority.${v}`) : v);
/** Type's values in words (common.json, as the card says them); none is "no type". */
export const typeShown = (v: string) => (v ? kindLabel(v) : tn("pane", "side.noType"));
/** For's two values in words: the owner's own task (human) or the agents' (ai). */
export const forShown = (v: string) => (v === "human" || v === "ai" ? tn("pane", `side.forValue.${v}`) : v);

/** The chip as drawn: each value in words through `shown` (what's saved stays the value). */
export function ValueChipView({ label, value, options, shown: say = (v) => v, open, onOpen, onClose, onPick }: {
  label: string; value: string; options: readonly string[]; shown?: (v: string) => string; open: boolean; onOpen: () => void; onClose: () => void; onPick: (v: string) => void;
}) {
  if (!open) {
    return <button type="button" className="vchip" title={say(value)} aria-label={tl("choice.current", { label, value: say(value) })} onClick={onOpen}>{say(value)}</button>;
  }
  // A value the options don't name (P1 from an older tool, or no type) still shows, as the one pressed.
  const shown = options.includes(value) ? options : [value, ...options];
  return (
    <Segmented
      aria-label={label}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          onClose();
        }
      }}
      onBlur={(e) => {
        // Tabbing away closes it. A click elsewhere leaves it open: WebKit doesn't focus a clicked
        // button, so a click on an option arrives with no new focus, and must still land.
        if (e.relatedTarget && !e.currentTarget.contains(e.relatedTarget)) onClose();
      }}
    >
      {shown.map((o) => (
        <Segment key={o} type="button" active={o === value} autoFocus={o === value} onClick={() => onPick(o)}>{say(o)}</Segment>
      ))}
    </Segmented>
  );
}

/** A goal's due date (mu-151), typed in the calendar Settings › Language picked and in any
 *  digits: in Jalali, «۱۴۰۵/۰۷/۲۲» or 1405/07/22, with the Gregorian ISO day it saves shown beside
 *  it. Saved on ↩ or on leaving it; a date that isn't one is refused here, with why, and nothing
 *  is saved. */
export function DueField({ task, onSave }: { task: Pick<Task, "due">; onSave: (field: string, value: string, done: string) => void }) {
  const calendar = calendarIn(useLocale());
  const [typed, setTyped] = useState(() => dueTyped(task.due, calendar));
  const [error, setError] = useState("");
  useEffect(() => setTyped(dueTyped(task.due, calendar)), [task.due, calendar]);
  const commit = () => setError(dueCommit(typed, calendar, task.due, onSave, tn("common", "dueField.saved")));
  return <DueFieldView typed={typed} error={error} calendar={calendar} onType={setTyped} onCommit={commit} />;
}

/** The due field as drawn from what's typed and the refusal (review R2: no state, so a test
 *  drives its input's own events): the ISO day it saves beside it whenever that isn't what's
 *  typed, in a `<time>`, and the refusal in an alert. */
export function DueFieldView({ typed, error, calendar, onType, onCommit }: {
  typed: string; error: string; calendar: Calendar; onType: (typed: string) => void; onCommit: () => void;
}) {
  const parsed = dueStored(typed, calendar);
  const stores = "iso" in parsed && parsed.iso !== latinDigits(typed).trim() ? parsed.iso : "";
  return (
    <span className="due-field">
      <input value={typed} onChange={(e) => onType(e.target.value)} onBlur={onCommit} onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()} placeholder={calendar === "jalali" ? "1405/07/22" : "2026-10-14"} size={10} maxLength={16} dir="ltr" aria-label={tn("common", "dueField.label")} aria-invalid={error ? true : undefined} title={tn("common", `dueField.${calendar}`)} />
      {stores && <span className="meta small"><time dateTime={stores}>{tn("common", "dueField.stores", { iso: stores })}</time></span>}
      {error && <span className="err" role="alert">{error}</span>}
    </span>
  );
}

/** Est's placeholders: examples of what each field stores, written as it's typed (Latin, in every
 *  language). */
const EST_EXAMPLE = { time: "2h", testTime: "30m", tokens: "500k" };

/** Estimated agent time (review and QA included), test time (the owner's check only) and tokens,
 *  saved when a field loses focus or on Enter. */
function Estimate({ task, onSave }: { task: Task; onSave: (field: string, value: string, done: string) => void }) {
  const t = useT("pane");
  const [time, setTime] = useState(task.estTime);
  const [testTime, setTestTime] = useState(task.estTestTime);
  const [tokens, setTokens] = useState(task.estTokens);
  useEffect(() => setTime(task.estTime), [task.estTime]);
  useEffect(() => setTestTime(task.estTestTime), [task.estTestTime]);
  useEffect(() => setTokens(task.estTokens), [task.estTokens]);
  const saved = { time: "side.saved.time", "test-time": "side.saved.testTime", tokens: "side.saved.tokens" } as const;
  const save = (field: "time" | "test-time" | "tokens", value: string, was: string) => {
    if (value.trim() === was) return;
    onSave(field, value, t(saved[field]));
  };
  const keys = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") (e.target as HTMLInputElement).blur();
  };
  return (
    <span className="est-pair">
      <input value={time} onChange={(e) => setTime(e.target.value)} onBlur={() => save("time", time, task.estTime)} onKeyDown={keys} placeholder={EST_EXAMPLE.time} size={4} maxLength={10} className="ltr" aria-label={t("side.estimate.agentTime")} title={t("common:estimateField.agentTime")} />
      <input value={testTime} onChange={(e) => setTestTime(e.target.value)} onBlur={() => save("test-time", testTime, task.estTestTime)} onKeyDown={keys} placeholder={EST_EXAMPLE.testTime} size={4} maxLength={10} className="ltr" aria-label={t("side.estimate.testTime")} title={t("common:estimateField.testTime")} />
      <input value={tokens} onChange={(e) => setTokens(e.target.value)} onBlur={() => save("tokens", tokens, task.estTokens)} onKeyDown={keys} placeholder={EST_EXAMPLE.tokens} size={4} maxLength={10} className="ltr" aria-label={t("side.estimate.tokens")} title={t("side.estimate.tokensTitle")} />
    </span>
  );
}

/** The task's files: images as small thumbnails, the others by name, and Attach. Paste or drop
 *  files anywhere on the pane too. The images come loaded from the pane (`images`, by file name),
 *  shared with Your check's proof. A click opens an image large in the pane's full-width row under
 *  the columns (FilesOpen, mu-91: this column is too narrow to read one in), several at once; a
 *  second click shuts it. The pane holds which are open (`open`, by file name). */
export function FilesView({ task, images, open, onToggle, onAttach, openId }: {
  task: Task;
  images: Record<string, string>;
  open: ReadonlySet<string>;
  onToggle: (name: string) => void;
  onAttach: () => void;
  /** The id of the row that shows the open images. */
  openId: string;
}) {
  const names = task.attachments.filter((n) => IMAGE.test(n));
  const others = task.attachments.filter((n) => !IMAGE.test(n));
  // tn, not useT: tests call it as a plain function; the pane re-renders it on a switch.
  const t = (k: Parameters<typeof tn<"pane">>[1], p?: Parameters<typeof tn<"pane">>[2]) => tn("pane", k, p);
  return (
    <div className="kv stack files" aria-label={t("side.files.label")}>
      <span className="lbl">{t("side.files.label")} <small>{fmt.num(task.attachments.length)}</small></span>
      <span className="file-list">
        {names.map((n, i) => <ImageThumb key={n} id={thumbId(openId, i)} name={n} url={images[n]} open={open.has(n)} onToggle={() => onToggle(n)} controls={openId} />)}
        {others.map((n) => <span key={n} className="other-file" title={n}>{n}</span>)}
        <Button type="button" className="attach" onClick={onAttach}>{t("side.files.attach")}</Button>
      </span>
    </div>
  );
}

/** The id of Files' i-th image thumbnail, from the open row's id. */
export function thumbId(openId: string, i: number): string {
  return `${openId}-thumb-${i}`;
}

function focusById(id: string) {
  document.getElementById(id)?.focus();
}

/** The id of Files' i-th image as it shows open, in the row under the columns. */
export function bigId(openId: string, i: number): string {
  return `${openId}-big-${i}`;
}

/** What toggling a Files image does (mu-91): flips it in `open`, and one that opens is brought
 *  into view (`show`, given its figure's id) once it's drawn, since the row can sit below the fold. */
export function fileToggler(task: Pick<Task, "attachments">, openId: string, open: ReadonlySet<string>, setOpen: (f: (o: ReadonlySet<string>) => Set<string>) => void, show: (id: string) => void = showSoon) {
  const names = task.attachments.filter((n) => IMAGE.test(n));
  return (n: string) => {
    setOpen((o) => toggled(o, n));
    if (!open.has(n) && names.includes(n)) show(bigId(openId, names.indexOf(n)));
  };
}

/** Scrolls the element with this id into view after the next render: as little as shows it, and
 *  not smoothly when the system asks for less motion. Again once its image has loaded, since it
 *  only then has its height. */
export function showSoon(id: string, win: Pick<Window, "requestAnimationFrame" | "matchMedia" | "document"> = window) {
  win.requestAnimationFrame(() => {
    const el = win.document.getElementById(id);
    if (!el) return;
    const still = win.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const go = () => el.scrollIntoView({ block: "nearest", behavior: still ? "auto" : "smooth" });
    go();
    const img = el.querySelector("img");
    if (img && !img.complete) img.addEventListener("load", go, { once: true });
  });
}

/** Files' open images (mu-91), each at the pane's full width in a row under the two columns, in
 *  the task's order, with its name under it; clicking one shuts it, as its thumbnail does. Nothing
 *  while none is open: the row stays, hidden, so the thumbnails' `aria-controls` always resolve; a
 *  task with no image has no row. Shutting one from here puts the focus on its thumbnail, since
 *  the button that had it is gone. */
export function FilesOpen({ task, images, open, onToggle, id, focus = focusById }: {
  task: Task;
  images: Record<string, string>;
  open: ReadonlySet<string>;
  onToggle: (name: string) => void;
  id: string;
  /** Puts the focus on the element with this id (a test passes its own). */
  focus?: (id: string) => void;
}) {
  // tn, not useT: tests call it as a plain function; the pane re-renders it on a switch.
  const t = (k: Parameters<typeof tn<"pane">>[1], p?: Parameters<typeof tn<"pane">>[2]) => tn("pane", k, p);
  const names = task.attachments.filter((n) => IMAGE.test(n));
  if (names.length === 0) return null;
  const shown = names.filter((n) => open.has(n));
  const shut = (n: string) => {
    onToggle(n);
    focus(thumbId(id, names.indexOf(n)));
  };
  return (
    <div className="files-open" id={id} data-part="files-open" aria-label={t("side.files.open")} hidden={shown.length === 0}>
      {shown.map((n) => (
        <figure key={n} id={bigId(id, names.indexOf(n))} className="file-big">
          <button type="button" className="file-big-btn" aria-label={t("side.files.shut", { name: n })} title={t("side.files.shut", { name: n })} onClick={() => shut(n)}>
            {images[n] ? <img src={images[n]} alt={n} /> : <span className="mini-img" />}
          </button>
          <figcaption dir="auto">{n}</figcaption>
        </figure>
      ))}
    </div>
  );
}
