//! A local queue's history (task 0109). A local `.muster/` queue is kept out of the repo's git
//! (its `.gitignore` ignores everything but `config.toml`), so it gets a git of its own: a bare
//! repository at `<git common dir>/muster/history.git` whose work tree is `.muster/`. It sits in
//! git's own folder, so `git clean -fdx` in the checkout never reaches it, and it never has a
//! remote: nothing is pushed.
//!
//! Every write muster makes goes through `queue::with_lock`, which calls `begin` before the write
//! and `finish` after it, still under the queue's lock: the files whose size, time or inode
//! changed in between are committed, and only those, with the message `<key>: <the log line>`.
//! `mail/` (the mailboxes and the lock), `INDEX.md` and `tasks.json` (rebuilt by the tools) are
//! never committed. A write that can't be committed is remembered in the history's
//! `muster-pending` file, retried by the next write, and reported by `muster check`.
//!
//! git always runs as an argument array with `GIT_DIR`, `GIT_WORK_TREE` and `GIT_INDEX_FILE` set
//! here, every other `GIT_*` dropped, and no global or system config read: the owner's signing,
//! hooks or templates never reach these commits.

use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The first commit of a history, of everything the queue had.
pub const STARTS: &str = "muster: history starts";
/// The subject of a `muster history removed` commit starts with this.
const REMOVED: &str = "muster: removed ";
/// Writes that couldn't be committed, one JSON line each, in the history's git folder.
const PENDING: &str = "muster-pending";
/// The last commit this history made and its tree, so the next write needn't ask git.
const LAST_TREE: &str = "muster-last-tree";
/// The most pending writes remembered: the newest are kept.
const MAX_PENDING: usize = 500;
/// Left out of the history, at the queue's top: per-Mac mail and the lock, the lists muster
/// rebuilds, and a Claude Code session's own folder.
const LEFT_OUT: [&str; 4] = ["mail", "INDEX.md", "tasks.json", ".claude"];
/// The deepest folder walked (`attachments/0042/…` is 3).
const MAX_DEPTH: usize = 8;
/// The author and committer of every history commit.
const AUTHOR: (&str, &str) = ("muster", "muster@localhost");

/// Where the history of the `.muster` queue at `root` lives (or would): in the repo's common
/// git folder. None when `root` isn't a `.muster` folder in a main checkout (`.git` a folder).
pub fn git_dir(root: &Path) -> Option<PathBuf> {
    if !crate::layout::is_muster(root) {
        return None;
    }
    // Absolute, so git (run in the queue's folder) reads the same paths (`--queue .muster`).
    let root = std::path::absolute(root).ok()?;
    let dotgit = root.parent()?.join(".git");
    dotgit
        .is_dir()
        .then(|| dotgit.join("muster").join("history.git"))
}

/// The history of the queue at `root` when it keeps one: a local `.muster` queue in a main
/// checkout. A committed queue (`local = false`) is in the repo's own history already, and an old
/// `<project>/tasks` queue has its own git. A queue whose `config.toml` is gone (a `git clean`
/// before it was committed) keeps the history it has, so `muster restore` can put it back.
pub fn of(root: &Path) -> Option<PathBuf> {
    let h = git_dir(root)?;
    match crate::layout::read_config(root) {
        Ok(c) => c.local.then_some(h),
        Err(_) => (!root.join(crate::layout::CONFIG).exists() && started(&h)).then_some(h),
    }
}

/// The highest task id the history has held (`tasks/NNNN-…`): in its last commit, or in a file
/// `muster history removed` took out of it, so a new task never takes a removed one's id. 0
/// without a commit.
pub fn last_id(root: &Path) -> u32 {
    let Some(h) = of(root).filter(|h| started(h)) else {
        return 0;
    };
    let id = |p: &str| p.strip_prefix("tasks/")?.get(..4)?.parse::<u32>().ok();
    let head = git(
        &h,
        root,
        &["ls-tree", "-z", "--name-only", "HEAD", "tasks/"],
        None,
    )
    .map(|out| {
        out.split(|b| *b == 0)
            .filter_map(|p| id(std::str::from_utf8(p).ok()?))
            .max()
            .unwrap_or(0)
    })
    .unwrap_or(0);
    let gone = removals(&h, root)
        .unwrap_or_default()
        .iter()
        .filter_map(|(_, p)| id(p))
        .max()
        .unwrap_or(0);
    head.max(gone)
}

/// The files `muster history removed` took out of the history, newest removal first, each with
/// the commit that removed it.
fn removals(h: &Path, root: &Path) -> Result<Vec<(String, String)>, String> {
    let out = git(
        h,
        root,
        &[
            "log",
            &format!("--grep=^{REMOVED}"),
            "--diff-filter=D",
            "--name-only",
            "-z",
            "--format=%x01%H",
            "HEAD",
        ],
        None,
    )?;
    let mut v = Vec::new();
    let mut commit = String::new();
    // `\x01<hash>\0`, then `\n<path>\0<path>\0…` per commit.
    for item in out.split(|b| *b == 0) {
        let item = String::from_utf8_lossy(item);
        let item = item.strip_prefix('\n').unwrap_or(&item);
        if let Some(c) = item.strip_prefix('\u{1}') {
            commit = c.to_string();
        } else if !item.is_empty() && !commit.is_empty() {
            v.push((commit.clone(), item.to_string()));
        }
    }
    Ok(v)
}

/// git for the history `h` with the work tree `root`, run in `root`.
fn command(h: &Path, root: &Path) -> Command {
    let root = &std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf());
    let mut c = bare_command();
    c.current_dir(root)
        .env("GIT_DIR", h)
        .env("GIT_WORK_TREE", root)
        .env("GIT_INDEX_FILE", h.join("index"));
    c
}

/// git with no `GIT_*` from the caller, no global or system config, no hooks or signing, plain
/// output, paths taken literally, and muster as author and committer.
fn bare_command() -> Command {
    let mut c = Command::new("git");
    for (k, _) in std::env::vars_os() {
        if k.to_string_lossy().starts_with("GIT_") {
            c.env_remove(k);
        }
    }
    c.args([
        "-c",
        "core.hooksPath=/dev/null",
        "-c",
        "commit.gpgsign=false",
        "-c",
        "core.autocrlf=false",
        "-c",
        "core.fsmonitor=false",
        "-c",
        "core.quotePath=false",
    ])
    .env("GIT_CONFIG_GLOBAL", "/dev/null")
    .env("GIT_CONFIG_NOSYSTEM", "1")
    .env("GIT_LITERAL_PATHSPECS", "1")
    .env("GIT_TERMINAL_PROMPT", "0")
    .env("LC_ALL", "C")
    .env("GIT_AUTHOR_NAME", AUTHOR.0)
    .env("GIT_AUTHOR_EMAIL", AUTHOR.1)
    .env("GIT_COMMITTER_NAME", AUTHOR.0)
    .env("GIT_COMMITTER_EMAIL", AUTHOR.1)
    .stdin(Stdio::null());
    c
}

/// Run `c` with `input` on stdin: its exit code, stdout and stderr.
fn run(mut c: Command, input: Option<&[u8]>) -> Result<(i32, Vec<u8>, Vec<u8>), String> {
    if input.is_some() {
        c.stdin(Stdio::piped());
    }
    c.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = c.spawn().map_err(|e| format!("couldn't run git: {e}"))?;
    if let (Some(bytes), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin
            .write_all(bytes)
            .map_err(|e| format!("couldn't run git: {e}"))?;
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("couldn't run git: {e}"))?;
    Ok((out.status.code().unwrap_or(-1), out.stdout, out.stderr))
}

/// git's first line of complaint, short, with the history's folder named `history.git` and the
/// queue's `.muster`: the folders above them (the owner's name among them) stay out of what
/// `muster check` and Settings show. None when git said nothing.
fn git_error(stderr: &[u8], h: &Path, root: &Path) -> Option<String> {
    let text = String::from_utf8_lossy(stderr);
    let mut line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())?
        .to_string();
    for (dir, name) in [(h, "history.git"), (root, ".muster")] {
        let given = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
        // git may name the folder as given or with its links resolved (/tmp, /private/tmp).
        let real = fs::canonicalize(dir).unwrap_or_else(|_| given.clone());
        for d in [real, given] {
            let d = d.to_string_lossy();
            if !d.is_empty() {
                line = line.replace(d.as_ref(), name);
            }
        }
    }
    Some(line.chars().take(300).collect())
}

/// `git args` in the history, which must succeed (exit 0); else git's reason.
fn git(h: &Path, root: &Path, args: &[&str], input: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let mut c = command(h, root);
    c.args(args);
    let (code, out, err) = run(c, input)?;
    if code != 0 {
        return Err(git_error(&err, h, root)
            .unwrap_or_else(|| format!("git {} failed", args.first().unwrap_or(&""))));
    }
    Ok(out)
}

/// Paths as git reads them from stdin with `-z`.
fn nul_list<S: AsRef<str>>(paths: &[S]) -> Vec<u8> {
    let mut out = Vec::new();
    for p in paths {
        out.extend_from_slice(p.as_ref().as_bytes());
        out.push(0);
    }
    out
}

/// Has the history its first commit? (`main`, loose or packed.)
pub fn started(h: &Path) -> bool {
    h.join("HEAD").is_file()
        && (h.join("refs/heads/main").is_file()
            || fs::read_to_string(h.join("packed-refs"))
                .is_ok_and(|t| t.lines().any(|l| l.ends_with(" refs/heads/main"))))
}

/// Make the history `h` for `root` if it has no first commit yet: an empty bare repository, then
/// one commit of everything the queue holds. Returns whether it started it now. The caller holds
/// the queue's lock (or the queue is new and no one else knows it yet).
fn ensure(h: &Path, root: &Path, message: &str) -> Result<bool, String> {
    if started(h) {
        return Ok(false);
    }
    if !h.join("HEAD").is_file() {
        let parent = h.parent().ok_or("the history has no folder")?;
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        let mut c = bare_command();
        c.args(["init", "-q", "--bare", "-b", "main"]).arg(h);
        let (code, _, err) = run(c, None)?;
        if code != 0 {
            return Err(git_error(&err, h, root).unwrap_or_else(|| "git init failed".into()));
        }
    }
    let paths: Vec<String> = snapshot(root).into_keys().collect();
    commit_paths(h, root, &paths, message, true)?;
    Ok(true)
}

