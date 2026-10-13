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

pub(crate) fn one_line(text: &str, max: usize, what: &str) -> Result<String, String> {
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

pub(crate) fn body_text(text: &str) -> Result<String, String> {
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
    slug_or(title, "task")
}

/// `slug`, with `fallback` when the text has no ASCII letters or digits at all.
pub(crate) fn slug_or(title: &str, fallback: &str) -> String {
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
        fallback.into()
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
pub(crate) fn has_status_folders(root: &Path) -> bool {
    root.join("todo").is_dir()
}

fn folder_for(root: &Path, status: &str) -> PathBuf {
    if has_status_folders(root) {
        root.join(status)
    } else {
        root.to_path_buf()
    }
}

/// A new task's fields, cleaned by `clean_new`, as `create_locked` writes them.
pub(crate) struct CleanTask {
    pub(crate) title: String,
    kind: &'static str,
    priority: &'static str,
    pub(crate) body: String,
    /// The front-matter values with their leading space (" a, b"), or "" for none.
    tags: String,
    est_time: String,
    est_tokens: String,
}

/// Clean a new task's fields: refused when the title is empty, a choice isn't one, or a text is
/// too long.
pub(crate) fn clean_new(new: &NewTask) -> Result<CleanTask, String> {
    let sp = |v: String| if v.is_empty() { v } else { format!(" {v}") };
    Ok(CleanTask {
        title: one_line(&new.title, MAX_TITLE, "the title")?,
        kind: pick(&new.kind, &KINDS, "task", "type")?,
        priority: pick(&new.priority, &PRIORITIES, "normal", "priority")?,
        body: body_text(&new.body)?,
        tags: sp(clean_tags(&new.tags)?.join(", ")),
        est_time: sp(clean_time(&new.est_time)?),
        est_tokens: sp(clean_tokens(&new.est_tokens)?),
    })
}

/// Write task `id` in `stage` (in its status mirror's folder in the v2 layout), logged as created
/// and then with `log`'s lines. The caller holds the queue's lock and took `id` from `next_id`
/// under it.
pub(crate) fn create_locked(
    root: &Path,
    id: &str,
    t: &CleanTask,
    stage: &str,
    who: &str,
    stamp: &str,
    log: &[String],
) -> Result<Created, String> {
    let status = mirror_status(stage, false);
    let dir = folder_for(root, status);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{id}-{}.md", slug(&t.title)));
    let details = if t.body.is_empty() {
        String::new()
    } else {
        format!("{}\n\n", t.body)
    };
    let date = stamp.get(..10).unwrap_or(stamp);
    let (title, kind, priority, tags) = (&t.title, t.kind, t.priority, &t.tags);
    let (est_time, est_tokens) = (&t.est_time, &t.est_tokens);
    let mut text = format!(
        "---\nid: {id}\ntype: {kind}\npriority: {priority}\ntags:{tags}\nest-time:{est_time}\nest-tokens:{est_tokens}\nstage: {stage}\nstatus: {status}\ncaptured: {date}\nclaimed-by:\nheartbeat:\nwaiting_on:\ncommit:\n---\n\n# {title}\n\n{details}## Log\n- {stamp} {who}: created\n"
    );
    for line in log {
        text = append_log(&text, line);
    }
    write_atomic(&path, &text)?;
    let rel = path
        .strip_prefix(root)
        .unwrap_or(&path)
        .to_string_lossy()
        .to_string();
    Ok(Created {
        id: id.to_string(),
        path: rel,
    })
}

/// Create a task in Ready (`todo/`, or the queue folder in the flat layout). `stamp` is local time.
pub fn create_task(root: &Path, new: &NewTask, who: &str, stamp: &str) -> Result<Created, String> {
    let who = actor(who)?;
    let t = clean_new(new)?;
    with_lock(root, || {
        let id = next_id(root)?;
        create_locked(root, &id, &t, "ready", &who, stamp, &[])
    })
}

/// Replace a front-matter field's value (adding it before the closing `---` if missing).
pub(crate) fn set_front(text: &str, key: &str, value: &str) -> Result<String, String> {
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
    with_lock(root, || edit_locked(root, id, f))
}

/// `edit` for a caller that already holds the queue's lock (the lock isn't reentrant).
fn edit_locked(
    root: &Path,
    id: &str,
    f: impl FnOnce(&Path, String) -> Result<(String, Option<PathBuf>), String>,
) -> Result<String, String> {
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
    change(root, id, &who, stamp, |t| {
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
    /// Unticked criteria only the owner verifies (`— verify: owner`): they don't hold Testing
    /// back, since Your check is where the owner verifies them (0028).
    owner_open: usize,
    /// Open vision conflicts (`vision: <n> open`, 0029) and their ids: they hold Shaping back.
    vision_open: usize,
    vision_ids: Vec<String>,
    /// Who passed it Ready → Building most recently (the builder), if anyone.
    builder: Option<String>,
    /// The claim's lease: the holder's last heartbeat stamp, and the file's modified time.
    heartbeat: String,
    modified: u64,
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
pub(crate) fn free_text_line(line: String, what: &str) -> Result<String, String> {
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
///
/// The claim's lease (`heartbeat:`) is stamped here, so no caller can forget it: when `who` is
/// the one holding the task in Building (their pass, return or block keeps it alive), and when the
/// change hands Building to a holder it didn't have (a claim, or a return to the builder).
fn change(
    root: &Path,
    id: &str,
    who: &str,
    stamp: &str,
    f: impl FnOnce(&Current) -> Result<Change, String>,
) -> Result<String, String> {
    with_lock(root, || change_locked(root, id, who, stamp, f))
}

/// `change` for a caller that already holds the queue's lock.
fn change_locked(
    root: &Path,
    id: &str,
    who: &str,
    stamp: &str,
    f: impl FnOnce(&Current) -> Result<Change, String>,
) -> Result<String, String> {
    edit_locked(root, id, |path, text| {
        let folder = queue::folder_of(root, path);
        let t = queue::parse_task("", root, &folder, path, &text, 0)
            .ok_or("the task has no front matter")?;
        let (_, body) = front_matter(&text).ok_or("the task has no front matter")?;
        let builder = queue::stage_lines(body)
            .into_iter()
            .rev()
            .find(|(s, detail)| s.from == "ready" && s.to == "building" && *detail == "claimed")
            .map(|(s, _)| s.who);
        let modified = fs::metadata(path)
            .map(|m| queue::mtime_secs(&m))
            .map_err(|e| e.to_string())?;
        let items = queue::criteria_items(body);
        let now = Current {
            criteria: (items.len(), items.iter().filter(|c| c.ticked).count()),
            owner_open: items.iter().filter(|c| c.owner && !c.ticked).count(),
            vision_open: t.vision_open.unwrap_or(0),
            vision_ids: t.vision_ids,
            stage: t.stage,
            blocked: t.blocked,
            waiting_on: t.waiting_on,
            claimed_by: t.claimed_by,
            builder,
            heartbeat: t.heartbeat,
            modified,
        };
        let c = f(&now)?;
        // A claim is held Building, not blocked. One per holder (the owner may hold several),
        // whichever change makes it: a pass from Ready, a return to the builder, an unblock, a set.
        let held_before = now.stage == "building" && !now.blocked && !now.claimed_by.is_empty();
        let held_after = c.stage == "building" && !c.blocked && !c.claimed_by.is_empty();
        let new_hold = held_after && !(held_before && now.claimed_by == c.claimed_by);
        if new_hold && c.claimed_by != "owner" {
            if let Some(held) = holding(&tasks_now(root), &c.claimed_by, id) {
                return Err(if c.claimed_by == who {
                    already_holds(&held)
                } else {
                    format!(
                        "{} already holds {held} in Building; one claim per holder, so they pass, return or block it first",
                        c.claimed_by
                    )
                });
            }
        }
        let status = mirror_status(&c.stage, c.blocked);
        let mut new = set_front(&text, "stage", &c.stage)?;
        new = set_front(&new, "status", status)?;
        new = set_front(&new, "waiting_on", &c.waiting_on)?;
        new = set_front(&new, "claimed-by", &c.claimed_by)?;
        if let Some(commit) = &c.commit {
            new = set_front(&new, "commit", commit)?;
        }
        // The lease restarts when the holder acts, and whenever a change makes a hold: a claim,
        // a return to the builder, or an unblock (time spent waiting isn't time gone quiet).
        let holder_acts =
            now.stage == "building" && !now.claimed_by.is_empty() && now.claimed_by == who;
        let new_claim = c.stage == "building"
            && !c.claimed_by.is_empty()
            && (now.stage != "building" || now.claimed_by != c.claimed_by);
        if holder_acts || new_claim || new_hold {
            new = set_front(&new, "heartbeat", stamp)?;
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
/// Ready or Building, Shaping → Ready without acceptance criteria or with an open vision conflict
/// (0029; a task never checked isn't held back), Building → Testing without
/// the gate's result line, Testing → Your check before every criterion but the owner's is
/// ticked, and Your check → Done before the owner's are (`— verify: owner`, 0028).
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
    change(root, id, &who, stamp, |t| {
        pass_change(id, t, &who, stamp, &gate, &commit, expected)
    })
}

/// What a pass does to the task `t` (the rules of `pass_task`, after its inputs are cleaned).
fn pass_change(
    id: &str,
    t: &Current,
    who: &str,
    stamp: &str,
    gate: &str,
    commit: &str,
    expected: Option<&str>,
) -> Result<Change, String> {
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
        "shaping" if t.vision_open > 0 => {
            return Err(crate::vision::holds_ready(
                id,
                t.vision_open,
                &t.vision_ids,
            ))
        }
        "testing" if ticked + t.owner_open < items => {
            return Err(format!(
                "{id}: {} of {items} acceptance criteria aren't ticked; tick each with its proof first{}",
                items - ticked - t.owner_open,
                if t.owner_open > 0 {
                    format!(" ({} more are the owner's, checked in Your check)", t.owner_open)
                } else {
                    String::new()
                }
            ))
        }
        "testing" if t.owner_open > 0 => format!(
            " ({} criteri{} for the owner to verify)",
            t.owner_open,
            if t.owner_open == 1 { "on" } else { "a" }
        ),
        "your-check" if t.owner_open > 0 => {
            return Err(format!(
                "{id}: {} of the owner's criteria aren't ticked; tick each once you've checked it (muster tick {id} <AC>), then pass it to Done",
                t.owner_open
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
            who.to_string()
        } else {
            released(t)
        },
        commit: (to == "done" && !commit.is_empty()).then(|| commit.to_string()),
        log: vec![stage_line(stamp, who, from, to, &detail)],
    })
}

/// A claim as `claim` and `next` report it: the task and its new relative path.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Claimed {
    pub id: String,
    pub path: String,
}

/// Every task of a queue as it is on disk now (files over 1 MB or without front matter are
/// skipped), with each file's modified time. For decisions taken under the lock.
fn tasks_now(root: &Path) -> Vec<queue::Task> {
    let mut out: Vec<queue::Task> = queue::task_files(root)
        .into_iter()
        .filter_map(|(folder, path)| {
            let meta = fs::metadata(&path).ok()?;
            if meta.len() > queue::MAX_TASK_BYTES {
                return None;
            }
            let text = fs::read_to_string(&path).ok()?;
            queue::parse_task("", root, &folder, &path, &text, queue::mtime_secs(&meta))
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// The task `who` holds in Building in this queue (not blocked: a blocked task is waiting, and
/// blocking it is how a holder lets go), other than `except`.
fn holding(tasks: &[queue::Task], who: &str, except: &str) -> Option<String> {
    tasks
        .iter()
        .find(|t| t.stage == "building" && !t.blocked && t.claimed_by == who && t.id != except)
        .map(|t| t.id.clone())
}

fn already_holds(id: &str) -> String {
    format!("you already hold {id}; pass, return or block it first")
}

fn is_id(id: &str) -> bool {
    id.len() == 4 && id.bytes().all(|b| b.is_ascii_digit())
}

/// Claim a Ready task for `who`: Ready → Building, as `pass --from ready`, and refused while
/// `who` already holds another task in Building in this queue. The check and the claim happen
/// under one lock, so of two claims at once only one can win.
pub fn claim_task(root: &Path, id: &str, who: &str, stamp: &str) -> Result<String, String> {
    let who = actor(who)?;
    // `change` refuses a second claim for `who`, under the same lock as the claim itself.
    change(root, id, &who, stamp, |t| {
        pass_change(id, t, &who, stamp, "", "", Some("ready"))
    })
}

fn priority_rank(p: &str) -> u8 {
    match p.to_ascii_lowercase().as_str() {
        "high" => 0,
        "low" => 2,
        _ => 1,
    }
}

/// Claim the top Ready task `who` may take: not blocked, not held in Ready by someone else;
/// high before normal before low, then the lowest id. None when there's nothing to take.
/// Refused while `who` already holds a task in Building here.
pub fn next_task(root: &Path, who: &str, stamp: &str) -> Result<Option<Claimed>, String> {
    let who = actor(who)?;
    with_lock(root, || {
        let tasks = tasks_now(root);
        // Checked before picking, so a holder is told so rather than handed a refusal for one
        // task; `change` checks again. The owner may hold several.
        if let Some(held) = holding(&tasks, &who, "").filter(|_| who != "owner") {
            return Err(already_holds(&held));
        }
        let Some(top) = tasks
            .iter()
            .filter(|t| t.stage == "ready" && !t.blocked && is_id(&t.id))
            .filter(|t| t.claimed_by.is_empty() || t.claimed_by == who)
            .min_by_key(|t| (priority_rank(&t.priority), t.id.clone()))
        else {
            return Ok(None);
        };
        let id = top.id.clone();
        let path = change_locked(root, &id, &who, stamp, |t| {
            pass_change(&id, t, &who, stamp, "", "", Some("ready"))
        })?;
        Ok(Some(Claimed { id, path }))
    })
}

/// Keep a claim alive: stamp `heartbeat:` on a task `who` holds in Building. No log line.
pub fn heartbeat_task(root: &Path, id: &str, who: &str, stamp: &str) -> Result<String, String> {
    let who = actor(who)?;
    edit(root, id, |path, text| {
        let t = queue::parse_task("", root, &queue::folder_of(root, path), path, &text, 0)
            .ok_or("the task has no front matter")?;
        if t.stage != "building" {
            return Err(format!(
                "{id} is in {}, not Building; only a claim in Building has a lease to keep alive",
                stage_label(&t.stage)
            ));
        }
        if t.claimed_by != who {
            return Err(if t.claimed_by.is_empty() {
                format!("nobody holds {id}; claim it first")
            } else {
                format!("{id} is held by {}, not {who}", t.claimed_by)
            });
        }
        Ok((set_front(&text, "heartbeat", stamp)?, None))
    })
}

/// The default lease: a claim with no heartbeat for this long can be reclaimed.
pub const LEASE: &str = "12h";
/// `LEASE` in minutes, what `lease_minutes("")` gives (a test pins the two together).
pub const LEASE_MINUTES: u64 = 12 * 60;

/// A lease's length in minutes, from a time like `clean_time` takes (30m, 2h, 1d). A day here is
/// 24 hours (not the 8-hour agent day of estimates). Empty is the default, 12 h.
pub fn lease_minutes(input: &str) -> Result<u64, String> {
    let input = if input.trim().is_empty() {
        LEASE
    } else {
        input
    };
    let t = clean_time(input)?;
    let (num, per) = match t.as_bytes().last() {
        Some(b'm') => (&t[..t.len() - 1], 1.0),
        Some(b'h') => (&t[..t.len() - 1], 60.0),
        Some(b'd') => (&t[..t.len() - 1], 1440.0),
        _ => return Err(format!("lease {input:?}: use 30m, 2h or 1d")),
    };
    let n: f64 = num
        .parse()
        .map_err(|_| format!("lease {input:?}: use 30m, 2h or 1d"))?;
    let minutes = (n * per).round();
    if minutes < 1.0 {
        return Err(format!("lease {input:?} is shorter than a minute"));
    }
    // clean_time caps the number at 1000, so this is at most 1,440,000 minutes.
    Ok(minutes as u64)
}

/// A tool stamp, `YYYY-MM-DD HH:MM` local time, as minutes since 1970-01-01 00:00 (same zone).
/// None for anything else, a date that doesn't exist included.
pub fn stamp_minutes(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    let shape = b.len() == 16
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b' ',
            13 => *c == b':',
            _ => c.is_ascii_digit(),
        });
    if !shape {
        return None;
    }
    let num = |a: usize, z: usize| s[a..z].parse::<i64>().ok();
    let (y, m, d, h, min) = (
        num(0, 4)?,
        num(5, 7)?,
        num(8, 10)?,
        num(11, 13)?,
        num(14, 16)?,
    );
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days_in = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    if !(1..=days_in).contains(&d) || h > 23 || min > 59 {
        return None;
    }
    // Days from the civil date (H. Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 1440 + h * 60 + min)
}

/// How long a claim has been quiet, in minutes: since its heartbeat when that's a tool stamp,
/// else since the file last changed. `stamp` and `now_secs` are the same moment, as local time
/// and as seconds since 1970, so neither needs the time zone.
pub fn quiet_minutes(heartbeat: &str, modified: u64, stamp: &str, now_secs: u64) -> u64 {
    match (stamp_minutes(heartbeat), stamp_minutes(stamp)) {
        (Some(beat), Some(now)) => u64::try_from(now - beat).unwrap_or(0),
        _ => now_secs.saturating_sub(modified) / 60,
    }
}

/// "13 h", or "45 min" under an hour.
fn quiet_text(minutes: u64) -> String {
    if minutes >= 60 {
        format!("{} h", minutes / 60)
    } else {
        format!("{minutes} min")
    }
}

/// A claim `reclaim` gave back to Ready.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Released {
    pub id: String,
    pub holder: String,
    /// How long it had been quiet ("13 h").
    pub quiet: String,
    pub path: String,
}

#[derive(Serialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Reclaimed {
    pub released: Vec<Released>,
    /// Claims past their lease that couldn't be released, with why.
    pub failed: Vec<String>,
}

/// What releasing the claim on `t` writes, or why it's refused: only a claim in Building, not
/// blocked, quiet for longer than `lease` minutes.
fn release_change(
    id: &str,
    t: &Current,
    lease: u64,
    who: &str,
    stamp: &str,
    now_secs: u64,
) -> Result<Change, String> {
    if t.stage != "building" {
        return Err(format!(
            "{id} is in {}, not Building; there's no claim to release",
            stage_label(&t.stage)
        ));
    }
    if t.claimed_by.is_empty() {
        return Err(format!("nobody holds {id}; there's no claim to release"));
    }
    if t.blocked {
        return Err(format!(
            "{id} is blocked on {}; it's waiting, not quiet",
            t.waiting_on
        ));
    }
    let quiet = quiet_minutes(&t.heartbeat, t.modified, stamp, now_secs);
    if quiet <= lease {
        return Err(format!(
            "{id}'s lease hasn't run out: {} was last heard from {} ago (the lease is {})",
            t.claimed_by,
            quiet_text(quiet),
            quiet_text(lease)
        ));
    }
    Ok(Change {
        stage: "ready".into(),
        blocked: false,
        waiting_on: t.waiting_on.clone(),
        claimed_by: String::new(),
        commit: None,
        log: vec![stage_line(
            stamp,
            who,
            "building",
            "ready",
            &format!(
                " (reclaimed from {} (no heartbeat for {}))",
                t.claimed_by,
                quiet_text(quiet)
            ),
        )],
    })
}

/// Give every claim whose lease ran out back to Ready: each task in Building, not blocked, held
/// by someone, with no heartbeat (or, without one, no change to its file) for longer than
/// `lease` minutes. Live claims are left alone. One lock for the whole pass, so a heartbeat
/// lands either before it (and counts) or after it (and is refused: the task isn't Building).
pub fn reclaim(
    root: &Path,
    lease: u64,
    who: &str,
    stamp: &str,
    now_secs: u64,
) -> Result<Reclaimed, String> {
    let who = actor(who)?;
    with_lock(root, || {
        let mut out = Reclaimed::default();
        let quiet = tasks_now(root).into_iter().filter(|t| {
            t.stage == "building"
                && !t.blocked
                && !t.claimed_by.is_empty()
                && quiet_minutes(&t.heartbeat, t.modified, stamp, now_secs) > lease
        });
        for t in quiet {
            match change_locked(root, &t.id, &who, stamp, |c| {
                release_change(&t.id, c, lease, &who, stamp, now_secs)
            }) {
                Ok(path) => out.released.push(Released {
                    quiet: quiet_text(quiet_minutes(&t.heartbeat, t.modified, stamp, now_secs)),
                    id: t.id,
                    holder: t.claimed_by,
                    path,
                }),
                Err(e) => out.failed.push(format!("{}: {e}", t.id)),
            }
        }
        Ok(out)
    })
}

/// Release one claim whose lease ran out (the app's Release): refused if it's live again.
pub fn reclaim_task(
    root: &Path,
    id: &str,
    lease: u64,
    who: &str,
    stamp: &str,
    now_secs: u64,
) -> Result<String, String> {
    let who = actor(who)?;
    change(root, id, &who, stamp, |t| {
        release_change(id, t, lease, &who, stamp, now_secs)
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
    change(root, id, &who, stamp, |t| {
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
    change(root, id, &who, stamp, |t| {
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
    change(root, id, &who, stamp, |t| {
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
    change(root, id, &who, stamp, |t| {
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
    edit(root, id, |path, text| {
        let mut new = append_log(&text, &line);
        // The holder's note keeps its claim alive.
        let t = queue::parse_task("", root, &queue::folder_of(root, path), path, &text, 0);
        if t.is_some_and(|t| {
            t.stage == "building" && !t.claimed_by.is_empty() && t.claimed_by == who
        }) {
            new = set_front(&new, "heartbeat", stamp)?;
        }
        Ok((new, None))
    })
}

/// Tick (or, with `undo`, untick) one of the criteria only the owner verifies, by label (`AC2`)
/// or position (`2`). Agents tick their own criteria with the proof in the text; this is for the
/// owner's, which have no proof but the owner's look. Logged with who and when.
pub fn tick_criterion(
    root: &Path,
    id: &str,
    which: &str,
    undo: bool,
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    let who = actor(who)?;
    let which = which.trim();
    if which.is_empty() || which.len() > 8 {
        return Err(format!(
            "say which criterion: its label (AC2) or number (2), got {which:?}"
        ));
    }
    edit(root, id, |_, text| {
        let (_, body) = front_matter(&text).ok_or("the task has no front matter")?;
        // The body is a slice of the text, so offsets in it map onto the file.
        let base = body.as_ptr() as usize - text.as_ptr() as usize;
        let items = queue::criteria_items(body);
        let c = items
            .iter()
            .find(|c| c.label.eq_ignore_ascii_case(which) || c.n.to_string() == which)
            .ok_or_else(|| format!("{id} has no acceptance criterion {which}"))?;
        let name = if c.label.is_empty() {
            format!("criterion {}", c.n)
        } else {
            c.label.clone()
        };
        if !c.owner {
            return Err(format!(
                "{name} isn't the owner's to verify (its verify part doesn't start with owner); tick it in the file with its proof"
            ));
        }
        if c.ticked != undo {
            return Err(format!(
                "{id} {name} is already {}",
                if undo { "unticked" } else { "ticked" }
            ));
        }
        let at = base + c.at;
        let mut new = String::with_capacity(text.len());
        new.push_str(&text[..at]);
        new.push_str(if undo { "- [ ]" } else { "- [x]" });
        new.push_str(&text[at + 5..]);
        let line = format!(
            "{stamp} {who}: {} {name}",
            if undo { "unticked" } else { "ticked" }
        );
        Ok((append_log(&new, &line), None))
    })
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

/// Where a task's write-up is in its text, as (start, end) byte offsets: from the end of the
/// front matter to the Log's heading (the last `## Log`, the one `queue::log_span` reads), or to
/// the end when it has no Log. The front matter and the Log are the tool's; this is the rest.
fn body_span(text: &str) -> Result<(usize, usize), String> {
    let (_, body) = front_matter(text).ok_or("the task has no front matter")?;
    let start = text.len() - body.len();
    let end = match queue::log_span(body) {
        Some((content, _)) => {
            let through_heading = &body[..content];
            let before = through_heading
                .strip_suffix('\n')
                .unwrap_or(through_heading);
            start + before.rfind('\n').map_or(0, |i| i + 1)
        }
        None => text.len(),
    };
    Ok((start, end))
}

/// The write-up as an editor opens it: the text between the front matter and the Log, without
/// the blank lines around it.
pub fn editable_body(text: &str) -> Result<String, String> {
    let (start, end) = body_span(text)?;
    Ok(text[start..end].trim_matches(['\n', '\r']).to_string())
}

/// Who holds `t` in Building, when that's someone other than `who`. While they do, its write-up
/// belongs to their branch (decision 0003), so nobody else edits it.
fn write_up_holder(t: &queue::Task, who: &str) -> Option<String> {
    (t.stage == "building" && !t.claimed_by.is_empty() && t.claimed_by != who)
        .then(|| t.claimed_by.clone())
}

fn held_by(holder: &str) -> String {
    format!("{holder} holds it; add a note instead (the write-up is theirs while they build it)")
}

/// The write-up of task `id` for `who` to edit; refused while someone else holds it in Building.
pub fn open_body(root: &Path, id: &str, who: &str) -> Result<String, String> {
    let who = actor(who)?;
    let path = find_task(root, id)?;
    let text = queue::read_body(root, id)?;
    let t = queue::parse_task("", root, &queue::folder_of(root, &path), &path, &text, 0)
        .ok_or("the task has no front matter")?;
    if let Some(holder) = write_up_holder(&t, &who) {
        return Err(held_by(&holder));
    }
    editable_body(&text)
}

/// A new write-up, cleaned like a new task's description (control characters out, newline and
/// tab kept, trimmed, 50 000 characters at most). Refused when it's empty, has no `# ` title line,
/// or has a line the tool would read as its own: a `## Log` heading or a stage change.
fn clean_write_up(text: &str) -> Result<String, String> {
    let t = body_text(text)?;
    if t.is_empty() {
        return Err("the write-up is empty".into());
    }
    for line in t.lines() {
        if line.trim_end() == "## Log" {
            return Err("the write-up has a \"## Log\" line, and the Log is the tool's: it stays below the write-up (add to it with a note)".into());
        }
        if queue::stage_line(line).is_some() {
            return Err(format!(
                "the write-up has a line that would read as a stage change in the log (\"<who>: stage <from> → <to>\"); only muster pass, return and stage write those: {line:?}"
            ));
        }
    }
    if !t.lines().any(|l| l.starts_with("# ")) {
        return Err("the write-up needs its \"# <title>\" line".into());
    }
    Ok(t)
}

/// Replace task `id`'s write-up (between the front matter and the Log) with `new_body`, as
/// `who`, under the queue's lock. `expected` is the write-up the editor opened (`open_body`): if
/// the file's is different now, the edit is refused and nothing changes. Refused too while
/// someone else holds the task in Building. The front matter and every Log line stay as they
/// are; the edit is logged. Returns the task's relative path, or None when the text is the same.
pub fn edit_body(
    root: &Path,
    id: &str,
    new_body: &str,
    expected: &str,
    who: &str,
    stamp: &str,
) -> Result<Option<String>, String> {
    let who = actor(who)?;
    let new_body = clean_write_up(new_body)?;
    with_lock(root, || {
        let path = find_task(root, id)?;
        let text = queue::read_body(root, id)?;
        let t = queue::parse_task("", root, &queue::folder_of(root, &path), &path, &text, 0)
            .ok_or("the task has no front matter")?;
        if let Some(holder) = write_up_holder(&t, &who) {
            return Err(held_by(&holder));
        }
        let (start, end) = body_span(&text)?;
        let current = text[start..end].trim_matches(['\n', '\r']);
        if current != expected {
            return Err(format!(
                "{id} changed since you opened it, so nothing was saved; open it again to edit what's there now"
            ));
        }
        if body_text(current).is_ok_and(|c| c == new_body) {
            return Ok(None);
        }
        let head = &text[..start];
        let mut new = if end == text.len() {
            format!("{head}\n{new_body}\n")
        } else {
            format!("{head}\n{new_body}\n\n{}", &text[end..])
        };
        new = append_log(&new, &format!("{stamp} {who}: edited the write-up"));
        // The holder's edit keeps its claim alive.
        if t.stage == "building" && t.claimed_by == who {
            new = set_front(&new, "heartbeat", stamp)?;
        }
        write_atomic(&path, &new)?;
        Ok(Some(
            path.strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string(),
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::{base64, read_state_at, Cache};

    fn queue() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for f in STATUS_FOLDERS {
            fs::create_dir(d.path().join(f)).unwrap();
        }
        fs::write(d.path().join("README.md"), "q").unwrap();
        d
    }
    const T: &str = "2026-10-12 14:05";
    /// The queue's state as the app reads it, at the tests' time `T`: a claim stamped at `T`
    /// is fresh whatever the wall clock says.
    fn state(d: &Path) -> crate::queue::State {
        read_state_at(&[("p".into(), d.to_path_buf())], &mut Cache::default(), T)
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

    const OWNERS: &str = "## Acceptance criteria\n- [x] AC1: built — verify: command\n- [ ] AC2: tried it — verify: owner\n- [ ] AC3: looks right —\n  verify: owner (screenshots)\n\n";

    #[test]
    fn testing_passes_without_the_owners_criteria_and_says_so() {
        let d = queue();
        put(
            d.path(),
            "doing/0001-t.md",
            &task_text(
                "status: doing\nstage: testing\nclaimed-by:\nwaiting_on:",
                OWNERS,
            ),
        );
        pass_task(d.path(), "0001", "qa", T, &no_gate()).unwrap();
        let t = only(d.path());
        assert_eq!(t.stage, "your-check");
        let text = fs::read_to_string(d.path().join(&t.path)).unwrap();
        assert!(
            text.ends_with("qa: stage testing → your-check (2 criteria for the owner to verify)\n"),
            "{text}"
        );
    }

    #[test]
    fn testing_still_refuses_an_agents_unticked_criterion() {
        let d = queue();
        let mixed = "## Acceptance criteria\n- [ ] AC1: a test — verify: command\n- [ ] AC2: tried it — verify: owner\n- [ ] AC3: test and look — verify: command (test) and owner\n\n";
        put(
            d.path(),
            "doing/0001-t.md",
            &task_text(
                "status: doing\nstage: testing\nclaimed-by:\nwaiting_on:",
                mixed,
            ),
        );
        let before = fs::read_to_string(d.path().join("doing/0001-t.md")).unwrap();
        let e = pass_task(d.path(), "0001", "qa", T, &no_gate()).unwrap_err();
        assert!(
            e.contains("2 of 3 acceptance criteria aren't ticked")
                && e.contains("(1 more are the owner's"),
            "{e}"
        );
        assert_eq!(
            fs::read_to_string(d.path().join("doing/0001-t.md")).unwrap(),
            before
        );
    }

    #[test]
    fn done_needs_the_owners_criteria_ticked_and_tick_does_it() {
        let d = queue();
        put(
            d.path(),
            "doing/0001-t.md",
            &task_text(
                "status: doing\nstage: your-check\nclaimed-by:\nwaiting_on:",
                OWNERS,
            ),
        );
        let e = pass_task(d.path(), "0001", "owner", T, &no_gate()).unwrap_err();
        assert!(
            e.contains("2 of the owner's criteria aren't ticked") && e.contains("muster tick 0001"),
            "{e}"
        );
        tick_criterion(d.path(), "0001", "AC2", false, "owner", T).unwrap();
        tick_criterion(d.path(), "0001", "3", false, "owner", T).unwrap();
        let t = only(d.path());
        assert!(t.owner_criteria.iter().all(|c| c.ticked));
        let text = fs::read_to_string(d.path().join(&t.path)).unwrap();
        assert!(text.contains("- [x] AC2: tried it — verify: owner\n- [x] AC3: looks right —\n  verify: owner (screenshots)\n"), "{text}");
        assert!(
            text.ends_with(
                "- 2026-10-12 14:05 owner: ticked AC2\n- 2026-10-12 14:05 owner: ticked AC3\n"
            ),
            "{text}"
        );
        pass_task(d.path(), "0001", "owner", T, &no_gate()).unwrap();
        assert_eq!(only(d.path()).stage, "done");
    }

    #[test]
    fn done_waits_only_on_the_owners_criteria() {
        // Put in Your check by the owner's override with an agent's criterion open: Done isn't
        // held for it (as before 0028); only the owner's criteria hold Done.
        let d = queue();
        let agents = "## Acceptance criteria\n- [ ] AC1: a test — verify: command\n\n";
        put(
            d.path(),
            "doing/0001-t.md",
            &task_text(
                "status: doing\nstage: your-check\nclaimed-by:\nwaiting_on:",
                agents,
            ),
        );
        pass_task(d.path(), "0001", "owner", T, &no_gate()).unwrap();
        assert_eq!(only(d.path()).stage, "done");
    }

    #[test]
    fn tick_changes_only_its_box_and_logs_it() {
        let d = queue();
        put(
            d.path(),
            "doing/0001-t.md",
            &task_text(
                "status: doing\nstage: your-check\nclaimed-by:\nwaiting_on:",
                OWNERS,
            ),
        );
        let before = fs::read_to_string(d.path().join("doing/0001-t.md")).unwrap();
        tick_criterion(d.path(), "0001", "ac2", false, "owner", T).unwrap();
        let after = fs::read_to_string(d.path().join("doing/0001-t.md")).unwrap();
        let expected =
            before.replacen("- [ ] AC2", "- [x] AC2", 1) + "- 2026-10-12 14:05 owner: ticked AC2\n";
        assert_eq!(after, expected);
        tick_criterion(d.path(), "0001", "AC2", true, "owner", T).unwrap();
        let undone = fs::read_to_string(d.path().join("doing/0001-t.md")).unwrap();
        assert_eq!(
            undone,
            before
                + "- 2026-10-12 14:05 owner: ticked AC2\n- 2026-10-12 14:05 owner: unticked AC2\n"
        );
    }

    #[test]
    fn tick_refusals_leave_the_file_alone() {
        let d = queue();
        put(
            d.path(),
            "doing/0001-t.md",
            &task_text(
                "status: doing\nstage: your-check\nclaimed-by:\nwaiting_on:",
                OWNERS,
            ),
        );
        let before = fs::read_to_string(d.path().join("doing/0001-t.md")).unwrap();
        for (which, undo, why) in [
            ("AC1", false, "isn't the owner's to verify"),
            ("AC9", false, "has no acceptance criterion AC9"),
            ("", false, "say which criterion"),
            ("123456789", false, "say which criterion"),
            ("AC2", true, "already unticked"),
        ] {
            let e = tick_criterion(d.path(), "0001", which, undo, "owner", T).unwrap_err();
            assert!(e.contains(why), "{which}: {e}");
        }
        assert_eq!(
            fs::read_to_string(d.path().join("doing/0001-t.md")).unwrap(),
            before
        );
        tick_criterion(d.path(), "0001", "AC2", false, "owner", T).unwrap();
        assert!(tick_criterion(d.path(), "0001", "AC2", false, "owner", T)
            .unwrap_err()
            .contains("already ticked"));
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

    /// A task file `<id>-t.md` in `folder` ("" for the flat layout) with these front-matter lines.
    fn put_task(d: &Path, folder: &str, id: &str, fm: &str) -> PathBuf {
        let dir = if folder.is_empty() {
            d.to_path_buf()
        } else {
            d.join(folder)
        };
        let p = dir.join(format!("{id}-t.md"));
        fs::write(
            &p,
            format!("---\nid: {id}\ntype: task\n{fm}\n---\n\n# Task {id}\n\n## Log\n- 2026-10-12 09:00 capture: created\n"),
        )
        .unwrap();
        p
    }
    fn ready_fm(priority: &str) -> String {
        format!("priority: {priority}\nstage: ready\nstatus: todo\nclaimed-by:\nheartbeat:\nwaiting_on:")
    }
    fn task_of(d: &Path, id: &str) -> crate::queue::Task {
        crate::queue::read_task(d, id).unwrap()
    }
    fn file_of(d: &Path, id: &str) -> String {
        crate::queue::read_body(d, id).unwrap()
    }

    #[test]
    fn claim_takes_ready_starts_the_lease_and_allows_one_claim_per_holder() {
        let d = queue();
        for id in ["0001", "0002", "0003"] {
            put_task(d.path(), "todo", id, &ready_fm("normal"));
        }
        assert_eq!(
            claim_task(d.path(), "0001", "wt-a", T).unwrap(),
            "doing/0001-t.md"
        );
        let t = task_of(d.path(), "0001");
        assert_eq!(
            (
                t.stage.as_str(),
                t.status.as_str(),
                t.claimed_by.as_str(),
                t.heartbeat.as_str()
            ),
            ("building", "doing", "wt-a", T)
        );
        assert!(file_of(d.path(), "0001")
            .ends_with("- 2026-10-12 14:05 wt-a: stage ready → building (claimed)\n"));
        // A second claim by the same holder: refused, and the file is left alone.
        let before = file_of(d.path(), "0002");
        let e = claim_task(d.path(), "0002", "wt-a", T).unwrap_err();
        assert_eq!(e, "you already hold 0001; pass, return or block it first");
        assert_eq!(file_of(d.path(), "0002"), before);
        // Who is cleaned the same way before it's compared.
        assert!(claim_task(d.path(), "0002", " wt-a\n", T)
            .unwrap_err()
            .contains("you already hold 0001"));
        // Another holder may; a task already in Building can't be claimed again.
        claim_task(d.path(), "0002", "wt-b", T).unwrap();
        assert!(claim_task(d.path(), "0001", "wt-c", T)
            .unwrap_err()
            .contains("0001 is now in Building, not Ready"));
        // Blocking your task frees you to claim another.
        block_task(d.path(), "0001", "0002 the API", "wt-a", T).unwrap();
        claim_task(d.path(), "0003", "wt-a", T).unwrap();
        assert_eq!(task_of(d.path(), "0003").claimed_by, "wt-a");
        assert!(state(d.path()).problems.is_empty());
    }

    #[test]
    fn claim_refusals() {
        let d = queue();
        put_task(
            d.path(),
            "inbox",
            "0001",
            "priority: normal\nstage: backlog\nstatus: inbox",
        );
        put_task(
            d.path(),
            "blocked",
            "0002",
            "priority: normal\nstage: ready\nstatus: blocked\nwaiting_on: owner",
        );
        assert!(claim_task(d.path(), "0001", "wt-a", T)
            .unwrap_err()
            .contains("0001 is now in Backlog, not Ready"));
        assert!(claim_task(d.path(), "0002", "wt-a", T)
            .unwrap_err()
            .contains("blocked on owner"));
        assert!(claim_task(d.path(), "0009", "wt-a", T)
            .unwrap_err()
            .contains("no task 0009"));
        assert!(claim_task(d.path(), "0001", " ", T)
            .unwrap_err()
            .contains("who"));
    }

    #[test]
    fn next_claims_the_top_ready_task_by_priority_then_id() {
        let d = queue();
        put_task(d.path(), "todo", "0001", &ready_fm("low"));
        put_task(d.path(), "todo", "0002", &ready_fm("normal"));
        put_task(
            d.path(),
            "blocked",
            "0003",
            "priority: high\nstage: ready\nstatus: blocked\nwaiting_on: 0001 first",
        );
        put_task(d.path(), "todo", "0005", &ready_fm("high"));
        put_task(d.path(), "todo", "0004", &ready_fm("High"));
        put_task(
            d.path(),
            "todo",
            "0006",
            "priority: high\nstage: ready\nstatus: todo\nclaimed-by: wt-z",
        );
        put_task(
            d.path(),
            "inbox",
            "0007",
            "priority: high\nstage: backlog\nstatus: inbox",
        );
        put_task(
            d.path(),
            "todo",
            "0008",
            "priority: high\nstage: shaping\nstatus: todo",
        );
        // An old file without a stage: status todo means Ready, and no priority means normal.
        put_task(d.path(), "todo", "0009", "status: todo");
        let next = |who: &str| next_task(d.path(), who, T).map(|c| c.map(|c| c.id));
        assert_eq!(next("wt-a").unwrap().as_deref(), Some("0004"));
        assert_eq!(
            next("wt-a").unwrap_err(),
            "you already hold 0004; pass, return or block it first"
        );
        assert_eq!(next("wt-b").unwrap().as_deref(), Some("0005"));
        // 0006 is held in Ready by wt-z, so only wt-z takes it.
        assert_eq!(next("wt-z").unwrap().as_deref(), Some("0006"));
        assert_eq!(next("wt-c").unwrap().as_deref(), Some("0002"));
        assert_eq!(next("wt-d").unwrap().as_deref(), Some("0009"));
        assert_eq!(next("wt-e").unwrap().as_deref(), Some("0001"));
        assert_eq!(next("wt-f").unwrap(), None, "nothing ready");
        let t = task_of(d.path(), "0004");
        assert_eq!(
            (
                t.stage.as_str(),
                t.claimed_by.as_str(),
                t.heartbeat.as_str()
            ),
            ("building", "wt-a", T)
        );
        assert_eq!(task_of(d.path(), "0003").stage, "ready", "blocked stays");
    }

    #[test]
    fn heartbeat_is_the_holders_and_only_in_building() {
        let d = queue();
        put_task(d.path(), "todo", "0001", &ready_fm("normal"));
        put_task(d.path(), "todo", "0002", &ready_fm("normal"));
        claim_task(d.path(), "0001", "wt-a", "2026-10-12 09:00").unwrap();
        assert_eq!(task_of(d.path(), "0001").heartbeat, "2026-10-12 09:00");
        let log_before = task_of(d.path(), "0001").log_tail;
        assert_eq!(
            heartbeat_task(d.path(), "0001", "wt-a", "2026-10-12 10:00").unwrap(),
            "doing/0001-t.md"
        );
        let t = task_of(d.path(), "0001");
        assert_eq!(t.heartbeat, "2026-10-12 10:00");
        assert_eq!(t.log_tail, log_before, "a heartbeat adds no log line");
        assert_eq!(
            heartbeat_task(d.path(), "0001", "wt-b", "2026-10-12 11:00").unwrap_err(),
            "0001 is held by wt-a, not wt-b"
        );
        assert!(heartbeat_task(d.path(), "0002", "wt-a", T)
            .unwrap_err()
            .contains("0002 is in Ready, not Building"));
        // Someone else's note doesn't renew the lease; the holder's note, block and pass do.
        add_note(d.path(), "0001", "looks good", "owner", "2026-10-12 11:00").unwrap();
        assert_eq!(task_of(d.path(), "0001").heartbeat, "2026-10-12 10:00");
        add_note(d.path(), "0001", "phase 1 done", "wt-a", "2026-10-12 11:30").unwrap();
        assert_eq!(task_of(d.path(), "0001").heartbeat, "2026-10-12 11:30");
        let gate = PassOpts {
            gate: "green".into(),
            ..Default::default()
        };
        pass_task(d.path(), "0001", "wt-a", "2026-10-12 12:00", &gate).unwrap();
        assert_eq!(task_of(d.path(), "0001").heartbeat, "2026-10-12 12:00");
        // Testing returns it to Building, to its builder: the lease starts again then.
        return_task(
            d.path(),
            "0001",
            "AC1 fails",
            None,
            "qa",
            "2026-10-13 09:00",
        )
        .unwrap();
        let t = task_of(d.path(), "0001");
        assert_eq!(
            (t.claimed_by.as_str(), t.heartbeat.as_str()),
            ("wt-a", "2026-10-13 09:00")
        );
        // Nobody holds it any more: no heartbeat.
        set_stage(d.path(), "0001", "ready", "owner", T).unwrap();
        assert_eq!(
            heartbeat_task(d.path(), "0001", "wt-a", T).unwrap_err(),
            "0001 is in Ready, not Building; only a claim in Building has a lease to keep alive"
        );
    }

    #[test]
    fn quiet_time_comes_from_the_heartbeat_else_the_file() {
        assert_eq!(
            stamp_minutes("2026-10-12 14:05"),
            stamp_minutes("2026-10-12 14:05")
        );
        let m = |a: &str, b: &str| stamp_minutes(b).unwrap() - stamp_minutes(a).unwrap();
        assert_eq!(m("2026-10-12 01:00", "2026-10-12 14:05"), 13 * 60 + 5);
        assert_eq!(m("2026-12-31 23:00", "2027-01-01 01:00"), 120);
        assert_eq!(
            m("2028-02-28 12:00", "2028-03-01 12:00"),
            2 * 1440,
            "leap year"
        );
        assert_eq!(m("2026-02-28 12:00", "2026-03-01 12:00"), 1440);
        for bad in [
            "",
            "2026-10-12",
            "2026-10-12T14:05",
            "2026-13-01 10:00",
            "2026-02-30 10:00",
            "2026-10-12 24:00",
            "2026-10-12 10:60",
            "2026-1é-12 10:00",
        ] {
            assert_eq!(stamp_minutes(bad), None, "{bad:?}");
        }
        let now = 1_000_000;
        assert_eq!(quiet_minutes("2026-10-12 01:00", 0, T, now), 13 * 60 + 5);
        // A heartbeat ahead of the clock is no time at all.
        assert_eq!(quiet_minutes("2026-10-13 01:00", 0, T, now), 0);
        // Empty or not a stamp: the file's modified time.
        assert_eq!(quiet_minutes("", now - 3 * 3600, T, now), 180);
        assert_eq!(quiet_minutes("yesterday", now - 600, T, now), 10);
        assert_eq!(lease_minutes("").unwrap(), LEASE_MINUTES);
        assert_eq!(LEASE_MINUTES, 12 * 60);
        assert_eq!(lease_minutes("30m").unwrap(), 30);
        assert_eq!(lease_minutes(" 2 h ").unwrap(), 120);
        assert_eq!(lease_minutes("1d").unwrap(), 24 * 60, "a lease day is 24 h");
        assert!(lease_minutes("soon").is_err() && lease_minutes("0.01m").is_err());
    }

    /// Back-date a file's modified time by `secs` from `now`.
    fn age(p: &Path, now: u64, secs: u64) {
        let f = fs::File::options().write(true).open(p).unwrap();
        f.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(now - secs))
            .unwrap();
    }

    #[test]
    fn reclaim_returns_quiet_claims_to_ready_and_leaves_live_ones() {
        let d = queue();
        let now = crate::queue::now_secs();
        let building = |hb: &str, who: &str| {
            format!("priority: normal\nstage: building\nstatus: doing\nclaimed-by: {who}\nheartbeat: {hb}\nwaiting_on:")
        };
        // 13 h since its heartbeat, though the file changed a minute ago (an answer, say).
        let p = put_task(
            d.path(),
            "doing",
            "0001",
            &building("2026-10-12 01:00", "wt-a"),
        );
        age(&p, now, 60);
        // 4 h: live.
        put_task(
            d.path(),
            "doing",
            "0002",
            &building("2026-10-12 10:00", "wt-b"),
        );
        // No heartbeat: the file's time, 13 h.
        let p = put_task(d.path(), "doing", "0003", &building("", "wt-c"));
        age(&p, now, 13 * 3600);
        // Blocked, Testing, or nobody holding it: not a lease, left alone.
        let p = put_task(
            d.path(),
            "blocked",
            "0004",
            "priority: normal\nstage: building\nstatus: blocked\nclaimed-by: wt-d\nheartbeat: 2026-10-01 10:00\nwaiting_on: owner",
        );
        age(&p, now, 99 * 3600);
        let p = put_task(
            d.path(),
            "doing",
            "0005",
            "priority: normal\nstage: testing\nstatus: doing\nclaimed-by:\nheartbeat: 2026-10-01 10:00",
        );
        age(&p, now, 99 * 3600);
        let p = put_task(d.path(), "doing", "0006", &building("2026-10-01 10:00", ""));
        age(&p, now, 99 * 3600);
        let r = reclaim(d.path(), lease_minutes("").unwrap(), "capture", T, now).unwrap();
        assert!(r.failed.is_empty(), "{:?}", r.failed);
        assert_eq!(
            r.released,
            vec![
                Released {
                    id: "0001".into(),
                    holder: "wt-a".into(),
                    quiet: "13 h".into(),
                    path: "todo/0001-t.md".into()
                },
                Released {
                    id: "0003".into(),
                    holder: "wt-c".into(),
                    quiet: "13 h".into(),
                    path: "todo/0003-t.md".into()
                },
            ]
        );
        let t = task_of(d.path(), "0001");
        assert_eq!(
            (
                t.stage.as_str(),
                t.status.as_str(),
                t.folder.as_str(),
                t.claimed_by.as_str()
            ),
            ("ready", "todo", "todo", "")
        );
        assert!(file_of(d.path(), "0001").ends_with(
            "- 2026-10-12 14:05 capture: stage building → ready (reclaimed from wt-a (no heartbeat for 13 h))\n"
        ));
        let step = t.stage_history.last().unwrap();
        assert_eq!(
            (step.who.as_str(), step.from.as_str(), step.to.as_str()),
            ("capture", "building", "ready")
        );
        for id in ["0002", "0004", "0005", "0006"] {
            assert_ne!(task_of(d.path(), id).stage, "ready", "{id} left alone");
        }
        // The old holder's heartbeat after the reclaim is refused; nothing is left to release.
        assert!(heartbeat_task(d.path(), "0001", "wt-a", T)
            .unwrap_err()
            .contains("not Building"));
        let again = reclaim(d.path(), 720, "capture", T, now).unwrap();
        assert!(again.released.is_empty() && again.failed.is_empty());
        // A shorter lease takes 0002 (4 h quiet) too.
        let short = reclaim(d.path(), lease_minutes("30m").unwrap(), "capture", T, now).unwrap();
        assert_eq!(
            short
                .released
                .iter()
                .map(|r| (r.id.as_str(), r.quiet.as_str()))
                .collect::<Vec<_>>(),
            [("0002", "4 h")]
        );
    }

    #[test]
    fn release_of_one_task_needs_its_lease_to_have_run_out() {
        let d = queue();
        let now = crate::queue::now_secs();
        put_task(
            d.path(),
            "doing",
            "0001",
            "priority: normal\nstage: building\nstatus: doing\nclaimed-by: wt-a\nheartbeat: 2026-10-12 10:00\nwaiting_on:",
        );
        put_task(d.path(), "todo", "0002", &ready_fm("normal"));
        let e = reclaim_task(d.path(), "0001", 720, "owner", T, now).unwrap_err();
        assert_eq!(
            e,
            "0001's lease hasn't run out: wt-a was last heard from 4 h ago (the lease is 12 h)"
        );
        assert!(reclaim_task(d.path(), "0002", 720, "owner", T, now)
            .unwrap_err()
            .contains("0002 is in Ready, not Building"));
        // A day later the same release goes through, logged as the owner's.
        assert_eq!(
            reclaim_task(d.path(), "0001", 720, "owner", "2026-10-13 10:01", now).unwrap(),
            "todo/0001-t.md"
        );
        assert!(file_of(d.path(), "0001").ends_with(
            "owner: stage building → ready (reclaimed from wt-a (no heartbeat for 24 h))\n"
        ));
        assert!(reclaim_task(d.path(), "0001", 720, "owner", T, now)
            .unwrap_err()
            .contains("not Building"));
    }

    /// Run `f` for each of `who` on its own thread, all at once.
    fn at_once<R: Send + 'static>(
        whos: &[&'static str],
        f: impl Fn(&'static str) -> R + Send + Sync + 'static,
    ) -> Vec<R> {
        let go = std::sync::Arc::new(std::sync::Barrier::new(whos.len()));
        let f = std::sync::Arc::new(f);
        let runs: Vec<_> = whos
            .iter()
            .map(|&who| {
                let (go, f) = (go.clone(), f.clone());
                std::thread::spawn(move || {
                    go.wait();
                    f(who)
                })
            })
            .collect();
        runs.into_iter().map(|h| h.join().unwrap()).collect()
    }

    fn layouts() -> [(tempfile::TempDir, &'static str); 2] {
        [(queue(), "todo"), (tempfile::tempdir().unwrap(), "")]
    }

    #[test]
    fn two_claims_of_one_task_at_once_one_wins_in_both_layouts() {
        for _ in 0..10 {
            for (d, folder) in layouts() {
                put_task(d.path(), folder, "0001", &ready_fm("normal"));
                let root = d.path().to_path_buf();
                let results = at_once(&["wt-a", "wt-b"], move |who| {
                    claim_task(&root, "0001", who, T)
                });
                assert_eq!(
                    results.iter().filter(|r| r.is_ok()).count(),
                    1,
                    "{results:?}"
                );
                let t = task_of(d.path(), "0001");
                assert_eq!((t.stage.as_str(), t.stage_history.len()), ("building", 1));
            }
        }
    }

    #[test]
    fn one_holder_claiming_twice_at_once_gets_one_task_in_both_layouts() {
        for _ in 0..10 {
            for (d, folder) in layouts() {
                for id in ["0001", "0002", "0003"] {
                    put_task(d.path(), folder, id, &ready_fm("normal"));
                }
                let root = d.path().to_path_buf();
                // Two `next`s, and a `next` racing a `claim`, by the same holder.
                let results = at_once(&["next", "next", "claim"], move |how| match how {
                    "next" => next_task(&root, "wt-a", T).map(|c| c.map(|c| c.id)),
                    _ => claim_task(&root, "0003", "wt-a", T).map(|_| Some("0003".into())),
                });
                assert_eq!(
                    results.iter().filter(|r| r.is_ok()).count(),
                    1,
                    "{results:?}"
                );
                for e in results.iter().filter_map(|r| r.as_ref().err()) {
                    assert!(e.starts_with("you already hold "), "{e}");
                }
                let held = state(d.path())
                    .tasks
                    .into_iter()
                    .filter(|t| t.stage == "building")
                    .count();
                assert_eq!(held, 1);
            }
        }
    }

    #[test]
    fn a_heartbeat_racing_a_reclaim_lands_before_it_or_is_refused() {
        let now = crate::queue::now_secs();
        for _ in 0..20 {
            for (d, folder) in layouts() {
                let folder = if folder.is_empty() { "" } else { "doing" };
                put_task(
                    d.path(),
                    folder,
                    "0001",
                    "priority: normal\nstage: building\nstatus: doing\nclaimed-by: wt-a\nheartbeat: 2026-10-12 01:00\nwaiting_on:",
                );
                let root = d.path().to_path_buf();
                let results = at_once(&["heartbeat", "reclaim"], move |how| match how {
                    "heartbeat" => heartbeat_task(&root, "0001", "wt-a", T).map(|_| 0),
                    _ => reclaim(&root, 720, "capture", T, now).map(|r| r.released.len()),
                });
                let t = task_of(d.path(), "0001");
                match (&results[0], &results[1]) {
                    // The heartbeat first: the claim is live, so the reclaim leaves it.
                    (Ok(_), Ok(0)) => {
                        assert_eq!((t.stage.as_str(), t.heartbeat.as_str()), ("building", T))
                    }
                    // The reclaim first: the heartbeat finds it in Ready and is refused.
                    (Err(e), Ok(1)) => {
                        assert!(e.contains("not Building"), "{e}");
                        assert_eq!((t.stage.as_str(), t.claimed_by.as_str()), ("ready", ""));
                    }
                    other => panic!("{other:?}"),
                }
            }
        }
    }

    fn gate() -> PassOpts {
        PassOpts {
            gate: "green".into(),
            ..Default::default()
        }
    }

    #[test]
    fn every_way_into_building_keeps_one_claim_per_holder_but_the_owner() {
        let d = queue();
        for id in ["0001", "0002", "0003", "0004", "0005"] {
            put_task(d.path(), "todo", id, &ready_fm("normal"));
        }
        // pass --from ready is a claim like any other.
        pass_task(d.path(), "0001", "wt-a", T, &no_gate()).unwrap();
        let before = file_of(d.path(), "0002");
        assert_eq!(
            pass_task(d.path(), "0002", "wt-a", T, &no_gate()).unwrap_err(),
            "you already hold 0001; pass, return or block it first"
        );
        assert_eq!(file_of(d.path(), "0002"), before);
        // Block 0001, claim 0002: unblocking 0001 would give wt-a two, so it's refused, for anyone.
        block_task(d.path(), "0001", "owner", "wt-a", T).unwrap();
        claim_task(d.path(), "0002", "wt-a", T).unwrap();
        assert_eq!(
            unblock_task(d.path(), "0001", "wt-a", T).unwrap_err(),
            "you already hold 0002; pass, return or block it first"
        );
        assert_eq!(
            unblock_task(d.path(), "0001", "owner", T).unwrap_err(),
            "wt-a already holds 0002 in Building; one claim per holder, so they pass, return or block it first"
        );
        assert!(task_of(d.path(), "0001").blocked);
        // A return to the builder who now holds another task: refused too.
        claim_task(d.path(), "0003", "wt-b", T).unwrap();
        pass_task(d.path(), "0003", "wt-b", T, &gate()).unwrap();
        claim_task(d.path(), "0004", "wt-b", T).unwrap();
        assert!(return_task(d.path(), "0003", "AC1 fails", None, "qa", T)
            .unwrap_err()
            .starts_with("wt-b already holds 0004 in Building"));
        assert_eq!(task_of(d.path(), "0003").stage, "testing");
        // The owner may hold several.
        pass_task(d.path(), "0005", "owner", T, &no_gate()).unwrap();
        set_stage(d.path(), "0002", "ready", "owner", T).unwrap();
        claim_task(d.path(), "0002", "owner", T).unwrap();
        assert_eq!(
            next_task(d.path(), "owner", T).unwrap(),
            None,
            "nothing left, but not refused"
        );
        assert!(state(d.path()).problems.is_empty());
    }

    #[test]
    fn a_pass_from_ready_racing_next_for_one_holder_claims_once() {
        for _ in 0..10 {
            for (d, folder) in layouts() {
                for id in ["0001", "0002"] {
                    put_task(d.path(), folder, id, &ready_fm("normal"));
                }
                let root = d.path().to_path_buf();
                let results = at_once(&["pass", "next"], move |how| match how {
                    "pass" => pass_task(&root, "0002", "wt-a", T, &PassOpts::default()).map(|_| ()),
                    _ => next_task(&root, "wt-a", T).map(|_| ()),
                });
                assert_eq!(
                    results.iter().filter(|r| r.is_ok()).count(),
                    1,
                    "{results:?}"
                );
                let held = state(d.path())
                    .tasks
                    .into_iter()
                    .filter(|t| t.stage == "building")
                    .count();
                assert_eq!(held, 1);
            }
        }
    }

    #[test]
    fn unblocking_or_asking_restarts_the_lease() {
        let d = queue();
        let now = crate::queue::now_secs();
        put_task(d.path(), "todo", "0001", &ready_fm("normal"));
        claim_task(d.path(), "0001", "wt-a", "2026-10-11 09:00").unwrap();
        block_task(
            d.path(),
            "0001",
            "owner: which colour?",
            "wt-a",
            "2026-10-11 10:00",
        )
        .unwrap();
        unblock_task(d.path(), "0001", "owner", "2026-10-12 11:00").unwrap();
        assert_eq!(task_of(d.path(), "0001").heartbeat, "2026-10-12 11:00");
        let r = reclaim(d.path(), 720, "capture", "2026-10-12 11:00", now).unwrap();
        assert!(r.released.is_empty(), "{r:?}");
        // The holder asking a question is being heard from; someone else asking isn't.
        crate::queue::ask(
            d.path(),
            "0001",
            "Port?",
            "qa",
            "bbbb2222",
            "2026-10-12 12:00",
        )
        .unwrap();
        assert_eq!(task_of(d.path(), "0001").heartbeat, "2026-10-12 11:00");
        crate::queue::ask(
            d.path(),
            "0001",
            "Colour?",
            "wt-a",
            "aaaa1111",
            "2026-10-12 13:00",
        )
        .unwrap();
        assert_eq!(task_of(d.path(), "0001").heartbeat, "2026-10-12 13:00");
    }

    #[test]
    fn the_lease_runs_out_after_exactly_its_length() {
        let now = crate::queue::now_secs();
        let d = queue();
        let fm = "priority: normal\nstage: building\nstatus: doing\nclaimed-by: wt-a\nheartbeat: 2026-10-12 02:05\nwaiting_on:";
        put_task(d.path(), "doing", "0001", fm);
        // 720 minutes quiet: kept, by both; 721: released.
        let r = reclaim(d.path(), 720, "capture", "2026-10-12 14:05", now).unwrap();
        assert!(r.released.is_empty());
        assert!(
            reclaim_task(d.path(), "0001", 720, "owner", "2026-10-12 14:05", now)
                .unwrap_err()
                .contains("lease hasn't run out")
        );
        let r = reclaim(d.path(), 720, "capture", "2026-10-12 14:06", now).unwrap();
        assert_eq!(r.released.len(), 1);
        put_task(d.path(), "doing", "0002", fm);
        fs::remove_file(d.path().join("todo/0001-t.md")).unwrap();
        reclaim_task(d.path(), "0002", 720, "owner", "2026-10-12 14:06", now).unwrap();
        assert_eq!(task_of(d.path(), "0002").stage, "ready");
    }
}

#[cfg(test)]
mod edit_tests {
    use super::*;

    const T: &str = "2026-10-12 14:05";
    const WRITE_UP: &str =
        "# Edit me\n\nThe old description.\n\n## Acceptance criteria\n- [ ] AC1: one";
    const LOG: &str = "## Log\n- 2026-10-12 09:00 capture: created\n- 2026-10-12 09:30 capture: stage shaping → ready\n- 2026-10-12 09:40 wt-a: a note\n";
    const READY: &str = "stage: ready\nstatus: todo\nclaimed-by:\nheartbeat:\nwaiting_on:";

    fn queue() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for f in STATUS_FOLDERS {
            fs::create_dir(d.path().join(f)).unwrap();
        }
        fs::write(d.path().join("README.md"), "q").unwrap();
        d
    }

    /// Task 0001 in `folder`: front matter with `fm`, the write-up, the Log, then `tail`.
    fn put(d: &Path, folder: &str, fm: &str, write_up: &str, tail: &str) -> PathBuf {
        let p = d.join(folder).join("0001-t.md");
        let text = format!(
            "---\nid: 0001\ntype: task\npriority: normal\n{fm}\n---\n\n{write_up}\n\n{LOG}{tail}"
        );
        fs::write(&p, text).unwrap();
        p
    }
    fn text(p: &Path) -> String {
        fs::read_to_string(p).unwrap()
    }
    /// The file from the Log's heading on.
    fn from_log(text: &str) -> &str {
        &text[text.rfind("## Log").unwrap()..]
    }
    /// The front matter, both `---` lines included.
    fn front(text: &str) -> &str {
        &text[..text[4..].find("\n---\n").unwrap() + 9]
    }

    #[test]
    fn edits_only_the_write_up_and_logs_it() {
        let d = queue();
        let tail = "\n## Comments\n- kept as is\n";
        let p = put(d.path(), "todo", READY, WRITE_UP, tail);
        let before = text(&p);
        let opened = open_body(d.path(), "0001", "owner").unwrap();
        assert_eq!(opened, WRITE_UP);
        let new = "# Edit me\n\nThe new description, with a list:\n- one\n\t- indented\n\n## Questions\n- [ ] **Q1 (owner, session aaaa1111, 2026-10-12 10:00):** Which colour?\n\n## Acceptance criteria\n- [x] AC1: one\n- [ ] AC2: two";
        let path = edit_body(d.path(), "0001", new, &opened, "owner", T).unwrap();
        assert_eq!(path.as_deref(), Some("todo/0001-t.md"));
        let after = text(&p);
        assert_eq!(front(&after), front(&before), "front matter untouched");
        assert_eq!(
            from_log(&after),
            format!("## Log\n- 2026-10-12 09:00 capture: created\n- 2026-10-12 09:30 capture: stage shaping → ready\n- 2026-10-12 09:40 wt-a: a note\n- {T} owner: edited the write-up\n{tail}"),
            "every Log line kept, the edit logged, what follows the Log kept"
        );
        assert_eq!(
            after,
            format!("{}\n{new}\n\n{}", front(&before), from_log(&after))
        );
        assert_eq!(open_body(d.path(), "0001", "owner").unwrap(), new);
        let t = crate::queue::read_task(d.path(), "0001").unwrap();
        assert_eq!(t.stage_history.len(), 1);
        assert_eq!(crate::queue::criteria(&after), (2, 1));
        assert_eq!(t.questions.len(), 1);
        assert_eq!((t.stage.as_str(), t.title.as_str()), ("ready", "Edit me"));
    }

    #[test]
    fn refuses_when_the_write_up_changed_since_it_was_opened() {
        let d = queue();
        let p = put(d.path(), "todo", READY, WRITE_UP, "");
        let opened = open_body(d.path(), "0001", "owner").unwrap();
        // A note only adds to the Log: the write-up the editor opened is still the file's.
        add_note(d.path(), "0001", "checked", "wt-b", T).unwrap();
        edit_body(
            d.path(),
            "0001",
            "# Edit me\n\nFirst edit.",
            &opened,
            "owner",
            T,
        )
        .unwrap();
        // The write-up changed meanwhile: a second editor, still on the old text, is refused.
        let before = text(&p);
        let e = edit_body(
            d.path(),
            "0001",
            "# Edit me\n\nSecond.",
            &opened,
            "owner",
            T,
        )
        .unwrap_err();
        assert!(e.contains("changed since you opened it"), "{e}");
        assert_eq!(text(&p), before, "nothing written");
    }

    #[test]
    fn unchanged_text_writes_nothing() {
        let d = queue();
        let p = put(d.path(), "todo", READY, WRITE_UP, "");
        let before = text(&p);
        let opened = open_body(d.path(), "0001", "owner").unwrap();
        for same in [
            opened.clone(),
            format!("\n{opened}\n\n"),
            format!("{opened}\u{7}"),
        ] {
            assert_eq!(
                edit_body(d.path(), "0001", &same, &opened, "owner", T).unwrap(),
                None
            );
        }
        assert_eq!(text(&p), before);
    }

    #[test]
    fn cleans_the_text_and_refuses_what_the_tool_owns() {
        let d = queue();
        let p = put(d.path(), "todo", READY, WRITE_UP, "");
        let before = text(&p);
        let opened = open_body(d.path(), "0001", "owner").unwrap();
        let refused = |new: &str, want: &str| {
            let e = edit_body(d.path(), "0001", new, &opened, "owner", T).unwrap_err();
            assert!(e.contains(want), "{new:?}: {e}");
            assert_eq!(text(&p), before, "{new:?} wrote");
        };
        refused("# T\n\nfine\n## Log\n- planted", "the Log is the tool's");
        refused("# T\n\n## Log   \nx", "the Log is the tool's");
        refused("# T\r\n\r\n## Log\r\nx", "the Log is the tool's");
        refused(
            "# T\n\n- 2026-10-12 15:00 evil: stage ready → building (claimed)",
            "stage change",
        );
        refused("  \n\t\n", "empty");
        refused("Just a description, no title line", "# ");
        refused(
            &format!("# T\n\n{}", "x".repeat(MAX_BODY)),
            "longer than 50000",
        );
        // Prose that only mentions the Log or a stage is fine; CRLF becomes LF, control
        // characters go, tabs stay.
        let new = "# T\r\n\r\nSee ## Log below.\r\nMoved it ready → building by hand.\u{1b}[31m\r\n\tcode";
        edit_body(d.path(), "0001", new, &opened, "owner", T).unwrap();
        assert_eq!(
            open_body(d.path(), "0001", "owner").unwrap(),
            "# T\n\nSee ## Log below.\nMoved it ready → building by hand.[31m\n\tcode"
        );
    }

    #[test]
    fn unicode_counts_characters_not_bytes() {
        let d = queue();
        let p = put(d.path(), "todo", READY, WRITE_UP, "");
        let opened = open_body(d.path(), "0001", "owner").unwrap();
        // 50 000 characters exactly, most of them four bytes long.
        let head = "# کار 🚀 e\u{301}\n\n";
        let new = format!("{head}{}", "🚀".repeat(MAX_BODY - head.chars().count()));
        assert_eq!(new.chars().count(), MAX_BODY);
        edit_body(d.path(), "0001", &new, &opened, "owner", T).unwrap();
        assert_eq!(open_body(d.path(), "0001", "owner").unwrap(), new);
        assert_eq!(
            crate::queue::read_task(d.path(), "0001").unwrap().title,
            "کار 🚀 e\u{301}"
        );
        assert!(text(&p).ends_with(&format!("- {T} owner: edited the write-up\n")));
    }

    #[test]
    fn a_task_someone_else_holds_in_building_is_theirs_to_edit() {
        let d = queue();
        let held =
            "stage: building\nstatus: doing\nclaimed-by: wt-a\nheartbeat: 2026-10-12 09:00\nwaiting_on:";
        let p = put(d.path(), "doing", held, WRITE_UP, "");
        let before = text(&p);
        let e = open_body(d.path(), "0001", "owner").unwrap_err();
        assert!(e.contains("wt-a holds it; add a note instead"), "{e}");
        let e =
            edit_body(d.path(), "0001", "# Edit me\n\nMine.", WRITE_UP, "owner", T).unwrap_err();
        assert!(e.contains("wt-a holds it; add a note instead"), "{e}");
        assert_eq!(text(&p), before);
        // Blocked, it keeps its claim, and the write-up stays the holder's.
        let blocked =
            "stage: building\nstatus: blocked\nclaimed-by: wt-a\nheartbeat:\nwaiting_on: owner";
        put(d.path(), "doing", blocked, WRITE_UP, "");
        assert!(
            edit_body(d.path(), "0001", "# Edit me\n\nMine.", WRITE_UP, "owner", T)
                .unwrap_err()
                .contains("wt-a holds it")
        );
        // The holder edits it, and that keeps the claim alive.
        put(d.path(), "doing", held, WRITE_UP, "");
        edit_body(
            d.path(),
            "0001",
            "# Edit me\n\nThe holder's.",
            WRITE_UP,
            "wt-a",
            T,
        )
        .unwrap();
        let t = crate::queue::read_task(d.path(), "0001").unwrap();
        assert_eq!((t.heartbeat.as_str(), t.claimed_by.as_str()), (T, "wt-a"));
        assert!(text(&p).ends_with(&format!("- {T} wt-a: edited the write-up\n")));
        // The owner's own claim, and a task nobody holds, edit normally.
        let mine = "stage: building\nstatus: doing\nclaimed-by: owner\nheartbeat:\nwaiting_on:";
        put(d.path(), "doing", mine, WRITE_UP, "");
        edit_body(
            d.path(),
            "0001",
            "# Edit me\n\nThe owner's.",
            WRITE_UP,
            "owner",
            T,
        )
        .unwrap();
        fs::remove_file(&p).unwrap();
        put(d.path(), "todo", READY, WRITE_UP, "");
        edit_body(
            d.path(),
            "0001",
            "# Edit me\n\nAnyone's.",
            WRITE_UP,
            "owner",
            T,
        )
        .unwrap();
    }

    #[test]
    fn a_claim_left_on_a_task_past_building_does_not_hold_its_write_up() {
        // Done by hand, with the old holder still named and no stage line in the Log.
        let d = queue();
        let stale =
            "stage: done\nstatus: done\nclaimed-by: wt-a\nheartbeat: 2026-10-01 09:00\nwaiting_on:";
        let p = put(d.path(), "done", stale, WRITE_UP, "");
        assert_eq!(open_body(d.path(), "0001", "owner").unwrap(), WRITE_UP);
        let new = "# Edit me\n\nCorrected after the fact.";
        edit_body(d.path(), "0001", new, WRITE_UP, "owner", T).unwrap();
        assert_eq!(open_body(d.path(), "0001", "owner").unwrap(), new);
        let t = crate::queue::read_task(d.path(), "0001").unwrap();
        assert_eq!(
            (
                t.stage.as_str(),
                t.claimed_by.as_str(),
                t.heartbeat.as_str()
            ),
            ("done", "wt-a", "2026-10-01 09:00"),
            "only the write-up changed"
        );
        assert!(text(&p).ends_with(&format!("- {T} owner: edited the write-up\n")));
    }

    #[test]
    fn a_task_without_a_log_gets_one_and_the_flat_layout_works() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("0001-t.md");
        fs::write(
            &p,
            "---\nid: 0001\nstatus: todo\n---\n\n# Bare\n\nNo log yet.\n",
        )
        .unwrap();
        let opened = open_body(d.path(), "0001", "owner").unwrap();
        assert_eq!(opened, "# Bare\n\nNo log yet.");
        let path = edit_body(
            d.path(),
            "0001",
            "# Bare\n\nNow logged.",
            &opened,
            "owner",
            T,
        )
        .unwrap();
        assert_eq!(path.as_deref(), Some("0001-t.md"));
        assert_eq!(
            text(&p),
            format!("---\nid: 0001\nstatus: todo\n---\n\n# Bare\n\nNow logged.\n\n## Log\n- {T} owner: edited the write-up\n")
        );
    }

    #[test]
    fn crlf_task_files_keep_their_log() {
        let d = queue();
        let p = d.path().join("todo/0001-t.md");
        let log = "## Log\r\n- 2026-10-12 09:00 capture: created\r\n";
        fs::write(
            &p,
            format!(
                "---\nid: 0001\n{READY}\n---\n\r\n# Windows\r\n\r\nLines end in CRLF.\r\n\r\n{log}"
            ),
        )
        .unwrap();
        let opened = open_body(d.path(), "0001", "owner").unwrap();
        assert_eq!(opened, "# Windows\r\n\r\nLines end in CRLF.");
        edit_body(
            d.path(),
            "0001",
            "# Windows\n\nNow LF.",
            &opened,
            "owner",
            T,
        )
        .unwrap();
        let after = text(&p);
        assert!(
            after.ends_with(&format!(
                "---\n\n# Windows\n\nNow LF.\n\n{log}- {T} owner: edited the write-up\n"
            )),
            "{after:?}"
        );
    }

    #[test]
    fn the_write_up_ends_at_the_last_log() {
        // A Log quoted in the description is text; the tool's Log is the last one.
        let quoted = "# Q\n\nThe template:\n\n## Log\n- 2026-01-01 00:00 x: stage ready → done";
        let text = format!("---\nid: 0001\n---\n\n{quoted}\n\n{LOG}");
        assert_eq!(editable_body(&text).unwrap(), quoted);
        assert!(editable_body("no front matter").is_err());
    }

    #[test]
    fn two_saves_from_one_opened_text_at_once_one_wins() {
        let d = queue();
        let p = put(d.path(), "todo", READY, WRITE_UP, "");
        let saves: Vec<_> = (0..2)
            .map(|i| {
                let root = d.path().to_path_buf();
                std::thread::spawn(move || {
                    let new = format!("# Edit me\n\nSave {i}.");
                    edit_body(&root, "0001", &new, WRITE_UP, "owner", T)
                })
            })
            .collect();
        let results: Vec<_> = saves.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(
            results.iter().filter(|r| r.is_ok()).count(),
            1,
            "{results:?}"
        );
        assert!(results.iter().any(|r| r
            .as_ref()
            .is_err_and(|e| e.contains("changed since you opened it"))));
        assert_eq!(text(&p).matches("edited the write-up").count(), 1);
    }
}
