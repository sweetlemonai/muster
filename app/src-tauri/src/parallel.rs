//! Parallel sessions (0023): which Ready tasks can be built side by side, and one task's life in
//! its own git worktree. `muster start` makes the branch and the worktree and claims the task;
//! `muster pass <id> --from testing` rebases it on the local main, runs the project's gate, merges
//! it into the local main, pushes main and cleans up; `muster release <id> --clean` gives it up;
//! `muster check` finds what's left behind. Every git call is an argument array: nothing runs
//! through a shell, nothing is ever force-pushed, and a worktree or branch is only removed once
//! its work is on main (or the caller said --force after being shown what goes).

use crate::queue::{self, StageStep, Task};
use crate::{ops, session};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

// ---------------------------------------------------------------------------------------------
// What can run together

/// The fixed part of a `touches:` entry: everything before its first wildcard (`*`, `?`, `[`,
/// `{`). `app/src/TaskPanel*` → `app/src/TaskPanel`; `**/*.rs` → "".
pub fn fixed_prefix(entry: &str) -> &str {
    let end = entry.find(['*', '?', '[', '{']).unwrap_or(entry.len());
    &entry[..end]
}

/// Do two `touches:` lists overlap? Two entries do when one's fixed prefix is a prefix of the
/// other's, so `app/src` overlaps `app/src/TaskPanel*` (and `app/src-tauri`: the rule is plain
/// text, on the safe side), and an entry that starts with a wildcard overlaps everything.
pub fn touches_overlap(a: &[String], b: &[String]) -> bool {
    a.iter().any(|x| {
        b.iter().any(|y| {
            let (p, q) = (fixed_prefix(x), fixed_prefix(y));
            p.starts_with(q) || q.starts_with(p)
        })
    })
}

/// May `a` and `b` be built at the same time? Both say what they touch and it doesn't overlap
/// (a task without `touches:` never pairs), and neither depends on the other.
pub fn compatible(a: &Task, b: &Task) -> bool {
    !a.touches.is_empty()
        && !b.touches.is_empty()
        && !touches_overlap(&a.touches, &b.touches)
        && !a.depends.contains(&b.id)
        && !b.depends.contains(&a.id)
}

/// Why `a` can't go beside `b`, or None when it can.
fn clash(a: &Task, b: &Task) -> Option<String> {
    if a.depends.contains(&b.id) {
        Some(format!("depends on {}", b.id))
    } else if b.depends.contains(&a.id) {
        Some(format!("{} depends on it", b.id))
    } else if a.touches.is_empty() {
        Some("no touches: says nothing of where it works".into())
    } else if b.touches.is_empty() {
        Some(format!("{} has no touches", b.id))
    } else if touches_overlap(&a.touches, &b.touches) {
        Some(format!("touches overlap {}", b.id))
    } else {
        None
    }
}

fn is_id(id: &str) -> bool {
    id.len() == 4 && id.bytes().all(|b| b.is_ascii_digit())
}

/// Could a session start `t` now? A task (not a goal or feature) in Ready, not blocked or paused
/// (0045), nobody's claim on it, an agent's (never for: human), and everything it depends on merged, in Your check
/// or Done (0042, `block::holds`; a dependency that doesn't exist doesn't hold it, as with `claim`).
pub fn claimable(t: &Task, tasks: &[Task]) -> bool {
    t.stage == "ready"
        && !t.blocked
        && t.paused.is_empty()
        && is_id(&t.id)
        && t.level == "task"
        && !t.is_human()
        && t.claimed_by.is_empty()
        && t.depends.iter().filter(|d| **d != t.id).all(|d| {
            !tasks
                .iter()
                .any(|o| o.id == *d && crate::block::holds(o, false))
        })
}

fn priority_rank(p: &str) -> u8 {
    match p.to_ascii_lowercase().as_str() {
        "high" => 0,
        "low" => 2,
        _ => 1,
    }
}

/// Every task a session could start now, high priority first, then the lowest id (`next`'s
/// order).
pub fn ready(tasks: &[Task]) -> Vec<&Task> {
    let mut out: Vec<&Task> = tasks.iter().filter(|t| claimable(t, tasks)).collect();
    out.sort_by_key(|t| (priority_rank(&t.priority), t.id.clone()));
    out
}

/// What `ready --parallel` picks: the set, and the claimable tasks left out with why.
#[derive(Debug)]
pub struct Pick<'a> {
    pub set: Vec<&'a Task>,
    pub left: Vec<(&'a Task, String)>,
}

/// Up to `n` claimable tasks that are safe together and beside every task in `running`: taken in
/// `ready`'s order, each kept when it's `compatible` with every one already in.
pub fn parallel_set<'a>(tasks: &'a [Task], n: usize, running: &[&Task]) -> Pick<'a> {
    let mut pick = Pick {
        set: Vec::new(),
        left: Vec::new(),
    };
    for t in ready(tasks) {
        if pick.set.len() >= n {
            pick.left.push((t, format!("the set is full ({n})")));
            continue;
        }
        // Alone (nothing running, nothing in the set yet), a task without touches is fine: it
        // just never pairs.
        match running
            .iter()
            .chain(pick.set.iter())
            .find_map(|o| clash(t, o))
        {
            Some(why) => pick.left.push((t, why)),
            None => pick.set.push(t),
        }
    }
    pick
}

/// Tasks someone is on (Building or Testing), other than `except`: what the next start must not
/// clash with.
pub fn running<'a>(tasks: &'a [Task], except: &str) -> Vec<&'a Task> {
    tasks
        .iter()
        .filter(|t| t.id != except && t.level == "task")
        .filter(|t| matches!(t.stage.as_str(), "building" | "testing"))
        .collect()
}

/// `ready --parallel n`: up to `n` tasks safe together and beside every task already in Building
/// or Testing.
pub fn ready_parallel(tasks: &[Task], n: usize) -> Pick<'_> {
    parallel_set(tasks, n, &running(tasks, ""))
}

/// The next task to start after `merged` (step 8 of the merge): the first `parallel_set` would
/// take beside the tasks still running.
pub fn next_after<'a>(tasks: &'a [Task], merged: &str) -> Option<&'a Task> {
    let run = running(tasks, merged);
    let pick = parallel_set(tasks, 1, &run);
    pick.set.first().copied()
}

// ---------------------------------------------------------------------------------------------
// Git, as argument arrays

fn git_output(dir: &Path, args: &[&str]) -> Result<Output, String> {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        // The repo is the one at `dir`, whatever a hook or a caller set; and git never asks.
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("couldn't run git: {e}"))
}

/// The last few lines of `text`, joined, git's hints left out: what a failure says without a
/// page of output.
fn last_lines(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("hint:"))
        .collect();
    lines[lines.len().saturating_sub(n)..].join(" / ")
}

/// `git -C <dir> <args>`: its output, trimmed, or what went wrong.
fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = git_output(dir, args)?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            last_lines(&String::from_utf8_lossy(&out.stderr), 4)
        ))
    }
}

fn utf8(p: &Path) -> Result<&str, String> {
    p.to_str()
        .ok_or_else(|| format!("{} isn't a UTF-8 path", p.display()))
}