/// Start the history of a new local queue (`muster init --local`, `migrate --local`). Nothing for
/// a queue that keeps none; a history already there is left as it is.
pub fn start(root: &Path, message: &str) -> Result<(), String> {
    match of(root) {
        Some(h) => {
            ensure(&h, root, message)?;
            pack(&h, root);
            Ok(())
        }
        None => Ok(()),
    }
}

/// Stage `paths` as they are in the work tree (a missing one leaves the index), then commit when
/// that changed anything (or always, with `always`). Returns whether it committed.
fn commit_paths(
    h: &Path,
    root: &Path,
    paths: &[String],
    message: &str,
    always: bool,
) -> Result<bool, String> {
    if !paths.is_empty() {
        git(
            h,
            root,
            &["update-index", "--add", "--remove", "-z", "--stdin"],
            Some(&nul_list(paths)),
        )?;
    }
    // Plumbing, not `git commit`: that one walks the work tree for its status, hundreds of
    // milliseconds on a queue with attachments; these touch only the index and the objects.
    let text = |out: Vec<u8>| String::from_utf8_lossy(&out).trim().to_string();
    let tree = text(git(h, root, &["write-tree"], None)?);
    // The last commit and its tree (none before the first commit).
    let parent = head(h);
    let parent_tree = match &parent {
        None => None,
        Some(c) => Some(tree_of(h, root, c)?),
    };
    if !always && parent_tree.as_deref() == Some(tree.as_str()) {
        return Ok(false);
    }
    let mut args = vec!["commit-tree", tree.as_str(), "-m", message];
    if let Some(c) = &parent {
        args.extend(["-p", c.as_str()]);
    }
    let commit = text(git(h, root, &args, None)?);
    // Moved only from the commit it was made on ("" when there was none), so a commit made in
    // between is never lost.
    let old = parent.unwrap_or_default();
    git(
        h,
        root,
        &[
            "update-ref",
            "refs/heads/main",
            commit.as_str(),
            old.as_str(),
        ],
        None,
    )?;
    let _ = fs::write(h.join(LAST_TREE), format!("{commit} {tree}\n"));
    Ok(true)
}

/// The history's last commit, read from `main`'s ref (loose or packed) without running git.
fn head(h: &Path) -> Option<String> {
    let is_id = |s: &str| s.len() >= 40 && s.bytes().all(|b| b.is_ascii_hexdigit());
    if let Ok(t) = fs::read_to_string(h.join("refs/heads/main")) {
        let t = t.trim();
        return is_id(t).then(|| t.to_string());
    }
    fs::read_to_string(h.join("packed-refs"))
        .ok()?
        .lines()
        .find_map(|l| {
            let (id, name) = l.split_once(' ')?;
            (name == "refs/heads/main" && is_id(id)).then(|| id.to_string())
        })
}

/// The tree of `commit`: from the note the last commit left (`muster-last-tree`), else git.
fn tree_of(h: &Path, root: &Path, commit: &str) -> Result<String, String> {
    if let Ok(t) = fs::read_to_string(h.join(LAST_TREE)) {
        if let Some((c, tree)) = t.trim().split_once(' ') {
            if c == commit && !tree.is_empty() {
                return Ok(tree.to_string());
            }
        }
    }
    let out = git(h, root, &["rev-parse", &format!("{commit}^{{tree}}")], None)?;
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}

/// What a file looked like: enough to see that a write replaced or changed it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Stamp {
    len: u64,
    mtime: i128,
    ino: u64,
    mode: u32,
}

fn stamp_of(m: &fs::Metadata) -> Stamp {
    use std::os::unix::fs::MetadataExt;
    Stamp {
        len: m.len(),
        mtime: i128::from(m.mtime()) * 1_000_000_000 + i128::from(m.mtime_nsec()),
        ino: m.ino(),
        mode: m.mode(),
    }
}

/// Every file of the queue the history keeps, by its path from `root` (`/` between folders).
fn snapshot(root: &Path) -> BTreeMap<String, Stamp> {
    let mut out = BTreeMap::new();
    walk(root, "", 0, &mut out);
    out
}

fn walk(dir: &Path, rel: &str, depth: usize, out: &mut BTreeMap<String, Stamp>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        // A name git can't be given as UTF-8 text is passed over (muster writes none).
        let Some(name) = name.to_str() else {
            continue;
        };
        if (rel.is_empty() && LEFT_OUT.contains(&name)) || !kept_name(name) {
            continue;
        }
        let Ok(meta) = entry.path().symlink_metadata() else {
            continue;
        };
        let path = if rel.is_empty() {
            name.to_string()
        } else {
            format!("{rel}/{name}")
        };
        if meta.is_dir() {
            walk(&entry.path(), &path, depth + 1, out);
        } else {
            out.insert(path, stamp_of(&meta));
        }
    }
}

/// A file or folder name the history keeps anywhere: not git's own folder, Finder's litter, or a
/// write's temporary file (`write_atomic`'s `<name>.muster-<pid>.tmp`), and nothing with a line
/// break or a tab.
fn kept_name(name: &str) -> bool {
    name != ".git"
        && name != ".DS_Store"
        && !(name.contains(".muster-") && name.ends_with(".tmp"))
        && !name.contains(['\n', '\r', '\t'])
}

/// Is `path` (from the queue's top) one the history keeps?
fn kept_path(path: &str) -> bool {
    let mut parts = path.split('/');
    let top = parts.next().unwrap_or("");
    !path.is_empty()
        && !LEFT_OUT.contains(&top)
        && kept_name(top)
        && parts.all(|p| !p.is_empty() && p != "." && p != ".." && kept_name(p))
        && top != "."
        && top != ".."
}

/// The queue as it was before a write: `with_lock` takes it before running the write.
pub struct Before {
    h: PathBuf,
    files: BTreeMap<String, Stamp>,
}

/// Before a write to the queue at `root` (under its lock): None when the queue keeps no history.
/// A local queue without one gets it now, of everything it holds ("history starts").
pub fn begin(root: &Path) -> Option<Before> {
    let h = of(root)?;
    // A history that can't start now leaves the write to `finish`, which remembers it.
    let _ = ensure(&h, root, STARTS);
    Some(Before {
        h,
        files: snapshot(root),
    })
}

/// After the write (still under the lock): commit the files it wrote, with any writes before it
/// that couldn't be committed. One that can't be committed is remembered for `muster check`.
/// Returns the history when it committed, for `pack` once the lock is let go.
pub fn finish(root: &Path, before: Before) -> Option<PathBuf> {
    let after = snapshot(root);
    let mut wrote: BTreeSet<String> = BTreeSet::new();
    for (p, s) in &after {
        if before.files.get(p) != Some(s) {
            wrote.insert(p.clone());
        }
    }
    for p in before.files.keys() {
        if !after.contains_key(p) {
            wrote.insert(p.clone());
        }
    }
    let pending = read_pending(&before.h, root);
    if wrote.is_empty() && pending.is_empty() {
        return None;
    }
    let mut paths = wrote.clone();
    paths.extend(retry(root, &pending));
    let paths: Vec<String> = paths.into_iter().collect();
    let wrote: Vec<String> = wrote.into_iter().collect();
    let message = message(root, if wrote.is_empty() { &paths } else { &wrote });
    let done = if started(&before.h) {
        commit_paths(&before.h, root, &paths, &message, false)
    } else {
        ensure(&before.h, root, STARTS).map(|_| true)
    };
    match done {
        Ok(committed) => {
            clear_pending(&before.h, root);
            return committed.then_some(before.h);
        }
        // Pending writes stay remembered as they are; this one joins them.
        Err(e) if !wrote.is_empty() => add_pending(root, &before.h, &wrote, &e),
        Err(_) => {}
    }
    None
}

/// After a commit, outside the queue's lock: `git gc --auto`, which packs the history once it
/// has enough loose objects (gc.auto, 6,700 by default, about 1,700 writes) and does nothing
/// otherwise. git packs in the background, so no write waits for it; a pack that fails leaves
/// the loose objects as they are, whole.
pub fn pack(h: &Path, root: &Path) {
    // gc --auto estimates the loose objects from objects/17 alone and packs only when it holds
    // more than gc.auto / 256 of them, rounded up: never with fewer than 2. Below that git would
    // do nothing, so most writes skip running it.
    let sample = fs::read_dir(h.join("objects").join("17")).map_or(0, |d| d.take(2).count());
    if sample < 2 {
        return;
    }
    let mut c = command(h, root);
    c.args(["gc", "--auto", "--quiet"]);
    let _ = run(c, None);
}

/// The commit message for a write of `paths`: `<key>: <the task's last log line>` for each task
/// file, the attachment or backlog note by name, else the path; the first is the subject and the
/// rest follow it. Control characters are dropped and each line is cut at 500 characters.
pub fn message(root: &Path, paths: &[String]) -> String {
    let prefix = crate::layout::config_prefix(root).unwrap_or_default();
    let key = |id: &str| crate::key::key(&prefix, id);
    // (is it a task file's line, the line): the task's own lines lead, so a write of a task and
    // its review, attachment or lessons is named by the task's key and log line.
    let mut lines: Vec<(bool, String)> = Vec::new();
    for p in paths {
        let task = p.strip_prefix("tasks/").is_some_and(|n| !n.contains('/'));
        let line = if let Some(name) = p.strip_prefix("tasks/").filter(|n| !n.contains('/')) {
            let id = name.split(['-', '.']).next().unwrap_or(name);
            match last_log_line(&root.join(p)) {
                Some(l) => format!("{}: {l}", key(id)),
                None if !root.join(p).exists() => format!("{}: {name} removed", key(id)),
                None => format!("{}: {name}", key(id)),
            }
        } else if let Some(rest) = p.strip_prefix("attachments/") {
            let (id, file) = rest.split_once('/').unwrap_or((rest, ""));
            let gone = if root.join(p).exists() {
                ""
            } else {
                " removed"
            };
            format!("{}: attachment {file}{gone}", key(id))
        } else if let Some(name) = p.strip_prefix("backlog/") {
            let gone = if root.join(p).exists() { "" } else { " moved" };
            format!("backlog: {name}{gone}")
        } else {
            format!("muster: {p}")
        };
        let line: String = line.chars().filter(|c| !c.is_control()).take(500).collect();
        if !lines.iter().any(|(_, l)| *l == line) {
            lines.push((task, line));
        }
    }
    lines.sort_by_key(|(task, _)| !task);
    let lines: Vec<String> = lines.into_iter().map(|(_, l)| l).collect();
    match lines.split_first() {
        None => "muster: a write".into(),
        Some((first, [])) => first.clone(),
        Some((first, rest)) => format!("{first}\n\n{}", rest.join("\n")),
    }
}

