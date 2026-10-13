//! Where a queue keeps what (task 0005, decision 0003). Two layouts are read and written:
//!
//! - The old one, `<project>/tasks` beside the repo with its own git: task files in status
//!   folders (`inbox/ todo/ doing/ blocked/ done/`, v2) or in the queue folder itself (flat),
//!   with `status:` written as a mirror of the stage. Drover's queue stays on it.
//! - The new one, `<repo>/.muster/`: task files in `tasks/`, flat, never moved; `stage:` is the
//!   truth and no `status:` is written. Backlog notes in `backlog/` (`backlog/dropped/`),
//!   `attachments/NNNN/`, `mail/`, `INDEX.md`, `tasks.json`, and `config.toml`.
//!
//! A queue is in the new layout when its folder is named `.muster`. This module is the one place
//! that says where a layout keeps its task files, and how a folder finds its repo's `.muster/`.

use crate::queue::STATUS_FOLDERS;
use std::path::{Path, PathBuf};

/// The new layout's folder, in a repo's main checkout.
pub const DIR: &str = ".muster";
/// Its task files' folder.
const TASKS: &str = "tasks";

/// Is this queue in the new layout (its folder is `.muster`)?
pub fn is_muster(root: &Path) -> bool {
    root.file_name().is_some_and(|n| n == DIR)
}

/// Is this an old queue with status folders (v2)? Otherwise it's flat, or `.muster/`.
pub fn has_status_folders(root: &Path) -> bool {
    !is_muster(root) && root.join("todo").is_dir()
}

/// Does the layout write `status:`, the stage's mirror for older readers (Drover, the old
/// `manifest.py`)? Only the old one: in `.muster/` the stage is the only truth.
pub fn keeps_status(root: &Path) -> bool {
    !is_muster(root)
}

/// The folders a queue's task files are read from, each with the status folder it stands for
/// ("" when the file alone says): `.muster/tasks/`, or the old queue folder itself (flat) and its
/// status folders.
pub fn task_dirs(root: &Path) -> Vec<(&'static str, PathBuf)> {
    if is_muster(root) {
        return vec![("", root.join(TASKS))];
    }
    let mut out = vec![("", root.to_path_buf())];
    out.extend(STATUS_FOLDERS.iter().map(|f| (*f, root.join(f))));
    out
}

/// Where a task file goes for this status mirror: `.muster/tasks/` whatever it is (files never
/// move there), its status folder in the v2 layout, else the queue folder (flat).
pub fn task_dir(root: &Path, status: &str) -> PathBuf {
    if is_muster(root) {
        root.join(TASKS)
    } else if has_status_folders(root) {
        root.join(status)
    } else {
        root.to_path_buf()
    }
}

/// The status folder a task file is in: "" in `.muster/tasks/` and in the flat layout.
pub fn folder_of(root: &Path, path: &Path) -> String {
    if is_muster(root) {
        return String::new();
    }
    match path.parent() {
        Some(p) if p != root => p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// The new layout's settings file, in `.muster/`.
pub const CONFIG: &str = "config.toml";

/// Is `dir` a queue in the new layout: a `.muster` folder with its `config.toml`?
pub fn is_queue(dir: &Path) -> bool {
    is_muster(dir) && dir.join(CONFIG).is_file()
}

/// What a project's own checkout is called: `<project>/main` (or `master`) holds the repo.
const MAIN_NAMES: [&str; 2] = ["main", "master"];

/// Is this `.muster`'s checkout a project's `main` or `master`?
fn checkout_is_main(root: &Path) -> bool {
    root.parent()
        .and_then(Path::file_name)
        .and_then(|n| n.to_str())
        .is_some_and(|n| MAIN_NAMES.contains(&n))
}

/// A `.muster` queue's name: its project folder's when the checkout is the project's `main` or
/// `master` (`~/playground/muster/main/.muster` is `muster`, as `~/playground/muster/tasks` was),
/// else the checkout's own: a repo that is a project of its own (`~/playground/chit/.muster` is
/// `chit`, `~/playground/harness/harness40/.muster` is `harness40`).
pub fn project_name(root: &Path) -> Option<String> {
    let checkout = root.parent()?;
    let named = if checkout_is_main(root) {
        checkout.parent()?
    } else {
        checkout
    };
    named.file_name().map(|n| n.to_string_lossy().to_string())
}

/// The folder `muster start` puts a task's worktree in, for the repo named `repo`:
/// `<project>/worktrees/<repo>` for an old `<project>/tasks` and for a project's
/// `main`/`master` checkout, beside it; for a checkout that is a project of its own, the folder
/// `<checkout>.worktrees` beside it, never the folder that holds it (that may be `~/playground`).
pub fn worktrees_dir(root: &Path, repo: &str) -> Option<PathBuf> {
    let up = root.parent()?;
    if is_muster(root) && !checkout_is_main(root) {
        let name = up.file_name()?.to_string_lossy();
        return Some(up.parent()?.join(format!("{name}.worktrees")));
    }
    let project = if is_muster(root) { up.parent()? } else { up };
    Some(project.join("worktrees").join(repo))
}

/// `config.toml` as muster reads it: `local = true|false` (default false). Other keys are left
/// for whoever owns them (a project key prefix, later settings).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Config {
    /// The queue stays on this Mac: `.muster/.gitignore` keeps everything but `config.toml` out
    /// of git, and every worktree reads the main checkout's `.muster/`.
    pub local: bool,
}

/// `config.toml`'s top level (before any `[table]`): each `key = value` line as (its line
/// number, the key unquoted, the value with its comment off). Lines without `=` (the rest of
/// another tool's multi-line value) are passed over.
fn top_level(text: &str) -> Vec<(usize, &str, &str)> {
    let mut out = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') {
            break;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let key = key
            .strip_prefix('"')
            .and_then(|k| k.strip_suffix('"'))
            .unwrap_or(key);
        out.push((n, key, value.trim()));
    }
    out
}

