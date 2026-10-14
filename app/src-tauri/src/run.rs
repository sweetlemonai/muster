//! Run from the app (task 0079): a project's run command (0077, projects.json `run`) started in
//! its main checkout, so the owner can try what they're accepting without a terminal.
//!
//! The command is split into words without a shell (`cli::split_words`, again on every start: a
//! hand-edited one may hold an unclosed quote, and then it's refused, never handed to a shell),
//! and started as the gate is (`parallel::gate_command`): a relative program with a `/` found
//! from the checkout, no stdin, and without the queue variables. It leads a process group of its
//! own, so Stop, Restart and Muster quitting end everything it started: SIGTERM to the group,
//! then SIGKILL after `GRACE`, through `/bin/kill` with an argument array as `agents` does (the
//! standard library has no group kill).
//!
//! One run per project, by name; Run while one runs restarts it. Its output, both streams, goes
//! into a bounded ring (`Ring`) in memory only, never logged or written to disk, and to the window
//! as `Snap` events from a thread of its own, at most one every `BATCH`. Readers and the waiter run
//! on threads of their own too: nothing here blocks the main thread, and nothing polls the
//! process. A run has ended once its leader exited and both its streams closed: while something it
//! started still writes there, it runs.

use crate::projects::Project;
use crate::{cli, layout, parallel, projects, queue};
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::{OsStr, OsString};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

/// The most lines a run's ring keeps; older ones fall out.
pub const KEEP_LINES: usize = 2000;
/// The most bytes of text a run's ring keeps, whichever bound comes first.
pub const KEEP_BYTES: usize = 256 * 1024;
/// The most bytes of one line kept; the rest of it is cut, with "…".
pub const LONGEST_LINE: usize = 4096;
/// The least time between two of a run's events to the window.
pub const BATCH: Duration = Duration::from_millis(200);
/// How long Stop waits after SIGTERM before SIGKILL.
pub const GRACE: Duration = Duration::from_secs(5);
/// How long Stop waits for the streams to close after SIGKILL, before it calls the run stopped.
const KILL_WAIT: Duration = Duration::from_secs(2);

/// Where a run stands.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Status {
    Running,
    /// It ended on its own: with its exit code, None when a signal ended it.
    Exited {
        code: Option<i32>,
    },
    /// Stop, Restart or Muster quitting ended it.
    Stopped,
}

/// A run as the window sees it: the whole ring from `run_state`, or what's new in an event.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snap {
    /// The project's name.
    pub project: String,
    /// The project's key, its queue folder, which the window matches events on: a name can pass
    /// to another project while the run goes on.
    pub key: String,
    /// Counts up across every run, so the window tells a restart's run from the one before.
    pub id: u64,
    /// The command as it was started, and the folder it runs in.
    pub command: String,
    pub folder: String,
    pub status: Status,
    /// When it started and ended, in seconds since 1970.
    pub since: u64,
    pub ended: Option<u64>,
    /// Where `lines[0]` is among every line the run printed, counted from 0.
    pub first: u64,
    pub lines: Vec<String>,
}

/// What Run knows of a project: its command, where it runs, why it can't, and its latest run.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub project: String,
    /// The project's key (its queue folder): its run's events carry it.
    pub key: String,
    /// Its run command as projects.json has it, "" for none.
    pub command: String,
    /// The main checkout it runs in, "" when there's none.
    pub folder: String,
    /// Why Run can't start it, "" when it can.
    pub refused: String,
    /// Its latest run since Muster started, with the whole ring; None before the first.
    pub run: Option<Snap>,
}

/// A run's last lines: at most `KEEP_LINES` and `KEEP_BYTES`, the oldest going first.
#[derive(Debug, Default)]
pub struct Ring {
    lines: VecDeque<String>,
    bytes: usize,
    /// Every line pushed, kept or not.
    total: u64,
}

impl Ring {
    pub fn push(&mut self, line: String) {
        self.bytes += line.len() + 1;
        self.lines.push_back(line);
        self.total += 1;
        while self.lines.len() > KEEP_LINES || self.bytes > KEEP_BYTES {
            let Some(old) = self.lines.pop_front() else {
                break;
            };
            self.bytes -= old.len() + 1;
        }
    }

    /// Where the oldest line kept is among every line pushed.
    pub fn first(&self) -> u64 {
        self.total - self.lines.len() as u64
    }

    /// The lines kept from the `n`th on (from the oldest kept when that fell out), and where the
    /// first of them is.
    pub fn since(&self, n: u64) -> (u64, Vec<String>) {
        let from = n.clamp(self.first(), self.total);
        let skip = usize::try_from(from - self.first()).unwrap_or(usize::MAX);
        (from, self.lines.iter().skip(skip).cloned().collect())
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn total(&self) -> u64 {
        self.total
    }
}

/// Reads `from` line by line until it closes, calling `on_line` with each: `\n` or `\r\n` ends a
/// line, text that isn't UTF-8 is read leniently, and a line past `LONGEST_LINE` bytes is cut
/// with "…" (the rest of it is read and dropped), so one endless line can't fill memory.
pub fn read_lines(from: impl Read, mut on_line: impl FnMut(String)) {
    let mut r = BufReader::with_capacity(64 * 1024, from);
    let mut line: Vec<u8> = Vec::new();
    let mut cut = false;
    let mut finish = |line: &mut Vec<u8>, cut: &mut bool| {
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        let mut text = String::from_utf8_lossy(line).into_owned();
        if *cut {
            text.push('…');
        }
        line.clear();
        *cut = false;
        on_line(text);
    };
    loop {
        let buf = match r.fill_buf() {
            Ok([]) => break,
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        let end = buf.iter().position(|&b| b == b'\n');
        let chunk = &buf[..end.unwrap_or(buf.len())];
        let room = LONGEST_LINE.saturating_sub(line.len());
        if chunk.len() > room {
            line.extend_from_slice(&chunk[..room]);
            cut = true;
        } else {
            line.extend_from_slice(chunk);
        }
        let used = chunk.len() + usize::from(end.is_some());
        r.consume(used);
        if end.is_some() {
            finish(&mut line, &mut cut);
        }
    }
    if !line.is_empty() || cut {
        finish(&mut line, &mut cut);
    }
}

/// What a start runs: the command, its words, and the checkout.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub command: String,
    pub argv: Vec<String>,
    pub folder: PathBuf,
}

/// How Run would start `p`, or why it can't: it has no run command, the command doesn't split
/// into words or isn't one projects.json keeps, or there's no main checkout.
pub fn plan(p: &Project) -> Result<Plan, String> {
    let command = p.run.trim();
    if command.is_empty() {
        return Err(format!(
            "{name} has no run command: muster projects set {name} run \"<command>\"",
            name = p.name
        ));
    }
    if let Some(why) = projects::command_refused("run", command) {
        return Err(format!("{}: {why}", p.name));
    }
    let argv = cli::split_words(command)
        .filter(|w| !w.is_empty())
        .ok_or_else(|| {
            format!(
                "{}'s run command {command:?} has an unclosed quote or a trailing backslash: set it again",
                p.name
            )
        })?;
    Ok(Plan {
        command: command.to_string(),
        argv,
        folder: checkout(Path::new(&p.path))?,
    })
}

/// The main checkout of the project whose queue is `queue`: the repo a `.muster/` queue is in;
/// for an old-layout `<project>/tasks`, the `<project>/main` (or `master`) git checkout beside
/// it, else none.
pub fn checkout(queue: &Path) -> Result<PathBuf, String> {
    if layout::is_muster(queue) {
        return queue
            .parent()
            .filter(|c| c.is_dir())
            .map(Path::to_path_buf)
            .ok_or_else(|| format!("{} has no checkout to run in", queue.display()));
    }
    let project = queue.parent().unwrap_or(queue);
    layout::MAIN_NAMES
        .iter()
        .map(|n| project.join(n))
        .find(|c| c.join(".git").exists())
        .ok_or_else(|| {
            format!(
                "no checkout to run in: {} is an old-layout queue, and {} isn't a git checkout",
                queue.display(),
                project.join("main").display()
            )
        })
}

/// Where a run's events go: the window, or a test's list.
pub type Emit = Arc<dyn Fn(&Snap) + Send + Sync>;

/// Every project's latest run since Muster started, by project name.
pub struct Runs {
    runs: Mutex<HashMap<String, Arc<Live>>>,
    /// Starts and stops one at a time, so two clicks can't leave two runs of one project.
    ops: Mutex<()>,
    next: AtomicU64,
    emit: Emit,
    grace: Duration,
    /// The owner's login shell, whose PATH runs get (`login_path`); None reads none.
    shell: Option<PathBuf>,
    /// That PATH, read once; None when there's no shell or it didn't print one in time.
    login: Arc<OnceLock<Option<OsString>>>,
}

/// One run, shared by its threads and `Runs`.
struct Live {
    st: Mutex<St>,
    changed: Condvar,
}

struct St {
    project: String,
    key: String,
    id: u64,
    command: String,
    folder: String,
    status: Status,
    since: u64,
    ended: Option<u64>,
    ring: Ring,
    /// The leader's process id, which is its group's id.
    pgid: u32,
    /// The leader's exit, once reaped: its code, None for a signal.
    leader: Option<Option<i32>>,
    /// Output streams still open.
    streams: u8,
    stop_asked: bool,
    /// What the window hasn't been sent yet: new lines (from `sent` on), or a new status.
    dirty: bool,
    sent: u64,
}

impl St {
    /// Ended once the leader exited and both streams closed.
    fn settle(&mut self) {
        if self.status != Status::Running || self.streams > 0 {
            return;
        }
        let Some(code) = self.leader else {
            return;
        };
        self.status = if self.stop_asked {
            Status::Stopped
        } else {
            Status::Exited { code }
        };
        self.ended = Some(queue::now_secs());
        self.dirty = true;
    }

