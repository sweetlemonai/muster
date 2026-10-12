//! Reading Muster queues and writing the owner's answers (file contract v1, docs/PROTOCOL.md).
//!
//! A queue is a folder of task files `NNNN-slug.md` with front matter. Two layouts are read:
//! v2, where the subfolder is the status (`inbox/ todo/ doing/ blocked/ done/`), and the flat
//! layout, where files sit in the queue folder and `status:` alone says where they are. Task
//! text is untrusted data: it's parsed, never executed, and sizes are capped.

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const STATUS_FOLDERS: [&str; 5] = ["inbox", "todo", "doing", "blocked", "done"];
const MAX_TASK_BYTES: u64 = 1024 * 1024;
const MAX_ANSWER_CHARS: usize = 4000;
const MAX_IMAGE_BYTES: u64 = 15 * 1024 * 1024;
const STALE_SECS: u64 = 12 * 3600;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    pub n: Option<u32>,
    pub ticked: bool,
    pub asker: String,
    pub text: String,
    pub escalated: bool,
    pub owner_answered: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub queue: String,
    pub id: String,
    pub title: String,
    pub kind: String,
    pub priority: String,
    pub status: String,
    pub folder: String,
    pub path: String,
    pub claimed_by: String,
    pub heartbeat: String,
    pub waiting_on: String,
    pub commit: String,
    pub captured: String,
    pub questions: Vec<Question>,
    pub open_questions: usize,
    pub waiting_for_owner: bool,
    pub tags: Vec<String>,
    /// Estimates ("2h", "400k"), and the same as numbers for totals.
    pub est_time: String,
    pub est_tokens: String,
    pub est_minutes: u64,
    pub est_token_count: u64,
    /// From the log: the day work started (first "claimed" line) and the day it was done.
    pub claimed_on: String,
    pub done_on: String,
    pub attachments: Vec<String>,
    pub log_tail: Vec<String>,
    pub modified: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub queue: String,
    pub at: u64,
    pub what: String,
    pub source: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QueueInfo {
    pub name: String,
    pub path: String,
    pub counts: HashMap<String, usize>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub queues: Vec<QueueInfo>,
    pub tasks: Vec<Task>,
    pub problems: Vec<String>,
    pub activity: Vec<Activity>,
    pub now: u64,
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Queues under `<base>/*/tasks` that look like Muster queues (a README plus a status
/// folder or a manifest). The name is the project folder's name.
pub fn discover(base: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(base) else {
        return out;
    };
    for e in entries.flatten() {
        let q = e.path().join("tasks");
        let looks = q.join("README.md").is_file()
            && (q.join("todo").is_dir() || q.join("tasks.json").is_file());
        if looks {
            let name = e.file_name().to_string_lossy().to_string();
            out.push((name, q));
        }
    }
    out.sort();
    out
}

/// Every task file of a queue: (folder it's in, path). The folder is "" for the flat layout.
pub fn task_files(root: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let mut scan = |folder: &str, dir: PathBuf| {
        if let Ok(rd) = fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                let name = e.file_name().to_string_lossy().to_string();
                if p.is_file() && is_task_name(&name) {
                    out.push((folder.to_string(), p));
                }
            }
        }
    };
    scan("", root.to_path_buf());
    for f in STATUS_FOLDERS {
        scan(f, root.join(f));
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

fn is_task_name(name: &str) -> bool {
    let b = name.as_bytes();
    name.len() > 8 && b[..4].iter().all(u8::is_ascii_digit) && b[4] == b'-' && name.ends_with(".md")
}

pub fn front_matter(text: &str) -> Option<(HashMap<String, String>, &str)> {
    let rest = text.strip_prefix("---\n")?;
    let end = rest.find("\n---\n")?;
    let mut fields = HashMap::new();
    for line in rest[..end].lines() {
        if line.starts_with(' ') {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            let v = v.split("  #").next().unwrap_or("").trim();
            fields.insert(k.trim().to_string(), v.to_string());
        }
    }
    Some((fields, &rest[end + 5..]))
}

/// The `## name` section's text (up to the next `## `), if present.
fn section<'a>(body: &'a str, name: &str) -> Option<&'a str> {
    let head = format!("## {name}");
    let mut start = None;
    let mut pos = 0;
    for line in body.split_inclusive('\n') {
        if start.is_none() && line.trim_end() == head {
            start = Some(pos + line.len());
        } else if start.is_some() && line.starts_with("## ") {
            return Some(&body[start.unwrap()..pos]);
        }
        pos += line.len();
    }
    start.map(|s| &body[s..])
}

