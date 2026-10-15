import { type Ref, useEffect, useRef, useState } from "react";
import { api, type HelperInfo } from "./api";
import Appearance from "./Appearance";
import { DisplayModes } from "./Display";
import { AccountPicker } from "./Helper";
import { HelpButton } from "./HelpButton";
import { HomePath } from "./HomePath";
import Notifications from "./Notifications";
import Projects from "./Projects";
import { sessionStatesLine } from "./sessionState";
import { homePath } from "./lib";
import type { DisplayMode } from "./modes";
import { ACCOUNTS_WORDS, accountsEdit, DEFAULT_MACHINE, SETTINGS_GROUPS, type SettingsGroup, type SettingsInfo, spanInput, spanText } from "./settingsData";
import type { Project, QueueInfo } from "./types";
import { Button, Pill } from "./ui";

/** Settings (0036): one place for Appearance, Display, Projects, Agents, Notifications and About. A view in the
 *  header's row, never a window: the groups are a docked list on the left, and the open group's
 *  settings on the right, all of them at once. Every change applies at once and is remembered. */
export default function Settings({ group, onGroup, queues, onChanged, mode, onMode }: {
  group: SettingsGroup;
  onGroup: (g: SettingsGroup) => void;
  queues: QueueInfo[];
  /** Read the queues again, so the board uses a change now, not at the next poll. */
  onChanged: () => void;
  /** The display mode (0038), which the window keeps. */
  mode: DisplayMode;
  onMode: (m: DisplayMode) => void;
}) {
  const [info, setInfo] = useState<SettingsInfo | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    let live = true;
    api.settingsInfo().then(
      (i) => live && setInfo(i),
      (e: unknown) => live && setError(String(e)),
    );
    return () => {
      live = false;
    };
  }, []);
  const g = SETTINGS_GROUPS.find((x) => x.id === group) ?? SETTINGS_GROUPS[0];
  return (
    <div className="settings">
      <nav className="settings-groups" aria-label="Settings groups">
        {SETTINGS_GROUPS.map((x) => (
          <button key={x.id} type="button" className={x.id === g.id ? "settings-group on" : "settings-group"} aria-current={x.id === g.id ? "page" : undefined} onClick={() => onGroup(x.id)}>
            {x.label}<small>{x.holds}</small>
          </button>
        ))}
      </nav>
      <section className="settings-body" aria-label={g.label}>
        <header className="settings-head">
          <h1>{g.label} <HelpButton topic={g.topic} /></h1>
          <p className="meta">{g.holds}</p>
        </header>
        {error && <p className="err" role="alert">Can't read this Mac's settings: {error}</p>}
        {g.id === "appearance" && <Appearance />}
        {g.id === "display" && <DisplayModes mode={mode} onMode={onMode} />}
        {g.id === "projects" && <Projects queues={queues} home={info?.home ?? ""} onChanged={onChanged} />}
        {g.id === "agents" && <Agents info={info} queues={queues} onInfo={setInfo} onChanged={onChanged} />}
        {g.id === "notifications" && <Notifications info={info} onInfo={setInfo} />}
        {g.id === "about" && (info ? <About info={info} /> : !error && <p className="none">Loading…</p>)}
      </section>
    </div>
  );
}

/** Agents with its data: the helper's (asked of the core, which checks each account's login) and
 *  this machine's settings. */
function Agents({ info, queues, onInfo, onChanged }: { info: SettingsInfo | null; queues: QueueInfo[]; onInfo: (i: SettingsInfo) => void; onChanged: () => void }) {
  const [helper, setHelper] = useState<HelperInfo | null>(null);
  const [error, setError] = useState("");
  const [asked, setAsked] = useState(0);
  // The helper's account and model are the same for every project; any project's name will do.
  const queue = queues[0]?.name ?? "";
  useEffect(() => {
    let live = true;
    api.helperInfo(queue).then(
      (h) => live && setHelper(h),
      (e: unknown) => live && setError(String(e)),
    );
    return () => {
      live = false;
    };
  }, [queue, asked]);
  if (!info) return <p className="none">Loading…</p>;
  return (
    <AgentsView
      info={info}
      helper={helper}
      helperError={error}
      onHelper={() => setAsked((n) => n + 1)}
      // Lit as soon as the core saved it; the accounts aren't checked again for a model.
      onModel={(m) => void api.helperSetModel(m).then(() => setHelper((h) => withModel(h, m)), (e: unknown) => setError(String(e)))}
      onSet={(name, value) => saveAgentsSetting(name, value, { set: api.setSetting, onInfo, onChanged, onHelper: () => setAsked((n) => n + 1) })}
    />
  );
}

