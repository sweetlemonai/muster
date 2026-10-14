import { useState } from "react";
import { api } from "./api";
import { HomePath } from "./HomePath";
import type { SettingsInfo } from "./settingsData";

/** Settings › Notifications (0052) with its data: the switch saved by the core, which shows its
 *  refusal when it can't save. */
export default function Notifications({ info, onInfo }: { info: SettingsInfo | null; onInfo: (i: SettingsInfo) => void }) {
  const [error, setError] = useState("");
  if (!info) return <p className="none">Loading…</p>;
  return (
    <NotificationsView
      info={info}
      error={error}
      onSet={(on) =>
        void api.setSetting("notify", on ? "on" : "off").then(
          (i) => {
            setError("");
            onInfo(i);
          },
          (e: unknown) => setError(String(e)),
        )
      }
    />
  );
}

/** Notifications as drawn: On and Off, the one in force lit (on unless turned off), what
 *  notifies and when it doesn't, and where it's saved. The core decides and posts them
 *  (notify.rs); the window only turns them on and off. */
export function NotificationsView({ info, error = "", onSet }: { info: SettingsInfo; error?: string; onSet: (on: boolean) => void }) {
  const on = info.settings.notify ?? true;
  return (
    <div className="settings-sections">
      <section>
        <h2>Mac notifications</h2>
        <div className="look-colour" role="group" aria-label="Mac notifications">
          <span className="meta">Notify me</span>
          {[true, false].map((v) => (
            <button key={String(v)} type="button" className={on === v ? "chip on" : "chip"} aria-pressed={on === v} onClick={() => onSet(v)}>
              {v ? "On" : "Off"}
            </button>
          ))}
        </div>
        {error && <p className="err" role="alert">{error}</p>}
        <p className="meta">
          One notification when something new waits for you: a task in Your check, a question escalated to you, a task blocked on you or by a permission, a
          session that waits for your input in Claude Code, a task of your own, or lessons to approve. Several at once come as one.
        </p>
        <p className="meta">
          It's never twice for the same thing, not for what was already waiting when Muster started, and not while Muster's window is in front showing its
          project (or All).
        </p>
        <p className="meta">
          macOS asks whether Muster may notify the first time it has something to tell you; change that later in System Settings › Notifications. Nothing
          leaves this Mac.
        </p>
        <p className="meta">Saved on this Mac, in <HomePath path={`${info.configDir}/settings.json`} home={info.home} />, with what it has notified beside it.</p>
      </section>
    </div>
  );
}
