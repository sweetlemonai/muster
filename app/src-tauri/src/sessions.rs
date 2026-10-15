//! The Sessions view (task 0098): every session of the Claude accounts Muster reads, grouped as
//! Claude Code's agents view groups them (Needs input, Working, Completed), all projects in one or
//! one project's, and a recap of one session: the tasks it worked on, its history from their logs,
//! its handoff chain, and its time, tokens and progress.
//!
//! Accounts: only `accounts::allowed` (0099, never `~/.claude`, the company account). The list
//! comes from `agents::read_lists`, one `claude agents --json --all` per allowed account, always
//! with `CLAUDE_CONFIG_DIR` set; transcripts come through `transcripts::Reader`, which reads only
//! the allowed accounts' `~/playground` project folders.
//!
//! Privacy (decision 0017): a session's name, folder and last message are read for the owner's
//! window only. The folder (`cwd`) never leaves this module: it becomes the name of the project it
//! is in, or nothing. Nothing here is written to a file, a queue, a log or a disk cache: the list
//! and the transcript text live in memory, in bounded caches that drop a transcript's text once
//! the transcript is gone.
//!
//! Handoffs: a successor session starts with `/handoff resume <path>`; that handoff's front matter
//! names the predecessor (`session:`) and the successor (`successor:`, a name). A handoff path from
//! a transcript is read only when it's a regular file (not a link), its real place is directly in
//! a `.claude/work/handoffs/` folder under `~/playground`, and it's at most `MAX_HANDOFF_BYTES`; only
//! its front matter is read.

use crate::accounts::{self, Account};
use crate::agents::{self, AccountList, Fetch, ListEntry, ListState, Spawn};
use crate::projects::Project;
use crate::queue::{self, Task};
use crate::transcripts::{self, Reader, Tail, Tokens, Usage};
use crate::work;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

/// The biggest handoff file read, in bytes.
pub const MAX_HANDOFF_BYTES: u64 = 256 * 1024;
/// The most links a handoff chain follows each way.
const MAX_CHAIN: usize = 32;
/// Characters of a history line.
const HISTORY_CHARS: usize = 200;
/// Characters of a successor's name read from a handoff.
const MAX_SUCCESSOR: usize = 64;

/// A project's folders a session's working folder is matched against: the project's own folder,
/// which holds its checkouts and `worktrees/`, and for a checkout that's a project of its own,
/// the `<checkout>.worktrees` beside it (`layout::worktrees_dir`).
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectFolders {
    pub name: String,
    pub folders: Vec<PathBuf>,
}

/// The folders of each shown project in `list` (hidden ones and folders that aren't queues any
/// more are left out), canonical where they exist.
pub fn project_folders(list: &[Project]) -> Vec<ProjectFolders> {
    let canon = |p: PathBuf| fs::canonicalize(&p).unwrap_or(p);
    list.iter()
        .filter(|p| !p.hidden && p.ok)
        .filter_map(|p| {
            let queue = Path::new(&p.path);
            let up = queue.parent()?;
            let mut folders = Vec::new();
            if crate::layout::is_muster(queue) && !crate::layout::checkout_is_main(queue) {
                folders.push(up.to_path_buf());
                let name = up.file_name()?.to_string_lossy();
                folders.push(up.parent()?.join(format!("{name}.worktrees")));
            } else if crate::layout::is_muster(queue) {
                folders.push(up.parent()?.to_path_buf());
            } else {
                folders.push(up.to_path_buf());
            }
            Some(ProjectFolders {
                name: p.name.clone(),
                folders: folders.into_iter().map(canon).collect(),
            })
        })
        .collect()
}

/// The project `cwd` is in: the one with the deepest folder holding it (whole path components,
/// so `~/playground/muster2` isn't in `~/playground/muster`). None for a folder in no project, or
/// none at all.
pub fn project_of<'a>(cwd: &Path, projects: &'a [ProjectFolders]) -> Option<&'a str> {
    if cwd.as_os_str().is_empty() || !cwd.is_absolute() {
        return None;
    }
    let cwd = fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    let mut best: Option<(&str, usize)> = None;
    for p in projects {
        for f in &p.folders {
            let depth = f.components().count();
            if cwd.starts_with(f) && best.is_none_or(|(_, d)| depth > d) {
                best = Some((&p.name, depth));
            }
        }
    }
    best.map(|(n, _)| n)
}

/// One row of the list, as the window gets it. No folder: only the project's name.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    /// The account label, `mg`.
    pub account: String,
    pub id: String,
    /// Its name, or "" (the window shows the id then).
    pub name: String,
    pub group: ListState,
    /// The word its group comes from: `stopped`, `failed`, `idle`…
    pub word: String,
    /// Its last message, or what it waits for, on one line.
    pub message: String,
    /// The project it runs in, "" for none (Other).
    pub project: String,
    /// When it started and when it was last active (its transcript changed), seconds since 1970.
    pub started: Option<u64>,
    pub at: Option<u64>,
}

/// A row's message: what it waits for when it waits on the owner (the pending tool call, else
/// its last text, else `waitingFor`), and its last text otherwise; its state's word when there's
/// nothing to say.
pub fn message(e: &ListEntry, tail: Option<&Tail>) -> String {
    let text = tail.map(|t| t.text.as_str()).unwrap_or("");
    let pending = tail.map(|t| t.pending.as_str()).unwrap_or("");
    let said = if e.state == ListState::NeedsInput && !pending.is_empty() {
        format!("approve {pending}")
    } else if !text.is_empty() {
        text.to_string()
    } else if e.state == ListState::NeedsInput && !e.waiting_for.is_empty() {
        format!("waiting for {}", e.waiting_for)
    } else {
        e.word.clone()
    };
    agents::plain_line(&said, transcripts::MESSAGE_CHARS)
}

/// The rows of `lists`, each session once (by account and id), with their last messages read
/// through `reader` in `allowed` and their projects matched in `projects`.
pub fn rows(
    lists: &[AccountList],
    reader: &Reader,
    allowed: &[Account],
    projects: &[ProjectFolders],
) -> Vec<Row> {
    let mut seen: HashSet<(&str, &str)> = HashSet::new();
    let entries: Vec<(&str, &ListEntry)> = lists
        .iter()
        .filter_map(|(label, r)| r.as_ref().ok().map(|es| (label.as_str(), es)))
        .flat_map(|(label, es)| es.iter().map(move |e| (label, e)))
        .filter(|(label, e)| seen.insert((label, e.id.as_str())))
        .collect();
    let ids: Vec<String> = entries.iter().map(|(_, e)| e.id.clone()).collect();
    let tails = reader.tails(allowed, &ids);
    entries
        .iter()
        .zip(tails)
        .map(|((label, e), tail)| Row {
            account: label.to_string(),
            id: e.id.clone(),
            name: e.name.clone(),
            group: e.state,
            word: e.word.clone(),
            message: message(e, tail.as_ref()),
            project: project_of(&e.cwd, projects).unwrap_or("").to_string(),
            started: e.started,
            at: tail.map(|t| t.modified).or(e.started),
        })
        .collect()
}

/// The list as the window gets it.
#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct List {
    pub rows: Vec<Row>,
    /// No read has landed yet, or one is running for the first time: "Reading sessions…".
    pub reading: bool,
    /// An account that couldn't be read: `cc-mg: claude isn't installed`.
    pub problems: Vec<String>,
    /// The accounts read, `cc-mg`, for the header's words.
    pub accounts: Vec<String>,
}

#[derive(Default)]
struct Inner {
    lists: Vec<AccountList>,
    rows: Vec<Row>,
    /// When the last read started (seconds), if one has.
    last: Option<u64>,
    running: bool,
    /// The latest poll's time (seconds).
    polled: u64,
    /// When the latest read landed, as the latest poll before it had the time: past
    /// `agents::MAX_AGE_SECS` the list is stale, and the view reads again before showing it.
    landed: Option<u64>,
}

/// A poisoned lock only means a read panicked mid-update; the data is a cache, so use it.
fn lock(m: &Mutex<Inner>) -> MutexGuard<'_, Inner> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Marks the read done however it ends, a panic included.
struct Running<'a>(&'a Mutex<Inner>);

impl Drop for Running<'_> {
    fn drop(&mut self) {
        lock(self.0).running = false;
    }
}

/// The list's reader: polled by the view while it's open, it starts a read when one is due (at
/// most every `agents::EVERY_SECS`, never two at once) on a thread of its own and hands back the
/// latest. While the view is closed nobody polls, and nothing runs.
pub struct Lister {
    fetch: Fetch,
    spawn: Spawn,
    reader: Arc<Reader>,
    inner: Mutex<Inner>,
}

impl Lister {
    pub fn new(fetch: Fetch, spawn: Spawn, reader: Arc<Reader>) -> Arc<Lister> {
        Arc::new(Lister {
            fetch,
            spawn,
            reader,
            inner: Mutex::new(Inner::default()),
        })
    }

    /// The app's: the owner's `claude` under `home`, read on a thread of its own.
    pub fn app(home: PathBuf, reader: Arc<Reader>) -> Arc<Lister> {
        Lister::new(agents::claude_fetch(home), agents::on_thread, reader)
    }

    /// The latest list of the accounts in `allowed` now (one taken off the list since the read
    /// isn't shown, L16), starting a read of `allowed` with `projects` when one is due at `now`.
    pub fn poll(
        self: &Arc<Self>,
        allowed: &[Account],
        projects: Vec<ProjectFolders>,
        now: u64,
    ) -> List {
        {
            let mut g = lock(&self.inner);
            g.polled = now;
            let due = g
                .last
                .is_none_or(|last| now >= last.saturating_add(agents::EVERY_SECS) || now < last);
            if due && !g.running && !allowed.is_empty() {
                g.running = true;
                g.last = Some(now);
                let me = Arc::clone(self);
                let accounts = allowed.to_vec();
                drop(g);
                (self.spawn)(Box::new(move || {
                    let _running = Running(&me.inner);
                    let lists = agents::read_lists(&me.fetch, &accounts);
                    let rows = rows(&lists, &me.reader, &accounts, &projects);
                    let mut g = lock(&me.inner);
                    g.lists = lists;
                    g.rows = rows;
                    g.landed = Some(g.polled);
                }));
            }
        }
        let g = lock(&self.inner);
        // A read older than MAX_AGE (the view was closed meanwhile), or from a clock ahead of
        // now, isn't shown as now: "Reading sessions…" until the new one lands.
        let fresh = g
            .landed
            .and_then(|at| now.checked_sub(at))
            .is_some_and(|age| age <= agents::MAX_AGE_SECS);
        let on = |label: &str| fresh && accounts::is_allowed(label, allowed);
        List {
            rows: g.rows.iter().filter(|r| on(&r.account)).cloned().collect(),
            reading: !fresh && !allowed.is_empty(),
            problems: g
                .lists
                .iter()
                .filter(|(l, _)| on(l))
                .filter_map(|(l, r)| r.as_ref().err().map(|why| format!("cc-{l}: {why}")))
                .collect(),
            accounts: allowed
                .iter()
                .map(|a| format!("cc-{}", a.label()))
                .collect(),
        }
    }

