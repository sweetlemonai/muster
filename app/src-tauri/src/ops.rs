//! Changing tasks: the one implementation of Muster's rules for creating and managing tasks,
//! used by the app's window and by the `muster` command line alike. Every change holds the
//! queue's lock, writes atomically, stamps the time itself and logs who did it.

use crate::queue::{
    self, actor, append_log, find_task, front_matter, mirror_status, with_lock, write_atomic,
    STATUS_FOLDERS,
};
pub use crate::queue::{stage_label, stage_role, StageStep, STAGES};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const KINDS: [&str; 3] = ["bug", "task", "idea"];
pub const PRIORITIES: [&str; 3] = ["high", "normal", "low"];
/// Statuses the owner may set. `doing` is a claim, and claims belong to agents.
pub const OWNER_STATUSES: [&str; 4] = ["inbox", "todo", "blocked", "done"];
const MAX_TITLE: usize = 200;
const MAX_BODY: usize = 50_000;
const MAX_ATTACHMENT: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct NewTask {
    pub title: String,
    pub kind: String,
    pub priority: String,
    pub body: String,
    pub tags: Vec<String>,
    pub est_time: String,
    pub est_tokens: String,
}

/// Agent time: a number and m, h or d ("45m", "1.5h", "2d"). Empty is fine (no estimate).
pub fn clean_time(input: &str) -> Result<String, String> {
    let t = input.trim().to_lowercase().replace(' ', "");
    if t.is_empty() {
        return Ok(t);
    }
    let (num, unit) = t.split_at(t.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(t.len()));
    let unit = match unit {
        "m" | "min" | "mins" => "m",
        "h" | "hr" | "hrs" | "hour" | "hours" => "h",
        "d" | "day" | "days" => "d",
        _ => {
            return Err(format!(
                "time {input:?}: use a number and m, h or d, like 45m, 2h or 1.5d"
            ))
        }
    };
    let n: f64 = num
        .parse()
        .map_err(|_| format!("time {input:?}: use a number and m, h or d, like 45m, 2h or 1.5d"))?;
    if !(n > 0.0 && n <= 1000.0) {
        return Err(format!("time {input:?} is out of range"));
    }
    Ok(format!("{}{unit}", trim_num(n)))
}

/// Tokens: a number with an optional k or M ("400k", "1.2M", "50000"). Empty is fine.
pub fn clean_tokens(input: &str) -> Result<String, String> {
    let t = input.trim().replace([' ', ','], "");
    if t.is_empty() {
        return Ok(t);
    }
    let (num, unit) = t.split_at(t.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(t.len()));
    let mult = match unit.to_lowercase().as_str() {
        "" => 1.0,
        "k" => 1e3,
        "m" => 1e6,
        _ => {
            return Err(format!(
                "tokens {input:?}: use a number with k or M, like 400k or 1.2M"
            ))
        }
    };
    let n: f64 = num
        .parse()
        .map_err(|_| format!("tokens {input:?}: use a number with k or M, like 400k or 1.2M"))?;
    let total = n * mult;
    if !(1.0..=1e9).contains(&total) {
        return Err(format!("tokens {input:?} is out of range"));
    }
    Ok(if total >= 1e6 {
        format!("{}M", trim_num(total / 1e6))
    } else if total >= 1e3 {
        format!("{}k", trim_num(total / 1e3))
    } else {
        trim_num(total)
    })
}

fn trim_num(n: f64) -> String {
    let s = format!("{:.1}", n);
    s.trim_end_matches(".0").to_string()
}

/// Minutes in a cleaned time ("1.5h" → 90), for totals.
pub fn time_minutes(t: &str) -> u64 {
    let Some(unit) = t.chars().last() else {
        return 0;
    };
    let n: f64 = t[..t.len() - 1].parse().unwrap_or(0.0);
    (n * match unit {
        'm' => 1.0,
        'h' => 60.0,
        'd' => 480.0,
        _ => 0.0,
    })
    .round() as u64
}

/// Tokens in a cleaned count ("1.2M" → 1_200_000), for totals.
pub fn token_count(t: &str) -> u64 {
    let (num, mult) = match t.chars().last() {
        Some('k') => (&t[..t.len() - 1], 1e3),
        Some('M') => (&t[..t.len() - 1], 1e6),
        _ => (t, 1.0),
    };
    (num.parse::<f64>().unwrap_or(0.0) * mult).round() as u64
}

/// Tags the app suggests. Any other tag works too.
pub const SUGGESTED_TAGS: [&str; 10] = [
    "spike",
    "chore",
    "tech-debt",
    "code",
    "docs",
    "design",
    "research",
    "test",
    "perf",
    "security",
];
const MAX_TAGS: usize = 8;

/// Clean a list of tags: lowercase, spaces and underscores become hyphens, only letters, digits
/// and hyphens kept, 24 characters at most, no duplicates, 8 tags at most.
pub fn clean_tags(input: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for raw in input.iter().flat_map(|t| t.split(',')) {
        let t: String = raw
            .trim()
            .to_lowercase()
            .chars()
            .map(|c| if c == ' ' || c == '_' { '-' } else { c })
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect();
        let t = t.trim_matches('-').to_string();
        if t.is_empty() {
            continue;
        }
        if t.chars().count() > 24 {
            return Err(format!("the tag {t:?} is longer than 24 characters"));
        }
        if !out.contains(&t) {
            out.push(t);
        }
    }
    if out.len() > MAX_TAGS {
        return Err(format!("a task can have at most {MAX_TAGS} tags"));
    }
    Ok(out)
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Created {
    pub id: String,
    pub path: String,
}

fn one_line(text: &str, max: usize, what: &str) -> Result<String, String> {
    let t: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() {
        return Err(format!("{what} is empty"));
    }
    if t.chars().count() > max {
        return Err(format!("{what} is longer than {max} characters"));
    }
    Ok(t)
}

fn body_text(text: &str) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok(String::new());
    }
    let t: String = text
        .chars()
        .filter(|&c| c == '\n' || c == '\t' || !(c < '\u{20}' || c == '\u{7f}'))
        .collect();
    if t.chars().count() > MAX_BODY {
        return Err(format!(
            "the description is longer than {MAX_BODY} characters"
        ));
    }
    Ok(t.trim().to_string())
}

fn pick<'a>(
    value: &str,
    allowed: &[&'a str],
    default: &'a str,
    what: &str,
) -> Result<&'a str, String> {
    if value.is_empty() {
        return Ok(default);
    }
    allowed
        .iter()
        .find(|a| a.eq_ignore_ascii_case(value))
        .copied()
        .ok_or_else(|| format!("{what} must be one of {}", allowed.join(", ")))
}

/// A file-name slug: lowercase ASCII words, cut at a word boundary under 48 characters.
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    for word in title
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        if out.len() + word.len() + 1 > 48 {
            break;
        }
        if !out.is_empty() {
            out.push('-');
        }
        out.push_str(word);
    }
    if out.is_empty() {
        "task".into()
    } else {
        out
    }
}

/// The next free id: one more than the highest in any task file (four digits).
pub fn next_id(root: &Path) -> Result<String, String> {
    // The id written inside each task file, falling back to its file name.
    let max = queue::task_files(root)
        .iter()
        .filter_map(|(_, p)| {
            let text = std::fs::read_to_string(p).ok();
            text.as_deref()
                .and_then(front_matter)
                .and_then(|(fm, _)| fm.get("id").and_then(|i| i.parse::<u32>().ok()))
                .or_else(|| p.file_name()?.to_str()?.get(..4)?.parse::<u32>().ok())
        })
        .max()
        .unwrap_or(0);
    if max >= 9999 {
        return Err("the queue is out of four-digit ids".into());
    }
    Ok(format!("{:04}", max + 1))
}

/// Is this queue laid out with status folders (v2) or flat?
fn has_status_folders(root: &Path) -> bool {
    root.join("todo").is_dir()
}

fn folder_for(root: &Path, status: &str) -> PathBuf {
    if has_status_folders(root) {
        root.join(status)
    } else {
        root.to_path_buf()
    }
}

/// Create a task in Ready (`todo/`, or the queue folder in the flat layout). `stamp` is local time.
pub fn create_task(root: &Path, new: &NewTask, who: &str, stamp: &str) -> Result<Created, String> {
    let who = actor(who)?;
    let title = one_line(&new.title, MAX_TITLE, "the title")?;
    let kind = pick(&new.kind, &KINDS, "task", "type")?;
    let priority = pick(&new.priority, &PRIORITIES, "normal", "priority")?;
    let body = body_text(&new.body)?;
    let tags = clean_tags(&new.tags)?.join(", ");
    let tags = if tags.is_empty() {
        String::new()
    } else {
        format!(" {tags}")
    };
    let sp = |v: String| if v.is_empty() { v } else { format!(" {v}") };
    let est_time = sp(clean_time(&new.est_time)?);
    let est_tokens = sp(clean_tokens(&new.est_tokens)?);
    let date = &stamp[..stamp.len().min(10)];
    with_lock(root, || {
        let id = next_id(root)?;
        let dir = folder_for(root, "todo");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("{id}-{}.md", slug(&title)));
        let details = if body.is_empty() {
            String::new()
        } else {
            format!("{body}\n\n")
        };
        let text = format!(
            "---\nid: {id}\ntype: {kind}\npriority: {priority}\ntags:{tags}\nest-time:{est_time}\nest-tokens:{est_tokens}\nstage: ready\nstatus: todo\ncaptured: {date}\nclaimed-by:\nheartbeat:\nwaiting_on:\ncommit:\n---\n\n# {title}\n\n{details}## Log\n- {stamp} {who}: created\n"
        );
        write_atomic(&path, &text)?;
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        Ok(Created { id, path: rel })
    })
}

