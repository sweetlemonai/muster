//! `muster migrate <old-queue> <repo> [--local] [--dry-run]` (task 0005, decision 0003): an old
//! `<project>/tasks` queue (status folders, or flat) copied into the repo's `.muster/`.
//!
//! Copied, never moved: the old queue is left exactly as it was (its lock aside), for the owner
//! to retire once they've checked the new one. Every task file goes to `.muster/tasks/` with
//! `stage:` set from its old stage or status and `status:` dropped; the backlog folder's notes,
//! dropped notes and anything else in it to `backlog/`; `attachments/`, `mail/`, and the review
//! of record's `reviews/` and `lessons.md` (0027) as they are;
//! `WAITING-FOR-OWNER.md`; the old README's front matter at the top of the new README. Anything
//! else is listed as not copied. Refused while an agent holds a task in Building or Testing, when
//! the repo already has a `.muster`, when the repo and the old queue overlap, on a folder it can't
//! read, on a task file whose front matter it can't read, and on two names that would be one file
//! on this Mac. Built in a folder beside it, every file made new (never over another), and
//! renamed into place at the end, so a failure leaves no half queue.

use crate::layout;
use crate::queue::{self, Task};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// What `muster migrate` was asked for.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    /// Write `local = true`: the queue stays on this Mac.
    pub local: bool,
    /// Print the plan, write nothing.
    pub dry_run: bool,
}

/// What it did, or would do: the new queue, and the summary's lines.
#[derive(Debug, Clone)]
pub struct Report {
    pub queue: PathBuf,
    pub lines: Vec<String>,
}

/// What `waiting_on` says for a task an old queue had blocked without saying on what, so it
/// stays blocked once the status that said so is gone.
const NO_REASON: &str = "blocked before the move to .muster/, no reason recorded";

/// What a refusal over two names says: on this Mac's disks they're one file.
const ONE_FILE: &str =
    "names that differ only in case or accents are one file on this Mac; rename one in the old queue first";

/// A file of the new queue: where it comes from (and as the old queue names it), and its new
/// text when it's a task changed on the way (None: copied as it is).
struct Item {
    src: PathBuf,
    rel: String,
    text: Option<String>,
}

/// Everything the migration copies and says, worked out from the old queue without writing.
#[derive(Default)]
struct Plan {
    /// By path in the new queue.
    items: BTreeMap<PathBuf, Item>,
    /// The new paths in lowercase, with the old file each came from: two that match are one
    /// file on a case-insensitive disk.
    folded: HashMap<String, String>,
    stages: HashMap<String, usize>,
    tasks: usize,
    notes: usize,
    backlog_other: usize,
    attachments: usize,
    mail: usize,
    /// The review of record's files and its lessons (0027): `reviews/` and `lessons.md`.
    reviews: usize,
    waiting: bool,
    front: String,
    /// The old queue's prefix (0033), stored or the default its keys have now: the new queue's,
    /// in `config.toml`, so no key changes.
    prefix: String,
    warnings: Vec<String>,
    not_copied: Vec<String>,
}

/// Migrate `old` into the `.muster/` of the repo `repo` is in (its main checkout). `tools` holds
/// `manifest.py`, which writes `tasks.json`; `stamp` is local time for `migrated-from.txt`;
/// `known` the projects Muster lists (`key::known`), for the default prefix of an old queue with
/// none stored (0033).
pub fn migrate(
    old: &Path,
    repo: &Path,
    opts: &Options,
    tools: &Path,
    stamp: &str,
    known: &[crate::projects::Project],
) -> Result<Report, String> {
    let old = fs::canonicalize(old).map_err(|e| format!("{}: {e}", old.display()))?;
    if layout::is_muster(&old) {
        return Err(format!("{} is already a .muster/ queue", old.display()));
    }
    if !crate::projects::looks_like_queue(&old) {
        return Err(format!(
            "{} isn't a Muster queue (a README.md with a todo/ folder or a tasks.json)",
            old.display()
        ));
    }
    let main = layout::main_checkout(repo).ok_or_else(|| {
        format!(
            "{} isn't in a git repo: the queue goes in a repo's .muster/",
            repo.display()
        )
    })?;
    let main = fs::canonicalize(&main).map_err(|e| format!("{}: {e}", main.display()))?;
    if main.starts_with(&old) || old.starts_with(&main) {
        return Err(format!(
            "{} and the repo {} overlap: the new queue would land inside the old one, or the old one is inside the repo; name the project's code repo",
            old.display(),
            main.display()
        ));
    }
    let q = main.join(layout::DIR);
    let taken = || -> Result<(), String> {
        match fs::symlink_metadata(&q) {
            Ok(_) => Err(format!(
                "{} already exists; migrate never writes over it",
                q.display()
            )),
            Err(_) => Ok(()),
        }
    };
    taken()?;
    if opts.dry_run {
        let plan = plan(&old, known)?;
        return Ok(Report {
            lines: summary(&plan, &old, &q, opts, true),
            queue: q,
        });
    }
    // The old queue's lock, so no muster write lands in it while it's read and copied.
    queue::hold_lock(&old, || {
        let plan = plan(&old, known)?;
        taken()?;
        let staging = main.join(format!(".muster-migrating-{}", std::process::id()));
        fs::create_dir(&staging).map_err(|e| format!("{}: {e}", staging.display()))?;
        let built = staging.join(layout::DIR);
        let project = crate::projects::default_name(&q);
        let result = build(&plan, &old, &built, &project, opts, tools, stamp).and_then(|()| {
            taken()?;
            fs::rename(&built, &q).map_err(|e| format!("{}: {e}", q.display()))
        });
        let _ = fs::remove_dir_all(&staging);
        result?;
        // A local queue's history (0109) starts with everything copied; failing here, it starts
        // on the first write.
        let _ = crate::history::start(&q, &format!("{} (migrated)", crate::history::STARTS));
        Ok(Report {
            lines: summary(&plan, &old, &q, opts, false),
            queue: q.clone(),
        })
    })
}