/** The settings Agents changes. */
type AgentsSettingName = "lease" | "live" | "claude-accounts";

/** Saves one of Agents' settings: Settings takes the core's answer and the board reads the
 *  queues again. A change to the accounts list asks for the helper's accounts again too, so its
 *  picker on the same page follows the list. Null when saved, else the core's refusal. */
export async function saveAgentsSetting(name: AgentsSettingName, value: string, to: {
  set: (name: AgentsSettingName, value: string) => Promise<SettingsInfo>;
  onInfo: (i: SettingsInfo) => void;
  onChanged: () => void;
  onHelper: () => void;
}): Promise<string | null> {
  try {
    to.onInfo(await to.set(name, value));
    to.onChanged();
    if (name === "claude-accounts") to.onHelper();
    return null;
  } catch (e) {
    return String(e);
  }
}

/** The helper's info with `model` in use, once the core has saved it. */
export const withModel = (h: HelperInfo | null, model: string): HelperInfo | null => h && { ...h, model };

const MODEL_NAMES: Record<string, string> = { sonnet: "Sonnet", haiku: "Haiku", opus: "Opus" };

/** Agents as drawn: the claim lease and the LIVE window first (they're there at once), then the
 *  helper's account and model, which load slowly (the core asks each account's login), below
 *  them so nothing moves when they arrive. */