/// The `prefix` (0033) in `config.toml`'s text, quoted or not, as written; None without one.
pub fn config_prefix_in(text: &str) -> Option<String> {
    top_level(text)
        .into_iter()
        .rev()
        .find(|(_, k, _)| *k == "prefix")
        .map(|(_, _, v)| {
            v.strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(v)
                .to_string()
        })
}

/// `config.toml`'s text with `prefix = "<p>"`: in place of the one there, else after the top
/// level's last line (before any `[table]`), the rest as it was.
pub fn with_config_prefix(text: &str, prefix: &str) -> String {
    let line = format!("prefix = \"{prefix}\"");
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let top = top_level(text);
    if let Some((n, _, _)) = top.iter().rev().find(|(_, k, _)| *k == "prefix") {
        lines[*n] = line;
    } else {
        let table = lines
            .iter()
            .position(|l| l.split('#').next().unwrap_or("").trim().starts_with('['));
        let at = table.unwrap_or(lines.len());
        lines.insert(at, line);
    }
    format!("{}\n", lines.join("\n"))
}

/// The prefix stored in a `.muster` queue's `config.toml` (read by `key::stored_prefix`).
pub fn config_prefix(root: &Path) -> Option<String> {
    config_prefix_in(&std::fs::read_to_string(root.join(CONFIG)).ok()?)
}

/// Store `prefix` in a `.muster` queue's `config.toml` (for `key::store_prefix`).
pub fn store_config_prefix(root: &Path, prefix: &str) -> Result<(), String> {
    let file = root.join(CONFIG);
    let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    crate::queue::write_atomic(&file, &with_config_prefix(&text, prefix))
}

/// `config.toml`'s text: `key = value` lines and comments; only the top level (before any
/// `[table]`) is muster's: `local` (quoted or not) here, and `prefix` (`config_prefix_in`) for the
/// task keys. Lines without `=` (the rest of another tool's multi-line value) are passed over.
pub fn parse_config(text: &str) -> Result<Config, String> {
    let mut config = Config::default();
    for (_, key, value) in top_level(text) {
        if key == "local" {
            config.local = match value.trim() {
                "true" => true,
                "false" => false,
                v => {
                    let v: String = v.chars().take(40).collect();
                    return Err(format!(
                        "config.toml: local must be true or false, got {v:?}"
                    ));
                }
            };
        }
    }
    Ok(config)
}

/// A queue's `config.toml`.
pub fn read_config(root: &Path) -> Result<Config, String> {
    let text =
        std::fs::read_to_string(root.join(CONFIG)).map_err(|e| format!("config.toml: {e}"))?;
    parse_config(&text)
}

/// `config.toml` as `muster init` and `migrate` write it, with the queue's prefix when given.
pub fn config_text(local: bool, prefix: Option<&str>) -> String {
    let prefix = prefix
        .map(|p| format!("# prefix: what this queue's task keys start with (mu in mu-12).\nprefix = \"{p}\"\n"))
        .unwrap_or_default();
    format!(
        "# Muster's settings for this project (decision 0003). Other tools may add their own keys.\n\
         # local = true: the queue stays on this Mac. .muster/.gitignore keeps everything but this\n\
         # file out of git, and every worktree of the repo uses the main checkout's .muster/.\n\
         local = {local}\n{prefix}"
    )
}

/// `.muster/.gitignore`: a local queue commits only `config.toml` (and this file); a committed
/// one leaves out what's per Mac (`mail/`, the lock) or rebuilt (`INDEX.md`, `tasks.json`).
pub fn gitignore_text(local: bool) -> &'static str {
    if local {
        "# Muster: a local queue (config.toml says local = true). Only config.toml is committed.\n*\n!.gitignore\n!config.toml\n"
    } else {
        "# Muster: per-Mac mail and the lists muster rebuilds stay out of git.\n/mail/\n/INDEX.md\n/tasks.json\n"
    }
}

/// What `muster check` says is wrong with a `.muster` queue's settings: a `config.toml` it can't
/// read, or a local queue whose `.gitignore` would let its tasks into git.
pub fn problems(queue: &str, root: &Path) -> Vec<String> {
    if !is_muster(root) {
        return Vec::new();
    }
    match read_config(root) {
        Err(e) => vec![format!("{queue}: {e}")],
        Ok(c) if c.local => {
            let ignore = std::fs::read_to_string(root.join(".gitignore")).unwrap_or_default();
            if ignore.lines().any(|l| l.trim() == "*") {
                Vec::new()
            } else {
                vec![format!(
                    "{queue}: config.toml says local = true but .muster/.gitignore doesn't keep the queue out of git; it should hold: {:?}",
                    gitignore_text(true)
                )]
            }
        }
        Ok(_) => Vec::new(),
    }
}

/// `git -C dir args`'s output, trimmed; None when git fails. The repo is the one at `dir`,
/// whatever GIT_* a caller set, and git never asks.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