/// Question items in a Questions section: each starts at `- [ ] **Q` or `- [x] **Q`.
pub fn question_items(section: &str) -> Vec<(usize, usize)> {
    let mut starts = Vec::new();
    let mut pos = 0;
    for line in section.split_inclusive('\n') {
        if line.starts_with("- [ ] **Q") || line.starts_with("- [x] **Q") {
            starts.push(pos);
        }
        pos += line.len();
    }
    let mut spans = Vec::new();
    for (i, &s) in starts.iter().enumerate() {
        let e = starts.get(i + 1).copied().unwrap_or(section.len());
        let trimmed = section[s..e].trim_end_matches('\n').len();
        spans.push((s, s + trimmed));
    }
    spans
}

fn parse_question(item: &str) -> Question {
    let ticked = item.starts_with("- [x]");
    let after = &item[9..];
    let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
    let n = digits.parse().ok();
    let rest = &after[digits.len()..];
    let asker = rest
        .strip_prefix(" (")
        .and_then(|r| r.split_once(')'))
        .map(|(a, _)| a.to_string())
        .unwrap_or_default();
    let text = match item.find(":**") {
        Some(i) => item[i + 3..].trim().to_string(),
        None => item.to_string(),
    };
    Question {
        n,
        ticked,
        asker,
        text,
        escalated: item.contains("Escalated to owner"),
        owner_answered: item.contains("**A (owner"),
    }
}

pub fn parse_task(
    queue: &str,
    root: &Path,
    folder: &str,
    path: &Path,
    text: &str,
    modified: u64,
) -> Option<Task> {
    let (fm, body) = front_matter(text)?;
    let get = |k: &str| fm.get(k).cloned().unwrap_or_default();
    let title = body
        .lines()
        .find_map(|l| l.strip_prefix("# "))
        .unwrap_or("")
        .trim()
        .to_string();
    let questions: Vec<Question> = section(body, "Questions")
        .map(|s| {
            question_items(s)
                .into_iter()
                .map(|(a, b)| parse_question(&s[a..b]))
                .collect()
        })
        .unwrap_or_default();
    let log_tail: Vec<String> = section(body, "Log")
        .map(|s| {
            s.lines()
                .filter(|l| l.starts_with("- "))
                .map(|l| l[2..].to_string())
                .collect::<Vec<_>>()
        })
        .map(|v| v[v.len().saturating_sub(3)..].to_vec())
        .unwrap_or_default();
    let log_lines: Vec<&str> = section(body, "Log")
        .map(|s| s.lines().filter(|l| l.starts_with("- ")).collect())
        .unwrap_or_default();
    let day_of = |l: &str| -> Option<String> {
        let d = l.get(2..12)?;
        let b = d.as_bytes();
        (b.len() == 10
            && b[4] == b'-'
            && b[7] == b'-'
            && d.bytes()
                .enumerate()
                .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit()))
        .then(|| d.to_string())
    };
    let claimed_on = log_lines
        .iter()
        .find(|l| l.to_lowercase().contains("claimed"))
        .and_then(|l| day_of(l))
        .unwrap_or_default();
    let id = get("id");
    let mut status = get("status");
    if status.is_empty() {
        status = folder.to_string();
    }
    let waiting_on = get("waiting_on");
    let open_questions = questions.iter().filter(|q| !q.ticked).count();
    let waiting_for_owner = questions.iter().any(|q| !q.ticked && q.escalated)
        || (status == "blocked" && waiting_on.to_lowercase().starts_with("owner"));
    let attachments = list_attachments(root, &id);
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    let done_on = if status == "done" {
        log_lines
            .iter()
            .rev()
            .find_map(|l| day_of(l))
            .unwrap_or_default()
    } else {
        String::new()
    };
    Some(Task {
        queue: queue.to_string(),
        id,
        title,
        kind: get("type"),
        priority: get("priority"),
        status,
        folder: folder.to_string(),
        path: rel,
        claimed_by: get("claimed-by"),
        heartbeat: get("heartbeat"),
        waiting_on,
        commit: get("commit"),
        captured: get("captured"),
        questions,
        open_questions,
        waiting_for_owner,
        done_on,
        claimed_on,
        est_minutes: crate::ops::time_minutes(
            &crate::ops::clean_time(&get("est-time")).unwrap_or_default(),
        ),
        est_token_count: crate::ops::token_count(
            &crate::ops::clean_tokens(&get("est-tokens")).unwrap_or_default(),
        ),
        est_time: crate::ops::clean_time(&get("est-time")).unwrap_or_default(),
        est_tokens: crate::ops::clean_tokens(&get("est-tokens")).unwrap_or_default(),
        tags: get("tags")
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        attachments,
        log_tail,
        modified,
    })
}

