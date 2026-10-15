//! The accounts Muster runs work on, each with what the owner says of it (task 0113): its
//! provider, its plan (free text, only shown), the roles and models it may serve, how many runs
//! it takes at once, the share of each window the owner keeps for themselves (the reserve),
//! whether it's paused, and the projects pinned to it (mu-122). Saved as `accounts.json` beside
//! `settings.json` in the app's config folder. A missing or broken file reads as one profile per
//! allowed Claude account (`accounts::allowed`) with the defaults; a field out of bounds reads as
//! its default, field by field, as `settings` does.
//!
//! Only an account on mu-99's allowlist can be added, and never the company `~/.claude`
//! (decision 0012). There's no balancing across accounts (decision 0013): a profile says what the
//! owner pinned, and its use is read, not guessed (`usage`).

use crate::accounts::{self, Account};
use crate::roles::breaks_line;
use crate::settings::{self, Settings};
use crate::usage::{self, Reading, Source, Window};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::Path;

/// The file's name, in the app's config folder.
pub const FILE: &str = "accounts.json";
/// Runs an account takes at once: the default and the most.
pub const CONCURRENCY: u32 = 2;
pub const CONCURRENCY_MAX: u32 = 8;
/// The owner's reserve, in percent of each window: the default and the most.
pub const RESERVE: u32 = 20;
pub const RESERVE_MAX: u32 = 90;
/// The longest plan, in characters.
pub const MAX_PLAN: usize = 80;
/// A role, model or project name's most characters, and the most of them in one list.
const MAX_NAME: usize = 40;
const MAX_NAMES: usize = 20;

/// Refused: the company account (decision 0012).
const COMPANY: &str =
    "the company account (~/.claude) is never added: Muster reads only the personal accounts (decision 0012)";

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Claude,
    Codex,
}

impl Provider {
    fn parse(s: &str) -> Result<Provider, String> {
        match s.trim() {
            "claude" | "" => Ok(Provider::Claude),
            "codex" => Ok(Provider::Codex),
            other => {
                let other: String = other.chars().take(24).collect();
                Err(format!("the provider is claude or codex, not {other:?}"))
            }
        }
    }

    /// As `--provider` takes it: claude, codex.
    fn id(self) -> &'static str {
        match self {
            Provider::Claude => "claude",
            Provider::Codex => "codex",
        }
    }

    /// The account as the owner names it: `cc-mg` (the launcher's name), `codex-x`.
    pub fn name(self, label: &str) -> String {
        match self {
            Provider::Claude => format!("cc-{label}"),
            Provider::Codex => format!("codex-{label}"),
        }
    }

    /// Its config folder's name under the home folder: `.claude-mg`, `.codex-x`.
    fn folder(self, label: &str) -> String {
        match self {
            Provider::Claude => format!(".claude-{label}"),
            Provider::Codex => format!(".codex-{label}"),
        }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub provider: Provider,
    /// `mg` for `~/.claude-mg` (`accounts::is_label`).
    pub label: String,
    /// The plan as the owner writes it ("Max 20x"), only shown; at most `MAX_PLAN` characters.
    pub plan: String,
    /// The roles and models it may serve.
    pub roles: Vec<String>,
    pub models: Vec<String>,
    /// Runs at once, 1 to `CONCURRENCY_MAX`.
    pub concurrency: u32,
    /// Percent of each window kept for the owner, 0 to `RESERVE_MAX`.
    pub reserve: u32,
    pub paused: bool,
    /// The projects pinned to it, by name (mu-122).
    pub projects: Vec<String>,
}

impl Profile {
    /// An account with the defaults.
    pub fn new(provider: Provider, label: &str) -> Profile {
        Profile {
            provider,
            label: label.to_string(),
            plan: String::new(),
            roles: vec![],
            models: vec![],
            concurrency: CONCURRENCY,
            reserve: RESERVE,
            paused: false,
            projects: vec![],
        }
    }

    pub fn name(&self) -> String {
        self.provider.name(&self.label)
    }
}

/// What the app's Settings › Accounts saves of one account: every field but its projects.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Edit {
    pub label: String,
    pub plan: String,
    pub roles: Vec<String>,
    pub models: Vec<String>,
    pub concurrency: u32,
    pub reserve: u32,
    pub paused: bool,
}

/// The provider and label in what the owner typed: `cc-mg` is Claude's `mg`, `codex-x` Codex's
/// `x`, and a bare `mg` names no provider.
fn label_of(typed: &str) -> (Option<Provider>, &str) {
    let t = typed.trim();
    if let Some(l) = t.strip_prefix("cc-") {
        (Some(Provider::Claude), l)
    } else if let Some(l) = t.strip_prefix("codex-") {
        (Some(Provider::Codex), l)
    } else {
        (None, t)
    }
}

/// One profile per allowed Claude account, with the defaults.
fn seed(allowed: &[Account]) -> Vec<Profile> {
    allowed
        .iter()
        .map(|a| Profile::new(Provider::Claude, a.label()))
        .collect()
}

/// A whole number within `min..=max`, or None.
fn bounded(v: Option<&Value>, min: u32, max: u32) -> Option<u32> {
    v?.as_u64()
        .filter(|n| (u64::from(min)..=u64::from(max)).contains(n))
        .map(|n| n as u32)
}

/// A plan as typed, trimmed: at most `MAX_PLAN` characters on one line.
fn check_plan(plan: &str) -> Result<String, String> {
    let p = plan.trim();
    if p.chars().count() > MAX_PLAN {
        return Err(format!("a plan is at most {MAX_PLAN} characters"));
    }
    if p.chars().any(breaks_line) {
        return Err("a plan is one line of text".into());
    }
    Ok(p.to_string())
}

