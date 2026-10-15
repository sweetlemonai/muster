//! Claude Code's own word on its sessions (task 0048): `claude agents --json --all`, run once per
//! account Muster reads (0099, `accounts::allowed`: never `~/.claude`, the company account)
//! whenever the open work has a holder stamped with one of them (0030), off the main thread,
//! cached, at most every `EVERY_SECS`. A state is matched to its holder by session id in every
//! account read, not only the one its stamp names (0182): a session resumed on another account
//! (cc-mg out of its limit, picked up on cc-hg with the same id and claim) still reads as itself. It tells a session that's working from one waiting for input (a permission
//! prompt) and one that has ended, which heartbeats alone can't.
//!
//! The format is a research preview, so it's read defensively: anything that isn't a JSON array
//! of entries, a missing `claude`, a non-zero exit, a run over `TIMEOUT` or output over
//! `MAX_OUTPUT` is "not available" with the reason, and the cards read as before (heartbeats and
//! the live dot). Only each session's short id and its state are kept: `cwd` and `name` can carry
//! user names and are never read into Muster (the privacy baseline).
//!
//! The source is `Fetch`, one function: when Drover supplies session states (Drover 0018), it
//! swaps here, and the matching (`apply`) and the window stay.

use crate::accounts::Account;
use crate::queue::Task;
use crate::session;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How long one `claude agents` run may take.
pub const TIMEOUT: Duration = Duration::from_secs(10);
/// The most output read from one run.
pub const MAX_OUTPUT: usize = 1024 * 1024;
/// The app reads at most this often: with the window's polls while it's visible, and with the Dock
/// badge's own reads (0006) while it's hidden.
pub const EVERY_SECS: u64 = 15;
/// A read older than this (two rounds and a timeout) isn't shown: after the window was hidden,
/// the cards wait for a fresh one rather than show what was true then.
pub const MAX_AGE_SECS: u64 = 2 * EVERY_SECS + 10;

/// What Claude Code says a session is doing, in Muster's words.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SessionState {
    Working,
    /// Waiting on the owner in Claude Code: a permission prompt or a question.
    NeedsInput,
    /// Done, completed or ready for review: the session isn't working any more.
    Ended,
    /// Not read, not matched, or a state Muster doesn't know: the heartbeat lines decide.
    #[default]
    Unknown,
}

impl SessionState {
    /// A `state` as Claude Code writes it (`working`, `blocked`, `done`, `failed`, `stopped` in
    /// 2.1.x), or as agent view names its groups (Needs input, Completed, Ready for review).
    /// Anything else, `failed` and `stopped` included, is unknown: a failed turn or a paused
    /// session may still be holding the task, so the heartbeats judge it.
    pub fn from_state(raw: &str) -> SessionState {
        match normal(raw).as_str() {
            "working" => SessionState::Working,
            "blocked" | "needs_input" => SessionState::NeedsInput,
            "done" | "completed" | "ready_for_review" => SessionState::Ended,
            _ => SessionState::Unknown,
        }
    }

    /// An interactive session's `status` (it has no `state`): `busy` is working, `waiting` is
    /// waiting for input, `idle` (its turn is over, no prompt open) is unknown.
    pub fn from_status(raw: &str) -> SessionState {
        match normal(raw).as_str() {
            "busy" => SessionState::Working,
            "waiting" => SessionState::NeedsInput,
            _ => SessionState::Unknown,
        }
    }
}

/// Lowercase, with spaces and dashes as underscores.
fn normal(raw: &str) -> String {
    raw.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

/// One session from the output: its short id and state. Nothing else is kept.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub id: String,
    pub state: SessionState,
}

/// An entry as read: only the fields Muster uses, so `cwd`, `name` and anything new are never
/// held. An entry whose `sessionId` is missing or not a string fails this and is skipped.
#[derive(Deserialize)]
struct Raw {
    #[serde(rename = "sessionId")]
    session_id: String,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    status: Option<String>,
    /// Any JSON: only a number orders entries; anything else is as if missing.
    #[serde(default, rename = "startedAt")]
    started_at: Option<serde_json::Value>,
}

/// The sessions in one run's output, or why it can't be read. Entries without a session id (or
/// one that isn't a session id), or with neither a `state` nor an interactive session's `status`,
/// are skipped; a state Muster doesn't know reads as unknown, and so does a background session
/// whose state says working while its process sits idle (it may be ready for review). A session
/// listed twice (a resumed id) is its latest entry: the later `startedAt`, else the later listed.
pub fn parse(out: &[u8]) -> Result<Vec<Entry>, String> {
    if out.len() > MAX_OUTPUT {
        return Err("its output is over 1 MB".into());
    }
    let value: serde_json::Value =
        serde_json::from_slice(out).map_err(|_| "its output isn't JSON".to_string())?;
    let serde_json::Value::Array(items) = value else {
        return Err("its output isn't a JSON array".into());
    };
    let mut entries: Vec<(Entry, f64)> = Vec::new();
    for item in items {
        let Ok(raw) = serde_json::from_value::<Raw>(item) else {
            continue;
        };
        let Some(id) = session::short_id(&raw.session_id) else {
            continue;
        };
        let state = match (&raw.state, &raw.status) {
            (Some(s), Some(st))
                if SessionState::from_state(s) == SessionState::Working && normal(st) == "idle" =>
            {
                SessionState::Unknown
            }
            (Some(s), _) => SessionState::from_state(s),
            (None, Some(s)) => SessionState::from_status(s),
            (None, None) => continue,
        };
        let started = raw
            .started_at
            .as_ref()
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(f64::MIN);
        match entries.iter_mut().find(|(e, _)| e.id == id) {
            Some((e, at)) if started >= *at => {
                e.state = state;
                *at = started;
            }
            Some(_) => {}
            None => entries.push((Entry { id, state }, started)),
        }
    }
    Ok(entries.into_iter().map(|(e, _)| e).collect())
}

/// Set for every run: without them `claude agents --json` reports its use to Anthropic (it tried
/// api.anthropic.com on each run, 2.1.296), and Muster runs it every 15 s. Nothing is sent to
/// third parties (the privacy baseline).
pub const QUIET_ENV: [(&str, &str); 3] = [
    ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1"),
    ("DISABLE_TELEMETRY", "1"),
    ("DO_NOT_TRACK", "1"),
];

/// Run `claude agents --json --all` as an argument array (never a shell), for the account whose
/// config folder is `config_dir`, always set as `CLAUDE_CONFIG_DIR` (unset, it would read
/// `~/.claude`, the company account: 0099), with the router and token variables taken out
/// (`helper::STRIP_ENV`) and its usage reporting off (`QUIET_ENV`). Its output, at most
/// `MAX_OUTPUT` + 1 bytes, or why not: not installed, a non-zero exit, or still running after
/// `timeout` (then it's stopped).
pub fn run(claude: &Path, config_dir: &Path, timeout: Duration) -> Result<Vec<u8>, String> {
    run_with(claude, config_dir, timeout, &RealClock(Instant::now()))
}

/// Time as `run` waits on it, so a test can stand in for it instead of depending on how fast the
/// machine is.
trait Clock {
    /// How long since the run started.
    fn elapsed(&self) -> Duration;
    /// Wait a moment before looking at the process again.
    fn pause(&self);
}

struct RealClock(Instant);

impl Clock for RealClock {
    fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }

    fn pause(&self) {
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// How waiting on a process ended.
enum Wait<T> {
    Done(T),
    TimedOut,
    Failed(std::io::ErrorKind),
}

/// Look at the process (`poll`: Some once it's finished) until it is, or `timeout` has gone by on
/// `clock`. A finish seen at the deadline still counts: it's looked at before the time is.
fn wait<T>(
    mut poll: impl FnMut() -> std::io::Result<Option<T>>,
    clock: &impl Clock,
    timeout: Duration,
) -> Wait<T> {
    loop {
        match poll() {
            Ok(Some(done)) => return Wait::Done(done),
            Ok(None) if clock.elapsed() < timeout => clock.pause(),
            Ok(None) => return Wait::TimedOut,
            Err(e) => return Wait::Failed(e.kind()),
        }
    }
}

fn run_with(
    claude: &Path,
    config_dir: &Path,
    timeout: Duration,
    clock: &impl Clock,
) -> Result<Vec<u8>, String> {
    let mut cmd = Command::new(claude);
    cmd.args(["agents", "--json", "--all"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // A group of its own, so a timeout stops anything it started too (`stop`).
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    for k in crate::helper::STRIP_ENV {
        cmd.env_remove(k);
    }
    cmd.envs(QUIET_ENV);
    cmd.env("CLAUDE_CONFIG_DIR", config_dir);
    let mut child = cmd.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => "claude isn't installed".to_string(),
        _ => format!("couldn't run claude ({})", e.kind()),
    })?;
    // Read on a thread of its own, so a full pipe never stalls the wait below; past the cap it
    // stops reading and closes the pipe.
    let (tx, rx) = mpsc::channel();
    if let Some(stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let cap = u64::try_from(MAX_OUTPUT + 1).unwrap_or(u64::MAX);
            let _ = stdout.take(cap).read_to_end(&mut buf);
            let _ = tx.send(buf);
        });
    }
    let status = match wait(|| child.try_wait(), clock, timeout) {
        Wait::Done(status) => status,
        Wait::TimedOut => {
            stop(&mut child);
            return Err(format!("claude agents didn't finish within {timeout:?}"));
        }
        Wait::Failed(kind) => {
            stop(&mut child);
            return Err(format!("couldn't wait for claude ({kind})"));
        }
    };
    let left = timeout.saturating_sub(clock.elapsed());
    let out = rx
        .recv_timeout(left.max(Duration::from_millis(200)))
        .unwrap_or_default();
    if out.len() > MAX_OUTPUT {
        return Err("its output is over 1 MB".into());
    }
    if !status.success() {
        return Err(match status.code() {
            Some(code) => format!("claude agents exited with code {code}"),
            None => "claude agents was stopped by a signal".into(),
        });
    }
    Ok(out)
}