fn list_attachments(root: &Path, id: &str) -> Vec<String> {
    if id.len() != 4 || !id.bytes().all(|b| b.is_ascii_digit()) {
        return Vec::new();
    }
    let dir = root.join("attachments").join(id);
    let mut out: Vec<String> = fs::read_dir(&dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_file())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| !n.starts_with('.'))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// Cache of parsed task files, keyed by path, valid while size and mtime match.
#[derive(Default)]
pub struct Cache {
    files: HashMap<PathBuf, (u64, u64, Option<Task>)>,
    git: HashMap<PathBuf, (u64, Vec<Activity>)>,
}

fn mtime_secs(m: &fs::Metadata) -> u64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn read_state(queues: &[(String, PathBuf)], cache: &mut Cache) -> State {
    let mut state = State {
        now: now_secs(),
        ..Default::default()
    };
    let mut seen = HashSet::new();
    for (name, root) in queues {
        let mut counts: HashMap<String, usize> = HashMap::new();
        let mut tasks = Vec::new();
        for (folder, path) in task_files(root) {
            seen.insert(path.clone());
            let Ok(meta) = fs::metadata(&path) else {
                continue;
            };
            if meta.len() > MAX_TASK_BYTES {
                state.problems.push(format!(
                    "{name}: {} is over 1 MB and wasn't read",
                    path.display()
                ));
                continue;
            }
            let key = (meta.len(), mtime_secs(&meta));
            let task = match cache.files.get(&path) {
                Some((len, m, t)) if (*len, *m) == key => t.clone(),
                _ => {
                    let t = fs::read_to_string(&path)
                        .ok()
                        .and_then(|text| parse_task(name, root, &folder, &path, &text, key.1));
                    cache.files.insert(path.clone(), (key.0, key.1, t.clone()));
                    t
                }
            };
            match task {
                Some(t) => tasks.push(t),
                None => state
                    .problems
                    .push(format!("{name}: {} has no front matter", path.display())),
            }
        }
        state.problems.extend(problems(name, &tasks, state.now));
        for t in &tasks {
            *counts.entry(t.status.clone()).or_default() += 1;
        }
        state.activity.extend(git_activity(name, root, cache));
        state.queues.push(QueueInfo {
            name: name.clone(),
            path: root.to_string_lossy().to_string(),
            counts,
        });
        state.tasks.extend(tasks);
    }
    cache.files.retain(|p, _| seen.contains(p));
    state.activity.sort_by_key(|a| std::cmp::Reverse(a.at));
    state.activity.truncate(80);
    state
}

pub fn problems(queue: &str, tasks: &[Task], now: u64) -> Vec<String> {
    let mut out = Vec::new();
    let mut ids: HashMap<&str, Vec<&str>> = HashMap::new();
    for t in tasks {
        ids.entry(t.id.as_str()).or_default().push(t.path.as_str());
        if !t.folder.is_empty() && t.status != t.folder {
            out.push(format!(
                "{queue}: {} says status '{}' but it's in {}/",
                t.path, t.status, t.folder
            ));
        }
        let file = t.path.rsplit('/').next().unwrap_or("");
        if t.id.len() != 4
            || !t.id.bytes().all(|b| b.is_ascii_digit())
            || !file.starts_with(&format!("{}-", t.id))
        {
            out.push(format!(
                "{queue}: {} has id '{}', which doesn't match its file name",
                t.path, t.id
            ));
        }
        if t.status == "doing" && t.claimed_by.is_empty() {
            out.push(format!(
                "{queue}: {} is being worked on but nobody has claimed it",
                t.path
            ));
        }
        if t.status == "doing" && now.saturating_sub(t.modified) > STALE_SECS {
            out.push(format!(
                "{queue}: {} is claimed but hasn't changed in {} h",
                t.path,
                (now - t.modified) / 3600
            ));
        }
        if t.status == "blocked" && t.waiting_on.is_empty() {
            out.push(format!(
                "{queue}: {} is blocked but doesn't say on what",
                t.path
            ));
        }
    }
    for (id, paths) in &ids {
        if paths.len() > 1 {
            out.push(format!(
                "{queue}: id {id} is used twice: {}",
                paths.join(", ")
            ));
        }
    }
    let known: HashSet<&str> = ids.keys().copied().collect();
    for t in tasks {
        let r: String = t.waiting_on.chars().take(4).collect();
        if r.len() == 4 && r.bytes().all(|b| b.is_ascii_digit()) && !known.contains(r.as_str()) {
            out.push(format!(
                "{queue}: {} waits on {r}, which doesn't exist",
                t.path
            ));
        }
    }
    out.sort();
    out
}