/// A role, model or project name: trimmed, 1 to `MAX_NAME` characters, no comma and nothing that
/// breaks a line (`roles::breaks_line`).
fn name_ok(n: &str) -> bool {
    !n.is_empty() && n.chars().count() <= MAX_NAME && !n.chars().any(|c| c == ',' || breaks_line(c))
}

/// A list of names as typed (`what`: roles, models, projects): each trimmed and checked, once,
/// at most `MAX_NAMES`. Empty ones are skipped.
fn check_names<'a>(
    items: impl IntoIterator<Item = &'a str>,
    what: &str,
) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for n in items.into_iter().map(str::trim).filter(|n| !n.is_empty()) {
        if !name_ok(n) {
            let n: String = n.chars().take(24).collect();
            return Err(format!(
                "{n:?} isn't one of the {what}: at most {MAX_NAME} characters, no comma"
            ));
        }
        if !out.iter().any(|o| o == n) {
            out.push(n.to_string());
        }
    }
    if out.len() > MAX_NAMES {
        return Err(format!("at most {MAX_NAMES} {what}"));
    }
    Ok(out)
}

fn check_concurrency(n: u32) -> Result<u32, String> {
    if (1..=CONCURRENCY_MAX).contains(&n) {
        Ok(n)
    } else {
        Err(format!(
            "concurrency can be from 1 to {CONCURRENCY_MAX}, not {n}"
        ))
    }
}

fn check_reserve(n: u32) -> Result<u32, String> {
    if n <= RESERVE_MAX {
        Ok(n)
    } else {
        Err(format!(
            "the reserve can be from 0 to {RESERVE_MAX}%, not {n}%"
        ))
    }
}

/// A stored list of names: the good ones kept, once, at most `MAX_NAMES`; anything else is empty.
fn stored_names(v: Option<&Value>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for n in v.and_then(Value::as_array).into_iter().flatten() {
        if let Some(n) = n.as_str().map(str::trim).filter(|n| name_ok(n)) {
            if !out.iter().any(|o| o == n) && out.len() < MAX_NAMES {
                out.push(n.to_string());
            }
        }
    }
    out
}

/// A stored profile: None without a provider and a valid label; any other field out of bounds
/// is its default.
fn stored(v: &Value) -> Option<Profile> {
    let provider = Provider::parse(v.get("provider")?.as_str()?).ok()?;
    let label = v.get("label")?.as_str().filter(|l| accounts::is_label(l))?;
    let d = Profile::new(provider, label);
    Some(Profile {
        plan: v
            .get("plan")
            .and_then(Value::as_str)
            .and_then(|p| check_plan(p).ok())
            .unwrap_or(d.plan),
        roles: stored_names(v.get("roles")),
        models: stored_names(v.get("models")),
        concurrency: bounded(v.get("concurrency"), 1, CONCURRENCY_MAX).unwrap_or(d.concurrency),
        reserve: bounded(v.get("reserve"), 0, RESERVE_MAX).unwrap_or(d.reserve),
        paused: v.get("paused").and_then(Value::as_bool).unwrap_or(d.paused),
        projects: stored_names(v.get("projects")),
        ..d
    })
}

/// The profiles in `file`. Missing, broken or with no list of accounts: one per account in
/// `allowed`, with the defaults. An entry without a provider or a valid label, or a second one
/// of a provider and label already read, is dropped (counted on stderr, never echoed).
pub fn load(file: &Path, allowed: &[Account]) -> Vec<Profile> {
    let Some(items) = fs::read_to_string(file)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("accounts").and_then(Value::as_array).cloned())
    else {
        return seed(allowed);
    };
    let mut out: Vec<Profile> = Vec::new();
    let mut dropped = 0;
    for item in &items {
        match stored(item) {
            Some(p)
                if !out
                    .iter()
                    .any(|o| o.provider == p.provider && o.label == p.label) =>
            {
                out.push(p)
            }
            _ => dropped += 1,
        }
    }
    if dropped > 0 {
        eprintln!("muster: accounts.json: {dropped} entries aren't accounts and were dropped");
    }
    out
}

#[derive(Serialize)]
struct Stored<'a> {
    accounts: &'a [Profile],
}

fn save(file: &Path, profiles: &[Profile]) -> Result<(), String> {
    let text =
        serde_json::to_string_pretty(&Stored { accounts: profiles }).map_err(|e| e.to_string())?;
    crate::queue::write_atomic(file, &(text + "\n"))
}

/// Change accounts.json: load, `f`, save, holding an exclusive flock on `accounts.json.lock`
/// beside it the whole time, so the app and `muster accounts` never lose each other's edits.
/// Nothing is written when `f` refuses.
fn edit<T>(
    file: &Path,
    allowed: &[Account],
    f: impl FnOnce(&mut Vec<Profile>) -> Result<T, String>,
) -> Result<T, String> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(file.with_extension("json.lock"))
        .map_err(|e| e.to_string())?;
    lock.lock().map_err(|e| e.to_string())?;
    let result = (|| {
        let mut profiles = load(file, allowed);
        let out = f(&mut profiles)?;
        save(file, &profiles)?;
        Ok(out)
    })();
    let _ = lock.unlock();
    result
}