/// The main checkout of the repo `dir` is in, from its main checkout or any linked worktree: the
/// folder holding git's common folder (`git rev-parse --git-common-dir`) when that's a `.git`
/// folder. None outside a repo, and where git's folder is something else (a bare repo's
/// worktree, a submodule, `--separate-git-dir`): there is no main checkout to hold `.muster/`,
/// and never git's own folder. `answer-hook.py` works out the same from the `.git` files.
pub fn main_checkout(dir: &Path) -> Option<PathBuf> {
    let common = git(
        dir,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    let common = PathBuf::from(common);
    if !common.file_name().is_some_and(|n| n == ".git") {
        return None;
    }
    let main = common.parent()?;
    common.is_dir().then(|| main.to_path_buf())
}

/// The `.muster/` queue a folder belongs to: in a repo, its main checkout's, from the checkout or
/// any worktree of it, so every worktree shares one queue (a worktree's own `.muster/` holds
/// only what's committed). Only outside any repo (no `.git` on the way up), a `.muster` folder
/// it's in or beside: in a repo git can't read, or one without a main checkout, there's none,
/// rather than a worktree's own copy. None when there's none: the old `<project>/tasks` may still
/// be the folder's queue.
pub fn find(cwd: &Path) -> Option<PathBuf> {
    if let Some(main) = main_checkout(cwd) {
        let q = main.join(DIR);
        return is_queue(&q).then_some(q);
    }
    if cwd.ancestors().any(|d| d.join(".git").exists()) {
        return None;
    }
    cwd.ancestors().find_map(|dir| {
        if is_queue(dir) {
            return Some(dir.to_path_buf());
        }
        let q = dir.join(DIR);
        is_queue(&q).then_some(q)
    })
}

/// Is this `.muster` queue one `queue::discover` lists: in a main checkout (`.git` is a folder)?
/// A worktree beside it (`.git` is a file) has its own `.muster/` with what's committed, but its
/// queue is the main checkout's.
pub fn found_queue(root: &Path) -> bool {
    is_queue(root) && root.parent().is_some_and(|c| c.join(".git").is_dir())
}

/// The group `queue::discover` gives `<base>/<group>/<checkout>/.muster`: none when the checkout
/// is a project's `main` or `master` (the queue is the project's), else `group` (the checkout is
/// a project of its own, like `harness/harness40`).
pub fn found_group(root: &Path, group: &str) -> String {
    if checkout_is_main(root) {
        String::new()
    } else {
        group.to_string()
    }
}

/// Finding a repo's `.muster/` from its main checkout and its worktrees, discovering queues, and
/// `muster init`, on temp repos with real `git worktree add`.
#[cfg(test)]
pub(crate) mod resolve {
    use super::*;
    use crate::ops::{self, NewTask};
    use std::fs;
    use std::process::Command;

    const T: &str = "2026-10-13 09:00";

    /// `git -C dir args`, as a test identity, without any GIT_* the test runs under.
    pub(crate) fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
            .args(args)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim_end().to_string()
    }

    /// `<base>/<project>/main`, a repo with one commit on main.
    pub(crate) fn repo(base: &Path, project: &str) -> PathBuf {
        let main = base.join(project).join("main");
        fs::create_dir_all(&main).unwrap();
        git(&main, &["init", "-q", "-b", "main"]);
        fs::write(main.join("README.md"), "code\n").unwrap();
        git(&main, &["add", "README.md"]);
        git(&main, &["commit", "-q", "-m", "first"]);
        main
    }

    fn canon(p: &Path) -> PathBuf {
        fs::canonicalize(p).unwrap()
    }

    #[test]
    fn a_local_queue_is_the_main_checkouts_from_every_worktree_and_stays_out_of_git() {
        let base = tempfile::tempdir().unwrap();
        let main = repo(base.path(), "proj");
        let q = crate::projects::init(&main, true, None, &[]).unwrap().0;
        assert_eq!(canon(&q), canon(&main.join(DIR)));
        assert_eq!(read_config(&q), Ok(Config { local: true }));
        // Only config.toml (and the .gitignore that says so) go into git.
        git(&main, &["add", "-A"]);
        git(&main, &["commit", "-q", "-m", "muster"]);
        assert_eq!(
            git(&main, &["ls-files", DIR]),
            ".muster/.gitignore\n.muster/config.toml"
        );
        // Worktrees beside main (Orca, wt-…) and inside it (Claude Code's), each with the
        // committed config.toml in its own `.muster/`: all find the main checkout's queue.
        let beside = base.path().join("proj/wt-1");
        git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "wt-1",
                beside.to_str().unwrap(),
            ],
        );
        let inside = main.join(".claude/worktrees/agent-x");
        git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "agent-x",
                inside.to_str().unwrap(),
            ],
        );
        assert!(beside.join(DIR).join(CONFIG).is_file());
        fs::create_dir_all(beside.join("src/deep")).unwrap();
        for dir in [
            main.clone(),
            beside.join("src/deep"),
            inside.clone(),
            q.join("tasks"),
        ] {
            fs::create_dir_all(&dir).unwrap();
            let found = find(&dir).unwrap_or_else(|| panic!("no queue from {}", dir.display()));
            assert_eq!(canon(&found), canon(&q), "from {}", dir.display());
        }
        // A task made from a worktree lands in main's queue, and git sees nothing of it.
        let from_wt = find(&beside).unwrap();
        let new = NewTask {
            title: "Shared".into(),
            ..Default::default()
        };
        let c = ops::create_task(&from_wt, &new, "owner", T).unwrap();
        assert!(q.join(&c.path).is_file());
        crate::backlog::add(&from_wt, "A note", "", "wt-1", T).unwrap();
        assert!(q.join("tasks").is_dir() && q.join("backlog").is_dir());
        assert_eq!(
            git(
                &main,
                &["status", "--porcelain", "--untracked-files=all", "--", DIR]
            ),
            ""
        );
        assert!(!beside.join(DIR).join("tasks").exists());
    }

    #[test]
    fn two_worktrees_claiming_one_task_at_once_one_wins() {
        let base = tempfile::tempdir().unwrap();
        let main = repo(base.path(), "proj");
        let q = crate::projects::init(&main, true, None, &[]).unwrap().0;
        let wts: Vec<PathBuf> = ["wt-a", "wt-b"]
            .iter()
            .map(|n| {
                let p = base.path().join("proj").join(n);
                git(
                    &main,
                    &["worktree", "add", "-q", "-b", n, p.to_str().unwrap()],
                );
                p
            })
            .collect();
        for round in 0..10 {
            let id = format!("{:04}", round + 1);
            let new = NewTask {
                title: format!("Task {round}"),
                ..Default::default()
            };
            ops::create_task(&q, &new, "owner", T).unwrap();
            let runs: Vec<_> = wts
                .iter()
                .map(|wt| {
                    let (wt, id) = (wt.clone(), id.clone());
                    std::thread::spawn(move || {
                        let root = find(&wt).unwrap();
                        let who = wt.file_name().unwrap().to_string_lossy().to_string();
                        ops::claim_task(&root, &id, &format!("{who}-{id}"), T)
                    })
                })
                .collect();
            let results: Vec<_> = runs.into_iter().map(|h| h.join().unwrap()).collect();
            assert_eq!(
                results.iter().filter(|r| r.is_ok()).count(),
                1,
                "{results:?}"
            );
        }
    }

    #[test]
    fn a_muster_queue_keeps_its_prefix_in_config_toml_never_in_a_readme() {
        let base = tempfile::tempdir().unwrap();
        let main = repo(base.path(), "proj");
        let (q, prefix) = crate::projects::init(&main, true, Some("pq"), &[]).unwrap();
        assert_eq!(prefix, "pq");
        let config = || fs::read_to_string(q.join(CONFIG)).unwrap();
        let readme = fs::read_to_string(q.join("README.md")).unwrap();
        assert!(config().contains("\nprefix = \"pq\"\n"), "{}", config());
        assert!(!readme.starts_with("---"), "{readme}");
        assert!(readme.contains("muster show pq-1"), "{readme}");
        assert_eq!(crate::key::stored_prefix(&q).as_deref(), Some("pq"));
        assert_eq!(read_config(&q), Ok(Config { local: true }));
        // A new prefix goes into config.toml, in place, and the rest of it stays.
        crate::key::store_prefix(&q, "zz").unwrap();
        assert_eq!(crate::key::stored_prefix(&q).as_deref(), Some("zz"));
        assert_eq!(
            config().lines().filter(|l| l.starts_with("prefix")).count(),
            1,
            "{}",
            config()
        );
        assert_eq!(read_config(&q), Ok(Config { local: true }));
        assert_eq!(fs::read_to_string(q.join("README.md")).unwrap(), readme);
        // A prefix in a .muster README isn't the queue's: config.toml is. A queue without one
        // gets its default pinned there on its first write, never into the README.
        let other = repo(base.path(), "other");
        let q2 = other.join(DIR);
        fs::create_dir(&q2).unwrap();
        fs::write(q2.join(CONFIG), config_text(true, None)).unwrap();
        let readme2 = "---\nprefix: ab\n---\n\n# other\n";
        fs::write(q2.join("README.md"), readme2).unwrap();
        assert_eq!(crate::key::stored_prefix(&q2), None);
        let new = crate::ops::NewTask {
            title: "First".into(),
            ..Default::default()
        };
        // The pin every write makes (`with_lock`), here beside no other project: "other" → ot.
        crate::key::pin_prefix_in(&q2, &[]);
        assert_eq!(crate::key::stored_prefix(&q2).as_deref(), Some("ot"));
        assert!(fs::read_to_string(q2.join(CONFIG))
            .unwrap()
            .contains("\nprefix = \"ot\"\n"));
        assert_eq!(fs::read_to_string(q2.join("README.md")).unwrap(), readme2);
        // The master list names tasks by key.
        crate::ops::create_task(&q2, &new, "owner", T).unwrap();
        let index = fs::read_to_string(q2.join(crate::index::FILE)).unwrap();
        assert!(index.contains("ot-1"), "{index}");
        assert_eq!(fs::read_to_string(q2.join("README.md")).unwrap(), readme2);
    }

    #[test]
    fn init_writes_a_committed_queue_when_asked_from_any_folder_of_the_repo_and_never_twice() {
        let base = tempfile::tempdir().unwrap();
        let main = repo(base.path(), "proj");
        let wt = base.path().join("proj/wt-1");
        git(
            &main,
            &["worktree", "add", "-q", "-b", "wt-1", wt.to_str().unwrap()],
        );
        // From a worktree, it goes in the main checkout.
        let q = crate::projects::init(&wt, false, None, &[]).unwrap().0;
        assert_eq!(canon(&q), canon(&main.join(DIR)));
        assert_eq!(read_config(&q), Ok(Config { local: false }));
        for d in ["tasks", "backlog", "attachments"] {
            assert!(q.join(d).is_dir(), "{d}");
        }
        let ignore = fs::read_to_string(q.join(".gitignore")).unwrap();
        // Anchored to .muster/ itself, so a task's attachment named mail/ isn't left out.
        for line in ["/mail/", "/INDEX.md", "/tasks.json"] {
            assert!(ignore.lines().any(|l| l == line), "{line}: {ignore}");
        }
        assert!(!ignore.lines().any(|l| l == "mail/"), "{ignore}");
        assert!(!ignore.contains("\n*\n"), "{ignore}");
        let readme = fs::read_to_string(q.join("README.md")).unwrap();
        assert!(readme.starts_with("# proj — task queue"), "{readme}");
        assert!(readme.contains("tasks/"), "{readme}");
        assert!(!readme.contains("inbox/"), "{readme}");
        assert!(q.join(crate::index::FILE).is_file());
        assert!(crate::projects::looks_like_queue(&q));
        assert_eq!(crate::projects::default_name(&q), "proj");
        let e = crate::projects::init(&main, true, None, &[]).unwrap_err();
        assert!(e.contains("already exists"), "{e}");
        assert_eq!(read_config(&q), Ok(Config { local: false }), "left alone");
        let outside = tempfile::tempdir().unwrap();
        let e = crate::projects::init(outside.path(), false, None, &[]).unwrap_err();
        assert!(e.contains("isn't in a git repo"), "{e}");
        assert!(!outside.path().join(DIR).exists());
    }

    #[test]
    fn discovery_finds_main_checkouts_queues_beside_the_old_ones_never_a_worktrees() {
        let base = tempfile::tempdir().unwrap();
        let main = repo(base.path(), "proj");
        let q = crate::projects::init(&main, true, None, &[]).unwrap().0;
        git(&main, &["add", "-A"]);
        git(&main, &["commit", "-q", "-m", "muster"]);
        let wt = base.path().join("proj/wt-1");
        git(
            &main,
            &["worktree", "add", "-q", "-b", "wt-1", wt.to_str().unwrap()],
        );
        // The project's old queue too (before it's retired), and another project's.
        for old in ["proj/tasks", "other/tasks"] {
            let o = base.path().join(old);
            fs::create_dir_all(o.join("todo")).unwrap();
            fs::write(o.join("README.md"), "q").unwrap();
        }
        // A repo straight in the base (chit), and repos that are projects of their own in a
        // group folder (harness/harness40, harness/c3).
        for (dir, local) in [
            ("chit", true),
            ("harness/harness40", false),
            ("harness/c3", true),
        ] {
            let r = base.path().join(dir);
            fs::create_dir_all(&r).unwrap();
            git(&r, &["init", "-q", "-b", "main"]);
            crate::projects::init(&r, local, None, &[]).unwrap();
        }
        let found: Vec<(PathBuf, String)> = crate::queue::discover(base.path())
            .into_iter()
            .map(|f| {
                (
                    f.path.strip_prefix(base.path()).unwrap().to_path_buf(),
                    f.group,
                )
            })
            .collect();
        let want: Vec<(PathBuf, String)> = [
            ("chit/.muster", ""),
            ("harness/c3/.muster", "harness"),
            ("harness/harness40/.muster", "harness"),
            ("other/tasks", ""),
            ("proj/main/.muster", ""),
            ("proj/tasks", ""),
        ]
        .iter()
        .map(|(p, g)| (PathBuf::from(p), g.to_string()))
        .collect();
        assert_eq!(found, want, "never proj/wt-1/.muster");
        // The app's list: a project's main checkout's queue takes the project's name; a repo
        // that is a project of its own, its own name.
        let list = crate::projects::list(base.path(), &Default::default());
        let named: Vec<(&str, &str, bool)> = list
            .iter()
            .map(|p| (p.name.as_str(), p.group.as_str(), p.ok))
            .collect();
        assert_eq!(
            named,
            vec![
                ("chit", "", true),
                ("c3", "harness", true),
                ("harness40", "harness", true),
                ("other", "", true),
                ("proj", "", true),
                ("proj-2", "", true),
            ]
        );
        assert_eq!(canon(Path::new(&list[4].path)), canon(&q));
        // `--queue chit` finds it, and its worktrees never land in the base itself.
        let chit = base.path().join("chit/.muster");
        let known = crate::projects::active(base.path(), &Default::default());
        assert!(
            known.iter().any(|(n, p)| n == "chit" && *p == chit),
            "{known:?}"
        );
        assert_eq!(
            worktrees_dir(&chit, "chit"),
            Some(base.path().join("chit.worktrees"))
        );
        assert_eq!(
            worktrees_dir(&q, "main"),
            Some(canon(base.path()).join("proj/worktrees/main"))
        );
        assert_eq!(
            worktrees_dir(&base.path().join("proj/tasks"), "main"),
            Some(base.path().join("proj/worktrees/main"))
        );
    }

    #[test]
    fn a_repo_whose_git_folder_isnt_a_dot_git_has_no_queue_and_a_broken_worktree_never_splits_one()
    {
        let base = tempfile::tempdir().unwrap();
        let b = base.path();
        // A worktree of a bare repo: git's own folder is bare.git.
        let src = repo(b, "src");
        let bare = b.join("bare.git");
        git(
            b,
            &[
                "clone",
                "-q",
                "--bare",
                src.to_str().unwrap(),
                bare.to_str().unwrap(),
            ],
        );
        let bwt = b.join("bwt");
        git(
            &bare,
            &["worktree", "add", "-q", bwt.to_str().unwrap(), "main"],
        );
        // A checkout whose git folder lives elsewhere (--separate-git-dir).
        let sep = b.join("sep");
        let sep_git = b.join("sep.gitdir");
        git(
            b,
            &[
                "init",
                "-q",
                "--separate-git-dir",
                sep_git.to_str().unwrap(),
                sep.to_str().unwrap(),
            ],
        );
        // A submodule, inside a repo that has a queue.
        let outer = repo(b, "outer");
        crate::projects::init(&outer, true, None, &[]).unwrap();
        let inner = repo(b, "inner");
        git(
            &outer,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                "-q",
                inner.to_str().unwrap(),
                "sub",
            ],
        );
        let sub = outer.join("sub");
        for dir in [&bwt, &sep, &sub] {
            assert_eq!(main_checkout(dir), None, "{}", dir.display());
            assert_eq!(find(dir), None, "{}", dir.display());
            let e = crate::projects::init(dir, true, None, &[]).unwrap_err();
            assert!(e.contains("main checkout"), "{e}");
        }
        assert!(!bare.join(DIR).exists() && !sep_git.join(DIR).exists() && !sub.join(DIR).exists());
        // A worktree git can't read any more (its main checkout's record of it is gone): never
        // its own `.muster/`, which holds only what's committed.
        let main = repo(b, "proj");
        crate::projects::init(&main, true, None, &[]).unwrap();
        git(&main, &["add", "-A"]);
        git(&main, &["commit", "-q", "-m", "muster"]);
        let wt = b.join("proj/wt-1");
        git(
            &main,
            &["worktree", "add", "-q", "-b", "wt-1", wt.to_str().unwrap()],
        );
        assert!(wt.join(DIR).join(CONFIG).is_file());
        fs::remove_dir_all(main.join(".git/worktrees/wt-1")).unwrap();
        assert_eq!(find(&wt), None);
        // Outside any repo, the walk up still finds a `.muster` folder it's in or beside.
        let plain = b.join("plain");
        fs::create_dir_all(plain.join(DIR)).unwrap();
        fs::write(plain.join(DIR).join(CONFIG), config_text(true, None)).unwrap();
        assert_eq!(find(&plain), Some(plain.join(DIR)));
    }

    #[test]
    fn config_reads_local_and_leaves_other_keys_alone() {
        assert_eq!(parse_config(""), Ok(Config { local: false }));
        assert_eq!(
            parse_config("# c\nprefix = \"mu\"  # 0033\nlocal = true\n"),
            Ok(Config { local: true })
        );
        assert_eq!(
            parse_config("local = false\n[later]\nlocal = true\n"),
            Ok(Config { local: false }),
            "only the top level is muster's"
        );
        assert!(parse_config("local = yes\n")
            .unwrap_err()
            .contains("true or false"));
        // Lines another tool writes that aren't `key = value` (a multi-line array) are its own.
        assert_eq!(
            parse_config("names = [\n  \"a\",\n  \"b\",\n]\nlocal = true\n"),
            Ok(Config { local: true })
        );
        assert_eq!(parse_config("local\n"), Ok(Config { local: false }));
        // A quoted key is the same key.
        assert_eq!(
            parse_config("\"local\" = true\n"),
            Ok(Config { local: true })
        );
        for local in [true, false] {
            assert_eq!(
                parse_config(&config_text(local, None)),
                Ok(Config { local })
            );
            let with = config_text(local, Some("mu"));
            assert_eq!(parse_config(&with), Ok(Config { local }));
            assert_eq!(config_prefix_in(&with).as_deref(), Some("mu"));
        }
        // The prefix (0033), quoted or not, the top level only.
        assert_eq!(config_prefix_in("prefix = mu\n").as_deref(), Some("mu"));
        assert_eq!(config_prefix_in("[t]\nprefix = \"mu\"\n"), None);
        for (text, want) in [
            ("local = true\n", "local = true\nprefix = \"ab\"\n"),
            (
                "local = true\nprefix = \"mu\" # 0033\n",
                "local = true\nprefix = \"ab\"\n",
            ),
            (
                "local = true\n[later]\nx = 1\n",
                "local = true\nprefix = \"ab\"\n[later]\nx = 1\n",
            ),
        ] {
            assert_eq!(with_config_prefix(text, "ab"), want, "{text:?}");
        }
    }

    #[test]
    fn check_reports_a_bad_config_and_a_local_queue_git_would_see() {
        let d = tempfile::tempdir().unwrap();
        let q = d.path().join(DIR);
        fs::create_dir(&q).unwrap();
        fs::write(q.join(CONFIG), "local = maybe\n").unwrap();
        let p = problems("p", &q);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].starts_with("p: config.toml: local must be"), "{p:?}");
        fs::write(q.join(CONFIG), config_text(true, None)).unwrap();
        let p = problems("p", &q);
        assert!(p[0].contains(".gitignore"), "{p:?}");
        fs::write(q.join(".gitignore"), gitignore_text(true)).unwrap();
        assert_eq!(problems("p", &q), Vec::<String>::new());
        let s = crate::queue::read_state_at(&[("p".into(), q.clone())], &mut Default::default(), T);
        assert_eq!(s.problems, Vec::<String>::new());
        fs::remove_file(q.join(".gitignore")).unwrap();
        let s = crate::queue::read_state_at(&[("p".into(), q)], &mut Default::default(), T);
        assert_eq!(s.problems.len(), 1, "{:?}", s.problems);
    }
}

