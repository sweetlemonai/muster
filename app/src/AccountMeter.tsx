import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "./api";
import { type AccountsInfo, type AccountView, barLabel, intoReserve, nextAsk, reserveAt, rowLines, rowName, rowStatus, rowStatusWidest, shownPercent, type WindowView, WINDOWS } from "./accountsData";
import { useT } from "./i18n";

/** setTimeout's longest wait; a week's reset is well inside it. */
const MAX_WAIT_MS = 2 ** 31 - 1;

/** The accounts and their latest readings (0113), from the core's memory: asked once, again on
 *  each `accounts-changed` (the core watches accounts.json and usage/), and again when the next
 *  live window resets, so it turns stale on time. Never polled. An answer that comes after a
 *  newer question is dropped. */
export function useAccounts(): { info: AccountsInfo | null; error: string; reload: () => void } {
  const [info, setInfo] = useState<AccountsInfo | null>(null);
  const [error, setError] = useState("");
  const asked = useRef(0);
  const ask = useCallback(() => {
    const n = ++asked.current;
    api.accountsInfo().then(
      (i) => {
        if (n !== asked.current) return;
        setInfo(i);
        setError("");
      },
      (e: unknown) => {
        if (n === asked.current) setError(String(e));
      },
    );
  }, []);
  useEffect(() => {
    let live = true;
    // Listening first, then reading, so no change falls between the two.
    const off = listen("accounts-changed", () => {
      if (live) ask();
    });
    const first = () => {
      if (live) ask();
    };
    void off.then(first, first);
    return () => {
      live = false;
      asked.current++;
      void off.then(
        (f) => f(),
        () => {},
      );
    };
  }, [ask]);
  useEffect(() => {
    const secs = info ? nextAsk(info.accounts, info.now) : null;
    if (secs === null) return;
    const timer = window.setTimeout(ask, Math.min(secs * 1000, MAX_WAIT_MS));
    return () => window.clearTimeout(timer);
  }, [info, ask]);
  return { info, error, reload: ask };
}

/** The header's account meter, in its status area; a click on an account opens Settings ›
 *  Accounts. */
export default function AccountMeter({ onOpen }: { onOpen: () => void }) {
  const { info, error } = useAccounts();
  return <AccountMeterView info={info} error={error} onOpen={onOpen} />;
}

/** The meter as drawn: per account one button, its short name (`cc-mg`), its 5 h and 7 d bars and
 *  the 5 h window's next reset, or stale, or no reading. Still a button into Settings with no
 *  accounts or when they can't be read; an empty group while they load. */
export function AccountMeterView({ info, error, onOpen }: { info: AccountsInfo | null; error: string; onOpen: () => void }) {
  const t = useT("shell");
  return (
    <span className="meter" role="group" aria-label={t("meter.aria")}>
      {error ? (
        <button type="button" className="meter-account" onClick={onOpen} title={error}>
          <span className="err">{t("meter.cantRead")}</span>
        </button>
      ) : info && info.accounts.length === 0 ? (
        <button type="button" className="meter-account" onClick={onOpen} title={t("meter.noneTitle")}>
          {t("meter.none")}
        </button>
      ) : (
        info?.accounts.map((a) => <MeterRow key={`${a.provider}/${a.label}`} a={a} now={info.now} onOpen={onOpen} />)
      )}
    </span>
  );
}

/** One account's row: the whole of it is the button, its accessible name both bars' words. */
function MeterRow({ a, now, onOpen }: { a: AccountView; now: number; onOpen: () => void }) {
  const status = rowStatus(a, now);
  const lines = rowLines(a, now);
  return (
    <button type="button" className="meter-account" onClick={onOpen} aria-label={rowName(lines)} title={lines.join("\n")}>
      <span className="meter-name">{a.name}</span>
      <span className="meter-bars">
        {WINDOWS.map((w) => (
          <MeterBar key={w.key} label={barLabel(a.name, w.short, a[w.key], a.reserve, now)} w={a[w.key]} reserve={a.reserve} />
        ))}
      </span>
      {/* The shown words, and in the same cell the widest the readings can give, hidden and never
          read out, so the row keeps its width as readings change (mu-216). */}
      <span className={status.stale ? "meter-when stale" : "meter-when"}>
        <span className="meter-shown">{status.text}</span>
        {rowStatusWidest(a).map((w) => (
          <span key={w} className="meter-room" aria-hidden="true">
            {w}
          </span>
        ))}
      </span>
    </button>
  );
}

/** A 3 px bar: the use, clamped to the bar, in the accent, or the warning colour once it's into
 *  the reserve; hatched when stale; the reserve's mark where the reserve starts. */
function MeterBar({ label, w, reserve }: { label: string; w: WindowView | null; reserve: number }) {
  const cls = ["meter-bar", w?.stale && "stale", w && intoReserve(w.usedPercentage, reserve) && "warn"].filter(Boolean).join(" ");
  return (
    <span className={cls} role="img" aria-label={label}>
      <span className="meter-fill" style={{ inlineSize: `${w ? shownPercent(w.usedPercentage) : 0}%` }} />
      <span className="meter-reserve" style={{ insetInlineStart: `${reserveAt(reserve)}%` }} />
    </span>
  );
}