/// Add the account `typed` (`mg`, `cc-mg` or `codex-mg`) of `provider` (claude or codex; when
/// empty, the one `typed` names, else claude), with the defaults. Refused, with why: a provider
/// other than the one `typed` names, the company account (empty, `.claude`, or a folder that
/// leads into `~/.claude`), a label that isn't one, a Claude account off `settings`' allowlist,
/// a config folder (`~/.claude-<label>`, `~/.codex-<label>`) that isn't there or isn't a folder,
/// and an account already added. Claude and Codex accounts may share a label.
pub fn add(
    file: &Path,
    settings: &Settings,
    home: &Path,
    typed: &str,
    provider: &str,
) -> Result<Profile, String> {
    let (named, label) = label_of(typed);
    let provider = match (provider.trim(), named) {
        ("", Some(named)) => named,
        (given, _) => Provider::parse(given)?,
    };
    let bare = label.trim_start_matches("~/").trim_start_matches('.');
    if bare.is_empty() || bare == "claude" {
        return Err(COMPANY.into());
    }
    if !accounts::is_label(label) {
        let l: String = label.chars().take(24).collect();
        return Err(format!(
            "{l:?} isn't a valid account label: 1 to 12 lowercase letters and digits, like mg"
        ));
    }
    if let Some(n) = named.filter(|&n| n != provider) {
        let t: String = typed.trim().chars().take(24).collect();
        return Err(format!(
            "{t:?} is a {} account, not a {} one",
            n.id(),
            provider.id()
        ));
    }
    let name = provider.name(label);
    if provider == Provider::Claude && !settings.claude_accounts.iter().any(|l| l == label) {
        return Err(format!(
            "{name} isn't on the allowlist of Claude accounts (Settings): add it there first"
        ));
    }
    let folder = format!("~/{}", provider.folder(label));
    let dir = home.join(provider.folder(label));
    let real = fs::canonicalize(&dir)
        .map_err(|_| format!("{folder} isn't there: sign in to {name} first"))?;
    if accounts::is_company(&real, home) {
        return Err(format!(
            "{folder} leads into the company account (~/.claude): {COMPANY}"
        ));
    }
    if !real.is_dir() {
        return Err(format!("{folder} isn't a folder"));
    }
    let allowed = accounts::allowed(settings, home);
    edit(file, &allowed, |profiles| {
        if profiles
            .iter()
            .any(|p| p.provider == provider && p.label == label)
        {
            return Err(format!("{name} is already an account"));
        }
        let p = Profile::new(provider, label);
        profiles.push(p.clone());
        Ok(p)
    })
}

/// Where the profile `typed` names is: by its provider and label (`cc-mg`, `codex-mg`), or by
/// its label alone (`mg`) when only one provider has it.
fn position(profiles: &[Profile], typed: &str) -> Result<usize, String> {
    let (named, label) = label_of(typed);
    let t: String = typed.trim().chars().take(24).collect();
    let mut hits = profiles
        .iter()
        .enumerate()
        .filter(|(_, p)| p.label == label && named.is_none_or(|n| p.provider == n))
        .map(|(i, _)| i);
    match (hits.next(), hits.next()) {
        (Some(i), None) => Ok(i),
        (None, _) => Err(format!("no account {t:?} (see `muster accounts`)")),
        (Some(_), Some(_)) => Err(format!(
            "{t:?} is both cc-{label} and codex-{label}: say which"
        )),
    }
}

/// The profile `typed` names (`position`).
fn find<'a>(profiles: &'a mut [Profile], typed: &str) -> Result<&'a mut Profile, String> {
    let i = position(profiles, typed)?;
    Ok(&mut profiles[i])
}

/// Remove the account `typed` (`mg`, `cc-mg` or `codex-mg`), and a Claude account's readings
/// with it.
pub fn remove(file: &Path, allowed: &[Account], typed: &str) -> Result<Profile, String> {
    let p = edit(file, allowed, |profiles| {
        let i = position(profiles, typed)?;
        Ok(profiles.remove(i))
    })?;
    if p.provider == Provider::Claude {
        if let Some(dir) = file.parent() {
            let _ = fs::remove_file(usage::file(dir, &p.label));
        }
    }
    Ok(p)
}

/// Change one field of the account `typed`: `plan` (text, empty clears it), `roles`, `models`
/// or `projects` (names separated by commas, empty clears them), `concurrency` (1 to 8),
/// `reserve` (0 to 90, a `%` allowed) or `paused` (yes or no), each checked; and save.
pub fn set(
    file: &Path,
    allowed: &[Account],
    typed: &str,
    field: &str,
    value: &str,
) -> Result<Profile, String> {
    let number = |what: &str| {
        let v = value.trim().trim_end_matches('%').trim();
        v.parse::<u32>().map_err(|_| {
            let v: String = value.chars().take(24).collect();
            format!("{what} is a whole number, not {v:?}")
        })
    };
    let change: Box<dyn FnOnce(&mut Profile)> = match field {
        "plan" => {
            let plan = check_plan(value)?;
            Box::new(move |p| p.plan = plan)
        }
        "roles" | "models" | "projects" => {
            let names = check_names(value.split(','), field)?;
            let field = field.to_string();
            Box::new(move |p| match field.as_str() {
                "roles" => p.roles = names,
                "models" => p.models = names,
                _ => p.projects = names,
            })
        }
        "concurrency" => {
            let n = check_concurrency(number("concurrency")?)?;
            Box::new(move |p| p.concurrency = n)
        }
        "reserve" => {
            let n = check_reserve(number("the reserve")?)?;
            Box::new(move |p| p.reserve = n)
        }
        "paused" => {
            let paused = match value.trim() {
                "yes" | "on" | "true" => true,
                "no" | "off" | "false" | "" => false,
                other => {
                    let other: String = other.chars().take(24).collect();
                    return Err(format!("paused is yes or no, not {other:?}"));
                }
            };
            Box::new(move |p| p.paused = paused)
        }
        other => {
            let other: String = other.chars().take(24).collect();
            return Err(format!(
                "no account field named {other:?}: plan, roles, models, projects, concurrency, reserve or paused"
            ));
        }
    };
    edit(file, allowed, |profiles| {
        let p = find(profiles, typed)?;
        change(p);
        Ok(p.clone())
    })
}

