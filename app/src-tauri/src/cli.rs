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
}

fn parse(argv: &[String]) -> Result<Args, String> {
    let mut a = Args {
        pos: vec![],
        flags: Default::default(),
        json: false,
    };
    let mut it = argv.iter();
    while let Some(x) = it.next() {
        match x.as_str() {
            "--json" => a.json = true,
            "--queue" | "--as" | "--type" | "--priority" | "--body" | "--status" | "--on"
            | "--tags" | "--tag" | "--time" | "--tokens" => {
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

/// Local time as `YYYY-MM-DD HH:MM`, from the system clock (`date`), so agents never type times.
pub fn local_stamp() -> String {
    std::process::Command::new("date")
        .arg("+%Y-%m-%d %H:%M")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| s.len() == 16)
        .unwrap_or_else(|| "1970-01-01 00:00".into())
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

fn resolve_queue(flag: Option<&String>) -> Result<PathBuf, String> {
    if let Some(q) = flag {
        let p = Path::new(q);
        if p.is_dir() && projects::looks_like_queue(p) {
            return Ok(p.to_path_buf());
        }
        return projects::active(&base(), &projects::load(&config_file()))
            .into_iter()
            .find(|(n, _)| n == q)
            .map(|(_, p)| p)
            .ok_or_else(|| format!("no queue named {q:?} (see `muster projects`)"));
    }
    if let Some(r) = std::env::var_os("MUSTER_ROOT").or_else(|| std::env::var_os("TASKS_ROOT")) {
        return Ok(PathBuf::from(r));
    }
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    for dir in cwd.ancestors() {
        if projects::looks_like_queue(dir) {
            return Ok(dir.to_path_buf());
        }
        let beside = dir.join("tasks");
        if projects::looks_like_queue(&beside) {
            return Ok(beside);
        }
    }
    Err("no queue here: pass --queue <name|folder> or set MUSTER_ROOT".into())
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

fn print_json<T: serde::Serialize>(v: &T) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(v).map_err(|e| e.to_string())?
    );
    Ok(())
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
            let root = resolve_queue(a.flags.get("queue"))?;
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
            let s = tasks_of(root);
            let only = a.flags.get("status");
            let tag = a.flags.get("tag");
            let mut tasks: Vec<_> = s
                .tasks
                .iter()
                .filter(|t| only.is_none_or(|o| &t.status == o))
                .filter(|t| tag.is_none_or(|g| t.tags.iter().any(|x| x == g)))
                .collect();
            tasks.sort_by(|x, y| x.id.cmp(&y.id));
            if a.json {
                print_json(&tasks)?;
            } else {
                for st in ops::status_folders() {
                    let group: Vec<_> = tasks.iter().filter(|t| t.status == *st).collect();
                    if group.is_empty() {
                        continue;
                    }
                    println!("{st} ({})", group.len());
                    for t in group {
                        let mut extra = String::new();
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
                        println!(
                            "  {} {:<6} {:<7} {}{extra}",
                            t.id, t.kind, t.priority, t.title
                        );
                    }
                }
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
            let n: u32 = q
                .trim_start_matches(['Q', 'q'])
                .parse()
                .map_err(|_| format!("{q} isn't like Q2"))?;
            let asker = queue::answer(root, id, n, &a.pos[3..].join(" "), &me, stamp)?;
            match asker {
                Some(s) => println!("{id}: answered {q}; delivered to session {s}"),
                None => println!("{id}: answered {q}; no asking session on record"),
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
    fn stamp_shape() {
        let s = local_stamp();
        assert_eq!(s.len(), 16);
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[10..11], " ");
    }
}
