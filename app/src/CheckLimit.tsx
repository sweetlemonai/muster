import { useState } from "react";
import type { QueueInfo } from "./types";
import { checkCount, LIMIT_DEFAULT, LIMIT_MAX, LIMIT_MIN, limitText, type Over, overLine, sideLimit } from "./yourCheckLimit";

// The Your check limit's places (0050): the column's count and the line under its header, the
// sidebar row's line, and the setting in Settings › Projects. Words in the Your check orange,
// never a badge or an overlay.

/** The Your check column's count: "7 of 5" in orange over the limit, else the plain count. */
export function CheckCount({ count, over, queue }: { count: number; over: Over[]; queue: string }) {
  const c = checkCount(count, over, queue);
  return c.over ? <small className="over-limit" title={c.title}>{c.text}</small> : <small>{c.text}</small>;
}

/** The line under the Your check header. Always there, empty within the limit, so the cards
 *  under it never move when it comes or goes. */
export function OverLine({ over, queue }: { over: Over[]; queue: string }) {
  const text = overLine(over, queue);
  return <p className={text ? "limit-line over-limit" : "limit-line"} role="status">{text}</p>;
}

/** A sidebar row's line when its project is over its limit; nothing within it. */
export function SideLimit({ q }: { q: QueueInfo | undefined }) {
  const text = sideLimit(q);
  return text ? <span className="side-limit over-limit">{text}</span> : null;
}

/** A project's limit in Settings › Projects: the limit with its count ("limit 5 · 7 waiting",
 *  orange when over), the Off chip and a number field. `onSet` gets "off" or the number; the core
 *  checks it, and the board uses it at once. */
export function LimitCell({ q, onSet }: { q: QueueInfo; onSet: (value: string) => void }) {
  const on = typeof q.yourCheckLimit === "number";
  const [text, setText] = useState(String(on ? q.yourCheckLimit : LIMIT_DEFAULT));
  const over = sideLimit(q) !== "";
  return (
    <div className="limit-cell" role="group" aria-label={`Your check limit for ${q.name}`}>
      <span className={over ? "over-limit" : "meta"}>{limitText(q)}</span>
      <form
        className="limit-edit"
        onSubmit={(e) => {
          e.preventDefault();
          onSet(text.trim());
        }}
      >
        <button type="button" className={on ? "chip" : "chip on"} aria-pressed={!on} onClick={() => onSet("off")}>
          Off
        </button>
        <input type="number" min={LIMIT_MIN} max={LIMIT_MAX} value={text} onChange={(e) => setText(e.target.value)} aria-label={`Limit for ${q.name}, ${LIMIT_MIN} to ${LIMIT_MAX}`} />
        <button type="submit" disabled={on && text.trim() === String(q.yourCheckLimit)}>Set</button>
      </form>
    </div>
  );
}