/// The queue's recent commits, re-read only when the git log changes.
fn git_activity(queue: &str, root: &Path, cache: &mut Cache) -> Vec<Activity> {
    let head_log = root.join(".git").join("logs").join("HEAD");
    let stamp = fs::metadata(&head_log).map(|m| mtime_secs(&m)).unwrap_or(0);
    if stamp == 0 {
        return Vec::new();
    }
    if let Some((s, a)) = cache.git.get(root) {
        if *s == stamp {
            return a.clone();
        }
    }
    let out = std::process::Command::new("git")
        .args(["-C"])
        .arg(root)
        .args(["log", "-n", "40", "--format=%ct%x1f%s"])
        .output();
    let mut list = Vec::new();
    if let Ok(o) = out {
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            if let Some((at, what)) = line.split_once('\u{1f}') {
                list.push(Activity {
                    queue: queue.to_string(),
                    at: at.parse().unwrap_or(0),
                    what: what.chars().take(300).collect(),
                    source: "commit".into(),
                });
            }
        }
    }
    cache.git.insert(root.to_path_buf(), (stamp, list.clone()));
    list
}

/// The task file with this id: exactly one, or an error.
pub fn find_task(root: &Path, id: &str) -> Result<PathBuf, String> {
    if id.len() != 4 || !id.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("task id must be four digits, got {id:?}"));
    }
    let prefix = format!("{id}-");
    let hits: Vec<PathBuf> = task_files(root)
        .into_iter()
        .map(|(_, p)| p)
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().starts_with(&prefix))
                .unwrap_or(false)
        })
        .collect();
    match hits.len() {
        0 => Err(format!("no task {id}")),
        1 => Ok(hits[0].clone()),
        _ => Err(format!("task {id} exists twice")),
    }
}

pub fn read_body(root: &Path, id: &str) -> Result<String, String> {
    let p = find_task(root, id)?;
    let meta = fs::metadata(&p).map_err(|e| e.to_string())?;
    if meta.len() > MAX_TASK_BYTES {
        return Err("task file is over 1 MB".into());
    }
    fs::read_to_string(&p).map_err(|e| e.to_string())
}

/// An image attachment as a data URL. Only plain file names in `attachments/<id>/`.
pub fn read_image(root: &Path, id: &str, name: &str) -> Result<String, String> {
    if id.len() != 4 || !id.bytes().all(|b| b.is_ascii_digit()) {
        return Err("bad task id".into());
    }
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.starts_with('.') {
        return Err("bad file name".into());
    }
    let lower = name.to_lowercase();
    let mime = if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else {
        return Err("not an image".into());
    };
    let p = root.join("attachments").join(id).join(name);
    let meta = fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > MAX_IMAGE_BYTES {
        return Err("not a regular file under 15 MB".into());
    }
    let bytes = fs::read(&p).map_err(|e| e.to_string())?;
    Ok(format!("data:{mime};base64,{}", base64(&bytes)))
}

pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Contract rule 3: control characters out (newline kept), trimmed, capped.
pub fn clean_text(text: &str) -> Result<String, String> {
    let t: String = text
        .chars()
        .filter(|&c| c == '\n' || !(c < '\u{20}' || c == '\u{7f}'))
        .collect();
    let t = t.trim().to_string();
    if t.is_empty() {
        return Err("the answer is empty".into());
    }
    if t.chars().count() > MAX_ANSWER_CHARS {
        return Err(format!(
            "the answer is longer than {MAX_ANSWER_CHARS} characters"
        ));
    }
    Ok(t)
}