/// Save Settings › Accounts' fields of one account, each checked first; its projects are kept.
pub fn save_edit(file: &Path, allowed: &[Account], e: &Edit) -> Result<Profile, String> {
    let plan = check_plan(&e.plan)?;
    let roles = check_names(e.roles.iter().map(String::as_str), "roles")?;
    let models = check_names(e.models.iter().map(String::as_str), "models")?;
    let concurrency = check_concurrency(e.concurrency)?;
    let reserve = check_reserve(e.reserve)?;
    edit(file, allowed, |profiles| {
        let p = find(profiles, &e.label)?;
        p.plan = plan;
        p.roles = roles;
        p.models = models;
        p.concurrency = concurrency;
        p.reserve = reserve;
        p.paused = e.paused;
        Ok(p.clone())
    })
}

/// A window as the header and `muster accounts --json` show it.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WindowView {
    /// As read: past 100 when use ran past the cap. The display clamps it.
    pub used_percentage: f64,
    pub resets_at: i64,
    /// It has reset since it was read.
    pub stale: bool,
    /// What's left above the reserve, in percent; below 0 is into the reserve.
    pub headroom: f64,
}

/// An account with its latest reading, as the header, Settings › Accounts and `muster accounts
/// --json` show it.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub provider: Provider,
    pub label: String,
    /// `cc-mg`.
    pub name: String,
    pub plan: String,
    pub roles: Vec<String>,
    pub models: Vec<String>,
    pub concurrency: u32,
    pub reserve: u32,
    pub paused: bool,
    pub projects: Vec<String>,
    pub five_hour: Option<WindowView>,
    pub seven_day: Option<WindowView>,
    /// When the latest reading was taken, in seconds since 1970; None without one.
    pub reading_at: Option<i64>,
    pub source: Option<Source>,
}

/// Every account, and the time the views were judged at (seconds since 1970).
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub accounts: Vec<AccountView>,
    pub now: i64,
}

fn window_view(w: Option<Window>, reserve: u32, now: i64) -> Option<WindowView> {
    w.map(|w| WindowView {
        used_percentage: w.used_percentage,
        resets_at: w.resets_at,
        stale: w.stale(now),
        headroom: w.headroom(reserve),
    })
}

/// `p` with its latest reading `r`, judged at `now`.
pub fn view(p: &Profile, r: Option<&Reading>, now: i64) -> AccountView {
    AccountView {
        provider: p.provider,
        label: p.label.clone(),
        name: p.name(),
        plan: p.plan.clone(),
        roles: p.roles.clone(),
        models: p.models.clone(),
        concurrency: p.concurrency,
        reserve: p.reserve,
        paused: p.paused,
        projects: p.projects.clone(),
        five_hour: window_view(r.and_then(|r| r.five_hour), p.reserve, now),
        seven_day: window_view(r.and_then(|r| r.seven_day), p.reserve, now),
        reading_at: r.map(|r| r.at),
        source: r.map(|r| r.source),
    }
}

/// The profiles in the app's config folder `dir`, each with its latest reading; a Codex
/// account has none yet (only Claude Code is read). The readings are held to their 7 days at
/// `now` first, and an account off `allowed` loses its own (`usage::prune`).
pub fn read(dir: &Path, allowed: &[Account], now: i64) -> Vec<(Profile, Option<Reading>)> {
    let keep: Vec<&str> = allowed.iter().map(Account::label).collect();
    usage::prune(dir, &keep, now);
    load(&dir.join(FILE), allowed)
        .into_iter()
        .map(|p| {
            let r = (p.provider == Provider::Claude)
                .then(|| usage::latest(dir, &p.label))
                .flatten();
            (p, r)
        })
        .collect()
}

/// `read`'s accounts as views, judged at `now`.
pub fn info_of(read: &[(Profile, Option<Reading>)], now: i64) -> Info {
    Info {
        accounts: read.iter().map(|(p, r)| view(p, r.as_ref(), now)).collect(),
        now,
    }
}

/// A percentage as `muster accounts` says it: whole, or to one decimal.
fn pct(x: f64) -> String {
    let r = (x * 10.0).round() / 10.0 + 0.0;
    if r.fract() == 0.0 {
        format!("{r:.0}%")
    } else {
        format!("{r:.1}%")
    }
}