    /// The run with its lines from the `n`th on.
    fn snap(&self, n: u64) -> Snap {
        let (first, lines) = self.ring.since(n);
        Snap {
            project: self.project.clone(),
            key: self.key.clone(),
            id: self.id,
            command: self.command.clone(),
            folder: self.folder.clone(),
            status: self.status.clone(),
            since: self.since,
            ended: self.ended,
            first,
            lines,
        }
    }
}

impl Live {
    fn lock(&self) -> MutexGuard<'_, St> {
        self.st.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn running(&self) -> bool {
        self.lock().status == Status::Running
    }

    /// Waits until it has ended, or `until`.
    fn wait_end(&self, until: Instant) {
        let mut st = self.lock();
        while st.status == Status::Running {
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return;
            }
            st = self
                .changed
                .wait_timeout(st, left)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }

    /// Calls a run that couldn't be watched stopped, so nothing waits on it.
    fn abandon(&self) {
        let mut st = self.lock();
        st.stop_asked = true;
        if st.status == Status::Running {
            st.status = Status::Stopped;
            st.ended = Some(queue::now_secs());
            st.dirty = true;
        }
        drop(st);
        self.changed.notify_all();
    }

    /// Sends `signal` (`-TERM`, `-KILL`) to the run's process group, and with KILL to the leader
    /// too while it isn't reaped, in case it left its group. The leader isn't reaped while the run
    /// goes (`wait_leader`), so its id still names this run's group.
    fn signal(&self, signal: &str) {
        let (pgid, leader_alive) = {
            let st = self.lock();
            (st.pgid, st.leader.is_none())
        };
        let mut args = vec![signal.to_string(), "--".into(), format!("-{pgid}")];
        if signal == "-KILL" && leader_alive {
            args.push(pgid.to_string());
        }
        let _ = Command::new("/bin/kill")
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

impl Runs {
    /// No runs yet; each run's events go to `emit`. Runs get the PATH of the owner's login shell
    /// (`$SHELL`), read now on a thread of its own, so the first Run seldom waits for it.
    pub fn new(emit: Emit) -> Runs {
        let shell = std::env::var_os("SHELL")
            .map(PathBuf::from)
            .filter(|s| s.is_absolute());
        let runs = Runs::with_shell(emit, GRACE, shell);
        let (login, shell) = (runs.login.clone(), runs.shell.clone());
        // Without the thread, the first run reads it instead.
        let _ = thread("run-path", move || {
            login.get_or_init(|| shell.and_then(|s| login_path(&s, LOGIN_WAIT)));
        });
        runs
    }

    /// `new`, waiting `grace` between SIGTERM and SIGKILL, and without a login shell's PATH.
    pub fn with_grace(emit: Emit, grace: Duration) -> Runs {
        Runs::with_shell(emit, grace, None)
    }

    /// `with_grace`, with runs getting the PATH `shell` prints as a login shell.
    pub fn with_shell(emit: Emit, grace: Duration, shell: Option<PathBuf>) -> Runs {
        Runs {
            runs: Mutex::new(HashMap::new()),
            ops: Mutex::new(()),
            next: AtomicU64::new(1),
            emit,
            grace,
            shell,
            login: Arc::new(OnceLock::new()),
        }
    }

    /// The login shell's PATH: read once, by the first to ask (the thread `new` starts, or a run).
    fn login_path(&self) -> Option<&OsStr> {
        self.login
            .get_or_init(|| {
                self.shell
                    .as_deref()
                    .and_then(|s| login_path(s, LOGIN_WAIT))
            })
            .as_deref()
    }

    fn get(&self, project: &str) -> Option<Arc<Live>> {
        self.runs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(project)
            .cloned()
    }

    /// What Run knows of the project named `name` in `list`; refused for a name that isn't one.
    pub fn view(&self, list: &[Project], name: &str) -> Result<View, String> {
        let p = find(list, name)?;
        let (folder, refused) = match plan(p) {
            Ok(plan) => (plan.folder, String::new()),
            Err(why) => (checkout(Path::new(&p.path)).unwrap_or_default(), why),
        };
        Ok(View {
            project: p.name.clone(),
            key: p.key.clone(),
            command: p.run.clone(),
            folder: folder.to_string_lossy().into_owned(),
            refused,
            run: self.get(&p.key).map(|l| {
                let mut snap = l.lock().snap(0);
                snap.project = p.name.clone();
                snap
            }),
        })
    }

    /// Run: starts the run command of the project named `name` in `list`, restarting a run of it
    /// that's still going. Refused for a name that isn't a project, and for what `plan` refuses.
    pub fn start_named(&self, list: &[Project], name: &str) -> Result<View, String> {
        let p = find(list, name)?;
        self.start_as(&p.key, &p.name, plan(p)?)?;
        self.view(list, name)
    }

    /// Stop: ends the run of the project named `name` in `list`, if one is going.
    pub fn stop_named(&self, list: &[Project], name: &str) -> Result<View, String> {
        let p = find(list, name)?;
        self.stop(&p.key);
        self.view(list, name)
    }

    /// Starts `plan` as `project`'s run, stopping the one before if it's still going.
    pub fn start(&self, project: &str, plan: Plan) -> Result<Snap, String> {
        self.start_as(project, project, plan)
    }

    /// `start`, keeping the run under `key` (the project's queue folder, which a rename or another
    /// project's coming or going doesn't change) and naming it `name` to the window.
    pub fn start_as(&self, key: &str, name: &str, plan: Plan) -> Result<Snap, String> {
        // Before the lock: reading it may take up to `LOGIN_WAIT`, and Stop shouldn't wait on it.
        let login = self.login_path();
        let _one = self.ops.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(old) = self.get(key) {
            self.end(&[old]);
        }
        let Some(prog) = plan.argv.first() else {
            return Err("the run command is empty".into());
        };
        let mut child = command(&plan, login)
            .spawn()
            .map_err(|e| format!("couldn't start {prog:?} in {}: {e}", plan.folder.display()))?;
        let (out, err) = (child.stdout.take(), child.stderr.take());
        let live = Arc::new(Live {
            st: Mutex::new(St {
                project: name.to_string(),
                key: key.to_string(),
                id: self.next.fetch_add(1, Ordering::Relaxed),
                command: plan.command,
                folder: plan.folder.to_string_lossy().into_owned(),
                status: Status::Running,
                since: queue::now_secs(),
                ended: None,
                ring: Ring::default(),
                pgid: child.id(),
                leader: None,
                streams: u8::from(out.is_some()) + u8::from(err.is_some()),
                stop_asked: false,
                dirty: true,
                sent: 0,
            }),
            changed: Condvar::new(),
        });
        let snap = live.lock().snap(0);
        self.runs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key.to_string(), live.clone());
        // The waiter first: it owns the child, and reaps it whatever else fails to start.
        let slot = Arc::new(Mutex::new(Some(child)));
        let (l, held) = (live.clone(), slot.clone());
        if let Err(e) = thread("run-wait", move || wait_leader(&l, &held)) {
            if let Some(mut child) = slot.lock().unwrap_or_else(|e| e.into_inner()).take() {
                live.signal("-KILL");
                let _ = child.wait();
            }
            live.abandon();
            return Err(e);
        }
        let started = (|| {
            if let Some(out) = out {
                let l = live.clone();
                thread("run-out", move || read_into(&l, out))?;
            }
            if let Some(err) = err {
                let l = live.clone();
                thread("run-err", move || read_into(&l, err))?;
            }
            let (l, emit) = (live.clone(), self.emit.clone());
            thread("run-send", move || send(&l, &emit))
        })();
        if let Err(e) = started {
            // Stopped, so the waiter reaps the leader without waiting for streams no one reads.
            live.signal("-KILL");
            live.abandon();
            return Err(e);
        }
        Ok(snap)
    }

    /// Stops the run kept under `project` (a key, or `start`'s name), if one is going. Also for a
    /// project taken off the list, which the window can't name any more.
    pub fn stop(&self, project: &str) {
        let _one = self.ops.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(live) = self.get(project) {
            self.end(&[live]);
        }
    }

    /// Stops every run that's going: Muster is quitting.
    pub fn stop_all(&self) {
        let _one = self.ops.lock().unwrap_or_else(|e| e.into_inner());
        let all: Vec<Arc<Live>> = self
            .runs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect();
        self.end(&all);
    }

    /// The keys (queue folders) of the projects with a run going.
    pub fn running(&self) -> HashSet<String> {
        self.runs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter(|(_, l)| l.running())
            .map(|(p, _)| p.clone())
            .collect()
    }

    /// Ends the runs in `lives` that are going, all at once: SIGTERM to each group, SIGKILL to
    /// those still going after `grace`, and stopped they are once their streams close, or after
    /// `KILL_WAIT` even if something that left the group still holds them.
    fn end(&self, lives: &[Arc<Live>]) {
        let going: Vec<&Arc<Live>> = lives
            .iter()
            .filter(|l| {
                let mut st = l.lock();
                st.stop_asked |= st.status == Status::Running;
                st.status == Status::Running
            })
            .collect();
        for l in &going {
            l.signal("-TERM");
        }
        let until = Instant::now() + self.grace;
        for l in &going {
            l.wait_end(until);
        }
        let left: Vec<&&Arc<Live>> = going.iter().filter(|l| l.running()).collect();
        for l in &left {
            l.signal("-KILL");
        }
        let until = Instant::now() + KILL_WAIT;
        for l in &left {
            l.wait_end(until);
            let mut st = l.lock();
            if st.status == Status::Running {
                st.status = Status::Stopped;
                st.ended = Some(queue::now_secs());
                st.dirty = true;
            }
            drop(st);
            l.changed.notify_all();
        }
    }
}

/// `plan` as a process: as the gate starts (no shell, no stdin, without `MUSTER_ROOT` and
/// `TASKS_ROOT`), both streams piped, leading a group of its own so Stop ends everything it started.
/// Its PATH: `login` (the owner's login shell's, when it was read), Muster's own, and the fixed
/// folders (`search_path`).
fn command(plan: &Plan, login: Option<&OsStr>) -> Command {
    let prog = plan.argv.first().map(String::as_str).unwrap_or_default();
    let program = parallel::program_in(prog, &plan.folder);
    let mut cmd = parallel::gate_command(&program, &plan.argv, &plan.folder);
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    cmd.env("PATH", search_path(&crate::home(), login, &inherited))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    cmd
}

/// The PATH a run gets, each folder once and only absolute ones: the owner's login shell's
/// (`login`, what a terminal has, nvm's node and the like included), then Muster's own (a
/// terminal's when started from one), then where the owner's tools usually live, for when the
/// login shell's couldn't be read. Muster opened from the Dock or Finder has only the system's
/// PATH, where `pnpm`, `cargo` or `node` aren't, so a run command, or what it calls, couldn't be
/// found.
pub fn search_path(home: &Path, login: Option<&OsStr>, inherited: &OsStr) -> OsString {
    let fixed = [home.join(".cargo/bin"), home.join(".local/bin")]
        .into_iter()
        .chain(
            [
                "/opt/homebrew/bin",
                "/usr/local/bin",
                "/usr/bin",
                "/bin",
                "/usr/sbin",
                "/sbin",
            ]
            .map(PathBuf::from),
        );
    let given = login
        .into_iter()
        .chain([inherited])
        .flat_map(std::env::split_paths);
    let mut dirs: Vec<PathBuf> = Vec::new();
    for d in given.chain(fixed) {
        if d.is_absolute() && !dirs.contains(&d) {
            dirs.push(d);
        }
    }
    std::env::join_paths(dirs).unwrap_or_else(|_| inherited.to_os_string())
}

/// How long the login shell gets to print its PATH.
pub const LOGIN_WAIT: Duration = Duration::from_secs(5);
/// The most bytes read from the login shell; its startup files may print more than the PATH.
const LOGIN_OUTPUT: usize = 64 * 1024;

/// Around the PATH the login shell prints, with a number of this call's own, so nothing its
/// startup files print is taken for it.
struct Marks {
    start: String,
    end: String,
}

impl Marks {
    fn new() -> Marks {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or_default();
        let n = format!("{}-{nanos}", std::process::id());
        Marks {
            start: format!("<muster-path {n}>"),
            end: format!("</muster-path {n}>"),
        }
    }

