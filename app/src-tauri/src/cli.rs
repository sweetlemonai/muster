//! `muster` on the command line: the same rules as the app (`ops`, `queue`, `projects`).
//! The queue is `--queue <name|folder>`, else `$MUSTER_ROOT`, else the nearest queue found
//! from the current folder (a queue itself, or a `tasks/` beside or above it).

use crate::session::{self, Session};
use crate::{
    accounts, agents, backlog, check_limit, fields, index, lessons, old_pause, ops, pause, ping,
    projects, queue, review, search, settings, vision,
};
use std::path::{Path, PathBuf};

/// The environment a command reads its caller from (`std::env::var`, or a test's own).
type Env<'a> = &'a dyn Fn(&str) -> Option<String>;

/// A command's stdout (0170). Every line a command prints goes through `out!`/`outln!` to this
/// one writer, never the std print macros, which panic when the reader has left
/// (`muster show mu-1 | head -1`). A write that fails ends the output and nothing else: the
/// command finishes its queue writes and exits with its own status. A reader that left
/// (`BrokenPipe`) is quiet; any other failure (a full disk) is said once on stderr when the
/// command has ended (`failure_line`) and turns a status of 0 into 1, so a script never takes
/// cut-off output for the whole of it.
struct Stdout {
    ended: std::sync::atomic::AtomicBool,
    /// The failure that ended the output, unless it was a reader that left.
    error: std::sync::OnceLock<String>,
    /// The command, when it may write to the queue (`reads_only` says it doesn't).
    writer: std::sync::OnceLock<String>,
}

impl Stdout {
    const fn new() -> Self {
        Self {
            ended: std::sync::atomic::AtomicBool::new(false),
            error: std::sync::OnceLock::new(),
            writer: std::sync::OnceLock::new(),
        }
    }

    /// Writes `s` to `w`, unless an earlier write failed.
    fn write(&self, w: &mut impl std::io::Write, s: &str) {
        if !self.ended() {
            let done = w.write_all(s.as_bytes());
            self.settle(done);
        }
    }

    /// Sends on what `w` still buffers (the last line, when it has no newline).
    fn flush(&self, w: &mut impl std::io::Write) {
        if !self.ended() {
            let done = w.flush();
            self.settle(done);
        }
    }

    fn settle(&self, done: std::io::Result<()>) {
        use std::sync::atomic::Ordering::Relaxed;
        let Err(e) = done else { return };
        if self.ended.swap(true, Relaxed) || e.kind() == std::io::ErrorKind::BrokenPipe {
            return;
        }
        let _ = self.error.set(e.to_string());
    }

    /// Marks the command as one that may write to the queue.
    fn writes(&self, cmd: &str) {
        let _ = self.writer.set(cmd.to_string());
    }

    fn ended(&self) -> bool {
        self.ended.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn failed(&self) -> bool {
        self.error.get().is_some()
    }

    /// The stderr line for output that failed, once the command ended with `code`. A command
    /// that may write and succeeded says its change is saved, so nobody runs it again for the
    /// lost output (a second note, a refused second pass); a read or a refusal says only that
    /// the output failed.
    fn failure_line(&self, code: i32) -> Option<String> {
        let e = self.error.get()?;
        Some(match self.writer.get() {
            Some(cmd) if code == 0 => format!(
                "muster: {cmd} ran and its change is saved, but its output couldn't be written ({e}): don't run it again"
            ),
            _ => format!("muster: can't write the output: {e}"),
        })
    }
}

static STDOUT: Stdout = Stdout::new();

/// Does this command only read (0170)? Anything not known to only read counts as a write, so
/// lost output never leads to the same write done twice.
fn reads_only(a: &Args) -> bool {
    let sub = a.pos.get(1).map(String::as_str);
    let has = |keys: &[&str]| keys.iter().any(|k| a.flags.contains_key(*k));
    match a.pos.first().map_or("help", String::as_str) {
        "help" | "-h" | "--help" | "team" | "instructions" | "list" | "show" | "ready" => true,
        "projects" => sub != Some("set"),
        "history" | "lessons" => sub.is_none(),
        "inbox" => a.keep,
        "index" => a.check,
        "check" => !a.fix_pauses || a.dry_run,
        "shape" => a.peek,
        "vision" => !has(&["record", "resolve"]),
        "review" => !has(&["record", "fixed", "dispute", "settle"]),
        _ => false,
    }
}

/// One piece of a command's output, through `STDOUT`. Unit tests drop it: a direct write to
/// stdout would land in the test runner's own output, past its capture.
fn out_str(s: &str) {
    if cfg!(test) {
        STDOUT.write(&mut std::io::sink(), s);
    } else {
        STDOUT.write(&mut std::io::stdout().lock(), s);
    }
}

/// `print!` for a command's output, through `out_str`.
macro_rules! out {
    ($($arg:tt)*) => {
        out_str(&format!($($arg)*))
    };
}

/// `println!` for a command's output, through `out_str`.
macro_rules! outln {
    ($($arg:tt)*) => {
        out_str(&format!("{}\n", format_args!($($arg)*)))
    };
}

#[derive(Debug, Clone)]
struct Args {
    pos: Vec<String>,
    flags: std::collections::HashMap<String, String>,
    json: bool,
    keep: bool,
    check: bool,
    /// `vision --all`: the whole queue.
    all: bool,
    /// `--blocked`: `list` keeps blocked tasks only.
    blocked: bool,
    human: bool,
    /// `ready --parallel [N]`, `start --here`, `release --clean [--force]` (0023).
    parallel: bool,
    here: bool,
    clean: bool,
    force: bool,
    /// `ask --option <text>`, once per option, in order (0049).
    options: Vec<String>,
    /// `init --local`, `migrate --local` (0005): a queue that stays on this Mac.
    local: bool,
    /// `init --committed`, `migrate --committed`: a queue committed with the code.
    committed: bool,
    /// `migrate --dry-run`, `check --fix-pauses --dry-run`: the plan, nothing written.
    dry_run: bool,
    /// `check --fix-pauses` (0059): the old paused blocks become pauses, in every queue.
    fix_pauses: bool,
    /// `claim --takeover` (0090): take a claim under this label over from another session.
    takeover: bool,
    /// `checkin --handoff` (0090): the holder's last check-in; its successor may take over.
    handoff: bool,
    /// `clarify --send` (mu-103): escalate the batch to the owner.
    send: bool,
    /// `shape --peek` (0097): the brief only, the note not claimed.
    peek: bool,
}

/// Options and positionals; after `--`, everything is positional (text that starts with `--`).
fn parse(argv: &[String]) -> Result<Args, String> {
    let mut a = Args {
        pos: vec![],
        flags: Default::default(),
        json: false,
        keep: false,
        check: false,
        all: false,
        blocked: false,
        human: false,
        parallel: false,
        here: false,
        clean: false,
        force: false,
        options: vec![],
        local: false,
        committed: false,
        dry_run: false,
        fix_pauses: false,
        takeover: false,
        handoff: false,
        send: false,
        peek: false,
    };
    let mut it = argv.iter();
    let mut rest = false;
    while let Some(x) = it.next() {
        if rest {
            a.pos.push(x.clone());
            continue;
        }
        match x.as_str() {
            "--" => rest = true,
            "--json" => a.json = true,
            "--keep" => a.keep = true,
            "--check" => a.check = true,
            "--all" => a.all = true,
            "--blocked" => a.blocked = true,
            "--human" => a.human = true,
            "--parallel" => a.parallel = true,
            "--here" => a.here = true,
            "--clean" => a.clean = true,
            "--force" => a.force = true,
            "--option" => a
                .options
                .push(it.next().ok_or("--option needs a value")?.clone()),
            "--local" => a.local = true,
            "--committed" => a.committed = true,
            "--dry-run" => a.dry_run = true,
            "--fix-pauses" => a.fix_pauses = true,
            "--takeover" => a.takeover = true,
            "--handoff" => a.handoff = true,
            "--send" => a.send = true,
            "--peek" => a.peek = true,
            "--queue" | "--as" | "--type" | "--priority" | "--body" | "--status" | "--on"
            | "--tags" | "--tag" | "--time" | "--test-time" | "--tokens" | "--stage" | "--gate"
            | "--commit" | "--to" | "--from" | "--after" | "--session" | "--level" | "--parent"
            | "--due" | "--pct" | "--remove" | "--record" | "--resolve" | "--search"
            | "--holder" | "--sort" | "--at" | "--touches" | "--prefix" | "--pick"
            | "--no-answer" | "--fixed" | "--dispute" | "--settle" | "--summary" | "--as-built"
            | "--agent" | "--now-unchanged" | "--role" => {
                let v = it.next().ok_or_else(|| format!("{x} needs a value"))?;
                a.flags
                    .insert(x.trim_start_matches("--").to_string(), v.clone());
            }
            s if s.starts_with("--") => return Err(format!("unknown option {s}")),
            _ => a.pos.push(x.clone()),
        }
    }
    Ok(a)
}

/// Local time as `YYYY-MM-DD HH:MM`, from the system clock, so agents never type times.
pub fn local_stamp() -> String {
    queue::local_stamp()
}

/// projects.json, where `projects::places` says it is.
pub fn config_file() -> PathBuf {
    projects::places().config_file
}

/// This machine's settings (0036): `settings.json` beside `projects.json` under the caller's
/// `$HOME`, which the app writes and both read. The defaults without a home (a test's empty
/// environment), so nothing on this Mac changes what a test sees.
fn machine_settings(env: Env) -> settings::Settings {
    env("HOME")
        .filter(|h| !h.is_empty())
        .map(|h| {
            settings::load(
                &Path::new(&h)
                    .join("Library/Application Support/ai.sweetlemon.muster")
                    .join(settings::FILE),
            )
        })
        .unwrap_or_default()
}

/// What `muster migrate` carries in projects.json from the old queue to the new (0005, with 0050's
/// limits): worked out again under projects.json's lock and saved there (`projects::edit`), so an
/// edit the app makes meanwhile is kept. With `dry_run`, or nothing to carry, nothing is written.
fn carry_projects(
    file: &Path,
    base: &Path,
    old: &Path,
    new: &Path,
    dry_run: bool,
) -> Result<Vec<String>, String> {
    let lines = projects::carry_over(&mut projects::load(file), base, old, new);
    if dry_run || lines.is_empty() {
        return Ok(lines);
    }
    projects::edit(file, |c| Ok(projects::carry_over(c, base, old, new)))
}

/// The line `next`, `start`, `ready --parallel` and `list` print on stderr when more waits in Your
/// check than the project's limit allows (0050), or None. It only warns: nothing is refused. The
/// limit is projects.json's for the queue's folder (`config`), 5 unless set.
fn your_check_line(
    root: &Path,
    tasks: &[queue::Task],
    config: &projects::Config,
) -> Option<String> {
    let limit = check_limit::limit_for(&config.your_check, root);
    check_limit::over_line(check_limit::waiting(tasks), limit)
}

/// `your_check_line` on stderr, for `next` and `start`: the queue read again, as they read it.
fn warn_your_check(root: &Path, prefix: &str) {
    let config = projects::load(&config_file());
    if let Some(l) = your_check_line(root, &keyed(root, prefix).tasks, &config) {
        eprintln!("{l}");
    }
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

/// `init` and `migrate` (0005): a local queue unless `--committed` says otherwise. Local is the
/// default until a committed queue's live state moves to `.git/muster/`: until then every claim
/// and pass rewrites files that git tracks.
fn local_or_committed(a: &Args) -> Result<bool, String> {
    if a.local && a.committed {
        return Err("--local and --committed: pick one".into());
    }
    Ok(!a.committed)
}

/// Muster's tools (`bin/`): what an old queue's stand-ins run, and `manifest.py` for `migrate`.
fn tools_dir() -> PathBuf {
    std::env::var_os("MUSTER_TOOLS")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join("playground/muster/main/bin"))
}

/// The folder queues are found under, from `projects::places`.
pub(crate) fn base() -> PathBuf {
    projects::places().base
}

/// Where the queue a command runs on came from.
#[derive(Debug, Clone, PartialEq)]
enum Source {
    /// `--queue`: always what the caller meant.
    Flag,
    /// `$MUSTER_ROOT` (or the older `$TASKS_ROOT`), named.
    Env(&'static str),
    /// The current folder's own queue.
    Here,
    /// A task's key (0033): the project its prefix names, whatever the folder or the environment.
    Key,
}

/// The queue root from the environment, if set (and not empty).
fn env_root() -> Option<(&'static str, PathBuf)> {
    ["MUSTER_ROOT", "TASKS_ROOT"].into_iter().find_map(|var| {
        std::env::var_os(var)
            .filter(|v| !v.is_empty())
            .map(|v| (var, PathBuf::from(v)))
    })
}

/// The queue a folder belongs to: its repo's `.muster/` (the main checkout's, from any worktree,
/// task 0005); else the folder itself or one above it, or a `tasks/` beside it or beside a folder
/// above it (so a repo, or a worktree inside it, finds `<project>/tasks`).
fn queue_of(cwd: &Path) -> Option<PathBuf> {
    if let Some(q) = crate::layout::find(cwd) {
        return Some(q);
    }
    cwd.ancestors().find_map(|dir| {
        if projects::looks_like_queue(dir) {
            return Some(dir.to_path_buf());
        }
        let beside = dir.join("tasks");
        projects::looks_like_queue(&beside).then_some(beside)
    })
}

/// The queue to use: `--queue <name|folder>`, else the environment's, else the current folder's.
/// `known` lists the queues Muster knows, by name (read only when a name needs it).
fn resolve_queue(
    flag: Option<&str>,
    env: Option<(&'static str, PathBuf)>,
    cwd: Option<&Path>,
    known: &dyn Fn() -> Vec<(String, PathBuf)>,
) -> Result<(PathBuf, Source), String> {
    if let Some(q) = flag {
        let p = Path::new(q);
        if p.is_dir() && projects::looks_like_queue(p) {
            return Ok((p.to_path_buf(), Source::Flag));
        }
        return known()
            .into_iter()
            .find(|(n, _)| n == q)
            .map(|(_, p)| (p, Source::Flag))
            .ok_or_else(|| format!("no queue named {q:?} (see `muster projects`)"));
    }
    if let Some((var, root)) = env {
        return Ok((root, Source::Env(var)));
    }
    let cwd = cwd.ok_or("can't read the current folder: pass --queue <name|folder>")?;
    queue_of(cwd)
        .map(|q| (q, Source::Here))
        .ok_or_else(|| "no queue here: pass --queue <name|folder> or set MUSTER_ROOT".into())
}

/// Commands that claim, hand on or hold a task, rewrite its write-up, and the backlog's writes
/// (add, file, drop). A slip of the environment would do them in another project's queue (it
/// happened: drover's 0022 claimed from muster), so they're checked. `start` and `release` (0023)
/// claim and give up too, and `pause` and `resume` (0045) hold and free one.
const TARGETED: [&str; 17] = [
    "claim",
    "next",
    "start",
    "release",
    "pass",
    "return",
    "block",
    "unblock",
    "pause",
    "resume",
    "stage",
    "heartbeat",
    "checkin",
    "add",
    "file",
    "drop",
    "edit",
];

/// Is this command checked like a claim (`check_target`)? The TARGETED ones, `vision` when it
/// writes (`--record`, `--resolve`; its brief only reads), `review` when it writes (0027),
/// `lessons accept|reject`, and `shape` unless `--peek` (0097: it claims the note).
fn targeted(cmd: &str, a: &Args) -> bool {
    let writes = |keys: &[&str]| keys.iter().any(|k| a.flags.contains_key(*k));
    TARGETED.contains(&cmd)
        || (cmd == "shape" && !a.peek)
        || (cmd == "vision" && writes(&["record", "resolve"]))
        || (cmd == "review" && writes(&["record", "fixed", "dispute", "settle"]))
        || (cmd == "lessons" && a.pos.len() > 1)
}

/// The same folder, through symlinks too.
fn same_folder(a: &Path, b: &Path) -> bool {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    canon(a) == canon(b)
}

/// `drover/tasks`, `muster/main/.muster`: a queue as people name it, its project folder (and
/// checkout) and its own.
fn short_queue(root: &Path) -> String {
    let name = |p: Option<&Path>| {
        p.and_then(Path::file_name)
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    };
    // A `.muster` names its checkout too: the project folder alone would read like its old queue.
    let up = if crate::layout::is_muster(root) { 2 } else { 1 };
    let mut parts: Vec<String> = root
        .ancestors()
        .take(up + 1)
        .map(|p| name(Some(p)))
        .collect();
    if parts.len() <= up || parts.iter().any(String::is_empty) {
        return root.display().to_string();
    }
    parts.reverse();
    parts.join("/")
}

/// Refuse a targeted command on a queue that came from the environment while the current folder
/// has a different queue of its own. `--queue` always wins, and a folder without a queue is fine.
fn check_target(
    source: &Source,
    root: &Path,
    cwd: Option<&Path>,
    known: &dyn Fn() -> Vec<(String, PathBuf)>,
) -> Result<(), String> {
    let Source::Env(var) = source else {
        return Ok(());
    };
    let Some(here) = cwd.and_then(queue_of) else {
        return Ok(());
    };
    if same_folder(&here, root) {
        return Ok(());
    }
    let name = known()
        .into_iter()
        .find(|(_, p)| same_folder(p, root))
        .map(|(n, _)| n)
        .unwrap_or_else(|| root.display().to_string());
    Err(format!(
        "{var} points at {} but you're in {}/…; pass --queue {name} to mean it",
        short_queue(root),
        projects::default_name(&here)
    ))
}

/// Commands whose first word after the command is a task (0033: `mu-12`, `MU-12`, `12` or `0012`).
const TAKES_A_TASK: [&str; 28] = [
    "review",
    "show",
    "move",
    "pass",
    "return",
    "block",
    "unblock",
    "stage",
    "set",
    "note",
    "tick",
    "untick",
    "edit",
    "link",
    "comment",
    "attach",
    "answer",
    "claim",
    "start",
    "release",
    "heartbeat",
    "checkin",
    "ask",
    "escalate",
    "vision",
    "ping",
    "pause",
    "resume",
];

/// Where the task is in `a.pos` for this command, when it takes one and was given it. `release`
/// and `claim --takeover` take a backlog note's file name too (0097), which isn't a task.
fn task_arg(cmd: &str, a: &Args) -> Option<usize> {
    let takes = TAKES_A_TASK.contains(&cmd) && !(cmd == "vision" && a.all);
    let note = (cmd == "release" || (cmd == "claim" && a.takeover))
        && a.pos.get(1).is_some_and(|p| names_a_note(p));
    (takes && !note && a.pos.len() > 1).then_some(1)
}

/// A `.md` file name (any case): what `release` reads as a backlog note rather than a task.
fn names_a_note(arg: &str) -> bool {
    arg.to_ascii_lowercase().ends_with(".md")
}

/// The queue a key names (0033): the one `--queue` names, refused unless it's that project's;
/// else the environment's or the current folder's, when it's that project's; else the project
/// Muster knows by that prefix.
fn key_queue(
    typed: &str,
    prefix: &str,
    flag: Option<&str>,
    env: Option<(&'static str, PathBuf)>,
    cwd: Option<&Path>,
    known: &dyn Fn() -> Vec<(String, PathBuf)>,
) -> Result<(PathBuf, Source), String> {
    let owner = || crate::key::owner_of(&crate::key::known(), prefix).cloned();
    if let Some(q) = flag {
        let (root, source) = resolve_queue(Some(q), None, None, known)?;
        let has = crate::key::prefix_for(&root);
        if has == prefix {
            return Ok((root, source));
        }
        let whose = owner()
            .map(|p| format!("{}'s task", p.name))
            .unwrap_or_else(|| format!("a task of the project with the prefix {prefix}"));
        return Err(format!(
            "{typed} is {whose}, but --queue {q} is {has}: leave --queue out, or name the task {has}-<number>"
        ));
    }
    if let Ok((root, _)) = resolve_queue(None, env, cwd, known) {
        if crate::key::prefix_for(&root) == prefix {
            return Ok((root, Source::Key));
        }
    }
    owner()
        .map(|p| (PathBuf::from(p.path), Source::Key))
        .ok_or_else(|| format!("no project has the prefix {prefix} (see `muster projects`)"))
}

/// The task lists in a command's options and values (`--parent`, `block --on`, `set <id>
/// depends|parent`), keys made ids; a key of another project is refused.
fn task_lists(a: &mut Args, cmd: &str, prefix: &str, project: &str) -> Result<(), String> {
    let ids = |v: &str| crate::key::same_project_ids(v, prefix, project);
    if let Some(v) = a.flags.get("parent") {
        if let Some(i) = ids(v)? {
            a.flags.insert("parent".into(), i.join(", "));
        }
    }
    // `--on mu-14, 16 [what]`: the tasks it starts with, then what it says about them.
    if let Some(on) = a.flags.get("on") {
        if let Some((i, rest)) = crate::key::leading_tasks(on, prefix, project)? {
            let joined = i.join(",");
            // The ids and the reason apart, so a reason that starts with a digit stays one.
            let on = if rest.is_empty() {
                joined
            } else {
                format!("{joined}: {rest}")
            };
            a.flags.insert("on".into(), on);
        }
    }
    let field = a.pos.get(2).map(String::as_str);
    if cmd == "set" && matches!(field, Some("depends" | "parent")) && a.pos.len() > 3 {
        if let Some(i) = ids(&a.pos[3..].join(" "))? {
            a.pos.truncate(3);
            a.pos.push(i.join(", "));
        }
    }
    Ok(())
}

/// What `return` says after its stage line, for task `k`: the owner finding a QA send-back
/// recorded (mu-101), and whether the builder's session was told, whenever it was meant to be.
fn returned_lines(k: &str, r: &ops::Returned) -> Vec<String> {
    let mut out = Vec::new();
    if !r.finding.is_empty() {
        out.push(format!(
            "{k}: owner finding {} for the builder to fix or dispute; first in line",
            r.finding
        ));
    }
    if !r.not_told.is_empty() {
        out.push(format!(
            "{k}: the builder's session wasn't told ({}); muster next shows it",
            r.not_told
        ));
    } else if !r.told.is_empty() {
        out.push(format!("{k}: told session {}", r.told));
    }
    out
}

/// `claimed mu-19 in muster (/…/muster/tasks)`, then where it is now.
fn claimed_message(root: &Path, prefix: &str, id: &str, path: &str) -> String {
    format!(
        "claimed {} in {} ({})\n{}",
        crate::key::key(prefix, id),
        projects::default_name(root),
        root.display(),
        stage_message(root, prefix, id, path)
    )
}

/// What `muster shape` says of the note's claim (0097), before the brief.
fn claim_line(c: &backlog::Claim) -> String {
    let ends = format!(
        "muster file or drop ends it; muster release {} lets it go",
        backlog::sh(&c.note)
    );
    if c.renewed {
        format!(
            "you're shaping {} (since {}); heartbeat {}. {ends}",
            c.note, c.hold.at, c.hold.heartbeat
        )
    } else if c.took_over.is_empty() {
        format!("you're shaping {} now. {ends}", c.note)
    } else {
        format!(
            "took {} over from {} (no heartbeat for {}); you're shaping it now. {ends}",
            c.note, c.took_over, c.quiet
        )
    }
}

/// What `muster reclaim` says, a line per claim released (tasks, then backlog notes, 0097), or
/// that there was nothing to release, every claim heard from `within`. Failures go to stderr.
fn reclaim_text(r: &ops::Reclaimed, within: &str, key: impl Fn(&str) -> String) -> String {
    let mut out = String::new();
    for x in &r.released {
        let now = if x.stage == "shaping" {
            "unheld, still in Shaping"
        } else {
            "back in Ready"
        };
        out += &format!(
            "released {} from {} (no heartbeat for {}): {now} ({})\n",
            key(&x.id),
            x.holder,
            x.quiet,
            x.path
        );
    }
    for n in &r.notes {
        out += &format!(
            "released backlog note {} from {} (no heartbeat for {}): nobody is shaping it now\n",
            n.note, n.holder, n.quiet
        );
    }
    if r.released.is_empty() && r.failed.is_empty() && r.notes.is_empty() {
        out += &format!("nothing to release: every claim in Shaping and Building, and on a backlog note, was heard from within {within}\n");
    }
    out
}

/// What `muster shape --json` prints: the brief, with the note's claim under `claim` unless it
/// was only peeked at (0097).
fn shape_json(b: &backlog::Brief, claim: Option<&backlog::Claim>) -> serde_json::Value {
    let mut v = serde_json::json!(b);
    if let Some(c) = claim {
        v["claim"] = serde_json::json!(c);
    }
    v
}

/// `filed mu-31 in muster (Shaping)`: what `muster file` made, and where.
fn filed_message(root: &Path, prefix: &str, f: &backlog::Filed) -> String {
    format!(
        "filed {} in {} ({})",
        crate::key::key(prefix, &f.id),
        projects::default_name(root),
        queue::stage_label("shaping")
    )
}

/// What `muster file` warns, task `k` filed, when its body has no line for some of the project's
/// lenses (0076), by name; None when it has one for each.
fn lens_warning(k: &str, missing: &[String]) -> Option<String> {
    let n = match missing.len() {
        0 => return None,
        1 => "1 lens".to_string(),
        n => format!("{n} lenses"),
    };
    Some(format!(
        "warning: {k} has no Lenses line for {n}: {}. It's filed anyway: add one line each under **Lenses:** (what the task must do for it, or \"n/a\" and why) with muster edit {k}, or skip a lens for the whole project with its reason (muster help shape).",
        missing.join(", ")
    ))
}

/// What `muster file` warns, task `k` filed, when its body still has `[NEEDS CLARIFICATION: …]`
/// markers (mu-103), naming them; None when it has none.
fn marker_warning(k: &str, markers: &[String]) -> Option<String> {
    if markers.is_empty() {
        return None;
    }
    Some(format!(
        "warning: {k} has {} in its write-up: {}. It's filed anyway, but it can't pass to Ready until each is decided (under ## Questions, \"Decided by <you>, overrule any time: …\") or asked (muster ask {k} …, the marker replaced with \"(see Q<n>)\") with muster edit {k}.",
        match markers.len() {
            1 => "1 [NEEDS CLARIFICATION] marker".to_string(),
            n => format!("{n} [NEEDS CLARIFICATION] markers"),
        },
        markers.join("; ")
    ))
}

/// `Q3`, `q3` or `3`.
fn question_number(q: &str) -> Result<u32, String> {
    let digits = q.strip_prefix(['Q', 'q']).unwrap_or(q);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("question must look like Q3, got {q:?}"));
    }
    digits
        .parse()
        .map_err(|_| format!("question must look like Q3, got {q:?}"))
}

/// A question's options as `ask` and `escalate` print them: one indented line each, the asker's
/// pick marked, then what it does with no answer.
fn options_text(c: &crate::choices::Choices) -> String {
    let mut out = String::new();
    for (i, o) in c.options.iter().enumerate() {
        let mine = if c.pick == Some(i) { "  ← pick" } else { "" };
        out.push_str(&format!("  ({}) {o}{mine}\n", crate::choices::letter(i)));
    }
    if let Some(i) = c.no_answer {
        out.push_str(&format!(
            "  With no answer: ({})\n",
            crate::choices::letter(i)
        ));
    }
    out
}

/// The session asking or collecting answers: `--session`, else `$CLAUDE_CODE_SESSION_ID`; with
/// a teammate's agent id after it (`<session>/<agent>`, 0090), so its mail is its own.
fn session(a: &Args, env: Env) -> String {
    let s = a
        .flags
        .get("session")
        .filter(|s| !s.is_empty())
        .cloned()
        .or_else(|| env("CLAUDE_CODE_SESSION_ID"))
        .unwrap_or_default();
    match env("MUSTER_AGENT") {
        Some(agent) if !s.is_empty() => format!("{s}/{agent}"),
        _ => s,
    }
}

/// A teammate's agent id (0090): `--agent`, else `$MUSTER_AGENT` when set, cut to its first 8
/// characters (`session::agent_id`). Refused when it isn't one; None for a lead.
fn agent_arg(a: &Args, env: Env) -> Result<Option<String>, String> {
    let raw = match a.flags.get("agent") {
        Some(f) => f.clone(),
        None => match env("MUSTER_AGENT").filter(|v| !v.trim().is_empty()) {
            Some(v) => v,
            None => return Ok(None),
        },
    };
    session::agent_id(&raw).map(Some).ok_or_else(|| {
        "the agent id (--agent or MUSTER_AGENT) must start with 8 hex characters and be only letters, digits and dashes".to_string()
    })
}

/// Who's acting, without the session: `--as`, else `$MUSTER_AS`, else the git user name for the
/// queue's folder, else `owner` (outside a session) or `agent` (in one: a session without a name
/// mustn't pass for the owner, who may hold several claims). Never the login name (`$USER`): it
/// isn't read.
fn label(a: &Args, env: Env, root: &Path) -> String {
    let in_session = env("CLAUDE_CODE_SESSION_ID")
        .and_then(|i| session::short_id(&i))
        .is_some();
    pick_label(
        // A blank --as stays blank, so actor() refuses it rather than guessing who.
        a.flags.get("as").cloned(),
        env("MUSTER_AS").filter(|s| !s.trim().is_empty()),
        || git_name(root),
        in_session,
    )
}

fn pick_label(
    flag: Option<String>,
    muster_as: Option<String>,
    git: impl FnOnce() -> Option<String>,
    in_session: bool,
) -> String {
    flag.or(muster_as)
        .or_else(git)
        .unwrap_or_else(|| if in_session { "agent" } else { "owner" }.into())
}

/// `git config user.name`, run in the queue's folder so the folder's identity applies (personal
/// and work folders have their own), as one safe line; None when git gives nothing usable.
fn git_name(root: &Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["config", "user.name"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    clean_git_name(&String::from_utf8_lossy(&out.stdout))
}

/// A git user name as a label: control characters out, one line, at most 64 characters, and
/// nothing the log would read as a stage change, a check-in or a session stamp.
fn clean_git_name(raw: &str) -> Option<String> {
    let first = raw.lines().next()?;
    let name: String = queue::actor(first).ok()?.chars().take(64).collect();
    let name = name.trim().to_string();
    (!name.is_empty() && session::split(&name).1.is_none()).then_some(name)
}

/// Who's acting as it's written: the label, with the session stamped after it when the command
/// runs in one (0030), and the role slot when the label names one (0112, `Kaveh-2`: `slot
/// coder-2`). A label that named a slot before a rename is the slot this session holds under it
/// (in Building or Testing, in any queue or `here`), so the slot goes on being held. The profiles
/// are read only for a label shaped like a slot; a refused roles.toml stamps no slot rather than
/// stopping the command.
fn stamped_who(label: &str, env: Env, cwd: Option<&Path>, here: Option<&Path>) -> String {
    let session = Session::from_env(env, cwd);
    let slot = crate::roles::looks_like_slot(label)
        .then(|| crate::roles::load(&roles_file()).ok())
        .flatten()
        .and_then(|roles| {
            crate::roles::slot_of_label(&roles, label).or_else(|| {
                let (slot, hold) = session_slot(&roles, &every_task(here), session.as_ref()?)?;
                (session::split(&hold.holder).0 == label.trim()).then_some(slot)
            })
        })
        .map(|s| s.id());
    session::stamped_slot(label, session.as_ref(), slot.as_deref())
}

/// The role slot this session (and agent) holds, in Building or Testing, and the task holding it
/// (0112); None when it holds none.
fn session_slot(
    roles: &[crate::roles::Role],
    tasks: &[queue::Task],
    me: &Session,
) -> Option<(crate::roles::Slot, crate::roles::Hold)> {
    let me = Some((me.id.clone(), me.agent.clone().unwrap_or_default()));
    crate::roles::held(roles, tasks)
        .into_iter()
        .find_map(|(slot, holds)| {
            holds
                .into_iter()
                .find(|h| session::session_of(&h.holder) == me)
                .map(|h| (slot, h))
        })
}

/// roles.toml (0112), beside projects.json.
fn roles_file() -> PathBuf {
    crate::roles::file_beside(&config_file())
}

/// Every task in Muster's queues, and in `here` when it's a queue they don't list, for the slots
/// they hold (0112). Each queue is read once.
fn every_task(here: Option<&Path>) -> Vec<queue::Task> {
    let cfg = projects::load(&config_file());
    let list = projects::list(&base(), &cfg);
    let mut tasks = projects::read_state(
        &cfg,
        &list,
        &crate::settings::Settings::default(),
        &mut queue::Cache::default(),
    )
    .tasks;
    if let Some(root) = here {
        let same = |p: &Path| match (p.canonicalize(), root.canonicalize()) {
            (Ok(x), Ok(y)) => x == y,
            _ => p == root,
        };
        if !projects::active(&base(), &cfg).iter().any(|(_, p)| same(p)) {
            tasks.extend(keyed(root, &crate::key::prefix_for(root)).tasks);
        }
    }
    tasks
}

/// `--role <role>` on claim, next and start (0112): the label becomes the role's lowest free
/// slot (`Kaveh-2`), so the claim is stamped with it. One session (and agent) is one slot: when
/// a task in Building or Testing already holds a slot of this role in the caller's session, that
/// slot is the label again, so a second claim is refused as any is (one claim per holder) and a
/// re-run of start resumes; a slot of another role held in the session refuses it. Without a
/// session nothing tells callers apart, so it's the lowest free slot. Refused beside `--as` or
/// `--takeover`, on another command, for a role that isn't one, and when every slot of the role
/// is held.
fn role_label(
    a: &Args,
    cmd: &str,
    root: &Path,
    env: Env,
    cwd: Option<&Path>,
) -> Result<Option<String>, String> {
    let Some(role) = a.flags.get("role") else {
        return Ok(None);
    };
    if !matches!(cmd, "claim" | "next" | "start") {
        return Err(format!(
            "--role is for claim, next and start, not {cmd} (see `muster help`)"
        ));
    }
    if a.flags.contains_key("as") {
        return Err("--role names who you are, so it doesn't go with --as: the slot it hands out is your --as from then on (see `muster help`)".into());
    }
    if a.takeover {
        return Err("--takeover keeps the claim's name: use --as <the slot's name>, not --role (see `muster help`)".into());
    }
    let roles = crate::roles::load(&roles_file())?;
    let r = crate::roles::find(&roles, role).ok_or_else(|| {
        format!(
            "unknown role {role:?}: one of {} (muster team) (see `muster help`)",
            roles
                .iter()
                .map(|r| format!("{} ({})", r.id, r.name))
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;
    let tasks = every_task(Some(root));
    let held = crate::roles::held(&roles, &tasks);
    if let Some(me) = Session::from_env(env, cwd) {
        if let Some((slot, h)) = session_slot(&roles, &tasks, &me) {
            let name = crate::roles::slot_name(&roles, &slot);
            if slot.role != r.id {
                return Err(format!(
                    "this session is {name} already ({} holds it): one session is one slot, so act as {name} (--as {name}), or start another session for {}",
                    h.task, r.name
                ));
            }
            return Ok(Some(name));
        }
    }
    let slot = crate::roles::lowest_free(r, &held).ok_or_else(|| {
        format!(
            "every {} slot is held ({} of {}): muster team shows by whom",
            r.name, r.slots, r.slots
        )
    })?;
    Ok(Some(crate::roles::slot_name(&roles, &slot)))
}

fn need<'a>(a: &'a Args, i: usize, what: &str) -> Result<&'a str, String> {
    a.pos
        .get(i)
        .map(String::as_str)
        .ok_or_else(|| format!("missing {what} (see `muster help`)"))
}

/// The queue at `root` as `muster` reads it, each task with its project's prefix (0033) for its
/// key.
fn keyed(root: &Path, prefix: &str) -> queue::State {
    queue::read_state_keyed(
        &[("q".into(), root.to_path_buf())],
        &mut queue::Cache::default(),
        &crate::settings::Settings::default(),
        &|_| prefix.to_string(),
    )
}

/// After a stage change: `mu-7: Building (doing/0007-x.md)`, with what it waits on if blocked.
/// The change is already written, so a failed re-read only shortens the message: it never
/// turns a done change into an error.
fn stage_message(root: &Path, prefix: &str, id: &str, path: &str) -> String {
    let k = crate::key::key(prefix, id);
    match ops::tasks_now(root).into_iter().find(|t| t.id == id) {
        Some(mut t) => {
            t.prefix = prefix.to_string();
            let why = crate::block::sentence_of(&t, false);
            let blocked = if why.is_empty() {
                why
            } else {
                format!(", {why}")
            };
            format!("{k}: {}{blocked} ({path})", queue::stage_label(&t.stage))
        }
        None => format!("{k}: changed ({path})"),
    }
}

/// What `muster list` shows: the old status, the app's search and filters, and a sort.
#[derive(Debug, Default)]
struct ListQuery {
    status: Option<String>,
    filter: search::Filter,
    sort: Option<search::Sort>,
}

/// A list option's values: `ready,building` → ["ready", "building"].
fn values(a: &Args, flag: &str) -> Vec<String> {
    a.flags
        .get(flag)
        .map(|v| {
            v.split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// `muster list`'s options, checked: stages and the sort must be ones Muster has.
fn list_query(a: &Args) -> Result<ListQuery, String> {
    let stages: Vec<String> = values(a, "stage")
        .into_iter()
        .map(|s| s.to_lowercase())
        .collect();
    if stages.iter().any(|s| !ops::STAGES.contains(&s.as_str())) {
        return Err(format!("stage must be one of {}", ops::STAGES.join(", ")));
    }
    let sort = match a.flags.get("sort") {
        None => None,
        Some(s) => Some(
            search::Sort::parse(&s.to_lowercase())
                .ok_or_else(|| format!("sort must be one of {}", search::Sort::NAMES.join(", ")))?,
        ),
    };
    Ok(ListQuery {
        status: a.flags.get("status").cloned(),
        filter: search::Filter {
            words: search::query_words(a.flags.get("search").map(String::as_str).unwrap_or("")),
            stages,
            // As a task's type is read (`task` is code), so `--type task` finds what it means.
            kinds: values(a, "type")
                .iter()
                .map(|k| ops::read_kind(k))
                .collect(),
            tags: values(a, "tag"),
            holders: values(a, "holder"),
            blocked: a.blocked,
        },
        sort,
    })
}

/// `muster list`'s tasks: those the status, search and filters keep, in id order or the sort's.
fn list_filter<'a>(tasks: &'a [queue::Task], q: &ListQuery) -> Vec<&'a queue::Task> {
    let mut out: Vec<_> = tasks
        .iter()
        .filter(|t| q.status.as_ref().is_none_or(|o| &t.status == o))
        .filter(|t| q.filter.matches(t))
        .collect();
    match q.sort {
        Some(by) => search::sort(&mut out, by),
        None => out.sort_by(|x, y| x.id.cmp(&y.id)),
    }
    out
}

/// A check-in in one line: `60% · gate green, writing tests (2026-10-12 23:40)`, its text cut at
/// `max` characters.
fn checkin_text(c: &queue::Checkin, max: usize) -> String {
    let mut text: String = c.text.chars().take(max).collect();
    if c.text.chars().count() > max {
        text = format!("{}…", text.trim_end());
    }
    match c.pct {
        Some(n) => format!("{n}% · {text} ({})", c.at),
        None => format!("{text} ({})", c.at),
    }
}

/// `muster show`'s lines after the file: its key (0033), who's on it in this stage, since when,
/// and the latest check-in. Empty when there's nothing to say.
fn show_footer(t: &queue::Task) -> String {
    let mut out = String::new();
    if !t.prefix.is_empty() {
        out.push_str(&format!("key: {}\n", crate::key::key(&t.prefix, &t.id)));
    }
    let holder = t.holder();
    if !holder.is_empty() {
        let since = if t.stage_since.is_empty() {
            String::new()
        } else {
            format!(", since {}", t.stage_since)
        };
        out.push_str(&format!(
            "on it: {holder} ({}{since})\n",
            queue::stage_label(&t.stage)
        ));
    }
    if let Some(c) = t.current_checkin() {
        out.push_str(&format!(
            "latest check-in, by {}: {}\n",
            c.who,
            checkin_text(c, usize::MAX)
        ));
    }
    if out.is_empty() {
        out
    } else {
        format!("\n---\n{out}")
    }
}

/// `muster list`'s backlog notes: where Backlog is listed (no stage, or one that includes
/// backlog) and no filter needs what only tasks have (status, type, tag, holder, blocked);
/// then those the search finds in their title and text.
fn list_notes<'a>(notes: &'a [backlog::Note], q: &ListQuery) -> Vec<&'a backlog::Note> {
    if q.status.is_some() || q.filter.hides_notes() {
        return Vec::new();
    }
    notes
        .iter()
        .filter(|n| search::note_matches(&n.title, &n.text, &q.filter.words))
        .collect()
}

/// A task as `--json` gives it: without its search text (`Task::search` is never serialized),
/// which is only for matching and would cost an agent reading the list a few KB a task; with its
/// key (0033) beside its id.
fn task_json(t: &queue::Task) -> serde_json::Value {
    with_key(serde_json::json!(t), crate::key::key(&t.prefix, &t.id))
}

/// A JSON object with the task's key (0033) beside its id.
fn with_key(mut v: serde_json::Value, key: String) -> serde_json::Value {
    v["key"] = key.into();
    v
}

/// `muster list --json`'s `sessionState` (0048): Claude Code's word on each holder's session, read
/// now from every allowed account once a stamp in Shaping, Building or Testing names one (0182),
/// matched by session id in any of them, under the caller's `$HOME`.
/// What each session holds is counted across this queue and `others` (every other queue Muster
/// knows), as the app counts across all of them. Unknown without a home or a stamped holder (then
/// nothing runs, and `others` isn't read). Only this read runs it, never a write.
fn session_states(tasks: &mut [queue::Task], env: Env, others: &dyn Fn() -> Vec<queue::Task>) {
    let Some(home) = env("HOME").filter(|h| !h.is_empty()) else {
        return;
    };
    let allowed = accounts::allowed(&machine_settings(env), Path::new(&home));
    let wanted = agents::wanted(tasks, &allowed);
    if wanted.is_empty() {
        return;
    }
    let snap = agents::read_all(&agents::claude_fetch(PathBuf::from(home)), &wanted);
    let others = others();
    let holds = agents::holds(tasks.iter().chain(&others));
    agents::apply_with(tasks, &holds, &snap, &allowed);
}

/// The tasks of every queue Muster knows other than `root` (the app's list: found and added).
fn other_queues(root: &Path) -> Vec<queue::Task> {
    let here = std::fs::canonicalize(root).ok();
    let others: Vec<(String, PathBuf)> = projects::active(&base(), &projects::load(&config_file()))
        .into_iter()
        .filter(|(_, p)| std::fs::canonicalize(p).ok() != here)
        .collect();
    queue::read_state(&others, &mut queue::Cache::default()).tasks
}

/// `muster list --json`: the tasks, and the backlog notes beside them.
fn list_json(notes: &[&backlog::Note], tasks: &[&queue::Task]) -> serde_json::Value {
    let tasks: Vec<_> = tasks.iter().map(|t| task_json(t)).collect();
    serde_json::json!({ "tasks": tasks, "backlog": notes })
}

/// A held note's part of its `muster list` line (0097): `  · shaping: Shapour, 12 min`, the
/// holder by label and how long since they took it at `stamp`; "" when nobody holds it.
fn shaping_text(n: &backlog::Note, stamp: &str) -> String {
    if n.held_by.is_empty() {
        return String::new();
    }
    let who = session::split(&n.held_by).0;
    match (ops::stamp_minutes(&n.held_since), ops::stamp_minutes(stamp)) {
        (Some(at), Some(now)) => format!(
            "  · shaping: {who}, {}",
            ops::quiet_text(u64::try_from(now - at).unwrap_or(0))
        ),
        _ => format!("  · shaping: {who}"),
    }
}

/// `muster list` as text: the backlog notes first (file name and title, and who is shaping it
/// for how long at `stamp`, 0097), then a heading per stage in STAGES order, then one line per
/// task, by its key (0033). In two parts, cut after Your check, so the over-limit line (0050), on
/// stderr, shows under it in a terminal.
fn list_parts(notes: &[&backlog::Note], tasks: &[&queue::Task], stamp: &str) -> (String, String) {
    let mut head = String::new();
    let mut tail = String::new();
    let mut out = &mut head;
    let key = |t: &queue::Task| crate::key::key(&t.prefix, &t.id);
    let width = tasks.iter().map(|t| key(t).len()).max().unwrap_or(0);
    if !notes.is_empty() {
        out.push_str(&format!("Backlog notes ({})\n", notes.len()));
        for n in notes {
            let title = if n.title.is_empty() {
                "(empty note)"
            } else {
                &n.title
            };
            out.push_str(&format!(
                "  {}  {title}{}\n",
                n.file,
                shaping_text(n, stamp)
            ));
        }
    }
    for st in ops::STAGES {
        let group: Vec<_> = tasks.iter().filter(|t| t.stage == st).collect();
        if group.is_empty() {
            continue;
        }
        if st == "done" {
            out = &mut tail;
        }
        out.push_str(&format!("{} ({})\n", ops::stage_label(st), group.len()));
        for t in group {
            let mut extra = String::new();
            // The owner's own task (0022): only they do it.
            if t.is_human() {
                extra.push_str("  [you]");
            }
            if !t.blocked_kind.is_empty() {
                extra.push_str(&format!(" [{}]", crate::block::sentence_of(t, true)));
            }
            if !t.paused.is_empty() {
                extra.push_str(&format!("  [paused since {}]", t.paused));
            }
            if t.waiting_for_owner {
                extra.push_str("  [needs you]");
            }
            let estimate = ops::estimate_text(t);
            if !estimate.is_empty() {
                extra.push_str(&format!("  ~{estimate}"));
            }
            if !t.tags.is_empty() {
                extra.push_str(&format!("  #{}", t.tags.join(" #")));
            }
            if t.open_questions > 0 {
                extra.push_str(&format!("  [{} open Q]", t.open_questions));
            }
            if !t.holder().is_empty() {
                extra.push_str(&format!("  [on it: {}]", t.holder()));
            }
            if let Some(c) = t.current_checkin() {
                extra.push_str(&format!("  [check-in {}]", checkin_text(c, 60)));
            }
            out.push_str(&format!(
                "  {:<width$} {:<9} {:<7} {}{extra}\n",
                key(t),
                t.kind,
                t.priority,
                t.title
            ));
        }
    }
    (head, tail)
}

/// `muster check --fix-pauses` (0059): every old paused block in `queues` listed (key, queue,
/// title, stage, since when, the reason it keeps), then, unless `dry_run`, each turned into a pause
/// by `who` (of its queue's folder), then one line saying what happened. The lines to print, and
/// the exit code: 1 when one couldn't be converted.
fn fix_pauses(
    queues: &[(String, PathBuf)],
    dry_run: bool,
    who: &dyn Fn(&Path) -> String,
    stamp: &str,
) -> (Vec<String>, i32) {
    let n = |k: usize, one: &str, many: &str| format!("{k} {}", if k == 1 { one } else { many });
    let found = old_pause::find_all(queues, stamp);
    if found.is_empty() {
        return (
            vec![format!(
                "no old paused blocks in {}",
                n(queues.len(), "queue", "queues")
            )],
            0,
        );
    }
    let mut lines: Vec<String> = found
        .iter()
        .map(|p| {
            let why = if p.reason.is_empty() {
                "no reason given".to_string()
            } else {
                format!("reason: {}", p.reason)
            };
            format!(
                "{} ({}) {} · {} · paused since {} · {why}",
                p.key,
                p.queue,
                p.title,
                queue::stage_label(&p.stage),
                p.since
            )
        })
        .collect();
    let blocks = |k: usize| n(k, "old paused block", "old paused blocks");
    let pauses = |k: usize| if k == 1 { "a pause" } else { "pauses" };
    if dry_run {
        lines.push(format!(
            "dry run: {} would become {}; nothing changed",
            blocks(found.len()),
            pauses(found.len())
        ));
        return (lines, 0);
    }
    // Only what was listed, each checked again under its queue's lock.
    let mut fixed = old_pause::Fixed::default();
    for (name, root) in queues {
        let ids: Vec<String> = found
            .iter()
            .filter(|p| p.queue == *name)
            .map(|p| p.id.clone())
            .collect();
        match old_pause::convert(name, root, &ids, &who(root), stamp) {
            Ok(f) => {
                fixed.converted.extend(f.converted);
                fixed.failed.extend(f.failed);
            }
            Err(e) => fixed.failed.push(format!("{name}: {e}")),
        }
    }
    lines.extend(fixed.failed.iter().map(|f| format!("not converted: {f}")));
    let k = fixed.converted.len();
    let mut done = format!("turned {} into {}", blocks(k), pauses(k));
    if !fixed.failed.is_empty() {
        done.push_str(&format!(
            "; {} couldn't be, and {} as it was",
            fixed.failed.len(),
            if fixed.failed.len() == 1 { "is" } else { "are" }
        ));
    }
    lines.push(done);
    (lines, if fixed.failed.is_empty() { 0 } else { 1 })
}

/// What `muster check` says about INDEX.md, in the problems' form (`q:` like `tasks_of`'s): a
/// list that shows something other than the files is a problem; one only due a refresh (the
/// files changed outside muster, as Drover's answers do) is a warning; none yet is fine.
fn index_check_line(status: index::Status) -> Option<String> {
    match status {
        index::Status::Stale => Some(format!("q: {}", index::STALE)),
        index::Status::Refresh => Some(format!("q: warning: {}", index::REFRESH)),
        index::Status::Current | index::Status::Missing => None,
    }
}

/// `muster history [commit]` (0109): the local queue's history now and its last commits, or
/// commit what it doesn't have yet.
fn run_history(a: &Args, root: &Path, me: &dyn Fn() -> String) -> Result<i32, String> {
    match a.pos.get(1).map(String::as_str) {
        None => {}
        Some("commit") => {
            let done = crate::history::commit_outside(root)?;
            if done.is_empty() {
                outln!("nothing to commit: the history has every file");
            } else {
                outln!("committed {}", done.join(", "));
            }
            return Ok(0);
        }
        Some("removed") => {
            let done = crate::history::removed(root, &a.pos[2..], a.all, &me())?;
            if done.is_empty() {
                outln!("nothing to record: no file is missing since the last commit");
            } else {
                outln!(
                    "recorded as removed: {} (older commits keep them: muster restore <path> brings one back)",
                    done.join(", ")
                );
            }
            return Ok(0);
        }
        Some(other) => {
            let help = "see `muster help history`";
            return Err(format!(
                "history takes nothing, commit or removed, not {other:?} ({help})"
            ));
        }
    }
    let s = crate::history::status(root);
    if a.json {
        print_json(&s)?;
        return Ok(0);
    }
    if !s.kept {
        outln!("this queue keeps no history of its own: only a local .muster queue (local = true) in a repo's main checkout has one");
        return Ok(0);
    }
    let problems = crate::history::problems("q", root);
    if !s.started && problems.is_empty() {
        outln!("history: not started yet: the queue's next write starts it");
    } else if problems.is_empty() {
        outln!("history: every file committed");
    }
    for p in &problems {
        outln!("{}", p.trim_start_matches("q: "));
    }
    for (at, what) in crate::history::log(root, 10)? {
        outln!("{at}  {what}");
    }
    Ok(0)
}

/// `--body`: the text, `-` for stdin (up to 1 MB), or None when not given. A one-line command can
/// carry line breaks as `\n` (the helper writes them so, because a permission rule only matches a
/// command on one line).
fn body_arg(a: &Args) -> Result<Option<String>, String> {
    Ok(match a.flags.get("body").map(String::as_str) {
        Some("-") => {
            let mut s = String::new();
            std::io::Read::read_to_string(
                &mut std::io::Read::take(std::io::stdin(), 1 << 20),
                &mut s,
            )
            .map_err(|e| e.to_string())?;
            Some(s)
        }
        Some(b) => Some(b.replace("\\n", "\n")),
        None => None,
    })
}

/// A new task's fields from the options `new` and `file` share.
fn new_task(a: &Args, title: &str, body: String) -> ops::NewTask {
    ops::NewTask {
        title: title.into(),
        kind: a.flags.get("type").cloned().unwrap_or_default(),
        priority: a.flags.get("priority").cloned().unwrap_or_default(),
        body,
        tags: a
            .flags
            .get("tags")
            .map(|t| vec![t.clone()])
            .unwrap_or_default(),
        est_time: a.flags.get("time").cloned().unwrap_or_default(),
        est_test_time: a.flags.get("test-time").cloned().unwrap_or_default(),
        est_tokens: a.flags.get("tokens").cloned().unwrap_or_default(),
        touches: a.flags.get("touches").cloned().unwrap_or_default(),
        ..Default::default()
    }
}

fn print_json<T: serde::Serialize>(v: &T) -> Result<(), String> {
    outln!(
        "{}",
        serde_json::to_string_pretty(v).map_err(|e| e.to_string())?
    );
    Ok(())
}

/// The most text `edit` takes from stdin or the editor's file.
const MAX_EDIT_BYTES: u64 = 1 << 20;

/// Text from `r`, at most 1 MB, as UTF-8.
fn read_capped(r: impl std::io::Read, what: &str) -> Result<String, String> {
    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut r.take(MAX_EDIT_BYTES + 1), &mut buf)
        .map_err(|e| format!("{what}: {e}"))?;
    if buf.len() as u64 > MAX_EDIT_BYTES {
        return Err(format!("{what} is over 1 MB"));
    }
    String::from_utf8(buf).map_err(|_| format!("{what} isn't UTF-8 text"))
}

/// Split a command line into words the way a shell would quote them, without a shell: whitespace
/// separates words; '…' is literal; in "…" a backslash escapes `"` and `\` only; elsewhere a
/// backslash escapes the next character. Nothing is expanded. None for an unclosed quote or a
/// trailing backslash.
pub(crate) fn split_words(line: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut word: Option<String> = None;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => words.extend(word.take()),
            '\'' => {
                let w = word.get_or_insert_with(String::new);
                loop {
                    match chars.next()? {
                        '\'' => break,
                        c => w.push(c),
                    }
                }
            }
            '"' => {
                let w = word.get_or_insert_with(String::new);
                loop {
                    match chars.next()? {
                        '"' => break,
                        '\\' => match chars.next()? {
                            c @ ('"' | '\\') => w.push(c),
                            c => {
                                w.push('\\');
                                w.push(c);
                            }
                        },
                        c => w.push(c),
                    }
                }
            }
            '\\' => word.get_or_insert_with(String::new).push(chars.next()?),
            c => word.get_or_insert_with(String::new).push(c),
        }
    }
    words.extend(word);
    Some(words)
}

/// The editor `edit` runs: `$VISUAL`, else `$EDITOR`, else `vi`, as the program and its arguments
/// (`split_words`: quotes and backslashes as a shell reads them, but no shell runs it). A blank
/// variable counts as unset; one with an unclosed quote is refused.
fn editor_command(visual: Option<String>, editor: Option<String>) -> Result<Vec<String>, String> {
    for (name, value) in [("$VISUAL", visual), ("$EDITOR", editor)] {
        let Some(value) = value else { continue };
        let words = split_words(&value).ok_or_else(|| {
            format!("{name} has an unclosed quote or a trailing backslash: {value:?}")
        })?;
        if !words.is_empty() {
            return Ok(words);
        }
    }
    Ok(vec!["vi".into()])
}

/// Open the editor on a private temp copy of `text`; returns what it holds once the editor exits
/// cleanly, and the copy's path (the caller removes it, or keeps it when the save is refused).
/// When the editor fails, the copy is kept and named if it was typed into, else removed.
fn edit_in_editor(id: &str, text: &str) -> Result<(String, PathBuf), String> {
    use std::io::Write;
    let cmd = editor_command(std::env::var("VISUAL").ok(), std::env::var("EDITOR").ok())?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let file = std::env::temp_dir().join(format!(
        "muster-edit-{id}-{}-{nanos}.md",
        std::process::id()
    ));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    let mut f = opts
        .open(&file)
        .map_err(|e| format!("can't make a temp file to edit in: {e}"))?;
    let first = format!("{text}\n");
    let written = f.write_all(first.as_bytes()).and_then(|()| f.sync_all());
    drop(f);
    let gone = |e: String| {
        let _ = std::fs::remove_file(&file);
        e
    };
    written.map_err(|e| gone(format!("can't write the temp file to edit in: {e}")))?;
    let status = std::process::Command::new(&cmd[0])
        .args(&cmd[1..])
        .arg(&file)
        .status()
        .map_err(|e| gone(format!("couldn't run the editor {:?}: {e}", cmd[0])))?;
    let new = std::fs::File::open(&file)
        .map_err(|e| e.to_string())
        .and_then(|f| read_capped(f, "the edited text"));
    if !status.success() {
        let failed = format!("the editor exited with {status}; nothing was saved");
        // Something was typed before it failed: keep it rather than throw it away.
        return Err(match &new {
            Ok(t) if *t != first => format!("{failed}\nyour text is kept in {}", file.display()),
            _ => gone(failed),
        });
    }
    Ok((new.map_err(gone)?, file))
}

/// One line of `muster projects`: the name, its prefix (0033), where it came from (found under
/// ~/playground, or added in the app), the folder, whether it's hidden or no longer a queue, its
/// gate and run command (0077), and the label and category the app shows it with (0044), when set.
fn project_line(p: &projects::Project) -> String {
    let state = if p.hidden {
        " (hidden)"
    } else if !p.ok {
        " (not a queue)"
    } else {
        ""
    };
    let shown = |what: &str, v: &str| {
        if v.is_empty() {
            String::new()
        } else {
            format!("  {what}: {v}")
        }
    };
    format!(
        "{:<16} {:<4} {:<6} {}{state}{}{}{}{}",
        p.name,
        p.prefix,
        p.source,
        p.path,
        shown("gate", &p.gate),
        shown("run", &p.run),
        shown("label", &p.label),
        shown("category", &p.category)
    )
}

/// `muster projects set <name> gate|run "<command>"` (0023, 0077), on `config`: the command must
/// split into words the way it will be run (no shell), one argument (quoted), empty to clear it.
/// Returns what to print.
fn set_project_command(
    a: &Args,
    config: &mut projects::Config,
    base: &Path,
) -> Result<String, String> {
    let name = need(a, 2, "a project name")?;
    let field = need(a, 3, "the field to set (gate, run or prefix)")?;
    let set = match field {
        "gate" => projects::set_gate,
        "run" => projects::set_run,
        _ => {
            return Err(format!(
                "projects set takes gate, run or prefix, not {field:?} (see `muster help`)"
            ))
        }
    };
    let cmd = need(
        a,
        4,
        &format!("the {field} command, in quotes (\"\" clears it)"),
    )?;
    if a.pos.len() > 5 {
        return Err(format!(
            "put the {field} command in quotes: muster projects set <name> {field} \"<command>\" (see `muster help`)"
        ));
    }
    if !cmd.trim().is_empty() {
        split_words(cmd).ok_or_else(|| {
            format!("the {field} command has an unclosed quote or a trailing backslash: {cmd:?}")
        })?;
    }
    set(config, base, name, cmd)?;
    Ok(if cmd.trim().is_empty() {
        format!("{name}: {field} cleared")
    } else {
        format!("{name}: {field} set to {}", cmd.trim())
    })
}

/// `muster projects set <name> prefix <p>` (0033): stored with the queue, refused when it isn't 2
/// to 4 lowercase letters or another project has it. Returns what to print.
fn set_project_prefix(a: &Args, config: &projects::Config, base: &Path) -> Result<String, String> {
    let name = need(a, 2, "a project name")?;
    let prefix = need(a, 4, "the prefix (2 to 4 lowercase letters)")?;
    if a.pos.len() > 5 {
        return Err("projects set <name> prefix takes one prefix (see `muster help`)".into());
    }
    projects::set_prefix(config, base, name, prefix)?;
    Ok(format!(
        "{name}: prefix set to {prefix} (its tasks are {prefix}-1, {prefix}-2…)"
    ))
}

/// The gate command as words, from the project's `gate` (None when it has none).
fn gate_words(project: &str, gate: &str) -> Result<Vec<String>, String> {
    if gate.trim().is_empty() {
        return Err(format!(
            "no gate set for {project}, and a merge never runs without one: muster projects set {project} gate \"<command>\" (a script in the repo, like bin/gate, works)"
        ));
    }
    split_words(gate)
        .filter(|w| !w.is_empty())
        .ok_or_else(|| format!("{project}'s gate {gate:?} has an unclosed quote: set it again"))
}

/// `muster pass <id> --from testing` for a started task (0023): the merge, with `gate` (the
/// project's name and gate command) run in its worktree. Prints each step; 1 when it went back to
/// Building.
fn merge_cmd(
    a: &Args,
    root: &Path,
    prefix: &str,
    id: &str,
    who: &str,
    gate: Option<(String, String)>,
    cwd: Option<&Path>,
) -> Result<i32, String> {
    let k = crate::key::key(prefix, id);
    if a.flags.contains_key("gate") || a.flags.contains_key("commit") {
        return Err(format!(
            "{k} merges when it passes from Testing: the gate runs itself, and the merge commit is recorded; leave --gate and --commit out (see `muster help`)"
        ));
    }
    let (project, gate) = gate.unwrap_or_else(|| (projects::default_name(root), String::new()));
    let argv = gate_words(&project, &gate)?;
    outln!("{k}: merging into main (gate: {})", argv.join(" "));
    let mut say = |line: String| outln!("{line}");
    match crate::parallel::merge(root, id, who, &local_stamp, &argv, cwd, &mut say)? {
        crate::parallel::Merge::Returned(why) => {
            outln!("{k}: back in Building: {why}");
            Ok(1)
        }
        crate::parallel::Merge::Merged {
            commit: _,
            report,
            path,
            notes,
            next,
        } => {
            for n in &notes {
                outln!("left behind: {n} (muster check lists it)");
            }
            outln!("{}", stage_message(root, prefix, id, &path));
            outln!("{k}: {report}");
            match next {
                Some((n, title)) => {
                    let n = crate::key::key(prefix, &n);
                    outln!("next: {n} {title}: muster start {n}")
                }
                None => {
                    outln!("next: nothing else ready that's safe beside the tasks still running")
                }
            }
            Ok(0)
        }
    }
}

/// What `muster start` prints: where, the stage, and the brief (the task file, what it touches,
/// its criteria), then what to do.
fn started_text(root: &Path, prefix: &str, id: &str, s: &crate::parallel::Started) -> String {
    let k = crate::key::key(prefix, id);
    let mut out = format!(
        "{} {k} in {} ({})\n{}\nworktree: {}\nbranch:   {}\nbrief:    {}\n",
        if s.resumed { "picked up" } else { "started" },
        projects::default_name(root),
        root.display(),
        stage_message(root, prefix, id, &s.path),
        s.worktree.display(),
        s.branch,
        root.join(&s.path).display()
    );
    let touches = queue::read_task(root, id)
        .map(|t| t.touches)
        .unwrap_or_default();
    if !touches.is_empty() {
        out.push_str(&format!("touches:  {}\n", touches.join(", ")));
    }
    if let Ok(text) = queue::read_body(root, id) {
        let body = queue::front_matter(&text).map(|(_, b)| b).unwrap_or("");
        let items = queue::criteria_items(body);
        if !items.is_empty() {
            out.push_str("criteria:\n");
            for c in items {
                let proof = if c.proof.is_empty() {
                    String::new()
                } else {
                    format!(" — proof: {}", c.proof)
                };
                out.push_str(&format!(
                    "  - [{}] {}{proof}\n",
                    if c.ticked { "x" } else { " " },
                    c.text
                ));
            }
        }
    }
    // What reviews of earlier tasks taught, for where this one works (0027).
    let book = lessons::load(root).unwrap_or_default();
    let learned = lessons::for_builders(&book, &review::all(root), &touches, &local_stamp());
    // Sent back from Your check (mu-101): the owner's findings come first, and this branch's
    // review round settles them.
    let owners = review::owner_to_recheck(root, id);
    if !owners.is_empty() {
        out.push_str(&format!(
            "owner findings first: {} (sent back from Your check; see the task's Review section): fix each here and answer it with muster review {k} --fixed R<n> \"<how>\" or --dispute R<n> \"<why>\"; this branch's review round re-checks them\n",
            owners.join(", ")
        ));
    }
    if !learned.is_empty() {
        out.push_str("lessons (the owner accepted these from earlier reviews; follow them):\n");
        for l in &learned {
            out.push_str(&format!(
                "  - {}\n",
                backlog::one_line_name(&lessons::line(l))
            ));
        }
    }
    out.push_str(&format!(
        "Work in the worktree. Check in every 30 minutes (muster checkin {k} \"…\" --pct <n>); when the gate is green: muster pass {k} --from building --gate \"<the gate's line>\" --summary \"<what was built, one plain line in the owner's words>\". In Testing the engine reviews the branch (muster review {k}); answer what it finds with muster review {k} --fixed or --dispute.\n"
    ));
    out
}

/// `start` and `init` in a repo (0067): the work log's merge driver in the repo's own git config,
/// for the keys git doesn't have yet, said when it writes any. A failure (a sandboxed session
/// can't write `.git/config`) stops nothing: it's said last on stdout, where a session reads,
/// with the two lines that set it (0094); `muster check` keeps saying so until they're run.
fn set_merge_driver(dir: &Path) {
    match crate::merge_log::configure(dir) {
        Ok(keys) if keys.is_empty() => {}
        Ok(keys) => outln!(
            "set {} in the repo's git config, so work log entries merge whole (muster merge-log)",
            keys.join(" and ")
        ),
        Err(e) => out!("\n{}", crate::merge_log::not_set(&e)),
    }
}

/// `muster check`'s warning (0094) when the queue's repo has git merge with the work log's driver
/// (`.gitattributes`) and git's config doesn't have it yet, with the two lines that set it.
fn merge_driver_warning(root: &Path) -> Option<String> {
    let repo = root
        .parent()
        .filter(|_| root.file_name().is_some_and(|n| n == crate::layout::DIR))?;
    if !crate::merge_log::wanted(repo) || crate::merge_log::missing(repo).ok()?.is_empty() {
        return None;
    }
    Some(format!(
        "the work log's merge driver isn't in the repo's git config, though .gitattributes uses merge=muster-log.\n{}",
        crate::merge_log::fix_lines()
    ))
}

/// One line per task for `muster ready`: key, priority, title, and what it touches.
fn ready_line(t: &queue::Task) -> String {
    let touches = if t.touches.is_empty() {
        "no touches".to_string()
    } else {
        format!("touches: {}", t.touches.join(", "))
    };
    format!(
        "  {} {:<7} {}  [{touches}]\n",
        crate::key::key(&t.prefix, &t.id),
        t.priority,
        t.title
    )
}

/// `muster ready [--parallel [N]] [--json]` (0023): the tasks a session could start now, or with
/// --parallel up to N (default 3) safe together, and the ones left out with why.
fn ready_cmd(a: &Args, root: &Path, prefix: &str) -> Result<i32, String> {
    let s = keyed(root, prefix);
    let n = match (a.parallel, a.pos.get(1)) {
        (_, None) => 3,
        (true, Some(v)) => v
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=20).contains(n))
            .ok_or_else(|| format!("--parallel takes how many, 1 to 20, got {v:?}"))?,
        (false, Some(_)) => {
            return Err(
                "ready takes no id; how many goes after --parallel (see `muster help`)".into(),
            )
        }
    };
    if a.pos.len() > 2 {
        return Err("ready takes --parallel and how many, nothing more (see `muster help`)".into());
    }
    let (set, left) = if a.parallel {
        // Planning parallel sessions (0023): say first when the owner's review is the bottleneck.
        if let Some(l) = your_check_line(root, &s.tasks, &projects::load(&config_file())) {
            eprintln!("{l}");
        }
        let p = crate::parallel::ready_parallel(&s.tasks, n);
        (p.set, p.left)
    } else {
        (crate::parallel::ready(&s.tasks), Vec::new())
    };
    if a.json {
        let tasks: Vec<_> = set.iter().map(|t| task_json(t)).collect();
        let left: Vec<_> = left
            .iter()
            .map(|(t, why)| {
                serde_json::json!({ "id": t.id, "key": crate::key::key(prefix, &t.id), "why": why })
            })
            .collect();
        print_json(&serde_json::json!({ "tasks": tasks, "left": left }))?;
        return Ok(0);
    }
    if set.is_empty() {
        outln!(
            "nothing ready to start in {} ({})",
            projects::default_name(root),
            root.display()
        );
        return Ok(0);
    }
    let mut out = if a.parallel {
        format!("Can start together ({} of up to {n}):\n", set.len())
    } else {
        format!("Ready to start ({}):\n", set.len())
    };
    for t in &set {
        out.push_str(&ready_line(t));
    }
    if !left.is_empty() {
        out.push_str("Not with them:\n");
        for (t, why) in &left {
            out.push_str(&format!(
                "  {} {}: {why}\n",
                crate::key::key(prefix, &t.id),
                t.title
            ));
        }
    }
    out!("{out}");
    Ok(0)
}

/// Runs a command; returns the process exit code (0 ok, 1 refused or failed, 2 usage).
pub fn main(argv: &[String]) -> i32 {
    let code = main_with(argv, &|k: &str| std::env::var(k).ok());
    STDOUT.flush(&mut std::io::stdout().lock());
    if let Some(line) = STDOUT.failure_line(code) {
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), "{line}");
    }
    out_status(code, STDOUT.failed())
}

/// The status a command exits with (0170): its own, except that output it couldn't write, for
/// any reason but a reader that left, turns a success into 1.
fn out_status(code: i32, failed: bool) -> i32 {
    if code == 0 && failed {
        1
    } else {
        code
    }
}

/// `main`, with the caller's environment given.
fn main_with(argv: &[String], env: Env) -> i32 {
    match run_with(argv, env) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("muster: {e}");
            if e.contains("see `muster help`") || e.starts_with("unknown") {
                2
            } else {
                1
            }
        }
    }
}

fn run_with(argv: &[String], env: Env) -> Result<i32, String> {
    // Git's merge driver (0067): three file paths, as git gives them, never options.
    if argv.first().map(String::as_str) == Some("merge-log") {
        return crate::merge_log::run(&argv[1..]);
    }
    let a = parse(argv)?;
    if !reads_only(&a) {
        STDOUT.writes(a.pos.first().map_or("help", String::as_str));
    }
    // A teammate's agent id (0090): `--agent`, else `$MUSTER_AGENT`, checked once here and read
    // from then on as `MUSTER_AGENT`, so the stamp and the mailbox see the same one.
    let agent = agent_arg(&a, env)?;
    let with_agent = |k: &str| {
        if k == "MUSTER_AGENT" {
            agent.clone()
        } else {
            env(k)
        }
    };
    let env: Env = &with_agent;
    let cmd = a.pos.first().map(String::as_str).unwrap_or("help");
    let stamp = local_stamp();
    match cmd {
        "help" | "-h" | "--help" => {
            if a.json {
                print_json(&crate::help::help())?;
                return Ok(0);
            }
            match a.pos.get(1) {
                Some(name) => match crate::help::command_text(name) {
                    Some(t) => out!("{t}"),
                    None => return Err(format!("unknown command {name:?} (see `muster help`)")),
                },
                None => out!("{}", crate::help::text()),
            }
            Ok(0)
        }
        "team" => {
            let roles = crate::roles::load(&roles_file())?;
            let cwd = std::env::current_dir().ok();
            let known = || projects::active(&base(), &projects::load(&config_file()));
            let flag = a.flags.get("queue").map(String::as_str);
            let here = resolve_queue(flag, env_root(), cwd.as_deref(), &known)
                .ok()
                .map(|(root, _)| root);
            let team = crate::roles::team(&roles, &every_task(here.as_deref()));
            if a.json {
                print_json(&team)?;
            } else {
                out!("{}", crate::roles::team_text(&team));
            }
            Ok(0)
        }
        "instructions" => {
            let roles = crate::roles::load(&roles_file())?;
            let who = need(&a, 1, "a role")?;
            let r = crate::roles::find(&roles, who).ok_or_else(|| {
                format!(
                    "unknown role {who:?}: one of {} (see `muster help`)",
                    roles
                        .iter()
                        .map(|r| r.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?;
            out!("{}", crate::roles::instructions(r));
            Ok(0)
        }
        "projects" => {
            if a.pos.get(1).map(String::as_str) == Some("set") {
                let file = config_file();
                // A prefix lives with the queue (0033), not in projects.json.
                if a.pos.get(3).map(String::as_str) == Some("prefix") {
                    outln!(
                        "{}",
                        set_project_prefix(&a, &projects::load(&file), &base())?
                    );
                    return Ok(0);
                }
                let said = projects::edit(&file, |c| set_project_command(&a, c, &base()))?;
                outln!("{said}");
                return Ok(0);
            }
            let list = projects::list(&base(), &projects::load(&config_file()));
            if a.json {
                print_json(&list)?;
            } else {
                for p in &list {
                    outln!("{}", project_line(p));
                }
            }
            Ok(0)
        }
        "init" => {
            // The folder it runs in when none is named (0004): `muster init` in a repo. Outside
            // one, the old tasks/ only for a folder named on purpose: nothing is written.
            let dir = match a.pos.get(1) {
                Some(d) => PathBuf::from(d),
                None => {
                    let cwd =
                        std::env::current_dir().map_err(|e| format!("the current folder: {e}"))?;
                    if !projects::in_a_repo(&cwd) {
                        return Err(format!(
                            "{} isn't in a git repo: run muster init in the repo, or name a folder for an old tasks/ queue",
                            cwd.display()
                        ));
                    }
                    cwd
                }
            };
            local_or_committed(&a)?;
            let committed = (a.local || a.committed).then_some(a.committed);
            let wanted = a.flags.get("prefix").map(String::as_str);
            let known = crate::key::known();
            // In a repo, its `.muster/` (0005); elsewhere, the old `<folder>/tasks`.
            let (q, prefix) = projects::new_queue(&dir, committed, wanted, &known, &tools_dir())?;
            outln!(
                "created {} (prefix {prefix}: its tasks are {prefix}-1, {prefix}-2…)",
                q.display()
            );
            if q.file_name().is_some_and(|n| n == crate::layout::DIR) {
                set_merge_driver(&dir);
            }
            Ok(0)
        }
        // Every queue Muster knows (0059), or the one --queue names.
        "check" if a.fix_pauses => {
            let cwd = std::env::current_dir().ok();
            let known = || projects::active(&base(), &projects::load(&config_file()));
            let queues = match a.flags.get("queue") {
                Some(q) => {
                    let (root, _) = resolve_queue(Some(q), None, cwd.as_deref(), &known)?;
                    let name = known()
                        .into_iter()
                        .find(|(_, p)| same_folder(p, &root))
                        .map(|(n, _)| n)
                        .unwrap_or_else(|| projects::default_name(&root));
                    vec![(name, root)]
                }
                None => known(),
            };
            let who =
                |root: &Path| stamped_who(&label(&a, env, root), env, cwd.as_deref(), Some(root));
            let (lines, code) = fix_pauses(&queues, a.dry_run, &who, &stamp);
            for l in &lines {
                outln!("{l}");
            }
            Ok(code)
        }
        "migrate" => {
            let old = need(&a, 1, "the old queue's folder")?;
            let repo = need(&a, 2, "the repo to put .muster/ in")?;
            let opts = crate::migrate::Options {
                local: local_or_committed(&a)?,
                dry_run: a.dry_run,
            };
            let r = crate::migrate::migrate(
                Path::new(old),
                Path::new(repo),
                &opts,
                &tools_dir(),
                &stamp,
                &crate::key::known(),
            )?;
            for line in &r.lines {
                outln!("{line}");
            }
            // What projects.json says of the old queue (gate, run, name, hidden, Your check limit), said
            // of the new.
            let carried =
                carry_projects(&config_file(), &base(), Path::new(old), &r.queue, a.dry_run)?;
            for line in &carried {
                outln!("{}{line}", if a.dry_run { "would: " } else { "" });
            }
            Ok(0)
        }
        _ => {
            let cwd = std::env::current_dir().ok();
            let known = || projects::active(&base(), &projects::load(&config_file()));
            let flag = a.flags.get("queue").map(String::as_str);
            // The task as typed (0033): a key with a prefix names its project's queue.
            let typed = task_arg(cmd, &a)
                .map(|i| crate::key::typed(&a.pos[i]).map(|t| (i, t)))
                .transpose()?;
            let (root, source) = match &typed {
                Some((
                    i,
                    crate::key::Typed {
                        prefix: Some(p), ..
                    },
                )) => key_queue(&a.pos[*i], p, flag, env_root(), cwd.as_deref(), &known)?,
                _ => resolve_queue(flag, env_root(), cwd.as_deref(), &known)?,
            };
            if targeted(cmd, &a) {
                check_target(&source, &root, cwd.as_deref(), &known)?;
            }
            let prefix = crate::key::prefix_for(&root);
            let cmd = cmd.to_string();
            let mut a = a;
            if let Some((i, t)) = typed {
                a.pos[i] = t.id;
            }
            task_lists(&mut a, &cmd, &prefix, &projects::default_name(&root))?;
            // What the core refuses with names tasks by their keys too (0033).
            let typed_id = task_arg(&cmd, &a).map(|i| a.pos[i].clone());
            run_on(&a, &cmd, &root, &prefix, &stamp, env, cwd.as_deref()).map_err(|e| {
                crate::key::name_tasks(
                    &root,
                    &e,
                    &typed_id.iter().map(String::as_str).collect::<Vec<_>>(),
                )
            })
        }
    }
}

fn run_on(
    a: &Args,
    cmd: &str,
    root: &Path,
    prefix: &str,
    stamp: &str,
    env: Env,
    cwd: Option<&Path>,
) -> Result<i32, String> {
    // --role (0112) is the slot's name as --as from here on.
    if let Some(slot) = role_label(a, cmd, root, env, cwd)? {
        let mut a = a.clone();
        a.flags.remove("role");
        a.flags.insert("as".into(), slot);
        return run_on(&a, cmd, root, prefix, stamp, env, cwd);
    }
    // A task as it's shown (0033): `mu-12`.
    let key = |id: &str| crate::key::key(prefix, id);
    // Who's acting, worked out only by the commands that write: the label, and the label with the
    // session (the questions' commands record the session on their own).
    let label = || label(a, env, root);
    let me = || stamped_who(&label(), env, cwd, Some(root));
    match cmd {
        "new" => {
            let title = need(a, 1, "a title")?;
            // Level, parent, due (0016) and --human (0022) are new's own; `file` takes the rest of
            // these options.
            let new = ops::NewTask {
                level: a.flags.get("level").cloned().unwrap_or_default(),
                parent: a.flags.get("parent").cloned().unwrap_or_default(),
                due: a.flags.get("due").cloned().unwrap_or_default(),
                for_who: if a.human { "human" } else { "" }.into(),
                ..new_task(a, title, body_arg(a)?.unwrap_or_default())
            };
            let c = ops::create_task(root, &new, &me(), stamp)?;
            if a.json {
                print_json(&with_key(serde_json::json!(c), key(&c.id)))?;
            } else {
                outln!("{} {}", key(&c.id), c.path);
            }
            Ok(0)
        }
        "list" => {
            let q = list_query(a)?;
            let mut s = keyed(root, prefix);
            if a.json {
                session_states(&mut s.tasks, env, &|| other_queues(root));
            }
            let tasks = list_filter(&s.tasks, &q);
            let notes = list_notes(&s.backlog, &q);
            let over = your_check_line(root, &s.tasks, &projects::load(&config_file()));
            if a.json {
                print_json(&list_json(&notes, &tasks))?;
                if let Some(l) = over {
                    eprintln!("{l}");
                }
            } else {
                let (head, tail) = list_parts(&notes, &tasks, stamp);
                out!("{head}");
                if let Some(l) = over {
                    eprintln!("{l}");
                }
                out!("{tail}");
            }
            Ok(0)
        }
        "show" => {
            let id = need(a, 1, "a task id")?;
            if a.json {
                let s = keyed(root, prefix);
                let t = s
                    .tasks
                    .iter()
                    .find(|t| t.id == id)
                    .ok_or(format!("no task {id}"))?;
                print_json(&task_json(t))?;
            } else {
                out!("{}", queue::read_body(root, id)?);
                // Who's on it and the latest check-in, which the file only has in pieces.
                if let Ok(mut t) = queue::read_task(root, id) {
                    t.prefix = prefix.to_string();
                    out!("{}", show_footer(&t));
                }
            }
            Ok(0)
        }
        "move" => {
            let id = need(a, 1, "a task id")?;
            let status = need(a, 2, "a status")?;
            let p = ops::move_task(
                root,
                id,
                status,
                a.flags.get("on").map(String::as_str).unwrap_or(""),
                &me(),
                stamp,
            )?;
            outln!("{} → {p}", key(id));
            Ok(0)
        }
        "pass" => {
            let id = need(a, 1, "a task id")?;
            // A started task (0023) passing from Testing is the merge.
            let from = a.flags.get("from").map(|f| f.trim().to_lowercase());
            let started = queue::read_task(root, id)
                .is_ok_and(|t| t.stage == "testing" && !t.branch.is_empty());
            if started && from.as_deref().is_none_or(|f| f == "testing") {
                let gate = projects::gate_of(&base(), &projects::load(&config_file()), root);
                return merge_cmd(a, root, prefix, id, &me(), gate, cwd);
            }
            let opts = ops::PassOpts {
                gate: a.flags.get("gate").cloned().unwrap_or_default(),
                commit: a.flags.get("commit").cloned().unwrap_or_default(),
                summary: a.flags.get("summary").cloned().unwrap_or_default(),
                from: a.flags.get("from").cloned(),
                now_unchanged: a.flags.get("now-unchanged").cloned().unwrap_or_default(),
            };
            let p = ops::pass_task(root, id, &me(), stamp, &opts)?;
            outln!("{}", stage_message(root, prefix, id, &p));
            // Into Testing on its own branch: the review of record comes first (0027).
            if queue::read_task(root, id)
                .is_ok_and(|t| t.stage == "testing" && !t.branch.is_empty())
            {
                let k = key(id);
                outln!("next: the review of record, by a session that didn't build it: muster review {k} prints its brief");
            }
            Ok(0)
        }
        "return" => {
            let id = need(a, 1, "a task id")?;
            need(a, 2, "the reason")?;
            let why = a.pos[2..].join(" ").replace("\\n", " ");
            let to = a.flags.get("to").map(String::as_str);
            let r = ops::return_back(root, id, &why, to, &me(), stamp)?;
            outln!("{}", stage_message(root, prefix, id, &r.path));
            for l in returned_lines(&key(id), &r) {
                outln!("{l}");
            }
            Ok(0)
        }
        "block" => {
            let id = need(a, 1, "a task id")?;
            let on = a.flags.get("on").ok_or(
                "block needs --on <0014,0016|you|permission|question|outside> and what it waits on (see `muster help`)",
            )?;
            // The kind, and what it waits on: the rest of --on, then the words after the id.
            // After bare task ids, a colon sets those words apart, so a reason that starts with
            // a digit (`--on 0014 3pm call`) isn't read as one more id.
            let (kind, mut detail) = crate::block::split_on(on);
            let more = a.pos.get(2..).unwrap_or_default().join(" ");
            if !more.trim().is_empty() {
                if kind == "tasks" && !detail.contains(':') {
                    detail.push(':');
                }
                if !detail.is_empty() {
                    detail.push(' ');
                }
                detail.push_str(&more);
            }
            let p = ops::block_as(root, id, &kind, &detail, &me(), stamp)?;
            outln!("{}", stage_message(root, prefix, id, &p));
            Ok(0)
        }
        "unblock" => {
            let id = need(a, 1, "a task id")?;
            let p = ops::unblock_task(root, id, &me(), stamp)?;
            outln!("{}", stage_message(root, prefix, id, &p));
            Ok(0)
        }
        "pause" | "resume" => {
            let id = need(a, 1, "a task id")?;
            let p = if cmd == "pause" {
                pause::pause(root, id, &a.pos[2..].join(" "), &me(), stamp)?
            } else {
                pause::resume(root, id, &me(), stamp)?
            };
            outln!("{}: {cmd}d ({})", key(id), p.path);
            if !p.session.is_empty() {
                let ask = if cmd == "pause" {
                    "to stop"
                } else {
                    "to carry on"
                };
                outln!(
                    "Asked session {} {ask}: it arrives after that session's next tool call.",
                    p.session
                );
            } else if !p.unreached.is_empty() {
                outln!("Its session can't be told: {}.", p.unreached);
            }
            Ok(0)
        }
        "stage" => {
            let id = need(a, 1, "a task id")?;
            let stage = need(a, 2, "a stage")?;
            let from = a.flags.get("from").map(String::as_str);
            let p = ops::set_stage(root, id, stage, from, &me(), stamp)?;
            outln!("{}", stage_message(root, prefix, id, &p));
            Ok(0)
        }
        "set" => {
            let id = need(a, 1, "a task id")?;
            let field = need(a, 2, "a field")?;
            let value = a.pos[3..].join(" ");
            ops::set_field(root, id, field, &value, &me(), stamp)?;
            outln!("{}: {field} set", key(id));
            Ok(0)
        }
        "note" => {
            let id = need(a, 1, "a task id")?;
            ops::add_note(
                root,
                id,
                &a.pos[2..].join(" ").replace("\\n", " "),
                &me(),
                stamp,
            )?;
            outln!("{}: noted", key(id));
            Ok(0)
        }
        "tick" | "untick" => {
            let id = need(a, 1, "a task id")?;
            let which = need(a, 2, "which criterion (AC2 or 2)")?;
            let undo = cmd == "untick";
            ops::tick_criterion(root, id, which, undo, &me(), stamp)?;
            outln!(
                "{}: {} {which}",
                key(id),
                if undo { "unticked" } else { "ticked" }
            );
            Ok(0)
        }
        "edit" => {
            let id = need(a, 1, "a task id")?;
            let from_stdin = match a.flags.get("body").map(String::as_str) {
                None => false,
                Some("-") => true,
                Some(_) => return Err("edit takes --body - (the new text on stdin), or no --body to open $VISUAL or $EDITOR (see `muster help`)".into()),
            };
            // Refused here (someone else holds it) before anything is typed.
            let me = me();
            let opened = ops::open_body(root, id, &me)?;
            let (new, copy) = if from_stdin {
                (read_capped(std::io::stdin(), "stdin")?, None)
            } else {
                let (text, file) = edit_in_editor(id, &opened)?;
                (text, Some(file))
            };
            // Stamped when it's saved, not when the command started: an editor can stay open
            // for hours.
            let saved = ops::edit_body(root, id, &new, &opened, &me, &local_stamp());
            match (&saved, &copy) {
                (Err(e), Some(file)) => {
                    return Err(format!("{e}\nyour text is kept in {}", file.display()))
                }
                (_, Some(file)) => {
                    // Saved or unchanged, the copy has done its job; one left behind is only a
                    // private file in the temp folder, so it doesn't fail the edit.
                    let _ = std::fs::remove_file(file);
                }
                _ => {}
            }
            match saved? {
                Some(p) => outln!("{}: edited ({p})", key(id)),
                None => outln!("{}: no change", key(id)),
            }
            Ok(0)
        }
        "link" => {
            let id = need(a, 1, "a task id")?;
            match a.flags.get("remove") {
                Some(which) => {
                    if a.pos.len() > 2 {
                        return Err("link --remove takes the link's number or address, nothing more (see `muster help`)".into());
                    }
                    let gone = fields::remove_link(root, id, which, &me(), stamp)?;
                    outln!("{}: unlinked {}", key(id), gone.url);
                }
                None => {
                    let url = need(a, 2, "an https link")?;
                    let l = fields::add_link(root, id, url, &a.pos[3..].join(" "), &me(), stamp)?;
                    outln!("{}: linked {}", key(id), l.url);
                }
            }
            Ok(0)
        }
        "comment" => {
            let id = need(a, 1, "a task id")?;
            let text = match body_arg(a)? {
                Some(_) if a.pos.len() > 2 => {
                    return Err(
                        "comment takes its text or --body, not both (see `muster help`)".into(),
                    )
                }
                Some(body) => body,
                None => {
                    need(a, 2, "the comment")?;
                    a.pos[2..].join(" ")
                }
            };
            let c = fields::comment(root, id, &text, &me(), stamp)?;
            if c.did.is_empty() {
                outln!("{}: commented", key(id));
            } else if c.unsaved.is_empty() {
                outln!("{}: {}; commented", key(id), c.did);
            } else {
                // The action ran: sending the comment again would run it twice.
                outln!("{}: {}", key(id), c.did);
                eprintln!(
                    "{}: the comment couldn't be saved ({}); the action stands, so don't send it again",
                    key(id),
                    c.unsaved
                );
                return Ok(1);
            }
            Ok(0)
        }
        "attach" => {
            let id = need(a, 1, "a task id")?;
            need(a, 2, "a file")?;
            let me = me();
            for f in &a.pos[2..] {
                let p = Path::new(f);
                let meta = std::fs::metadata(p).map_err(|e| format!("{f}: {e}"))?;
                if !meta.is_file() {
                    return Err(format!("{f} isn't a file"));
                }
                let bytes = std::fs::read(p).map_err(|e| format!("{f}: {e}"))?;
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let saved = ops::attach(root, id, &name, &bytes, &me, stamp)?;
                outln!("{}: attached {saved}", key(id));
            }
            Ok(0)
        }
        "answer" => {
            let id = need(a, 1, "a task id")?;
            let q = need(a, 2, "a question like Q2")?;
            let n = question_number(q)?;
            let text = a.pos[3..].join(" ");
            let (asker, how) = match a.flags.get("pick") {
                Some(pick) => {
                    let (asker, written) =
                        queue::answer_pick(root, id, n, pick, &text, &label(), stamp)?;
                    (asker, format!(" with {written}"))
                }
                None => (
                    queue::answer(root, id, n, &text, &label(), stamp)?,
                    String::new(),
                ),
            };
            match asker {
                Some(s) => outln!("{}: answered {q}{how}; delivered to session {s}", key(id)),
                None => outln!(
                    "{}: answered {q}{how}; no asking session on record",
                    key(id)
                ),
            }
            Ok(0)
        }
        "claim" if a.takeover => {
            let id = need(a, 1, "a task id, or a backlog note's file name")?;
            if names_a_note(id) {
                let t = backlog::takeover_note(root, id, &me(), stamp)?;
                outln!(
                    "took {} over from {} ({}); you're shaping it now, heartbeat {stamp}",
                    t.note,
                    t.from,
                    t.why
                );
                return Ok(0);
            }
            let t = ops::takeover_task(root, id, &me(), stamp, queue::now_secs())?;
            if let Some(line) = t.rerun_line(&key(id)) {
                outln!("{line}");
                return Ok(0);
            }
            outln!(
                "{}: took over the claim from {} ({}); heartbeat {stamp} ({})",
                key(id),
                t.from,
                t.why,
                t.path
            );
            Ok(0)
        }
        "claim" => {
            let id = need(a, 1, "a task id")?;
            let p = ops::claim_task(root, id, &me(), stamp)?;
            outln!("{}", claimed_message(root, prefix, id, &p));
            Ok(0)
        }
        "next" => {
            let me = me();
            warn_your_check(root, prefix);
            match ops::next_task(root, &me, stamp)? {
                Some(c) => {
                    outln!("{}", claimed_message(root, prefix, &c.id, &c.path));
                    Ok(0)
                }
                None => {
                    outln!(
                        "nothing ready in {} ({}) that {me} may take",
                        projects::default_name(root),
                        root.display()
                    );
                    Ok(1)
                }
            }
        }
        "start" => {
            let id = need(a, 1, "a task id")?;
            let place = match (a.flags.get("at"), a.here) {
                (Some(_), true) => {
                    return Err(
                        "start takes --at <path> or --here, not both (see `muster help`)".into(),
                    )
                }
                (Some(p), false) => crate::parallel::Place::At(PathBuf::from(p)),
                (None, true) => crate::parallel::Place::Here,
                (None, false) => crate::parallel::Place::Default,
            };
            let cwd =
                cwd.ok_or("can't read the current folder: run start from the project's repo")?;
            warn_your_check(root, prefix);
            let s = crate::parallel::start(root, id, &me(), stamp, cwd, &place)?;
            out!("{}", started_text(root, prefix, id, &s));
            set_merge_driver(cwd);
            Ok(0)
        }
        "ready" => ready_cmd(a, root, prefix),
        "release" => {
            let id = need(a, 1, "a task id, or a backlog note's file name")?;
            if names_a_note(id) {
                let h = backlog::release_note(root, id, &me())?;
                outln!(
                    "released {id}: {} isn't shaping it any more",
                    session::split(&h.by).0
                );
                return Ok(0);
            }
            if !a.clean {
                return Err(format!(
                    "release takes --clean: it gives {} back to Ready and removes its worktree and branch (to send it back and keep them: muster return) (see `muster help`)",
                    key(id)
                ));
            }
            for line in crate::parallel::release(root, id, &me(), stamp, a.force, cwd)? {
                outln!("{line}");
            }
            Ok(0)
        }
        "heartbeat" => {
            let id = need(a, 1, "a task id")?;
            ops::heartbeat_task(root, id, &me(), stamp)?;
            outln!("{}: heartbeat {stamp}", key(id));
            Ok(0)
        }
        "checkin" => {
            let id = need(a, 1, "a task id")?;
            need(a, 2, "what's done and what's next")?;
            let text = a.pos[2..].join(" ").replace("\\n", " ");
            let pct = a.flags.get("pct").map(String::as_str).unwrap_or("");
            ops::checkin_task_with(root, id, &text, pct, a.handoff, &me(), stamp)?;
            if a.handoff {
                outln!(
                    "{}: checked in, handing off; heartbeat {stamp}. Your successor runs: muster claim {} --takeover",
                    key(id),
                    key(id)
                );
            } else {
                outln!("{}: checked in; heartbeat {stamp}", key(id));
            }
            Ok(0)
        }
        "ping" => {
            let id = need(a, 1, "a task id")?;
            if a.pos.len() > 2 {
                let p = ping::message(root, id, &a.pos[2..].join(" "), &me(), stamp)?;
                outln!("{}: message sent to session {}", key(id), p.session);
            } else {
                let p = ping::ask_checkin(root, id, &me(), stamp)?;
                outln!("{}: asked session {} for a check-in", key(id), p.session);
            }
            outln!("It arrives after that session's next tool call.");
            Ok(0)
        }
        "reclaim" => {
            // --after, else this machine's lease (0036; 12 h unless changed in Settings).
            let after = a.flags.get("after").map(String::as_str).unwrap_or("");
            let lease = if after.trim().is_empty() {
                machine_settings(env).lease_minutes
            } else {
                ops::lease_minutes(after)?
            };
            let r = ops::reclaim(root, lease, &me(), stamp, queue::now_secs())?;
            if a.json {
                print_json(&r)?;
            } else {
                let within = if after.trim().is_empty() {
                    settings::span_text(lease)
                } else {
                    after.to_string()
                };
                out!("{}", reclaim_text(&r, &within, key));
            }
            for f in &r.failed {
                eprintln!("muster: couldn't release {f}");
            }
            Ok(if r.failed.is_empty() { 0 } else { 1 })
        }
        "ask" => {
            let id = need(a, 1, "a task id")?;
            need(a, 2, "the question")?;
            let choices = crate::choices::clean(
                &a.options,
                a.flags.get("pick").map(String::as_str),
                a.flags.get("no-answer").map(String::as_str),
            )?;
            let asked = queue::ask_with(
                root,
                id,
                &a.pos[2..].join(" "),
                &choices,
                &label(),
                &session(a, env),
                stamp,
            )?;
            outln!(
                "asked Q{} on {}. The answer will be delivered to this session.",
                asked.n,
                key(id)
            );
            if !choices.options.is_empty() {
                out!("{}", options_text(&choices));
            }
            if !asked.earlier.is_empty() {
                outln!("\nQuestions on this task so far (check that yours isn't answered here):");
                for (answered, line) in &asked.earlier {
                    let tag = if *answered { "[answered]" } else { "[open]" };
                    outln!("  {tag} {line}");
                }
            }
            Ok(0)
        }
        "escalate" => {
            let id = need(a, 1, "a task id")?;
            let n = question_number(need(a, 2, "a question like Q2")?)?;
            queue::escalate(root, id, n, &label(), stamp)?;
            // The question is written either way: a failed re-read only leaves its options out.
            let choices = queue::read_task(root, id)
                .ok()
                .and_then(|t| t.questions.into_iter().find(|q| q.n == Some(n)))
                .and_then(|q| q.choices);
            match choices {
                Some(c) => out!(
                    "escalated Q{n} on {}; add it to WAITING-FOR-OWNER.md with its options:\n{}",
                    key(id),
                    options_text(&c)
                ),
                None => outln!(
                    "escalated Q{n} on {}; add it to WAITING-FOR-OWNER.md",
                    key(id)
                ),
            }
            Ok(0)
        }
        "clarify" => {
            if a.pos.len() > 1 {
                return Err("clarify takes no task: it lists every Shaping and Ready task's open questions (muster help clarify)".into());
            }
            if !a.send {
                let b = crate::clarify::read(root);
                if a.json {
                    print_json(&b)?;
                } else {
                    out!("{}", crate::clarify::text(&b));
                }
                return Ok(0);
            }
            let sent = crate::clarify::send(root, &label(), stamp)?;
            let n: usize = sent.iter().map(|t| t.questions.len()).sum();
            if a.json {
                print_json(&sent)?;
            } else if sent.is_empty() {
                outln!("nothing to send: no open question on a Shaping or Ready task that isn't blocked is waiting to go to the owner");
            } else {
                let each: Vec<String> = sent
                    .iter()
                    .map(|t| {
                        let qs: Vec<String> =
                            t.questions.iter().map(|q| format!("Q{}", q.n)).collect();
                        format!("{} {}", t.key, qs.join(", "))
                    })
                    .collect();
                outln!(
                    "sent {} to the owner as one batch ({}); they wait in Waiting for you. Add them to WAITING-FOR-OWNER.md.",
                    crate::clarify::counted(n, sent.len()),
                    each.join("; ")
                );
            }
            Ok(0)
        }
        "inbox" => {
            let mail = queue::take_mail(root, &session(a, env), a.keep)?;
            if mail.is_empty() {
                outln!("no answers waiting");
            } else {
                outln!("{}", mail.join("\n"));
            }
            Ok(0)
        }
        "add" => {
            need(a, 1, "the note (one line)")?;
            let line = a.pos[1..].join(" ");
            let body = body_arg(a)?.unwrap_or_default();
            outln!("{}", backlog::add(root, &line, &body, &me(), stamp)?);
            Ok(0)
        }
        "shape" => {
            let note = need(a, 1, "a backlog note's file name")?;
            // The brief is read first, so a note it can't read is never claimed (0097).
            let b = backlog::shape(root, note)?;
            let claim = if a.peek {
                None
            } else {
                let lease = machine_settings(env).lease_minutes;
                Some(backlog::claim_note(root, note, &me(), stamp, lease)?)
            };
            if a.json {
                print_json(&shape_json(&b, claim.as_ref()))?;
            } else {
                if let Some(c) = &claim {
                    outln!("{}", claim_line(c));
                }
                out!("{}", backlog::brief_text(&b));
            }
            Ok(0)
        }
        "file" => {
            let note = need(a, 1, "a backlog note's file name")?;
            need(a, 2, "the task's title")?;
            let body = body_arg(a)?
                .ok_or("file needs --body <text>|- with the shaped task (see `muster help`)")?;
            let new = new_task(a, &a.pos[2..].join(" "), body);
            let lease = machine_settings(env).lease_minutes;
            let f = backlog::file_note(root, note, &new, &me(), stamp, lease)?;
            if a.json {
                print_json(&with_key(serde_json::json!(f), key(&f.id)))?;
            } else {
                outln!("{}", filed_message(root, prefix, &f));
            }
            // On stderr, so stdout stays the one line (or the JSON) a script reads.
            if let Some(w) = lens_warning(&key(&f.id), &f.missing_lenses) {
                eprintln!("{w}");
            }
            if let Some(w) = marker_warning(&key(&f.id), &f.markers) {
                eprintln!("{w}");
            }
            Ok(0)
        }
        "drop" => {
            let note = need(a, 1, "a backlog note's file name")?;
            need(a, 2, "why it's dropped")?;
            let why = a.pos[2..].join(" ").replace("\\n", " ");
            let lease = machine_settings(env).lease_minutes;
            let p = backlog::drop_note(root, note, &why, &me(), stamp, lease)?;
            outln!("dropped {note} ({p})");
            Ok(0)
        }
        "restore" => {
            if a.all == (a.pos.len() > 1) {
                return Err(
                    "restore takes the files or folders to put back, or --all (see `muster help restore`)"
                        .into(),
                );
            }
            let back = crate::history::restore(root, &a.pos[1..], a.all, &me())?;
            if back.is_empty() {
                outln!("nothing to restore: the queue matches its last commit");
            } else {
                outln!(
                    "restored {} file{} from the queue's history: {}",
                    back.len(),
                    if back.len() == 1 { "" } else { "s" },
                    back.join(", ")
                );
            }
            Ok(0)
        }
        "history" => run_history(a, root, &me),
        "vision" => run_vision(a, root, prefix, &me, stamp),
        "review" => run_review(a, root, prefix, &me, stamp, cwd),
        "lessons" => run_lessons(a, root, &me, stamp),
        "check" => {
            if a.dry_run {
                return Err(
                    "--dry-run goes with --fix-pauses: muster check --fix-pauses --dry-run".into(),
                );
            }
            // A claim is quiet past this machine's lease (0036), as `reclaim` judges it; each task
            // with its key (0033).
            let s = queue::read_state_keyed(
                &[("q".into(), root.to_path_buf())],
                &mut queue::Cache::default(),
                &machine_settings(env),
                &|_| prefix.to_string(),
            );
            for p in &s.problems {
                outln!("{p}");
            }
            // Not problems either: the vision check is the shaper's job (0029).
            for w in vision::unchecked(&s.tasks) {
                outln!("{w}");
            }
            // Nor what parallel sessions left behind (0023): each line says how to clean it. The
            // repo looked in is the one `check` runs in, when it's this queue's project's.
            let here: Vec<PathBuf> = cwd
                .filter(|c| queue_of(c).is_some_and(|q| same_folder(&q, root)))
                .map(|c| vec![c.to_path_buf()])
                .unwrap_or_default();
            for w in crate::parallel::stale(&s.tasks, &here) {
                outln!("{w}");
            }
            // Nor a now page over its cap, without covers:, sharing another's, or a symlink
            // (mu-102): each says what to do, and none of them stops a command.
            for w in crate::now::warnings(&crate::now::for_queue(root)) {
                outln!("{w}");
            }
            // Not problems: what waits on the owner's look (0028). The exit code ignores them.
            for t in s.tasks.iter().filter(|t| t.stage == "your-check") {
                let open: Vec<String> = t
                    .owner_criteria
                    .iter()
                    .filter(|c| !c.ticked)
                    .map(|c| {
                        if c.label.is_empty() {
                            c.n.to_string()
                        } else {
                            c.label.clone()
                        }
                    })
                    .collect();
                if !open.is_empty() {
                    outln!(
                        "{}: waits on the owner to verify {}",
                        key(&t.id),
                        open.join(", ")
                    );
                }
            }
            // And lessons from reviews waiting for the owner's yes (0027).
            let lessons = lessons::waiting(root);
            if lessons > 0 {
                outln!(
                    "{lessons} lesson{} from reviews wait{} for the owner's yes (muster lessons)",
                    if lessons == 1 { "" } else { "s" },
                    if lessons == 1 { "s" } else { "" }
                );
            }
            let index_line = index_check_line(index::status(root));
            if let Some(l) = &index_line {
                outln!("{l}");
            }
            // A local queue's history (0109): files missing since its last commit and writes it
            // couldn't commit fail the check; the rest are warnings.
            let history = crate::history::problems("q", root);
            for l in &history {
                outln!("{l}");
            }
            // A prefix another project has too (0005: an old queue not retired yet after a
            // migrate), and where its keys go: a warning, the exit code ignores it.
            if let Some(l) = crate::key::shared_prefix(&crate::key::known(), root, prefix) {
                outln!("{l}");
            }
            if let Some(w) = merge_driver_warning(root) {
                out!("{w}");
            }
            // Warnings (a goal past its due date, a list due a refresh) are shown but don't fail
            // the check.
            let failed = s
                .problems
                .iter()
                .chain(&index_line)
                .chain(&history)
                .any(|p| !queue::is_warning(p));
            Ok(if failed { 1 } else { 0 })
        }
        "index" => {
            let path = root.join(index::FILE);
            let as_built = a.flags.get("as-built").map(PathBuf::from);
            if a.check && as_built.is_some() {
                return Err(
                    "--check writes nothing, so it doesn't go with --as-built (see `muster help`)"
                        .into(),
                );
            }
            if let Some(doc) = as_built {
                // The caller's home reads as ~ in the section (0080), so no path under it is written.
                let home = env("HOME").unwrap_or_default();
                let (rebuilt, wrote) = index::rebuild_with_glance(root, &doc, &home)?;
                if rebuilt {
                    outln!("rebuilt {}", path.display());
                } else {
                    outln!("{} is up to date", path.display());
                }
                if wrote {
                    outln!("wrote the queue's section of {}", doc.display());
                } else {
                    outln!("{}'s queue section is up to date", doc.display());
                }
                return Ok(0);
            }
            if a.check {
                return Ok(match index::status(root) {
                    index::Status::Current => {
                        outln!("{} is up to date", path.display());
                        0
                    }
                    index::Status::Stale => {
                        outln!("{}", index::STALE);
                        1
                    }
                    index::Status::Refresh => {
                        outln!("{}", index::REFRESH);
                        1
                    }
                    index::Status::Missing => {
                        outln!("there's no INDEX.md yet (make it with `muster index`)");
                        1
                    }
                });
            }
            if index::rebuild(root)? {
                outln!("rebuilt {}", path.display());
            } else {
                outln!("{} is up to date", path.display());
            }
            Ok(0)
        }
        other => Err(format!("unknown command {other:?} (see `muster help`)")),
    }
}

/// `muster review` (0027): the brief for the next round of a started task, `--record` the
/// reviewer's result (`-` reads it from stdin), the builder's `--fixed R1 ["<how>"]` or `--dispute
/// R1 "<why>"`, or the owner's `--settle R1 <label> "<why>"`.
fn run_review(
    a: &Args,
    root: &Path,
    prefix: &str,
    me: &dyn Fn() -> String,
    stamp: &str,
    cwd: Option<&Path>,
) -> Result<i32, String> {
    let id = need(a, 1, "a task id")?;
    let k = crate::key::key(prefix, id);
    let writes: Vec<&str> = ["record", "fixed", "dispute", "settle"]
        .into_iter()
        .filter(|f| a.flags.contains_key(*f))
        .collect();
    if writes.len() > 1 {
        return Err(
            "review takes one of --record, --fixed, --dispute or --settle (see `muster help`)"
                .into(),
        );
    }
    let still_open = |open: Vec<String>| match open.len() {
        0 => "nothing open".to_string(),
        n => format!("{n} still open ({})", open.join(", ")),
    };
    let rest = |from: usize| {
        a.pos
            .get(from..)
            .unwrap_or_default()
            .join(" ")
            .replace("\\n", " ")
    };
    match writes.first().copied() {
        None => {
            if a.pos.len() > 2 {
                return Err(format!(
                    "review takes one task id; to answer a finding: muster review {k} --fixed R1 \"<how>\" (see `muster help`)"
                ));
            }
            let span = review::span_in(root, id, cwd)?;
            let b = review::brief(root, id, &span, stamp)?;
            if a.json {
                print_json(&b)?;
            } else {
                out!("{}", review::brief_text(&b));
            }
            Ok(0)
        }
        Some("record") => {
            if a.pos.len() > 2 {
                return Err("review --record takes the JSON as its value, or - to read it from stdin; nothing after the id (see `muster help`)".into());
            }
            let json = match a.flags.get("record").map(String::as_str) {
                Some("-") => read_capped(std::io::stdin(), "stdin")?,
                Some(j) => j.to_string(),
                None => String::new(),
            };
            let span = review::span_in(root, id, cwd)?;
            let r = review::record(root, id, &json, &me(), stamp, &span)?;
            if a.json {
                print_json(&r)?;
            } else {
                out!("{}", review::recorded_text(&k, &r));
            }
            Ok(if r.then_failed.is_empty() { 0 } else { 1 })
        }
        Some(flag) if flag == "fixed" || flag == "dispute" => {
            let which = a.flags.get(flag).map(String::as_str).unwrap_or("");
            let (reply, word) = if flag == "fixed" {
                (review::Reply::Fixed, "fixed")
            } else {
                need(a, 2, "why it's wrong")?;
                (review::Reply::Disputed, "disputed")
            };
            let open = review::answer(root, id, which, reply, &rest(2), &me(), stamp)?;
            outln!(
                "{k}: {} answered as {word}; {}. Pass it to Testing once every finding is answered and the gate is green.",
                which.trim().to_uppercase(),
                still_open(open)
            );
            Ok(0)
        }
        Some(_) => {
            let which = a.flags.get("settle").map(String::as_str).unwrap_or("");
            let label = need(
                a,
                2,
                "the label: confirmed, false-positive, wont-fix or open",
            )?;
            need(a, 3, "why")?;
            let open = review::settle(root, id, which, label, &rest(3), &me(), stamp)?;
            outln!(
                "{k}: {} settled as {label}; {}",
                which.trim().to_uppercase(),
                still_open(open)
            );
            Ok(0)
        }
    }
}

/// `L2`, `l2` or `2`, comma-separated, as `L2`s.
fn lesson_ids(list: &str) -> Result<Vec<String>, String> {
    list.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            let n = s.strip_prefix(['L', 'l']).unwrap_or(s);
            match n.parse::<u32>() {
                Ok(v) if n.len() <= 4 && n.bytes().all(|b| b.is_ascii_digit()) => {
                    Ok(format!("L{v}"))
                }
                _ => Err(format!(
                    "a lesson looks like L2, got {:?}",
                    s.chars().take(20).collect::<String>()
                )),
            }
        })
        .collect()
}

/// `muster lessons` (0027): what waits for the owner and what's live; the owner's `accept
/// <L1,L2|--all>` and `reject <L1,L2> "<why>"`.
fn run_lessons(a: &Args, root: &Path, me: &dyn Fn() -> String, stamp: &str) -> Result<i32, String> {
    let decided = match a.pos.get(1).map(String::as_str) {
        None => {
            let l = lessons::listing(root, stamp)?;
            if a.json {
                print_json(&l)?;
            } else {
                out!("{}", lessons::listing_text(&l));
            }
            return Ok(0);
        }
        Some("accept") => {
            let ids = match (a.all, a.pos.get(2)) {
                (true, None) => Vec::new(),
                (false, Some(list)) if a.pos.len() == 3 => lesson_ids(list)?,
                _ => {
                    return Err(
                        "lessons accept takes the lessons (L1,L2) or --all (see `muster help`)"
                            .into(),
                    )
                }
            };
            lessons::decide(root, &ids, a.all, &[], "", &me(), stamp)?
        }
        Some("reject") => {
            let ids = lesson_ids(need(a, 2, "the lessons to reject (L1,L2)")?)?;
            need(a, 3, "why")?;
            let why = a.pos[3..].join(" ").replace("\\n", " ");
            lessons::decide(root, &[], false, &ids, &why, &me(), stamp)?
        }
        Some(other) => {
            return Err(format!(
                "lessons takes accept or reject, got {:?} (see `muster help`)",
                other.chars().take(20).collect::<String>()
            ))
        }
    };
    if a.json {
        print_json(&decided)?;
    } else {
        let mut parts = Vec::new();
        for (word, ids) in [
            ("accepted", &decided.accepted),
            ("rejected", &decided.rejected),
            ("expired (over the cap)", &decided.expired),
        ] {
            if !ids.is_empty() {
                parts.push(format!("{word} {}", ids.join(", ")));
            }
        }
        outln!("lessons: {}", parts.join("; "));
    }
    Ok(0)
}

/// `muster vision`: one task's brief, `--record` an agent's result (`-` reads it from stdin),
/// `--resolve V2 "<how>"`, or `--all` for the whole queue's inputs.
fn run_vision(
    a: &Args,
    root: &Path,
    prefix: &str,
    me: &dyn Fn() -> String,
    stamp: &str,
) -> Result<i32, String> {
    let (record, resolve) = (a.flags.get("record"), a.flags.get("resolve"));
    if a.all {
        if a.pos.len() > 1 || record.is_some() || resolve.is_some() {
            return Err(
                "vision --all takes no task id, --record or --resolve (see `muster help`)".into(),
            );
        }
        let all = vision::all(root);
        if a.json {
            print_json(&all)?;
        } else {
            out!("{}", vision::all_text(&all));
        }
        return Ok(0);
    }
    let id = need(a, 1, "a task id, or --all")?;
    match (record, resolve) {
        (Some(_), Some(_)) => {
            Err("vision takes --record or --resolve, not both (see `muster help`)".into())
        }
        (Some(json), None) => {
            if a.pos.len() > 2 {
                return Err("vision --record takes the JSON as its value, or - to read it from stdin; nothing after the id (see `muster help`)".into());
            }
            let json = if json == "-" {
                read_capped(std::io::stdin(), "stdin")?
            } else {
                json.clone()
            };
            let r = vision::record(root, id, &json, &me(), stamp)?;
            if a.json {
                print_json(&r)?;
            } else {
                out!("{}", vision::recorded_text(&r));
            }
            Ok(0)
        }
        (None, Some(which)) => {
            need(a, 2, "how it was resolved")?;
            let how = a.pos[2..].join(" ").replace("\\n", " ");
            let left = vision::resolve(root, id, which, &how, &me(), stamp)?;
            outln!(
                "{}: vision conflict {} resolved; {left} still open",
                crate::key::key(prefix, id),
                which.trim()
            );
            Ok(0)
        }
        (None, None) => {
            if a.pos.len() > 2 {
                return Err(format!(
                    "vision takes one task id; to resolve a conflict: muster vision {} --resolve V1 \"<how>\" (see `muster help`)",
                    crate::key::key(prefix, id)
                ));
            }
            let b = vision::brief(root, id)?;
            if a.json {
                print_json(&b)?;
            } else {
                out!("{}", vision::brief_text(&b));
            }
            Ok(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The queue at `root`, as the rules read it (no prefix).
    fn tasks_of(root: &Path) -> queue::State {
        queue::read_state(
            &[("q".into(), root.to_path_buf())],
            &mut queue::Cache::default(),
        )
    }

    fn sv(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    /// A stdout that takes `room` bytes, then fails every write and flush with `kind`.
    struct Pipe {
        got: Vec<u8>,
        room: usize,
        kind: std::io::ErrorKind,
        tries: usize,
    }

    impl Pipe {
        fn new(room: usize, kind: std::io::ErrorKind) -> Self {
            Self {
                got: Vec::new(),
                room,
                kind,
                tries: 0,
            }
        }
    }

    impl std::io::Write for Pipe {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.tries += 1;
            let n = b.len().min(self.room - self.got.len());
            if n == 0 {
                return Err(self.kind.into());
            }
            self.got.extend_from_slice(&b[..n]);
            Ok(n)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.tries += 1;
            if self.got.len() == self.room {
                Err(self.kind.into())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn stdout_passes_text_through_unchanged() {
        let out = Stdout::new();
        let mut w = Pipe::new(1024, std::io::ErrorKind::BrokenPipe);
        out.write(&mut w, "صف کارها: mu-1\n");
        out.write(&mut w, "next");
        out.flush(&mut w);
        assert_eq!(String::from_utf8(w.got).unwrap(), "صف کارها: mu-1\nnext");
        assert!(!out.ended() && !out.failed());
        assert_eq!(out.failure_line(0), None);
    }

    #[test]
    fn stdout_ends_quietly_when_the_reader_leaves() {
        let out = Stdout::new();
        out.writes("note");
        let mut w = Pipe::new(6, std::io::ErrorKind::BrokenPipe);
        out.write(&mut w, "first\n");
        out.write(&mut w, "second\n");
        let tries = w.tries;
        out.write(&mut w, "third\n");
        out.flush(&mut w);
        assert_eq!(w.got, b"first\n");
        assert_eq!(w.tries, tries, "no write after the pipe broke");
        assert!(out.ended() && !out.failed());
        assert_eq!(out.failure_line(0), None, "nothing on stderr");
        assert_eq!(out_status(0, out.failed()), 0);
    }

    /// The command as `run_with` marks it: a write unless `reads_only` says otherwise.
    fn ran(argv: &[&str]) -> Stdout {
        let a = parse(&argv.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap();
        let out = Stdout::new();
        if !reads_only(&a) {
            out.writes(argv[0]);
        }
        out
    }

    #[test]
    fn a_write_whose_output_fails_says_its_change_is_saved_and_not_to_run_it_again() {
        let out = ran(&["note", "mu-7", "x"]);
        let mut w = Pipe::new(3, std::io::ErrorKind::StorageFull);
        out.write(&mut w, "mu-7: note added\n");
        out.write(&mut w, "second\n");
        out.flush(&mut w);
        assert!(out.ended() && out.failed());
        let line = out.failure_line(0).unwrap();
        assert!(
            line.starts_with(
                "muster: note ran and its change is saved, but its output couldn't be written ("
            ) && line.ends_with("): don't run it again"),
            "{line}"
        );
        assert_eq!(line.lines().count(), 1, "{line}");
        assert_eq!(out_status(0, out.failed()), 1);
        // A refusal changed nothing: it says only the output failed and keeps its status.
        assert!(out
            .failure_line(2)
            .unwrap()
            .starts_with("muster: can't write the output: "));
        assert_eq!(
            out_status(2, out.failed()),
            2,
            "a refusal keeps its own status"
        );
    }

    #[test]
    fn a_read_whose_output_fails_says_only_that() {
        let out = ran(&["show", "mu-7"]);
        let mut w = Pipe::new(3, std::io::ErrorKind::StorageFull);
        out.write(&mut w, "mu-7 · Building\n");
        out.flush(&mut w);
        let line = out.failure_line(0).unwrap();
        assert!(
            line.starts_with("muster: can't write the output: "),
            "{line}"
        );
        assert!(!line.contains("don't run it again"), "{line}");
        assert_eq!(out_status(0, out.failed()), 1);
    }

    #[test]
    fn reads_only_knows_the_reads_and_takes_the_rest_for_writes() {
        let reads = |argv: &[&str]| {
            reads_only(&parse(&argv.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap())
        };
        for r in [
            &["show", "mu-7"][..],
            &["list", "--json"],
            &["help"],
            &["inbox", "--keep"],
            &["review", "mu-7"],
            &["vision", "mu-7"],
            &["projects"],
            &["index", "--check"],
            &["check"],
        ] {
            assert!(reads(r), "{r:?}");
        }
        for w in [
            &["note", "mu-7", "x"][..],
            &["add", "x"],
            &["new", "x"],
            &["ask", "mu-7", "why"],
            &["comment", "mu-7", "x"],
            &["pass", "mu-7"],
            &["inbox"],
            &["review", "mu-7", "--fixed", "R1", "how"],
            &["projects", "set", "x", "gate", "y"],
            &["index"],
            &["check", "--fix-pauses"],
            &["some-new-command"],
        ] {
            assert!(!reads(w), "{w:?}");
        }
    }

    #[test]
    fn stdout_reports_a_failure_found_by_the_last_flush() {
        let out = Stdout::new();
        let mut w = Pipe::new(4, std::io::ErrorKind::StorageFull);
        out.write(&mut w, "last");
        out.flush(&mut w);
        assert_eq!(w.got, b"last");
        assert!(out.failed());
        let err = out.failure_line(0).unwrap();
        assert!(err.starts_with("muster: can't write the output: "), "{err}");
        assert_eq!(err.lines().count(), 1, "{err}");
    }

    #[test]
    fn muster_projects_lists_the_real_name_and_says_the_label_and_category_after() {
        let base = crate::testkit::tempdir().unwrap();
        let q = base.path().join("muster/tasks");
        std::fs::create_dir_all(q.join("todo")).unwrap();
        std::fs::write(q.join("README.md"), "q").unwrap();
        let mut c = projects::Config::default();
        let line = |c: &projects::Config| project_line(&projects::list(base.path(), c)[0]);
        assert!(!line(&c).contains("label"), "{}", line(&c));
        let key = projects::list(base.path(), &c)[0].key.clone();
        c.labels.insert(key.clone(), "Muster".into());
        c.categories.insert(key, "Apps".into());
        let l = line(&c);
        assert!(
            l.starts_with("muster "),
            "the name commands take comes first: {l}"
        );
        assert!(l.ends_with("  label: Muster  category: Apps"), "{l}");
    }

    /// No caller environment: the tests run inside Claude Code sessions, whose own session must
    /// not be stamped on what they check.
    fn no_env(_: &str) -> Option<String> {
        None
    }
    fn main(argv: &[String]) -> i32 {
        main_with(argv, &no_env)
    }
    fn run(argv: &[String]) -> Result<i32, String> {
        run_with(argv, &no_env)
    }

    /// The lease and LIVE window come from the caller's $HOME (0036), so these tests, which give
    /// no environment, use the defaults whatever this Mac's settings.json says.
    #[test]
    fn settings_come_from_the_callers_home_and_are_the_defaults_without_one() {
        assert_eq!(machine_settings(&no_env), settings::Settings::default());
        let home = crate::testkit::tempdir().unwrap();
        let dir = home
            .path()
            .join("Library/Application Support/ai.sweetlemon.muster");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(settings::FILE), r#"{"leaseMinutes": 90}"#).unwrap();
        let h = home.path().to_string_lossy().to_string();
        let env = move |k: &str| (k == "HOME").then(|| h.clone());
        assert_eq!(machine_settings(&env).lease_minutes, 90);
        assert_eq!(
            machine_settings(&|_: &str| Some(String::new())),
            settings::Settings::default()
        );
    }

    /// Every file under `root`, with what it says, by path: to see that nothing was written.
    fn files_under(root: &Path) -> Vec<(PathBuf, String)> {
        let mut out = Vec::new();
        let mut dirs = vec![root.to_path_buf()];
        while let Some(d) = dirs.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    dirs.push(p);
                } else {
                    out.push((p.clone(), std::fs::read_to_string(&p).unwrap()));
                }
            }
        }
        out.sort();
        out
    }

    /// Drover's old queue with dr-22 paused before 0045 and a real block beside it, and Muster's
    /// `.muster/` with mu-42 blocked on you as paused, holding them both.
    fn old_pause_queues() -> (crate::testkit::TestDir, Vec<(String, PathBuf)>) {
        let base = crate::testkit::tempdir().unwrap();
        let dr = base.path().join("drover/tasks");
        for f in queue::STATUS_FOLDERS {
            std::fs::create_dir_all(dr.join(f)).unwrap();
        }
        std::fs::write(dr.join("README.md"), "---\nprefix: dr\n---\n\nq").unwrap();
        std::fs::write(
            dr.join("blocked/0022-pets.md"),
            "---\nid: 0022\ntype: feature\nstatus: blocked\nclaimed-by:\nheartbeat:\nwaiting_on: owner (paused)\n---\n\n# Desk pets\n\n## Log\n- 2026-10-12 capture: created\n- 2026-10-13 02:51 owner: \"pause the pet project\". Paused; claim released.\n",
        )
        .unwrap();
        std::fs::write(
            dr.join("blocked/0023-api.md"),
            "---\nid: 0023\nstatus: blocked\nstage: ready\nwaiting_on: waiting on the design\nblocked-kind: outside\n---\n\n# The API\n\n## Log\n- 2026-10-13 08:00 capture: created\n",
        )
        .unwrap();
        let mu = base.path().join("muster/main").join(crate::layout::DIR);
        std::fs::create_dir_all(mu.join("tasks")).unwrap();
        std::fs::write(
            mu.join(crate::layout::CONFIG),
            crate::layout::config_text(true, Some("mu")),
        )
        .unwrap();
        std::fs::write(
            mu.join("tasks/0042-t.md"),
            "---\nid: 0042\nstage: building\nclaimed-by: wt-a\nheartbeat: 2026-10-13 09:00\nwaiting_on: owner: paused: the design first\nblocked-kind: you\n---\n\n# The parser\n\n## Log\n- 2026-10-13 08:00 capture: created\n- 2026-10-13 10:00 owner: blocked on you: paused: the design first\n",
        )
        .unwrap();
        let queues = vec![("drover".to_string(), dr), ("muster".to_string(), mu)];
        (base, queues)
    }

    #[test]
    fn fix_pauses_lists_first_then_converts_every_queue_and_a_dry_run_changes_nothing() {
        let (base, queues) = old_pause_queues();
        let now = "2026-10-13 18:00";
        let who = |_: &Path| "owner".to_string();
        let before = files_under(base.path());
        let listed = [
            "dr-22 (drover) Desk pets · Ready · paused since 2026-10-13 02:51 · no reason given",
            "mu-42 (muster) The parser · Building · paused since 2026-10-13 10:00 · reason: the design first",
        ];
        let (lines, code) = fix_pauses(&queues, true, &who, now);
        assert_eq!(
            lines,
            [
                listed[0],
                listed[1],
                "dry run: 2 old paused blocks would become pauses; nothing changed"
            ]
        );
        assert_eq!(code, 0);
        assert_eq!(
            files_under(base.path()),
            before,
            "nothing written, not even the lock"
        );
        let (lines, code) = fix_pauses(&queues, false, &who, now);
        assert_eq!(
            lines,
            [
                listed[0],
                listed[1],
                "turned 2 old paused blocks into pauses"
            ]
        );
        assert_eq!(code, 0);
        let dr = queue::read_task(&queues[0].1, "0022").unwrap();
        assert_eq!(
            (dr.paused.as_str(), dr.blocked, dr.path.as_str()),
            ("2026-10-13 02:51", false, "todo/0022-pets.md")
        );
        assert!(
            queue::read_task(&queues[0].1, "0023").unwrap().blocked,
            "a real block stays"
        );
        let mu = queue::read_task(&queues[1].1, "0042").unwrap();
        assert_eq!(
            (
                mu.paused_why.as_str(),
                mu.stage.as_str(),
                mu.claimed_by.as_str()
            ),
            ("the design first", "building", "wt-a")
        );
        // Again: nothing to do.
        let after = files_under(base.path());
        assert_eq!(
            fix_pauses(&queues, false, &who, "2026-10-13 19:00"),
            (vec!["no old paused blocks in 2 queues".to_string()], 0)
        );
        assert_eq!(files_under(base.path()), after);
    }

    #[test]
    fn fix_pauses_says_which_it_couldnt_convert_and_exits_1() {
        let (_base, queues) = old_pause_queues();
        let (lines, _) = fix_pauses(
            &queues[..1],
            true,
            &|_: &Path| "owner".to_string(),
            "2026-10-13 18:00",
        );
        assert_eq!(
            lines.last().map(String::as_str),
            Some("dry run: 1 old paused block would become a pause; nothing changed")
        );
        // wt-a holds another task in Building, so mu-42 can't be unblocked for them.
        std::fs::write(
            queues[1].1.join("tasks/0041-t.md"),
            "---\nid: 0041\nstage: building\nclaimed-by: wt-a\nheartbeat: 2026-10-13 09:00\nwaiting_on:\n---\n\n# Other\n\n## Log\n- 2026-10-13 08:00 capture: created\n",
        )
        .unwrap();
        let (lines, code) = fix_pauses(
            &queues,
            false,
            &|_: &Path| "owner".to_string(),
            "2026-10-13 18:00",
        );
        assert_eq!(code, 1);
        assert_eq!(lines.len(), 4, "{lines:?}");
        assert!(
            lines[2].starts_with("not converted: mu-42 (muster): wt-a already holds mu-41"),
            "{lines:?}"
        );
        assert_eq!(
            lines[3],
            "turned 1 old paused block into a pause; 1 couldn't be, and is as it was"
        );
        assert!(queue::read_task(&queues[1].1, "0042").unwrap().blocked);
    }

    #[test]
    fn check_takes_dry_run_only_with_fix_pauses() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        assert_eq!(code_in(&q, &[], &["check", "--dry-run"]), 1);
        assert_eq!(code_in(&q, &[], &["check"]), 0);
    }

    #[test]
    fn check_fix_pauses_on_the_command_line_dry_run_first_then_for_real() {
        let (base, queues) = old_pause_queues();
        let dr = queues[0].1.clone();
        let q = dr.to_string_lossy().to_string();
        let owner = [("MUSTER_AS", "owner")];
        let before = files_under(base.path());
        assert_eq!(
            code_in(&q, &owner, &["check", "--fix-pauses", "--dry-run"]),
            0
        );
        assert_eq!(files_under(base.path()), before, "a dry run writes nothing");
        assert_eq!(code_in(&q, &owner, &["check", "--fix-pauses"]), 0);
        let t = queue::read_task(&dr, "0022").unwrap();
        assert_eq!(
            (t.paused.as_str(), t.blocked, t.path.as_str()),
            ("2026-10-13 02:51", false, "todo/0022-pets.md")
        );
        let text = std::fs::read_to_string(dr.join("todo/0022-pets.md")).unwrap();
        assert!(
            text.contains(
                "\n- 2026-10-13 02:51 owner: paused\n- 2026-10-13 02:51 owner: unblocked"
            ),
            "{text}"
        );
        // Only the queue --queue named: Muster's mu-42 is still an old pause.
        assert!(queue::read_task(&queues[1].1, "0042").unwrap().blocked);
        assert_eq!(code_in(&q, &owner, &["check"]), 0);
    }

    /// With no --queue, `check --fix-pauses` works on every queue Muster knows: in a test, the
    /// ones it gave (`projects::use_places`), else none, never this Mac's (task 0062: a test once
    /// converted Drover's real dr-22 this way).
    #[test]
    fn check_fix_pauses_with_no_queue_converts_only_the_projects_the_test_gave() {
        let (base, queues) = old_pause_queues();
        let owner = |k: &str| (k == "MUSTER_AS").then(|| "owner".to_string());
        let fix = sv(&["check", "--fix-pauses"]);
        let before = files_under(base.path());
        assert_eq!(main_with(&fix, &owner), 0);
        assert_eq!(files_under(base.path()), before, "not given: none known");
        // Drover's found under the base, Muster's `.muster/` (no repo around it) added by hand.
        let given = crate::testkit::tempdir().unwrap();
        let config_file = given.path().join("projects.json");
        let mut c = projects::Config::default();
        c.added.push(queues[1].1.to_string_lossy().to_string());
        projects::save(&config_file, &c).unwrap();
        projects::use_places(projects::Places {
            base: base.path().to_path_buf(),
            config_file,
        });
        assert_eq!(main_with(&fix, &owner), 0);
        let dr = queue::read_task(&queues[0].1, "0022").unwrap();
        assert_eq!(
            (dr.paused.as_str(), dr.blocked),
            ("2026-10-13 02:51", false)
        );
        let mu = queue::read_task(&queues[1].1, "0042").unwrap();
        assert_eq!(
            (mu.paused_why.as_str(), mu.blocked),
            ("the design first", false)
        );
    }

    /// `muster list` as text, whole.
    fn list_text(notes: &[&backlog::Note], tasks: &[&queue::Task]) -> String {
        let (head, tail) = list_parts(notes, tasks, "2026-10-14 14:14");
        head + &tail
    }

    /// A queue with `checking` agents' tasks in Your check, the owner's own and a feature there
    /// too (neither counts), one task Ready and one Done.
    fn checking_queue(checking: usize) -> crate::testkit::TestDir {
        let d = v2_queue();
        let file = |id: String, fm: &str| {
            std::fs::write(
                d.path().join(format!("doing/{id}-t.md")),
                format!("---\nid: {id}\ntype: task\npriority: normal\n{fm}\n---\n\n# T\n\n## Log\n- 2026-10-12 09:00 capture: created\n"),
            )
            .unwrap();
        };
        for n in 0..checking {
            file(format!("{:04}", n + 10), "status: doing\nstage: your-check");
        }
        file("0001".into(), "status: todo\nstage: ready");
        file("0002".into(), "status: done\nstage: done");
        file(
            "0003".into(),
            "status: doing\nstage: your-check\nfor: human",
        );
        file(
            "0004".into(),
            "status: doing\nstage: your-check\nlevel: feature",
        );
        d
    }

    /// projects.json with the queue at `root` limited to `value` (as Settings › Projects sets it).
    fn limited(root: &Path, value: &str) -> projects::Config {
        let mut c = projects::Config::default();
        check_limit::set(&mut c.your_check, &root.to_string_lossy(), value).unwrap();
        c
    }

    #[test]
    fn migrate_carries_the_limit_into_projects_json_under_its_lock_and_a_dry_run_writes_nothing() {
        let dir = crate::testkit::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let old = base.join("proj/tasks");
        std::fs::create_dir_all(old.join("todo")).unwrap();
        std::fs::write(old.join("README.md"), "q").unwrap();
        let main = base.join("proj/main");
        std::fs::create_dir_all(main.join(".git")).unwrap();
        let new = main.join(crate::layout::DIR);
        std::fs::create_dir(&new).unwrap();
        std::fs::write(
            new.join(crate::layout::CONFIG),
            crate::layout::config_text(true, None),
        )
        .unwrap();
        let file = base.join("cfg/projects.json");
        let old_key = old.to_string_lossy().to_string();
        projects::edit(&file, |c| {
            check_limit::set(&mut c.your_check, &old_key, "8")
        })
        .unwrap();
        let before = std::fs::read_to_string(&file).unwrap();
        // A dry run says what it would carry and writes nothing.
        let said = carry_projects(&file, &base, &old, &new, true).unwrap();
        assert_eq!(
            said,
            ["projects.json: Your check limit 8 carried to the new queue"]
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
        // The real one saves it; the old queue keeps its own.
        let said = carry_projects(&file, &base, &old, &new, false).unwrap();
        assert_eq!(said.len(), 1);
        let c = projects::load(&file);
        assert_eq!(check_limit::limit_for(&c.your_check, &new), Some(8));
        assert_eq!(check_limit::limit_for(&c.your_check, &old), Some(8));
        // Nothing to carry: projects.json isn't written (or made).
        let none = base.join("cfg2/projects.json");
        assert!(carry_projects(&none, &base, &old, &new, false)
            .unwrap()
            .is_empty());
        assert!(!none.exists());
    }

    #[test]
    fn the_your_check_line_shows_only_over_the_projects_limit() {
        let none = projects::Config::default();
        let over = |n: usize| {
            let d = checking_queue(n);
            your_check_line(d.path(), &tasks_of(d.path()).tasks, &none)
        };
        // Nothing set: the default, 5.
        assert_eq!(over(5), None, "at the limit");
        assert_eq!(
            over(6).as_deref(),
            Some("muster: 6 tasks wait in Your check (limit 5): the owner's review is the bottleneck.")
        );
        // The queue folder's own limit.
        let d = checking_queue(3);
        let tasks = tasks_of(d.path()).tasks;
        let with = |v: &str| your_check_line(d.path(), &tasks, &limited(d.path(), v));
        assert_eq!(
            with("2").as_deref(),
            Some("muster: 3 tasks wait in Your check (limit 2): the owner's review is the bottleneck.")
        );
        assert_eq!(with("3"), None);
        assert_eq!(with("off"), None);
        // Another folder's limit leaves this one at 5.
        let other = checking_queue(0);
        assert_eq!(
            your_check_line(d.path(), &tasks, &limited(other.path(), "1")),
            None
        );
        let d = checking_queue(40);
        assert_eq!(
            your_check_line(
                d.path(),
                &tasks_of(d.path()).tasks,
                &limited(d.path(), "off")
            ),
            None,
            "off"
        );
    }

    #[test]
    fn over_the_limit_the_commands_still_do_their_work_with_the_same_exit_codes() {
        let d = checking_queue(7);
        let q = d.path().to_string_lossy().to_string();
        let code = |args: &[&str]| {
            let mut v = sv(args);
            v.extend(sv(&["--queue", &q, "--as", "wt-a"]));
            main(&v)
        };
        assert!(your_check_line(
            d.path(),
            &tasks_of(d.path()).tasks,
            &projects::Config::default()
        )
        .is_some());
        assert_eq!(code(&["list"]), 0);
        assert_eq!(code(&["list", "--json"]), 0);
        assert_eq!(code(&["ready", "--parallel"]), 0);
        assert_eq!(code(&["ready", "--parallel", "2", "--json"]), 0);
        assert_eq!(code(&["next"]), 0, "claims 0001");
        assert_eq!(
            queue::read_task(d.path(), "0001").unwrap().stage,
            "building"
        );
        assert_eq!(
            code(&["next"]),
            1,
            "one claim per holder, as without the limit"
        );
        // list cuts its text after Your check, where the line goes.
        let s = tasks_of(d.path());
        let all = list_filter(&s.tasks, &lq(None, None, None));
        let (head, tail) = list_parts(&[], &all, "2026-10-14 14:14");
        assert!(head.contains("Your check (9)\n"), "{head}");
        assert!(!head.contains("Done ("), "{head}");
        assert!(tail.starts_with("Done (1)\n"), "{tail}");
        // Without Done, everything is above the line.
        let open: Vec<_> = all.iter().copied().filter(|t| t.stage != "done").collect();
        assert_eq!(list_parts(&[], &open, "2026-10-14 14:14").1, "");
    }

    #[test]
    fn start_over_the_limit_still_starts() {
        use crate::parallel::tests::{fx, task};
        let f = fx();
        let id = task(&f.root, "Started anyway", "cli/");
        for n in 10..17 {
            std::fs::write(
                f.root.join(format!("doing/00{n}-t.md")),
                format!("---\nid: 00{n}\nstatus: doing\nstage: your-check\n---\n\n# T\n"),
            )
            .unwrap();
        }
        assert!(your_check_line(
            &f.root,
            &tasks_of(&f.root).tasks,
            &projects::Config::default()
        )
        .is_some());
        let a = parse(&sv(&["start", &id, "--as", "wt-a"])).unwrap();
        assert_eq!(
            run_on(
                &a,
                "start",
                &f.root,
                "pj",
                "2026-10-13 10:00",
                &no_env,
                Some(&f.repo),
            ),
            Ok(0)
        );
        assert_eq!(queue::read_task(&f.root, &id).unwrap().stage, "building");
    }

    #[test]
    fn a_task_merged_by_hand_is_a_warning_check_passes_with() {
        use crate::parallel::tests::{commit, fx, g, task};
        let f = fx();
        let id = task(&f.root, "Merged by hand", "cli/");
        // Stamped now, so the claim isn't quiet past the lease, which would fail the check.
        let now = local_stamp();
        let check = || {
            run_on(
                &parse(&sv(&["check"])).unwrap(),
                "check",
                &f.root,
                "pj",
                &now,
                &no_env,
                Some(&f.repo),
            )
        };
        assert_eq!(check(), Ok(0));
        let s = crate::parallel::start(
            &f.root,
            &id,
            "wt-a",
            &now,
            &f.repo,
            &crate::parallel::Place::Default,
        )
        .unwrap();
        commit(&s.worktree, "cli/x.txt", "x\n", "built");
        g(&f.repo, &["merge", "-q", "--no-edit", &s.branch]);
        let worktree = s.worktree.to_str().unwrap().to_string();
        // What check reads and prints: the state keyed with the project's prefix.
        let lines = || {
            let s = queue::read_state_keyed(
                &[("q".into(), f.root.clone())],
                &mut queue::Cache::default(),
                &settings::Settings::default(),
                &|_| "pj".to_string(),
            );
            crate::parallel::stale(&s.tasks, std::slice::from_ref(&f.repo))
        };
        let first = lines();
        assert!(
            first[0].starts_with("pj-1 is in Building, but its branch task/pj-1-merged-by-hand"),
            "{first:#?}"
        );
        assert_eq!(check(), Ok(0));
        // Each printed command, run as printed with its placeholders filled in, catches the stage
        // up: on a task with no summary, and with its worktree gone by the second.
        let (_, fix) = first[0].split_once(": muster ").unwrap();
        let cmds: Vec<String> = format!("muster {fix}")
            .split(", then ")
            .map(String::from)
            .collect();
        assert_eq!(cmds.len(), 2, "{cmds:?}");
        let argv = |c: &str, who: &str| {
            let c = c
                .replace("<the gate's line>", "gate: green (by hand)")
                .replace("<what was built>", "Built and merged by hand");
            let mut words: Vec<String> = c
                .split('"')
                .enumerate()
                .flat_map(|(i, part)| match i % 2 {
                    1 => vec![part.to_string()],
                    _ => part.split_whitespace().map(String::from).collect(),
                })
                .collect();
            assert_eq!(words.remove(0), "muster");
            // A key on the command line is its id by the time run_on sees it.
            words[1] = crate::key::typed(&words[1]).unwrap().id;
            words.extend(["--as".to_string(), who.to_string()]);
            parse(&words).unwrap()
        };
        assert_eq!(queue::read_task(&f.root, &id).unwrap().summary, "");
        let built = argv(&cmds[0], "wt-a");
        assert_eq!(
            run_on(&built, "pass", &f.root, "pj", &now, &no_env, Some(&f.repo)),
            Ok(0)
        );
        assert_eq!(queue::read_task(&f.root, &id).unwrap().stage, "testing");
        crate::parallel::tests::review_clean(&f, &id);
        g(&f.repo, &["worktree", "remove", &worktree]);
        let second = lines();
        assert!(
            second[0].ends_with(&format!(": {}", cmds[1])),
            "{second:#?}"
        );
        // The pass from Testing is the merge. Its gate is the project's, from projects.json,
        // which a test never reads: handed in as `pass` finds it.
        let tested = argv(&cmds[1], "qa");
        assert_eq!(
            merge_cmd(
                &tested,
                &f.root,
                "pj",
                &tested.pos[1],
                "qa",
                Some(("proj".into(), "true".into())),
                Some(&f.repo)
            ),
            Ok(0)
        );
        let t = queue::read_task(&f.root, &id).unwrap();
        assert_eq!((t.stage.as_str(), t.branch.as_str()), ("your-check", ""));
        assert_eq!(lines(), Vec::<String>::new());
        assert_eq!(check(), Ok(0));
    }

    /// `list --status --stage --tag`, as the options give it.
    fn lq(status: Option<&str>, stage: Option<&str>, tag: Option<&str>) -> ListQuery {
        let one = |v: Option<&str>| v.map(String::from).into_iter().collect();
        ListQuery {
            status: status.map(String::from),
            filter: search::Filter {
                stages: one(stage),
                tags: one(tag),
                ..Default::default()
            },
            sort: None,
        }
    }

    #[test]
    fn parses_flags_and_positionals() {
        let a = parse(&sv(&["new", "A title", "--type", "bug", "--json"])).unwrap();
        assert_eq!(a.pos, vec!["new", "A title"]);
        assert_eq!(a.flags.get("type").map(String::as_str), Some("bug"));
        assert!(a.json);
        assert!(parse(&sv(&["new", "--type"]))
            .unwrap_err()
            .contains("needs a value"));
        assert!(parse(&sv(&["new", "--bogus"]))
            .unwrap_err()
            .contains("unknown option"));
    }

    /// `muster block <id> --on <kind> "<what>"` (0024): each kind, the old owner and capture, task
    /// ids into depends, and the refusals with their exit codes (1 refused, 2 usage).
    #[test]
    fn block_takes_a_kind_and_what_it_waits_on() {
        let d = crate::testkit::tempdir().unwrap();
        for f in queue::STATUS_FOLDERS {
            std::fs::create_dir(d.path().join(f)).unwrap();
        }
        std::fs::write(d.path().join("README.md"), "---\nprefix: tm\n---\nq").unwrap();
        let q = d.path().to_string_lossy().to_string();
        let code = |args: &[&str]| {
            let mut v = sv(args);
            v.extend(sv(&["--queue", &q, "--as", "wt-a"]));
            main(&v)
        };
        for title in ["One", "Two", "Three", "Four"] {
            assert_eq!(code(&["new", title]), 0);
        }
        assert_eq!(
            code(&["ask", "0001", "Which port?", "--session", "aaaa1111"]),
            0
        );
        let t = |id: &str| {
            ops::tasks_now(d.path())
                .into_iter()
                .find(|t| t.id == id)
                .unwrap()
        };
        let why = |id: &str| crate::block::sentence_of(&t(id), false);
        // Usage: no --on.
        assert_eq!(code(&["block", "0001"]), 2);
        // Refused: an unknown kind, a missing detail, a question that isn't there or is closed.
        assert_eq!(code(&["block", "0001", "--on", "someday", "x"]), 1);
        assert_eq!(code(&["block", "0001", "--on", "you"]), 1);
        assert_eq!(code(&["block", "0001", "--on", "outside", " "]), 1);
        assert_eq!(code(&["block", "0001", "--on", "question", "Q5"]), 1);
        assert_eq!(
            code(&["block", "0001", "--on", "question", "which port"]),
            1
        );
        assert_eq!(code(&["block", "0001", "--on", "0099"]), 1);
        assert!(!t("0001").blocked);
        // Each kind, the detail after --on's value or in it.
        assert_eq!(
            code(&["block", "0001", "--on", "you", "set the API key"]),
            0
        );
        assert_eq!(why("0001"), "Your action: set the API key");
        assert_eq!(
            code(&[
                "block",
                "0001",
                "--on",
                "permission",
                "write to ~/x/tasks (sandbox)"
            ]),
            0
        );
        assert_eq!(why("0001"), "Permission: write to ~/x/tasks (sandbox)");
        assert_eq!(code(&["block", "0001", "--on", "question", "Q1"]), 0);
        assert_eq!(why("0001"), "Question: Q1 Which port?, waiting on capture");
        assert_eq!(code(&["block", "0001", "--on", "outside: Apple review"]), 0);
        assert_eq!(why("0001"), "Outside: Apple review");
        // The old words still work, as you and question.
        assert_eq!(code(&["block", "0001", "--on", "owner: which colour?"]), 0);
        assert_eq!(why("0001"), "Your action: which colour?");
        assert_eq!(code(&["block", "0001", "--on", "capture", "Q1"]), 0);
        assert_eq!(why("0001"), "Question: Q1 Which port?, waiting on capture");
        assert_eq!(
            code(&["block", "0001", "--on", "capture", "Q1"]),
            1,
            "the same block"
        );
        // Task ids go into depends, with no flag.
        assert_eq!(code(&["block", "0003", "--on", "0001,0002"]), 0);
        let three = t("0003");
        assert_eq!((three.blocked, three.depends.len()), (false, 2));
        assert_eq!(why("0003"), "After 0001 (Ready), 0002 (Ready)");
        assert_eq!(
            code(&["block", "0003", "--on", "0001"]),
            1,
            "already waits on it"
        );
        assert_eq!(code(&["unblock", "0003"]), 1, "it clears by itself");
        assert_eq!(code(&["unblock", "0001"]), 0);
        // A reason that starts with a digit stays the reason (mu-101 R4).
        assert_eq!(code(&["block", "0002", "--on", "0001", "3pm call"]), 0);
        assert_eq!(t("0002").depends, ["0001"]);
        let two = queue::read_body(d.path(), "0002").unwrap();
        assert!(
            two.contains("wt-a: blocked on tasks: tm-1 (3pm call)\n"),
            "{two}"
        );
        // The same as one --on value, through the key rewrite (mu-101 R15).
        assert_eq!(code(&["block", "0004", "--on", "tm-1 4pm call"]), 0);
        assert_eq!(t("0004").depends, ["0001"]);
        let four = queue::read_body(d.path(), "0004").unwrap();
        assert!(
            four.contains("wt-a: blocked on tasks: tm-1 (4pm call)\n"),
            "{four}"
        );
        // A clock time isn't a task (R11): refused, not a block on 0009.
        assert_eq!(code(&["block", "0001", "--on", "9:30", "standup"]), 1);
        assert_eq!(code(&["block", "0001", "--on", "9:30 standup"]), 1);
        assert!(t("0001").depends.is_empty() && !t("0001").blocked);
        // The help says it all.
        let help = crate::help::command_text("block").unwrap();
        for k in [
            "you",
            "permission",
            "question",
            "outside",
            "0014,0016",
            "owner",
            "capture",
        ] {
            assert!(help.contains(k), "{k}: {help}");
        }
    }

    #[test]
    fn runs_against_a_queue_folder() {
        let d = crate::testkit::tempdir().unwrap();
        for f in queue::STATUS_FOLDERS {
            std::fs::create_dir(d.path().join(f)).unwrap();
        }
        std::fs::write(d.path().join("README.md"), "q").unwrap();
        let q = d.path().to_string_lossy().to_string();
        let ok = |args: &[&str]| {
            let mut v = sv(args);
            v.extend(sv(&["--queue", &q, "--as", "tester"]));
            run(&v).unwrap()
        };
        assert_eq!(
            ok(&["new", "From the CLI", "--type", "bug", "--priority", "high"]),
            0
        );
        assert_eq!(ok(&["note", "0001", "first", "note"]), 0);
        assert_eq!(
            ok(&["new", "Two lines", "--body", "line one\\nline two"]),
            0
        );
        assert!(queue::read_body(d.path(), "0002")
            .unwrap()
            .contains("line one\nline two"));
        assert_eq!(ok(&["set", "0001", "priority", "low"]), 0);
        assert_eq!(
            ok(&["move", "0001", "blocked", "--on", "owner: pick a colour"]),
            0
        );
        assert_eq!(ok(&["list"]), 0);
        assert_eq!(ok(&["check"]), 0);
        assert_eq!(
            ok(&[
                "new",
                "Estimated",
                "--time",
                "90 min",
                "--tokens",
                "250k",
                "--tags",
                "spike"
            ]),
            0
        );
        let est = tasks_of(d.path())
            .tasks
            .into_iter()
            .find(|t| t.title == "Estimated")
            .unwrap();
        assert_eq!(
            (
                est.est_time.as_str(),
                est.est_tokens.as_str(),
                est.tags.clone()
            ),
            ("90m", "250k", vec!["spike".to_string()])
        );
        let t = &tasks_of(d.path()).tasks[0];
        assert_eq!(
            (t.status.as_str(), t.priority.as_str(), t.kind.as_str()),
            ("blocked", "low", "bug")
        );
        assert!(t.log_tail.iter().any(|l| l.contains("tester: first note")));
        assert!(run(&sv(&["frobnicate", "--queue", &q]))
            .unwrap_err()
            .contains("unknown command"));
        assert!(run(&sv(&["move", "0001", "doing", "--queue", &q]))
            .unwrap_err()
            .contains("claim"));
    }

    #[test]
    fn types_and_test_time_on_the_command_line() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        let code = |args: &[&str]| {
            let mut v = sv(args);
            v.extend(sv(&["--queue", &q, "--as", "owner"]));
            main(&v)
        };
        let t = |id: &str| queue::read_task(d.path(), id).unwrap();
        assert_eq!(
            code(&[
                "new",
                "Typed",
                "--type",
                "tech-debt",
                "--time",
                "2h",
                "--test-time",
                "30m"
            ]),
            0
        );
        assert_eq!(
            (
                t("0001").kind,
                t("0001").est_test_time,
                t("0001").est_test_minutes
            ),
            ("tech-debt".to_string(), "30m".to_string(), 30)
        );
        assert_eq!(code(&["new", "Old name", "--type", "task"]), 0);
        assert_eq!(t("0002").kind, "code");
        // Refused (1): idea is the backlog's now. --test-time takes a value.
        assert_eq!(code(&["new", "An idea", "--type", "idea"]), 1);
        assert!(parse(&sv(&["new", "x", "--test-time"]))
            .unwrap_err()
            .contains("needs a value"));
        assert_eq!(code(&["set", "0001", "test-time", "1h"]), 0);
        assert_eq!(t("0001").est_test_minutes, 60);
        assert_eq!(code(&["set", "0001", "type", "idea"]), 1);
        assert_eq!(code(&["set", "0001", "type", "spike"]), 0);
        assert_eq!(t("0001").kind, "spike");
        assert!(queue::read_task(d.path(), "0003").is_err(), "nothing made");
    }

    #[test]
    fn tick_and_untick_the_owners_criteria() {
        let d = crate::testkit::tempdir().unwrap();
        for f in queue::STATUS_FOLDERS {
            std::fs::create_dir(d.path().join(f)).unwrap();
        }
        std::fs::write(d.path().join("README.md"), "q").unwrap();
        let q = d.path().to_string_lossy().to_string();
        let code = |args: &[&str]| {
            let mut v = sv(args);
            v.extend(sv(&["--queue", &q, "--as", "owner"]));
            main(&v)
        };
        assert_eq!(code(&["new", "Checked"]), 0);
        std::fs::write(
            d.path().join("todo/0001-checked.md"),
            queue::read_body(d.path(), "0001").unwrap().replace(
                "## Log",
                "## Acceptance criteria\n- [x] AC1: works — verify: command\n- [ ] AC2: tried it — verify: owner\n\n## Log",
            ),
        )
        .unwrap();
        let ticked = || queue::read_task(d.path(), "0001").unwrap().owner_criteria[0].ticked;
        assert_eq!(code(&["tick", "0001", "AC2"]), 0);
        assert!(ticked());
        // Refused (1): already ticked, not the owner's; usage (2): which one is missing.
        assert_eq!(code(&["tick", "0001", "AC2"]), 1);
        assert_eq!(code(&["tick", "0001", "AC1"]), 1);
        assert_eq!(code(&["tick", "0001"]), 2);
        assert_eq!(code(&["untick", "0001", "2"]), 0);
        assert!(!ticked());
        // check lists what waits on the owner without calling it a problem.
        assert_eq!(code(&["check"]), 0);
    }

    #[test]
    fn summary_from_the_command_line_with_pass_and_set() {
        let d = crate::testkit::tempdir().unwrap();
        for f in queue::STATUS_FOLDERS {
            std::fs::create_dir(d.path().join(f)).unwrap();
        }
        std::fs::write(d.path().join("README.md"), "q").unwrap();
        let q = d.path().to_string_lossy().to_string();
        let code_as = |who: &str, args: &[&str]| {
            let mut v = sv(args);
            v.extend(sv(&["--queue", &q, "--as", who]));
            main(&v)
        };
        let code = |args: &[&str]| code_as("wt-a", args);
        let task = || queue::read_task(d.path(), "0001").unwrap();
        assert_eq!(code(&["new", "Summed"]), 0);
        std::fs::write(
            d.path().join("todo/0001-summed.md"),
            queue::read_body(d.path(), "0001").unwrap().replace(
                "## Log",
                "## Acceptance criteria\n- [x] AC1: works — verify: command — proof: it does\n\n## Log",
            ),
        )
        .unwrap();
        assert_eq!(code(&["claim", "0001"]), 0);
        let gate = [
            "pass",
            "0001",
            "--from",
            "building",
            "--gate",
            "gate: green",
        ];
        // Refused (1) without one, and with one over 160 characters; --summary needs its value.
        assert_eq!(code(&gate), 1);
        let long = "a".repeat(161);
        assert_eq!(code(&[&gate[..], &["--summary", &long]].concat()), 1);
        assert!(parse(&sv(&["pass", "0001", "--summary"])).is_err());
        assert_eq!(task().stage, "building");
        assert_eq!(
            code(
                &[
                    &gate[..],
                    &["--summary", "A chat you step through; works offline"]
                ]
                .concat()
            ),
            0
        );
        let t = task();
        assert_eq!(t.stage, "testing");
        assert_eq!(t.summary, "A chat you step through; works offline");
        // set fixes it, in any stage, with the same rules; the newline stays a newline.
        assert_eq!(code(&["set", "0001", "summary", "A", "clearer", "line"]), 0);
        assert_eq!(task().summary, "A clearer line");
        assert_eq!(code(&["set", "0001", "summary", "two\nlines"]), 1);
        assert_eq!(code(&["set", "0001", "summary", &long]), 1);
        assert_eq!(task().summary, "A clearer line");
    }

    #[test]
    fn stage_commands_and_their_exit_codes() {
        let d = crate::testkit::tempdir().unwrap();
        for f in queue::STATUS_FOLDERS {
            std::fs::create_dir(d.path().join(f)).unwrap();
        }
        std::fs::write(d.path().join("README.md"), "q").unwrap();
        let q = d.path().to_string_lossy().to_string();
        let code_as = |who: &str, args: &[&str]| {
            let mut v = sv(args);
            v.extend(sv(&["--queue", &q, "--as", who]));
            main(&v)
        };
        let code = |args: &[&str]| code_as("wt-a", args);
        let task = || queue::read_task(d.path(), "0001").unwrap();
        assert_eq!(code(&["new", "Staged"]), 0);
        std::fs::write(
            d.path().join("todo/0001-staged.md"),
            queue::read_body(d.path(), "0001").unwrap().replace(
                "## Log",
                "## Acceptance criteria\n- [x] AC1: works\n\n## Log",
            ),
        )
        .unwrap();
        assert_eq!(code(&["pass", "0001"]), 0);
        assert_eq!(
            (task().stage, task().claimed_by),
            ("building".to_string(), "wt-a".to_string())
        );
        // Refused (1): no gate line; usage (2): no id, block without --on.
        assert_eq!(code(&["pass", "0001"]), 1);
        assert_eq!(code(&["pass"]), 2);
        assert_eq!(code(&["block", "0001"]), 2);
        assert_eq!(code(&["return", "0001"]), 2);
        assert_eq!(
            code(&["pass", "0001", "--from", "ready", "--gate", "x"]),
            1,
            "moved on since"
        );
        assert_eq!(
            code(&[
                "pass",
                "0001",
                "--from",
                "building",
                "--gate",
                "gate full: green",
                "--summary",
                "A synthetic summary"
            ]),
            0
        );
        assert_eq!(task().stage, "testing");
        assert_eq!(code(&["block", "0001", "--on", "someday"]), 1);
        assert_eq!(code(&["block", "0001", "--on", "owner: which colour?"]), 0);
        let t = task();
        assert_eq!(
            (t.stage.as_str(), t.status.as_str()),
            ("testing", "blocked")
        );
        assert_eq!(code(&["list"]), 0);
        assert_eq!(code(&["list", "--stage", "testing", "--json"]), 0);
        assert_eq!(code(&["list", "--stage", "qa"]), 1);
        assert_eq!(code(&["pass", "0001"]), 1, "blocked");
        assert_eq!(code(&["unblock", "0001"]), 0);
        assert_eq!(code(&["unblock", "0001"]), 1, "not blocked");
        assert_eq!(
            code(&["return", "0001", "AC1", "fails", "--to", "done"]),
            1,
            "not earlier"
        );
        assert_eq!(code(&["return", "0001", "AC1", "fails"]), 0);
        let t = task();
        assert_eq!(
            (t.stage.as_str(), t.claimed_by.as_str()),
            ("building", "wt-a")
        );
        assert!(queue::read_body(d.path(), "0001")
            .unwrap()
            .contains("wt-a: stage testing → building (returned: AC1 fails)"));
        assert_eq!(
            code(&["stage", "0001", "your-check", "--from", "testing"]),
            1,
            "moved on since"
        );
        assert_eq!(task().stage, "building");
        // The builder can't skip its tester with the override (0047); the owner can, and the log
        // says so.
        assert_eq!(
            code(&["stage", "0001", "your-check", "--from", "building"]),
            1,
            "the builder"
        );
        assert_eq!(task().stage, "building");
        assert_eq!(
            code_as(
                "owner",
                &["stage", "0001", "your-check", "--from", "building"]
            ),
            0
        );
        assert!(queue::read_body(d.path(), "0001").unwrap().contains(
            ": stage building → your-check (set; owner's override: no separate tester)\n"
        ));
        assert!(task().waiting_for_owner);
        assert_eq!(code(&["stage", "0001", "testing"]), 0);
        assert_eq!(code(&["stage", "0001", "your-check"]), 1, "the builder");
        assert_eq!(code_as("owner", &["stage", "0001", "your-check"]), 0);
        assert_eq!(code(&["stage", "0001", "nowhere"]), 1);
        assert_eq!(code(&["stage", "0001"]), 2);
        assert_eq!(code(&["pass", "0001", "--commit", "abc1234"]), 0);
        let t = task();
        assert_eq!(
            (t.stage.as_str(), t.commit.as_str(), t.path.as_str()),
            ("done", "abc1234", "done/0001-staged.md")
        );
        assert_eq!(code(&["pass", "0001"]), 1, "nothing after Done");
        assert_eq!(code(&["show", "0001", "--json"]), 0);
        assert_eq!(code(&["check"]), 0);
    }

    #[test]
    fn list_groups_by_stage_and_filters() {
        let d = crate::testkit::tempdir().unwrap();
        for f in queue::STATUS_FOLDERS {
            std::fs::create_dir(d.path().join(f)).unwrap();
        }
        std::fs::write(d.path().join("README.md"), "q").unwrap();
        let file = |rel: &str, id: &str, fm: &str, title: &str| {
            std::fs::write(
                d.path().join(rel),
                format!("---\nid: {id}\ntype: task\npriority: normal\n{fm}\n---\n\n# {title}\n\n## Log\n- 2026-10-12 09:00 capture: created\n"),
            )
            .unwrap();
        };
        file(
            "done/0001-a.md",
            "0001",
            "status: done\nstage: done",
            "Shipped",
        );
        file(
            "blocked/0002-b.md",
            "0002",
            "status: blocked\nstage: testing\nwaiting_on: 0012 the API\ntags: ui",
            "Under test",
        );
        file(
            "todo/0003-c.md",
            "0003",
            "status: todo\nstage: shaping\nest-time: 2h\nest-test-time: 30m\nest-tokens: 400k",
            "Shaping it",
        );
        file("inbox/0004-d.md", "0004", "status: inbox", "Raw");
        let s = tasks_of(d.path());
        let none: Vec<&crate::backlog::Note> = Vec::new();
        let text = list_text(&none, &list_filter(&s.tasks, &lq(None, None, None)));
        assert!(s.backlog.is_empty());
        assert_eq!(
            text,
            "Backlog (1)\n  0004 code      normal  Raw\nShaping (1)\n  0003 code      normal  Shaping it  ~agent 2h · test 30m · 400k\nTesting (1)\n  0002 code      normal  Under test [After 0012]  #ui\nDone (1)\n  0001 code      normal  Shipped\n"
        );
        // Backlog notes come first, by file name and title, and only where Backlog is listed.
        for (name, text, secs) in [
            ("inbox/2026-10-12-0900-an-idea.md", "# An idea\n", 2000),
            ("inbox/blank.md", "", 1000),
        ] {
            let p = d.path().join(name);
            std::fs::write(&p, text).unwrap();
            std::fs::File::options()
                .write(true)
                .open(&p)
                .unwrap()
                .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs))
                .unwrap();
        }
        let s = tasks_of(d.path());
        let notes = |stage: Option<&str>, status: Option<&str>, tag: Option<&str>| {
            list_notes(&s.backlog, &lq(status, stage, tag))
                .iter()
                .map(|n| n.file.clone())
                .collect::<Vec<_>>()
        };
        let all = list_notes(&s.backlog, &lq(None, None, None));
        assert!(list_text(&all, &list_filter(&s.tasks, &lq(None, None, None))).starts_with(
            "Backlog notes (2)\n  2026-10-12-0900-an-idea.md  An idea\n  blank.md  (empty note)\nBacklog (1)\n"
        ));
        assert_eq!(notes(Some("backlog"), None, None).len(), 2);
        assert!(notes(Some("ready"), None, None).is_empty());
        assert!(notes(None, Some("inbox"), None).is_empty());
        assert!(notes(None, None, Some("ui")).is_empty());
        let json = list_json(&all, &list_filter(&s.tasks, &lq(None, None, None)));
        assert_eq!(json["backlog"].as_array().unwrap().len(), 2);
        assert_eq!(json["tasks"].as_array().unwrap().len(), 4);
        let ids = |v: Vec<&queue::Task>| v.iter().map(|t| t.id.clone()).collect::<Vec<_>>();
        assert_eq!(
            ids(list_filter(&s.tasks, &lq(None, Some("testing"), None))),
            ["0002"]
        );
        assert_eq!(
            ids(list_filter(&s.tasks, &lq(Some("todo"), None, None))),
            ["0003"]
        );
        assert_eq!(
            ids(list_filter(&s.tasks, &lq(None, None, Some("ui")))),
            ["0002"]
        );
        assert!(list_filter(&s.tasks, &lq(None, Some("ready"), None)).is_empty());
        // After a change: the stage, and what it waits on; a failed re-read still says it changed.
        assert_eq!(
            stage_message(d.path(), "", "0002", "blocked/0002-b.md"),
            "0002: Testing, After 0012 (blocked/0002-b.md)"
        );
        // By keys (0033), the tasks it waits on too.
        assert_eq!(
            stage_message(d.path(), "mu", "0002", "blocked/0002-b.md"),
            "mu-2: Testing, After mu-12 (blocked/0002-b.md)"
        );
        assert_eq!(
            stage_message(d.path(), "mu", "0009", "doing/0009-x.md"),
            "mu-9: changed (doing/0009-x.md)"
        );
    }

    #[test]
    fn list_searches_filters_and_sorts_like_the_app() {
        let d = crate::testkit::tempdir().unwrap();
        for f in queue::STATUS_FOLDERS {
            std::fs::create_dir(d.path().join(f)).unwrap();
        }
        std::fs::write(d.path().join("README.md"), "q").unwrap();
        let file = |rel: &str, fm: &str, body: &str| {
            std::fs::write(
                d.path().join(rel),
                format!("---\n{fm}\n---\n\n{body}\n## Log\n- 2026-10-19 09:00 capture: created\n"),
            )
            .unwrap();
        };
        file(
            "doing/0019-a.md",
            "id: 0019\ntype: bug\npriority: low\nstatus: doing\nstage: building\ncaptured: 2026-10-01\nclaimed-by: drover/main (session 1)\ntags: ui, design",
            "# Board flickers on résumé\n\nThe columns jump.\n",
        );
        file(
            "blocked/0020-b.md",
            "id: 0020\ntype: task\npriority: high\nstatus: blocked\nstage: ready\ncaptured: 2026-10-03\nwaiting_on: 0019\ntags: docs",
            "# Write the guide\n\nAfter the board settles.\n",
        );
        file(
            "todo/0119-c.md",
            "id: 0119\ntype: idea\nstatus: todo\nstage: shaping\ncaptured: 2026-10-02",
            "# Export CSV\n",
        );
        std::fs::write(d.path().join("inbox/an-idea.md"), "# Board idea\n").unwrap();
        let s = tasks_of(d.path());
        let list = |args: &[&str]| -> Result<(Vec<String>, Vec<String>), String> {
            let a = parse(&sv(&[&["list"], args].concat()))?;
            let q = list_query(&a)?;
            let ids = list_filter(&s.tasks, &q)
                .iter()
                .map(|t| t.id.clone())
                .collect();
            let notes = list_notes(&s.backlog, &q)
                .iter()
                .map(|n| n.file.clone())
                .collect();
            Ok((ids, notes))
        };
        let ids = |args: &[&str]| list(args).unwrap().0;
        assert_eq!(ids(&[]), ["0019", "0020", "0119"]);
        assert_eq!(ids(&["--search", "BOARD"]), ["0019", "0020"]);
        assert_eq!(ids(&["--search", "jump resume"]), ["0019"]);
        assert_eq!(ids(&["--search", "19"]), ["0019"]);
        assert_eq!(ids(&["--search", "0119"]), ["0119"]);
        assert_eq!(ids(&["--search", "drover"]), ["0019"]);
        assert_eq!(ids(&["--stage", "ready,Building"]), ["0019", "0020"]);
        assert_eq!(ids(&["--type", "bug,idea"]), ["0019", "0119"]);
        // `type: task` reads as code (0017), and `--type task` means code too.
        assert_eq!(ids(&["--type", "code"]), ["0020"]);
        assert_eq!(ids(&["--type", "Task"]), ["0020"]);
        assert_eq!(ids(&["--tag", "docs,design"]), ["0019", "0020"]);
        assert_eq!(ids(&["--holder", "drover/main"]), ["0019"]);
        assert_eq!(ids(&["--blocked"]), ["0020"]);
        assert_eq!(ids(&["--blocked", "--search", "board"]), ["0020"]);
        assert_eq!(ids(&["--sort", "priority"]), ["0020", "0119", "0019"]);
        assert_eq!(ids(&["--sort", "newest"]), ["0020", "0119", "0019"]);
        assert_eq!(
            ids(&["--sort", "oldest", "--search", "board"]),
            ["0019", "0020"]
        );
        // Notes: matched by the search, hidden by a filter only tasks have.
        assert_eq!(list(&["--search", "idea"]).unwrap().1, ["an-idea.md"]);
        assert!(list(&["--search", "csv"]).unwrap().1.is_empty());
        assert!(list(&["--blocked"]).unwrap().1.is_empty());
        assert!(list(&["--holder", "x"]).unwrap().1.is_empty());
        assert_eq!(list(&["--stage", "backlog"]).unwrap().1, ["an-idea.md"]);
        // Wrong values are refused with what's allowed.
        let err = |args: &[&str]| list(args).unwrap_err();
        assert!(err(&["--sort", "sideways"])
            .contains("priority, newest, oldest, changed, estimate, id"));
        assert!(err(&["--stage", "ready,qa"]).contains("stage must be one of"));
        assert!(err(&["--search"]).contains("needs a value"));
        // JSON leaves the search text out: agents read it, and it's only for matching.
        let json = list_json(
            &[],
            &list_filter(
                &s.tasks,
                &list_query(&parse(&sv(&["list"])).unwrap()).unwrap(),
            ),
        );
        assert_eq!(json["tasks"].as_array().unwrap().len(), 3);
        assert!(json["tasks"][0].get("search").is_none());
        assert!(json["tasks"][0].get("title").is_some());
        assert!(task_json(&s.tasks[0]).get("search").is_none());
        let q = d.path().to_string_lossy().to_string();
        let code = |args: &[&str]| run(&sv(&[args, &["--queue", &q]].concat())).unwrap_or(9);
        assert_eq!(
            code(&["list", "--search", "board", "--sort", "id", "--json"]),
            0
        );
        assert_eq!(code(&["list", "--sort", "nope"]), 9);
    }

    #[test]
    fn everything_after_double_dash_is_text() {
        let a = parse(&sv(&[
            "ask",
            "--session",
            "aaaa1111",
            "--keep",
            "--",
            "0007",
            "--from",
            "--weird?",
        ]))
        .unwrap();
        assert_eq!(a.pos, vec!["ask", "0007", "--from", "--weird?"]);
        assert_eq!(a.flags.get("session").map(String::as_str), Some("aaaa1111"));
        assert!(a.keep);
        let a = parse(&sv(&["reclaim", "--after", "2h"])).unwrap();
        assert_eq!(a.flags.get("after").map(String::as_str), Some("2h"));
        assert!(question_number("Q3").is_ok() && question_number("3").is_ok());
        for bad in ["seven", "Q", "QQ2", "Q-1", "Q3a", ""] {
            assert!(
                question_number(bad).unwrap_err().contains("look like Q3"),
                "{bad:?}"
            );
        }
    }

    fn v2_queue() -> crate::testkit::TestDir {
        let d = crate::testkit::tempdir().unwrap();
        for f in queue::STATUS_FOLDERS {
            std::fs::create_dir(d.path().join(f)).unwrap();
        }
        std::fs::write(d.path().join("README.md"), "q").unwrap();
        d
    }

    #[test]
    fn claim_next_heartbeat_and_reclaim_with_their_exit_codes() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        let code = |who: &str, args: &[&str]| {
            let mut v = sv(args);
            v.extend(sv(&["--queue", &q, "--as", who]));
            main(&v)
        };
        for title in ["One", "Two", "Three"] {
            assert_eq!(code("owner", &["new", title]), 0);
        }
        assert_eq!(code("wt-a", &["claim", "0002"]), 0);
        assert_eq!(code("wt-a", &["claim", "0001"]), 1, "one claim per holder");
        assert_eq!(code("wt-a", &["next"]), 1, "one claim per holder");
        assert_eq!(code("wt-a", &["claim"]), 2, "usage");
        assert_eq!(code("wt-b", &["next"]), 0);
        assert_eq!(code("wt-c", &["next"]), 0);
        assert_eq!(code("wt-d", &["next"]), 1, "nothing ready");
        let t = |id: &str| queue::read_task(d.path(), id).unwrap();
        assert_eq!(
            (
                t("0001").claimed_by,
                t("0002").claimed_by,
                t("0003").claimed_by
            ),
            ("wt-b".into(), "wt-a".into(), "wt-c".into())
        );
        assert_eq!(code("wt-a", &["heartbeat", "0002"]), 0);
        assert_eq!(code("wt-b", &["heartbeat", "0002"]), 1, "not the holder");
        assert_eq!(code("wt-a", &["heartbeat"]), 2);
        // Every claim was just made: nothing to release, and a lease that isn't a time is refused.
        assert_eq!(code("capture", &["reclaim"]), 0);
        assert_eq!(code("capture", &["reclaim", "--after", "30m", "--json"]), 0);
        assert_eq!(code("capture", &["reclaim", "--after", "soon"]), 1);
        assert_eq!(t("0002").stage, "building");
        assert_eq!(code("wt-a", &["show", "0002", "--json"]), 0);
    }

    #[test]
    fn an_owners_at_comment_acts_and_an_agents_stays_a_comment() {
        // mu-101: `muster comment … --as owner` runs the @verb; anyone else's is a plain comment.
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        let args = |who: &str, a: &[&str]| {
            let mut v = sv(a);
            v.extend(sv(&["--queue", &q, "--as", who]));
            v
        };
        let code = |who: &str, a: &[&str]| main(&args(who, a));
        std::fs::write(
            d.path().join("todo/0001-t.md"),
            "---\nid: 0001\ntype: code\npriority: normal\nstage: ready\nstatus: todo\nclaimed-by:\nheartbeat:\nwaiting_on:\n---\n\n# One\n\n## Acceptance criteria\n- [x] AC1: one — verify: command\n\n## Log\n- 2026-10-14 08:00 capture: created\n",
        )
        .unwrap();
        let t = || queue::read_task(d.path(), "0001").unwrap();
        // Started on its own branch, built, and merged into Your check by the real path (the git
        // side left out): the merge's pass clears branch:.
        let at = "2026-10-14 09:00";
        ops::start_task(
            d.path(),
            "0001",
            "wt-a",
            at,
            "/nowhere/wt",
            "task/tm-1-one",
            false,
        )
        .unwrap();
        let built = [
            "pass",
            "0001",
            "--from",
            "building",
            "--gate",
            "green",
            "--summary",
            "Built it",
        ];
        assert_eq!(code("wt-a", &built), 0);
        ops::pass_merged(
            d.path(),
            "0001",
            "qa",
            at,
            &"d".repeat(40),
            "merged into main",
        )
        .unwrap();
        assert_eq!(
            (t().stage, t().branch),
            ("your-check".into(), String::new())
        );
        assert_eq!(code("wt-b", &["comment", "0001", "@sendback the icon"]), 0);
        assert_eq!((t().stage, t().comments.len()), ("your-check".into(), 1));
        assert_eq!(code("owner", &["comment", "0001", "@sendbak the icon"]), 1);
        assert_eq!(t().comments.len(), 1, "an unknown verb saves nothing");
        assert_eq!(
            code(
                "owner",
                &["comment", "0001", "@sendback", "the icon is bad"]
            ),
            0
        );
        let after = t();
        assert_eq!(
            (
                after.stage.as_str(),
                after.sent_back,
                after.sent_back_why.as_str()
            ),
            ("building", true, "the icon is bad")
        );
        assert_eq!(
            after.comments[1].text,
            "@sendback the icon is bad\n→ sent back to Building (owner finding R1)"
        );
        // Answered, but with no branch no round can re-check it: the pass waits for a start.
        assert_eq!(
            code(
                "wt-a",
                &["review", "0001", "--fixed", "R1", "a chevron now"]
            ),
            0
        );
        let fixed = [
            "pass",
            "0001",
            "--from",
            "building",
            "--gate",
            "green",
            "--summary",
            "Fixed it",
        ];
        assert_eq!(code("wt-a", &fixed), 1);
        assert_eq!(t().stage, "building");
        // Started again (start's write once git made the branch), gated, reviewed (the round
        // settles R1) and merged into Your check again: `return --to building` is the same
        // send-back, its finding numbered on.
        ops::restart_task(
            d.path(),
            "0001",
            "wt-a",
            at,
            "/nowhere/wt2",
            "task/tm-1-one",
            false,
        )
        .unwrap();
        assert_eq!(code("wt-a", &fixed), 0);
        let head = "5".repeat(40);
        let span = crate::review::Span {
            base: "1".repeat(40),
            head: head.clone(),
            since: None,
        };
        let settles = format!(
            r#"{{"head":"{head}","tier":"light","answers":[{{"finding":"R1","accept":true,"why":"a chevron"}}]}}"#
        );
        assert_eq!(
            crate::review::record(d.path(), "0001", &settles, "qa", at, &span)
                .unwrap()
                .then,
            "clean"
        );
        ops::pass_merged(
            d.path(),
            "0001",
            "qa",
            at,
            &"e".repeat(40),
            "merged into main",
        )
        .unwrap();
        assert_eq!(t().stage, "your-check");
        assert_eq!(
            code(
                "owner",
                &["return", "0001", "still", "bad", "--to", "building"]
            ),
            0
        );
        let r = crate::review::load(d.path(), "0001").unwrap().unwrap();
        assert_eq!(
            r.findings
                .iter()
                .map(|f| (f.id.as_str(), f.lens.as_str()))
                .collect::<Vec<_>>(),
            [("R1", "owner"), ("R2", "owner")]
        );
    }

    #[test]
    fn return_says_whether_the_builder_was_told_with_or_without_a_finding() {
        // R14: the told / not-told line doesn't hang on a finding.
        let r = |finding: &str, told: &str, not_told: &str| ops::Returned {
            finding: finding.into(),
            told: told.into(),
            not_told: not_told.into(),
            ..Default::default()
        };
        assert_eq!(
            returned_lines("mu-1", &r("R2", "7b5e8856", "")),
            [
                "mu-1: owner finding R2 for the builder to fix or dispute; first in line",
                "mu-1: told session 7b5e8856"
            ]
        );
        assert_eq!(
            returned_lines("mu-1", &r("", "", "no session")),
            ["mu-1: the builder's session wasn't told (no session); muster next shows it"]
        );
        assert_eq!(
            returned_lines("mu-1", &r("", "7b5e8856", "")),
            ["mu-1: told session 7b5e8856"]
        );
        assert!(returned_lines("mu-1", &r("", "", "")).is_empty());
    }

    #[test]
    fn a_return_from_testing_goes_through_while_the_builder_holds_the_next_task() {
        // mu-66: the send-back comes first; `next` says so, and a third claim is still refused.
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        let args = |who: &str, a: &[&str]| {
            let mut v = sv(a);
            v.extend(sv(&["--queue", &q, "--as", who]));
            v
        };
        let code = |who: &str, a: &[&str]| main(&args(who, a));
        for title in ["One", "Two", "Three"] {
            assert_eq!(code("owner", &["new", title]), 0);
        }
        assert_eq!(code("wt-a", &["claim", "0001"]), 0);
        assert_eq!(
            code(
                "wt-a",
                &[
                    "pass",
                    "0001",
                    "--from",
                    "building",
                    "--gate",
                    "green",
                    "--summary",
                    "Built one"
                ]
            ),
            0
        );
        assert_eq!(code("wt-a", &["claim", "0002"]), 0);
        assert_eq!(
            code("qa", &["return", "0001", "AC1 fails", "--to", "building"]),
            0
        );
        let t = |id: &str| queue::read_task(d.path(), id).unwrap();
        let prefix = crate::key::prefix_for(d.path());
        let (k1, k2) = (
            crate::key::key(&prefix, "0001"),
            crate::key::key(&prefix, "0002"),
        );
        assert_eq!(
            (t("0001").stage, t("0002").stage),
            ("building".into(), "building".into())
        );
        assert_eq!(
            (t("0001").claimed_by, t("0002").claimed_by),
            ("wt-a".into(), "wt-a".into())
        );
        assert_eq!(
            run(&args("wt-a", &["next"])).unwrap_err(),
            format!("fix {k1} first (sent back by the review); you also hold {k2}, which waits behind it")
        );
        assert_eq!(
            code("wt-a", &["claim", "0003"]),
            1,
            "a third claim is still refused"
        );
        assert_eq!(t("0003").stage, "ready");
    }

    #[test]
    fn human_tasks_on_the_command_line() {
        // 0022: new --human (the owner's only), set for, and an agent's claim refused.
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        let code = |who: &str, args: &[&str]| {
            let mut v = sv(args);
            v.extend(sv(&["--queue", &q, "--as", who]));
            main(&v)
        };
        let t = |id: &str| queue::read_task(d.path(), id).unwrap();
        assert!(parse(&sv(&["new", "x", "--human"])).unwrap().human);
        assert!(!parse(&sv(&["new", "x"])).unwrap().human);
        assert_eq!(code("owner", &["new", "Buy the domain", "--human"]), 0);
        assert!(t("0001").is_human());
        assert_eq!(
            code("wt-a", &["new", "Set the secret", "--human"]),
            1,
            "agents propose in the backlog"
        );
        assert_eq!(code("owner", &["new", "Agent work"]), 0);
        assert_eq!(t("0002").for_who, "ai");
        assert_eq!(code("wt-a", &["claim", "0001"]), 1, "the owner's to do");
        assert_eq!(code("wt-a", &["next"]), 0);
        assert_eq!(t("0002").claimed_by, "wt-a");
        assert_eq!(code("wt-b", &["next"]), 1, "nothing an agent may take");
        assert_eq!(code("owner", &["set", "0001", "for", "robot"]), 1);
        assert_eq!(code("owner", &["new", "Try it on the phone"]), 0);
        assert_eq!(code("capture", &["set", "0003", "for", "human"]), 0);
        assert!(t("0003").is_human());
        // To do → Doing → Done by the owner, no gate line.
        assert_eq!(code("owner", &["pass", "0001", "--from", "ready"]), 0);
        assert_eq!(code("owner", &["pass", "0001", "--from", "building"]), 0);
        assert_eq!(t("0001").stage, "done");
        // list marks the owner's own tasks.
        let s = tasks_of(d.path());
        let all: Vec<&queue::Task> = s.tasks.iter().collect();
        let text = list_text(&[], &all);
        assert!(
            text.contains("  0003 code      normal  Try it on the phone  [you]\n"),
            "{text}"
        );
        assert!(
            text.contains("  0001 code      normal  Buy the domain  [you]\n"),
            "{text}"
        );
        assert!(
            text.contains("  0002 code      normal  Agent work  [on it: wt-a]\n"),
            "{text}"
        );
        assert_eq!(code("owner", &["check"]), 0);
    }

    #[test]
    fn clarify_lists_and_sends_the_batch_on_the_command_line() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        let code = |args: &[&str]| {
            let mut v = sv(&args[..1]);
            v.extend(sv(&["--queue", &q]));
            v.extend(sv(&args[1..]));
            main(&v)
        };
        assert_eq!(code(&["new", "Asked", "--as", "owner"]), 0);
        assert_eq!(code(&["stage", "0001", "shaping", "--as", "owner"]), 0);
        let s = ["--session", "abcd1234-full"];
        assert_eq!(
            code(&[
                "ask",
                "--as",
                "shapour",
                s[0],
                s[1],
                "--option",
                "8080",
                "--option",
                "9090",
                "--pick",
                "a",
                "--no-answer",
                "a",
                "--",
                "0001",
                "Which port?"
            ]),
            0
        );
        assert_eq!(code(&["clarify"]), 0);
        assert_eq!(code(&["clarify", "--json"]), 0);
        assert_eq!(code(&["clarify", "0001"]), 1, "takes no task");
        let before = queue::read_body(d.path(), "0001").unwrap();
        assert_eq!(code(&["clarify", "--send", "--as", "shapour"]), 0);
        let after = queue::read_body(d.path(), "0001").unwrap();
        assert!(
            after.contains("  **Escalated to owner (shapour, "),
            "{after}"
        );
        assert!(after.contains(" shapour: escalated Q1 to the owner (muster clarify)\n"));
        // Sent once: a second send writes nothing.
        assert_eq!(code(&["clarify", "--send", "--as", "shapour"]), 0);
        assert_eq!(queue::read_body(d.path(), "0001").unwrap(), after);
        assert_ne!(before, after);
        // The defaulted question doesn't hold Shaping; the task has no criteria, which does.
        let e = code(&["pass", "0001", "--from", "shaping", "--as", "shapour"]);
        assert_eq!(e, 1);
    }

    #[test]
    fn ask_escalate_and_inbox_on_the_command_line() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        // --queue goes right after the command: never after a `--`, where it would be text and
        // the queue would come from the current folder instead.
        let code = |args: &[&str]| {
            let mut v = sv(&args[..1]);
            v.extend(sv(&["--queue", &q]));
            v.extend(sv(&args[1..]));
            main(&v)
        };
        assert_eq!(code(&["new", "Asked", "--as", "owner"]), 0);
        let s = ["--session", "abcd1234-full"];
        let ask = |text: &str| code(&["ask", "--as", "wt-a", s[0], s[1], "--", "0001", text]);
        assert_eq!(ask("--port? 5173 or \"8080\"\nsecond line"), 0);
        let body = queue::read_body(d.path(), "0001").unwrap();
        assert!(
            body.contains(":** --port? 5173 or \"8080\"\n  second line\n"),
            "{body}"
        );
        assert_eq!(
            code(&["ask", "0001", "x?", "--as", "wt-a", "--session", "nothex!!"]),
            1
        );
        assert_eq!(
            code(&["ask", "0001", "--as", "wt-a", s[0], s[1]]),
            2,
            "no question"
        );
        assert_eq!(code(&["escalate", "0001", "Q1", "--as", "capture"]), 0);
        assert_eq!(
            code(&["escalate", "0001", "Q1", "--as", "capture"]),
            1,
            "again"
        );
        assert_eq!(code(&["escalate", "0001", "one", "--as", "capture"]), 1);
        assert_eq!(code(&["answer", "0001", "Q1", "5173", "--as", "owner"]), 0);
        assert!(d.path().join("mail/abcd1234/0001-Q1.md").is_file());
        assert_eq!(code(&["inbox", s[0], s[1], "--keep"]), 0);
        assert!(d.path().join("mail/abcd1234/0001-Q1.md").is_file(), "kept");
        assert_eq!(code(&["inbox", s[0], s[1]]), 0);
        assert!(d
            .path()
            .join("mail/abcd1234/delivered/0001-Q1.md")
            .is_file());
        assert_eq!(code(&["inbox", "--session", "zz"]), 1);
        let t = queue::read_task(d.path(), "0001").unwrap();
        assert_eq!((t.questions.len(), t.open_questions), (1, 0));
        assert!(t.questions[0].escalated && t.questions[0].owner_answered);
    }

    #[test]
    fn ask_with_options_and_answer_with_pick_on_the_command_line() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        let code = |args: &[&str]| {
            let mut v = sv(&args[..1]);
            v.extend(sv(&["--queue", &q]));
            v.extend(sv(&args[1..]));
            main(&v)
        };
        assert_eq!(code(&["new", "Picked", "--as", "owner"]), 0);
        let me = ["--as", "wt-a", "--session", "abcd1234"];
        let ask = |extra: &[&str]| {
            code(
                &[
                    &["ask", "0001"],
                    &me[..],
                    extra,
                    &["--", "How long may a lease be?"],
                ]
                .concat(),
            )
        };
        let body = || queue::read_body(d.path(), "0001").unwrap();
        // Refused, with nothing written: one option, seven, one too long, a pick or fallback that
        // isn't one, a pick without options.
        let before = body();
        let long = "x".repeat(201);
        for bad in [
            &["--option", "only"][..],
            &[
                "--option", "1", "--option", "2", "--option", "3", "--option", "4", "--option",
                "5", "--option", "6", "--option", "7",
            ],
            &["--option", "a", "--option", &long],
            &["--option", "a", "--option", "b", "--pick", "c"],
            &["--option", "a", "--option", "b", "--no-answer", "3"],
            &["--pick", "a"],
        ] {
            assert_eq!(ask(bad), 1, "{bad:?}");
        }
        assert_eq!(body(), before);
        assert!(!d.path().join("mail/asks.json").exists());
        let opts = [
            "--option",
            "1 h to 7 d",
            "--option",
            "Any length",
            "--option",
            "No lease",
        ];
        assert_eq!(
            ask(&[&opts[..], &["--pick", "a", "--no-answer", "C"]].concat()),
            0
        );
        assert!(
            body().contains(":** How long may a lease be?\n  - (a) 1 h to 7 d (my pick)\n  - (b) Any length\n  - (c) No lease\n  With no answer: (c)\n"),
            "{}",
            body()
        );
        // Escalated, then answered by picking: a bad pick is refused with nothing written.
        assert_eq!(code(&["escalate", "0001", "Q1", "--as", "capture"]), 0);
        let before = body();
        assert_eq!(
            code(&["answer", "0001", "Q1", "--pick", "d", "--as", "owner"]),
            1
        );
        assert_eq!(body(), before);
        assert!(!d.path().join("mail/abcd1234").exists());
        assert_eq!(
            code(&["answer", "0001", "Q1", "--pick", "b", "--as", "owner", "--", "but", "warn"]),
            0
        );
        assert!(body().contains("  **A (owner, "), "{}", body());
        assert!(
            body().contains("):** (b) Any length — but warn\n"),
            "{}",
            body()
        );
        let mail = std::fs::read_to_string(d.path().join("mail/abcd1234/0001-Q1.md")).unwrap();
        assert!(
            mail.contains("\nAnswer: (b) Any length — but warn\n"),
            "{mail}"
        );
        // A question in words has no options to pick.
        assert_eq!(
            code(&[&["ask", "0001"], &me[..], &["--", "Which port?"]].concat()),
            0
        );
        assert_eq!(
            code(&["answer", "0001", "Q2", "--pick", "a", "--as", "owner"]),
            1
        );
        assert_eq!(code(&["answer", "0001", "Q2", "5173", "--as", "owner"]), 0);
    }

    #[test]
    fn options_text_lists_each_option_with_the_pick_and_the_fallback() {
        let c = crate::choices::clean(&sv(&["One", "Two"]), Some("b"), Some("a")).unwrap();
        assert_eq!(
            options_text(&c),
            "  (a) One\n  (b) Two  ← pick\n  With no answer: (a)\n"
        );
        let c = crate::choices::clean(&sv(&["One", "Two"]), None, None).unwrap();
        assert_eq!(options_text(&c), "  (a) One\n  (b) Two\n");
    }

    #[test]
    fn index_rebuilds_the_master_list_and_check_reports_a_stale_one() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        let code = |args: &[&str]| {
            let mut v = sv(&args[..1]);
            v.extend(sv(&["--queue", &q, "--as", "owner"]));
            v.extend(sv(&args[1..]));
            main(&v)
        };
        let index = d.path().join(crate::index::FILE);
        // No list yet: `index --check` says so; `check` doesn't call it a problem.
        assert_eq!(code(&["index", "--check"]), 1);
        assert_eq!(code(&["check"]), 0);
        assert_eq!(code(&["index"]), 0);
        assert!(std::fs::read_to_string(&index)
            .unwrap()
            .starts_with("# Master list\n"));
        // Every write keeps it current.
        assert_eq!(code(&["new", "Listed"]), 0);
        assert_eq!(code(&["index", "--check"]), 0);
        assert!(std::fs::read_to_string(&index)
            .unwrap()
            .contains("[Listed](todo/0001-listed.md)"));
        // A hand edit leaves it stale: both checks fail until it's rebuilt.
        let f = d.path().join("todo/0001-listed.md");
        let text = std::fs::read_to_string(&f).unwrap();
        std::fs::write(&f, text.replace("# Listed", "# Edited by hand")).unwrap();
        assert_eq!(code(&["index", "--check"]), 1);
        assert_eq!(code(&["check"]), 1);
        assert_eq!(code(&["index"]), 0);
        assert_eq!(code(&["index", "--check"]), 0);
        assert_eq!(code(&["check"]), 0);
        assert!(std::fs::read_to_string(&index)
            .unwrap()
            .contains("[Edited by hand]"));
        // A write outside muster that the list doesn't show (Drover's answer): check warns
        // without failing; `index --check` still says it isn't as muster would write it.
        let text = std::fs::read_to_string(&f).unwrap();
        std::fs::write(&f, format!("{text}- 2026-10-14 09:00 owner: answered Q1\n")).unwrap();
        assert_eq!(code(&["check"]), 0);
        assert_eq!(code(&["index", "--check"]), 1);
        assert_eq!(code(&["index"]), 0);
        assert_eq!(code(&["index", "--check"]), 0);
        // The lines check prints: a warning by the problems' convention, or a problem.
        let refresh = index_check_line(index::Status::Refresh).unwrap();
        assert!(queue::is_warning(&refresh), "{refresh}");
        assert!(refresh.contains("INDEX.md: refresh due"), "{refresh}");
        let stale = index_check_line(index::Status::Stale).unwrap();
        assert!(!queue::is_warning(&stale), "{stale}");
        assert_eq!(index_check_line(index::Status::Missing), None);
        assert_eq!(index_check_line(index::Status::Current), None);
    }

    #[test]
    fn index_as_built_fills_the_documents_queue_section_from_the_callers_home() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        let out = crate::testkit::tempdir().unwrap();
        let file = out.path().join("AS_BUILT.md");
        let f = file.to_string_lossy().to_string();
        let home = out.path().join("home").to_string_lossy().to_string();
        let env = |k: &str| (k == "HOME").then(|| home.clone());
        let code = |args: &[&str]| {
            let mut v = sv(&args[..1]);
            v.extend(sv(&["--queue", &q, "--as", "owner"]));
            v.extend(sv(&args[1..]));
            main_with(&v, &env)
        };
        assert_eq!(code(&["new", &format!("Under {home}/x")]), 0);
        let head = "# As built\n\nMine.\n";
        let doc = format!(
            "{head}{}\nold\n{}\nAlso mine.\n",
            index::GLANCE_START,
            index::GLANCE_END
        );
        std::fs::write(&file, &doc).unwrap();
        assert_eq!(code(&["index", "--as-built", &f]), 0);
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.starts_with(&format!("{head}{}\n", index::GLANCE_START)));
        assert!(text.ends_with(&format!("{}\nAlso mine.\n", index::GLANCE_END)));
        assert!(text.contains("-1 | Under \\~/x |  |\n"), "{text}");
        assert!(!text.contains(&home), "{text}");
        assert_eq!(code(&["index", "--check"]), 0);
        // Without its markers: exit 1, nothing written.
        std::fs::write(&file, head).unwrap();
        assert_eq!(code(&["index", "--as-built", &f]), 1);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), head);
        // --check writes nothing, so it doesn't go with --as-built.
        assert_eq!(code(&["index", "--check", "--as-built", &f]), 2);
        assert_eq!(code(&["index", "--as-built"]), 1);
    }

    /// `<base>/<project>/tasks` as a queue, and `<base>/<project>/main` as its repo.
    fn project(base: &Path, name: &str) -> (PathBuf, PathBuf) {
        let q = base.join(name).join("tasks");
        std::fs::create_dir_all(q.join("todo")).unwrap();
        std::fs::write(q.join("README.md"), "q").unwrap();
        let repo = base.join(name).join("main");
        std::fs::create_dir_all(repo.join(".claude/worktrees/wt-x/src")).unwrap();
        (q, repo)
    }

    #[test]
    fn a_repos_muster_queue_is_found_from_its_worktrees_and_the_env_check_knows_it() {
        use crate::layout::resolve::{git, repo};
        let base = crate::testkit::tempdir().unwrap();
        let (drover, drover_repo) = project(base.path(), "drover");
        let main = repo(base.path(), "muster");
        let q = projects::init(&main, true, None, &[]).unwrap().0;
        let wt = base.path().join("muster/wt-1");
        git(
            &main,
            &["worktree", "add", "-q", "-b", "wt-1", wt.to_str().unwrap()],
        );
        // The project's old queue is still there, as it is until it's retired: .muster wins.
        let old = base.path().join("muster/tasks");
        std::fs::create_dir_all(old.join("todo")).unwrap();
        std::fs::write(old.join("README.md"), "q").unwrap();
        let known = || projects::active(base.path(), &Default::default());
        let canon = |p: &Path| std::fs::canonicalize(p).unwrap();
        for from in [&wt, &main, &q] {
            let (root, source) = resolve_queue(None, None, Some(from), &known).unwrap();
            assert_eq!(
                (canon(&root), source),
                (canon(&q), Source::Here),
                "{from:?}"
            );
        }
        // --queue by the project's name, or by the folder.
        for flag in ["muster", q.to_str().unwrap()] {
            let (root, source) = resolve_queue(Some(flag), None, None, &known).unwrap();
            assert_eq!((canon(&root), source), (canon(&q), Source::Flag), "{flag}");
        }
        // MUSTER_ROOT at another project's old queue, from a worktree of this repo: refused.
        let env = Some(("MUSTER_ROOT", drover.clone()));
        let (root, source) = resolve_queue(None, env, Some(&wt), &known).unwrap();
        assert_eq!(
            check_target(&source, &root, Some(&wt), &known).unwrap_err(),
            "MUSTER_ROOT points at drover/tasks but you're in muster/…; pass --queue drover to mean it"
        );
        // MUSTER_ROOT at this repo's .muster: its own queue from any worktree, refused elsewhere.
        let env = Some(("MUSTER_ROOT", q.clone()));
        let (root, source) = resolve_queue(None, env, Some(&wt), &known).unwrap();
        assert!(check_target(&source, &root, Some(&wt), &known).is_ok());
        assert!(check_target(&source, &root, Some(&main), &known).is_ok());
        assert_eq!(
            check_target(&source, &root, Some(&drover_repo), &known).unwrap_err(),
            "MUSTER_ROOT points at muster/main/.muster but you're in drover/…; pass --queue muster to mean it"
        );
    }

    #[test]
    fn init_makes_a_repos_muster_and_migrate_takes_its_options() {
        let base = crate::testkit::tempdir().unwrap();
        let main_dir = crate::layout::resolve::repo(base.path(), "p");
        let m = main_dir.to_string_lossy().to_string();
        let outside = crate::testkit::tempdir().unwrap();
        let o = outside.path().to_string_lossy().to_string();
        // --local outside a repo is refused; without it, the old tasks/ as before.
        assert_eq!(main(&sv(&["init", &o, "--local"])), 1);
        assert!(!outside.path().join("tasks").exists());
        let old = base.path().join("p/tasks");
        std::fs::create_dir_all(old.join("todo")).unwrap();
        std::fs::write(old.join("README.md"), "q").unwrap();
        // The plan, nothing written; then init, after which migrate refuses.
        let q = old.to_string_lossy().to_string();
        assert_eq!(main(&sv(&["migrate", &q, &m, "--local", "--dry-run"])), 0);
        assert!(!main_dir.join(".muster").exists());
        // Local by default, until live state moves to .git/muster (0005's next step).
        assert_eq!(main(&sv(&["init", &m])), 0);
        assert_eq!(
            crate::layout::read_config(&main_dir.join(".muster")),
            Ok(crate::layout::Config { local: true })
        );
        // The work log's merge driver (0067), in the repo's own config.
        assert_eq!(
            crate::parallel::tests::g(&main_dir, &["config", "--local", "merge.muster-log.driver"]),
            crate::merge_log::driver_command("muster")
        );
        assert_eq!(main(&sv(&["migrate", &q, &m, "--dry-run"])), 1);
        assert_eq!(main(&sv(&["migrate", &q])), 2, "needs the repo");
        // A committed queue only when asked; --local and --committed together are refused.
        let other = crate::layout::resolve::repo(base.path(), "c");
        let c = other.to_string_lossy().to_string();
        assert_eq!(main(&sv(&["init", &c, "--local", "--committed"])), 1);
        assert_eq!(main(&sv(&["init", &c, "--committed"])), 0);
        assert_eq!(
            crate::layout::read_config(&other.join(".muster")),
            Ok(crate::layout::Config { local: false })
        );
    }

    #[test]
    fn an_env_queue_is_refused_for_claims_made_from_another_projects_folder() {
        let base = crate::testkit::tempdir().unwrap();
        let (drover, _) = project(base.path(), "drover");
        let (muster, repo) = project(base.path(), "muster");
        let known = || vec![("drover".to_string(), drover.clone())];
        let env = || Some(("MUSTER_ROOT", drover.clone()));
        // From muster's repo, MUSTER_ROOT at drover's queue: the queue resolves, and claims are refused.
        let (root, source) = resolve_queue(None, env(), Some(&repo), &known).unwrap();
        assert_eq!(
            (root.as_path(), &source),
            (drover.as_path(), &Source::Env("MUSTER_ROOT"))
        );
        let e = check_target(&source, &root, Some(&repo), &known).unwrap_err();
        assert_eq!(
            e,
            "MUSTER_ROOT points at drover/tasks but you're in muster/…; pass --queue drover to mean it"
        );
        // Inside a worktree of that repo too, and from TASKS_ROOT, naming the variable used.
        let wt = repo.join(".claude/worktrees/wt-x/src");
        let old = Some(("TASKS_ROOT", drover.clone()));
        let (root, source) = resolve_queue(None, old, Some(&wt), &known).unwrap();
        assert!(check_target(&source, &root, Some(&wt), &known)
            .unwrap_err()
            .starts_with("TASKS_ROOT points at drover/tasks but you're in muster/…"));
        // A queue Muster doesn't know by name: the message gives its folder.
        let none = || Vec::new();
        assert!(check_target(&source, &root, Some(&wt), &none)
            .unwrap_err()
            .ends_with(&format!("pass --queue {} to mean it", drover.display())));
        // --queue always wins, by name or by folder.
        for flag in ["drover", drover.to_str().unwrap()] {
            let (root, source) = resolve_queue(Some(flag), env(), Some(&repo), &known).unwrap();
            assert_eq!((root.as_path(), &source), (drover.as_path(), &Source::Flag));
            assert!(check_target(&source, &root, Some(&repo), &known).is_ok());
        }
        // The env's queue is the folder's own (by a symlink, say), or the folder has none: fine.
        let link = base.path().join("link-to-muster-tasks");
        std::os::unix::fs::symlink(&muster, &link).unwrap();
        let (root, source) =
            resolve_queue(None, Some(("MUSTER_ROOT", link)), Some(&repo), &known).unwrap();
        assert!(check_target(&source, &root, Some(&repo), &known).is_ok());
        let elsewhere = crate::testkit::tempdir().unwrap();
        let (root, source) = resolve_queue(None, env(), Some(elsewhere.path()), &known).unwrap();
        assert!(check_target(&source, &root, Some(elsewhere.path()), &known).is_ok());
        assert!(check_target(&source, &root, None, &known).is_ok());
        // With no env, the folder's own queue; with neither, a clear refusal.
        let (root, source) = resolve_queue(None, None, Some(&wt), &known).unwrap();
        assert_eq!((root, source), (muster.clone(), Source::Here));
        assert!(resolve_queue(None, None, Some(elsewhere.path()), &known)
            .unwrap_err()
            .contains("no queue here"));
        assert!(resolve_queue(Some("nowhere"), None, None, &known)
            .unwrap_err()
            .contains("no queue named"));
        // Which commands are checked: the ones that claim, hand on or hold a task, or rewrite its
        // write-up, not reads.
        for cmd in [
            "claim",
            "next",
            "pass",
            "return",
            "block",
            "unblock",
            "stage",
            "heartbeat",
            "checkin",
            "add",
            "file",
            "drop",
            "edit",
        ] {
            assert!(TARGETED.contains(&cmd), "{cmd}");
        }
        for cmd in ["list", "show", "check", "projects", "help", "shape"] {
            assert!(!TARGETED.contains(&cmd), "{cmd}");
        }
    }

    #[test]
    fn shape_says_whether_the_claim_is_new_renewed_or_taken_over() {
        let hold = backlog::Hold {
            by: "shapour".into(),
            at: "2026-10-12 09:00".into(),
            heartbeat: "2026-10-12 14:05".into(),
        };
        let mut c = backlog::Claim {
            note: "a b.md".into(),
            hold,
            renewed: false,
            took_over: String::new(),
            quiet: String::new(),
        };
        let ends = "muster file or drop ends it; muster release 'a b.md' lets it go";
        assert_eq!(claim_line(&c), format!("you're shaping a b.md now. {ends}"));
        c.took_over = "wt-b".into();
        c.quiet = "13 h".into();
        assert_eq!(
            claim_line(&c),
            format!(
                "took a b.md over from wt-b (no heartbeat for 13 h); you're shaping it now. {ends}"
            )
        );
        c.renewed = true;
        assert_eq!(
            claim_line(&c),
            format!("you're shaping a b.md (since 2026-10-12 09:00); heartbeat 2026-10-12 14:05. {ends}")
        );
        // --json: the brief, with the claim under `claim` unless peeked.
        let d = v2_queue();
        std::fs::write(d.path().join("inbox/a b.md"), "An idea").unwrap();
        let b = backlog::shape(d.path(), "a b.md").unwrap();
        let v = shape_json(&b, Some(&c));
        assert_eq!(v["claim"]["note"], "a b.md");
        assert_eq!(v["claim"]["renewed"], true);
        assert_eq!(v["claim"]["hold"]["by"], "shapour");
        assert_eq!(v["note"], serde_json::json!(b)["note"]);
        assert_eq!(v["measured"], serde_json::json!(b.measured));
        assert_eq!(v["measured"]["overall"]["n"], 0);
        assert!(shape_json(&b, None).get("claim").is_none());
    }

    #[test]
    fn reclaim_says_each_task_and_note_it_released_or_that_nothing_was() {
        let key = |id: &str| format!("mu-{}", id.trim_start_matches('0'));
        let mut r = ops::Reclaimed::default();
        assert_eq!(
            reclaim_text(&r, "12 h", key),
            "nothing to release: every claim in Shaping and Building, and on a backlog note, was heard from within 12 h\n"
        );
        // A note alone is something released.
        r.notes.push(backlog::ReleasedNote {
            note: "a.md".into(),
            holder: "shapour".into(),
            quiet: "13 h".into(),
        });
        assert_eq!(
            reclaim_text(&r, "12 h", key),
            "released backlog note a.md from shapour (no heartbeat for 13 h): nobody is shaping it now\n"
        );
        r.released = vec![
            ops::Released {
                id: "0007".into(),
                holder: "wt-a".into(),
                quiet: "14 h".into(),
                path: "todo/0007-t.md".into(),
                stage: "shaping".into(),
            },
            ops::Released {
                id: "0008".into(),
                holder: "wt-b".into(),
                quiet: "15 h".into(),
                path: "todo/0008-t.md".into(),
                stage: "ready".into(),
            },
        ];
        assert_eq!(
            reclaim_text(&r, "12 h", key),
            "released mu-7 from wt-a (no heartbeat for 14 h): unheld, still in Shaping (todo/0007-t.md)\n\
             released mu-8 from wt-b (no heartbeat for 15 h): back in Ready (todo/0008-t.md)\n\
             released backlog note a.md from shapour (no heartbeat for 13 h): nobody is shaping it now\n"
        );
        // Only failures: no "nothing to release" (they're on stderr).
        let failed = ops::Reclaimed {
            failed: vec!["0009: no".into()],
            ..Default::default()
        };
        assert_eq!(reclaim_text(&failed, "12 h", key), "");
    }

    #[test]
    fn a_note_too_big_to_shape_is_never_claimed() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        std::fs::write(d.path().join("inbox/huge.md"), vec![b'a'; 1024 * 1024 + 1]).unwrap();
        std::fs::write(d.path().join("inbox/idea.md"), "An idea").unwrap();
        let shape = |note: &str| main(&sv(&["shape", "--queue", &q, "--as", "wt-a", note]));
        assert_eq!(shape("huge.md"), 1);
        assert!(!d.path().join("shaping.json").exists(), "nothing claimed");
        // So the same session may shape another note.
        assert_eq!(shape("idea.md"), 0);
        let s = tasks_of(d.path());
        let held: Vec<_> = s
            .backlog
            .iter()
            .map(|n| (n.file.as_str(), n.held_by.as_str()))
            .collect();
        assert!(held.contains(&("idea.md", "wt-a")), "{held:?}");
        let holds = std::fs::read_to_string(d.path().join("shaping.json")).unwrap();
        assert!(!holds.contains("huge.md"), "{holds}");
    }

    #[test]
    fn shape_claims_peek_reads_and_release_lets_go_with_their_exit_codes() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        std::fs::write(d.path().join("inbox/idea.md"), "An idea").unwrap();
        std::fs::write(d.path().join("inbox/other.md"), "Another").unwrap();
        let argv = |who: &str, args: &[&str]| {
            let mut v = sv(&args[..1]);
            v.extend(sv(&["--queue", &q, "--as", who]));
            v.extend(sv(&args[1..]));
            v
        };
        let code = |who: &str, args: &[&str]| main(&argv(who, args));
        let held = |file: &str| {
            let s = tasks_of(d.path());
            let n = s.backlog.iter().find(|n| n.file == file).unwrap();
            n.held_by.clone()
        };
        // --peek prints the brief and writes nothing.
        let before = files_under(d.path());
        assert_eq!(code("wt-a", &["shape", "idea.md", "--peek"]), 0);
        assert_eq!(code("wt-a", &["shape", "idea.md", "--peek", "--json"]), 0);
        assert_eq!(files_under(d.path()), before, "peek writes nothing");
        // Without it, shape claims the note; again by the holder renews it.
        assert_eq!(code("wt-a", &["shape", "idea.md"]), 0);
        assert_eq!(held("idea.md"), "wt-a");
        assert_eq!(code("wt-a", &["shape", "idea.md", "--json"]), 0);
        // Someone else is refused, told who and since when; they may peek.
        let e = run(&argv("wt-b", &["shape", "idea.md"])).unwrap_err();
        assert!(
            e.starts_with("wt-a is shaping it (since ") && e.ends_with("); --peek to read it"),
            "{e}"
        );
        assert_eq!(code("wt-b", &["shape", "idea.md"]), 1);
        assert_eq!(code("wt-b", &["shape", "idea.md", "--peek"]), 0);
        // One note at a time.
        let e = run(&argv("wt-a", &["shape", "other.md"])).unwrap_err();
        assert!(e.contains("you're shaping idea.md already"), "{e}");
        // release <note>: the holder or the owner, never someone else.
        assert_eq!(code("wt-b", &["release", "idea.md"]), 1);
        assert_eq!(code("wt-a", &["release", "idea.md"]), 0);
        assert_eq!(held("idea.md"), "");
        assert_eq!(code("wt-a", &["release", "idea.md"]), 1, "nothing held");
        assert_eq!(code("wt-a", &["release"]), 2);
        assert_eq!(code("wt-b", &["shape", "idea.md"]), 0);
        // Typed in another case, it's the same note where the file system says so (APFS).
        let same = d.path().join("inbox/IDEA.MD").is_file();
        assert_eq!(
            code("owner", &["release", "IDEA.MD"]),
            if same { 0 } else { 1 }
        );
        if !same {
            assert_eq!(code("owner", &["release", "idea.md"]), 0);
        }
        assert_eq!(held("idea.md"), "");
        // A task is still released with --clean, by its key too.
        let e = run(&argv("wt-a", &["release", "0001"])).unwrap_err();
        assert!(e.contains("release takes --clean"), "{e}");
        // check reports a shaping.json it can't read, and is clean again once it's gone.
        assert_eq!(code("wt-a", &["check"]), 0);
        std::fs::write(d.path().join("shaping.json"), "{not json").unwrap();
        assert_eq!(code("wt-a", &["check"]), 1);
        assert_eq!(code("wt-a", &["shape", "idea.md"]), 0, "writes it anew");
        assert_eq!(code("wt-a", &["check"]), 0);
        // A claim on a note that went some other way is reported until it's released.
        std::fs::remove_file(d.path().join("inbox/idea.md")).unwrap();
        assert_eq!(code("wt-a", &["check"]), 1);
        assert_eq!(code("wt-a", &["release", "idea.md"]), 0);
        assert_eq!(code("wt-a", &["check"]), 0);
    }

    #[test]
    fn claim_takeover_takes_a_note_from_the_command_line() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        std::fs::write(d.path().join("inbox/idea.md"), "An idea").unwrap();
        let code = |who: &str, args: &[&str]| {
            let mut v = sv(&args[..1]);
            v.extend(sv(&["--queue", &q, "--as", who]));
            v.extend(sv(&args[1..]));
            main(&v)
        };
        let holds = || std::fs::read_to_string(d.path().join("shaping.json")).unwrap();
        assert_eq!(code("wt-a", &["shape", "idea.md"]), 0);
        // Another label is refused, and a plain claim never takes a note.
        assert_eq!(code("wt-b", &["claim", "idea.md", "--takeover"]), 1);
        assert_eq!(code("wt-b", &["claim", "idea.md"]), 1);
        assert!(holds().contains("\"by\": \"wt-a\""), "{}", holds());
        // The owner overrides.
        assert_eq!(code("owner", &["claim", "idea.md", "--takeover"]), 0);
        assert!(holds().contains("\"by\": \"owner\""), "{}", holds());
        assert!(!holds().contains("wt-a"), "{}", holds());
    }

    #[test]
    fn list_shows_who_is_shaping_a_note_and_json_gives_held_by_since_and_heartbeat() {
        let d = v2_queue();
        let p = d.path();
        std::fs::write(p.join("inbox/idea.md"), "Board flickers").unwrap();
        std::fs::write(p.join("inbox/other.md"), "Export CSV").unwrap();
        let who = "Shapour (main · session 145fae12 · cc-mg)";
        // list_text's clock is 2026-10-14 14:14: held for 12 minutes.
        backlog::claim_note(p, "idea.md", who, "2026-10-14 14:02", ops::LEASE_MINUTES).unwrap();
        let s = tasks_of(p);
        let notes: Vec<&backlog::Note> = s.backlog.iter().collect();
        let text = list_text(&notes, &[]);
        assert!(
            text.contains("  idea.md  Board flickers  · shaping: Shapour, 12 min\n"),
            "{text}"
        );
        assert!(text.contains("  other.md  Export CSV\n"), "{text}");
        let json = list_json(&notes, &[]);
        let note = |file: &str| {
            json["backlog"]
                .as_array()
                .unwrap()
                .iter()
                .find(|n| n["file"] == file)
                .unwrap()
                .clone()
        };
        let held = note("idea.md");
        assert_eq!(
            (&held["heldBy"], &held["heldSince"], &held["heartbeat"]),
            (
                &serde_json::json!(who),
                &serde_json::json!("2026-10-14 14:02"),
                &serde_json::json!("2026-10-14 14:02")
            )
        );
        let free = note("other.md");
        assert_eq!(
            (&free["heldBy"], &free["heldSince"], &free["heartbeat"]),
            (
                &serde_json::json!(""),
                &serde_json::json!(""),
                &serde_json::json!("")
            )
        );
        // Over an hour: in hours, as reclaim says it.
        let (head, _) = list_parts(&notes, &[], "2026-10-14 16:30");
        assert!(head.contains("· shaping: Shapour, 2 h\n"), "{head}");
        // Filed, the task in Shaping is the shaper's, and its line says who's on it.
        let new = ops::NewTask {
            title: "Board flickers".into(),
            ..Default::default()
        };
        backlog::file_note(
            p,
            "idea.md",
            &new,
            who,
            "2026-10-14 14:20",
            ops::LEASE_MINUTES,
        )
        .unwrap();
        let s = tasks_of(p);
        let tasks: Vec<&queue::Task> = s.tasks.iter().collect();
        let text = list_text(&[], &tasks);
        assert!(
            text.contains(&format!("Board flickers  [on it: {who}]\n")),
            "{text}"
        );
    }

    #[test]
    fn backlog_add_shape_file_and_drop_with_their_exit_codes() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        let code = |args: &[&str]| {
            let mut v = sv(&args[..1]);
            v.extend(sv(&["--queue", &q, "--as", "wt-a"]));
            v.extend(sv(&args[1..]));
            main(&v)
        };
        // A one-liner needs no quotes; the body takes \n like new's.
        assert_eq!(code(&["add", "Board", "flickers", "on", "resize"]), 0);
        assert_eq!(code(&["add", "With a body", "--body", "one\\ntwo"]), 0);
        assert_eq!(code(&["add"]), 2);
        assert_eq!(code(&["add", "  "]), 1);
        let notes = tasks_of(d.path()).backlog;
        let file_of = |title: &str| {
            notes
                .iter()
                .find(|n| n.title == title)
                .map(|n| n.file.clone())
                .unwrap()
        };
        let (flicker, other) = (file_of("Board flickers on resize"), file_of("With a body"));
        assert!(notes
            .iter()
            .any(|n| n.text.contains("With a body\n\none\ntwo\n")));
        assert_eq!(code(&["shape", &flicker]), 0);
        assert_eq!(code(&["shape", &flicker, "--json"]), 0);
        assert_eq!(code(&["shape"]), 2);
        assert_eq!(code(&["shape", "gone.md"]), 1);
        assert_eq!(code(&["file", &flicker]), 2, "no title");
        assert_eq!(code(&["file", &flicker, "Board flickers"]), 2, "no --body");
        let criteria = "## Acceptance criteria\\n- [ ] AC1: steady — verify: screenshot";
        let filed = ["file", &flicker, "Board", "flickers", "--type", "bug"];
        assert_eq!(code(&[&filed[..], &["--body", criteria]].concat()), 0);
        assert_eq!(code(&["file", &flicker, "Again", "--body", "x"]), 1, "gone");
        let t = queue::read_task(d.path(), "0001").unwrap();
        assert_eq!(
            (t.stage.as_str(), t.kind.as_str(), t.title.as_str()),
            ("shaping", "bug", "Board flickers")
        );
        assert_eq!(t.attachments, vec![flicker.clone()]);
        assert_eq!(code(&["pass", "0001", "--from", "shaping"]), 0);
        assert_eq!(queue::read_task(d.path(), "0001").unwrap().stage, "ready");
        assert_eq!(code(&["drop", &other]), 2, "no reason");
        assert_eq!(code(&["drop", &other, "not", "needed"]), 0);
        assert!(tasks_of(d.path()).backlog.is_empty());
        assert!(
            std::fs::read_to_string(d.path().join("inbox/dropped").join(&other))
                .unwrap()
                .contains(": not needed\n")
        );
        assert_eq!(code(&["check"]), 0);
        // A skip with no reason, or of a lens that doesn't exist, fails the check (0076).
        let readme = d.path().join("README.md");
        let before = std::fs::read_to_string(&readme).unwrap();
        std::fs::write(&readme, "---\nshaping-skip: payments\n---\n\nq").unwrap();
        assert_eq!(code(&["check"]), 1);
        std::fs::write(
            &readme,
            "---\nshaping-skip: payments\nshaping-reason-payments: free\n---\n\nq",
        )
        .unwrap();
        assert_eq!(code(&["check"]), 0);
        std::fs::write(&readme, before).unwrap();
        // A note over 1 MB isn't read, and check says so.
        std::fs::write(d.path().join("inbox/huge.md"), vec![b'a'; 1024 * 1024 + 1]).unwrap();
        assert_eq!(code(&["check"]), 1);
        let f = crate::backlog::Filed {
            id: "0031".into(),
            path: "todo/0031-x.md".into(),
            attachment: "n.md".into(),
            missing_lenses: vec!["Themes".into(), "Docs".into()],
            markers: vec!["[NEEDS CLARIFICATION: which port?]".into()],
        };
        let base = crate::testkit::tempdir().unwrap();
        let (q, _) = project(base.path(), "drover");
        assert_eq!(
            filed_message(&q, "dr", &f),
            "filed dr-31 in drover (Shaping)"
        );
        // A body with no line for a lens is filed (exit 0 above), with a warning naming them.
        assert_eq!(
            lens_warning("dr-31", &f.missing_lenses).as_deref(),
            Some("warning: dr-31 has no Lenses line for 2 lenses: Themes, Docs. It's filed anyway: add one line each under **Lenses:** (what the task must do for it, or \"n/a\" and why) with muster edit dr-31, or skip a lens for the whole project with its reason (muster help shape).")
        );
        assert!(lens_warning("dr-31", &["Docs".into()])
            .unwrap()
            .contains(" for 1 lens: Docs. "));
        assert_eq!(lens_warning("dr-31", &[]), None);
        // So is one with a marker left (mu-103), named.
        assert_eq!(
            marker_warning("dr-31", &f.markers).as_deref(),
            Some("warning: dr-31 has 1 [NEEDS CLARIFICATION] marker in its write-up: [NEEDS CLARIFICATION: which port?]. It's filed anyway, but it can't pass to Ready until each is decided (under ## Questions, \"Decided by <you>, overrule any time: …\") or asked (muster ask dr-31 …, the marker replaced with \"(see Q<n>)\") with muster edit dr-31.")
        );
        assert!(marker_warning("dr-31", &["[a]".into(), "[b]".into()])
            .unwrap()
            .contains(" has 2 [NEEDS CLARIFICATION] markers in its write-up: [a]; [b]. "));
        assert_eq!(marker_warning("dr-31", &[]), None);
    }

    #[test]
    fn a_claim_says_which_queue_it_was_made_in() {
        let base = crate::testkit::tempdir().unwrap();
        let (q, _) = project(base.path(), "muster");
        for f in queue::STATUS_FOLDERS {
            std::fs::create_dir_all(q.join(f)).unwrap();
        }
        let c = ops::create_task(
            &q,
            &ops::NewTask {
                title: "Said".into(),
                ..Default::default()
            },
            "o",
            "2026-10-12 10:00",
        )
        .unwrap();
        let p = ops::claim_task(&q, &c.id, "wt-a", "2026-10-12 10:01").unwrap();
        assert_eq!(
            claimed_message(&q, "mu", &c.id, &p),
            format!(
                "claimed mu-1 in muster ({})\nmu-1: Building (doing/0001-said.md)",
                q.display()
            )
        );
    }

    /// `cmd` with the queue `q`, run with the environment `vars`.
    fn code_in(q: &str, vars: &[(&str, &str)], args: &[&str]) -> i32 {
        let vars: std::collections::HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let mut v = sv(&args[..1]);
        v.extend(sv(&["--queue", q]));
        v.extend(sv(&args[1..]));
        main_with(&v, &|k: &str| vars.get(k).cloned())
    }

    /// A v2 queue that is its own git repo with `user.name` set, as `<project>/tasks` is.
    fn git_queue(name: &str) -> crate::testkit::TestDir {
        let d = v2_queue();
        let git = |args: &[&str]| {
            let ok = std::process::Command::new("git")
                .arg("-C")
                .arg(d.path())
                .args(args)
                .output()
                .unwrap()
                .status
                .success();
            assert!(ok, "git {args:?}");
        };
        git(&["init", "-q"]);
        git(&["config", "user.name", name]);
        d
    }

    #[test]
    fn without_as_the_actor_is_the_git_name_with_the_session_and_never_the_login() {
        let d = git_queue("Test Person");
        let q = d.path().to_string_lossy().to_string();
        let login = [("USER", "loginname-xyz"), ("LOGNAME", "loginname-xyz")];
        assert_eq!(code_in(&q, &login, &["new", "Mine"]), 0);
        let session = [
            ("USER", "loginname-xyz"),
            ("LOGNAME", "loginname-xyz"),
            (
                "CLAUDE_CODE_SESSION_ID",
                "7b5e8856-c9c8-45ec-aa03-c0ddbf1d4394",
            ),
            ("CLAUDE_CONFIG_DIR", "/nowhere/.claude-hg"),
        ];
        assert_eq!(code_in(&q, &session, &["claim", "0001"]), 0);
        let t = queue::read_task(d.path(), "0001").unwrap();
        let body = queue::read_body(d.path(), "0001").unwrap();
        assert!(body.contains("Test Person: created"), "{body}");
        let (label, stamp) = session::split(&t.claimed_by);
        let stamp = stamp.unwrap();
        assert_eq!(
            (label, stamp.id.as_str(), stamp.account.as_str()),
            ("Test Person", "7b5e8856", "hg")
        );
        assert!(body.contains(&format!(
            "{}: stage ready → building (claimed)",
            t.claimed_by
        )));
        assert!(!body.contains("loginname-xyz"), "{body}");
        // `--as` alone still works, stamped with the session it runs in.
        assert_eq!(code_in(&q, &session, &["new", "Theirs"]), 0);
        assert_eq!(code_in(&q, &session, &["claim", "0002", "--as", "wt-b"]), 0);
        let claim = queue::read_task(d.path(), "0002").unwrap().claimed_by;
        assert!(
            claim.starts_with("wt-b (") && claim.contains("session 7b5e8856"),
            "{claim}"
        );
        // MUSTER_AS stands in for --as.
        let helper = [("MUSTER_AS", "helper")];
        assert_eq!(
            code_in(&q, &helper, &["note", "0001", "from", "the", "helper"]),
            0
        );
        assert!(queue::read_body(d.path(), "0001")
            .unwrap()
            .contains(" helper: from the helper\n"));
        // The questions' commands record the session on their own, so the asker isn't stamped.
        assert_eq!(code_in(&q, &session, &["ask", "0001", "Which port?"]), 0);
        assert!(queue::read_body(d.path(), "0001")
            .unwrap()
            .contains("**Q1 (Test Person, session 7b5e8856, "));
    }

    #[test]
    fn takeover_from_the_command_line_after_checkin_handoff() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        let a = [(
            "CLAUDE_CODE_SESSION_ID",
            "1c6af0d9-aaaa-bbbb-cccc-0123456789ab",
        )];
        let b = [(
            "CLAUDE_CODE_SESSION_ID",
            "d6419719-aaaa-bbbb-cccc-0123456789ab",
        )];
        assert_eq!(code_in(&q, &[], &["new", "Handed", "--as", "owner"]), 0);
        assert_eq!(code_in(&q, &a, &["claim", "0001", "--as", "asghar"]), 0);
        // Fresh and not handed off: refused, and B can't act on A's claim.
        assert_eq!(
            code_in(&q, &b, &["claim", "0001", "--takeover", "--as", "asghar"]),
            1
        );
        assert_eq!(
            code_in(&q, &b, &["checkin", "0001", "x", "--as", "asghar"]),
            1
        );
        assert_eq!(
            code_in(
                &q,
                &a,
                &[
                    "checkin",
                    "0001",
                    "parser",
                    "done",
                    "--handoff",
                    "--pct",
                    "40",
                    "--as",
                    "asghar"
                ]
            ),
            0
        );
        assert_eq!(
            code_in(&q, &b, &["claim", "0001", "--takeover", "--as", "asghar"]),
            0
        );
        assert_eq!(
            code_in(&q, &b, &["checkin", "0001", "on", "it", "--as", "asghar"]),
            0
        );
        assert_eq!(
            code_in(&q, &a, &["checkin", "0001", "me", "too", "--as", "asghar"]),
            1
        );
        let body = queue::read_body(d.path(), "0001").unwrap();
        // Stamped with the checkout the test runs in too, so only the stamps' ends are pinned.
        assert!(
            body.contains("session 1c6af0d9): check-in 40% · handing off · parser done\n"),
            "{body}"
        );
        let took = body
            .lines()
            .find(|l| l.contains(": took over the claim from "))
            .unwrap();
        assert!(
            took.contains(" asghar (")
                && took.contains("session d6419719): took over the claim from asghar (")
                && took.ends_with("session 1c6af0d9) (handed off)"),
            "{took}"
        );
        let t = queue::read_task(d.path(), "0001").unwrap();
        assert_eq!(
            session::session_of(&t.claimed_by),
            Some(("d6419719".into(), String::new()))
        );
    }

    #[test]
    fn a_teammates_commands_are_stamped_with_its_agent_and_a_bad_one_is_refused() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        let sid = (
            "CLAUDE_CODE_SESSION_ID",
            "1c6af0d9-aaaa-bbbb-cccc-0123456789ab",
        );
        assert_eq!(code_in(&q, &[], &["new", "One", "--as", "owner"]), 0);
        assert_eq!(code_in(&q, &[], &["new", "Two", "--as", "owner"]), 0);
        for bad in [vec![sid, ("MUSTER_AGENT", "nothex!!")], vec![sid]] {
            let mut args = vec!["claim", "0001", "--as", "asghar"];
            if bad.len() == 1 {
                args.extend(["--agent", "a1b2/../x"]);
            }
            assert_eq!(code_in(&q, &bad, &args), 1, "{bad:?}");
        }
        assert_eq!(queue::read_task(d.path(), "0001").unwrap().claimed_by, "");
        // From the environment, and from the flag.
        let tm = [sid, ("MUSTER_AGENT", "A1B2C3D4E5F6A7B8C")];
        assert_eq!(code_in(&q, &tm, &["claim", "0001", "--as", "asghar"]), 0);
        let held = |id: &str| {
            let c = queue::read_task(d.path(), id).unwrap().claimed_by;
            (session::split(&c).0.to_string(), session::session_of(&c))
        };
        let pair = |a: &str| Some(("1c6af0d9".to_string(), a.to_string()));
        assert_eq!(held("0001"), ("asghar".into(), pair("a1b2c3d4")));
        assert_eq!(
            code_in(
                &q,
                &[sid],
                &["claim", "0002", "--as", "asghar", "--agent", "0f0f0f0f99"]
            ),
            0
        );
        assert_eq!(held("0002"), ("asghar".into(), pair("0f0f0f0f")));
        // Its lead under the same label is another holder.
        assert_eq!(
            code_in(&q, &[sid], &["checkin", "0001", "x", "--as", "asghar"]),
            1
        );
        // A teammate's question is answered into its own folder, which only its inbox takes.
        assert_eq!(
            code_in(&q, &tm, &["ask", "0001", "Which port?", "--as", "asghar"]),
            0
        );
        assert!(queue::read_body(d.path(), "0001")
            .unwrap()
            .contains("**Q1 (asghar, session 1c6af0d9 · agent a1b2c3d4, "));
        assert_eq!(
            code_in(&q, &[], &["answer", "0001", "Q1", "5173", "--as", "owner"]),
            0
        );
        let agent_box = d.path().join("mail/1c6af0d9/a1b2c3d4");
        assert!(agent_box.join("0001-Q1.md").is_file());
        assert_eq!(code_in(&q, &[sid], &["inbox"]), 0);
        assert!(
            agent_box.join("0001-Q1.md").is_file(),
            "the lead's inbox leaves it"
        );
        assert_eq!(code_in(&q, &tm, &["inbox"]), 0);
        assert!(agent_box.join("delivered/0001-Q1.md").is_file());
    }

    #[test]
    fn the_label_falls_back_to_the_owner_only_when_git_gives_nothing_usable() {
        let some = |s: &str| Some(s.to_string());
        assert_eq!(
            pick_label(some("wt-a"), some("helper"), || some("G"), false),
            "wt-a"
        );
        assert_eq!(
            pick_label(None, some("helper"), || some("G"), false),
            "helper"
        );
        assert_eq!(pick_label(None, None, || some("G"), false), "G");
        assert_eq!(pick_label(None, None, || None, false), "owner");
        // In a session with no name to go by: an agent, never the owner.
        assert_eq!(pick_label(None, None, || None, true), "agent");
        assert_eq!(clean_git_name("Massihx\n").as_deref(), Some("Massihx"));
        assert_eq!(
            clean_git_name("  Two  Words \n").as_deref(),
            Some("Two Words")
        );
        assert_eq!(clean_git_name(&"n".repeat(90)).map(|n| n.len()), Some(64));
        for bad in [
            "",
            "\n",
            "x: stage ready → done",
            "x: check-in 50% · y",
            "Me (session 7b5e8856)",
            &"n".repeat(101),
        ] {
            assert_eq!(clean_git_name(bad), None, "{bad:?}");
        }
        assert_eq!(clean_git_name("a\u{7}b").as_deref(), Some("a b"));
    }

    #[test]
    fn checkin_on_the_command_line_with_its_exit_codes() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        let code = |who: &str, args: &[&str]| {
            let mut v = sv(args);
            v.extend(sv(&["--queue", &q, "--as", who]));
            main(&v)
        };
        assert_eq!(code("owner", &["new", "Long work"]), 0);
        assert_eq!(
            code("wt-a", &["checkin", "0001", "too early"]),
            1,
            "not Building"
        );
        assert_eq!(code("wt-a", &["claim", "0001"]), 0);
        assert_eq!(
            code(
                "wt-a",
                &["checkin", "0001", "gate", "green,", "writing", "tests", "--pct", "60"]
            ),
            0
        );
        assert_eq!(code("wt-a", &["checkin", "0001", "no percentage"]), 0);
        assert_eq!(
            code("wt-b", &["checkin", "0001", "not mine"]),
            1,
            "not the holder"
        );
        assert_eq!(code("wt-a", &["checkin", "0001", "x", "--pct", "101"]), 1);
        assert_eq!(code("wt-a", &["checkin", "0001", "x", "--pct", "high"]), 1);
        assert_eq!(
            code("wt-a", &["checkin", "0001", "x", "--pct"]),
            1,
            "needs a value"
        );
        assert_eq!(code("wt-a", &["checkin", "0001"]), 2, "no text");
        assert_eq!(code("wt-a", &["checkin"]), 2, "no id");
        let t = queue::read_task(d.path(), "0001").unwrap();
        let got: Vec<_> = t
            .checkins
            .iter()
            .map(|c| (c.pct, c.text.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                (Some(60), "gate green, writing tests"),
                (None, "no percentage")
            ]
        );
        assert_eq!(code("wt-a", &["show", "0001"]), 0);
        assert_eq!(code("wt-a", &["list"]), 0);
    }

    #[test]
    fn ping_asks_for_a_check_in_or_sends_a_message_to_the_holders_session() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        let owner = [("MUSTER_AS", "owner")];
        let session = [
            ("MUSTER_AS", "wt-a"),
            (
                "CLAUDE_CODE_SESSION_ID",
                "7b5e8856-c9c8-45ec-aa03-c0ddbf1d4394",
            ),
        ];
        for title in ["Stamped", "Unstamped"] {
            assert_eq!(code_in(&q, &owner, &["new", title]), 0);
        }
        assert_eq!(code_in(&q, &session, &["claim", "0001"]), 0);
        assert_eq!(code_in(&q, &[("MUSTER_AS", "wt-b")], &["claim", "0002"]), 0);
        let boxed = d.path().join("mail/7b5e8856");
        // No text: a check-in request.
        assert_eq!(code_in(&q, &owner, &["ping", "0001"]), 0);
        assert!(std::fs::read_to_string(boxed.join("0001-checkin.md"))
            .unwrap()
            .contains("run `muster checkin 0001 "));
        // Text: a message, every word of it.
        assert_eq!(
            code_in(&q, &owner, &["ping", "0001", "look", "at", "the", "test"]),
            0
        );
        assert!(std::fs::read_to_string(boxed.join("0001-note-0001.md"))
            .unwrap()
            .contains("> look at the test\n"));
        let body = queue::read_body(d.path(), "0001").unwrap();
        assert!(body.contains(" owner: asked for a check-in (session 7b5e8856)\n"));
        assert!(body.contains(" owner: messaged session 7b5e8856: look at the test\n"));
        // A claim without a session stamp: refused, with why.
        assert_eq!(code_in(&q, &owner, &["ping", "0002"]), 1);
        assert_eq!(code_in(&q, &owner, &["ping", "0002", "hello"]), 1);
        assert_eq!(code_in(&q, &owner, &["ping"]), 2, "no id");
    }

    #[test]
    fn pause_and_resume_a_task_its_session_told_each_time() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        let owner = [("MUSTER_AS", "owner")];
        let session = [
            ("MUSTER_AS", "wt-a"),
            (
                "CLAUDE_CODE_SESSION_ID",
                "7b5e8856-c9c8-45ec-aa03-c0ddbf1d4394",
            ),
        ];
        assert_eq!(code_in(&q, &owner, &["new", "Stamped"]), 0);
        assert_eq!(code_in(&q, &session, &["claim", "0001"]), 0);
        // The reason is every word after the id.
        assert_eq!(
            code_in(&q, &owner, &["pause", "0001", "the", "design", "changes"]),
            0
        );
        let t = queue::read_task(d.path(), "0001").unwrap();
        assert_eq!(
            (t.stage.as_str(), t.paused_why.as_str()),
            ("building", "the design changes")
        );
        assert!(d.path().join("mail/7b5e8856/0001-pause.md").exists());
        assert_eq!(code_in(&q, &owner, &["pause", "0001"]), 1, "already paused");
        assert_eq!(code_in(&q, &session, &["pass", "0001", "--gate", "g"]), 1);
        assert_eq!(code_in(&q, &owner, &["list"]), 0);
        assert_eq!(code_in(&q, &owner, &["resume", "0001"]), 0);
        assert!(d.path().join("mail/7b5e8856/0001-resume.md").exists());
        assert!(queue::read_task(d.path(), "0001")
            .unwrap()
            .paused
            .is_empty());
        assert_eq!(code_in(&q, &owner, &["resume", "0001"]), 1, "not paused");
        assert_eq!(code_in(&q, &owner, &["pause"]), 2, "no id");
        assert_eq!(code_in(&q, &owner, &["resume"]), 2, "no id");
        // By its key too (0033), and what it refuses with names the task by its key.
        std::fs::write(d.path().join("README.md"), "---\nprefix: mu\n---\n\nq").unwrap();
        assert_eq!(code_in(&q, &owner, &["pause", "mu-1", "later"]), 0);
        assert_eq!(
            queue::read_task(d.path(), "0001").unwrap().paused_why,
            "later"
        );
        assert_eq!(code_in(&q, &owner, &["pause", "MU-1"]), 1, "already paused");
        let since = queue::read_task(d.path(), "0001").unwrap().paused;
        assert_eq!(
            crate::key::name_tasks(d.path(), &pause::refusal("0001", &since), &[]),
            format!("mu-1 is paused (since {since}); resume it first: muster resume mu-1")
        );
        assert_eq!(code_in(&q, &owner, &["resume", "mu-1"]), 0);
        assert!(queue::read_task(d.path(), "0001")
            .unwrap()
            .paused
            .is_empty());
    }

    #[test]
    fn list_and_show_say_who_is_on_it_and_the_latest_check_in() {
        let d = v2_queue();
        let file = |id: &str, fm: &str, log: &str| {
            std::fs::write(
                d.path().join(format!("doing/{id}-t.md")),
                format!("---\nid: {id}\ntype: task\npriority: normal\n{fm}\n---\n\n# T\n\n## Log\n- 2026-10-12 09:00 capture: created\n{log}"),
            )
            .unwrap();
        };
        let a = "wt-a (wt-0030 · session 7b5e8856)";
        file(
            "0001",
            &format!("status: doing\nstage: building\nclaimed-by: {a}"),
            &format!("- 2026-10-12 10:00 {a}: stage ready → building (claimed)\n- 2026-10-12 10:30 {a}: check-in 60% · gate green, writing tests that take a long while to describe in full\n"),
        );
        file(
            "0002",
            "status: doing\nstage: testing\nheld-by: qa",
            "- 2026-10-12 11:00 wt-b: stage building → testing (gate: green)\n",
        );
        file("0003", "status: doing\nstage: your-check", "");
        let s = tasks_of(d.path());
        assert!(s.problems.is_empty(), "{:?}", s.problems);
        assert_eq!(
            list_text(&[], &list_filter(&s.tasks, &lq(None, None, None))),
            format!("Building (1)\n  0001 code      normal  T  [on it: {a}]  [check-in 60% · gate green, writing tests that take a long while to describe… (2026-10-12 10:30)]\nTesting (1)\n  0002 code      normal  T  [on it: qa]\nYour check (1)\n  0003 code      normal  T  [needs you]  [on it: owner]\n")
        );
        let by = |id: &str| s.tasks.iter().find(|t| t.id == id).unwrap();
        assert_eq!(
            show_footer(by("0001")),
            format!("\n---\non it: {a} (Building, since 2026-10-12 10:00)\nlatest check-in, by {a}: 60% · gate green, writing tests that take a long while to describe in full (2026-10-12 10:30)\n")
        );
        assert_eq!(
            show_footer(by("0002")),
            "\n---\non it: qa (Testing, since 2026-10-12 11:00)\n"
        );
    }

    #[test]
    fn the_editor_comes_from_visual_then_editor_then_vi_split_without_a_shell() {
        let s = |v: &str| Some(v.to_string());
        let ok = |visual: Option<String>, editor: Option<String>| {
            editor_command(visual, editor).unwrap()
        };
        assert_eq!(ok(s("code --wait"), s("nano")), ["code", "--wait"]);
        assert_eq!(ok(None, s("  nano  -w ")), ["nano", "-w"]);
        assert_eq!(ok(s(" "), s("nano")), ["nano"], "blank is unset");
        assert_eq!(ok(None, None), ["vi"]);
        // Quotes and backslashes, as a shell would split them, but no shell runs anything.
        assert_eq!(
            ok(s("\"/Applications/Sublime Text.app/x/subl\" -w"), None),
            ["/Applications/Sublime Text.app/x/subl", "-w"]
        );
        assert_eq!(
            ok(s("'/My Apps/ed it' --title 'a \"b\"'"), None),
            ["/My Apps/ed it", "--title", "a \"b\""]
        );
        assert_eq!(
            ok(s(r#"/My\ Apps/ed "x\"y" "a\\b" 'c\d' """#), None),
            ["/My Apps/ed", "x\"y", "a\\b", "c\\d", ""]
        );
        assert_eq!(ok(s("a\"b c\"d"), None), ["ab cd"], "quotes join a word");
        // `;` and `$(…)` are just words handed to the program.
        assert_eq!(
            ok(s("vi; rm -rf ~ $(x)"), None),
            ["vi;", "rm", "-rf", "~", "$(x)"]
        );
        for bad in ["\"subl -w", "'vi", "vi \\"] {
            let e = editor_command(s(bad), s("nano")).unwrap_err();
            assert!(e.contains("$VISUAL"), "{bad}: {e}");
        }
        assert!(editor_command(None, s("'vi"))
            .unwrap_err()
            .contains("$EDITOR"));
    }

    #[test]
    fn edit_refuses_before_opening_an_editor() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        let code = |who: &str, args: &[&str]| {
            let mut v = sv(args);
            v.extend(sv(&["--queue", &q, "--as", who]));
            main(&v)
        };
        assert_eq!(code("owner", &["new", "Held"]), 0);
        assert_eq!(code("wt-a", &["claim", "0001"]), 0);
        let before = queue::read_body(d.path(), "0001").unwrap();
        // Someone else holds it: refused (1) before any editor runs; usage (2) for the rest.
        assert_eq!(code("owner", &["edit", "0001"]), 1);
        assert_eq!(code("owner", &["edit"]), 2);
        assert_eq!(code("owner", &["edit", "0001", "--body", "new text"]), 2);
        assert_eq!(queue::read_body(d.path(), "0001").unwrap(), before);
        let e = run(&sv(&["edit", "0001", "--queue", &q, "--as", "owner"])).unwrap_err();
        assert!(e.contains("wt-a holds it; add a note instead"), "{e}");
    }

    #[test]
    fn levels_on_the_command_line_and_check_warns_without_failing() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        let code = |args: &[&str]| {
            let mut v = sv(&args[..1]);
            v.extend(sv(&["--queue", &q, "--as", "owner"]));
            v.extend(sv(&args[1..]));
            main(&v)
        };
        assert_eq!(
            code(&["new", "Ship it", "--level", "goal", "--due", "2026-12-01"]),
            0
        );
        assert_eq!(
            code(&[
                "new",
                "Stage track",
                "--level",
                "feature",
                "--parent",
                "0001"
            ]),
            0
        );
        assert_eq!(code(&["new", "Pass button", "--parent", "0002"]), 0);
        assert_eq!(
            code(&["new", "Bad", "--level", "feature", "--parent", "0002"]),
            1
        );
        assert_eq!(code(&["new", "Bad", "--level", "goal", "--due", "soon"]), 1);
        let t = |id: &str| queue::read_task(d.path(), id).unwrap();
        assert_eq!(
            (
                t("0001").level,
                t("0001").due,
                t("0002").parent,
                t("0003").parent
            ),
            (
                "goal".into(),
                "2026-12-01".into(),
                "0001".into(),
                "0002".into()
            )
        );
        assert_eq!(code(&["claim", "0002"]), 1, "a feature isn't built");
        assert_eq!(code(&["set", "0003", "parent", "0001"]), 0);
        assert_eq!(code(&["set", "0003", "parent", ""]), 0);
        assert_eq!(t("0003").parent, "");
        assert_eq!(code(&["set", "0003", "due", "2026-12-01"]), 1);
        assert_eq!(code(&["set", "0002", "level", "goal"]), 1);
        assert_eq!(code(&["check"]), 0);
        assert_eq!(code(&["show", "0001", "--json"]), 0);
        // A goal past its due with a task open is a warning: shown, but check still passes.
        assert_eq!(code(&["set", "0003", "parent", "0002"]), 0);
        assert_eq!(code(&["set", "0001", "due", "2020-01-01"]), 0);
        let s = tasks_of(d.path());
        assert_eq!(
            s.problems,
            ["q: warning: todo/0001-ship-it.md is past its due date (2020-01-01) with 1 of 1 tasks open"]
        );
        assert_eq!(code(&["check"]), 0);
        // A parent that doesn't exist (written by hand) fails it.
        let p = d.path().join("todo/0003-pass-button.md");
        let text = std::fs::read_to_string(&p).unwrap();
        std::fs::write(&p, text.replace("parent: 0002", "parent: 0099")).unwrap();
        assert_eq!(code(&["check"]), 1);
    }

    #[test]
    fn vision_brief_record_resolve_and_all_with_their_exit_codes() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        let code = |args: &[&str]| {
            let mut v = sv(&args[..1]);
            v.extend(sv(&["--queue", &q, "--as", "wt-shape"]));
            v.extend(sv(&args[1..]));
            main(&v)
        };
        std::fs::write(
            d.path().join("todo/0001-shaped.md"),
            "---\nid: 0001\ntype: task\npriority: normal\nstage: shaping\nstatus: todo\nclaimed-by:\nheartbeat:\nwaiting_on:\n---\n\n# Shaped\n\n## Acceptance criteria\n- [ ] AC1: works — verify: command\n\n## Log\n- 2026-10-12 09:00 capture: created\n",
        )
        .unwrap();
        let two = r#"{"conflicts": [{"title": "A", "this": {"quote": "a", "at": "0001"}, "other": {"quote": "b", "at": "docs/VISION.md:3"}, "fix": "f"}, {"title": "B", "this": {"quote": "c", "at": "0001"}, "other": {"quote": "d", "at": "docs/DESIGN.md:9"}, "fix": "g"}]}"#;
        let task = || queue::read_task(d.path(), "0001").unwrap();
        // The brief, and the whole queue's inputs: read only.
        assert_eq!(code(&["vision", "0001"]), 0);
        assert_eq!(code(&["vision", "0001", "--json"]), 0);
        assert_eq!(code(&["vision", "--all"]), 0);
        assert_eq!(code(&["vision", "--all", "--json"]), 0);
        assert_eq!(task().vision_open, None);
        // Usage (2) and refusals (1).
        assert_eq!(code(&["vision"]), 2, "no id");
        assert_eq!(code(&["vision", "0001", "--all"]), 2);
        assert_eq!(code(&["vision", "0001", "extra"]), 2);
        assert_eq!(
            code(&["vision", "0001", "--record", two, "--resolve", "V1"]),
            2
        );
        assert_eq!(code(&["vision", "0009"]), 1, "no such task");
        assert_eq!(code(&["vision", "0001", "--record", "not json"]), 1);
        assert_eq!(task().vision_open, None, "a refused record writes nothing");
        // Record, then Shaping → Ready waits for the conflicts.
        assert_eq!(code(&["vision", "0001", "--record", two]), 0);
        assert_eq!((task().vision_open, task().vision_ids.len()), (Some(2), 2));
        assert_eq!(code(&["pass", "0001", "--from", "shaping"]), 1);
        assert_eq!(code(&["check"]), 0, "open conflicts aren't problems");
        assert_eq!(code(&["vision", "0001", "--resolve", "V1"]), 2, "no how");
        assert_eq!(
            code(&["vision", "0001", "--resolve", "V1", "kept", "the", "spine"]),
            0
        );
        assert_eq!(code(&["vision", "0001", "--resolve", "V1", "again"]), 1);
        assert_eq!(
            code(&[
                "vision",
                "0001",
                "--resolve",
                "v2",
                "--",
                "--by",
                "is",
                "gone"
            ]),
            0
        );
        let body = queue::read_body(d.path(), "0001").unwrap();
        assert!(
            body.contains(": vision check: resolved V2: --by is gone\n"),
            "{body}"
        );
        assert_eq!(task().vision_open, Some(0));
        assert_eq!(code(&["pass", "0001", "--from", "shaping"]), 0);
        assert_eq!(task().stage, "ready");
    }

    #[test]
    fn review_and_lessons_usage_refusals_and_exit_codes() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        let code = |args: &[&str]| {
            let mut v = sv(&args[..1]);
            v.extend(sv(&["--queue", &q, "--as", "qa"]));
            v.extend(sv(&args[1..]));
            main(&v)
        };
        std::fs::write(
            d.path().join("doing/0001-plain.md"),
            "---\nid: 0001\ntype: task\npriority: normal\nstage: testing\nstatus: doing\nclaimed-by:\nheartbeat:\nwaiting_on:\n---\n\n# Plain\n\n## Acceptance criteria\n- [x] AC1: works — verify: command\n\n## Log\n- 2026-10-12 09:00 capture: created\n",
        )
        .unwrap();
        // Usage (2): no id, two writes at once, words after a record, an unknown lessons verb.
        assert_eq!(code(&["review"]), 2);
        assert_eq!(
            code(&["review", "0001", "--fixed", "R1", "--dispute", "R1", "x"]),
            2
        );
        assert_eq!(code(&["review", "0001", "extra"]), 2);
        assert_eq!(code(&["review", "0001", "--record", "{}", "extra"]), 2);
        assert_eq!(code(&["review", "0001", "--dispute", "R1"]), 2, "no why");
        assert_eq!(
            code(&["review", "0001", "--settle", "R1", "wont-fix"]),
            2,
            "no why"
        );
        assert_eq!(code(&["lessons", "approve"]), 2);
        assert_eq!(code(&["lessons", "accept"]), 2, "which, or --all");
        assert_eq!(code(&["lessons", "reject", "L1"]), 2, "no why");
        // Refused (1): a task not started on its own branch has nothing to review.
        assert_eq!(code(&["review", "0001"]), 1);
        assert_eq!(
            code(&["review", "0001", "--record", r#"{"head":"abcdef1"}"#]),
            1
        );
        assert_eq!(code(&["review", "0009"]), 1, "no such task");
        // Lessons: listing reads; deciding is the owner's and needs something waiting.
        assert_eq!(code(&["lessons"]), 0);
        assert_eq!(code(&["lessons", "--json"]), 0);
        assert_eq!(code(&["lessons", "accept", "--all"]), 1, "not the owner");
        let mut owner = sv(&["lessons", "--queue", &q, "--as", "owner", "accept", "--all"]);
        assert_eq!(main(&owner), 1, "nothing waiting");
        owner.truncate(5);
        owner.extend(sv(&["reject", "L1", "taste"]));
        assert_eq!(main(&owner), 1, "no lesson L1");
        assert!(!d.path().join("reviews").exists(), "nothing written");
    }

    #[test]
    fn review_and_lessons_are_checked_like_a_claim_only_when_they_write() {
        let a = |args: &[&str]| parse(&sv(args)).unwrap();
        assert!(!targeted("review", &a(&["review", "0001"])));
        for w in [
            &["review", "0001", "--record", "-"][..],
            &["review", "0001", "--fixed", "R1"],
            &["review", "0001", "--dispute", "R1", "why"],
            &["review", "0001", "--settle", "R1", "open", "why"],
        ] {
            assert!(targeted("review", &a(w)), "{w:?}");
        }
        assert!(!targeted("lessons", &a(&["lessons"])));
        assert!(targeted("lessons", &a(&["lessons", "accept", "--all"])));
    }

    #[test]
    fn vision_is_checked_like_a_claim_only_when_it_writes() {
        let a = |args: &[&str]| parse(&sv(args)).unwrap();
        assert!(!targeted("vision", &a(&["vision", "0001"])));
        assert!(!targeted("vision", &a(&["vision", "--all"])));
        assert!(targeted("vision", &a(&["vision", "0001", "--record", "-"])));
        assert!(targeted(
            "vision",
            &a(&["vision", "0001", "--resolve", "V1", "how"])
        ));
        assert!(targeted("claim", &a(&["claim", "0001"])));
        // Shape claims the note (0097); --peek only reads it.
        assert!(targeted("shape", &a(&["shape", "n.md"])));
        assert!(!targeted("shape", &a(&["shape", "n.md", "--peek"])));
        assert!(a(&["vision", "--all"]).all);
    }

    #[test]
    fn stamp_shape() {
        let s = local_stamp();
        assert_eq!(s.len(), 16);
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[10..11], " ");
    }

    #[test]
    fn touches_ready_start_and_release_parse_and_say_what_they_need() {
        let a = parse(&sv(&["ready", "--parallel", "2", "--json"])).unwrap();
        assert!(a.parallel && a.json);
        assert_eq!(a.pos, ["ready", "2"]);
        let a = parse(&sv(&["start", "0007", "--here"])).unwrap();
        assert!(a.here);
        let a = parse(&sv(&["start", "0007", "--at", "../wt"])).unwrap();
        assert_eq!(a.flags.get("at").map(String::as_str), Some("../wt"));
        let a = parse(&sv(&["release", "0007", "--clean", "--force"])).unwrap();
        assert!(a.clean && a.force);
        assert!(parse(&sv(&["start", "0007", "--at"]))
            .unwrap_err()
            .contains("needs a value"));
        for cmd in ["start", "release"] {
            assert!(TARGETED.contains(&cmd), "{cmd}");
        }
        assert!(!TARGETED.contains(&"ready"));
        // touches: at new, file and set; cleaned, and refused outside the repo.
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        let code = |args: &[&str]| {
            let mut v = sv(&args[..1]);
            v.extend(sv(&["--queue", &q, "--as", "owner"]));
            v.extend(sv(&args[1..]));
            main(&v)
        };
        assert_eq!(
            code(&["new", "Panel", "--touches", "./app/src/TaskPanel*, docs/"]),
            0
        );
        let t = || queue::read_task(d.path(), "0001").unwrap();
        assert_eq!(t().touches, ["app/src/TaskPanel*", "docs/"]);
        assert_eq!(
            code(&["set", "0001", "touches", "bin/gate,bin/gate , app/"]),
            0
        );
        assert_eq!(t().touches, ["bin/gate", "app/"]);
        assert_eq!(code(&["set", "0001", "touches", "../elsewhere"]), 1);
        assert_eq!(code(&["set", "0001", "touches", "/etc"]), 1);
        assert_eq!(code(&["set", "0001", "touches", "a#b"]), 1);
        assert_eq!(code(&["set", "0001", "touches", ""]), 0);
        assert!(t().touches.is_empty());
        assert!(queue::read_body(d.path(), "0001")
            .unwrap()
            .contains("owner: touches set to bin/gate, app/"));
        // ready: exit 0 with or without a set; how many is 1 to 20, after --parallel only.
        assert_eq!(code(&["ready"]), 0);
        assert_eq!(code(&["ready", "--parallel"]), 0);
        assert_eq!(code(&["ready", "--parallel", "2", "--json"]), 0);
        assert_eq!(code(&["ready", "--parallel", "0"]), 1);
        assert_eq!(code(&["ready", "--parallel", "many"]), 1);
        assert_eq!(code(&["ready", "3"]), 2);
        assert_eq!(code(&["ready", "--parallel", "3", "4"]), 2);
        // release needs --clean; start and release need a task.
        assert_eq!(code(&["release", "0001"]), 2);
        assert_eq!(code(&["release"]), 2);
        assert_eq!(code(&["start"]), 2);
        assert_eq!(code(&["start", "0001", "--here", "--at", "x"]), 2);
        assert_eq!(code(&["release", "0001", "--clean"]), 1, "never started");
    }

    #[test]
    fn a_finding_that_recurs_becomes_a_lesson_in_the_next_start_brief_once_the_owner_accepts_it() {
        use crate::parallel::tests::{commit, fx, task};
        let f = fx();
        let run_in = |dir: &Path, args: &[&str]| {
            let a = parse(&sv(args)).unwrap();
            run_on(
                &a,
                args[0],
                &f.root,
                "pj",
                "2026-10-13 10:00",
                &no_env,
                Some(dir),
            )
        };
        let to_testing = |id: &str, who: &str| {
            let opts = ops::PassOpts {
                gate: "gate: green (built)".into(),
                summary: "A synthetic summary".into(),
                from: Some("building".into()),
                ..Default::default()
            };
            ops::pass_task(&f.root, id, who, "2026-10-13 11:00", &opts).unwrap();
        };
        let started = |id: &str| {
            let t = queue::read_task(&f.root, id).unwrap();
            started_text(
                &f.root,
                "pj",
                id,
                &crate::parallel::Started {
                    worktree: PathBuf::from(&t.worktree),
                    branch: t.branch.clone(),
                    path: t.path.clone(),
                    resumed: false,
                },
            )
        };
        let finding = r#"{"lens":"correctness","severity":"should","confidence":90,"file":"les/x.rs","line":3,"title":"Reads all of the output","scenario":"2 GB → out of memory","kind":"unbounded-output","lesson":{"rule":"Cap every external program's output.","bad":"cmd.output()","good":"take(1 MB)"}}"#;
        // The same kind of mistake, found and confirmed fixed on two tasks.
        for (n, who) in [("1", "wt-a"), ("2", "wt-b")] {
            let id = task(&f.root, &format!("Task {n}"), "les/");
            assert_eq!(run_in(&f.repo, &["start", &id, "--as", who]), Ok(0));
            let wt = PathBuf::from(queue::read_task(&f.root, &id).unwrap().worktree);
            commit(&wt, "les/x.rs", "read all\n", "build");
            to_testing(&id, who);
            let head = crate::review::span(&f.root, &id).unwrap().head;
            let json = format!(r#"{{"head":"{head}","findings":[{finding}]}}"#);
            assert_eq!(
                run_in(&f.repo, &["review", &id, "--record", &json, "--as", "qa"]),
                Ok(0)
            );
            assert_eq!(queue::read_task(&f.root, &id).unwrap().stage, "building");
            assert_eq!(
                run_in(
                    &wt,
                    &["review", &id, "--fixed", "R1", "capped", "at", "1", "MB", "--as", who]
                ),
                Ok(0)
            );
            commit(&wt, "les/x.rs", "read 1 MB\n", "cap it");
            to_testing(&id, who);
            let head = crate::review::span(&f.root, &id).unwrap().head;
            let json = format!(
                r#"{{"head":"{head}","answers":[{{"finding":"R1","accept":true,"why":"capped"}}]}}"#
            );
            assert_eq!(
                run_in(&f.repo, &["review", &id, "--record", &json, "--as", "qa"]),
                Ok(0)
            );
        }
        assert_eq!(
            lessons::waiting(&f.root),
            1,
            "proposed after the second task"
        );
        // Proposed isn't accepted: the next start in that area doesn't carry it yet.
        let c = task(&f.root, "Task 3", "les/c.rs");
        assert_eq!(run_in(&f.repo, &["start", &c, "--as", "wt-c"]), Ok(0));
        assert!(!started(&c).contains("lessons"), "{}", started(&c));
        // Only the owner accepts it, in one batch.
        assert!(run_in(&f.repo, &["lessons", "accept", "--all", "--as", "wt-c"]).is_err());
        assert_eq!(
            run_in(&f.repo, &["lessons", "accept", "--all", "--as", "owner"]),
            Ok(0)
        );
        let d = task(&f.root, "Task 4", "les/");
        assert_eq!(run_in(&f.repo, &["start", &d, "--as", "wt-d"]), Ok(0));
        let text = started(&d);
        assert!(
            text.contains("lessons (the owner accepted these from earlier reviews; follow them):\n  - L1 (unbounded-output): Cap every external program's output. Bad: cmd.output() Good: take(1 MB)\n"),
            "{text}"
        );
        // Another area's start doesn't get it.
        let e = task(&f.root, "Task 5", "elsewhere/");
        assert_eq!(run_in(&f.repo, &["start", &e, "--as", "wt-e"]), Ok(0));
        // The lesson block, not a bare "L1": the brief holds a temp path that may contain it.
        assert!(!started(&e).contains("lessons (the owner accepted these"));
    }

    #[test]
    fn start_merge_and_release_from_the_command_line() {
        use crate::parallel::tests::{commit, fx, g, green, task, to_testing};
        let f = fx();
        let id = task(&f.root, "From the CLI", "cli/");
        // Its criterion ticked with a proof (mu-78 R5): the brief lists the proof with it.
        let file = f.root.join(queue::read_task(&f.root, &id).unwrap().path);
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(
            text.contains("- [x] AC1: works — verify: command\n"),
            "{text}"
        );
        std::fs::write(
            &file,
            text.replace(
                "- [x] AC1: works — verify: command\n",
                "- [x] AC1: works — verify: command - proof: it does\n",
            ),
        )
        .unwrap();
        let run_in = |dir: &Path, args: &[&str]| {
            let a = parse(&sv(args)).unwrap();
            run_on(
                &a,
                args[0],
                &f.root,
                "pj",
                "2026-10-13 10:00",
                &no_env,
                Some(dir),
            )
        };
        assert_eq!(run_in(&f.repo, &["start", &id, "--as", "wt-a"]), Ok(0));
        let t = queue::read_task(&f.root, &id).unwrap();
        let wt = PathBuf::from(&t.worktree);
        assert!(wt.is_dir() && t.branch == "task/pj-1-from-the-cli");
        let text = started_text(
            &f.root,
            "pj",
            &id,
            &crate::parallel::Started {
                worktree: wt.clone(),
                branch: t.branch.clone(),
                path: t.path.clone(),
                resumed: false,
            },
        );
        assert!(text.starts_with(&format!(
            "started pj-1 in proj ({})\npj-1: Building (doing/0001-from-the-cli.md)\nworktree: {}\nbranch:   task/pj-1-from-the-cli\nbrief:    {}\ntouches:  cli/\ncriteria:\n  - [x] AC1: works — verify: command — proof: it does\n",
            f.root.display(),
            wt.display(),
            f.root.join("doing/0001-from-the-cli.md").display()
        )), "{text}");
        commit(&wt, "cli/x.txt", "x\n", "cli");
        to_testing(&f, &id);
        // From Testing, with no gate set for the project: refused, nothing touched.
        let a = parse(&sv(&["pass", &id, "--from", "testing"])).unwrap();
        let e = merge_cmd(&a, &f.root, "pj", &id, "qa", None, None).unwrap_err();
        assert!(
            e.contains("no gate set for proj") && e.contains("muster projects set proj gate"),
            "{e}"
        );
        let e = merge_cmd(
            &a,
            &f.root,
            "pj",
            &id,
            "qa",
            Some(("proj".into(), "  ".into())),
            None,
        )
        .unwrap_err();
        assert!(e.contains("no gate set"), "{e}");
        let with_gate = parse(&sv(&["pass", &id, "--from", "testing", "--gate", "x"])).unwrap();
        assert!(merge_cmd(
            &with_gate,
            &f.root,
            "pj",
            &id,
            "qa",
            Some(("proj".into(), "true".into())),
            None
        )
        .unwrap_err()
        .contains("leave --gate and --commit out"));
        assert_eq!(queue::read_task(&f.root, &id).unwrap().stage, "testing");
        // With the project's gate (quoted words, no shell): merged, pushed, cleaned, next named.
        let gate = format!("'{}'", green(&f)[0]);
        assert_eq!(
            merge_cmd(
                &a,
                &f.root,
                "pj",
                &id,
                "qa",
                Some(("proj".into(), gate)),
                None
            ),
            Ok(0)
        );
        let t = queue::read_task(&f.root, &id).unwrap();
        assert_eq!((t.stage.as_str(), t.worktree.as_str()), ("your-check", ""));
        assert!(!wt.exists());
        assert_eq!(
            g(&f.origin, &["rev-parse", "main"]),
            g(&f.repo, &["rev-parse", "main"])
        );
        // Release from the command line: back to Ready with the worktree gone.
        let other = task(&f.root, "Given up", "gone/");
        assert_eq!(run_in(&f.repo, &["start", &other, "--as", "wt-b"]), Ok(0));
        let owt = PathBuf::from(queue::read_task(&f.root, &other).unwrap().worktree);
        assert_eq!(
            run_in(&f.repo, &["release", &other, "--clean", "--as", "wt-b"]),
            Ok(0)
        );
        assert!(!owt.exists());
        assert_eq!(queue::read_task(&f.root, &other).unwrap().stage, "ready");
        // check lists nothing left behind, and passes.
        assert_eq!(run_in(&f.repo, &["check"]), Ok(0));
    }

    #[test]
    fn projects_says_which_queues_were_found_and_which_were_added() {
        let base = crate::testkit::tempdir().unwrap();
        let (top, _) = project(base.path(), "drover");
        let (nested, _) = project(&base.path().join("persianUtils"), "colors");
        let elsewhere = crate::testkit::tempdir().unwrap();
        let (added, _) = project(elsewhere.path(), "notes");
        let mut c = projects::Config::default();
        projects::add(&mut c, base.path(), &added).unwrap();
        c.gate
            .insert(top.to_string_lossy().to_string(), "bin/gate".into());
        c.run
            .insert(top.to_string_lossy().to_string(), "bin/run".into());
        c.run.insert(
            added.canonicalize().unwrap().to_string_lossy().to_string(),
            "swift run".into(),
        );
        c.hidden.push(nested.to_string_lossy().to_string());
        let lines: Vec<String> = projects::list(base.path(), &c)
            .iter()
            .map(project_line)
            .collect();
        let added = added.canonicalize().unwrap();
        assert_eq!(
            lines,
            vec![
                format!(
                    "drover           dr   found  {}  gate: bin/gate  run: bin/run",
                    top.display()
                ),
                format!("colors           co   found  {} (hidden)", nested.display()),
                format!(
                    "notes            no   added  {}  run: swift run",
                    added.display()
                ),
            ]
        );
    }

    #[test]
    fn the_projects_gate_is_set_checked_and_cleared() {
        let base = crate::testkit::tempdir().unwrap();
        let (q, _) = project(base.path(), "proj");
        let mut c = projects::Config::default();
        let set = |c: &mut projects::Config, args: &[&str]| {
            set_project_command(&parse(&sv(args)).unwrap(), c, base.path())
        };
        assert_eq!(
            set(
                &mut c,
                &["projects", "set", "proj", "gate", "bin/gate --fast"]
            ),
            Ok("proj: gate set to bin/gate --fast".into())
        );
        let (name, gate) = projects::gate_of(base.path(), &c, &q).unwrap();
        assert_eq!((name.as_str(), gate.as_str()), ("proj", "bin/gate --fast"));
        assert_eq!(gate_words(&name, &gate).unwrap(), ["bin/gate", "--fast"]);
        assert_eq!(projects::list(base.path(), &c)[0].gate, "bin/gate --fast");
        // Refused: unclosed quotes, a second word outside the quotes, another field, no project.
        for (args, says) in [
            (
                &["projects", "set", "proj", "gate", "\"bin/gate"][..],
                "unclosed quote",
            ),
            (
                &["projects", "set", "proj", "gate", "bin/gate", "fast"][..],
                "in quotes",
            ),
            (&["projects", "set", "proj", "name", "x"][..], "takes gate"),
            (
                &["projects", "set", "nope", "gate", "x"][..],
                "no project named",
            ),
            (&["projects", "set", "proj", "gate", "a\nb"][..], "one line"),
            (&["projects", "set", "proj"][..], "missing"),
        ] {
            let e = set(&mut c, args).unwrap_err();
            assert!(e.contains(says), "{args:?}: {e}");
        }
        assert_eq!(c.gate.len(), 1, "a refusal changes nothing");
        assert_eq!(
            set(&mut c, &["projects", "set", "proj", "gate", ""]),
            Ok("proj: gate cleared".into())
        );
        assert!(c.gate.is_empty());
        assert!(gate_words("proj", "")
            .unwrap_err()
            .contains("no gate set for proj"));
    }

    #[test]
    fn the_projects_run_command_is_set_checked_and_cleared() {
        // 0077: beside the gate, with the gate's checks; only the app's Run starts it (0079).
        let base = crate::testkit::tempdir().unwrap();
        let (q, _) = project(base.path(), "proj");
        let mut c = projects::Config::default();
        let set = |c: &mut projects::Config, args: &[&str]| {
            set_project_command(&parse(&sv(args)).unwrap(), c, base.path())
        };
        assert_eq!(
            set(
                &mut c,
                &[
                    "projects",
                    "set",
                    "proj",
                    "run",
                    "pnpm --dir app 'tauri dev'"
                ]
            ),
            Ok("proj: run set to pnpm --dir app 'tauri dev'".into())
        );
        let key = q.to_string_lossy().to_string();
        assert_eq!(
            c.run.get(&key).map(String::as_str),
            Some("pnpm --dir app 'tauri dev'")
        );
        assert!(c.gate.is_empty(), "the gate is its own");
        let long = "x".repeat(501);
        let listed = projects::list(base.path(), &c);
        assert_eq!(listed[0].run, "pnpm --dir app 'tauri dev'");
        assert_eq!(
            serde_json::to_value(&listed).unwrap()[0]["run"],
            "pnpm --dir app 'tauri dev'",
            "projects --json has it"
        );
        for (args, says) in [
            (
                &["projects", "set", "proj", "run", "'bin/run"][..],
                "the run command has an unclosed quote",
            ),
            (
                &["projects", "set", "proj", "run", "bin/run\\"][..],
                "trailing backslash",
            ),
            (
                &["projects", "set", "proj", "run", "bin/run", "fast"][..],
                "put the run command in quotes",
            ),
            (
                &["projects", "set", "nope", "run", "bin/run"][..],
                "no project named",
            ),
            (
                &["projects", "set", "proj", "run", "a\nb"][..],
                "the run command is one line",
            ),
            (
                &["projects", "set", "proj", "run", long.as_str()][..],
                "the run command is longer than 500 characters",
            ),
            (
                &["projects", "set", "proj", "run"][..],
                "the run command, in quotes",
            ),
            (
                &["projects", "set", "proj", "start", "x"][..],
                "takes gate, run or prefix",
            ),
        ] {
            let e = set(&mut c, args).unwrap_err();
            assert!(e.contains(says), "{args:?}: {e}");
        }
        assert_eq!(c.run.len(), 1, "a refusal changes nothing");
        assert_eq!(
            set(&mut c, &["projects", "set", "proj", "run", ""]),
            Ok("proj: run cleared".into())
        );
        assert!(c.run.is_empty());
    }

    #[test]
    fn list_json_carries_each_tasks_session_state_from_claude_code() {
        let d = v2_queue();
        let write = |id: &str, who: &str| {
            std::fs::write(
                d.path().join(format!("doing/{id}-t.md")),
                format!("---\nid: {id}\nstage: building\nstatus: doing\nclaimed-by: {who}\nheartbeat:\nwaiting_on:\n---\n\n# T\n"),
            )
            .unwrap();
        };
        write("0001", "wt-a (session 7b5e8856 · cc-hg)");
        write("0002", "wt-b");
        // Bare `claude` (the company account) and an account not on the list (0099).
        write("0003", "wt-c (main · session 11111111)");
        write("0004", "wt-d (main · session 22222222 · cc-ih)");
        // A fake `claude` where the CLI looks first, and the accounts' folders. Each run notes
        // the account folder it was given; every session is waiting on a prompt.
        let home = crate::testkit::tempdir().unwrap();
        let bin = home.path().join(".local/bin");
        std::fs::create_dir_all(&bin).unwrap();
        for a in [".claude", ".claude-hg", ".claude-mg", ".claude-ih"] {
            std::fs::create_dir(home.path().join(a)).unwrap();
        }
        let claude = bin.join("claude");
        std::fs::write(
            &claude,
            "#!/bin/sh\necho \"${CLAUDE_CONFIG_DIR##*/}\" >> \"$0.runs\"\nprintf '%s' '[{\"sessionId\": \"7b5e8856-c9c8\", \"cwd\": \"/Users/someone\", \"state\": \"blocked\"}, {\"sessionId\": \"11111111\", \"state\": \"blocked\"}, {\"sessionId\": \"22222222\", \"state\": \"blocked\"}]'\n",
        )
        .unwrap();
        let mut perms = std::fs::metadata(&claude).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
        std::fs::set_permissions(&claude, perms).unwrap();
        let runs = || std::fs::read_to_string(bin.join("claude.runs")).unwrap_or_default();

        // No home folder (a test's empty environment): nothing runs, every state unknown.
        let mut s = tasks_of(d.path());
        session_states(&mut s.tasks, &no_env, &|| {
            panic!("nothing to count without a home")
        });
        assert_eq!(runs(), "");
        let json = list_json(&[], &s.tasks.iter().collect::<Vec<_>>());
        assert_eq!(json["tasks"][0]["sessionState"], "unknown");

        let h = home.path().to_string_lossy().to_string();
        let env = move |k: &str| (k == "HOME").then(|| h.clone());
        let mut s = tasks_of(d.path());
        session_states(&mut s.tasks, &env, &Vec::new);
        assert_eq!(
            runs(),
            ".claude-mg\n.claude-hg\n",
            "once for each allowed account (0182): never ih or ~/.claude"
        );
        let json = list_json(&[], &s.tasks.iter().collect::<Vec<_>>());
        assert_eq!(json["tasks"][0]["sessionState"], "needs-input");
        assert_eq!(json["tasks"][0]["waitingForOwner"], true);
        for t in 1..4 {
            assert_eq!(json["tasks"][t]["sessionState"], "unknown", "task {t}");
            assert_eq!(json["tasks"][t]["waitingForOwner"], false, "task {t}");
        }
        assert!(!json.to_string().contains("someone"), "cwd isn't kept");

        // The allowlist is read from this Mac's settings under that home: with only mg on it,
        // the hg task reads unknown and nothing runs.
        let app_dir = home
            .path()
            .join("Library/Application Support/ai.sweetlemon.muster");
        std::fs::create_dir_all(&app_dir).unwrap();
        std::fs::write(app_dir.join(settings::FILE), r#"{"claudeAccounts":["mg"]}"#).unwrap();
        std::fs::remove_file(bin.join("claude.runs")).unwrap();
        let mut s = tasks_of(d.path());
        session_states(&mut s.tasks, &env, &|| {
            panic!("nothing to count with no account to read")
        });
        assert_eq!(runs(), "", "no run");
        let json = list_json(&[], &s.tasks.iter().collect::<Vec<_>>());
        assert_eq!(json["tasks"][0]["sessionState"], "unknown");
        assert_eq!(json["tasks"][0]["waitingForOwner"], false);
        std::fs::remove_file(app_dir.join(settings::FILE)).unwrap();

        // The same session holds a task in another queue too: unknown, as the app says.
        let other = v2_queue();
        std::fs::write(
            other.path().join("doing/0101-t.md"),
            "---\nid: 0101\nstage: building\nstatus: doing\nclaimed-by: wt-c (session 7b5e8856 · cc-hg)\nheartbeat:\nwaiting_on:\n---\n\n# T\n",
        )
        .unwrap();
        let mut s = tasks_of(d.path());
        session_states(&mut s.tasks, &env, &|| tasks_of(other.path()).tasks);
        let json = list_json(&[], &s.tasks.iter().collect::<Vec<_>>());
        assert_eq!(json["tasks"][0]["sessionState"], "unknown");
        assert_eq!(json["tasks"][0]["waitingForOwner"], false);
    }

    #[test]
    fn stage_and_move_refuse_a_started_task_until_its_branch_is_in_main() {
        use crate::parallel::tests::{commit, fx, g, task};
        let f = fx();
        let run_in = |args: &[&str]| {
            let a = parse(&sv(args)).unwrap();
            run_on(
                &a,
                args[0],
                &f.root,
                "pj",
                "2026-10-13 10:00",
                &no_env,
                Some(&f.repo),
            )
        };
        let id = task(&f.root, "Unmerged", "m/");
        assert_eq!(run_in(&["start", &id, "--as", "wt-a"]), Ok(0));
        let t = queue::read_task(&f.root, &id).unwrap();
        commit(Path::new(&t.worktree), "m/x.txt", "built\n", "build it");
        for args in [
            ["stage", &id, "your-check", "--as", "owner"],
            ["stage", &id, "done", "--as", "owner"],
            ["move", &id, "done", "--as", "owner"],
        ] {
            let e = run_in(&args).unwrap_err();
            assert!(
                e.contains("isn't in main")
                    && e.contains(&format!("muster pass {id} --from testing"))
                    && e.contains(&format!("muster release {id} --clean")),
                "{args:?}: {e}"
            );
        }
        assert_eq!(queue::read_task(&f.root, &id).unwrap().stage, "building");
        g(&f.repo, &["merge", "-q", "--ff-only", &t.branch]);
        assert_eq!(
            run_in(&["stage", &id, "your-check", "--as", "owner"]),
            Ok(0)
        );
        assert_eq!(run_in(&["move", &id, "done", "--as", "owner"]), Ok(0));
        assert_eq!(queue::read_task(&f.root, &id).unwrap().stage, "done");
    }
    // Task 0112: role slots from the command line.

    #[test]
    fn role_hands_out_the_lowest_free_slot_and_stamps_it() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        let a = [(
            "CLAUDE_CODE_SESSION_ID",
            "1c6af0d9-aaaa-bbbb-cccc-0123456789ab",
        )];
        let b = [(
            "CLAUDE_CODE_SESSION_ID",
            "d6419719-aaaa-bbbb-cccc-0123456789ab",
        )];
        let c = [(
            "CLAUDE_CODE_SESSION_ID",
            "0f0f0f0f-aaaa-bbbb-cccc-0123456789ab",
        )];
        let e = [(
            "CLAUDE_CODE_SESSION_ID",
            "e0e0e0e0-aaaa-bbbb-cccc-0123456789ab",
        )];
        for t in ["One", "Two", "Three", "Four", "Five"] {
            assert_eq!(code_in(&q, &[], &["new", t, "--as", "owner"]), 0);
        }
        assert_eq!(code_in(&q, &a, &["claim", "0001", "--role", "coder"]), 0);
        assert_eq!(code_in(&q, &b, &["claim", "0002", "--role", "Kaveh"]), 0);
        let held = |id: &str| queue::read_task(d.path(), id).unwrap().claimed_by;
        let one = held("0001");
        assert!(
            one.starts_with("Kaveh-1 (") && one.contains("session 1c6af0d9 · slot coder-1"),
            "{one}"
        );
        let two = held("0002");
        assert!(
            two.starts_with("Kaveh-2 (") && two.contains("session d6419719 · slot coder-2"),
            "{two}"
        );
        // The slot acts as its name or as its id; another session can't.
        assert_eq!(
            code_in(&q, &a, &["checkin", "0001", "on", "it", "--as", "Kaveh-1"]),
            0
        );
        assert_eq!(
            code_in(&q, &a, &["checkin", "0001", "still", "--as", "coder-1"]),
            0
        );
        assert_eq!(
            code_in(&q, &b, &["checkin", "0001", "mine", "--as", "Kaveh-1"]),
            1
        );
        assert_eq!(code_in(&q, &c, &["claim", "0003", "--role", "coder"]), 0);
        assert!(held("0003").starts_with("Kaveh-3 ("), "{}", held("0003"));
        // One session is one slot (R1): c holds Kaveh-3, so --role hands it Kaveh-3 again and the
        // claim is refused as any second claim is; another role's slot is refused too.
        assert_eq!(code_in(&q, &c, &["claim", "0004", "--role", "coder"]), 1);
        assert_eq!(code_in(&q, &c, &["next", "--role", "coder"]), 1);
        assert_eq!(code_in(&q, &c, &["claim", "0004", "--role", "tester"]), 1);
        assert_eq!(held("0004"), "");
        assert_eq!(held("0005"), "");
        // Refusals: beside --as or --takeover, an unknown role, another command.
        assert_eq!(
            code_in(&q, &c, &["claim", "0004", "--role", "coder", "--as", "x"]),
            2
        );
        assert_eq!(
            code_in(&q, &c, &["claim", "0004", "--role", "coder", "--takeover"]),
            2
        );
        assert_eq!(code_in(&q, &c, &["claim", "0004", "--role", "cook"]), 2);
        assert_eq!(
            code_in(&q, &c, &["checkin", "0001", "x", "--role", "coder"]),
            2
        );
        // The team and the instructions read, and write nothing to the queue.
        let before = files_under(d.path());
        assert_eq!(code_in(&q, &[], &["team"]), 0);
        assert_eq!(code_in(&q, &[], &["team", "--json"]), 0);
        assert_eq!(code_in(&q, &[], &["instructions", "coder"]), 0);
        assert_eq!(code_in(&q, &[], &["instructions", "Pouya"]), 0);
        assert_eq!(code_in(&q, &[], &["instructions", "cook"]), 2);
        assert_eq!(code_in(&q, &[], &["instructions"]), 2);
        assert_eq!(files_under(d.path()), before);
        // A renamed role: the stamps keep the slot, the team names it anew.
        let file = roles_file();
        std::fs::write(&file, "[coder]\nname = \"کاوه\"\n").unwrap();
        let roles = crate::roles::load(&file).unwrap();
        let tasks = every_task(Some(d.path()));
        let team = crate::roles::team(&roles, &tasks);
        let coder = team.iter().find(|t| t.role.id == "coder").unwrap();
        let names: Vec<(&str, usize)> = coder
            .slot_lines
            .iter()
            .map(|l| (l.name.as_str(), l.held_by.len()))
            .collect();
        assert_eq!(names, [("کاوه-1", 1), ("کاوه-2", 1), ("کاوه-3", 1)]);
        assert_eq!(
            code_in(&q, &e, &["claim", "0004", "--role", "کاوه"]),
            1,
            "all three held"
        );
        // Done frees a slot: 0002 leaves, and the next claim gets کاوه-2; c still holds کاوه-3,
        // so its claim is refused, and a session that holds nothing gets the free slot.
        assert_eq!(
            code_in(&q, &[], &["move", "0002", "done", "--as", "owner"]),
            0
        );
        assert_eq!(code_in(&q, &c, &["claim", "0004", "--role", "کاوه"]), 1);
        assert_eq!(held("0004"), "");
        assert_eq!(code_in(&q, &e, &["claim", "0004", "--role", "کاوه"]), 0);
        assert!(
            held("0004").starts_with("کاوه-2 (") && held("0004").contains("slot coder-2"),
            "{}",
            held("0004")
        );
        // A refused roles.toml refuses --role, and every other command still works, unslotted.
        std::fs::write(&file, "[coder]\ncolour = \"red\"\n").unwrap();
        assert_eq!(code_in(&q, &b, &["claim", "0005", "--role", "coder"]), 1);
        assert_eq!(code_in(&q, &c, &["team"]), 1);
        assert_eq!(
            code_in(&q, &a, &["checkin", "0001", "x", "--as", "Kaveh-1"]),
            0
        );
    }

    #[test]
    fn role_is_refused_when_every_slot_is_held() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        std::fs::create_dir_all(roles_file().parent().unwrap()).unwrap();
        std::fs::write(roles_file(), "[lead]\nslots = 1\n").unwrap();
        for t in ["One", "Two"] {
            assert_eq!(code_in(&q, &[], &["new", t, "--as", "owner"]), 0);
        }
        let a = [(
            "CLAUDE_CODE_SESSION_ID",
            "1c6af0d9-aaaa-bbbb-cccc-0123456789ab",
        )];
        let b = [(
            "CLAUDE_CODE_SESSION_ID",
            "d6419719-aaaa-bbbb-cccc-0123456789ab",
        )];
        assert_eq!(code_in(&q, &a, &["claim", "0001", "--role", "lead"]), 0);
        assert_eq!(code_in(&q, &b, &["claim", "0002", "--role", "lead"]), 1);
        assert_eq!(queue::read_task(d.path(), "0002").unwrap().claimed_by, "");
    }

    #[test]
    fn role_works_on_start_and_next_and_a_rerun_of_start_resumes_the_slot() {
        use crate::parallel::tests::{fx, task};
        let f = fx();
        let s = |k: &str| {
            (k == "CLAUDE_CODE_SESSION_ID").then(|| "1c6af0d9-aaaa-bbbb-cccc-0123456789ab".into())
        };
        let t = |k: &str| {
            (k == "CLAUDE_CODE_SESSION_ID").then(|| "d6419719-aaaa-bbbb-cccc-0123456789ab".into())
        };
        let run_in = |env: Env, args: &[&str]| {
            let a = parse(&sv(args)).unwrap();
            run_on(
                &a,
                args[0],
                &f.root,
                "pj",
                "2026-10-13 10:00",
                env,
                Some(&f.repo),
            )
        };
        let held = |id: &str| queue::read_task(&f.root, id).unwrap().claimed_by;
        let one = task(&f.root, "One", "a/");
        let two = task(&f.root, "Two", "b/");
        // The coder's brief: muster start <id> --role coder.
        assert_eq!(run_in(&s, &["start", &one, "--role", "coder"]), Ok(0));
        let first = held(&one);
        assert!(
            first.starts_with("Kaveh-1 (") && first.contains("session 1c6af0d9 · slot coder-1"),
            "{first}"
        );
        let wt = queue::read_task(&f.root, &one).unwrap().worktree;
        assert!(Path::new(&wt).is_dir(), "{wt}");
        // Run again in the same session: the same slot, so it resumes the claim and its worktree.
        assert_eq!(run_in(&s, &["start", &one, "--role", "coder"]), Ok(0));
        assert_eq!(held(&one), first);
        assert_eq!(queue::read_task(&f.root, &one).unwrap().worktree, wt);
        // Not a second task, though.
        assert!(run_in(&s, &["start", &two, "--role", "coder"])
            .unwrap_err()
            .contains("you already hold"));
        // next --role in another session takes the top Ready task as the next slot.
        assert_eq!(run_in(&t, &["next", "--role", "coder"]), Ok(0));
        let second = held(&two);
        assert!(
            second.starts_with("Kaveh-2 (") && second.contains("session d6419719 · slot coder-2"),
            "{second}"
        );
    }

    #[test]
    fn a_slot_passed_on_under_its_old_name_after_a_rename_keeps_its_slot() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        for t in ["One", "Two"] {
            assert_eq!(code_in(&q, &[], &["new", t, "--as", "owner"]), 0);
        }
        let a = [(
            "CLAUDE_CODE_SESSION_ID",
            "1c6af0d9-aaaa-bbbb-cccc-0123456789ab",
        )];
        let b = [(
            "CLAUDE_CODE_SESSION_ID",
            "d6419719-aaaa-bbbb-cccc-0123456789ab",
        )];
        assert_eq!(code_in(&q, &a, &["claim", "0001", "--role", "coder"]), 0);
        // The owner renames Kaveh; the session hands off, and its successor takes the claim over
        // as it was told, as Kaveh-1: the slot goes with it.
        std::fs::write(roles_file(), "[coder]\nname = \"کاوه\"\n").unwrap();
        assert_eq!(
            code_in(
                &q,
                &a,
                &["checkin", "0001", "half", "--handoff", "--as", "Kaveh-1"]
            ),
            0
        );
        let c = [(
            "CLAUDE_CODE_SESSION_ID",
            "0f0f0f0f-aaaa-bbbb-cccc-0123456789ab",
        )];
        assert_eq!(
            code_in(&q, &c, &["claim", "0001", "--takeover", "--as", "Kaveh-1"]),
            0
        );
        let claim = queue::read_task(d.path(), "0001").unwrap().claimed_by;
        assert!(
            claim.starts_with("Kaveh-1 (") && claim.contains("session 0f0f0f0f · slot coder-1"),
            "{claim}"
        );
        // It passes on as Kaveh-1 too.
        assert_eq!(
            code_in(
                &q,
                &c,
                &[
                    "pass",
                    "0001",
                    "--from",
                    "building",
                    "--gate",
                    "gate: green",
                    "--summary",
                    "Built",
                    "--as",
                    "Kaveh-1"
                ]
            ),
            0
        );
        let t = queue::read_task(d.path(), "0001").unwrap();
        assert_eq!(t.stage, "testing");
        assert!(
            t.held_by.starts_with("Kaveh-1 (")
                && t.held_by.contains("session 0f0f0f0f · slot coder-1"),
            "{}",
            t.held_by
        );
        // So coder-1 is still held, and the next session gets کاوه-2.
        assert_eq!(code_in(&q, &b, &["claim", "0002", "--role", "coder"]), 0);
        let two = queue::read_task(d.path(), "0002").unwrap().claimed_by;
        assert!(
            two.starts_with("کاوه-2 (") && two.contains("slot coder-2"),
            "{two}"
        );
    }

    #[test]
    fn one_session_is_one_slot_whatever_the_role_says() {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        for t in ["One", "Two", "Three"] {
            assert_eq!(code_in(&q, &[], &["new", t, "--as", "owner"]), 0);
        }
        let a = [(
            "CLAUDE_CODE_SESSION_ID",
            "1c6af0d9-aaaa-bbbb-cccc-0123456789ab",
        )];
        let held = |id: &str| queue::read_task(d.path(), id).unwrap().claimed_by;
        assert_eq!(code_in(&q, &a, &["claim", "0001", "--role", "coder"]), 0);
        // Its slot is held in this session: --role names it again, and it's refused as Kaveh-1.
        let s = roles_file();
        let roles = crate::roles::load(&s).unwrap();
        let tasks = every_task(Some(d.path()));
        let a_env = |k: &str| (k == "CLAUDE_CODE_SESSION_ID").then(|| a[0].1.to_string());
        let args = parse(&sv(&["claim", "0002", "--role", "coder"])).unwrap();
        let held_now = crate::roles::held(&roles, &tasks);
        assert_eq!(held_now.len(), 1);
        let key = held_now.values().next().unwrap()[0].task.clone();
        assert_eq!(
            role_label(&args, "claim", d.path(), &a_env, None),
            Ok(Some("Kaveh-1".into()))
        );
        let e = role_label(
            &parse(&sv(&["claim", "0002", "--role", "tester"])).unwrap(),
            "claim",
            d.path(),
            &a_env,
            None,
        )
        .unwrap_err();
        assert!(
            e.contains("Kaveh-1") && e.contains(&key) && !e.contains("see `muster help`"),
            "{e}"
        );
        // A teammate in the same session is another holder: its own slot.
        let mate = |k: &str| match k {
            "CLAUDE_CODE_SESSION_ID" => Some(a[0].1.to_string()),
            "MUSTER_AGENT" => Some("a1b2c3d4".to_string()),
            _ => None,
        };
        assert_eq!(
            role_label(&args, "claim", d.path(), &mate, None),
            Ok(Some("Kaveh-2".into()))
        );
        // Passed to Testing, the slot is still this session's: the next claim is Kaveh-1 again.
        assert_eq!(
            code_in(
                &q,
                &a,
                &[
                    "pass",
                    "0001",
                    "--from",
                    "building",
                    "--gate",
                    "gate: green",
                    "--summary",
                    "Built",
                    "--as",
                    "Kaveh-1"
                ]
            ),
            0
        );
        assert_eq!(code_in(&q, &a, &["next", "--role", "coder"]), 0);
        assert!(
            held("0002").starts_with("Kaveh-1 (") && held("0002").contains("slot coder-1"),
            "{}",
            held("0002")
        );
        // Without a session nothing tells callers apart: the lowest free slot, as before.
        assert_eq!(
            role_label(&args, "claim", d.path(), &no_env, None),
            Ok(Some("Kaveh-2".into()))
        );
    }

    const SESSION_S: [(&str, &str); 1] = [(
        "CLAUDE_CODE_SESSION_ID",
        "1c6af0d9-aaaa-bbbb-cccc-0123456789ab",
    )];
    const SESSION_T: [(&str, &str); 1] = [(
        "CLAUDE_CODE_SESSION_ID",
        "d6419719-aaaa-bbbb-cccc-0123456789ab",
    )];
    const SESSION_U: [(&str, &str); 1] = [(
        "CLAUDE_CODE_SESSION_ID",
        "0f0f0f0f-aaaa-bbbb-cccc-0123456789ab",
    )];

    /// A queue with `n` tasks the owner made, 0001 up.
    fn queue_of(n: usize) -> (crate::testkit::TestDir, String) {
        let d = v2_queue();
        let q = d.path().to_string_lossy().to_string();
        for t in ["One", "Two", "Three", "Four"].iter().take(n) {
            assert_eq!(code_in(&q, &[], &["new", t, "--as", "owner"]), 0);
        }
        (d, q)
    }

    fn pass_as(q: &str, env: &[(&str, &str)], id: &str, who: &str) -> i32 {
        code_in(
            q,
            env,
            &[
                "pass",
                id,
                "--from",
                "building",
                "--gate",
                "gate: green",
                "--summary",
                "Built",
                "--as",
                who,
            ],
        )
    }

    #[test]
    fn a_takeover_never_gives_a_session_a_second_slot() {
        let (d, q) = queue_of(3);
        let claim = |id: &str| queue::read_task(d.path(), id).unwrap().claimed_by;
        // T is Kaveh-1 on 0002; S is Kaveh-2 on 0001 and hands it off.
        assert_eq!(
            code_in(&q, &SESSION_T, &["claim", "0002", "--role", "coder"]),
            0
        );
        assert_eq!(
            code_in(&q, &SESSION_S, &["claim", "0001", "--role", "coder"]),
            0
        );
        let s_claim = claim("0001");
        assert!(s_claim.contains("slot coder-2"), "{s_claim}");
        assert_eq!(
            code_in(
                &q,
                &SESSION_S,
                &["checkin", "0001", "half", "--handoff", "--as", "Kaveh-2"]
            ),
            0
        );
        // T, still Kaveh-1 in Building, can't take Kaveh-2's claim over too.
        assert_eq!(
            code_in(
                &q,
                &SESSION_T,
                &["claim", "0001", "--takeover", "--as", "Kaveh-2"]
            ),
            1
        );
        assert_eq!(claim("0001"), s_claim);
        // Nor once its own is in Testing: one session is one slot, Building or Testing.
        assert_eq!(pass_as(&q, &SESSION_T, "0002", "Kaveh-1"), 0);
        assert_eq!(
            code_in(
                &q,
                &SESSION_T,
                &["claim", "0001", "--takeover", "--as", "Kaveh-2"]
            ),
            1
        );
        assert_eq!(claim("0001"), s_claim);
        // A session that holds no slot takes it over, as before.
        assert_eq!(
            code_in(
                &q,
                &SESSION_U,
                &["claim", "0001", "--takeover", "--as", "Kaveh-2"]
            ),
            0
        );
        assert!(claim("0001").contains("session 0f0f0f0f · slot coder-2"));
    }

    #[test]
    fn as_another_slot_name_is_refused_while_the_session_holds_one() {
        let (d, q) = queue_of(3);
        assert_eq!(
            code_in(&q, &SESSION_S, &["claim", "0001", "--role", "coder"]),
            0
        );
        // --as another slot's name: still this session's second Building task.
        assert_eq!(
            code_in(&q, &SESSION_S, &["claim", "0002", "--as", "Kaveh-2"]),
            1
        );
        assert_eq!(code_in(&q, &SESSION_S, &["next", "--as", "Kaveh-2"]), 1);
        // And a plain label beside a slot: one session holds one Building task.
        assert_eq!(
            code_in(&q, &SESSION_S, &["claim", "0002", "--as", "akbar"]),
            1
        );
        for id in ["0002", "0003"] {
            assert_eq!(queue::read_task(d.path(), id).unwrap().stage, "ready");
        }
        // The other way round: a plain label's claim, then --role.
        assert_eq!(
            code_in(&q, &SESSION_T, &["claim", "0002", "--as", "akbar"]),
            0
        );
        assert_eq!(
            code_in(&q, &SESSION_T, &["claim", "0003", "--role", "coder"]),
            1
        );
        assert_eq!(queue::read_task(d.path(), "0003").unwrap().stage, "ready");
    }

    #[test]
    fn a_slot_in_testing_keeps_its_session_from_claiming_as_another() {
        let (d, q) = queue_of(3);
        assert_eq!(
            code_in(&q, &SESSION_S, &["claim", "0001", "--role", "coder"]),
            0
        );
        assert_eq!(pass_as(&q, &SESSION_S, "0001", "Kaveh-1"), 0);
        // Nothing in Building, but Kaveh-1 is still this session's in Testing.
        assert_eq!(
            code_in(&q, &SESSION_S, &["claim", "0002", "--as", "Kaveh-2"]),
            1
        );
        assert_eq!(queue::read_task(d.path(), "0002").unwrap().stage, "ready");
        // As itself it claims, and so does a plain label.
        assert_eq!(
            code_in(&q, &SESSION_S, &["claim", "0002", "--as", "Kaveh-1"]),
            0
        );
        assert_eq!(pass_as(&q, &SESSION_S, "0002", "Kaveh-1"), 0);
        assert_eq!(
            code_in(&q, &SESSION_S, &["claim", "0003", "--as", "akbar"]),
            0
        );
    }

    #[test]
    fn an_old_slot_name_after_a_rename_acts_as_the_sessions_slot() {
        let (d, q) = queue_of(3);
        let claim = |id: &str| queue::read_task(d.path(), id).unwrap().claimed_by;
        assert_eq!(
            code_in(&q, &SESSION_S, &["claim", "0001", "--role", "coder"]),
            0
        );
        assert_eq!(pass_as(&q, &SESSION_S, "0001", "Kaveh-1"), 0);
        std::fs::write(roles_file(), "[coder]\nname = \"Kamran\"\n").unwrap();
        // S claims as it was told, as Kaveh-1: the name is gone, so it's the session's slot.
        assert_eq!(
            code_in(&q, &SESSION_S, &["claim", "0002", "--as", "Kaveh-1"]),
            0
        );
        let two = claim("0002");
        assert!(
            two.starts_with("Kaveh-1 (") && two.contains("session 1c6af0d9 · slot coder-1"),
            "{two}"
        );
        // So --role names that slot again, and it's refused as a second claim.
        assert_eq!(
            code_in(&q, &SESSION_S, &["claim", "0003", "--role", "coder"]),
            1
        );
        assert_eq!(queue::read_task(d.path(), "0003").unwrap().stage, "ready");
        // Another session gets the next free slot under the new name.
        assert_eq!(
            code_in(&q, &SESSION_T, &["claim", "0003", "--role", "coder"]),
            0
        );
        assert!(claim("0003").starts_with("Kamran-2 ("), "{}", claim("0003"));
    }

    #[test]
    fn a_send_back_keeps_the_builders_slot() {
        let (d, q) = queue_of(3);
        let claim = |id: &str| queue::read_task(d.path(), id).unwrap().claimed_by;
        assert_eq!(
            code_in(&q, &SESSION_S, &["claim", "0001", "--role", "coder"]),
            0
        );
        let built = claim("0001");
        assert_eq!(pass_as(&q, &SESSION_S, "0001", "Kaveh-1"), 0);
        // Renamed while it's in Testing, then a reviewer sends it back.
        std::fs::write(roles_file(), "[coder]\nname = \"Kamran\"\n").unwrap();
        assert_eq!(
            code_in(
                &q,
                &SESSION_U,
                &["return", "0001", "AC1", "fails", "--as", "reviewer"]
            ),
            0
        );
        let t = queue::read_task(d.path(), "0001").unwrap();
        assert_eq!(t.stage, "building");
        assert_eq!(t.claimed_by, built);
        // Its builder can't take another slot beside it.
        assert_eq!(
            code_in(&q, &SESSION_S, &["claim", "0002", "--role", "coder"]),
            1
        );
        assert_eq!(queue::read_task(d.path(), "0002").unwrap().stage, "ready");
        // And coder-1 is still held: a fresh session gets Kamran-2.
        assert_eq!(
            code_in(&q, &SESSION_T, &["claim", "0002", "--role", "coder"]),
            0
        );
        assert!(claim("0002").starts_with("Kamran-2 ("), "{}", claim("0002"));
    }

    #[test]
    fn a_send_back_without_a_slot_goes_to_the_builders_session() {
        let (d, q) = queue_of(1);
        assert_eq!(
            code_in(&q, &SESSION_S, &["claim", "0001", "--as", "akbar"]),
            0
        );
        let built = queue::read_task(d.path(), "0001").unwrap().claimed_by;
        assert!(built.starts_with("akbar ("), "{built}");
        assert_eq!(pass_as(&q, &SESSION_S, "0001", "akbar"), 0);
        assert_eq!(
            code_in(
                &q,
                &SESSION_U,
                &["return", "0001", "AC1", "fails", "--as", "reviewer"]
            ),
            0
        );
        // Its exact session (mu-184), so Ping reaches it; never the bare label.
        assert_eq!(
            queue::read_task(d.path(), "0001").unwrap().claimed_by,
            built
        );
    }

    /// Two queues Muster knows (`projects::use_places`, this thread only), `al` and `be`, with
    /// `n` tasks the owner made in each.
    fn two_queues(n: usize) -> (crate::testkit::TestDir, PathBuf, String, PathBuf, String) {
        let base = crate::testkit::tempdir().unwrap();
        let mut c = projects::Config::default();
        let mut made = Vec::new();
        for (name, prefix) in [("alpha", "al"), ("beta", "be")] {
            let root = base.path().join(name).join("tasks");
            for f in queue::STATUS_FOLDERS {
                std::fs::create_dir_all(root.join(f)).unwrap();
            }
            std::fs::write(
                root.join("README.md"),
                format!("---\nprefix: {prefix}\n---\n\nq"),
            )
            .unwrap();
            c.added.push(root.to_string_lossy().to_string());
            made.push(root);
        }
        let config_file = base.path().join("projects.json");
        projects::save(&config_file, &c).unwrap();
        projects::use_places(projects::Places {
            base: base.path().join("none"),
            config_file,
        });
        let b = made.pop().unwrap();
        let a = made.pop().unwrap();
        let (qa, qb) = (
            a.to_string_lossy().to_string(),
            b.to_string_lossy().to_string(),
        );
        for q in [&qa, &qb] {
            for t in ["One", "Two", "Three"].iter().take(n) {
                assert_eq!(code_in(q, &[], &["new", t, "--as", "owner"]), 0);
            }
        }
        (base, a, qa, b, qb)
    }

    #[test]
    fn one_session_is_one_slot_across_every_queue() {
        let (_base, a, qa, b, qb) = two_queues(2);
        assert_eq!(
            code_in(&qa, &SESSION_S, &["claim", "0001", "--role", "coder"]),
            0
        );
        // In the other queue: another slot's name, a plain label, or the same slot again.
        for who in ["Kaveh-2", "akbar", "Kaveh-1"] {
            assert_eq!(
                code_in(&qb, &SESSION_S, &["claim", "0001", "--as", who]),
                1,
                "{who}"
            );
        }
        assert_eq!(code_in(&qb, &SESSION_S, &["next", "--as", "Kaveh-2"]), 1);
        assert_eq!(queue::read_task(&b, "0001").unwrap().stage, "ready");
        // Passed to Testing in al, Kaveh-1 is still this session's: another slot is refused in
        // be, its own slot claims.
        assert_eq!(pass_as(&qa, &SESSION_S, "0001", "Kaveh-1"), 0);
        assert_eq!(
            code_in(&qb, &SESSION_S, &["claim", "0001", "--as", "Kaveh-2"]),
            1
        );
        assert_eq!(
            code_in(&qb, &SESSION_S, &["claim", "0001", "--as", "Kaveh-1"]),
            0
        );
        // Building in be now: a second Building task back in al is refused too.
        assert_eq!(
            code_in(&qa, &SESSION_S, &["claim", "0002", "--as", "Kaveh-1"]),
            1
        );
        assert_eq!(queue::read_task(&a, "0002").unwrap().stage, "ready");
        // Another session gets its own slot in either queue.
        assert_eq!(
            code_in(&qa, &SESSION_T, &["claim", "0002", "--role", "coder"]),
            0
        );
        let two = queue::read_task(&a, "0002").unwrap().claimed_by;
        assert!(
            two.starts_with("Kaveh-2 (") && two.contains("slot coder-2"),
            "{two}"
        );
    }

    #[test]
    fn a_slot_another_session_holds_is_refused_for_as() {
        let (_base, a, qa, b, qb) = two_queues(2);
        assert_eq!(
            code_in(&qa, &SESSION_S, &["claim", "0001", "--role", "coder"]),
            0
        );
        // T names S's slot, by its name or its id, in this queue or the other.
        for (q, args) in [
            (&qa, ["claim", "0002", "--as", "Kaveh-1"]),
            (&qa, ["claim", "0002", "--as", "coder-1"]),
            (&qb, ["claim", "0001", "--as", "Kaveh-1"]),
        ] {
            assert_eq!(code_in(q, &SESSION_T, &args), 1, "{q} {args:?}");
        }
        assert_eq!(code_in(&qa, &SESSION_T, &["next", "--as", "Kaveh-1"]), 1);
        assert_eq!(queue::read_task(&a, "0002").unwrap().stage, "ready");
        assert_eq!(queue::read_task(&b, "0001").unwrap().stage, "ready");
        // In Testing it's still S's.
        assert_eq!(pass_as(&qa, &SESSION_S, "0001", "Kaveh-1"), 0);
        assert_eq!(
            code_in(&qb, &SESSION_T, &["claim", "0001", "--as", "Kaveh-1"]),
            1
        );
        // A free slot by name is anyone's.
        assert_eq!(
            code_in(&qb, &SESSION_T, &["claim", "0001", "--as", "Kaveh-2"]),
            0
        );
    }

    /// How many holders slot `slot` has across the queues `every_task` reads.
    fn holders_of(slot: &str, here: &Path) -> usize {
        let roles = crate::roles::load(&roles_file()).unwrap();
        crate::roles::held(&roles, &every_task(Some(here)))
            .into_iter()
            .filter(|(s, _)| s.id() == slot)
            .map(|(_, h)| h.len())
            .sum()
    }

    #[test]
    fn a_task_returned_after_its_slot_went_to_another_session_comes_back_without_it() {
        let (d, q) = queue_of(3);
        let task = |id: &str| queue::read_task(d.path(), id).unwrap();
        let owner = [("MUSTER_AS", "owner")];
        assert_eq!(
            code_in(&q, &SESSION_S, &["claim", "0001", "--role", "coder"]),
            0
        );
        assert_eq!(pass_as(&q, &SESSION_S, "0001", "Kaveh-1"), 0);
        // Back from Your check while coder-1 is free: the tester's slot comes back with it.
        assert_eq!(code_in(&q, &owner, &["stage", "0001", "your-check"]), 0);
        assert_eq!(code_in(&q, &owner, &["return", "0001", "again"]), 0);
        let t = task("0001");
        assert_eq!(t.stage, "testing");
        assert!(
            t.held_by.contains("session 1c6af0d9 · slot coder-1"),
            "{}",
            t.held_by
        );
        // In Your check coder-1 is free, and T gets it.
        assert_eq!(code_in(&q, &owner, &["stage", "0001", "your-check"]), 0);
        assert_eq!(
            code_in(&q, &SESSION_T, &["claim", "0002", "--role", "coder"]),
            0
        );
        assert!(task("0002")
            .claimed_by
            .contains("session d6419719 · slot coder-1"));
        // Returned to Testing: S's, without the slot.
        assert_eq!(code_in(&q, &owner, &["return", "0001", "AC1", "fails"]), 0);
        let t = task("0001");
        assert_eq!(t.stage, "testing");
        assert!(
            t.held_by.starts_with("Kaveh-1 (")
                && t.held_by.contains("session 1c6af0d9")
                && !t.held_by.contains("slot "),
            "{}",
            t.held_by
        );
        assert_eq!(holders_of("coder-1", d.path()), 1);
        // On to Building, from Testing: S's claim, without the slot.
        assert_eq!(
            code_in(&q, &owner, &["return", "0001", "fix", "--to", "building"]),
            0
        );
        let t = task("0001");
        assert_eq!(t.stage, "building");
        assert!(
            t.claimed_by.contains("session 1c6af0d9") && !t.claimed_by.contains("slot "),
            "{}",
            t.claimed_by
        );
        assert_eq!(holders_of("coder-1", d.path()), 1);
        // Acting as Kaveh-1 again, S doesn't take the slot back on its pass.
        assert_eq!(
            code_in(
                &q,
                &SESSION_S,
                &["checkin", "0001", "on", "it", "--as", "Kaveh-1"]
            ),
            0
        );
        assert_eq!(pass_as(&q, &SESSION_S, "0001", "Kaveh-1"), 0);
        let t = task("0001");
        assert!(
            t.stage == "testing" && !t.held_by.contains("slot "),
            "{}",
            t.held_by
        );
        assert_eq!(holders_of("coder-1", d.path()), 1);
        // Straight from Your check to Building too.
        assert_eq!(code_in(&q, &owner, &["stage", "0001", "your-check"]), 0);
        assert_eq!(
            code_in(&q, &owner, &["return", "0001", "fix", "--to", "building"]),
            0
        );
        let t = task("0001");
        assert_eq!(t.stage, "building");
        assert!(
            t.claimed_by.contains("session 1c6af0d9") && !t.claimed_by.contains("slot "),
            "{}",
            t.claimed_by
        );
        assert_eq!(holders_of("coder-1", d.path()), 1);
    }

    #[test]
    fn a_task_back_without_its_slot_is_taken_over_without_it() {
        let (d, q) = queue_of(3);
        let task = |id: &str| queue::read_task(d.path(), id).unwrap();
        let owner = [("MUSTER_AS", "owner")];
        // S builds 0001 as Kaveh-1 (coder-1); it reaches Your check, and T gets coder-1.
        assert_eq!(
            code_in(&q, &SESSION_S, &["claim", "0001", "--role", "coder"]),
            0
        );
        assert_eq!(pass_as(&q, &SESSION_S, "0001", "Kaveh-1"), 0);
        assert_eq!(code_in(&q, &owner, &["stage", "0001", "your-check"]), 0);
        assert_eq!(
            code_in(&q, &SESSION_T, &["claim", "0002", "--role", "coder"]),
            0
        );
        // Back to Building: S's claim, without the slot. S hands off.
        assert_eq!(
            code_in(&q, &owner, &["return", "0001", "fix", "--to", "building"]),
            0
        );
        assert!(!task("0001").claimed_by.contains("slot "));
        assert_eq!(
            code_in(
                &q,
                &SESSION_S,
                &["checkin", "0001", "half", "--handoff", "--as", "Kaveh-1"]
            ),
            0
        );
        // U, its successor under Kaveh-1, takes it over without the slot T holds.
        assert_eq!(
            code_in(
                &q,
                &SESSION_U,
                &["claim", "0001", "--takeover", "--as", "Kaveh-1"]
            ),
            0
        );
        let t = task("0001");
        assert!(
            t.stage == "building"
                && t.claimed_by.starts_with("Kaveh-1 (")
                && t.claimed_by.contains("session 0f0f0f0f")
                && !t.claimed_by.contains("slot "),
            "{}",
            t.claimed_by
        );
        assert_eq!(holders_of("coder-1", d.path()), 1);
    }

    #[test]
    fn a_takeover_as_a_slot_is_refused_beside_a_plain_claim() {
        let (d, q) = queue_of(2);
        assert_eq!(
            code_in(&q, &SESSION_S, &["claim", "0001", "--role", "coder"]),
            0
        );
        assert_eq!(
            code_in(&q, &SESSION_U, &["claim", "0002", "--as", "akbar"]),
            0
        );
        std::fs::write(roles_file(), "[coder]\nname = \"Kamran\"\n").unwrap();
        assert_eq!(
            code_in(
                &q,
                &SESSION_S,
                &["checkin", "0001", "half", "--handoff", "--as", "Kaveh-1"]
            ),
            0
        );
        let before = queue::read_task(d.path(), "0001").unwrap().claimed_by;
        // U holds 0002 as akbar: taking Kaveh-1's claim over would be a slot beside it.
        assert_eq!(
            code_in(
                &q,
                &SESSION_U,
                &["claim", "0001", "--takeover", "--as", "Kaveh-1"]
            ),
            1
        );
        assert_eq!(
            queue::read_task(d.path(), "0001").unwrap().claimed_by,
            before
        );
        // A session holding nothing takes it over.
        assert_eq!(
            code_in(
                &q,
                &SESSION_T,
                &["claim", "0001", "--takeover", "--as", "Kaveh-1"]
            ),
            0
        );
    }

    #[test]
    fn an_unblocked_slot_claim_is_released_beside_a_plain_claim() {
        let (_base, a, qa, b, qb) = two_queues(3);
        let owner = [("MUSTER_AS", "owner")];
        let task = |root: &Path, id: &str| queue::read_task(root, id).unwrap();
        // S is Kaveh-1 on al-2, which waits on al-1; it then claims al-3 as a plain label.
        assert_eq!(
            code_in(&qa, &SESSION_S, &["claim", "0002", "--role", "coder"]),
            0
        );
        assert_eq!(
            code_in(
                &qa,
                &SESSION_S,
                &["block", "0002", "--on", "0001", "--as", "Kaveh-1"]
            ),
            0
        );
        assert_eq!(
            code_in(&qa, &SESSION_S, &["claim", "0003", "--as", "akbar"]),
            0
        );
        // al-1 merged: al-2 is free, but S holds al-3, so al-2 goes back to Ready.
        assert_eq!(code_in(&qa, &owner, &["stage", "0001", "your-check"]), 0);
        let two = task(&a, "0002");
        assert_eq!((two.stage.as_str(), two.claimed_by.as_str()), ("ready", ""));
        assert!(task(&a, "0003").claimed_by.starts_with("akbar ("));
        // The same across queues: Kaveh-1 on be-2 waits on be-1, a plain claim on al-1.
        assert_eq!(
            code_in(
                &qa,
                &SESSION_S,
                &[
                    "pass",
                    "0003",
                    "--from",
                    "building",
                    "--gate",
                    "gate: green",
                    "--summary",
                    "Built",
                    "--as",
                    "akbar"
                ]
            ),
            0
        );
        assert_eq!(
            code_in(&qb, &SESSION_S, &["claim", "0002", "--role", "coder"]),
            0
        );
        assert_eq!(
            code_in(
                &qb,
                &SESSION_S,
                &["block", "0002", "--on", "0001", "--as", "Kaveh-1"]
            ),
            0
        );
        assert_eq!(
            code_in(&qa, &owner, &["return", "0001", "again", "--to", "ready"]),
            0
        );
        assert_eq!(
            code_in(&qa, &SESSION_S, &["claim", "0001", "--as", "akbar"]),
            0
        );
        assert_eq!(code_in(&qb, &owner, &["stage", "0001", "your-check"]), 0);
        let two = task(&b, "0002");
        assert_eq!((two.stage.as_str(), two.claimed_by.as_str()), ("ready", ""));
    }

    #[test]
    fn restore_and_history_put_a_cleaned_queue_back_and_check_fails_until_then() {
        let base = crate::testkit::tempdir().unwrap();
        let main_repo = crate::layout::resolve::repo(base.path(), "proj");
        let q = crate::projects::init(&main_repo, true, Some("pq"), &[])
            .unwrap()
            .0;
        crate::layout::resolve::git(&main_repo, &["add", ".muster"]);
        crate::layout::resolve::git(&main_repo, &["commit", "-q", "-m", "muster"]);
        let qs = q.to_string_lossy().to_string();
        let code = |args: &[&str]| {
            let mut v = sv(&args[..1]);
            v.extend(sv(&["--queue", &qs, "--as", "owner"]));
            v.extend(sv(&args[1..]));
            main(&v)
        };
        assert_eq!(code(&["new", "Kept"]), 0);
        assert_eq!(code(&["check"]), 0);
        assert_eq!(code(&["history"]), 0);
        assert_eq!(code(&["history", "--json"]), 0);
        let task = std::fs::read_dir(q.join("tasks"))
            .unwrap()
            .flatten()
            .next()
            .unwrap()
            .path();
        let text = std::fs::read(&task).unwrap();
        crate::layout::resolve::git(&main_repo, &["clean", "-fdx"]);
        assert!(!task.exists());
        // Missing files fail the check until they're restored.
        assert_eq!(code(&["check"]), 1);
        // restore takes paths or --all, not both, not neither.
        assert_eq!(code(&["restore"]), 1);
        assert_eq!(code(&["restore", "tasks", "--all"]), 1);
        assert_eq!(code(&["restore", "--all"]), 0);
        assert_eq!(std::fs::read(&task).unwrap(), text);
        assert_eq!(code(&["check"]), 0);
        // A hand edit is a warning; history commit takes it in.
        let mut edited = text.clone();
        edited.extend_from_slice(b"\nedited by hand\n");
        std::fs::write(&task, edited).unwrap();
        assert_eq!(code(&["check"]), 0);
        assert_eq!(code(&["history", "commit"]), 0);
        assert!(crate::history::status(&q).changed.is_empty());
        assert_eq!(code(&["history", "nonsense"]), 1);
        assert_eq!(code(&["restore", "tasks/0042-none.md"]), 1);
        // A file deleted on purpose: check fails until it's restored or recorded as removed.
        std::fs::remove_file(&task).unwrap();
        assert_eq!(code(&["check"]), 1);
        assert_eq!(code(&["history", "removed"]), 1, "names files or --all");
        assert_eq!(code(&["history", "removed", "tasks", "--all"]), 1);
        assert_eq!(code(&["history", "removed", "tasks"]), 0);
        assert_eq!(code(&["check"]), 0);
        assert_eq!(
            code(&["history", "removed", "--all"]),
            0,
            "nothing left to record"
        );
        assert!(crate::history::status(&q).missing.is_empty());
        assert_eq!(code(&["restore", "--all"]), 0);
        assert!(!task.exists(), "restore --all leaves it gone");
    }
}