/// The last line of a task file's Log, without its `- `.
fn last_log_line(path: &Path) -> Option<String> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.len() > 4 << 20 {
        return None;
    }
    let text = fs::read_to_string(path).ok()?;
    let (start, end) = crate::queue::log_span(&text)?;
    text[start..end]
        .lines()
        .rev()
        .find_map(|l| l.strip_prefix("- "))
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
}

/// A write that couldn't be committed.
#[derive(serde::Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
struct Pending {
    at: u64,
    error: String,
    paths: Vec<String>,
    /// The paths the write itself removed (a note moved to `dropped/`).
    #[serde(default)]
    gone: Vec<String>,
}

/// The paths of pending writes to commit now: each file that's there, and each the write itself
/// removed. A file that was written but is gone now (a `git clean` since) is left out, so it's
/// never committed as removed: `muster restore` brings back its last committed copy.
fn retry(root: &Path, pending: &[Pending]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for w in pending {
        for p in w.paths.iter().filter(|p| kept_path(p)) {
            let there = fs::symlink_metadata(root.join(p)).is_ok();
            if there || w.gone.contains(p) {
                out.insert(p.clone());
            }
        }
    }
    out
}

/// Where writes that couldn't be committed are remembered: the history's folder, and the queue's
/// own `mail/` when that can't be written (a sandbox that lets a session write the queue but not
/// the repo's git folder), which the write itself just reached.
fn pending_files(h: &Path, root: &Path) -> [PathBuf; 2] {
    [h.join(PENDING), root.join("mail").join("history-pending")]
}

fn read_pending(h: &Path, root: &Path) -> Vec<Pending> {
    let mut out: Vec<Pending> = Vec::new();
    for f in pending_files(h, root) {
        let Ok(text) = fs::read_to_string(f) else {
            continue;
        };
        for p in text.lines().filter_map(|l| serde_json::from_str(l).ok()) {
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

fn add_pending(root: &Path, h: &Path, paths: &[String], error: &str) {
    let mut all = read_pending(h, root);
    all.push(Pending {
        at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        error: error.to_string(),
        paths: paths.to_vec(),
        gone: paths
            .iter()
            .filter(|p| fs::symlink_metadata(root.join(p)).is_err())
            .cloned()
            .collect(),
    });
    let skip = all.len().saturating_sub(MAX_PENDING);
    let text: String = all[skip..]
        .iter()
        .filter_map(|p| serde_json::to_string(p).ok())
        .map(|l| l + "\n")
        .collect();
    // The whole list goes to the first place that takes it; the other copy, if any, is read
    // with it and its duplicates dropped.
    let [in_history, in_mail] = pending_files(h, root);
    let kept =
        fs::create_dir_all(h).is_ok() && crate::queue::write_atomic(&in_history, &text).is_ok();
    if !kept {
        let _ = crate::queue::write_atomic(&in_mail, &text);
    }
}

fn clear_pending(h: &Path, root: &Path) {
    for f in pending_files(h, root) {
        let _ = fs::remove_file(f);
    }
}

/// A local queue's history as `muster check`, `muster history` and Settings › Projects show it.
#[derive(Serialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// The queue keeps a history (a local `.muster` queue in a main checkout).
    pub kept: bool,
    /// It has its first commit.
    pub started: bool,
    /// When the last commit was made (seconds since 1970).
    pub last_commit: Option<i64>,
    /// Writes that couldn't be committed, and git's words for the last of them.
    pub pending: usize,
    pub pending_error: String,
    /// Files in the last commit that are gone from the queue.
    pub missing: Vec<String>,
    /// Files changed or added since the last commit, outside muster.
    pub changed: Vec<String>,
}

/// The work tree against the last commit: (missing, changed), each sorted, without what writes
/// not committed yet account for.
fn tree_state(h: &Path, root: &Path) -> Result<(Vec<String>, Vec<String>), String> {
    let out = git(
        h,
        root,
        &[
            "--no-optional-locks",
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignored=traditional",
            "--no-renames",
        ],
        None,
    )?;
    let (mut missing, mut changed) = (Vec::new(), Vec::new());
    for entry in out.split(|b| *b == 0) {
        if entry.len() < 4 {
            continue;
        }
        let code = &entry[..2];
        let path = String::from_utf8_lossy(&entry[3..]).to_string();
        if !kept_path(&path) {
            continue;
        }
        match code {
            b"??" | b"!!" => changed.push(path),
            [_, b'D'] => missing.push(path),
            [_, b' '] => {}
            _ => changed.push(path),
        }
    }
    // A write that couldn't be committed is reported as that, not as files outside muster: its
    // files aren't "changed", and a file it removed itself (a note moved to dropped/) isn't
    // "missing", so restore --all never undoes it.
    let pending = read_pending(h, root);
    let gone: BTreeSet<&String> = pending.iter().flat_map(|w| &w.gone).collect();
    let wrote: BTreeSet<&String> = pending.iter().flat_map(|w| &w.paths).collect();
    missing.retain(|p| !gone.contains(p));
    changed.retain(|p| !wrote.contains(p));
    missing.sort();
    changed.sort();
    Ok((missing, changed))
}

/// The history of the queue at `root` now. Reads only: no lock, no write.
pub fn status(root: &Path) -> Status {
    let Some(h) = of(root) else {
        return Status::default();
    };
    let pending = read_pending(&h, root);
    let mut s = Status {
        kept: true,
        started: started(&h),
        pending: pending.len(),
        pending_error: pending.last().map(|p| p.error.clone()).unwrap_or_default(),
        ..Status::default()
    };
    if !s.started {
        return s;
    }
    s.last_commit = git(&h, root, &["log", "-1", "--format=%ct"], None)
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o).trim().parse().ok());
    match tree_state(&h, root) {
        Ok((missing, changed)) => {
            s.missing = missing;
            s.changed = changed;
        }
        Err(e) => {
            s.pending_error = e;
        }
    }
    s
}

/// A few paths for a line: the first three, then how many more.
fn some(paths: &[String]) -> String {
    let mut out = paths.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
    if paths.len() > 3 {
        out.push_str(&format!(" and {} more", paths.len() - 3));
    }
    out
}

fn files(n: usize) -> String {
    format!("{n} file{}", if n == 1 { "" } else { "s" })
}

/// What `muster check` says about the history of the queue `queue` at `root`: files missing
/// since the last commit and writes not committed fail it, each with the command that fixes it;
/// files changed outside muster are a warning. A history not started yet isn't a problem: the
/// queue's next write starts it (a fresh clone of a local queue has only its settings).
pub fn problems(queue: &str, root: &Path) -> Vec<String> {
    let s = status(root);
    let mut out = Vec::new();
    if !s.kept {
        return out;
    }
    if s.pending > 0 {
        out.push(format!(
            "{queue}: history: {} not committed ({}): muster history commit",
            if s.pending == 1 {
                "1 write".to_string()
            } else {
                format!("{} writes", s.pending)
            },
            s.pending_error
        ));
    }
    if !s.missing.is_empty() {
        out.push(format!(
            "{queue}: history: {} missing since the last commit ({}): muster restore --all, or when deleted on purpose: muster history removed <path>…",
            files(s.missing.len()),
            some(&s.missing)
        ));
    }
    if !s.changed.is_empty() {
        out.push(format!(
            "{queue}: warning: history: {} changed outside muster since the last commit ({}): muster history commit",
            files(s.changed.len()),
            some(&s.changed)
        ));
    }
    out
}

/// The queue's history or why there's none.
fn need(root: &Path) -> Result<PathBuf, String> {
    of(root).ok_or_else(|| {
        format!(
            "{} keeps no history: only a local .muster queue (local = true) in a repo's main checkout has one",
            root.display()
        )
    })
}