    /// Every session the latest read listed, with its account, of the accounts in `allowed`
    /// only: the recap finds its session and the chain's successors among them.
    fn listed(&self, allowed: &[Account]) -> Vec<(String, ListEntry)> {
        lock(&self.inner)
            .lists
            .iter()
            .filter(|(l, _)| accounts::is_allowed(l, allowed))
            .filter_map(|(l, r)| r.as_ref().ok().map(|es| (l, es)))
            .flat_map(|(l, es)| es.iter().map(move |e| (l.clone(), e.clone())))
            .collect()
    }
}

/// `list` for one project (`Some(name)`: only its sessions), or for all (`None`: every session,
/// those in no project too).
pub fn for_project(mut list: List, project: Option<&str>) -> List {
    if let Some(p) = project {
        list.rows.retain(|r| r.project == p);
    }
    list
}

/// The handoff path a first prompt resumes: `/handoff resume <path>` as the slash command writes
/// it (`<command-name>/handoff</command-name>` with `<command-args>resume <path></command-args>`),
/// or typed in a prompt (`/handoff resume <path>`, `/handoff resume with <path>`). Only an
/// absolute path to a `.md` file.
pub fn resume_path(prompt: &str) -> Option<PathBuf> {
    let args = if prompt.contains("<command-name>/handoff</command-name>") {
        let start = prompt.find("<command-args>")? + "<command-args>".len();
        let end = prompt[start..].find("</command-args>")? + start;
        prompt[start..end]
            .trim()
            .strip_prefix("resume")?
            .to_string()
    } else {
        let at = prompt.find("/handoff resume ")? + "/handoff resume ".len();
        prompt[at..].to_string()
    };
    let mut words = args.split_whitespace();
    let mut path = words.next()?;
    if path == "with" {
        path = words.next()?;
    }
    // A sentence may end right after the path (`….md.`).
    let path = path.trim_end_matches([',', ';', ')', '.', ':']);
    if !path.ends_with(".md") {
        return None;
    }
    let p = PathBuf::from(path);
    (p.is_absolute() && !p.components().any(|c| c == Component::ParentDir)).then_some(p)
}

/// What a handoff's front matter says of the chain.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Handoff {
    /// The session that wrote it: its short id, "" when none is named.
    pub session: String,
    /// Who it hands to, by name, "" when none.
    pub successor: String,
}

/// The handoff at `path`, or, once resumed and renamed, at `<name>.done.md` beside it; None
/// unless it's safe to read (`read_one`).
pub fn read_handoff(path: &Path, home: &Path) -> Option<Handoff> {
    let name = path.file_name()?.to_str()?;
    let done = name
        .strip_suffix(".md")
        .filter(|stem| !stem.ends_with(".done"))
        .map(|stem| path.with_file_name(format!("{stem}.done.md")));
    read_one(path, home).or_else(|| done.and_then(|d| read_one(&d, home)))
}

/// A handoff file's front matter, only when the file is a regular file (not a link) of at most
/// `MAX_HANDOFF_BYTES` whose real place is directly in a `.claude/work/handoffs/` folder under
/// `<home>/playground`.
fn read_one(path: &Path, home: &Path) -> Option<Handoff> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.file_type().is_file() || meta.len() > MAX_HANDOFF_BYTES {
        return None;
    }
    let real = fs::canonicalize(path).ok()?;
    let root = fs::canonicalize(home.join("playground")).ok()?;
    let handoffs = real.parent()?;
    let work = handoffs.parent()?;
    let claude = work.parent()?;
    let named = |p: &Path, n: &str| p.file_name().is_some_and(|f| f == n);
    if !real.starts_with(&root)
        || !named(handoffs, "handoffs")
        || !named(work, "work")
        || !named(claude, ".claude")
    {
        return None;
    }
    let mut text = String::new();
    fs::File::open(&real)
        .ok()?
        .take(MAX_HANDOFF_BYTES)
        .read_to_string(&mut text)
        .ok()?;
    let mut lines = text.lines();
    if lines.next()?.trim_end() != "---" {
        return None;
    }
    let mut h = Handoff::default();
    for line in lines {
        if line.trim_end() == "---" {
            return Some(h);
        }
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        match k.trim() {
            "session" => {
                h.session = v
                    .split_whitespace()
                    .next()
                    .and_then(crate::session::short_id)
                    .unwrap_or_default();
            }
            "successor" => h.successor = agents::plain_line(v.trim(), MAX_SUCCESSOR),
            _ => {}
        }
    }
    // No closing line: not front matter.
    None
}

/// One link of a handoff chain.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub id: String,
    /// Its name from the list, else the name the handoff before it gave its successor, else "".
    pub name: String,
    /// It's the session the recap is of.
    pub current: bool,
}

/// The predecessor `first` (a session's first prompt) names: the `session:` of the handoff it
/// resumes, with that handoff.
fn predecessor(first: Option<&str>, home: &Path) -> Option<Handoff> {
    let h = read_handoff(&resume_path(first?)?, home)?;
    (!h.session.is_empty()).then_some(h)
}

/// The chain through session `id`: back through the handoffs each first prompt resumes, and
/// forward through the listed sessions whose first prompt resumes a handoff `session:` names the
/// one before. Where two sessions resumed the same handoff, the one named as its `successor:`
/// wins, else the one that started first. Each session once, at most `MAX_CHAIN` each way.
/// Going back, a session's first prompt is read only when `vouched` says an allowed account
/// vouches for it: one it doesn't is the chain's first link, by id, with nothing read.
pub fn chain(
    id: &str,
    listed: &[(String, ListEntry)],
    vouched: &dyn Fn(&str) -> bool,
    first_prompt: &dyn Fn(&[String]) -> Vec<Option<String>>,
    home: &Path,
) -> Vec<Link> {
    let name_of = |s: &str| {
        listed
            .iter()
            .find(|(_, e)| e.id == s)
            .map(|(_, e)| e.name.clone())
            .unwrap_or_default()
    };
    let mut seen: HashSet<String> = HashSet::from([id.to_string()]);
    let mut back: Vec<Link> = Vec::new();
    let mut current_hint = String::new();
    let mut cur = id.to_string();
    while back.len() < MAX_CHAIN && vouched(&cur) {
        let first = first_prompt(std::slice::from_ref(&cur)).pop().flatten();
        let Some(h) = predecessor(first.as_deref(), home) else {
            break;
        };
        if !seen.insert(h.session.clone()) {
            break;
        }
        // The handoff names the session after it as its successor: its name, when the list
        // has none.
        match back.last_mut() {
            Some(l) if l.name.is_empty() => l.name.clone_from(&h.successor),
            Some(_) => {}
            None => current_hint.clone_from(&h.successor),
        }
        back.push(Link {
            id: h.session.clone(),
            name: name_of(&h.session),
            current: false,
        });
        cur = h.session;
    }
    back.reverse();
    // Forward: who resumed a handoff of whom, among the listed sessions.
    let ids: Vec<String> = listed.iter().map(|(_, e)| e.id.clone()).collect();
    let firsts = first_prompt(&ids);
    let mut next: HashMap<String, Vec<(String, String, Option<u64>)>> = HashMap::new();
    for ((_, e), first) in listed.iter().zip(firsts) {
        if let Some(h) = predecessor(first.as_deref(), home) {
            next.entry(h.session)
                .or_default()
                .push((e.id.clone(), h.successor, e.started));
        }
    }
    let mut ahead: Vec<Link> = Vec::new();
    let mut cur = id.to_string();
    while ahead.len() < MAX_CHAIN {
        let Some(found) = next.get(&cur) else { break };
        let named = |(sid, successor, _): &&(String, String, Option<u64>)| {
            !successor.is_empty() && name_of(sid) == *successor
        };
        let pick = found
            .iter()
            .filter(|(sid, _, _)| !seen.contains(sid))
            .find(named)
            .or_else(|| {
                found
                    .iter()
                    .filter(|(sid, _, _)| !seen.contains(sid))
                    .min_by_key(|(_, _, started)| started.unwrap_or(u64::MAX))
            });
        let Some((sid, successor, _)) = pick else {
            break;
        };
        seen.insert(sid.clone());
        let listed_name = name_of(sid);
        ahead.push(Link {
            id: sid.clone(),
            name: if listed_name.is_empty() {
                successor.clone()
            } else {
                listed_name
            },
            current: false,
        });
        cur = sid.clone();
    }
    let mut out = back;
    let own = name_of(id);
    out.push(Link {
        id: id.to_string(),
        name: if own.is_empty() { current_hint } else { own },
        current: true,
    });
    out.extend(ahead);
    out
}

/// A task a session worked on, as the recap lists it.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskRef {
    pub queue: String,
    pub id: String,
    /// `mu-12` (`key::key`).
    pub key: String,
    pub title: String,
    pub stage: String,
    /// The session holds it now (`Task::holder`).
    pub holds: bool,
}

/// One line of a recap's history.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    /// When, seconds since 1970: a log line's stamp read in this Mac's time zone, a session's
    /// first transcript time.
    pub at: Option<u64>,
    /// The task's key, "" for a handoff.
    pub key: String,
    /// `claim`, `check-in`, `pass`, `return`, `stage`, `takeover`, `note`, `handoff-from` or
    /// `handoff-to`.
    pub kind: String,
    pub text: String,
    pub pct: Option<u8>,
}