export function AgentsView({ info, helper, helperError = "", onHelper, onModel, onSet }: {
  info: SettingsInfo;
  /** Null while the core checks the accounts. */
  helper: HelperInfo | null;
  helperError?: string;
  onHelper: () => void;
  onModel: (model: string) => void;
  /** Resolves to null when saved, else the core's refusal to show. */
  onSet: (name: AgentsSettingName, value: string) => Promise<string | null>;
}) {
  const s = info.settings;
  return (
    <div className="settings-sections">
      <section>
        <h2>Claims</h2>
        <SpanSetting
          key={`lease-${s.leaseMinutes}`}
          label="Claim lease"
          minutes={s.leaseMinutes}
          def={DEFAULT_MACHINE.leaseMinutes}
          bounds={info.leaseBounds}
          what="A claim in Building that nobody has heard from for longer than this can go back to Ready: the card says quiet, the task pane offers Release, and muster reclaim takes it."
          onSet={(v) => onSet("lease", v)}
        />
        <SpanSetting
          key={`live-${s.liveMinutes}`}
          label="LIVE window"
          minutes={s.liveMinutes}
          def={DEFAULT_MACHINE.liveMinutes}
          bounds={info.liveBounds}
          what="How recent a change counts as working now: the card's dot (a session's transcript) and the task pane's LIVE (the task's file)."
          onSet={(v) => onSet("live", v)}
        />
        {info.sessionStates && <p className="meta">{sessionStatesLine(info.sessionStates)}</p>}
        <AccountsSetting labels={s.claudeAccounts} onSet={(v) => onSet("claude-accounts", v)} />
        <p className="meta">Saved on this Mac, in <HomePath path={`${info.configDir}/settings.json`} home={info.home} />; the muster command reads the same file.</p>
      </section>
      <section>
        <h2>The helper</h2>
        {helperError && <p className="err" role="alert">{helperError}</p>}
        {helper === null && !helperError && <p className="none">Checking the accounts…</p>}
        {helper && !helper.claude && <p className="err">Couldn't find the <code>claude</code> command. The helper runs your own Claude Code: install it, then come back here.</p>}
        {helper?.claude && <AccountPicker info={helper} listed={s.claudeAccounts.length} onPicked={onHelper} onRefresh={onHelper} />}
        {helper && (
          <>
            <div className="look-colour" role="group" aria-label="Helper model">
              <span className="meta">Model</span>
              {helper.models.map((m) => (
                <Pill key={m} type="button" on={helper.model === m} aria-pressed={helper.model === m} onClick={() => onModel(m)}>
                  {MODEL_NAMES[m] ?? m}
                </Pill>
              ))}
            </div>
            <p className="meta">
              The model the helper files tasks with, from its next message: Sonnet unless you pick another. Haiku is quicker and costs less; Opus
              thinks hardest and costs most.
            </p>
          </>
        )}
      </section>
    </div>
  );
}

/** One span setting: its value and whether it's the default, what it changes, and a field to
 *  change it (30m, 2h, 1d), checked by the core against its bounds. */
export function SpanSetting({ label, minutes, def, bounds, what, onSet }: {
  label: string;
  minutes: number;
  def: number;
  bounds: [number, number];
  what: string;
  onSet: (value: string) => Promise<string | null>;
}) {
  const [text, setText] = useState(spanInput(minutes));
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const save = async (value: string) => {
    setBusy(true);
    const refused = await onSet(value);
    setBusy(false);
    setError(refused ?? "");
  };
  return (
    <div className="setting" role="group" aria-label={label}>
      <p className="setting-line">
        <b>{label}</b> <span className="setting-value">{spanText(minutes)}</span> {minutes === def && <span className="meta">default</span>}
      </p>
      <p className="meta">{what}</p>
      <form
        className="setting-edit"
        onSubmit={(e) => {
          e.preventDefault();
          void save(text);
        }}
      >
        <input value={text} onChange={(e) => setText(e.target.value)} size={7} maxLength={12} aria-label={`${label}, as 30m, 2h or 1d`} disabled={busy} />
        <Button type="submit" disabled={busy || text.trim() === spanInput(minutes)}>Save</Button>
        <Button type="button" disabled={busy || minutes === def} onClick={() => void save("")}>
          Back to {spanText(def)}
        </Button>
        <span className="meta">From {spanText(bounds[0])} to {spanText(bounds[1])}.</span>
      </form>
      {error && <p className="err" role="alert">{error}</p>}
    </div>
  );
}

/** The Claude accounts Muster reads (0099), with what's typed in the add field and the last
 *  refusal: the client's (accountsEdit) or the core's. One change saves at a time. */
function AccountsSetting({ labels, onSet }: { labels: string[]; onSet: (value: string) => Promise<string | null> }) {
  const [typed, setTyped] = useState("");
  const [error, setError] = useState("");
  const busy = useRef(false);
  const field = useRef<HTMLInputElement>(null);
  const change = async (edit: { add: string } | { remove: string }) => {
    if (busy.current) return;
    busy.current = true;
    const refused = await saveAccounts(labels, edit, onSet);
    busy.current = false;
    setError(refused ?? "");
    if (refused === null && "add" in edit) setTyped("");
  };
  return (
    <AccountsRow
      labels={labels}
      typed={typed}
      error={error}
      field={field}
      onTyped={setTyped}
      onAdd={() => void change({ add: typed })}
      onRemove={(l) => {
        // The ✕ goes with its chip: the keyboard carries on from the field.
        field.current?.focus();
        void change({ remove: l });
      }}
    />
  );
}

/** One change to the accounts list, saved: null when the core saved it, else the refusal to
 *  show, the client's (accountsEdit, never sent) or the core's. */
export async function saveAccounts(labels: string[], edit: { add: string } | { remove: string }, onSet: (value: string) => Promise<string | null>): Promise<string | null> {
  const next = accountsEdit(labels, edit);
  if ("error" in next) return next.error;
  return onSet(next.value);
}

/** The accounts row as drawn: a chip per label (`cc-mg`) with its ✕, or the warning that none is
 *  read, then the add field in place (↩ adds), the refusal where a screen reader hears it, and
 *  that ~/.claude is never read. */
export function AccountsRow({ labels, typed, error, field, onTyped, onAdd, onRemove }: {
  labels: string[];
  typed: string;
  error: string;
  field?: Ref<HTMLInputElement>;
  onTyped: (typed: string) => void;
  onAdd: () => void;
  onRemove: (label: string) => void;
}) {
  const w = ACCOUNTS_WORDS;
  return (
    <div className="setting" role="group" aria-label={w.group}>
      <div className="accounts-row">
        <b>{w.label}</b>
        {labels.length === 0 && <span className="accounts-none">{w.none}</span>}
        {labels.map((l) => (
          <span key={l} className="account-chip">
            {w.chip(l)}
            <button type="button" className="account-x" aria-label={w.remove(l)} title={w.remove(l)} onClick={() => onRemove(l)}>✕</button>
          </span>
        ))}
        <form
          className="account-form"
          onSubmit={(e) => {
            e.preventDefault();
            onAdd();
          }}
        >
          <label className="account-add"><span className="meta">{w.add}</span> <input ref={field} value={typed} onChange={(e) => onTyped(e.target.value)} placeholder="mg" size={10} maxLength={20} autoCapitalize="off" autoCorrect="off" spellCheck={false} /></label>
        </form>
      </div>
      <p className="err account-error" aria-live="polite">{error}</p>
      <p className="meta account-never">{w.never}</p>
    </div>
  );
}

/** About with the projects as the core lists them, asked again each time About opens. */
function About({ info }: { info: SettingsInfo }) {
  const [projects, setProjects] = useState<Project[] | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    let live = true;
    api.projects().then(
      (l) => live && setProjects(l),
      (e: unknown) => live && setError(String(e)),
    );
    return () => {
      live = false;
    };
  }, []);
  return <AboutView info={info} projects={projects} error={error} />;
}