/// The old queue as a plan: every file it has, where each goes, and what's said about it. Refused
/// while an agent holds a task in Building or Testing, and when two files would land on one path.
fn plan(old: &Path, known: &[crate::projects::Project]) -> Result<Plan, String> {
    let mut p = Plan {
        prefix: crate::key::prefix_in(old, known),
        ..Plan::default()
    };
    let mut busy = Vec::new();
    let mut unreadable = Vec::new();
    let mut task_paths = HashSet::new();
    // Every folder task files are read from must be readable: one that isn't would be skipped.
    for (_, dir) in layout::task_dirs(old) {
        if dir.is_dir() {
            fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
    }
    for (folder, path) in queue::task_files(old) {
        let rel = rel(old, &path);
        let name = path.file_name().map(PathBuf::from).unwrap_or_default();
        let (text, edits) = task_text(&path)?;
        let parsed = text
            .as_deref()
            .and_then(|t| queue::parse_task("", old, &folder, &path, t, 0).map(|task| (t, task)));
        let Some((t, task)) = parsed else {
            unreadable.push(rel);
            continue;
        };
        if held_by_an_agent(&task) {
            let who = holder(&task).chars().take(100).collect::<String>();
            busy.push(format!(
                "{} ({}, held by {who})",
                task.id,
                queue::stage_label(&task.stage)
            ));
        }
        if !edits.is_empty() {
            p.warnings.push(format!("{rel}: {}", edits.join(" and ")));
        }
        *p.stages.entry(task.stage.clone()).or_default() += 1;
        let new_text = moved(t, &task, &rel, &mut p.warnings)?;
        p.tasks += 1;
        let dest = PathBuf::from("tasks").join(name);
        task_paths.insert(path.clone());
        add(&mut p, dest, path, Some(new_text), &rel)?;
    }
    if !busy.is_empty() {
        return Err(format!(
            "not while an agent holds a task in Building or Testing (pass, return or release it first): {}",
            busy.join("; ")
        ));
    }
    if !unreadable.is_empty() {
        return Err(format!(
            "can't read the front matter of {}, so its stage would be lost: fix it in the old queue first (`---`, its fields, `---` at the top, in UTF-8)",
            unreadable.join(", ")
        ));
    }
    let notes = crate::backlog::backlog_dir(old);
    let notes_name = notes
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    for (path, rel_path) in walk(old)? {
        if task_paths.contains(&path) {
            continue;
        }
        let rel = rel_path.to_string_lossy().to_string();
        let first = rel_path
            .components()
            .next()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .unwrap_or_default();
        let file = rel_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let top = rel_path.components().count() == 1;
        if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            p.not_copied.push(format!("{rel} (a symlink)"));
            continue;
        }
        match first.as_str() {
            ".git" => {}
            "attachments" => {
                p.attachments += 1;
                add(&mut p, rel_path.clone(), path, None, &rel)?;
            }
            f if f == layout::REVIEWS || (f == layout::LESSONS && top) => {
                p.reviews += 1;
                add(&mut p, rel_path.clone(), path, None, &rel)?;
            }
            "mail" if file == ".lock" => {}
            "mail" => {
                p.mail += 1;
                add(&mut p, rel_path.clone(), path, None, &rel)?;
            }
            f if f == notes_name && !top => {
                let rest = rel_path.strip_prefix(&notes_name).unwrap_or(&rel_path);
                let note = rest.components().count() == 1 && crate::backlog::is_note_file(&file);
                if note {
                    p.notes += 1;
                } else if file != ".gitkeep" && file != ".DS_Store" {
                    p.backlog_other += 1;
                } else {
                    continue;
                }
                add(&mut p, Path::new("backlog").join(rest), path, None, &rel)?;
            }
            "WAITING-FOR-OWNER.md" if top => {
                p.waiting = true;
                add(&mut p, rel_path.clone(), path, None, &rel)?;
            }
            "README.md" if top => {
                p.front = front_matter_block(&read_small(&path).unwrap_or_default());
            }
            "INDEX.md" | "tasks.json" | ".gitignore" if top => {}
            _ if file == ".gitkeep" || file == ".DS_Store" => {}
            _ => p.not_copied.push(rel),
        }
    }
    p.not_copied.sort();
    probe(&p, &probe_dir())?;
    Ok(p)
}

/// Who holds a task: its holder, or for Testing from before `held-by` (0030), its claim.
fn holder(t: &Task) -> &str {
    match t.holder() {
        "" if t.stage == "testing" => &t.claimed_by,
        h => h,
    }
}

/// A task held in Building or Testing by an agent, not the owner (the owner's own task, or the
/// owner holding one, doesn't stop a migration).
fn held_by_an_agent(t: &Task) -> bool {
    let holder = holder(t);
    matches!(t.stage.as_str(), "building" | "testing")
        && !holder.is_empty()
        && !crate::session::is_owner(holder)
}

/// A task file's text as the readers take it, changed only when its front matter can't be read
/// as it is: a byte-order mark taken off, then the front matter's own CRLF line ends made LF,
/// the body kept byte for byte. Returns what was changed. None when it isn't UTF-8. An error
/// names a file it can't read.
fn task_text(path: &Path) -> Result<(Option<String>, Vec<&'static str>), String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let Ok(text) = String::from_utf8(bytes) else {
        return Ok((None, Vec::new()));
    };
    let mut edits = Vec::new();
    if queue::front_matter(&text).is_some() {
        return Ok((Some(text), edits));
    }
    let text = match text.strip_prefix('\u{feff}') {
        Some(rest) => {
            edits.push("its byte-order mark taken off");
            rest.to_string()
        }
        None => text,
    };
    if queue::front_matter(&text).is_some() {
        return Ok((Some(text), edits));
    }
    let text = match plain_front(&text) {
        Some(plain) => {
            edits.push("its front matter's CRLF line ends made LF");
            plain
        }
        None => text,
    };
    Ok((Some(text), edits))
}

/// The text with its front matter's CRLF line ends made LF, through the closing `---` line, and
/// the rest as it was; None when it doesn't start with a CRLF front matter.
fn plain_front(text: &str) -> Option<String> {
    let open = "---\r\n";
    let rest = text.strip_prefix(open)?;
    let close = ["\r\n---\r\n", "\r\n---\n"]
        .iter()
        .filter_map(|c| rest.find(c).map(|i| i + c.len()))
        .min()?;
    let (head, body) = text.split_at(open.len() + close);
    Some(format!("{}{body}", head.replace("\r\n", "\n")))
}

/// A task's text for `.muster/tasks/`: `stage:` set from what it was read as, `status:` gone,
/// and a block that only the status said kept, with what it waits on.
fn moved(text: &str, t: &Task, rel: &str, warnings: &mut Vec<String>) -> Result<String, String> {
    let mut new = crate::ops::set_front(text, "stage", &t.stage)?;
    new = crate::ops::clear_front(&new, "status")?;
    if t.status == "blocked" && t.waiting_on.is_empty() {
        new = crate::ops::set_front(&new, "waiting_on", NO_REASON)?;
        warnings.push(format!(
            "{rel}: blocked without saying on what; waiting_on now says {NO_REASON:?}"
        ));
    }
    if !t.bad_stage.is_empty() {
        warnings.push(format!(
            "{rel}: stage {:?} isn't a stage; set to {}",
            t.bad_stage, t.stage
        ));
    }
    Ok(new)
}