/// A task's progress in the recap: the session's latest check-in on it, else 0025's guess.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub key: String,
    pub pct: u32,
    pub text: String,
    /// It's 0025's guess, not a check-in: the window says so.
    pub guess: bool,
}

/// Who the session is.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Who {
    pub account: String,
    pub id: String,
    pub name: String,
    pub project: String,
    /// The checkout's folder name (`session::checkout`), "" when none.
    pub checkout: String,
    pub group: ListState,
    pub word: String,
    pub at: Option<u64>,
}

/// Time and tokens of the session, and of its chain.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Cost {
    pub found: transcripts::Found,
    pub start: Option<u64>,
    pub end: Option<u64>,
    pub minutes: u64,
    pub tokens: Tokens,
    pub total_tokens: u64,
    /// The chain's sums (this session's alone when it has no chain); `read` of `sessions`.
    pub chain_minutes: u64,
    pub chain_tokens: Tokens,
    pub chain_total_tokens: u64,
    pub chain_read: usize,
    pub chain_sessions: usize,
    /// The task's `est-tokens` when the session worked on one task, and the session's and the
    /// chain's share of it.
    pub est_tokens: u64,
    pub est_pct: Option<u32>,
    pub chain_est_pct: Option<u32>,
}

/// A session's recap.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Recap {
    pub who: Who,
    pub tasks: Vec<TaskRef>,
    pub history: Vec<Event>,
    pub chain: Vec<Link>,
    pub cost: Cost,
    pub progress: Vec<Progress>,
}

/// A log line of `who` as history: what it did, in words.
fn event_of(
    line: &str,
    key: &str,
    id: &str,
    to_epoch: &dyn Fn(&str) -> Option<i64>,
) -> Option<Event> {
    let who = work::log_who(line)?;
    let names = |w: &str| work::mentions(w).iter().any(|m| m == id);
    // A takeover is the old holder's too: it handed the claim on.
    let took = queue::takeover_line(line);
    let handed = took.as_ref().is_some_and(|(_, _, old)| names(old));
    if !names(who) && !handed {
        return None;
    }
    let stamp = line.strip_prefix("- ")?.get(..16)?;
    let at = to_epoch(stamp).and_then(|s| u64::try_from(s).ok());
    let ev = |kind: &str, text: String, pct: Option<u8>| Event {
        at,
        key: key.to_string(),
        kind: kind.to_string(),
        text: agents::plain_line(&text, HISTORY_CHARS),
        pct,
    };
    if let Some((step, detail)) = queue::stage_line(line) {
        let rank = |s: &str| queue::STAGES.iter().position(|x| *x == s);
        let move_words = format!(
            "{} → {}",
            queue::stage_label(&step.from),
            queue::stage_label(&step.to)
        );
        return Some(if detail == "claimed" {
            ev("claim", format!("claimed it ({move_words})"), None)
        } else if rank(&step.to) < rank(&step.from) {
            ev("return", format!("sent it back {move_words}"), None)
        } else if detail.is_empty() {
            ev("pass", format!("passed it {move_words}"), None)
        } else {
            ev("pass", format!("passed it {move_words} ({detail})"), None)
        });
    }
    if let Some(c) = queue::checkin_line(line) {
        return Some(ev("check-in", c.text, c.pct));
    }
    if let Some((_, new, old)) = took {
        let text = if names(&new) {
            format!("took over the claim from {}", work::label(&old))
        } else {
            format!("handed the claim to {}", work::label(&new))
        };
        return Some(ev("takeover", text, None));
    }
    let what = line.split_once(": ").map(|(_, w)| w).unwrap_or("");
    Some(ev("note", what.to_string(), None))
}

/// What a recap reads besides the list: the queues' tasks with their text, and each session's
/// transcript numbers and first prompt.
pub struct Sources<'a> {
    /// The accounts Muster reads: a session not listed is read only when a task stamps it with
    /// one of them.
    pub allowed: &'a [Account],
    /// Every task of every queue Muster reads, keyed (`Task::prefix` set).
    pub tasks: &'a [Task],
    /// A task's text after its front matter, None when it can't be read.
    pub body: &'a dyn Fn(&Task) -> Option<String>,
    pub usages: &'a dyn Fn(&[String]) -> Vec<Usage>,
    pub first_prompt: &'a dyn Fn(&[String]) -> Vec<Option<String>>,
    /// A log stamp in seconds since 1970 (`queue::stamp_epoch`).
    pub to_epoch: &'a dyn Fn(&str) -> Option<i64>,
    pub home: &'a Path,
}

/// The recap of session `id` of account `account`, among `listed` (every session the latest
/// read listed, of the allowed accounts) with `projects` for its project.
pub fn recap(
    account: &str,
    id: &str,
    listed: &[(String, ListEntry)],
    projects: &[ProjectFolders],
    src: &Sources,
) -> Result<Recap, String> {
    if !transcripts::is_short_id(id) {
        return Err("bad session id".into());
    }
    let (_, entry) = listed
        .iter()
        .find(|(l, e)| l == account && e.id == id)
        .ok_or_else(|| format!("session {id} isn't in cc-{account}'s list now"))?;
    let worked: Vec<&Task> = src
        .tasks
        .iter()
        .filter(|t| t.session_ids.iter().any(|s| s.id == id))
        .collect();
    let holds = |t: &Task| work::mentions(t.holder()).iter().any(|m| m == id);
    let key_of = |t: &Task| crate::key::key(&t.prefix, &t.id);
    let tasks: Vec<TaskRef> = worked
        .iter()
        .map(|t| TaskRef {
            queue: t.queue.clone(),
            id: t.id.clone(),
            key: key_of(t),
            title: t.title.clone(),
            stage: t.stage.clone(),
            holds: holds(t),
        })
        .collect();
    let mut history: Vec<Event> = Vec::new();
    let mut progress: Vec<Progress> = Vec::new();
    for t in &worked {
        let body = (src.body)(t).unwrap_or_default();
        let key = key_of(t);
        for line in queue::log_section(&body).unwrap_or("").lines() {
            if let Some(e) = event_of(line, &key, id, src.to_epoch) {
                history.push(e);
            }
        }
        let mine = t
            .checkins
            .iter()
            .rev()
            .find(|c| work::mentions(&c.who).iter().any(|m| m == id));
        progress.push(match mine {
            Some(c) => Progress {
                key,
                pct: c.pct.map(u32::from).unwrap_or(0),
                text: agents::plain_line(&c.text, HISTORY_CHARS),
                guess: false,
            },
            None => {
                let g = work::progress_guess(&t.stage, queue::criteria(&body));
                Progress {
                    key,
                    pct: g.pct,
                    text: g.basis,
                    guess: true,
                }
            }
        });
    }
    // An allowed account vouches for a session its list has or a task stamps with it (0099,
    // work.rs's rule). A transcript reachable through an allowed folder proves nothing: on the
    // owner's Mac `~/.claude-hg/projects` links to the company account's.
    let vouched: HashSet<&str> = listed
        .iter()
        .map(|(_, e)| e.id.as_str())
        .chain(
            src.tasks
                .iter()
                .flat_map(|t| &t.session_ids)
                .filter(|s| s.readable(src.allowed))
                .map(|s| s.id.as_str()),
        )
        .collect();
    let links = chain(
        id,
        listed,
        &|s| vouched.contains(s),
        src.first_prompt,
        src.home,
    );
    let ids: Vec<String> = links
        .iter()
        .filter(|l| vouched.contains(l.id.as_str()))
        .map(|l| l.id.clone())
        .collect();
    let mut read = (src.usages)(&ids).into_iter();
    let usages: Vec<Usage> = links
        .iter()
        .map(|l| {
            if vouched.contains(l.id.as_str()) {
                read.next().unwrap_or_else(Usage::missing)
            } else {
                Usage::not_read()
            }
        })
        .collect();
    let at = links.iter().position(|l| l.current).unwrap_or(0);
    let own = usages.get(at).cloned().unwrap_or_else(Usage::missing);
    // Handoffs, at the start of the session after each one.
    let start_of = |i: usize| usages.get(i).and_then(|u| u.start);
    let shown = |l: &Link| {
        if l.name.is_empty() {
            l.id.clone()
        } else {
            format!("{} ({})", l.name, l.id)
        }
    };
    if at > 0 {
        // First among what happened at its start (the sort keeps the order of equal times).
        history.insert(
            0,
            Event {
                at: start_of(at),
                key: String::new(),
                kind: "handoff-from".into(),
                text: format!("took over from {} by a handoff", shown(&links[at - 1])),
                pct: None,
            },
        );
    }
    if let Some(next) = links.get(at + 1) {
        history.push(Event {
            at: start_of(at + 1),
            key: String::new(),
            kind: "handoff-to".into(),
            text: format!("handed off to {}", shown(next)),
            pct: None,
        });
    }
    history.sort_by_key(|e| e.at.unwrap_or(0));
    let sum = transcripts::actuals(&usages);
    let est = match worked.as_slice() {
        [one] => one.est_token_count,
        _ => 0,
    };
    let cost = Cost {
        found: own.found,
        start: own.start,
        end: own.end,
        minutes: own.minutes,
        tokens: own.tokens,
        total_tokens: own.tokens.total(),
        chain_minutes: sum.minutes,
        chain_tokens: sum.tokens,
        chain_total_tokens: sum.tokens.total(),
        chain_read: sum.read,
        chain_sessions: links.len(),
        est_tokens: est,
        est_pct: work::pct(own.tokens.total(), est),
        chain_est_pct: work::pct(sum.tokens.total(), est),
    };
    let project = project_of(&entry.cwd, projects).unwrap_or("").to_string();
    let who = Who {
        account: account.to_string(),
        id: id.to_string(),
        name: entry.name.clone(),
        project,
        checkout: if entry.cwd.is_absolute() {
            crate::session::checkout(&entry.cwd, Some(src.home)).unwrap_or_default()
        } else {
            String::new()
        },
        group: entry.state,
        word: entry.word.clone(),
        at: own.end.or(entry.started),
    };
    Ok(Recap {
        who,
        tasks,
        history,
        chain: links,
        cost,
        progress,
    })
}

