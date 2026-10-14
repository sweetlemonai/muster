//! Changing tasks: the one implementation of Muster's rules for creating and managing tasks,
//! used by the app's window and by the `muster` command line alike. Every change holds the
//! queue's lock, writes atomically, stamps the time itself and logs who did it.

use crate::queue::{
    self, actor, append_log, find_task, front_matter, mirror_status, with_lock, write_atomic,
    STATUS_FOLDERS,
};
pub use crate::queue::{stage_label, stage_role, StageStep, STAGES};
use crate::session::{is_owner, same_holder};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// A task's type (decision 0003, task 0017). `new` and `set` take these; old files may still say
/// `task` (read as code) or `idea` (kept as it is: an idea belongs in the backlog now).
pub const KINDS: [&str; 8] = [
    "code",
    "bug",
    "chore",
    "tech-debt",
    "spike",
    "design",
    "docs",
    "research",
];

/// A type as `new` and `set` take it: one of KINDS in any case, `task` for code, `default` when
/// empty. `idea` is refused: an idea goes in the backlog as a note.
fn clean_kind(value: &str, default: &'static str) -> Result<&'static str, String> {
    let v = value.trim();
    if v.eq_ignore_ascii_case("task") {
        return Ok("code");
    }
    if v.eq_ignore_ascii_case("idea") {
        return Err(
            "idea isn't a type any more: put an idea in the backlog with muster add, and shape it into a task from there".into(),
        );
    }
    pick(v, &KINDS, default, "type")
}

/// A type as read from a task file: lowercase, with the old `task` read as `code`. Anything else
/// (`idea` in older files) is kept as it is.
pub fn read_kind(raw: &str) -> String {
    let k = raw.trim().to_lowercase();
    if k == "task" {
        "code".into()
    } else {
        k
    }
}
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
    /// Test time: the QA agent's and the owner's check, the same format as `est_time`.
    pub est_test_time: String,
    pub est_tokens: String,
    /// goal, feature or task (empty is a task).
    pub level: String,
    /// The id of the feature or goal it sits under, or empty.
    pub parent: String,
    /// A goal's date, `YYYY-MM-DD`, or empty.
    pub due: String,
    /// Who does it (0022): `human` (the owner) or `ai`; empty is ai.
    pub for_who: String,
    /// The folders and globs it works in (0023), a comma list; empty is none said.
    pub touches: String,
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

/// A task's estimates in one line, as the card shows them: "agent 2h · test 30m · 400k", with
/// only the parts it has ("" for none).
pub fn estimate_text(t: &queue::Task) -> String {
    let agent = (!t.est_time.is_empty()).then(|| format!("agent {}", t.est_time));
    let test = (!t.est_test_time.is_empty()).then(|| format!("test {}", t.est_test_time));
    let tokens = (!t.est_tokens.is_empty()).then(|| t.est_tokens.clone());
    [agent, test, tokens]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ")
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

/// Is this queue laid out with status folders (v2), or flat (or `.muster/`, 0005)?
pub(crate) fn has_status_folders(root: &Path) -> bool {
    crate::layout::has_status_folders(root)
}

fn folder_for(root: &Path, status: &str) -> PathBuf {
    crate::layout::task_dir(root, status)
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
    est_test_time: String,
    est_tokens: String,
    /// Level, parent and due (0016): a task on its own unless `create_task` sets them, after
    /// `clean_link`, and checks the parent under the lock.
    level: &'static str,
    parent: String,
    due: String,
    /// Who does it (0022): ai unless `create_task` makes it the owner's.
    for_who: &'static str,
    /// `touches:` as written (0023), or "" for none.
    touches: String,
}

/// Clean a new task's fields: refused when the title is empty, a choice isn't one, or a text is
/// too long.
pub(crate) fn clean_new(new: &NewTask) -> Result<CleanTask, String> {
    let sp = |v: String| if v.is_empty() { v } else { format!(" {v}") };
    let title = one_line(&new.title, MAX_TITLE, "the title")?;
    let body = body_text(&new.body)?;
    no_comments_heading(&body, "the description")?;
    Ok(CleanTask {
        title,
        kind: clean_kind(&new.kind, "code")?,
        priority: pick(&new.priority, &PRIORITIES, "normal", "priority")?,
        body,
        tags: sp(clean_tags(&new.tags)?.join(", ")),
        est_time: sp(clean_time(&new.est_time)?),
        est_test_time: sp(clean_time(&new.est_test_time)?),
        est_tokens: sp(clean_tokens(&new.est_tokens)?),
        level: "task",
        parent: String::new(),
        due: String::new(),
        for_who: "ai",
        touches: clean_touches(&new.touches)?,
    })
}

/// The most entries `touches:` takes, and the longest one.
const MAX_TOUCHES: usize = 20;
const MAX_TOUCH: usize = 200;

/// `touches:` (0023): folders and globs in the repo, comma-separated (`app/src/TaskPanel*,
/// docs/PROTOCOL.md`), written `a, b`; empty clears it. Each is relative to the repo (no leading
/// `/`, no `..`), a leading `./` is dropped, and none holds a control character or `#`.
pub fn clean_touches(value: &str) -> Result<String, String> {
    let mut out: Vec<String> = Vec::new();
    for raw in value.split(',') {
        let t = raw.trim();
        let t = t.strip_prefix("./").unwrap_or(t);
        if t.is_empty() {
            continue;
        }
        let shown: String = t.chars().take(40).collect();
        if t.chars().any(|c| c.is_control() || c == '#') {
            return Err(format!(
                "touches takes folders and globs without control characters or #, got {shown:?}"
            ));
        }
        if t.starts_with('/') || t.split('/').any(|part| part == "..") {
            return Err(format!(
                "touches takes folders and globs inside the repo (no leading / or ..), got {shown:?}"
            ));
        }
        if t.chars().count() > MAX_TOUCH {
            return Err(format!(
                "a touches entry is longer than {MAX_TOUCH} characters: {shown:?}…"
            ));
        }
        if !out.iter().any(|o| o == t) {
            out.push(t.to_string());
        }
    }
    if out.len() > MAX_TOUCHES {
        return Err(format!("touches takes at most {MAX_TOUCHES} entries"));
    }
    Ok(out.join(", "))
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
    let (est_time, est_test_time, est_tokens) = (&t.est_time, &t.est_test_time, &t.est_tokens);
    // The status mirror only where the layout keeps one (not in `.muster/`, 0005).
    let status = if crate::layout::keeps_status(root) {
        format!("status: {status}\n")
    } else {
        String::new()
    };
    let mut text = format!(
        "---\nid: {id}\ntype: {kind}\npriority: {priority}\ntags:{tags}\nest-time:{est_time}\nest-test-time:{est_test_time}\nest-tokens:{est_tokens}\nstage: {stage}\n{status}captured: {date}\nclaimed-by:\nheartbeat:\nwaiting_on:\ncommit:\n---\n\n# {title}\n\n{details}## Log\n- {stamp} {who}: created\n"
    );
    // Level, parent, due and for only when given: a file without them is an agent's task on its
    // own.
    for (key, value) in [
        ("level", t.level),
        ("parent", t.parent.as_str()),
        ("due", t.due.as_str()),
        ("for", t.for_who),
        ("touches", t.touches.as_str()),
    ] {
        let default = (key == "level" && value == "task") || (key == "for" && value == "ai");
        if !value.is_empty() && !default {
            text = set_front(&text, key, value)?;
        }
    }
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
/// A human task (`for: human`, 0022) is only the owner's to make: an agent proposes one in the
/// backlog, and the owner files it.
pub fn create_task(root: &Path, new: &NewTask, who: &str, stamp: &str) -> Result<Created, String> {
    let who = actor(who)?;
    let mut t = clean_new(new)?;
    (t.level, t.parent, t.due) = clean_link(new)?;
    t.for_who = pick(new.for_who.trim(), &queue::FOR_WHO, "ai", "for")?;
    if t.for_who == "human" {
        if !is_owner(&who) {
            return Err(format!(
                "only the owner makes a human task; propose it in the backlog instead: muster add \"<what the owner needs to do>\"{AS_OWNER}"
            ));
        }
        if t.level != "task" {
            return Err(only_a_task_is_human(t.level));
        }
    }
    with_lock(root, || {
        if !t.parent.is_empty() {
            parent_fits(t.level, &t.parent, &tasks_now(root))?;
        }
        let id = next_id(root)?;
        create_locked(root, &id, &t, "ready", &who, stamp, &[])
    })
}

/// A new task's level, parent and due date, cleaned and checked on their own (the parent's
/// level is checked under the lock): a goal has no parent, and only a goal has a due date.
fn clean_link(new: &NewTask) -> Result<(&'static str, String, String), String> {
    let level = pick(new.level.trim(), &queue::LEVELS, "task", "level")?;
    let parent = clean_parent(&new.parent)?;
    let due = clean_due(&new.due)?;
    if level == "goal" && !parent.is_empty() {
        return Err(parent_rule("goal").into());
    }
    if level != "goal" && !due.is_empty() {
        return Err("only a goal has a due date".into());
    }
    Ok((level, parent, due))
}

/// The refusal when someone who isn't the owner takes, starts or finishes the owner's own task,
/// with the fix for the owner acting under another label (a terminal's git name).
fn owners_to_do(id: &str) -> String {
    format!("{id} is the owner's to do; agents never take a human task{AS_OWNER}")
}
const AS_OWNER: &str = " (if you're the owner, add --as owner)";

/// Does `who` hold a claim made as `claimed_by`? The same holder; and for the owner's own task
/// (`human`), any owner session, since every session of the owner is the owner.
fn holds(claimed_by: &str, who: &str, human: bool) -> bool {
    same_holder(claimed_by, who) || (human && is_owner(claimed_by) && is_owner(who))
}

/// Why a goal or feature can't be the owner's to do: its tasks are.
fn only_a_task_is_human(level: &str) -> String {
    format!("only a task can be the owner's to do (for: human), not a {level}; its tasks are")
}

/// `muster set <id> for human|ai` (0022), under the lock: only a task, and not while someone is
/// on it (an agent's claim in Building, the owner doing their own, or Testing and Your check), so
/// a claim never changes hands by a field. Logged. Made human in Backlog or Shaping, it moves to
/// Ready (To do).
fn set_for(root: &Path, id: &str, value: &str, who: &str, stamp: &str) -> Result<String, String> {
    let who = actor(who)?;
    let val = pick(value.trim(), &queue::FOR_WHO, "", "for")?;
    if val.is_empty() {
        return Err("for is empty".into());
    }
    let line = free_text_line(format!("{stamp} {who}: for set to {val}"), "the for")?;
    with_lock(root, || {
        let tasks = tasks_now(root);
        find_task(root, id)?;
        let me = tasks
            .iter()
            .find(|t| t.id == id)
            .ok_or_else(|| format!("task {id} has no front matter"))?;
        if val == "human" && me.level != "task" {
            return Err(only_a_task_is_human(&me.level));
        }
        let at_work = matches!(me.stage.as_str(), "building" | "testing" | "your-check");
        let held = me.holder();
        if at_work || (me.stage == "ready" && !held.is_empty()) {
            let by = if held.is_empty() {
                String::new()
            } else {
                format!(" (held by {held})")
            };
            return Err(format!(
                "{id} is in {}{by}; return or pass it first",
                stage_label(&me.stage)
            ));
        }
        let path = edit_locked(root, id, |_, text| {
            Ok((append_log(&set_front(&text, "for", val)?, &line), None))
        })?;
        // The owner's own task waits in To do, not in Backlog or Shaping: it moves there, logged
        // as a stage change like any other (review C1).
        if val == "human" && matches!(me.stage.as_str(), "backlog" | "shaping") {
            return change_locked(root, id, &who, stamp, |t| {
                Ok(Change {
                    stage: "ready".into(),
                    blocked: t.blocked,
                    waiting_on: t.waiting_on.clone(),
                    blocked_kind: t.blocked_kind.clone(),
                    depends: None,
                    claimed_by: t.claimed_by.clone(),
                    commit: None,
                    log: vec![stage_line(stamp, &who, &t.stage, "ready", " (yours to do)")],
                })
            });
        }
        Ok(path)
    })
}

/// A parent: a task id, or empty for none.
fn clean_parent(input: &str) -> Result<String, String> {
    let p = input.trim();
    if p.is_empty() || is_id(p) {
        Ok(p.to_string())
    } else {
        Err(format!(
            "parent must be a task id (four digits), got {:?}",
            p.chars().take(40).collect::<String>()
        ))
    }
}

/// A due date: `YYYY-MM-DD` that exists, or empty for none.
fn clean_due(input: &str) -> Result<String, String> {
    let d = input.trim();
    if d.is_empty() || queue::is_date(d) {
        Ok(d.to_string())
    } else {
        Err(format!(
            "due must be a date (YYYY-MM-DD), got {:?}",
            d.chars().take(40).collect::<String>()
        ))
    }
}

/// Where a level may sit (decision 0003: Goal → Feature → Task).
fn parent_rule(level: &str) -> &'static str {
    match level {
        "goal" => "a goal has no parent",
        "feature" => "a feature's parent is a goal",
        _ => "a task's parent is a feature or a goal",
    }
}

/// May an item of `level` sit under one of `parent_level`?
fn fits_under(level: &str, parent_level: &str) -> bool {
    matches!(
        (level, parent_level),
        ("feature", "goal") | ("task", "goal" | "feature")
    )
}

/// Refused unless task `parent` exists in `tasks` and an item of `level` may sit under it.
fn parent_fits(level: &str, parent: &str, tasks: &[queue::Task]) -> Result<(), String> {
    let p = tasks
        .iter()
        .find(|t| t.id == parent)
        .ok_or_else(|| format!("no task {parent}"))?;
    if fits_under(level, &p.level) {
        Ok(())
    } else if level == "goal" {
        Err(parent_rule(level).into())
    } else {
        Err(format!("{}; {parent} is a {}", parent_rule(level), p.level))
    }
}

/// `muster set <id> level|parent|due <value>`, by the rules `create_task` keeps, under the lock
/// and against every task as it is now: the parent exists and may hold it, and it isn't already
/// under this one (a loop); a new level still fits its parent and its children; only a goal has a
/// due date. An empty parent or due clears it. Logged.
fn set_link(
    root: &Path,
    id: &str,
    field: &str,
    value: &str,
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    let who = actor(who)?;
    let (key, val) = match field {
        "level" => {
            let level = pick(value.trim(), &queue::LEVELS, "", "level")?;
            if level.is_empty() {
                return Err("level is empty".into());
            }
            ("level", level.to_string())
        }
        "parent" => ("parent", clean_parent(value)?),
        _ => ("due", clean_due(value)?),
    };
    let line = if val.is_empty() {
        format!("{stamp} {who}: {key} cleared")
    } else if key == "parent" {
        // By its key in the log (0033); its id in the front matter.
        format!(
            "{stamp} {who}: parent set to {}",
            keys_of(root, std::slice::from_ref(&val))
        )
    } else {
        format!("{stamp} {who}: {key} set to {val}")
    };
    let line = free_text_line(line, &format!("the {key}"))?;
    with_lock(root, || {
        find_task(root, id)?;
        let tasks = tasks_now(root);
        let me = tasks
            .iter()
            .find(|t| t.id == id)
            .ok_or_else(|| format!("task {id} has no front matter"))?;
        match key {
            "level" => {
                // Only tasks are built: one being built or tested, or held, stays a task.
                let at_work = matches!(me.stage.as_str(), "building" | "testing");
                if val != "task" && (at_work || !me.claimed_by.is_empty()) {
                    let held = if me.claimed_by.is_empty() {
                        String::new()
                    } else {
                        format!(" (held by {})", me.claimed_by)
                    };
                    return Err(format!(
                        "{id} is in {}{held}; return or pass it first",
                        stage_label(&me.stage)
                    ));
                }
                if val != "goal" && !me.due.is_empty() {
                    return Err(format!("clear {id}'s due date first; only a goal has one"));
                }
                if val != "task" && me.is_human() {
                    return Err(format!(
                        "{id} is the owner's to do (for: human), so it stays a task; set it for ai first"
                    ));
                }
                let under = tasks.iter().find(|t| t.id == me.parent);
                let parent_ok = match under {
                    _ if me.parent.is_empty() => true,
                    _ if val == "goal" => false,
                    Some(p) => fits_under(&val, &p.level),
                    None => true,
                };
                if !parent_ok {
                    return Err(format!(
                        "can't make {id} a {val}: it's under {}, and {}",
                        me.parent,
                        parent_rule(&val)
                    ));
                }
                let child = tasks
                    .iter()
                    .find(|c| c.parent == id && c.id != id && !fits_under(&c.level, &val));
                if let Some(c) = child {
                    return Err(format!(
                        "can't make {id} a {val}: {} ({}) is under it, and {}",
                        c.id,
                        c.level,
                        parent_rule(&c.level)
                    ));
                }
            }
            "parent" if !val.is_empty() => {
                if val == id {
                    return Err(format!("{id} can't be its own parent"));
                }
                parent_fits(&me.level, &val, &tasks)?;
                // Up from the new parent: reaching this task again would close a loop.
                let mut at = val.clone();
                let mut seen = std::collections::HashSet::new();
                while let Some(t) = tasks.iter().find(|t| t.id == at) {
                    if t.parent == id {
                        return Err(format!(
                            "{id} under {val} would make a loop: {} is under {id}",
                            t.id
                        ));
                    }
                    if t.parent.is_empty() || !seen.insert(t.id.clone()) {
                        break;
                    }
                    at = t.parent.clone();
                }
            }
            "due" if !val.is_empty() && me.level != "goal" => {
                return Err(format!(
                    "only a goal has a due date; {id} is a {}",
                    me.level
                ));
            }
            _ => {}
        }
        edit_locked(root, id, |_, text| {
            Ok((append_log(&set_front(&text, key, &val)?, &line), None))
        })
    })
}

/// Replace a front-matter field's value (its last line, which readers read when it's written
/// twice; adding it before the closing `---` if missing). `clear_front` drops every line of it.
pub(crate) fn set_front(text: &str, key: &str, value: &str) -> Result<String, String> {
    let rest = text
        .strip_prefix("---\n")
        .ok_or("the task has no front matter")?;
    let end = rest
        .find("\n---\n")
        .ok_or("the task's front matter isn't closed")?;
    let mut lines: Vec<String> = rest[..end].lines().map(String::from).collect();
    let prefix = format!("{key}:");
    // The last line with the key: the one `queue::front_matter` reads when a key is written twice.
    match lines.iter_mut().rev().find(|l| l.starts_with(&prefix)) {
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

/// `depends` for `set`: task ids in this queue, never the task itself, and never closing a loop.
/// Written as `[0014, 0016]`; empty clears it. The caller holds the queue's lock.
fn clean_depends(root: &Path, id: &str, value: &str) -> Result<String, String> {
    let (ids, bad) = queue::parse_depends(value);
    if let Some(b) = bad.first() {
        return Err(format!("depends takes task ids like 0014, got {b:?}"));
    }
    if ids.iter().any(|d| d == id) {
        return Err(format!("{id} can't depend on itself"));
    }
    if ids.len() > 50 {
        return Err("depends takes at most 50 tasks".into());
    }
    if ids.is_empty() {
        return Ok(String::new());
    }
    let tasks = tasks_now(root);
    let known: std::collections::HashSet<&str> = tasks.iter().map(|t| t.id.as_str()).collect();
    if let Some(d) = ids.iter().find(|d| !known.contains(d.as_str())) {
        return Err(format!("there's no task {d} in this queue"));
    }
    // Would the new list close a loop through this task?
    let mut edges: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
    for t in &tasks {
        let deps: Vec<&str> = if t.id == id {
            ids.iter().map(String::as_str).collect()
        } else {
            t.depends
                .iter()
                .map(String::as_str)
                .filter(|d| *d != t.id)
                .collect()
        };
        edges.entry(t.id.as_str()).or_default().extend(deps);
    }
    if let Some(l) = queue::dependency_loops(&edges)
        .into_iter()
        .find(|l| l.iter().any(|x| x == id))
    {
        return Err(format!(
            "that would make a loop: {} would wait on each other",
            l.join(", ")
        ));
    }
    Ok(format!("[{}]", ids.join(", ")))
}

/// Set priority, type, title, tags, estimates or depends.
pub fn set_field(
    root: &Path,
    id: &str,
    field: &str,
    value: &str,
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    if matches!(field, "level" | "parent" | "due") {
        return set_link(root, id, field, value, who, stamp);
    }
    if field == "for" {
        return set_for(root, id, value, who, stamp);
    }
    let who = actor(who)?;
    let (key, val) = match field {
        "priority" => (
            "priority",
            pick(value, &PRIORITIES, "", "priority")?.to_string(),
        ),
        "type" | "kind" => ("type", clean_kind(value, "")?.to_string()),
        "title" => ("title", one_line(value, MAX_TITLE, "the title")?),
        "tags" => ("tags", clean_tags(&[value.to_string()])?.join(", ")),
        "time" | "est-time" => ("est-time", clean_time(value)?),
        "test-time" | "est-test-time" => ("est-test-time", clean_time(value)?),
        "tokens" | "est-tokens" => ("est-tokens", clean_tokens(value)?),
        "touches" => ("touches", clean_touches(value)?),
        // Checked against the other tasks and written under one lock, so two sets at once can't
        // close a loop between them.
        "depends" => {
            return with_lock(root, || {
                let val = clean_depends(root, id, value)?;
                // By their keys in the log; `[0014, 0016]` in the front matter.
                let shown = match queue::parse_depends(&val).0 {
                    ids if ids.is_empty() => val.clone(),
                    ids => format!("[{}]", keys_of(root, &ids)),
                };
                let line = free_text_line(
                    format!("{stamp} {who}: depends set to {shown}"),
                    "the depends",
                )?;
                // Blocked only by tasks before, and nothing open left in the new list: say it's
                // free, so its blocked span on the timeline ends here (0024).
                let tasks = tasks_now(root);
                let freed = tasks.iter().find(|t| t.id == id).is_some_and(|t| {
                    let under_way = crate::block::started(t);
                    let now_open = queue::parse_depends(&val).0.iter().any(|d| {
                        *d != t.id
                            && tasks
                                .iter()
                                .any(|o| o.id == *d && crate::block::holds(o, under_way))
                    });
                    t.blocked_kind == "tasks" && !t.blocked && !now_open
                });
                edit_locked(root, id, |_, text| {
                    let mut new = append_log(&set_front(&text, "depends", &val)?, &line);
                    if freed {
                        new = append_log(&new, &format!("{stamp} {who}: unblocked: no open depends left"));
                    }
                    Ok((new, None))
                })
            })
        }
        _ => {
            return Err(format!(
                "can't set {field:?}: use priority, type, title, tags, time, test-time, tokens, touches, level, parent, due, depends or for"
            ))
        }
    };
    if val.is_empty()
        && !matches!(
            key,
            "tags" | "est-time" | "est-test-time" | "est-tokens" | "touches"
        )
    {
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
        not_paused(id, t)?;
        if t.stage == stage && !t.blocked {
            return Err(format!("{id} is already in {}", stage_label(stage)));
        }
        merge_rule(id, root, t, stage)?;
        // Like `set_stage`, the move doesn't let a builder skip its tester (0047).
        let note = tester_rule(id, t, &who, stage)?;
        let mut log = Vec::new();
        if t.blocked {
            log.push(unblocked_line(stamp, &who, &was_waiting(t)));
        }
        if t.stage != stage {
            log.push(stage_line(
                stamp,
                &who,
                &t.stage,
                stage,
                &with_note(" (set)".into(), note),
            ));
        }
        Ok(Change {
            stage: stage.to_string(),
            blocked: false,
            waiting_on: String::new(),
            blocked_kind: String::new(),
            depends: None,
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
    /// `blocked-kind:` as the file has it (0024), "" when it has none.
    blocked_kind: String,
    /// The tasks it depends on (`depends:`).
    depends: Vec<String>,
    claimed_by: String,
    /// Who holds it in Testing or Your check (`held-by:`).
    held_by: String,
    /// Acceptance criteria: (items, ticked).
    criteria: (usize, usize),
    /// Unticked criteria only the owner verifies (`— verify: owner`): they don't hold Testing
    /// back, since Your check is where the owner verifies them (0028).
    owner_open: usize,
    /// Open vision conflicts (`vision: <n> open`, 0029) and their ids: they hold Shaping back.
    vision_open: usize,
    vision_ids: Vec<String>,
    /// Review findings its builder hasn't answered (0027, `review: … <n> open …`) and their ids:
    /// they hold Building back.
    review_open: usize,
    review_ids: Vec<String>,
    /// Who passed it Ready → Building most recently (the builder), if anyone.
    builder: Option<String>,
    /// Who passed it Building → Testing most recently (Testing's holder), if anyone.
    tester: Option<String>,
    /// Who passed it Building → Testing with the gate's line most recently (a builder, 0047),
    /// unlike an override into Testing.
    gated_by: Option<String>,
    /// The claim's lease: the holder's last heartbeat stamp, and the file's modified time.
    heartbeat: String,
    modified: u64,
    /// goal, feature or task: only tasks go through Building and Testing.
    level: String,
    /// The owner's own task (`for: human`): To do → Doing → Done, never an agent's.
    human: bool,
    /// The branch `muster start` made for it (0023), or "": passing such a task from Testing is
    /// the merge's (`parallel::merge`), not a plain pass.
    branch: String,
    /// Its worktree (0023), "" when it has none: where the branch is looked up.
    worktree: String,
    /// Whom it holds back as a dependency (0042, `block::holds`): tasks not started yet, and
    /// tasks under way.
    kept_back: (bool, bool),
    /// When the owner paused it (0045, `paused:`), "" when it isn't: nobody passes it then.
    paused: String,
}

/// What a stage change writes.
struct Change {
    stage: String,
    blocked: bool,
    waiting_on: String,
    /// `blocked-kind:` (0024): one of `block::TYPED` for a new block, "" to clear it, else as the
    /// file had it.
    blocked_kind: String,
    /// A new `depends:` (a block on tasks), or None to leave it.
    depends: Option<String>,
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

/// Would this log line read as a holder's check-in (`queue::checkin_line`)?
fn reads_as_checkin(line: &str) -> bool {
    queue::checkin_line(&format!("- {line}")).is_some()
}

/// A log line carrying free text (a note, a field's value, what it waits on). Only the stage
/// writer may log stage history, and only `checkin` a check-in, so a line the log's reader would
/// take for either (`queue::stage_line`, `queue::checkin_line`, whatever part of it the who and
/// the text make up) is refused. Prose that only mentions a stage or a check-in doesn't parse as
/// one, so it's fine.
pub(crate) fn free_text_line(line: String, what: &str) -> Result<String, String> {
    if reads_as_stage(&line) {
        return Err(format!(
            "{what} would read as a stage change in the log (\"<who>: stage <from> → <to>\"); only muster pass, return and stage write those"
        ));
    }
    if reads_as_checkin(&line) {
        return Err(format!(
            "{what} would read as a check-in in the log (\"<who>: check-in …\"); only muster checkin writes those"
        ));
    }
    if crate::ping::reads_as_ask(&line) {
        return Err(format!(
            "{what} would read as a check-in request in the log (\"<who>: asked for a check-in …\"); only muster ping writes those"
        ));
    }
    if crate::pause::reads_as_pause(&line) {
        return Err(format!(
            "{what} would read as a pause or resume in the log (\"<who>: paused …\"); only muster pause and resume write those"
        ));
    }
    Ok(line)
}

/// What a blocked task was waiting on, for the unblock's log line: `you: pick a colour` with a
/// kind (0024), `waiting_on` as written without one.
fn was_waiting(t: &Current) -> String {
    if t.blocked_kind.is_empty() || t.waiting_on.is_empty() {
        t.waiting_on.clone()
    } else {
        let kind = t.blocked_kind.to_lowercase();
        format!(
            "{kind}: {}",
            crate::block::guessed_detail(&t.waiting_on, &kind)
        )
    }
}

fn unblocked_line(stamp: &str, who: &str, was: &str) -> String {
    let line = format!("{stamp} {who}: unblocked (was waiting on {was})");
    // `was` comes from the file and may be hand-written; never let it log a stage change or a
    // check-in.
    if was.is_empty() || reads_as_stage(&line) || reads_as_checkin(&line) {
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

/// The task in `text` (its file at `path`) as a stage change sees it.
fn current(root: &Path, path: &Path, text: &str) -> Result<Current, String> {
    let folder = queue::folder_of(root, path);
    let t = queue::parse_task("", root, &folder, path, text, 0)
        .ok_or("the task has no front matter")?;
    let (fields, body) = front_matter(text).ok_or("the task has no front matter")?;
    let builder = (!t.built_by.is_empty()).then(|| t.built_by.clone());
    let gated_by = (!t.gated_by.is_empty()).then(|| t.gated_by.clone());
    let tester = queue::stage_lines(body)
        .into_iter()
        .rev()
        .find(|(s, _)| s.from == "building" && s.to == "testing")
        .map(|(s, _)| s.who);
    let modified = fs::metadata(path)
        .map(|m| queue::mtime_secs(&m))
        .map_err(|e| e.to_string())?;
    let items = queue::criteria_items(body);
    let human = t.is_human();
    let kept_back = (
        crate::block::holds(&t, false),
        crate::block::holds(&t, true),
    );
    Ok(Current {
        kept_back,
        human,
        criteria: (items.len(), items.iter().filter(|c| c.ticked).count()),
        owner_open: items.iter().filter(|c| c.owner && !c.ticked).count(),
        vision_open: t.vision_open.unwrap_or(0),
        vision_ids: t.vision_ids,
        review_open: t.review_open,
        review_ids: crate::review::open_in_section(body),
        stage: t.stage,
        blocked: t.blocked,
        waiting_on: t.waiting_on,
        blocked_kind: fields.get("blocked-kind").cloned().unwrap_or_default(),
        depends: t.depends,
        claimed_by: t.claimed_by,
        held_by: t.held_by,
        builder,
        tester,
        gated_by,
        heartbeat: t.heartbeat,
        modified,
        level: t.level,
        branch: t.branch,
        worktree: t.worktree,
        paused: t.paused,
    })
}

/// `change` for a caller that already holds the queue's lock.
fn change_locked(
    root: &Path,
    id: &str,
    who: &str,
    stamp: &str,
    f: impl FnOnce(&Current) -> Result<Change, String>,
) -> Result<String, String> {
    // Dependents this change frees (it's just been merged), and where it went back to with why
    // (it was merged and isn't any more), for the lines written once it's saved.
    let mut freed: Option<Vec<String>> = None;
    let mut went_back: Option<(String, String)> = None;
    let path = edit_locked(root, id, |path, text| {
        let now = current(root, path, &text)?;
        let (fields, _) = front_matter(&text).ok_or("the task has no front matter")?;
        let mut c = f(&now)?;
        // The owner's own task in Building is the owner's, however it got there (an override, a
        // return): never "Building with no claim", and never an agent's claim (review C4).
        if now.human && c.stage == "building" && !is_owner(&c.claimed_by) {
            c.claimed_by = if is_owner(who) {
                who.to_string()
            } else {
                "owner".to_string()
            };
        }
        // A claim is held Building, not blocked. One per holder (the owner may hold several),
        // whichever change makes it: a pass from Ready, a return to the builder, an unblock, a set.
        let held_before = now.stage == "building" && !now.blocked && !now.claimed_by.is_empty();
        let held_after = c.stage == "building" && !c.blocked && !c.claimed_by.is_empty();
        let new_hold = held_after && !(held_before && same_holder(&now.claimed_by, &c.claimed_by));
        if new_hold && !is_owner(&c.claimed_by) {
            let tasks = tasks_now(root);
            // Still blocked by tasks it depends on (0024): waiting, not held.
            let depends = match &c.depends {
                Some(d) => queue::parse_depends(d).0,
                None => now.depends.clone(),
            };
            // A hold is a task under way (`block::started`).
            let waits = depends.iter().any(|d| {
                d != id
                    && tasks
                        .iter()
                        .any(|o| o.id == *d && crate::block::holds(o, true))
            });
            if let Some(held) = holding(&tasks, &c.claimed_by, id).filter(|_| !waits) {
                return Err(if same_holder(&c.claimed_by, who) {
                    already_holds(&held)
                } else {
                    format!(
                        "{} already holds {} in Building; one claim per holder, so they pass, return or block it first",
                        c.claimed_by,
                        keys_of(root, &[held])
                    )
                });
            }
        }
        // Merged now (0042: Your check or Done; the owner's own task, Done) or no longer. Read
        // before this change is written, so `freed` is what it was holding back.
        let met_before = crate::block::met_at(&now.stage, now.human);
        let met_after = crate::block::met_at(&c.stage, now.human);
        if !met_before && met_after {
            let tasks = tasks_now(root);
            freed = Some(
                tasks
                    .iter()
                    .filter(|o| {
                        o.id != id && o.stage != "done" && o.depends.iter().any(|d| d == id)
                    })
                    .filter(|o| {
                        if crate::block::started(o) {
                            now.kept_back.1
                        } else {
                            now.kept_back.0
                        }
                    })
                    .map(|o| o.id.clone())
                    .collect(),
            );
        } else if met_before && !met_after {
            // The reason a return gives, from the stage line it writes (`returned: <why>`).
            let why = c
                .log
                .iter()
                .find_map(|l| {
                    queue::stage_line(&format!("- {l}")).map(|(_, detail)| {
                        detail
                            .strip_prefix("returned: ")
                            .unwrap_or_default()
                            .to_string()
                    })
                })
                .unwrap_or_default();
            went_back = Some((c.stage.clone(), why));
        }
        let status = mirror_status(&c.stage, c.blocked);
        let mut new = set_front(&text, "stage", &c.stage)?;
        if crate::layout::keeps_status(root) {
            new = set_front(&new, "status", status)?;
        }
        new = set_front(&new, "waiting_on", &c.waiting_on)?;
        // The block's kind sits next to what it waits on; a file that never had one gets none.
        if !c.blocked_kind.is_empty() || fields.contains_key("blocked-kind") {
            new = set_front_after(&new, "blocked-kind", &c.blocked_kind, "waiting_on")?;
        }
        if let Some(depends) = &c.depends {
            new = set_front(&new, "depends", depends)?;
        }
        new = set_front(&new, "claimed-by", &c.claimed_by)?;
        if let Some(commit) = &c.commit {
            new = set_front(&new, "commit", commit)?;
        }
        // Testing and Your check have a holder too (0030), shown like the claim but not one: who
        // may pass is unchanged. Testing goes to whoever passes it there from Building; back in
        // Testing any other way, to whoever did that last. Your check is the owner's.
        let held_by = match c.stage.as_str() {
            s if s == now.stage => now.held_by.clone(),
            "testing" if now.stage == "building" => who.to_string(),
            "testing" => now.tester.clone().unwrap_or_else(|| who.to_string()),
            "your-check" => "owner".to_string(),
            _ => String::new(),
        };
        if !held_by.is_empty() || fields.contains_key("held-by") {
            new = set_front(&new, "held-by", &held_by)?;
        }
        // The lease restarts when the holder acts, and whenever a change makes a hold: a claim,
        // a return to the builder, or an unblock (time spent waiting isn't time gone quiet).
        let holder_acts = now.stage == "building"
            && !now.claimed_by.is_empty()
            && same_holder(&now.claimed_by, who);
        let new_claim = c.stage == "building"
            && !c.claimed_by.is_empty()
            && (now.stage != "building" || !same_holder(&now.claimed_by, &c.claimed_by));
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
    })?;
    if let Some(held) = freed {
        log_unblocked(root, id, &held, who, stamp);
    }
    if let Some((to, why)) = went_back {
        note_went_back(root, id, &to, &why, who, stamp);
    }
    Ok(path)
}

/// Task `dep_id` was just merged (0042: it reached Your check, or Done; the owner's own task,
/// Done). Of the tasks it was holding back (`waiting`, read before the change), every one that's
/// blocked only by tasks (not flagged) and that waits on nothing else now gets a log line saying
/// it's free, `unblocked: mu-14, mu-16 done; mu-17 in Your check` (by their keys, 0033), which
/// also ends its blocked span on the timeline. The block itself is derived from `depends:`, so it
/// has cleared whether or not this line is written; a line that can't be written loses only the
/// line, and never turns the change that's already written into an error. Done after Your check
/// frees nothing more, so it writes nothing. The caller holds the queue's lock.
///
/// A freed task in Building held by someone (not the owner) is theirs again: its lease starts
/// over (time spent waiting isn't time gone quiet). If they already hold another task in Building,
/// one holder would hold two, so the freed one goes back to Ready instead, with its claim
/// released and a stage line saying why (`released: wt-a already holds mu-7`).
fn log_unblocked(root: &Path, dep_id: &str, waiting: &[String], who: &str, stamp: &str) {
    if waiting.is_empty() {
        return;
    }
    let prefix = crate::key::prefix_for(root);
    let key = |id: &str| crate::key::key(&prefix, id);
    let tasks = tasks_now(root);
    let mut released: Vec<&str> = Vec::new();
    for t in &tasks {
        let freed = t.id != dep_id
            && waiting.contains(&t.id)
            && t.stage != "done"
            && !t.blocked
            && waiting_on_deps(t, &tasks).is_empty();
        if !freed {
            continue;
        }
        // What it waited on, by how each is met now: done, or merged and in Your check.
        let state = |d: &str| {
            tasks
                .iter()
                .find(|o| o.id == d && crate::block::met(o))
                .map(|o| o.stage == "done")
        };
        let deps = || t.depends.iter().map(String::as_str).filter(|d| *d != t.id);
        let done: Vec<String> = deps().filter(|d| state(d) == Some(true)).map(key).collect();
        let merged: Vec<String> = deps()
            .filter(|d| state(d) == Some(false))
            .map(key)
            .collect();
        let mut parts = Vec::new();
        if !done.is_empty() {
            parts.push(format!("{} done", done.join(", ")));
        }
        if !merged.is_empty() {
            parts.push(format!("{} in Your check", merged.join(", ")));
        }
        let line = format!("{stamp} {who}: unblocked: {}", parts.join("; "));
        let held = t.stage == "building" && !t.claimed_by.is_empty() && !is_owner(&t.claimed_by);
        // Another task this holder has in Building, not blocked (one freed by this same Done
        // counts until it's been released here).
        let other = tasks.iter().find(|o| {
            o.id != t.id
                && o.stage == "building"
                && o.blocked_kind.is_empty()
                && !o.claimed_by.is_empty()
                && same_holder(&o.claimed_by, &t.claimed_by)
                && !released.contains(&o.id.as_str())
        });
        let _ = match other.filter(|_| held) {
            Some(o) => {
                released.push(&t.id);
                let why = format!(
                    " (released: {} already holds {})",
                    crate::session::split(&t.claimed_by).0,
                    key(&o.id)
                );
                change_locked(root, &t.id, who, stamp, |c| {
                    Ok(Change {
                        stage: "ready".into(),
                        blocked: c.blocked,
                        waiting_on: c.waiting_on.clone(),
                        blocked_kind: c.blocked_kind.clone(),
                        depends: None,
                        claimed_by: String::new(),
                        commit: None,
                        log: vec![
                            line.clone(),
                            stage_line(stamp, who, "building", "ready", &why),
                        ],
                    })
                })
            }
            None => edit_locked(root, &t.id, |_, text| {
                let text = if held {
                    set_front(&text, "heartbeat", stamp)?
                } else {
                    text
                };
                Ok((append_log(&text, &line), None))
            }),
        };
    }
}

/// Task `dep_id` was merged and isn't any more: sent back to `to` (a return from Your check, or an
/// override), with `why` when a return gave one (0042). The tasks that depend on it and haven't
/// started are blocked by it again, derived, with nothing written. Each one already under way
/// (`block::started`) keeps going, its claim and stage as they are, and gets a note on its log
/// instead: `mu-2 went back to Building: <why>` (by its key, 0033). A note that can't be
/// written loses only the note. The caller holds the queue's lock.
fn note_went_back(root: &Path, dep_id: &str, to: &str, why: &str, who: &str, stamp: &str) {
    let dep_key = crate::key::key(&crate::key::prefix_for(root), dep_id);
    let head = format!("{stamp} {who}: {dep_key} went back to {}", stage_label(to));
    // `why` was checked as one line by `return`; still, never let it log a stage change.
    let line = free_text_line(format!("{head}: {why}"), "the reason")
        .ok()
        .filter(|_| !why.is_empty())
        .unwrap_or(head);
    for t in tasks_now(root).iter().filter(|t| {
        t.id != dep_id
            && t.stage != "done"
            && t.depends.iter().any(|d| d == dep_id)
            && crate::block::started(t)
    }) {
        let _ = edit_locked(root, &t.id, |_, text| Ok((append_log(&text, &line), None)));
    }
}

/// `set_front`, but a key the front matter doesn't have yet goes right after `after` (when that's
/// there), so related fields sit together.
fn set_front_after(text: &str, key: &str, value: &str, after: &str) -> Result<String, String> {
    let has = |k: &str| front_matter(text).is_some_and(|(fm, _)| fm.contains_key(k));
    if has(key) || !has(after) {
        return set_front(text, key, value);
    }
    let rest = text
        .strip_prefix("---\n")
        .ok_or("the task has no front matter")?;
    let end = rest
        .find("\n---\n")
        .ok_or("the task's front matter isn't closed")?;
    let prefix = format!("{after}:");
    let mut lines: Vec<String> = rest[..end].lines().map(String::from).collect();
    let at = lines
        .iter()
        .position(|l| l.starts_with(&prefix))
        .map_or(lines.len(), |i| i + 1);
    lines.insert(
        at,
        if value.is_empty() {
            format!("{key}:")
        } else {
            format!("{key}: {value}")
        },
    );
    Ok(format!("---\n{}{}", lines.join("\n"), &rest[end..]))
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

/// A git commit id: 7 to 64 hex characters (a SHA-1 or a SHA-256 repo's), kept lowercase. Empty
/// is fine (none given).
fn clean_commit(input: &str) -> Result<String, String> {
    let c = input.trim().to_lowercase();
    if c.is_empty() || ((7..=64).contains(&c.len()) && c.bytes().all(|b| b.is_ascii_hexdigit())) {
        Ok(c)
    } else {
        Err(format!(
            "commit must be a git commit id (7 to 64 hex characters), got {input:?}"
        ))
    }
}

/// Hand a task to the next stage. Refused when it's blocked or done, when someone else holds
/// Ready or Building, Shaping → Ready without acceptance criteria or with an open vision conflict
/// (0029; a task never checked isn't held back), Building → Testing without
/// the gate's result line, Testing → Your check before every criterion but the owner's is
/// ticked, and Your check → Done before the owner's are (`— verify: owner`, 0028). The owner's
/// own task (`for: human`, 0022) goes Ready → Building (To do → Doing) only for the owner, then
/// straight to Done, with no gate line and no criteria. Into Building from an earlier stage is
/// refused, as `claim` is, while a task it depends on isn't merged (0042); the owner's
/// `muster stage` override isn't.
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
    let expected = seen_stage(opts.from.as_deref())?;
    change(root, id, &who, stamp, |t| {
        if t.stage == "testing" && !t.branch.is_empty() {
            return Err(started_pass(id, &t.branch));
        }
        let c = pass_change(id, t, &who, stamp, &gate, &commit, expected)?;
        // Into Building from before it is a start, like a claim: refused, for anyone, while what
        // it depends on isn't merged (0042). Read under the lock, before anything is written.
        if c.stage == "building" && stage_index(&t.stage) < stage_index("building") {
            if let Some(e) = deps_refusal(id, &tasks_now(root), "muster next") {
                return Err(e);
            }
        }
        Ok(c)
    })
}

/// Why a started task (0023) isn't passed from Testing like any other: that pass is the merge.
fn started_pass(id: &str, branch: &str) -> String {
    format!(
        "{id} was started on its own branch ({branch}), so passing it from Testing merges it into main: run muster pass {id} --from testing from a terminal or a session"
    )
}

/// Refused, with why, unless a pass from Testing of a started task (0023) would be accepted now
/// as `who` (every criterion but the owner's ticked, not blocked, still in Testing). Writes
/// nothing: the merge checks this before it rebases, gates and merges, so a pass that would be
/// refused never merges.
pub fn check_merge_pass(root: &Path, id: &str, who: &str) -> Result<(), String> {
    let who = actor(who)?;
    let path = find_task(root, id)?;
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let t = current(root, &path, &text)?;
    if t.branch.is_empty() {
        return Err(format!(
            "{id} wasn't started on its own branch (muster start)"
        ));
    }
    pass_change(id, &t, &who, "", "", "", Some("testing")).map(|_| ())
}

/// The merge's pass (0023): Testing → Your check as `pass` makes it, then, under the same lock,
/// the worktree and branch cleared, `commit:` set to the merge commit, and the report logged.
pub fn pass_merged(
    root: &Path,
    id: &str,
    who: &str,
    stamp: &str,
    commit: &str,
    report: &str,
) -> Result<String, String> {
    let (commit, line) = check_merge_facts(who, stamp, commit, report)?;
    let who = actor(who)?;
    with_lock(root, || {
        // The merge is on main whatever the pass says: try the pass, then record the facts
        // either way, so a task that moved or was blocked meanwhile still says it's merged.
        let passed = change_locked(root, id, &who, stamp, |t| {
            pass_change(id, t, &who, stamp, "", "", Some("testing"))
        });
        let path = edit_locked(root, id, |_, text| {
            let mut new = set_front(&text, "worktree", "")?;
            new = set_front(&new, "branch", "")?;
            new = clear_front(&new, "worktree-runner")?;
            new = set_front(&new, "commit", &commit)?;
            Ok((append_log(&new, &line), None))
        })?;
        passed.map_err(|why| {
            let stage = queue::read_task(root, id)
                .map(|t| {
                    let blocked = if t.blocked {
                        format!(", blocked on {}", t.waiting_on)
                    } else {
                        String::new()
                    };
                    format!("{}{blocked}", stage_label(&t.stage))
                })
                .unwrap_or_else(|_| "unreadable".into());
            format!(
                "merged into main as {}; the task is {stage}: use muster stage {id} your-check once it's settled (the pass was refused: {why})",
                commit.chars().take(7).collect::<String>()
            )
        })?;
        Ok(path)
    })
}

/// The merge's facts as `pass_merged` will write them, checked before anything is merged: the
/// commit (7 to 64 hex characters) and the report's log line (one line, nothing that reads as a
/// stage change or a check-in). Returns both, cleaned.
pub fn check_merge_facts(
    who: &str,
    stamp: &str,
    commit: &str,
    report: &str,
) -> Result<(String, String), String> {
    let who = actor(who)?;
    let commit = clean_commit(commit)?;
    if commit.is_empty() {
        return Err("the merge has no commit".into());
    }
    let line = free_text_line(
        format!(
            "{stamp} {who}: {}",
            one_line(report, 1500, "the merge report")?
        ),
        "the merge report",
    )?;
    Ok((commit, line))
}

/// Drop a front-matter field (a no-op when it isn't there).
pub(crate) fn clear_front(text: &str, key: &str) -> Result<String, String> {
    let rest = text
        .strip_prefix("---\n")
        .ok_or("the task has no front matter")?;
    let end = rest
        .find("\n---\n")
        .ok_or("the task's front matter isn't closed")?;
    let prefix = format!("{key}:");
    let kept: Vec<&str> = rest[..end]
        .lines()
        .filter(|l| !l.starts_with(&prefix))
        .collect();
    Ok(format!("---\n{}{}", kept.join("\n"), &rest[end..]))
}

/// `muster start`'s claim (0023): Ready → Building for `who` by `claim_task`'s rules (one claim
/// per holder, everything it depends on merged, 0042), with the worktree and branch recorded and logged,
/// all under one lock. The git side is made before this; if the claim is refused, the caller
/// removes it again. `runner`: the worktree is one a runner made (`start --here`), recorded as
/// `worktree-runner: yes` so muster never removes it.
pub fn start_task(
    root: &Path,
    id: &str,
    who: &str,
    stamp: &str,
    worktree: &str,
    branch: &str,
    runner: bool,
) -> Result<String, String> {
    let who = actor(who)?;
    let line = free_text_line(
        format!(
            "{stamp} {who}: started in {}worktree {} on branch {}",
            if runner { "the runner's " } else { "" },
            one_line(worktree, 1000, "the worktree")?,
            one_line(branch, 200, "the branch")?
        ),
        "the worktree",
    )?;
    with_lock(root, || {
        let tasks = tasks_now(root);
        if let Some(t) = tasks.iter().find(|t| t.id == id) {
            if t.is_human() {
                return Err(format!(
                    "{id} is the owner's to do; a human task isn't built in a worktree"
                ));
            }
        }
        if let Some(e) = deps_refusal(id, &tasks, "muster ready --parallel") {
            return Err(e);
        }
        change_locked(root, id, &who, stamp, |t| {
            pass_change(id, t, &who, stamp, "", "", Some("ready"))
        })?;
        edit_locked(root, id, |_, text| {
            let mut new = set_front(&text, "worktree", worktree)?;
            new = set_front(&new, "branch", branch)?;
            new = if runner {
                set_front(&new, "worktree-runner", "yes")?
            } else {
                clear_front(&new, "worktree-runner")?
            };
            Ok((append_log(&new, &line), None))
        })
    })
}

/// Refused unless `who` may release task `id`'s worktree (`release --clean`, 0023): it isn't
/// paused (0045), it was started (a branch on record), it's in Ready, Building or Testing, and in Building it's
/// `who`'s claim or `who` is the owner. Writes nothing.
pub fn check_release(root: &Path, id: &str, who: &str) -> Result<(), String> {
    let who = actor(who)?;
    let path = find_task(root, id)?;
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let t = current(root, &path, &text)?;
    release_rules(id, &t, &who)
}

fn release_rules(id: &str, t: &Current, who: &str) -> Result<(), String> {
    // A paused task is given up only once it's resumed (0045), before its worktree is touched.
    not_paused(id, t)?;
    if t.branch.is_empty() {
        return Err(format!(
            "{id} has no worktree or branch on record (it wasn't started with muster start)"
        ));
    }
    if !matches!(t.stage.as_str(), "ready" | "building" | "testing") {
        return Err(format!(
            "{id} is in {}; release takes a task back to Ready from Building or Testing",
            stage_label(&t.stage)
        ));
    }
    if t.stage == "building"
        && !t.claimed_by.is_empty()
        && !holds(&t.claimed_by, who, t.human)
        && !is_owner(who)
    {
        return Err(format!(
            "{id} is claimed by {}; only they or the owner release it",
            t.claimed_by
        ));
    }
    Ok(())
}

/// `release --clean`'s write (0023), once the worktree and branch are gone: back to Ready with the
/// claim released (logged as a stage change) and the worktree and branch cleared. A task already
/// in Ready only has them cleared, with a log line.
pub fn release_task(
    root: &Path,
    id: &str,
    who: &str,
    stamp: &str,
    what: &str,
) -> Result<String, String> {
    let who = actor(who)?;
    let what = one_line(what, 500, "what was removed")?;
    let line = free_text_line(format!("{stamp} {who}: released: {what}"), "the release")?;
    with_lock(root, || {
        let path = find_task(root, id)?;
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let t = current(root, &path, &text)?;
        release_rules(id, &t, &who)?;
        if t.stage != "ready" {
            change_locked(root, id, &who, stamp, |t| {
                Ok(Change {
                    stage: "ready".into(),
                    blocked: t.blocked,
                    waiting_on: t.waiting_on.clone(),
                    blocked_kind: t.blocked_kind.clone(),
                    depends: None,
                    claimed_by: String::new(),
                    commit: None,
                    log: vec![stage_line(stamp, &who, &t.stage, "ready", " (released)")],
                })
            })?;
        }
        edit_locked(root, id, |_, text| {
            let mut new = set_front(&text, "worktree", "")?;
            new = set_front(&new, "branch", "")?;
            new = clear_front(&new, "worktree-runner")?;
            new = set_front(&new, "claimed-by", "")?;
            Ok((append_log(&new, &line), None))
        })
    })
}

/// The stage a caller says it saw the task in (`--from`, the window's): None when not given.
fn seen_stage(from: Option<&str>) -> Result<Option<&'static str>, String> {
    match from.map(str::trim) {
        Some(s) if !s.is_empty() => Ok(Some(pick(s, &STAGES, "", "stage")?)),
        _ => Ok(None),
    }
}

/// Refuses moving a paused task (0045), for anyone, the owner's override included: resume it first.
fn not_paused(id: &str, t: &Current) -> Result<(), String> {
    if t.paused.is_empty() {
        Ok(())
    } else {
        Err(crate::pause::refusal(id, &t.paused))
    }
}

/// Refuses a change made from a stage the task has since left (a double click, a stale window).
fn check_seen(id: &str, t: &Current, expected: Option<&str>) -> Result<(), String> {
    match expected.filter(|s| *s != t.stage) {
        Some(seen) => Err(format!(
            "{id} is now in {}, not {}; it moved on since you looked",
            stage_label(&t.stage),
            stage_label(seen)
        )),
        None => Ok(()),
    }
}

/// A started task (0023) goes to Your check or Done by the merge, so a stage change or move there
/// is refused until its branch is in main (merged some other way); the owner too, as this is about
/// the merge, not who asks. The owner's own tasks and tasks with no branch aren't held to it.
fn merge_rule(id: &str, root: &Path, t: &Current, to: &str) -> Result<(), String> {
    if t.human || t.branch.is_empty() || !matches!(to, "your-check" | "done") {
        return Ok(());
    }
    crate::parallel::check_in_main(id, root, &t.worktree, &t.branch, stage_label(to))
}

/// The tester isn't the builder (0047). Moving an agent's task from Testing (or before it) to Your
/// check or Done is refused when `who` built it: claimed it (Ready → Building) or passed it into
/// Testing with the gate's line, as `session::same_holder` matches holders. Otherwise, what the
/// stage line adds: the owner's override (the owner never built an agent's task), or a tester
/// under another label in a builder's session (a coordinator's subagents share its session id),
/// so the owner sees it. Nothing for the owner's own task, or one nobody built.
fn tester_rule(id: &str, t: &Current, who: &str, to: &str) -> Result<Option<&'static str>, String> {
    let past_testing =
        stage_index(&t.stage) <= stage_index("testing") && stage_index(to) > stage_index("testing");
    let builders: Vec<&str> = [&t.builder, &t.gated_by]
        .into_iter()
        .flatten()
        .map(String::as_str)
        .collect();
    if t.human || !past_testing || builders.is_empty() {
        return Ok(None);
    }
    if is_owner(who) {
        return Ok(Some("owner's override: no separate tester"));
    }
    if let Some(b) = builders.iter().find(|b| same_holder(b, who)) {
        return Err(built_it(id, b));
    }
    let session = |h: &str| crate::session::split(h).1.map(|s| s.id);
    let mine = session(who);
    let shared = mine.is_some() && builders.iter().any(|b| session(b) == mine);
    Ok(shared.then_some("tested under another label in the builder's session"))
}

/// The refusal of a builder's pass out of Testing: who built it (the label, and its session when
/// stamped) and what to do, with a QA label beside the builder's (`muster/wt-0042` → `muster/qa-0042`).
fn built_it(id: &str, builder: &str) -> String {
    let (label, stamp) = crate::session::split(builder);
    let named = match stamp {
        Some(s) => format!("{label} (session {})", s.id),
        None => label.to_string(),
    };
    let word = label.split_whitespace().next().unwrap_or(label);
    let qa = match word.rfind('/') {
        Some(i) => format!("{}qa-{id}", &word[..=i]),
        None => format!("qa-{id}"),
    };
    format!("{id} was built by {named}; a different session tests it: run the QA pass with its own --as (e.g. {qa}), or the owner passes it")
}

/// A stage line's detail with `note` added: ` (<note>)`, or `; <note>` inside the brackets.
fn with_note(detail: String, note: Option<&str>) -> String {
    match (note, detail.strip_suffix(')')) {
        (None, _) => detail,
        (Some(n), Some(open)) => format!("{open}; {n})"),
        (Some(n), None) => format!("{detail} ({n})"),
    }
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
    check_seen(id, t, expected)?;
    not_paused(id, t)?;
    if t.blocked {
        let on = if t.waiting_on.is_empty() {
            "something it doesn't name"
        } else {
            &t.waiting_on
        };
        return Err(format!("{id} is blocked on {on}; unblock it first"));
    }
    let from = t.stage.as_str();
    let Some(&next) = STAGES.get(stage_index(from) + 1) else {
        return Err(format!("{id} is done; there's no stage after Done"));
    };
    // The owner's own task (0022) has no Testing or Your check: Done comes after Doing. And the
    // owner's Start takes it to Doing from wherever it waits (Backlog or Shaping after an
    // override), with no criteria to write first.
    let to = if t.human && stage_index(from) >= stage_index("building") {
        "done"
    } else if t.human && is_owner(who) {
        "building"
    } else {
        next
    };
    // Only the owner starts or finishes it, whoever's claim is (or isn't) on the file.
    if t.human && matches!(to, "building" | "done") && !is_owner(who) {
        return Err(owners_to_do(id));
    }
    if !gate.is_empty() && t.human {
        return Err(format!(
            "--gate is only for an agent's task passing Building → Testing; {id} is the owner's to do, and Done comes straight after Doing"
        ));
    }
    if to == "building" && t.level != "task" {
        return Err(format!(
            "{id} is a {}; its tasks are what agents build",
            t.level
        ));
    }
    if !gate.is_empty() && from != "building" {
        return Err("--gate is only for passing Building → Testing".into());
    }
    if !commit.is_empty() && from != "your-check" {
        return Err("--commit is only for passing Your check → Done".into());
    }
    if matches!(from, "ready" | "building")
        && !t.claimed_by.is_empty()
        && !holds(&t.claimed_by, who, t.human)
    {
        return Err(format!("{id} is claimed by {}", t.claimed_by));
    }
    // Out of Testing only by someone who didn't build it (0047), before the criteria are counted.
    let note = tester_rule(id, t, who, to)?;
    let (items, ticked) = t.criteria;
    let detail = match from {
        // The owner's own task is done when they say so: no gate line, no criteria to tick.
        _ if t.human && to == "done" => String::new(),
        _ if t.human && to == "building" => " (claimed)".to_string(),
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
        "building" if t.review_open > 0 => {
            return Err(crate::review::holds_testing(
                id,
                t.review_open,
                &t.review_ids,
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
        blocked_kind: t.blocked_kind.clone(),
        depends: None,
        claimed_by: if to == "building" {
            who.to_string()
        } else {
            released(t)
        },
        commit: (to == "done" && !commit.is_empty()).then(|| commit.to_string()),
        log: vec![stage_line(stamp, who, from, to, &with_note(detail, note))],
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
/// skipped), with each file's modified time and what blocks it (`block::derive`). For decisions
/// taken under the lock.
pub(crate) fn tasks_now(root: &Path) -> Vec<queue::Task> {
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
    crate::block::derive(&mut out);
    out
}

/// The task `who` holds in Building in this queue (not blocked, by tasks either: a blocked task is
/// waiting, and blocking it is how a holder lets go), other than `except`.
fn holding(tasks: &[queue::Task], who: &str, except: &str) -> Option<String> {
    tasks
        .iter()
        .find(|t| {
            t.stage == "building"
                && t.blocked_kind.is_empty()
                && !t.claimed_by.is_empty()
                && same_holder(&t.claimed_by, who)
                && t.id != except
        })
        .map(|t| t.id.clone())
}

/// Tasks of the queue at `root` as a log line names them (0033): by their keys, `mu-14, mu-16`.
/// The front matter keeps their ids.
fn keys_of(root: &Path, ids: &[String]) -> String {
    let prefix = crate::key::prefix_for(root);
    ids.iter()
        .map(|d| crate::key::key(&prefix, d))
        .collect::<Vec<_>>()
        .join(", ")
}

fn already_holds(id: &str) -> String {
    format!("you already hold {id}; pass, return or block it first")
}

fn is_id(id: &str) -> bool {
    id.len() == 4 && id.bytes().all(|b| b.is_ascii_digit())
}

/// What `t` still waits on among `tasks` (0019): its `depends:` not merged yet (0042,
/// `block::holds`). A dependency that doesn't exist doesn't hold it (check reports it).
fn waiting_on_deps(t: &queue::Task, tasks: &[queue::Task]) -> Vec<String> {
    let under_way = crate::block::started(t);
    t.depends
        .iter()
        .filter(|d| **d != t.id)
        .filter(|d| {
            tasks
                .iter()
                .any(|o| o.id == **d && crate::block::holds(o, under_way))
        })
        .cloned()
        .collect()
}

/// Why task `id` can't start yet: what it depends on that isn't merged (`waiting_on_deps`), in the
/// words `claim`, `next`, a pass into Building and `start` share, ending with where to pick
/// another (`pick`); None when nothing holds it.
pub(crate) fn deps_refusal(id: &str, tasks: &[queue::Task], pick: &str) -> Option<String> {
    let t = tasks.iter().find(|t| t.id == id)?;
    let open = waiting_on_deps(t, tasks);
    if open.is_empty() {
        return None;
    }
    // The owner's own tasks are only ever done; an agent's merged once in Your check.
    let human = |d: &String| tasks.iter().any(|o| o.id == *d && o.is_human());
    let not_yet = if open.iter().all(human) {
        "not done yet"
    } else {
        "not merged yet (an agent's task counts once it's in Your check, the owner's own once it's Done)"
    };
    Some(format!(
        "{id} depends on {}, {not_yet}; pick another ({pick})",
        open.join(", ")
    ))
}

/// Claim a Ready task for `who`: Ready → Building, as `pass --from ready`, and refused while
/// `who` already holds another task in Building in this queue, or while a task it depends on
/// isn't merged (in Your check or Done; the owner's own, Done). The checks and the claim happen
/// under one lock, so of two claims at once only one can win.
pub fn claim_task(root: &Path, id: &str, who: &str, stamp: &str) -> Result<String, String> {
    let who = actor(who)?;
    with_lock(root, || {
        if let Some(e) = deps_refusal(id, &tasks_now(root), "muster next") {
            return Err(e);
        }
        // `change_locked` refuses a second claim for `who`, under the same lock.
        change_locked(root, id, &who, stamp, |t| {
            pass_change(id, t, &who, stamp, "", "", Some("ready"))
        })
    })
}

fn priority_rank(p: &str) -> u8 {
    match p.to_ascii_lowercase().as_str() {
        "high" => 0,
        "low" => 2,
        _ => 1,
    }
}

/// Claim the top Ready task `who` may take: not blocked, not held in Ready by someone else,
/// everything it depends on merged (`waiting_on_deps`), and not the owner's own (`for: human`)
/// unless `who` is the owner; high before normal before low, then the lowest id. None when
/// there's nothing to take.
/// Refused while `who` already holds a task in Building here.
pub fn next_task(root: &Path, who: &str, stamp: &str) -> Result<Option<Claimed>, String> {
    let who = actor(who)?;
    with_lock(root, || {
        let tasks = tasks_now(root);
        // Checked before picking, so a holder is told so rather than handed a refusal for one
        // task; `change` checks again. The owner may hold several.
        if let Some(held) = holding(&tasks, &who, "").filter(|_| !is_owner(&who)) {
            return Err(already_holds(&held));
        }
        let Some(top) = tasks
            .iter()
            .filter(|t| t.stage == "ready" && !t.blocked && t.paused.is_empty() && is_id(&t.id))
            .filter(|t| t.level == "task")
            // The owner's own tasks are never an agent's to take (0022).
            .filter(|t| !t.is_human() || is_owner(&who))
            .filter(|t| t.claimed_by.is_empty() || same_holder(&t.claimed_by, &who))
            .filter(|t| waiting_on_deps(t, &tasks).is_empty())
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
        held_in_building(root, id, path, &text, &who)?;
        Ok((set_front(&text, "heartbeat", stamp)?, None))
    })
}

/// Refused unless the task is in Building and `who` holds it: whose lease a heartbeat or a
/// check-in may renew.
fn held_in_building(
    root: &Path,
    id: &str,
    path: &Path,
    text: &str,
    who: &str,
) -> Result<(), String> {
    let t = queue::parse_task("", root, &queue::folder_of(root, path), path, text, 0)
        .ok_or("the task has no front matter")?;
    if t.stage != "building" {
        return Err(format!(
            "{id} is in {}, not Building; only a claim in Building has a lease to keep alive",
            stage_label(&t.stage)
        ));
    }
    if t.claimed_by.is_empty() {
        return Err(format!("nobody holds {id}; claim it first"));
    }
    if !holds(&t.claimed_by, who, t.is_human()) {
        return Err(format!("{id} is held by {}, not {who}", t.claimed_by));
    }
    Ok(())
}

/// A check-in's percentage (`--pct`): a whole number from 0 to 100, `%` allowed. Empty is none.
fn clean_pct(input: &str) -> Result<Option<u8>, String> {
    let t = input.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let digits = t.strip_suffix('%').unwrap_or(t);
    let n = (!digits.is_empty() && digits.len() <= 3 && digits.bytes().all(|b| b.is_ascii_digit()))
        .then(|| digits.parse::<u8>().ok())
        .flatten()
        .filter(|n| *n <= 100);
    match n {
        Some(n) => Ok(Some(n)),
        None => Err(format!(
            "--pct must be a whole number from 0 to 100, got {:?}",
            t.chars().take(20).collect::<String>()
        )),
    }
}

/// The longest check-in, in characters: one line on the card, the rest in the task pane.
const MAX_CHECKIN: usize = 500;

/// A holder's check-in (0030): what's done and what's next, with the session's own estimate of how
/// far along it is (`pct`, 0–100, or "" for none). Logged as `<who>: check-in 60% · <text>`, and
/// it renews the claim's lease exactly as `heartbeat_task` does, under the same lock. Only the
/// holder, only in Building. Text that would read as a stage change is refused.
pub fn checkin_task(
    root: &Path,
    id: &str,
    text: &str,
    pct: &str,
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    let who = actor(who)?;
    let text = one_line(text, MAX_CHECKIN, "the check-in")?;
    let pct = clean_pct(pct)?;
    let line = match pct {
        Some(n) => format!("{stamp} {who}: check-in {n}% · {text}"),
        None => format!("{stamp} {who}: check-in · {text}"),
    };
    if reads_as_stage(&line) {
        return Err(
            "the check-in would read as a stage change in the log (\"<who>: stage <from> → <to>\"); say it another way".into(),
        );
    }
    if crate::pause::pause_shaped(&format!("- {line}")).is_some() {
        return Err(
            "the check-in would read as a pause or resume in the log (\"…: paused\", \"…: resumed\"); say it another way".into(),
        );
    }
    // What the log's reader takes from the line is exactly this check-in.
    let back = queue::checkin_line(&format!("- {line}"));
    if back
        .as_ref()
        .map(|c| (c.who.as_str(), c.pct, c.text.as_str()))
        != Some((&who, pct, &text))
    {
        return Err("the check-in wouldn't read back as written; say it another way".into());
    }
    edit(root, id, |path, file| {
        held_in_building(root, id, path, &file, &who)?;
        let new = set_front(&file, "heartbeat", stamp)?;
        Ok((append_log(&new, &line), None))
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
    // As typed (1.25h is 75 minutes), the way Settings reads it (0036).
    let minutes = crate::settings::span_minutes(input)
        .ok_or_else(|| format!("lease {input:?}: use 30m, 2h or 1d"))?;
    if minutes < 1 {
        return Err(format!("lease {input:?} is shorter than a minute"));
    }
    Ok(minutes)
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

/// What releasing the claim on `t` writes, or why it's refused: only an agent's claim in
/// Building, not blocked, quiet for longer than `lease` minutes.
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
    if t.human {
        return Err(format!(
            "{id} is the owner's to do; their own work has no lease to run out"
        ));
    }
    if t.blocked {
        return Err(format!(
            "{id} is blocked on {}; it's waiting, not quiet",
            t.waiting_on
        ));
    }
    if !t.paused.is_empty() {
        return Err(format!(
            "{id} is paused (since {}); its lease doesn't run while it is",
            t.paused
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
        blocked_kind: t.blocked_kind.clone(),
        depends: None,
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

/// Give every claim whose lease ran out back to Ready: each task in Building, not blocked, not
/// the owner's own (`for: human`), held by someone, with no heartbeat (or, without one, no change to its file) for longer than
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
                && t.blocked_kind.is_empty()
                && t.paused.is_empty()
                && !t.is_human()
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
    with_lock(root, || {
        let tasks = tasks_now(root);
        if let Some(t) = tasks
            .iter()
            .find(|t| t.id == id && t.blocked_kind == "tasks")
        {
            let ids: Vec<&str> = t.blocked_by.iter().map(|b| b.id.as_str()).collect();
            return Err(format!(
                "{id} is blocked by tasks {}; it's waiting, not quiet",
                ids.join(", ")
            ));
        }
        change_locked(root, id, &who, stamp, |t| {
            release_change(id, t, lease, &who, stamp, now_secs)
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
    change(root, id, &who, stamp, |t| {
        not_paused(id, t)?;
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
            blocked_kind: t.blocked_kind.clone(),
            depends: None,
            // Back to the builder. By label only when no other session under that label holds a
            // task in Building, so whichever session works under it next (the first may have
            // ended) carries on; otherwise to the exact session that built it, which keeps one
            // claim per holder. The stage lines still say which session built it.
            claimed_by: if target == "building" {
                let builder = t.builder.clone().unwrap_or_default();
                let label = crate::session::split(&builder).0.to_string();
                if !label.is_empty() && holding(&tasks_now(root), &label, id).is_none() {
                    label
                } else {
                    builder
                }
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

/// The kinds `block` takes, for its refusals.
const BLOCK_USAGE: &str =
    "task ids (0014,0016), you, permission, question Q2 or outside, then what it waits on";

/// Block a task on `on`, as `--on` reads (`block::split_on`): `"you: pick a colour"`,
/// `"0014,0016"`, `"owner: which colour?"`. See `block_as`.
pub fn block_task(
    root: &Path,
    id: &str,
    on: &str,
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    let (kind, detail) = crate::block::split_on(on);
    block_as(root, id, &kind, &detail, who, stamp)
}

/// Block a task (0024), with what it waits on and why:
/// - `tasks`: `detail` starts with task ids (`0014,0016`, maybe then a reason). They're added to
///   `depends:` and no flag is set: the task shows blocked by them until they're done. An old
///   block on task ids (no kind) is cleared, since the ids now say it.
/// - `you` (`owner`), `permission`, `outside`: the flag with that kind and `detail`, which they need.
/// - `question` (`capture`): a question's number, `Q2`, which must be open in this task; `capture`
///   also takes words, as before kinds.
///
/// The task keeps its stage (and its claim). Refused: an unknown kind, a missing detail, a closed
/// or missing question, the same block again.
pub fn block_as(
    root: &Path,
    id: &str,
    kind: &str,
    detail: &str,
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    let who = actor(who)?;
    let typed = match kind.trim().to_lowercase().as_str() {
        "tasks" => return block_on_tasks(root, id, detail, &who, stamp),
        "" => {
            return Err(format!(
                "say what it's blocked by and what it's waiting on: {BLOCK_USAGE}"
            ))
        }
        "you" | "owner" => "you",
        "permission" => "permission",
        "question" | "capture" => "question",
        "outside" => "outside",
        other => {
            let other: String = other.chars().take(24).collect();
            return Err(format!(
                "{other:?} isn't a kind of block: use {BLOCK_USAGE}"
            ));
        }
    };
    let need = match typed {
        "you" => "what you have to do or decide",
        "permission" => "what access it lacks and where to grant it",
        "question" => "the question's number, like Q2",
        _ => "what it waits on outside",
    };
    if detail.trim().is_empty() {
        return Err(format!("a block on {typed} needs {need}"));
    }
    let mut detail = one_line(detail, 200, "what it's waiting on")?;
    let qn = crate::block::question_number(&detail);
    if kind.trim().eq_ignore_ascii_case("question") && qn.is_none() {
        return Err(format!(
            "a block on question needs {need} (ask it first with muster ask), got {detail:?}"
        ));
    }
    if let Some(n) = qn {
        detail = format!("Q{n}");
    }
    let stored = crate::block::stored_waiting_on(typed, &detail);
    let line = free_text_line(
        format!("{stamp} {who}: blocked on {typed}: {detail}"),
        "what it waits on",
    )?;
    with_lock(root, || {
        if let Some(n) = qn {
            let t = queue::read_task(root, id)?;
            match t.questions.iter().find(|q| q.n == Some(n)) {
                None => return Err(format!("{id} has no Q{n}; ask it first (muster ask)")),
                Some(q) if q.ticked => return Err(format!("{id}'s Q{n} is answered already")),
                Some(_) => {}
            }
        }
        change_locked(root, id, &who, stamp, |t| {
            if t.blocked && t.blocked_kind == typed && t.waiting_on == stored {
                return Err(format!("{id} is already blocked on {typed}: {detail}"));
            }
            Ok(Change {
                stage: t.stage.clone(),
                blocked: true,
                waiting_on: stored.clone(),
                blocked_kind: typed.to_string(),
                depends: None,
                claimed_by: t.claimed_by.clone(),
                commit: None,
                log: vec![line.clone()],
            })
        })
    })
}

/// `block_as` on tasks: the ids `detail` starts with go into `depends:` (checked as `muster set
/// depends` checks them: each a task here, not this one, no loop), each one that would hold it
/// (`block::holds`: not merged yet). An old block on task ids is cleared, as these ids say it now;
/// for it, ids that are merged or gone are dropped, and with none left the old block just ends
/// (logged as an unblock). Logged `blocked on tasks: 0014, 0016`, with the words after the ids as
/// the reason.
fn block_on_tasks(
    root: &Path,
    id: &str,
    detail: &str,
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    let detail = detail.trim();
    let split = detail
        .find(|c: char| !(c.is_ascii_digit() || c == ',' || c.is_whitespace()))
        .unwrap_or(detail.len());
    let (ids, bad) = queue::parse_depends(&detail[..split]);
    if let Some(b) = bad.first() {
        return Err(format!(
            "a block on tasks takes task ids like 0014, got {b:?}"
        ));
    }
    if ids.is_empty() {
        return Err(format!(
            "a block on tasks needs their ids, like 0014,0016; {BLOCK_USAGE}"
        ));
    }
    let why = detail[split..].trim();
    let why = if why.is_empty() {
        String::new()
    } else {
        format!(" ({})", one_line(why, 200, "the reason")?)
    };
    // Checked before the lock, so a line that would read as a stage change is refused first.
    free_text_line(
        format!(
            "{stamp} {who}: blocked on tasks: {}{why}",
            keys_of(root, &ids)
        ),
        "the reason",
    )?;
    with_lock(root, || {
        let tasks = tasks_now(root);
        let me = tasks
            .iter()
            .find(|t| t.id == id)
            .ok_or_else(|| format!("no task {id}"))?;
        // An old block on task ids (no kind): the ids now say it, so the flag goes. Its ids that
        // are merged or gone are dropped rather than refused: they don't hold it any more.
        let legacy = me.kind_guessed && me.blocked_kind == "tasks";
        let under_way = crate::block::started(me);
        let open = |d: &str| {
            tasks
                .iter()
                .any(|o| o.id == d && crate::block::holds(o, under_way))
        };
        let ids: Vec<String> = if legacy {
            ids.iter().filter(|d| open(d.as_str())).cloned().collect()
        } else {
            // One that exists but wouldn't hold it (merged, or merged once and it's under way).
            let idle = ids
                .iter()
                .filter(|d| !open(d.as_str()))
                .find_map(|d| tasks.iter().find(|o| o.id == *d));
            if let Some(dep) = idle {
                return Err(crate::block::nothing_to_wait_for(dep, id));
            }
            ids.clone()
        };
        if ids.is_empty() {
            // Nothing it named still holds it: the old block simply ends.
            return change_locked(root, id, who, stamp, |t| {
                Ok(Change {
                    stage: t.stage.clone(),
                    blocked: false,
                    waiting_on: String::new(),
                    blocked_kind: String::new(),
                    depends: None,
                    claimed_by: t.claimed_by.clone(),
                    commit: None,
                    log: vec![unblocked_line(stamp, who, &t.waiting_on)],
                })
            });
        }
        let line = format!(
            "{stamp} {who}: blocked on tasks: {}{why}",
            keys_of(root, &ids)
        );
        let mut all = me.depends.clone();
        all.extend(ids.iter().filter(|d| !me.depends.contains(d)).cloned());
        let depends = clean_depends(root, id, &all.join(", "))?;
        let added = all.len() > me.depends.len();
        if !added && !legacy {
            return Err(format!("{id} already waits on {}", ids.join(", ")));
        }
        change_locked(root, id, who, stamp, |t| {
            let (blocked, waiting_on, blocked_kind) = if legacy {
                (false, String::new(), String::new())
            } else {
                (t.blocked, t.waiting_on.clone(), t.blocked_kind.clone())
            };
            Ok(Change {
                stage: t.stage.clone(),
                blocked,
                waiting_on,
                blocked_kind,
                depends: Some(depends.clone()),
                claimed_by: t.claimed_by.clone(),
                commit: None,
                log: vec![line.clone()],
            })
        })
    })
}

/// Under the queue's lock, once task `id`'s question `n` is answered (`queue::answer`): a block on
/// that question clears, logged `unblocked: Q2 answered`. If clearing it is refused (its holder
/// now holds another task in Building), it stays blocked and the log says why. Any other block
/// stays.
pub(crate) fn unblock_answered_locked(
    root: &Path,
    id: &str,
    n: u32,
    who: &str,
    stamp: &str,
) -> Result<(), String> {
    let path = find_task(root, id)?;
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let typed = front_matter(&text)
        .and_then(|(fm, _)| fm.get("blocked-kind").cloned())
        .unwrap_or_default();
    let t = queue::read_task(root, id)?;
    let on_it = t.blocked
        && typed.eq_ignore_ascii_case("question")
        && crate::block::question_number(&t.block_detail) == Some(n);
    if !on_it {
        return Ok(());
    }
    let cleared = change_locked(root, id, who, stamp, |c| {
        Ok(Change {
            stage: c.stage.clone(),
            blocked: false,
            waiting_on: String::new(),
            blocked_kind: String::new(),
            depends: None,
            claimed_by: c.claimed_by.clone(),
            commit: None,
            log: vec![format!("{stamp} {who}: unblocked: Q{n} answered")],
        })
    });
    if let Err(e) = cleared {
        let line = free_text_line(
            format!("{stamp} {who}: Q{n} answered, still blocked: {e}"),
            "the reason",
        )
        .unwrap_or_else(|_| format!("{stamp} {who}: Q{n} answered, still blocked"));
        edit_locked(root, id, |_, text| Ok((append_log(&text, &line), None)))?;
    }
    Ok(())
}

/// Clear the blocked flag; the status mirror goes back to what the stage means.
/// A block on tasks (0024) isn't a flag: it clears by itself once they're done.
pub fn unblock_task(root: &Path, id: &str, who: &str, stamp: &str) -> Result<String, String> {
    let who = actor(who)?;
    with_lock(root, || {
        let tasks = tasks_now(root);
        let open = tasks
            .iter()
            .find(|t| t.id == id)
            .map(|t| waiting_on_deps(t, &tasks))
            .unwrap_or_default();
        change_locked(root, id, &who, stamp, |t| {
            if !t.blocked && !open.is_empty() {
                return Err(format!(
                    "{id} is blocked by tasks {}: that clears by itself once they're merged, in Your check or Done (the owner's own once Done), or change its depends: muster set {id} depends …",
                    open.join(", ")
                ));
            }
            if !t.blocked {
                return Err(format!("{id} isn't blocked"));
            }
            Ok(Change {
                stage: t.stage.clone(),
                blocked: false,
                waiting_on: String::new(),
                blocked_kind: String::new(),
                depends: None,
                claimed_by: t.claimed_by.clone(),
                commit: None,
                log: vec![unblocked_line(stamp, &who, &was_waiting(t))],
            })
        })
    })
}

/// `unblock_task`'s change for a caller that holds the queue's lock (0059, `old_pause::convert`):
/// the flag cleared, the status mirror (and in an old queue the folder) back to what the stage
/// means, the claim's lease restarted at `stamp` as for any unblock. The unblocked line is stamped
/// `at` instead, and `then` follows it. Refused when it isn't flagged. Returns the new path.
pub(crate) fn unblock_locked_at(
    root: &Path,
    id: &str,
    who: &str,
    stamp: &str,
    at: &str,
    then: &[String],
) -> Result<String, String> {
    change_locked(root, id, who, stamp, |t| {
        if !t.blocked {
            return Err(format!("{id} isn't blocked"));
        }
        let mut log = vec![unblocked_line(at, who, &was_waiting(t))];
        log.extend(then.iter().cloned());
        Ok(Change {
            stage: t.stage.clone(),
            blocked: false,
            waiting_on: String::new(),
            blocked_kind: String::new(),
            depends: None,
            claimed_by: t.claimed_by.clone(),
            commit: None,
            log,
        })
    })
}

/// The owner's (or capture's) override: put a task in any stage, logged with `(set)`. Leaving
/// Building releases its claim; the blocked flag stays. `from` is the stage the caller saw it in:
/// when given, the move is refused if the task has moved on since (as `pass_task`).
pub fn set_stage(
    root: &Path,
    id: &str,
    stage: &str,
    from: Option<&str>,
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    let who = actor(who)?;
    let stage = pick(stage.trim(), &STAGES, "", "stage")?;
    if stage.is_empty() {
        return Err(format!("stage must be one of {}", STAGES.join(", ")));
    }
    let expected = seen_stage(from)?;
    change(root, id, &who, stamp, |t| {
        check_seen(id, t, expected)?;
        not_paused(id, t)?;
        if t.stage == stage {
            return Err(format!("{id} is already in {}", stage_label(stage)));
        }
        merge_rule(id, root, t, stage)?;
        // The override doesn't let a builder skip its tester (0047).
        let note = tester_rule(id, t, &who, stage)?;
        Ok(Change {
            stage: stage.to_string(),
            blocked: t.blocked,
            waiting_on: t.waiting_on.clone(),
            blocked_kind: t.blocked_kind.clone(),
            depends: None,
            claimed_by: released(t),
            commit: None,
            log: vec![stage_line(
                stamp,
                &who,
                &t.stage,
                stage,
                &with_note(" (set)".into(), note),
            )],
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
            t.stage == "building" && !t.claimed_by.is_empty() && same_holder(&t.claimed_by, &who)
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
/// front matter to the tool's `## Comments` heading (`queue::comments_span`), else the Log's
/// heading (the last `## Log`, the one `queue::log_span` reads), or to the end when it has
/// neither. Once the task has a vision check (0029), it ends at the tool's `## Vision` section
/// too (`vision::section_span`); a description's own `## Vision` before any check is the
/// write-up's. The front matter, the Vision section, the Comments and the Log are the tool's;
/// this is the rest.
fn body_span(text: &str) -> Result<(usize, usize), String> {
    let (_, body) = front_matter(text).ok_or("the task has no front matter")?;
    let start = text.len() - body.len();
    let vision = crate::vision::checked(text)
        .then(|| crate::vision::section_span(body).map(|(at, _)| at))
        .flatten();
    // The review's section (0027) is the tool's the same way, once it has a round.
    let review = crate::review::reviewed(text)
        .then(|| crate::review::section_span(body).map(|(at, _)| at))
        .flatten();
    let tail = queue::comments_span(body)
        .map(|(at, _, _)| at)
        .or_else(|| queue::log_heading(body));
    let end = [vision, review, tail].into_iter().flatten().min();
    Ok((start, end.map_or(text.len(), |at| start + at)))
}

/// How many lines of `text` are a `## Vision` heading.
fn vision_headings(text: &str) -> usize {
    text.lines()
        .filter(|l| l.trim_end() == crate::vision::HEAD)
        .count()
}

/// Refused when a line of `text` is a `## Comments` heading: the section is the tool's
/// (`muster comment` writes it), so a write-up or a description can't start one.
fn no_comments_heading(text: &str, what: &str) -> Result<(), String> {
    if text.lines().any(|l| l.trim_end() == "## Comments") {
        return Err(format!(
            "{what} has a \"## Comments\" line, and Comments are the tool's: add one with muster comment (or the comment box in the task's pane)"
        ));
    }
    Ok(())
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
    (t.stage == "building" && !t.claimed_by.is_empty() && !holds(&t.claimed_by, who, t.is_human()))
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
        no_comments_heading(line, "the write-up")?;
        if queue::stage_line(line).is_some() {
            return Err(format!(
                "the write-up has a line that would read as a stage change in the log (\"<who>: stage <from> → <to>\"); only muster pass, return and stage write those: {line:?}"
            ));
        }
        if queue::checkin_line(line).is_some() {
            return Err(format!(
                "the write-up has a line that would read as a check-in in the log (\"<who>: check-in …\"); only muster checkin writes those: {line:?}"
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
        // Once checked, the Vision section is the tool's (0029): a new heading in the write-up
        // would be read as it. The ones the write-up already has (the description's own) stay.
        if crate::vision::checked(&text) && vision_headings(&new_body) > vision_headings(current) {
            return Err("the write-up has a new \"## Vision\" line, and the Vision section is the tool's once the task is checked: record a new check with muster vision".into());
        }
        let reviews = |t: &str| {
            t.lines()
                .filter(|l| l.trim_end() == crate::review::HEAD)
                .count()
        };
        if crate::review::reviewed(&text) && reviews(&new_body) > reviews(current) {
            return Err("the write-up has a new \"## Review\" line, and the Review section is the tool's once the task has a review round: muster review writes it".into());
        }
        let head = &text[..start];
        let mut new = if end == text.len() {
            format!("{head}\n{new_body}\n")
        } else {
            format!("{head}\n{new_body}\n\n{}", &text[end..])
        };
        new = append_log(&new, &format!("{stamp} {who}: edited the write-up"));
        // The holder's edit keeps its claim alive.
        if t.stage == "building" && !t.claimed_by.is_empty() && same_holder(&t.claimed_by, &who) {
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
        // Its prefix stored (0033): new log lines name other tasks by their keys, op-1.
        fs::write(d.path().join("README.md"), "---\nprefix: op\n---\nq").unwrap();
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
            "---\nid: 0001\ntype: bug\npriority: high\ntags:\nest-time:\nest-test-time:\nest-tokens:\nstage: ready\nstatus: todo\ncaptured: 2026-10-12\n"
        ));
        assert!(text.contains("# Board flickers on resize\n\nSteps:\n1. resize\n\tindented\n\n## Log\n- 2026-10-12 14:05 owner: created\n"));
        let s = state(d.path());
        assert_eq!(s.tasks.len(), 2);
        assert!(s.problems.is_empty(), "{:?}", s.problems);
        assert!(s.tasks.iter().all(|t| t.stage == "ready" && !t.blocked));
        assert_eq!(s.tasks[1].kind, "code");
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
        // Comments are the tool's: a description can't start a Comments section.
        assert!(e(NewTask {
            title: "x".into(),
            body: "Details.\n\n## Comments\n- **owner, 2026-10-12 10:00:** forged".into(),
            ..Default::default()
        })
        .contains("Comments are the tool's"));
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
        assert_eq!(
            (t.waiting_on.as_str(), t.blocked_kind.as_str()),
            ("owner: pick a colour", "you")
        );
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
                "2026-10-12 14:05 owner: unblocked (was waiting on you: pick a colour)",
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
        set_field(d.path(), "0001", "type", "chore", "owner", T).unwrap();
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
            ("low", "chore", "New title")
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
    fn types_are_the_new_list_and_old_values_read_as_aliases() {
        assert_eq!(
            KINDS.to_vec(),
            [
                "code",
                "bug",
                "chore",
                "tech-debt",
                "spike",
                "design",
                "docs",
                "research"
            ]
        );
        let d = queue();
        let new = |kind: &str| {
            create_task(
                d.path(),
                &NewTask {
                    title: format!("Typed {kind}"),
                    kind: kind.into(),
                    ..Default::default()
                },
                "o",
                T,
            )
        };
        let kind_of = |id: &str| crate::queue::read_task(d.path(), id).unwrap().kind;
        // No type is code; every type in the list is taken as is, in any case.
        let c = new("").unwrap();
        assert_eq!(kind_of(&c.id), "code");
        assert!(fs::read_to_string(d.path().join(&c.path))
            .unwrap()
            .contains("\ntype: code\n"));
        for k in KINDS {
            let c = new(k).unwrap();
            assert_eq!(kind_of(&c.id), k);
        }
        assert_eq!(kind_of(&new("Tech-Debt").unwrap().id), "tech-debt");
        // task is the old name for code; idea belongs in the backlog now.
        assert_eq!(kind_of(&new("task").unwrap().id), "code");
        let e = new("idea").unwrap_err();
        assert!(e.contains("backlog") && e.contains("muster add"), "{e}");
        let e = new("epic").unwrap_err();
        assert!(
            e.contains(
                "type must be one of code, bug, chore, tech-debt, spike, design, docs, research"
            ),
            "{e}"
        );
        let id = c.id.clone();
        set_field(d.path(), &id, "type", "chore", "owner", T).unwrap();
        assert_eq!(kind_of(&id), "chore");
        set_field(d.path(), &id, "type", "Task", "owner", T).unwrap();
        assert_eq!(kind_of(&id), "code");
        assert!(set_field(d.path(), &id, "type", "idea", "owner", T)
            .unwrap_err()
            .contains("backlog"));
        assert_eq!(kind_of(&id), "code");
        // Old files: task reads as code, idea stays idea, and other fields still change.
        for (raw, read) in [
            ("task", "code"),
            ("Task", "code"),
            ("idea", "idea"),
            ("bug", "bug"),
        ] {
            let d = queue();
            put(
                d.path(),
                "todo/0001-t.md",
                &task_text("status: todo", "").replace("type: task", &format!("type: {raw}")),
            );
            assert_eq!(only(d.path()).kind, read, "{raw}");
            set_field(d.path(), "0001", "priority", "high", "owner", T).unwrap();
            let t = only(d.path());
            assert_eq!((t.kind.as_str(), t.priority.as_str()), (read, "high"));
            assert!(state(d.path()).problems.is_empty());
        }
    }

    #[test]
    fn test_time_is_an_estimate_like_agent_time() {
        let d = queue();
        let c = create_task(
            d.path(),
            &NewTask {
                title: "Tested".into(),
                est_time: "2h".into(),
                est_test_time: " 30 min ".into(),
                est_tokens: "400k".into(),
                ..Default::default()
            },
            "o",
            T,
        )
        .unwrap();
        let text = fs::read_to_string(d.path().join(&c.path)).unwrap();
        assert!(
            text.contains("\nest-time: 2h\nest-test-time: 30m\nest-tokens: 400k\n"),
            "{text}"
        );
        let t = only(d.path());
        assert_eq!(
            (
                t.est_time.as_str(),
                t.est_test_time.as_str(),
                t.est_minutes,
                t.est_test_minutes
            ),
            ("2h", "30m", 120, 30)
        );
        let json = serde_json::to_value(&t).unwrap();
        assert_eq!(
            (&json["estTestTime"], &json["estTestMinutes"]),
            (&serde_json::json!("30m"), &serde_json::json!(30))
        );
        set_field(d.path(), "0001", "test-time", "1.5h", "owner", T).unwrap();
        assert_eq!(only(d.path()).est_test_minutes, 90);
        set_field(d.path(), "0001", "est-test-time", "45m", "owner", T).unwrap();
        assert_eq!(only(d.path()).est_test_time, "45m");
        let before = fs::read_to_string(d.path().join(&c.path)).unwrap();
        assert!(set_field(d.path(), "0001", "test-time", "soon", "owner", T).is_err());
        assert_eq!(fs::read_to_string(d.path().join(&c.path)).unwrap(), before);
        set_field(d.path(), "0001", "test-time", "", "owner", T).unwrap();
        let t = only(d.path());
        assert_eq!((t.est_test_time.as_str(), t.est_test_minutes), ("", 0));
        assert!(t
            .log_tail
            .last()
            .unwrap()
            .ends_with("owner: est-test-time set to "));
        assert!(create_task(
            d.path(),
            &NewTask {
                title: "Bad".into(),
                est_test_time: "5w".into(),
                ..Default::default()
            },
            "o",
            T
        )
        .is_err());
        // A hand-written value that isn't a time reads as no estimate.
        put(
            d.path(),
            "todo/0002-t.md",
            &task_text("status: todo\nest-test-time: later", "").replace("id: 0001", "id: 0002"),
        );
        let t = state(d.path())
            .tasks
            .into_iter()
            .find(|t| t.id == "0002")
            .unwrap();
        assert_eq!((t.est_test_time.as_str(), t.est_test_minutes), ("", 0));
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
    fn set_depends_checks_ids_itself_and_loops() {
        let d = queue();
        for id in ["0001", "0002", "0003"] {
            put(
                d.path(),
                &format!("todo/{id}-t.md"),
                &task_text("status: todo", "").replace("id: 0001", &format!("id: {id}")),
            );
        }
        set_field(d.path(), "0002", "depends", "0001", "capture", T).unwrap();
        set_field(d.path(), "0003", "depends", " 0002, 0001 ", "capture", T).unwrap();
        let text = fs::read_to_string(d.path().join("todo/0003-t.md")).unwrap();
        assert!(text.contains("\ndepends: [0002, 0001]\n"), "{text}");
        assert!(
            text.ends_with("capture: depends set to [op-2, op-1]\n"),
            "{text}"
        );
        let before = fs::read_to_string(d.path().join("todo/0001-t.md")).unwrap();
        for (value, why) in [
            ("0003", "would make a loop: 0001, 0002, 0003"),
            ("0001", "can't depend on itself"),
            ("0009", "no task 0009"),
            ("next week", "takes task ids"),
        ] {
            let e = set_field(d.path(), "0001", "depends", value, "capture", T).unwrap_err();
            assert!(e.contains(why), "{value}: {e}");
        }
        assert_eq!(
            fs::read_to_string(d.path().join("todo/0001-t.md")).unwrap(),
            before
        );
        // Empty clears it.
        set_field(d.path(), "0003", "depends", "", "capture", T).unwrap();
        let t = state(d.path())
            .tasks
            .into_iter()
            .find(|t| t.id == "0003")
            .unwrap();
        assert!(t.depends.is_empty());
        assert!(
            state(d.path()).problems.is_empty(),
            "{:?}",
            state(d.path()).problems
        );
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
        // The builder passes it on, except out of Testing: that's a tester's (0047).
        let steps: [(&PassOpts, &str, &str, &str, &str); 6] = [
            (&no_gate(), "wt-a", "shaping", "todo", ""),
            (&no_gate(), "wt-a", "ready", "todo", ""),
            (&no_gate(), "wt-a", "building", "doing", "wt-a"),
            (&gate, "wt-a", "testing", "doing", ""),
            (&no_gate(), "qa", "your-check", "doing", ""),
            (&done, "wt-a", "done", "done", ""),
        ];
        for (opts, who, stage, folder, claim) in steps {
            let p = pass_task(d.path(), "0001", who, T, opts).unwrap();
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
            block_task(d.path(), "0001", "owner: pick a colour", "o", T).unwrap(),
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
        set_stage(d.path(), "0001", "backlog", None, "owner", T).unwrap();
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
        for bad in [
            "", "soon", "12 days", "owners", "00123", "captured", "owner", "you", "0012",
        ] {
            assert!(
                block_task(d.path(), "0001", bad, "qa", T).is_err(),
                "{bad:?} is refused"
            );
        }
        // `--on owner …` still works: a block on you, with the words as its detail.
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
                t.blocked_kind.as_str(),
                t.claimed_by.as_str()
            ),
            (
                "testing",
                "blocked",
                true,
                "owner: which colour?",
                "you",
                "qa"
            )
        );
        assert!(t.waiting_for_owner && !t.kind_guessed);
        assert!(t
            .log_tail
            .last()
            .unwrap()
            .ends_with("qa: blocked on you: which colour?"));
        let file = fs::read_to_string(d.path().join("blocked/0001-t.md")).unwrap();
        assert!(
            file.contains("\nwaiting_on: owner: which colour?\nblocked-kind: you\n"),
            "{file}"
        );
        assert!(block_task(d.path(), "0001", "you: which colour?", "qa", T)
            .unwrap_err()
            .contains("already blocked"));
        block_task(d.path(), "0001", "outside: Apple review", "qa", T).unwrap();
        assert!(!only(d.path()).waiting_for_owner);
        block_task(d.path(), "0001", "capture, needs a spec", "qa", T).unwrap();
        let t = only(d.path());
        assert_eq!(
            (t.blocked_kind.as_str(), t.waiting_on.as_str()),
            ("question", "capture: needs a spec")
        );
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
            .ends_with("owner: unblocked (was waiting on question: needs a spec)"));
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
        set_stage(d.path(), "0001", "shaping", None, "owner", T).unwrap();
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
        assert!(set_stage(d.path(), "0001", "building", None, "owner", T)
            .unwrap_err()
            .contains("already in Building"));
        assert!(set_stage(d.path(), "0001", "qa", None, "owner", T)
            .unwrap_err()
            .contains("stage must be one of"));
        assert!(set_stage(d.path(), "0001", "", None, "owner", T)
            .unwrap_err()
            .contains("stage must be one of"));
        assert_eq!(
            set_stage(d.path(), "0001", "Your-Check", None, "owner", T).unwrap(),
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
            set_stage(d.path(), "0001", "done", None, "owner", T).unwrap(),
            "done/0001-t.md"
        );
    }

    #[test]
    fn set_stage_with_from_refuses_a_task_that_moved_on() {
        let d = queue();
        put(
            d.path(),
            "doing/0001-t.md",
            &task_text("status: doing\nclaimed-by: wt-a\nwaiting_on:", ""),
        );
        // Seen in Testing, but it's in Building: refused, nothing written.
        let before = fs::read_to_string(d.path().join("doing/0001-t.md")).unwrap();
        assert_eq!(
            set_stage(d.path(), "0001", "your-check", Some("testing"), "owner", T).unwrap_err(),
            "0001 is now in Building, not Testing; it moved on since you looked"
        );
        // Even when it already sits where the move would put it.
        assert!(
            set_stage(d.path(), "0001", "building", Some("ready"), "owner", T)
                .unwrap_err()
                .contains("it moved on since you looked")
        );
        assert!(set_stage(d.path(), "0001", "ready", Some("qa"), "owner", T)
            .unwrap_err()
            .contains("stage must be one of"));
        assert_eq!(
            fs::read_to_string(d.path().join("doing/0001-t.md")).unwrap(),
            before
        );
        // Seen where it is (any case, padded): it moves, and releases the claim as before.
        set_stage(d.path(), "0001", "testing", Some(" Building "), "owner", T).unwrap();
        let t = only(d.path());
        assert_eq!((t.stage.as_str(), t.claimed_by.as_str()), ("testing", ""));
        // None or empty: the old override, whatever stage it's in.
        set_stage(d.path(), "0001", "ready", None, "owner", T).unwrap();
        set_stage(d.path(), "0001", "shaping", Some(""), "owner", T).unwrap();
        assert_eq!(only(d.path()).stage, "shaping");
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
        move_task(
            d.path(),
            "0001",
            "blocked",
            "outside: the release",
            "owner",
            T,
        )
        .unwrap();
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
    fn two_opposite_depends_at_once_never_make_a_loop() {
        for _ in 0..20 {
            let d = queue();
            for id in ["0001", "0002"] {
                put(
                    d.path(),
                    &format!("todo/{id}-t.md"),
                    &task_text("status: todo", "").replace("id: 0001", &format!("id: {id}")),
                );
            }
            let root = d.path().to_path_buf();
            let go = std::sync::Arc::new(std::sync::Barrier::new(2));
            let runs: Vec<_> = [("0001", "0002"), ("0002", "0001")]
                .into_iter()
                .map(|(id, dep)| {
                    let (root, go) = (root.clone(), go.clone());
                    std::thread::spawn(move || {
                        go.wait();
                        set_field(&root, id, "depends", dep, "capture", T)
                    })
                })
                .collect();
            let results: Vec<_> = runs.into_iter().map(|h| h.join().unwrap()).collect();
            assert_eq!(
                results.iter().filter(|r| r.is_ok()).count(),
                1,
                "{results:?}"
            );
            let refused = results.iter().find_map(|r| r.as_ref().err()).unwrap();
            assert!(refused.contains("would make a loop"), "{refused}");
            assert!(
                state(d.path()).problems.is_empty(),
                "{:?}",
                state(d.path()).problems
            );
        }
    }

    #[test]
    fn claim_and_next_wait_for_what_a_task_depends_on() {
        let d = queue();
        let t = |id: &str, fm: &str| task_text(fm, "").replace("id: 0001", &format!("id: {id}"));
        put(
            d.path(),
            "doing/0001-t.md",
            &t("0001", "status: doing\nstage: building\nclaimed-by: wt-x"),
        );
        put(
            d.path(),
            "todo/0002-t.md",
            &t("0002", "status: todo\npriority: high\ndepends: [0001]"),
        );
        put(
            d.path(),
            "todo/0003-t.md",
            &t("0003", "status: todo\npriority: low"),
        );
        let e = claim_task(d.path(), "0002", "wt-a", T).unwrap_err();
        assert!(e.contains("depends on 0001, not merged yet"), "{e}");
        // next skips the high-priority 0002 and takes 0003.
        assert_eq!(next_task(d.path(), "wt-a", T).unwrap().unwrap().id, "0003");
        // Once 0001 is done, 0002 can be claimed.
        put(
            d.path(),
            "done/0001-t.md",
            &t("0001", "status: done\nstage: done"),
        );
        fs::remove_file(d.path().join("doing/0001-t.md")).unwrap();
        claim_task(d.path(), "0002", "wt-b", T).unwrap();
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
        set_stage(d.path(), "0001", "ready", None, "owner", T).unwrap();
        set_stage(d.path(), "0001", "building", None, "owner", T).unwrap();
        assert_eq!(only(d.path()).claimed_by, "");
        set_stage(d.path(), "0001", "testing", None, "owner", T).unwrap();
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
        block_task(d.path(), "0001", "outside: the API", "qa", T).unwrap();
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
                t.blocked_kind.as_str(),
                t.blocked
            ),
            ("building", "blocked", "blocked", "the API", "outside", true)
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
        set_stage(d.path(), "0001", "ready", None, "owner", T).unwrap();
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
        // As typed, not rounded to one decimal first (0036 review): 1.25 h is 75 minutes.
        assert_eq!(lease_minutes("1.25h").unwrap(), 75);
        assert_eq!(lease_minutes("0.75d").unwrap(), 1080);
        assert!(lease_minutes("-2h").is_err() && lease_minutes("1.2.3h").is_err());
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
        block_task(d.path(), "0001", "owner: pick a colour", "wt-a", T).unwrap();
        claim_task(d.path(), "0002", "wt-a", T).unwrap();
        assert_eq!(
            unblock_task(d.path(), "0001", "wt-a", T).unwrap_err(),
            "you already hold 0002; pass, return or block it first"
        );
        assert_eq!(
            unblock_task(d.path(), "0001", "owner", T).unwrap_err(),
            "wt-a already holds op-2 in Building; one claim per holder, so they pass, return or block it first"
        );
        assert!(task_of(d.path(), "0001").blocked);
        // A return to the builder who now holds another task: refused too.
        claim_task(d.path(), "0003", "wt-b", T).unwrap();
        pass_task(d.path(), "0003", "wt-b", T, &gate()).unwrap();
        claim_task(d.path(), "0004", "wt-b", T).unwrap();
        assert!(return_task(d.path(), "0003", "AC1 fails", None, "qa", T)
            .unwrap_err()
            .starts_with("wt-b already holds op-4 in Building"));
        assert_eq!(task_of(d.path(), "0003").stage, "testing");
        // The owner may hold several.
        pass_task(d.path(), "0005", "owner", T, &no_gate()).unwrap();
        set_stage(d.path(), "0002", "ready", None, "owner", T).unwrap();
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

    /// Create a task with a level, parent and due date, as the owner, at `T`.
    fn new_at(
        d: &Path,
        title: &str,
        level: &str,
        parent: &str,
        due: &str,
    ) -> Result<Created, String> {
        let n = NewTask {
            title: title.into(),
            level: level.into(),
            parent: parent.into(),
            due: due.into(),
            ..Default::default()
        };
        create_task(d, &n, "owner", T)
    }

    #[test]
    fn creates_goals_features_and_tasks_under_their_parents() {
        let d = queue();
        let g = new_at(d.path(), "Ship 1.0", "Goal", "", " 2026-12-01 ").unwrap();
        let f = new_at(d.path(), "Stage track", "feature", "0001", "").unwrap();
        let t = new_at(d.path(), "Pass button", "", "0002", "").unwrap();
        new_at(d.path(), "Release notes", "task", " 0001 ", "").unwrap();
        assert_eq!(
            (g.id.as_str(), f.id.as_str(), t.id.as_str()),
            ("0001", "0002", "0003")
        );
        let text = fs::read_to_string(d.path().join(&g.path)).unwrap();
        assert!(
            text.contains("\ncommit:\nlevel: goal\ndue: 2026-12-01\n---\n"),
            "{text}"
        );
        let text = fs::read_to_string(d.path().join(&f.path)).unwrap();
        assert!(
            text.contains("\ncommit:\nlevel: feature\nparent: 0001\n---\n"),
            "{text}"
        );
        // A task writes no level (none is a task).
        let text = fs::read_to_string(d.path().join(&t.path)).unwrap();
        assert!(text.contains("\ncommit:\nparent: 0002\n---\n"), "{text}");
        let s = state(d.path());
        assert!(s.problems.is_empty(), "{:?}", s.problems);
        let got: Vec<_> = s
            .tasks
            .iter()
            .map(|t| format!("{} {} {} {}", t.id, t.level, t.parent, t.due))
            .collect();
        assert_eq!(
            got,
            [
                "0001 goal  2026-12-01",
                "0002 feature 0001 ",
                "0003 task 0002 ",
                "0004 task 0001 "
            ]
        );
        assert_eq!(s.tasks[0].progress.as_ref().map(|p| p.total), Some(2));
    }

    #[test]
    fn create_refuses_a_parent_or_due_against_the_rules() {
        let d = queue();
        new_at(d.path(), "Goal", "goal", "", "").unwrap();
        new_at(d.path(), "Feature", "feature", "0001", "").unwrap();
        new_at(d.path(), "Task", "", "0002", "").unwrap();
        let before: Vec<_> = queue::task_files(d.path());
        for (level, parent, due, want) in [
            (
                "feature",
                "0002",
                "",
                "a feature's parent is a goal; 0002 is a feature",
            ),
            (
                "task",
                "0003",
                "",
                "a task's parent is a feature or a goal; 0003 is a task",
            ),
            ("goal", "0001", "", "a goal has no parent"),
            ("task", "0099", "", "no task 0099"),
            (
                "task",
                "30",
                "",
                "parent must be a task id (four digits), got \"30\"",
            ),
            ("epic", "", "", "level must be one of goal, feature, task"),
            (
                "feature",
                "0001",
                "2026-12-01",
                "only a goal has a due date",
            ),
            (
                "goal",
                "",
                "soon",
                "due must be a date (YYYY-MM-DD), got \"soon\"",
            ),
            ("goal", "", "2026-02-30", "due must be a date"),
        ] {
            let e = new_at(d.path(), "x", level, parent, due).unwrap_err();
            assert!(e.contains(want), "{level} {parent} {due}: {e}");
        }
        assert_eq!(queue::task_files(d.path()), before, "nothing written");
    }

    #[test]
    fn sets_level_parent_and_due_by_the_same_rules() {
        let d = queue();
        new_at(d.path(), "Goal", "goal", "", "").unwrap();
        new_at(d.path(), "Feature", "feature", "0001", "").unwrap();
        new_at(d.path(), "Task", "", "0002", "").unwrap();
        new_at(d.path(), "Loose", "", "", "").unwrap();
        let set =
            |id: &str, field: &str, value: &str| set_field(d.path(), id, field, value, "owner", T);
        set("0004", "parent", "0002").unwrap();
        assert_eq!(task_of(d.path(), "0004").parent, "0002");
        assert!(
            file_of(d.path(), "0004").ends_with("- 2026-10-12 14:05 owner: parent set to op-2\n")
        );
        set("0004", "parent", "").unwrap();
        assert_eq!(task_of(d.path(), "0004").parent, "");
        assert!(file_of(d.path(), "0004").ends_with("- 2026-10-12 14:05 owner: parent cleared\n"));
        set("0004", "level", "Feature").unwrap();
        set("0004", "parent", "0001").unwrap();
        assert_eq!(
            (
                task_of(d.path(), "0004").level,
                task_of(d.path(), "0004").parent
            ),
            ("feature".to_string(), "0001".to_string())
        );
        set("0001", "due", "2027-01-15").unwrap();
        assert_eq!(task_of(d.path(), "0001").due, "2027-01-15");
        assert!(file_of(d.path(), "0001").ends_with("owner: due set to 2027-01-15\n"));
        assert!(
            state(d.path()).problems.is_empty(),
            "{:?}",
            state(d.path()).problems
        );
        let files: Vec<_> = ["0001", "0002", "0003", "0004"]
            .iter()
            .map(|id| file_of(d.path(), id))
            .collect();
        for (id, field, value, want) in [
            ("0003", "due", "2026-12-01", "only a goal has a due date; 0003 is a task"),
            ("0001", "level", "feature", "clear 0001's due date first; only a goal has one"),
            ("0002", "level", "task", "can't make 0002 a task: 0003 (task) is under it, and a task's parent is a feature or a goal"),
            ("0003", "level", "goal", "can't make 0003 a goal: it's under 0002, and a goal has no parent"),
            ("0003", "level", "feature", "can't make 0003 a feature: it's under 0002, and a feature's parent is a goal"),
            ("0003", "level", "epic", "level must be one of goal, feature, task"),
            ("0003", "level", "", "level is empty"),
            ("0001", "parent", "0003", "a goal has no parent"),
            ("0003", "parent", "0003", "0003 can't be its own parent"),
            ("0003", "parent", "0099", "no task 0099"),
            ("0003", "parent", "0004x", "parent must be a task id"),
            ("0004", "parent", "0002", "a feature's parent is a goal; 0002 is a feature"),
            ("0003", "due", "tomorrow", "due must be a date"),
            ("0009", "parent", "0001", "no task 0009"),
        ] {
            let e = set(id, field, value).unwrap_err();
            assert!(e.contains(want), "{id} {field} {value}: {e}");
        }
        let after: Vec<_> = ["0001", "0002", "0003", "0004"]
            .iter()
            .map(|id| file_of(d.path(), id))
            .collect();
        assert_eq!(after, files, "refusals leave the files alone");
        set("0001", "due", "").unwrap();
        assert_eq!(task_of(d.path(), "0001").due, "");
        // A loop through a parent written by hand is refused too.
        put_task(
            d.path(),
            "todo",
            "0005",
            "level: goal\nparent: 0006\nstage: ready\nstatus: todo",
        );
        put_task(d.path(), "todo", "0006", "stage: ready\nstatus: todo");
        assert_eq!(
            set("0006", "parent", "0005").unwrap_err(),
            "0006 under 0005 would make a loop: 0005 is under 0006"
        );
    }

    #[test]
    fn a_task_in_building_testing_or_held_cant_become_a_feature_or_goal() {
        let d = queue();
        put_task(
            d.path(),
            "doing",
            "0005",
            "priority: normal\nstage: building\nstatus: doing\nclaimed-by: wt-a\nheartbeat:\nwaiting_on:",
        );
        put_task(
            d.path(),
            "doing",
            "0006",
            "priority: normal\nstage: testing\nstatus: doing\nclaimed-by:\nwaiting_on:",
        );
        put_task(
            d.path(),
            "todo",
            "0007",
            "priority: normal\nstage: ready\nstatus: todo\nclaimed-by: wt-b\nwaiting_on:",
        );
        put_task(d.path(), "todo", "0008", &ready_fm("normal"));
        let files: Vec<_> = ["0005", "0006", "0007"]
            .iter()
            .map(|id| file_of(d.path(), id))
            .collect();
        let set = |id: &str, level: &str| set_field(d.path(), id, "level", level, "owner", T);
        assert_eq!(
            set("0005", "feature").unwrap_err(),
            "0005 is in Building (held by wt-a); return or pass it first"
        );
        assert_eq!(
            set("0006", "goal").unwrap_err(),
            "0006 is in Testing; return or pass it first"
        );
        assert_eq!(
            set("0007", "feature").unwrap_err(),
            "0007 is in Ready (held by wt-b); return or pass it first"
        );
        let after: Vec<_> = ["0005", "0006", "0007"]
            .iter()
            .map(|id| file_of(d.path(), id))
            .collect();
        assert_eq!(after, files, "refusals leave the files alone");
        // Staying a task is fine wherever it is; a free task in Ready may change.
        set("0005", "task").unwrap();
        set("0008", "feature").unwrap();
        assert_eq!(task_of(d.path(), "0008").level, "feature");
    }

    #[test]
    fn goals_and_features_are_never_built() {
        let d = queue();
        put_task(
            d.path(),
            "todo",
            "0001",
            &format!("level: feature\n{}", ready_fm("high")),
        );
        put_task(
            d.path(),
            "todo",
            "0002",
            &format!("level: Goal\n{}", ready_fm("high")),
        );
        put_task(
            d.path(),
            "todo",
            "0003",
            &format!("parent: 0001\n{}", ready_fm("low")),
        );
        let before = file_of(d.path(), "0001");
        assert_eq!(
            claim_task(d.path(), "0001", "wt-a", T).unwrap_err(),
            "0001 is a feature; its tasks are what agents build"
        );
        assert_eq!(
            pass_task(d.path(), "0002", "owner", T, &no_gate()).unwrap_err(),
            "0002 is a goal; its tasks are what agents build"
        );
        assert_eq!(file_of(d.path(), "0001"), before);
        // next passes over them for the task under them, though they come first by priority.
        assert_eq!(
            next_task(d.path(), "wt-a", T)
                .unwrap()
                .map(|c| c.id)
                .as_deref(),
            Some("0003")
        );
        assert_eq!(next_task(d.path(), "wt-b", T).unwrap(), None);
        // They're shaped like tasks, and the owner may still set their stage.
        put_task(
            d.path(),
            "inbox",
            "0004",
            "level: feature\npriority: normal\nstage: backlog\nstatus: inbox",
        );
        pass_task(d.path(), "0004", "capture", T, &no_gate()).unwrap();
        assert_eq!(task_of(d.path(), "0004").stage, "shaping");
        set_stage(d.path(), "0001", "your-check", None, "owner", T).unwrap();
        let s = state(d.path());
        let shown = |id: &str| {
            s.tasks
                .iter()
                .find(|t| t.id == id)
                .map(|t| t.shown_stage.clone())
        };
        assert_eq!(
            shown("0001").as_deref(),
            Some("your-check"),
            "the owner's override"
        );
        assert_eq!(shown("0002").as_deref(), Some("ready"));
        assert_eq!(shown("0004").as_deref(), Some("shaping"));
    }

    // Task 0030: the session stamped on a claim, a holder for Testing and Your check, check-ins.

    const A: &str = "wt-a (wt-0030 · session 7b5e8856 · cc-hg)";

    #[test]
    fn a_claim_records_the_session_and_its_holder_is_matched_by_label_and_session() {
        let d = queue();
        for id in ["0001", "0002", "0003"] {
            put_task(d.path(), "todo", id, &ready_fm("normal"));
        }
        claim_task(d.path(), "0001", A, T).unwrap();
        let t = task_of(d.path(), "0001");
        assert_eq!((t.claimed_by.as_str(), t.heartbeat.as_str()), (A, T));
        assert!(file_of(d.path(), "0001")
            .ends_with(&format!("- {T} {A}: stage ready → building (claimed)\n")));
        // The same session from another folder is the same holder; so is its bare label.
        let elsewhere = "wt-a (main · session 7b5e8856 · cc-hg)";
        heartbeat_task(d.path(), "0001", elsewhere, "2026-10-12 14:10").unwrap();
        heartbeat_task(d.path(), "0001", "wt-a", "2026-10-12 14:11").unwrap();
        assert!(claim_task(d.path(), "0002", elsewhere, T)
            .unwrap_err()
            .contains("you already hold 0001"));
        // Another session under the same label is someone else: it can't renew or pass the claim.
        let other = "wt-a (session 11111111)";
        assert_eq!(
            heartbeat_task(d.path(), "0001", other, T).unwrap_err(),
            format!("0001 is held by {A}, not {other}")
        );
        assert!(pass_task(d.path(), "0001", other, T, &gate())
            .unwrap_err()
            .contains("0001 is claimed by"));
        claim_task(d.path(), "0002", other, T).unwrap();
        // The owner, from a session too, may hold several.
        let owner = "owner (session 22222222)";
        put_task(d.path(), "todo", "0004", &ready_fm("normal"));
        claim_task(d.path(), "0003", owner, T).unwrap();
        claim_task(d.path(), "0004", owner, T).unwrap();
        // A holder's note keeps the claim alive from any folder of its session.
        add_note(d.path(), "0001", "phase 1", elsewhere, "2026-10-12 14:30").unwrap();
        assert_eq!(task_of(d.path(), "0001").heartbeat, "2026-10-12 14:30");
        // Its pass out of Building, and a return to it, go back to the builder: here the exact
        // session, since another wt-a session (`other`) holds 0002 in Building.
        pass_task(d.path(), "0001", elsewhere, T, &gate()).unwrap();
        return_task(d.path(), "0001", "AC1 fails", None, "qa", T).unwrap();
        assert_eq!(task_of(d.path(), "0001").claimed_by, A);
        // With no other wt-a session holding work, a return goes to the label, so a new session
        // under it carries on even when the first has ended.
        pass_task(d.path(), "0002", other, T, &gate()).unwrap();
        pass_task(d.path(), "0001", elsewhere, T, &gate()).unwrap();
        return_task(d.path(), "0001", "AC1 fails again", None, "qa", T).unwrap();
        assert_eq!(task_of(d.path(), "0001").claimed_by, "wt-a");
        let next_session = "wt-a (session 33333333)";
        heartbeat_task(d.path(), "0001", next_session, "2026-10-12 15:00").unwrap();
        pass_task(d.path(), "0001", next_session, T, &gate()).unwrap();
        assert!(state(d.path()).problems.is_empty());
    }

    #[test]
    fn a_stamp_never_lets_who_run_past_its_label_cap_or_forge_the_log() {
        let d = queue();
        put_task(d.path(), "todo", "0001", &ready_fm("normal"));
        let label = "w".repeat(100);
        let long = format!(
            "{label} (main-29-is-a-long-name-of-32-ch · wt-0030 · session 7b5e8856 · cc-hg)"
        );
        claim_task(d.path(), "0001", &long, T).unwrap();
        assert_eq!(task_of(d.path(), "0001").claimed_by, long);
        let too_long = format!("w{long}");
        assert!(heartbeat_task(d.path(), "0001", &too_long, T)
            .unwrap_err()
            .contains("longer than 100"));
        for who in [
            "x: check-in 50% · done",
            "wt-a (session 7b5e8856): stage ready → done",
        ] {
            assert!(
                add_note(d.path(), "0001", "hi", who, T)
                    .unwrap_err()
                    .contains("who"),
                "{who}"
            );
        }
    }

    #[test]
    fn pass_gives_testing_to_whoever_passes_it_there_and_your_check_to_the_owner() {
        let d = queue();
        put(
            d.path(),
            "todo/0001-t.md",
            &task_text("status: todo\nclaimed-by:\nwaiting_on:", TICKED),
        );
        let held = |d: &Path| {
            let t = only(d);
            (
                t.stage.clone(),
                t.claimed_by.clone(),
                t.held_by.clone(),
                t.holder().to_string(),
            )
        };
        let s = |a: &str, b: &str, c: &str, e: &str| {
            (a.to_string(), b.to_string(), c.to_string(), e.to_string())
        };
        pass_task(d.path(), "0001", A, "2026-10-12 10:00", &no_gate()).unwrap();
        assert_eq!(held(d.path()), s("building", A, "", A));
        assert!(
            !fs::read_to_string(d.path().join("doing/0001-t.md"))
                .unwrap()
                .contains("held-by"),
            "no field until it's needed"
        );
        pass_task(d.path(), "0001", A, "2026-10-12 11:00", &gate()).unwrap();
        assert_eq!(held(d.path()), s("testing", "", A, A));
        assert_eq!(only(d.path()).stage_since, "2026-10-12 11:00");
        // A block and an unblock keep the holder; a pass by anyone moves on (who may pass is
        // unchanged: Testing isn't a claim).
        block_task(d.path(), "0001", "owner: which port?", "owner", T).unwrap();
        unblock_task(d.path(), "0001", "owner", T).unwrap();
        assert_eq!(held(d.path()), s("testing", "", A, A));
        pass_task(d.path(), "0001", "qa", T, &no_gate()).unwrap();
        assert_eq!(held(d.path()), s("your-check", "", "owner", "owner"));
        // Sent back to Testing, it goes to whoever passed it there last, not the sender.
        return_task(d.path(), "0001", "AC2 fails on the Mac", None, "owner", T).unwrap();
        assert_eq!(held(d.path()), s("testing", "", A, A));
        // Back to Building: the claim again (to the builder's label, as no other wt-a session
        // holds work), and no Testing holder left behind.
        return_task(d.path(), "0001", "rework", None, "qa", T).unwrap();
        assert_eq!(held(d.path()), s("building", "wt-a", "", "wt-a"));
        pass_task(d.path(), "0001", A, T, &gate()).unwrap();
        pass_task(d.path(), "0001", "qa", T, &no_gate()).unwrap();
        pass_task(d.path(), "0001", "owner", T, &no_gate()).unwrap();
        assert_eq!(held(d.path()), s("done", "", "", ""));
        // The owner's override into Testing: back to whoever last passed it there.
        set_stage(d.path(), "0001", "ready", None, "owner", T).unwrap();
        set_stage(d.path(), "0001", "testing", None, "owner", T).unwrap();
        assert_eq!(held(d.path()).2, A, "the last tester");
        assert!(state(d.path()).problems.is_empty());
    }

    #[test]
    fn a_check_in_is_the_holders_heartbeat_with_words() {
        let d = queue();
        put_task(d.path(), "todo", "0001", &ready_fm("normal"));
        put_task(d.path(), "todo", "0002", &ready_fm("normal"));
        claim_task(d.path(), "0001", A, "2026-10-12 09:00").unwrap();
        assert_eq!(
            checkin_task(
                d.path(),
                "0001",
                "gate green, writing tests",
                "60",
                A,
                "2026-10-12 10:00"
            )
            .unwrap(),
            "doing/0001-t.md"
        );
        let t = task_of(d.path(), "0001");
        assert_eq!(t.heartbeat, "2026-10-12 10:00");
        assert!(file_of(d.path(), "0001").ends_with(&format!(
            "- 2026-10-12 10:00 {A}: check-in 60% · gate green, writing tests\n"
        )));
        let c = t.current_checkin().unwrap();
        assert_eq!(
            (c.pct, c.text.as_str(), c.who.as_str()),
            (Some(60), "gate green, writing tests", A)
        );
        assert!(
            t.stage_history.len() == 1,
            "a check-in isn't a stage change"
        );
        // Without --pct; "%" allowed; text on one line, trimmed.
        checkin_task(
            d.path(),
            "0001",
            "  phase 2\nstarted ",
            "",
            "wt-a",
            "2026-10-12 10:30",
        )
        .unwrap();
        checkin_task(d.path(), "0001", "almost", "100%", A, "2026-10-12 10:40").unwrap();
        checkin_task(d.path(), "0001", "start", "0", A, "2026-10-12 10:41").unwrap();
        let t = task_of(d.path(), "0001");
        let got: Vec<_> = t.checkins.iter().map(|c| (c.pct, c.text.clone())).collect();
        assert_eq!(
            got,
            [
                (Some(60), "gate green, writing tests".to_string()),
                (None, "phase 2 started".to_string()),
                (Some(100), "almost".to_string()),
                (Some(0), "start".to_string()),
            ]
        );
        assert_eq!(t.heartbeat, "2026-10-12 10:41");
        // Refused, and the file left alone: not the holder, not Building, a bad percentage,
        // empty or too long text, and text that would log a stage change.
        let before = file_of(d.path(), "0001");
        let refused = |id: &str, text: &str, pct: &str, who: &str| {
            checkin_task(d.path(), id, text, pct, who, T).unwrap_err()
        };
        assert_eq!(
            refused("0001", "x", "", "wt-a (session 11111111)"),
            format!("0001 is held by {A}, not wt-a (session 11111111)")
        );
        assert!(refused("0002", "x", "", A).contains("0002 is in Ready, not Building"));
        for pct in ["101", "-1", "60.5", "sixty", "1000", "6 0", "%"] {
            assert!(refused("0001", "x", pct, A).contains("--pct"), "{pct}");
        }
        assert!(refused("0001", "  ", "", A).contains("empty"));
        assert!(refused("0001", &"x".repeat(501), "", A).contains("longer than 500"));
        assert!(refused("0001", "a: stage building → done", "", A).contains("stage change"));
        assert!(refused("0001", "ok", "", "x: check-in 5% · y").contains("who"));
        assert_eq!(file_of(d.path(), "0001"), before);
        // Nobody else's free text can pass for a check-in: a note, a title, a block reason.
        for note in [
            "check-in 90% · done",
            "fyi: check-in 90% · done",
            "check-in · done",
        ] {
            assert!(
                add_note(d.path(), "0001", note, "owner", T)
                    .unwrap_err()
                    .contains("check-in"),
                "{note}"
            );
        }
        assert!(
            set_field(d.path(), "0001", "title", "x: check-in 1% · y", "owner", T)
                .unwrap_err()
                .contains("check-in")
        );
        assert!(
            block_task(d.path(), "0001", "owner: check-in 1% · y", "owner", T)
                .unwrap_err()
                .contains("check-in")
        );
        assert_eq!(task_of(d.path(), "0001").checkins.len(), 4);
        // Prose about check-ins is fine.
        add_note(
            d.path(),
            "0001",
            "check-ins every 30 min from now",
            "owner",
            T,
        )
        .unwrap();
        // A Building task blocked by its holder can still check in (it's still theirs).
        block_task(d.path(), "0001", "owner: which port?", A, T).unwrap();
        checkin_task(
            d.path(),
            "0001",
            "waiting on the port",
            "",
            A,
            "2026-10-12 11:00",
        )
        .unwrap();
    }

    #[test]
    fn a_check_in_racing_a_reclaim_lands_before_it_or_is_refused() {
        let now = crate::queue::now_secs();
        for _ in 0..20 {
            let d = queue();
            put_task(
                d.path(),
                "doing",
                "0001",
                &format!("priority: normal\nstage: building\nstatus: doing\nclaimed-by: {A}\nheartbeat: 2026-10-12 01:00\nwaiting_on:"),
            );
            let root = d.path().to_path_buf();
            let results = at_once(&["checkin", "reclaim"], move |how| match how {
                "checkin" => checkin_task(&root, "0001", "still here", "", A, T).map(|_| 0),
                _ => reclaim(&root, 720, "capture", T, now).map(|r| r.released.len()),
            });
            let t = task_of(d.path(), "0001");
            match (&results[0], &results[1]) {
                (Ok(_), Ok(0)) => assert_eq!(
                    (t.stage.as_str(), t.heartbeat.as_str(), t.checkins.len()),
                    ("building", T, 1)
                ),
                (Err(e), Ok(1)) => {
                    assert!(e.contains("not Building"), "{e}");
                    assert_eq!((t.stage.as_str(), t.checkins.len()), ("ready", 0));
                }
                other => panic!("{other:?}"),
            }
        }
    }

    // Task 0047: the tester isn't the builder.

    /// Task 0001 with `criteria`, claimed by `builder` at 10:00 and passed into Testing at 11:00.
    fn built_with(d: &Path, builder: &str, criteria: &str) {
        put(
            d,
            "todo/0001-t.md",
            &task_text("status: todo\nclaimed-by:\nwaiting_on:", criteria),
        );
        pass_task(d, "0001", builder, "2026-10-12 10:00", &no_gate()).unwrap();
        pass_task(d, "0001", builder, "2026-10-12 11:00", &gate()).unwrap();
    }
    fn testing_file(d: &Path) -> String {
        fs::read_to_string(d.join("doing/0001-t.md")).unwrap()
    }
    fn from_testing() -> PassOpts {
        PassOpts {
            from: Some("testing".into()),
            ..Default::default()
        }
    }

    #[test]
    fn pass_from_testing_refuses_the_builder_and_writes_nothing() {
        let d = queue();
        built_with(d.path(), A, TICKED);
        let before = testing_file(d.path());
        let why = "0001 was built by wt-a (session 7b5e8856); a different session tests it: run the QA pass with its own --as (e.g. qa-0001), or the owner passes it";
        // The builder's session, from any folder, and its bare label (a terminal with no stamp).
        for who in [A, "wt-a (main · session 7b5e8856 · cc-hg)", "wt-a"] {
            assert_eq!(
                pass_task(d.path(), "0001", who, T, &from_testing()).unwrap_err(),
                why,
                "{who}"
            );
            assert_eq!(testing_file(d.path()), before, "{who}");
        }
        // Without --from too.
        assert_eq!(
            pass_task(d.path(), "0001", A, T, &no_gate()).unwrap_err(),
            why
        );
        assert_eq!(testing_file(d.path()), before);
    }

    #[test]
    fn an_unstamped_builder_is_its_label_in_any_session() {
        let d = queue();
        let builder = "muster/wt-0042 (deps)";
        built_with(d.path(), builder, TICKED);
        let before = testing_file(d.path());
        for who in [builder, "muster/wt-0042 (deps) (session 7b5e8856)"] {
            assert_eq!(
                pass_task(d.path(), "0001", who, T, &from_testing()).unwrap_err(),
                "0001 was built by muster/wt-0042 (deps); a different session tests it: run the QA pass with its own --as (e.g. muster/qa-0001), or the owner passes it",
                "{who}"
            );
        }
        assert_eq!(testing_file(d.path()), before);
        pass_task(d.path(), "0001", "muster/qa-0001", T, &from_testing()).unwrap();
        assert_eq!(only(d.path()).stage, "your-check");
    }

    #[test]
    fn a_builder_sent_back_that_passes_again_still_cant_test_it() {
        let d = queue();
        built_with(d.path(), A, TICKED);
        return_task(
            d.path(),
            "0001",
            "AC2 fails",
            None,
            "qa",
            "2026-10-12 12:00",
        )
        .unwrap();
        pass_task(d.path(), "0001", A, "2026-10-12 13:00", &gate()).unwrap();
        let e = pass_task(d.path(), "0001", A, T, &from_testing()).unwrap_err();
        assert!(
            e.starts_with("0001 was built by wt-a (session 7b5e8856);"),
            "{e}"
        );
        // Sent back again, a new session under the label carries on: it built this round, so it
        // can't test it either, and nor can the session that claimed it.
        return_task(
            d.path(),
            "0001",
            "AC2 still fails",
            None,
            "qa",
            "2026-10-12 14:00",
        )
        .unwrap();
        let next = "wt-a (session 33333333)";
        pass_task(d.path(), "0001", next, "2026-10-12 15:00", &gate()).unwrap();
        let before = testing_file(d.path());
        let e = pass_task(d.path(), "0001", next, T, &from_testing()).unwrap_err();
        assert!(
            e.starts_with("0001 was built by wt-a (session 33333333);"),
            "{e}"
        );
        assert!(pass_task(d.path(), "0001", A, T, &from_testing()).is_err());
        assert_eq!(testing_file(d.path()), before);
        pass_task(
            d.path(),
            "0001",
            "qa-0001 (session 99999999)",
            T,
            &from_testing(),
        )
        .unwrap();
        assert_eq!(only(d.path()).stage, "your-check");
    }

    #[test]
    fn another_label_tests_it_and_the_log_names_builder_and_tester() {
        let d = queue();
        built_with(d.path(), A, TICKED);
        let qa = "qa-0001 (session 99999999)";
        pass_task(d.path(), "0001", qa, T, &from_testing()).unwrap();
        let t = only(d.path());
        assert_eq!(
            (t.stage.as_str(), t.built_by.as_str(), t.tested_by.as_str()),
            ("your-check", A, qa)
        );
        let text = fs::read_to_string(d.path().join(&t.path)).unwrap();
        assert!(
            text.contains(&format!(
                "- 2026-10-12 10:00 {A}: stage ready → building (claimed)\n"
            )) && text.ends_with(&format!("- {T} {qa}: stage testing → your-check\n")),
            "{text}"
        );
        // Another session under the builder's label is another holder (`session::same_holder`).
        let d = queue();
        built_with(d.path(), A, TICKED);
        pass_task(
            d.path(),
            "0001",
            "wt-a (session 11111111)",
            T,
            &from_testing(),
        )
        .unwrap();
        assert_eq!(only(d.path()).stage, "your-check");
    }

    #[test]
    fn another_label_in_the_builders_session_is_accepted_and_the_log_says_so() {
        let d = queue();
        built_with(d.path(), A, OWNERS);
        let sub = "muster/qa-0001 (session 7b5e8856 · cc-hg)";
        pass_task(d.path(), "0001", sub, T, &from_testing()).unwrap();
        let t = only(d.path());
        assert_eq!(
            (t.stage.as_str(), t.tested_by.as_str()),
            ("your-check", sub)
        );
        let text = fs::read_to_string(d.path().join(&t.path)).unwrap();
        assert!(
            text.ends_with(&format!("- {T} {sub}: stage testing → your-check (2 criteria for the owner to verify; tested under another label in the builder's session)\n")),
            "{text}"
        );
        assert!(state(d.path()).problems.is_empty());
    }

    #[test]
    fn the_owner_passes_without_a_tester_and_the_log_says_so() {
        let d = queue();
        built_with(d.path(), A, TICKED);
        pass_task(d.path(), "0001", "owner", T, &from_testing()).unwrap();
        let t = only(d.path());
        assert_eq!(t.tested_by, "owner");
        let file = |d: &Path| fs::read_to_string(d.join(&only(d).path)).unwrap();
        assert!(
            file(d.path()).ends_with(&format!(
                "- {T} owner: stage testing → your-check (owner's override: no separate tester)\n"
            )),
            "{}",
            file(d.path())
        );
        // `muster stage <id> your-check`, the owner's override, from a session too.
        return_task(d.path(), "0001", "look again", None, "owner", T).unwrap();
        let owner = "owner (session 22222222)";
        set_stage(d.path(), "0001", "your-check", Some("testing"), owner, T).unwrap();
        assert!(
            file(d.path()).ends_with(&format!(
                "- {T} {owner}: stage testing → your-check (set; owner's override: no separate tester)\n"
            )),
            "{}",
            file(d.path())
        );
        assert!(state(d.path()).problems.is_empty());
    }

    #[test]
    fn the_builder_cant_skip_its_tester_with_the_override() {
        let d = queue();
        built_with(d.path(), A, TICKED);
        let before = testing_file(d.path());
        for to in ["your-check", "done"] {
            let e = set_stage(d.path(), "0001", to, None, A, T).unwrap_err();
            assert!(
                e.starts_with("0001 was built by wt-a (session 7b5e8856);"),
                "{e}"
            );
        }
        assert_eq!(testing_file(d.path()), before);
        // From Building as well, once it's back there.
        return_task(d.path(), "0001", "rework", None, "qa", T).unwrap();
        assert!(set_stage(d.path(), "0001", "your-check", None, "wt-a", T).is_err());
        // Moving it anywhere else, and anyone else's override, are as before.
        set_stage(d.path(), "0001", "testing", None, "capture", T).unwrap();
        set_stage(d.path(), "0001", "your-check", None, "capture", T).unwrap();
        let t = only(d.path());
        let text = fs::read_to_string(d.path().join(&t.path)).unwrap();
        assert!(
            text.ends_with(&format!(
                "- {T} capture: stage testing → your-check (set)\n"
            )),
            "{text}"
        );
    }

    #[test]
    fn whoever_gated_it_into_testing_is_a_builder_and_the_task_says_who() {
        let d = queue();
        built_with(d.path(), A, TICKED);
        assert_eq!(only(d.path()).gated_by, A);
        // Put back in Building by the owner's override with nobody's claim, then gated into
        // Testing by wt-b: wt-b built this round, though it never claimed it.
        set_stage(d.path(), "0001", "ready", None, "owner", T).unwrap();
        set_stage(d.path(), "0001", "building", None, "owner", T).unwrap();
        assert_eq!(only(d.path()).claimed_by, "");
        pass_task(d.path(), "0001", "wt-b", "2026-10-12 15:00", &gate()).unwrap();
        let t = only(d.path());
        assert_eq!(
            (t.built_by.as_str(), t.gated_by.as_str(), t.held_by.as_str()),
            (A, "wt-b", "wt-b")
        );
        let before = testing_file(d.path());
        let e = pass_task(d.path(), "0001", "wt-b", T, &from_testing()).unwrap_err();
        assert!(e.starts_with("0001 was built by wt-b;"), "{e}");
        assert_eq!(testing_file(d.path()), before);
        // An override into Testing gates nothing: the last gated passer stays.
        return_task(d.path(), "0001", "again", Some("building"), "qa", T).unwrap();
        set_stage(d.path(), "0001", "testing", None, "capture", T).unwrap();
        assert_eq!(only(d.path()).gated_by, "wt-b");
    }

    #[test]
    fn the_builder_cant_move_it_to_done_and_the_owners_move_is_the_override() {
        let d = queue();
        built_with(d.path(), A, TICKED);
        let before = testing_file(d.path());
        for who in [A, "wt-a"] {
            let e = move_task(d.path(), "0001", "done", "", who, T).unwrap_err();
            assert!(e.starts_with("0001 was built by wt-a"), "{who}: {e}");
            assert_eq!(testing_file(d.path()), before, "{who}");
        }
        move_task(d.path(), "0001", "done", "", "owner", T).unwrap();
        let t = only(d.path());
        let text = fs::read_to_string(d.path().join(&t.path)).unwrap();
        assert!(
            text.ends_with(&format!(
                "- {T} owner: stage testing → done (set; owner's override: no separate tester)\n"
            )),
            "{text}"
        );
    }

    #[test]
    fn with_no_builder_on_record_anyone_passes_and_nothing_is_added() {
        let d = queue();
        put(
            d.path(),
            "doing/0001-t.md",
            &task_text(
                "status: doing\nstage: testing\nclaimed-by:\nwaiting_on:",
                TICKED,
            ),
        );
        pass_task(d.path(), "0001", "owner", T, &from_testing()).unwrap();
        let t = only(d.path());
        assert_eq!((t.built_by.as_str(), t.tested_by.as_str()), ("", "owner"));
        let text = fs::read_to_string(d.path().join(&t.path)).unwrap();
        assert!(
            text.ends_with(&format!("- {T} owner: stage testing → your-check\n")),
            "{text}"
        );
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
        // Its prefix stored (0033), so nothing here hangs on the projects this Mac knows.
        fs::write(d.path().join("README.md"), "---\nprefix: oq\n---\nq").unwrap();
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
        refused(
            "# T\n\n- 2026-10-12 15:00 evil: check-in 90% · done",
            "check-in",
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
    fn the_holder_is_matched_by_label_and_session_when_it_edits() {
        let d = queue();
        let a = "wt-a (wt-0030 · session 7b5e8856 · cc-hg)";
        let held = format!(
            "stage: building\nstatus: doing\nclaimed-by: {a}\nheartbeat: 2026-10-12 09:00\nwaiting_on:"
        );
        let p = put(d.path(), "doing", &held, WRITE_UP, "");
        let before = text(&p);
        // Another session under the same label is someone else.
        let other = "wt-a (session 11111111)";
        let e = open_body(d.path(), "0001", other).unwrap_err();
        assert!(e.contains(&format!("{a} holds it")), "{e}");
        let e = edit_body(d.path(), "0001", "# Edit me\n\nMine.", WRITE_UP, other, T).unwrap_err();
        assert!(e.contains(&format!("{a} holds it")), "{e}");
        assert_eq!(text(&p), before);
        // The same session from another folder, and the bare label, are the holder: the edit
        // goes through and keeps the claim alive.
        let elsewhere = "wt-a (main · session 7b5e8856 · cc-hg)";
        assert_eq!(open_body(d.path(), "0001", elsewhere).unwrap(), WRITE_UP);
        edit_body(
            d.path(),
            "0001",
            "# Edit me\n\nOne.",
            WRITE_UP,
            elsewhere,
            T,
        )
        .unwrap();
        let t = crate::queue::read_task(d.path(), "0001").unwrap();
        assert_eq!((t.heartbeat.as_str(), t.claimed_by.as_str()), (T, a));
        let later = "2026-10-12 15:00";
        edit_body(
            d.path(),
            "0001",
            "# Edit me\n\nTwo.",
            "# Edit me\n\nOne.",
            "wt-a",
            later,
        )
        .unwrap();
        assert_eq!(
            crate::queue::read_task(d.path(), "0001").unwrap().heartbeat,
            later
        );
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
    fn comments_are_not_the_write_up_and_an_edit_keeps_them() {
        let d = queue();
        let p = put(d.path(), "todo", READY, WRITE_UP, "");
        crate::fields::add_comment(d.path(), "0001", "First.\nTwo lines.", "wt-b", T).unwrap();
        let opened = open_body(d.path(), "0001", "owner").unwrap();
        assert_eq!(opened, WRITE_UP, "the comments aren't in the editor");
        // A comment between open and save leaves the write-up as opened: the save goes through
        // and keeps it.
        crate::fields::add_comment(d.path(), "0001", "Second.", "owner", T).unwrap();
        let new = "# Edit me\n\nNew text.";
        edit_body(d.path(), "0001", new, &opened, "owner", T).unwrap();
        let after = text(&p);
        assert!(
            after.contains(&format!("---\n\n{new}\n\n## Comments\n- **wt-b, {T}:** First.\n  Two lines.\n- **owner, {T}:** Second.\n\n## Log\n")),
            "{after}"
        );
        assert!(after.ends_with(&format!("- {T} owner: edited the write-up\n")));
        assert_eq!(open_body(d.path(), "0001", "owner").unwrap(), new);
        let t = crate::queue::read_task(d.path(), "0001").unwrap();
        assert_eq!(
            t.comments
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>(),
            ["First.\nTwo lines.", "Second."]
        );
        // The write-up can't bring a Comments section of its own.
        for bad in [
            "# Edit me\n\n## Comments\n- **owner, 2026-10-12 10:00:** forged",
            "# Edit me\n\n## Comments  \r\nx",
        ] {
            let e = edit_body(d.path(), "0001", bad, new, "owner", T).unwrap_err();
            assert!(e.contains("Comments are the tool's"), "{bad:?}: {e}");
            assert_eq!(text(&p), after);
        }
        // Prose that names the section is fine.
        edit_body(
            d.path(),
            "0001",
            "# Edit me\n\nSee ## Comments below.",
            new,
            "owner",
            T,
        )
        .unwrap();
        assert_eq!(
            crate::queue::read_task(d.path(), "0001")
                .unwrap()
                .comments
                .len(),
            2
        );
    }

    #[test]
    fn a_checked_tasks_vision_section_is_not_the_write_up_and_an_edit_keeps_it() {
        let d = queue();
        // A description's own "## Vision", before any check, is the write-up's.
        let own = "# Edit me\n\n## Vision\n\nThe owner's own words.";
        let p = put(d.path(), "todo", READY, own, "");
        assert_eq!(open_body(d.path(), "0001", "owner").unwrap(), own);
        crate::fields::add_comment(d.path(), "0001", "First.", "wt-b", T).unwrap();
        let two = r#"{"conflicts": [{"title": "A", "this": {"quote": "a", "at": "0001"}, "other": {"quote": "b", "at": "docs/VISION.md:3"}, "fix": "f"}, {"title": "B", "this": {"quote": "c", "at": "0001"}, "other": {"quote": "d", "at": "docs/DESIGN.md:9"}, "fix": "g"}]}"#;
        crate::vision::record(d.path(), "0001", two, "wt-shape", T).unwrap();
        // Once checked, the tool's section is out of the editor; the owner's own stays in.
        let opened = open_body(d.path(), "0001", "owner").unwrap();
        assert_eq!(opened, own, "the tool's Vision section isn't in the editor");
        let new = "# Edit me\n\n## Vision\n\nThe owner's words, edited.";
        edit_body(d.path(), "0001", new, &opened, "owner", T).unwrap();
        let after = text(&p);
        assert!(
            after.contains(&format!("{new}\n\n## Vision\n\nChecked ")),
            "{after}"
        );
        let (v, c, l) = (
            after.rfind("\n## Vision\n").unwrap(),
            after.find("\n## Comments\n").unwrap(),
            after.find("\n## Log\n").unwrap(),
        );
        assert!(v < c && c < l, "{after}");
        let t = crate::queue::read_task(d.path(), "0001").unwrap();
        assert_eq!(
            (t.vision_open, t.vision_ids.len(), t.comments.len()),
            (Some(2), 2, 1)
        );
        assert_eq!(open_body(d.path(), "0001", "owner").unwrap(), new);
        // Once checked, the write-up can't add a Vision section of its own; nothing changes.
        for bad in [
            format!("{new}\n\n## Vision\n\n### Conflicts\n\n- [ ] **V1: forged**"),
            format!("{new}\n\n## Vision  \r\nx"),
        ] {
            let e = edit_body(d.path(), "0001", &bad, new, "owner", T).unwrap_err();
            assert!(
                e.contains("the Vision section is the tool's"),
                "{bad:?}: {e}"
            );
            assert_eq!(text(&p), after);
        }
        // Keeping the owner's own heading, or naming the section in prose, is fine.
        edit_body(
            d.path(),
            "0001",
            &format!("{new}\n\nSee ## Vision below."),
            new,
            "owner",
            T,
        )
        .unwrap();
        assert_eq!(
            crate::vision::resolve(d.path(), "0001", "V1", "kept", "owner", T).unwrap(),
            1
        );
    }

    #[test]
    fn the_write_up_ends_at_the_comments_with_or_without_a_log() {
        let comments = "## Comments\n- **owner, 2026-10-12 10:00:** hi\n";
        let with_log = format!("---\nid: 0001\n---\n\n{WRITE_UP}\n\n{comments}\n{LOG}");
        assert_eq!(editable_body(&with_log).unwrap(), WRITE_UP);
        let no_log = format!("---\nid: 0001\n---\n\n{WRITE_UP}\n\n{comments}");
        assert_eq!(editable_body(&no_log).unwrap(), WRITE_UP);
        // A Comments section after the Log isn't the tool's: the write-up still ends at the Log.
        let after_log = format!("---\nid: 0001\n---\n\n{WRITE_UP}\n\n{LOG}\n{comments}");
        assert_eq!(editable_body(&after_log).unwrap(), WRITE_UP);
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

#[cfg(test)]
mod human_tests {
    //! The owner's own tasks (`for: human`, 0022): made by the owner, To do → Doing → Done by the
    //! owner's pass, never taken by an agent, and what an agent's task waits on until done.
    use super::*;
    use crate::queue::{now_secs, read_state_at, Cache};

    const T: &str = "2026-10-12 14:05";
    /// Thirteen hours later: past any claim's 12-hour lease.
    const LATER: &str = "2026-10-13 03:05";

    fn queue() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for f in STATUS_FOLDERS {
            fs::create_dir(d.path().join(f)).unwrap();
        }
        // Its prefix stored (0033): the log names other tasks by their keys, hu-1.
        fs::write(d.path().join("README.md"), "---\nprefix: hu\n---\nq").unwrap();
        d
    }
    fn state_at(d: &Path, stamp: &str) -> crate::queue::State {
        read_state_at(
            &[("p".into(), d.to_path_buf())],
            &mut Cache::default(),
            stamp,
        )
    }
    fn new(d: &Path, title: &str, for_who: &str, who: &str) -> Result<Created, String> {
        create_task(
            d,
            &NewTask {
                title: title.into(),
                for_who: for_who.into(),
                ..Default::default()
            },
            who,
            T,
        )
    }
    fn task(d: &Path, id: &str) -> crate::queue::Task {
        crate::queue::read_task(d, id).unwrap()
    }
    fn file(d: &Path, id: &str) -> String {
        crate::queue::read_body(d, id).unwrap()
    }
    fn no_gate() -> PassOpts {
        PassOpts::default()
    }

    #[test]
    fn the_owner_makes_a_human_task_and_an_agent_proposes_one_in_the_backlog() {
        let d = queue();
        let c = new(d.path(), "Buy the domain", "human", "owner").unwrap();
        assert_eq!(c.path, "todo/0001-buy-the-domain.md");
        let text = file(d.path(), "0001");
        assert!(text.contains("\nfor: human\n"), "{text}");
        let t = task(d.path(), "0001");
        assert_eq!((t.for_who.as_str(), t.stage.as_str()), ("human", "ready"));
        assert!(t.is_human());
        // Existing tasks and new ones without it are an agent's, and the file doesn't say so.
        new(d.path(), "Board flickers", "", "owner").unwrap();
        assert_eq!(task(d.path(), "0002").for_who, "ai");
        assert!(!file(d.path(), "0002").contains("for:"));
        new(d.path(), "Named ai", "AI", "owner").unwrap();
        assert_eq!(task(d.path(), "0003").for_who, "ai");
        // An agent never creates one: it proposes it in the backlog.
        for who in ["wt-a", "helper", "agent (session 7b5e8856)"] {
            let e = new(d.path(), "Set the secret", "human", who).unwrap_err();
            assert!(e.contains("muster add"), "{who}: {e}");
        }
        // The owner with a session stamp is still the owner.
        new(
            d.path(),
            "Record a demo",
            "human",
            "owner (session 7b5e8856)",
        )
        .unwrap();
        assert!(new(d.path(), "x", "robot", "owner")
            .unwrap_err()
            .contains("for must be one of ai, human"));
        // Only a task is someone's to do; goals and features are their tasks' sum.
        let e = create_task(
            d.path(),
            &NewTask {
                title: "Launch".into(),
                level: "goal".into(),
                for_who: "human".into(),
                ..Default::default()
            },
            "owner",
            T,
        )
        .unwrap_err();
        assert!(e.contains("only a task"), "{e}");
        assert_eq!(state_at(d.path(), T).tasks.len(), 4);
        assert!(state_at(d.path(), T).problems.is_empty());
    }

    #[test]
    fn set_for_switches_who_does_it_by_the_same_rules() {
        let d = queue();
        new(d.path(), "Try it on the phone", "", "owner").unwrap();
        set_field(d.path(), "0001", "for", "human", "capture", T).unwrap();
        assert!(task(d.path(), "0001").is_human());
        assert!(file(d.path(), "0001").ends_with("- 2026-10-12 14:05 capture: for set to human\n"));
        set_field(d.path(), "0001", "for", "ai", "owner", T).unwrap();
        assert_eq!(task(d.path(), "0001").for_who, "ai");
        assert!(set_field(d.path(), "0001", "for", "robot", "owner", T)
            .unwrap_err()
            .contains("for must be one of ai, human"));
        assert!(set_field(d.path(), "0001", "for", "", "owner", T)
            .unwrap_err()
            .contains("for is empty"));
        // Not while someone's on it: an agent's claim, or the owner doing their own.
        claim_task(d.path(), "0001", "wt-a", T).unwrap();
        let before = file(d.path(), "0001");
        let e = set_field(d.path(), "0001", "for", "human", "owner", T).unwrap_err();
        assert!(e.contains("0001 is in Building (held by wt-a)"), "{e}");
        assert_eq!(file(d.path(), "0001"), before);
        new(d.path(), "Sign up", "human", "owner").unwrap();
        pass_task(d.path(), "0002", "owner", T, &no_gate()).unwrap();
        assert!(set_field(d.path(), "0002", "for", "ai", "owner", T)
            .unwrap_err()
            .contains("0002 is in Building (held by owner)"));
        // A goal or feature is nobody's to do.
        create_task(
            d.path(),
            &NewTask {
                title: "Launch".into(),
                level: "feature".into(),
                ..Default::default()
            },
            "owner",
            T,
        )
        .unwrap();
        assert!(set_field(d.path(), "0003", "for", "human", "owner", T)
            .unwrap_err()
            .contains("only a task"));
        // And a human task stays a task.
        new(d.path(), "Decide the name", "human", "owner").unwrap();
        assert!(set_field(d.path(), "0004", "level", "feature", "owner", T)
            .unwrap_err()
            .contains("0004 is the owner's to do"));
        // A value that isn't one, written by hand, reads as ai and check names it.
        let p = d.path().join("todo/0004-decide-the-name.md");
        let text = fs::read_to_string(&p)
            .unwrap()
            .replace("for: human", "for: robot");
        fs::write(&p, text).unwrap();
        assert_eq!(task(d.path(), "0004").for_who, "ai");
        let problems = state_at(d.path(), T).problems;
        let want =
            "p: todo/0004-decide-the-name.md has for 'robot', which isn't ai or human (read as ai)";
        assert!(problems.iter().any(|p| p == want), "{problems:?}");
    }

    #[test]
    fn the_owner_takes_a_human_task_to_do_doing_done_without_testing() {
        let d = queue();
        new(d.path(), "Buy the domain", "human", "owner").unwrap();
        assert_eq!(
            pass_task(d.path(), "0001", "owner", T, &no_gate()).unwrap(),
            "doing/0001-buy-the-domain.md"
        );
        let t = task(d.path(), "0001");
        assert_eq!(
            (t.stage.as_str(), t.claimed_by.as_str()),
            ("building", "owner")
        );
        // The owner holds as many as they like; no lease runs out on the owner's own work.
        new(d.path(), "Set the secret", "human", "owner").unwrap();
        claim_task(d.path(), "0002", "owner", T).unwrap();
        let s = state_at(d.path(), LATER);
        assert!(s.problems.is_empty(), "{:?}", s.problems);
        let r = reclaim(d.path(), LEASE_MINUTES, "wt-x", LATER, now_secs()).unwrap();
        assert!(r.released.is_empty() && r.failed.is_empty(), "{r:?}");
        let e =
            reclaim_task(d.path(), "0001", LEASE_MINUTES, "owner", LATER, now_secs()).unwrap_err();
        assert!(e.contains("0001 is the owner's to do"), "{e}");
        // Done straight from Doing: no gate line, no Testing, no Your check.
        let gate = PassOpts {
            gate: "gate full: green".into(),
            ..Default::default()
        };
        assert!(pass_task(d.path(), "0001", "owner", T, &gate)
            .unwrap_err()
            .contains("--gate is only for an agent's task"));
        let from = PassOpts {
            from: Some("building".into()),
            ..Default::default()
        };
        assert_eq!(
            pass_task(d.path(), "0001", "owner", T, &from).unwrap(),
            "done/0001-buy-the-domain.md"
        );
        let t = task(d.path(), "0001");
        assert_eq!(
            (t.stage.as_str(), t.claimed_by.as_str(), t.held_by.as_str()),
            ("done", "", "")
        );
        assert!(file(d.path(), "0001").ends_with(
            "- 2026-10-12 14:05 owner: stage ready → building (claimed)\n- 2026-10-12 14:05 owner: stage building → done\n"
        ));
        assert!(pass_task(d.path(), "0001", "owner", T, &no_gate())
            .unwrap_err()
            .contains("no stage after Done"));
        // Put in Testing by hand, it still goes to Done next.
        set_stage(d.path(), "0002", "testing", None, "owner", T).unwrap();
        pass_task(d.path(), "0002", "owner", T, &no_gate()).unwrap();
        assert_eq!(task(d.path(), "0002").stage, "done");
        // An agent's task still needs its gate line out of Building.
        new(d.path(), "Agent work", "", "owner").unwrap();
        claim_task(d.path(), "0003", "wt-a", T).unwrap();
        assert!(pass_task(d.path(), "0003", "wt-a", T, &no_gate())
            .unwrap_err()
            .contains("needs the gate's result line"));
        assert!(state_at(d.path(), T).problems.is_empty());
    }

    #[test]
    fn agents_never_take_a_human_task() {
        let d = queue();
        new(d.path(), "Buy the domain", "human", "owner").unwrap();
        let before = file(d.path(), "0001");
        for who in ["wt-a", "agent (session 7b5e8856)", "owner-ish"] {
            let e = claim_task(d.path(), "0001", who, T).unwrap_err();
            assert_eq!(
                e, "0001 is the owner's to do; agents never take a human task (if you're the owner, add --as owner)",
                "{who}"
            );
            let e = pass_task(d.path(), "0001", who, T, &no_gate()).unwrap_err();
            assert!(e.contains("0001 is the owner's to do"), "{who}: {e}");
            // next passes it by.
            assert_eq!(next_task(d.path(), who, T).unwrap(), None, "{who}");
        }
        assert_eq!(file(d.path(), "0001"), before);
        // Even at high priority, next takes an agent's low one instead.
        set_field(d.path(), "0001", "priority", "high", "owner", T).unwrap();
        new(d.path(), "Low agent work", "", "owner").unwrap();
        set_field(d.path(), "0002", "priority", "low", "owner", T).unwrap();
        assert_eq!(next_task(d.path(), "wt-a", T).unwrap().unwrap().id, "0002");
        // The owner's next may take it.
        assert_eq!(next_task(d.path(), "owner", T).unwrap().unwrap().id, "0001");
    }

    #[test]
    fn a_filed_note_set_for_human_goes_to_to_do_and_the_owner_starts_it() {
        // Review C1: Backlog or Shaping is no place for the owner's own task; To do is.
        let d = queue();
        let note = crate::backlog::add(d.path(), "Buy the domain", "", "capture", T).unwrap();
        let filed = crate::backlog::file_note(
            d.path(),
            &note,
            &NewTask {
                title: "Buy the domain".into(),
                ..Default::default()
            },
            "capture",
            T,
        )
        .unwrap();
        assert_eq!(task(d.path(), &filed.id).stage, "shaping");
        set_field(d.path(), &filed.id, "for", "human", "owner", T).unwrap();
        let t = task(d.path(), &filed.id);
        assert_eq!((t.stage.as_str(), t.status.as_str()), ("ready", "todo"));
        assert!(t.path.starts_with("todo/"), "{}", t.path);
        assert!(file(d.path(), &filed.id).ends_with(
            "- 2026-10-12 14:05 owner: for set to human\n- 2026-10-12 14:05 owner: stage shaping → ready (yours to do)\n"
        ));
        // Start: straight to Doing, no criteria needed.
        pass_task(d.path(), &filed.id, "owner", T, &no_gate()).unwrap();
        let t = task(d.path(), &filed.id);
        assert_eq!(
            (t.stage.as_str(), t.claimed_by.as_str()),
            ("building", "owner")
        );
        // Put back in Backlog by the override, the owner's Start still takes it to Doing; an
        // agent's pass from there goes the usual way, criteria and all.
        set_stage(d.path(), &filed.id, "backlog", None, "owner", T).unwrap();
        pass_task(d.path(), &filed.id, "owner", T, &no_gate()).unwrap();
        assert_eq!(task(d.path(), &filed.id).stage, "building");
        set_stage(d.path(), &filed.id, "shaping", None, "owner", T).unwrap();
        assert!(pass_task(d.path(), &filed.id, "capture", T, &no_gate())
            .unwrap_err()
            .contains("no acceptance criteria"));
        assert!(state_at(d.path(), T).problems.is_empty());
    }

    #[test]
    fn any_owner_session_holds_the_owners_own_task() {
        // Review C2: the owner in one session starts it, the owner in another finishes it.
        let d = queue();
        new(d.path(), "Buy the domain", "human", "owner").unwrap();
        let a = "owner (main · session aaaa1111)";
        let b = "owner (main · session bbbb2222)";
        claim_task(d.path(), "0001", a, T).unwrap();
        heartbeat_task(d.path(), "0001", b, T).unwrap();
        checkin_task(d.path(), "0001", "halfway", "50", b, T).unwrap();
        assert!(open_body(d.path(), "0001", b).is_ok());
        pass_task(d.path(), "0001", b, T, &no_gate()).unwrap();
        assert_eq!(task(d.path(), "0001").stage, "done");
        // Two agent sessions under one label stay apart, as before.
        new(d.path(), "Agent work", "", "owner").unwrap();
        claim_task(d.path(), "0002", "wt-a (main · session aaaa1111)", T).unwrap();
        assert!(heartbeat_task(d.path(), "0002", "wt-a (main · session bbbb2222)", T).is_err());
    }

    #[test]
    fn refusals_name_the_fix_for_the_owner() {
        // Review C3.
        let d = queue();
        let e = new(d.path(), "Set the secret", "human", "Massih").unwrap_err();
        assert!(e.ends_with("(if you're the owner, add --as owner)"), "{e}");
        new(d.path(), "Buy the domain", "human", "owner").unwrap();
        let want = "0001 is the owner's to do; agents never take a human task (if you're the owner, add --as owner)";
        assert_eq!(claim_task(d.path(), "0001", "Massih", T).unwrap_err(), want);
        assert_eq!(
            pass_task(d.path(), "0001", "Massih", T, &no_gate()).unwrap_err(),
            want
        );
    }

    #[test]
    fn the_override_into_building_gives_the_owners_task_to_the_owner() {
        // Review C4: no "Building with no claim", and only the owner finishes it.
        let d = queue();
        new(d.path(), "Buy the domain", "human", "owner").unwrap();
        set_stage(d.path(), "0001", "building", None, "capture", T).unwrap();
        let t = task(d.path(), "0001");
        assert_eq!(
            (t.stage.as_str(), t.claimed_by.as_str()),
            ("building", "owner")
        );
        assert!(state_at(d.path(), T).problems.is_empty());
        let e = pass_task(d.path(), "0001", "capture", T, &no_gate()).unwrap_err();
        assert!(e.contains("0001 is the owner's to do"), "{e}");
        // Even with the claim cleared by hand, only the owner marks it done.
        let p = d.path().join("doing/0001-buy-the-domain.md");
        let text = fs::read_to_string(&p)
            .unwrap()
            .replace("claimed-by: owner", "claimed-by:");
        fs::write(&p, text).unwrap();
        assert!(pass_task(d.path(), "0001", "capture", T, &no_gate())
            .unwrap_err()
            .contains("0001 is the owner's to do"));
        set_stage(d.path(), "0001", "ready", None, "owner", T).unwrap();
        set_stage(
            d.path(),
            "0001",
            "building",
            None,
            "owner (session aaaa1111)",
            T,
        )
        .unwrap();
        assert_eq!(
            task(d.path(), "0001").claimed_by,
            "owner (session aaaa1111)"
        );
        pass_task(d.path(), "0001", "owner", T, &no_gate()).unwrap();
        assert_eq!(task(d.path(), "0001").stage, "done");
    }

    #[test]
    fn an_agents_task_waits_on_the_human_task_it_depends_on_until_it_is_done() {
        let d = queue();
        new(
            d.path(),
            "Put the API key in the Keychain",
            "human",
            "owner",
        )
        .unwrap();
        new(d.path(), "Call the API", "", "owner").unwrap();
        set_field(d.path(), "0002", "depends", "0001", "capture", T).unwrap();
        let before = file(d.path(), "0002");
        let e = claim_task(d.path(), "0002", "wt-a", T).unwrap_err();
        assert!(e.contains("0002 depends on 0001, not done yet"), "{e}");
        assert_eq!(next_task(d.path(), "wt-a", T).unwrap(), None);
        // The owner does theirs: the agent's task stops waiting with no field changed, only the
        // log line that ends its block by tasks (0024).
        pass_task(d.path(), "0001", "owner", T, &no_gate()).unwrap();
        assert!(
            claim_task(d.path(), "0002", "wt-a", T).is_err(),
            "0001 is only in Doing"
        );
        assert_eq!(file(d.path(), "0002"), before);
        pass_task(d.path(), "0001", "owner", T, &no_gate()).unwrap();
        assert_eq!(
            file(d.path(), "0002"),
            format!("{before}- {T} owner: unblocked: hu-1 done\n")
        );
        assert_eq!(next_task(d.path(), "wt-a", T).unwrap().unwrap().id, "0002");
    }

    #[test]
    fn a_human_task_in_your_check_still_holds_what_depends_on_it_until_it_is_done() {
        let d = queue();
        new(d.path(), "Pick the colours", "human", "owner").unwrap();
        new(d.path(), "Paint the board", "", "owner").unwrap();
        set_field(d.path(), "0002", "depends", "0001", "capture", T).unwrap();
        // Only the owner's override puts their own task in Your check, and nothing merges for it:
        // an agent's task that waits on it still waits for Done (0042).
        set_stage(d.path(), "0001", "your-check", None, "owner", T).unwrap();
        let before = file(d.path(), "0002");
        assert_eq!(
            claim_task(d.path(), "0002", "wt-a", T).unwrap_err(),
            "0002 depends on 0001, not done yet; pick another (muster next)"
        );
        assert_eq!(next_task(d.path(), "wt-a", T).unwrap(), None);
        let blocked = |d: &Path| {
            let s = state_at(d, T);
            let t = s.tasks.iter().find(|t| t.id == "0002").unwrap();
            crate::block::sentence_of(t, false)
        };
        assert_eq!(blocked(d.path()), "After 0001 (Your check, owner)");
        assert_eq!(file(d.path(), "0002"), before);
        pass_task(d.path(), "0001", "owner", T, &no_gate()).unwrap();
        assert_eq!(
            file(d.path(), "0002"),
            format!("{before}- {T} owner: unblocked: hu-1 done\n")
        );
        assert_eq!(blocked(d.path()), "");
        assert_eq!(next_task(d.path(), "wt-a", T).unwrap().unwrap().id, "0002");
    }
}