/// The core flows (create, claim, pass, block, comment, ask and answer, the backlog's add, file
/// and drop, the master list) run the same in every layout; in `.muster/` the file never moves.
#[cfg(test)]
mod flows {
    use super::*;
    use crate::ops::{self, NewTask, PassOpts};
    use crate::queue::{self, read_state_at, Cache, STATUS_FOLDERS};
    use std::fs;

    const T: &str = "2026-10-13 09:00";

    /// One empty queue in each layout, by name: v2 (status folders), flat, and `.muster/` in a
    /// repo folder. The temp folder is kept alive with it.
    fn layouts() -> Vec<(tempfile::TempDir, PathBuf, &'static str)> {
        let v2 = tempfile::tempdir().unwrap();
        for f in STATUS_FOLDERS {
            fs::create_dir(v2.path().join(f)).unwrap();
        }
        // Each with its prefix stored where its layout keeps it (0033), so no write works one out
        // from the projects on this Mac.
        fs::write(v2.path().join("README.md"), "---\nprefix: pp\n---\nq\n").unwrap();
        let v2_root = v2.path().to_path_buf();
        let flat = tempfile::tempdir().unwrap();
        fs::write(flat.path().join("README.md"), "---\nprefix: pp\n---\nq\n").unwrap();
        let flat_root = flat.path().to_path_buf();
        let repo = tempfile::tempdir().unwrap();
        let m = repo.path().join(DIR);
        fs::create_dir(&m).unwrap();
        fs::write(m.join("config.toml"), config_text(false, Some("pp"))).unwrap();
        vec![
            (v2, v2_root, "v2"),
            (flat, flat_root, "flat"),
            (repo, m, "muster"),
        ]
    }