/// The recap of session `id` in account `account` through the app's reader and lister, with the
/// tasks of `queues` (name, folder; each task's text read through `queue::read_body`, so at most
/// 1 MB): refused for an account Muster doesn't read and a session the latest list doesn't have.
/// It polls the lister as the view does, so a recap left open on another view, where nothing
/// else polls, still shows the session's state as Claude Code reports it now.
#[allow(clippy::too_many_arguments)] // Each is one source the command already holds.
pub fn recap_for(
    lister: &Arc<Lister>,
    reader: &Reader,
    allowed: &[Account],
    projects: &[ProjectFolders],
    queues: &[(String, PathBuf)],
    tasks: &[Task],
    account: &str,
    id: &str,
    now: u64,
    home: &Path,
) -> Result<Recap, String> {
    if accounts::find(account, allowed).is_none() {
        return Err(format!("cc-{account} isn't an account Muster reads"));
    }
    lister.poll(allowed, projects.to_vec(), now);
    let listed = lister.listed(allowed);
    let body = |t: &Task| {
        let root = queues.iter().find(|(n, _)| *n == t.queue).map(|(_, p)| p)?;
        let text = queue::read_body(root, &t.id).ok()?;
        Some(queue::front_matter(&text).map_or(text.clone(), |(_, b)| b.to_string()))
    };
    let usages = |ids: &[String]| reader.usages(allowed, ids, now);
    let first_prompt = |ids: &[String]| reader.first_prompts(allowed, ids);
    // In-process: a busy session is named on hundreds of log lines, read every 15 s.
    let to_epoch = queue::local_epoch;
    recap(
        account,
        id,
        &listed,
        projects,
        &Sources {
            allowed,
            tasks,
            body: &body,
            usages: &usages,
            first_prompt: &first_prompt,
            to_epoch: &to_epoch,
            home,
        },
    )
}

