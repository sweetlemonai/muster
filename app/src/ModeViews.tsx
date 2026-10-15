import { useState, type ReactNode } from "react";
import { blockedRed } from "./blockWords";
import { needsWords } from "./human";
import { fmt, tl, tn, useT } from "./i18n";
import { modeLabel, useLook, type ColumnFold, type DisplayMode } from "./modes";
import type { Task } from "./types";
import { Pill } from "./ui";

// The small views the display modes (0038) share: the header's mode chip, a board column's head
// (a strip that opens when Focus folds it), a task pane section folded to its header, and a row
// of choices shown whole or as its value. Each folds in place: nothing opens over anything.

/** The header's mode: its name, as a chip that opens Settings › Display. */
export function ModeChip({ mode, onOpen }: { mode: DisplayMode; onOpen: () => void }) {
  return (
    <Pill type="button" className="mode-chip" onClick={onOpen} title={tn("shell", "toolbar.modeChip")}>
      {modeLabel(mode)}
    </Pill>
  );
}

/** How many of `tasks` are blocked, as a card counts it (`cardClass`): for a folded strip, whose
 *  cards don't show. Not a wait on other tasks alone, which isn't red (0060). */
export const blockedCount = (tasks: Pick<Task, "blocked" | "blockedKind">[]) => tasks.filter(blockedRed).length;

/** A column's count in words, as its strip and chip say it: "empty", "1 item", "4 items". */
export const itemWords = (n: number) => (n === 0 ? tn("board", "modes.empty") : tn("board", "modes.items", { count: n }));

/** "2 blocked", in the language shown. */
const blockedWords = (n: number) => tn("board", "modes.blocked", { count: n });

/** A folded place's counts as a screen reader says them (mu-159), its badges in words: "Shaping: 4
 *  items, 1 needs you, 2 blocked". A strip, a band and a chip in the row above the board say it
 *  the same way. `count` is its count in words, as shown ("4 items", "2 tasks"). */
export function placeWords(label: string, count: string, needs: number, blocked: number): string {
  const params = { label, items: count, needs: needsWords(needs), blocked: blockedWords(blocked) };
  if (needs > 0 && blocked > 0) return tn("board", "modes.placeBoth", params);
  if (needs > 0) return tn("board", "modes.placeNeeds", params);
  if (blocked > 0) return tn("board", "modes.placeBlocked", params);
  return tn("board", "modes.place", params);
}

/** What a folded strip says about the cards it hides: how many need you and how many are blocked. */
export function FoldCounts({ needs, blocked }: { needs: number; blocked: number }) {
  useT("board");
  return (
    <>
      {needs > 0 && <b className="count warn" title={needsWords(needs)}>{fmt.num(needs)}</b>}
      {blocked > 0 && <b className="count blocked-count" title={blockedWords(blocked)}>{blockedWords(blocked)}</b>}
    </>
  );
}

/** A board column's head. A column the mode folds is a button: its name, count, needs-you and
 *  blocked counts on a thin strip, and a click opens the column there (or folds it back). An
 *  empty column's strip has the same head without the button (mu-83: one strip anatomy). */
export function ColumnHead({ stage, count, countNode, needs, blocked, fold, onFold, empty = false, onHide, children }: {
  stage: { id: string; label: string; role: string };
  count: number;
  /** The count as shown, when it says more than the number (Your check over its limit, 0050:
   *  "7 of 5"); in every mode, on a folded strip too. */
  countNode?: ReactNode;
  /** How many in the column wait on the owner, and how many are blocked: said on the strip,
   *  since its cards don't show. */
  needs: number;
  blocked: number;
  fold: ColumnFold;
  onFold: () => void;
  /** An empty column's strip (mu-83, modes.ts `columnState`): its head is the strip's, as a
   *  folded column's is, with no role line. */
  empty?: boolean;
  /** An open column's Fold, on the plain board (mu-84): it sends the column to the row above it. */
  onHide?: () => void;
  /** After the count in an open column's head (the board's role line). */
  children?: ReactNode;
}) {
  const t = useT("board");
  const title = stage.role ? `${stage.label}: ${stage.role}` : stage.label;
  const folded = fold === "folded";
  // A strip, empty or folded, says "empty" for nothing in it: a bare 0 down a strip reads as a mark.
  const shown = countNode ?? <small>{count === 0 && (empty || folded) ? t("modes.empty") : fmt.num(count)}</small>;
  const foldInto = t("modes.foldInto", { name: stage.label });
  if (fold === "fixed") {
    if (empty) {
      return (
        <h2 title={title}>
          <span className="col-head">
            {stage.label} {shown}
          </span>
        </h2>
      );
    }
    return (
      <h2 title={title}>
        {stage.label} {shown}
        {onHide && (
          <Pill type="button" className="col-hide" aria-expanded={true} aria-label={foldInto} title={foldInto} onClick={onHide}>
            {t("common:fold")}
          </Pill>
        )}
        {children}
      </h2>
    );
  }
  return (
    <h2 title={title}>
      <button type="button" className="col-fold" aria-expanded={!folded} aria-label={placeWords(stage.label, itemWords(count), needs, blocked)} onClick={onFold} title={folded ? t("row.openHere", { name: stage.label }) : t("modes.foldBack", { name: stage.label })}>
        {stage.label} {shown}
        <FoldCounts needs={needs} blocked={blocked} />
      </button>
    </h2>
  );
}

/** A task pane section that Standard and Focus fold to its header, with a word on what's in it.
 *  The header opens it in place; ADHD shows the section as it is (its wrapper takes no room). The
 *  section stays mounted, folded or not and in every mode, at the same place in the tree, so what's
 *  typed in it survives a fold and a mode switch. */
export function Fold({ label, summary, children }: { label: string; summary?: string; children?: ReactNode }) {
  const how = useLook("pane-sections");
  const [open, setOpen] = useState(false);
  const folds = how === "folded";
  return (
    <div className={!folds ? "fold all" : open ? "fold open" : "fold"}>
      {folds && (
        <button type="button" className="fold-head" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
          {label} {summary && <small>{summary}</small>}
        </button>
      )}
      <div className="fold-body" hidden={folds && !open}>
        {children}
      </div>
    </div>
  );
}

/** A row of choices (Who, Group by): every choice as a chip, or the current one alone, which
 *  lays the others out in its place when clicked; picking one folds them back. */
export function ChoiceChips<T extends string>({ label, className, aria, metaClass = "meta", value, options, onPick }: {
  label: string;
  className: string;
  aria: string;
  metaClass?: string;
  value: T;
  options: readonly { id: T; label: string }[];
  onPick: (v: T) => void;
}) {
  const how = useLook("choices");
  const [open, setOpen] = useState(false);
  const current = options.find((o) => o.id === value) ?? options[0];
  const pick = (v: T) => {
    setOpen(false);
    if (v !== value) onPick(v);
  };
  return (
    <span className={className} role="group" aria-label={aria}>
      <span className={metaClass}>{label}</span>
      {how === "value" && !open ? (
        <Pill on aria-label={tl("choice.current", { label, value: current.label })} onClick={() => setOpen(true)}>
          {current.label}
        </Pill>
      ) : (
        options.map((o) => (
          <Pill key={o.id} on={value === o.id} aria-pressed={value === o.id} onClick={() => pick(o.id)}>
            {o.label}
          </Pill>
        ))
      )}
    </span>
  );
}