    /// Where a task file sits in each layout, for a task whose status mirror is `folder`.
    fn place(layout: &str, folder: &str, file: &str) -> String {
        match layout {
            "v2" => format!("{folder}/{file}"),
            "flat" => file.to_string(),
            _ => format!("tasks/{file}"),
        }
    }

    fn task(root: &Path, id: &str) -> queue::Task {
        state(root).tasks.into_iter().find(|t| t.id == id).unwrap()
    }

    fn state(root: &Path) -> queue::State {
        read_state_at(
            &[("p".into(), root.to_path_buf())],
            &mut Cache::default(),
            T,
        )
    }

    fn problems(root: &Path) -> Vec<String> {
        state(root)
            .problems
            .into_iter()
            .filter(|p| !queue::is_warning(p))
            .collect()
    }

    fn gate() -> PassOpts {
        PassOpts {
            gate: "gate: green (all)".into(),
            ..Default::default()
        }
    }

    #[test]
    fn create_claim_and_pass_to_done_in_every_layout() {
        for (_d, root, name) in layouts() {
            let new = NewTask {
                title: "Ship it".into(),
                body: "## Acceptance criteria\n- [ ] AC1: it works. — verify: owner (try it)"
                    .into(),
                ..Default::default()
            };
            let c = ops::create_task(&root, &new, "owner", T).unwrap();
            let at = |folder: &str| place(name, folder, "0001-ship-it.md");
            assert_eq!(c.path, at("todo"), "{name}");
            ops::claim_task(&root, "0001", "wt-a", T).unwrap();
            let t = task(&root, "0001");
            assert_eq!(
                (t.stage.as_str(), t.status.as_str(), t.path.as_str()),
                ("building", "doing", at("doing").as_str()),
                "{name}"
            );
            ops::pass_task(&root, "0001", "wt-a", T, &gate()).unwrap();
            ops::pass_task(&root, "0001", "qa", T, &PassOpts::default()).unwrap();
            ops::tick_criterion(&root, "0001", "AC1", false, "owner", T).unwrap();
            ops::pass_task(&root, "0001", "owner", T, &PassOpts::default()).unwrap();
            let t = task(&root, "0001");
            assert_eq!(
                (t.stage.as_str(), t.status.as_str(), t.path.as_str()),
                ("done", "done", at("done").as_str()),
                "{name}"
            );
            assert_eq!(t.folder, if name == "v2" { "done" } else { "" }, "{name}");
            // The status mirror is written only for the old layouts' readers.
            let text = fs::read_to_string(root.join(&t.path)).unwrap();
            assert_eq!(
                text.contains("\nstatus:"),
                name != "muster",
                "{name}: {text}"
            );
            assert!(text.contains("\nstage: done\n"), "{name}: {text}");
            assert_eq!(problems(&root), Vec::<String>::new(), "{name}");
        }
    }

