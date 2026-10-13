//! `muster` on the command line: the same rules as the app (`ops`, `queue`, `projects`).
//! The queue is `--queue <name|folder>`, else `$MUSTER_ROOT`, else the nearest queue found
//! from the current folder (a queue itself, or a `tasks/` beside or above it).

use crate::{ops, projects, queue};
use std::path::{Path, PathBuf};

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
            | "--tags" | "--tag" | "--time" | "--tokens" | "--stage" | "--gate" | "--commit"
            | "--to" | "--from" | "--after" | "--session" => {
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

/// Commands that claim, hand on or hold a task, or rewrite its write-up. A slip of the environment
/// would do them in another project's queue (it happened: drover's 0022 claimed from muster), so
/// they're checked.
const TARGETED: [&str; 9] = [
    "claim",
    "next",
    "pass",
    "return",
    "block",
    "unblock",
    "stage",
    "heartbeat",
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
fn session(a: &Args) -> String {
    a.flags
        .get("session")
        .filter(|s| !s.is_empty())
        .cloned()
        .or_else(|| std::env::var("CLAUDE_CODE_SESSION_ID").ok())
        .unwrap_or_default()
}

fn who(a: &Args) -> String {
    a.flags
        .get("as")
        .cloned()
        .or_else(|| std::env::var("MUSTER_AS").ok())
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_else(|| "cli".into())
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

/// `muster list` as text: a heading per stage in STAGES order, then one line per task.
fn list_text(tasks: &[&queue::Task]) -> String {
    let mut out = String::new();
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
            if !t.est_time.is_empty() || !t.est_tokens.is_empty() {
                extra.push_str(&format!(
                    "  ~{}",
                    [t.est_time.as_str(), t.est_tokens.as_str()]
                        .iter()
                        .filter(|x| !x.is_empty())
                        .copied()
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
            if !t.tags.is_empty() {
                extra.push_str(&format!("  #{}", t.tags.join(" #")));
            }
            if t.open_questions > 0 {
                extra.push_str(&format!("  [{} open Q]", t.open_questions));
            }
            out.push_str(&format!(
                "  {} {:<6} {:<7} {}{extra}\n",
                t.id, t.kind, t.priority, t.title
            ));
        }
    }
    out
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
    match run(argv) {
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

fn run(argv: &[String]) -> Result<i32, String> {
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
            run_on(&a, cmd, &root, &stamp)
        }
    }
}

fn run_on(a: &Args, cmd: &str, root: &Path, stamp: &str) -> Result<i32, String> {
    let me = who(a);
    match cmd {
        "new" => {
            let title = need(a, 1, "a title")?;
            let body = match a.flags.get("body").map(String::as_str) {
                Some("-") => {
                    let mut s = String::new();
                    std::io::Read::read_to_string(
                        &mut std::io::Read::take(std::io::stdin(), 1 << 20),
                        &mut s,
                    )
                    .map_err(|e| e.to_string())?;
                    s
                }
                // A one-line command can carry line breaks as `\n` (the helper writes them so, because
                // a permission rule only matches a command on one line).
                Some(b) => b.replace("\\n", "\n"),
                None => String::new(),
            };
            let new = ops::NewTask {
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
                est_tokens: a.flags.get("tokens").cloned().unwrap_or_default(),
            };
            let c = ops::create_task(root, &new, &me, stamp)?;
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
            let tasks = list_filter(
                &s.tasks,
                a.flags.get("status").map(String::as_str),
                stage.as_deref(),
                a.flags.get("tag").map(String::as_str),
            );
            if a.json {
                print_json(&tasks)?;
            } else {
                print!("{}", list_text(&tasks));
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
                &me,
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
            let p = ops::pass_task(root, id, &me, stamp, &opts)?;
            println!("{}", stage_message(root, id, &p));
            Ok(0)
        }
        "return" => {
            let id = need(a, 1, "a task id")?;
            need(a, 2, "the reason")?;
            let why = a.pos[2..].join(" ").replace("\\n", " ");
            let to = a.flags.get("to").map(String::as_str);
            let p = ops::return_task(root, id, &why, to, &me, stamp)?;
            println!("{}", stage_message(root, id, &p));
            Ok(0)
        }
        "block" => {
            let id = need(a, 1, "a task id")?;
            let on = a
                .flags
                .get("on")
                .ok_or("block needs --on \"<task id|owner|capture> …\" (see `muster help`)")?;
            let p = ops::block_task(root, id, on, &me, stamp)?;
            println!("{}", stage_message(root, id, &p));
            Ok(0)
        }
        "unblock" => {
            let id = need(a, 1, "a task id")?;
            let p = ops::unblock_task(root, id, &me, stamp)?;
            println!("{}", stage_message(root, id, &p));
            Ok(0)
        }
        "stage" => {
            let id = need(a, 1, "a task id")?;
            let stage = need(a, 2, "a stage")?;
            let p = ops::set_stage(root, id, stage, &me, stamp)?;
            println!("{}", stage_message(root, id, &p));
            Ok(0)
        }
        "set" => {
            let id = need(a, 1, "a task id")?;
            let field = need(a, 2, "a field")?;
            let value = a.pos[3..].join(" ");
            ops::set_field(root, id, field, &value, &me, stamp)?;
            println!("{id}: {field} set");
            Ok(0)
        }
        "note" => {
            let id = need(a, 1, "a task id")?;
            ops::add_note(
                root,
                id,
                &a.pos[2..].join(" ").replace("\\n", " "),
                &me,
                stamp,
            )?;
            println!("{id}: noted");
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
        "attach" => {
            let id = need(a, 1, "a task id")?;
            need(a, 2, "a file")?;
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
            let asker = queue::answer(root, id, n, &a.pos[3..].join(" "), &me, stamp)?;
            match asker {
                Some(s) => println!("{id}: answered {q}; delivered to session {s}"),
                None => println!("{id}: answered {q}; no asking session on record"),
            }
            Ok(0)
        }
        "claim" => {
            let id = need(a, 1, "a task id")?;
            let p = ops::claim_task(root, id, &me, stamp)?;
            println!("{}", claimed_message(root, id, &p));
            Ok(0)
        }
        "next" => match ops::next_task(root, &me, stamp)? {
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
        },
        "heartbeat" => {
            let id = need(a, 1, "a task id")?;
            ops::heartbeat_task(root, id, &me, stamp)?;
            println!("{id}: heartbeat {stamp}");
            Ok(0)
        }
        "reclaim" => {
            let after = a.flags.get("after").map(String::as_str).unwrap_or("");
            let lease = ops::lease_minutes(after)?;
            let r = ops::reclaim(root, lease, &me, stamp, queue::now_secs())?;
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
            let asked = queue::ask(root, id, &a.pos[2..].join(" "), &me, &session(a), stamp)?;
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
            queue::escalate(root, id, n, &me, stamp)?;
            println!("escalated Q{n} on {id}; add it to WAITING-FOR-OWNER.md");
            Ok(0)
        }
        "inbox" => {
            let mail = queue::take_mail(root, &session(a), a.keep)?;
            if mail.is_empty() {
                println!("no answers waiting");
            } else {
                println!("{}", mail.join("\n"));
            }
            Ok(0)
        }
        "check" => {
            let s = tasks_of(root);
            for p in &s.problems {
                println!("{p}");
            }
            Ok(if s.problems.is_empty() { 0 } else { 1 })
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
        assert_eq!(code(&["stage", "0001", "your-check"]), 0);
        assert!(task().waiting_for_owner);
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
            "status: todo\nstage: shaping",
            "Shaping it",
        );
        file("inbox/0004-d.md", "0004", "status: inbox", "Raw");
        let s = tasks_of(d.path());
        let text = list_text(&list_filter(&s.tasks, None, None, None));
        assert_eq!(
            text,
            "Backlog (1)\n  0004 task   normal  Raw\nShaping (1)\n  0003 task   normal  Shaping it\nTesting (1)\n  0002 task   normal  Under test blocked: 0012 the API  #ui\nDone (1)\n  0001 task   normal  Shipped\n"
        );
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
            "edit",
        ] {
            assert!(TARGETED.contains(&cmd), "{cmd}");
        }
        for cmd in ["list", "show", "check", "projects", "help"] {
            assert!(!TARGETED.contains(&cmd), "{cmd}");
        }
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
    fn stamp_shape() {
        let s = local_stamp();
        assert_eq!(s.len(), 16);
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[10..11], " ");
    }
}