/// Add `dest`, refusing a second file for the same path, or for one that differs only in case
/// (one file on a case-insensitive disk).
fn add(
    p: &mut Plan,
    dest: PathBuf,
    src: PathBuf,
    text: Option<String>,
    rel: &str,
) -> Result<(), String> {
    let key = dest.to_string_lossy().to_lowercase();
    if let Some(first) = p.folded.get(&key) {
        return Err(format!(
            "{first} and {rel} would both be .muster/{} ({ONE_FILE})",
            dest.display()
        ));
    }
    p.folded.insert(key, rel.to_string());
    let rel = rel.to_string();
    p.items.insert(dest, Item { src, rel, text });
    Ok(())
}

/// The plan's paths made as empty files in a scratch folder, so the disk itself says which
/// names it takes for one (accents written two ways, composed and decomposed, on APFS): refused,
/// naming both. `scratch` is a folder of this probe's own (`probe_dir`), removed after.
fn probe(p: &Plan, scratch: &Path) -> Result<(), String> {
    let result = (|| {
        let mut made: HashMap<(u64, u64), &str> = HashMap::new();
        for (dest, item) in &p.items {
            let at = scratch.join(dest);
            if let Some(dir) = at.parent() {
                fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&at)
            {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let first = fs::metadata(&at)
                        .ok()
                        .and_then(|m| made.get(&file_id(&m)).copied())
                        .unwrap_or("another file");
                    return Err(format!(
                        "{first} and {} would both be .muster/{} ({ONE_FILE})",
                        item.rel,
                        dest.display()
                    ));
                }
                Err(e) => return Err(format!("{}: {e}", at.display())),
            }
            let meta = fs::metadata(&at).map_err(|e| format!("{}: {e}", at.display()))?;
            made.insert(file_id(&meta), &item.rel);
        }
        Ok(())
    })();
    let _ = fs::remove_dir_all(scratch);
    result
}

/// A scratch folder name for one probe: the process id and a counter, so two probes never
/// share one, in one process (parallel tests) or in two.
fn probe_dir() -> PathBuf {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("muster-migrate-probe-{}-{n}", std::process::id()))
}

/// A file's identity on disk: its device and inode.
fn file_id(m: &fs::Metadata) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt;
    (m.dev(), m.ino())
}

/// Every file under `dir` (symlinks included, never followed), with its path relative to `dir`,
/// in order. `.git` isn't walked. A folder or entry it can't read is an error naming it: a
/// folder skipped would be files lost.
fn walk(dir: &Path) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let rd = fs::read_dir(&d).map_err(|e| format!("{}: {e}", d.display()))?;
        for e in rd {
            let e = e.map_err(|e| format!("{}: {e}", d.display()))?;
            let path = e.path();
            let kind = e
                .file_type()
                .map_err(|e| format!("{}: {e}", path.display()))?;
            if kind.is_dir() {
                if e.file_name() != ".git" || d != dir {
                    stack.push(path);
                }
            } else {
                let rel = path.strip_prefix(dir).unwrap_or(&path).to_path_buf();
                out.push((path, rel));
            }
        }
    }
    out.sort();
    Ok(out)
}

/// A text file of at most 1 MB, or None.
fn read_small(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    if meta.len() > queue::MAX_TASK_BYTES {
        return None;
    }
    fs::read_to_string(path).ok()
}

/// A README's front matter (`---` … `---` at its very start, with the line after), or "".
/// Its `prefix:` left out (a `.muster` queue keeps it in `config.toml`), and nothing when that was
/// all there was.
fn front_matter_block(text: &str) -> String {
    let Some((fields, body)) = queue::front_matter(text) else {
        return String::new();
    };
    if fields.keys().all(|k| k == "prefix") {
        return String::new();
    }
    let block = text[..text.len() - body.len()].trim_end();
    let kept: Vec<&str> = block
        .lines()
        .filter(|l| !(l.starts_with("prefix:") || l.starts_with("prefix :")))
        .collect();
    format!("{}\n", kept.join("\n"))
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

/// An item's file at `to`, which must not exist yet: never over another file, even one the disk
/// takes for the same name.
fn write_new(to: &Path, item: &Item) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)?;
    match &item.text {
        Some(text) => file.write_all(text.as_bytes())?,
        None => {
            std::io::copy(&mut fs::File::open(&item.src)?, &mut file)?;
        }
    }
    file.sync_all()
}