    #[test]
    fn block_comment_ask_and_answer_in_every_layout() {
        for (_d, root, name) in layouts() {
            let new = NewTask {
                title: "Port".into(),
                ..Default::default()
            };
            ops::create_task(&root, &new, "owner", T).unwrap();
            ops::claim_task(&root, "0001", "wt-a", T).unwrap();
            ops::block_as(&root, "0001", "you", "pick a colour", "wt-a", T).unwrap();
            let t = task(&root, "0001");
            assert!(t.blocked && t.stage == "building", "{name}");
            assert_eq!(t.path, place(name, "blocked", "0001-port.md"), "{name}");
            ops::unblock_task(&root, "0001", "owner", T).unwrap();
            let t = task(&root, "0001");
            assert!(!t.blocked, "{name}");
            assert_eq!(t.path, place(name, "doing", "0001-port.md"), "{name}");
            crate::fields::add_comment(&root, "0001", "Looks right.", "owner", T).unwrap();
            queue::ask(&root, "0001", "Which port?", "wt-a", "aaaa1111-x", T).unwrap();
            let asker = queue::answer(&root, "0001", 1, "5173", "owner", T).unwrap();
            assert_eq!(asker.as_deref(), Some("aaaa1111"), "{name}");
            assert!(root.join("mail/aaaa1111/0001-Q1.md").is_file(), "{name}");
            let t = task(&root, "0001");
            assert_eq!((t.comments.len(), t.open_questions), (1, 0), "{name}");
            assert_eq!(problems(&root), Vec::<String>::new(), "{name}");
        }
    }