/// Replace a front-matter field's value (adding it before the closing `---` if missing).
fn set_front(text: &str, key: &str, value: &str) -> Result<String, String> {
    let rest = text
        .strip_prefix("---\n")
        .ok_or("the task has no front matter")?;
    let end = rest
        .find("\n---\n")
        .ok_or("the task's front matter isn't closed")?;
    let mut lines: Vec<String> = rest[..end].lines().map(String::from).collect();
    let prefix = format!("{key}:");
    match lines.iter_mut().find(|l| l.starts_with(&prefix)) {
        Some(l) => {
            *l = if value.is_empty() {
                prefix
            } else {
                format!("{prefix} {value}")
            }
        }
        None => lines.push(if value.is_empty() {
            prefix
        } else {
            format!("{prefix} {value}")
        }),
    }
    Ok(format!("---\n{}{}", lines.join("\n"), &rest[end..]))
}

fn edit(
    root: &Path,
    id: &str,
    f: impl FnOnce(&Path, String) -> Result<(String, Option<PathBuf>), String>,
) -> Result<String, String> {
    with_lock(root, || {
        let path = find_task(root, id)?;
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let (new, move_to) = f(&path, text)?;
        match move_to {
            Some(dest) if dest != path => {
                if dest.exists() {
                    return Err(format!("{} already exists", dest.display()));
                }
                write_atomic(&path, &new)?;
                fs::create_dir_all(dest.parent().unwrap_or(root)).map_err(|e| e.to_string())?;
                fs::rename(&path, &dest).map_err(|e| e.to_string())?;
                Ok(dest
                    .strip_prefix(root)
                    .unwrap_or(&dest)
                    .to_string_lossy()
                    .to_string())
            }
            _ => {
                write_atomic(&path, &new)?;
                Ok(path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string())
            }
        }
    })
}

/// Set priority, type or title.
pub fn set_field(
    root: &Path,
    id: &str,
    field: &str,
    value: &str,
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    let who = actor(who)?;
    let (key, val) = match field {
        "priority" => (
            "priority",
            pick(value, &PRIORITIES, "", "priority")?.to_string(),
        ),
        "type" | "kind" => ("type", pick(value, &KINDS, "", "type")?.to_string()),
        "title" => ("title", one_line(value, MAX_TITLE, "the title")?),
        "tags" => ("tags", clean_tags(&[value.to_string()])?.join(", ")),
        "time" | "est-time" => ("est-time", clean_time(value)?),
        "tokens" | "est-tokens" => ("est-tokens", clean_tokens(value)?),
        _ => {
            return Err(format!(
                "can't set {field:?}: use priority, type, title, tags, time or tokens"
            ))
        }
    };
    if val.is_empty() && !matches!(key, "tags" | "est-time" | "est-tokens") {
        return Err(format!("{field} is empty"));
    }
    let line = free_text_line(
        format!("{stamp} {who}: {key} set to {val}"),
        &format!("the {key}"),
    )?;
    edit(root, id, |_, text| {
        let new = if key == "title" {
            let (_, body) = front_matter(&text).ok_or("the task has no front matter")?;
            let head_at = text.len() - body.len();
            let line = body
                .lines()
                .find(|l| l.starts_with("# "))
                .ok_or("the task has no title line")?;
            let i = head_at + body.find(line).unwrap_or(0);
            format!("{}# {val}{}", &text[..i], &text[i + line.len()..])
        } else {
            set_front(&text, key, &val)?
        };
        Ok((append_log(&new, &line), None))
    })
}

/// Move a task to an owner status, the pre-stage way: inbox → Backlog, todo → Ready and
/// done → Done (as `set_stage`, unblocking it too), blocked → `block_task`.
pub fn move_task(
    root: &Path,
    id: &str,
    status: &str,
    waiting_on: &str,
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    let status = pick(status, &OWNER_STATUSES, "", "status").map_err(|_| {
        format!(
            "status must be one of {} (doing is an agent's claim)",
            OWNER_STATUSES.join(", ")
        )
    })?;
    let stage = match status {
        "blocked" => return block_task(root, id, waiting_on, who, stamp),
        "inbox" => "backlog",
        "todo" => "ready",
        _ => "done",
    };
    let who = actor(who)?;
    change(root, id, |t| {
        if t.stage == stage && !t.blocked {
            return Err(format!("{id} is already in {}", stage_label(stage)));
        }
        let mut log = Vec::new();
        if t.blocked {
            log.push(unblocked_line(stamp, &who, &t.waiting_on));
        }
        if t.stage != stage {
            log.push(stage_line(stamp, &who, &t.stage, stage, " (set)"));
        }
        Ok(Change {
            stage: stage.to_string(),
            blocked: false,
            waiting_on: String::new(),
            claimed_by: released(t),
            commit: None,
            log,
        })
    })
}

/// A task as a stage change sees it.
struct Current {
    stage: String,
    blocked: bool,
    waiting_on: String,
    claimed_by: String,
    /// Acceptance criteria: (items, ticked).
    criteria: (usize, usize),
    /// Who passed it Ready → Building most recently (the builder), if anyone.
    builder: Option<String>,
}

/// What a stage change writes.
struct Change {
    stage: String,
    blocked: bool,
    waiting_on: String,
    claimed_by: String,
    commit: Option<String>,
    log: Vec<String>,
}

fn stage_index(stage: &str) -> usize {
    STAGES.iter().position(|s| *s == stage).unwrap_or(0)
}

/// The claim after leaving the current stage for another: Building's holder lets go.
fn released(t: &Current) -> String {
    if t.stage == "building" {
        String::new()
    } else {
        t.claimed_by.clone()
    }
}

fn stage_line(stamp: &str, who: &str, from: &str, to: &str, detail: &str) -> String {
    format!("{stamp} {who}: stage {from} → {to}{detail}")
}

/// Would this log line read as tool-written stage history?
fn reads_as_stage(line: &str) -> bool {
    queue::stage_line(&format!("- {line}")).is_some()
}

/// A log line carrying free text (a note, a field's value, what it waits on). Only the stage
/// writer may log stage history, so a line the log's reader would take for a stage change
/// (`queue::stage_line`, whatever part of it the who and the text make up) is refused. Prose
/// that only mentions a stage doesn't parse as one, so it's fine.
fn free_text_line(line: String, what: &str) -> Result<String, String> {
    if reads_as_stage(&line) {
        return Err(format!(
            "{what} would read as a stage change in the log (\"<who>: stage <from> → <to>\"); only muster pass, return and stage write those"
        ));
    }
    Ok(line)
}

fn unblocked_line(stamp: &str, who: &str, was: &str) -> String {
    let line = format!("{stamp} {who}: unblocked (was waiting on {was})");
    // `was` comes from the file and may be hand-written; never let it log a stage change.
    if was.is_empty() || reads_as_stage(&line) {
        format!("{stamp} {who}: unblocked")
    } else {
        line
    }
}

/// Change a task's stage or blocked flag under the lock: read it, let `f` decide, then write
/// `stage:`, the status mirror, `waiting_on`, `claimed-by` (and `commit`), the log lines, and
/// move the file to the mirror's folder in the v2 layout. Returns the new relative path.
fn change(
    root: &Path,
    id: &str,
    f: impl FnOnce(&Current) -> Result<Change, String>,
) -> Result<String, String> {
    edit(root, id, |path, text| {
        let folder = queue::folder_of(root, path);
        let t = queue::parse_task("", root, &folder, path, &text, 0)
            .ok_or("the task has no front matter")?;
        let (_, body) = front_matter(&text).ok_or("the task has no front matter")?;
        let builder = queue::stage_lines(body)
            .into_iter()
            .rev()
            .find(|(s, detail)| s.from == "ready" && s.to == "building" && *detail == "claimed")
            .map(|(s, _)| s.who);
        let now = Current {
            criteria: queue::criteria(body),
            stage: t.stage,
            blocked: t.blocked,
            waiting_on: t.waiting_on,
            claimed_by: t.claimed_by,
            builder,
        };
        let c = f(&now)?;
        let status = mirror_status(&c.stage, c.blocked);
        let mut new = set_front(&text, "stage", &c.stage)?;
        new = set_front(&new, "status", status)?;
        new = set_front(&new, "waiting_on", &c.waiting_on)?;
        new = set_front(&new, "claimed-by", &c.claimed_by)?;
        if let Some(commit) = &c.commit {
            new = set_front(&new, "commit", commit)?;
        }
        for line in &c.log {
            new = append_log(&new, line);
        }
        let dest = if has_status_folders(root) {
            Some(folder_for(root, status).join(path.file_name().ok_or("bad path")?))
        } else {
            None
        };
        Ok((new, dest))
    })
}

/// What `pass_task` may need besides the task: the gate's result line (Building → Testing)
/// and the commit (Your check → Done).
#[derive(Debug, Clone, Default)]
pub struct PassOpts {
    pub gate: String,
    pub commit: String,
    /// The stage the caller saw the task in. When given, the pass is refused if the task has
    /// moved on since (a double click, a stale window), so one click never moves two stages.
    pub from: Option<String>,
}

