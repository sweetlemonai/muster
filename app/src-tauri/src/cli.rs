//! `muster` on the command line: the same rules as the app (`ops`, `queue`, `projects`).
//! The queue is `--queue <name|folder>`, else `$MUSTER_ROOT`, else the nearest queue found
//! from the current folder (a queue itself, or a `tasks/` beside or above it).

use crate::session::{self, Session};
use crate::{
    agents, backlog, check_limit, fields, index, ops, pause, ping, projects, queue, search,
    settings, vision,
};
use std::path::{Path, PathBuf};

/// The environment a command reads its caller from (`std::env::var`, or a test's own).
type Env<'a> = &'a dyn Fn(&str) -> Option<String>;

#[derive(Debug)]
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
    /// `migrate --dry-run`: the plan, nothing written.
    dry_run: bool,
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
            "--queue" | "--as" | "--type" | "--priority" | "--body" | "--status" | "--on"
            | "--tags" | "--tag" | "--time" | "--test-time" | "--tokens" | "--stage" | "--gate"
            | "--commit" | "--to" | "--from" | "--after" | "--session" | "--level" | "--parent"
            | "--due" | "--pct" | "--remove" | "--record" | "--resolve" | "--search"
            | "--holder" | "--sort" | "--at" | "--touches" | "--prefix" | "--pick"
            | "--no-answer" => {
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

pub fn config_file() -> PathBuf {
    home().join("Library/Application Support/ai.sweetlemon.muster/projects.json")
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

pub(crate) fn base() -> PathBuf {
    std::env::var_os("MUSTER_BASE")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join("playground"))
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

/// Is this command checked like a claim (`check_target`)? The TARGETED ones, and `vision` when it
/// writes (`--record`, `--resolve`); its brief only reads.
fn targeted(cmd: &str, a: &Args) -> bool {
    TARGETED.contains(&cmd)
        || (cmd == "vision" && (a.flags.contains_key("record") || a.flags.contains_key("resolve")))
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
const TAKES_A_TASK: [&str; 27] = [
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

/// Where the task is in `a.pos` for this command, when it takes one and was given it.
fn task_arg(cmd: &str, a: &Args) -> Option<usize> {
    let takes = TAKES_A_TASK.contains(&cmd) && !(cmd == "vision" && a.all);
    (takes && a.pos.len() > 1).then_some(1)
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
            let on = if rest.is_empty() {
                joined
            } else {
                format!("{joined} {rest}")
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

/// `filed mu-31 in muster (Shaping)`: what `muster file` made, and where.
fn filed_message(root: &Path, prefix: &str, f: &backlog::Filed) -> String {
    format!(
        "filed {} in {} ({})",
        crate::key::key(prefix, &f.id),
        projects::default_name(root),
        queue::stage_label("shaping")
    )
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

/// The session asking or collecting answers: `--session`, else `$CLAUDE_CODE_SESSION_ID`.
fn session(a: &Args, env: Env) -> String {
    a.flags
        .get("session")
        .filter(|s| !s.is_empty())
        .cloned()
        .or_else(|| env("CLAUDE_CODE_SESSION_ID"))
        .unwrap_or_default()
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
/// runs in one (0030).
fn stamped_who(label: &str, env: Env, cwd: Option<&Path>) -> String {
    session::stamped(label, Session::from_env(env, cwd).as_ref())
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
/// now from the accounts the stamps in Building and Testing name, under the caller's `$HOME`.
/// What each session holds is counted across this queue and `others` (every other queue Muster
/// knows), as the app counts across all of them. Unknown without a home or a stamped holder (then
/// nothing runs, and `others` isn't read). Only this read runs it, never a write.
fn session_states(tasks: &mut [queue::Task], env: Env, others: &dyn Fn() -> Vec<queue::Task>) {
    let Some(home) = env("HOME").filter(|h| !h.is_empty()) else {
        return;
    };
    let wanted = agents::wanted(tasks);
    if wanted.is_empty() {
        return;
    }
    let snap = agents::read_all(&agents::claude_fetch(PathBuf::from(home)), &wanted);
    let others = others();
    let holds = agents::holds(tasks.iter().chain(&others));
    agents::apply_with(tasks, &holds, &snap);
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

/// `muster list` as text: the backlog notes first (file name and title), then a heading per
/// stage in STAGES order, then one line per task, by its key (0033). In two parts, cut after Your
/// check, so the over-limit line (0050), on stderr, shows under it in a terminal.
fn list_parts(notes: &[&backlog::Note], tasks: &[&queue::Task]) -> (String, String) {
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
            out.push_str(&format!("  {}  {title}\n", n.file));
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
    println!(
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
fn split_words(line: &str) -> Option<Vec<String>> {
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
/// ~/playground, or added in the app), the folder, whether it's hidden or no longer a queue, and
/// its gate.
fn project_line(p: &projects::Project) -> String {
    let state = if p.hidden {
        " (hidden)"
    } else if !p.ok {
        " (not a queue)"
    } else {
        ""
    };
    let gate = if p.gate.is_empty() {
        String::new()
    } else {
        format!("  gate: {}", p.gate)
    };
    format!(
        "{:<16} {:<4} {:<6} {}{state}{gate}",
        p.name, p.prefix, p.source, p.path
    )
}

/// `muster projects set <name> gate "<command>"` (0023), on `config`: the command must split into
/// words the way the gate will be run (no shell), one argument (quoted), empty to clear it.
/// Returns what to print.
fn set_project_gate(
    a: &Args,
    config: &mut projects::Config,
    base: &Path,
) -> Result<String, String> {
    let name = need(a, 2, "a project name")?;
    let field = need(a, 3, "the field to set (gate or prefix)")?;
    if field != "gate" {
        return Err(format!(
            "projects set takes gate or prefix, not {field:?} (see `muster help`)"
        ));
    }
    let cmd = need(a, 4, "the gate command, in quotes (\"\" clears it)")?;
    if a.pos.len() > 5 {
        return Err(
            "put the gate command in quotes: muster projects set <name> gate \"<command>\" (see `muster help`)"
                .into(),
        );
    }
    if !cmd.trim().is_empty() {
        split_words(cmd).ok_or_else(|| {
            format!("the gate command has an unclosed quote or a trailing backslash: {cmd:?}")
        })?;
    }
    projects::set_gate(config, base, name, cmd)?;
    Ok(if cmd.trim().is_empty() {
        format!("{name}: gate cleared")
    } else {
        format!("{name}: gate set to {}", cmd.trim())
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
) -> Result<i32, String> {
    let k = crate::key::key(prefix, id);
    if a.flags.contains_key("gate") || a.flags.contains_key("commit") {
        return Err(format!(
            "{k} merges when it passes from Testing: the gate runs itself, and the merge commit is recorded; leave --gate and --commit out (see `muster help`)"
        ));
    }
    let (project, gate) = gate.unwrap_or_else(|| (projects::default_name(root), String::new()));
    let argv = gate_words(&project, &gate)?;
    println!("{k}: merging into main (gate: {})", argv.join(" "));
    let mut say = |line: String| println!("{line}");
    match crate::parallel::merge(root, id, who, &local_stamp, &argv, &mut say)? {
        crate::parallel::Merge::Returned(why) => {
            println!("{k}: back in Building: {why}");
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
                println!("left behind: {n} (muster check lists it)");
            }
            println!("{}", stage_message(root, prefix, id, &path));
            println!("{k}: {report}");
            match next {
                Some((n, title)) => {
                    let n = crate::key::key(prefix, &n);
                    println!("next: {n} {title}: muster start {n}")
                }
                None => {
                    println!("next: nothing else ready that's safe beside the tasks still running")
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
    if let Ok(t) = queue::read_task(root, id) {
        if !t.touches.is_empty() {
            out.push_str(&format!("touches:  {}\n", t.touches.join(", ")));
        }
    }
    if let Ok(text) = queue::read_body(root, id) {
        let body = queue::front_matter(&text).map(|(_, b)| b).unwrap_or("");
        let items = queue::criteria_items(body);
        if !items.is_empty() {
            out.push_str("criteria:\n");
            for c in items {
                out.push_str(&format!(
                    "  - [{}] {}\n",
                    if c.ticked { "x" } else { " " },
                    c.text
                ));
            }
        }
    }
    out.push_str(&format!(
        "Work in the worktree. Check in every 30 minutes (muster checkin {k} \"…\" --pct <n>); when the gate is green: muster pass {k} --from building --gate \"<the gate's line>\"\n"
    ));
    out
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
        println!(
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
    print!("{out}");
    Ok(0)
}

/// Runs a command; returns the process exit code (0 ok, 1 refused or failed, 2 usage).
pub fn main(argv: &[String]) -> i32 {
    main_with(argv, &|k: &str| std::env::var(k).ok())
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
    let a = parse(argv)?;
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
                    Some(t) => print!("{t}"),
                    None => return Err(format!("unknown command {name:?} (see `muster help`)")),
                },
                None => print!("{}", crate::help::text()),
            }
            Ok(0)
        }
        "projects" => {
            if a.pos.get(1).map(String::as_str) == Some("set") {
                let file = config_file();
                // A prefix lives with the queue (0033), not in projects.json.
                if a.pos.get(3).map(String::as_str) == Some("prefix") {
                    println!(
                        "{}",
                        set_project_prefix(&a, &projects::load(&file), &base())?
                    );
                    return Ok(0);
                }
                let said = projects::edit(&file, |c| set_project_gate(&a, c, &base()))?;
                println!("{said}");
                return Ok(0);
            }
            let list = projects::list(&base(), &projects::load(&config_file()));
            if a.json {
                print_json(&list)?;
            } else {
                for p in &list {
                    println!("{}", project_line(p));
                }
            }
            Ok(0)
        }
        "init" => {
            let dir = Path::new(need(&a, 1, "the project folder")?);
            let local = local_or_committed(&a)?;
            let wanted = a.flags.get("prefix").map(String::as_str);
            let known = crate::key::known();
            // In a repo, its `.muster/` (0005); elsewhere, the old `<folder>/tasks`.
            let (q, prefix) = if crate::layout::main_checkout(dir).is_some() {
                projects::init(dir, local, wanted, &known)?
            } else if a.local || a.committed {
                return Err(format!(
                    "{} isn't in a git repo: --local and --committed are for a repo's .muster/",
                    dir.display()
                ));
            } else {
                projects::create(dir, &tools_dir(), wanted, &known)?
            };
            println!(
                "created {} (prefix {prefix}: its tasks are {prefix}-1, {prefix}-2…)",
                q.display()
            );
            Ok(0)
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
                println!("{line}");
            }
            // What projects.json says of the old queue (gate, name, hidden, Your check limit), said
            // of the new.
            let carried =
                carry_projects(&config_file(), &base(), Path::new(old), &r.queue, a.dry_run)?;
            for line in &carried {
                println!("{}{line}", if a.dry_run { "would: " } else { "" });
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
    // A task as it's shown (0033): `mu-12`.
    let key = |id: &str| crate::key::key(prefix, id);
    // Who's acting, worked out only by the commands that write: the label, and the label with the
    // session (the questions' commands record the session on their own).
    let label = || label(a, env, root);
    let me = || stamped_who(&label(), env, cwd);
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
                println!("{} {}", key(&c.id), c.path);
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
                let (head, tail) = list_parts(&notes, &tasks);
                print!("{head}");
                if let Some(l) = over {
                    eprintln!("{l}");
                }
                print!("{tail}");
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
                print!("{}", queue::read_body(root, id)?);
                // Who's on it and the latest check-in, which the file only has in pieces.
                if let Ok(mut t) = queue::read_task(root, id) {
                    t.prefix = prefix.to_string();
                    print!("{}", show_footer(&t));
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
            println!("{} → {p}", key(id));
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
                return merge_cmd(a, root, prefix, id, &me(), gate);
            }
            let opts = ops::PassOpts {
                gate: a.flags.get("gate").cloned().unwrap_or_default(),
                commit: a.flags.get("commit").cloned().unwrap_or_default(),
                from: a.flags.get("from").cloned(),
            };
            let p = ops::pass_task(root, id, &me(), stamp, &opts)?;
            println!("{}", stage_message(root, prefix, id, &p));
            Ok(0)
        }
        "return" => {
            let id = need(a, 1, "a task id")?;
            need(a, 2, "the reason")?;
            let why = a.pos[2..].join(" ").replace("\\n", " ");
            let to = a.flags.get("to").map(String::as_str);
            let p = ops::return_task(root, id, &why, to, &me(), stamp)?;
            println!("{}", stage_message(root, prefix, id, &p));
            Ok(0)
        }
        "block" => {
            let id = need(a, 1, "a task id")?;
            let on = a.flags.get("on").ok_or(
                "block needs --on <0014,0016|you|permission|question|outside> and what it waits on (see `muster help`)",
            )?;
            // The kind, and what it waits on: the rest of --on, then the words after the id.
            let (kind, mut detail) = crate::block::split_on(on);
            let more = a.pos.get(2..).unwrap_or_default().join(" ");
            if !more.trim().is_empty() {
                if !detail.is_empty() {
                    detail.push(' ');
                }
                detail.push_str(&more);
            }
            let p = ops::block_as(root, id, &kind, &detail, &me(), stamp)?;
            println!("{}", stage_message(root, prefix, id, &p));
            Ok(0)
        }
        "unblock" => {
            let id = need(a, 1, "a task id")?;
            let p = ops::unblock_task(root, id, &me(), stamp)?;
            println!("{}", stage_message(root, prefix, id, &p));
            Ok(0)
        }
        "pause" | "resume" => {
            let id = need(a, 1, "a task id")?;
            let p = if cmd == "pause" {
                pause::pause(root, id, &a.pos[2..].join(" "), &me(), stamp)?
            } else {
                pause::resume(root, id, &me(), stamp)?
            };
            println!("{}: {cmd}d ({})", key(id), p.path);
            if !p.session.is_empty() {
                let ask = if cmd == "pause" {
                    "to stop"
                } else {
                    "to carry on"
                };
                println!(
                    "Asked session {} {ask}: it arrives after that session's next tool call.",
                    p.session
                );
            } else if !p.unreached.is_empty() {
                println!("Its session can't be told: {}.", p.unreached);
            }
            Ok(0)
        }
        "stage" => {
            let id = need(a, 1, "a task id")?;
            let stage = need(a, 2, "a stage")?;
            let from = a.flags.get("from").map(String::as_str);
            let p = ops::set_stage(root, id, stage, from, &me(), stamp)?;
            println!("{}", stage_message(root, prefix, id, &p));
            Ok(0)
        }
        "set" => {
            let id = need(a, 1, "a task id")?;
            let field = need(a, 2, "a field")?;
            let value = a.pos[3..].join(" ");
            ops::set_field(root, id, field, &value, &me(), stamp)?;
            println!("{}: {field} set", key(id));
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
            println!("{}: noted", key(id));
            Ok(0)
        }
        "tick" | "untick" => {
            let id = need(a, 1, "a task id")?;
            let which = need(a, 2, "which criterion (AC2 or 2)")?;
            let undo = cmd == "untick";
            ops::tick_criterion(root, id, which, undo, &me(), stamp)?;
            println!(
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
                Some(p) => println!("{}: edited ({p})", key(id)),
                None => println!("{}: no change", key(id)),
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
                    println!("{}: unlinked {}", key(id), gone.url);
                }
                None => {
                    let url = need(a, 2, "an https link")?;
                    let l = fields::add_link(root, id, url, &a.pos[3..].join(" "), &me(), stamp)?;
                    println!("{}: linked {}", key(id), l.url);
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
            fields::add_comment(root, id, &text, &me(), stamp)?;
            println!("{}: commented", key(id));
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
                println!("{}: attached {saved}", key(id));
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
                Some(s) => println!("{}: answered {q}{how}; delivered to session {s}", key(id)),
                None => println!(
                    "{}: answered {q}{how}; no asking session on record",
                    key(id)
                ),
            }
            Ok(0)
        }
        "claim" => {
            let id = need(a, 1, "a task id")?;
            let p = ops::claim_task(root, id, &me(), stamp)?;
            println!("{}", claimed_message(root, prefix, id, &p));
            Ok(0)
        }
        "next" => {
            let me = me();
            warn_your_check(root, prefix);
            match ops::next_task(root, &me, stamp)? {
                Some(c) => {
                    println!("{}", claimed_message(root, prefix, &c.id, &c.path));
                    Ok(0)
                }
                None => {
                    println!(
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
            print!("{}", started_text(root, prefix, id, &s));
            Ok(0)
        }
        "ready" => ready_cmd(a, root, prefix),
        "release" => {
            let id = need(a, 1, "a task id")?;
            if !a.clean {
                return Err(format!(
                    "release takes --clean: it gives {} back to Ready and removes its worktree and branch (to send it back and keep them: muster return) (see `muster help`)",
                    key(id)
                ));
            }
            for line in crate::parallel::release(root, id, &me(), stamp, a.force, cwd)? {
                println!("{line}");
            }
            Ok(0)
        }
        "heartbeat" => {
            let id = need(a, 1, "a task id")?;
            ops::heartbeat_task(root, id, &me(), stamp)?;
            println!("{}: heartbeat {stamp}", key(id));
            Ok(0)
        }
        "checkin" => {
            let id = need(a, 1, "a task id")?;
            need(a, 2, "what's done and what's next")?;
            let text = a.pos[2..].join(" ").replace("\\n", " ");
            let pct = a.flags.get("pct").map(String::as_str).unwrap_or("");
            ops::checkin_task(root, id, &text, pct, &me(), stamp)?;
            println!("{}: checked in; heartbeat {stamp}", key(id));
            Ok(0)
        }
        "ping" => {
            let id = need(a, 1, "a task id")?;
            if a.pos.len() > 2 {
                let p = ping::message(root, id, &a.pos[2..].join(" "), &me(), stamp)?;
                println!("{}: message sent to session {}", key(id), p.session);
            } else {
                let p = ping::ask_checkin(root, id, &me(), stamp)?;
                println!("{}: asked session {} for a check-in", key(id), p.session);
            }
            println!("It arrives after that session's next tool call.");
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
                for x in &r.released {
                    println!(
                        "released {} from {} (no heartbeat for {}): back in Ready ({})",
                        key(&x.id),
                        x.holder,
                        x.quiet,
                        x.path
                    );
                }
                if r.released.is_empty() && r.failed.is_empty() {
                    let within = if after.trim().is_empty() {
                        settings::span_text(lease)
                    } else {
                        after.to_string()
                    };
                    println!("nothing to release: every claim in Building was heard from within {within}");
                }
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
            println!(
                "asked Q{} on {}. The answer will be delivered to this session.",
                asked.n,
                key(id)
            );
            if !choices.options.is_empty() {
                print!("{}", options_text(&choices));
            }
            if !asked.earlier.is_empty() {
                println!("\nQuestions on this task so far (check that yours isn't answered here):");
                for (answered, line) in &asked.earlier {
                    let tag = if *answered { "[answered]" } else { "[open]" };
                    println!("  {tag} {line}");
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
                Some(c) => print!(
                    "escalated Q{n} on {}; add it to WAITING-FOR-OWNER.md with its options:\n{}",
                    key(id),
                    options_text(&c)
                ),
                None => println!(
                    "escalated Q{n} on {}; add it to WAITING-FOR-OWNER.md",
                    key(id)
                ),
            }
            Ok(0)
        }
        "inbox" => {
            let mail = queue::take_mail(root, &session(a, env), a.keep)?;
            if mail.is_empty() {
                println!("no answers waiting");
            } else {
                println!("{}", mail.join("\n"));
            }
            Ok(0)
        }
        "add" => {
            need(a, 1, "the note (one line)")?;
            let line = a.pos[1..].join(" ");
            let body = body_arg(a)?.unwrap_or_default();
            println!("{}", backlog::add(root, &line, &body, &me(), stamp)?);
            Ok(0)
        }
        "shape" => {
            let b = backlog::shape(root, need(a, 1, "a backlog note's file name")?)?;
            if a.json {
                print_json(&b)?;
            } else {
                print!("{}", backlog::brief_text(&b));
            }
            Ok(0)
        }
        "file" => {
            let note = need(a, 1, "a backlog note's file name")?;
            need(a, 2, "the task's title")?;
            let body = body_arg(a)?
                .ok_or("file needs --body <text>|- with the shaped task (see `muster help`)")?;
            let new = new_task(a, &a.pos[2..].join(" "), body);
            let f = backlog::file_note(root, note, &new, &me(), stamp)?;
            if a.json {
                print_json(&with_key(serde_json::json!(f), key(&f.id)))?;
            } else {
                println!("{}", filed_message(root, prefix, &f));
            }
            Ok(0)
        }
        "drop" => {
            let note = need(a, 1, "a backlog note's file name")?;
            need(a, 2, "why it's dropped")?;
            let why = a.pos[2..].join(" ").replace("\\n", " ");
            let p = backlog::drop_note(root, note, &why, &me(), stamp)?;
            println!("dropped {note} ({p})");
            Ok(0)
        }
        "vision" => run_vision(a, root, prefix, &me, stamp),
        "check" => {
            // A claim is quiet past this machine's lease (0036), as `reclaim` judges it; each task
            // with its key (0033).
            let s = queue::read_state_keyed(
                &[("q".into(), root.to_path_buf())],
                &mut queue::Cache::default(),
                &machine_settings(env),
                &|_| prefix.to_string(),
            );
            for p in &s.problems {
                println!("{p}");
            }
            // Not problems either: the vision check is the shaper's job (0029).
            for w in vision::unchecked(&s.tasks) {
                println!("{w}");
            }
            // Nor what parallel sessions left behind (0023): each line says how to clean it. The
            // repo looked in is the one `check` runs in, when it's this queue's project's.
            let here: Vec<PathBuf> = cwd
                .filter(|c| queue_of(c).is_some_and(|q| same_folder(&q, root)))
                .map(|c| vec![c.to_path_buf()])
                .unwrap_or_default();
            for w in crate::parallel::stale(&s.tasks, &here) {
                println!("{w}");
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
                    println!(
                        "{}: waits on the owner to verify {}",
                        key(&t.id),
                        open.join(", ")
                    );
                }
            }
            let index_line = index_check_line(index::status(root));
            if let Some(l) = &index_line {
                println!("{l}");
            }
            // A prefix another project has too (0005: an old queue not retired yet after a
            // migrate), and where its keys go: a warning, the exit code ignores it.
            if let Some(l) = crate::key::shared_prefix(&crate::key::known(), root, prefix) {
                println!("{l}");
            }
            // Warnings (a goal past its due date, a list due a refresh) are shown but don't fail
            // the check.
            let failed = s
                .problems
                .iter()
                .chain(&index_line)
                .any(|p| !queue::is_warning(p));
            Ok(if failed { 1 } else { 0 })
        }
        "index" => {
            let path = root.join(index::FILE);
            if a.check {
                return Ok(match index::status(root) {
                    index::Status::Current => {
                        println!("{} is up to date", path.display());
                        0
                    }
                    index::Status::Stale => {
                        println!("{}", index::STALE);
                        1
                    }
                    index::Status::Refresh => {
                        println!("{}", index::REFRESH);
                        1
                    }
                    index::Status::Missing => {
                        println!("there's no INDEX.md yet (make it with `muster index`)");
                        1
                    }
                });
            }
            if index::rebuild(root)? {
                println!("rebuilt {}", path.display());
            } else {
                println!("{} is up to date", path.display());
            }
            Ok(0)
        }
        other => Err(format!("unknown command {other:?} (see `muster help`)")),
    }
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
            print!("{}", vision::all_text(&all));
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
                print!("{}", vision::recorded_text(&r));
            }
            Ok(0)
        }
        (None, Some(which)) => {
            need(a, 2, "how it was resolved")?;
            let how = a.pos[2..].join(" ").replace("\\n", " ");
            let left = vision::resolve(root, id, which, &how, &me(), stamp)?;
            println!(
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
                print!("{}", vision::brief_text(&b));
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
        let home = tempfile::tempdir().unwrap();
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

    /// `muster list` as text, whole.
    fn list_text(notes: &[&backlog::Note], tasks: &[&queue::Task]) -> String {
        let (head, tail) = list_parts(notes, tasks);
        head + &tail
    }

    /// A queue with `checking` agents' tasks in Your check, the owner's own and a feature there
    /// too (neither counts), one task Ready and one Done.
    fn checking_queue(checking: usize) -> tempfile::TempDir {
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
        let dir = tempfile::tempdir().unwrap();
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
        let (head, tail) = list_parts(&[], &all);
        assert!(head.contains("Your check (9)\n"), "{head}");
        assert!(!head.contains("Done ("), "{head}");
        assert!(tail.starts_with("Done (1)\n"), "{tail}");
        // Without Done, everything is above the line.
        let open: Vec<_> = all.iter().copied().filter(|t| t.stage != "done").collect();
        assert_eq!(list_parts(&[], &open).1, "");
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
        let d = tempfile::tempdir().unwrap();
        for f in queue::STATUS_FOLDERS {
            std::fs::create_dir(d.path().join(f)).unwrap();
        }
        std::fs::write(d.path().join("README.md"), "q").unwrap();
        let q = d.path().to_string_lossy().to_string();
        let code = |args: &[&str]| {
            let mut v = sv(args);
            v.extend(sv(&["--queue", &q, "--as", "wt-a"]));
            main(&v)
        };
        for title in ["One", "Two", "Three"] {
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
        assert_eq!(why("0001"), "Blocked on you: set the API key");
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
        assert_eq!(
            why("0001"),
            "Blocked by permission: write to ~/x/tasks (sandbox)"
        );
        assert_eq!(code(&["block", "0001", "--on", "question", "Q1"]), 0);
        assert_eq!(why("0001"), "Blocked by a question: Q1, waiting on capture");
        assert_eq!(code(&["block", "0001", "--on", "outside: Apple review"]), 0);
        assert_eq!(why("0001"), "Blocked by outside: Apple review");
        // The old words still work, as you and question.
        assert_eq!(code(&["block", "0001", "--on", "owner: which colour?"]), 0);
        assert_eq!(why("0001"), "Blocked on you: which colour?");
        assert_eq!(code(&["block", "0001", "--on", "capture", "Q1"]), 0);
        assert_eq!(why("0001"), "Blocked by a question: Q1, waiting on capture");
        assert_eq!(
            code(&["block", "0001", "--on", "capture", "Q1"]),
            1,
            "the same block"
        );
        // Task ids go into depends, with no flag.
        assert_eq!(code(&["block", "0003", "--on", "0001,0002"]), 0);
        let three = t("0003");
        assert_eq!((three.blocked, three.depends.len()), (false, 2));
        assert_eq!(why("0003"), "Blocked by tasks: 0001 (Ready), 0002 (Ready)");
        assert_eq!(
            code(&["block", "0003", "--on", "0001"]),
            1,
            "already waits on it"
        );
        assert_eq!(code(&["unblock", "0003"]), 1, "it clears by itself");
        assert_eq!(code(&["unblock", "0001"]), 0);
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
        let d = tempfile::tempdir().unwrap();
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
        let d = tempfile::tempdir().unwrap();
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
    fn stage_commands_and_their_exit_codes() {
        let d = tempfile::tempdir().unwrap();
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
                "gate full: green"
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
        let d = tempfile::tempdir().unwrap();
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
            "Backlog (1)\n  0004 code      normal  Raw\nShaping (1)\n  0003 code      normal  Shaping it  ~agent 2h · test 30m · 400k\nTesting (1)\n  0002 code      normal  Under test [Blocked by tasks: 0012]  #ui\nDone (1)\n  0001 code      normal  Shipped\n"
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
            "0002: Testing, Blocked by tasks: 0012 (blocked/0002-b.md)"
        );
        // By keys (0033), the tasks it waits on too.
        assert_eq!(
            stage_message(d.path(), "mu", "0002", "blocked/0002-b.md"),
            "mu-2: Testing, Blocked by tasks: mu-12 (blocked/0002-b.md)"
        );
        assert_eq!(
            stage_message(d.path(), "mu", "0009", "doing/0009-x.md"),
            "mu-9: changed (doing/0009-x.md)"
        );
    }

    #[test]
    fn list_searches_filters_and_sorts_like_the_app() {
        let d = tempfile::tempdir().unwrap();
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

    fn v2_queue() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
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
        let base = tempfile::tempdir().unwrap();
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
        let base = tempfile::tempdir().unwrap();
        let main_dir = crate::layout::resolve::repo(base.path(), "p");
        let m = main_dir.to_string_lossy().to_string();
        let outside = tempfile::tempdir().unwrap();
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
        let base = tempfile::tempdir().unwrap();
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
        let elsewhere = tempfile::tempdir().unwrap();
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
        // A note over 1 MB isn't read, and check says so.
        std::fs::write(d.path().join("inbox/huge.md"), vec![b'a'; 1024 * 1024 + 1]).unwrap();
        assert_eq!(code(&["check"]), 1);
        let f = crate::backlog::Filed {
            id: "0031".into(),
            path: "todo/0031-x.md".into(),
            attachment: "n.md".into(),
        };
        let base = tempfile::tempdir().unwrap();
        let (q, _) = project(base.path(), "drover");
        assert_eq!(
            filed_message(&q, "dr", &f),
            "filed dr-31 in drover (Shaping)"
        );
    }

    #[test]
    fn a_claim_says_which_queue_it_was_made_in() {
        let base = tempfile::tempdir().unwrap();
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
    fn git_queue(name: &str) -> tempfile::TempDir {
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
        assert!(!targeted("shape", &a(&["shape", "n.md"])));
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
    fn start_merge_and_release_from_the_command_line() {
        use crate::parallel::tests::{commit, fx, g, green, task, to_testing};
        let f = fx();
        let id = task(&f.root, "From the CLI", "cli/");
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
            "started pj-1 in proj ({})\npj-1: Building (doing/0001-from-the-cli.md)\nworktree: {}\nbranch:   task/pj-1-from-the-cli\nbrief:    {}\ntouches:  cli/\ncriteria:\n  - [x] AC1: works — verify: command\n",
            f.root.display(),
            wt.display(),
            f.root.join("doing/0001-from-the-cli.md").display()
        )), "{text}");
        commit(&wt, "cli/x.txt", "x\n", "cli");
        to_testing(&f, &id);
        // From Testing, with no gate set for the project: refused, nothing touched.
        let a = parse(&sv(&["pass", &id, "--from", "testing"])).unwrap();
        let e = merge_cmd(&a, &f.root, "pj", &id, "qa", None).unwrap_err();
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
            Some(("proj".into(), "true".into()))
        )
        .unwrap_err()
        .contains("leave --gate and --commit out"));
        assert_eq!(queue::read_task(&f.root, &id).unwrap().stage, "testing");
        // With the project's gate (quoted words, no shell): merged, pushed, cleaned, next named.
        let gate = format!("'{}'", green(&f)[0]);
        assert_eq!(
            merge_cmd(&a, &f.root, "pj", &id, "qa", Some(("proj".into(), gate))),
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
        let base = tempfile::tempdir().unwrap();
        let (top, _) = project(base.path(), "drover");
        let (nested, _) = project(&base.path().join("persianUtils"), "colors");
        let elsewhere = tempfile::tempdir().unwrap();
        let (added, _) = project(elsewhere.path(), "notes");
        let mut c = projects::Config::default();
        projects::add(&mut c, base.path(), &added).unwrap();
        c.gate
            .insert(top.to_string_lossy().to_string(), "bin/gate".into());
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
                    "drover           dr   found  {}  gate: bin/gate",
                    top.display()
                ),
                format!("colors           co   found  {} (hidden)", nested.display()),
                format!("notes            no   added  {}", added.display()),
            ]
        );
    }

    #[test]
    fn the_projects_gate_is_set_checked_and_cleared() {
        let base = tempfile::tempdir().unwrap();
        let (q, _) = project(base.path(), "proj");
        let mut c = projects::Config::default();
        let set = |c: &mut projects::Config, args: &[&str]| {
            set_project_gate(&parse(&sv(args)).unwrap(), c, base.path())
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
        // A fake `claude` where the CLI looks first, and the account's folder.
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join(".local/bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir(home.path().join(".claude-hg")).unwrap();
        let claude = bin.join("claude");
        std::fs::write(
            &claude,
            "#!/bin/sh\necho ran >> \"$0.runs\"\nprintf '%s' '[{\"sessionId\": \"7b5e8856-c9c8\", \"cwd\": \"/Users/someone\", \"state\": \"blocked\"}]'\n",
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
        assert_eq!(runs(), "ran\n", "once, for the one account");
        let json = list_json(&[], &s.tasks.iter().collect::<Vec<_>>());
        assert_eq!(json["tasks"][0]["sessionState"], "needs-input");
        assert_eq!(json["tasks"][0]["waitingForOwner"], true);
        assert_eq!(json["tasks"][1]["sessionState"], "unknown");
        assert!(!json.to_string().contains("someone"), "cwd isn't kept");

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
}