/// `muster restore`: put back the last committed copy of `paths` (files or folders, from the
/// queue's top: each file under them that's missing or changed), or with `all` of every file
/// missing since the last commit, under the queue's lock. A named path the history removed on
/// purpose (`muster history removed`) comes back from the commit before its removal, and is in
/// the history again. The restore is logged as a commit of its own. Returns the files put back.
pub fn restore(root: &Path, paths: &[String], all: bool, who: &str) -> Result<Vec<String>, String> {
    let h = need(root)?;
    if !started(&h) {
        return Err("the queue's history has no commit yet: nothing to restore".into());
    }
    let back = crate::queue::hold_lock(root, || {
        let (missing, changed) = tree_state(&h, root)?;
        // The files to put back, by the commit each comes from ("HEAD", or the one before a
        // removal).
        let mut from: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        if all {
            // Only what's gone: a file changed since (a hand edit, Drover's answer) is put back
            // only when it's named.
            from.entry("HEAD".into()).or_default().extend(missing);
        } else {
            let differs: BTreeSet<String> = missing.into_iter().chain(changed).collect();
            let in_head = tracked(&h, root, "HEAD")?;
            let removals = removals(&h, root)?;
            for p in paths {
                let rel = relative(root, p)?;
                let under =
                    |t: &String| rel.is_empty() || *t == rel || t.starts_with(&format!("{rel}/"));
                let now: Vec<&String> = in_head.iter().filter(|t| under(t)).collect();
                // Files under it `muster history removed` took out and still gone, each from
                // before its last removal; one a write deleted after that stays gone.
                let mut gone: BTreeMap<&str, &str> = BTreeMap::new();
                for (c, t) in &removals {
                    if under(t)
                        && !in_head.contains(t)
                        && fs::symlink_metadata(root.join(t)).is_err()
                    {
                        gone.entry(t.as_str()).or_insert(c.as_str());
                    }
                }
                for (t, c) in gone.clone() {
                    if removed_in(&h, root, t)?.as_deref() != Some(c) {
                        gone.remove(t);
                    }
                }
                for (t, c) in &gone {
                    from.entry(format!("{c}^"))
                        .or_default()
                        .insert(t.to_string());
                }
                if !now.is_empty() || !gone.is_empty() {
                    let set = from.entry("HEAD".into()).or_default();
                    set.extend(now.into_iter().filter(|t| differs.contains(*t)).cloned());
                    continue;
                }
                let Some(c) = removed_in(&h, root, &rel)? else {
                    return Err(format!("{p} isn't in the queue's history"));
                };
                let before = format!("{c}^");
                let set = from.entry(before.clone()).or_default();
                set.extend(
                    tracked(&h, root, &before)?
                        .into_iter()
                        .filter(|t| under(t) && fs::symlink_metadata(root.join(t)).is_err()),
                );
            }
        }
        from.retain(|_, set| !set.is_empty());
        let targets: Vec<String> = from
            .values()
            .flatten()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if targets.is_empty() {
            return Ok(targets);
        }
        taken_ids(root, &targets)?;
        for (commit, set) in &from {
            let set: Vec<&String> = set.iter().collect();
            git(
                &h,
                root,
                &[
                    "checkout",
                    commit.as_str(),
                    "--pathspec-from-file=-",
                    "--pathspec-file-nul",
                ],
                Some(&nul_list(&set)),
            )?;
        }
        let who: String = who.chars().filter(|c| !c.is_control()).take(200).collect();
        let message = format!(
            "muster: restored {} ({who})\n\n{}",
            files(targets.len()),
            targets.join("\n")
        );
        // The checkout staged what it put back: a file from before a removal is in this commit.
        commit_paths(&h, root, &[], &message, true)?;
        let _ = crate::index::refresh_locked(root);
        Ok(targets)
    })?;
    pack(&h, root);
    Ok(back)
}

/// Refuses to bring back a task file whose id another task file holds now (one made since its
/// removal, or by hand), or that another of `targets` brings back: two tasks with one key.
fn taken_ids(root: &Path, targets: &[String]) -> Result<(), String> {
    let id_of = |name: &str| -> Option<u32> {
        crate::queue::is_task_name(name)
            .then(|| name.get(..4)?.parse().ok())
            .flatten()
    };
    // The task files there now: each holds its name's id and the one written in it.
    let mut held: BTreeMap<u32, String> = BTreeMap::new();
    for (_, p) in crate::queue::task_files(root) {
        let Ok(rel) = p.strip_prefix(root) else {
            continue;
        };
        let rel = rel.to_string_lossy().to_string();
        let named = p.file_name().and_then(|n| id_of(&n.to_string_lossy()));
        let written = fs::read_to_string(&p)
            .ok()
            .as_deref()
            .and_then(crate::queue::front_matter)
            .and_then(|(fm, _)| fm.get("id").and_then(|i| i.parse().ok()));
        for id in named.into_iter().chain(written) {
            held.entry(id).or_insert_with(|| rel.clone());
        }
    }
    let mut refused = Vec::new();
    for t in targets {
        let Some(id) = t.strip_prefix("tasks/").and_then(id_of) else {
            continue;
        };
        if root.join(t).exists() {
            continue; // put back over itself
        }
        match held.get(&id) {
            Some(other) if other != t => {
                let id = format!("{id:04}");
                let prefix = crate::key::stored_prefix(root).unwrap_or_default();
                let key = crate::key::key(&prefix, &id);
                refused.push(format!("{t}: {key} (id {id}) is {other} now"));
            }
            _ => {
                held.insert(id, t.clone());
            }
        }
    }
    if refused.is_empty() {
        return Ok(());
    }
    Err(format!(
        "can't bring back {}; two tasks can't share one id. Nothing was restored: name the other paths on their own",
        refused.join("; ")
    ))
}

/// The last commit that removed `rel` (a file, or files under a folder), if one did.
fn removed_in(h: &Path, root: &Path, rel: &str) -> Result<Option<String>, String> {
    if rel.is_empty() {
        return Ok(None);
    }
    let out = git(
        h,
        root,
        &[
            "log",
            "-1",
            "--format=%H",
            "--diff-filter=D",
            "HEAD",
            "--",
            rel,
        ],
        None,
    )?;
    let c = String::from_utf8_lossy(&out).trim().to_string();
    Ok((!c.is_empty()).then_some(c))
}

/// `muster history removed`: record files deleted on purpose as removed, so `muster check` stops
/// asking to restore them and `restore --all` leaves them gone: `paths` (files or folders, from
/// the queue's top), or with `all` every file missing since the last commit. Only a file that's
/// gone and in the last commit can be; older commits keep it, and `muster restore <path>` brings
/// it back. Returns the files recorded.
pub fn removed(root: &Path, paths: &[String], all: bool, who: &str) -> Result<Vec<String>, String> {
    let h = need(root)?;
    if !started(&h) {
        return Err("the queue's history has no commit yet: nothing to record".into());
    }
    if all == !paths.is_empty() {
        return Err("name the files or folders deleted on purpose, or --all".into());
    }
    let done = crate::queue::hold_lock(root, || {
        let (missing, _) = tree_state(&h, root)?;
        let mut targets: BTreeSet<String> = BTreeSet::new();
        if all {
            targets.extend(missing);
        } else {
            for p in paths {
                let rel = relative(root, p)?;
                let under: Vec<&String> = missing
                    .iter()
                    .filter(|t| rel.is_empty() || **t == rel || t.starts_with(&format!("{rel}/")))
                    .collect();
                if under.is_empty() {
                    return Err(format!("{p} isn't a file missing since the last commit"));
                }
                targets.extend(under.into_iter().cloned());
            }
        }
        let targets: Vec<String> = targets.into_iter().collect();
        if targets.is_empty() {
            return Ok(targets);
        }
        let who: String = who.chars().filter(|c| !c.is_control()).take(200).collect();
        let message = format!(
            "{REMOVED}{} ({who})\n\n{}",
            files(targets.len()),
            targets.join("\n")
        );
        commit_paths(&h, root, &targets, &message, false)?;
        // A task file among them is gone from the master list too.
        let _ = crate::index::refresh_locked(root);
        Ok(targets)
    })?;
    pack(&h, root);
    Ok(done)
}

/// Every file in `commit` (`HEAD`, the last).
fn tracked(h: &Path, root: &Path, commit: &str) -> Result<BTreeSet<String>, String> {
    let out = git(
        h,
        root,
        &["ls-tree", "-r", "-z", "--name-only", commit],
        None,
    )?;
    Ok(out
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).to_string())
        .collect())
}

/// A path given to `restore` as the history names it: from the queue's top, or absolute inside
/// it; `.` or the queue itself is all of it ("").
fn relative(root: &Path, given: &str) -> Result<String, String> {
    let p = Path::new(given);
    let rel = if p.is_absolute() {
        let canon = |x: &Path| fs::canonicalize(x).unwrap_or_else(|_| x.to_path_buf());
        // The file may be gone: its folder is what's canonicalised.
        let base = canon(root);
        let full = match (p.parent(), p.file_name()) {
            (Some(dir), Some(name)) if !dir.as_os_str().is_empty() => canon(dir).join(name),
            _ => canon(p),
        };
        full.strip_prefix(&base)
            .map(Path::to_path_buf)
            .map_err(|_| format!("{given} isn't in the queue {}", root.display()))?
    } else {
        p.to_path_buf()
    };
    let text = rel.to_string_lossy().replace('\\', "/");
    let text = text.trim_start_matches("./").trim_end_matches('/');
    if text.is_empty() || text == "." {
        return Ok(String::new());
    }
    if !kept_path(text) {
        return Err(format!("{given} isn't a path the queue's history keeps"));
    }
    Ok(text.to_string())
}

/// `muster history commit`: commit what the history doesn't have yet (files changed or added
/// outside muster, and writes that couldn't be committed), starting it when it has no commit.
/// A file that's gone is never committed as gone here: `muster restore` brings it back, and
/// `removed` records one deleted on purpose. Returns the files committed.
pub fn commit_outside(root: &Path) -> Result<Vec<String>, String> {
    let h = need(root)?;
    let done = crate::queue::hold_lock(root, || {
        if ensure(&h, root, STARTS)? {
            clear_pending(&h, root);
            return Ok(vec!["(everything: the history starts)".to_string()]);
        }
        let (_, changed) = tree_state(&h, root)?;
        let mut paths: BTreeSet<String> = changed.into_iter().collect();
        paths.extend(retry(root, &read_pending(&h, root)));
        let paths: Vec<String> = paths.into_iter().collect();
        if paths.is_empty() {
            clear_pending(&h, root);
            return Ok(paths);
        }
        let message = format!(
            "muster: {} changed outside muster or not committed yet\n\n{}",
            files(paths.len()),
            paths.join("\n")
        );
        let committed = commit_paths(&h, root, &paths, &message, false)?;
        clear_pending(&h, root);
        Ok(if committed { paths } else { Vec::new() })
    })?;
    pack(&h, root);
    Ok(done)
}