/** One project in About's lists: its name and folder, and whether it's read. */
function AboutProject({ p, home }: { p: Project; home: string }) {
  const note = p.hidden ? " (hidden, not read)" : !p.ok ? " (not a queue any more)" : "";
  return (
    <li>
      {p.name}: <HomePath className="path" path={p.path} home={home} />{note}
    </li>
  );
}

/** About: the version, where this Mac's settings are kept, and every folder the app reads, with
 *  the queues it found on its own apart from the ones the owner added. `projects` is null while
 *  they load. */
export function AboutView({ info, projects, error = "" }: { info: SettingsInfo; projects: Project[] | null; error?: string }) {
  const found = projects?.filter((p) => p.source === "found") ?? [];
  const added = projects?.filter((p) => p.source === "added") ?? [];
  const where = `${homePath(info.base, info.home)}/<project>/tasks`;
  return (
    <div className="settings-sections">
      <section>
        <h2>Version</h2>
        <p>Muster {info.version}</p>
      </section>
      <section>
        <h2>Fonts</h2>
        <p>Persian text is drawn in Vazirmatn by Saber Rastikerdar, under the SIL Open Font License 1.1; the licence is in <code>Muster.app/Contents/Resources/fonts/Vazirmatn-OFL.txt</code>.</p>
      </section>
      <section>
        <h2>Settings on this Mac</h2>
        <p><HomePath className="path" path={info.configDir} home={info.home} /></p>
        <ul className="about-list">
          <li><code>projects.json</code>: the projects you added or hid, their labels, categories, colours, gates and Your check limits, and the sidebar's order</li>
          <li><code>settings.json</code>: the Claude accounts Muster reads, the claim lease, the LIVE window, whether notifications are on and whether the menu bar icon shows; the muster command reads it too</li>
          <li><code>notified.json</code>: what Muster won't notify you of again: what it notified, and what you'd already seen (waiting at start or when notifications were turned on, or shown in the window in front)</li>
          <li><code>helper/</code>: the helper's account, model and conversations</li>
        </ul>
        <p className="meta">Appearance, the display mode, text size, Group by, saved views and the pane's width are kept by the window itself, on this Mac.</p>
      </section>
      <section>
        <h2>What it reads</h2>
        {error && <p className="err" role="alert">Can't read the projects: {error}</p>}
        {projects === null && !error && <p className="none">Loading the projects…</p>}
        {projects && (
          <>
            <p>Found on their own ({where} and one level deeper){found.length === 0 && ": none."}</p>
            {found.length > 0 && (
              <ul className="about-list">
                {found.map((p) => <AboutProject key={p.key} p={p} home={info.home} />)}
              </ul>
            )}
            <p>Added by you{added.length === 0 && ": none. Add a queue kept anywhere else in Projects."}</p>
            {added.length > 0 && (
              <ul className="about-list">
                {added.map((p) => <AboutProject key={p.key} p={p} home={info.home} />)}
              </ul>
            )}
          </>
        )}
        <ul className="about-list">
          {info.transcripts.length === 0 && <li>{ACCOUNTS_WORDS.transcriptsNone(info.settings.claudeAccounts.length)}</li>}
          {info.transcripts.map((f) => (
            <li key={f}>{ACCOUNTS_WORDS.transcripts} <HomePath className="path" path={f} home={info.home} /></li>
          ))}
          {info.transcripts.length > 0 && <li>{ACCOUNTS_WORDS.transcriptsUnder} <HomePath className="path" path={info.transcriptsUnder} home={info.home} /></li>}
        </ul>
        <p className="meta">All of it is read on this Mac and nothing is sent anywhere. Only what you write to the helper goes out, to Anthropic, through your own Claude Code.</p>
      </section>
    </div>
  );
}