fn canon(p: &Path) -> PathBuf {
    fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// A repo as the lifecycle needs it: the main checkout, git's common folder (`.git`), and its
/// main branch (`main`, else `master`).
#[derive(Debug, Clone)]
pub struct Repo {
    pub main: PathBuf,
    pub common: PathBuf,
    pub trunk: String,
}

/// The repo `dir` is in (its main checkout or one of its worktrees).
pub fn repo_of(dir: &Path) -> Result<Repo, String> {
    let common = git(
        dir,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .map_err(|_| format!("{} isn't in a git repo", dir.display()))?;
    let common = canon(Path::new(&common));
    let main = match common.file_name().and_then(|n| n.to_str()) {
        Some(".git") => common.parent().map(Path::to_path_buf),
        _ => None,
    }
    .ok_or_else(|| {
        format!(
            "{} isn't a checkout with its own .git folder (a bare repo?)",
            common.display()
        )
    })?;
    let trunk = ["main", "master"]
        .into_iter()
        .find(|b| {
            git(
                &main,
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    &format!("refs/heads/{b}"),
                ],
            )
            .is_ok()
        })
        .ok_or_else(|| format!("{} has no main or master branch", main.display()))?;
    Ok(Repo {
        main,
        common,
        trunk: trunk.to_string(),
    })
}

fn branch_exists(repo: &Repo, branch: &str) -> bool {
    git(
        &repo.main,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
    .is_ok()
}

/// Task `id`'s key (0033), as its branch and worktree are named: `mu-12`.
fn task_key(root: &Path, id: &str) -> String {
    crate::key::key(&crate::key::prefix_for(root), id)
}

/// A branch muster made for task `id`: `task/<key>-…` (0033: `task/mu-12-…`) with whatever prefix
/// the project had when it started, or `task/<id>-…` from before keys, so nothing else is ever
/// deleted by a hand-edited `branch:`. `key` is what the message calls the task.
fn task_branch(id: &str, key: &str, branch: &str) -> Result<(), String> {
    let ok = branch_task(branch).is_some_and(|b| b == id)
        && branch.len() < 200
        && !branch.contains("..")
        && branch
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/-_.".contains(&b));
    if ok {
        Ok(())
    } else {
        Err(format!(
            "{key}'s branch {branch:?} isn't one muster made (task/{key}-…); fix it by hand"
        ))
    }
}

/// Lines of `git status --porcelain` in `dir`: what isn't committed (untracked files too).
fn uncommitted(dir: &Path) -> Result<Vec<String>, String> {
    Ok(git(dir, &["status", "--porcelain"])?
        .lines()
        .map(str::to_string)
        .collect())
}

/// Commits on `branch` that are neither on the main branch nor on any remote branch: what
/// deleting it would lose.
fn unpushed(repo: &Repo, branch: &str) -> Result<Vec<String>, String> {
    if !branch_exists(repo, branch) {
        return Ok(Vec::new());
    }
    Ok(git(
        &repo.main,
        &[
            "log",
            "--format=%h %s",
            branch,
            "--not",
            &repo.trunk,
            "--remotes",
        ],
    )?
    .lines()
    .map(str::to_string)
    .collect())
}

/// Files with a conflict in `dir` (during a rebase or a merge).
fn conflicted(dir: &Path) -> Vec<String> {
    git(dir, &["diff", "--name-only", "--diff-filter=U"])
        .map(|s| s.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

/// `a, b and 3 more`: files named in a one-line reason.
fn name_files(files: &[String]) -> String {
    let mut shown = Vec::new();
    let mut len = 0;
    for f in files {
        if len + f.len() > 240 && !shown.is_empty() {
            break;
        }
        len += f.len() + 2;
        shown.push(f.as_str());
    }
    let more = files.len() - shown.len();
    if more > 0 {
        format!("{} and {more} more", shown.join(", "))
    } else {
        shown.join(", ")
    }
}

/// The merge lock (step 1): an exclusive flock on `<common git dir>/muster/merge.lock`, so one
/// merge runs at a time per repo, from any worktree. Held until the file is dropped.
fn merge_lock(repo: &Repo, say: &mut dyn FnMut(String)) -> Result<fs::File, String> {
    let dir = repo.common.join("muster");
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("merge.lock"))
        .map_err(|e| e.to_string())?;
    match file.try_lock() {
        Ok(()) => {}
        Err(fs::TryLockError::WouldBlock) => {
            say("waiting for another merge in this repo to finish…".into());
            file.lock().map_err(|e| e.to_string())?;
        }
        Err(fs::TryLockError::Error(e)) => return Err(e.to_string()),
    }
    Ok(file)
}

// ---------------------------------------------------------------------------------------------
// The gate

/// A gate run: green or not, its result line, and the end of its output.
#[derive(Debug, Clone)]
pub struct GateRun {
    pub green: bool,
    pub line: String,
    pub tail: String,
}

/// Run the project's gate in `dir`, as `argv` (split from projects.json's `gate` without a
/// shell). A relative program with a `/` (`bin/gate`) is the repo's script, found from `dir`.
/// The result line is the last output line that starts with "gate" (else one is made from the
/// exit). Green needs both: exit 0 and no "RED" in that line.
pub fn run_gate(argv: &[String], dir: &Path) -> Result<GateRun, String> {
    let Some(prog) = argv.first() else {
        return Err("the gate command is empty".into());
    };
    let program = if prog.contains('/') && Path::new(prog).is_relative() {
        dir.join(prog)
    } else {
        PathBuf::from(prog)
    };
    let out = gate_command(&program, argv, dir)
        .output()
        .map_err(|e| format!("couldn't run the gate {prog:?}: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let gate_lines = |text: &str| -> Vec<String> {
        text.lines()
            .map(str::trim)
            .filter(|l| l.to_ascii_lowercase().starts_with("gate"))
            .map(|l| l.chars().take(300).collect())
            .collect()
    };
    let (out_lines, err_lines) = (gate_lines(&stdout), gate_lines(&stderr));
    // Any gate line saying RED, in either stream, makes the run red.
    let red_line = out_lines
        .iter()
        .chain(err_lines.iter())
        .rfind(|l| l.contains("RED"))
        .cloned();
    let ok = out.status.success() && red_line.is_none();
    // The result line is stdout's last, stderr's only when stdout has none; a red run never
    // reports a green line.
    let said = out_lines.last().or(err_lines.last()).cloned();
    let line = match (said, red_line) {
        (Some(l), _) if ok || l.contains("RED") => l,
        (_, Some(red)) => red,
        (Some(l), None) => format!(
            "gate: RED ({} exited with {}; it said: {l})",
            argv.join(" "),
            out.status
        ),
        (None, None) if ok => format!("gate: green ({})", argv.join(" ")),
        (None, None) => format!("gate: RED ({} exited with {})", argv.join(" "), out.status),
    };
    let line = ops::one_line(&line, 300, "the gate line")?;
    let combined = format!("{stdout}\n{stderr}");
    let tail: Vec<&str> = combined.lines().filter(|l| !l.trim().is_empty()).collect();
    Ok(GateRun {
        green: ok,
        tail: tail[tail.len().saturating_sub(15)..].join("\n"),
        line,
    })
}

/// The gate as a process: `program` with `argv`'s arguments, in `dir`, with no stdin, and without
/// the queue variables (`MUSTER_ROOT`, `TASKS_ROOT`), so muster in a gate's tests can't write to
/// the queue this merge runs on.
fn gate_command(program: &Path, argv: &[String], dir: &Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(argv.get(1..).unwrap_or_default())
        .current_dir(dir)
        .env_remove("MUSTER_ROOT")
        .env_remove("TASKS_ROOT")
        .stdin(Stdio::null());
    cmd
}

// ---------------------------------------------------------------------------------------------
// Start

/// Where `start` puts the worktree.
#[derive(Debug, Clone, PartialEq)]
pub enum Place {
    /// `<project>/worktrees/<repo folder>/<id>-<slug>`, beside Orca's.
    Default,
    /// `--at <path>`.
    At(PathBuf),
    /// `--here`: the linked worktree the session is already in.
    Here,
}

/// What `start` did.
#[derive(Debug, Clone, PartialEq)]
pub struct Started {
    pub worktree: PathBuf,
    pub branch: String,
    /// The task file, relative to the queue.
    pub path: String,
    /// It already had a worktree on record (started before, returned or reclaimed): picked up
    /// again rather than made.
    pub resumed: bool,
}

/// `muster start <id>`: claim it (0002's rules) with a branch `task/<id>-<slug>` from the local
/// main's commit and a worktree for it, both recorded in the task. Git first, then the claim
/// under the queue's lock; a refused claim removes the fresh branch and worktree again. A task
/// with a worktree on record (returned or reclaimed since) is picked up there instead. `cwd` is
/// where the command runs: it finds the repo.
pub fn start(
    root: &Path,
    id: &str,
    who: &str,
    stamp: &str,
    cwd: &Path,
    place: &Place,
) -> Result<Started, String> {
    let t = queue::read_task(root, id)?;
    if !t.branch.is_empty() {
        return resume(root, &t, who, stamp, cwd, place);
    }
    if t.stage != "ready" {
        return Err(format!(
            "{id} is in {}, not Ready; start takes a Ready task",
            queue::stage_label(&t.stage)
        ));
    }
    if t.level != "task" {
        return Err(format!(
            "{id} is a {}; its tasks are what agents build",
            t.level
        ));
    }
    if t.is_human() {
        return Err(format!(
            "{id} is the owner's to do; a human task isn't built in a worktree"
        ));
    }
    if t.blocked {
        return Err(format!(
            "{id} is blocked on {}; unblock it first",
            t.waiting_on
        ));
    }
    // What it depends on, before any branch or worktree is made (`start_task` checks again under
    // the lock).
    if let Some(e) = ops::deps_refusal(id, &ops::tasks_now(root), "muster ready --parallel") {
        return Err(e);
    }
    let repo = repo_of(cwd)
        .map_err(|e| format!("muster start runs in the project's repo or a worktree of it: {e}"))?;
    let base = git(
        &repo.main,
        &[
            "rev-parse",
            "--verify",
            &format!("refs/heads/{}", repo.trunk),
        ],
    )?;
    let name = format!("{}-{}", task_key(root, id), ops::slug(&t.title));
    let branch = format!("task/{name}");
    if branch_exists(&repo, &branch) {
        return Err(format!(
            "the branch {branch} already exists in {} (from an earlier start?): delete it or rename it first",
            repo.main.display()
        ));
    }
    let (worktree, undo) = match place {
        Place::Here => here(&repo, cwd, &branch, &base)?,
        Place::Default | Place::At(_) => {
            let wt = match place {
                Place::At(p) if p.is_absolute() => p.clone(),
                Place::At(p) => cwd.join(p),
                _ => {
                    let project = root.parent().ok_or("the queue has no project folder")?;
                    let repo_name = repo.main.file_name().ok_or("the repo has no folder name")?;
                    project.join("worktrees").join(repo_name).join(&name)
                }
            };
            if wt.exists() {
                return Err(format!(
                    "{} already exists; pick another place with --at",
                    wt.display()
                ));
            }
            if let Some(parent) = wt.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
            }
            git(
                &repo.main,
                &["worktree", "add", "-b", &branch, utf8(&wt)?, &base],
            )?;
            (canon(&wt), Undo::Remove)
        }
    };
    let wt_text = utf8(&worktree)?.to_string();
    let runner = matches!(place, Place::Here);
    match ops::start_task(root, id, who, stamp, &wt_text, &branch, runner) {
        Ok(path) => Ok(Started {
            worktree,
            branch,
            path,
            resumed: false,
        }),
        Err(e) => {
            let left = undo_start(&repo, &worktree, &branch, &base, &undo);
            Err(match left {
                Some(l) => format!("{e}\n{l}"),
                None => e,
            })
        }
    }
}

/// How to take a fresh start back when the claim is refused.
enum Undo {
    /// Remove the worktree `start` made.
    Remove,
    /// Switch the current worktree back to what it had checked out (`--here`).
    SwitchBack(String),
}

/// `--here`: the current worktree, which must be a linked one with nothing uncommitted, switched
/// to a new branch at `base`.
fn here(repo: &Repo, cwd: &Path, branch: &str, base: &str) -> Result<(PathBuf, Undo), String> {
    let top = canon(Path::new(&git(cwd, &["rev-parse", "--show-toplevel"])?));
    if top == canon(&repo.main) {
        return Err(format!(
            "--here is for a worktree a runner made; {} is the main checkout: leave --here out and start makes one",
            top.display()
        ));
    }
    if !uncommitted(&top)?.is_empty() {
        return Err(format!(
            "{} has uncommitted changes; commit them first, or start without --here",
            top.display()
        ));
    }
    let was = git(&top, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let back = if was == "HEAD" {
        git(&top, &["rev-parse", "HEAD"])?
    } else {
        was
    };
    git(&top, &["switch", "-c", branch, base])?;
    Ok((top, Undo::SwitchBack(back)))
}

/// Remove what a refused start made; None when it's all gone, else what's left.
fn undo_start(repo: &Repo, wt: &Path, branch: &str, base: &str, undo: &Undo) -> Option<String> {
    let mut left = Vec::new();
    let gone = match undo {
        Undo::Remove => utf8(wt)
            .and_then(|w| git(&repo.main, &["worktree", "remove", w]))
            .err(),
        Undo::SwitchBack(back) => {
            let r = if back.len() == 40 && back.bytes().all(|b| b.is_ascii_hexdigit()) {
                git(wt, &["switch", "--detach", back])
            } else {
                git(wt, &["switch", back])
            };
            r.err()
        }
    };
    if let Some(e) = gone {
        left.push(e);
    }
    // Only a branch still where start put it: never one with work on it.
    let tip = git(&repo.main, &["rev-parse", &format!("refs/heads/{branch}")]).unwrap_or_default();
    if tip == base {
        if let Err(e) = git(&repo.main, &["branch", "-D", branch]) {
            left.push(e);
        }
    } else if !tip.is_empty() {
        left.push(format!("kept the branch {branch}: it has commits"));
    }
    (!left.is_empty()).then(|| format!("couldn't take the start back: {}", left.join("; ")))
}

/// A task started before (returned to Ready, reclaimed, or still yours): picked up in the
/// worktree on record.
fn resume(
    root: &Path,
    t: &Task,
    who: &str,
    stamp: &str,
    cwd: &Path,
    place: &Place,
) -> Result<Started, String> {
    let id = &t.id;
    let wt = PathBuf::from(&t.worktree);
    if t.worktree.is_empty() || !wt.is_dir() {
        return Err(format!(
            "{id}'s worktree {} is gone: muster release {id} --clean, then start it again",
            t.worktree
        ));
    }
    let elsewhere = match place {
        Place::Default => false,
        Place::At(p) => {
            canon(&if p.is_absolute() {
                p.clone()
            } else {
                cwd.join(p)
            }) != canon(&wt)
        }
        Place::Here => git(cwd, &["rev-parse", "--show-toplevel"])
            .map(|top| canon(Path::new(&top)) != canon(&wt))
            .unwrap_or(true),
    };
    if elsewhere {
        return Err(format!(
            "{id} already has a worktree, {} on branch {}: start it without --at or --here to work there",
            wt.display(),
            t.branch
        ));
    }
    let mine = t.stage == "building" && session::same_holder(&t.claimed_by, &queue::actor(who)?);
    let path = if mine {
        t.path.clone()
    } else {
        ops::start_task(
            root,
            id,
            who,
            stamp,
            &t.worktree,
            &t.branch,
            t.worktree_runner,
        )?
    };
    Ok(Started {
        worktree: wt,
        branch: t.branch.clone(),
        path,
        resumed: true,
    })
}

// ---------------------------------------------------------------------------------------------
// Merge: `pass --from testing` of a started task

/// How a merge ended, short of an error (which stops it with nothing deleted).
#[derive(Debug, Clone, PartialEq)]
pub enum Merge {
    /// On main (and pushed), cleaned up, in Your check; with the report and the next task.
    Merged {
        commit: String,
        report: String,
        path: String,
        /// What cleanup couldn't do (left for `muster check` to show).
        notes: Vec<String>,
        /// The next task to start: (id, title).
        next: Option<(String, String)>,
    },
    /// Back to Building with the reason (a rebase conflict, a red gate): nothing deleted.
    Returned(String),
}

/// Minutes `history` spent in `stage`, the open stretch counted to `now` (all
/// `YYYY-MM-DD HH:MM`).
pub fn minutes_in(history: &[StageStep], stage: &str, now: &str) -> i64 {
    crate::pause::working_minutes(history, &[], stage, now)
}

/// `2h 10m`, `35m`.
pub fn duration_text(minutes: i64) -> String {
    if minutes >= 60 {
        format!("{}h {}m", minutes / 60, minutes % 60)
    } else {
        format!("{minutes}m")
    }
}

/// Return the task to its builder with `why` (nothing deleted).
fn back_to_building(
    root: &Path,
    id: &str,
    why: &str,
    who: &str,
    now: &str,
) -> Result<Merge, String> {
    let why: String = why.chars().take(500).collect();
    ops::return_task(root, id, &why, Some("building"), who, now)?;
    Ok(Merge::Returned(why))
}

/// Is the main checkout ready to take the merge (step 4): on the main branch, nothing changed in
/// its tracked files?
fn main_ready(repo: &Repo) -> Result<(), String> {
    let on = git(&repo.main, &["symbolic-ref", "--quiet", "--short", "HEAD"]).unwrap_or_default();
    if on != repo.trunk {
        let on = if on.is_empty() {
            "a detached HEAD".to_string()
        } else {
            on
        };
        return Err(format!(
            "the main checkout isn't on {} ({} is on {on}): switch it back to {} and pass again. Nothing was merged or deleted",
            repo.trunk,
            repo.main.display(),
            repo.trunk
        ));
    }
    let dirty = git(
        &repo.main,
        &["status", "--porcelain", "--untracked-files=no"],
    )?;
    if !dirty.is_empty() {
        return Err(format!(
            "the main checkout isn't clean ({} has changes in tracked files): commit or stash them and pass again. Nothing was merged or deleted",
            repo.main.display()
        ));
    }
    Ok(())
}

/// How many more times a merge rebases and gates when main moved during the gate.
const REGATES: usize = 2;

/// `muster pass <id> --from testing` for a started task (0023), in order, stopping at the first
/// failure: (1) take the repo's merge lock; (2) rebase the branch on the local main, and on a
/// conflict abort it and return the task to Building naming the files; (3) run the gate in the
/// worktree, and on red return it with the gate line; if main moved while it ran, rebase and gate
/// again (twice at most, then stop: "main keeps moving"), so main only ever gets a tree the gate
/// saw; (4) fast-forward the local main to the branch, only if the main checkout is clean and on
/// main (never a merge commit); (5) push main to origin, plainly (rejected: fetch, say so, stop);
/// (6) remove the worktree (never a runner's, `start --here`) and delete the branch, and origin's
/// copy only when it's the same commit; (7) record the merge in the task (worktree and branch
/// cleared, `commit:` the merge, the report logged) and pass it to Your check: once main has the
/// merge this is written whatever, and a pass refused meanwhile (blocked, moved) is an error that
/// says so; (8) name the next task to start. The report and the commit are checked before step 4.
/// A refusal before step 4 or a stop at 4 or 5 leaves the worktree, the branch and the task as
/// they were. `now` gives the local time for each write; `say` gets a line per step done.
pub fn merge(
    root: &Path,
    id: &str,
    who: &str,
    now: &dyn Fn() -> String,
    gate: &[String],
    say: &mut dyn FnMut(String),
) -> Result<Merge, String> {
    let t = queue::read_task(root, id)?;
    if t.stage != "testing" {
        return Err(format!(
            "{id} is in {}, not Testing; it merges when it passes from Testing",
            queue::stage_label(&t.stage)
        ));
    }
    task_branch(id, &task_key(root, id), &t.branch)?;
    // Refused now if the pass itself would be (criteria not ticked, blocked): never merge first.
    ops::check_merge_pass(root, id, who)?;
    if gate.is_empty() {
        return Err("the gate command is empty".into());
    }
    let wt = PathBuf::from(&t.worktree);
    if !wt.is_dir() {
        return Err(format!(
            "{id}'s worktree {} is gone, so there's nothing to merge from: muster release {id} --clean",
            t.worktree
        ));
    }
    let repo = repo_of(&wt)?;
    if canon(&wt) == canon(&repo.main) {
        return Err(format!(
            "{id}'s worktree is the main checkout; muster only merges from a worktree of its own"
        ));
    }
    let branch = t.branch.clone();
    let on = git(&wt, &["symbolic-ref", "--quiet", "--short", "HEAD"]).unwrap_or_default();
    if on != branch {
        return Err(format!(
            "{} is on {}, not {branch}: switch it back and pass again",
            wt.display(),
            if on.is_empty() {
                "a detached HEAD"
            } else {
                &on
            }
        ));
    }
    if !uncommitted(&wt)?.is_empty() {
        return Err(format!(
            "{} has uncommitted changes: commit them (or drop them) and pass again. Nothing was merged",
            wt.display()
        ));
    }
    // Checked now so a gate isn't run for nothing; checked again under the lock, before the merge.
    main_ready(&repo)?;

    // 1. One merge at a time in this repo.
    let _lock = merge_lock(&repo, say)?;

    // 2 and 3. Rebase on the local main, then the gate in the worktree; again if main moved
    // while the gate ran, so what lands is what the gate saw.
    let trunk = repo.trunk.clone();
    let mut again = 0;
    let run = loop {
        if let Err(e) = git(&wt, &["rebase", &trunk]) {
            let files = conflicted(&wt);
            let aborted = git(&wt, &["rebase", "--abort"]);
            if files.is_empty() {
                return Err(format!(
                    "the rebase on {trunk} failed: {e}{}. Nothing was merged or deleted",
                    aborted.err().map(|a| format!(" ({a})")).unwrap_or_default()
                ));
            }
            let why = format!(
                "rebase on {trunk} conflicts in {}: rebase {branch} on {trunk} in its worktree, resolve them, and pass it again",
                name_files(&files)
            );
            say(format!("rebase on {trunk} conflicts; back to Building"));
            return back_to_building(root, id, &why, who, &now());
        }
        let onto = git(&repo.main, &["rev-parse", &trunk])?;
        say(format!(
            "rebased {branch} on {trunk} ({})",
            onto.chars().take(7).collect::<String>()
        ));
        let run = run_gate(gate, &wt)?;
        if !run.green {
            say(format!("{}\n{}", run.line, run.tail));
            let why = format!("gate after the rebase on {trunk}: {}", run.line);
            return back_to_building(root, id, &why, who, &now());
        }
        say(run.line.clone());
        if git(&repo.main, &["rev-parse", &trunk])? == onto {
            break run;
        }
        if again == REGATES {
            return Err(format!(
                "main keeps moving; pass again ({trunk} moved during each of {} gate runs, so nothing the gate saw could land). Nothing was merged or deleted",
                REGATES + 1
            ));
        }
        again += 1;
        say("main moved during the gate; rebasing on it and running the gate again".into());
    };

    // The facts the task will record, checked before anything lands. Only a fast-forward lands,
    // so the merge commit is the branch's tip.
    let commit = git(&wt, &["rev-parse", "HEAD"])?;
    let short: String = commit.chars().take(7).collect();
    let has_origin = git(&repo.main, &["remote", "get-url", "origin"]).is_ok();
    let task = queue::read_task(root, id)?;
    let at = now();
    let report = format!(
        "merged {branch} into {trunk} as {short} (fast-forward{}) · {} · Building {} · Testing {} · {} check-in{}",
        if has_origin { ", pushed to origin" } else { ", not pushed: no origin" },
        run.line,
        // Paused stretches (0045) aren't time spent on it.
        duration_text(crate::pause::working_minutes(&task.stage_history, &task.pause_history, "building", &at)),
        duration_text(crate::pause::working_minutes(&task.stage_history, &task.pause_history, "testing", &at)),
        task.checkins.len(),
        if task.checkins.len() == 1 { "" } else { "s" },
    );
    ops::check_merge_facts(who, &at, &commit, &report)?;

    // 4. Fast-forward the local main: never a merge commit, never a tree the gate didn't see.
    main_ready(&repo)?;
    if let Err(e) = git(&repo.main, &["merge", "--ff-only", "--no-edit", &branch]) {
        let _ = git(&repo.main, &["merge", "--abort"]);
        return Err(format!(
            "the fast-forward of {trunk} to {branch} failed: {e}. Nothing was merged or deleted: fix what it says and pass again"
        ));
    }
    if git(&repo.main, &["rev-parse", "HEAD"])? != commit {
        return Err(format!(
            "{trunk} isn't at {short} after the fast-forward; look at {} before passing again. Nothing was deleted",
            repo.main.display()
        ));
    }
    say(format!(
        "merged {branch} into {trunk} as {short} (fast-forward)"
    ));

    // 5. Push main, plainly. Never forced.
    if has_origin {
        let out = git_output(&repo.main, &["push", "origin", &trunk])?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let e = last_lines(&stderr, 4);
            let _ = git(&repo.main, &["fetch", "origin"]);
            let rejected = ["[rejected]", "non-fast-forward", "fetch first"]
                .iter()
                .any(|w| stderr.contains(w));
            return Err(if rejected {
                format!(
                    "origin moved; rebase main and pass again ({trunk} on origin has commits the local {trunk} doesn't; fetched them). The merge is on the local {trunk} as {short}; the worktree, the branch and the task are as they were"
                )
            } else {
                format!(
                    "couldn't push {trunk} to origin: {e}. The merge is on the local {trunk} as {short}; the worktree, the branch and the task are as they were"
                )
            });
        }
        say(format!("pushed {trunk} to origin"));
    } else {
        say(format!("no origin remote: {trunk} isn't pushed"));
    }

    // 6. Clean up, now the branch is on main. A runner's worktree stays (it's theirs): it's
    // let go of the branch so the branch can go.
    let mut notes = Vec::new();
    let local_tip = git(&repo.main, &["rev-parse", &format!("refs/heads/{branch}")])?;
    if t.worktree_runner {
        match git(&wt, &["switch", "--detach", &commit]) {
            Ok(_) => say(format!(
                "left the worktree {}: it's the runner's (now on {short}, detached)",
                wt.display()
            )),
            Err(e) => notes.push(format!(
                "the runner's worktree {} is still on {branch}: {e}",
                wt.display()
            )),
        }
    } else {
        match git(&repo.main, &["worktree", "remove", utf8(&wt)?]) {
            Ok(_) => say(format!("removed the worktree {}", wt.display())),
            Err(e) => notes.push(format!("the worktree {} is still there: {e}", wt.display())),
        }
    }
    match git(&repo.main, &["branch", "-d", &branch]) {
        Ok(_) => say(format!("deleted the branch {branch}")),
        Err(e) => notes.push(format!("the branch {branch} is still there: {e}")),
    }
    if has_origin {
        // Only origin's copy that is this very commit: anything else is work this merge didn't
        // take, so it stays.
        let head = format!("refs/heads/{branch}");
        match git(&repo.main, &["ls-remote", "origin", &head]) {
            Ok(listed) => {
                let remote_tip = listed.split_whitespace().next().unwrap_or("").to_string();
                if remote_tip.is_empty() {
                } else if remote_tip == local_tip {
                    match git(&repo.main, &["push", "origin", "--delete", &branch]) {
                        Ok(_) => say(format!("deleted origin's {branch}")),
                        Err(e) => notes.push(format!("origin's {branch} is still there: {e}")),
                    }
                } else {
                    notes.push(format!(
                        "origin's {branch} is at {} and the merged one was at {}, so it's kept",
                        remote_tip.chars().take(7).collect::<String>(),
                        local_tip.chars().take(7).collect::<String>()
                    ));
                }
            }
            Err(e) => notes.push(format!("couldn't look for origin's {branch}: {e}")),
        }
    }

    // 7. The merge is on main: the task records it whatever happens to the pass.
    let path = ops::pass_merged(root, id, who, &now(), &commit, &report)?;

    // 8. What's next.
    let state = queue::read_state(
        &[("q".into(), root.to_path_buf())],
        &mut queue::Cache::default(),
    );
    let next = next_after(&state.tasks, id).map(|t| (t.id.clone(), t.title.clone()));
    Ok(Merge::Merged {
        commit,
        report,
        path,
        notes,
        next,
    })
}

// ---------------------------------------------------------------------------------------------
// Release

/// `muster release <id> --clean [--force]`: back to Ready, with its worktree removed and its
/// branch deleted (the local one: a branch on origin is pushed work, and stays). First it lists
/// what that would lose (uncommitted files, commits on no other branch); with any of that it
/// refuses unless `force`. Returns the lines to show.
pub fn release(
    root: &Path,
    id: &str,
    who: &str,
    stamp: &str,
    force: bool,
    cwd: Option<&Path>,
) -> Result<Vec<String>, String> {
    ops::check_release(root, id, who)?;
    let t = queue::read_task(root, id)?;
    task_branch(id, &task_key(root, id), &t.branch)?;
    let wt = PathBuf::from(&t.worktree);
    let present = !t.worktree.is_empty() && wt.is_dir();
    let repo = if present {
        repo_of(&wt)?
    } else {
        let gone = |e: String| format!("{id}'s worktree {} is gone, and {e}", t.worktree);
        let cwd = cwd.ok_or_else(|| gone("the current folder can't be read".into()))?;
        repo_of(cwd).map_err(|e| gone(format!("{e}: run release from the project's repo")))?
    };
    if present && canon(&wt) == canon(&repo.main) {
        return Err(format!(
            "{id}'s worktree is the main checkout; release never removes that"
        ));
    }
    // A runner's worktree is never removed, so what's uncommitted in it isn't lost.
    let runner = present && t.worktree_runner;
    let dirty = if present && !runner {
        uncommitted(&wt)?
    } else {
        Vec::new()
    };
    let commits = unpushed(&repo, &t.branch)?;
    let mut lines = Vec::new();
    for f in &dirty {
        lines.push(format!("uncommitted: {f}"));
    }
    for c in &commits {
        lines.push(format!("unpushed commit: {c}"));
    }
    if !lines.is_empty() && !force {
        lines.push(format!(
            "{id}: releasing would lose the work above; commit and push it, or run muster release {id} --clean --force to throw it away"
        ));
        return Err(lines.join("\n"));
    }
    let mut removed = Vec::new();
    if runner {
        // Let go of the branch (detached where it is, uncommitted files carried along) so the
        // branch can go; the worktree is its runner's to remove.
        git(&wt, &["switch", "--detach"])?;
        lines.push(format!(
            "left the worktree {}: it's the runner's (now detached)",
            wt.display()
        ));
    } else if present {
        let w = utf8(&wt)?;
        let args: Vec<&str> = if dirty.is_empty() {
            vec!["worktree", "remove", w]
        } else {
            vec!["worktree", "remove", "--force", w]
        };
        git(&repo.main, &args)?;
        removed.push(format!("the worktree {}", wt.display()));
    } else {
        git(&repo.main, &["worktree", "prune"])?;
    }
    if branch_exists(&repo, &t.branch) {
        git(&repo.main, &["branch", "-D", &t.branch])?;
        removed.push(format!("the branch {}", t.branch));
    }
    let what = if removed.is_empty() {
        "nothing left to remove".to_string()
    } else {
        format!("removed {}", removed.join(" and "))
    };
    let path = ops::release_task(root, id, who, stamp, &what)?;
    lines.push(format!("{id}: back in Ready ({path}); {what}"));
    Ok(lines)
}

// ---------------------------------------------------------------------------------------------
// Stale worktrees and claims (`muster check`)

/// One entry of `git worktree list --porcelain`: its folder and branch (if on one).
fn worktrees(repo: &Repo) -> Vec<(PathBuf, String)> {
    let Ok(text) = git(&repo.main, &["worktree", "list", "--porcelain"]) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut path: Option<PathBuf> = None;
    for line in text.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            path = Some(PathBuf::from(p));
        } else if let Some(b) = line.strip_prefix("branch refs/heads/") {
            if let Some(p) = path.take() {
                out.push((p, b.to_string()));
            }
        }
    }
    out
}

/// The id of the task a branch muster made is for: `task/mu-12-…` (0033, whatever the prefix) or
/// `task/0012-…` from before keys. A project's prefix can change after a start, so only the
/// number names the task.
fn branch_task(branch: &str) -> Option<String> {
    let rest = branch.strip_prefix("task/")?;
    if let Some(id) = rest
        .get(..4)
        .filter(|i| is_id(i) && rest.get(4..5) == Some("-"))
    {
        return Some(id.to_string());
    }
    let (prefix, after) = rest.split_once('-')?;
    let (number, _) = after.split_once('-')?;
    crate::key::parse(&format!("{prefix}-{number}"))
        .filter(|t| t.prefix.is_some())
        .map(|t| t.id)
}

/// What `muster check` lists for 0023, none of it failing the check: claims whose worktree is gone,
/// worktrees of tasks released to Ready, and worktrees on a `task/<key>-…` branch whose task is
/// past Testing (or no longer names that branch), each with the command that cleans it. The
/// repos looked in are those of the tasks' worktrees, plus `extra` (the repo `check` runs in).
/// Tasks are named by their keys when they have their project's prefix.
pub fn stale(tasks: &[Task], extra: &[PathBuf]) -> Vec<String> {
    let key = |t: &Task| crate::key::key(&t.prefix, &t.id);
    let mut out = Vec::new();
    let mut repos: Vec<Repo> = Vec::new();
    let mut add = |r: Repo| {
        if !repos.iter().any(|x| canon(&x.main) == canon(&r.main)) {
            repos.push(r);
        }
    };
    for t in tasks.iter().filter(|t| !t.branch.is_empty()) {
        let wt = Path::new(&t.worktree);
        let present = !t.worktree.is_empty() && wt.is_dir();
        match (t.stage.as_str(), present) {
            ("building" | "testing", false) => out.push(format!(
                "{}: its worktree {} is gone ({}, branch {}): muster release {} --clean",
                key(t),
                t.worktree,
                queue::stage_label(&t.stage),
                t.branch,
                key(t)
            )),
            ("ready", true) => out.push(format!(
                "{}: released to Ready but its worktree {} is still there: muster release {} --clean (or muster start {} to pick it up again)",
                key(t),
                t.worktree,
                key(t),
                key(t)
            )),
            ("ready", false) => out.push(format!(
                "{}: released to Ready with its worktree gone: muster release {} --clean",
                key(t),
                key(t)
            )),
            _ => {}
        }
        if present {
            if let Ok(r) = repo_of(wt) {
                add(r);
            }
        }
    }
    for dir in extra {
        if let Ok(r) = repo_of(dir) {
            add(r);
        }
    }
    for repo in &repos {
        for (path, branch) in worktrees(repo) {
            let Some(id) = branch_task(&branch) else {
                continue;
            };
            let Some(t) = tasks.iter().find(|t| t.id == id) else {
                continue;
            };
            let current =
                t.branch == branch && matches!(t.stage.as_str(), "ready" | "building" | "testing");
            if current {
                continue;
            }
            out.push(format!(
                "stale worktree {} (branch {branch}): {} is {}: git -C {} worktree remove {}, then git -C {} branch -d {branch}",
                path.display(),
                key(t),
                queue::stage_label(&t.stage),
                repo.main.display(),
                path.display(),
                repo.main.display()
            ));
        }
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::ops::{NewTask, PassOpts};
    use std::os::unix::fs::PermissionsExt;

    const T0: &str = "2026-10-13 10:00";

    /// `git -C <dir> <args>` in a test, which must work.
    pub(crate) fn g(dir: &Path, args: &[&str]) -> String {
        let out = git_output(dir, args).unwrap();
        assert!(
            out.status.success(),
            "git {args:?} in {}: {}",
            dir.display(),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// A temp repo's own settings, so the owner's global git config (signing, hooks, merge.ff)
    /// can't change what the tests see.
    fn configure(repo: &Path, hooks: &Path) {
        for (k, v) in [
            ("user.name", "Test Person"),
            ("user.email", "test@example.com"),
            ("commit.gpgsign", "false"),
            ("tag.gpgsign", "false"),
            ("merge.ff", "true"),
            ("pull.rebase", "false"),
            ("rebase.autoStash", "false"),
            ("core.hooksPath", hooks.to_str().unwrap()),
        ] {
            g(repo, &["config", k, v]);
        }
    }

    /// `<base>/proj/tasks` (a queue), `<base>/proj/main` (its repo, on main, with `origin` a bare
    /// repo at `<base>/origin.git`), all in a temp folder.
    pub(crate) struct Fx {
        _dir: tempfile::TempDir,
        pub(crate) base: PathBuf,
        pub(crate) root: PathBuf,
        pub(crate) repo: PathBuf,
        pub(crate) origin: PathBuf,
        hooks: PathBuf,
    }

    pub(crate) fn fx() -> Fx {
        let dir = tempfile::tempdir().unwrap();
        let base = canon(dir.path());
        let root = base.join("proj/tasks");
        for f in queue::STATUS_FOLDERS {
            fs::create_dir_all(root.join(f)).unwrap();
        }
        // Its prefix stored (0033), so branch names don't hang on the projects this Mac knows.
        fs::write(root.join("README.md"), "---\nprefix: pj\n---\nq").unwrap();
        let hooks = base.join("no-hooks");
        fs::create_dir(&hooks).unwrap();
        let origin = base.join("origin.git");
        fs::create_dir(&origin).unwrap();
        g(&origin, &["init", "-q", "--bare", "-b", "main"]);
        g(
            &origin,
            &["config", "core.hooksPath", hooks.to_str().unwrap()],
        );
        let repo = base.join("proj/main");
        fs::create_dir_all(&repo).unwrap();
        g(&repo, &["init", "-q", "-b", "main"]);
        configure(&repo, &hooks);
        fs::write(repo.join("README.md"), "one\ntwo\nthree\n").unwrap();
        fs::write(repo.join(".gitignore"), "target/\n").unwrap();
        g(&repo, &["add", "-A"]);
        g(&repo, &["commit", "-qm", "first"]);
        g(
            &repo,
            &["remote", "add", "origin", origin.to_str().unwrap()],
        );
        g(&repo, &["push", "-q", "-u", "origin", "main"]);
        Fx {
            _dir: dir,
            base,
            root,
            repo,
            origin,
            hooks,
        }
    }

    /// A Ready task with its one criterion ticked, touching `touches`.
    pub(crate) fn task(root: &Path, title: &str, touches: &str) -> String {
        ops::create_task(
            root,
            &NewTask {
                title: title.into(),
                body: "## Acceptance criteria\n- [x] AC1: works — verify: command".into(),
                touches: touches.into(),
                ..Default::default()
            },
            "owner",
            T0,
        )
        .unwrap()
        .id
    }

    fn read(root: &Path, id: &str) -> Task {
        queue::read_task(root, id).unwrap()
    }

    /// A script that prints a line, then runs `out`, and exits with `code`: a gate.
    pub(crate) fn script(dir: &Path, name: &str, out: &str, code: i32) -> Vec<String> {
        let p = dir.join(name);
        fs::write(
            &p,
            format!("#!/bin/sh\necho 'running the checks'\n{out}\nexit {code}\n"),
        )
        .unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        vec![p.to_str().unwrap().to_string()]
    }

    pub(crate) fn green(f: &Fx) -> Vec<String> {
        script(
            &f.base,
            "gate-green",
            "echo 'gate: green (fake, 2 steps)'",
            0,
        )
    }

    /// Commit `text` to `file` in `dir`.
    pub(crate) fn commit(dir: &Path, file: &str, text: &str, msg: &str) {
        if let Some(p) = Path::new(file).parent() {
            fs::create_dir_all(dir.join(p)).unwrap();
        }
        fs::write(dir.join(file), text).unwrap();
        g(dir, &["add", "-A"]);
        g(dir, &["commit", "-qm", msg]);
    }

    pub(crate) fn to_testing(f: &Fx, id: &str) {
        let opts = PassOpts {
            gate: "gate: green (built)".into(),
            from: Some("building".into()),
            ..Default::default()
        };
        ops::pass_task(&f.root, id, "wt-a", "2026-10-13 11:00", &opts).unwrap();
    }

    /// Start `id` as wt-a, commit a change in its worktree, and pass it Building → Testing.
    fn built(f: &Fx, id: &str, file: &str) -> Started {
        let s = start(&f.root, id, "wt-a", T0, &f.repo, &Place::Default).unwrap();
        commit(&s.worktree, file, "built\n", "build it");
        to_testing(f, id);
        s
    }

    fn noon() -> String {
        "2026-10-13 12:30".into()
    }

    fn run_merge(f: &Fx, id: &str, gate: &[String]) -> (Result<Merge, String>, Vec<String>) {
        let mut said = Vec::new();
        let r = merge(&f.root, id, "qa", &noon, gate, &mut |l| said.push(l));
        (r, said)
    }

    fn has_branch(f: &Fx, b: &str) -> bool {
        git(
            &f.repo,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("refs/heads/{b}"),
            ],
        )
        .is_ok()
    }

    fn state(root: &Path) -> queue::State {
        queue::read_state(
            &[("q".into(), root.to_path_buf())],
            &mut queue::Cache::default(),
        )
    }

    fn ids(v: &[&Task]) -> Vec<String> {
        v.iter().map(|t| t.id.clone()).collect()
    }

    #[test]
    fn touches_overlap_by_their_fixed_prefix() {
        let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(fixed_prefix("app/src/TaskPanel*"), "app/src/TaskPanel");
        assert_eq!(fixed_prefix("docs/**/x.md"), "docs/");
        assert_eq!(fixed_prefix("a/[bc]d"), "a/");
        assert_eq!(fixed_prefix("a/{b,c}"), "a/");
        assert_eq!(fixed_prefix("*.rs"), "");
        assert_eq!(fixed_prefix("bin/gate"), "bin/gate");
        let overlap = |a: &[&str], b: &[&str]| touches_overlap(&v(a), &v(b));
        assert!(overlap(&["app/src"], &["app/src/TaskPanel*"]));
        assert!(overlap(
            &["app/src/TaskPanel*"],
            &["app/src/TaskPanel.test.ts"]
        ));
        assert!(overlap(&["docs/PROTOCOL.md"], &["docs/*"]));
        assert!(
            overlap(&["**/*.rs"], &["bin/task"]),
            "a leading wildcard overlaps all"
        );
        assert!(
            overlap(&["app/src"], &["app/src-tauri"]),
            "plain text: the safe side"
        );
        assert!(overlap(&["bin", "docs/A.md"], &["x", "docs/A.md"]));
        assert!(!overlap(&["app/src/TaskPanel*"], &["app/src/Card*"]));
        assert!(!overlap(&["bin/gate"], &["docs/"]));
        assert!(!overlap(&[], &["docs/"]), "an empty list overlaps nothing");
    }

    #[test]
    fn ready_parallel_never_pairs_dependent_overlapping_or_unsaid_tasks() {
        let f = fx();
        let r = &f.root;
        let a = task(r, "Panel", "app/src/TaskPanel*");
        let b = task(r, "Card", "app/src/Card*");
        let c = task(r, "Panel test", "app/src/TaskPanel.test.ts");
        let d = task(r, "Docs", "docs/");
        let e = task(r, "No touches", "");
        let dep = task(r, "After docs", "bin/");
        ops::set_field(r, &dep, "depends", &d, "owner", T0).unwrap();
        let human = task(r, "Buy the domain", "infra/");
        ops::set_field(r, &human, "for", "human", "owner", T0).unwrap();
        let blocked = task(r, "Blocked", "x/");
        ops::block_task(r, &blocked, "owner: which?", "owner", T0).unwrap();
        let claimed = task(r, "Claimed", "y/");
        ops::claim_task(r, &claimed, "wt-z", T0).unwrap();
        ops::set_field(r, &b, "priority", "high", "owner", T0).unwrap();
        let s = state(r);
        // `ready`: claimable only, high first; the human, blocked and claimed ones aren't, nor
        // the one whose dependency isn't merged.
        assert_eq!(ids(&ready(&s.tasks)), [b.as_str(), &a, &c, &d, &e]);
        let pick = parallel_set(&s.tasks, 3, &[]);
        assert_eq!(ids(&pick.set), [b.as_str(), &a, &d]);
        let left: Vec<(String, String)> = pick
            .left
            .iter()
            .map(|(t, w)| (t.id.clone(), w.clone()))
            .collect();
        assert_eq!(
            left,
            [
                (c.clone(), format!("touches overlap {a}")),
                (e.clone(), "the set is full (3)".to_string()),
            ]
        );
        // Room for more: the one without touches never pairs.
        let pick = parallel_set(&s.tasks, 10, &[]);
        assert_eq!(ids(&pick.set), [b.as_str(), &a, &d]);
        let why = |id: &str| {
            pick.left
                .iter()
                .find(|(t, _)| t.id == id)
                .unwrap()
                .1
                .clone()
        };
        assert_eq!(why(&e), "no touches: says nothing of where it works");
        // A task never goes beside one it depends on, or one that depends on it.
        let by = |id: &str| s.tasks.iter().find(|t| t.id == id).unwrap();
        assert!(!compatible(by(&dep), by(&d)) && !compatible(by(&d), by(&dep)));
        assert!(compatible(by(&dep), by(&a)));
        assert!(!compatible(by(&e), by(&a)) && !compatible(by(&a), by(&e)));
        assert_eq!(clash(by(&dep), by(&d)), Some(format!("depends on {d}")));
        assert_eq!(
            clash(by(&d), by(&dep)),
            Some(format!("{dep} depends on it"))
        );
        // First in line and alone, a task without touches is fine, and nothing pairs with it.
        ops::set_field(r, &e, "priority", "high", "owner", T0).unwrap();
        ops::set_field(r, &b, "priority", "normal", "owner", T0).unwrap();
        let s = state(r);
        assert_eq!(ids(&parallel_set(&s.tasks, 3, &[]).set), [e.as_str()]);
        // Beside running tasks: none that clashes with them, and none at all beside one that
        // says nothing of where it works.
        let running_a: Vec<&Task> = s.tasks.iter().filter(|t| t.id == a).collect();
        let set = ids(&parallel_set(&s.tasks, 3, &running_a).set);
        assert!(
            !set.contains(&c) && !set.contains(&e) && !set.is_empty(),
            "{set:?}"
        );
        let running_e: Vec<&Task> = s.tasks.iter().filter(|t| t.id == e).collect();
        assert!(parallel_set(&s.tasks, 3, &running_e).set.is_empty());
        // A goal is never in it; done dependencies free a task.
        let goal = ops::create_task(
            r,
            &NewTask {
                title: "Ship".into(),
                level: "goal".into(),
                touches: "z/".into(),
                ..Default::default()
            },
            "owner",
            T0,
        )
        .unwrap()
        .id;
        ops::set_stage(r, &d, "done", None, "owner", T0).unwrap();
        let s = state(r);
        let all = ids(&ready(&s.tasks));
        assert!(!all.contains(&goal) && all.contains(&dep), "{all:?}");
        // After a merge: the next one that doesn't clash with what's still running.
        ops::claim_task(r, &a, "wt-a", T0).unwrap();
        let s = state(r);
        assert_eq!(
            next_after(&s.tasks, "9999").map(|t| t.id.clone()),
            Some(b.clone())
        );
    }

    #[test]
    fn ready_parallel_keeps_clear_of_the_tasks_already_running() {
        let f = fx();
        let r = &f.root;
        let run = task(r, "Running", "app/src");
        let testing = task(r, "In testing", "bin/");
        let near = task(r, "Overlaps the running one", "app/src/Card*");
        let after = task(r, "Overlaps the tested one", "bin/gate");
        let free = task(r, "Free", "docs/");
        ops::claim_task(r, &run, "wt-a", T0).unwrap();
        ops::claim_task(r, &testing, "wt-b", T0).unwrap();
        let opts = PassOpts {
            gate: "gate: green".into(),
            from: Some("building".into()),
            ..Default::default()
        };
        ops::pass_task(r, &testing, "wt-b", T0, &opts).unwrap();
        let s = state(r);
        let pick = ready_parallel(&s.tasks, 3);
        assert_eq!(ids(&pick.set), [free.as_str()]);
        let why = |id: &str| {
            pick.left
                .iter()
                .find(|(t, _)| t.id == id)
                .unwrap()
                .1
                .clone()
        };
        assert_eq!(why(&near), format!("touches overlap {run}"));
        assert_eq!(why(&after), format!("touches overlap {testing}"));
    }

    #[test]
    fn ready_parallel_and_start_count_a_merged_dependency_as_met() {
        let f = fx();
        let r = &f.root;
        let base = task(r, "Base", "lib/");
        let testing = task(r, "Still in testing", "t/");
        let after = task(r, "After the base", "app/");
        let waits = task(r, "After the one in testing", "docs/");
        ops::set_field(r, &after, "depends", &base, "owner", T0).unwrap();
        ops::set_field(r, &waits, "depends", &testing, "owner", T0).unwrap();
        let gate = PassOpts {
            gate: "gate: green".into(),
            from: Some("building".into()),
            ..Default::default()
        };
        for id in [&base, &testing] {
            ops::claim_task(r, id, "wt-x", T0).unwrap();
            ops::pass_task(r, id, "wt-x", T0, &gate).unwrap();
        }
        // The base merged, in Your check (0042): what waits on it is free; Testing isn't merged.
        ops::pass_task(r, &base, "qa", T0, &PassOpts::default()).unwrap();
        let s = state(r);
        let all = ids(&ready(&s.tasks));
        assert!(all.contains(&after) && !all.contains(&waits), "{all:?}");
        // Sent back: not claimable, and start refuses before it makes a branch or a worktree.
        ops::return_task(r, &base, "redo it", Some("building"), "owner", T0).unwrap();
        let s = state(r);
        assert!(!ids(&ready(&s.tasks)).contains(&after));
        let e = start(r, &after, "wt-a", T0, &f.repo, &Place::Default).unwrap_err();
        assert_eq!(
            e,
            format!("{after} depends on {base}, not merged yet (an agent's task counts once it's in Your check, the owner's own once it's Done); pick another (muster ready --parallel)")
        );
        assert_eq!(g(&f.repo, &["branch", "--list", "task/*"]), "");
        assert!(!f.base.join("proj/worktrees").exists());
        // Merged again: start takes it.
        ops::pass_task(r, &base, "wt-x", T0, &gate).unwrap();
        ops::pass_task(r, &base, "qa", T0, &PassOpts::default()).unwrap();
        start(r, &after, "wt-a", T0, &f.repo, &Place::Default).unwrap();
    }

    #[test]
    fn start_makes_the_branch_and_worktree_and_records_them() {
        let f = fx();
        let id = task(&f.root, "Board flickers on resize", "app/src/Board*");
        let s = start(&f.root, &id, "wt-a", T0, &f.repo, &Place::Default).unwrap();
        let wt = f
            .base
            .join("proj/worktrees/main/pj-1-board-flickers-on-resize");
        assert_eq!(
            (s.worktree.clone(), s.branch.as_str(), s.resumed),
            (wt.clone(), "task/pj-1-board-flickers-on-resize", false)
        );
        assert!(wt.join("README.md").is_file(), "a checkout of main");
        assert_eq!(g(&wt, &["rev-parse", "--abbrev-ref", "HEAD"]), s.branch);
        assert_eq!(
            g(&wt, &["rev-parse", "HEAD"]),
            g(&f.repo, &["rev-parse", "main"])
        );
        let t = read(&f.root, &id);
        assert_eq!(
            (
                t.stage.as_str(),
                t.claimed_by.as_str(),
                t.worktree.as_str(),
                t.branch.as_str()
            ),
            ("building", "wt-a", wt.to_str().unwrap(), s.branch.as_str())
        );
        let body = queue::read_body(&f.root, &id).unwrap();
        assert!(
            body.contains("wt-a: stage ready → building (claimed)"),
            "{body}"
        );
        assert!(body.contains(&format!(
            "wt-a: started in worktree {} on branch {}",
            wt.display(),
            s.branch
        )));
        // Again, by its holder: picked up there, nothing new made.
        let again = start(&f.root, &id, "wt-a", T0, &f.repo, &Place::Default).unwrap();
        assert!(again.resumed);
        assert_eq!(again.worktree, wt);
        // Elsewhere: refused.
        let e = start(
            &f.root,
            &id,
            "wt-a",
            T0,
            &f.repo,
            &Place::At(f.base.join("x")),
        )
        .unwrap_err();
        assert!(e.contains("already has a worktree"), "{e}");
        // A second start by the same holder: the claim is refused, and the fresh worktree and
        // branch are taken back.
        let other = task(&f.root, "Second", "docs/");
        let e = start(&f.root, &other, "wt-a", T0, &f.repo, &Place::Default).unwrap_err();
        assert!(e.contains("you already hold 0001"), "{e}");
        assert!(!f.base.join("proj/worktrees/main/0002-second").exists());
        assert!(!has_branch(&f, "task/pj-2-second"));
        assert_eq!(read(&f.root, &other).stage, "ready");
        // --at, relative to where it runs.
        let s2 = start(
            &f.root,
            &other,
            "wt-b",
            T0,
            &f.repo,
            &Place::At("../elsewhere".into()),
        )
        .unwrap();
        assert_eq!(s2.worktree, f.base.join("proj/elsewhere"));
        assert!(s2.worktree.join("README.md").is_file());
        // Not Ready, a human task, a branch that's there already: refused.
        let h = task(&f.root, "Human", "h/");
        ops::set_field(&f.root, &h, "for", "human", "owner", T0).unwrap();
        assert!(start(&f.root, &h, "owner", T0, &f.repo, &Place::Default)
            .unwrap_err()
            .contains("owner's to do"));
        let third = task(&f.root, "Third", "t/");
        g(&f.repo, &["branch", "task/pj-4-third"]);
        let e = start(&f.root, &third, "wt-c", T0, &f.repo, &Place::Default).unwrap_err();
        assert!(e.contains("already exists"), "{e}");
        assert_eq!(read(&f.root, &third).stage, "ready");
        // Outside a repo: refused, nothing claimed.
        let fourth = task(&f.root, "Fourth", "f/");
        let e = start(&f.root, &fourth, "wt-d", T0, &f.root, &Place::Default).unwrap_err();
        assert!(e.contains("runs in the project's repo"), "{e}");
        assert_eq!(read(&f.root, &fourth).stage, "ready");
    }

    #[test]
    fn start_here_uses_the_linked_worktree_it_runs_in() {
        let f = fx();
        let id = task(&f.root, "Lane", "lane/");
        // The main checkout isn't a runner's worktree.
        let e = start(&f.root, &id, "wt-a", T0, &f.repo, &Place::Here).unwrap_err();
        assert!(e.contains("is the main checkout"), "{e}");
        let lane = f.base.join("lane");
        g(
            &f.repo,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "lane-1",
                lane.to_str().unwrap(),
            ],
        );
        fs::write(lane.join("wip.txt"), "x").unwrap();
        let e = start(&f.root, &id, "wt-a", T0, &lane, &Place::Here).unwrap_err();
        assert!(e.contains("uncommitted changes"), "{e}");
        fs::remove_file(lane.join("wip.txt")).unwrap();
        let s = start(&f.root, &id, "wt-a", T0, &lane, &Place::Here).unwrap();
        assert_eq!(s.worktree, lane);
        assert_eq!(
            g(&lane, &["rev-parse", "--abbrev-ref", "HEAD"]),
            "task/pj-1-lane"
        );
        assert_eq!(read(&f.root, &id).worktree, lane.to_str().unwrap());
        // A refused claim switches the lane back.
        let other = task(&f.root, "Other", "o/");
        let lane2 = f.base.join("lane2");
        g(
            &f.repo,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "lane-2",
                lane2.to_str().unwrap(),
            ],
        );
        let e = start(&f.root, &other, "wt-a", T0, &lane2, &Place::Here).unwrap_err();
        assert!(e.contains("you already hold"), "{e}");
        assert_eq!(g(&lane2, &["rev-parse", "--abbrev-ref", "HEAD"]), "lane-2");
        assert!(!has_branch(&f, "task/pj-2-other"));
    }

    #[test]
    fn a_pass_from_building_of_a_started_task_works_as_before() {
        let f = fx();
        let id = task(&f.root, "Built", "b/");
        let s = built(&f, &id, "b/x.txt");
        let t = read(&f.root, &id);
        assert_eq!((t.stage.as_str(), t.held_by.as_str()), ("testing", "wt-a"));
        // Nothing pushed, nothing merged, the worktree stays for Testing's fixes.
        assert!(s.worktree.is_dir());
        assert_eq!(
            g(&f.origin, &["for-each-ref", "--format=%(refname)"]),
            "refs/heads/main"
        );
        assert_eq!(
            g(&f.repo, &["rev-parse", "main"]),
            g(&f.origin, &["rev-parse", "main"])
        );
        // The app's (or any plain) pass from Testing is refused: that pass is the merge.
        let e = ops::pass_task(
            &f.root,
            &id,
            "owner",
            T0,
            &PassOpts {
                from: Some("testing".into()),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(e.contains("merges it into main"), "{e}");
        assert_eq!(read(&f.root, &id).stage, "testing");
    }

    #[test]
    fn a_clean_merge_lands_on_main_pushes_cleans_up_reports_and_names_the_next() {
        let f = fx();
        let id = task(&f.root, "Merge me", "m/");
        let next = task(&f.root, "Next up", "n/");
        let s = start(&f.root, &id, "wt-a", T0, &f.repo, &Place::Default).unwrap();
        ops::checkin_task(&f.root, &id, "halfway", "50", "wt-a", "2026-10-13 10:30").unwrap();
        commit(&s.worktree, "m/x.txt", "built\n", "build it");
        to_testing(&f, &id);
        // Build folders (ignored) go with the worktree; a branch on origin goes too.
        fs::create_dir_all(s.worktree.join("target/debug")).unwrap();
        fs::write(s.worktree.join("target/debug/big"), "x").unwrap();
        g(&s.worktree, &["push", "-q", "origin", &s.branch]);
        let tip = g(&s.worktree, &["rev-parse", "HEAD"]);
        let (r, said) = run_merge(&f, &id, &green(&f));
        let Merge::Merged {
            commit: merged,
            report,
            notes,
            next: n,
            ..
        } = r.unwrap()
        else {
            panic!("not merged")
        };
        assert_eq!(merged, tip, "a fast-forward");
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(g(&f.repo, &["rev-parse", "main"]), tip);
        assert_eq!(g(&f.origin, &["rev-parse", "main"]), tip, "pushed");
        assert!(!s.worktree.exists(), "worktree removed");
        assert!(!has_branch(&f, &s.branch));
        assert_eq!(
            g(&f.origin, &["for-each-ref", "--format=%(refname)"]),
            "refs/heads/main"
        );
        let t = read(&f.root, &id);
        assert_eq!(
            (
                t.stage.as_str(),
                t.worktree.as_str(),
                t.branch.as_str(),
                t.commit.as_str()
            ),
            ("your-check", "", "", merged.as_str())
        );
        assert_eq!(
            report,
            format!(
                "merged {} into main as {} (fast-forward, pushed to origin) · gate: green (fake, 2 steps) · Building 1h 0m · Testing 1h 30m · 1 check-in",
                s.branch,
                &merged[..7]
            )
        );
        assert_eq!(t.merge_report, report);
        assert_eq!(n, Some((next.clone(), "Next up".into())));
        assert!(
            said.iter().any(|l| l == "pushed main to origin"),
            "{said:?}"
        );
        assert!(
            said.iter().any(|l| l.starts_with("removed the worktree")),
            "{said:?}"
        );
    }

    /// A gate that runs `body` (shell lines) before saying green.
    fn shell_gate(f: &Fx, name: &str, body: &str) -> Vec<String> {
        script(
            &f.base,
            name,
            &format!("{body}\necho 'gate: green ({name})'"),
            0,
        )
    }

    /// Lines that commit `file` to the main checkout, once (`once` set) or on every run.
    fn owner_commits(f: &Fx, file: &str, text: &str, once: bool) -> String {
        let marker = f.base.join(format!("{file}.done"));
        let guard = if once {
            format!("[ -e '{m}' ] || {{ touch '{m}'; ", m = marker.display())
        } else {
            "{ ".to_string()
        };
        format!(
            "{guard}( cd '{}' && printf '{text}' >> {file} && git add {file} && git commit -qm owner{file} ) >/dev/null; }}",
            f.repo.display()
        )
    }

    #[test]
    fn main_moving_during_the_gate_rebases_and_gates_again_and_lands_only_fast_forwards() {
        let f = fx();
        // Once: rebased and gated again, then a fast-forward onto the owner's commit.
        let id = task(&f.root, "Moves once", "s/");
        let s = built(&f, &id, "s/y.txt");
        let gate = shell_gate(&f, "once", &owner_commits(&f, "z.txt", "z", true));
        let (r, said) = run_merge(&f, &id, &gate);
        let Merge::Merged {
            commit: merged,
            report,
            ..
        } = r.unwrap()
        else {
            panic!("not merged: {said:?}")
        };
        assert!(
            said.iter()
                .any(|l| l.contains("main moved during the gate")),
            "{said:?}"
        );
        assert!(
            report.contains("(fast-forward, pushed to origin)"),
            "{report}"
        );
        assert_eq!(
            g(&f.repo, &["log", "-1", "--format=%s", &merged]),
            "build it"
        );
        assert_eq!(
            g(
                &f.repo,
                &["log", "-1", "--format=%s", &format!("{merged}^")]
            ),
            "ownerz.txt"
        );
        assert_eq!(
            g(&f.repo, &["rev-list", "--merges", "--count", "main"]),
            "0"
        );
        assert_eq!(g(&f.origin, &["rev-parse", "main"]), merged);
        assert!(!s.worktree.exists());
        // Every time: it stops after rebasing and gating twice more, nothing merged or deleted.
        let id = task(&f.root, "Moves always", "t/");
        let s = built(&f, &id, "t/y.txt");
        let gate = shell_gate(&f, "always", &owner_commits(&f, "w.txt", "w$$", false));
        let (r, _) = run_merge(&f, &id, &gate);
        let e = r.unwrap_err();
        assert!(e.starts_with("main keeps moving; pass again"), "{e}");
        assert_eq!(read(&f.root, &id).stage, "testing");
        assert!(s.worktree.is_dir() && has_branch(&f, &s.branch));
        assert!(!g(&f.repo, &["branch", "--contains", &s.branch]).contains("main"));
        // A conflicting commit on main during the gate: the second rebase sends it back.
        let id = task(&f.root, "Conflicts later", "README.md");
        let s = start(&f.root, &id, "wt-a", T0, &f.repo, &Place::Default).unwrap();
        commit(&s.worktree, "README.md", "one\nTASK\nthree\n", "task edit");
        to_testing(&f, &id);
        let main_moved = f.base.join("conflict-gate");
        fs::write(
            &main_moved,
            format!(
                "#!/bin/sh\n[ -e '{m}.done' ] || {{ touch '{m}.done'; ( cd '{r}' && printf 'one\\nMAIN\\nthree\\n' > README.md && git commit -qam 'main edit' ) >/dev/null; }}\necho 'gate: green'\n",
                m = main_moved.display(),
                r = f.repo.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&main_moved, fs::Permissions::from_mode(0o755)).unwrap();
        let (r, _) = run_merge(&f, &id, &[main_moved.to_str().unwrap().to_string()]);
        let Merge::Returned(why) = r.unwrap() else {
            panic!("not returned")
        };
        assert!(
            why.starts_with("rebase on main conflicts in README.md"),
            "{why}"
        );
        assert_eq!(read(&f.root, &id).stage, "building");
        assert!(!f.repo.join(".git/MERGE_HEAD").exists());
        let rebasing = g(&s.worktree, &["rev-parse", "--git-path", "rebase-merge"]);
        assert!(
            !s.worktree.join(rebasing).exists(),
            "no rebase left in progress"
        );
        assert_eq!(
            g(&f.repo, &["status", "--porcelain", "--untracked-files=no"]),
            ""
        );
        assert_eq!(g(&s.worktree, &["status", "--porcelain"]), "");
        assert!(s.worktree.is_dir() && has_branch(&f, &s.branch));
    }

    #[test]
    fn a_merge_that_lands_is_recorded_even_when_the_pass_is_refused_meanwhile() {
        let f = fx();
        let id = task(&f.root, "Blocked midway", "b/");
        let s = built(&f, &id, "b/x.txt");
        // The gate blocks the task while it runs (as `muster block` would).
        let file = f.root.join(read(&f.root, &id).path);
        let gate = shell_gate(
            &f,
            "blocks",
            &format!(
                "sed -i '' 's/^waiting_on:$/waiting_on: owner: which colour?/' '{}'",
                file.display()
            ),
        );
        let tip = g(&s.worktree, &["rev-parse", "HEAD"]);
        let e = run_merge(&f, &id, &gate).0.unwrap_err();
        assert!(
            e.starts_with(&format!(
                "merged into main as {}; the task is Testing",
                &tip[..7]
            )),
            "{e}"
        );
        assert!(e.contains("muster stage") && !e.contains("release"), "{e}");
        assert_eq!(g(&f.repo, &["rev-parse", "main"]), tip);
        assert_eq!(g(&f.origin, &["rev-parse", "main"]), tip);
        assert!(!s.worktree.exists() && !has_branch(&f, &s.branch));
        let t = read(&f.root, &id);
        assert_eq!(
            (
                t.stage.as_str(),
                t.blocked,
                t.worktree.as_str(),
                t.branch.as_str(),
                t.commit.as_str()
            ),
            ("testing", true, "", "", tip.as_str())
        );
        assert!(t.merge_report.starts_with(&format!(
            "merged {} into main as {}",
            s.branch,
            &tip[..7]
        )));
        // Nothing for check to send to release --clean.
        assert!(stale(&state(&f.root).tasks, std::slice::from_ref(&f.repo)).is_empty());
    }

    #[test]
    fn merge_facts_are_checked_before_the_merge() {
        assert!(
            ops::check_merge_facts("qa", T0, &"a".repeat(64), "merged task/0001-x into main")
                .is_ok()
        );
        assert!(ops::check_merge_facts("qa", T0, &"a".repeat(40), "merged task/0001-x").is_ok());
        assert!(ops::check_merge_facts("qa", T0, &"a".repeat(65), "merged task/0001-x").is_err());
        assert!(ops::check_merge_facts("qa", T0, "abc", "merged task/0001-x").is_err());
        assert!(ops::check_merge_facts("qa", T0, &"a".repeat(7), "stage testing → done").is_err());
    }

    #[test]
    fn an_untracked_file_in_main_that_the_branch_adds_stops_it_and_keeps_everything() {
        let f = fx();
        let id = task(&f.root, "Adds a file", "new/");
        let s = built(&f, &id, "new/file.txt");
        fs::create_dir_all(f.repo.join("new")).unwrap();
        fs::write(f.repo.join("new/file.txt"), "the owner's own\n").unwrap();
        let head = g(&f.repo, &["rev-parse", "HEAD"]);
        let e = run_merge(&f, &id, &green(&f)).0.unwrap_err();
        assert!(e.contains("Nothing was merged or deleted"), "{e}");
        assert_eq!(g(&f.repo, &["rev-parse", "HEAD"]), head);
        assert_eq!(
            fs::read_to_string(f.repo.join("new/file.txt")).unwrap(),
            "the owner's own\n"
        );
        assert_eq!(read(&f.root, &id).stage, "testing");
        assert!(s.worktree.is_dir() && has_branch(&f, &s.branch));
        assert!(!f.repo.join(".git/MERGE_HEAD").exists());
    }

    #[test]
    fn with_no_origin_it_merges_says_so_and_cleans_up() {
        let f = fx();
        g(&f.repo, &["remote", "remove", "origin"]);
        let id = task(&f.root, "Local only", "l/");
        let s = built(&f, &id, "l/x.txt");
        let (r, said) = run_merge(&f, &id, &green(&f));
        let Merge::Merged { report, .. } = r.unwrap() else {
            panic!("not merged")
        };
        assert!(
            report.contains("(fast-forward, not pushed: no origin)"),
            "{report}"
        );
        assert!(
            said.iter()
                .any(|l| l == "no origin remote: main isn't pushed"),
            "{said:?}"
        );
        assert!(!s.worktree.exists() && !has_branch(&f, &s.branch));
        assert_eq!(read(&f.root, &id).stage, "your-check");
    }

    #[test]
    fn a_runners_worktree_is_never_removed_only_its_branch() {
        let f = fx();
        let lane = f.base.join("lane");
        g(
            &f.repo,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "lane-1",
                lane.to_str().unwrap(),
            ],
        );
        let id = task(&f.root, "In a lane", "lane/");
        let s = start(&f.root, &id, "wt-a", T0, &lane, &Place::Here).unwrap();
        assert!(read(&f.root, &id).worktree_runner);
        commit(&lane, "lane/x.txt", "x\n", "lane work");
        to_testing(&f, &id);
        let (r, said) = run_merge(&f, &id, &green(&f));
        let Merge::Merged { notes, .. } = r.unwrap() else {
            panic!("not merged")
        };
        assert!(notes.is_empty(), "{notes:?}");
        assert!(
            said.iter()
                .any(|l| l.contains("left the worktree") && l.contains("the runner's")),
            "{said:?}"
        );
        assert!(
            lane.join("lane/x.txt").is_file(),
            "the runner's worktree stays"
        );
        assert!(!has_branch(&f, &s.branch));
        assert!(!read(&f.root, &id).worktree_runner);
        // release --clean leaves a runner's worktree too, and its uncommitted files.
        let id = task(&f.root, "Given up in a lane", "lane2/");
        let s = start(&f.root, &id, "wt-a", T0, &lane, &Place::Here).unwrap();
        fs::write(lane.join("wip.txt"), "mine").unwrap();
        let lines = release(&f.root, &id, "wt-a", T0, false, None).unwrap();
        assert!(lines.last().unwrap().contains("back in Ready"), "{lines:?}");
        assert!(lane.join("wip.txt").is_file() && !has_branch(&f, &s.branch));
    }

    #[test]
    fn a_new_prefix_still_finds_the_branch_a_task_was_started_on() {
        let f = fx();
        let id = task(&f.root, "Renamed later", "r/");
        let s = start(&f.root, &id, "wt-a", T0, &f.repo, &Place::Default).unwrap();
        assert_eq!(s.branch, "task/pj-1-renamed-later");
        let done = task(&f.root, "Done by hand later", "d/");
        let sd = start(&f.root, &done, "wt-b", T0, &f.repo, &Place::Default).unwrap();
        ops::set_stage(&f.root, &done, "done", None, "owner", T0).unwrap();
        crate::key::store_prefix(&f.root, "pk").unwrap();
        // check still knows whose worktree each is, by the task's number.
        let keyed = queue::read_state_keyed(
            &[("p".into(), f.root.clone())],
            &mut queue::Cache::default(),
            &crate::settings::Settings::default(),
            &|_| "pk".to_string(),
        );
        let lines = stale(&keyed.tasks, std::slice::from_ref(&f.repo));
        assert_eq!(lines.len(), 1, "{lines:#?}");
        assert!(
            lines[0].starts_with(&format!(
                "stale worktree {} (branch {}): pk-2 is Done",
                sd.worktree.display(),
                sd.branch
            )),
            "{lines:#?}"
        );
        let lines = release(&f.root, &id, "wt-a", T0, false, None).unwrap();
        assert!(lines.last().unwrap().contains("back in Ready"), "{lines:?}");
        assert!(!has_branch(&f, &s.branch));
    }

    #[test]
    fn origins_task_branch_is_deleted_only_when_it_matches_the_local_one() {
        let f = fx();
        let id = task(&f.root, "Pushed early", "p/");
        let s = start(&f.root, &id, "wt-a", T0, &f.repo, &Place::Default).unwrap();
        commit(&s.worktree, "p/x.txt", "x\n", "first");
        g(&s.worktree, &["push", "-q", "origin", &s.branch]);
        commit(&s.worktree, "p/y.txt", "y\n", "second");
        to_testing(&f, &id);
        let (r, _) = run_merge(&f, &id, &green(&f));
        let Merge::Merged { notes, .. } = r.unwrap() else {
            panic!("not merged")
        };
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(
            notes[0].contains("origin's task/pj-1-pushed-early") && notes[0].contains("kept"),
            "{notes:?}"
        );
        assert!(git(
            &f.origin,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("refs/heads/{}", s.branch)
            ]
        )
        .is_ok());
    }

    #[test]
    fn the_gate_never_sees_the_queue_variables() {
        let cmd = gate_command(Path::new("bin/gate"), &["bin/gate".into()], Path::new("/x"));
        let removed: Vec<_> = cmd
            .get_envs()
            .filter(|(_, v)| v.is_none())
            .map(|(k, _)| k.to_string_lossy().to_string())
            .collect();
        assert!(
            removed.contains(&"MUSTER_ROOT".to_string())
                && removed.contains(&"TASKS_ROOT".to_string()),
            "{removed:?}"
        );
    }

    #[test]
    fn a_rebase_conflict_returns_it_to_building_naming_the_files_and_deletes_nothing() {
        let f = fx();
        let id = task(&f.root, "Conflict", "README.md");
        let s = start(&f.root, &id, "wt-a", T0, &f.repo, &Place::Default).unwrap();
        commit(
            &s.worktree,
            "README.md",
            "one\nTWO (task)\nthree\n",
            "task edit",
        );
        to_testing(&f, &id);
        commit(
            &f.repo,
            "README.md",
            "one\nTWO (main)\nthree\n",
            "main edit",
        );
        let main_before = g(&f.repo, &["rev-parse", "main"]);
        let tip = g(&s.worktree, &["rev-parse", "HEAD"]);
        let (r, _) = run_merge(&f, &id, &green(&f));
        let Merge::Returned(why) = r.unwrap() else {
            panic!("not returned")
        };
        assert!(
            why.starts_with(
                "rebase on main conflicts in README.md: rebase task/pj-1-conflict on main"
            ),
            "{why}"
        );
        let t = read(&f.root, &id);
        assert_eq!(
            (t.stage.as_str(), t.claimed_by.as_str(), t.branch.as_str()),
            ("building", "wt-a", s.branch.as_str())
        );
        assert!(queue::read_body(&f.root, &id).unwrap().contains(
            "qa: stage testing → building (returned: rebase on main conflicts in README.md"
        ));
        // Nothing deleted, the rebase undone, main untouched.
        assert!(s.worktree.is_dir() && has_branch(&f, &s.branch));
        assert_eq!(g(&s.worktree, &["rev-parse", "HEAD"]), tip);
        assert_eq!(g(&s.worktree, &["status", "--porcelain"]), "");
        assert_eq!(g(&f.repo, &["rev-parse", "main"]), main_before);
    }

    #[test]
    fn a_red_gate_after_the_rebase_returns_it_with_the_gate_line_and_merges_nothing() {
        let f = fx();
        let id = task(&f.root, "Red", "r/");
        let s = built(&f, &id, "r/x.txt");
        let main_before = g(&f.repo, &["rev-parse", "main"]);
        // Exit 1, or a RED line even with exit 0: both are red.
        for (name, line, code) in [
            ("red1", "echo 'gate: RED (cargo test)'", 1),
            ("red2", "echo 'gate: RED (lint)'", 0),
        ] {
            if read(&f.root, &id).stage == "building" {
                to_testing(&f, &id);
            }
            let gate = script(&f.base, name, line, code);
            let (r, said) = run_merge(&f, &id, &gate);
            let Merge::Returned(why) = r.unwrap() else {
                panic!("not returned")
            };
            assert!(
                why.starts_with("gate after the rebase on main: gate: RED ("),
                "{why}"
            );
            assert!(
                said.iter().any(|l| l.contains("running the checks")),
                "the gate's output is shown: {said:?}"
            );
            assert_eq!(read(&f.root, &id).stage, "building");
            assert!(s.worktree.is_dir() && has_branch(&f, &s.branch));
            assert_eq!(g(&f.repo, &["rev-parse", "main"]), main_before);
        }
    }

    #[test]
    fn a_dirty_or_moved_main_checkout_stops_the_merge_with_everything_left() {
        let f = fx();
        let id = task(&f.root, "Waits", "w/");
        let s = built(&f, &id, "w/x.txt");
        fs::write(f.repo.join("README.md"), "changed by the owner\n").unwrap();
        let e = run_merge(&f, &id, &green(&f)).0.unwrap_err();
        assert!(e.contains("main checkout isn't clean"), "{e}");
        g(&f.repo, &["checkout", "--", "README.md"]);
        // An untracked file doesn't count; another branch does.
        fs::write(f.repo.join("notes.txt"), "mine").unwrap();
        g(&f.repo, &["switch", "-q", "-c", "owner-work"]);
        let e = run_merge(&f, &id, &green(&f)).0.unwrap_err();
        assert!(e.contains("main checkout isn't on main"), "{e}");
        assert_eq!(read(&f.root, &id).stage, "testing");
        assert!(s.worktree.is_dir() && has_branch(&f, &s.branch));
        // Uncommitted work in the worktree stops it too.
        g(&f.repo, &["switch", "-q", "main"]);
        fs::write(s.worktree.join("w/new.txt"), "wip").unwrap();
        let e = run_merge(&f, &id, &green(&f)).0.unwrap_err();
        assert!(e.contains("uncommitted changes"), "{e}");
        fs::remove_file(s.worktree.join("w/new.txt")).unwrap();
        // Back to normal, it merges.
        assert!(matches!(
            run_merge(&f, &id, &green(&f)).0.unwrap(),
            Merge::Merged { .. }
        ));
    }

    #[test]
    fn a_rejected_push_fetches_says_so_and_stops_without_forcing_or_deleting() {
        let f = fx();
        let id = task(&f.root, "Pushy", "p/");
        let s = built(&f, &id, "p/x.txt");
        // Someone else pushed to origin's main first.
        let other = f.base.join("other");
        g(
            &f.base,
            &[
                "clone",
                "-q",
                f.origin.to_str().unwrap(),
                other.to_str().unwrap(),
            ],
        );
        configure(&other, &f.hooks);
        commit(&other, "theirs.txt", "theirs\n", "theirs");
        g(&other, &["push", "-q", "origin", "main"]);
        let theirs = g(&other, &["rev-parse", "HEAD"]);
        let (r, said) = run_merge(&f, &id, &green(&f));
        let e = r.unwrap_err();
        assert!(
            e.starts_with("origin moved; rebase main and pass again"),
            "{e}"
        );
        assert!(said.iter().any(|l| l.starts_with("merged ")), "{said:?}");
        assert_eq!(g(&f.origin, &["rev-parse", "main"]), theirs, "never forced");
        assert_eq!(g(&f.repo, &["rev-parse", "origin/main"]), theirs, "fetched");
        assert_eq!(read(&f.root, &id).stage, "testing");
        assert!(s.worktree.is_dir() && has_branch(&f, &s.branch));
        // The owner rebases main on origin, then the pass goes through.
        g(&f.repo, &["rebase", "-q", "origin/main"]);
        let (r, _) = run_merge(&f, &id, &green(&f));
        assert!(matches!(r.unwrap(), Merge::Merged { .. }));
        assert_eq!(
            g(&f.origin, &["rev-parse", "main"]),
            g(&f.repo, &["rev-parse", "main"])
        );
        assert!(!s.worktree.exists());
    }

    #[test]
    fn merge_refuses_what_a_pass_would_refuse_before_touching_git() {
        let f = fx();
        let id = ops::create_task(
            &f.root,
            &NewTask {
                title: "Unticked".into(),
                body: "## Acceptance criteria\n- [ ] AC1: not yet — verify: command".into(),
                ..Default::default()
            },
            "owner",
            T0,
        )
        .unwrap()
        .id;
        let s = start(&f.root, &id, "wt-a", T0, &f.repo, &Place::Default).unwrap();
        commit(&s.worktree, "u.txt", "u\n", "u");
        to_testing(&f, &id);
        let main_before = g(&f.repo, &["rev-parse", "main"]);
        let e = run_merge(&f, &id, &green(&f)).0.unwrap_err();
        assert!(e.contains("acceptance criteria aren't ticked"), "{e}");
        assert_eq!(g(&f.repo, &["rev-parse", "main"]), main_before);
        assert!(run_merge(&f, &id, &[]).0.is_err(), "no gate, no merge");
        // Not in Testing: refused.
        let plain = task(&f.root, "Plain", "");
        assert!(run_merge(&f, &plain, &green(&f))
            .0
            .unwrap_err()
            .contains("not Testing"));
    }

    #[test]
    fn the_builders_merge_is_refused_before_the_rebase_the_gate_or_the_merge() {
        let f = fx();
        let id = task(&f.root, "Self-tested", "s/");
        let s = built(&f, &id, "s/x.txt");
        // Main moves on, so a rebase would rewrite the branch.
        commit(&f.repo, "other.txt", "other\n", "main moves");
        let main_before = g(&f.repo, &["rev-parse", "main"]);
        let origin_before = g(&f.origin, &["rev-parse", "main"]);
        let tip = g(&s.worktree, &["rev-parse", "HEAD"]);
        let file = f.root.join(read(&f.root, &id).path);
        let before = fs::read_to_string(&file).unwrap();
        let ran = f.base.join("gate-ran");
        let gate = shell_gate(&f, "marks", &format!("touch '{}'", ran.display()));
        let mut said = Vec::new();
        let e = merge(&f.root, &id, "wt-a", &noon, &gate, &mut |l| said.push(l)).unwrap_err();
        assert_eq!(
            e,
            format!("{id} was built by wt-a; a different session tests it: run the QA pass with its own --as (e.g. qa-{id}), or the owner passes it")
        );
        assert!(said.is_empty(), "{said:?}");
        assert!(!ran.exists(), "the gate never ran");
        assert_eq!(g(&s.worktree, &["rev-parse", "HEAD"]), tip, "not rebased");
        assert_eq!(g(&f.repo, &["rev-parse", "main"]), main_before);
        assert_eq!(g(&f.origin, &["rev-parse", "main"]), origin_before);
        assert!(s.worktree.is_dir() && has_branch(&f, &s.branch));
        assert_eq!(fs::read_to_string(&file).unwrap(), before);
        // A QA session merges it.
        let merged = merge(&f.root, &id, "qa-0001", &noon, &green(&f), &mut |_| {});
        assert!(matches!(merged, Ok(Merge::Merged { .. })), "{merged:?}");
        let t = read(&f.root, &id);
        assert_eq!(
            (t.built_by.as_str(), t.tested_by.as_str()),
            ("wt-a", "qa-0001")
        );
    }

    #[test]
    fn the_merges_own_pass_refuses_the_builder_too() {
        let f = fx();
        let id = task(&f.root, "Recorded", "r/");
        built(&f, &id, "r/x.txt");
        let e =
            ops::pass_merged(&f.root, &id, "wt-a", T0, &"a".repeat(40), "merged it").unwrap_err();
        assert!(
            e.contains("the pass was refused: 0001 was built by wt-a;"),
            "{e}"
        );
        assert_eq!(read(&f.root, &id).stage, "testing");
    }

    #[test]
    fn release_lists_what_would_be_lost_and_refuses_it_without_force() {
        let f = fx();
        let id = task(&f.root, "Abandon", "a/");
        let s = start(&f.root, &id, "wt-a", T0, &f.repo, &Place::Default).unwrap();
        fs::write(s.worktree.join("wip.txt"), "x").unwrap();
        let e = release(&f.root, &id, "wt-a", T0, false, None).unwrap_err();
        assert!(e.contains("uncommitted: ?? wip.txt"), "{e}");
        assert!(e.contains("--clean --force"), "{e}");
        g(&s.worktree, &["add", "-A"]);
        g(&s.worktree, &["commit", "-qm", "work in progress"]);
        let e = release(&f.root, &id, "wt-a", T0, false, None).unwrap_err();
        assert!(
            e.contains("unpushed commit: ") && e.contains("work in progress"),
            "{e}"
        );
        assert!(s.worktree.is_dir() && has_branch(&f, &s.branch));
        // Someone else's claim isn't theirs to release.
        assert!(release(&f.root, &id, "wt-b", T0, true, None)
            .unwrap_err()
            .contains("claimed by wt-a"));
        // Pushed work isn't lost: the branch on origin keeps it.
        g(&s.worktree, &["push", "-q", "origin", &s.branch]);
        let lines = release(&f.root, &id, "wt-a", T0, false, None).unwrap();
        assert!(lines.last().unwrap().contains("back in Ready"), "{lines:?}");
        assert!(!s.worktree.exists() && !has_branch(&f, &s.branch));
        assert!(
            git(
                &f.origin,
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    &format!("refs/heads/{}", s.branch)
                ]
            )
            .is_ok(),
            "origin's stays"
        );
        let t = read(&f.root, &id);
        assert_eq!(
            (
                t.stage.as_str(),
                t.claimed_by.as_str(),
                t.worktree.as_str(),
                t.branch.as_str()
            ),
            ("ready", "", "", "")
        );
        assert!(queue::read_body(&f.root, &id)
            .unwrap()
            .contains("wt-a: stage building → ready (released)"));
        // --force throws away what's shown; the owner may release anyone's.
        g(&f.repo, &["fetch", "-q", "origin"]);
        g(&f.repo, &["push", "-q", "origin", "--delete", &s.branch]);
        let s = start(&f.root, &id, "wt-b", T0, &f.repo, &Place::Default).unwrap();
        commit(&s.worktree, "lost.txt", "x\n", "lost");
        fs::write(s.worktree.join("also.txt"), "y").unwrap();
        let lines = release(&f.root, &id, "owner", T0, true, None).unwrap();
        assert!(
            lines.iter().any(|l| l.contains("unpushed commit")),
            "{lines:?}"
        );
        assert!(!s.worktree.exists() && !has_branch(&f, &s.branch));
        // Nothing on record: refused.
        assert!(release(&f.root, &id, "owner", T0, true, None)
            .unwrap_err()
            .contains("no worktree or branch on record"));
    }

    #[test]
    fn check_lists_stale_worktrees_and_claims_whose_worktree_is_gone() {
        let f = fx();
        let gone = task(&f.root, "Gone", "g/");
        let sg = start(&f.root, &gone, "wt-a", T0, &f.repo, &Place::Default).unwrap();
        g(
            &f.repo,
            &["worktree", "remove", sg.worktree.to_str().unwrap()],
        );
        let back = task(&f.root, "Back", "b/");
        let sb = start(&f.root, &back, "wt-b", T0, &f.repo, &Place::Default).unwrap();
        ops::return_task(&f.root, &back, "not now", Some("ready"), "owner", T0).unwrap();
        let done = task(&f.root, "Done by hand", "d/");
        let sd = start(&f.root, &done, "wt-c", T0, &f.repo, &Place::Default).unwrap();
        ops::set_stage(&f.root, &done, "done", None, "owner", T0).unwrap();
        let live = task(&f.root, "Live", "l/");
        start(&f.root, &live, "wt-d", T0, &f.repo, &Place::Default).unwrap();
        let lines = stale(&state(&f.root).tasks, &[]);
        assert_eq!(lines.len(), 3, "{lines:#?}");
        assert_eq!(
            lines[0],
            format!(
                "{gone}: its worktree {} is gone (Building, branch task/pj-1-gone): muster release {gone} --clean",
                sg.worktree.display()
            )
        );
        assert!(
            lines[1].starts_with(&format!(
                "{back}: released to Ready but its worktree {} is still there: muster release {back} --clean",
                sb.worktree.display()
            )),
            "{}",
            lines[1]
        );
        assert!(
            lines[2].starts_with(&format!(
                "stale worktree {} (branch task/pj-3-done-by-hand): {done} is Done: git -C {} worktree remove",
                sd.worktree.display(),
                f.repo.display()
            )),
            "{}",
            lines[2]
        );
        // Each one's command cleans it.
        release(&f.root, &gone, "wt-a", T0, false, Some(&f.repo)).unwrap();
        release(&f.root, &back, "owner", T0, false, None).unwrap();
        g(
            &f.repo,
            &["worktree", "remove", sd.worktree.to_str().unwrap()],
        );
        g(&f.repo, &["branch", "-D", "task/pj-3-done-by-hand"]);
        assert_eq!(
            stale(&state(&f.root).tasks, std::slice::from_ref(&f.repo)),
            Vec::<String>::new()
        );
    }

    #[test]
    fn the_gate_runs_as_words_in_the_worktree_and_reads_its_line() {
        let d = tempfile::tempdir().unwrap();
        let dir = canon(d.path());
        fs::create_dir(dir.join("bin")).unwrap();
        // A repo script by its relative path, with arguments as given, run in the folder.
        let p = dir.join("bin/gate");
        fs::write(
            &p,
            "#!/bin/sh\npwd\necho \"args: $1|$2\"\necho 'gate: green (2 steps)'\necho done\n",
        )
        .unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        let r = run_gate(&["bin/gate".into(), "a b".into(), "$HOME".into()], &dir).unwrap();
        assert!(r.green);
        assert_eq!(r.line, "gate: green (2 steps)");
        assert!(
            r.tail
                .contains(&format!("{}\nargs: a b|$HOME", dir.display())),
            "{}",
            r.tail
        );
        // No line: one made from the exit.
        let r = run_gate(&["true".into()], &dir).unwrap();
        assert_eq!((r.green, r.line.as_str()), (true, "gate: green (true)"));
        let r = run_gate(&["false".into()], &dir).unwrap();
        assert!(
            !r.green && r.line.starts_with("gate: RED (false exited with"),
            "{}",
            r.line
        );
        // A RED line is red whatever the exit; an exit that isn't 0 is red whatever the line.
        let red = script(&dir, "red", "echo 'gate full: RED (clippy)'", 0);
        assert!(!run_gate(&red, &dir).unwrap().green);
        let liar = script(&dir, "liar", "echo 'gate: green'", 3);
        assert!(!run_gate(&liar, &dir).unwrap().green);
        // The line is stdout's last, stderr's only when stdout has none; RED anywhere is red.
        let mixed = script(
            &dir,
            "mixed",
            "echo 'gate: RED (x)'\necho 'gate: green (y)' >&2",
            0,
        );
        let r = run_gate(&mixed, &dir).unwrap();
        assert_eq!((r.green, r.line.as_str()), (false, "gate: RED (x)"));
        let both = script(
            &dir,
            "both",
            "echo 'gate: green (a)'\necho 'gate: green (b)' >&2",
            0,
        );
        assert_eq!(run_gate(&both, &dir).unwrap().line, "gate: green (a)");
        let err_red = script(
            &dir,
            "errred",
            "echo 'gate: green (a)'\necho 'gate: RED (z)' >&2",
            0,
        );
        let r = run_gate(&err_red, &dir).unwrap();
        assert_eq!(
            (r.green, r.line.as_str()),
            (false, "gate: RED (z)"),
            "a red run never reports a green line"
        );
        let only_err = script(&dir, "onlyerr", "echo 'gate: green (e)' >&2", 0);
        assert_eq!(run_gate(&only_err, &dir).unwrap().line, "gate: green (e)");
        assert!(run_gate(&[], &dir).is_err());
        assert!(run_gate(&["bin/missing".into()], &dir)
            .unwrap_err()
            .contains("couldn't run the gate"));
    }

    #[test]
    fn time_in_a_stage_from_its_history_and_other_small_rules() {
        let s = |at: &str, from: &str, to: &str| StageStep {
            at: at.into(),
            who: "x".into(),
            from: from.into(),
            to: to.into(),
        };
        let h = [
            s("2026-10-13 09:00", "ready", "building"),
            s("2026-10-13 10:30", "building", "testing"),
            s("2026-10-13 10:45", "testing", "building"),
            s("2026-10-13 11:15", "building", "testing"),
        ];
        assert_eq!(minutes_in(&h, "building", "2026-10-13 12:00"), 120);
        assert_eq!(minutes_in(&h, "testing", "2026-10-13 12:00"), 15 + 45);
        assert_eq!(minutes_in(&h, "shaping", "2026-10-13 12:00"), 0);
        assert_eq!(duration_text(120), "2h 0m");
        assert_eq!(duration_text(35), "35m");
        assert_eq!(name_files(&["a".into(), "b".into()]), "a, b");
        let many: Vec<String> = (0..40)
            .map(|i| format!("src/some/long/path/file{i}.rs"))
            .collect();
        let named = name_files(&many);
        assert!(named.ends_with(" more") && named.len() < 300, "{named}");
        assert!(task_branch("0001", "mu-1", "task/mu-1-x").is_ok());
        assert!(
            task_branch("0001", "mu-1", "task/0001-x").is_ok(),
            "from before keys"
        );
        assert!(
            task_branch("0001", "mus-1", "task/mu-1-x").is_ok(),
            "from before a new prefix"
        );
        for bad in [
            "main",
            "task/0002-x",
            "task/mu-2-x",
            "task/mu-10-x",
            "task/0001-x y",
            "task/0001-../main",
            "task/mu-1-../main",
        ] {
            assert!(task_branch("0001", "mu-1", bad).is_err(), "{bad}");
        }
        assert_eq!(branch_task("task/mu-12-board").as_deref(), Some("0012"));
        assert_eq!(branch_task("task/mus-12-board").as_deref(), Some("0012"));
        assert_eq!(branch_task("task/0012-board").as_deref(), Some("0012"));
        for not in [
            "main",
            "task/mu-board",
            "task/mu-12",
            "task/m-1-x",
            "feature/mu-1-x",
        ] {
            assert_eq!(branch_task(not), None, "{not}");
        }
    }
}