/// Contract v1: answer question `n` of task `id` as `who`, then drop the asker's mailbox file.
/// `stamp` is local time as `YYYY-MM-DD HH:MM`. Returns the asker's session, if one was found.
pub fn answer(
    root: &Path,
    id: &str,
    n: u32,
    text: &str,
    who: &str,
    stamp: &str,
) -> Result<Option<String>, String> {
    let text = clean_text(text)?;
    let mail = root.join("mail");
    with_lock(root, || {
        let path = find_task(root, id)?;
        let body = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let head = "## Questions\n";
        let start = body
            .find(head)
            .map(|i| i + head.len())
            .ok_or("the task has no Questions section")?;
        let end = body[start..]
            .find("\n## ")
            .map(|i| start + i + 1)
            .unwrap_or(body.len());
        let sect = &body[start..end];
        let (a, b) = question_items(sect)
            .into_iter()
            .find(|&(a, b)| parse_question(&sect[a..b]).n == Some(n))
            .ok_or(format!("no Q{n} in {id}"))?;
        let item = &sect[a..b];
        let new_item = format!(
            "{}\n  **A ({who}, {stamp}):** {text}",
            item.replacen("- [ ] **Q", "- [x] **Q", 1)
        );
        let new_sect = format!("{}{}{}", &sect[..a], new_item, &sect[b..]);
        let mut new_body = format!("{}{}{}", &body[..start], new_sect, &body[end..]);
        new_body = format!(
            "{}\n- {stamp} {who}: answered Q{n}\n",
            new_body.trim_end_matches('\n')
        );
        write_atomic(&path, &new_body)?;
        let asker = asker_session(&mail, id, n, item);
        if let Some(s) = &asker {
            let title = new_body
                .lines()
                .find_map(|l| l.strip_prefix("# "))
                .unwrap_or("")
                .trim()
                .to_string();
            let q = parse_question(item).text;
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            let msg = format!(
                "Answer to Q{n} on task {id} ({title}), from {who}:\nYour question: {q}\nAnswer: {text}\n(Also written in {rel} in the task queue.)\n"
            );
            let dir = mail.join(s);
            fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            write_atomic(&dir.join(format!("{id}-Q{n}.md")), &msg)?;
        }
        Ok(asker)
    })
}

/// Run `f` holding the queue's lock (an exclusive flock on `mail/.lock`, file contract rule 1).
pub fn with_lock<T>(root: &Path, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let mail = root.join("mail");
    fs::create_dir_all(&mail).map_err(|e| e.to_string())?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(mail.join(".lock"))
        .map_err(|e| e.to_string())?;
    lock.lock().map_err(|e| e.to_string())?;
    let result = f();
    let _ = lock.unlock();
    result
}

/// Contract rule 4: `mail/asks.json`, else `session xxxxxxxx` in the question line.
fn asker_session(mail: &Path, id: &str, n: u32, item: &str) -> Option<String> {
    let valid = |s: &str| {
        s.len() == 8
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    if let Ok(text) = fs::read_to_string(mail.join("asks.json")) {
        if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&text) {
            if let Some(s) = map.get(&format!("{id}-Q{n}")) {
                if valid(s) {
                    return Some(s.clone());
                }
            }
        }
    }
    let i = item.find("session ")? + 8;
    let s: String = item[i..].chars().take(8).collect();
    valid(&s).then_some(s)
}

