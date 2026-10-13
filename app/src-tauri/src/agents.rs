//! Claude Code's own word on its sessions (task 0048): `claude agents --json --all`, run once per
//! account the open work's stamps name (0030), off the main thread, cached, at most every
//! `EVERY_SECS`. It tells a session that's working from one waiting for input (a permission
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

use crate::queue::Task;
use crate::session;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
/// config folder is `config_dir` (None: the default `~/.claude`), with the router and token
/// variables taken out (`helper::STRIP_ENV`) and its usage reporting off (`QUIET_ENV`). Its
/// output, at most `MAX_OUTPUT` + 1 bytes, or why not: not installed, a non-zero exit, or still
/// running after `timeout` (then it's stopped).
pub fn run(claude: &Path, config_dir: Option<&Path>, timeout: Duration) -> Result<Vec<u8>, String> {
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
    if let Some(dir) = config_dir {
        cmd.env("CLAUDE_CONFIG_DIR", dir);
    }
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
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                stop(&mut child);
                return Err(format!("claude agents didn't finish within {timeout:?}"));
            }
            Err(e) => {
                stop(&mut child);
                return Err(format!("couldn't wait for claude ({})", e.kind()));
            }
        }
    };
    let left = deadline.saturating_duration_since(Instant::now());
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

/// Where the states come from: one account's raw output (by its label, "" for the default
/// account), or why there's none.
pub type Fetch = Arc<dyn Fn(&str) -> Result<Vec<u8>, String> + Send + Sync>;

/// How a refresh is run: on a thread of its own in the app (`on_thread`), in place in tests.
pub type Spawn = fn(Box<dyn FnOnce() + Send>);

/// The app's `Spawn`: off the thread that polls.
pub fn on_thread(job: Box<dyn FnOnce() + Send>) {
    std::thread::spawn(job);
}

/// The real source: the owner's `claude` (found where the helper finds it), once per account,
/// `CLAUDE_CONFIG_DIR=~/.claude-<label>` as `session.rs` reads it back.
pub fn claude_fetch(home: PathBuf) -> Fetch {
    Arc::new(move |account: &str| {
        let claude = crate::helper::find_claude(&home).ok_or("claude isn't installed")?;
        let dir = if account.is_empty() {
            None
        } else {
            let dir = home.join(format!(".claude-{account}"));
            if !dir.is_dir() {
                return Err(format!("there's no ~/.claude-{account} folder"));
            }
            Some(dir)
        };
        run(&claude, dir.as_deref(), TIMEOUT)
    })
}

/// What one round of reading found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    /// (account label, short id) → state, from every account that could be read.
    pub states: HashMap<(String, String), SessionState>,
    /// Each account read, in order: how many sessions it listed, or why it couldn't be read.
    pub accounts: Vec<(String, Result<usize, String>)>,
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