    #[test]
    fn the_backlog_adds_files_and_drops_in_every_layout() {
        for (_d, root, name) in layouts() {
            let notes = match name {
                "v2" => "inbox",
                _ => "backlog",
            };
            let n = crate::backlog::add(&root, "Dark mode", "", "wt-a", T).unwrap();
            assert!(root.join(notes).join(&n).is_file(), "{name}");
            assert_eq!(state(&root).backlog.len(), 1, "{name}");
            let new = NewTask {
                title: "Dark mode".into(),
                ..Default::default()
            };
            let f = crate::backlog::file_note(&root, &n, &new, "shaper", T).unwrap();
            assert_eq!(f.path, place(name, "todo", "0001-dark-mode.md"), "{name}");
            assert!(root.join("attachments/0001").join(&f.attachment).is_file());
            assert_eq!(task(&root, "0001").stage, "shaping", "{name}");
            let old = crate::backlog::add(&root, "Old idea", "", "wt-a", T).unwrap();
            let gone = crate::backlog::drop_note(&root, &old, "not now", "owner", T).unwrap();
            assert_eq!(gone, format!("{notes}/dropped/{old}"), "{name}");
            let s = state(&root);
            assert_eq!((s.backlog.len(), s.tasks.len()), (0, 1), "{name}");
            assert_eq!(problems(&root), Vec::<String>::new(), "{name}");
        }
    }

