import { type ReactNode, type Ref, useEffect, useRef, useState } from "react";
import { api, type HelperInfo } from "./api";
import { useAccounts } from "./AccountMeter";
import { type AccountsInfo, type AccountView, addEdit, type ProfileForm, profileEdit, profileForm, type ProfileSave, type Provider, readingText, WINDOWS, windowLine } from "./accountsData";
import Appearance from "./Appearance";
import { DisplayModes } from "./Display";
import { AccountPicker } from "./Helper";
import { HelpButton } from "./HelpButton";
import { HomePath } from "./HomePath";
import { calendarIn, CALENDARS, digitsIn, DIGITS, dirOf, followLocale, followNumbering, LOCALES, NATIVE_NAMES, tl, tn, Trans, useLocale, useT, type AnyLocale, type Calendar, type Digits, type Locale, type TFn } from "./i18n";
import Notifications from "./Notifications";
import Projects from "./Projects";
import Roles from "./Roles";
import { sessionStatesLine } from "./sessionState";
import { homePath } from "./lib";
import type { DisplayMode } from "./modes";
import { ACCOUNTS_WORDS, accountsEdit, DEFAULT_MACHINE, numberingSetting, SETTINGS_GROUPS, type SettingsGroup, type SettingsInfo, spanInput, spanText } from "./settingsData";
import type { Project, QueueInfo } from "./types";
import { Button, Pill, Segment, Segmented } from "./ui";

/** Settings (0036): one place for Appearance, Display, Projects, Agents, Accounts, Notifications and About. A view in the
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
  const t = useT("shell");
  const g = SETTINGS_GROUPS.find((x) => x.id === group) ?? SETTINGS_GROUPS[0];
  return (
    <div className="settings">
      <div className="settings-side">
        <Language setting={info ? info.settings.lang : null} onInfo={setInfo} />
        <nav className="settings-groups" aria-label={t("settings.groupsAria")}>
          {SETTINGS_GROUPS.map((x) => (
            <button key={x.id} type="button" className={x.id === g.id ? "settings-group on" : "settings-group"} aria-current={x.id === g.id ? "page" : undefined} onClick={() => onGroup(x.id)}>
              {x.label}<small>{x.holds}</small>
            </button>
          ))}
        </nav>
      </div>
      <section className="settings-body" aria-label={g.label}>
        <header className="settings-head">
          <h1>{g.label} <HelpButton topic={g.topic} /></h1>
          <p className="meta">{g.holds}</p>
        </header>
        {error && <p className="err" role="alert">{t("settings.cantRead", { error })}</p>}
        {g.id === "appearance" && <Appearance />}
        {g.id === "display" && <DisplayModes mode={mode} onMode={onMode} />}
        {g.id === "projects" && <Projects queues={queues} home={info?.home ?? ""} onChanged={onChanged} />}
        {g.id === "agents" && <Agents info={info} queues={queues} onInfo={setInfo} onChanged={onChanged} />}
        {g.id === "accounts" && <Accounts />}
        {g.id === "notifications" && <Notifications info={info} onInfo={setInfo} />}
        {g.id === "about" && (info ? <About info={info} /> : !error && <p className="none">{t("common:loading")}</p>)}
      </section>
    </div>
  );
}

/** Save the language setting (`code`, or "" to follow macOS) and switch the window to the one
 *  the core says is in effect, without waiting for its event (mu-146). Resolves to the settings
 *  now in force. */
export async function pickLanguage(code: Locale | ""): Promise<SettingsInfo> {
  const info = await api.setSetting("lang", code);
  await followLocale(info.locale?.effective ?? code);
  return info;
}

/** Settings' Language (mu-146), at the top of Settings whatever group is open: a chip per
 *  language, each name in its own script and never translated, so a person who can't read the
 *  language shown can still find theirs. The chip of the language in effect is on; while the
 *  setting is unset (`setting` undefined) that's macOS's, and a line says so. `null`: the
 *  settings haven't come yet, so it says nothing either way. */
