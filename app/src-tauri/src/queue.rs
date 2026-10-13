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
/// The stages a task moves through, in order (decision 0003). `stage:` in the front matter is
/// the truth; the old `status:` (and the v2 folder) is kept as a mirror for older tools.
pub const STAGES: [&str; 7] = [
    "backlog",
    "shaping",
    "ready",
    "building",
    "testing",
    "your-check",
    "done",
];
pub(crate) const MAX_TASK_BYTES: u64 = 1024 * 1024;
const MAX_ANSWER_CHARS: usize = 4000;
const MAX_IMAGE_BYTES: u64 = 15 * 1024 * 1024;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    pub n: Option<u32>,
    pub ticked: bool,
    pub asker: String,
    /// The question and what's under it (escalation, answers), as written: continuation lines
    /// without the two-space indent they have in the file (contract rule 3).
    pub text: String,
    pub escalated: bool,
    pub owner_answered: bool,
}

/// One stage change from the log: `- <at> <who>: stage <from> → <to>[ (<detail>)]`.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StageStep {
    pub at: String,
    pub who: String,
    pub from: String,
    pub to: String,
}

/// The stage's name as people read it, or "" for an unknown stage.
pub fn stage_label(stage: &str) -> &'static str {
    match stage {
        "backlog" => "Backlog",
        "shaping" => "Shaping",
        "ready" => "Ready",
        "building" => "Building",
        "testing" => "Testing",
        "your-check" => "Your check",
        "done" => "Done",
        _ => "",
    }
}

/// Who works a stage by default (a label, not enforced). "" where nobody holds it: Ready is
/// claimable, Done is finished.
pub fn stage_role(stage: &str) -> &'static str {
    match stage {
        "backlog" => "anyone",
        "shaping" => "product + tech + UI agents",
        "building" => "code agent",
        "testing" => "QA agent",
        "your-check" => "owner",
        _ => "",
    }
}

/// The old status a stage and the blocked flag mean, kept for Drover's tools and `manifest.py`.
pub fn mirror_status(stage: &str, blocked: bool) -> &'static str {
    if blocked {
        return "blocked";
    }
    match stage {
        "backlog" => "inbox",
        "shaping" | "ready" => "todo",
        "done" => "done",
        _ => "doing",
    }
}

/// The stage an old status means, for files written before stages.
fn legacy_stage(status: &str) -> &'static str {
    match status {
        "todo" | "blocked" => "ready",
        "doing" => "building",
        "done" => "done",
        _ => "backlog",
    }
}

/// A tool-written stage line from the log, with its detail (the text in the closing brackets).
/// Anything else, a hand-typed "moved todo → doing" included, is not stage history.
pub fn stage_line(line: &str) -> Option<(StageStep, &str)> {
    let rest = line.strip_prefix("- ")?;
    let at = rest.get(..16)?;
    let stamp_ok = at.bytes().enumerate().all(|(i, c)| match i {
        4 | 7 => c == b'-',
        10 => c == b' ',
        13 => c == b':',
        _ => c.is_ascii_digit(),
    });
    if !stamp_ok {
        return None;
    }
    let (who, tail) = rest.get(16..)?.strip_prefix(' ')?.split_once(": stage ")?;
    let (from, tail) = tail.split_once(" → ")?;
    let (to, detail) = match tail.split_once(' ') {
        Some((to, d)) => (to, d.strip_prefix('(')?.strip_suffix(')')?),
        None => (tail, ""),
    };
    let known = |s: &str| STAGES.contains(&s);
    if who.trim().is_empty() || !known(from) || !known(to) || from == to {
        return None;
    }
    Some((
        StageStep {
            at: at.to_string(),
            who: who.to_string(),
            from: from.to_string(),
            to: to.to_string(),
        },
        detail,
    ))
}

/// Stage lines of a task's Log section, oldest first.
pub fn stage_lines(body: &str) -> Vec<(StageStep, &str)> {
    log_section(body)
        .map(|s| s.lines().filter_map(stage_line).collect())
        .unwrap_or_default()
}

/// One acceptance criterion: a `- [ ]` / `- [x]` line at the start of a line under
/// `## Acceptance criteria`, with its indented continuation lines.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Criterion {
    /// 1-based position in the list.
    pub n: usize,
    /// Its label (`AC2`) when the text starts with one, else empty.
    pub label: String,
    /// The text after the box, continuation lines joined with a space.
    pub text: String,
    pub ticked: bool,
    /// Only the owner verifies it: its `verify:` part starts with "owner" ("— verify: owner",
    /// "— verify: owner (screenshots…)"). "verify: command … and owner" isn't: it needs the
    /// agent's proof first.
    pub owner: bool,
    /// Byte offset of its box line within the body.
    #[serde(skip)]
    pub at: usize,
}

/// The acceptance criteria, in order. Sub-steps (`  - [ ]`, indented) belong to the item above.
pub fn criteria_items(body: &str) -> Vec<Criterion> {
    let Some(s) = section(body, "Acceptance criteria") else {
        return Vec::new();
    };
    // `section` returns a slice of `body`, so its offset is the distance between the two.
    let base = s.as_ptr() as usize - body.as_ptr() as usize;
    let mut out: Vec<Criterion> = Vec::new();
    let mut pos = 0;
    // Whether indented lines still continue the item above (prose or a blank line ends it).
    let mut open = false;
    for line in s.split_inclusive('\n') {
        let l = line.trim_end_matches(['\n', '\r']);
        let ticked = l.starts_with("- [x]") || l.starts_with("- [X]");
        if ticked || l.starts_with("- [ ]") {
            let text = l[5..].trim().to_string();
            let label = text
                .split(|c: char| c == ':' || c.is_whitespace())
                .next()
                .filter(|w| {
                    w.len() > 2 && w.starts_with("AC") && w[2..].bytes().all(|b| b.is_ascii_digit())
                })
                .unwrap_or("")
                .to_string();
            out.push(Criterion {
                n: out.len() + 1,
                label,
                text,
                ticked,
                owner: false,
                at: base + pos,
            });
            open = true;
        } else if let Some(c) = out
            .last_mut()
            .filter(|_| open && l.starts_with([' ', '\t']) && !l.trim().is_empty())
        {
            c.text.push(' ');
            c.text.push_str(l.trim());
        } else {
            open = false;
        }
        pos += line.len();
    }
    for c in &mut out {
        c.owner = owner_verifies(&c.text);
    }
    out
}

/// Does the criterion's `verify:` part (the last one) start with "owner"?
fn owner_verifies(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower
        .rfind("verify:")
        .map(|i| {
            lower[i + "verify:".len()..]
                .trim_start()
                .starts_with("owner")
        })
        .unwrap_or(false)
}