/// Write the plan into `q` (an empty folder to be) for `project`: the new layout's own files,
/// every item, `migrated-from.txt`, `INDEX.md`, and `tasks.json` from `manifest.py`.
fn build(
    plan: &Plan,
    old: &Path,
    q: &Path,
    project: &str,
    opts: &Options,
    tools: &Path,
    stamp: &str,
) -> Result<(), String> {
    fs::create_dir(q).map_err(|e| format!("{}: {e}", q.display()))?;
    crate::projects::write_muster(q, opts.local, &plan.front, project, &plan.prefix)?;
    for (dest, item) in &plan.items {
        let to = q.join(dest);
        if let Some(dir) = to.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        write_new(&to, item).map_err(|e| {
            let why = if e.kind() == std::io::ErrorKind::AlreadyExists {
                format!("another file already took its name ({ONE_FILE})")
            } else {
                e.to_string()
            };
            format!("{} → {}: {why}", item.rel, to.display())
        })?;
    }
    fs::write(q.join("migrated-from.txt"), migrated_from(old, stamp))
        .map_err(|e| format!("migrated-from.txt: {e}"))?;
    crate::index::rebuild(q)?;
    let out = std::process::Command::new("python3")
        .arg("-I")
        .arg(tools.join("manifest.py"))
        .env("MUSTER_ROOT", q)
        .env_remove("TASKS_ROOT")
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("couldn't run manifest.py for tasks.json: {e}"))?;
    if !out.status.success() || !q.join("tasks.json").is_file() {
        return Err(format!(
            "manifest.py didn't write tasks.json: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

/// Where the queue came from: its folder, when, and its git history's last commit (the history
/// stays in the old queue's own git).
fn migrated_from(old: &Path, stamp: &str) -> String {
    let git = |args: &[&str]| -> Option<String> {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(old)
            .args(args)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let history = if old.join(".git").exists() {
        match git(&["rev-parse", "HEAD"]) {
            Some(sha) => {
                // Without the optional locks, so the old queue's index is never rewritten.
                let dirty = git(&["--no-optional-locks", "status", "--porcelain"])
                    .is_some_and(|s| !s.is_empty());
                let note = if dirty {
                    "\nIt had changes not committed there; they're in this copy, not in that commit."
                } else {
                    ""
                };
                format!("Its last commit: {sha}{note}")
            }
            None => "Its git has no commit yet.".into(),
        }
    } else {
        "It had no git history of its own.".into()
    };
    format!(
        "Migrated by muster from {} on {stamp}.\n{history}\n",
        old.display()
    )
}

/// The summary `muster migrate` prints (or, for a dry run, the plan).
fn summary(p: &Plan, old: &Path, q: &Path, opts: &Options, dry: bool) -> Vec<String> {
    let mode = if opts.local {
        "local = true: only config.toml goes into git"
    } else {
        "committed"
    };
    let mut out = vec![if dry {
        format!(
            "dry run, nothing written: would copy {} into {} ({mode})",
            old.display(),
            q.display()
        )
    } else {
        format!("copied {} into {} ({mode})", old.display(), q.display())
    }];
    let stages: Vec<String> = queue::STAGES
        .iter()
        .filter_map(|s| {
            p.stages
                .get(*s)
                .map(|n| format!("{n} {}", queue::stage_label(s)))
        })
        .collect();
    let by_stage = if stages.is_empty() {
        String::new()
    } else {
        format!(": {}", stages.join(", "))
    };
    out.push(format!("tasks: {} into tasks/{by_stage}", p.tasks));
    out.push(format!(
        "backlog: {} notes, {} other files (dropped notes, files) into backlog/",
        p.notes, p.backlog_other
    ));
    out.push(format!(
        "attachments: {} files; mail: {} files",
        p.attachments, p.mail
    ));
    if p.reviews > 0 {
        out.push(format!(
            "reviews: {} files (the review of record and its lessons) into reviews/ and lessons.md",
            p.reviews
        ));
    }
    let mut kept = vec![
        "config.toml",
        ".gitignore",
        "README.md",
        "migrated-from.txt",
        "INDEX.md",
        "tasks.json",
    ];
    if p.waiting {
        kept.push("WAITING-FOR-OWNER.md");
    }
    out.push(format!("writes: {}", kept.join(", ")));
    out.push(format!(
        "prefix: {}, kept in config.toml, so the task keys stay {}-1, {}-2…",
        p.prefix, p.prefix, p.prefix
    ));
    out.push(format!(
        "the old queue keeps prefix {} until you retire it; keys go to the new queue",
        p.prefix
    ));
    if !p.front.is_empty() {
        out.push("README.md keeps the old README's front matter (but its prefix)".into());
    }
    out.extend(p.warnings.iter().map(|w| format!("note: {w}")));
    if !p.not_copied.is_empty() {
        out.push(format!(
            "not copied (still in the old queue): {}",
            p.not_copied.join(", ")
        ));
    }
    out.push(format!(
        "the old queue is left as it is; retire it once `muster check --queue {}` is clean",
        q.display()
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::resolve::{git, repo};
    use crate::queue::{read_state_at, Cache, STATUS_FOLDERS};

    const T: &str = "2026-10-13 09:00";

    /// `migrate` as the tests run it: beside no other project, so a default prefix comes from the
    /// queue's own name and nothing of this Mac's projects is read.
    fn migrate(
        old: &Path,
        repo: &Path,
        opts: &Options,
        tools: &Path,
        stamp: &str,
    ) -> Result<Report, String> {
        super::migrate(old, repo, opts, tools, stamp, &[])
    }

    fn tools() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bin")
    }

    fn put(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    fn task(id: &str, fm: &str) -> String {
        format!("---\nid: {id}\ntype: code\npriority: normal\n{fm}\n---\n\n# Task {id}\n\n## Log\n- 2026-10-12 09:00 capture: created\n")
    }

    /// Every file under `dir` but its git and its lock, with its bytes.
    fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        walk(dir)
            .unwrap()
            .into_iter()
            .filter(|(_, r)| !r.starts_with(".git") && !r.ends_with("mail/.lock"))
            .map(|(p, r)| (r, fs::read(p).unwrap_or_default()))
            .collect()
    }

    /// An old v2 queue with its own git and every kind of file, odd ones too.
    fn v2_queue(base: &Path) -> PathBuf {
        let old = base.join("proj/tasks");
        for f in STATUS_FOLDERS {
            put(&old, &format!("{f}/.gitkeep"), "");
        }
        put(
            &old,
            "README.md",
            "---\nprefix: mu\nowner: x\n---\n\n# proj — task queue\n\nold rules\n",
        );
        put(
            &old,
            "WAITING-FOR-OWNER.md",
            "# Waiting for you\n\nNothing.\n",
        );
        put(&old, "INDEX.md", "old list\n");
        put(&old, "tasks.json", "{}\n");
        // Who is shaping which note is live state: listed as not copied (0097).
        put(&old, "shaping.json", "{}\n");
        put(&old, ".gitignore", "mail/\n");
        put(&old, "bin/task", "#!/bin/sh\n");
        put(&old, "inbox/0001-raw.md", &task("0001", "status: inbox"));
        put(
            &old,
            "todo/0002-ready.md",
            &task("0002", "stage: ready\nstatus: todo"),
        );
        put(
            &old,
            "todo/0003-shaped.md",
            &task("0003", "stage: shaping\nstatus: todo"),
        );
        put(
            &old,
            "doing/0004-checking.md",
            &task("0004", "stage: your-check\nstatus: doing\nheld-by: owner"),
        );
        put(
            &old,
            "blocked/0005-waits.md",
            &task("0005", "status: blocked\nwaiting_on: owner: pick a colour"),
        );
        put(
            &old,
            "blocked/0006-why.md",
            &task("0006", "status: blocked\nwaiting_on:"),
        );
        put(&old, "done/0007-shipped.md", &task("0007", "status: done"));
        put(
            &old,
            "doing/0008-mine.md",
            &task(
                "0008",
                "for: human\nstage: building\nstatus: doing\nclaimed-by: owner",
            ),
        );
        put(
            &old,
            "todo/0009-odd.md",
            &task("0009", "stage: someday\nstatus: todo"),
        );
        put(
            &old,
            "todo/0010-crlf.md",
            "\u{feff}---\r\nid: 0010\r\ntype: code\r\nstatus: todo\r\n---\r\n\r\n# Windows\r\n\r\n## Log\r\n- 2026-10-12 09:00 capture: created\r\n",
        );
        put(&old, "todo/notes.txt", "a stray file\n");
        put(&old, "inbox/2026-10-12-0900-an-idea.md", "An idea\n");
        put(
            &old,
            "inbox/dropped/2026-10-01-old.md",
            "Old\n\n— dropped by owner: no\n",
        );
        put(&old, "inbox/helper/2026-10-12-09-00-shot.png", "png");
        put(&old, "attachments/0002/shot.png", "png");
        put(&old, "attachments/0099/orphan.txt", "kept anyway");
        put(&old, "mail/asks.json", "{\"0002-Q1\": \"aaaa1111\"}\n");
        put(&old, "mail/aaaa1111/0002-Q1.md", "Answer to Q1\n");
        put(&old, "mail/.lock", "");
        std::os::unix::fs::symlink("/etc/hosts", old.join("attachments/0002/link.txt")).unwrap();
        git(&old, &["init", "-q", "-b", "main"]);
        git(&old, &["add", "-A"]);
        git(&old, &["commit", "-q", "-m", "queue"]);
        // Touched later, unchanged: the index's stat data is stale, so a plain `git status`
        // would rewrite the index to refresh it.
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
        fs::File::options()
            .write(true)
            .open(old.join("WAITING-FOR-OWNER.md"))
            .and_then(|f| f.set_modified(later))
            .unwrap();
        old
    }

    fn state(q: &Path) -> crate::queue::State {
        read_state_at(&[("p".into(), q.to_path_buf())], &mut Cache::default(), T)
    }

    #[test]
    fn copies_a_v2_queue_with_every_task_note_file_and_answer_and_leaves_it_as_it_was() {
        let base = tempfile::tempdir().unwrap();
        let old = v2_queue(base.path());
        let main = repo(base.path(), "proj");
        let before = snapshot(&old);
        let head = git(&old, &["rev-parse", "HEAD"]);
        // The old queue's git index, bytes and time: `snapshot` leaves `.git` out.
        let index = || {
            let p = old.join(".git/index");
            (
                fs::read(&p).unwrap(),
                fs::metadata(&p).unwrap().modified().unwrap(),
            )
        };
        let index_before = index();
        let opts = Options {
            local: true,
            dry_run: false,
        };
        let r = migrate(&old, &main, &opts, &tools(), T).unwrap();
        let q = main.join(layout::DIR);
        assert_eq!(
            fs::canonicalize(&r.queue).unwrap(),
            fs::canonicalize(&q).unwrap()
        );
        // Never lost, never moved: the old queue is byte for byte what it was, its git index too.
        assert_eq!(snapshot(&old), before);
        assert!(
            index() == index_before,
            "the old queue's git index was rewritten"
        );
        // Every task, flat in tasks/, its stage from the old stage or status, no status left.
        let s = state(&q);
        let stages: BTreeMap<String, (String, bool)> = s
            .tasks
            .iter()
            .map(|t| (t.id.clone(), (t.stage.clone(), t.blocked)))
            .collect();
        let want = [
            ("0001", "backlog", false),
            ("0002", "ready", false),
            ("0003", "shaping", false),
            ("0004", "your-check", false),
            ("0005", "ready", true),
            ("0006", "ready", true),
            ("0007", "done", false),
            ("0008", "building", false),
            ("0009", "ready", false),
            ("0010", "ready", false),
        ];
        for (id, stage, blocked) in want {
            assert_eq!(stages[id], (stage.to_string(), blocked), "{id}");
        }
        for t in &s.tasks {
            assert!(t.path.starts_with("tasks/"), "{}", t.path);
            let text = fs::read_to_string(q.join(&t.path)).unwrap();
            assert!(!text.contains("\nstatus:"), "{text}");
            assert!(text.contains(&format!("\nstage: {}\n", t.stage)), "{text}");
        }
        // Its front matter is made plain (CRLF, a byte-order mark), so its stage is set like every
        // other's; the body is kept byte for byte.
        let crlf = fs::read_to_string(q.join("tasks/0010-crlf.md")).unwrap();
        let (front, body) = crlf.split_once("\n---\n").unwrap();
        assert!(
            front.starts_with("---\nid: 0010\n") && !front.contains('\r'),
            "{crlf:?}"
        );
        assert_eq!(
            body,
            "\r\n# Windows\r\n\r\n## Log\r\n- 2026-10-12 09:00 capture: created\r\n"
        );
        assert!(fs::read_to_string(q.join("tasks/0006-why.md"))
            .unwrap()
            .contains(&format!("waiting_on: {NO_REASON}")));
        // Notes, dropped notes and other backlog files; attachments; mail but its lock.
        for rel in [
            "backlog/2026-10-12-0900-an-idea.md",
            "backlog/dropped/2026-10-01-old.md",
            "backlog/helper/2026-10-12-09-00-shot.png",
            "attachments/0002/shot.png",
            "attachments/0099/orphan.txt",
            "mail/asks.json",
            "mail/aaaa1111/0002-Q1.md",
            "WAITING-FOR-OWNER.md",
        ] {
            assert_eq!(
                fs::read(q.join(rel)).unwrap(),
                fs::read(old.join(rel.replacen("backlog/", "inbox/", 1))).unwrap(),
                "{rel}"
            );
        }
        assert_eq!(s.backlog.len(), 1);
        assert!(
            !q.join("attachments/0002/link.txt").exists(),
            "symlinks aren't followed"
        );
        assert!(!q.join("bin").exists() && !q.join("todo").exists());
        // Its own files: settings, the README with the old front matter, the record, the lists.
        assert_eq!(layout::read_config(&q), Ok(layout::Config { local: true }));
        assert_eq!(
            fs::read_to_string(q.join(".gitignore")).unwrap(),
            layout::gitignore_text(true)
        );
        // A local queue's history (0109) starts with everything copied, the lists left out.
        let h = crate::history::of(&q).expect("a local queue keeps a history");
        assert!(crate::history::started(&h));
        let s = crate::history::status(&q);
        assert!(s.missing.is_empty() && s.changed.is_empty(), "{s:?}");
        let log = crate::history::log(&q, 5).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].1, "muster: history starts (migrated)");
        let readme = fs::read_to_string(q.join("README.md")).unwrap();
        assert!(
            readme.starts_with("---\nowner: x\n---\n# proj — task queue"),
            "{readme}"
        );
        let from = fs::read_to_string(q.join("migrated-from.txt")).unwrap();
        assert!(from.contains(&old.display().to_string()), "{from}");
        assert!(
            from.contains(&format!("Its last commit: {head}\n")),
            "{from}"
        );
        assert_eq!(crate::index::status(&q), crate::index::Status::Current);
        let manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(q.join("tasks.json")).unwrap()).unwrap();
        assert_eq!(manifest["tasks"].as_array().unwrap().len(), 10);
        assert!(!main
            .join(format!(".muster-migrating-{}", std::process::id()))
            .exists());
        // The summary says what was copied and what wasn't, and how to check.
        let text = r.lines.join("\n");
        for part in [
            "tasks: 10 into tasks/: 1 Backlog, 1 Shaping, 5 Ready, 1 Building, 1 Your check, 1 Done",
            "backlog: 1 notes, 2 other files",
            "attachments: 2 files; mail: 2 files",
            "README.md keeps the old README's front matter (but its prefix)",
            "prefix: mu, kept in config.toml",
            "the old queue keeps prefix mu until you retire it; keys go to the new queue",
            "note: todo/0010-crlf.md: its byte-order mark taken off and its front matter's CRLF line ends made LF",
            "note: blocked/0006-why.md: blocked without saying on what",
            "note: todo/0009-odd.md: stage \"someday\" isn't a stage; set to ready",
            "not copied (still in the old queue): attachments/0002/link.txt (a symlink), bin/task, shaping.json, todo/notes.txt",
            "muster check --queue",
        ] {
            assert!(text.contains(part), "{part:?} in:\n{text}");
        }
        // Once there, never again.
        let again = migrate(&old, &main, &opts, &tools(), T).unwrap_err();
        assert!(again.contains("already exists"), "{again}");
    }

    /// Each task's key in the queue at `root`, by id, as `muster list` shows them.
    fn keys(root: &Path) -> BTreeMap<String, String> {
        let prefix = crate::key::prefix_in(root, &[]);
        state(root)
            .tasks
            .iter()
            .map(|t| (t.id.clone(), crate::key::key(&prefix, &t.id)))
            .collect()
    }

    #[test]
    fn task_keys_are_the_same_after_a_migrate_stored_prefix_or_default() {
        let base = tempfile::tempdir().unwrap();
        // A stored prefix in the old README goes to config.toml, and only there.
        let old = v2_queue(base.path());
        let main = repo(base.path(), "proj");
        let before = keys(&old);
        assert!(before.values().all(|k| k.starts_with("mu-")), "{before:?}");
        let r = migrate(&old, &main, &Options::default(), &tools(), T).unwrap();
        // Every key the old queue showed is the same (its CRLF+BOM task, unreadable there, is
        // read in the new queue too, under the same prefix).
        let after = keys(&r.queue);
        let kept: BTreeMap<String, String> = after
            .iter()
            .filter(|(id, _)| before.contains_key(*id))
            .map(|(i, k)| (i.clone(), k.clone()))
            .collect();
        assert_eq!(kept, before);
        assert_eq!(after.get("0010").map(String::as_str), Some("mu-10"));
        assert_eq!(crate::key::stored_prefix(&r.queue).as_deref(), Some("mu"));
        let config = fs::read_to_string(r.queue.join(layout::CONFIG)).unwrap();
        assert!(config.contains("\nprefix = \"mu\"\n"), "{config}");
        let readme = fs::read_to_string(r.queue.join("README.md")).unwrap();
        assert!(!readme.contains("prefix:"), "{readme}");
        // None stored: the default the old queue's keys have is the one written.
        let (old, main) = queue_with(
            base.path(),
            "nostore",
            &[("todo/0001-a.md", &task("0001", "status: todo"))],
        );
        let before = keys(&old);
        // Beside no other project, "nostore" defaults to its first two letters.
        assert!(before.values().all(|k| k.starts_with("no-")), "{before:?}");
        let r = migrate(&old, &main, &Options::default(), &tools(), T).unwrap();
        assert_eq!(keys(&r.queue), before);
        assert_eq!(crate::key::stored_prefix(&r.queue).as_deref(), Some("no"));
        // The old queue wasn't given one on the way.
        assert_eq!(crate::key::stored_prefix(&old), None);
    }

    #[test]
    fn a_flat_queue_migrates_committed_and_its_check_is_clean() {
        let base = tempfile::tempdir().unwrap();
        let old = base.path().join("flat/tasks");
        put(&old, "README.md", "# flat\n");
        put(&old, "tasks.json", "{}\n");
        put(
            &old,
            "0001-a.md",
            &task(
                "0001",
                "stage: ready\nstatus: todo\nclaimed-by:\nheartbeat:\nwaiting_on:",
            ),
        );
        put(&old, "0002-b.md", &task("0002", "status: done"));
        put(&old, "backlog/2026-10-12-0900-note.md", "A note\n");
        put(&old, "backlog/dropped/x.md", "x\n");
        let main = repo(base.path(), "flat");
        let r = migrate(&old, &main, &Options::default(), &tools(), T).unwrap();
        let q = r.queue;
        let s = state(&q);
        assert_eq!(s.problems, Vec::<String>::new());
        assert_eq!((s.tasks.len(), s.backlog.len()), (2, 1));
        assert!(q.join("backlog/dropped/x.md").is_file());
        assert_eq!(layout::read_config(&q), Ok(layout::Config { local: false }));
        assert!(fs::read_to_string(q.join("migrated-from.txt"))
            .unwrap()
            .contains("no git history of its own"));
        assert!(!fs::read_to_string(q.join("README.md"))
            .unwrap()
            .starts_with("---"));
        // `muster check` on it: clean.
        let code = crate::cli::main(&[
            "check".to_string(),
            "--queue".to_string(),
            q.to_string_lossy().to_string(),
        ]);
        assert_eq!(code, 0);
    }

    #[test]
    fn the_review_of_record_and_its_lessons_are_copied_and_left_in_the_old_queue() {
        let base = tempfile::tempdir().unwrap();
        let old = base.path().join("rev/tasks");
        put(&old, "README.md", "# rev\n");
        put(&old, "tasks.json", "{}\n");
        put(&old, "0001-a.md", &task("0001", "stage: ready"));
        let review = r#"{"task":"0001","head":"abc1234","rounds":[],"findings":[]}"#;
        let book = r#"{"lessons":[{"id":"L1","kind":"unbounded-output","for":"builders","state":"proposed","rule":"Cap it.","proposed":"2026-10-13 08:00"}]}"#;
        put(&old, "reviews/0001.json", review);
        put(&old, "reviews/lessons.json", book);
        put(&old, "lessons.md", "# Lessons from reviews\n");
        let before = snapshot(&old);
        let main = repo(base.path(), "rev");
        let r = migrate(
            &old,
            &main,
            &Options {
                local: true,
                dry_run: false,
            },
            &tools(),
            T,
        )
        .unwrap();
        let q = r.queue;
        assert_eq!(
            fs::read_to_string(crate::layout::reviews_dir(&q).join("0001.json")).unwrap(),
            review
        );
        assert_eq!(
            fs::read_to_string(crate::layout::reviews_dir(&q).join("lessons.json")).unwrap(),
            book
        );
        assert_eq!(
            fs::read_to_string(crate::layout::lessons_file(&q)).unwrap(),
            "# Lessons from reviews\n"
        );
        assert!(q.ends_with(".muster/") || q.ends_with(".muster"));
        assert_eq!(
            crate::lessons::waiting(&q),
            1,
            "the copied lessons read in the new queue"
        );
        assert!(
            r.lines.iter().any(|l| l == "reviews: 3 files (the review of record and its lessons) into reviews/ and lessons.md"),
            "{:?}",
            r.lines
        );
        assert!(
            !r.lines.iter().any(|l| l.contains("not copied")),
            "{:?}",
            r.lines
        );
        assert_eq!(snapshot(&old), before, "copied, never moved");
        // Local: git sees none of it.
        let status = git(&main, &["status", "--porcelain", "--untracked-files=all"]);
        assert!(
            !status.contains("reviews") && !status.contains("lessons"),
            "{status}"
        );
    }

    #[test]
    fn refuses_while_an_agent_holds_building_or_testing_and_writes_nothing() {
        let base = tempfile::tempdir().unwrap();
        let old = base.path().join("p/tasks");
        for f in STATUS_FOLDERS {
            fs::create_dir_all(old.join(f)).unwrap();
        }
        put(&old, "README.md", "q\n");
        put(
            &old,
            "doing/0001-a.md",
            &task(
                "0001",
                "stage: building\nstatus: doing\nclaimed-by: wt-a (main · session aaaa1111)",
            ),
        );
        put(
            &old,
            "doing/0002-b.md",
            &task("0002", "stage: testing\nstatus: doing\nheld-by: qa"),
        );
        put(
            &old,
            "todo/0003-c.md",
            &task("0003", "stage: ready\nstatus: todo"),
        );
        // From before held-by (0030): the agent testing it is only in claimed-by.
        put(
            &old,
            "doing/0004-d.md",
            &task("0004", "stage: testing\nstatus: doing\nclaimed-by: wt-b"),
        );
        let main = repo(base.path(), "p");
        for dry_run in [false, true] {
            let opts = Options {
                local: false,
                dry_run,
            };
            let e = migrate(&old, &main, &opts, &tools(), T).unwrap_err();
            assert!(
                e.starts_with("not while an agent holds a task in Building or Testing"),
                "{e}"
            );
            assert!(
                e.contains("0001 (Building, held by wt-a (main · session aaaa1111))"),
                "{e}"
            );
            assert!(e.contains("0002 (Testing, held by qa)"), "{e}");
            assert!(e.contains("0004 (Testing, held by wt-b)"), "{e}");
            assert!(!e.contains("0003"), "{e}");
            assert!(!main.join(layout::DIR).exists());
        }
        assert_eq!(
            fs::read_dir(&main).unwrap().count(),
            2,
            "only README.md and .git: no staging folder left"
        );
    }

    #[test]
    fn a_dry_run_prints_the_plan_and_writes_nothing() {
        let base = tempfile::tempdir().unwrap();
        let old = v2_queue(base.path());
        let main = repo(base.path(), "proj");
        let before = snapshot(&old);
        let opts = Options {
            local: false,
            dry_run: true,
        };
        let r = migrate(&old, &main, &opts, &tools(), T).unwrap();
        assert!(
            r.lines[0].starts_with("dry run, nothing written: would copy"),
            "{:?}",
            r.lines
        );
        assert!(r.lines.join("\n").contains("tasks: 10 into tasks/"));
        assert!(!main.join(layout::DIR).exists());
        assert_eq!(snapshot(&old), before);
    }

    #[test]
    fn refuses_what_it_cant_migrate() {
        let base = tempfile::tempdir().unwrap();
        let main = repo(base.path(), "p");
        let opts = Options::default();
        let not = base.path().join("not");
        fs::create_dir(&not).unwrap();
        let e = migrate(&not, &main, &opts, &tools(), T).unwrap_err();
        assert!(e.contains("isn't a Muster queue"), "{e}");
        let old = base.path().join("p/tasks");
        put(&old, "README.md", "q\n");
        put(&old, "todo/0001-a.md", &task("0001", "status: todo"));
        put(&old, "doing/0001-a.md", &task("0001", "status: doing"));
        let e = migrate(&old, &main, &opts, &tools(), T).unwrap_err();
        assert!(e.contains("would both be .muster/tasks/0001-a.md"), "{e}");
        fs::remove_file(old.join("doing/0001-a.md")).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let e = migrate(&old, outside.path(), &opts, &tools(), T).unwrap_err();
        assert!(e.contains("isn't in a git repo"), "{e}");
        let q = crate::projects::init(&main, false, None, &[]).unwrap().0;
        let e = migrate(&old, &main, &opts, &tools(), T).unwrap_err();
        assert!(e.contains("already exists"), "{e}");
        let e = migrate(&q, &main, &opts, &tools(), T).unwrap_err();
        assert!(e.contains("already a .muster/ queue"), "{e}");
        assert!(!main.join(layout::DIR).join("tasks/0001-a.md").exists());
    }

    /// An old v2 queue under `<base>/<name>/tasks` with these files, and the project's repo.
    fn queue_with(base: &Path, name: &str, files: &[(&str, &str)]) -> (PathBuf, PathBuf) {
        let old = base.join(name).join("tasks");
        for f in STATUS_FOLDERS {
            fs::create_dir_all(old.join(f)).unwrap();
        }
        put(&old, "README.md", "q\n");
        for (rel, text) in files {
            put(&old, rel, text);
        }
        (old, repo(base, name))
    }

    #[test]
    fn names_that_are_one_file_on_this_mac_are_refused_in_the_plan_never_overwritten() {
        let base = tempfile::tempdir().unwrap();
        // Case: one file on a case-insensitive disk.
        let (old, main) = queue_with(
            base.path(),
            "case",
            &[
                ("todo/0005-Fix.md", &task("0005", "status: todo")),
                ("doing/0005-fix.md", &task("0005", "status: doing")),
            ],
        );
        for dry_run in [true, false] {
            let opts = Options {
                local: true,
                dry_run,
            };
            let e = migrate(&old, &main, &opts, &tools(), T).unwrap_err();
            assert!(e.contains("todo/0005-Fix.md"), "{e}");
            assert!(e.contains("doing/0005-fix.md"), "{e}");
            assert!(!main.join(layout::DIR).exists());
        }
        // Accents written two ways (composed and decomposed é): one file on APFS too.
        let (old, main) = queue_with(
            base.path(),
            "accents",
            &[
                ("todo/0006-caf\u{e9}.md", &task("0006", "status: todo")),
                ("done/0006-cafe\u{301}.md", &task("0006", "status: done")),
            ],
        );
        for dry_run in [true, false] {
            let opts = Options {
                local: true,
                dry_run,
            };
            let e = migrate(&old, &main, &opts, &tools(), T).unwrap_err();
            assert!(e.contains("differ only in case or accents"), "{e}");
            assert!(!main.join(layout::DIR).exists());
        }
    }

    #[test]
    fn an_unreadable_folder_stops_it_naming_the_folder() {
        use std::os::unix::fs::PermissionsExt;
        let base = tempfile::tempdir().unwrap();
        let (old, main) = queue_with(
            base.path(),
            "p",
            &[
                ("done/0001-a.md", &task("0001", "status: done")),
                ("attachments/0001/shot.png", "png"),
            ],
        );
        let mode = |p: &Path, m: u32| fs::set_permissions(p, fs::Permissions::from_mode(m));
        for rel in ["done", "attachments/0001"] {
            mode(&old.join(rel), 0o000).unwrap();
            let e = migrate(&old, &main, &Options::default(), &tools(), T);
            mode(&old.join(rel), 0o755).unwrap();
            let e = e.unwrap_err();
            assert!(e.contains(&old.join(rel).display().to_string()), "{e}");
            assert!(!main.join(layout::DIR).exists());
        }
    }

    #[test]
    fn big_and_crlf_task_files_keep_their_folders_stage_and_one_it_cant_read_stops_it() {
        let base = tempfile::tempdir().unwrap();
        let big = format!(
            "{}\n{}",
            task("0001", "status: done"),
            "x".repeat(1_100_000)
        );
        let crlf = task("0002", "status: done").replace('\n', "\r\n");
        // Plain front matter, a CRLF line in the body: it reads as it is, so only its stage and
        // status lines change.
        let mixed = format!(
            "{}A pasted line\r\nand another\r\n",
            task("0003", "status: done")
        );
        let (old, main) = queue_with(
            base.path(),
            "p",
            &[
                ("done/0001-big.md", &big),
                ("done/0002-crlf.md", &crlf),
                ("done/0003-mixed.md", &mixed),
            ],
        );
        let r = migrate(&old, &main, &Options::default(), &tools(), T).unwrap();
        for f in [
            "tasks/0001-big.md",
            "tasks/0002-crlf.md",
            "tasks/0003-mixed.md",
        ] {
            let text = fs::read_to_string(r.queue.join(f)).unwrap();
            assert!(
                text.contains("\nstage: done\n") && !text.contains("status:"),
                "{f}"
            );
        }
        let out = fs::read_to_string(r.queue.join("tasks/0003-mixed.md")).unwrap();
        assert_eq!(
            out.replace("\nstage: done\n---\n", "\nstatus: done\n---\n"),
            mixed,
            "only the stage and status lines change"
        );
        let text = r.lines.join("\n");
        assert!(
            text.contains("note: done/0002-crlf.md: its front matter's CRLF line ends made LF"),
            "{text}"
        );
        assert!(!text.contains("0003-mixed"), "{text}");
        // Without front matter there's no stage to keep: refused, naming each.
        let (old, main) = queue_with(
            base.path(),
            "q",
            &[
                ("todo/0003-broken.md", "no front matter at all\n"),
                ("todo/0004-ok.md", &task("0004", "status: todo")),
            ],
        );
        let e = migrate(&old, &main, &Options::default(), &tools(), T).unwrap_err();
        assert!(
            e.contains("can't read the front matter of todo/0003-broken.md"),
            "{e}"
        );
        assert!(!e.contains("0004"), "{e}");
        assert!(!main.join(layout::DIR).exists());
    }

    #[test]
    fn refuses_a_repo_that_is_the_old_queue_or_holds_it() {
        let base = tempfile::tempdir().unwrap();
        // The old queue's own git: `.muster` would land inside it.
        let (old, _) = queue_with(
            base.path(),
            "p",
            &[("todo/0001-a.md", &task("0001", "status: todo"))],
        );
        git(&old, &["init", "-q", "-b", "main"]);
        let e = migrate(&old, &old, &Options::default(), &tools(), T).unwrap_err();
        assert!(e.contains("overlap"), "{e}");
        assert!(!old.join(layout::DIR).exists());
        // An old queue inside the repo it would go into.
        let r = base.path().join("inrepo");
        fs::create_dir(&r).unwrap();
        git(&r, &["init", "-q", "-b", "main"]);
        let inner = r.join("tasks");
        fs::create_dir_all(inner.join("todo")).unwrap();
        put(&inner, "README.md", "q\n");
        let e = migrate(&inner, &r, &Options::default(), &tools(), T).unwrap_err();
        assert!(e.contains("overlap"), "{e}");
        assert!(!r.join(layout::DIR).exists());
    }

    #[test]
    fn probe_folders_taken_in_a_row_all_differ_and_keep_the_pid() {
        let (a, b) = (probe_dir(), probe_dir());
        assert_ne!(a, b);
        // Many in a row, so a name from the clock (µs on macOS) repeats too.
        let many: Vec<PathBuf> = (0..10_000).map(|_| probe_dir()).collect();
        assert_eq!(many.iter().collect::<HashSet<_>>().len(), many.len());
        let pid = format!("muster-migrate-probe-{}-", std::process::id());
        for d in [&a, &b].into_iter().chain(&many) {
            assert_eq!(d.parent(), Some(std::env::temp_dir().as_path()));
            let name = d.file_name().unwrap().to_string_lossy();
            assert!(name.starts_with(&pid), "{name}");
        }
    }

    #[test]
    fn eight_probes_of_one_plan_at_once_are_all_ok_and_leave_nothing() {
        let mut p = Plan::default();
        for i in 0..40 {
            let rel = format!("todo/{i:04}-t.md");
            let item = Item {
                src: PathBuf::from(&rel),
                rel,
                text: None,
            };
            p.items
                .insert(Path::new("tasks").join(format!("{i:04}-t.md")), item);
        }
        let p = std::sync::Arc::new(p);
        let runs: Vec<_> = (0..8)
            .map(|_| {
                let p = p.clone();
                std::thread::spawn(move || {
                    let dir = probe_dir();
                    (probe(&p, &dir), dir)
                })
            })
            .collect();
        for run in runs {
            let (result, dir) = run.join().unwrap();
            assert_eq!(result, Ok(()));
            assert!(!dir.exists(), "{}", dir.display());
        }
    }
}