function Language({ setting, onInfo }: { setting: string | undefined | null; onInfo: (i: SettingsInfo) => void }) {
  const [error, setError] = useState("");
  const [numberingError, setNumberingError] = useState("");
  const shown = useLocale();
  const pick = (name: "digits" | "calendar", value: string) =>
    void pickNumbering(name, value, shown).then(
      (i) => {
        setNumberingError("");
        onInfo(i);
      },
      (e: unknown) => setNumberingError(String(e)),
    );
  return (
    <>
      <LanguageView
        shown={shown}
        following={setting === undefined}
        error={error}
        onPick={(code) =>
          void pickLanguage(code).then(
            (i) => {
              setError("");
              onInfo(i);
            },
            (e: unknown) => setError(String(e)),
          )
        }
      />
      <NumberingView digits={digitsIn(shown)} calendar={calendarIn(shown)} error={numberingError} onDigits={(d) => pick("digits", d)} onCalendar={(c) => pick("calendar", c)} />
    </>
  );
}

/** Save Digits or Calendar (mu-151) and show it at once. The value the language gives by itself
 *  saves as "" (`numberingSetting`), so it keeps following the language. */
export async function pickNumbering(name: "digits" | "calendar", value: string, shown: AnyLocale): Promise<SettingsInfo> {
  const info = await api.setSetting(name, numberingSetting(name, value, shown));
  followNumbering(info.settings);
  return info;
}

/** The chip rows' text: the digits as themselves (never translated, like the language names),
 *  the calendars in the language shown. */
const DIGIT_SAMPLES: Record<Digits, string> = { persian: "۱۲۳", latin: "123" };

/** Settings › Language's Digits and Calendar (mu-151), under the language chips and always
 *  shown: two quiet chip rows, each with its group label; the chip in effect is on. */
export function NumberingView({ digits, calendar, error = "", onDigits, onCalendar }: { digits: Digits; calendar: Calendar; error?: string; onDigits: (d: Digits) => void; onCalendar: (c: Calendar) => void }) {
  const t = useT("shell");
  return (
    <section className="settings-lang settings-numbering" aria-label={`${t("language.digits")} · ${t("language.calendar")}`}>
      <h3>{t("language.digits")}</h3>
      <Segmented aria-label={t("language.digits")}>
        {DIGITS.map((d) => (
          <Segment key={d} type="button" lang={d === "persian" ? "fa" : "en"} dir="ltr" active={d === digits} onClick={() => onDigits(d)}>
            {DIGIT_SAMPLES[d]}
          </Segment>
        ))}
      </Segmented>
      <h3>{t("language.calendar")}</h3>
      <Segmented aria-label={t("language.calendar")}>
        {CALENDARS.map((c) => (
          <Segment key={c} type="button" active={c === calendar} onClick={() => onCalendar(c)}>
            {t(`language.${c}`)}
          </Segment>
        ))}
      </Segmented>
      <p className="meta">{t("language.numberingMeta")}</p>
      {error && <p className="err" role="alert">{t("language.failedNumbering", { error })}</p>}
    </section>
  );
}

/** Language as drawn: `shown` is the language the window shows, `following` whether that's
 *  macOS's (no setting saved). Every chip saves its language when clicked, the one on too: `shown`
 *  changes only once a language's files are in, so a change of mind while one loads must still
 *  go through (saving and switching are idempotent, and the last pick wins). */