/// Stop a run and everything it started: its process group (it leads one, `run`), through
/// `/bin/kill` with an argument array since the standard library has no group kill, then the
/// process itself in case that failed.
fn stop(child: &mut std::process::Child) {
    let _ = Command::new("/bin/kill")
        .args(["-KILL", "--", &format!("-{}", child.id())])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
    let _ = child.wait();
}

/// Where the states come from: one allowed account's raw output, or why there's none. It's given
/// an `Account`, which only `accounts::allowed` makes, so it can't be asked for another.
pub type Fetch = Arc<dyn Fn(&Account) -> Result<Vec<u8>, String> + Send + Sync>;

/// How a refresh is run: on a thread of its own in the app (`on_thread`), in place in tests.
pub type Spawn = fn(Box<dyn FnOnce() + Send>);

/// The app's `Spawn`: off the thread that polls.
pub fn on_thread(job: Box<dyn FnOnce() + Send>) {
    std::thread::spawn(job);
}

/// The real source: the owner's `claude` (found where the helper finds it), once per allowed
/// account, `CLAUDE_CONFIG_DIR=~/.claude-<label>` as `session.rs` reads it back.
pub fn claude_fetch(home: PathBuf) -> Fetch {
    Arc::new(move |account: &Account| {
        let claude = crate::helper::find_claude(&home).ok_or("claude isn't installed")?;
        if !account.dir().is_dir() {
            return Err(format!("there's no ~/{} folder", account.folder_name()));
        }
        run(&claude, account.dir(), TIMEOUT)
    })
}

/// What one round of reading found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    /// (account label, short id) → state, from every account that could be read.
    pub states: HashMap<(String, String), SessionState>,
    /// Each account read, in order: how many sessions it listed, or why it couldn't be read.
    pub accounts: Vec<(String, Result<usize, String>)>,
    /// The accounts this round is of, by label, in the allowlist's order: each one asked, whether
    /// its read has landed yet or not (`known`).
    pub wanted: Vec<String>,
}

/// How much a state says the session runs on that account now: a session runs on one account at
/// a time, so where it's working or waiting is where it is; where it ended is where it was.
fn liveness(s: SessionState) -> u8 {
    match s {
        SessionState::Working | SessionState::NeedsInput => 2,
        SessionState::Unknown => 1,
        SessionState::Ended => 0,
    }
}

impl Snapshot {
    /// Session `id`'s state and the account it was found on, matched by id in every account of
    /// the snapshot that `allowed` still has (0182), whichever one its stamp names
    /// (`stamped_on`). Listed on more than one, the liveliest state wins (`liveness`: resumed on
    /// cc-hg after cc-mg's limit, it works on hg whatever mg's list still says), then the stamp's
    /// account, then the first label in order. None when no allowed account lists it.
    pub fn find(
        &self,
        id: &str,
        stamped_on: &str,
        allowed: &[Account],
    ) -> Option<(SessionState, String)> {
        self.states
            .iter()
            .filter(|((label, i), _)| i == id && crate::accounts::is_allowed(label, allowed))
            .max_by(|((a, _), sa), ((b, _), sb)| {
                liveness(**sa)
                    .cmp(&liveness(**sb))
                    .then((a == stamped_on).cmp(&(b == stamped_on)))
                    .then(b.cmp(a))
            })
            .map(|((label, _), s)| (*s, label.clone()))
    }
}

/// The stamp of the holder a session state can apply to: someone other than the owner holding
/// the task in Building or Testing, stamped with a session (0030). Not while the task is paused
/// (0045): its session was told to stop, so it's neither working nor waiting on the owner for it.
fn stamp_of(t: &Task) -> Option<session::Stamp> {
    let works = matches!(t.stage.as_str(), "building" | "testing") && t.paused.is_empty();
    if !works || session::is_owner(t.holder()) {
        return None;
    }
    session::split(t.holder()).1
}

/// The account labels named in a stamp that `stamp_of` takes, once, sorted: "" for a session
/// that ran without `CLAUDE_CONFIG_DIR` (bare `claude`, the company account).
fn stamped(tasks: &[Task]) -> Vec<String> {
    let mut out: Vec<String> = tasks
        .iter()
        .filter_map(stamp_of)
        .map(|s| s.account)
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The accounts to read: every one of `allowed`, in its order, once some task in Building or
/// Testing has a holder (`stamp_of`) stamped with one of them; empty otherwise, and then nothing
/// runs. Every allowed account, not only the stamp's (0182): a session keeps its id and claim
/// when it's resumed on another account, so its state may be on any of them. A stamp with no
/// account, or one not on the list, never causes a run (0099).
pub fn wanted(tasks: &[Task], allowed: &[Account]) -> Vec<Account> {
    if stamped(tasks)
        .iter()
        .any(|l| crate::accounts::is_allowed(l, allowed))
    {
        allowed.to_vec()
    } else {
        Vec::new()
    }
}

/// Read every account in `accounts` through `fetch`, each once.
pub fn read_all(fetch: &Fetch, accounts: &[Account]) -> Snapshot {
    let mut snap = Snapshot {
        wanted: accounts.iter().map(|a| a.label().to_string()).collect(),
        ..Snapshot::default()
    };
    for account in accounts {
        let label = account.label().to_string();
        let read = fetch(account).and_then(|out| parse(&out));
        let result = read.map(|entries| {
            let n = entries.len();
            for e in entries {
                snap.states.insert((label.clone(), e.id), e.state);
            }
            n
        });
        snap.accounts.push((label, result));
    }
    snap
}

/// What one read says of the Claude accounts (0069): those it wanted, and those whose sessions it
/// knows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Known {
    /// The accounts the open work's stamps name and every one the round asks (0182: each allowed
    /// one), read or not (one Muster doesn't read never is).
    pub wanted: HashSet<String>,
    /// The accounts read.
    pub accounts: HashSet<String>,
}

impl Known {
    /// Does it know the sessions of `account`?
    pub fn knows(&self, account: &str) -> bool {
        self.accounts.contains(account)
    }
}

/// What `snap` says of the accounts `tasks` want: each one their stamps name and each one the
/// round asks (`Snapshot::wanted`: every allowed one, 0182), and each one it read. None is read
/// before the first read lands, nor one that couldn't be read, so a session waiting for input
/// there isn't known to wait yet.
pub fn known(tasks: &[Task], snap: &Snapshot) -> Known {
    Known {
        wanted: stamped(tasks)
            .into_iter()
            .chain(snap.wanted.iter().cloned())
            .collect(),
        accounts: snap
            .accounts
            .iter()
            .filter(|(_, r)| r.is_ok())
            .map(|(a, _)| a.clone())
            .collect(),
    }
}

/// The Claude account of the session a state of `t` would come from (`stamp_of`), if any.
pub fn account_of(t: &Task) -> Option<String> {
    stamp_of(t).map(|s| s.account)
}

/// Who holds what, by session id: the claims (Ready and Building) and the Testing holds (`held-by`
/// there is the builder that passed it).
#[derive(Debug, Default)]
pub struct Holds {
    claims: HashMap<String, usize>,
    testing: HashMap<String, usize>,
}

/// What every session holds among `tasks`: every queue Muster reads, so a session holding a task
/// in two projects is seen holding two (the app and `muster list --json` count alike). A paused
/// task (0045) isn't counted: its session was told to stop, so its next claim gets the state.
pub fn holds<'a>(tasks: impl IntoIterator<Item = &'a Task>) -> Holds {
    let mut h = Holds::default();
    // A paused task's session was told to stop (0045): it isn't what the session is doing.
    for t in tasks.into_iter().filter(|t| t.paused.is_empty()) {
        let Some(stamp) = session::split(t.holder()).1 else {
            continue;
        };
        let count = match t.stage.as_str() {
            "ready" | "building" => &mut h.claims,
            "testing" => &mut h.testing,
            _ => continue,
        };
        *count.entry(stamp.id).or_default() += 1;
    }
    h
}

/// Give each task its session's state, counting holds among the tasks themselves.
pub fn apply(tasks: &mut [Task], snap: &Snapshot, allowed: &[Account]) {
    let h = holds(tasks.iter());
    apply_with(tasks, &h, snap, allowed);
}