/// A git commit id: 7 to 40 hex characters, kept lowercase. Empty is fine (none given).
fn clean_commit(input: &str) -> Result<String, String> {
    let c = input.trim().to_lowercase();
    if c.is_empty() || ((7..=40).contains(&c.len()) && c.bytes().all(|b| b.is_ascii_hexdigit())) {
        Ok(c)
    } else {
        Err(format!(
            "commit must be a git commit id (7 to 40 hex characters), got {input:?}"
        ))
    }
}

/// Hand a task to the next stage. Refused when it's blocked or done, when someone else holds
/// Ready or Building, Shaping → Ready without acceptance criteria, Building → Testing without
/// the gate's result line, and Testing → Your check before every criterion is ticked.
pub fn pass_task(
    root: &Path,
    id: &str,
    who: &str,
    stamp: &str,
    opts: &PassOpts,
) -> Result<String, String> {
    let who = actor(who)?;
    let gate = if opts.gate.trim().is_empty() {
        String::new()
    } else {
        one_line(&opts.gate, 500, "the gate line")?
    };
    let commit = clean_commit(&opts.commit)?;
    let expected = match opts.from.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => Some(pick(s, &STAGES, "", "stage")?),
        _ => None,
    };
    change(root, id, |t| {
        if let Some(seen) = expected.filter(|s| *s != t.stage) {
            return Err(format!(
                "{id} is now in {}, not {}; it moved on since you looked",
                stage_label(&t.stage),
                stage_label(seen)
            ));
        }
        if t.blocked {
            let on = if t.waiting_on.is_empty() {
                "something it doesn't name"
            } else {
                &t.waiting_on
            };
            return Err(format!("{id} is blocked on {on}; unblock it first"));
        }
        let from = t.stage.as_str();
        let Some(&to) = STAGES.get(stage_index(from) + 1) else {
            return Err(format!("{id} is done; there's no stage after Done"));
        };
        if !gate.is_empty() && from != "building" {
            return Err("--gate is only for passing Building → Testing".into());
        }
        if !commit.is_empty() && from != "your-check" {
            return Err("--commit is only for passing Your check → Done".into());
        }
        if matches!(from, "ready" | "building") && !t.claimed_by.is_empty() && t.claimed_by != who {
            return Err(format!("{id} is claimed by {}", t.claimed_by));
        }
        let (items, ticked) = t.criteria;
        let detail = match from {
            "shaping" | "testing" if items == 0 => {
                return Err(format!(
                    "{id} has no acceptance criteria: add `- [ ]` items under `## Acceptance criteria` first"
                ))
            }
            "testing" if ticked < items => {
                return Err(format!(
                    "{id}: {} of {items} acceptance criteria aren't ticked; tick each with its proof first",
                    items - ticked
                ))
            }
            "building" if gate.is_empty() => {
                return Err(format!(
                    "passing {id} Building → Testing needs the gate's result line (--gate \"…\")"
                ))
            }
            "building" => format!(" (gate: {gate})"),
            "ready" => " (claimed)".to_string(),
            _ => String::new(),
        };
        Ok(Change {
            stage: to.to_string(),
            blocked: false,
            waiting_on: t.waiting_on.clone(),
            claimed_by: if to == "building" {
                who.clone()
            } else {
                released(t)
            },
            commit: (to == "done" && !commit.is_empty()).then(|| commit.clone()),
            log: vec![stage_line(stamp, &who, from, to, &detail)],
        })
    })
}

/// Send a task back one stage, or to the earlier stage `to`, with the reason. Back to Building
/// gives it to whoever last claimed it there.
pub fn return_task(
    root: &Path,
    id: &str,
    why: &str,
    to: Option<&str>,
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    let who = actor(who)?;
    let why = one_line(why, 500, "the reason")?;
    let to = match to.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Some(pick(s, &STAGES, "", "stage")?),
        None => None,
    };
    change(root, id, |t| {
        let i = stage_index(&t.stage);
        if i == 0 {
            return Err(format!("{id} is in Backlog; there's no stage before it"));
        }
        let target = to.unwrap_or(STAGES[i - 1]);
        if stage_index(target) >= i {
            return Err(format!(
                "{id} is in {}; return sends it to an earlier stage, not {}",
                stage_label(&t.stage),
                stage_label(target)
            ));
        }
        Ok(Change {
            stage: target.to_string(),
            blocked: t.blocked,
            waiting_on: t.waiting_on.clone(),
            claimed_by: if target == "building" {
                t.builder.clone().unwrap_or_default()
            } else {
                String::new()
            },
            commit: None,
            log: vec![stage_line(
                stamp,
                &who,
                &t.stage,
                target,
                &format!(" (returned: {why})"),
            )],
        })
    })
}

/// What a task waits on: a task id (`0012`), `owner` or `capture`, then optional words.
fn clean_on(on: &str) -> Result<String, String> {
    let on = one_line(on, 200, "what it's waiting on")?;
    let rest_ok = |r: &str| !r.starts_with(|c: char| c.is_alphanumeric());
    let task = on.len() >= 4
        && on.as_bytes()[..4].iter().all(u8::is_ascii_digit)
        && on.get(4..).is_some_and(rest_ok);
    if task {
        return Ok(on);
    }
    for word in ["owner", "capture"] {
        let head_ok = on
            .get(..word.len())
            .is_some_and(|h| h.eq_ignore_ascii_case(word));
        if let Some(tail) = on.get(word.len()..).filter(|t| head_ok && rest_ok(t)) {
            return Ok(format!("{word}{tail}"));
        }
    }
    Err(format!(
        "what it's waiting on must start with a task id (0012), owner or capture, then a few words; got {on:?}"
    ))
}

/// Set the blocked flag: the task keeps its stage (and its claim) and waits on `on`.
pub fn block_task(
    root: &Path,
    id: &str,
    on: &str,
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    let who = actor(who)?;
    let on = clean_on(on)?;
    let line = free_text_line(
        format!("{stamp} {who}: blocked on {on}"),
        "what it waits on",
    )?;
    change(root, id, |t| {
        if t.waiting_on == on {
            return Err(format!("{id} is already blocked on {on}"));
        }
        Ok(Change {
            stage: t.stage.clone(),
            blocked: true,
            waiting_on: on.clone(),
            claimed_by: t.claimed_by.clone(),
            commit: None,
            log: vec![line.clone()],
        })
    })
}

/// Clear the blocked flag; the status mirror goes back to what the stage means.
pub fn unblock_task(root: &Path, id: &str, who: &str, stamp: &str) -> Result<String, String> {
    let who = actor(who)?;
    change(root, id, |t| {
        if !t.blocked {
            return Err(format!("{id} isn't blocked"));
        }
        Ok(Change {
            stage: t.stage.clone(),
            blocked: false,
            waiting_on: String::new(),
            claimed_by: t.claimed_by.clone(),
            commit: None,
            log: vec![unblocked_line(stamp, &who, &t.waiting_on)],
        })
    })
}

/// The owner's (or capture's) override: put a task in any stage, logged with `(set)`. Leaving
/// Building releases its claim; the blocked flag stays.
pub fn set_stage(
    root: &Path,
    id: &str,
    stage: &str,
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    let who = actor(who)?;
    let stage = pick(stage.trim(), &STAGES, "", "stage")?;
    if stage.is_empty() {
        return Err(format!("stage must be one of {}", STAGES.join(", ")));
    }
    change(root, id, |t| {
        if t.stage == stage {
            return Err(format!("{id} is already in {}", stage_label(stage)));
        }
        Ok(Change {
            stage: stage.to_string(),
            blocked: t.blocked,
            waiting_on: t.waiting_on.clone(),
            claimed_by: released(t),
            commit: None,
            log: vec![stage_line(stamp, &who, &t.stage, stage, " (set)")],
        })
    })
}

/// Add a line to the task's log.
pub fn add_note(
    root: &Path,
    id: &str,
    note: &str,
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    let who = actor(who)?;
    let note = one_line(note, 2000, "the note")?;
    let line = free_text_line(format!("{stamp} {who}: {note}"), "a note")?;
    edit(root, id, |_, text| Ok((append_log(&text, &line), None)))
}

/// A safe attachment name: the base name, with only letters, digits and `-_.` kept.
pub fn attachment_name(name: &str) -> Result<String, String> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "-_.".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect();
    let cleaned = cleaned.trim_start_matches(['.', '-']).to_string();
    if cleaned.is_empty() || cleaned.len() > 120 {
        return Err("bad file name".into());
    }
    Ok(cleaned)
}

/// Save bytes as an attachment of task `id`. Never overwrites: a clash gets `-2`, `-3`…
pub fn attach(
    root: &Path,
    id: &str,
    name: &str,
    bytes: &[u8],
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    let who = actor(who)?;
    if bytes.len() > MAX_ATTACHMENT {
        return Err("attachments are limited to 25 MB".into());
    }
    let name = attachment_name(name)?;
    with_lock(root, || {
        let path = find_task(root, id)?;
        let dir = root.join("attachments").join(id);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let (stem, ext) = match name.rsplit_once('.') {
            Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
            _ => (name.clone(), String::new()),
        };
        let mut file = name.clone();
        let mut n = 2;
        while dir.join(&file).exists() {
            file = format!("{stem}-{n}{ext}");
            n += 1;
        }
        let tmp = dir.join(format!(".{file}.tmp"));
        fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        fs::rename(&tmp, dir.join(&file)).map_err(|e| e.to_string())?;
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        write_atomic(
            &path,
            &append_log(&text, &format!("{stamp} {who}: attached {file}")),
        )?;
        Ok(file)
    })
}