    #[test]
    fn the_master_list_follows_every_layout() {
        for (_d, root, name) in layouts() {
            let new = NewTask {
                title: "Listed".into(),
                ..Default::default()
            };
            ops::create_task(&root, &new, "owner", T).unwrap();
            crate::backlog::add(&root, "A note", "", "wt-a", T).unwrap();
            let index = fs::read_to_string(root.join(crate::index::FILE)).unwrap();
            let link = format!("[Listed]({})", place(name, "todo", "0001-listed.md"));
            assert!(index.contains(&link), "{name}: {index}");
            assert_eq!(
                crate::index::status(&root),
                crate::index::Status::Current,
                "{name}"
            );
            // A hand edit to a task file leaves it stale, in `.muster/tasks/` too.
            let t = task(&root, "0001");
            let f = root.join(&t.path);
            let text = fs::read_to_string(&f).unwrap();
            fs::write(&f, text.replace("# Listed", "# Renamed")).unwrap();
            assert_eq!(
                crate::index::status(&root),
                crate::index::Status::Stale,
                "{name}"
            );
        }
    }

    #[test]
    fn a_field_written_twice_is_changed_where_it_is_read_the_last_one() {
        for (_d, root, name) in layouts() {
            let dir = task_dir(&root, "todo");
            fs::create_dir_all(&dir).unwrap();
            // Readers take the last `stage:`; a change must land there, or it's lost.
            let status = if keeps_status(&root) {
                "status: todo\n"
            } else {
                ""
            };
            fs::write(
                dir.join("0001-twice.md"),
                format!("---\nid: 0001\nstage: backlog\n{status}claimed-by:\nheartbeat:\nwaiting_on:\nstage: ready\n---\n\n# Twice\n\n## Log\n- {T} owner: created\n"),
            )
            .unwrap();
            assert_eq!(task(&root, "0001").stage, "ready", "{name}");
            ops::claim_task(&root, "0001", "wt-a", T).unwrap();
            let t = task(&root, "0001");
            assert_eq!(
                (t.stage.as_str(), t.claimed_by.as_str()),
                ("building", "wt-a"),
                "{name}"
            );
        }
    }

    #[test]
    fn a_status_left_in_a_muster_task_is_ignored_and_the_stage_rules() {
        let (_d, root, _) = layouts().pop().unwrap();
        fs::create_dir(root.join("tasks")).unwrap();
        fs::write(
            root.join("tasks/0001-x.md"),
            "---\nid: 0001\nstage: ready\nstatus: done\nwaiting_on:\n---\n\n# X\n\n## Log\n- 2026-10-13 09:00 owner: created\n",
        )
        .unwrap();
        let t = task(&root, "0001");
        assert_eq!(
            (t.stage.as_str(), t.status.as_str(), t.folder.as_str()),
            ("ready", "todo", "")
        );
        assert_eq!(problems(&root), Vec::<String>::new());
        // A file left directly in `.muster/` isn't a task.
        fs::write(
            root.join("0002-loose.md"),
            "---\nid: 0002\nstage: ready\n---\n",
        )
        .unwrap();
        assert_eq!(state(&root).tasks.len(), 1);
    }
}