/// Acceptance criteria: (items, ticked).
pub fn criteria(body: &str) -> (usize, usize) {
    let items = criteria_items(body);
    (items.len(), items.iter().filter(|c| c.ticked).count())
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
    /// One of STAGES: `stage:` from the file, else what the old status means.
    pub stage: String,
    /// Waiting on something (`waiting_on`, or an old file with status blocked). Any stage.
    pub blocked: bool,
    pub stage_history: Vec<StageStep>,
    /// A `stage:` value that isn't a stage (read as `stage` instead), for `check`.
    #[serde(skip)]
    pub bad_stage: String,
    /// The file says its stage (`stage:`), so its status must mirror it, for `check`.
    #[serde(skip)]
    pub stage_set: bool,
    pub folder: String,
    pub path: String,
    pub claimed_by: String,
    pub heartbeat: String,
    pub waiting_on: String,
    pub commit: String,
    pub captured: String,
    pub questions: Vec<Question>,
    pub open_questions: usize,
    /// The criteria only the owner verifies (`— verify: owner`), ticked or not, for the task
    /// pane's checklist in Your check (0028).
    pub owner_criteria: Vec<Criterion>,
    pub waiting_for_owner: bool,
    pub tags: Vec<String>,
    /// Estimates ("2h", "400k"), and the same as numbers for totals. Agent time is `est_time`;
    /// `est_test_time` is the QA agent's and the owner's check (task 0017).
    pub est_time: String,
    pub est_test_time: String,
    pub est_tokens: String,
    pub est_minutes: u64,
    pub est_test_minutes: u64,
    pub est_token_count: u64,
    /// From the log: the day work started (first "claimed" line) and the day it was done.
    pub claimed_on: String,
    pub done_on: String,
    /// External links (`links:`), and the comment thread (`## Comments`), oldest first (0017).
    pub links: Vec<crate::fields::Link>,
    pub comments: Vec<crate::fields::Comment>,
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
    /// Every queue's backlog notes (task 0015), newest first.
    pub backlog: Vec<crate::backlog::Note>,
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

/// `NNNN-slug.md`. Not a date-named note (`2026-10-12-0900-….md`, what agents drop in inbox/).
pub(crate) fn is_task_name(name: &str) -> bool {
    let b = name.as_bytes();
    let task = name.len() > 8
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && name.ends_with(".md");
    let dated = name.len() > 11
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'-'
        && b[8..10].iter().all(u8::is_ascii_digit)
        && b[10] == b'-';
    task && !dated
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

/// Where the Log's lines are: the content of the LAST `## Log` section, as (start, end) byte
/// offsets. The tool only ever appends there (`append_log`), so a `## Log` line quoted in a
/// description or an answer above it is just text: it can't forge or hide stage history.
pub(crate) fn log_span(text: &str) -> Option<(usize, usize)> {
    let mut start = None;
    let mut pos = 0;
    for line in text.split_inclusive('\n') {
        pos += line.len();
        if line.trim_end() == "## Log" {
            start = Some(pos);
        }
    }
    let start = start?;
    let mut end = start;
    for line in text[start..].split_inclusive('\n') {
        if line.starts_with("## ") {
            break;
        }
        end += line.len();
    }
    Some((start, end))
}

/// Where the last `## Log` heading line starts (the Log `log_span` reads), as a byte offset.
pub(crate) fn log_heading(text: &str) -> Option<usize> {
    let mut at = None;
    let mut pos = 0;
    for line in text.split_inclusive('\n') {
        if line.trim_end() == "## Log" {
            at = Some(pos);
        }
        pos += line.len();
    }
    at
}

/// The tool's `## Comments` section (task 0017): the last `## Comments` heading above the last
/// `## Log` (anywhere, in a task without a Log), as (heading start, content start, content end)
/// byte offsets; the content runs to the next `## ` line. Comments are written only by the tool,
/// with continuation lines indented, so none of their lines can start a heading.
pub(crate) fn comments_span(text: &str) -> Option<(usize, usize, usize)> {
    let log = log_heading(text).unwrap_or(text.len());
    let mut head = None;
    let mut pos = 0;
    for line in text.split_inclusive('\n') {
        if pos >= log {
            break;
        }
        if line.trim_end() == "## Comments" {
            head = Some((pos, pos + line.len()));
        }
        pos += line.len();
    }
    let (at, start) = head?;
    let mut end = start;
    for line in text[start..].split_inclusive('\n') {
        if line.starts_with("## ") {
            break;
        }
        end += line.len();
    }
    Some((at, start, end))
}

/// The task's Log section (the last `## Log`; see `log_span`).
fn log_section(body: &str) -> Option<&str> {
    log_span(body).map(|(a, b)| &body[a..b])
}

/// Add `- <line>` at the end of the Log section `log_section` reads, before any section that
/// follows it; a task without one gets a Log at the end.
pub fn append_log(text: &str, line: &str) -> String {
    match log_span(text) {
        None => format!("{}\n\n## Log\n- {line}\n", text.trim_end_matches('\n')),
        Some((_, end)) => {
            let (head, tail) = text.split_at(end);
            let head = head.trim_end_matches('\n');
            if tail.is_empty() {
                format!("{head}\n- {line}\n")
            } else {
                format!("{head}\n- {line}\n\n{tail}")
            }
        }
    }
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
        Some(i) => item[i + 3..].trim(),
        None => item,
    };
    Question {
        n,
        ticked,
        asker,
        text: unindent_text(text),
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
    let log_tail: Vec<String> = log_section(body)
        .map(|s| {
            s.lines()
                .filter(|l| l.starts_with("- "))
                .map(|l| l[2..].to_string())
                .collect::<Vec<_>>()
        })
        .map(|v| v[v.len().saturating_sub(3)..].to_vec())
        .unwrap_or_default();
    let log_lines: Vec<&str> = log_section(body)
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
    let raw_stage = get("stage").to_lowercase();
    let stage_set = STAGES.contains(&raw_stage.as_str());
    let (stage, bad_stage) = match STAGES.iter().find(|s| **s == raw_stage) {
        Some(s) => (s.to_string(), String::new()),
        None => (
            legacy_stage(&status).to_string(),
            raw_stage.chars().take(40).collect(),
        ),
    };
    let blocked = !waiting_on.is_empty() || status == "blocked";
    let stage_history = stage_lines(body).into_iter().map(|(s, _)| s).collect();
    let open_questions = questions.iter().filter(|q| !q.ticked).count();
    let owner_criteria: Vec<Criterion> = criteria_items(body)
        .into_iter()
        .filter(|c| c.owner)
        .collect();
    let waiting_for_owner = questions.iter().any(|q| !q.ticked && q.escalated)
        || stage == "your-check"
        || (blocked && waiting_on.to_lowercase().starts_with("owner"));
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
    let est_test_time = crate::ops::clean_time(&get("est-test-time")).unwrap_or_default();
    Some(Task {
        queue: queue.to_string(),
        id,
        title,
        kind: crate::ops::read_kind(&get("type")),
        priority: get("priority"),
        status,
        stage,
        blocked,
        stage_history,
        bad_stage,
        stage_set,
        folder: folder.to_string(),
        path: rel,
        claimed_by: get("claimed-by"),
        heartbeat: get("heartbeat"),
        waiting_on,
        commit: get("commit"),
        captured: get("captured"),
        questions,
        open_questions,
        owner_criteria,
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
        est_test_minutes: crate::ops::time_minutes(&est_test_time),
        est_test_time,
        est_tokens: crate::ops::clean_tokens(&get("est-tokens")).unwrap_or_default(),
        tags: get("tags")
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        links: crate::fields::parse_links(&get("links")),
        comments: crate::fields::comments(body),
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
    /// Backlog notes, the same way: by path, while size and mtime match.
    pub(crate) notes: HashMap<PathBuf, (u64, u64, crate::backlog::Note)>,
    git: HashMap<PathBuf, (u64, Vec<Activity>)>,
    /// The local time stamp, and the minute (seconds since 1970 / 60) it was read in.
    stamp: Option<(u64, String)>,
}

impl Cache {
    /// Local time at `now` (seconds since 1970), read from the clock once a minute: a stamp only
    /// changes per minute, and the window asks for the state every two seconds.
    fn stamp_at(&mut self, now: u64) -> String {
        match &self.stamp {
            Some((minute, s)) if *minute == now / 60 => s.clone(),
            _ => {
                let s = local_stamp();
                self.stamp = Some((now / 60, s.clone()));
                s
            }
        }
    }
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

pub(crate) fn mtime_secs(m: &fs::Metadata) -> u64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn read_state(queues: &[(String, PathBuf)], cache: &mut Cache) -> State {
    let stamp = cache.stamp_at(now_secs());
    read_state_at(queues, cache, &stamp)
}

/// `read_state` with the local time given: `stamp` (`YYYY-MM-DD HH:MM`) is "now" for the claims'
/// heartbeats, so tests that stamp a claim at a fixed time judge its lease at that time, not at
/// whatever the wall clock says. File times are still judged by the real clock.
pub fn read_state_at(queues: &[(String, PathBuf)], cache: &mut Cache, stamp: &str) -> State {
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
        state
            .problems
            .extend(problems(name, &tasks, state.now, stamp));
        for t in &tasks {
            *counts.entry(t.stage.clone()).or_default() += 1;
            if t.blocked {
                *counts.entry("blocked".into()).or_default() += 1;
            }
        }
        state.activity.extend(git_activity(name, root, cache));
        state.queues.push(QueueInfo {
            name: name.clone(),
            path: root.to_string_lossy().to_string(),
            counts,
        });
        state.tasks.extend(tasks);
        let notes = crate::backlog::read_notes(name, root, &mut cache.notes, &mut seen);
        state.problems.extend(notes.problems);
        state.backlog.extend(notes.notes);
    }
    cache.files.retain(|p, _| seen.contains(p));
    cache.notes.retain(|p, _| seen.contains(p));
    crate::backlog::newest_first(&mut state.backlog);
    state.activity.sort_by_key(|a| std::cmp::Reverse(a.at));
    state.activity.truncate(80);
    state
}

/// What's wrong in a queue's tasks. `now` and `stamp` are the same moment, as seconds since 1970
/// and as local time (`YYYY-MM-DD HH:MM`), for the claims' lease.
pub fn problems(queue: &str, tasks: &[Task], now: u64, stamp: &str) -> Vec<String> {
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
        if t.stage_set && mirror_status(&t.stage, t.blocked) != t.status {
            out.push(format!(
                "{queue}: {} says stage {} but status {} (moved by hand? use `muster stage`)",
                t.path, t.stage, t.status
            ));
        }
        if !t.bad_stage.is_empty() {
            out.push(format!(
                "{queue}: {} has stage '{}', which isn't a stage (read as {})",
                t.path, t.bad_stage, t.stage
            ));
        }
        // Only Building is held by someone; a blocked task isn't being worked on.
        let building = t.stage == "building" && !t.blocked;
        if building && t.claimed_by.is_empty() {
            out.push(format!(
                "{queue}: {} is being worked on but nobody has claimed it",
                t.path
            ));
        }
        // The lease `muster reclaim` uses: since the heartbeat, else since the file changed.
        let quiet = crate::ops::quiet_minutes(&t.heartbeat, t.modified, stamp, now);
        if building && quiet > crate::ops::LEASE_MINUTES {
            out.push(format!(
                "{queue}: {} is claimed but hasn't been heard from in {} h",
                t.path,
                quiet / 60
            ));
        }
        if t.status == "blocked" && t.waiting_on.is_empty() {
            out.push(format!(
                "{queue}: {} is blocked but doesn't say on what",
                t.path
            ));
        }
        for p in t.links.iter().filter_map(crate::fields::link_problem) {
            out.push(format!("{queue}: {} has {p}", t.path));
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

/// The status folder a task file is in, or "" when it sits in the queue folder (flat layout).
pub fn folder_of(root: &Path, path: &Path) -> String {
    match path.parent() {
        Some(p) if p != root => p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// One task, parsed.
pub fn read_task(root: &Path, id: &str) -> Result<Task, String> {
    let text = read_body(root, id)?;
    let path = find_task(root, id)?;
    parse_task("", root, &folder_of(root, &path), &path, &text, 0)
        .ok_or_else(|| format!("task {id} has no front matter"))
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

/// Who's acting, as it goes into the front matter and the log: one line of at most 100
/// characters, and nothing that would let a log line pass for tool-written stage history.
pub fn actor(who: &str) -> Result<String, String> {
    let t: String = who
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() {
        return Err("who (--as) is empty".into());
    }
    if t.chars().count() > 100 {
        return Err("who (--as) is longer than 100 characters".into());
    }
    if t.contains(": stage ") || t.contains(" → ") {
        return Err(format!(
            "who (--as) can't contain \": stage \" or \" → \" (that's stage history): {t:?}"
        ));
    }
    Ok(t)
}

/// Contract rule 3, as text goes into a task: every line after the first is indented by two
/// spaces (a Markdown list continuation), so no line of a question or an answer can start a
/// heading or a new item and end the item it belongs to.
pub(crate) fn indent_text(text: &str) -> String {
    text.replace('\n', "\n  ")
}

/// The text as it was written: the two-space continuation indent of `indent_text` taken off.
pub(crate) fn unindent_text(text: &str) -> String {
    text.replace("\n  ", "\n")
}

/// Contract rule 3: control characters out (newline kept), trimmed, capped.
pub fn clean_text(text: &str) -> Result<String, String> {
    clean_text_as(text, "the answer")
}

/// `clean_text`, with `what` naming the text in the refusal ("the question").
pub(crate) fn clean_text_as(text: &str, what: &str) -> Result<String, String> {
    let t: String = text
        .chars()
        .filter(|&c| c == '\n' || !(c < '\u{20}' || c == '\u{7f}'))
        .collect();
    let t = t.trim().to_string();
    if t.is_empty() {
        return Err(format!("{what} is empty"));
    }
    if t.chars().count() > MAX_ANSWER_CHARS {
        return Err(format!(
            "{what} is longer than {MAX_ANSWER_CHARS} characters"
        ));
    }
    Ok(t)
}

/// Contract rule 4: a session's mailbox name, the first 8 characters of its id in lowercase,
/// which must be hex (a Claude Code or Codex session id).
pub fn session8(session: &str) -> Result<String, String> {
    let s = session.trim().to_lowercase();
    if s.is_empty() {
        return Err("no session id: pass --session or run inside Claude Code".into());
    }
    let short: String = s.chars().take(8).collect();
    if short.len() == 8 && short.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(short)
    } else {
        Err(format!(
            "session id must start with 8 hex characters (a Claude Code or Codex session id), got {:?}",
            session.chars().take(20).collect::<String>()
        ))
    }
}

/// The first `## Questions` section's content: (start, end) byte offsets, from the line after
/// the heading to the next `## ` line or the end.
fn questions_span(body: &str) -> Option<(usize, usize)> {
    let mut start = None;
    let mut pos = 0;
    for line in body.split_inclusive('\n') {
        match start {
            Some(s) if line.starts_with("## ") => return Some((s, pos)),
            None if line.ends_with('\n')
                && line.trim_end_matches('\n').trim_end_matches([' ', '\t']) == "## Questions" =>
            {
                start = Some(pos + line.len());
            }
            _ => {}
        }
        pos += line.len();
    }
    start.map(|s| (s, body.len()))
}

/// The task text with a `## Questions` section: as is, or with an empty one added before the
/// tool's Comments (`comments_span`), else before the Log (the last `## Log`, the one
/// `append_log` writes to), or at the end without either.
fn with_questions(text: &str) -> String {
    if questions_span(text).is_some() {
        return text.to_string();
    }
    let tail = comments_span(text)
        .map(|(at, _, _)| at)
        .or_else(|| log_heading(text));
    match tail {
        Some(at) => format!("{}## Questions\n\n{}", &text[..at], &text[at..]),
        None => format!("{}\n\n## Questions\n\n", text.trim_end_matches('\n')),
    }
}

/// `mail/asks.json` (contract rule 4) to add to: empty when there's none yet. A file that isn't
/// a JSON object is refused rather than overwritten, since other askers' entries are in it.
fn read_asks(mail: &Path) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let file = mail.join("asks.json");
    let text = match fs::read_to_string(&file) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Default::default()),
        Err(e) => return Err(format!("mail/asks.json: {e}")),
    };
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(serde_json::Value::Object(map)) => Ok(map),
        _ => Err("mail/asks.json isn't a JSON object of asks; fix or remove it first".into()),
    }
}

/// What `ask` did: the question's number, and the questions the task had before it as
/// (answered, first line).
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Asked {
    pub n: u32,
    pub earlier: Vec<(bool, String)>,
}

/// Ask question `text` in task `id` as `who` from `session`: numbered on from the task's
/// questions, written as an item in `## Questions` (lines after the first indented, contract
/// rule 3), logged, and its asker recorded in `mail/asks.json` so the answer reaches that
/// session. Refused when a question already answered in the task contains the same words.
pub fn ask(
    root: &Path,
    id: &str,
    text: &str,
    who: &str,
    session: &str,
    stamp: &str,
) -> Result<Asked, String> {
    let who = actor(who)?;
    let question = clean_text_as(text, "the question")?;
    let sid = session8(session)?;
    let mail = root.join("mail");
    with_lock(root, || {
        let path = find_task(root, id)?;
        let mut asks = read_asks(&mail)?;
        let body = with_questions(&read_body(root, id)?);
        let (start, end) = questions_span(&body).ok_or("the task's Questions section is gone")?;
        let sect = &body[start..end];
        let items: Vec<Question> = question_items(sect)
            .into_iter()
            .map(|(a, b)| parse_question(&sect[a..b]))
            .collect();
        let raw: Vec<&str> = question_items(sect)
            .into_iter()
            .map(|(a, b)| &sect[a..b])
            .collect();
        let wanted = question.to_lowercase();
        for (q, item) in items.iter().zip(&raw) {
            let shown = unindent_text(item);
            if q.ticked && shown.to_lowercase().contains(&wanted) {
                return Err(format!("already answered in {id}:\n{shown}"));
            }
        }
        let highest = items.iter().filter_map(|q| q.n).max().unwrap_or(0);
        let count = u32::try_from(items.len()).unwrap_or(u32::MAX);
        let n = highest
            .max(count)
            .checked_add(1)
            .ok_or("the task has too many questions")?;
        let item = format!(
            "- [ ] **Q{n} ({who}, session {sid}, {stamp}):** {}\n",
            indent_text(&question)
        );
        let kept = sect.trim_end_matches('\n');
        let new_sect = if kept.trim().is_empty() {
            format!("{item}\n")
        } else {
            format!("{kept}\n{item}\n")
        };
        let mut new_body = append_log(
            &format!("{}{new_sect}{}", &body[..start], &body[end..]),
            &format!("{stamp} {who}: asked Q{n}"),
        );
        // The holder asking is the holder being heard from: it keeps the claim alive.
        let held =
            parse_task("", root, &folder_of(root, &path), &path, &body, 0).is_some_and(|t| {
                t.stage == "building" && !t.claimed_by.is_empty() && t.claimed_by == who
            });
        if held {
            new_body = crate::ops::set_front(&new_body, "heartbeat", stamp)?;
        }
        // The asker first: a question without its entry is still answered by the session in
        // its line, while an entry without its question is only overwritten by the next ask.
        asks.insert(format!("{id}-Q{n}"), serde_json::Value::String(sid.clone()));
        let reg = serde_json::to_string_pretty(&asks).map_err(|e| e.to_string())?;
        write_atomic(&mail.join("asks.json"), &format!("{reg}\n"))?;
        write_atomic(&path, &new_body)?;
        let earlier = items
            .iter()
            .zip(&raw)
            .map(|(q, item)| {
                let first = item.lines().next().unwrap_or("");
                (q.ticked, first.chars().take(200).collect())
            })
            .collect();
        Ok(Asked { n, earlier })
    })
}

/// Mark question `n` of task `id` "Escalated to owner" (capture takes it to the owner), once.
pub fn escalate(root: &Path, id: &str, n: u32, who: &str, stamp: &str) -> Result<(), String> {
    let who = actor(who)?;
    with_lock(root, || {
        let path = find_task(root, id)?;
        let body = read_body(root, id)?;
        let no_q = || format!("no Q{n} in {id}");
        let (start, end) = questions_span(&body).ok_or_else(no_q)?;
        let sect = &body[start..end];
        let (a, b) = question_items(sect)
            .into_iter()
            .find(|&(a, b)| parse_question(&sect[a..b]).n == Some(n))
            .ok_or_else(no_q)?;
        let item = &sect[a..b];
        if item.contains("Escalated to owner") {
            return Err(format!("Q{n} is already escalated"));
        }
        let new_body = append_log(
            &format!(
                "{}{item}\n  **Escalated to owner ({who}, {stamp}).**{}",
                &body[..start + a],
                &body[start + b..]
            ),
            &format!("{stamp} {who}: escalated Q{n} to the owner"),
        );
        write_atomic(&path, &new_body)
    })
}

/// The answers waiting in `session`'s mailbox (contract rule 5), oldest name first. Each is
/// moved to `delivered/` as it's taken, unless `keep`; one another reader moved first is skipped,
/// so an answer is handed over once.
pub fn take_mail(root: &Path, session: &str, keep: bool) -> Result<Vec<String>, String> {
    let dir = root.join("mail").join(session8(session)?);
    let entries = match fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", dir.display())),
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".md"))
        .collect();
    names.sort();
    let mut out = Vec::new();
    for name in names {
        let from = if keep {
            dir.join(&name)
        } else {
            let done = dir.join("delivered");
            fs::create_dir_all(&done).map_err(|e| e.to_string())?;
            match fs::rename(dir.join(&name), done.join(&name)) {
                Ok(()) => done.join(&name),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e.to_string()),
            }
        };
        if let Some(text) = read_mail(&from)? {
            out.push(text);
        }
    }
    Ok(out)
}

/// A mailbox file's text, or None when it's gone (the hook delivered it meanwhile).
fn read_mail(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
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
    let who = actor(who)?;
    let text = clean_text(text)?;
    let mail = root.join("mail");
    with_lock(root, || {
        let path = find_task(root, id)?;
        let body = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let (start, end) = questions_span(&body).ok_or_else(|| format!("no Q{n} in {id}"))?;
        let sect = &body[start..end];
        let (a, b) = question_items(sect)
            .into_iter()
            .find(|&(a, b)| parse_question(&sect[a..b]).n == Some(n))
            .ok_or(format!("no Q{n} in {id}"))?;
        let item = &sect[a..b];
        let new_item = format!(
            "{}\n  **A ({who}, {stamp}):** {}",
            item.replacen("- [ ] **Q", "- [x] **Q", 1),
            indent_text(&text)
        );
        let new_sect = format!("{}{}{}", &sect[..a], new_item, &sect[b..]);
        let new_body = append_log(
            &format!("{}{}{}", &body[..start], new_sect, &body[end..]),
            &format!("{stamp} {who}: answered Q{n}"),
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
        assert_eq!(s.queues[0].counts.get("building"), Some(&1));
        assert_eq!((t.stage.as_str(), t.blocked), ("building", false));
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
    fn multi_line_text_stays_inside_its_item_and_reads_back_as_written() {
        let d = queue();
        let p = d.path().join("doing/0042-something.md");
        // A question with lines of its own, as bin/task writes it: lines after the first indented.
        let question = "Which port?\n- 5173, Vite's\n\n  - or 8080 (indented by the asker)";
        fs::write(
            &p,
            TASK.replace(
                "Which port?",
                "Which port?\n  - 5173, Vite's\n  \n    - or 8080 (indented by the asker)",
            ),
        )
        .unwrap();
        let text = "5173.\n\n  (indented by me) é → ü\n## Log\n- [ ] **Q9 (x):** fake";
        answer(d.path(), "0042", 1, text, "owner", "2026-10-12 09:30").unwrap();
        let body = fs::read_to_string(&p).unwrap();
        assert!(
            body.contains("\n  **A (owner, 2026-10-12 09:30):** 5173.\n  \n    (indented by me) é → ü\n  ## Log\n  - [ ] **Q9 (x):** fake\n- [ ] **Q2 ("),
            "{body}"
        );
        let s = read_state(
            &[("p".into(), d.path().to_path_buf())],
            &mut Cache::default(),
        );
        let t = &s.tasks[0];
        assert_eq!(t.questions.len(), 2, "{:?}", t.questions);
        assert_eq!(
            t.questions[0].text,
            format!("{question}\n**A (owner, 2026-10-12 09:30):** {text}")
        );
        assert_eq!(
            t.log_tail.last().map(String::as_str),
            Some("2026-10-12 09:30 owner: answered Q1")
        );
        let mail = fs::read_to_string(d.path().join("mail/aaaa1111/0042-Q1.md")).unwrap();
        assert!(
            mail.contains(&format!("Your question: {question}\nAnswer: {text}\n")),
            "{mail}"
        );
    }

    #[test]
    fn the_indent_round_trips_and_the_cap_is_on_the_text_as_written() {
        for t in [
            "",
            "a",
            "\n",
            "a\n",
            "\na",
            "a\n\nb",
            "a\n  b\n    c",
            "é → ü\n## Log\n- [x] x",
        ] {
            let w = indent_text(t);
            assert_eq!(unindent_text(&w), t);
            assert!(w.split('\n').skip(1).all(|l| l.starts_with("  ")), "{w:?}");
        }
        // 3,999 characters on 2,000 lines: indented, it's near 8,000 in the file, and it's still
        // taken, because the 4,000 cap is on the text as written.
        let d = queue();
        let long = format!("{}b", "a\n".repeat(1999));
        answer(d.path(), "0042", 1, &long, "owner", "2026-10-12 09:30").unwrap();
        let s = read_state(
            &[("p".into(), d.path().to_path_buf())],
            &mut Cache::default(),
        );
        assert!(s.tasks[0].questions[0]
            .text
            .ends_with(&format!("):** {long}")));
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
    fn dated_inbox_notes_are_not_tasks() {
        assert!(is_task_name("0019-guard-escapes.md"));
        assert!(is_task_name("0020-10-things.md"));
        assert!(!is_task_name("2026-10-12-0900-i18n-followups.md"));
        let d = queue();
        fs::write(
            d.path().join("inbox/2026-10-12-0900-note.md"),
            "a raw note, no front matter",
        )
        .unwrap();
        let s = read_state(
            &[("p".into(), d.path().to_path_buf())],
            &mut Cache::default(),
        );
        assert_eq!(s.tasks.len(), 1);
        assert!(s.problems.is_empty(), "{:?}", s.problems);
    }

    fn parse_one(text: &str, folder: &str) -> Task {
        let root = Path::new("/nonexistent-queue");
        parse_task("p", root, folder, &root.join("0042-x.md"), text, 0).unwrap()
    }

    #[test]
    fn stages_in_the_order_the_app_relies_on() {
        assert_eq!(
            STAGES,
            [
                "backlog",
                "shaping",
                "ready",
                "building",
                "testing",
                "your-check",
                "done"
            ]
        );
        let labels: Vec<_> = STAGES.iter().map(|s| stage_label(s)).collect();
        assert_eq!(
            labels,
            [
                "Backlog",
                "Shaping",
                "Ready",
                "Building",
                "Testing",
                "Your check",
                "Done"
            ]
        );
        let roles: Vec<_> = STAGES.iter().map(|s| stage_role(s)).collect();
        assert_eq!(
            roles,
            [
                "anyone",
                "product + tech + UI agents",
                "",
                "code agent",
                "QA agent",
                "owner",
                ""
            ]
        );
        assert_eq!((stage_label("qa"), stage_role("qa")), ("", ""));
    }

    #[test]
    fn legacy_statuses_map_to_stages() {
        for (status, stage, blocked) in [
            ("inbox", "backlog", false),
            ("todo", "ready", false),
            ("doing", "building", false),
            ("blocked", "ready", true),
            ("done", "done", false),
            ("parked", "backlog", false),
        ] {
            let t = parse_one(
                &TASK.replace("status: doing", &format!("status: {status}")),
                "",
            );
            assert_eq!((t.stage.as_str(), t.blocked), (stage, blocked), "{status}");
        }
        // No status: the folder says it.
        let t = parse_one(&TASK.replace("status: doing\n", ""), "todo");
        assert_eq!((t.status.as_str(), t.stage.as_str()), ("todo", "ready"));
        // A reason to wait is the blocked flag, whatever the status.
        let t = parse_one(&TASK.replace("waiting_on:", "waiting_on: 0012"), "");
        assert_eq!((t.stage.as_str(), t.blocked), ("building", true));
    }

    #[test]
    fn stage_in_the_file_wins_and_serializes_camel_case() {
        let t = parse_one(
            &TASK.replace("status: doing", "status: doing\nstage: Testing"),
            "doing",
        );
        assert_eq!((t.stage.as_str(), t.blocked), ("testing", false));
        let v = serde_json::to_value(&t).unwrap();
        assert_eq!(v["stage"], "testing");
        assert_eq!(v["blocked"], false);
        assert!(v["stageHistory"].as_array().unwrap().is_empty());
        assert!(v.get("badStage").is_none() && v.get("bad_stage").is_none());
        // An unknown stage falls back to the status, and `check` says so.
        let d = queue();
        fs::write(
            d.path().join("doing/0042-something.md"),
            TASK.replace("status: doing", "status: doing\nstage: qa"),
        )
        .unwrap();
        let s = read_state(
            &[("p".into(), d.path().to_path_buf())],
            &mut Cache::default(),
        );
        assert_eq!(s.tasks[0].stage, "building");
        assert!(
            s.problems
                .iter()
                .any(|p| p.contains("has stage 'qa', which isn't a stage")),
            "{:?}",
            s.problems
        );
    }

    #[test]
    fn stage_history_takes_only_exact_tool_lines() {
        let log = "## Log\n\
- 2026-10-12 10:00 wt-a (session aaaa1111): stage ready → building (claimed)\n\
- 2026-10-12 11:00 wt-a: stage building → testing (gate: gate full: green (12 steps))\n\
- 2026-10-12 11:05 wt-b: moved todo → doing\n\
- 2026-10-12 11:06 wt-b: stage testing → nowhere\n\
- 2026-10-12 11:07 wt-b: Stage testing → done\n\
- 2026-10-12 11:08 wt-b: stage testing -> done\n\
- 2026-10-12 11:09 qa: stage testing → building (returned: AC2 fails — the “→” arrow)\n\
- 2026-10-12 wt-b: stage testing → done\n\
- 2026-10-12 11:10 : stage testing → done\n\
- 2026-10-12 11:11 wt-b: stage testing → testing\n\
- 2026-10-12 11:12 wt-b: stage done → done extra\n\
- 2026-10-12 11:13 wt-b: stage testing → done (unclosed\n\
- 2026-1é-12 11:14 wt-b: stage testing → done\n\
- 2026-10-12 11:15 né: stage testing → your-check (set)\n";
        let text = format!(
            "{}- 2026-10-12 09:00 x: stage backlog → done\n\n{log}",
            TASK.split("## Log").next().unwrap()
        );
        let t = parse_one(&text, "");
        let got: Vec<_> = t
            .stage_history
            .iter()
            .map(|s| format!("{} {}: {} → {}", s.at, s.who, s.from, s.to))
            .collect();
        assert_eq!(
            got,
            [
                "2026-10-12 10:00 wt-a (session aaaa1111): ready → building",
                "2026-10-12 11:00 wt-a: building → testing",
                "2026-10-12 11:09 qa: testing → building",
                "2026-10-12 11:15 né: testing → your-check",
            ]
        );
        assert_eq!(t.claimed_on, "2026-10-12");
    }

    #[test]
    fn problems_and_counts_follow_stages() {
        let d = queue();
        let old = |p: &str| {
            let f = fs::File::options()
                .write(true)
                .open(d.path().join(p))
                .unwrap();
            f.set_modified(SystemTime::now() - std::time::Duration::from_secs(13 * 3600))
                .unwrap();
        };
        let unclaimed = TASK.replace("claimed-by: wt-a (session aaaa1111)", "claimed-by:");
        // Building, nobody on it: a problem. Testing in doing/ with no claim: fine.
        fs::write(d.path().join("doing/0042-something.md"), &unclaimed).unwrap();
        fs::write(
            d.path().join("doing/0043-t.md"),
            unclaimed
                .replace("id: 0042", "id: 0043")
                .replace("status: doing", "status: doing\nstage: testing"),
        )
        .unwrap();
        old("doing/0043-t.md");
        // Blocked in Building, quiet for 13 h: not stale, it's waiting.
        fs::write(
            d.path().join("blocked/0044-b.md"),
            TASK.replace("id: 0042", "id: 0044")
                .replace("status: doing", "status: blocked\nstage: building")
                .replace("waiting_on:", "waiting_on: owner"),
        )
        .unwrap();
        old("blocked/0044-b.md");
        fs::write(
            d.path().join("doing/0045-y.md"),
            TASK.replace("id: 0042", "id: 0045")
                .replace("status: doing", "status: doing\nstage: your-check")
                .replace("**Escalated to owner (capture, 2026-10-11 10:06).**\n", ""),
        )
        .unwrap();
        // Claimed, not blocked, in Building and quiet for 13 h: stale.
        fs::write(
            d.path().join("doing/0046-q.md"),
            TASK.replace("id: 0042", "id: 0046")
                .replace("status: doing", "status: doing\nstage: building"),
        )
        .unwrap();
        old("doing/0046-q.md");
        // Claimed by hand (mv todo/ doing/ and status edited): the stage still says Ready.
        fs::write(
            d.path().join("doing/0047-h.md"),
            TASK.replace("id: 0042", "id: 0047")
                .replace("status: doing", "status: doing\nstage: ready"),
        )
        .unwrap();
        let s = read_state(
            &[("p".into(), d.path().to_path_buf())],
            &mut Cache::default(),
        );
        let all = s.problems.join("\n");
        assert!(
            all.contains("0042-something.md is being worked on but nobody has claimed it"),
            "{all}"
        );
        assert!(
            all.contains("doing/0046-q.md is claimed but hasn't been heard from in 13 h"),
            "{all}"
        );
        assert!(
            all.contains("doing/0047-h.md says stage ready but status doing (moved by hand? use `muster stage`)"),
            "{all}"
        );
        assert_eq!(s.problems.len(), 3, "{all}");
        let c = &s.queues[0].counts;
        assert_eq!(
            (
                c.get("building"),
                c.get("testing"),
                c.get("your-check"),
                c.get("blocked"),
                c.get("doing")
            ),
            (Some(&3), Some(&1), Some(&1), Some(&1), None)
        );
        let y = s.tasks.iter().find(|t| t.id == "0045").unwrap();
        assert!(y.waiting_for_owner, "Your check needs the owner");
        assert_eq!(y.open_questions, 2);
    }

    #[test]
    fn check_calls_a_claim_quiet_by_the_same_lease_as_reclaim() {
        let now = 2_000_000_000;
        let stamp = "2026-10-12 14:05";
        let held = |id: &str, hb: &str, modified: u64| {
            let text = TASK
                .replace("id: 0042", &format!("id: {id}"))
                .replace("heartbeat:", &format!("heartbeat: {hb}"));
            let root = Path::new("/nonexistent-queue");
            parse_task(
                "p",
                root,
                "doing",
                &root.join(format!("doing/{id}-x.md")),
                &text,
                modified,
            )
            .unwrap()
        };
        let tasks = [
            // An old heartbeat, though the file just changed (an answer, say): quiet.
            held("0001", "2026-10-12 01:00", now - 60),
            // A fresh heartbeat in an old file: live.
            held("0002", "2026-10-12 13:00", now - 99 * 3600),
            // No heartbeat: the file's time.
            held("0003", "", now - 13 * 3600),
            // Exactly the lease: not yet.
            held("0004", "2026-10-12 02:05", now - 99 * 3600),
        ];
        let quiet: Vec<_> = problems("p", &tasks, now, stamp)
            .into_iter()
            .filter(|p| p.contains("heard from"))
            .collect();
        assert_eq!(
            quiet,
            [
                "p: doing/0001-x.md is claimed but hasn't been heard from in 13 h",
                "p: doing/0003-x.md is claimed but hasn't been heard from in 13 h"
            ]
        );
    }

    #[test]
    fn state_judges_a_claims_lease_at_the_time_given() {
        let d = queue();
        fs::write(
            d.path().join("doing/0042-something.md"),
            TASK.replace("heartbeat:", "heartbeat: 2026-10-12 01:00"),
        )
        .unwrap();
        let q = [("p".to_string(), d.path().to_path_buf())];
        let quiet = |stamp: &str| {
            read_state_at(&q, &mut Cache::default(), stamp)
                .problems
                .into_iter()
                .filter(|p| p.contains("heard from"))
                .count()
        };
        assert_eq!(quiet("2026-10-12 13:00"), 0);
        assert_eq!(quiet("2026-10-12 13:01"), 1);
    }

    #[test]
    fn criteria_counts() {
        assert_eq!(
            criteria("# T\n\n## Acceptance criteria\n- [ ] AC1\n- [x] AC2\n- [X] AC3\n  - [ ] a sub-step\n\n## Log\n- [ ] not a criterion\n"),
            (3, 2)
        );
        assert_eq!(criteria("# T\n\n- [ ] outside\n"), (0, 0));
    }

    /// A task without questions, as most start.
    const PLAIN: &str = "---\nid: 0007\ntype: task\npriority: normal\nstatus: doing\nstage: building\nclaimed-by: wt-a\nheartbeat:\nwaiting_on:\n---\n\n# Plain\n\n**Details:** synthetic.\n\n## Log\n- 2026-10-11 captured\n";
    const S: &str = "2026-10-12 09:00";

    fn plain() -> tempfile::TempDir {
        let d = queue();
        fs::write(d.path().join("doing/0007-plain.md"), PLAIN).unwrap();
        d
    }
    fn body_of(d: &Path, id: &str) -> String {
        read_body(d, id).unwrap()
    }

    #[test]
    fn session_ids_become_mailbox_names() {
        assert_eq!(session8("aaaa1111-full-id").unwrap(), "aaaa1111");
        assert_eq!(session8(" ABCD1234-5678 ").unwrap(), "abcd1234");
        assert!(session8("").unwrap_err().contains("no session id"));
        for bad in ["codex-ru", "a1", "../../etc", "aaaa111g", "é"] {
            assert!(session8(bad).unwrap_err().contains("8 hex"), "{bad:?}");
        }
    }

    #[test]
    fn ask_writes_the_question_above_the_log_and_records_the_asker() {
        let d = plain();
        let a = ask(d.path(), "0007", "Which port?", "wt-a", "aaaa1111-x", S).unwrap();
        assert_eq!((a.n, a.earlier.len()), (1, 0));
        let b = body_of(d.path(), "0007");
        assert!(
            b.contains("**Details:** synthetic.\n\n## Questions\n- [ ] **Q1 (wt-a, session aaaa1111, 2026-10-12 09:00):** Which port?\n\n## Log\n"),
            "{b}"
        );
        assert!(b.ends_with("- 2026-10-12 09:00 wt-a: asked Q1\n"), "{b}");
        let reg: HashMap<String, String> =
            serde_json::from_str(&fs::read_to_string(d.path().join("mail/asks.json")).unwrap())
                .unwrap();
        assert_eq!(reg.get("0007-Q1").map(String::as_str), Some("aaaa1111"));
        // The next one is numbered on, in the same section, and lists the first.
        let a = ask(d.path(), "0007", "And the colour?", "wt-b", "bbbb2222", S).unwrap();
        assert_eq!(a.n, 2);
        assert_eq!(
            a.earlier,
            vec![(
                false,
                "- [ ] **Q1 (wt-a, session aaaa1111, 2026-10-12 09:00):** Which port?".to_string()
            )]
        );
        let b = body_of(d.path(), "0007");
        assert_eq!(b.matches("## Questions").count(), 1);
        assert!(b.contains("Which port?\n- [ ] **Q2 (wt-b, session bbbb2222, "));
        let t = read_task(d.path(), "0007").unwrap();
        assert_eq!((t.questions.len(), t.open_questions), (2, 2));
        // The app's answer reaches the asker recorded at ask time.
        assert_eq!(
            answer(d.path(), "0007", 2, "Violet.", "owner", S)
                .unwrap()
                .as_deref(),
            Some("bbbb2222")
        );
    }

    #[test]
    fn ask_refuses_what_is_already_answered_and_shows_it_as_written() {
        let d = plain();
        let q = "Which layout?\n- A: a window\n\n  (indented by me) é → ü";
        ask(d.path(), "0007", q, "wt-a", "aaaa1111", S).unwrap();
        answer(d.path(), "0007", 1, "B.\n## Log", "capture", S).unwrap();
        let before = body_of(d.path(), "0007");
        let e = ask(d.path(), "0007", "WHICH LAYOUT?", "wt-b", "bbbb2222", S).unwrap_err();
        assert!(
            e.starts_with("already answered in 0007:\n- [x] **Q1 ("),
            "{e}"
        );
        assert!(
            e.contains(&format!(":** {q}\n**A (capture, {S}):** B.\n## Log")),
            "{e}"
        );
        assert_eq!(body_of(d.path(), "0007"), before);
        // An open question with the same words doesn't block asking again.
        ask(d.path(), "0007", "Open one?", "wt-a", "aaaa1111", S).unwrap();
        ask(d.path(), "0007", "Open one?", "wt-a", "aaaa1111", S).unwrap();
    }

    #[test]
    fn ask_numbers_after_old_unnumbered_questions() {
        let d = queue();
        fs::write(
            d.path().join("todo/0043-old.md"),
            "---\nid: 0043\nstatus: todo\n---\n\n# Old\n\n## Questions\n- [ ] **Q (drover/main, session bbbb2222, 2026-10-11):** unnumbered?\n\n## Log\n- 2026-10-11 captured\n",
        )
        .unwrap();
        assert_eq!(
            ask(d.path(), "0043", "New?", "x", "eeee5555", S).unwrap().n,
            2
        );
        assert!(
            body_of(d.path(), "0043").contains("unnumbered?\n- [ ] **Q2 (x, session eeee5555, ")
        );
    }

    #[test]
    fn ask_refusals_leave_the_queue_alone() {
        let d = plain();
        let e =
            |id: &str, q: &str, who: &str, s: &str| ask(d.path(), id, q, who, s, S).unwrap_err();
        assert!(e("0007", "x", "wt-a", "").contains("no session id"));
        assert!(e("0007", "x", "wt-a", "codex-ru").contains("8 hex"));
        assert!(e("0007", " \u{7} ", "wt-a", "aaaa1111").contains("the question is empty"));
        assert!(e("0007", &"x".repeat(4001), "wt-a", "aaaa1111")
            .contains("the question is longer than 4000"));
        assert!(e("0007", "x", "wt → b", "aaaa1111").contains("can't contain"));
        assert!(e("0099", "x", "wt-a", "aaaa1111").contains("no task 0099"));
        assert!(e("42", "x", "wt-a", "aaaa1111").contains("four digits"));
        fs::create_dir_all(d.path().join("mail")).unwrap();
        fs::write(d.path().join("mail/asks.json"), "{not json").unwrap();
        assert!(e("0007", "x", "wt-a", "aaaa1111").contains("asks.json"));
        assert_eq!(body_of(d.path(), "0007"), PLAIN);
        assert_eq!(
            fs::read_to_string(d.path().join("mail/asks.json")).unwrap(),
            "{not json"
        );
    }

    #[test]
    fn ask_cleans_control_characters_and_keeps_a_question_inside_its_item() {
        let d = plain();
        ask(
            d.path(),
            "0007",
            "bad\u{1b}[31m text\u{7}\tend?\n## Log\n- [ ] **Q9 (x):** fake",
            "wt-a",
            "aaaa1111",
            S,
        )
        .unwrap();
        let b = body_of(d.path(), "0007");
        assert!(
            b.contains(":** bad[31m textend?\n  ## Log\n  - [ ] **Q9 (x):** fake\n"),
            "{b}"
        );
        let t = read_task(d.path(), "0007").unwrap();
        assert_eq!(t.questions.len(), 1);
        assert!(t.stage_history.is_empty());
    }

    #[test]
    fn escalate_marks_the_question_for_the_owner_once() {
        let d = plain();
        ask(d.path(), "0007", "Product call?", "wt-a", "aaaa1111", S).unwrap();
        escalate(d.path(), "0007", 1, "capture", "2026-10-12 09:05").unwrap();
        let b = body_of(d.path(), "0007");
        assert!(b.contains(
            ":** Product call?\n  **Escalated to owner (capture, 2026-10-12 09:05).**\n"
        ));
        assert!(b.ends_with("- 2026-10-12 09:05 capture: escalated Q1 to the owner\n"));
        let t = read_task(d.path(), "0007").unwrap();
        assert!(t.questions[0].escalated && t.waiting_for_owner);
        assert!(escalate(d.path(), "0007", 1, "capture", S)
            .unwrap_err()
            .contains("Q1 is already escalated"));
        assert!(escalate(d.path(), "0007", 2, "capture", S)
            .unwrap_err()
            .contains("no Q2 in 0007"));
        let d = plain();
        assert!(escalate(d.path(), "0007", 1, "capture", S)
            .unwrap_err()
            .contains("no Q1 in 0007"));
        assert_eq!(
            body_of(d.path(), "0007"),
            PLAIN,
            "no Questions section added"
        );
        assert!(answer(d.path(), "0007", 1, "x", "owner", S)
            .unwrap_err()
            .contains("no Q1 in 0007"));
    }

    #[test]
    fn inbox_hands_over_each_answer_once_unless_kept() {
        let d = plain();
        ask(d.path(), "0007", "Which port?", "wt-a", "aaaa1111", S).unwrap();
        ask(d.path(), "0007", "Which colour?", "wt-a", "aaaa1111", S).unwrap();
        assert!(take_mail(d.path(), "aaaa1111", false).unwrap().is_empty());
        answer(d.path(), "0007", 2, "Violet.", "owner", S).unwrap();
        answer(d.path(), "0007", 1, "5173.", "capture", S).unwrap();
        assert!(take_mail(d.path(), "bbbb2222", false).unwrap().is_empty());
        let kept = take_mail(d.path(), "AAAA1111-full", true).unwrap();
        assert_eq!(kept.len(), 2);
        assert!(kept[0].contains("Answer to Q1 on task 0007 (Plain), from capture:\nYour question: Which port?\nAnswer: 5173.\n"));
        assert!(kept[1].contains("Answer: Violet."));
        assert_eq!(take_mail(d.path(), "aaaa1111", false).unwrap(), kept);
        assert!(take_mail(d.path(), "aaaa1111", false).unwrap().is_empty());
        assert!(d
            .path()
            .join("mail/aaaa1111/delivered/0007-Q1.md")
            .is_file());
        assert!(take_mail(d.path(), "../../etc", false)
            .unwrap_err()
            .contains("8 hex"));
        // One the hook moved away between the listing and the read is skipped, not an error.
        assert_eq!(
            read_mail(&d.path().join("mail/aaaa1111/0007-Q9.md")).unwrap(),
            None
        );
        assert!(read_mail(&d.path().join("mail/aaaa1111"))
            .unwrap_err()
            .contains("aaaa1111"));
    }

    #[test]
    fn criteria_items_read_labels_ticks_and_whose_they_are() {
        let body = concat!(
            "# T\n\n## Acceptance criteria\n",
            "- [x] AC1: works — verify: command (test)\n",
            "- [ ] AC2: the owner tried it — verify: owner\n",
            "- [ ] AC3: looks right in light and dark, wrapped onto the next line. —\n",
            "  verify: owner (screenshots in attachments/0026/)\n",
            "- [ ] AC4: a test, then a look — verify: command (test) and owner\n",
            "- [X] no label, ticked — verify: Owner\n",
            "  - [ ] a sub-step, part of the item above\n",
            "Some prose.\n",
            "  verify: owner (indented, but after prose: not part of any item)\n",
            "- [ ] AC10: says nothing about who verifies\n",
            "\n## Log\n- [ ] not a criterion\n",
        );
        let items = criteria_items(body);
        let got: Vec<_> = items
            .iter()
            .map(|c| (c.n, c.label.as_str(), c.ticked, c.owner))
            .collect();
        assert_eq!(
            got,
            vec![
                (1, "AC1", true, false),
                (2, "AC2", false, true),
                (3, "AC3", false, true),
                (4, "AC4", false, false),
                (5, "", true, true),
                (6, "AC10", false, false),
            ]
        );
        assert!(
            items[2]
                .text
                .ends_with("— verify: owner (screenshots in attachments/0026/)"),
            "{}",
            items[2].text
        );
        assert!(
            items[4]
                .text
                .ends_with("- [ ] a sub-step, part of the item above"),
            "{}",
            items[4].text
        );
        assert!(!items[5].text.contains("indented"), "{}", items[5].text);
        // Each offset points at its box in the body.
        for c in &items {
            assert!(
                body[c.at..].starts_with("- ["),
                "{}: {:?}",
                c.n,
                &body[c.at..c.at + 10]
            );
        }
        assert_eq!(criteria(body), (6, 2));
    }

    #[test]
    fn a_label_is_ac_and_digits_only() {
        let labels = |b: &str| {
            criteria_items(b)
                .into_iter()
                .map(|c| c.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels(
                "## Acceptance criteria\n- [ ] AC: x\n- [ ] ACT1: x\n- [ ] AC12 x\n- [ ] ac3: x\n"
            ),
            vec!["", "", "AC12", ""]
        );
    }

    #[test]
    fn the_task_carries_only_the_owners_criteria() {
        let d = tempfile::tempdir().unwrap();
        let text = "---\nid: 0001\nstatus: doing\nstage: your-check\n---\n\n# T\n\n## Acceptance criteria\n- [x] AC1: a — verify: command\n- [ ] AC2: b — verify: owner\n\n## Log\n";
        let path = d.path().join("doing/0001-t.md");
        let t = parse_task("q", d.path(), "doing", &path, text, 0).unwrap();
        assert_eq!(
            t.owner_criteria
                .iter()
                .map(|c| c.label.as_str())
                .collect::<Vec<_>>(),
            vec!["AC2"]
        );
        let json = serde_json::to_value(&t).unwrap();
        assert_eq!(json["ownerCriteria"][0]["label"], "AC2");
        assert_eq!(json["ownerCriteria"][0]["owner"], true);
        assert!(json["ownerCriteria"][0].get("at").is_none());
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