/// Session `id`'s transcript as Markdown (0025's view), only in an account Muster reads and only
/// for a session the latest list has.
pub fn transcript_for(
    lister: &Lister,
    reader: &Reader,
    allowed: &[Account],
    account: &str,
    id: &str,
) -> Result<String, String> {
    if !transcripts::is_short_id(id) {
        return Err("bad session id".into());
    }
    if accounts::find(account, allowed).is_none() {
        return Err(format!("cc-{account} isn't an account Muster reads"));
    }
    if !lister
        .listed(allowed)
        .iter()
        .any(|(l, e)| l == account && e.id == id)
    {
        return Err(format!("session {id} isn't in cc-{account}'s list now"));
    }
    reader.transcript_markdown(allowed, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::parse_list;
    use crate::transcripts::tests::{answers, calls, home, prompt, reader, said, PROJECT};
    use crate::transcripts::Found;
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    fn project(name: &str, path: &Path) -> Project {
        Project {
            key: path.to_string_lossy().to_string(),
            name: name.into(),
            path: path.to_string_lossy().to_string(),
            source: "found".into(),
            group: String::new(),
            hidden: false,
            ok: true,
            gate: String::new(),
            run: String::new(),
            prefix: String::new(),
            label: String::new(),
            category: String::new(),
            colour: String::new(),
            group_name: String::new(),
        }
    }

    /// A playground with a `.muster` project in its main checkout (with worktrees), an old
    /// `<project>/tasks` one, a checkout that's a project of its own, and a hidden one.
    fn playground() -> (crate::testkit::TestDir, Vec<ProjectFolders>) {
        let d = crate::testkit::tempdir().unwrap();
        let pg = fs::canonicalize(d.path()).unwrap().join("playground");
        for f in [
            "muster/main/.muster",
            "muster/main/app",
            "muster/worktrees/main/mu-98-x",
            "muster2/main",
            "drover/tasks",
            "drover/main/src",
            "solo/.muster",
            "solo.worktrees/mu-1",
            "hidden/tasks",
        ] {
            fs::create_dir_all(pg.join(f)).unwrap();
        }
        let mut hidden = project("hidden", &pg.join("hidden/tasks"));
        hidden.hidden = true;
        let list = vec![
            project("muster", &pg.join("muster/main/.muster")),
            project("drover", &pg.join("drover/tasks")),
            project("solo", &pg.join("solo/.muster")),
            hidden,
        ];
        let folders = project_folders(&list);
        (d, folders)
    }

    #[test]
    fn a_sessions_folder_is_matched_to_its_project_worktrees_included() {
        let (d, folders) = playground();
        let pg = fs::canonicalize(d.path()).unwrap().join("playground");
        let names: Vec<&str> = folders.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["muster", "drover", "solo"], "the hidden one isn't");
        for (cwd, want) in [
            ("muster/main", Some("muster")),
            ("muster/main/app", Some("muster")),
            ("muster/worktrees/main/mu-98-x", Some("muster")),
            ("drover/main/src", Some("drover")),
            ("drover/tasks", Some("drover")),
            ("solo", Some("solo")),
            ("solo.worktrees/mu-1", Some("solo")),
            ("muster2/main", None),
            ("hidden/tasks", None),
            ("", None),
            ("gone/away", None),
        ] {
            assert_eq!(project_of(&pg.join(cwd), &folders), want, "{cwd}");
        }
        assert_eq!(project_of(Path::new(""), &folders), None);
        assert_eq!(project_of(Path::new("muster/main"), &folders), None);
        // Through a link to the folder, the same project.
        symlink(pg.join("muster/main"), d.path().join("link")).unwrap();
        assert_eq!(project_of(&d.path().join("link"), &folders), Some("muster"));
    }

    const A: &str = "aaaaaaa1";
    const B: &str = "aaaaaaa2";
    const C: &str = "aaaaaaa3";

    /// `claude agents` output for one account: (id, name, state fields, cwd).
    fn listing(entries: &[(&str, &str, &str, &str)]) -> String {
        let items: Vec<String> = entries
            .iter()
            .enumerate()
            .map(|(i, (id, name, state, cwd))| {
                format!(
                    r#"{{"sessionId": "{id}-0000-4000-8000-000000000000", "name": {}, "cwd": {}, "startedAt": {}, {state}}}"#,
                    serde_json::to_string(name).unwrap(),
                    serde_json::to_string(cwd).unwrap(),
                    1_792_000_000_000u64 + i as u64 * 1000
                )
            })
            .collect();
        format!("[{}]", items.join(","))
    }

    fn transcript(h: &Path, id: &str, lines: &[String]) {
        let p = h
            .join(".claude-mg/projects")
            .join(PROJECT)
            .join(format!("{id}-0000-4000-8000-000000000000.jsonl"));
        fs::write(p, lines.join("\n") + "\n").unwrap();
    }

    #[test]
    fn rows_group_like_the_agents_view_with_messages_projects_and_other() {
        let h = home();
        let (pd, folders) = playground();
        let pg = fs::canonicalize(pd.path()).unwrap().join("playground");
        let cwd = |f: &str| pg.join(f).to_string_lossy().to_string();
        transcript(h.path(), "aaaaaaa1", &[said("Merged the sidebar fix.")]);
        transcript(
            h.path(),
            "aaaaaaa2",
            &[
                said("Pushing."),
                calls("t1", "Bash", "git push origin main"),
            ],
        );
        transcript(h.path(), "aaaaaaa3", &[said("Working on it.")]);
        let out = listing(&[
            (
                "aaaaaaa1",
                "Asghar_6",
                r#""state": "done""#,
                &cwd("muster/main"),
            ),
            (
                "aaaaaaa2",
                "Gholi_3",
                r#""state": "blocked", "status": "waiting", "waitingFor": "permission prompt""#,
                &cwd("drover/main/src"),
            ),
            (
                "aaaaaaa3",
                "builder",
                r#""state": "working", "status": "busy""#,
                &cwd("muster/worktrees/main/mu-98-x"),
            ),
            (
                "aaaaaaa4",
                "QA_2",
                r#""state": "failed""#,
                "/Users/someone/code/app",
            ),
            ("aaaaaaa5", "", r#""state": "stopped""#, ""),
            (
                "aaaaaaa6",
                "waits",
                r#""state": "blocked", "waitingFor": "permission prompt""#,
                &cwd("solo"),
            ),
            (
                "aaaaaaa7",
                "chat",
                r#""status": "idle""#,
                &cwd("muster/main"),
            ),
        ]);
        let (r, a) = reader(h.path());
        let lists = vec![
            ("mg".to_string(), parse_list(out.as_bytes())),
            ("hg".to_string(), Err("claude isn't installed".to_string())),
        ];
        let rows = rows(&lists, &r, &a, &folders);
        let got: Vec<(&str, ListState, &str, &str, &str)> = rows
            .iter()
            .map(|r| {
                (
                    r.id.as_str(),
                    r.group,
                    r.word.as_str(),
                    r.message.as_str(),
                    r.project.as_str(),
                )
            })
            .collect();
        assert_eq!(
            got,
            [
                (
                    "aaaaaaa1",
                    ListState::Completed,
                    "done",
                    "Merged the sidebar fix.",
                    "muster"
                ),
                (
                    "aaaaaaa2",
                    ListState::NeedsInput,
                    "blocked",
                    "approve Bash: git push origin main",
                    "drover"
                ),
                (
                    "aaaaaaa3",
                    ListState::Working,
                    "working",
                    "Working on it.",
                    "muster"
                ),
                ("aaaaaaa4", ListState::Completed, "failed", "failed", ""),
                ("aaaaaaa5", ListState::Completed, "stopped", "stopped", ""),
                (
                    "aaaaaaa6",
                    ListState::NeedsInput,
                    "blocked",
                    "waiting for permission prompt",
                    "solo"
                ),
                ("aaaaaaa7", ListState::Completed, "idle", "idle", "muster"),
            ]
        );
        assert!(rows.iter().all(|r| r.account == "mg"));
        // Last active: the transcript's time when there is one, else the start.
        assert_eq!(rows[3].at, Some(1_792_000_003));
        assert!(rows[0].at.unwrap() > 1_792_000_003);
        // Per project: only its own; under All, every one, those in no project too.
        let list = List {
            rows: rows.clone(),
            ..List::default()
        };
        assert_eq!(for_project(list.clone(), Some("muster")).rows.len(), 3);
        assert_eq!(for_project(list.clone(), None).rows.len(), 7);
        assert!(for_project(list, Some("nope")).rows.is_empty());
    }

    /// AC6: what crosses to the window has no folder in it, and names and messages are the
    /// owner's text, as plain text.
    #[test]
    fn the_window_gets_no_folder_and_hostile_text_as_text() {
        let h = home();
        let (_pd, folders) = playground();
        let hostile = "<img src=x onerror=alert(1)>\u{202e} **x** [l](javascript:1)";
        transcript(h.path(), "aaaaaaa1", &[said(hostile)]);
        let out = listing(&[(
            "aaaaaaa1",
            hostile,
            r#""state": "done""#,
            "/Users/someone/playground/secret-folder",
        )]);
        let (r, a) = reader(h.path());
        let rows = rows(
            &[("mg".into(), parse_list(out.as_bytes()))],
            &r,
            &a,
            &folders,
        );
        let json = serde_json::to_string(&List {
            rows,
            ..List::default()
        })
        .unwrap();
        assert!(
            !json.contains("secret-folder") && !json.contains("cwd"),
            "{json}"
        );
        assert!(!json.contains("/Users/"), "{json}");
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let row = &v["rows"][0];
        let plain = "<img src=x onerror=alert(1)> **x** [l](javascript:1)";
        assert_eq!(row["name"], plain);
        assert_eq!(row["message"], plain);
        let keys: Vec<&str> = row
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            ["account", "at", "group", "id", "message", "name", "project", "started", "word"]
        );
    }

    fn inline(job: Box<dyn FnOnce() + Send>) {
        job();
    }

    fn counting(out: String) -> (Fetch, Arc<Mutex<Vec<String>>>) {
        let asked: Arc<Mutex<Vec<String>>> = Arc::default();
        let seen = Arc::clone(&asked);
        let fetch: Fetch = Arc::new(move |a: &Account| {
            seen.lock().unwrap().push(a.label().to_string());
            Ok(out.clone().into_bytes())
        });
        (fetch, asked)
    }

    /// AC2: with mu-99's fixture home (`~/.claude`, `~/.claude-ih`, mg and hg), only mg and hg
    /// are ever run for, and a transcript in the company account's work project is never read.
    #[test]
    fn the_list_runs_only_for_allowed_accounts_at_most_every_15_seconds() {
        let d = crate::accounts::tests::fake_home();
        let h = d.path();
        let work = h.join(".claude/projects/-Users-x-code-app");
        fs::write(
            work.join("aaaaaaa1-0000-4000-8000-000000000000.jsonl"),
            said("company secret") + "\n",
        )
        .unwrap();
        let (fetch, asked) = counting(listing(&[(
            "aaaaaaa1",
            "s",
            r#""state": "done""#,
            "/Users/x/code/app",
        )]));
        let reader = Arc::new(Reader::new("-Users-x-playground".into()));
        let lister = Lister::new(fetch, inline, reader);
        // Settings as a hand-edited file might hold them: "" and .claude never resolve.
        let labels: Vec<String> = ["", ".claude", "mg", "hg"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let allowed = accounts::resolve(&labels, h);
        let list = lister.poll(&allowed, Vec::new(), 1000);
        assert_eq!(*asked.lock().unwrap(), ["mg", "hg"]);
        assert_eq!(list.rows.len(), 2, "one per account");
        assert!(
            list.rows.iter().all(|r| r.message == "done"),
            "{:?}",
            list.rows
        );
        assert_eq!(list.accounts, ["cc-mg", "cc-hg"]);
        assert!(!list.reading);
        // Not again within 15 s; again after.
        lister.poll(&allowed, Vec::new(), 1014);
        assert_eq!(asked.lock().unwrap().len(), 2);
        lister.poll(&allowed, Vec::new(), 1015);
        assert_eq!(asked.lock().unwrap().len(), 4);
        // An account taken off the list: its rows go at once, before any new read.
        let mg = accounts::resolve(&["mg".into()], h);
        let list = lister.poll(&mg, Vec::new(), 1016);
        assert_eq!(list.rows.len(), 1);
        assert_eq!(list.rows[0].account, "mg");
        // No account at all: nothing runs and nothing shows.
        let list = lister.poll(&[], Vec::new(), 5000);
        assert!(list.rows.is_empty() && !list.reading);
        assert_eq!(asked.lock().unwrap().len(), 4);
    }

    #[test]
    fn the_list_reads_off_the_polling_thread_and_never_twice_at_once() {
        static JOBS: Mutex<Vec<Box<dyn FnOnce() + Send>>> = Mutex::new(Vec::new());
        fn later(job: Box<dyn FnOnce() + Send>) {
            JOBS.lock().unwrap().push(job);
        }
        let d = crate::accounts::tests::fake_home();
        let calls = Arc::new(AtomicUsize::new(0));
        let n = Arc::clone(&calls);
        let fetch: Fetch = Arc::new(move |_: &Account| {
            n.fetch_add(1, Ordering::SeqCst);
            Ok(b"[]".to_vec())
        });
        let lister = Lister::new(
            fetch,
            later,
            Arc::new(Reader::new("-Users-x-playground".into())),
        );
        let allowed = accounts::resolve(&accounts::default_labels(), d.path());
        // The poll hands back at once, reading, with nothing fetched on its thread.
        let list = lister.poll(&allowed, Vec::new(), 100);
        assert!(list.reading && list.rows.is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        // Due again, but one is running: no second.
        lister.poll(&allowed, Vec::new(), 200);
        let jobs: Vec<_> = JOBS.lock().unwrap().drain(..).collect();
        assert_eq!(jobs.len(), 1);
        for job in jobs {
            job();
        }
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(!lister.poll(&allowed, Vec::new(), 201).reading);
        let fetch: Fetch = Arc::new(|_: &Account| Err("claude isn't installed".to_string()));
        let l = Lister::new(
            fetch,
            inline,
            Arc::new(Reader::new("-Users-x-playground".into())),
        );
        assert_eq!(
            l.poll(&allowed, Vec::new(), 1).problems,
            [
                "cc-mg: claude isn't installed",
                "cc-hg: claude isn't installed"
            ]
        );
    }

    /// R8: reopened long after its last read, the view says it's reading rather than showing
    /// that read as now, until the new read lands.
    #[test]
    fn a_list_older_than_max_age_reads_as_reading_until_the_new_one_lands() {
        static JOBS: Mutex<Vec<Box<dyn FnOnce() + Send>>> = Mutex::new(Vec::new());
        fn later(job: Box<dyn FnOnce() + Send>) {
            JOBS.lock().unwrap().push(job);
        }
        let run = || {
            let jobs: Vec<_> = JOBS.lock().unwrap().drain(..).collect();
            jobs.into_iter().for_each(|job| job());
        };
        let d = crate::accounts::tests::fake_home();
        let (fetch, _) = counting(listing(&[(B, "b", r#""state": "done""#, "")]));
        let lister = Lister::new(
            fetch,
            later,
            Arc::new(Reader::new("-Users-x-playground".into())),
        );
        let allowed = accounts::resolve(&["mg".into()], d.path());
        lister.poll(&allowed, Vec::new(), 1000);
        run();
        let list = lister.poll(&allowed, Vec::new(), 1000 + agents::MAX_AGE_SECS);
        assert!(!list.reading && list.rows.len() == 1, "{list:?}");
        // The view closed at 1000 + MAX_AGE and opened three hours later: no rows, reading.
        let list = lister.poll(&allowed, Vec::new(), 12_000);
        assert!(list.reading && list.rows.is_empty(), "{list:?}");
        run();
        let list = lister.poll(&allowed, Vec::new(), 12_001);
        assert!(!list.reading && list.rows.len() == 1, "{list:?}");
    }

    #[test]
    fn a_resumed_handoffs_path_is_read_from_the_first_prompt() {
        let cmd = "<command-message>handoff</command-message>\n<command-name>/handoff</command-name>\n<command-args>resume /Users/x/playground/m/.claude/work/handoffs/a.md</command-args>";
        assert_eq!(
            resume_path(cmd),
            Some(PathBuf::from(
                "/Users/x/playground/m/.claude/work/handoffs/a.md"
            ))
        );
        for (typed, want) in [
            (
                "Run /handoff resume with /p/.claude/work/handoffs/b.md and follow it",
                Some("/p/.claude/work/handoffs/b.md"),
            ),
            ("/handoff resume /p/h/c.md.", Some("/p/h/c.md")),
            ("/handoff resume relative/c.md", None),
            ("/handoff resume /p/c.txt", None),
            ("/handoff resume /p/../etc/c.md", None),
            ("/handoff resume", None),
            ("please resume /p/c.md", None),
            (
                "<command-name>/handoff</command-name><command-args>write</command-args>",
                None,
            ),
            ("<command-name>/handoff</command-name>", None),
            ("", None),
        ] {
            assert_eq!(resume_path(typed), want.map(PathBuf::from), "{typed}");
        }
    }

    /// A home with `playground/m/.claude/work/handoffs/`.
    fn handoff_home() -> (crate::testkit::TestDir, PathBuf) {
        let d = crate::testkit::tempdir().unwrap();
        let dir = fs::canonicalize(d.path())
            .unwrap()
            .join("playground/m/.claude/work/handoffs");
        fs::create_dir_all(&dir).unwrap();
        (d, dir)
    }

    #[test]
    fn a_handoff_is_read_only_from_a_regular_file_in_a_handoffs_folder_under_the_playground() {
        let (d, dir) = handoff_home();
        let h = d.path();
        let body = "---\nsession: 1c6af0d9-886e-45ab-a1e3-de978458ce1e\ngeneration: 1\nsuccessor: Asghar_2\n---\n\n# Notes\nsession: ffffffff\n";
        fs::write(dir.join("a.md"), body).unwrap();
        let want = Handoff {
            session: "1c6af0d9".into(),
            successor: "Asghar_2".into(),
        };
        assert_eq!(read_handoff(&dir.join("a.md"), h), Some(want.clone()));
        // Renamed once resumed: found as .done.md.
        fs::rename(dir.join("a.md"), dir.join("a.done.md")).unwrap();
        assert_eq!(read_handoff(&dir.join("a.md"), h), Some(want.clone()));
        assert_eq!(read_handoff(&dir.join("a.done.md"), h), Some(want));
        // The session's other shapes.
        fs::write(dir.join("b.md"), "---\nsession: 34fa2f95 (QA_5)\n---\n").unwrap();
        assert_eq!(
            read_handoff(&dir.join("b.md"), h).unwrap().session,
            "34fa2f95"
        );
        fs::write(
            dir.join("c.md"),
            "---\nsession: none\nsuccessor: x\u{202e}y\n---\n",
        )
        .unwrap();
        assert_eq!(
            read_handoff(&dir.join("c.md"), h),
            Some(Handoff {
                session: String::new(),
                successor: "xy".into()
            })
        );
        // No front matter, or one that never closes.
        fs::write(dir.join("d.md"), "# Handoff\nsession: 1c6af0d9\n").unwrap();
        fs::write(dir.join("e.md"), "---\nsession: 1c6af0d9\n").unwrap();
        assert_eq!(read_handoff(&dir.join("d.md"), h), None);
        assert_eq!(read_handoff(&dir.join("e.md"), h), None);
        // A link, even to a good handoff in the same folder.
        fs::write(dir.join("good.md"), body).unwrap();
        symlink(dir.join("good.md"), dir.join("link.md")).unwrap();
        assert_eq!(read_handoff(&dir.join("link.md"), h), None);
        // Over 256 KB.
        fs::write(
            dir.join("big.md"),
            format!("{body}{}", "x".repeat(MAX_HANDOFF_BYTES as usize)),
        )
        .unwrap();
        assert_eq!(read_handoff(&dir.join("big.md"), h), None);
        // Outside a handoffs folder, nested deeper, or outside the playground.
        let real = fs::canonicalize(h).unwrap();
        for bad in [
            real.join("playground/m/.claude/work/x.md"),
            real.join("playground/m/.claude/work/handoffs/sub/x.md"),
            real.join("playground/m/work/handoffs/x.md"),
            real.join("other/.claude/work/handoffs/x.md"),
        ] {
            fs::create_dir_all(bad.parent().unwrap()).unwrap();
            fs::write(&bad, body).unwrap();
            assert_eq!(read_handoff(&bad, h), None, "{}", bad.display());
        }
        // A handoffs folder that's a link out of the playground.
        fs::create_dir_all(real.join("playground/n/.claude/work")).unwrap();
        symlink(
            real.join("other/.claude/work/handoffs"),
            real.join("playground/n/.claude/work/handoffs"),
        )
        .unwrap();
        assert_eq!(
            read_handoff(&real.join("playground/n/.claude/work/handoffs/x.md"), h),
            None
        );
        // A folder, and a file that isn't there.
        assert_eq!(read_handoff(&dir, h), None);
        assert_eq!(read_handoff(&dir.join("missing.md"), h), None);
    }

    fn entry(id: &str, name: &str, started: u64, state: ListState) -> (String, ListEntry) {
        (
            "mg".to_string(),
            ListEntry {
                id: id.into(),
                name: name.into(),
                cwd: PathBuf::from("/Users/someone/playground/muster/main"),
                started: Some(started),
                state,
                word: "done".into(),
                waiting_for: String::new(),
            },
        )
    }

    fn resume(dir: &Path, file: &str) -> String {
        format!(
            "<command-message>handoff</command-message>\n<command-name>/handoff</command-name>\n<command-args>resume {}</command-args>",
            dir.join(file).display()
        )
    }

    type Prompts = HashMap<String, String>;

    /// A → B → C by handoffs: B resumed A's (`session: A`, `successor: Asghar_5`), and both C and
    /// a stray session resumed B's, which names Asghar_6 (C) as its successor.
    fn chain_fixture() -> (crate::testkit::TestDir, Vec<(String, ListEntry)>, Prompts) {
        let (d, dir) = handoff_home();
        fs::write(
            dir.join("ha.md"),
            format!("---\nsession: {A}-1111\nsuccessor: Asghar_5\n---\n"),
        )
        .unwrap();
        fs::write(
            dir.join("hb.done.md"),
            format!("---\nsession: {B}\nsuccessor: Asghar_6\n---\n"),
        )
        .unwrap();
        let listed = vec![
            entry(B, "", 200, ListState::Completed),
            entry("aaaaaaa4", "Stray", 250, ListState::Completed),
            entry(C, "Asghar_6", 300, ListState::Working),
        ];
        let prompts = HashMap::from([
            (A.to_string(), "Build mu-1".to_string()),
            (B.to_string(), resume(&dir, "ha.md")),
            (C.to_string(), resume(&dir, "hb.md")),
            ("aaaaaaa4".to_string(), resume(&dir, "hb.md")),
        ]);
        (d, listed, prompts)
    }

    fn prompts_of(m: &Prompts) -> impl Fn(&[String]) -> Vec<Option<String>> + '_ {
        move |ids: &[String]| ids.iter().map(|i| m.get(i).cloned()).collect()
    }

    /// Every session vouched for, as when each is listed or stamped by an allowed account.
    fn any(_: &str) -> bool {
        true
    }

    fn ids(links: &[Link]) -> Vec<(&str, &str, bool)> {
        links
            .iter()
            .map(|l| (l.id.as_str(), l.name.as_str(), l.current))
            .collect()
    }

    #[test]
    fn handoffs_make_one_chain_both_ways() {
        let (d, listed, prompts) = chain_fixture();
        let fp = prompts_of(&prompts);
        let want = |cur: &str| {
            vec![
                (A, "", cur == A),
                (B, "Asghar_5", cur == B),
                (C, "Asghar_6", cur == C),
            ]
        };
        assert_eq!(ids(&chain(B, &listed, &any, &fp, d.path())), want(B));
        assert_eq!(ids(&chain(C, &listed, &any, &fp, d.path())), want(C));
        // A isn't listed any more: its successors still are.
        assert_eq!(ids(&chain(A, &listed, &any, &fp, d.path())), want(A));
        // Without names, the first to start wins.
        let mut unnamed = listed.clone();
        unnamed[2].1.name.clear();
        let got = chain(B, &unnamed, &any, &fp, d.path());
        assert_eq!(got.last().unwrap().id, "aaaaaaa4");
        // A session on its own is a chain of one.
        assert_eq!(
            ids(&chain("aaaaaaa9", &listed, &any, &fp, d.path())),
            [("aaaaaaa9", "", true)]
        );
    }

    #[test]
    fn a_chain_ignores_unsafe_handoffs_and_never_loops() {
        let (d, dir) = handoff_home();
        let h = d.path();
        // B's handoff is a link; C's sits outside a handoffs folder; the next one is too big.
        let good = format!("---\nsession: {A}\n---\n");
        fs::write(dir.join("real.md"), &good).unwrap();
        symlink(dir.join("real.md"), dir.join("link.md")).unwrap();
        let loose = fs::canonicalize(h).unwrap().join("playground/m/loose.md");
        fs::write(&loose, &good).unwrap();
        fs::write(
            dir.join("big.md"),
            format!("{good}{}", " ".repeat(MAX_HANDOFF_BYTES as usize)),
        )
        .unwrap();
        // Two sessions that resumed each other's handoffs.
        fs::write(dir.join("he.md"), "---\nsession: aaaaaaa6\n---\n").unwrap();
        fs::write(dir.join("hf.md"), "---\nsession: aaaaaaa5\n---\n").unwrap();
        let prompts = HashMap::from([
            (B.to_string(), resume(&dir, "link.md")),
            (
                C.to_string(),
                format!("/handoff resume {}", loose.display()),
            ),
            ("aaaaaaa4".to_string(), resume(&dir, "big.md")),
            ("aaaaaaa5".to_string(), resume(&dir, "he.md")),
            ("aaaaaaa6".to_string(), resume(&dir, "hf.md")),
        ]);
        let fp = prompts_of(&prompts);
        let listed: Vec<_> = [B, C, "aaaaaaa4", "aaaaaaa5", "aaaaaaa6"]
            .iter()
            .map(|i| entry(i, "", 1, ListState::Completed))
            .collect();
        for one in [B, C, "aaaaaaa4"] {
            assert_eq!(chain(one, &listed, &any, &fp, h).len(), 1, "{one}");
        }
        assert_eq!(
            chain(A, &listed, &any, &fp, h).len(),
            1,
            "no safe handoff names A"
        );
        let looped = chain("aaaaaaa5", &listed, &any, &fp, h);
        assert_eq!(looped.len(), 2, "{looped:?}");
    }

    const TASK1: &str = "---
id: 0001
title: Sidebar heading fix
stage: testing
status: doing
est-tokens: 1M
claimed-by:
held-by: asghar_6 (main · session aaaaaaa3 · cc-mg)
heartbeat:
waiting_on:
---

# Sidebar heading fix

## Acceptance criteria
- [x] AC1: one
- [ ] AC2: two

## Log
- 2026-10-14 09:00 shapour (main · session 99999999 · cc-mg): created
- 2026-10-14 09:05 asghar_4 (main · session aaaaaaa1 · cc-mg): stage ready → building (claimed)
- 2026-10-14 09:30 asghar_4 (main · session aaaaaaa1 · cc-mg): check-in 30% · tests first
- 2026-10-14 10:00 asghar_5 (main · session aaaaaaa2 · cc-mg): took over the claim from asghar_4 (main · session aaaaaaa1 · cc-mg) (handed off)
- 2026-10-14 10:20 asghar_5 (main · session aaaaaaa2 · cc-mg): check-in 60% · core done, now the window
- 2026-10-14 10:25 asghar_5 (main · session aaaaaaa2 · cc-mg): note: <b>keep</b> the old chip
- 2026-10-14 10:40 asghar_5 (main · session aaaaaaa2 · cc-mg): check-in 70% · the window
- 2026-10-14 11:00 asghar_6 (main · session aaaaaaa3 · cc-mg): took over the claim from asghar_5 (main · session aaaaaaa2 · cc-mg) (handed off)
- 2026-10-14 11:30 asghar_6 (main · session aaaaaaa3 · cc-mg): stage building → testing
- 2026-10-14 11:45 qa (main · session 88888888 · cc-mg): stage testing → building
- 2026-10-14 12:00 asghar_6 (main · session aaaaaaa3 · cc-mg): stage building → testing
";

    const TASK2: &str = "---
id: 0002
title: Another task
stage: building
status: doing
claimed-by: asghar_5 (main · session aaaaaaa2 · cc-mg)
heartbeat:
waiting_on:
---

# Another task

## Acceptance criteria
- [x] AC1: one
- [ ] AC2: two

## Log
- 2026-10-14 10:30 asghar_5 (main · session aaaaaaa2 · cc-mg): stage ready → building (claimed)
";

    fn tasks_fixture() -> (crate::testkit::TestDir, Vec<Task>) {
        let q = crate::testkit::tempdir().unwrap();
        for f in queue::STATUS_FOLDERS {
            fs::create_dir(q.path().join(f)).unwrap();
        }
        fs::write(q.path().join("doing/0001-sidebar.md"), TASK1).unwrap();
        fs::write(q.path().join("doing/0002-another.md"), TASK2).unwrap();
        let mut tasks = queue::read_state(
            &[("muster".into(), q.path().to_path_buf())],
            &mut queue::Cache::default(),
        )
        .tasks;
        for t in &mut tasks {
            t.prefix = "mu".into();
        }
        (q, tasks)
    }

    fn usage(start: u64, minutes: u64, input: u64) -> Usage {
        Usage {
            found: Found::Read,
            tokens: Tokens {
                input,
                output: 10,
                cache_read: 100,
                cache_write: 1,
            },
            start: Some(start),
            end: Some(start + minutes * 60),
            minutes,
            ..Usage::missing()
        }
    }

    #[test]
    fn a_recap_has_its_tasks_history_chain_time_tokens_and_progress() {
        let (d, listed, prompts) = chain_fixture();
        let (q, tasks) = tasks_fixture();
        let fp = prompts_of(&prompts);
        let body = |t: &Task| {
            let text = queue::read_body(q.path(), &t.id).ok()?;
            queue::front_matter(&text).map(|(_, b)| b.to_string())
        };
        let to_epoch = |s: &str| queue::stamp_epoch(s, None);
        let e = |s: &str| to_epoch(s).map(|x| x as u64).unwrap();
        let starts = HashMap::from([
            (A.to_string(), e("2026-10-14 09:00")),
            (B.to_string(), e("2026-10-14 10:00")),
            (C.to_string(), e("2026-10-14 11:00")),
        ]);
        let usages = |ids: &[String]| -> Vec<Usage> {
            ids.iter()
                .map(|i| match starts.get(i) {
                    Some(s) if i == C => usage(*s, 60, 300_000),
                    Some(s) => usage(*s, 50, 200_000),
                    None => Usage::missing(),
                })
                .collect()
        };
        // A isn't listed: its stamp (cc-mg) vouches for it.
        fs::create_dir(d.path().join(".claude-mg")).unwrap();
        let allowed = accounts::resolve(&["mg".into()], d.path());
        let src = Sources {
            allowed: &allowed,
            tasks: &tasks,
            body: &body,
            usages: &usages,
            first_prompt: &fp,
            to_epoch: &to_epoch,
            home: d.path(),
        };
        let r = recap("mg", B, &listed, &[], &src).unwrap();
        assert_eq!((r.who.id.as_str(), r.who.account.as_str()), (B, "mg"));
        let keys: Vec<(&str, &str, &str, bool)> = r
            .tasks
            .iter()
            .map(|t| (t.key.as_str(), t.title.as_str(), t.stage.as_str(), t.holds))
            .collect();
        assert_eq!(
            keys,
            [
                ("mu-1", "Sidebar heading fix", "testing", false),
                ("mu-2", "Another task", "building", true)
            ]
        );
        let history: Vec<(&str, &str, &str)> = r
            .history
            .iter()
            .map(|h| (h.key.as_str(), h.kind.as_str(), h.text.as_str()))
            .collect();
        assert_eq!(
            history,
            [
                ("", "handoff-from", "took over from aaaaaaa1 by a handoff"),
                ("mu-1", "takeover", "took over the claim from asghar_4"),
                ("mu-1", "check-in", "core done, now the window"),
                ("mu-1", "note", "note: <b>keep</b> the old chip"),
                ("mu-2", "claim", "claimed it (Ready → Building)"),
                ("mu-1", "check-in", "the window"),
                ("mu-1", "takeover", "handed the claim to asghar_6"),
                ("", "handoff-to", "handed off to Asghar_6 (aaaaaaa3)"),
            ]
        );
        assert_eq!(r.history[2].pct, Some(60));
        assert_eq!(r.history[0].at, Some(starts[B]));
        assert_eq!(
            ids(&r.chain),
            [
                (A, "", false),
                (B, "Asghar_5", true),
                (C, "Asghar_6", false)
            ]
        );
        // Time and tokens: its own, and the chain's beside it. Two tasks: no estimate to hold to.
        assert_eq!((r.cost.minutes, r.cost.total_tokens), (50, 200_111));
        assert_eq!(
            (r.cost.chain_minutes, r.cost.chain_total_tokens),
            (160, 700_333)
        );
        assert_eq!((r.cost.chain_read, r.cost.chain_sessions), (3, 3));
        assert_eq!((r.cost.est_tokens, r.cost.est_pct), (0, None));
        // Progress: its latest check-in on mu-1; a guess on mu-2, which has none of its.
        assert_eq!(
            r.progress,
            [
                Progress {
                    key: "mu-1".into(),
                    pct: 70,
                    text: "the window".into(),
                    guess: false
                },
                Progress {
                    key: "mu-2".into(),
                    pct: 50,
                    text: "1 of 2 criteria ticked, in Building".into(),
                    guess: true
                },
            ]
        );
        // C: one task, so its estimate; the return by QA isn't its.
        let r = recap("mg", C, &listed, &[], &src).unwrap();
        let kinds: Vec<(&str, &str)> = r
            .history
            .iter()
            .map(|h| (h.kind.as_str(), h.text.as_str()))
            .collect();
        assert_eq!(
            kinds,
            [
                (
                    "handoff-from",
                    "took over from Asghar_5 (aaaaaaa2) by a handoff"
                ),
                ("takeover", "took over the claim from asghar_5"),
                ("pass", "passed it Building → Testing"),
                ("pass", "passed it Building → Testing"),
            ]
        );
        assert_eq!((r.cost.est_tokens, r.cost.est_pct), (1_000_000, Some(30)));
        assert_eq!(r.cost.chain_est_pct, Some(70));
        assert!(r.progress[0].guess);
        // Refused: a session the list doesn't have, another account's, a bad id.
        assert!(recap("mg", "aaaaaaa9", &listed, &[], &src).is_err());
        assert!(recap("hg", B, &listed, &[], &src).is_err());
        assert!(recap("mg", "../x", &listed, &[], &src).is_err());
    }

    #[test]
    fn a_session_sent_back_reads_as_a_return() {
        let line =
            "- 2026-10-14 11:45 qa (main · session 88888888 · cc-mg): stage testing → building";
        let to_epoch = |s: &str| queue::stamp_epoch(s, None);
        let e = event_of(line, "mu-1", "88888888", &to_epoch).unwrap();
        assert_eq!(
            (e.kind.as_str(), e.text.as_str()),
            ("return", "sent it back Testing → Building")
        );
        assert!(event_of(line, "mu-1", "aaaaaaa1", &to_epoch).is_none());
        assert!(event_of("free text session 88888888", "mu-1", "88888888", &to_epoch).is_none());
    }

    #[test]
    fn the_recap_and_the_transcript_are_refused_outside_the_allowlist() {
        let h = home();
        transcript(
            h.path(),
            B,
            &[prompt("2026-10-14T10:00:00Z", "go"), said("hi")],
        );
        // R11: an unlisted session whose transcript is readable: the company account's, in
        // `.claude`, reached through `.claude-hg/projects`' link. Only the list check stops it.
        const U: &str = "aaaaaaa9";
        transcripts::tests::write(
            h.path(),
            ".claude",
            &format!("{U}-0000-4000-8000-000000000000"),
            &[prompt("2026-10-14T10:00:00Z", "go"), said("company words")],
            1_792_000_000,
        );
        let (r, a) = reader(h.path());
        let reader = Arc::new(r);
        assert!(reader
            .transcript_markdown(&a, U)
            .unwrap()
            .contains("company words"));
        let (fetch, _) = counting(listing(&[(B, "b", r#""state": "done""#, "")]));
        let lister = Lister::new(fetch, inline, Arc::clone(&reader));
        lister.poll(&a, Vec::new(), 1);
        assert!(transcript_for(&lister, &reader, &a, "mg", B)
            .unwrap()
            .contains("hi"));
        for account in ["mg", "hg"] {
            assert_eq!(
                transcript_for(&lister, &reader, &a, account, U),
                Err(format!("session {U} isn't in cc-{account}'s list now"))
            );
            assert!(
                recap_for(&lister, &reader, &a, &[], &[], &[], account, U, 1, h.path()).is_err()
            );
        }
        for (account, id) in [("ih", B), ("", B), ("mg", "../../x")] {
            assert!(
                transcript_for(&lister, &reader, &a, account, id).is_err(),
                "{account} {id}"
            );
        }
        let r = recap_for(&lister, &reader, &a, &[], &[], &[], "mg", B, 1, h.path()).unwrap();
        assert_eq!(r.cost.found, Found::Read);
        assert!(recap_for(&lister, &reader, &a, &[], &[], &[], "ih", B, 1, h.path()).is_err());
        // An account off the list now: refused, though the read had it.
        assert!(recap_for(&lister, &reader, &[], &[], &[], &[], "mg", B, 1, h.path()).is_err());
        assert!(transcript_for(&lister, &reader, &[], "mg", B).is_err());
    }

    /// R11: what the lists say is used only while they say it. A session the list had and a
    /// read since hasn't is refused, and an account taken off the allowlist after a read
    /// (the lists are that read's until the next, L16) contributes nothing: not its sessions,
    /// not a successor in the chain.
    #[test]
    fn a_session_or_account_off_the_list_since_the_read_is_refused() {
        let h = home();
        let dir = fs::canonicalize(h.path())
            .unwrap()
            .join("playground/m/.claude/work/handoffs");
        fs::create_dir_all(&dir).unwrap();
        // H (cc-hg) handed off; M (cc-mg) resumed it. On the owner's Mac both accounts'
        // projects link to `.claude/projects`, so both transcripts sit there.
        const H: &str = "aaaaaab1";
        const M: &str = "aaaaaab2";
        fs::write(dir.join("hh.md"), format!("---\nsession: {H}\n---\n")).unwrap();
        let full = |id: &str| format!("{id}-0000-4000-8000-000000000000");
        let at = "2026-10-14T10:00:00Z";
        transcripts::tests::write(
            h.path(),
            ".claude",
            &full(H),
            &[
                prompt(at, "go"),
                transcripts::tests::reply("h1", at, 5, 0, 0, 0),
            ],
            1_792_000_000,
        );
        transcripts::tests::write(
            h.path(),
            ".claude",
            &full(M),
            &[
                prompt(at, &resume(&dir, "hh.md")),
                transcripts::tests::reply("m1", at, 7_000, 0, 0, 0),
            ],
            1_792_000_000,
        );
        let (r, a) = reader(h.path());
        let reader = Arc::new(r);
        let mg_lists = Arc::new(Mutex::new(vec![M]));
        let now_mg = Arc::clone(&mg_lists);
        let fetch: Fetch = Arc::new(move |acc: &Account| {
            let ids = if acc.label() == "mg" {
                now_mg.lock().unwrap().clone()
            } else {
                vec![H]
            };
            let shaped: Vec<_> = ids
                .iter()
                .map(|id| (*id, "", r#""state": "done""#, ""))
                .collect();
            Ok(listing(&shaped).into_bytes())
        });
        let lister = Lister::new(fetch, inline, Arc::clone(&reader));
        lister.poll(&a, Vec::new(), 1);
        let ids_of = |allowed: &[Account]| -> Vec<(String, String)> {
            let mut v: Vec<_> = lister
                .listed(allowed)
                .into_iter()
                .map(|(l, e)| (l, e.id))
                .collect();
            v.sort();
            v
        };
        let pair = |l: &str, id: &str| (l.to_string(), id.to_string());
        assert_eq!(ids_of(&a), [pair("hg", H), pair("mg", M)]);
        // Both allowed: M follows H in H's chain, and M's tokens count.
        let r = recap_for(&lister, &reader, &a, &[], &[], &[], "hg", H, 1, h.path()).unwrap();
        assert_eq!(ids(&r.chain), [(H, "", true), (M, "", false)]);
        assert_eq!(r.cost.chain_total_tokens, 7_005);
        // cc-mg taken off the allowlist; the lists are still that read's.
        let hg: Vec<Account> = a.iter().filter(|x| x.label() == "hg").cloned().collect();
        assert_eq!(hg.len(), 1);
        assert_eq!(ids_of(&hg), [pair("hg", H)]);
        assert!(transcript_for(&lister, &reader, &hg, "mg", M).is_err());
        assert!(recap_for(&lister, &reader, &hg, &[], &[], &[], "mg", M, 1, h.path()).is_err());
        let r = recap_for(&lister, &reader, &hg, &[], &[], &[], "hg", H, 1, h.path()).unwrap();
        assert_eq!(ids(&r.chain), [(H, "", true)]);
        assert_eq!((r.cost.chain_read, r.cost.chain_total_tokens), (1, 5));
        // M listed, then not: once a read drops it, it's refused.
        assert!(transcript_for(&lister, &reader, &a, "mg", M).is_ok());
        mg_lists.lock().unwrap().clear();
        lister.poll(&a, Vec::new(), 1 + agents::EVERY_SECS);
        assert_eq!(ids_of(&a), [pair("hg", H)]);
        assert!(transcript_for(&lister, &reader, &a, "mg", M).is_err());
        let late = 1 + agents::EVERY_SECS;
        assert!(recap_for(&lister, &reader, &a, &[], &[], &[], "mg", M, late, h.path()).is_err());
    }

    /// R2: a recap left open on another view (no list polls there) still follows the session:
    /// its own refresh polls the lister.
    #[test]
    fn a_recap_polls_the_list_so_its_state_stays_current() {
        let h = home();
        transcript(h.path(), B, &[prompt("2026-10-14T10:00:00Z", "go")]);
        let (r, a) = reader(h.path());
        let reader = Arc::new(r);
        let out = Arc::new(Mutex::new(listing(&[(
            B,
            "b",
            r#""state": "working""#,
            "",
        )])));
        let now_out = Arc::clone(&out);
        let fetch: Fetch =
            Arc::new(move |_: &Account| Ok(now_out.lock().unwrap().clone().into_bytes()));
        let lister = Lister::new(fetch, inline, Arc::clone(&reader));
        lister.poll(&a, Vec::new(), 1);
        let group = |now: u64| {
            recap_for(&lister, &reader, &a, &[], &[], &[], "mg", B, now, h.path())
                .unwrap()
                .who
                .group
        };
        assert_eq!(group(2), ListState::Working);
        *out.lock().unwrap() = listing(&[(B, "b", r#""state": "needs_input""#, "")]);
        assert_eq!(group(1 + agents::EVERY_SECS), ListState::NeedsInput);
    }

    /// A task file in a queue of its own whose log stamps session `id` with `account` (`cc-hg`).
    fn stamped(id: &str, account: &str) -> (crate::testkit::TestDir, Vec<Task>) {
        let q = crate::testkit::tempdir().unwrap();
        for f in queue::STATUS_FOLDERS {
            fs::create_dir(q.path().join(f)).unwrap();
        }
        fs::write(
            q.path().join("doing/0003-old.md"),
            format!("---\nid: 0003\ntitle: Old\nstage: building\nstatus: doing\nclaimed-by:\n---\n\n# Old\n\n## Log\n- 2026-10-14 08:00 gholi_1 (main · session {id} · {account}): check-in 10% · started\n"),
        )
        .unwrap();
        let tasks = queue::read_state(
            &[("muster".into(), q.path().to_path_buf())],
            &mut queue::Cache::default(),
        )
        .tasks;
        (q, tasks)
    }

    /// R1: a predecessor's transcript is read only when an allowed account vouches for it (its
    /// list has it, or a task stamps it with an allowed account). On the owner's Mac
    /// `~/.claude-hg/projects` links to `~/.claude/projects`, the company account's, so a
    /// transcript reachable through an allowed folder proves nothing.
    #[test]
    fn a_predecessor_no_allowed_account_vouches_for_is_never_read() {
        let h = home();
        let dir = fs::canonicalize(h.path())
            .unwrap()
            .join("playground/m/.claude/work/handoffs");
        fs::create_dir_all(&dir).unwrap();
        // P, a bare `claude` session, resumed Z's handoff and handed off to B (cc-mg).
        const P: &str = "aaaaaaa8";
        const Z: &str = "aaaaaaa7";
        fs::write(dir.join("hz.md"), format!("---\nsession: {Z}\n---\n")).unwrap();
        fs::write(
            dir.join("hp.md"),
            format!("---\nsession: {P}\nsuccessor: Gholi_1\n---\n"),
        )
        .unwrap();
        let full = |id: &str| format!("{id}-0000-4000-8000-000000000000");
        let at = "2026-10-14T10:00:00Z";
        // P's transcript sits in the company folder, reachable through .claude-hg's link.
        transcripts::tests::write(
            h.path(),
            ".claude",
            &full(P),
            &[
                prompt(at, &resume(&dir, "hz.md")),
                transcripts::tests::reply("p1", at, 5_000, 0, 0, 0),
            ],
            1_792_000_000,
        );
        transcripts::tests::write(
            h.path(),
            ".claude-mg",
            &full(B),
            &[
                prompt(at, &resume(&dir, "hp.md")),
                transcripts::tests::reply("b1", at, 7, 0, 0, 0),
            ],
            1_792_000_000,
        );
        let (r, a) = reader(h.path());
        let reader = Arc::new(r);
        let (fetch, _) = counting(listing(&[(B, "", r#""state": "done""#, "")]));
        let lister = Lister::new(fetch, inline, Arc::clone(&reader));
        lister.poll(&a, Vec::new(), 1);
        let got = |tasks: &[Task]| {
            recap_for(&lister, &reader, &a, &[], &[], tasks, "mg", B, 1, h.path()).unwrap()
        };
        // Unvouched: P is a link by id, not read (so Z behind it isn't found), nothing summed.
        let r = got(&[]);
        assert_eq!(ids(&r.chain), [(P, "", false), (B, "Gholi_1", true)]);
        assert_eq!((r.cost.chain_read, r.cost.chain_sessions), (1, 2));
        assert_eq!(r.cost.chain_total_tokens, 7);
        // Stamped by the company account: still not read.
        let (_q, tasks) = stamped(P, "cc-ih");
        assert_eq!(got(&tasks).cost.chain_read, 1);
        // Stamped by an allowed account: read, and Z behind it with it.
        let (_q, tasks) = stamped(P, "cc-hg");
        let r = got(&tasks);
        assert_eq!(
            ids(&r.chain),
            [(Z, "", false), (P, "", false), (B, "Gholi_1", true)]
        );
        assert_eq!((r.cost.chain_read, r.cost.chain_total_tokens), (2, 5_007));
    }

    /// AC8: the first read of 60 sessions' last messages, from fixture transcripts of about
    /// 1 MB each, in under 300 ms on a release build (`cargo test --release`); a debug build is
    /// held to ten times that.
    #[test]
    fn sixty_last_messages_read_in_under_300_ms() {
        let h = home();
        let filler: Vec<String> = (0..400)
            .flat_map(|i| {
                let id = format!("t{i}");
                [
                    said(&"words ".repeat(400)),
                    calls(&id, "Bash", "ls"),
                    answers(&id),
                ]
            })
            .collect();
        let mut ids = Vec::new();
        for i in 0..60u32 {
            let id = format!("{:08x}", 0xaaaa_0000u32 + i);
            let mut lines = filler.clone();
            lines.push(said(&format!("Last words of {i}.")));
            transcript(h.path(), &id, &lines);
            ids.push(id);
        }
        let shaped: Vec<(&str, &str, &str, &str)> = ids
            .iter()
            .map(|id| (id.as_str(), "s", r#""state": "done""#, ""))
            .collect();
        let out = listing(&shaped);
        let (r, a) = reader(h.path());
        let lists = vec![("mg".to_string(), parse_list(out.as_bytes()))];
        let t = Instant::now();
        let rows = rows(&lists, &r, &a, &[]);
        let took = t.elapsed();
        assert_eq!(rows.len(), 60);
        assert_eq!(rows[59].message, "Last words of 59.");
        let budget = if cfg!(debug_assertions) { 3000 } else { 300 };
        eprintln!("60 last messages, first read: {took:?}");
        assert!(took.as_millis() < budget, "{took:?}");
    }
}