/// The history's last `n` commits, newest first: (when, local `YYYY-MM-DD HH:MM`; subject).
pub fn log(root: &Path, n: usize) -> Result<Vec<(String, String)>, String> {
    let h = need(root)?;
    if !started(&h) {
        return Ok(Vec::new());
    }
    let out = git(
        &h,
        root,
        &[
            "log",
            &format!("-n{n}"),
            "--date=format-local:%Y-%m-%d %H:%M",
            "--format=%cd%x1f%s",
        ],
        None,
    )?;
    Ok(String::from_utf8_lossy(&out)
        .lines()
        .filter_map(|l| {
            let (at, what) = l.split_once('\u{1f}')?;
            Some((at.to_string(), what.to_string()))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::resolve::{git as repo_git, repo};
    use crate::ops::{self, NewTask};

    const T: &str = "2026-10-14 09:00";

    /// A repo `<base>/proj/main` with a local queue (prefix `pq`), its .muster/ settings committed
    /// in the repo as `muster init` leaves them for the owner to commit.
    fn local_queue(base: &Path) -> (PathBuf, PathBuf) {
        let main = repo(base, "proj");
        let q = crate::projects::init(&main, true, Some("pq"), &[])
            .unwrap()
            .0;
        repo_git(&main, &["add", ".muster"]);
        repo_git(&main, &["commit", "-q", "-m", "muster"]);
        (main, q)
    }

    /// `git args` in the queue's history, as text.
    fn hist(q: &Path, args: &[&str]) -> String {
        let h = of(q).expect("a local queue keeps a history");
        String::from_utf8(git(&h, q, args, None).unwrap()).unwrap()
    }

    fn commits(q: &Path) -> usize {
        hist(q, &["rev-list", "--count", "HEAD"])
            .trim()
            .parse()
            .unwrap()
    }

    /// The files the last commit changed, sorted.
    fn last_files(q: &Path) -> Vec<String> {
        let mut v: Vec<String> = hist(q, &["show", "--name-only", "--format=", "HEAD"])
            .lines()
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect();
        v.sort();
        v
    }

    fn last_subject(q: &Path) -> String {
        hist(q, &["log", "-1", "--format=%s"]).trim().to_string()
    }

    fn task(q: &Path, title: &str) -> (String, String) {
        let new = NewTask {
            title: title.into(),
            ..Default::default()
        };
        let c = ops::create_task(q, &new, "owner", T).unwrap();
        (c.id.clone(), c.path.clone())
    }

    /// Every file the history keeps, with its bytes.
    fn files_of(q: &Path) -> BTreeMap<String, Vec<u8>> {
        snapshot(q)
            .into_keys()
            .map(|p| {
                let bytes = fs::read(q.join(&p)).unwrap();
                (p, bytes)
            })
            .collect()
    }

    #[test]
    fn init_local_starts_the_history_in_the_common_git_folder_and_a_committed_queue_has_none() {
        let base = tempfile::tempdir().unwrap();
        let (main, q) = local_queue(base.path());
        let h = q.parent().unwrap().join(".git/muster/history.git");
        assert!(main.join(".git/muster/history.git").join("HEAD").is_file());
        assert_eq!(of(&q), Some(h.clone()));
        assert!(started(&h));
        assert_eq!(commits(&q), 1);
        assert_eq!(last_subject(&q), STARTS);
        let tree = hist(&q, &["ls-tree", "-r", "--name-only", "HEAD"]);
        for f in [".gitignore", "README.md", "config.toml"] {
            assert!(tree.lines().any(|l| l == f), "{f} in {tree}");
        }
        for f in ["INDEX.md", "tasks.json", "mail"] {
            assert!(!tree.contains(f), "{f} kept out of {tree}");
        }
        let s = status(&q);
        assert!(s.kept && s.started && s.last_commit.is_some());
        assert_eq!((s.pending, s.missing.len(), s.changed.len()), (0, 0, 0));
        assert!(problems("q", &q).is_empty());

        let other = repo(base.path(), "committed");
        let c = crate::projects::init(&other, false, Some("cq"), &[])
            .unwrap()
            .0;
        task(&c, "In the repo's own history");
        assert_eq!(of(&c), None);
        assert!(!other.join(".git/muster/history.git").exists());
        assert_eq!(status(&c), Status::default());
        assert!(problems("c", &c).is_empty());
    }

    #[test]
    fn a_queue_outside_a_main_checkout_or_an_old_queue_keeps_none() {
        let base = tempfile::tempdir().unwrap();
        // A .muster outside any repo.
        let q = base.path().join(".muster");
        fs::create_dir(&q).unwrap();
        crate::projects::write_muster(&q, true, "", "x", "xq").unwrap();
        assert_eq!(git_dir(&q), None);
        assert!(begin(&q).is_none());
        // An old <project>/tasks queue beside a repo.
        let old = base.path().join("tasks");
        fs::create_dir(&old).unwrap();
        fs::create_dir(old.join(".git")).unwrap();
        assert_eq!(git_dir(&old), None);
        assert!(restore(&q, &[], true, "owner")
            .unwrap_err()
            .contains("keeps no history"));
    }

    #[test]
    fn an_existing_local_queue_gets_its_history_on_its_first_write() {
        let base = tempfile::tempdir().unwrap();
        let (main, q) = local_queue(base.path());
        let (id, path) = task(&q, "Made before the history");
        fs::remove_dir_all(main.join(".git/muster")).unwrap();
        assert!(!status(&q).started);
        assert!(problems("q", &q).is_empty());
        ops::claim_task(&q, &id, "wt-a", T).unwrap();
        assert_eq!(commits(&q), 2);
        let first = hist(&q, &["log", "--reverse", "--format=%s"]);
        assert_eq!(first.lines().next(), Some(STARTS));
        // The first commit has the task as it was; the second, the claim of it.
        let then = hist(&q, &["show", &format!("HEAD~1:{path}")]);
        assert!(!then.contains("claimed-by: wt-a"));
        assert_eq!(last_files(&q), vec![path.clone()]);
        assert!(last_subject(&q).starts_with("pq-1: "));
        assert!(problems("q", &q).is_empty());
    }

    #[test]
    fn every_write_is_one_commit_of_only_its_paths_named_by_its_key_and_log_line() {
        let base = tempfile::tempdir().unwrap();
        let (_, q) = local_queue(base.path());
        let mut n = commits(&q);
        let (id, path) = task(&q, "First");
        n += 1;
        assert_eq!(commits(&q), n);
        assert_eq!(last_files(&q), vec![path.clone()]);
        let text = fs::read_to_string(q.join(&path)).unwrap();
        let line = last_log_line(&q.join(&path)).unwrap();
        assert!(text.contains(&format!("- {line}\n")));
        assert_eq!(last_subject(&q), format!("pq-1: {line}"));
        let h = of(&q).unwrap();
        assert_eq!(
            String::from_utf8(git(&h, &q, &["log", "-1", "--format=%an <%ae>"], None).unwrap())
                .unwrap()
                .trim(),
            "muster <muster@localhost>"
        );

        // A question writes the task and the asker's mailbox: only the task is committed.
        crate::queue::ask(&q, &id, "Which port?", "wt-a", "aaaa1111", T).unwrap();
        n += 1;
        assert_eq!(commits(&q), n);
        assert_eq!(last_files(&q), vec![path.clone()]);
        assert!(
            last_subject(&q).contains("asked Q1"),
            "{}",
            last_subject(&q)
        );

        // A backlog note.
        let note = crate::backlog::add(&q, "A note", "", "wt-a", T).unwrap();
        n += 1;
        assert_eq!(commits(&q), n);
        let files = last_files(&q);
        assert_eq!(files.len(), 1);
        assert!(files[0].starts_with("backlog/"), "{files:?}");
        assert!(last_subject(&q).starts_with("backlog: "), "{note}");

        // An attachment.
        ops::attach(&q, &id, "shot.png", b"\x89PNG bytes", "wt-a", T).unwrap();
        n += 1;
        assert_eq!(commits(&q), n);
        let files = last_files(&q);
        assert!(
            files.contains(&format!("attachments/{id}/shot.png")),
            "{files:?}"
        );

        // A refused write makes no commit; nor does INDEX.md, tasks.json or mail, ever.
        fs::write(q.join("tasks.json"), "{}").unwrap();
        assert!(ops::claim_task(&q, "0099", "wt-a", T).is_err());
        assert_eq!(commits(&q), n);
        let all = hist(&q, &["log", "--name-only", "--format="]);
        for f in ["INDEX.md", "tasks.json", "mail/"] {
            assert!(!all.contains(f), "{f} in {all}");
        }
        assert!(q.join("mail").is_dir() && q.join("INDEX.md").is_file());
        assert!(problems("q", &q).is_empty(), "{:?}", problems("q", &q));
    }

    #[test]
    fn a_hand_edit_is_left_out_of_another_write_and_history_commit_takes_it_in() {
        let base = tempfile::tempdir().unwrap();
        let (_, q) = local_queue(base.path());
        let (_, a) = task(&q, "A");
        let (b_id, b) = task(&q, "B");
        // A hand edit of A, then a write to B: B's commit holds B only.
        let edited = fs::read_to_string(q.join(&a)).unwrap() + "\nhand edit\n";
        fs::write(q.join(&a), &edited).unwrap();
        ops::claim_task(&q, &b_id, "wt-b", T).unwrap();
        assert_eq!(last_files(&q), vec![b.clone()]);
        let s = status(&q);
        assert_eq!(s.changed, vec![a.clone()]);
        let p = problems("q", &q);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(crate::queue::is_warning(&p[0]), "{p:?}");
        assert!(
            p[0].contains("changed outside muster") && p[0].ends_with("muster history commit"),
            "{p:?}"
        );
        assert_eq!(commit_outside(&q).unwrap(), vec![a.clone()]);
        assert_eq!(last_files(&q), vec![a.clone()]);
        assert!(problems("q", &q).is_empty());
        assert!(commit_outside(&q).unwrap().is_empty());

        // The message of a write of two task files names both, the first as the subject.
        let msg = message(&q, &[a.clone(), b.clone()]);
        let mut lines = msg.lines();
        assert!(lines.next().unwrap().starts_with("pq-1: "));
        assert_eq!(lines.next(), Some(""));
        assert!(lines.next().unwrap().starts_with("pq-2: "));
    }

    #[test]
    fn a_write_that_cannot_be_committed_is_reported_and_the_next_one_commits_it() {
        let base = tempfile::tempdir().unwrap();
        let (_, q) = local_queue(base.path());
        let h = of(&q).unwrap();
        let (id, path) = task(&q, "Held");
        let n = commits(&q);
        // Another git holding the history's index: the commit fails, the write doesn't.
        fs::write(h.join("index.lock"), "").unwrap();
        ops::claim_task(&q, &id, "wt-a", T).unwrap();
        assert!(fs::read_to_string(q.join(&path))
            .unwrap()
            .contains("claimed-by: wt-a"));
        let s = status(&q);
        assert_eq!(s.pending, 1);
        assert!(
            s.pending_error.contains("index.lock"),
            "{}",
            s.pending_error
        );
        let p = problems("q", &q);
        assert!(
            p.iter()
                .any(|l| l.starts_with("q: history: 1 write not committed (")
                    && l.ends_with("): muster history commit")
                    && !crate::queue::is_warning(l)),
            "{p:?}"
        );
        // A second one fails too, and is counted.
        crate::backlog::add(&q, "Also held", "", "wt-a", T).unwrap();
        assert_eq!(status(&q).pending, 2);
        fs::remove_file(h.join("index.lock")).unwrap();
        // The next write commits them with its own.
        task(&q, "Next");
        let files = last_files(&q);
        assert!(files.contains(&path), "{files:?}");
        assert!(files.iter().any(|f| f.starts_with("backlog/")), "{files:?}");
        assert_eq!(files.len(), 3, "{files:?}");
        assert_eq!(commits(&q), n + 1);
        assert_eq!(status(&q).pending, 0);
        assert!(problems("q", &q).is_empty(), "{:?}", problems("q", &q));
    }

    #[test]
    fn after_git_clean_fdx_restore_all_brings_back_every_file_byte_for_byte() {
        let base = tempfile::tempdir().unwrap();
        let (main, q) = local_queue(base.path());
        let (id, _) = task(&q, "Kept → نگه‌داشته");
        task(&q, "Second");
        ops::claim_task(&q, &id, "wt-a", T).unwrap();
        crate::queue::ask(&q, &id, "Which port?", "wt-a", "aaaa1111", T).unwrap();
        let note = crate::backlog::add(&q, "A note", "Body.", "wt-a", T).unwrap();
        crate::backlog::drop_note(&q, &note, "not now", "owner", T, crate::ops::LEASE_MINUTES)
            .unwrap();
        crate::backlog::add(&q, "Another note", "", "wt-a", T).unwrap();
        ops::attach(&q, &id, "shot.png", &[0u8, 1, 2, 255, 254], "wt-a", T).unwrap();
        // A review of record, written under the lock as the review module writes it.
        crate::queue::with_lock(&q, || {
            fs::create_dir_all(q.join("reviews")).unwrap();
            fs::write(
                q.join("reviews").join(format!("{id}.json")),
                "{\"round\":1}\n",
            )
            .unwrap();
            Ok(())
        })
        .unwrap();
        let before = files_of(&q);
        assert!(before.keys().any(|k| k.starts_with("reviews/")));
        assert!(before.keys().any(|k| k.starts_with("attachments/")));
        assert!(
            before.keys().any(|k| k.starts_with("backlog/dropped/")),
            "{:?}",
            before.keys()
        );
        assert!(problems("q", &q).is_empty(), "{:?}", problems("q", &q));

        repo_git(&main, &["clean", "-fdx"]);
        assert!(!q.join("tasks").exists() && !q.join("backlog").exists());
        assert!(
            q.join("config.toml").is_file(),
            "the repo keeps config.toml"
        );
        let p = problems("q", &q);
        assert!(
            p.iter().any(|l| l.contains("missing since the last commit")
                && l.contains(": muster restore --all, or when deleted on purpose: ")
                && !crate::queue::is_warning(l)),
            "{p:?}"
        );
        let n = commits(&q);
        let back = restore(&q, &[], true, "owner").unwrap();
        // Everything but what the repo itself keeps (config.toml, .gitignore).
        assert_eq!(back.len(), before.len() - 2, "{back:?}");
        assert_eq!(files_of(&q), before);
        assert_eq!(commits(&q), n + 1);
        assert_eq!(
            last_subject(&q),
            format!("muster: restored {} files (owner)", back.len())
        );
        assert!(last_files(&q).is_empty(), "a restore changes no file");
        assert!(problems("q", &q).is_empty(), "{:?}", problems("q", &q));
        assert!(q.join("INDEX.md").is_file(), "the list is rebuilt");
        assert!(restore(&q, &[], true, "owner").unwrap().is_empty());
        assert_eq!(commits(&q), n + 1);
    }

    #[test]
    fn restore_of_a_path_puts_back_that_file_or_folder_and_refuses_what_the_history_lacks() {
        let base = tempfile::tempdir().unwrap();
        let (_, q) = local_queue(base.path());
        let (_, a) = task(&q, "A");
        let (_, b) = task(&q, "B");
        let a_text = fs::read(q.join(&a)).unwrap();
        fs::write(q.join(&a), "broken").unwrap();
        fs::remove_file(q.join(&b)).unwrap();
        assert_eq!(
            restore(&q, std::slice::from_ref(&a), false, "owner").unwrap(),
            vec![a.clone()]
        );
        assert_eq!(fs::read(q.join(&a)).unwrap(), a_text);
        assert!(!q.join(&b).exists());
        // A folder, given absolute.
        let abs = q.join("tasks").to_string_lossy().to_string();
        assert_eq!(
            restore(&q, &[abs], false, "owner").unwrap(),
            vec![b.clone()]
        );
        assert!(q.join(&b).is_file());
        for bad in [
            "tasks/0099-none.md",
            "../README.md",
            "mail/.lock",
            "INDEX.md",
            "/elsewhere/x.md",
        ] {
            assert!(
                restore(&q, &[bad.to_string()], false, "owner").is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn the_message_is_the_key_and_log_line_cleaned_and_names_notes_and_attachments() {
        let base = tempfile::tempdir().unwrap();
        let q = base.path().join(".muster");
        fs::create_dir_all(q.join("tasks")).unwrap();
        fs::write(q.join("config.toml"), "local = true\nprefix = \"mu\"\n").unwrap();
        fs::write(
            q.join("tasks/0042-x.md"),
            "---\nid: 0042\n---\n\n# X\n\n## Log\n- 2026-10-14 09:00 owner: created\n- 2026-10-14 09:01 شاپور: برچسب \u{7}set\n\n## Questions\n- [ ] **Q1 (a, 2026):** not this\n",
        )
        .unwrap();
        assert_eq!(
            message(&q, &["tasks/0042-x.md".into()]),
            "mu-42: 2026-10-14 09:01 شاپور: برچسب set"
        );
        assert_eq!(
            message(&q, &["tasks/0043-gone.md".into()]),
            "mu-43: 0043-gone.md removed"
        );
        assert_eq!(
            message(
                &q,
                &[
                    "attachments/0042/a.png".into(),
                    "backlog/2026-n.md".into(),
                    "lessons.md".into()
                ]
            ),
            "mu-42: attachment a.png removed\n\nbacklog: 2026-n.md moved\nmuster: lessons.md"
        );
        assert_eq!(message(&q, &[]), "muster: a write");
    }

    #[test]
    fn kept_paths_leave_out_mail_the_lists_temp_files_and_climbing() {
        for p in [
            "tasks/0001-a.md",
            "backlog/dropped/x.md",
            "attachments/0001/a b.png",
            "config.toml",
        ] {
            assert!(kept_path(p), "{p}");
        }
        for p in [
            "mail/.lock",
            "mail/aaaa1111/0001-Q1.md",
            "INDEX.md",
            "tasks.json",
            ".claude/settings.local.json",
            "tasks/0001-a.muster-123.tmp",
            "../x",
            "tasks/../../x",
            "tasks/.git/config",
            ".DS_Store",
            "",
        ] {
            assert!(!kept_path(p), "{p}");
        }
    }

    #[test]
    fn a_pending_write_whose_file_was_cleaned_since_is_never_committed_as_removed() {
        let base = tempfile::tempdir().unwrap();
        let (_, q) = local_queue(base.path());
        let h = of(&q).unwrap();
        let (id, path) = task(&q, "Held");
        let committed = fs::read(q.join(&path)).unwrap();
        fs::write(h.join("index.lock"), "").unwrap();
        ops::claim_task(&q, &id, "wt-a", T).unwrap();
        // A note dropped while the history is held: the move's old path is the write's own removal.
        let note = crate::backlog::add(&q, "Note", "", "wt-a", T).unwrap();
        crate::backlog::drop_note(&q, &note, "not now", "owner", T, crate::ops::LEASE_MINUTES)
            .unwrap();
        assert_eq!(status(&q).pending, 3);
        // The claimed task's file is cleaned away before anything commits it.
        fs::remove_file(q.join(&path)).unwrap();
        fs::remove_file(h.join("index.lock")).unwrap();
        task(&q, "Next");
        assert_eq!(status(&q).pending, 0);
        let tree = hist(&q, &["ls-tree", "-r", "--name-only", "HEAD"]);
        assert!(tree.lines().any(|l| l == path), "{path} still in {tree}");
        assert!(
            !tree.contains(&format!("backlog/{note}")),
            "the moved note's old path went: {tree}"
        );
        assert!(tree.contains(&format!("backlog/dropped/{note}")), "{tree}");
        assert_eq!(status(&q).missing, vec![path.clone()]);
        assert_eq!(restore(&q, &[], true, "owner").unwrap(), vec![path.clone()]);
        assert_eq!(fs::read(q.join(&path)).unwrap(), committed);
        // history commit leaves a missing file alone too.
        fs::remove_file(q.join(&path)).unwrap();
        assert!(commit_outside(&q).unwrap().is_empty());
        assert_eq!(status(&q).missing, vec![path]);
    }

    #[test]
    fn restore_all_brings_back_what_is_gone_and_leaves_changed_files_as_they_are() {
        let base = tempfile::tempdir().unwrap();
        let (_, q) = local_queue(base.path());
        let (_, a) = task(&q, "A");
        let (_, b) = task(&q, "B");
        let edited =
            fs::read_to_string(q.join(&a)).unwrap() + "\nan answer written outside muster\n";
        fs::write(q.join(&a), &edited).unwrap();
        fs::remove_file(q.join(&b)).unwrap();
        assert_eq!(restore(&q, &[], true, "owner").unwrap(), vec![b.clone()]);
        assert!(q.join(&b).is_file());
        assert_eq!(fs::read_to_string(q.join(&a)).unwrap(), edited);
    }

    #[test]
    fn a_task_made_before_the_restore_never_takes_an_id_the_history_holds() {
        let base = tempfile::tempdir().unwrap();
        let (main, q) = local_queue(base.path());
        task(&q, "One");
        task(&q, "Two");
        repo_git(&main, &["clean", "-fdx"]);
        assert_eq!(last_id(&q), 2);
        let (id, _) = task(&q, "Made after the clean");
        assert_eq!(id, "0003");
        assert_eq!(
            restore(&q, &[], true, "owner").unwrap().len(),
            3,
            "README and both tasks"
        );
        let ids: Vec<String> = crate::queue::task_files(&q)
            .iter()
            .map(|(_, p)| p.file_name().unwrap().to_string_lossy()[..4].to_string())
            .collect();
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "no id twice: {ids:?}");
    }

    #[test]
    fn a_queue_cleaned_before_its_settings_were_committed_comes_back_whole() {
        let base = tempfile::tempdir().unwrap();
        let main = repo(base.path(), "proj");
        let q = crate::projects::init(&main, true, Some("pq"), &[])
            .unwrap()
            .0;
        let (_, path) = task(&q, "Kept");
        let before = files_of(&q);
        // .muster/config.toml isn't committed in the repo yet: the clean takes all of it.
        repo_git(&main, &["clean", "-fdx"]);
        assert!(!q.exists());
        assert!(of(&q).is_some(), "the history is still found");
        let back = restore(&q, &[], true, "owner").unwrap();
        assert!(
            back.contains(&"config.toml".to_string()) && back.contains(&path),
            "{back:?}"
        );
        assert_eq!(files_of(&q), before);
        assert!(crate::layout::read_config(&q).unwrap().local);
    }

    /// Is `id` a loose object of the history (not packed)?
    fn loose(h: &Path, id: &str) -> bool {
        h.join("objects").join(&id[..2]).join(&id[2..]).is_file()
    }

    #[test]
    fn the_history_is_packed_once_git_gc_auto_finds_enough_loose_objects() {
        let base = tempfile::tempdir().unwrap();
        let (_, q) = local_queue(base.path());
        let h = of(&q).unwrap();
        let (id, path) = task(&q, "Packed");
        // gc --auto counts the loose objects in objects/17 against gc.auto / 256: with gc.auto 1,
        // two there are enough. These two files' blobs are 17c90b5… and 17d3e68….
        hist(&q, &["config", "gc.auto", "1"]);
        hist(&q, &["config", "gc.autoDetach", "false"]);
        assert!(!h.join("objects/pack").read_dir().unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".pack")));
        // Commits and trees carry the time, so one of them may land in objects/17 too.
        let before = fs::read_dir(h.join("objects/17")).map_or(0, |d| d.count());
        ops::attach(&q, &id, "a.txt", b"pack me 645\n", "wt-a", T).unwrap();
        let a = "17c90b51b7fab09bb5c9de14387c5eec8bfbb5ce";
        if before == 0 && fs::read_dir(h.join("objects/17")).unwrap().count() == 1 {
            assert!(loose(&h, a), "one in objects/17 isn't enough");
        }
        ops::attach(&q, &id, "b.txt", b"pack me 791\n", "wt-a", T).unwrap();
        let b = "17d3e6854aaf6af81749ff606b0453ee13f30bda";
        assert!(!loose(&h, a) && !loose(&h, b), "packed after the write");
        let packs = h
            .join("objects/pack")
            .read_dir()
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".pack")
            })
            .count();
        assert!(packs >= 1);
        // Still whole: the files read back from the packed history.
        assert_eq!(
            hist(&q, &["show", &format!("HEAD:attachments/{id}/b.txt")]),
            "pack me 791\n"
        );
        assert!(hist(&q, &["show", &format!("HEAD:{path}")]).contains("attached b.txt"));
        assert!(problems("q", &q).is_empty(), "{:?}", problems("q", &q));
    }

    /// A task of `q` in Testing, started on a branch as `muster start` records it.
    fn in_testing(q: &Path) -> (String, String) {
        let (id, path) = task(q, "Reviewed");
        ops::start_task(q, &id, "wt-a", T, "/nowhere/wt", "task/pq-1-x", false).unwrap();
        let opts = ops::PassOpts {
            gate: "gate: green (built)".into(),
            summary: "A synthetic summary".into(),
            from: Some("building".into()),
            ..Default::default()
        };
        ops::pass_task(q, &id, "wt-a", T, &opts).unwrap();
        (id, path)
    }

    #[test]
    fn a_write_of_several_files_is_named_by_its_task_and_log_line_not_its_first_path() {
        let base = tempfile::tempdir().unwrap();
        let (_, q) = local_queue(base.path());
        let (id, path) = in_testing(&q);

        // An attachment: the file and the task's log line.
        ops::attach(&q, &id, "shot.png", b"\x89PNG bytes", "wt-a", T).unwrap();
        let line = last_log_line(&q.join(&path)).unwrap();
        assert!(line.contains("attached shot.png"), "{line}");
        assert_eq!(last_subject(&q), format!("pq-1: {line}"));
        assert_eq!(
            last_files(&q),
            vec![format!("attachments/{id}/shot.png"), path.clone()]
        );

        // A review of record: reviews/<id>.json, the task and lessons.md, under one lock.
        let head = "1111111111111111111111111111111111111111";
        let json = format!(
            r#"{{"head":"{head}","tier":"light","tokens":1000,"findings":[{{"lens":"correctness","severity":"should","confidence":90,"file":"app/x.rs","line":4,"title":"Unbounded","scenario":"a 2 GB output","fix":"cap it","kind":"unbounded-output","lesson":{{"rule":"Cap every output.","bad":"output()","good":"take(1 MB)"}}}}],"answers":[]}}"#
        );
        let span = crate::review::Span {
            base: "0000000000000000000000000000000000000000".into(),
            head: head.into(),
            since: None,
        };
        let n = commits(&q);
        crate::review::record(&q, &id, &json, "qa", T, &span).unwrap();
        // Two writes: the record (one commit for all its files), then the return to Building.
        assert_eq!(commits(&q), n + 2);
        let mut files: Vec<String> = hist(&q, &["show", "--name-only", "--format=", "HEAD~1"])
            .lines()
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect();
        files.sort();
        assert!(
            files.contains(&format!("reviews/{id}.json")) && files.contains(&path),
            "{files:?}"
        );
        assert!(files.len() >= 2 && files.len() <= 3, "{files:?}");
        // The task as the record left it, and its last log line.
        let then = base.path().join("then.md");
        fs::write(&then, hist(&q, &["show", &format!("HEAD~1:{path}")])).unwrap();
        let line = last_log_line(&then).unwrap();
        assert!(line.contains("review round 1"), "{line}");
        assert_eq!(
            hist(&q, &["log", "-1", "--format=%s", "HEAD~1"]).trim(),
            format!("pq-1: {line}")
        );
        // The other files follow in the body.
        let body = hist(&q, &["log", "-1", "--format=%b", "HEAD~1"]);
        assert!(
            body.contains(&format!("muster: reviews/{id}.json")),
            "{body}"
        );
        let line = last_log_line(&q.join(&path)).unwrap();
        assert_eq!(last_subject(&q), format!("pq-1: {line}"));
    }

    #[test]
    fn a_file_deleted_on_purpose_is_recorded_as_removed_and_can_still_be_brought_back_by_name() {
        let base = tempfile::tempdir().unwrap();
        let (_, q) = local_queue(base.path());
        let (id, path) = task(&q, "To delete");
        let (_, kept) = task(&q, "Kept");
        ops::attach(&q, &id, "private.png", b"private bytes", "wt-a", T).unwrap();
        let shot = format!("attachments/{id}/private.png");
        let text = fs::read(q.join(&path)).unwrap();
        fs::remove_file(q.join(&path)).unwrap();
        fs::remove_file(q.join(&shot)).unwrap();
        let p = problems("q", &q);
        assert!(
            p.iter().any(|l| l.contains("2 files missing")
                && l.contains("muster restore --all")
                && l.ends_with("muster history removed <path>…")),
            "{p:?}"
        );
        // Only a file that's gone and in the last commit can be recorded as removed.
        assert!(removed(&q, std::slice::from_ref(&kept), false, "owner").is_err());
        assert!(removed(&q, &["tasks/0099-none.md".into()], false, "owner").is_err());
        assert!(removed(&q, &["../x".into()], false, "owner").is_err());
        assert!(removed(&q, &[], false, "owner").is_err());
        // A folder records every file gone under it.
        let n = commits(&q);
        assert_eq!(
            removed(&q, &[format!("attachments/{id}")], false, "owner").unwrap(),
            vec![shot.clone()]
        );
        assert_eq!(removed(&q, &[], true, "owner").unwrap(), vec![path.clone()]);
        assert_eq!(commits(&q), n + 2);
        assert_eq!(last_files(&q), vec![path.clone()]);
        assert_eq!(last_subject(&q), "muster: removed 1 file (owner)");
        assert!(problems("q", &q).is_empty(), "{:?}", problems("q", &q));
        assert!(removed(&q, &[], true, "owner").unwrap().is_empty());
        // restore --all leaves them gone; another write doesn't bring them back either.
        assert!(restore(&q, &[], true, "owner").unwrap().is_empty());
        task(&q, "Next");
        assert!(!q.join(&path).exists() && !q.join(&shot).exists());
        // Older commits keep them: named, restore brings one back from before its removal.
        assert_eq!(
            restore(&q, std::slice::from_ref(&path), false, "owner").unwrap(),
            vec![path.clone()]
        );
        assert_eq!(fs::read(q.join(&path)).unwrap(), text);
        assert_eq!(
            restore(&q, &[format!("attachments/{id}")], false, "owner").unwrap(),
            vec![shot.clone()]
        );
        assert_eq!(fs::read(q.join(&shot)).unwrap(), b"private bytes");
        // Back in the history too: the restore committed them.
        let tree = hist(&q, &["ls-tree", "-r", "--name-only", "HEAD"]);
        assert!(
            tree.lines().any(|l| l == path) && tree.lines().any(|l| l == shot),
            "{tree}"
        );
        assert!(last_subject(&q).starts_with("muster: restored 1 file"));
        assert!(problems("q", &q).is_empty(), "{:?}", problems("q", &q));
        // A path the history never had is still refused.
        assert!(restore(&q, &["tasks/0099-never.md".into()], false, "owner").is_err());
    }

    #[test]
    fn a_removed_task_s_id_is_never_given_again_and_restore_refuses_a_task_whose_id_is_taken() {
        let base = tempfile::tempdir().unwrap();
        let (_, q) = local_queue(base.path());
        let (_, one) = task(&q, "Task one");
        let (_, two) = task(&q, "Task two");
        fs::remove_file(q.join(&one)).unwrap();
        fs::remove_file(q.join(&two)).unwrap();
        assert_eq!(removed(&q, &[], true, "owner").unwrap().len(), 2);
        // The history's last commit has no task file now; its removal still holds both ids.
        assert_eq!(last_id(&q), 2);
        let (id, three) = task(&q, "Task three");
        assert_eq!(id, "0003");
        assert_eq!(
            restore(&q, std::slice::from_ref(&one), false, "owner").unwrap(),
            vec![one.clone()]
        );
        // A task file made by hand (or before this fix) holds 0002: its old file can't come back.
        let hand = "tasks/0002-by-hand.md";
        fs::write(q.join(hand), fs::read(q.join(&three)).unwrap()).unwrap();
        let n = commits(&q);
        for p in [two.clone(), "tasks".into(), ".".into()] {
            let e = restore(&q, std::slice::from_ref(&p), false, "owner").unwrap_err();
            assert!(
                e.contains(&two) && e.contains("pq-2") && e.contains(hand),
                "{p}: {e}"
            );
        }
        assert!(!q.join(&two).exists());
        assert_eq!(commits(&q), n, "a refused restore commits nothing");
        // Once the id is free again, it comes back.
        fs::remove_file(q.join(hand)).unwrap();
        assert_eq!(
            restore(&q, &["tasks".into()], false, "owner").unwrap(),
            vec![two.clone()]
        );
        let ids: Vec<String> = crate::queue::task_files(&q)
            .iter()
            .map(|(_, p)| p.file_name().unwrap().to_string_lossy()[..4].to_string())
            .collect();
        assert_eq!(ids, vec!["0001", "0002", "0003"]);
    }

    #[test]
    fn restore_of_a_folder_brings_back_what_was_removed_under_it_and_nothing_a_write_moved() {
        let base = tempfile::tempdir().unwrap();
        let (_, q) = local_queue(base.path());
        let (id, _) = task(&q, "With files");
        ops::attach(&q, &id, "a.txt", b"a", "wt-a", T).unwrap();
        ops::attach(&q, &id, "b.txt", b"b", "wt-a", T).unwrap();
        let b = format!("attachments/{id}/b.txt");
        fs::remove_file(q.join(&b)).unwrap();
        assert_eq!(
            removed(&q, std::slice::from_ref(&b), false, "owner").unwrap(),
            vec![b.clone()]
        );
        // A note removed, brought back, then moved to backlog/dropped/ by a write: its last
        // removal is the write's, so no folder restore undoes the drop.
        let note = crate::backlog::add(&q, "Note", "", "wt-a", T).unwrap();
        let at = format!("backlog/{note}");
        fs::remove_file(q.join(&at)).unwrap();
        assert_eq!(
            removed(&q, std::slice::from_ref(&at), false, "owner").unwrap(),
            vec![at.clone()]
        );
        assert_eq!(
            restore(&q, std::slice::from_ref(&at), false, "owner").unwrap(),
            vec![at.clone()]
        );
        crate::backlog::drop_note(&q, &note, "not now", "owner", T, crate::ops::LEASE_MINUTES)
            .unwrap();
        // The folder still holds a committed file (a.txt): b.txt comes back all the same.
        assert_eq!(
            restore(&q, &[format!("attachments/{id}")], false, "owner").unwrap(),
            vec![b.clone()]
        );
        assert_eq!(fs::read(q.join(&b)).unwrap(), b"b");
        assert!(restore(&q, &["backlog".into()], false, "owner")
            .unwrap()
            .is_empty());
        assert!(!q.join("backlog").join(&note).exists());
        // A whole queue removed, then a new task: `restore tasks` and `restore .` bring back the rest.
        let (_, t2) = task(&q, "Two");
        fs::remove_file(q.join(&b)).unwrap();
        fs::remove_file(q.join(&t2)).unwrap();
        assert_eq!(removed(&q, &[], true, "owner").unwrap().len(), 2);
        let (_, t3) = task(&q, "Three");
        assert_eq!(
            restore(&q, &["tasks".into()], false, "owner").unwrap(),
            vec![t2.clone()]
        );
        assert_eq!(
            restore(&q, &[".".into()], false, "owner").unwrap(),
            vec![b.clone()]
        );
        assert!(q.join(&t3).exists() && q.join(&t2).exists() && q.join(&b).exists());
        assert!(!q.join("backlog").join(&note).exists());
        assert!(problems("q", &q).is_empty(), "{:?}", problems("q", &q));
        // Everything recorded as removed is back: nothing left to restore.
        assert!(restore(&q, &[".".into()], false, "owner")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn restore_all_never_undoes_a_removal_a_pending_write_made() {
        let base = tempfile::tempdir().unwrap();
        let (_, q) = local_queue(base.path());
        let h = of(&q).unwrap();
        let note = crate::backlog::add(&q, "Note", "", "wt-a", T).unwrap();
        fs::write(h.join("index.lock"), "").unwrap();
        crate::backlog::drop_note(&q, &note, "not now", "owner", T, crate::ops::LEASE_MINUTES)
            .unwrap();
        fs::remove_file(h.join("index.lock")).unwrap();
        let s = status(&q);
        assert_eq!(s.pending, 1);
        assert!(s.missing.is_empty(), "{:?}", s.missing);
        let p = problems("q", &q);
        assert_eq!(p.len(), 1, "only the pending write: {p:?}");
        assert!(restore(&q, &[], true, "owner").unwrap().is_empty());
        assert!(!q.join("backlog").join(&note).exists());
        task(&q, "Next");
        let tree = hist(&q, &["ls-tree", "-r", "--name-only", "HEAD"]);
        assert!(!tree.contains(&format!("backlog/{note}")), "{tree}");
        assert!(tree.contains(&format!("backlog/dropped/{note}")), "{tree}");
        assert!(problems("q", &q).is_empty(), "{:?}", problems("q", &q));
    }

    #[test]
    fn a_write_whose_history_folder_cannot_be_written_is_remembered_in_the_queue_and_retried() {
        use std::os::unix::fs::PermissionsExt;
        let base = tempfile::tempdir().unwrap();
        let (_, q) = local_queue(base.path());
        let h = of(&q).unwrap();
        let (id, path) = task(&q, "Sandboxed");
        let n = commits(&q);
        // As a sandbox that allows .muster/ but not .git/ leaves it: nothing in the history's
        // folder can be made (no index.lock, no object, no muster-pending).
        let walk_mode = |m: u32| {
            for d in [h.join("objects"), h.clone()] {
                fs::set_permissions(&d, fs::Permissions::from_mode(m)).unwrap();
            }
        };
        walk_mode(0o555);
        ops::claim_task(&q, &id, "wt-a", T).unwrap();
        let s = status(&q);
        walk_mode(0o755);
        assert_eq!(s.pending, 1, "{s:?}");
        assert!(
            s.changed.is_empty(),
            "reported as not committed, not as outside muster"
        );
        assert!(q.join("mail/history-pending").is_file());
        assert!(!h.join(PENDING).exists());
        let p = problems("q", &q);
        assert!(
            p.iter().any(|l| l.contains("1 write not committed")),
            "{p:?}"
        );
        // The next write that can reach the history commits it and forgets it.
        task(&q, "Next");
        assert_eq!(commits(&q), n + 1);
        assert!(last_files(&q).contains(&path));
        assert!(!q.join("mail/history-pending").exists());
        assert!(problems("q", &q).is_empty(), "{:?}", problems("q", &q));
    }

    #[test]
    fn git_s_reason_is_kept_when_it_exits_1_and_paths_are_named_from_the_history() {
        let base = tempfile::tempdir().unwrap();
        let (_, q) = local_queue(base.path());
        let h = of(&q).unwrap();
        // git checkout of a path it doesn't know exits 1 with its reason on stderr.
        let e = git(&h, &q, &["checkout", "HEAD", "--", "nothing-here"], None).unwrap_err();
        assert!(e.contains("did not match"), "{e}");
        // A history git can't write to: the reason names it, not the folders above it.
        let (id, _) = task(&q, "Refused");
        let objects = h.join("objects");
        let mode = |m: u32| {
            use std::os::unix::fs::PermissionsExt;
            for d in fs::read_dir(&objects).unwrap().flatten() {
                fs::set_permissions(d.path(), fs::Permissions::from_mode(m)).unwrap();
            }
            fs::set_permissions(&objects, fs::Permissions::from_mode(m)).unwrap();
        };
        mode(0o555);
        ops::claim_task(&q, &id, "wt-a", T).unwrap();
        mode(0o755);
        let s = status(&q);
        assert_eq!(s.pending, 1);
        let root = base.path().to_string_lossy().to_string();
        let real = fs::canonicalize(base.path())
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert!(
            !s.pending_error.contains(&root) && !s.pending_error.contains(&real),
            "{}",
            s.pending_error
        );
        assert!(
            s.pending_error.contains("history.git/objects"),
            "{}",
            s.pending_error
        );
    }
}