    /// The PATH between the last marks in `text`; None without both, for an empty one, or one
    /// with a line break or NUL in it.
    fn path_in(&self, text: &str) -> Option<OsString> {
        let start = text.rfind(&self.start)? + self.start.len();
        let len = text[start..].find(&self.end)?;
        let path = &text[start..start + len];
        (!path.is_empty() && !path.contains(['\n', '\r', '\0'])).then(|| OsString::from(path))
    }
}

/// The PATH the owner's login shell `shell` sets, as a terminal has it: `shell -ilc` with a
/// `printf` of `$PATH` between marks, as an argument array, no stdin, in a session of its own (so
/// with no controlling terminal: an interactive zsh in a background group of Muster's terminal
/// would stop on SIGTTOU). Read up to the end mark, not to the end of output, since something its
/// startup files leave going may hold the pipe. None when it can't start, fails, prints no PATH,
/// or hasn't finished within `wait` (then it and whatever its startup files started are killed).
/// Run it off the main thread: it takes as long as the owner's startup files (nvm's take most of
/// a second).
pub fn login_path(shell: &Path, wait: Duration) -> Option<OsString> {
    let marks = Marks::new();
    let script = format!("printf '\\n{}%s{}\\n' \"$PATH\"", marks.start, marks.end);
    let mut cmd = Command::new(shell);
    cmd.args(["-ilc", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    new_session(&mut cmd);
    let mut child = cmd.spawn().ok()?;
    let out = child.stdout.take();
    let (tx, rx) = std::sync::mpsc::channel();
    // Read on a thread of its own, so a full pipe never stalls the wait below; it stops at the end
    // mark, at the end of the output, or at `LOGIN_OUTPUT`.
    let end = marks.end.clone();
    let read = out.map(|mut out| {
        thread("run-path-read", move || {
            let (mut buf, mut chunk) = (Vec::new(), [0u8; 4096]);
            while buf.len() < LOGIN_OUTPUT {
                let n = match out.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                };
                buf.extend_from_slice(&chunk[..n]);
                // Only where the mark could be new: this read, and a mark split across two reads.
                let from = buf.len().saturating_sub(n + end.len());
                if buf[from..].windows(end.len()).any(|w| w == end.as_bytes()) {
                    break;
                }
            }
            let _ = tx.send(buf);
        })
    });
    let reading = matches!(read, Some(Ok(())));
    let until = Instant::now() + wait;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) if reading => break status,
            Ok(None) if reading && Instant::now() < until => {
                std::thread::sleep(Duration::from_millis(20))
            }
            _ => {
                // Not reaped yet, so the group's id is still its own: it and whatever its startup
                // files started go.
                let _ = Command::new("/bin/kill")
                    .args(["-KILL", "--", &format!("-{}", child.id())])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    if !status.success() {
        return None;
    }
    // The end mark came before the shell exited; without one, the end of output or `wait`.
    let left = until.saturating_duration_since(Instant::now());
    let buf = rx.recv_timeout(left.max(Duration::from_millis(200))).ok()?;
    marks.path_in(&String::from_utf8(buf).ok()?)
}

/// `cmd` starts in a session of its own, leading its process group, with no controlling terminal.
fn new_session(cmd: &mut Command) {
    extern "C" {
        fn setsid() -> i32;
    }
    // SAFETY: setsid is async-signal-safe and touches no memory of this process; between fork and
    // exec nothing else runs. It fails only for a process that already leads a group, which a
    // freshly forked child doesn't.
    unsafe {
        std::os::unix::process::CommandExt::pre_exec(cmd, || {
            if setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

/// The project named `name` in `list`. The window names a project only by its name, never by a
/// folder or a command.
fn find<'a>(list: &'a [Project], name: &str) -> Result<&'a Project, String> {
    list.iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("no project named {name:?}"))
}

/// Starts `f` on a thread of its own named `name`.
fn thread(name: &str, f: impl FnOnce() + Send + 'static) -> Result<(), String> {
    std::thread::Builder::new()
        .name(name.to_string())
        .spawn(f)
        .map(|_| ())
        .map_err(|e| format!("couldn't start a thread for the run: {e}"))
}

/// The waiter thread: reaps the leader once both streams closed (or the run was called stopped),
/// never before. Until then the leader, even exited, stays a zombie that holds its process id, so
/// no other process group can take that id while Stop may still signal it.
fn wait_leader(live: &Live, child: &Mutex<Option<std::process::Child>>) {
    {
        let mut st = live.lock();
        while st.streams > 0 && st.status == Status::Running {
            st = live.changed.wait(st).unwrap_or_else(|e| e.into_inner());
        }
    }
    let Some(mut child) = child.lock().unwrap_or_else(|e| e.into_inner()).take() else {
        return;
    };
    let code = child.wait().ok().and_then(|s| s.code());
    let mut st = live.lock();
    st.leader = Some(code);
    st.settle();
    drop(st);
    live.changed.notify_all();
}

/// A reader thread: each line of `from` into the run's ring, until it closes.
fn read_into(live: &Live, from: impl Read) {
    read_lines(from, |line| {
        let mut st = live.lock();
        st.ring.push(line);
        st.dirty = true;
        drop(st);
        live.changed.notify_all();
    });
    let mut st = live.lock();
    st.streams = st.streams.saturating_sub(1);
    st.settle();
    drop(st);
    live.changed.notify_all();
}

/// The sending thread: whatever is new, at most once every `BATCH`, until the run has ended and
/// that was sent.
fn send(live: &Live, emit: &Emit) {
    loop {
        let snap = {
            let mut st = live.lock();
            while !st.dirty {
                st = live.changed.wait(st).unwrap_or_else(|e| e.into_inner());
            }
            st.dirty = false;
            let snap = st.snap(st.sent);
            st.sent = st.ring.total();
            snap
        };
        emit(&snap);
        if snap.status != Status::Running {
            return;
        }
        std::thread::sleep(BATCH);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn project(name: &str, path: &Path, run: &str) -> Project {
        Project {
            key: path.to_string_lossy().into_owned(),
            name: name.into(),
            path: path.to_string_lossy().into_owned(),
            source: "added".into(),
            group: String::new(),
            hidden: false,
            ok: true,
            gate: String::new(),
            run: run.into(),
            prefix: String::new(),
            label: String::new(),
            category: String::new(),
            colour: String::new(),
            group_name: String::new(),
        }
    }

    /// A repo with a `.muster/` queue, in `dir`.
    fn muster_repo(dir: &Path) -> PathBuf {
        let q = dir.join("repo/.muster");
        fs::create_dir_all(&q).unwrap();
        fs::create_dir_all(dir.join("repo/.git")).unwrap();
        fs::write(q.join("config.toml"), "").unwrap();
        q
    }

    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, format!("#!/bin/sh\n{body}")).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    /// The events a test's runs sent, each with when.
    type Sent = Arc<Mutex<Vec<(Instant, Snap)>>>;

    /// Runs whose events are kept, each checked to be sent off the caller's thread.
    fn runs(grace: Duration) -> (Runs, Sent) {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let keep = sent.clone();
        let main = std::thread::current().id();
        let emit: Emit = Arc::new(move |s: &Snap| {
            assert_ne!(
                std::thread::current().id(),
                main,
                "sent off the caller's thread"
            );
            keep.lock().unwrap().push((Instant::now(), s.clone()));
        });
        (Runs::with_grace(emit, grace), sent)
    }

    /// The events sent, once `done` holds for them or after 20 s (then the caller's assertions say
    /// what's missing): the sending thread may lag well behind `BATCH` on a busy machine.
    fn sent_once(
        sent: &Sent,
        done: impl Fn(&[(Instant, Snap)]) -> bool,
    ) -> MutexGuard<'_, Vec<(Instant, Snap)>> {
        let until = Instant::now() + Duration::from_secs(20);
        loop {
            let got = sent.lock().unwrap();
            if done(&got) || Instant::now() > until {
                return got;
            }
            drop(got);
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// `sent_once` the last event says the run ended: the sending thread sends nothing after it.
    fn sent_to_the_end(sent: &Sent) -> MutexGuard<'_, Vec<(Instant, Snap)>> {
        sent_once(sent, |s| {
            s.last().is_some_and(|(_, l)| l.status != Status::Running)
        })
    }

    /// Long enough for a stand-in login shell to finish on a machine under heavy load, for tests
    /// of what is read rather than when.
    const SLOW_WAIT: Duration = Duration::from_secs(30);

    fn plan_of(argv: &[&str], folder: &Path) -> Plan {
        Plan {
            command: argv.join(" "),
            argv: argv.iter().map(|a| a.to_string()).collect(),
            folder: folder.to_path_buf(),
        }
    }

    /// Waits until `project`'s run has ended, then its latest snap.
    fn ended(r: &Runs, project: &str) -> Snap {
        let live = r.get(project).unwrap();
        live.wait_end(Instant::now() + Duration::from_secs(20));
        let snap = live.lock().snap(0);
        assert_ne!(snap.status, Status::Running, "it ended");
        snap
    }

    /// Waits until the run's ring has a line starting with `prefix`; that line's last word.
    fn word_after(r: &Runs, project: &str, prefix: &str) -> String {
        let until = Instant::now() + Duration::from_secs(20);
        loop {
            let snap = r.get(project).unwrap().lock().snap(0);
            if let Some(l) = snap.lines.iter().find(|l| l.starts_with(prefix)) {
                return l.rsplit(' ').next().unwrap().to_string();
            }
            assert!(
                Instant::now() < until,
                "no {prefix:?} line: {:?}",
                snap.lines
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Is any process left in the group `pgid`, or is `pid` alive (a zombie counts until reaped)?
    fn alive(target: &str) -> bool {
        Command::new("/bin/kill")
            .args(["-0", "--", target])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    }

    /// Waits up to 5 s for `target` to be gone; whether it went.
    fn gone(target: &str) -> bool {
        let until = Instant::now() + Duration::from_secs(5);
        while alive(target) {
            if Instant::now() > until {
                return false;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        true
    }

    /// A run command that starts a grandchild, says both process ids and waits.
    const FAMILY: &str = "sleep 300 &\necho \"child $!\"\necho \"leader $$\"\nwait\n";

    #[test]
    fn the_ring_keeps_the_last_lines_within_both_bounds() {
        let mut r = Ring::default();
        for n in 0..KEEP_LINES + 10 {
            r.push(format!("line {n}"));
        }
        assert_eq!((r.len(), r.total(), r.first()), (KEEP_LINES, 2010, 10));
        assert_eq!(r.since(0), r.since(10), "what fell out is skipped");
        let (from, lines) = r.since(2008);
        assert_eq!(
            (from, lines),
            (2008, vec!["line 2008".into(), "line 2009".into()])
        );
        assert_eq!(r.since(5000), (2010, vec![]), "nothing past the end");
        // Long lines: the byte bound comes first.
        let mut r = Ring::default();
        let long = "x".repeat(LONGEST_LINE);
        for _ in 0..100 {
            r.push(long.clone());
        }
        assert!(r.bytes() <= KEEP_BYTES, "{}", r.bytes());
        assert_eq!(r.len(), KEEP_BYTES / (LONGEST_LINE + 1));
        assert_eq!(r.first() + r.len() as u64, 100);
    }

    #[test]
    fn lines_are_read_leniently_and_cut_when_too_long() {
        let mut got = Vec::new();
        let long = "y".repeat(LONGEST_LINE + 50);
        let text = format!("one\r\ntwo\n\nسلام دنیا\n\u{ff}bad\n{long}\nlast");
        let mut bytes = text.into_bytes();
        // Not UTF-8: read leniently, never dropped.
        let bad = bytes.iter().position(|&b| b == 0xc3).unwrap();
        bytes[bad] = 0xff;
        read_lines(&bytes[..], |l| got.push(l));
        assert_eq!(&got[..4], ["one", "two", "", "سلام دنیا"]);
        assert_eq!(got[4], "\u{fffd}\u{fffd}bad");
        assert_eq!(got[5], format!("{}…", "y".repeat(LONGEST_LINE)));
        assert_eq!(got[6], "last", "the last line without a newline counts");
        assert_eq!(got.len(), 7);
        // One endless line, in small reads: kept to the bound, and the next line reads.
        let endless = format!("{}\nnext\n", "z".repeat(1_000_000));
        let mut got = Vec::new();
        read_lines(std::io::Cursor::new(endless), |l| got.push(l));
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].len(), LONGEST_LINE + '…'.len_utf8());
        assert_eq!(got[1], "next");
    }

    #[test]
    fn a_muster_queue_runs_in_its_repo_and_an_old_one_in_the_main_checkout_beside_it() {
        let d = tempfile::tempdir().unwrap();
        let q = muster_repo(d.path());
        assert_eq!(checkout(&q).unwrap(), d.path().join("repo"));
        // Old layout: <project>/tasks runs in <project>/main once it's a git checkout.
        let old = d.path().join("proj/tasks");
        fs::create_dir_all(&old).unwrap();
        let e = checkout(&old).unwrap_err();
        assert!(e.contains("is an old-layout queue"), "{e}");
        assert!(
            e.contains(&format!(
                "{} isn't a git checkout",
                d.path().join("proj/main").display()
            )),
            "{e}"
        );
        fs::create_dir_all(d.path().join("proj/main")).unwrap();
        assert!(checkout(&old).is_err(), "a folder that isn't a checkout");
        fs::create_dir_all(d.path().join("proj/main/.git")).unwrap();
        assert_eq!(checkout(&old).unwrap(), d.path().join("proj/main"));
        // A project's master checkout counts, as everywhere else.
        let older = d.path().join("other/tasks");
        fs::create_dir_all(d.path().join("other/master/.git")).unwrap();
        assert_eq!(checkout(&older).unwrap(), d.path().join("other/master"));
    }

    #[test]
    fn a_plan_needs_a_run_command_that_splits_without_a_shell() {
        let d = tempfile::tempdir().unwrap();
        let q = muster_repo(d.path());
        let p = |run: &str| plan(&project("proj", &q, run));
        let ok = p("  bin/run --fresh 'two words' ").unwrap();
        assert_eq!(ok.argv, ["bin/run", "--fresh", "two words"]);
        assert_eq!(
            (ok.command.as_str(), ok.folder),
            ("bin/run --fresh 'two words'", d.path().join("repo"))
        );
        for (run, why) in [
            (
                "",
                "proj has no run command: muster projects set proj run \"<command>\"",
            ),
            ("   ", "proj has no run command"),
            (
                "bin/run 'open",
                "has an unclosed quote or a trailing backslash",
            ),
            (
                "bin/run \\",
                "has an unclosed quote or a trailing backslash",
            ),
            ("bin/run\nrm -rf x", "one line"),
            ("bin/run \u{202e}nur", "direction mark"),
        ] {
            let e = p(run).unwrap_err();
            assert!(e.contains(why), "{run:?}: {e}");
        }
        // No checkout: refused, with the command still shown by the view.
        let old = d.path().join("proj2/tasks");
        fs::create_dir_all(&old).unwrap();
        assert!(plan(&project("proj2", &old, "bin/run"))
            .unwrap_err()
            .contains("old-layout"));
    }

    #[test]
    fn a_run_starts_as_words_in_the_checkout_without_the_queue_variables_and_exits() {
        let d = tempfile::tempdir().unwrap();
        let q = muster_repo(d.path());
        let repo = d.path().join("repo");
        script(
            &repo,
            "bin/try",
            "pwd -P\necho \"[$1]\" \"[$#]\"\nread x || echo \"no stdin\"\necho oops >&2\nexit 3\n",
        );
        let list = [project("proj", &q, "bin/try 'a b'")];
        // Without the queue variables, whatever Muster was started with.
        let cmd = command(&plan(&list[0]).unwrap(), None);
        let removed: Vec<_> = cmd
            .get_envs()
            .filter(|(_, v)| v.is_none())
            .map(|(k, _)| k.to_owned())
            .collect();
        assert_eq!(removed, ["MUSTER_ROOT", "TASKS_ROOT"]);
        assert_eq!(cmd.get_program(), repo.join("bin/try"));
        let path = cmd
            .get_envs()
            .find(|(k, _)| *k == "PATH")
            .and_then(|(_, v)| v)
            .unwrap();
        let dirs: Vec<PathBuf> = std::env::split_paths(path).collect();
        assert!(
            dirs.contains(&crate::home().join(".cargo/bin"))
                && dirs.contains(&PathBuf::from("/opt/homebrew/bin"))
        );
        assert_eq!(cmd.get_current_dir(), Some(repo.as_path()));
        let (r, sent) = runs(GRACE);
        let v = r.start_named(&list, "proj").unwrap();
        assert_eq!(
            (v.command.as_str(), v.refused.as_str()),
            ("bin/try 'a b'", "")
        );
        let snap = ended(&r, q.to_str().unwrap());
        assert_eq!(snap.status, Status::Exited { code: Some(3) });
        let mut lines = snap.lines.clone();
        lines.sort();
        let real = fs::canonicalize(&repo)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let mut want = vec![
            real,
            "[a b] [1]".to_string(),
            "no stdin".into(),
            "oops".into(),
        ];
        want.sort();
        assert_eq!(
            lines, want,
            "both streams, in the checkout, one argument with its space"
        );
        assert_eq!(snap.command, "bin/try 'a b'");
        assert!(snap.ended.is_some_and(|e| e >= snap.since));
        // The last event says it ended, with nothing left unsent.
        let sent = sent_to_the_end(&sent);
        let last = &sent.last().unwrap().1;
        assert_eq!(last.status, Status::Exited { code: Some(3) });
        let n: usize = sent.iter().map(|(_, s)| s.lines.len()).sum();
        assert_eq!(n, 4, "each line sent once");
        assert!(r.running().is_empty());
    }

    #[test]
    fn stop_ends_the_run_and_everything_it_started() {
        let d = tempfile::tempdir().unwrap();
        let s = script(d.path(), "family", FAMILY);
        let (r, sent) = runs(GRACE);
        let snap = r
            .start("proj", plan_of(&[s.to_str().unwrap()], d.path()))
            .unwrap();
        assert_eq!(snap.status, Status::Running);
        let child = word_after(&r, "proj", "child");
        let leader = word_after(&r, "proj", "leader");
        assert!(alive(&child) && alive(&format!("-{leader}")));
        assert_eq!(r.running(), HashSet::from(["proj".to_string()]));
        r.stop("proj");
        assert!(
            gone(&format!("-{leader}")),
            "no process of the group is left"
        );
        assert!(gone(&child), "the grandchild is gone");
        assert_eq!(r.get("proj").unwrap().lock().status, Status::Stopped);
        assert!(r.running().is_empty());
        assert_eq!(
            sent_to_the_end(&sent).last().unwrap().1.status,
            Status::Stopped
        );
        // Stopping what isn't running does nothing.
        r.stop("proj");
        r.stop("nothing");
    }

    #[test]
    fn a_run_that_ignores_sigterm_is_killed_after_the_grace() {
        let d = tempfile::tempdir().unwrap();
        let s = script(d.path(), "stubborn", &format!("trap '' TERM\n{FAMILY}"));
        let (r, _) = runs(Duration::from_millis(300));
        r.start("proj", plan_of(&[s.to_str().unwrap()], d.path()))
            .unwrap();
        let child = word_after(&r, "proj", "child");
        let leader = word_after(&r, "proj", "leader");
        let t = Instant::now();
        r.stop("proj");
        assert!(
            t.elapsed() >= Duration::from_millis(300),
            "SIGTERM had its grace"
        );
        assert!(gone(&format!("-{leader}")) && gone(&child));
        assert_eq!(r.get("proj").unwrap().lock().status, Status::Stopped);
    }

    #[test]
    fn run_again_restarts_it_and_quitting_stops_every_run() {
        let d = tempfile::tempdir().unwrap();
        let s = script(d.path(), "family", FAMILY);
        let (r, sent) = runs(GRACE);
        let first = r
            .start("a", plan_of(&[s.to_str().unwrap()], d.path()))
            .unwrap();
        let old_child = word_after(&r, "a", "child");
        let old_leader = word_after(&r, "a", "leader");
        let second = r
            .start("a", plan_of(&[s.to_str().unwrap()], d.path()))
            .unwrap();
        assert!(second.id > first.id);
        assert!(
            gone(&format!("-{old_leader}")) && gone(&old_child),
            "the first run is gone"
        );
        let child = word_after(&r, "a", "child");
        let leader = word_after(&r, "a", "leader");
        assert_ne!(leader, old_leader);
        // Another project's run goes on beside it.
        r.start("b", plan_of(&[s.to_str().unwrap()], d.path()))
            .unwrap();
        let b_leader = word_after(&r, "b", "leader");
        assert_eq!(r.running().len(), 2);
        // The first run's events say it stopped, under its own id.
        let stopped = |s: &[(Instant, Snap)]| {
            s.iter()
                .any(|(_, s)| s.id == first.id && s.status == Status::Stopped)
        };
        assert!(stopped(&sent_once(&sent, stopped)));
        // Muster quits.
        r.stop_all();
        for g in [&leader, &b_leader] {
            assert!(gone(&format!("-{g}")));
        }
        assert!(gone(&child));
        assert!(r.running().is_empty());
    }

    #[test]
    fn a_hundred_thousand_lines_send_at_most_one_event_per_batch_and_keep_only_the_ring() {
        let d = tempfile::tempdir().unwrap();
        let (r, sent) = runs(GRACE);
        r.start("proj", plan_of(&["/usr/bin/seq", "1", "100000"], d.path()))
            .unwrap();
        let snap = ended(&r, "proj");
        assert_eq!(snap.status, Status::Exited { code: Some(0) });
        let live = r.get("proj").unwrap();
        {
            let st = live.lock();
            assert_eq!(st.ring.total(), 100_000);
            assert!(st.ring.len() <= KEEP_LINES && st.ring.bytes() <= KEEP_BYTES);
        }
        assert_eq!(snap.lines.last().map(String::as_str), Some("100000"));
        assert_eq!(snap.first, 100_000 - KEEP_LINES as u64);
        let sent = sent_to_the_end(&sent);
        for w in sent.windows(2) {
            assert!(w[1].0 - w[0].0 >= BATCH, "{:?} apart", w[1].0 - w[0].0);
        }
        for (_, s) in sent.iter() {
            assert!(
                s.lines.len() <= KEEP_LINES,
                "an event carries at most the ring"
            );
        }
        assert_eq!(
            sent.last().unwrap().1.lines.last().map(String::as_str),
            Some("100000")
        );
    }

    #[test]
    fn the_window_names_a_project_only_by_name_and_is_refused_the_rest() {
        let d = tempfile::tempdir().unwrap();
        let q = muster_repo(d.path());
        let list = [
            project("proj", &q, ""),
            project("other", &d.path().join("x/tasks"), "bin/run"),
        ];
        let (r, _) = runs(GRACE);
        for name in [
            "nope",
            "",
            "/bin/sh",
            q.to_str().unwrap(),
            "bin/run",
            "proj; rm -rf ~",
        ] {
            for e in [
                r.start_named(&list, name).unwrap_err(),
                r.stop_named(&list, name).unwrap_err(),
                r.view(&list, name).unwrap_err(),
            ] {
                assert_eq!(e, format!("no project named {name:?}"));
            }
        }
        // A project without a run command can't start one, and its view says why.
        let e = r.start_named(&list, "proj").unwrap_err();
        assert!(e.contains("proj has no run command"), "{e}");
        let v = r.view(&list, "proj").unwrap();
        assert_eq!((v.command.as_str(), v.run.is_none()), ("", true));
        assert_eq!(v.folder, d.path().join("repo").to_string_lossy());
        assert!(v.refused.contains("no run command"));
        // One with no checkout: refused, with nothing started.
        let e = r.start_named(&list, "other").unwrap_err();
        assert!(e.contains("old-layout"), "{e}");
        let v = r.view(&list, "other").unwrap();
        assert_eq!((v.command.as_str(), v.folder.as_str()), ("bin/run", ""));
        assert!(r.running().is_empty());
        // A command that can't start: said, and nothing kept running.
        let list = [project("proj", &q, "bin/missing")];
        let e = r.start_named(&list, "proj").unwrap_err();
        assert!(e.contains("couldn't start \"bin/missing\""), "{e}");
        assert!(r.running().is_empty());
    }

    #[test]
    fn a_run_gets_the_owners_tool_folders_after_its_own_path_each_once() {
        let home = Path::new("/Users/someone");
        let search_path = |inherited: &OsStr| super::search_path(home, None, inherited);
        let got = search_path(OsStr::new("/usr/bin:/bin:/usr/sbin:/sbin"));
        assert_eq!(
            got,
            "/usr/bin:/bin:/usr/sbin:/sbin:/Users/someone/.cargo/bin:/Users/someone/.local/bin:/opt/homebrew/bin:/usr/local/bin"
        );
        // A terminal's own comes first and stays as it was; nothing twice, no empty entry.
        let got = search_path(OsStr::new("/my/bin::/opt/homebrew/bin"));
        assert!(got
            .to_string_lossy()
            .starts_with("/my/bin:/opt/homebrew/bin:/Users/someone/.cargo/bin"));
        assert_eq!(
            got.to_string_lossy().matches("/opt/homebrew/bin").count(),
            1
        );
        assert_eq!(
            search_path(OsStr::new(""))
                .to_string_lossy()
                .split(':')
                .next(),
            Some("/Users/someone/.cargo/bin")
        );
    }

    #[test]
    fn the_login_shells_path_comes_first_then_musters_own_then_the_fixed_folders() {
        let home = Path::new("/Users/someone");
        let login =
            OsStr::new("/Users/someone/.nvm/versions/node/v24/bin:/opt/homebrew/bin:bin::.");
        let got = search_path(home, Some(login), OsStr::new("/usr/bin:/bin:/my/bin"));
        assert_eq!(
            got,
            "/Users/someone/.nvm/versions/node/v24/bin:/opt/homebrew/bin:/usr/bin:/bin:/my/bin:/Users/someone/.cargo/bin:/Users/someone/.local/bin:/usr/local/bin:/usr/sbin:/sbin",
            "each once, relative and empty folders dropped"
        );
    }

    /// A stand-in login shell in `dir`: it keeps its arguments in `args`, prints what a startup
    /// file might (a mark included), then runs the script it was given with `path` as PATH.
    fn stand_in_shell(dir: &Path, path: &str) -> PathBuf {
        let args = dir.join("args");
        script(
            dir,
            "shell",
            &format!(
                "printf '%s\\n' \"$@\" > '{}'\necho 'Welcome <muster-path>/old</muster-path> and <muster-path>'\nPATH='{path}'\neval \"$2\"\n",
                args.display()
            ),
        )
    }

    #[test]
    fn the_login_shells_path_is_what_it_prints_between_the_last_marks() {
        let d = tempfile::tempdir().unwrap();
        let shell = stand_in_shell(d.path(), "/nvm/bin:/usr/bin:/bin");
        assert_eq!(
            login_path(&shell, SLOW_WAIT),
            Some(OsString::from("/nvm/bin:/usr/bin:/bin"))
        );
        let args = fs::read_to_string(d.path().join("args")).unwrap();
        let args: Vec<&str> = args.lines().collect();
        assert_eq!(
            args.len(),
            2,
            "an interactive login shell, one script argument"
        );
        assert_eq!(args[0], "-ilc");
        assert!(
            args[1].starts_with("printf '\\n<muster-path ") && args[1].ends_with("\\n' \"$PATH\""),
            "{}",
            args[1]
        );
        let m = Marks {
            start: "<m>".into(),
            end: "</m>".into(),
        };
        assert_eq!(m.path_in("<m>/a</m> <m>/b</m>"), Some(OsString::from("/b")));
        assert_eq!(m.path_in("a<m></m>"), None, "empty");
        assert_eq!(m.path_in("<m>/a"), None, "no end mark");
        assert_eq!(m.path_in("<m>/a\n/b</m>"), None);
        assert_eq!(m.path_in("<muster-path>/a</muster-path>"), None);
        // Each call's marks are its own.
        assert_ne!(Marks::new().start, Marks::new().start);
    }

    #[test]
    fn the_path_is_taken_at_its_end_mark_while_a_startup_file_still_holds_the_output() {
        let d = tempfile::tempdir().unwrap();
        let pid = d.path().join("pid");
        let shell = script(
            d.path(),
            "shell",
            &format!(
                "sleep 30 &\necho $! > '{}'\nPATH=/nvm/bin:/usr/bin\neval \"$2\"\n",
                pid.display()
            ),
        );
        // Read to the end of output, it would wait out the whole wait (the sleeper outlives it) and
        // give None; the bound is half the wait, so a busy machine's slow start doesn't count.
        let wait = Duration::from_secs(20);
        let t = Instant::now();
        let got = login_path(&shell, wait);
        let took = t.elapsed();
        if let Ok(sleeper) = fs::read_to_string(&pid) {
            let _ = Command::new("/bin/kill")
                .args(["-KILL", sleeper.trim()])
                .status();
        }
        assert_eq!(
            got,
            Some(OsString::from("/nvm/bin:/usr/bin")),
            "after {took:?}"
        );
        assert!(took < wait / 2, "{took:?}");
    }

    #[test]
    fn the_login_shell_runs_without_a_controlling_terminal() {
        let d = tempfile::tempdir().unwrap();
        let shell = script(
            d.path(),
            "shell",
            "if (: </dev/tty) 2>/dev/null; then PATH=/has/a/tty; else PATH=/no/tty; fi\neval \"$2\"\n",
        );
        assert_eq!(
            login_path(&shell, SLOW_WAIT),
            Some(OsString::from("/no/tty"))
        );
    }

    /// Under a terminal of its own (`script`), a real interactive zsh with no startup files of the
    /// owner's (HOME and ZDOTDIR in a temporary folder) prints its PATH at once: in a background
    /// group of Muster's terminal it would stop on SIGTTOU until the timeout.
    #[test]
    fn a_real_zsh_under_a_terminal_prints_its_path_without_stopping() {
        if !Path::new("/bin/zsh").exists() || !Path::new("/usr/bin/script").exists() {
            return;
        }
        let d = tempfile::tempdir().unwrap();
        let mut cmd = Command::new("/usr/bin/script");
        cmd.arg("-q")
            .arg("/dev/null")
            .arg(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "run::tests::zsh_inside_a_terminal",
                "--ignored",
                "--test-threads=1",
            ])
            .stdin(Stdio::null());
        for var in ["HOME", "ZDOTDIR"] {
            cmd.env(var, d.path());
        }
        let out = cmd.output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        if String::from_utf8_lossy(&out.stderr).contains("openpty") {
            // A sandbox that allows no terminals: nothing to try here.
            eprintln!("skipped: script can't open a terminal here");
            return;
        }
        assert!(out.status.success(), "{text}");
        assert!(text.contains("1 passed"), "{text}");
    }

    #[test]
    #[ignore = "run by a_real_zsh_under_a_terminal_prints_its_path_without_stopping, under script"]
    fn zsh_inside_a_terminal() {
        assert!(
            Command::new("/bin/sh")
                .args(["-c", ": </dev/tty"])
                .status()
                .unwrap()
                .success(),
            "a terminal of its own"
        );
        // Stopped, it would give None at the wait; the bound is half of it, for a busy machine.
        let wait = Duration::from_secs(20);
        let t = Instant::now();
        let got = login_path(Path::new("/bin/zsh"), wait);
        assert!(got.is_some(), "after {:?}", t.elapsed());
        assert!(t.elapsed() < wait / 2, "{:?}", t.elapsed());
    }

    #[test]
    fn a_login_shell_that_fails_prints_no_path_or_hangs_gives_none() {
        let d = tempfile::tempdir().unwrap();
        let failing = script(
            d.path(),
            "fails",
            "echo '<muster-path>/a</muster-path>'\nexit 1\n",
        );
        assert_eq!(login_path(&failing, SLOW_WAIT), None, "a failing shell");
        let quiet = script(d.path(), "quiet", "echo hello\n");
        assert_eq!(login_path(&quiet, SLOW_WAIT), None, "no PATH printed");
        assert_eq!(
            login_path(&d.path().join("missing"), SLOW_WAIT),
            None,
            "no shell there"
        );
        // A hung startup file: given up at the wait, and what it started is gone too. The child's
        // id is written whole (a rename) once it runs; a machine so busy that the wait ran out
        // before that is tried again with twice the wait.
        let pid = d.path().join("pid");
        let hangs = script(
            d.path(),
            "hangs",
            &format!(
                "sleep 300 &\necho $! > '{0}.new'\nmv '{0}.new' '{0}'\nwait\n",
                pid.display()
            ),
        );
        let mut wait = Duration::from_secs(2);
        let child = loop {
            let t = Instant::now();
            assert_eq!(login_path(&hangs, wait), None);
            let took = t.elapsed();
            assert!(took < wait + Duration::from_secs(10), "{took:?}");
            if let Ok(child) = fs::read_to_string(&pid) {
                break child;
            }
            assert!(wait < Duration::from_secs(16), "it never started its child");
            wait *= 2;
        };
        assert!(gone(child.trim()), "its child is killed");
    }

    #[test]
    fn a_run_gets_the_login_shells_path_read_once() {
        let d = tempfile::tempdir().unwrap();
        let q = muster_repo(d.path());
        let repo = d.path().join("repo");
        script(&repo, "bin/path", "echo \"$PATH\"\n");
        let tools = d.path().join("tools");
        let shell = stand_in_shell(d.path(), &format!("{}:/usr/bin:/bin", tools.display()));
        let list = [project("proj", &q, "bin/path")];
        // The read gets `LOGIN_WAIT`; a machine so busy that the stand-in hadn't finished in it
        // gives the run no login PATH, and fresh runs try again.
        let mut tries = 0;
        let (r, snap) = loop {
            let r = Runs::with_shell(Arc::new(|_: &Snap| {}), GRACE, Some(shell.clone()));
            r.start_named(&list, "proj").unwrap();
            let snap = ended(&r, q.to_str().unwrap());
            tries += 1;
            if tries == 3 || r.login.get() != Some(&None) {
                break (r, snap);
            }
        };
        let path = snap.lines.first().unwrap();
        assert!(
            path.starts_with(&format!("{}:/usr/bin:/bin:", tools.display())),
            "{path}"
        );
        // Read once: a second run doesn't start the shell again.
        fs::remove_file(d.path().join("args")).unwrap();
        r.start_named(&list, "proj").unwrap();
        ended(&r, q.to_str().unwrap());
        assert!(!d.path().join("args").exists(), "the shell ran once");
    }

    #[test]
    fn a_run_is_kept_by_its_queue_folder_so_a_rename_or_another_project_cant_take_it() {
        let d = tempfile::tempdir().unwrap();
        let q = muster_repo(d.path());
        let s = script(&d.path().join("repo"), "bin/fam", FAMILY);
        assert!(s.exists());
        let (r, sent) = runs(GRACE);
        let list = [project("x-2", &q, "bin/fam")];
        r.start_named(&list, "x-2").unwrap();
        let key = q.to_string_lossy().into_owned();
        let leader = word_after(&r, q.to_str().unwrap(), "leader");
        // Another project now has its old name, and it's called x: it still sees and stops its run.
        let other = d.path().join("other/tasks");
        let list = [project("x", &q, "bin/fam"), project("x-2", &other, "")];
        assert!(
            r.view(&list, "x-2").unwrap().run.is_none(),
            "the other project has no run"
        );
        assert_eq!(r.view(&list, "x-2").unwrap().key, other.to_string_lossy());
        let v = r.view(&list, "x").unwrap();
        assert_eq!(v.key, key);
        assert_eq!(
            v.run.map(|s| (s.project, s.key, s.status)),
            Some(("x".to_string(), key.clone(), Status::Running))
        );
        assert_eq!(
            r.running(),
            HashSet::from([q.to_string_lossy().into_owned()])
        );
        r.stop_named(&list, "x").unwrap();
        assert!(gone(&format!("-{leader}")));
        // Every event says whose run it is by the key, which the window matches on: the name
        // went to another project.
        let sent = sent_to_the_end(&sent);
        assert!(!sent.is_empty());
        assert!(sent.iter().all(|(_, s)| s.key == key), "every event keyed");
    }

    #[test]
    fn a_snap_is_sent_to_the_window_in_its_own_words() {
        let snap = Snap {
            project: "muster".into(),
            key: "/p/muster/main/.muster".into(),
            id: 2,
            command: "bin/run".into(),
            folder: "/p/muster/main".into(),
            status: Status::Exited { code: Some(0) },
            since: 10,
            ended: Some(20),
            first: 5,
            lines: vec!["done".into()],
        };
        let v = serde_json::to_value(&snap).unwrap();
        assert_eq!(
            v["status"],
            serde_json::json!({"kind": "exited", "code": 0})
        );
        assert_eq!(v["first"], 5);
        assert_eq!(v["key"], "/p/muster/main/.muster");
        assert_eq!(
            serde_json::to_value(Status::Stopped).unwrap(),
            serde_json::json!({"kind": "stopped"})
        );
        assert_eq!(
            serde_json::to_value(Status::Running).unwrap(),
            serde_json::json!({"kind": "running"})
        );
    }
}