/// Decode standard base64 (what a browser's data URL carries). Whitespace is ignored.
pub fn unbase64(text: &str) -> Result<Vec<u8>, String> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let clean: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let trimmed: &[u8] = {
        let mut e = clean.len();
        while e > 0 && clean[e - 1] == b'=' {
            e -= 1;
        }
        &clean[..e]
    };
    let mut out = Vec::with_capacity(trimmed.len() * 3 / 4);
    for chunk in trimmed.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= val(c).ok_or("not base64")? << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
        if chunk.len() == 1 {
            return Err("not base64".into());
        }
    }
    Ok(out)
}

/// The data part of a `data:…;base64,…` URL.
pub fn data_url_bytes(url: &str) -> Result<Vec<u8>, String> {
    let (head, data) = url.split_once(',').ok_or("not a data URL")?;
    if !head.starts_with("data:") || !head.ends_with(";base64") {
        return Err("not a base64 data URL".into());
    }
    unbase64(data)
}

/// Statuses that exist as folders (for checks in tests and the CLI's list).
pub fn status_folders() -> &'static [&'static str] {
    &STATUS_FOLDERS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::{base64, read_state, Cache};

    fn queue() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for f in STATUS_FOLDERS {
            fs::create_dir(d.path().join(f)).unwrap();
        }
        fs::write(d.path().join("README.md"), "q").unwrap();
        d
    }
    const T: &str = "2026-10-12 14:05";
    fn state(d: &Path) -> crate::queue::State {
        read_state(&[("p".into(), d.to_path_buf())], &mut Cache::default())
    }

    #[test]
    fn creates_tasks_with_ids_and_template() {
        let d = queue();
        let a = create_task(
            d.path(),
            &NewTask {
                title: "Board  flickers\non resize".into(),
                kind: "Bug".into(),
                priority: "high".into(),
                body: "Steps:\n1. resize\n\tindented".into(),
                ..Default::default()
            },
            "owner",
            T,
        )
        .unwrap();
        assert_eq!(
            a,
            Created {
                id: "0001".into(),
                path: "todo/0001-board-flickers-on-resize.md".into()
            }
        );
        let b = create_task(
            d.path(),
            &NewTask {
                title: "Second".into(),
                ..Default::default()
            },
            "cli",
            T,
        )
        .unwrap();
        assert_eq!(b.id, "0002");
        let text = fs::read_to_string(d.path().join(&a.path)).unwrap();
        assert!(text.starts_with(
            "---\nid: 0001\ntype: bug\npriority: high\ntags:\nest-time:\nest-tokens:\nstage: ready\nstatus: todo\ncaptured: 2026-10-12\n"
        ));
        assert!(text.contains("# Board flickers on resize\n\nSteps:\n1. resize\n\tindented\n\n## Log\n- 2026-10-12 14:05 owner: created\n"));
        let s = state(d.path());
        assert_eq!(s.tasks.len(), 2);
        assert!(s.problems.is_empty(), "{:?}", s.problems);
        assert!(s.tasks.iter().all(|t| t.stage == "ready" && !t.blocked));
        assert_eq!(s.tasks[1].kind, "task");
        assert_eq!(s.tasks[1].priority, "normal");
    }

    #[test]
    fn create_refusals() {
        let d = queue();
        let e = |n: NewTask| create_task(d.path(), &n, "o", T).unwrap_err();
        assert!(e(NewTask {
            title: "  ".into(),
            ..Default::default()
        })
        .contains("title is empty"));
        assert!(e(NewTask {
            title: "x".repeat(201),
            ..Default::default()
        })
        .contains("longer"));
        assert!(e(NewTask {
            title: "x".into(),
            kind: "epic".into(),
            ..Default::default()
        })
        .contains("type must be"));
        assert!(e(NewTask {
            title: "x".into(),
            priority: "P0".into(),
            ..Default::default()
        })
        .contains("priority must be"));
        assert!(queue::task_files(d.path()).is_empty());
    }

    #[test]
    fn flat_layout_creates_in_the_queue_folder() {
        let d = tempfile::tempdir().unwrap();
        let c = create_task(
            d.path(),
            &NewTask {
                title: "Flat".into(),
                ..Default::default()
            },
            "o",
            T,
        )
        .unwrap();
        assert_eq!(c.path, "0001-flat.md");
        let p = move_task(d.path(), "0001", "done", "", "o", T).unwrap();
        assert_eq!(p, "0001-flat.md", "flat files never move");
        assert_eq!(state(d.path()).tasks[0].status, "done");
    }

    #[test]
    fn moves_between_status_folders_and_releases_claims() {
        let d = queue();
        create_task(
            d.path(),
            &NewTask {
                title: "Move me".into(),
                ..Default::default()
            },
            "o",
            T,
        )
        .unwrap();
        // An agent claimed it (stage, its status mirror and folder, claimed-by).
        let p = d.path().join("todo/0001-move-me.md");
        let t = fs::read_to_string(&p)
            .unwrap()
            .replace("stage: ready", "stage: building")
            .replace("status: todo", "status: doing")
            .replace("claimed-by:", "claimed-by: wt (session aaaa1111)");
        fs::remove_file(&p).unwrap();
        fs::write(d.path().join("doing/0001-move-me.md"), t).unwrap();
        assert!(move_task(d.path(), "0001", "blocked", "", "o", T)
            .unwrap_err()
            .contains("waiting on"));
        assert!(move_task(d.path(), "0001", "doing", "", "o", T)
            .unwrap_err()
            .contains("claim"));
        assert_eq!(
            move_task(
                d.path(),
                "0001",
                "blocked",
                "owner: pick a colour",
                "owner",
                T
            )
            .unwrap(),
            "blocked/0001-move-me.md"
        );
        let s = state(d.path());
        let t = &s.tasks[0];
        // Blocked is a flag: the task stays in Building and keeps its claim.
        assert_eq!(
            (
                t.status.as_str(),
                t.folder.as_str(),
                t.stage.as_str(),
                t.claimed_by.as_str()
            ),
            ("blocked", "blocked", "building", "wt (session aaaa1111)")
        );
        assert_eq!(t.waiting_on, "owner: pick a colour");
        assert!(t.waiting_for_owner);
        assert!(s.problems.is_empty(), "{:?}", s.problems);
        move_task(d.path(), "0001", "todo", "", "owner", T).unwrap();
        let t = &state(d.path()).tasks[0];
        assert_eq!(
            (
                t.status.as_str(),
                t.stage.as_str(),
                t.waiting_on.as_str(),
                t.claimed_by.as_str()
            ),
            ("todo", "ready", "", "")
        );
        assert_eq!(
            t.log_tail[1..],
            [
                "2026-10-12 14:05 owner: unblocked (was waiting on owner: pick a colour)",
                "2026-10-12 14:05 owner: stage building → ready (set)"
            ]
        );
    }

    #[test]
    fn sets_fields_and_notes() {
        let d = queue();
        create_task(
            d.path(),
            &NewTask {
                title: "Old title".into(),
                ..Default::default()
            },
            "o",
            T,
        )
        .unwrap();
        set_field(d.path(), "0001", "priority", "low", "owner", T).unwrap();
        set_field(d.path(), "0001", "type", "idea", "owner", T).unwrap();
        set_field(d.path(), "0001", "title", "New title", "owner", T).unwrap();
        add_note(d.path(), "0001", "Checked on the Mac.", "owner", T).unwrap();
        assert!(set_field(d.path(), "0001", "status", "done", "o", T)
            .unwrap_err()
            .contains("can't set"));
        assert!(set_field(d.path(), "0001", "priority", "urgent", "o", T)
            .unwrap_err()
            .contains("must be"));
        let t = &state(d.path()).tasks[0];
        assert_eq!(
            (t.priority.as_str(), t.kind.as_str(), t.title.as_str()),
            ("low", "idea", "New title")
        );
        assert!(t
            .log_tail
            .last()
            .unwrap()
            .ends_with("owner: Checked on the Mac."));
    }

    #[test]
    fn attaches_without_overwriting() {
        let d = queue();
        create_task(
            d.path(),
            &NewTask {
                title: "Shots".into(),
                ..Default::default()
            },
            "o",
            T,
        )
        .unwrap();
        assert_eq!(
            attach(d.path(), "0001", "Screen Shot 1.png", b"one", "owner", T).unwrap(),
            "Screen-Shot-1.png"
        );
        assert_eq!(
            attach(d.path(), "0001", "Screen Shot 1.png", b"two", "owner", T).unwrap(),
            "Screen-Shot-1-2.png"
        );
        assert_eq!(
            attach(d.path(), "0001", "../../etc/passwd", b"x", "owner", T).unwrap(),
            "passwd"
        );
        assert!(attach(d.path(), "0001", "...", b"x", "o", T).is_err());
        assert!(attach(
            d.path(),
            "0001",
            "big.bin",
            &vec![0u8; 25 * 1024 * 1024 + 1],
            "o",
            T
        )
        .unwrap_err()
        .contains("25 MB"));
        assert_eq!(
            fs::read(d.path().join("attachments/0001/Screen-Shot-1.png")).unwrap(),
            b"one"
        );
        let t = &state(d.path()).tasks[0];
        assert_eq!(
            t.attachments,
            vec!["Screen-Shot-1-2.png", "Screen-Shot-1.png", "passwd"]
        );
        assert!(t.log_tail.last().unwrap().contains("attached passwd"));
    }

    #[test]
    fn base64_round_trips() {
        for data in [
            &b""[..],
            b"f",
            b"fo",
            b"foo",
            b"foob",
            b"\x00\xff\x10binary",
        ] {
            assert_eq!(unbase64(&base64(data)).unwrap(), data);
        }
        assert_eq!(
            data_url_bytes("data:image/png;base64,iVBORw==").unwrap(),
            vec![137, 80, 78, 71]
        );
        assert!(data_url_bytes("data:text/plain,hi").is_err());
        assert!(unbase64("@@@@").is_err());
    }

    #[test]
    fn tags_are_cleaned_and_round_trip() {
        let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            clean_tags(&v(&["Tech Debt", "spike,  CODE", "spike", "my_tag!"])).unwrap(),
            v(&["tech-debt", "spike", "code", "my-tag"])
        );
        assert!(clean_tags(&v(&["a,b,c,d,e,f,g,h,i"]))
            .unwrap_err()
            .contains("at most 8"));
        assert!(clean_tags(&v(&["x".repeat(25).as_str()]))
            .unwrap_err()
            .contains("24"));
        assert!(clean_tags(&v(&[" , "])).unwrap().is_empty());
        let d = queue();
        create_task(
            d.path(),
            &NewTask {
                title: "Tagged".into(),
                tags: v(&["Spike", "tech debt"]),
                ..Default::default()
            },
            "o",
            T,
        )
        .unwrap();
        assert_eq!(state(d.path()).tasks[0].tags, v(&["spike", "tech-debt"]));
        set_field(d.path(), "0001", "tags", "chore, code", "o", T).unwrap();
        assert_eq!(state(d.path()).tasks[0].tags, v(&["chore", "code"]));
        set_field(d.path(), "0001", "tags", "", "o", T).unwrap();
        assert!(state(d.path()).tasks[0].tags.is_empty());
        assert!(state(d.path()).problems.is_empty());
    }

    #[test]
    fn estimates_are_cleaned_set_and_counted() {
        assert_eq!(clean_time(" 45 min ").unwrap(), "45m");
        assert_eq!(clean_time("1.5H").unwrap(), "1.5h");
        assert_eq!(clean_time("2 days").unwrap(), "2d");
        assert_eq!(clean_time("").unwrap(), "");
        assert!(
            clean_time("soon").is_err() && clean_time("5w").is_err() && clean_time("0h").is_err()
        );
        assert_eq!(clean_tokens("400k").unwrap(), "400k");
        assert_eq!(clean_tokens("1200000").unwrap(), "1.2M");
        assert_eq!(clean_tokens("1,500").unwrap(), "1.5k");
        assert_eq!(clean_tokens("2m").unwrap(), "2M");
        assert!(clean_tokens("lots").is_err() && clean_tokens("5g").is_err());
        assert_eq!(
            (
                time_minutes("1.5h"),
                time_minutes("45m"),
                time_minutes("1d"),
                time_minutes("")
            ),
            (90, 45, 480, 0)
        );
        assert_eq!(
            (
                token_count("400k"),
                token_count("1.2M"),
                token_count("900"),
                token_count("")
            ),
            (400_000, 1_200_000, 900, 0)
        );
        let d = queue();
        create_task(
            d.path(),
            &NewTask {
                title: "Est".into(),
                est_time: "2h".into(),
                est_tokens: "300k".into(),
                ..Default::default()
            },
            "o",
            T,
        )
        .unwrap();
        let t = &state(d.path()).tasks[0];
        assert_eq!(
            (
                t.est_time.as_str(),
                t.est_tokens.as_str(),
                t.est_minutes,
                t.est_token_count
            ),
            ("2h", "300k", 120, 300_000)
        );
        set_field(d.path(), "0001", "time", "30m", "o", T).unwrap();
        set_field(d.path(), "0001", "tokens", "", "o", T).unwrap();
        let t = &state(d.path()).tasks[0];
        assert_eq!((t.est_time.as_str(), t.est_tokens.as_str()), ("30m", ""));
        assert!(state(d.path()).problems.is_empty());
    }

    #[test]
    fn next_id_ignores_dated_notes() {
        let d = queue();
        fs::write(d.path().join("inbox/2026-10-12-0900-note.md"), "note").unwrap();
        create_task(
            d.path(),
            &NewTask {
                title: "One".into(),
                ..Default::default()
            },
            "o",
            T,
        )
        .unwrap();
        assert_eq!(next_id(d.path()).unwrap(), "0002");
    }

    /// A task file with the given front-matter lines (`status:`, `stage:`, …) and body sections.
    fn task_text(fm: &str, sections: &str) -> String {
        format!("---\nid: 0001\ntype: task\npriority: normal\n{fm}\n---\n\n# Staged\n\n{sections}## Log\n- 2026-10-12 09:00 capture: created\n")
    }
    const CRITERIA: &str = "## Acceptance criteria\n- [x] AC1: one\n- [ ] AC2: two\n\n";
    const TICKED: &str = "## Acceptance criteria\n- [x] AC1: one\n- [x] AC2: two\n\n";

    fn put(d: &Path, rel: &str, text: &str) {
        fs::write(d.join(rel), text).unwrap();
    }
    fn only(d: &Path) -> crate::queue::Task {
        let s = state(d);
        assert_eq!(s.tasks.len(), 1, "{:?}", s.problems);
        s.tasks.into_iter().next().unwrap()
    }
    fn no_gate() -> PassOpts {
        PassOpts::default()
    }

    #[test]
    fn pass_ready_to_building_claims_it() {
        let d = queue();
        put(
            d.path(),
            "todo/0001-t.md",
            &task_text("status: todo\nclaimed-by:\nwaiting_on:", CRITERIA),
        );
        assert_eq!(
            pass_task(d.path(), "0001", "wt-a", T, &no_gate()).unwrap(),
            "doing/0001-t.md"
        );
        let t = only(d.path());
        assert_eq!(
            (
                t.stage.as_str(),
                t.status.as_str(),
                t.folder.as_str(),
                t.claimed_by.as_str()
            ),
            ("building", "doing", "doing", "wt-a")
        );
        assert_eq!(
            t.stage_history,
            vec![crate::queue::StageStep {
                at: T.into(),
                who: "wt-a".into(),
                from: "ready".into(),
                to: "building".into()
            }]
        );
        assert_eq!(t.claimed_on, "2026-10-12");
        let text = fs::read_to_string(d.path().join("doing/0001-t.md")).unwrap();
        assert!(text.contains("\nstage: building\n"), "{text}");
        assert!(text.ends_with("- 2026-10-12 14:05 wt-a: stage ready → building (claimed)\n"));
        assert!(state(d.path()).problems.is_empty());
    }

    #[test]
    fn passes_through_every_stage_and_moves_the_file() {
        let d = queue();
        put(
            d.path(),
            "inbox/0001-t.md",
            &task_text("status: inbox\nclaimed-by:\nwaiting_on:\ncommit:", TICKED),
        );
        let gate = PassOpts {
            gate: "gate full: green (9 steps)".into(),
            ..Default::default()
        };
        let done = PassOpts {
            commit: "ABC1234".into(),
            ..Default::default()
        };
        let steps: [(&PassOpts, &str, &str, &str); 6] = [
            (&no_gate(), "shaping", "todo", ""),
            (&no_gate(), "ready", "todo", ""),
            (&no_gate(), "building", "doing", "wt-a"),
            (&gate, "testing", "doing", ""),
            (&no_gate(), "your-check", "doing", ""),
            (&done, "done", "done", ""),
        ];
        for (opts, stage, folder, claim) in steps {
            let p = pass_task(d.path(), "0001", "wt-a", T, opts).unwrap();
            assert_eq!(p, format!("{folder}/0001-t.md"));
            let t = only(d.path());
            assert_eq!(
                (t.stage.as_str(), t.status.as_str(), t.claimed_by.as_str()),
                (stage, folder, claim)
            );
            assert!(state(d.path()).problems.is_empty(), "{stage}");
        }
        let t = only(d.path());
        assert_eq!(t.commit, "abc1234");
        assert_eq!(t.stage_history.len(), 6);
        assert_eq!(t.done_on, "2026-10-12");
        let text = fs::read_to_string(d.path().join("done/0001-t.md")).unwrap();
        assert!(
            text.contains("wt-a: stage building → testing (gate: gate full: green (9 steps))\n")
        );
        assert!(text.ends_with("wt-a: stage your-check → done\n"));
    }

    #[test]
    fn flat_layout_passes_without_moving() {
        let d = tempfile::tempdir().unwrap();
        put(
            d.path(),
            "0001-t.md",
            &task_text("status: todo\nclaimed-by:\nwaiting_on:", CRITERIA),
        );
        assert_eq!(
            pass_task(d.path(), "0001", "wt-a", T, &no_gate()).unwrap(),
            "0001-t.md"
        );
        assert_eq!(
            block_task(d.path(), "0001", "owner", "o", T).unwrap(),
            "0001-t.md"
        );
        let t = only(d.path());
        assert_eq!(
            (t.stage.as_str(), t.status.as_str(), t.blocked),
            ("building", "blocked", true)
        );
    }

    #[test]
    fn pass_refusals_leave_the_file_alone() {
        let d = queue();
        let cases: [(&str, &str, &str, PassOpts, &str); 9] = [
            (
                "todo",
                "status: blocked\nwaiting_on: 0012 the API",
                CRITERIA,
                no_gate(),
                "blocked on 0012 the API; unblock it first",
            ),
            (
                "done",
                "status: done\nstage: done",
                CRITERIA,
                no_gate(),
                "no stage after Done",
            ),
            (
                "todo",
                "status: todo\nstage: shaping",
                "## Acceptance criteria\n\nnothing yet\n\n",
                no_gate(),
                "no acceptance criteria",
            ),
            (
                "todo",
                "status: todo\nclaimed-by: wt-b",
                CRITERIA,
                no_gate(),
                "claimed by wt-b",
            ),
            (
                "doing",
                "status: doing\nstage: building\nclaimed-by: wt-a",
                CRITERIA,
                PassOpts {
                    gate: "  \n ".into(),
                    ..Default::default()
                },
                "needs the gate's result line",
            ),
            (
                "doing",
                "status: doing\nstage: testing",
                CRITERIA,
                no_gate(),
                "1 of 2 acceptance criteria aren't ticked",
            ),
            (
                "doing",
                "status: doing\nstage: testing",
                "",
                no_gate(),
                "no acceptance criteria",
            ),
            (
                "doing",
                "status: doing\nstage: your-check",
                TICKED,
                PassOpts {
                    commit: "not a sha".into(),
                    ..Default::default()
                },
                "commit must be",
            ),
            (
                "todo",
                "status: todo",
                CRITERIA,
                PassOpts {
                    gate: "green".into(),
                    ..Default::default()
                },
                "--gate is only for",
            ),
        ];
        for (folder, fm, sections, opts, want) in cases {
            let rel = format!("{folder}/0001-t.md");
            let text = task_text(fm, sections);
            put(d.path(), &rel, &text);
            let e = pass_task(d.path(), "0001", "wt-a", T, &opts).unwrap_err();
            assert!(e.contains(want), "{fm}: {e}");
            assert_eq!(fs::read_to_string(d.path().join(&rel)).unwrap(), text);
            fs::remove_file(d.path().join(&rel)).unwrap();
        }
    }

    #[test]
    fn hand_typed_history_lines_do_not_count() {
        let d = queue();
        let mut text = task_text("status: doing\nstage: testing", TICKED);
        text.push_str("- 2026-10-12 10:00 wt-a: moved todo → doing (claimed)\n- 2026-10-12 10:01 wt-a: stage ready -> building\n");
        put(d.path(), "doing/0001-t.md", &text);
        assert!(only(d.path()).stage_history.is_empty());
    }

    #[test]
    fn return_goes_back_and_restores_the_builder() {
        let d = queue();
        put(
            d.path(),
            "todo/0001-t.md",
            &task_text("status: todo\nclaimed-by:\nwaiting_on:", TICKED),
        );
        pass_task(d.path(), "0001", "wt-a", T, &no_gate()).unwrap();
        pass_task(
            d.path(),
            "0001",
            "wt-a",
            T,
            &PassOpts {
                gate: "green".into(),
                ..Default::default()
            },
        )
        .unwrap();
        pass_task(d.path(), "0001", "qa", T, &no_gate()).unwrap();
        // Refusals first: a later or equal stage, an unknown one, no reason, a long reason.
        let r = |to: Option<&str>, why: &str| return_task(d.path(), "0001", why, to, "qa", T);
        assert!(r(Some("done"), "x").unwrap_err().contains("earlier stage"));
        assert!(r(Some("your-check"), "x")
            .unwrap_err()
            .contains("earlier stage"));
        assert!(r(Some("qa"), "x")
            .unwrap_err()
            .contains("stage must be one of"));
        assert!(r(None, " \n ").unwrap_err().contains("reason is empty"));
        assert!(r(None, &"x".repeat(501)).unwrap_err().contains("500"));
        // Your check → Testing (one back), then Testing → Building with the builder back.
        assert_eq!(r(None, "looks off").unwrap(), "doing/0001-t.md");
        assert_eq!(only(d.path()).stage, "testing");
        r(Some("building"), "AC2 fails (see shot)").unwrap();
        let t = only(d.path());
        assert_eq!(
            (t.stage.as_str(), t.claimed_by.as_str()),
            ("building", "wt-a")
        );
        assert_eq!(
            t.stage_history
                .last()
                .map(|s| (s.from.as_str(), s.to.as_str())),
            Some(("testing", "building"))
        );
        let text = fs::read_to_string(d.path().join("doing/0001-t.md")).unwrap();
        assert!(text.ends_with("qa: stage testing → building (returned: AC2 fails (see shot))\n"));
        // Building → Shaping releases the claim and moves back to todo/.
        assert_eq!(
            r(Some("Shaping"), "the spec is wrong").unwrap(),
            "todo/0001-t.md"
        );
        let t = only(d.path());
        assert_eq!((t.stage.as_str(), t.claimed_by.as_str()), ("shaping", ""));
        set_stage(d.path(), "0001", "backlog", "owner", T).unwrap();
        assert!(r(None, "x").unwrap_err().contains("no stage before"));
    }

    #[test]
    fn return_to_building_without_a_claim_on_record_leaves_it_unclaimed() {
        let d = queue();
        put(
            d.path(),
            "doing/0001-t.md",
            &task_text("status: doing\nstage: testing\nclaimed-by:", TICKED),
        );
        return_task(d.path(), "0001", "bug", None, "qa", T).unwrap();
        let t = only(d.path());
        assert_eq!((t.stage.as_str(), t.claimed_by.as_str()), ("building", ""));
    }

    #[test]
    fn block_keeps_the_stage_and_unblock_mirrors_back() {
        let d = queue();
        put(
            d.path(),
            "doing/0001-t.md",
            &task_text(
                "status: doing\nstage: testing\nclaimed-by: qa\nwaiting_on:",
                TICKED,
            ),
        );
        for bad in ["", "soon", "12 days", "owners", "00123", "captured"] {
            assert!(
                block_task(d.path(), "0001", bad, "qa", T).is_err(),
                "{bad:?} is refused"
            );
        }
        assert_eq!(
            block_task(d.path(), "0001", "Owner: which colour?", "qa", T).unwrap(),
            "blocked/0001-t.md"
        );
        let t = only(d.path());
        assert_eq!(
            (
                t.stage.as_str(),
                t.status.as_str(),
                t.blocked,
                t.waiting_on.as_str(),
                t.claimed_by.as_str()
            ),
            ("testing", "blocked", true, "owner: which colour?", "qa")
        );
        assert!(t.waiting_for_owner);
        assert!(t
            .log_tail
            .last()
            .unwrap()
            .ends_with("qa: blocked on owner: which colour?"));
        assert!(
            block_task(d.path(), "0001", "owner: which colour?", "qa", T)
                .unwrap_err()
                .contains("already blocked")
        );
        block_task(d.path(), "0001", "0012", "qa", T).unwrap();
        block_task(d.path(), "0001", "capture, needs a spec", "qa", T).unwrap();
        // Blocked: no pass, but return and set keep the flag.
        assert!(pass_task(d.path(), "0001", "qa", T, &no_gate())
            .unwrap_err()
            .contains("unblock it first"));
        assert_eq!(
            unblock_task(d.path(), "0001", "owner", T).unwrap(),
            "doing/0001-t.md"
        );
        let t = only(d.path());
        assert_eq!(
            (
                t.stage.as_str(),
                t.status.as_str(),
                t.blocked,
                t.waiting_on.as_str()
            ),
            ("testing", "doing", false, "")
        );
        assert!(t
            .log_tail
            .last()
            .unwrap()
            .ends_with("owner: unblocked (was waiting on capture, needs a spec)"));
        assert!(unblock_task(d.path(), "0001", "owner", T)
            .unwrap_err()
            .contains("isn't blocked"));
        assert!(state(d.path()).problems.is_empty());
    }

    #[test]
    fn legacy_blocked_without_a_reason_can_be_unblocked() {
        let d = queue();
        put(
            d.path(),
            "blocked/0001-t.md",
            &task_text("status: blocked\nwaiting_on:", ""),
        );
        let t = only(d.path());
        assert_eq!((t.stage.as_str(), t.blocked), ("ready", true));
        // Returning or setting keeps the flag even with nothing in waiting_on.
        set_stage(d.path(), "0001", "shaping", "owner", T).unwrap();
        let t = only(d.path());
        assert_eq!(
            (t.stage.as_str(), t.status.as_str(), t.blocked),
            ("shaping", "blocked", true)
        );
        assert_eq!(
            unblock_task(d.path(), "0001", "owner", T).unwrap(),
            "todo/0001-t.md"
        );
        assert_eq!(only(d.path()).status, "todo");
    }

    #[test]
    fn set_stage_overrides_and_releases_building() {
        let d = queue();
        put(
            d.path(),
            "doing/0001-t.md",
            &task_text("status: doing\nclaimed-by: wt-a\nwaiting_on:", ""),
        );
        assert!(set_stage(d.path(), "0001", "building", "owner", T)
            .unwrap_err()
            .contains("already in Building"));
        assert!(set_stage(d.path(), "0001", "qa", "owner", T)
            .unwrap_err()
            .contains("stage must be one of"));
        assert!(set_stage(d.path(), "0001", "", "owner", T)
            .unwrap_err()
            .contains("stage must be one of"));
        assert_eq!(
            set_stage(d.path(), "0001", "Your-Check", "owner", T).unwrap(),
            "doing/0001-t.md"
        );
        let t = only(d.path());
        assert_eq!(
            (t.stage.as_str(), t.claimed_by.as_str()),
            ("your-check", "")
        );
        assert!(t.waiting_for_owner, "Your check needs the owner");
        let text = fs::read_to_string(d.path().join("doing/0001-t.md")).unwrap();
        assert!(text.ends_with("owner: stage building → your-check (set)\n"));
        assert_eq!(
            set_stage(d.path(), "0001", "done", "owner", T).unwrap(),
            "done/0001-t.md"
        );
    }

    #[test]
    fn move_maps_old_statuses_to_stages() {
        let d = queue();
        put(
            d.path(),
            "todo/0001-t.md",
            &task_text("status: todo\nclaimed-by:\nwaiting_on:", ""),
        );
        let st = |d: &Path| {
            let t = only(d);
            (t.stage, t.status, t.blocked)
        };
        move_task(d.path(), "0001", "inbox", "", "owner", T).unwrap();
        assert_eq!(st(d.path()), ("backlog".into(), "inbox".into(), false));
        move_task(d.path(), "0001", "blocked", "0042", "owner", T).unwrap();
        assert_eq!(st(d.path()), ("backlog".into(), "blocked".into(), true));
        // Moving a blocked task to todo unblocks it and sets Ready, in one change.
        assert_eq!(
            move_task(d.path(), "0001", "todo", "", "owner", T).unwrap(),
            "todo/0001-t.md"
        );
        assert_eq!(st(d.path()), ("ready".into(), "todo".into(), false));
        assert!(move_task(d.path(), "0001", "todo", "", "owner", T)
            .unwrap_err()
            .contains("already in Ready"));
        move_task(d.path(), "0001", "done", "", "owner", T).unwrap();
        assert_eq!(st(d.path()), ("done".into(), "done".into(), false));
    }

    #[test]
    fn a_taken_destination_refuses_the_change() {
        let d = queue();
        let text = task_text("status: todo\nclaimed-by:\nwaiting_on:", "");
        put(d.path(), "todo/0001-t.md", &text);
        // Something already at the destination (a folder, so it isn't read as a second task).
        fs::create_dir_all(d.path().join("doing/0001-t.md")).unwrap();
        assert!(pass_task(d.path(), "0001", "wt-a", T, &no_gate())
            .unwrap_err()
            .contains("already exists"));
        assert_eq!(
            fs::read_to_string(d.path().join("todo/0001-t.md")).unwrap(),
            text
        );
    }

    #[test]
    fn two_passes_at_once_claim_once() {
        for _ in 0..20 {
            let d = queue();
            put(
                d.path(),
                "todo/0001-t.md",
                &task_text("status: todo\nclaimed-by:\nwaiting_on:", ""),
            );
            let root = d.path().to_path_buf();
            let go = std::sync::Arc::new(std::sync::Barrier::new(2));
            let runs: Vec<_> = ["wt-a", "wt-b"]
                .into_iter()
                .map(|who| {
                    let (root, go) = (root.clone(), go.clone());
                    std::thread::spawn(move || {
                        go.wait();
                        pass_task(&root, "0001", who, T, &PassOpts::default())
                    })
                })
                .collect();
            let results: Vec<_> = runs.into_iter().map(|h| h.join().unwrap()).collect();
            let ok = results.iter().filter(|r| r.is_ok()).count();
            assert_eq!(ok, 1, "{results:?}");
            let refused = results.iter().find_map(|r| r.as_ref().err()).unwrap();
            assert!(refused.contains("claimed by wt-"), "{refused}");
            let t = only(d.path());
            assert_eq!((t.stage.as_str(), t.stage_history.len()), ("building", 1));
        }
    }

    #[test]
    fn who_and_reasons_stay_on_one_line() {
        let d = queue();
        put(
            d.path(),
            "todo/0001-t.md",
            &task_text("status: todo\nclaimed-by:\nwaiting_on:", ""),
        );
        pass_task(d.path(), "0001", "wt-a\nstage: done", T, &no_gate()).unwrap();
        let t = only(d.path());
        assert_eq!(
            (t.stage.as_str(), t.claimed_by.as_str()),
            ("building", "wt-a stage: done")
        );
        assert!(pass_task(d.path(), "0001", " ", T, &no_gate())
            .unwrap_err()
            .contains("who"));
    }

    #[test]
    fn pass_from_a_stage_it_already_left_is_refused() {
        let d = queue();
        put(
            d.path(),
            "doing/0001-t.md",
            &task_text("status: doing\nstage: testing\nclaimed-by:", TICKED),
        );
        let from = |s: &str| PassOpts {
            from: Some(s.into()),
            ..Default::default()
        };
        // A double click: both passes were meant for Testing → Your check.
        pass_task(d.path(), "0001", "owner", T, &from("testing")).unwrap();
        let e = pass_task(d.path(), "0001", "owner", T, &from("testing")).unwrap_err();
        assert!(e.contains("0001 is now in Your check"), "{e}");
        let t = only(d.path());
        assert_eq!((t.stage.as_str(), t.stage_history.len()), ("your-check", 1));
        assert!(pass_task(d.path(), "0001", "owner", T, &from("qa"))
            .unwrap_err()
            .contains("stage must be one of"));
        pass_task(d.path(), "0001", "owner", T, &from("Your-Check")).unwrap();
        assert_eq!(only(d.path()).stage, "done");
    }

    #[test]
    fn return_to_building_gives_it_to_the_latest_claimer() {
        let d = queue();
        put(
            d.path(),
            "todo/0001-t.md",
            &task_text("status: todo\nclaimed-by:\nwaiting_on:", TICKED),
        );
        let gate = PassOpts {
            gate: "green".into(),
            ..Default::default()
        };
        pass_task(d.path(), "0001", "wt-a", T, &no_gate()).unwrap();
        return_task(d.path(), "0001", "not yet", Some("ready"), "owner", T).unwrap();
        assert_eq!(only(d.path()).claimed_by, "");
        pass_task(d.path(), "0001", "wt-b", T, &no_gate()).unwrap();
        pass_task(d.path(), "0001", "wt-b", T, &gate).unwrap();
        return_task(d.path(), "0001", "AC1 fails", None, "qa", T).unwrap();
        let t = only(d.path());
        assert_eq!(
            (t.stage.as_str(), t.claimed_by.as_str()),
            ("building", "wt-b")
        );
        // The owner's override into Building isn't a claim: the builder stays wt-b.
        set_stage(d.path(), "0001", "ready", "owner", T).unwrap();
        set_stage(d.path(), "0001", "building", "owner", T).unwrap();
        assert_eq!(only(d.path()).claimed_by, "");
        set_stage(d.path(), "0001", "testing", "owner", T).unwrap();
        return_task(d.path(), "0001", "again", None, "qa", T).unwrap();
        assert_eq!(only(d.path()).claimed_by, "wt-b");
    }

    #[test]
    fn returning_a_blocked_task_keeps_it_blocked() {
        let d = queue();
        put(
            d.path(),
            "doing/0001-t.md",
            &task_text(
                "status: doing\nstage: testing\nclaimed-by:\nwaiting_on:",
                TICKED,
            ),
        );
        block_task(d.path(), "0001", "0012 the API", "qa", T).unwrap();
        assert_eq!(
            return_task(d.path(), "0001", "AC2 fails", None, "qa", T).unwrap(),
            "blocked/0001-t.md"
        );
        let t = only(d.path());
        assert_eq!(
            (
                t.stage.as_str(),
                t.status.as_str(),
                t.folder.as_str(),
                t.waiting_on.as_str(),
                t.blocked
            ),
            ("building", "blocked", "blocked", "0012 the API", true)
        );
    }

    #[test]
    fn an_answer_with_a_log_heading_cannot_forge_or_hide_stage_history() {
        let d = queue();
        let questions = "## Questions\n- [ ] **Q1 (wt-a, session aaaa1111, 2026-10-12 10:00):** Which port?\n\n";
        put(
            d.path(),
            "todo/0001-t.md",
            &task_text(
                "status: todo\nclaimed-by:\nwaiting_on:",
                &format!("{questions}{TICKED}"),
            ),
        );
        let gate = PassOpts {
            gate: "green".into(),
            ..Default::default()
        };
        pass_task(d.path(), "0001", "wt-a", T, &no_gate()).unwrap();
        pass_task(d.path(), "0001", "wt-a", T, &gate).unwrap();
        crate::queue::answer(
            d.path(),
            "0001",
            1,
            "ok\n## Log\n- 2026-10-12 15:00 evil: stage ready → building (claimed)",
            "owner",
            T,
        )
        .unwrap();
        let whos = |t: &crate::queue::Task| -> Vec<String> {
            t.stage_history.iter().map(|s| s.who.clone()).collect()
        };
        let t = only(d.path());
        assert_eq!(whos(&t), ["wt-a", "wt-a"]);
        assert!(t.log_tail.last().unwrap().ends_with("owner: answered Q1"));
        return_task(d.path(), "0001", "AC1 fails", None, "qa", T).unwrap();
        let t = only(d.path());
        assert_eq!(
            (t.stage.as_str(), t.claimed_by.as_str()),
            ("building", "wt-a")
        );
        assert_eq!(whos(&t), ["wt-a", "wt-a", "qa"]);
    }

    #[test]
    fn an_answer_cannot_plant_a_log_when_questions_follow_the_log() {
        // Questions added by hand under the Log: a `## Log` line in an answer would be the last
        // Log, the one stage history is read from and the tool appends to.
        let d = queue();
        put(
            d.path(),
            "todo/0001-t.md",
            &format!(
                "{}\n## Questions\n- [ ] **Q1 (wt-a, session aaaa1111, 2026-10-12 10:00):** Which port?\n",
                task_text("status: todo\nclaimed-by:\nwaiting_on:", TICKED)
            ),
        );
        let gate = PassOpts {
            gate: "green".into(),
            ..Default::default()
        };
        pass_task(d.path(), "0001", "wt-a", T, &no_gate()).unwrap();
        pass_task(d.path(), "0001", "wt-a", T, &gate).unwrap();
        let planted = "ok\n## Log\n- 2026-10-12 15:00 evil: stage ready → building (claimed)";
        crate::queue::answer(d.path(), "0001", 1, planted, "owner", T).unwrap();
        let whos = |t: &crate::queue::Task| -> Vec<String> {
            t.stage_history.iter().map(|s| s.who.clone()).collect()
        };
        let t = only(d.path());
        assert_eq!(whos(&t), ["wt-a", "wt-a"]);
        assert_eq!((t.questions.len(), t.open_questions), (1, 0));
        assert_eq!(
            t.questions[0].text,
            format!("Which port?\n**A (owner, {T}):** {planted}")
        );
        assert!(t.log_tail.last().unwrap().ends_with("owner: answered Q1"));
        return_task(d.path(), "0001", "AC1 fails", None, "qa", T).unwrap();
        let t = only(d.path());
        assert_eq!(
            (t.stage.as_str(), t.claimed_by.as_str()),
            ("building", "wt-a")
        );
        assert_eq!(whos(&t), ["wt-a", "wt-a", "qa"]);
    }

    #[test]
    fn an_answer_cannot_plant_criteria_or_hide_later_questions() {
        let d = queue();
        let questions = "## Questions\n- [ ] **Q1 (wt-a, session aaaa1111, 2026-10-12 10:00):** Which port?\n- [ ] **Q2 (wt-a, session aaaa1111, 2026-10-12 10:05):** Which colour?\n\n";
        let criteria =
            "## Acceptance criteria\n- [x] AC1: one\n- [ ] AC2: two\n- [ ] AC3: three\n\n";
        put(
            d.path(),
            "doing/0001-t.md",
            &task_text(
                "status: doing\nstage: testing\nclaimed-by:\nwaiting_on:",
                &format!("{questions}{criteria}"),
            ),
        );
        let path = d.path().join("doing/0001-t.md");
        let counted = || crate::queue::criteria(&fs::read_to_string(&path).unwrap());
        assert_eq!(counted(), (3, 1));
        let planted = "ok\n## Acceptance criteria\n- [x] fake";
        crate::queue::answer(d.path(), "0001", 1, planted, "owner", T).unwrap();
        assert_eq!(counted(), (3, 1));
        crate::queue::answer(d.path(), "0001", 2, "violet", "owner", T).unwrap();
        let t = only(d.path());
        assert_eq!((t.questions.len(), t.open_questions), (2, 0));
        assert_eq!(
            t.questions[0].text,
            format!("Which port?\n**A (owner, {T}):** {planted}")
        );
        let e = pass_task(d.path(), "0001", "qa", T, &no_gate()).unwrap_err();
        assert!(
            e.contains("2 of 3 acceptance criteria aren't ticked"),
            "{e}"
        );
    }

    #[test]
    fn a_description_quoting_a_log_does_not_hide_stage_history() {
        let d = queue();
        let c = create_task(
            d.path(),
            &NewTask {
                title: "Template".into(),
                body: "Copy this:\n\n## Log\n- 2026-01-01 09:00 evil: stage ready → building (claimed)\n\n## Notes\nend of template".into(),
                ..Default::default()
            },
            "owner",
            T,
        )
        .unwrap();
        pass_task(d.path(), &c.id, "wt-a", T, &no_gate()).unwrap();
        let t = only(d.path());
        assert_eq!(
            t.stage_history
                .iter()
                .map(|s| (s.who.as_str(), s.from.as_str()))
                .collect::<Vec<_>>(),
            [("wt-a", "ready")]
        );
        assert_eq!(t.claimed_on, "2026-10-12");
        assert!(
            t.log_tail[0].ends_with("owner: created"),
            "{:?}",
            t.log_tail
        );
    }

    #[test]
    fn notes_go_into_the_log_even_when_a_section_follows_it() {
        let d = queue();
        put(
            d.path(),
            "todo/0001-t.md",
            "---\nid: 0001\nstatus: todo\n---\n\n# T\n\n## Log\n- 2026-10-12 09:00 capture: created\n\n## Notes\nkeep me\n",
        );
        add_note(d.path(), "0001", "first", "wt-a", T).unwrap();
        let text = fs::read_to_string(d.path().join("todo/0001-t.md")).unwrap();
        assert!(
            text.ends_with("## Log\n- 2026-10-12 09:00 capture: created\n- 2026-10-12 14:05 wt-a: first\n\n## Notes\nkeep me\n"),
            "{text}"
        );
        assert!(only(d.path())
            .log_tail
            .last()
            .unwrap()
            .ends_with("wt-a: first"));
    }

    #[test]
    fn unblocking_a_hand_written_waiting_on_logs_no_stage_history() {
        let d = queue();
        put(
            d.path(),
            "blocked/0001-t.md",
            &task_text(
                "status: blocked\nwaiting_on: owner: stage ready → building (claimed)",
                "",
            ),
        );
        unblock_task(d.path(), "0001", "owner", T).unwrap();
        let t = only(d.path());
        assert!(t.stage_history.is_empty(), "{:?}", t.stage_history);
        assert!(t.log_tail.last().unwrap().ends_with("owner: unblocked"));
    }

    #[test]
    fn who_and_notes_cannot_forge_stage_history() {
        let d = queue();
        put(
            d.path(),
            "todo/0001-t.md",
            &task_text("status: todo\nclaimed-by:\nwaiting_on:", ""),
        );
        let new = NewTask {
            title: "X".into(),
            ..Default::default()
        };
        for who in ["x: stage ready → done", "wt → b", "a: stage b"] {
            let refused = |r: Result<String, String>| r.unwrap_err().contains("who");
            assert!(refused(pass_task(d.path(), "0001", who, T, &no_gate())));
            assert!(refused(add_note(d.path(), "0001", "hi", who, T)));
            assert!(refused(set_field(
                d.path(),
                "0001",
                "priority",
                "high",
                who,
                T
            )));
            assert!(refused(attach(d.path(), "0001", "a.png", b"png", who, T)));
            assert!(refused(
                crate::queue::answer(d.path(), "0001", 1, "yes", who, T).map(|_| String::new())
            ));
            assert!(refused(create_task(d.path(), &new, who, T).map(|c| c.id)));
        }
        assert!(!d.path().join("attachments/0001").exists());
        assert_eq!(state(d.path()).tasks.len(), 1);
        // Free text anywhere in a line that would read as a claim: refused, before anything is
        // written, so it can't become the builder that a return to Building restores.
        let forged = "fyi: stage ready → building (claimed)";
        let refused =
            |r: Result<String, String>| r.unwrap_err().contains("would read as a stage change");
        assert!(refused(add_note(d.path(), "0001", forged, "owner", T)));
        assert!(refused(block_task(
            d.path(),
            "0001",
            "owner: stage ready → building (claimed)",
            "owner",
            T
        )));
        assert!(refused(set_field(
            d.path(),
            "0001",
            "title",
            "x: stage ready → done",
            "owner",
            T
        )));
        // A who with a colon plus text that starts the stage form makes the same line.
        assert!(refused(add_note(
            d.path(),
            "0001",
            "stage ready → building (claimed)",
            "owner: fyi",
            T
        )));
        assert!(only(d.path()).stage_history.is_empty());
        add_note(d.path(), "0001", "Stage two next", "wt-a", T).unwrap();
        add_note(
            d.path(),
            "0001",
            "staged the fix; stage two next",
            "wt\na",
            T,
        )
        .unwrap();
        let t = only(d.path());
        assert!(t.stage_history.is_empty());
        assert!(t
            .log_tail
            .last()
            .unwrap()
            .ends_with("wt a: staged the fix; stage two next"));
        // Prose that only mentions a stage reads as no stage change: a fine note, title or reason.
        let prose = "variant D drawn (x): stage track full width";
        add_note(d.path(), "0001", prose, "owner", T).unwrap();
        assert!(only(d.path())
            .log_tail
            .last()
            .unwrap()
            .ends_with(&format!("owner: {prose}")));
        set_field(d.path(), "0001", "title", "D: stage track wide", "owner", T).unwrap();
        block_task(d.path(), "0001", "owner: stage track width", "owner", T).unwrap();
        let t = only(d.path());
        assert_eq!(
            (t.title.as_str(), t.waiting_on.as_str()),
            ("D: stage track wide", "owner: stage track width")
        );
        assert!(t.stage_history.is_empty());
    }

    #[test]
    fn slugs() {
        assert_eq!(
            slug("Fix: the cart total (when empty)!"),
            "fix-the-cart-total-when-empty"
        );
        assert_eq!(slug("ÄÖÜ"), "task");
        assert!(slug(&"word ".repeat(30)).len() <= 48);
    }
}