/// Give each task its session's state. One applies when the holder's stamp (`stamp_of`) names
/// the session, and the session holds nothing else (`holds`): in Building, no
/// other claim; in Testing, no claim and no other Testing hold. A coordinator's subagents share
/// its id, and a builder that passed a task to Testing and claimed the next is working on that
/// one, so its state says nothing about the others. Ended shows only in Building: in Testing the
/// holder is the builder that passed it, whose session ending is the normal course. Every other
/// task is unknown, so its card reads as before. A session waiting for input waits on the owner,
/// so the task counts in the needs-you count. Only a stamp whose account is one of `allowed`
/// (0099) gets a state, and only from an account of `allowed`, whatever `snap` holds: one read
/// before the account left the list says nothing now. The state is matched by session id in
/// every allowed account (`Snapshot::find`, 0182), and the account it was found on is kept as
/// the task's `session_account`.
pub fn apply_with(tasks: &mut [Task], holds: &Holds, snap: &Snapshot, allowed: &[Account]) {
    let n = |m: &HashMap<String, usize>, id: &str| m.get(id).copied().unwrap_or(0);
    for t in tasks.iter_mut() {
        let building = t.stage == "building";
        let found = match stamp_of(t) {
            Some(s)
                if crate::accounts::is_allowed(&s.account, allowed)
                    && ((building && n(&holds.claims, &s.id) == 1)
                        || (!building
                            && n(&holds.claims, &s.id) == 0
                            && n(&holds.testing, &s.id) == 1)) =>
            {
                snap.find(&s.id, &s.account, allowed)
            }
            _ => None,
        };
        (t.session_state, t.session_account) = match found {
            Some((SessionState::Ended, _)) if !building => (SessionState::Unknown, String::new()),
            Some((state, account)) => (state, account),
            None => (SessionState::Unknown, String::new()),
        };
        if t.session_state == SessionState::NeedsInput {
            t.waiting_for_owner = true;
        }
    }
}

#[derive(Default)]
struct Inner {
    snapshot: Snapshot,
    /// When the read that made `snapshot` started.
    snapshot_at: u64,
    /// When the last read started (the poll's clock), if one has.
    last: Option<u64>,
    running: bool,
    /// The accounts the latest poll wanted, by label.
    wanted: Vec<String>,
    /// Whether the latest poll saw a stamped holder at all, of an account Muster reads or not.
    stamped: bool,
}

impl Inner {
    /// The snapshot, unless it's older than `MAX_AGE_SECS` at `now` (or from a clock ahead of it),
    /// with only the accounts labelled in `wanted`: a read made before an account left the list,
    /// or one still running for it, never hands its states on.
    fn fresh(&self, now: u64, wanted: &[String]) -> Snapshot {
        let age = now.checked_sub(self.snapshot_at);
        if !age.is_some_and(|a| a <= MAX_AGE_SECS) {
            return Snapshot {
                wanted: wanted.to_vec(),
                ..Snapshot::default()
            };
        }
        let keep = |label: &String| wanted.contains(label);
        Snapshot {
            states: self
                .snapshot
                .states
                .iter()
                .filter(|((label, _), _)| keep(label))
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
            accounts: self
                .snapshot
                .accounts
                .iter()
                .filter(|(label, _)| keep(label))
                .cloned()
                .collect(),
            wanted: wanted.to_vec(),
        }
    }
}

/// The app's reader: polled with every state read, it starts a fresh read when one is due and
/// hands back the latest it has, so a slow `claude` never holds the window's poll up.
pub struct Monitor {
    fetch: Fetch,
    spawn: Spawn,
    inner: Mutex<Inner>,
}

/// A poisoned lock only means a read panicked mid-update; the data is a cache, so use it.
fn lock(m: &Mutex<Inner>) -> MutexGuard<'_, Inner> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Marks the read done however it ends, a panic included, so the next one can start.
struct Running<'a>(&'a Mutex<Inner>);

impl Drop for Running<'_> {
    fn drop(&mut self) {
        lock(self.0).running = false;
    }
}

impl Monitor {
    pub fn new(fetch: Fetch, spawn: Spawn) -> Arc<Monitor> {
        Arc::new(Monitor {
            fetch,
            spawn,
            inner: Mutex::new(Inner::default()),
        })
    }

    /// The app's: the owner's `claude` under `home`, each read on a thread of its own.
    pub fn app(home: PathBuf) -> Arc<Monitor> {
        Monitor::new(claude_fetch(home), on_thread)
    }

    /// Called with every state read at `now` (seconds): when some task has a holder in Building
    /// or Testing stamped with one of the `allowed` accounts, no read is running and the last
    /// started `EVERY_SECS` or more ago (or the clock went back), a read of the accounts they name
    /// starts through `spawn`: of every allowed account (`wanted`). Returns the latest snapshot, of
    /// the accounts wanted now only; an empty one when nothing is wanted.
    pub fn poll(self: &Arc<Self>, tasks: &[Task], allowed: &[Account], now: u64) -> Snapshot {
        let wanted = wanted(tasks, allowed);
        let labels: Vec<String> = wanted.iter().map(|a| a.label().to_string()).collect();
        {
            let mut g = lock(&self.inner);
            g.wanted = labels.clone();
            g.stamped = !stamped(tasks).is_empty();
            if wanted.is_empty() {
                return Snapshot::default();
            }
            let due = g
                .last
                .is_none_or(|last| now >= last.saturating_add(EVERY_SECS) || now < last);
            if !due || g.running {
                return g.fresh(now, &labels);
            }
            g.running = true;
            g.last = Some(now);
        }
        let me = Arc::clone(self);
        (self.spawn)(Box::new(move || {
            let _running = Running(&me.inner);
            let snapshot = read_all(&me.fetch, &wanted);
            let mut g = lock(&me.inner);
            g.snapshot = snapshot;
            g.snapshot_at = now;
        }));
        lock(&self.inner).fresh(now, &labels)
    }

    /// Settings' line (0036's Agents): "on (2 accounts)", or "not available: <why>", so the
    /// fallback to heartbeats is never silent.
    pub fn summary(&self) -> String {
        let g = lock(&self.inner);
        if g.wanted.is_empty() && g.stamped {
            return "not read now: no task in Building or Testing has a session of an account Muster reads".into();
        }
        if g.wanted.is_empty() {
            return "not read now: no task in Building or Testing has a session stamp".into();
        }
        if g.last.is_none() || (g.running && g.snapshot.accounts.is_empty()) {
            return "reading…".into();
        }
        summary_of(&g.snapshot)
    }
}

/// An account as the window names it: `cc-hg`.
fn account_name(label: &str) -> String {
    format!("cc-{label}")
}

