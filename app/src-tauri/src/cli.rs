//! `muster` on the command line: the same rules as the app (`ops`, `queue`, `projects`).
//! The queue is `--queue <name|folder>`, else `$MUSTER_ROOT`, else the nearest queue found
//! from the current folder (a queue itself, or a `tasks/` beside or above it).

use crate::session::{self, Session};
use crate::{backlog, fields, ops, projects, queue};
use std::path::{Path, PathBuf};

/// The environment a command reads its caller from (`std::env::var`, or a test's own).
type Env<'a> = &'a dyn Fn(&str) -> Option<String>;

#[derive(Debug)]
struct Args {
    pos: Vec<String>,
    flags: std::collections::HashMap<String, String>,
    json: bool,
    keep: bool,
}

/// Options and positionals; after `--`, everything is positional (text that starts with `--`).
fn parse(argv: &[String]) -> Result<Args, String> {
    let mut a = Args {
        pos: vec![],
        flags: Default::default(),
        json: false,
        keep: false,
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
            "--queue" | "--as" | "--type" | "--priority" | "--body" | "--status" | "--on"
            | "--tags" | "--tag" | "--time" | "--test-time" | "--tokens" | "--stage" | "--gate"
            | "--commit" | "--to" | "--from" | "--after" | "--session" | "--level" | "--parent"
            | "--due" | "--pct" | "--remove" => {
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

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

fn base() -> PathBuf {
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
}

/// The queue root from the environment, if set (and not empty).
fn env_root() -> Option<(&'static str, PathBuf)> {
    ["MUSTER_ROOT", "TASKS_ROOT"].into_iter().find_map(|var| {
        std::env::var_os(var)
            .filter(|v| !v.is_empty())
            .map(|v| (var, PathBuf::from(v)))
    })
}

/// The queue a folder belongs to: the folder itself or one above it, or a `tasks/` beside it
/// or beside a folder above it (so a repo, or a worktree inside it, finds `<project>/tasks`).
fn queue_of(cwd: &Path) -> Option<PathBuf> {
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
/// happened: drover's 0022 claimed from muster), so they're checked.
const TARGETED: [&str; 13] = [
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
];

/// The same folder, through symlinks too.
fn same_folder(a: &Path, b: &Path) -> bool {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    canon(a) == canon(b)
}

/// `drover/tasks`: a queue as people name it, its project folder and its own.
fn short_queue(root: &Path) -> String {
    let name = |p: Option<&Path>| {
        p.and_then(Path::file_name)
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    };
    match name(root.parent()) {
        parent if !parent.is_empty() => format!("{parent}/{}", name(Some(root))),
        _ => root.display().to_string(),
    }
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

/// `claimed 0019 in muster (/…/muster/tasks)`, then where it is now.
fn claimed_message(root: &Path, id: &str, path: &str) -> String {
    format!(
        "claimed {id} in {} ({})\n{}",
        projects::default_name(root),
        root.display(),
        stage_message(root, id, path)
    )
}

/// `filed 0031 in muster (Shaping)`: what `muster file` made, and where.
fn filed_message(root: &Path, f: &backlog::Filed) -> String {
    format!(
        "filed {} in {} ({})",
        f.id,
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

fn tasks_of(root: &Path) -> queue::State {
    queue::read_state(
        &[("q".into(), root.to_path_buf())],
        &mut queue::Cache::default(),
    )
}

/// After a stage change: `0007: Building (doing/0007-x.md)`, with what it waits on if blocked.
/// The change is already written, so a failed re-read only shortens the message: it never
/// turns a done change into an error.
fn stage_message(root: &Path, id: &str, path: &str) -> String {
    match queue::read_task(root, id) {
        Ok(t) => {
            let blocked = if t.blocked {
                format!(", blocked: {}", t.waiting_on)
            } else {
                String::new()
            };
            format!("{id}: {}{blocked} ({path})", queue::stage_label(&t.stage))
        }
        Err(_) => format!("{id}: changed ({path})"),
    }
}

/// `muster list`'s tasks: by status, stage and tag when given, in id order.
fn list_filter<'a>(
    tasks: &'a [queue::Task],
    status: Option<&str>,
    stage: Option<&str>,
    tag: Option<&str>,
) -> Vec<&'a queue::Task> {
    let mut out: Vec<_> = tasks
        .iter()
        .filter(|t| status.is_none_or(|o| t.status == o))
        .filter(|t| stage.is_none_or(|g| t.stage == g))
        .filter(|t| tag.is_none_or(|g| t.tags.iter().any(|x| x == g)))
        .collect();
    out.sort_by(|x, y| x.id.cmp(&y.id));
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

/// `muster show`'s lines after the file: who's on it in this stage, since when, and the latest
/// check-in. Empty when there's nothing to say.
fn show_footer(t: &queue::Task) -> String {
    let mut out = String::new();
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

/// `muster list`'s backlog notes: all of them where Backlog is listed (no filter, or
/// `--stage backlog`); none under a status or tag filter, which notes don't have.
fn list_notes<'a>(
    notes: &'a [backlog::Note],
    status: Option<&str>,
    stage: Option<&str>,
    tag: Option<&str>,
) -> Vec<&'a backlog::Note> {
    if status.is_some() || tag.is_some() || stage.is_some_and(|s| s != "backlog") {
        return Vec::new();
    }
    notes.iter().collect()
}

/// `muster list --json`: the tasks, and the backlog notes beside them.
fn list_json(notes: &[&backlog::Note], tasks: &[&queue::Task]) -> serde_json::Value {
    serde_json::json!({ "tasks": tasks, "backlog": notes })
}

/// `muster list` as text: the backlog notes first (file name and title), then a heading per
/// stage in STAGES order, then one line per task.
fn list_text(notes: &[&backlog::Note], tasks: &[&queue::Task]) -> String {
    let mut out = String::new();
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
        out.push_str(&format!("{} ({})\n", ops::stage_label(st), group.len()));
        for t in group {
            let mut extra = String::new();
            if t.blocked {
                extra.push_str(&format!(" blocked: {}", t.waiting_on));
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
                "  {} {:<9} {:<7} {}{extra}\n",
                t.id, t.kind, t.priority, t.title
            ));
        }
    }
    out
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
            let list = projects::list(&base(), &projects::load(&config_file()));
            if a.json {
                print_json(&list)?;
            } else {
                for p in list {
                    let state = if p.hidden {
                        " (hidden)"
                    } else if !p.ok {
                        " (not a queue)"
                    } else {
                        ""
                    };
                    println!("{:<16} {}{state}", p.name, p.path);
                }
            }
            Ok(0)
        }
        "init" => {
            let dir = need(&a, 1, "the project folder")?;
            let tools = std::env::var_os("MUSTER_TOOLS")
                .map(PathBuf::from)
                .unwrap_or_else(|| home().join("playground/muster/main/bin"));
            let q = projects::create(Path::new(dir), &tools)?;
            println!("created {}", q.display());
            Ok(0)
        }
        _ => {
            let cwd = std::env::current_dir().ok();
            let known = || projects::active(&base(), &projects::load(&config_file()));
            let flag = a.flags.get("queue").map(String::as_str);
            let (root, source) = resolve_queue(flag, env_root(), cwd.as_deref(), &known)?;
            if TARGETED.contains(&cmd) {
                check_target(&source, &root, cwd.as_deref(), &known)?;
            }
            run_on(&a, cmd, &root, &stamp, env, cwd.as_deref())
        }
    }
}

fn run_on(
    a: &Args,
    cmd: &str,
    root: &Path,
    stamp: &str,
    env: Env,
    cwd: Option<&Path>,
) -> Result<i32, String> {
    // Who's acting, worked out only by the commands that write: the label, and the label with the
    // session (the questions' commands record the session on their own).
    let label = || label(a, env, root);
    let me = || stamped_who(&label(), env, cwd);
    match cmd {
        "new" => {
            let title = need(a, 1, "a title")?;
            // Level, parent and due (0016) are new's own; `file` takes the rest of these options.
            let new = ops::NewTask {
                level: a.flags.get("level").cloned().unwrap_or_default(),
                parent: a.flags.get("parent").cloned().unwrap_or_default(),
                due: a.flags.get("due").cloned().unwrap_or_default(),
                ..new_task(a, title, body_arg(a)?.unwrap_or_default())
            };
            let c = ops::create_task(root, &new, &me(), stamp)?;
            if a.json {
                print_json(&c)?;
            } else {
                println!("{} {}", c.id, c.path);
            }
            Ok(0)
        }
        "list" => {
            let stage = a.flags.get("stage").map(|s| s.to_lowercase());
            if let Some(s) = &stage {
                if !ops::STAGES.contains(&s.as_str()) {
                    return Err(format!("stage must be one of {}", ops::STAGES.join(", ")));
                }
            }
            let s = tasks_of(root);
            let (status, tag) = (
                a.flags.get("status").map(String::as_str),
                a.flags.get("tag").map(String::as_str),
            );
            let tasks = list_filter(&s.tasks, status, stage.as_deref(), tag);
            let notes = list_notes(&s.backlog, status, stage.as_deref(), tag);
            if a.json {
                print_json(&list_json(&notes, &tasks))?;
            } else {
                print!("{}", list_text(&notes, &tasks));
            }
            Ok(0)
        }
        "show" => {
            let id = need(a, 1, "a task id")?;
            if a.json {
                let s = tasks_of(root);
                let t = s
                    .tasks
                    .iter()
                    .find(|t| t.id == id)
                    .ok_or(format!("no task {id}"))?;
                print_json(t)?;
            } else {
                print!("{}", queue::read_body(root, id)?);
                // Who's on it and the latest check-in, which the file only has in pieces.
                if let Ok(t) = queue::read_task(root, id) {
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
            println!("{id} → {p}");
            Ok(0)
        }
        "pass" => {
            let id = need(a, 1, "a task id")?;
            let opts = ops::PassOpts {
                gate: a.flags.get("gate").cloned().unwrap_or_default(),
                commit: a.flags.get("commit").cloned().unwrap_or_default(),
                from: a.flags.get("from").cloned(),
            };
            let p = ops::pass_task(root, id, &me(), stamp, &opts)?;
            println!("{}", stage_message(root, id, &p));
            Ok(0)
        }
        "return" => {
            let id = need(a, 1, "a task id")?;
            need(a, 2, "the reason")?;
            let why = a.pos[2..].join(" ").replace("\\n", " ");
            let to = a.flags.get("to").map(String::as_str);
            let p = ops::return_task(root, id, &why, to, &me(), stamp)?;
            println!("{}", stage_message(root, id, &p));
            Ok(0)
        }
        "block" => {
            let id = need(a, 1, "a task id")?;
            let on = a
                .flags
                .get("on")
                .ok_or("block needs --on \"<task id|owner|capture> …\" (see `muster help`)")?;
            let p = ops::block_task(root, id, on, &me(), stamp)?;
            println!("{}", stage_message(root, id, &p));
            Ok(0)
        }
        "unblock" => {
            let id = need(a, 1, "a task id")?;
            let p = ops::unblock_task(root, id, &me(), stamp)?;
            println!("{}", stage_message(root, id, &p));
            Ok(0)
        }
        "stage" => {
            let id = need(a, 1, "a task id")?;
            let stage = need(a, 2, "a stage")?;
            let from = a.flags.get("from").map(String::as_str);
            let p = ops::set_stage(root, id, stage, from, &me(), stamp)?;
            println!("{}", stage_message(root, id, &p));
            Ok(0)
        }
        "set" => {
            let id = need(a, 1, "a task id")?;
            let field = need(a, 2, "a field")?;
            let value = a.pos[3..].join(" ");
            ops::set_field(root, id, field, &value, &me(), stamp)?;
            println!("{id}: {field} set");
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
            println!("{id}: noted");
            Ok(0)
        }
        "tick" | "untick" => {
            let id = need(a, 1, "a task id")?;
            let which = need(a, 2, "which criterion (AC2 or 2)")?;
            let undo = cmd == "untick";
            ops::tick_criterion(root, id, which, undo, &me(), stamp)?;
            println!("{id}: {} {which}", if undo { "unticked" } else { "ticked" });
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
                Some(p) => println!("{id}: edited ({p})"),
                None => println!("{id}: no change"),
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
                    println!("{id}: unlinked {}", gone.url);
                }
                None => {
                    let url = need(a, 2, "an https link")?;
                    let l = fields::add_link(root, id, url, &a.pos[3..].join(" "), &me(), stamp)?;
                    println!("{id}: linked {}", l.url);
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
            println!("{id}: commented");
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
                println!("{id}: attached {saved}");
            }
            Ok(0)
        }
        "answer" => {
            let id = need(a, 1, "a task id")?;
            let q = need(a, 2, "a question like Q2")?;
            let n = question_number(q)?;
            let asker = queue::answer(root, id, n, &a.pos[3..].join(" "), &label(), stamp)?;
            match asker {
                Some(s) => println!("{id}: answered {q}; delivered to session {s}"),
                None => println!("{id}: answered {q}; no asking session on record"),
            }
            Ok(0)
        }
        "claim" => {
            let id = need(a, 1, "a task id")?;
            let p = ops::claim_task(root, id, &me(), stamp)?;
            println!("{}", claimed_message(root, id, &p));
            Ok(0)
        }
        "next" => {
            let me = me();
            match ops::next_task(root, &me, stamp)? {
                Some(c) => {
                    println!("{}", claimed_message(root, &c.id, &c.path));
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
        "heartbeat" => {
            let id = need(a, 1, "a task id")?;
            ops::heartbeat_task(root, id, &me(), stamp)?;
            println!("{id}: heartbeat {stamp}");
            Ok(0)
        }
        "checkin" => {
            let id = need(a, 1, "a task id")?;
            need(a, 2, "what's done and what's next")?;
            let text = a.pos[2..].join(" ").replace("\\n", " ");
            let pct = a.flags.get("pct").map(String::as_str).unwrap_or("");
            ops::checkin_task(root, id, &text, pct, &me(), stamp)?;
            println!("{id}: checked in; heartbeat {stamp}");
            Ok(0)
        }
        "reclaim" => {
            let after = a.flags.get("after").map(String::as_str).unwrap_or("");
            let lease = ops::lease_minutes(after)?;
            let r = ops::reclaim(root, lease, &me(), stamp, queue::now_secs())?;
            if a.json {
                print_json(&r)?;
            } else {
                for x in &r.released {
                    println!(
                        "released {} from {} (no heartbeat for {}): back in Ready ({})",
                        x.id, x.holder, x.quiet, x.path
                    );
                }
                if r.released.is_empty() && r.failed.is_empty() {
                    let within = if after.trim().is_empty() {
                        ops::LEASE
                    } else {
                        after
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
            let asked = queue::ask(
                root,
                id,
                &a.pos[2..].join(" "),
                &label(),
                &session(a, env),
                stamp,
            )?;
            println!(
                "asked Q{} on {id}. The answer will be delivered to this session.",
                asked.n
            );
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
            println!("escalated Q{n} on {id}; add it to WAITING-FOR-OWNER.md");
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
                print_json(&f)?;
            } else {
                println!("{}", filed_message(root, &f));
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
        "check" => {
            let s = tasks_of(root);
            for p in &s.problems {
                println!("{p}");
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
                    println!("{}: waits on the owner to verify {}", t.id, open.join(", "));
                }
            }
            // Warnings (a goal past its due date) are shown but don't fail the check.
            let failed = s.problems.iter().any(|p| !queue::is_warning(p));
            Ok(if failed { 1 } else { 0 })
        }
        other => Err(format!("unknown command {other:?} (see `muster help`)")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(ok(&["move", "0001", "blocked", "--on", "owner"]), 0);
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
        let code = |args: &[&str]| {
            let mut v = sv(args);
            v.extend(sv(&["--queue", &q, "--as", "wt-a"]));
            main(&v)
        };
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
        assert_eq!(
            code(&["stage", "0001", "your-check", "--from", "building"]),
            0
        );
        assert!(task().waiting_for_owner);
        assert_eq!(code(&["stage", "0001", "testing"]), 0);
        assert_eq!(code(&["stage", "0001", "your-check"]), 0);
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
        let text = list_text(&none, &list_filter(&s.tasks, None, None, None));
        assert!(s.backlog.is_empty());
        assert_eq!(
            text,
            "Backlog (1)\n  0004 code      normal  Raw\nShaping (1)\n  0003 code      normal  Shaping it  ~agent 2h · test 30m · 400k\nTesting (1)\n  0002 code      normal  Under test blocked: 0012 the API  #ui\nDone (1)\n  0001 code      normal  Shipped\n"
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
            list_notes(&s.backlog, status, stage, tag)
                .iter()
                .map(|n| n.file.clone())
                .collect::<Vec<_>>()
        };
        let all = list_notes(&s.backlog, None, None, None);
        assert!(list_text(&all, &list_filter(&s.tasks, None, None, None)).starts_with(
            "Backlog notes (2)\n  2026-10-12-0900-an-idea.md  An idea\n  blank.md  (empty note)\nBacklog (1)\n"
        ));
        assert_eq!(notes(Some("backlog"), None, None).len(), 2);
        assert!(notes(Some("ready"), None, None).is_empty());
        assert!(notes(None, Some("inbox"), None).is_empty());
        assert!(notes(None, None, Some("ui")).is_empty());
        let json = list_json(&all, &list_filter(&s.tasks, None, None, None));
        assert_eq!(json["backlog"].as_array().unwrap().len(), 2);
        assert_eq!(json["tasks"].as_array().unwrap().len(), 4);
        let ids = |v: Vec<&queue::Task>| v.iter().map(|t| t.id.clone()).collect::<Vec<_>>();
        assert_eq!(
            ids(list_filter(&s.tasks, None, Some("testing"), None)),
            ["0002"]
        );
        assert_eq!(
            ids(list_filter(&s.tasks, Some("todo"), None, None)),
            ["0003"]
        );
        assert_eq!(ids(list_filter(&s.tasks, None, None, Some("ui"))), ["0002"]);
        assert!(list_filter(&s.tasks, None, Some("ready"), None).is_empty());
        // After a change: the stage, and what it waits on; a failed re-read still says it changed.
        assert_eq!(
            stage_message(d.path(), "0002", "blocked/0002-b.md"),
            "0002: Testing, blocked: 0012 the API (blocked/0002-b.md)"
        );
        assert_eq!(
            stage_message(d.path(), "0009", "doing/0009-x.md"),
            "0009: changed (doing/0009-x.md)"
        );
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
        assert_eq!(filed_message(&q, &f), "filed 0031 in drover (Shaping)");
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
            claimed_message(&q, &c.id, &p),
            format!(
                "claimed 0001 in muster ({})\n0001: Building (doing/0001-said.md)",
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
            list_text(&[], &list_filter(&s.tasks, None, None, None)),
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
    fn stamp_shape() {
        let s = local_stamp();
        assert_eq!(s.len(), 16);
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[10..11], " ");
    }
}
