import { useState } from "react";
import { api } from "./api";
import { HomePath } from "./HomePath";
import { Pill } from "./ui";
import type { SettingsInfo } from "./settingsData";

/** Settings › Notifications (0052) with its data: the switches (notifications, and the menu bar
 *  icon, 0085) saved by the core, which shows its refusal when it can't save. */
export default function Notifications({ info, onInfo }: { info: SettingsInfo | null; onInfo: (i: SettingsInfo) => void }) {
  const [error, setError] = useState("");
  if (!info) return <p className="none">Loading…</p>;
  return (
    <NotificationsView
      info={info}
      error={error}
      onSet={(name, on) =>
        void api.setSetting(name, on ? "on" : "off").then(
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

/** A switch Notifications saves: `settings::set`'s name for it. */
export type Switch = "notify" | "menu-bar";

/** Notifications as drawn: each switch's On and Off, the one in force lit (on unless turned off),
 *  what notifies and when it doesn't, what the menu bar icon shows, and where it's saved. The core
 *  decides, posts them and draws the icon (notify.rs, tray.rs); the window only turns them on and
 *  off. */
export function NotificationsView({ info, error = "", onSet }: { info: SettingsInfo; error?: string; onSet: (name: Switch, on: boolean) => void }) {
  const pills = (name: Switch, on: boolean) =>
    [true, false].map((v) => (
      <Pill key={String(v)} type="button" on={on === v} aria-pressed={on === v} onClick={() => onSet(name, v)}>
        {v ? "On" : "Off"}
      </Pill>
    ));
  return (
    <div className="settings-sections">
      <section>
        <h2>Mac notifications</h2>
        <div className="look-colour" role="group" aria-label="Mac notifications">
          <span className="meta">Notify me</span>
          {pills("notify", info.settings.notify ?? true)}
        </div>
        {error && <p className="err" role="alert">{error}</p>}
        <p className="meta">
          One notification when something new waits for you: a task in Your check, a question escalated to you, a task blocked on you or by a permission, a
          session that waits for your input in Claude Code, a task of your own, or lessons to approve. Several at once come as one.
        </p>
        <p className="meta">
          It's never twice for the same thing, and not for what was already waiting when Muster started. While Muster's window is in front, what it shows
          (its project, or All) counts as seen, and news from another project waits until you switch away.
        </p>
        <p className="meta">
          macOS asks whether Muster may notify the first time it has something to tell you; change that later in System Settings › Notifications. Nothing
          leaves this Mac.
        </p>
      </section>
      <section>
        <h2>Menu bar icon</h2>
        <div className="look-colour" role="group" aria-label="Menu bar icon">
          <span className="meta">Show it</span>
          {pills("menu-bar", info.settings.menuBar ?? true)}
        </div>
        <p className="meta">
          Muster's robot head in the menu bar, plain while nothing waits for you and with its antenna tips filled in when something does: the same count as the Dock
          badge, from the same reads. Its tooltip and menu say the count in words, never a task's text.
        </p>
        <p className="meta">Click it to bring Muster's window forward. Right-click or Control-click it for Show Muster, the count and Quit Muster.</p>
        <p className="meta">Both are saved on this Mac, in <HomePath path={`${info.configDir}/settings.json`} home={info.home} />, with what it won't notify again beside it.</p>
      </section>
    </div>
  );
}