export function LanguageView({ shown, following, error = "", onPick }: { shown: string; following: boolean; error?: string; onPick: (code: Locale) => void }) {
  const t = useT("shell");
  return (
    <section className="settings-lang" aria-label={t("language.label")}>
      <h2>{t("language.label")}</h2>
      <Segmented aria-label={t("language.label")}>
        {LOCALES.map((l) => (
          <Segment key={l} type="button" lang={l} dir={dirOf(l)} active={l === shown} onClick={() => onPick(l)}>
            {NATIVE_NAMES[l]}
          </Segment>
        ))}
      </Segmented>
      <p className="meta">{t("language.meta")}</p>
      {following && <p className="meta">{t("language.followsMac")}</p>}
      {error && <p className="err" role="alert">{t("language.failed", { error })}</p>}
    </section>
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
  if (!info) return <p className="none">{tl("loading")}</p>;
  return (
    <AgentsView
      info={info}
      helper={helper}
      helperError={error}
      onHelper={() => setAsked((n) => n + 1)}
      roles={<Roles onChanged={onChanged} />}
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
export function AgentsView({ info, helper, helperError = "", onHelper, onModel, onSet, roles }: {
  info: SettingsInfo;
  /** Null while the core checks the accounts. */
  helper: HelperInfo | null;
  helperError?: string;
  onHelper: () => void;
  onModel: (model: string) => void;
  /** Resolves to null when saved, else the core's refusal to show. */
  onSet: (name: AgentsSettingName, value: string) => Promise<string | null>;
  /** Roles (0112), the last section: its own data, so it's given drawn. */
  roles?: ReactNode;
}) {
  // Not useT: the view is a plain function its tests walk; Settings, which holds it, renders it
  // again on a language switch.
  const t: TFn<"shell"> = (key, params) => tn("shell", key, params);
  const s = info.settings;
  return (
    <div className="settings-sections">
      <section>
        <h2>{t("agents.claims")}</h2>
        <SpanSetting
          key={`lease-${s.leaseMinutes}`}
          label={t("agents.lease")}
          minutes={s.leaseMinutes}
          def={DEFAULT_MACHINE.leaseMinutes}
          bounds={info.leaseBounds}
          what={t("agents.leaseWhat")}
          onSet={(v) => onSet("lease", v)}
        />
        <SpanSetting
          key={`live-${s.liveMinutes}`}
          label={t("agents.live")}
          minutes={s.liveMinutes}
          def={DEFAULT_MACHINE.liveMinutes}
          bounds={info.liveBounds}
          what={t("agents.liveWhat")}
          onSet={(v) => onSet("live", v)}
        />
        {info.sessionStates && <p className="meta">{sessionStatesLine(info.sessionStates)}</p>}
        <AccountsSetting labels={s.claudeAccounts} onSet={(v) => onSet("claude-accounts", v)} />
        <p className="meta"><Trans ns="shell" k="agents.saved" components={{ path: <HomePath path={`${info.configDir}/settings.json`} home={info.home} /> }} /></p>
      </section>
      <section>
        <h2>{t("agents.helper")}</h2>
        {helperError && <p className="err" role="alert">{helperError}</p>}
        {helper === null && !helperError && <p className="none">{t("agents.checking")}</p>}
        {helper && !helper.claude && <p className="err"><Trans ns="shell" k="agents.noClaude" components={{ code: <code /> }} /></p>}
        {helper?.claude && <AccountPicker info={helper} listed={s.claudeAccounts.length} onPicked={onHelper} onRefresh={onHelper} />}
        {helper && (
          <>
            <div className="look-colour" role="group" aria-label={t("agents.modelAria")}>
              <span className="meta">{t("agents.model")}</span>
              {helper.models.map((m) => (
                <Pill key={m} type="button" on={helper.model === m} aria-pressed={helper.model === m} onClick={() => onModel(m)}>
                  {MODEL_NAMES[m] ?? m}
                </Pill>
              ))}
            </div>
            <p className="meta">{t("agents.modelMeta")}</p>
          </>
        )}
      </section>
      {roles}
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
  const t = useT("shell");
  return (
    <div className="setting" role="group" aria-label={label}>
      <p className="setting-line">
        <b>{label}</b> <span className="setting-value">{spanText(minutes)}</span> {minutes === def && <span className="meta">{t("agents.default")}</span>}
      </p>
      <p className="meta">{what}</p>
      <form
        className="setting-edit"
        onSubmit={(e) => {
          e.preventDefault();
          void save(text);
        }}
      >
        <input value={text} onChange={(e) => setText(e.target.value)} size={7} maxLength={12} aria-label={t("agents.spanAria", { label })} disabled={busy} />
        <Button type="submit" disabled={busy || text.trim() === spanInput(minutes)}>{t("common:save")}</Button>
        <Button type="button" disabled={busy || minutes === def} onClick={() => void save("")}>
          {t("agents.backTo", { span: spanText(def) })}
        </Button>
        <span className="meta">{t("agents.bounds", { from: spanText(bounds[0]), to: spanText(bounds[1]) })}</span>
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

/** What Settings › Accounts asks of the core, and `after`, which reads the accounts again once
 *  one of them is saved (the core's accounts-changed comes too; this doesn't wait for it). */
export type AccountCalls = {
  add: (label: string, provider: Provider) => Promise<void>;
  remove: (label: string) => Promise<void>;
  save: (profile: ProfileSave) => Promise<void>;
  after: () => void;
};

/** Settings › Accounts (0113) with its data: the same accounts the header's meter shows. */
function Accounts() {
  const { info, error, reload } = useAccounts();
  return <AccountsView info={info} error={error} calls={{ add: api.accountAdd, remove: api.accountRemove, save: api.accountSave, after: reload }} />;
}

/** Saves a profile from its form: null when the core saved it, else the refusal to show, the
 *  client's (profileEdit, never sent) or the core's. */
export async function saveProfile(label: string, form: ProfileForm, calls: Pick<AccountCalls, "save" | "after">): Promise<string | null> {
  const edit = profileEdit(label, form);
  if ("error" in edit) return edit.error;
  try {
    await calls.save(edit.profile);
  } catch (e) {
    return String(e);
  }
  calls.after();
  return null;
}

/** Adds an account by what was typed (`mg` or `cc-mg`): null when added, else why not. */
export async function addAccount(typed: string, provider: Provider, calls: Pick<AccountCalls, "add" | "after">): Promise<string | null> {
  const edit = addEdit(typed);
  if ("error" in edit) return edit.error;
  try {
    await calls.add(edit.label, provider);
  } catch (e) {
    return String(e);
  }
  calls.after();
  return null;
}

/** Removes an account's profile: null when removed, else the core's refusal. */
export async function removeAccount(label: string, calls: Pick<AccountCalls, "remove" | "after">): Promise<string | null> {
  try {
    await calls.remove(label);
  } catch (e) {
    return String(e);
  }
  calls.after();
  return null;
}

/** What a profile's form starts from: a new reading leaves the typing alone; a saved change to
 *  the profile, here or by `muster accounts set`, starts the form afresh. */
export const profileKey = (a: AccountView) => JSON.stringify([a.provider, a.label, a.plan, a.roles, a.models, a.concurrency, a.reserve, a.paused]);

// Product names, the same in every language.
const PROVIDERS: { id: Provider; label: string }[] = [
  { id: "claude", label: "Claude" },
  { id: "codex", label: "Codex" },
];
const providerName = (p: Provider) => PROVIDERS.find((x) => x.id === p)?.label ?? p;

/** Accounts as drawn: the quiet line on balancing, then each account with its reading and its
 *  fields inline, then the add form. `info` is null while the accounts load. */
export function AccountsView({ info, error, calls }: { info: AccountsInfo | null; error: string; calls: AccountCalls }) {
  // A removed account's section goes: the keyboard carries on from the add form's label.
  const addField = useRef<HTMLInputElement>(null);
  const t = useT("shell");
  return (
    <div className="settings-sections">
      <section>
        <p className="meta">{t("profiles.noBalancing")}</p>
        <p className="meta">{t("profiles.intro")}</p>
        {error && <p className="err" role="alert">{t("profiles.cantRead", { error })}</p>}
        {!info && !error && <p className="none">{t("common:loading")}</p>}
        {info?.accounts.length === 0 && <p className="none">{t("profiles.empty")}</p>}
      </section>
      {info?.accounts.map((a) => (
        <section key={`${a.provider}/${a.label}`} aria-label={a.name}>
          <h2>
            {a.name} <span className="meta">{[providerName(a.provider), a.plan].filter(Boolean).join(" · ")}</span>
          </h2>
          {WINDOWS.map((w) => (
            <p key={w.key} className={a[w.key]?.stale ? "profile-reading stale" : "profile-reading"}>{windowLine(w.short, a[w.key], info.now)}</p>
          ))}
          <p className="meta">{readingText(a, info.now)}</p>
          {a.projects.length > 0 && <p className="meta">{t("profiles.pinned", { projects: a.projects.join(t("common:listSep")) })}</p>}
          <Profile key={profileKey(a)} a={a} calls={calls} onRemoved={() => addField.current?.focus()} />
        </section>
      ))}
      <section>
        <h2>{t("accounts.add")}</h2>
        <AddAccount calls={calls} field={addField} />
      </section>
    </div>
  );
}

/** One account's fields with what's typed, its last refusal, and Remove's ask in place. It's
 *  named by provider and label (`cc-mg`, `codex-mg`), so the core finds this one. Remove… puts
 *  the focus on Keep, Keep puts it back on Remove…, and a removal hands it to `onRemoved`. */
function Profile({ a, calls, onRemoved }: { a: AccountView; calls: AccountCalls; onRemoved: () => void }) {
  const [form, setForm] = useState(() => profileForm(a));
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [removing, setRemoving] = useState(false);
  const removeButton = useRef<HTMLButtonElement>(null);
  const keepButton = useRef<HTMLButtonElement>(null);
  const backToRemove = useRef(false);
  useEffect(() => {
    if (removing) keepButton.current?.focus();
    else if (backToRemove.current) {
      backToRemove.current = false;
      removeButton.current?.focus();
    }
  }, [removing]);
  const run = async (call: () => Promise<string | null>) => {
    setBusy(true);
    const refused = await call();
    setBusy(false);
    setError(refused ?? "");
    return refused;
  };
  return (
    <ProfileEditor
      a={a}
      form={form}
      error={error}
      busy={busy}
      removing={removing}
      removeRef={removeButton}
      keepRef={keepButton}
      onForm={setForm}
      onSave={() => void run(() => saveProfile(a.name, form, calls))}
      onRemoving={(asking) => {
        backToRemove.current = !asking;
        setRemoving(asking);
      }}
      onRemove={() =>
        void run(() => removeAccount(a.name, calls)).then((refused) => {
          if (refused === null) onRemoved();
        })
      }
    />
  );
}

/** An account's fields as drawn, all inline: plan, reserve, concurrency, roles, models and
 *  paused, then Save, and Remove, which asks in place before it goes. The refusal under them. */
export function ProfileEditor({ a, form, error, busy, removing, removeRef, keepRef, onForm, onSave, onRemoving, onRemove }: {
  a: AccountView;
  form: ProfileForm;
  error: string;
  busy: boolean;
  removing: boolean;
  removeRef?: Ref<HTMLButtonElement>;
  keepRef?: Ref<HTMLButtonElement>;
  onForm: (f: ProfileForm) => void;
  onSave: () => void;
  onRemoving: (asking: boolean) => void;
  onRemove: () => void;
}) {
  // Not useT: its tests call it as a plain function; AccountsView, which holds it, renders it
  // again on a language switch.
  const t: TFn<"shell"> = (key, params) => tn("shell", key, params);
  const unchanged = JSON.stringify(form) === JSON.stringify(profileForm(a));
  const field = (label: string, key: "plan" | "reserve" | "concurrency" | "roles" | "models", size: number, more: { maxLength?: number; placeholder?: string; numeric?: boolean } = {}) => (
    <label className="profile-field">
      <span className="meta">{label}</span>{" "}
      <input
        value={form[key]}
        onChange={(e) => onForm({ ...form, [key]: e.target.value })}
        size={size}
        maxLength={more.maxLength}
        placeholder={more.placeholder}
        inputMode={more.numeric ? "numeric" : undefined}
        disabled={busy}
        spellCheck={false}
      />
    </label>
  );
  return (
    <>
      <form
        className="profile-form"
        aria-label={t("profiles.formAria", { name: a.name })}
        onSubmit={(e) => {
          e.preventDefault();
          onSave();
        }}
      >
        {field(t("profiles.plan"), "plan", 14, { maxLength: 80 })}
        {field(t("profiles.reserve"), "reserve", 3, { maxLength: 2, numeric: true })}
        {field(t("profiles.concurrency"), "concurrency", 2, { maxLength: 1, numeric: true })}
        {field(t("profiles.roles"), "roles", 16, { placeholder: "builder, tester" /* i18n-ignore: role names, as typed in every language */ })}
        {field(t("profiles.models"), "models", 16, { placeholder: "opus, sonnet" /* i18n-ignore: model names */ })}
        <label className="profile-field"><input type="checkbox" checked={form.paused} onChange={(e) => onForm({ ...form, paused: e.target.checked })} disabled={busy} /> {t("profiles.paused")}</label>
        <Button type="submit" disabled={busy || unchanged}>{t("common:save")}</Button>
        {removing ? (
          <span className="profile-remove">
            {t("profiles.removeAsk", { name: a.name })}{" "}
            <Button type="button" disabled={busy} onClick={onRemove}>{t("common:remove")}</Button>{" "}
            <Button type="button" ref={keepRef} onClick={() => onRemoving(false)}>{t("profiles.keep")}</Button>
          </span>
        ) : (
          <Button type="button" ref={removeRef} disabled={busy} onClick={() => onRemoving(true)}>{t("profiles.removeAsking")}</Button>
        )}
      </form>
      <p className="err profile-error" aria-live="polite">{error}</p>
    </>
  );
}

/** The add form with what's typed, the provider picked and the last refusal. */
function AddAccount({ calls, field }: { calls: AccountCalls; field: Ref<HTMLInputElement> }) {
  const [typed, setTyped] = useState("");
  const [provider, setProvider] = useState<Provider>("claude");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const add = async () => {
    setBusy(true);
    const refused = await addAccount(typed, provider, calls);
    setBusy(false);
    setError(refused ?? "");
    if (refused === null) setTyped("");
  };
  return <AccountAdd typed={typed} provider={provider} error={error} busy={busy} field={field} onTyped={setTyped} onProvider={setProvider} onAdd={() => void add()} />;
}

/** Adding an account as drawn: its label, Claude or Codex, Add (↩ too), the refusal in place
 *  where a screen reader hears it, and which folders can be added. */
export function AccountAdd({ typed, provider, error, busy, field, onTyped, onProvider, onAdd }: {
  typed: string;
  provider: Provider;
  error: string;
  busy: boolean;
  field?: Ref<HTMLInputElement>;
  onTyped: (typed: string) => void;
  onProvider: (p: Provider) => void;
  onAdd: () => void;
}) {
  const t = useT("shell");
  return (
    <>
      <form
        className="profile-form"
        onSubmit={(e) => {
          e.preventDefault();
          onAdd();
        }}
      >
        <label className="profile-field">
          <span className="meta">{t("profiles.label")}</span>{" "}
          <input ref={field} value={typed} onChange={(e) => onTyped(e.target.value)} placeholder="mg" size={10} maxLength={20} autoCapitalize="off" autoCorrect="off" spellCheck={false} disabled={busy} />
        </label>
        <span className="look-colour" role="group" aria-label={t("profiles.provider")}>
          {PROVIDERS.map((p) => (
            <Pill key={p.id} type="button" aria-pressed={provider === p.id} on={provider === p.id} onClick={() => onProvider(p.id)}>
              {p.label}
            </Pill>
          ))}
        </span>
        <Button type="submit" disabled={busy}>{t("profiles.add")}</Button>
      </form>
      <p className="err profile-error" aria-live="polite">{error}</p>
      <p className="meta">{t("profiles.which")}</p>
    </>
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
  const path = <HomePath className="path" path={p.path} home={home} />;
  const k = p.hidden ? "about.projectHidden" : !p.ok ? "about.projectGone" : "about.project";
  return (
    <li>
      <Trans ns="shell" k={k} params={{ name: p.name }} components={{ path }} />
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
  const t = useT("shell");
  const code = { code: <code /> };
  return (
    <div className="settings-sections">
      <section>
        <h2>{t("about.version")}</h2>
        <p>Muster {info.version}</p>
      </section>
      <section>
        <h2>{t("about.fonts")}</h2>
        <p><Trans ns="shell" k="about.fontsLicence" params={{ file: "Muster.app/Contents/Resources/fonts/Vazirmatn-OFL.txt" }} components={code} /></p>
      </section>
      <section>
        <h2>{t("about.settings")}</h2>
        <p><HomePath className="path" path={info.configDir} home={info.home} /></p>
        <ul className="about-list">
          <li><Trans ns="shell" k="about.projectsJson" params={{ file: "projects.json" }} components={code} /></li>
          <li><Trans ns="shell" k="about.settingsJson" params={{ file: "settings.json" }} components={code} /></li>
          <li><Trans ns="shell" k="about.notifiedJson" params={{ file: "notified.json" }} components={code} /></li>
          <li><Trans ns="shell" k="about.helperDir" params={{ file: "helper/" }} components={code} /></li>
          <li><Trans ns="shell" k="about.accountsJson" params={{ file: "accounts.json" }} components={code} /></li>
          <li><Trans ns="shell" k="about.usageDir" params={{ file: "usage/" }} components={code} /></li>
        </ul>
        <p className="meta">{t("about.window")}</p>
      </section>
      <section>
        <h2>{t("about.reads")}</h2>
        {error && <p className="err" role="alert">{t("about.cantRead", { error })}</p>}
        {projects === null && !error && <p className="none">{t("about.loading")}</p>}
        {projects && (
          <>
            <p>{found.length === 0 ? t("about.foundNone", { where }) : t("about.found", { where })}</p>
            {found.length > 0 && (
              <ul className="about-list">
                {found.map((p) => <AboutProject key={p.key} p={p} home={info.home} />)}
              </ul>
            )}
            <p>{added.length === 0 ? t("about.addedNone") : t("about.added")}</p>
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
            <li key={f}><Trans ns="shell" k="about.transcripts" components={{ path: <HomePath className="path" path={f} home={info.home} /> }} /></li>
          ))}
          {info.transcripts.length > 0 && <li><Trans ns="shell" k="about.transcriptsUnder" components={{ path: <HomePath className="path" path={info.transcriptsUnder} home={info.home} /> }} /></li>}
        </ul>
        <p className="meta">{t("about.private")}</p>
      </section>
    </div>
  );
}
