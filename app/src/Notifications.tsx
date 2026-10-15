import { useState } from "react";
import { api } from "./api";
import { HomePath } from "./HomePath";
import { tl, tn } from "./i18n";
import { renderTags } from "./i18n/react";
import { Pill } from "./ui";
import type { SettingsInfo } from "./settingsData";

/** Settings › Notifications (0052) with its data: the switches (notifications, and the menu bar
 *  icon, 0085) saved by the core, which shows its refusal when it can't save. */
export default function Notifications({ info, onInfo }: { info: SettingsInfo | null; onInfo: (i: SettingsInfo) => void }) {
  const [error, setError] = useState("");
  if (!info) return <p className="none">{tl("loading")}</p>;
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
        {v ? tl("on") : tl("off")}
      </Pill>
    ));
  return (
    <div className="settings-sections">
      <section>
        <h2>{tn("shell", "notifications.mac")}</h2>
        <div className="look-colour" role="group" aria-label={tn("shell", "notifications.mac")}>
          <span className="meta">{tn("shell", "notifications.notifyMe")}</span>
          {pills("notify", info.settings.notify ?? true)}
        </div>
        {error && <p className="err" role="alert">{error}</p>}
        <p className="meta">{tn("shell", "notifications.what")}</p>
        <p className="meta">{tn("shell", "notifications.once")}</p>
        <p className="meta">{tn("shell", "notifications.macos")}</p>
      </section>
      <section>
        <h2>{tn("shell", "notifications.menuBar")}</h2>
        <div className="look-colour" role="group" aria-label={tn("shell", "notifications.menuBar")}>
          <span className="meta">{tn("shell", "notifications.showIt")}</span>
          {pills("menu-bar", info.settings.menuBar ?? true)}
        </div>
        <p className="meta">{tn("shell", "notifications.icon")}</p>
        <p className="meta">{tn("shell", "notifications.click")}</p>
        {/* renderTags, not <Trans>: the view is drawn without hooks (its test walks it as plain calls). */}
        <p className="meta">{renderTags(tn("shell", "notifications.saved"), { path: <HomePath path={`${info.configDir}/settings.json`} home={info.home} /> })}</p>
      </section>
    </div>
  );
}