/// `muster accounts`: each account, its windows' use, reset, headroom and staleness, its
/// reserve, concurrency and pause, and its reading's age and source. `local` turns seconds since
/// 1970 into local `YYYY-MM-DD HH:MM` (`usage::local_time`); a reset today shows only its time.
pub fn text(info: &Info, local: &dyn Fn(i64) -> Option<String>) -> String {
    if info.accounts.is_empty() {
        return "no accounts: muster accounts add <label> adds one\n".into();
    }
    let today = local(info.now).map(|t| t[..10].to_string());
    let when = |secs: i64| match local(secs) {
        Some(t) if Some(&t[..10]) == today.as_deref() => t[11..].to_string(),
        Some(t) => t,
        None => secs.to_string(),
    };
    let mut out = String::new();
    for a in &info.accounts {
        let plan = if a.plan.is_empty() {
            String::new()
        } else {
            format!(", {}", a.plan)
        };
        let provider = match a.provider {
            Provider::Claude => "claude",
            Provider::Codex => "codex",
        };
        out.push_str(&format!("{} ({provider}{plan})\n", a.name));
        match a.reading_at {
            None => out.push_str("  no reading yet\n"),
            Some(_) => {
                for (what, w) in [("5 h", &a.five_hour), ("7 d", &a.seven_day)] {
                    let line = match w {
                        None => "not in the latest reading".to_string(),
                        Some(w) if w.stale => format!(
                            "{} used, reset at {}, stale",
                            pct(w.used_percentage),
                            when(w.resets_at)
                        ),
                        Some(w) => {
                            let over = if w.used_percentage > 100.0 {
                                " (past the cap)"
                            } else {
                                ""
                            };
                            let into = if w.headroom < 0.0 {
                                " (into the reserve)"
                            } else {
                                ""
                            };
                            format!(
                                "{} used{over}, resets {}, headroom {}{into}",
                                pct(w.used_percentage),
                                when(w.resets_at),
                                pct(w.headroom)
                            )
                        }
                    };
                    out.push_str(&format!("  {what}: {line}\n"));
                }
            }
        }
        let mut lines = vec![format!(
            "reserve {}%, concurrency {}{}",
            a.reserve,
            a.concurrency,
            if a.paused { ", paused" } else { "" }
        )];
        if let (Some(at), Some(source)) = (a.reading_at, a.source) {
            let age = info.now - at;
            let age = if age < 60 {
                "just now".to_string()
            } else {
                format!("{} ago", settings::span_text(age as u64 / 60))
            };
            let from = match source {
                Source::Stream => "a run",
                Source::Statusline => "the status line",
            };
            lines[0].push_str(&format!("; read {age} from {from}"));
        }
        for (what, list) in [
            ("roles", &a.roles),
            ("models", &a.models),
            ("projects", &a.projects),
        ] {
            if !list.is_empty() {
                lines.push(format!("{what}: {}", list.join(", ")));
            }
        }
        for l in lines {
            out.push_str(&format!("  {l}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::tests::fake_home;
    use crate::usage::tests::{windows, NOW};
    use crate::usage::Windows;
    use std::os::unix::fs::symlink;

    fn file(d: &crate::testkit::TestDir) -> std::path::PathBuf {
        d.path().join("ai.sweetlemon.muster").join(FILE)
    }

    fn labels(ps: &[Profile]) -> Vec<String> {
        ps.iter().map(Profile::name).collect()
    }

    #[test]
    fn a_missing_or_broken_file_reads_as_the_allowed_accounts_with_the_defaults() {
        let home = fake_home();
        let allowed = accounts::allowed(&Settings::default(), home.path());
        let d = crate::testkit::tempdir().unwrap();
        let f = file(&d);
        let seeded = load(&f, &allowed);
        assert_eq!(labels(&seeded), ["cc-mg", "cc-hg"]);
        assert_eq!(seeded[0], Profile::new(Provider::Claude, "mg"));
        assert_eq!(
            (seeded[0].concurrency, seeded[0].reserve, seeded[0].paused),
            (2, 20, false)
        );
        assert!(!f.exists(), "reading writes nothing");
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        for broken in ["", "not json", "[1,2]", "{}", r#"{"accounts": "mg"}"#] {
            fs::write(&f, broken).unwrap();
            assert_eq!(load(&f, &allowed), seeded, "{broken}");
        }
        // An empty list is kept: the owner removed every account.
        fs::write(&f, r#"{"accounts": []}"#).unwrap();
        assert!(load(&f, &allowed).is_empty());
    }

    #[test]
    fn each_field_out_of_bounds_reads_as_its_default_and_bad_entries_are_dropped() {
        let d = crate::testkit::tempdir().unwrap();
        let f = file(&d);
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        let long = "x".repeat(81);
        fs::write(
            &f,
            serde_json::json!({"accounts": [
                {"provider": "claude", "label": "mg", "plan": "Max 20x", "roles": ["builder", "", 7, "builder", "a,b"],
                 "models": ["opus"], "concurrency": 8, "reserve": 0, "paused": true, "projects": ["muster"],
                 "unknown": "ignored"},
                {"provider": "claude", "label": "hg", "plan": long, "concurrency": 9, "reserve": 91,
                 "paused": "yes", "roles": "builder"},
                {"provider": "codex", "label": "x", "concurrency": 0, "reserve": -1},
                {"provider": "claude", "label": "mg"},
                {"provider": "codex", "label": "mg", "plan": "Pro\u{2028}x", "roles": ["a\u{202e}b", "ok"],
                 "models": ["\u{2066}x"], "projects": ["p\u{2029}q"]},
                {"provider": "gemini", "label": "g"},
                {"provider": "claude", "label": ".claude"},
                {"provider": "claude", "label": ""},
                {"label": "nolabel"},
                "mg",
                null
            ]})
            .to_string(),
        )
        .unwrap();
        let got = load(&f, &[]);
        assert_eq!(labels(&got), ["cc-mg", "cc-hg", "codex-x", "codex-mg"]);
        // A plan or name that breaks the line reads as the default; the good names stay.
        assert_eq!(
            got[3],
            Profile {
                roles: vec!["ok".into()],
                ..Profile::new(Provider::Codex, "mg")
            }
        );
        assert_eq!(
            got[0],
            Profile {
                provider: Provider::Claude,
                label: "mg".into(),
                plan: "Max 20x".into(),
                roles: vec!["builder".into()],
                models: vec!["opus".into()],
                concurrency: 8,
                reserve: 0,
                paused: true,
                projects: vec!["muster".into()],
            }
        );
        assert_eq!(got[1], Profile::new(Provider::Claude, "hg"));
        assert_eq!(got[2], Profile::new(Provider::Codex, "x"));
    }

    #[test]
    fn profiles_round_trip_through_the_file() {
        let home = fake_home();
        let h = home.path();
        let s = Settings::default();
        let allowed = accounts::allowed(&s, h);
        let d = crate::testkit::tempdir().unwrap();
        let f = file(&d);
        set(&f, &allowed, "cc-mg", "plan", " Max 20x ").unwrap();
        set(&f, &allowed, "mg", "roles", "builder, tester,,builder").unwrap();
        set(&f, &allowed, "mg", "models", "opus,sonnet").unwrap();
        set(&f, &allowed, "mg", "projects", "muster").unwrap();
        set(&f, &allowed, "mg", "concurrency", "3").unwrap();
        set(&f, &allowed, "mg", "reserve", "25%").unwrap();
        let p = set(&f, &allowed, "mg", "paused", "yes").unwrap();
        let want = Profile {
            provider: Provider::Claude,
            label: "mg".into(),
            plan: "Max 20x".into(),
            roles: vec!["builder".into(), "tester".into()],
            models: vec!["opus".into(), "sonnet".into()],
            concurrency: 3,
            reserve: 25,
            paused: true,
            projects: vec!["muster".into()],
        };
        assert_eq!(p, want);
        assert_eq!(
            load(&f, &[]),
            [want.clone(), Profile::new(Provider::Claude, "hg")]
        );
        let text = fs::read_to_string(&f).unwrap();
        assert!(text.contains("\"provider\": \"claude\""), "{text}");
        assert!(!text.contains("tmp"), "no folder is stored: {text}");
        // The app's save: every field but the projects.
        let e = Edit {
            label: "cc-mg".into(),
            plan: "".into(),
            roles: vec![" reviewer ".into()],
            models: vec![],
            concurrency: 1,
            reserve: 90,
            paused: false,
        };
        let saved = save_edit(&f, &allowed, &e).unwrap();
        assert_eq!(
            saved,
            Profile {
                plan: "".into(),
                roles: vec!["reviewer".into()],
                models: vec![],
                concurrency: 1,
                reserve: 90,
                paused: false,
                ..want
            }
        );
        assert_eq!(load(&f, &[])[0], saved);
        // Empty clears.
        assert!(set(&f, &allowed, "mg", "roles", "")
            .unwrap()
            .roles
            .is_empty());
        assert!(!set(&f, &allowed, "mg", "paused", "no").unwrap().paused);
    }

    #[test]
    fn set_and_save_refuse_out_of_bounds_and_write_nothing() {
        let home = fake_home();
        let allowed = accounts::allowed(&Settings::default(), home.path());
        let d = crate::testkit::tempdir().unwrap();
        let f = file(&d);
        set(&f, &allowed, "mg", "reserve", "30").unwrap();
        let before = fs::read_to_string(&f).unwrap();
        let long = "x".repeat(81);
        for (field, value, why) in [
            ("reserve", "91", "the reserve can be from 0 to 90%, not 91%"),
            ("reserve", "-1", "the reserve is a whole number, not \"-1\""),
            ("reserve", "lots", "the reserve is a whole number, not \"lots\""),
            ("concurrency", "0", "concurrency can be from 1 to 8, not 0"),
            ("concurrency", "9", "concurrency can be from 1 to 8, not 9"),
            ("concurrency", "2.5", "concurrency is a whole number, not \"2.5\""),
            ("plan", long.as_str(), "a plan is at most 80 characters"),
            ("plan", "two\nlines", "a plan is one line of text"),
            ("plan", "two\u{2028}lines", "a plan is one line of text"),
            ("plan", "two\u{2029}paragraphs", "a plan is one line of text"),
            ("plan", "Max\u{202e}x02", "a plan is one line of text"),
            ("plan", "Max\u{2066}x", "a plan is one line of text"),
            ("paused", "maybe", "paused is yes or no, not \"maybe\""),
            ("colour", "red", "no account field named \"colour\": plan, roles, models, projects, concurrency, reserve or paused"),
        ] {
            assert_eq!(set(&f, &allowed, "mg", field, value).unwrap_err(), why, "{field}");
        }
        let many: Vec<String> = (0..21).map(|i| format!("r{i}")).collect();
        assert_eq!(
            set(&f, &allowed, "mg", "roles", &many.join(",")).unwrap_err(),
            "at most 20 roles"
        );
        let long_name = "y".repeat(41);
        for bad in [
            long_name.as_str(),
            "op\u{202e}us",
            "a\u{2028}b",
            "a\u{2029}b",
            "\u{2066}x",
            "a\tb",
        ] {
            assert!(
                set(&f, &allowed, "mg", "models", bad)
                    .unwrap_err()
                    .contains("isn't one of the models"),
                "{bad:?}"
            );
            assert!(
                set(&f, &allowed, "mg", "roles", bad)
                    .unwrap_err()
                    .contains("isn't one of the roles"),
                "{bad:?}"
            );
        }
        assert_eq!(
            set(&f, &allowed, "zz", "reserve", "10").unwrap_err(),
            "no account \"zz\" (see `muster accounts`)"
        );
        let e = Edit {
            label: "mg".into(),
            plan: "".into(),
            roles: vec![],
            models: vec![],
            concurrency: 2,
            reserve: 95,
            paused: false,
        };
        assert!(save_edit(&f, &allowed, &e).is_err());
        assert!(save_edit(
            &f,
            &allowed,
            &Edit {
                reserve: 20,
                concurrency: 0,
                ..e.clone()
            }
        )
        .is_err());
        assert!(save_edit(
            &f,
            &allowed,
            &Edit {
                reserve: 20,
                label: "nope".into(),
                ..e
            }
        )
        .is_err());
        assert_eq!(
            fs::read_to_string(&f).unwrap(),
            before,
            "a refusal writes nothing"
        );
    }

    #[test]
    fn add_refuses_the_company_account_and_any_off_the_allowlist_with_why() {
        let home = fake_home();
        let h = home.path();
        let s = Settings::default();
        let d = crate::testkit::tempdir().unwrap();
        let f = file(&d);
        for typed in ["", " ", ".claude", "claude", "~/.claude", "cc-"] {
            assert_eq!(
                add(&f, &s, h, typed, "claude").unwrap_err(),
                COMPANY,
                "{typed:?}"
            );
            assert_eq!(
                add(&f, &s, h, typed, "codex").unwrap_err(),
                COMPANY,
                "{typed:?}"
            );
        }
        assert_eq!(
            add(&f, &s, h, "ih", "claude").unwrap_err(),
            "cc-ih isn't on the allowlist of Claude accounts (Settings): add it there first"
        );
        assert_eq!(
            add(&f, &s, h, "cc-x", "claude").unwrap_err(),
            "cc-x isn't on the allowlist of Claude accounts (Settings): add it there first"
        );
        assert_eq!(
            add(&f, &s, h, "../x", "claude").unwrap_err(),
            "\"../x\" isn't a valid account label: 1 to 12 lowercase letters and digits, like mg"
        );
        assert!(add(&f, &s, h, "MG", "claude")
            .unwrap_err()
            .contains("isn't a valid account label"));
        assert_eq!(
            add(&f, &s, h, "mg", "gemini").unwrap_err(),
            "the provider is claude or codex, not \"gemini\""
        );
        // On the list, but its folder leads into ~/.claude, or isn't there.
        let listed = Settings {
            claude_accounts: vec!["mg".into(), "hg".into(), "x".into(), "gone".into()],
            ..Settings::default()
        };
        symlink(h.join(".claude"), h.join(".claude-x")).unwrap();
        let e = add(&f, &listed, h, "x", "claude").unwrap_err();
        assert!(
            e.starts_with("~/.claude-x leads into the company account (~/.claude)"),
            "{e}"
        );
        assert_eq!(
            add(&f, &listed, h, "gone", "claude").unwrap_err(),
            "~/.claude-gone isn't there: sign in to cc-gone first"
        );
        // Codex: its own folder, never one inside ~/.claude.
        symlink(h.join(".claude/projects"), h.join(".codex-c")).unwrap();
        assert!(add(&f, &s, h, "c", "codex")
            .unwrap_err()
            .starts_with("~/.codex-c leads into the company account"));
        assert_eq!(
            add(&f, &s, h, "w", "codex").unwrap_err(),
            "~/.codex-w isn't there: sign in to codex-w first"
        );
        fs::write(h.join(".codex-file"), "").unwrap();
        assert_eq!(
            add(&f, &s, h, "file", "codex").unwrap_err(),
            "~/.codex-file isn't a folder"
        );
        assert!(!f.exists(), "nothing refused is written");
    }

    #[test]
    fn add_and_remove_an_allowed_account() {
        let home = fake_home();
        let h = home.path();
        let s = Settings::default();
        let allowed = accounts::allowed(&s, h);
        let d = crate::testkit::tempdir().unwrap();
        let f = file(&d);
        // Seeded with both; removing one, then adding it back.
        assert_eq!(remove(&f, &allowed, "cc-hg").unwrap().label, "hg");
        assert_eq!(labels(&load(&f, &allowed)), ["cc-mg"]);
        assert_eq!(
            add(&f, &s, h, "cc-mg", "claude").unwrap_err(),
            "cc-mg is already an account"
        );
        assert_eq!(
            add(&f, &s, h, "cc-hg", "").unwrap(),
            Profile::new(Provider::Claude, "hg")
        );
        fs::create_dir(h.join(".codex-w")).unwrap();
        assert_eq!(add(&f, &s, h, "w", "codex").unwrap().name(), "codex-w");
        assert_eq!(labels(&load(&f, &allowed)), ["cc-mg", "cc-hg", "codex-w"]);
        assert_eq!(
            remove(&f, &allowed, "nope").unwrap_err(),
            "no account \"nope\" (see `muster accounts`)"
        );
        remove(&f, &allowed, "mg").unwrap();
        remove(&f, &allowed, "hg").unwrap();
        remove(&f, &allowed, "codex-w").unwrap();
        assert!(load(&f, &allowed).is_empty(), "none left stays none");
    }

    fn reading(at: i64, source: Source) -> Reading {
        Reading::new("mg", windows(35.0, 104.0), at, source)
    }

    #[test]
    fn a_view_has_both_windows_their_staleness_and_headroom_in_the_apps_shape() {
        let p = Profile {
            reserve: 25,
            ..Profile::new(Provider::Claude, "mg")
        };
        let r = reading(NOW - 120, Source::Statusline);
        let v = view(&p, Some(&r), NOW);
        assert_eq!(
            v.five_hour,
            Some(WindowView {
                used_percentage: 35.0,
                resets_at: 1_792_035_600,
                stale: false,
                headroom: 40.0
            })
        );
        assert_eq!(v.seven_day.as_ref().unwrap().headroom, -25.0);
        // Past its reset: stale.
        let later = view(&p, Some(&r), 1_792_035_600);
        assert!(later.five_hour.unwrap().stale);
        assert!(!later.seven_day.unwrap().stale);
        let none = view(&p, None, NOW);
        assert_eq!(
            (none.five_hour, none.reading_at, none.source),
            (None, None, None)
        );
        let json = serde_json::to_value(info_of(&[(p, Some(r))], NOW)).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"accounts": [{
                "provider": "claude", "label": "mg", "name": "cc-mg", "plan": "", "roles": [],
                "models": [], "concurrency": 2, "reserve": 25, "paused": false, "projects": [],
                "fiveHour": {"usedPercentage": 35.0, "resetsAt": 1_792_035_600_i64, "stale": false, "headroom": 40.0},
                "sevenDay": {"usedPercentage": 104.0, "resetsAt": 1_792_346_400_i64, "stale": false, "headroom": -25.0},
                "readingAt": NOW - 120, "source": "statusline"
            }], "now": NOW})
        );
        let empty =
            serde_json::to_value(view(&Profile::new(Provider::Codex, "w"), None, NOW)).unwrap();
        assert_eq!(empty["fiveHour"], Value::Null);
        assert_eq!(empty["source"], Value::Null);
        assert_eq!(empty["provider"], "codex");
    }

    #[test]
    fn read_takes_each_accounts_latest_reading() {
        let home = fake_home();
        let allowed = accounts::allowed(&Settings::default(), home.path());
        let d = crate::testkit::tempdir().unwrap();
        usage::record(d.path(), &reading(NOW - 10, Source::Stream), NOW).unwrap();
        let got = read(d.path(), &allowed, NOW);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].1.as_ref().unwrap().at, NOW - 10);
        assert_eq!(got[1].1, None);
        // Past 7 days, the reading goes on read, though nothing records for mg again.
        let got = read(d.path(), &allowed, NOW - 10 + usage::KEEP_SECS + 1);
        assert_eq!(got[0].1, None);
        assert!(!usage::file(d.path(), "mg").exists());
        // Off the allowlist: its readings go too.
        usage::record(d.path(), &reading(NOW, Source::Stream), NOW).unwrap();
        let hg_only: Vec<Account> = allowed
            .iter()
            .filter(|a| a.label() == "hg")
            .cloned()
            .collect();
        read(d.path(), &hg_only, NOW);
        assert!(!usage::file(d.path(), "mg").exists());
    }

    #[test]
    fn providers_are_told_apart_by_their_prefix_and_may_share_a_label() {
        let home = fake_home();
        let h = home.path();
        let s = Settings::default();
        let allowed = accounts::allowed(&s, h);
        let d = crate::testkit::tempdir().unwrap();
        let f = file(&d);
        let dir = f.parent().unwrap();
        usage::record(dir, &reading(NOW, Source::Stream), NOW).unwrap();
        // Only cc-mg and cc-hg: a Codex name isn't one of them.
        assert_eq!(
            remove(&f, &allowed, "codex-mg").unwrap_err(),
            "no account \"codex-mg\" (see `muster accounts`)"
        );
        assert!(set(&f, &allowed, "codex-hg", "paused", "yes").is_err());
        assert!(!load(&f, &allowed)[1].paused);
        // Codex's mg beside Claude's.
        fs::create_dir(h.join(".codex-mg")).unwrap();
        assert_eq!(add(&f, &s, h, "mg", "codex").unwrap().name(), "codex-mg");
        assert_eq!(labels(&load(&f, &allowed)), ["cc-mg", "cc-hg", "codex-mg"]);
        assert_eq!(
            add(&f, &s, h, "codex-mg", "").unwrap_err(),
            "codex-mg is already an account"
        );
        assert_eq!(
            add(&f, &s, h, "codex-mg", "claude").unwrap_err(),
            "\"codex-mg\" is a codex account, not a claude one"
        );
        assert_eq!(
            set(&f, &allowed, "mg", "plan", "x").unwrap_err(),
            "\"mg\" is both cc-mg and codex-mg: say which"
        );
        set(&f, &allowed, "codex-mg", "plan", "Pro").unwrap();
        let got = load(&f, &allowed);
        assert_eq!((got[0].plan.as_str(), got[2].plan.as_str()), ("", "Pro"));
        // Claude's readings are cc-mg's, never codex-mg's.
        let read = read(dir, &allowed, NOW);
        assert!(read[0].1.is_some());
        assert_eq!((read[2].0.name(), &read[2].1), ("codex-mg".into(), &None));
        // Removing codex-mg leaves cc-mg and its readings; removing cc-mg takes its readings.
        assert_eq!(remove(&f, &allowed, "codex-mg").unwrap().name(), "codex-mg");
        assert!(usage::file(dir, "mg").exists());
        assert_eq!(remove(&f, &allowed, "mg").unwrap().name(), "cc-mg");
        assert!(
            !usage::file(dir, "mg").exists(),
            "a removed account's readings go with it"
        );
        assert_eq!(labels(&load(&f, &allowed)), ["cc-hg"]);
    }

    #[test]
    fn muster_accounts_says_each_windows_use_reset_headroom_and_the_readings_age() {
        let local = |secs: i64| usage::local_time(secs, Some("UTC"));
        let mg = Profile {
            plan: "Max 20x".into(),
            roles: vec!["builder".into()],
            ..Profile::new(Provider::Claude, "mg")
        };
        let hg = Profile {
            paused: true,
            ..Profile::new(Provider::Claude, "hg")
        };
        let r = reading(NOW - 180, Source::Statusline);
        // NOW is 2026-10-15 02:06 UTC: the 5-hour reset is today, the weekly one isn't.
        let text = super::text(
            &info_of(&[(mg.clone(), Some(r.clone())), (hg, None)], NOW),
            &local,
        );
        assert_eq!(
            text,
            "cc-mg (claude, Max 20x)\n  5 h: 35% used, resets 03:40, headroom 45%\n  7 d: 104% used (past the cap), resets 2026-10-18 18:00, headroom -20% (into the reserve)\n  reserve 20%, concurrency 2; read 3 min ago from the status line\n  roles: builder\ncc-hg (claude)\n  no reading yet\n  reserve 20%, concurrency 2, paused\n"
        );
        // After the 5-hour reset: stale; a run's reading of a moment ago.
        let after = 1_792_035_600 + 30;
        let r = Reading::new(
            "mg",
            Windows {
                seven_day: None,
                ..windows(41.5, 0.0)
            },
            after - 20,
            Source::Stream,
        );
        let text = super::text(&info_of(&[(mg, Some(r))], after), &local);
        assert!(
            text.contains(
                "  5 h: 41.5% used, reset at 03:40, stale\n  7 d: not in the latest reading\n"
            ),
            "{text}"
        );
        assert!(text.contains("; read just now from a run\n"), "{text}");
        assert_eq!(
            super::text(&info_of(&[], NOW), &local),
            "no accounts: muster accounts add <label> adds one\n"
        );
    }
}