/// The accounts to read: each one named in a stamp that `stamp_of` takes, once, sorted. Empty
/// when no task in Building or Testing has a stamped holder: then nothing runs.
pub fn wanted(tasks: &[Task]) -> Vec<String> {
    let mut out: Vec<String> = tasks
        .iter()
        .filter_map(stamp_of)
        .map(|s| s.account)
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Read every account in `accounts` through `fetch`.
pub fn read_all(fetch: &Fetch, accounts: &[String]) -> Snapshot {
    let mut snap = Snapshot::default();
    for account in accounts {
        let read = fetch(account).and_then(|out| parse(&out));
        let result = read.map(|entries| {
            let n = entries.len();
            for e in entries {
                snap.states.insert((account.clone(), e.id), e.state);
            }
            n
        });
        snap.accounts.push((account.clone(), result));
    }
    snap
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
pub fn apply(tasks: &mut [Task], snap: &Snapshot) {
    let h = holds(tasks.iter());
    apply_with(tasks, &h, snap);
}

/// Give each task its session's state. One applies when the holder's stamp (`stamp_of`) names
/// the session and its account, and the session holds nothing else (`holds`): in Building, no
/// other claim; in Testing, no claim and no other Testing hold. A coordinator's subagents share
/// its id, and a builder that passed a task to Testing and claimed the next is working on that
/// one, so its state says nothing about the others. Ended shows only in Building: in Testing the
/// holder is the builder that passed it, whose session ending is the normal course. Every other
/// task is unknown, so its card reads as before. A session waiting for input waits on the owner,
/// so the task counts in the needs-you count.
pub fn apply_with(tasks: &mut [Task], holds: &Holds, snap: &Snapshot) {
    let n = |m: &HashMap<String, usize>, id: &str| m.get(id).copied().unwrap_or(0);
    for t in tasks.iter_mut() {
        let building = t.stage == "building";
        t.session_state = match stamp_of(t) {
            Some(s)
                if (building && n(&holds.claims, &s.id) == 1)
                    || (!building
                        && n(&holds.claims, &s.id) == 0
                        && n(&holds.testing, &s.id) == 1) =>
            {
                match snap.states.get(&(s.account, s.id)).copied() {
                    Some(SessionState::Ended) if !building => SessionState::Unknown,
                    state => state.unwrap_or_default(),
                }
            }
            _ => SessionState::Unknown,
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
    /// The accounts the latest poll wanted.
    wanted: Vec<String>,
}

impl Inner {
    /// The snapshot, unless it's older than `MAX_AGE_SECS` at `now` (or from a clock ahead of it).
    fn fresh(&self, now: u64) -> Snapshot {
        let age = now.checked_sub(self.snapshot_at);
        if age.is_some_and(|a| a <= MAX_AGE_SECS) {
            self.snapshot.clone()
        } else {
            Snapshot::default()
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

    /// Called with every state read at `now` (seconds): when some task has a stamped holder in
    /// Building or Testing, no read is running and the last started `EVERY_SECS` or more ago
    /// (or the clock went back), a read of the accounts they name starts through `spawn`. Returns
    /// the latest snapshot; an empty one when nothing is wanted.
    pub fn poll(self: &Arc<Self>, tasks: &[Task], now: u64) -> Snapshot {
        let wanted = wanted(tasks);
        {
            let mut g = lock(&self.inner);
            g.wanted = wanted.clone();
            if wanted.is_empty() {
                return Snapshot::default();
            }
            let due = g
                .last
                .is_none_or(|last| now >= last.saturating_add(EVERY_SECS) || now < last);
            if !due || g.running {
                return g.fresh(now);
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
        lock(&self.inner).fresh(now)
    }

    /// Settings' line (0036's Agents): "on (2 accounts)", or "not available: <why>", so the
    /// fallback to heartbeats is never silent.
    pub fn summary(&self) -> String {
        let g = lock(&self.inner);
        if g.wanted.is_empty() {
            return "not read now: no task in Building or Testing has a session stamp".into();
        }
        if g.last.is_none() || (g.running && g.snapshot.accounts.is_empty()) {
            return "reading…".into();
        }
        summary_of(&g.snapshot)
    }
}

/// An account as the window names it: `cc-hg`, or "the default account".
fn account_name(label: &str) -> String {
    if label.is_empty() {
        "the default account".into()
    } else {
        format!("cc-{label}")
    }
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
        let out = run(&claude, Some(&dir), Duration::from_secs(5)).unwrap();
        assert_eq!(
            parse(&out).unwrap(),
            [entry("7b5e8856", SessionState::Working)]
        );
        assert_eq!(read("claude.args"), "agents\n--json\n--all\n");
        assert_eq!(read("claude.env"), dir.to_string_lossy());
        assert_eq!(read("claude.quiet"), "1 1 1 ", "its usage reporting is off");
        // The default account: no CLAUDE_CONFIG_DIR at all, whatever this process has.
        run(&claude, None, Duration::from_secs(5)).unwrap();
        assert_eq!(read("claude.env"), "unset");
    }

    #[test]
    fn the_runner_falls_back_with_the_reason_on_a_missing_binary_an_exit_or_a_timeout() {
        let d = tempfile::tempdir().unwrap();
        let t = Duration::from_secs(5);
        assert_eq!(
            run(&d.path().join("no-claude"), None, t).unwrap_err(),
            "claude isn't installed"
        );
        let failing = fake(d.path(), "echo '[]'; exit 3");
        assert_eq!(
            run(&failing, None, t).unwrap_err(),
            "claude agents exited with code 3"
        );
        let slow = fake(d.path(), "exec sleep 30");
        let started = Instant::now();
        assert_eq!(
            run(&slow, None, Duration::from_millis(300)).unwrap_err(),
            "claude agents didn't finish within 300ms"
        );
        assert!(started.elapsed() < Duration::from_secs(5), "it was stopped");
        // A child it started (holding the pipe) is stopped with it: the whole group goes.
        let parent = fake(
            d.path(),
            "sleep 30 &\necho $! > \"$0.child\"\nexec sleep 30",
        );
        assert!(run(&parent, None, Duration::from_millis(300))
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
        let deadline = Instant::now() + Duration::from_secs(5);
        while alive() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!alive(), "the child it started was stopped too");
        // Output past the cap: stopped reading, not available.
        let loud = fake(d.path(), "head -c 2000000 /dev/zero | tr '\\0' ' '");
        assert_eq!(run(&loud, None, t).unwrap_err(), "its output is over 1 MB");
        assert_eq!(TIMEOUT, Duration::from_secs(10));
    }

    #[test]
    fn the_claude_source_needs_the_accounts_folder_and_finds_claude_where_the_helper_does() {
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join(".local/bin");
        fs::create_dir_all(&bin).unwrap();
        fake(&bin, RECORD);
        fs::create_dir(home.path().join(".claude-hg")).unwrap();
        let fetch = claude_fetch(home.path().to_path_buf());
        assert_eq!(
            fetch("mg").unwrap_err(),
            "there's no ~/.claude-mg folder",
            "no path with the user's name in it"
        );
        assert!(parse(&fetch("hg").unwrap()).is_ok());
        assert_eq!(
            fs::read_to_string(bin.join("claude.env")).unwrap(),
            home.path().join(".claude-hg").to_string_lossy()
        );
        fetch("").unwrap();
        assert_eq!(fs::read_to_string(bin.join("claude.env")).unwrap(), "unset");
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
            accounts: Vec::new(),
        }
    }

    fn states(tasks: &[Task]) -> Vec<SessionState> {
        tasks.iter().map(|t| t.session_state).collect()
    }

    #[test]
    fn a_state_applies_only_to_the_one_open_task_its_session_holds_in_its_account() {
        use SessionState::*;
        let s = snap(&[("hg", "7b5e8856", Working), ("hg", "4d517d9d", NeedsInput)]);
        let mut tasks = vec![task("0001", "building", A)];
        apply(&mut tasks, &s);
        assert_eq!(states(&tasks), [Working]);
        assert!(!tasks[0].waiting_for_owner);
        // The same id in another account's list isn't this session.
        let mut other = vec![task("0001", "building", "wt-a (session 7b5e8856 · cc-mg)")];
        apply(&mut other, &s);
        assert_eq!(states(&other), [Unknown]);
        // A coordinator's subagents share its id: neither task gets its state.
        let mut shared = vec![
            task("0001", "building", A),
            task(
                "0002",
                "building",
                "wt-b (wt-0049 · session 7b5e8856 · cc-hg)",
            ),
        ];
        apply(&mut shared, &s);
        assert_eq!(states(&shared), [Unknown, Unknown]);
        // A builder that passed a task to Testing (it's `held-by` there) and claimed the next one:
        // its state is the new claim's, and says nothing about the task in Testing.
        let mut testing = vec![task("0001", "building", A), task("0003", "testing", A)];
        apply(&mut testing, &s);
        assert_eq!(states(&testing), [Working, Unknown]);
        // In Testing with nothing claimed, one task: its state; two: neither.
        let mut alone = vec![task("0003", "testing", A)];
        apply(&mut alone, &s);
        assert_eq!(states(&alone), [Working]);
        let mut two = vec![task("0003", "testing", A), task("0009", "testing", A)];
        apply(&mut two, &s);
        assert_eq!(states(&two), [Unknown, Unknown]);
        // Your check's holder isn't working on it: it doesn't count either way.
        let mut checked = vec![task("0001", "building", A), task("0010", "your-check", A)];
        apply(&mut checked, &s);
        assert_eq!(states(&checked), [Working, Unknown]);
        let mut done = vec![task("0001", "building", A), task("0004", "done", A)];
        apply(&mut done, &s);
        assert_eq!(states(&done), [Working, Unknown]);
        // No stamp, the owner, or a stage nobody works in: today's lines.
        let mut rest = vec![
            task("0005", "building", "wt-a"),
            task("0006", "building", "owner (session 4d517d9d · cc-hg)"),
            task("0007", "your-check", "qa (session 4d517d9d · cc-hg)"),
            task("0008", "building", "wt-z (session 99999999 · cc-hg)"),
        ];
        apply(&mut rest, &s);
        assert_eq!(states(&rest), [Unknown, Unknown, Unknown, Unknown]);
        // Nothing read: unknown, and a state set by an earlier read is cleared.
        apply(&mut tasks, &Snapshot::default());
        assert_eq!(states(&tasks), [Unknown]);
    }

    #[test]
    fn waiting_for_input_waits_on_the_owner_and_ended_shows_only_in_building() {
        use SessionState::*;
        let b = "wt-b (session 4d517d9d · cc-hg)";
        let s = snap(&[("hg", "7b5e8856", NeedsInput), ("hg", "4d517d9d", Ended)]);
        let mut tasks = vec![task("0001", "building", A), task("0002", "testing", b)];
        assert!(!tasks[0].waiting_for_owner);
        apply(&mut tasks, &s);
        // In Testing, `held-by` is the builder that passed it, whose session has usually ended:
        // that isn't news, so Testing reads as before.
        assert_eq!(states(&tasks), [NeedsInput, Unknown]);
        let mut building = vec![task("0002", "building", b)];
        apply(&mut building, &s);
        assert_eq!(states(&building), [Ended]);
        assert!(tasks[0].waiting_for_owner, "counts in the needs-you count");
        assert!(!tasks[1].waiting_for_owner);
        let json = serde_json::to_value(&tasks[0]).unwrap();
        assert_eq!(json["sessionState"], "needs-input");
        // The default account's sessions have no cc- part, and match the default account's list.
        let mut plain = vec![task("0003", "building", "wt-c (session 11111111)")];
        apply(&mut plain, &snap(&[("", "11111111", Working)]));
        assert_eq!(states(&plain), [Working]);
    }

    #[test]
    fn a_paused_task_gets_no_session_state_and_isnt_read_for() {
        use SessionState::*;
        // Its session was told to stop (0045): whatever Claude Code says, the task is paused, not
        // working, and a prompt in that session isn't the paused task waiting on the owner.
        let paused = |mut t: Task| {
            t.paused = "2026-10-13 10:00".into();
            t
        };
        for state in [Working, NeedsInput, Ended] {
            let mut tasks = vec![paused(task("0001", "building", A))];
            apply(&mut tasks, &snap(&[("hg", "7b5e8856", state)]));
            assert_eq!(states(&tasks), [Unknown], "{state:?}");
            assert!(!tasks[0].waiting_for_owner, "{state:?}");
        }
        assert!(wanted(&[paused(task("0001", "building", A))]).is_empty());
        let mut resumed = vec![task("0001", "building", A)];
        apply(&mut resumed, &snap(&[("hg", "7b5e8856", NeedsInput)]));
        assert_eq!(states(&resumed), [NeedsInput]);
        // Paused, its session was told to stop: the task isn't what the session is doing, so it
        // doesn't count as its hold, and the session's next claim gets the state.
        let mut next = vec![
            paused(task("0001", "building", A)),
            task("0002", "building", A),
        ];
        apply(&mut next, &snap(&[("hg", "7b5e8856", NeedsInput)]));
        assert_eq!(states(&next), [Unknown, NeedsInput]);
        assert!(!next[0].waiting_for_owner && next[1].waiting_for_owner);
        let mut testing = vec![
            paused(task("0003", "testing", A)),
            task("0004", "testing", A),
        ];
        apply(&mut testing, &snap(&[("hg", "7b5e8856", Working)]));
        assert_eq!(states(&testing), [Unknown, Working]);
    }

    #[test]
    fn holds_count_across_every_queue_given() {
        use SessionState::*;
        let s = snap(&[("hg", "7b5e8856", NeedsInput)]);
        let mut here = vec![task("0001", "building", A)];
        // The same session holds a task in another queue: its state is about neither.
        let elsewhere = vec![task("0101", "building", A)];
        let h = holds(here.iter().chain(&elsewhere));
        apply_with(&mut here, &h, &s);
        assert_eq!(states(&here), [Unknown]);
        assert!(!here[0].waiting_for_owner);
        // Another queue's tasks under other sessions change nothing.
        let unrelated = vec![task("0102", "building", "wt-z (session 99999999 · cc-hg)")];
        let h = holds(here.iter().chain(&unrelated));
        apply_with(&mut here, &h, &s);
        assert_eq!(states(&here), [NeedsInput]);
    }

    #[test]
    fn only_the_accounts_of_stamped_holders_in_building_or_testing_are_read() {
        let tasks = vec![
            task("0001", "building", A),
            task("0002", "testing", "qa (session 4d517d9d · cc-mg)"),
            task("0003", "building", "wt-c (session 11111111)"),
            task("0004", "building", "wt-d (session 22222222 · cc-hg)"),
            task("0005", "your-check", "qa (session 33333333 · cc-ih)"),
            task("0006", "building", "owner (session 44444444 · cc-zz)"),
            task("0007", "building", "wt-unstamped"),
        ];
        assert_eq!(wanted(&tasks), ["", "hg", "mg"]);
        assert!(wanted(&tasks[4..]).is_empty());
    }

    /// A source that counts its reads per account and answers `out`.
    fn counting(out: &'static str) -> (Fetch, Arc<AtomicUsize>) {
        let n = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&n);
        let fetch: Fetch = Arc::new(move |_account: &str| {
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
        // Nothing in Building or Testing with a stamp: nothing runs, however often it's polled.
        let idle = vec![task("0001", "building", "wt-a"), task("0002", "ready", "")];
        for now in [1000, 1015, 1030, 5000] {
            assert_eq!(m.poll(&idle, now), Snapshot::default());
        }
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        assert!(m.summary().starts_with("not read now"), "{}", m.summary());
        // With one: read at once, then not again until 15 s have passed on the clock.
        let tasks = vec![task("0001", "building", A)];
        let s = m.poll(&tasks, 2000);
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        assert_eq!(
            s.states.get(&("hg".to_string(), "7b5e8856".to_string())),
            Some(&SessionState::Working)
        );
        for now in [2001, 2010, 2014] {
            assert_eq!(m.poll(&tasks, now), s, "cached at {now}");
        }
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        m.poll(&tasks, 2015);
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        // A clock that went back starts a read rather than waiting forever.
        m.poll(&tasks, 100);
        assert_eq!(reads.load(Ordering::SeqCst), 3);
        assert_eq!(m.summary(), "on (1 account)");
    }

    #[test]
    fn the_monitor_reads_off_the_polling_thread_and_never_twice_at_once() {
        let (release, wait) = mpsc::channel::<()>();
        let wait = Mutex::new(wait);
        let reads = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&reads);
        let fetch: Fetch = Arc::new(move |_account: &str| {
            seen.fetch_add(1, Ordering::SeqCst);
            // Hold the read until the test lets it go, like a slow `claude`.
            let _ = wait.lock().unwrap().recv_timeout(Duration::from_secs(10));
            Ok(br#"[{"sessionId": "7b5e8856", "state": "blocked"}]"#.to_vec())
        });
        let m = Monitor::new(fetch, on_thread);
        let tasks = vec![task("0001", "building", A)];
        let started = Instant::now();
        let first = m.poll(&tasks, 1000);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "the poll didn't wait"
        );
        assert_eq!(first, Snapshot::default(), "nothing read yet");
        assert_eq!(m.summary(), "reading…");
        // Due again by the clock, but the first read is still running: no second one.
        m.poll(&tasks, 1100);
        release.send(()).unwrap();
        // Then, polled again before the next read is due, the first read's result shows.
        let deadline = Instant::now() + Duration::from_secs(10);
        let s = loop {
            let s = m.poll(&tasks, 1001);
            if !s.states.is_empty() || Instant::now() > deadline {
                break s;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(
            s.states.get(&("hg".to_string(), "7b5e8856".to_string())),
            Some(&SessionState::NeedsInput)
        );
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        // Back after a long while (the window was hidden): a new read starts and, while it runs,
        // the old one is too old to show.
        assert_eq!(m.poll(&tasks, 1000 + MAX_AGE_SECS), s, "still fresh enough");
        assert_eq!(m.poll(&tasks, 5000), Snapshot::default());
        release.send(()).unwrap();
    }

    #[test]
    fn settings_says_on_with_the_accounts_or_not_available_with_why() {
        let snap = |accounts: Vec<(&str, Result<usize, &str>)>| Snapshot {
            states: HashMap::new(),
            accounts: accounts
                .into_iter()
                .map(|(a, r)| (a.to_string(), r.map_err(String::from)))
                .collect(),
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
                ("", Err("claude isn't installed")),
                ("hg", Err("claude isn't installed"))
            ])),
            "not available: the default account: claude isn't installed; cc-hg: claude isn't installed"
        );
        // Through the monitor: a source that fails says so.
        let fetch: Fetch = Arc::new(|_: &str| Err("claude agents exited with code 1".to_string()));
        let m = Monitor::new(fetch, inline);
        assert_eq!(
            m.summary(),
            "not read now: no task in Building or Testing has a session stamp"
        );
        m.poll(&[task("0001", "building", A)], 1000);
        assert_eq!(
            m.summary(),
            "not available: claude agents exited with code 1"
        );
    }
}