/// "on (1 account)", "on (2 accounts); cc-mg not available: <why>", or "not available: <why>".
pub fn summary_of(snap: &Snapshot) -> String {
    let ok = snap.accounts.iter().filter(|(_, r)| r.is_ok()).count();
    let failed: Vec<(&String, &String)> = snap
        .accounts
        .iter()
        .filter_map(|(a, r)| r.as_ref().err().map(|why| (a, why)))
        .collect();
    if ok == 0 {
        return match failed.as_slice() {
            [] => "not available: nothing was read".into(),
            [(_, why)] => format!("not available: {why}"),
            many => format!(
                "not available: {}",
                many.iter()
                    .map(|(a, why)| format!("{}: {why}", account_name(a)))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        };
    }
    let on = format!("on ({ok} account{})", if ok == 1 { "" } else { "s" });
    failed.iter().fold(on, |out, (a, why)| {
        format!("{out}; {} not available: {why}", account_name(a))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// What `claude agents --json --all` printed on 2026-10-13 (2.1.296), names and folders made
    /// up, plus an interactive session (no `state`, a `status`) as that version writes one.
    const OBSERVED: &str = r#"[
  {"id": "c7f6ddd2", "cwd": "/Users/someone/code/app", "kind": "background", "startedAt": 1760320000000,
   "sessionId": "c7f6ddd2-1111-4222-8333-444455556666", "name": "fix the build", "state": "blocked"},
  {"id": "531266d4", "cwd": "/Users/someone/code/muster", "kind": "background", "startedAt": 1760320100000,
   "sessionId": "531266d4-373f-4be6-836c-971ed9681022", "name": "muster", "state": "failed"},
  {"pid": 4242, "cwd": "/Users/someone/code/x", "kind": "interactive", "startedAt": 1760320200000,
   "sessionId": "7b5e8856-c9c8-45ec-aa03-c0ddbf1d4394", "status": "busy"}
]"#;

    fn entry(id: &str, state: SessionState) -> Entry {
        Entry {
            id: id.into(),
            state,
        }
    }

    #[test]
    fn the_observed_shape_reads_to_short_ids_and_states_and_nothing_else() {
        let entries = parse(OBSERVED.as_bytes()).unwrap();
        assert_eq!(
            entries,
            [
                entry("c7f6ddd2", SessionState::NeedsInput),
                entry("531266d4", SessionState::Unknown),
                entry("7b5e8856", SessionState::Working),
            ]
        );
        // Only ids and states are kept: no folder or name survives the read.
        let kept = format!("{entries:?}");
        for gone in ["someone", "fix the build", "/Users", "muster\""] {
            assert!(!kept.contains(gone), "{gone} in {kept}");
        }
    }

    #[test]
    fn unknown_fields_are_ignored_and_entries_without_an_id_or_a_state_are_skipped() {
        let out = r#"[
          {"sessionId": "11111111-aaaa", "state": "working", "brandNew": {"nested": [1, 2]}, "tempo": "active"},
          {"state": "working"},
          {"sessionId": null, "state": "working"},
          {"sessionId": 12345678, "state": "working"},
          {"sessionId": "not-a-session", "state": "working"},
          {"sessionId": "22222222-bbbb"},
          {"sessionId": "33333333-cccc", "state": 7},
          "a string",
          42,
          {"sessionId": "44444444-dddd", "state": "Ready for review"},
          {"sessionId": "55555555-eeee", "state": "something-new"},
          {"sessionId": "66666666-ffff", "status": "waiting", "waitingFor": "a permission"},
          {"sessionId": "77777777-0000", "status": "idle"}
        ]"#;
        assert_eq!(
            parse(out.as_bytes()).unwrap(),
            [
                entry("11111111", SessionState::Working),
                entry("44444444", SessionState::Ended),
                entry("55555555", SessionState::Unknown),
                entry("66666666", SessionState::NeedsInput),
                entry("77777777", SessionState::Unknown),
            ]
        );
        assert_eq!(parse(b"[]").unwrap(), []);
    }

    #[test]
    fn states_map_to_working_waiting_ended_or_unknown() {
        for (raw, want) in [
            ("working", SessionState::Working),
            ("WORKING", SessionState::Working),
            ("blocked", SessionState::NeedsInput),
            ("needs input", SessionState::NeedsInput),
            ("needs-input", SessionState::NeedsInput),
            ("done", SessionState::Ended),
            ("completed", SessionState::Ended),
            ("ready_for_review", SessionState::Ended),
            ("failed", SessionState::Unknown),
            ("stopped", SessionState::Unknown),
            ("", SessionState::Unknown),
        ] {
            assert_eq!(SessionState::from_state(raw), want, "{raw:?}");
        }
        assert_eq!(SessionState::from_status("busy"), SessionState::Working);
        assert_eq!(
            SessionState::from_status("waiting"),
            SessionState::NeedsInput
        );
        assert_eq!(SessionState::from_status("idle"), SessionState::Unknown);
        // A session listed twice (a resumed id) is its latest entry: by `startedAt`, else the
        // one listed last (Claude Code lists them oldest first).
        let twice = r#"[{"sessionId": "11111111", "state": "done", "startedAt": 100}, {"sessionId": "11111111", "status": "busy", "startedAt": 200}]"#;
        assert_eq!(
            parse(twice.as_bytes()).unwrap(),
            [entry("11111111", SessionState::Working)]
        );
        let reversed = r#"[{"sessionId": "11111111", "status": "busy", "startedAt": 200}, {"sessionId": "11111111", "state": "done", "startedAt": 100}]"#;
        assert_eq!(
            parse(reversed.as_bytes()).unwrap(),
            [entry("11111111", SessionState::Working)]
        );
        let unstamped = r#"[{"sessionId": "11111111", "state": "working"}, {"sessionId": "11111111", "state": "done"}, {"sessionId": "22222222", "state": "blocked"}]"#;
        assert_eq!(
            parse(unstamped.as_bytes()).unwrap(),
            [
                entry("11111111", SessionState::Ended),
                entry("22222222", SessionState::NeedsInput)
            ]
        );
        // A background session whose state says working but whose process sits idle may be
        // ready for review: unknown, not working.
        let idle = r#"[{"sessionId": "11111111", "state": "working", "status": "idle"}, {"sessionId": "22222222", "state": "working", "status": "busy"}]"#;
        assert_eq!(
            parse(idle.as_bytes()).unwrap(),
            [
                entry("11111111", SessionState::Unknown),
                entry("22222222", SessionState::Working)
            ]
        );
        assert_eq!(
            serde_json::to_string(&[
                SessionState::Working,
                SessionState::NeedsInput,
                SessionState::Ended,
                SessionState::Unknown
            ])
            .unwrap(),
            r#"["working","needs-input","ended","unknown"]"#
        );
    }

    #[test]
    fn anything_but_a_json_array_under_1_mb_is_not_available_with_the_reason() {
        for (out, why) in [
            (&b"{\"sessions\": []}"[..], "its output isn't a JSON array"),
            (b"null", "its output isn't a JSON array"),
            (b"\"working\"", "its output isn't a JSON array"),
            (b"", "its output isn't JSON"),
            (b"Error: unknown option --json", "its output isn't JSON"),
            (b"[{\"sessionId\": \"11111111\"", "its output isn't JSON"),
            (b"\xff\xfe[]", "its output isn't JSON"),
        ] {
            assert_eq!(
                parse(out).unwrap_err(),
                why,
                "{:?}",
                String::from_utf8_lossy(out)
            );
        }
        // Over 1 MB, even as a valid array, isn't read at all.
        let mut big = b"[".to_vec();
        let one = br#"{"sessionId": "11111111", "state": "working"},"#;
        while big.len() <= MAX_OUTPUT {
            big.extend_from_slice(one);
        }
        big.extend_from_slice(br#"{"sessionId": "22222222", "state": "working"}]"#);
        assert_eq!(parse(&big).unwrap_err(), "its output is over 1 MB");
        let mut fits = b"[".to_vec();
        fits.resize(MAX_OUTPUT - 1, b' ');
        fits.push(b']');
        assert_eq!(parse(&fits).unwrap(), []);
    }

    /// A fake `claude` in its own folder: `body` after the shebang. It can write what it was
    /// given next to itself (`$0.args`, `$0.env`).
    fn fake(dir: &Path, body: &str) -> PathBuf {
        let p = dir.join("claude");
        fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        let mut perms = fs::metadata(&p).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
        fs::set_permissions(&p, perms).unwrap();
        p
    }

    const RECORD: &str = r#"printf '%s\n' "$@" > "$0.args"
printf '%s' "${CLAUDE_CONFIG_DIR-unset}" > "$0.env"
printf '%s ' "$CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC" "$DISABLE_TELEMETRY" "$DO_NOT_TRACK" > "$0.quiet"
printf '%s' '[{"sessionId": "7b5e8856-c9c8", "state": "working"}]'"#;

    #[test]
    fn the_runner_passes_an_argument_array_and_the_accounts_config_folder() {
        let d = tempfile::tempdir().unwrap();
        let claude = fake(d.path(), RECORD);
        let read = |p: &str| fs::read_to_string(d.path().join(p)).unwrap();
        let dir = d.path().join(".claude-hg");
        let out = run(&claude, &dir, GENEROUS).unwrap();
        assert_eq!(
            parse(&out).unwrap(),
            [entry("7b5e8856", SessionState::Working)]
        );
        assert_eq!(read("claude.args"), "agents\n--json\n--all\n");
        assert_eq!(read("claude.env"), dir.to_string_lossy());
        assert_eq!(read("claude.quiet"), "1 1 1 ", "its usage reporting is off");
    }

    /// For runs that are expected to finish: far past any machine's slowness, and never waited out
    /// (a run that finishes returns at once). Timeouts are tested on a fake clock instead.
    const GENEROUS: Duration = Duration::from_secs(60);

    /// A clock that only moves when the test says so.
    struct Steps(std::cell::Cell<Duration>);

    impl Clock for Steps {
        fn elapsed(&self) -> Duration {
            self.0.get()
        }

        fn pause(&self) {
            self.0.set(self.0.get() + Duration::from_millis(20));
        }
    }

    /// A clock that stays at zero until `ready` (the fake claude has started what the test needs
    /// to see stopped), then reports `after`: the timeout fires when the test is ready for it,
    /// however slowly the machine started the process. Past a minute of real time it fires anyway,
    /// so a broken fake fails the test instead of hanging it.
    struct WhenReady<F: Fn() -> bool> {
        ready: F,
        after: Duration,
        started: Instant,
    }

    impl<F: Fn() -> bool> Clock for WhenReady<F> {
        fn elapsed(&self) -> Duration {
            if (self.ready)() || self.started.elapsed() > Duration::from_secs(60) {
                self.after
            } else {
                Duration::ZERO
            }
        }

        fn pause(&self) {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn waiting_gives_up_at_the_timeout_on_the_clock_it_is_given() {
        let polls = std::cell::Cell::new(0);
        let clock = Steps(std::cell::Cell::new(Duration::ZERO));
        let started = Instant::now();
        let waited = wait(
            || {
                polls.set(polls.get() + 1);
                Ok::<_, std::io::Error>(None::<()>)
            },
            &clock,
            Duration::from_secs(10),
        );
        assert!(matches!(waited, Wait::TimedOut));
        assert_eq!(
            clock.elapsed(),
            Duration::from_secs(10),
            "to the timeout, no further"
        );
        assert_eq!(
            polls.get(),
            501,
            "a look every 20 ms of its time, and one at the end"
        );
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "none of it was real time"
        );
    }

    #[test]
    fn waiting_returns_what_finished_and_looks_before_it_judges_the_time() {
        let clock = Steps(std::cell::Cell::new(Duration::ZERO));
        let mut left = 3;
        let waited = wait(
            || {
                left -= 1;
                Ok(if left == 0 { Some("done") } else { None })
            },
            &clock,
            Duration::from_secs(10),
        );
        assert!(matches!(waited, Wait::Done("done")));
        assert_eq!(
            clock.elapsed(),
            Duration::from_millis(40),
            "paused between looks only"
        );
        // Finished just as the time ran out: still finished.
        let late = Steps(std::cell::Cell::new(Duration::from_secs(10)));
        assert!(matches!(
            wait(
                || Ok::<_, std::io::Error>(Some(1)),
                &late,
                Duration::from_secs(10)
            ),
            Wait::Done(1)
        ));
        // No time at all: one look, then out.
        let none = Steps(std::cell::Cell::new(Duration::ZERO));
        assert!(matches!(
            wait(
                || Ok::<_, std::io::Error>(None::<()>),
                &none,
                Duration::ZERO
            ),
            Wait::TimedOut
        ));
    }

    #[test]
    fn waiting_reports_a_failed_look_without_waiting_on() {
        let clock = Steps(std::cell::Cell::new(Duration::ZERO));
        let waited = wait(
            || Err::<Option<()>, _>(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
            &clock,
            Duration::from_secs(10),
        );
        assert!(matches!(
            waited,
            Wait::Failed(std::io::ErrorKind::PermissionDenied)
        ));
        assert_eq!(clock.elapsed(), Duration::ZERO);
    }

    #[test]
    fn the_runner_falls_back_with_the_reason_on_a_missing_binary_or_an_exit() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(
            run(&d.path().join("no-claude"), d.path(), GENEROUS).unwrap_err(),
            "claude isn't installed"
        );
        let failing = fake(d.path(), "echo '[]'; exit 3");
        assert_eq!(
            run(&failing, d.path(), GENEROUS).unwrap_err(),
            "claude agents exited with code 3"
        );
        // Output past the cap: stopped reading, not available.
        let loud = fake(d.path(), "head -c 2000000 /dev/zero | tr '\\0' ' '");
        assert_eq!(
            run(&loud, d.path(), GENEROUS).unwrap_err(),
            "its output is over 1 MB"
        );
        assert_eq!(TIMEOUT, Duration::from_secs(10));
    }

    #[test]
    fn the_runner_stops_a_run_that_is_past_its_timeout_and_what_it_started() {
        let d = tempfile::tempdir().unwrap();
        let timeout = Duration::from_millis(300);
        // The clock runs out once the fake has said it is running (a file next to it).
        let clock = |marker: PathBuf| WhenReady {
            ready: move || marker.exists(),
            after: timeout,
            started: Instant::now(),
        };
        let slow = fake(d.path(), "touch \"$0.up\"\nexec sleep 30");
        let started = Instant::now();
        assert_eq!(
            run_with(&slow, d.path(), timeout, &clock(d.path().join("claude.up"))).unwrap_err(),
            "claude agents didn't finish within 300ms"
        );
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "it was stopped"
        );
        // A child it started (holding the pipe) is stopped with it: the whole group goes.
        let parent = fake(
            d.path(),
            "sleep 30 &\necho $! > \"$0.child.tmp\"\nmv \"$0.child.tmp\" \"$0.child\"\nexec sleep 30",
        );
        assert!(run_with(
            &parent,
            d.path(),
            timeout,
            &clock(d.path().join("claude.child"))
        )
        .unwrap_err()
        .contains("didn't finish"));
        let child = fs::read_to_string(d.path().join("claude.child")).unwrap();
        let alive = || {
            Command::new("/bin/kill")
                .args(["-0", child.trim()])
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success()
        };
        let deadline = Instant::now() + Duration::from_secs(30);
        while alive() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!alive(), "the child it started was stopped too");
    }

    #[test]
    fn the_claude_source_needs_the_accounts_folder_and_finds_claude_where_the_helper_does() {
        let home = crate::accounts::tests::fake_home();
        let bin = home.path().join(".local/bin");
        fs::create_dir_all(&bin).unwrap();
        fake(&bin, RECORD);
        let allowed = crate::accounts::resolve(&["mg".into(), "hg".into()], home.path());
        let fetch = claude_fetch(home.path().to_path_buf());
        assert!(parse(&fetch(&allowed[1]).unwrap()).is_ok());
        assert_eq!(
            fs::read_to_string(bin.join("claude.env")).unwrap(),
            home.path().join(".claude-hg").to_string_lossy()
        );
        // The folder went after it was resolved: no run, and no path with the user's name.
        fs::remove_file(home.path().join(".claude-mg/projects")).unwrap();
        fs::remove_dir(home.path().join(".claude-mg")).unwrap();
        fs::remove_file(bin.join("claude.env")).unwrap();
        assert_eq!(
            fetch(&allowed[0]).unwrap_err(),
            "there's no ~/.claude-mg folder"
        );
        assert!(!bin.join("claude.env").exists(), "nothing ran");
    }

    /// The allowlist on a fake home (`accounts::tests::fake_home`): mg and hg.
    fn allowed() -> Vec<Account> {
        let home = crate::accounts::tests::fake_home();
        let a = crate::accounts::resolve(&["mg".into(), "hg".into()], home.path());
        assert_eq!(a.len(), 2);
        a
    }

    fn labels(accounts: &[Account]) -> Vec<&str> {
        accounts.iter().map(Account::label).collect()
    }

    /// AC1 (0099): with a fake home holding the company account (`~/.claude`), `~/.claude-ih`,
    /// `~/.claude-mg` and `~/.claude-hg`, every `claude agents` run, the app's monitor's and
    /// `muster list --json`'s alike (both read through `wanted`, `read_all` and `claude_fetch`),
    /// has `CLAUDE_CONFIG_DIR` set to mg's or hg's folder: never unset, never ih's or `~/.claude`.
    /// A task stamped with no account or with ih reads "unknown" and causes no run.
    #[test]
    fn every_run_is_for_an_allowed_account_and_other_stamps_read_unknown_without_a_run() {
        let home = crate::accounts::tests::fake_home();
        let h = home.path();
        let bin = h.join(".local/bin");
        fs::create_dir_all(&bin).unwrap();
        // Each run appends the CLAUDE_CONFIG_DIR it got, so none is missed.
        fake(
            &bin,
            r#"printf '[%s]\n' "${CLAUDE_CONFIG_DIR-unset}" >> "$0.runs"
printf '%s' '[{"sessionId": "7b5e8856", "state": "blocked"}, {"sessionId": "4d517d9d", "state": "blocked"}, {"sessionId": "11111111", "state": "blocked"}, {"sessionId": "22222222", "state": "blocked"}]'"#,
        );
        let allowed = crate::accounts::resolve(&["mg".into(), "hg".into()], h);
        let mut tasks = vec![
            task("0001", "building", A),
            task("0002", "building", "qa (session 4d517d9d · cc-mg)"),
            task("0003", "building", "wt-c (session 11111111)"),
            task("0004", "building", "wt-d (session 22222222 · cc-ih)"),
        ];
        let runs = || fs::read_to_string(bin.join("claude.runs")).unwrap_or_default();
        // The app's monitor.
        let m = Monitor::new(claude_fetch(h.to_path_buf()), inline);
        let snap = m.poll(&tasks, &allowed, 1000);
        // `muster list --json`'s read.
        let snap2 = read_all(&claude_fetch(h.to_path_buf()), &wanted(&tasks, &allowed));
        assert_eq!(snap, snap2);
        let (mg, hg) = (h.join(".claude-mg"), h.join(".claude-hg"));
        assert_eq!(
            runs(),
            format!(
                "[{}]\n[{}]\n[{}]\n[{}]\n",
                mg.display(),
                hg.display(),
                mg.display(),
                hg.display()
            ),
            "only mg and hg, each set, each once per read"
        );
        apply(&mut tasks, &snap, &allowed);
        use SessionState::*;
        assert_eq!(states(&tasks), [NeedsInput, NeedsInput, Unknown, Unknown]);
        assert!(!tasks[2].waiting_for_owner && !tasks[3].waiting_for_owner);
        // Only unlabelled and ih stamps: nothing runs at all.
        fs::remove_file(bin.join("claude.runs")).unwrap();
        let others = vec![tasks[2].clone(), tasks[3].clone()];
        assert!(wanted(&others, &allowed).is_empty());
        let m = Monitor::new(claude_fetch(h.to_path_buf()), inline);
        assert_eq!(m.poll(&others, &allowed, 1000), Snapshot::default());
        assert_eq!(runs(), "", "no run");
        assert_eq!(
            m.summary(),
            "not read now: no task in Building or Testing has a session of an account Muster reads"
        );
        // An empty allowlist reads nothing.
        assert!(wanted(&tasks, &[]).is_empty());
    }

    /// A task as the queue reads it, in `stage`, held by `who` (the claim in Building, `held-by`
    /// in Testing and Your check).
    fn task(id: &str, stage: &str, who: &str) -> Task {
        let d = tempfile::tempdir().unwrap();
        let text = format!(
            "---\nid: {id}\nstage: {stage}\nstatus: doing\nclaimed-by: {}\nheld-by: {}\nheartbeat:\nwaiting_on:\n---\n\n# T\n",
            if stage == "building" { who } else { "" },
            if stage == "building" { "" } else { who },
        );
        let path = d.path().join(format!("doing/{id}-t.md"));
        queue::parse_task("q", d.path(), "doing", &path, &text, 0).unwrap()
    }

    const A: &str = "wt-a (wt-0048 · session 7b5e8856 · cc-hg)";

    fn snap(pairs: &[(&str, &str, SessionState)]) -> Snapshot {
        Snapshot {
            states: pairs
                .iter()
                .map(|(a, id, s)| ((a.to_string(), id.to_string()), *s))
                .collect(),
            ..Snapshot::default()
        }
    }

    fn states(tasks: &[Task]) -> Vec<SessionState> {
        tasks.iter().map(|t| t.session_state).collect()
    }

    #[test]
    fn a_state_applies_only_to_the_one_open_task_its_session_holds() {
        use SessionState::*;
        let allowed = allowed();
        let s = snap(&[("hg", "7b5e8856", Working), ("hg", "4d517d9d", NeedsInput)]);
        let mut tasks = vec![task("0001", "building", A)];
        apply(&mut tasks, &s, &allowed);
        assert_eq!(states(&tasks), [Working]);
        assert_eq!(tasks[0].session_account, "hg");
        assert!(!tasks[0].waiting_for_owner);
        // The same id in another allowed account's list is this session, resumed there (0182).
        let mut other = vec![task("0001", "building", "wt-a (session 7b5e8856 · cc-mg)")];
        apply(&mut other, &s, &allowed);
        assert_eq!(states(&other), [Working]);
        assert_eq!(other[0].session_account, "hg");
        // A coordinator's subagents share its id: neither task gets its state.
        let mut shared = vec![
            task("0001", "building", A),
            task(
                "0002",
                "building",
                "wt-b (wt-0049 · session 7b5e8856 · cc-hg)",
            ),
        ];
        apply(&mut shared, &s, &allowed);
        assert_eq!(states(&shared), [Unknown, Unknown]);
        // A builder that passed a task to Testing (it's `held-by` there) and claimed the next one:
        // its state is the new claim's, and says nothing about the task in Testing.
        let mut testing = vec![task("0001", "building", A), task("0003", "testing", A)];
        apply(&mut testing, &s, &allowed);
        assert_eq!(states(&testing), [Working, Unknown]);
        // In Testing with nothing claimed, one task: its state; two: neither.
        let mut alone = vec![task("0003", "testing", A)];
        apply(&mut alone, &s, &allowed);
        assert_eq!(states(&alone), [Working]);
        let mut two = vec![task("0003", "testing", A), task("0009", "testing", A)];
        apply(&mut two, &s, &allowed);
        assert_eq!(states(&two), [Unknown, Unknown]);
        // Your check's holder isn't working on it: it doesn't count either way.
        let mut checked = vec![task("0001", "building", A), task("0010", "your-check", A)];
        apply(&mut checked, &s, &allowed);
        assert_eq!(states(&checked), [Working, Unknown]);
        let mut done = vec![task("0001", "building", A), task("0004", "done", A)];
        apply(&mut done, &s, &allowed);
        assert_eq!(states(&done), [Working, Unknown]);
        // No stamp, the owner, or a stage nobody works in: today's lines.
        let mut rest = vec![
            task("0005", "building", "wt-a"),
            task("0006", "building", "owner (session 4d517d9d · cc-hg)"),
            task("0007", "your-check", "qa (session 4d517d9d · cc-hg)"),
            task("0008", "building", "wt-z (session 99999999 · cc-hg)"),
        ];
        apply(&mut rest, &s, &allowed);
        assert_eq!(states(&rest), [Unknown, Unknown, Unknown, Unknown]);
        // Nothing read: unknown, and a state set by an earlier read is cleared.
        apply(&mut tasks, &Snapshot::default(), &allowed);
        assert_eq!(states(&tasks), [Unknown]);
        assert_eq!(tasks[0].session_account, "");
    }

    /// AC1 (0182): cc-mg hit its weekly limit and its builders were resumed on cc-hg with the same
    /// session ids and claims, so every claim still says `· cc-mg`. With a stand-in `claude` for
    /// each account (mg lists nothing, hg lists the resumed session working), the app's monitor
    /// and `muster list --json`'s read (both through `wanted`, `read_all` and `claude_fetch`) ask
    /// every allowed account once per read, and the claim reads as working, found on hg. One
    /// listed by neither account reads unknown (the heartbeat lines decide, as before).
    #[test]
    fn a_session_stamped_on_one_account_and_listed_only_on_another_reads_as_working() {
        let home = crate::accounts::tests::fake_home();
        let h = home.path();
        let bin = h.join(".local/bin");
        fs::create_dir_all(&bin).unwrap();
        fake(
            &bin,
            r#"printf '[%s]\n' "${CLAUDE_CONFIG_DIR-unset}" >> "$0.runs"
case "$CLAUDE_CONFIG_DIR" in
  *-hg) printf '%s' '[{"sessionId": "3c09f6c7-1111", "state": "working"}, {"sessionId": "9d9d9d9d", "state": "blocked"}]' ;;
  *) printf '%s' '[]' ;;
esac"#,
        );
        let allowed = crate::accounts::resolve(&["mg".into(), "hg".into()], h);
        let mut tasks = vec![
            task(
                "0001",
                "building",
                "asghar_20 (main · session 3c09f6c7 · cc-mg)",
            ),
            task(
                "0002",
                "building",
                "gholi_3 (main · session 5e5e5e5e · cc-mg)",
            ),
            task("0003", "testing", "qa (main · session 9d9d9d9d · cc-mg)"),
        ];
        let runs = || fs::read_to_string(bin.join("claude.runs")).unwrap_or_default();
        let m = Monitor::new(claude_fetch(h.to_path_buf()), inline);
        let snap = m.poll(&tasks, &allowed, 1000);
        let (mg, hg) = (h.join(".claude-mg"), h.join(".claude-hg"));
        let once = format!("[{}]\n[{}]\n", mg.display(), hg.display());
        assert_eq!(runs(), once, "each allowed account, once");
        assert_eq!(snap.wanted, ["mg", "hg"]);
        let cli = read_all(&claude_fetch(h.to_path_buf()), &wanted(&tasks, &allowed));
        assert_eq!(snap, cli);
        assert_eq!(runs(), once.repeat(2));
        apply(&mut tasks, &snap, &allowed);
        use SessionState::*;
        assert_eq!(states(&tasks), [Working, Unknown, NeedsInput]);
        assert_eq!(tasks[0].session_account, "hg");
        assert_eq!(tasks[1].session_account, "");
        // Waiting for input on hg waits on the owner, and the item names hg, where it was read.
        assert!(!tasks[0].waiting_for_owner && tasks[2].waiting_for_owner);
        assert_eq!(tasks[2].session_account, "hg");
        let json = serde_json::to_value(&tasks[0]).unwrap();
        assert_eq!(json["sessionState"], "working");
        assert!(
            json.get("sessionAccount").is_none(),
            "not sent to the window"
        );
        // With hg off the list, nothing reads it, and the claim reads unknown again.
        fs::remove_file(bin.join("claude.runs")).unwrap();
        let mg_only = crate::accounts::resolve(&["mg".into()], h);
        let m = Monitor::new(claude_fetch(h.to_path_buf()), inline);
        let snap = m.poll(&tasks, &mg_only, 1000);
        assert_eq!(runs(), format!("[{}]\n", mg.display()));
        apply(&mut tasks, &snap, &mg_only);
        assert_eq!(states(&tasks), [Unknown, Unknown, Unknown]);
    }

    /// A session listed on more than one account: where it's working or waiting is where it runs
    /// now, ahead of an account whose list says it ended or doesn't know its state; on a tie, the
    /// stamp's account, then the first by name. Only allowed accounts count.
    #[test]
    fn a_session_listed_on_two_accounts_takes_the_liveliest_state() {
        use SessionState::*;
        let allowed = allowed();
        let mg_only: Vec<Account> = allowed
            .iter()
            .filter(|a| a.label() == "mg")
            .cloned()
            .collect();
        let id = "3c09f6c7";
        let find = |pairs: &[(&str, SessionState)], stamped: &str, allowed: &[Account]| {
            let pairs: Vec<(&str, &str, SessionState)> =
                pairs.iter().map(|(a, s)| (*a, id, *s)).collect();
            snap(&pairs).find(id, stamped, allowed)
        };
        let at = |s: SessionState, a: &str| Some((s, a.to_string()));
        assert_eq!(
            find(&[("mg", Ended), ("hg", Working)], "mg", &allowed),
            at(Working, "hg")
        );
        assert_eq!(
            find(&[("mg", Unknown), ("hg", NeedsInput)], "mg", &allowed),
            at(NeedsInput, "hg")
        );
        assert_eq!(
            find(&[("mg", NeedsInput), ("hg", Ended)], "hg", &allowed),
            at(NeedsInput, "mg")
        );
        assert_eq!(
            find(&[("mg", Ended), ("hg", Unknown)], "mg", &allowed),
            at(Unknown, "hg")
        );
        // A tie: the stamp's account, else the first by name.
        assert_eq!(
            find(&[("mg", Working), ("hg", Working)], "hg", &allowed),
            at(Working, "hg")
        );
        assert_eq!(
            find(&[("mg", Ended), ("hg", Ended)], "mg", &allowed),
            at(Ended, "mg")
        );
        assert_eq!(
            find(&[("mg", Unknown), ("hg", Unknown)], "", &allowed),
            at(Unknown, "hg")
        );
        // Only one listing: that one, whatever the stamp says.
        assert_eq!(find(&[("hg", Ended)], "mg", &allowed), at(Ended, "hg"));
        // An account off the list never lends its state, nor one never allowed (ih, the company's "").
        assert_eq!(
            find(&[("mg", Ended), ("hg", Working)], "mg", &mg_only),
            at(Ended, "mg")
        );
        assert_eq!(
            find(&[("ih", Working), ("", Working)], "mg", &allowed),
            None
        );
        assert_eq!(find(&[], "mg", &allowed), None);
        // Another id's state is never this one's.
        assert_eq!(
            snap(&[("hg", "4d517d9d", Working)]).find(id, "hg", &allowed),
            None
        );
        // In Testing an ended session reads unknown, wherever it was found.
        let mut testing = vec![task("0003", "testing", "qa (session 3c09f6c7 · cc-mg)")];
        apply(&mut testing, &snap(&[("hg", id, Ended)]), &allowed);
        assert_eq!(states(&testing), [Unknown]);
        assert_eq!(testing[0].session_account, "");
    }

    #[test]
    fn waiting_for_input_waits_on_the_owner_and_ended_shows_only_in_building() {
        use SessionState::*;
        let allowed = allowed();
        let b = "wt-b (session 4d517d9d · cc-hg)";
        let s = snap(&[("hg", "7b5e8856", NeedsInput), ("hg", "4d517d9d", Ended)]);
        let mut tasks = vec![task("0001", "building", A), task("0002", "testing", b)];
        assert!(!tasks[0].waiting_for_owner);
        apply(&mut tasks, &s, &allowed);
        // In Testing, `held-by` is the builder that passed it, whose session has usually ended:
        // that isn't news, so Testing reads as before.
        assert_eq!(states(&tasks), [NeedsInput, Unknown]);
        let mut building = vec![task("0002", "building", b)];
        apply(&mut building, &s, &allowed);
        assert_eq!(states(&building), [Ended]);
        assert!(tasks[0].waiting_for_owner, "counts in the needs-you count");
        assert!(!tasks[1].waiting_for_owner);
        let json = serde_json::to_value(&tasks[0]).unwrap();
        assert_eq!(json["sessionState"], "needs-input");
        // A session with no cc- part ran on the company account (0099): never given a state, even
        // by a snapshot that has one for it.
        let mut plain = vec![task("0003", "building", "wt-c (session 11111111)")];
        apply(&mut plain, &snap(&[("", "11111111", Working)]), &allowed);
        assert_eq!(states(&plain), [Unknown]);
    }

    /// R5 (0099): an account taken off the list stops counting at once. The monitor's cached
    /// snapshot, not due for a new read yet, hands on only the accounts wanted now, and `apply`
    /// gives no state to a stamp whose account isn't allowed, whatever the snapshot holds.
    #[test]
    fn an_account_taken_off_the_list_loses_its_states_before_the_next_read() {
        use SessionState::*;
        let (fetch, reads) = counting(
            r#"[{"sessionId": "7b5e8856", "state": "blocked"}, {"sessionId": "4d517d9d", "state": "working"}]"#,
        );
        let m = Monitor::new(fetch, inline);
        let both = allowed();
        let mg_only: Vec<Account> = both.iter().filter(|a| a.label() == "mg").cloned().collect();
        let tasks = vec![
            task("0001", "building", A),
            task("0002", "building", "qa (session 4d517d9d · cc-mg)"),
        ];
        let s = m.poll(&tasks, &both, 1000);
        assert_eq!(reads.load(Ordering::SeqCst), 2, "one read per account");
        assert!(s
            .states
            .contains_key(&("hg".to_string(), "7b5e8856".to_string())));
        // hg removed; the next poll comes before a read is due and gets the cache.
        let s = m.poll(&tasks, &mg_only, 1001);
        assert_eq!(reads.load(Ordering::SeqCst), 2, "no new read");
        let labels: HashSet<&str> = s.states.keys().map(|(a, _)| a.as_str()).collect();
        assert_eq!(labels, HashSet::from(["mg"]));
        assert_eq!(
            s.accounts
                .iter()
                .map(|(a, _)| a.as_str())
                .collect::<Vec<_>>(),
            ["mg"]
        );
        let mut now = tasks.clone();
        apply(&mut now, &s, &mg_only);
        assert_eq!(states(&now), [Unknown, Working]);
        assert!(
            !now[0].waiting_for_owner,
            "hg's prompt isn't the owner's any more"
        );
        // Given a snapshot that still has hg's state, apply doesn't use it either.
        let old = snap(&[("hg", "7b5e8856", NeedsInput), ("mg", "4d517d9d", Working)]);
        let mut now = tasks.clone();
        apply(&mut now, &old, &mg_only);
        assert_eq!(states(&now), [Unknown, Working]);
        assert!(!now[0].waiting_for_owner);
        apply(&mut now, &old, &both);
        assert_eq!(states(&now), [NeedsInput, Working]);
    }

    #[test]
    fn a_paused_task_gets_no_session_state_and_isnt_read_for() {
        use SessionState::*;
        let allowed = allowed();
        // Its session was told to stop (0045): whatever Claude Code says, the task is paused, not
        // working, and a prompt in that session isn't the paused task waiting on the owner.
        let paused = |mut t: Task| {
            t.paused = "2026-10-13 10:00".into();
            t
        };
        for state in [Working, NeedsInput, Ended] {
            let mut tasks = vec![paused(task("0001", "building", A))];
            apply(&mut tasks, &snap(&[("hg", "7b5e8856", state)]), &allowed);
            assert_eq!(states(&tasks), [Unknown], "{state:?}");
            assert!(!tasks[0].waiting_for_owner, "{state:?}");
        }
        assert!(wanted(&[paused(task("0001", "building", A))], &allowed).is_empty());
        let mut resumed = vec![task("0001", "building", A)];
        apply(
            &mut resumed,
            &snap(&[("hg", "7b5e8856", NeedsInput)]),
            &allowed,
        );
        assert_eq!(states(&resumed), [NeedsInput]);
        // Paused, its session was told to stop: the task isn't what the session is doing, so it
        // doesn't count as its hold, and the session's next claim gets the state.
        let mut next = vec![
            paused(task("0001", "building", A)),
            task("0002", "building", A),
        ];
        apply(
            &mut next,
            &snap(&[("hg", "7b5e8856", NeedsInput)]),
            &allowed,
        );
        assert_eq!(states(&next), [Unknown, NeedsInput]);
        assert!(!next[0].waiting_for_owner && next[1].waiting_for_owner);
        let mut testing = vec![
            paused(task("0003", "testing", A)),
            task("0004", "testing", A),
        ];
        apply(
            &mut testing,
            &snap(&[("hg", "7b5e8856", Working)]),
            &allowed,
        );
        assert_eq!(states(&testing), [Unknown, Working]);
    }

    #[test]
    fn holds_count_across_every_queue_given() {
        use SessionState::*;
        let allowed = allowed();
        let s = snap(&[("hg", "7b5e8856", NeedsInput)]);
        let mut here = vec![task("0001", "building", A)];
        // The same session holds a task in another queue: its state is about neither.
        let elsewhere = vec![task("0101", "building", A)];
        let h = holds(here.iter().chain(&elsewhere));
        apply_with(&mut here, &h, &s, &allowed);
        assert_eq!(states(&here), [Unknown]);
        assert!(!here[0].waiting_for_owner);
        // Another queue's tasks under other sessions change nothing.
        let unrelated = vec![task("0102", "building", "wt-z (session 99999999 · cc-hg)")];
        let h = holds(here.iter().chain(&unrelated));
        apply_with(&mut here, &h, &s, &allowed);
        assert_eq!(states(&here), [NeedsInput]);
    }

    #[test]
    fn every_allowed_account_is_read_once_a_holder_in_building_or_testing_is_stamped_with_one() {
        let tasks = vec![
            task("0001", "building", A),
            task("0002", "testing", "qa (session 4d517d9d · cc-mg)"),
            task("0003", "building", "wt-c (session 11111111)"),
            task("0004", "building", "wt-d (session 22222222 · cc-hg)"),
            task("0005", "your-check", "qa (session 33333333 · cc-ih)"),
            task("0006", "building", "owner (session 44444444 · cc-zz)"),
            task("0007", "building", "wt-unstamped"),
        ];
        let allowed = allowed();
        // Every allowed account, in the allowlist's order (0182), once some stamp names one.
        assert_eq!(labels(&wanted(&tasks, &allowed)), ["mg", "hg"]);
        assert_eq!(labels(&wanted(&tasks[1..2], &allowed)), ["mg", "hg"]);
        assert!(wanted(&tasks[4..], &allowed).is_empty());
        // Only the listed ones, by an exact match.
        assert_eq!(labels(&wanted(&tasks, &allowed[1..])), ["hg"]);
        // A stamp of an account off the list causes no run, even with another account on it.
        assert!(wanted(&tasks[1..2], &allowed[1..]).is_empty());
        // The stamps say which accounts they name, all of them, for `known`.
        assert_eq!(stamped(&tasks), ["", "hg", "mg"]);
    }

    #[test]
    fn a_read_says_which_accounts_it_wanted_and_which_it_read() {
        let tasks = vec![task("0001", "building", A)];
        let read = |r: Result<usize, &str>| Snapshot {
            accounts: vec![("hg".to_string(), r.map_err(String::from))],
            ..Snapshot::default()
        };
        let hg_wanted: HashSet<String> = ["hg".to_string()].into();
        // Before the first read lands: wanted, none read.
        assert_eq!(
            known(&tasks, &Snapshot::default()),
            Known {
                wanted: hg_wanted.clone(),
                accounts: HashSet::new(),
            }
        );
        let hg = known(&tasks, &read(Ok(0)));
        assert!(hg.knows("hg") && hg.wanted == hg_wanted);
        // An account that couldn't be read says nothing of its sessions, and is still wanted.
        let failed = known(&tasks, &read(Err("claude isn't installed")));
        assert!(!failed.knows("hg") && failed.wanted == hg_wanted);
        // Each account on its own: one read Ok doesn't make another known.
        let two = Snapshot {
            accounts: vec![
                ("hg".to_string(), Ok(1)),
                ("mg".to_string(), Err("timed out".to_string())),
            ],
            wanted: vec!["mg".to_string(), "hg".to_string()],
            ..Snapshot::default()
        };
        let k = known(&tasks, &two);
        assert!(k.knows("hg") && !k.knows("mg"));
        // Every account the round asks is wanted (0182), before its read lands too.
        assert_eq!(k.wanted, ["hg".to_string(), "mg".to_string()].into());
        let asked = Snapshot {
            wanted: vec!["mg".to_string(), "hg".to_string()],
            ..Snapshot::default()
        };
        let k = known(&tasks, &asked);
        assert!(k.wanted.contains("mg") && !k.knows("mg") && !k.knows("hg"));
        // Nothing to read: nothing wanted, none read.
        let unstamped = vec![task("0002", "building", "wt-unstamped")];
        assert_eq!(known(&unstamped, &Snapshot::default()), Known::default());
    }

    #[test]
    fn a_task_names_its_sessions_account_only_where_a_state_applies() {
        assert_eq!(
            account_of(&task("0001", "building", "wt-d (session 22222222 · cc-hg)")).as_deref(),
            Some("hg")
        );
        assert_eq!(account_of(&task("0002", "building", "wt-unstamped")), None);
        assert_eq!(
            account_of(&task("0003", "your-check", "qa (session 33333333 · cc-ih)")),
            None
        );
    }

    /// A source that counts its reads per account and answers `out`.
    fn counting(out: &'static str) -> (Fetch, Arc<AtomicUsize>) {
        let n = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&n);
        let fetch: Fetch = Arc::new(move |_account: &Account| {
            seen.fetch_add(1, Ordering::SeqCst);
            Ok(out.as_bytes().to_vec())
        });
        (fetch, n)
    }

    fn inline(job: Box<dyn FnOnce() + Send>) {
        job()
    }

    #[test]
    fn the_monitor_reads_at_most_every_15_seconds_and_not_at_all_without_a_stamped_holder() {
        let (fetch, reads) = counting(r#"[{"sessionId": "7b5e8856", "state": "working"}]"#);
        let m = Monitor::new(fetch, inline);
        let allowed = allowed();
        // Nothing in Building or Testing with a stamp: nothing runs, however often it's polled.
        let idle = vec![task("0001", "building", "wt-a"), task("0002", "ready", "")];
        for now in [1000, 1015, 1030, 5000] {
            assert_eq!(m.poll(&idle, &allowed, now), Snapshot::default());
        }
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        assert!(m.summary().starts_with("not read now"), "{}", m.summary());
        // With one: read at once, then not again until 15 s have passed on the clock.
        let tasks = vec![task("0001", "building", A)];
        // Each round reads every allowed account once: mg and hg.
        let s = m.poll(&tasks, &allowed, 2000);
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        assert_eq!(
            s.states.get(&("hg".to_string(), "7b5e8856".to_string())),
            Some(&SessionState::Working)
        );
        for now in [2001, 2010, 2014] {
            assert_eq!(m.poll(&tasks, &allowed, now), s, "cached at {now}");
        }
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        m.poll(&tasks, &allowed, 2015);
        assert_eq!(reads.load(Ordering::SeqCst), 4);
        // A clock that went back starts a read rather than waiting forever.
        m.poll(&tasks, &allowed, 100);
        assert_eq!(reads.load(Ordering::SeqCst), 6);
        assert_eq!(m.summary(), "on (2 accounts)");
    }

    #[test]
    fn the_monitor_reads_off_the_polling_thread_and_never_twice_at_once() {
        let (release, wait) = mpsc::channel::<()>();
        let wait = Mutex::new(wait);
        let reads = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&reads);
        let unreleased = Arc::new(AtomicUsize::new(0));
        let gave_up = Arc::clone(&unreleased);
        let fetch: Fetch = Arc::new(move |_account: &Account| {
            seen.fetch_add(1, Ordering::SeqCst);
            // Hold the read until the test lets it go, like a slow `claude`. The bound is far
            // past any machine's slowness; a read that reaches it was never let go.
            if wait
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(60))
                .is_err()
            {
                gave_up.fetch_add(1, Ordering::SeqCst);
            }
            Ok(br#"[{"sessionId": "7b5e8856", "state": "blocked"}]"#.to_vec())
        });
        let m = Monitor::new(fetch, on_thread);
        let allowed = allowed();
        let tasks = vec![task("0001", "building", A)];
        // The read is held until the test lets it go, below: a poll that waited for it would sit
        // here until the read gave up (`unreleased`), not return.
        let first = m.poll(&tasks, &allowed, 1000);
        assert!(
            first.states.is_empty() && first.accounts.is_empty(),
            "nothing read yet"
        );
        assert_eq!(first.wanted, ["mg", "hg"], "both asked");
        assert_eq!(m.summary(), "reading…");
        // Due again by the clock, but the first read is still running: no second one.
        m.poll(&tasks, &allowed, 1100);
        // One release for each account's run.
        release.send(()).unwrap();
        release.send(()).unwrap();
        // Then, polled again before the next read is due, the first read's result shows.
        let deadline = Instant::now() + Duration::from_secs(60);
        let s = loop {
            let s = m.poll(&tasks, &allowed, 1001);
            if !s.states.is_empty() || Instant::now() > deadline {
                break s;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(
            s.states.get(&("hg".to_string(), "7b5e8856".to_string())),
            Some(&SessionState::NeedsInput)
        );
        assert_eq!(reads.load(Ordering::SeqCst), 2, "one round: mg and hg");
        // Back after a long while (the window was hidden): a new read starts and, while it runs,
        // the old one is too old to show.
        assert_eq!(
            m.poll(&tasks, &allowed, 1000 + MAX_AGE_SECS),
            s,
            "still fresh enough"
        );
        let stale = m.poll(&tasks, &allowed, 5000);
        assert!(stale.states.is_empty() && stale.accounts.is_empty());
        release.send(()).unwrap();
        release.send(()).unwrap();
        assert_eq!(
            unreleased.load(Ordering::SeqCst),
            0,
            "no read waited on its own: the poll never held one up"
        );
    }

    #[test]
    fn settings_says_on_with_the_accounts_or_not_available_with_why() {
        let snap = |accounts: Vec<(&str, Result<usize, &str>)>| Snapshot {
            accounts: accounts
                .into_iter()
                .map(|(a, r)| (a.to_string(), r.map_err(String::from)))
                .collect(),
            ..Snapshot::default()
        };
        assert_eq!(summary_of(&snap(vec![("hg", Ok(2))])), "on (1 account)");
        assert_eq!(
            summary_of(&snap(vec![("hg", Ok(2)), ("mg", Ok(0))])),
            "on (2 accounts)"
        );
        assert_eq!(
            summary_of(&snap(vec![
                ("hg", Ok(2)),
                ("mg", Err("its output isn't JSON"))
            ])),
            "on (1 account); cc-mg not available: its output isn't JSON"
        );
        assert_eq!(
            summary_of(&snap(vec![("hg", Err("claude isn't installed"))])),
            "not available: claude isn't installed"
        );
        assert_eq!(
            summary_of(&snap(vec![
                ("mg", Err("claude isn't installed")),
                ("hg", Err("claude isn't installed"))
            ])),
            "not available: cc-mg: claude isn't installed; cc-hg: claude isn't installed"
        );
        // Through the monitor: a source that fails says so.
        let fetch: Fetch =
            Arc::new(|_: &Account| Err("claude agents exited with code 1".to_string()));
        let m = Monitor::new(fetch, inline);
        assert_eq!(
            m.summary(),
            "not read now: no task in Building or Testing has a session stamp"
        );
        m.poll(&[task("0001", "building", A)], &allowed(), 1000);
        assert_eq!(
            m.summary(),
            "not available: cc-mg: claude agents exited with code 1; cc-hg: claude agents exited with code 1"
        );
    }
}