pub fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    let tmp = path.with_extension(format!("muster-{}.tmp", std::process::id()));
    let mut f = fs::File::create(&tmp).map_err(|e| e.to_string())?;
    f.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TASK: &str = "---\nid: 0042\ntype: bug\npriority: high\nstatus: doing\ncaptured: 2026-10-11\nclaimed-by: wt-a (session aaaa1111)\nheartbeat:\nwaiting_on:\ncommit:\n---\n\n# Something broke\n\n**Details:** test.\n\n## Questions\n- [ ] **Q1 (wt-a, session aaaa1111, 2026-10-11 10:00):** Which port?\n- [ ] **Q2 (wt-a, session aaaa1111, 2026-10-11 10:05):** Product call?\n  **Escalated to owner (capture, 2026-10-11 10:06).**\n\n## Log\n- 2026-10-11 captured\n- 2026-10-11 wt-a: asked Q1\n";

    fn queue() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for f in STATUS_FOLDERS {
            fs::create_dir(d.path().join(f)).unwrap();
        }
        fs::write(d.path().join("README.md"), "queue").unwrap();
        fs::write(d.path().join("doing/0042-something.md"), TASK).unwrap();
        d
    }

    #[test]
    fn parses_a_task() {
        let d = queue();
        let mut c = Cache::default();
        let s = read_state(&[("p".into(), d.path().to_path_buf())], &mut c);
        assert_eq!(s.tasks.len(), 1);
        let t = &s.tasks[0];
        assert_eq!(
            (
                t.id.as_str(),
                t.title.as_str(),
                t.status.as_str(),
                t.folder.as_str()
            ),
            ("0042", "Something broke", "doing", "doing")
        );
        assert_eq!(t.questions.len(), 2);
        assert_eq!(t.questions[0].n, Some(1));
        assert_eq!(t.questions[0].text, "Which port?");
        assert!(t.questions[0].asker.starts_with("wt-a"));
        assert!(t.questions[1].escalated);
        assert_eq!(t.open_questions, 2);
        assert!(t.waiting_for_owner);
        assert_eq!(t.log_tail.len(), 2);
        assert_eq!((t.claimed_on.as_str(), t.done_on.as_str()), ("", ""));
        assert!(s.problems.is_empty(), "{:?}", s.problems);
        assert_eq!(s.queues[0].counts.get("doing"), Some(&1));
    }

    #[test]
    fn reads_claim_and_done_days_from_the_log() {
        let d = tempfile::tempdir().unwrap();
        let text = TASK.replace("status: doing", "status: done").replace(
            "- 2026-10-11 wt-a: asked Q1\n",
            "- 2026-10-11 10:00 claimed by wt-a\n- 2026-10-12 09:00 note\n- 2026-10-13 18:00 done in abc123\n",
        );
        std::fs::write(d.path().join("0042-x.md"), text).unwrap();
        let s = read_state(
            &[("p".into(), d.path().to_path_buf())],
            &mut Cache::default(),
        );
        assert_eq!(
            (s.tasks[0].claimed_on.as_str(), s.tasks[0].done_on.as_str()),
            ("2026-10-11", "2026-10-13")
        );
    }

    #[test]
    fn reads_the_flat_layout_too() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("0042-x.md"), TASK).unwrap();
        let s = read_state(
            &[("p".into(), d.path().to_path_buf())],
            &mut Cache::default(),
        );
        assert_eq!(s.tasks[0].status, "doing");
        assert_eq!(s.tasks[0].folder, "");
        assert!(s.problems.is_empty());
    }

    #[test]
    fn finds_problems() {
        let d = queue();
        fs::write(d.path().join("todo/0042-dup.md"), TASK).unwrap();
        fs::write(
            d.path().join("blocked/0050-b.md"),
            "---\nid: 0050\nstatus: blocked\nwaiting_on: 0099 x\n---\n# B\n",
        )
        .unwrap();
        fs::write(
            d.path().join("blocked/0051-c.md"),
            "---\nid: 0051\nstatus: blocked\nwaiting_on:\n---\n# C\n",
        )
        .unwrap();
        fs::write(d.path().join("todo/0052-n.md"), "no front matter").unwrap();
        let s = read_state(
            &[("p".into(), d.path().to_path_buf())],
            &mut Cache::default(),
        );
        let all = s.problems.join("\n");
        assert!(all.contains("used twice"), "{all}");
        assert!(all.contains("status 'doing' but it's in todo/"), "{all}");
        assert!(all.contains("waits on 0099"), "{all}");
        assert!(all.contains("doesn't say on what"), "{all}");
        assert!(all.contains("no front matter"), "{all}");
    }

    #[test]
    fn cache_rereads_only_changed_files() {
        let d = queue();
        let q = [("p".to_string(), d.path().to_path_buf())];
        let mut c = Cache::default();
        assert_eq!(read_state(&q, &mut c).tasks[0].title, "Something broke");
        let p = d.path().join("doing/0042-something.md");
        fs::write(
            &p,
            TASK.replace("# Something broke", "# Renamed, and longer"),
        )
        .unwrap();
        assert_eq!(read_state(&q, &mut c).tasks[0].title, "Renamed, and longer");
        fs::remove_file(&p).unwrap();
        assert!(read_state(&q, &mut c).tasks.is_empty());
        assert!(c.files.is_empty());
    }

    #[test]
    fn answers_by_the_contract() {
        let d = queue();
        let asker = answer(
            d.path(),
            "0042",
            2,
            "Ship it.\u{1b}[31m\ttoday",
            "owner",
            "2026-10-12 09:30",
        )
        .unwrap();
        assert_eq!(asker.as_deref(), Some("aaaa1111"));
        let body = fs::read_to_string(d.path().join("doing/0042-something.md")).unwrap();
        assert!(body.contains("- [ ] **Q1 ("), "Q1 untouched");
        assert!(body.contains("- [x] **Q2 ("));
        assert!(body.contains("  **Escalated to owner (capture, 2026-10-11 10:06).**\n  **A (owner, 2026-10-12 09:30):** Ship it.[31mtoday\n"), "{body}");
        assert!(body
            .trim_end()
            .ends_with("- 2026-10-12 09:30 owner: answered Q2"));
        assert!(body.find("## Questions").unwrap() < body.find("## Log").unwrap());
        let mail = fs::read_to_string(d.path().join("mail/aaaa1111/0042-Q2.md")).unwrap();
        assert!(mail.contains("Answer to Q2 on task 0042 (Something broke), from owner:"));
        assert!(mail.contains("Your question: Product call?"));
        let s = read_state(
            &[("p".into(), d.path().to_path_buf())],
            &mut Cache::default(),
        );
        assert!(!s.tasks[0].waiting_for_owner);
        assert!(s.tasks[0].questions[1].owner_answered);
    }

    #[test]
    fn asks_json_wins_over_the_question_line() {
        let d = queue();
        fs::create_dir(d.path().join("mail")).unwrap();
        fs::write(
            d.path().join("mail/asks.json"),
            r#"{"0042-Q1": "bbbb2222"}"#,
        )
        .unwrap();
        assert_eq!(
            answer(d.path(), "0042", 1, "5173", "owner", "2026-10-12 09:30")
                .unwrap()
                .as_deref(),
            Some("bbbb2222")
        );
        assert!(d.path().join("mail/bbbb2222/0042-Q1.md").is_file());
    }

    #[test]
    fn answer_refusals() {
        let d = queue();
        assert!(answer(d.path(), "0042", 9, "x", "owner", "t")
            .unwrap_err()
            .contains("no Q9"));
        assert!(answer(d.path(), "42", 1, "x", "owner", "t")
            .unwrap_err()
            .contains("four digits"));
        assert!(answer(d.path(), "0043", 1, "x", "owner", "t")
            .unwrap_err()
            .contains("no task"));
        assert!(answer(d.path(), "0042", 1, "  \u{7}  ", "owner", "t")
            .unwrap_err()
            .contains("empty"));
        assert!(answer(d.path(), "0042", 1, &"x".repeat(4001), "owner", "t")
            .unwrap_err()
            .contains("longer"));
        let body = fs::read_to_string(d.path().join("doing/0042-something.md")).unwrap();
        assert_eq!(body, TASK, "refusals leave the file alone");
    }

    #[test]
    fn images_only_from_the_attachments_folder() {
        let d = queue();
        fs::create_dir_all(d.path().join("attachments/0042")).unwrap();
        fs::write(
            d.path().join("attachments/0042/shot.png"),
            [137u8, 80, 78, 71],
        )
        .unwrap();
        assert_eq!(
            read_image(d.path(), "0042", "shot.png").unwrap(),
            "data:image/png;base64,iVBORw=="
        );
        assert!(read_image(d.path(), "0042", "../../README.md").is_err());
        assert!(read_image(d.path(), "0042", "notes.txt").is_err());
        assert!(read_image(d.path(), "../x", "shot.png").is_err());
        let s = read_state(
            &[("p".into(), d.path().to_path_buf())],
            &mut Cache::default(),
        );
        assert_eq!(s.tasks[0].attachments, vec!["shot.png".to_string()]);
    }

    #[test]
    fn base64_matches_known_values() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn discovers_queues() {
        let base = tempfile::tempdir().unwrap();
        let q = base.path().join("proj/tasks");
        fs::create_dir_all(q.join("todo")).unwrap();
        fs::write(q.join("README.md"), "q").unwrap();
        fs::create_dir_all(base.path().join("other/tasks")).unwrap();
        let found = discover(base.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "proj");
    }
}
