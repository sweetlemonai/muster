//! Changing tasks: the one implementation of Muster's rules for creating and managing tasks,
//! used by the app's window and by the `muster` command line alike. Every change holds the
//! queue's lock, writes atomically, stamps the time itself and logs who did it.

use crate::now;
use crate::queue::{
    self, actor, append_log, find_task, front_matter, mirror_status, with_lock, write_atomic,
    STATUS_FOLDERS, SUMMARY_MAX,
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
    /// Test time: the owner's check only (QA is in `est_time`), the same format as `est_time`.
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

/// A task's summary (mu-78): what was built, in one plain line of at most `SUMMARY_MAX`
/// characters. A line break or any other control character is refused, not folded away, so what
/// the owner reads is what was written; runs of spaces become one.
pub(crate) fn clean_summary(text: &str) -> Result<String, String> {
    if text.contains(['\n', '\r']) {
        return Err("the summary is one line: no line breaks".into());
    }
    if text.chars().any(char::is_control) {
        return Err("the summary can't hold control characters (tabs, escapes)".into());
    }
    let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() {
        return Err("the summary is empty: say what was built, in one line".into());
    }
    if t.chars().count() > SUMMARY_MAX {
        return Err(format!(
            "the summary is longer than {SUMMARY_MAX} characters ({}): say it in one shorter line",
            t.chars().count()
        ));
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
        .unwrap_or(0)
        // A local queue's history (0109) remembers ids whose files are gone (a `git clean -x`
        // before `muster restore`), so a new task never takes one of them.
        .max(crate::history::last_id(root));
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
    /// Who holds it from the start, with its heartbeat at the creation's stamp: a filed note's
    /// shaper (0097), or "" for nobody.
    pub(crate) claimed_by: String,
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
        claimed_by: String::new(),
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
/// and then with `log`'s lines, claimed by `t.claimed_by` when it names someone. The caller holds
/// the queue's lock and took `id` from `next_id` under it.
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
    if !t.claimed_by.is_empty() {
        text = set_front(&text, "claimed-by", &t.claimed_by)?;
        text = set_front(&text, "heartbeat", stamp)?;
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
    crate::verify::refuse_owner_criteria(&t.body, &who)?;
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
                    claimed_by: released(t, "ready"),
                    commit: None,
                    summary: None,
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

/// `qa:` as set (mu-131): reference, walkthrough or both, in any case; "" clears it.
fn clean_qa(value: &str) -> Result<String, String> {
    let v = value.trim().to_lowercase();
    if v.is_empty() || crate::gates::QA.contains(&v.as_str()) {
        Ok(v)
    } else {
        Err(format!(
            "qa is reference, walkthrough or both (or \"\" for none), got {:?}",
            value.chars().take(40).collect::<String>()
        ))
    }
}

/// Set priority, type, title, tags, estimates, touches, the summary, qa or depends.
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
        "summary" => ("summary", clean_summary(value)?),
        "qa" => ("qa", clean_qa(value)?),
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
                "can't set {field:?}: use priority, type, title, tags, time, test-time, tokens, touches, summary, qa, level, parent, due, depends or for"
            ))
        }
    };
    if val.is_empty()
        && !matches!(
            key,
            "tags" | "est-time" | "est-test-time" | "est-tokens" | "touches" | "qa"
        )
    {
        return Err(format!("{field} is empty"));
    }
    let line = free_text_line(
        format!("{stamp} {who}: {key} set to {val}"),
        &format!("the {key}"),
    )?;
    edit(root, id, |_, text| {
        // qa: decides whether the merge needs visual QA (G4): once the task is under way, only
        // the owner changes it, so a builder or tester can't drop the gate it would be held by.
        // Under way is started on its branch (a return to Ready keeps it) or in a later stage,
        // the stage read as parse_task reads it.
        if key == "qa" && !is_owner(&who) {
            let fm = front_matter(&text).map(|(fm, _)| fm).unwrap_or_default();
            let field = |k: &str| fm.get(k).map_or(String::new(), |v| v.trim().to_lowercase());
            let stage = field("stage");
            if matches!(
                stage.as_str(),
                "building" | "testing" | "your-check" | "done"
            ) {
                return Err(format!(
                    "{id} is in {}: once it's under way only the owner changes qa (the visual QA its merge needs)",
                    stage_label(&stage)
                ));
            }
            if !field("branch").is_empty() {
                return Err(format!(
                    "{id} was started on its own branch: once it's under way only the owner changes qa (the visual QA its merge needs)"
                ));
            }
        }
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
            claimed_by: released(t, stage),
            commit: None,
            summary: None,
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
    /// Who passed it Ready → Building most recently (the builder), or took that claim over since
    /// (0090), if anyone.
    builder: Option<String>,
    /// Every holder that built it since that claim (`queue::builder_chain`): the claimer and the
    /// sessions that took it over.
    builders: Vec<String>,
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
    /// What was built, in one line (mu-78, `summary:`), "" before anyone wrote it.
    summary: String,
    /// The `[NEEDS CLARIFICATION: …]` markers left in its write-up (mu-103, `clarify::markers`)
    /// and its open questions with no default (`clarify::waits`): both hold Shaping back.
    markers: Vec<String>,
    waits: Vec<u32>,
    /// Owner findings still open or answered (mu-101) of an agent's task in Building or Testing
    /// with no branch (merged, or never started): no review round can re-check them until
    /// `muster start` makes a branch, so they hold its pass. Empty otherwise.
    owner_unbranched: Vec<String>,
    /// Started with gate records (mu-131, `gates: on`): its criteria record ticks its criteria
    /// (mu-132).
    gates_on: bool,
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
    /// A new `summary:` (mu-78, a pass out of Building), or None to leave it.
    summary: Option<String>,
    log: Vec<String>,
}

fn stage_index(stage: &str) -> usize {
    STAGES.iter().position(|s| *s == stage).unwrap_or(0)
}

/// The claim after leaving the current stage for `to`: Building's holder lets go, and so does
/// Shaping's (0097: the shaper of a filed note). Nothing but `muster file` brings a claim into
/// Shaping, so a claim held anywhere else (a reservation in Ready) doesn't follow a task there.
fn released(t: &Current, to: &str) -> String {
    if queue::has_lease(&t.stage) || to == "shaping" {
        String::new()
    } else {
        t.claimed_by.clone()
    }
}

fn stage_line(stamp: &str, who: &str, from: &str, to: &str, detail: &str) -> String {
    format!("{stamp} {who}: stage {from} → {to}{detail}")
}

/// Do these new log lines hold a return's stage line (`… (returned: <why>)`)?
fn is_return(log: &[String]) -> bool {
    log.iter().any(|l| {
        queue::stage_line(&format!("- {l}"))
            .is_some_and(|(_, detail)| detail.starts_with("returned: "))
    })
}

/// Would this log line read as tool-written stage history?
fn reads_as_stage(line: &str) -> bool {
    queue::stage_line(&format!("- {line}")).is_some()
}

/// Would this log line read as a takeover (`queue::takeover_line`, 0090)?
fn reads_as_takeover(line: &str) -> bool {
    queue::takeover_line(&format!("- {line}")).is_some()
}

/// The claim `claim` re-stamped for `who`, its holder acting with a session (mu-184,
/// `session::restamped`), with the takeover line that logs it once, `<who>: took over the claim
/// from <claim> (stamped)`; None when the claim reads right already, or the line wouldn't read back
/// as a takeover.
fn restamp(claim: &str, who: &str, stamp: &str) -> Option<(String, String)> {
    let new = crate::session::restamped(claim, who)?;
    let line = format!(
        "{stamp} {new}{}{claim} ({})",
        queue::TOOK_OVER,
        queue::STAMPED
    );
    reads_as_takeover(&line).then_some((new, line))
}

/// `text` (a task file) with its claim `claim` re-stamped for `who` and logged (`restamp`), or as
/// is. For a holder's act that writes the file itself: a heartbeat, a check-in, a note.
fn restamped_text(text: &str, claim: &str, who: &str, stamp: &str) -> Result<String, String> {
    match restamp(claim, who, stamp) {
        Some((new, line)) => Ok(append_log(&set_front(text, "claimed-by", &new)?, &line)),
        None => Ok(text.to_string()),
    }
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
    if reads_as_takeover(&line) {
        return Err(format!(
            "{what} would read as a takeover in the log (\"<who>: took over the claim from …\"); only muster claim --takeover writes those"
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
    let owner_unbranched =
        if t.branch.is_empty() && !human && matches!(t.stage.as_str(), "building" | "testing") {
            crate::review::owner_to_recheck(root, &t.id)
        } else {
            Vec::new()
        };
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
        builders: t.builders,
        tester,
        gated_by,
        heartbeat: t.heartbeat,
        modified,
        level: t.level,
        branch: t.branch,
        worktree: t.worktree,
        owner_unbranched,
        paused: t.paused,
        summary: t.summary,
        markers: crate::clarify::markers(text),
        waits: crate::clarify::waits(&t.questions),
        gates_on: t.gates_on,
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
        // The holder acting with a session on a claim that names none is stamped now (mu-184),
        // logged once as a takeover before what it did: a claim the change keeps carries the
        // stamp on, and one it lets go of still records who built it.
        // A sweep that lets the claim go (a reclaim, a freed task's release) isn't the holder
        // acting, whoever runs it: only a change that keeps the claim or passes it on stamps it.
        let keeps = c.claimed_by == now.claimed_by;
        if queue::has_lease(&now.stage)
            && same_holder(&now.claimed_by, who)
            && (keeps || c.stage == "testing")
        {
            if let Some((new, line)) = restamp(&now.claimed_by, who, stamp) {
                if keeps {
                    c.claimed_by = new;
                }
                c.log.insert(0, line);
            }
        }
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
        // A send-back from Testing isn't starting anything (mu-66), nor is a return from Your check
        // to Building (the owner's QA, mu-101): its builder gets it back even while holding
        // another task, which then waits behind it.
        let sent_back = c.stage == "building"
            && (now.stage == "testing" || (now.stage == "your-check" && is_return(&c.log)));
        if new_hold && !sent_back && !is_owner(&c.claimed_by) {
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
            let others = elsewhere(root);
            if let Some(e) = slot_refusal(root, &tasks, &others, &c.claimed_by, id, "") {
                return Err(e);
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
        if let Some(summary) = &c.summary {
            new = set_front(&new, "summary", summary)?;
        }
        // Testing and Your check have a holder too (0030), shown like the claim but not one: who
        // may pass is unchanged. Testing goes to whoever passes it there from Building; back in
        // Testing any other way, to whoever did that last. Your check is the owner's.
        let held_by = match c.stage.as_str() {
            s if s == now.stage => now.held_by.clone(),
            // The claim's role slot stays with its holder (0112), whatever name it acts as.
            // Unless the slot went to another session while it came back without it.
            "testing" if now.stage == "building" => {
                unless_taken(root, id, &crate::session::keep_slot(&now.claimed_by, who))
            }
            // A tester whose slot went to another session meanwhile gets it back without the slot.
            "testing" => now
                .tester
                .as_deref()
                .map(|t| unless_taken(root, id, t))
                .unwrap_or_else(|| who.to_string()),
            "your-check" => "owner".to_string(),
            _ => String::new(),
        };
        if !held_by.is_empty() || fields.contains_key("held-by") {
            new = set_front(&new, "held-by", &held_by)?;
        }
        // The lease restarts when the holder acts, and whenever a change makes a hold: a claim,
        // a return to the builder, or an unblock (time spent waiting isn't time gone quiet). In
        // Shaping too (0097): the shaper's acts and an unblock renew it, as in Building.
        let holder_acts = queue::has_lease(&now.stage)
            && !now.claimed_by.is_empty()
            && same_holder(&now.claimed_by, who);
        let new_claim = c.stage == "building"
            && !c.claimed_by.is_empty()
            && (now.stage != "building" || !same_holder(&now.claimed_by, &c.claimed_by));
        let unblocked =
            queue::has_lease(&c.stage) && now.blocked && !c.blocked && !c.claimed_by.is_empty();
        if holder_acts || new_claim || new_hold || unblocked {
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
    let far = std::cell::OnceCell::new();
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
        // counts until it's been released here); its session's, in any queue, when either acts
        // as a role slot (0112: one session is one slot).
        let building = |o: &queue::Task| {
            o.stage == "building" && o.blocked_kind.is_empty() && !o.claimed_by.is_empty()
        };
        let one = |o: &queue::Task| {
            building(o) && crate::session::one_session_slot(&o.claimed_by, &t.claimed_by)
        };
        let other = tasks
            .iter()
            .find(|o| {
                o.id != t.id
                    && !released.contains(&o.id.as_str())
                    && (one(o) || building(o) && same_holder(&o.claimed_by, &t.claimed_by))
            })
            .map(|o| key(&o.id))
            .or_else(|| {
                let session = held && crate::session::session_of(&t.claimed_by).is_some();
                session
                    .then(|| far.get_or_init(|| elsewhere(root)))
                    .and_then(|e| e.iter().find(|o| one(o)))
                    .map(|o| match o.queue.as_str() {
                        "" => crate::key::key(&o.prefix, &o.id),
                        q => format!("{} in {q}", crate::key::key(&o.prefix, &o.id)),
                    })
            });
        let _ = match other.filter(|_| held) {
            Some(o) => {
                released.push(&t.id);
                let why = format!(
                    " (released: {} already holds {o})",
                    crate::session::split(&t.claimed_by).0,
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
                        summary: None,
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
    /// What was built, in one plain line (mu-78): needed to pass a task out of Building, unless
    /// it has one already; stored as `summary:`.
    pub summary: String,
    /// The stage the caller saw the task in. When given, the pass is refused if the task has
    /// moved on since (a double click, a stale window), so one click never moves two stages.
    pub from: Option<String>,
    /// Why the now pages a task covers (mu-102) are still true though its branch doesn't change
    /// them: needed to pass such a task out of Building, and logged.
    pub now_unchanged: String,
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
    let summary = if opts.summary.trim().is_empty() {
        String::new()
    } else {
        clean_summary(&opts.summary)?
    };
    let expected = seen_stage(opts.from.as_deref())?;
    let now_why = if opts.now_unchanged.trim().is_empty() {
        String::new()
    } else {
        one_line(&opts.now_unchanged, now::MAX_WHY, "--now-unchanged")?
    };
    // The now pages a task leaving Building covers (mu-102), read before the lock: git runs here.
    let now_pass = queue::read_task(root, id)
        .ok()
        .filter(|t| t.stage == "building" && t.level == "task" && !t.is_human())
        .map_or(now::Pass::Free, |t| now::pass_check(root, &t));
    change(root, id, &who, stamp, |t| {
        if t.stage == "testing" && !t.branch.is_empty() {
            return Err(started_pass(id, &t.branch));
        }
        let mut c = pass_change(id, t, &who, stamp, &gate, &commit, expected)?;
        // What was built, in one line, for the owner's check (mu-78): a task (not a goal, a
        // feature or the owner's own) leaves Building with one, given now or written before.
        let out_of_building = t.stage == "building" && c.stage == "testing";
        if !summary.is_empty() && !out_of_building {
            return Err(format!(
                "--summary is only for a task passing Building → Testing; muster set {id} summary \"<line>\" writes or fixes it in any stage"
            ));
        }
        if !now_why.is_empty() && !out_of_building {
            return Err(
                "--now-unchanged is only for a task passing Building → Testing".to_string(),
            );
        }
        // Its now pages are updated on its branch, or it says why they still hold (mu-102).
        if out_of_building {
            // The app's Pass sends no reason: the owner's way past is the track.
            let line = now::pass_line(id, &now_pass, &now_why).map_err(|e| {
                if is_owner(&who) {
                    format!("{e}, or move it to Testing on the track")
                } else {
                    e
                }
            })?;
            if let Some(text) = line {
                let line = free_text_line(format!("{stamp} {who}: {text}"), "--now-unchanged")?;
                c.log.insert(0, line);
            }
        }
        if out_of_building
            && t.level == "task"
            && !t.human
            && summary.is_empty()
            && t.summary.is_empty()
        {
            return Err(format!(
                "passing {id} Building → Testing needs a summary: what was built, in one plain line in the owner's words, {SUMMARY_MAX} characters at most (--summary \"…\"; or muster set {id} summary \"…\" first)"
            ));
        }
        if !summary.is_empty() && summary != t.summary {
            let line = free_text_line(
                format!("{stamp} {who}: summary set to {summary}"),
                "the summary",
            )?;
            c.log.insert(0, line);
            c.summary = Some(summary.clone());
        }
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
            // Started with gate records (mu-131): the merge needs its criteria gate (G3).
            new = set_front(&new, "gates", "on")?;
            new = if runner {
                set_front(&new, "worktree-runner", "yes")?
            } else {
                clear_front(&new, "worktree-runner")?
            };
            Ok((append_log(&new, &line), None))
        })
    })
}

/// `muster start` of a task back in Building with no branch (mu-101: sent back from Your check
/// after its merge cleared `branch:`, or never started on one): `who` holds it, so there's no
/// claim to make; the fresh worktree and branch are recorded and logged under the lock, its
/// stage and claim kept. Refused (before anything is written) unless it's an agent's task in
/// Building held by `who`, not paused, with no branch on record. The git side is made before
/// this; if it's refused, the caller removes it again.
pub fn restart_task(
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
            "{stamp} {who}: started again in {}worktree {} on branch {}",
            if runner { "the runner's " } else { "" },
            one_line(worktree, 1000, "the worktree")?,
            one_line(branch, 200, "the branch")?
        ),
        "the worktree",
    )?;
    with_lock(root, || {
        edit_locked(root, id, |path, text| {
            let t = current(root, path, &text)?;
            restart_rules(id, &t, &who)?;
            let mut new = set_front(&text, "worktree", worktree)?;
            new = set_front(&new, "branch", branch)?;
            // A fresh branch with gate records, as start_task's (mu-131): the merge needs G3.
            new = set_front(&new, "gates", "on")?;
            new = if runner {
                set_front(&new, "worktree-runner", "yes")?
            } else {
                clear_front(&new, "worktree-runner")?
            };
            new = set_front(&new, "heartbeat", stamp)?;
            Ok((append_log(&new, &line), None))
        })
    })
}

/// Refused unless `muster start` may give task `t` (in Building, no branch) a fresh branch as
/// `who`: `restart_task`'s rules. Writes nothing; `parallel::start` checks them before git.
pub fn check_restart(root: &Path, id: &str, who: &str) -> Result<(), String> {
    let who = actor(who)?;
    let path = find_task(root, id)?;
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let t = current(root, &path, &text)?;
    restart_rules(id, &t, &who)
}

fn restart_rules(id: &str, t: &Current, who: &str) -> Result<(), String> {
    not_paused(id, t)?;
    if t.human {
        return Err(format!(
            "{id} is the owner's to do; a human task isn't built in a worktree"
        ));
    }
    if t.stage != "building" {
        return Err(format!(
            "{id} is in {}, not Ready; start takes a Ready task, or one back in Building with no branch",
            stage_label(&t.stage)
        ));
    }
    if !t.branch.is_empty() {
        return Err(format!("{id} is on branch {} already", t.branch));
    }
    if !holds(&t.claimed_by, who, t.human) {
        let by = if t.claimed_by.is_empty() {
            "nobody".to_string()
        } else {
            t.claimed_by.clone()
        };
        return Err(format!(
            "{id} is in Building, claimed by {by}; only its holder starts it again on a fresh branch"
        ));
    }
    Ok(())
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
                    summary: None,
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
/// check or Done is refused when `who` built it: claimed it (Ready → Building), took that claim
/// over (0090: a successor isn't a new tester), or passed it into Testing with the gate's line, as
/// `session::same_holder` matches holders (session and agent). Otherwise, what the stage line
/// adds: the owner's override (the owner never built an agent's task), or a tester under another
/// label in a builder's session and agent (0090: a teammate with its own agent id is apart), so
/// the owner sees it. Nothing for the owner's own task, or one nobody built.
fn tester_rule(id: &str, t: &Current, who: &str, to: &str) -> Result<Option<&'static str>, String> {
    let past_testing =
        stage_index(&t.stage) <= stage_index("testing") && stage_index(to) > stage_index("testing");
    let builders: Vec<&str> = t
        .builders
        .iter()
        .chain([&t.builder, &t.gated_by].into_iter().flatten())
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
    let mine = crate::session::session_of(who);
    let shared = mine.is_some()
        && builders
            .iter()
            .any(|b| crate::session::session_of(b) == mine);
    Ok(shared.then_some("tested under another label in the builder's session"))
}

/// The refusal of a builder's pass out of Testing: who built it (the label, and its session when
/// stamped) and what to do, with a QA label beside the builder's (`muster/wt-0042` → `muster/qa-0042`).
fn built_it(id: &str, builder: &str) -> String {
    let (label, stamp) = crate::session::split(builder);
    let named = match stamp {
        Some(s) if !s.agent.is_empty() => format!("{label} (session {} · agent {})", s.id, s.agent),
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
        "shaping" if !t.markers.is_empty() => {
            return Err(crate::clarify::holds_ready(id, &t.markers))
        }
        "shaping" if !t.waits.is_empty() => {
            return Err(crate::clarify::holds_on_questions(id, &t.waits))
        }
        "testing" if ticked + t.owner_open < items => {
            return Err(format!(
                "{id}: {} of {items} acceptance criteria aren't ticked; {}{}",
                items - ticked - t.owner_open,
                // Started with gate records: the criteria record ticks them (mu-132).
                if t.gates_on {
                    format!("the criteria record ticks each met one: muster verify {id}")
                } else {
                    "tick each with its proof first".to_string()
                },
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
        "building" | "testing" if !t.owner_unbranched.is_empty() => {
            return Err(crate::review::holds_unbranched(id, &t.owner_unbranched))
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
            released(t, to)
        },
        commit: (to == "done" && !commit.is_empty()).then(|| commit.to_string()),
        summary: None,
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
/// waiting, and blocking it is how a holder lets go), other than `except`. A claim of `who`'s
/// session and agent is `who`'s whatever its label when either acts as a role slot (0112): one
/// session holds one Building task, however it names itself.
fn holding(tasks: &[queue::Task], who: &str, except: &str) -> Option<String> {
    tasks
        .iter()
        .find(|t| {
            t.stage == "building"
                && t.blocked_kind.is_empty()
                && !t.claimed_by.is_empty()
                && (same_holder(&t.claimed_by, who)
                    || crate::session::one_session_slot(&t.claimed_by, who))
                && t.id != except
        })
        .map(|t| t.id.clone())
}

/// Every task of the other queues Muster knows (0112): the ones `muster team` and the window read
/// (`projects::places`), not the one at `root`, each with its project's prefix for its key. A role
/// slot is held across them all, so one session is one slot whichever queue it claims in. They're
/// read without their locks, as `muster team` reads them.
pub(crate) fn elsewhere(root: &Path) -> Vec<queue::Task> {
    let places = crate::projects::places();
    let cfg = crate::projects::load(&places.config_file);
    let list = crate::projects::list(&places.base, &cfg);
    let here = root.canonicalize().ok();
    let queues: Vec<(String, PathBuf)> = crate::projects::shown(&list)
        .into_iter()
        .filter(|(_, p)| match (&here, p.canonicalize().ok()) {
            (Some(x), Some(y)) => *x != y,
            _ => p != root,
        })
        .collect();
    if queues.is_empty() {
        return Vec::new();
    }
    let prefix_of = |name: &str| {
        list.iter()
            .find(|p| p.name == name)
            .map(|p| p.prefix.clone())
            .unwrap_or_default()
    };
    queue::read_state_keyed(
        &queues,
        &mut queue::Cache::default(),
        &crate::settings::Settings::default(),
        &prefix_of,
    )
    .tasks
}

/// Why `who` may not hold `id` (0112), from this queue's `tasks` and every other queue's
/// (`others`, `elsewhere`); None when it may. One session (and agent) is one slot across every
/// queue: refused while it holds a Building task in another queue and either acts as a slot, or
/// holds another slot than `who`'s in Building or Testing anywhere (this queue's Building claims
/// are `holding`'s). And a slot is one session's: refused while another session holds `who`'s
/// slot in Building or Testing anywhere, except `lineage`'s session (the claim a takeover
/// succeeds, whose slot it carries on).
fn slot_refusal(
    root: &Path,
    tasks: &[queue::Task],
    others: &[queue::Task],
    who: &str,
    id: &str,
    lineage: &str,
) -> Option<String> {
    use crate::session::{one_session_slot, session_of, slot_of};
    let me = session_of(who)?;
    let mine = slot_of(who);
    // This queue's by its key; another's by its key and its project's name.
    let key = |(here, t): (bool, &queue::Task)| {
        if here {
            keys_of(root, std::slice::from_ref(&t.id))
        } else if t.queue.is_empty() {
            crate::key::key(&t.prefix, &t.id)
        } else {
            format!("{} in {}", crate::key::key(&t.prefix, &t.id), t.queue)
        }
    };
    if let Some(t) = others.iter().find(|t| {
        t.stage == "building"
            && t.blocked_kind.is_empty()
            && !t.claimed_by.is_empty()
            && one_session_slot(&t.claimed_by, who)
    }) {
        return Some(format!(
            "you already hold {}; one session is one slot in every queue, so pass, return or block it first",
            key((false, t))
        ));
    }
    let any = || {
        tasks
            .iter()
            .filter(|t| t.id != id)
            .map(|t| (true, t))
            .chain(others.iter().map(|t| (false, t)))
            .filter(|(_, t)| matches!(t.stage.as_str(), "building" | "testing"))
    };
    if !mine.is_empty() {
        if let Some((here, t)) = any().find(|(_, t)| {
            let slot = slot_of(t.holder());
            session_of(t.holder()).as_ref() == Some(&me) && !slot.is_empty() && slot != mine
        }) {
            let label = crate::session::split(t.holder()).0;
            return Some(format!(
                "this session is {label} already ({} holds it): one session is one slot, so act as {label} (--as {label}), or start another session",
                key((here, t))
            ));
        }
        let heir = session_of(lineage);
        if let Some((here, t)) = any().find(|(_, t)| {
            let theirs = session_of(t.holder());
            slot_of(t.holder()) == mine && theirs.as_ref() != Some(&me) && theirs != heir
        }) {
            let label = crate::session::split(who).0;
            return Some(format!(
                "{label} is another session's ({} holds {}): a slot is one session's, so take a free one (--role) or act as your own",
                crate::session::split(t.holder()).0,
                key((here, t))
            ));
        }
    }
    None
}

/// `who` without its role slot when another session holds that slot now, in Building or Testing
/// in any queue (`slot_refusal`), other than task `id` (0112): a task coming back to a holder whose
/// slot went to someone else meanwhile comes back to the holder alone, so no slot has two.
fn unless_taken(root: &Path, id: &str, who: &str) -> String {
    if crate::session::slot_of(who).is_empty() {
        return who.to_string();
    }
    let me = crate::session::session_of(who);
    let taken = tasks_now(root)
        .iter()
        .filter(|t| t.id != id)
        .chain(elsewhere(root).iter())
        .any(|t| {
            matches!(t.stage.as_str(), "building" | "testing")
                && crate::session::slot_of(t.holder()) == crate::session::slot_of(who)
                && crate::session::session_of(t.holder()) != me
        });
    if taken {
        crate::session::without_slot(who)
    } else {
        who.to_string()
    }
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

/// What `next` says to a holder with a task sent back (mu-66): fix the lowest-id one first, and
/// the others it holds wait behind it. None when none of theirs was sent back.
fn sent_back_first(root: &Path, tasks: &[queue::Task], who: &str) -> Option<String> {
    let mine = |t: &&queue::Task| {
        t.stage == "building"
            && t.blocked_kind.is_empty()
            && !t.claimed_by.is_empty()
            && same_holder(&t.claimed_by, who)
    };
    let first = tasks.iter().filter(mine).find(|t| t.sent_back)?;
    let others: Vec<String> = tasks
        .iter()
        .filter(mine)
        .filter(|t| t.id != first.id)
        .map(|t| t.id.clone())
        .collect();
    let key = keys_of(root, std::slice::from_ref(&first.id));
    let by = sent_back_words(first);
    Some(if others.is_empty() {
        format!("fix {key} first ({by}); pass, return or block it before you take another")
    } else {
        format!(
            "fix {key} first ({by}); you also hold {}, which {} behind it",
            keys_of(root, &others),
            if others.len() == 1 { "waits" } else { "wait" }
        )
    })
}

/// Who sent task `t` back, as `next` says it: "sent back by the review" from Testing (mu-66), and
/// from Your check (mu-101) "sent back by you: <why>" when the owner did, else "sent back by
/// <label>: <why>".
fn sent_back_words(t: &queue::Task) -> String {
    if t.sent_back_by.is_empty() {
        return "sent back by the review".to_string();
    }
    let by = if is_owner(&t.sent_back_by) {
        "you"
    } else {
        t.sent_back_by.as_str()
    };
    format!("sent back by {by}: {}", t.sent_back_why)
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
            return Err(sent_back_first(root, &tasks, &who).unwrap_or_else(|| already_holds(&held)));
        }
        // A send-back to another session under this label that hasn't picked it up (mu-184):
        // this session carries it on, as when it went back by the bare label.
        if let Some(t) = tasks.iter().find(|t| {
            !is_owner(&who)
                && t.sent_back
                && !same_holder(&t.claimed_by, &who)
                && crate::session::split(&t.claimed_by).0 == crate::session::split(&who).0
                && queue::read_body(root, &t.id)
                    .is_ok_and(|body| untouched_send_back(t, &body, &tasks))
        }) {
            let key = keys_of(root, std::slice::from_ref(&t.id));
            return Err(format!(
                "fix {key} first ({}): it went back to {}, which hasn't picked it up; take it over: muster claim {} --takeover",
                sent_back_words(t),
                t.claimed_by,
                t.id
            ));
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

/// Keep a claim alive: stamp `heartbeat:` on a task `who` holds in Building. No log line, but
/// the takeover line of a claim it re-stamps (mu-184, `restamp`).
pub fn heartbeat_task(root: &Path, id: &str, who: &str, stamp: &str) -> Result<String, String> {
    let who = actor(who)?;
    edit(root, id, |path, text| {
        let claim = held_in_building(root, id, path, &text, &who)?;
        let new = restamped_text(&text, &claim, &who, stamp)?;
        Ok((set_front(&new, "heartbeat", stamp)?, None))
    })
}

/// Refused unless the task is in Building or Shaping (a filed note's shaper, 0097) and `who`
/// holds it: whose lease a heartbeat or a check-in may renew. The claim, as the file has it.
fn held_in_building(
    root: &Path,
    id: &str,
    path: &Path,
    text: &str,
    who: &str,
) -> Result<String, String> {
    let t = queue::parse_task("", root, &queue::folder_of(root, path), path, text, 0)
        .ok_or("the task has no front matter")?;
    if !queue::has_lease(&t.stage) {
        return Err(format!(
            "{id} is in {}, not Building or Shaping; only a claim there has a lease to keep alive",
            stage_label(&t.stage)
        ));
    }
    if t.claimed_by.is_empty() {
        return Err(format!("nobody holds {id}; claim it first"));
    }
    if !holds(&t.claimed_by, who, t.is_human()) {
        return Err(format!("{id} is held by {}, not {who}", t.claimed_by));
    }
    Ok(t.claimed_by)
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
    checkin_task_with(root, id, text, pct, false, who, stamp)
}

/// The words a handing-off check-in starts with (0090): `<who>: check-in 40% · handing off · <text>`.
pub const HANDING_OFF: &str = "handing off · ";

/// `checkin_task`, or with `handoff` the holder's last check-in before its session ends (0090),
/// logged as `<who>: check-in · handing off · <text>`: after it, a session under the same label may
/// take the claim over at once (`takeover_task`).
pub fn checkin_task_with(
    root: &Path,
    id: &str,
    text: &str,
    pct: &str,
    handoff: bool,
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    let who = actor(who)?;
    let text = one_line(text, MAX_CHECKIN, "the check-in")?;
    let text = if handoff {
        format!("{HANDING_OFF}{text}")
    } else {
        text
    };
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
    if !handoff && text.starts_with(HANDING_OFF) {
        return Err(
            "the check-in would read as handing off; to hand off, add --handoff, else say it another way".into(),
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
        let claim = held_in_building(root, id, path, &file, &who)?;
        let new = restamped_text(&file, &claim, &who, stamp)?;
        let new = set_front(&new, "heartbeat", stamp)?;
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
pub(crate) fn quiet_text(minutes: u64) -> String {
    if minutes >= 60 {
        format!("{} h", minutes / 60)
    } else {
        format!("{minutes} min")
    }
}

/// A claim `reclaim` released: back to Ready from Building, unheld in Shaping (0097).
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Released {
    pub id: String,
    pub holder: String,
    /// How long it had been quiet ("13 h").
    pub quiet: String,
    pub path: String,
    /// The stage it's in now: `ready`, or `shaping`.
    pub stage: String,
}

#[derive(Serialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Reclaimed {
    pub released: Vec<Released>,
    /// Claims past their lease that couldn't be released, with why.
    pub failed: Vec<String>,
    /// Backlog notes whose shaper's lease ran out (0097), unheld now.
    pub notes: Vec<crate::backlog::ReleasedNote>,
}

/// What releasing the claim on `t` writes, or why it's refused: only an agent's claim in
/// Building or Shaping (0097), not blocked, quiet for longer than `lease` minutes. Building goes
/// back to Ready; Shaping stays, unheld.
fn release_change(
    id: &str,
    t: &Current,
    lease: u64,
    who: &str,
    stamp: &str,
    now_secs: u64,
) -> Result<Change, String> {
    if !queue::has_lease(&t.stage) {
        return Err(format!(
            "{id} is in {}, not Building or Shaping; there's no claim to release",
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
    if t.stage == "shaping" {
        // No stage line: it stays where it is. The holder comes from the file, so the line is
        // checked like any free text, and says less when it would read as something else.
        let quiet = quiet_text(quiet);
        let line = free_text_line(
            format!(
                "{stamp} {who}: reclaimed from {} (no heartbeat for {quiet}); still in Shaping",
                t.claimed_by
            ),
            "the holder",
        )
        .unwrap_or_else(|_| {
            format!("{stamp} {who}: reclaimed (no heartbeat for {quiet}); still in Shaping")
        });
        return Ok(Change {
            stage: "shaping".into(),
            blocked: false,
            waiting_on: t.waiting_on.clone(),
            blocked_kind: t.blocked_kind.clone(),
            depends: None,
            claimed_by: String::new(),
            commit: None,
            summary: None,
            log: vec![line],
        });
    }
    Ok(Change {
        stage: "ready".into(),
        blocked: false,
        waiting_on: t.waiting_on.clone(),
        blocked_kind: t.blocked_kind.clone(),
        depends: None,
        claimed_by: String::new(),
        commit: None,
        summary: None,
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
/// `lease` minutes. A task held in Shaping (0097) is released the same way and stays in Shaping,
/// and so is a backlog note's claim (`backlog::reclaim_notes_locked`). Live claims are left
/// alone. One lock for the whole pass, so a heartbeat lands either before it (and counts) or after
/// it (and is refused: the task isn't Building).
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
            queue::has_lease(&t.stage)
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
                    stage: if t.stage == "shaping" {
                        "shaping"
                    } else {
                        "ready"
                    }
                    .into(),
                    id: t.id,
                    holder: t.claimed_by,
                    path,
                }),
                Err(e) => out.failed.push(format!("{}: {e}", t.id)),
            }
        }
        match crate::backlog::reclaim_notes_locked(root, lease, stamp) {
            Ok(notes) => out.notes = notes,
            Err(e) => out
                .failed
                .push(format!("{}: {e}", crate::backlog::SHAPING_FILE)),
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

/// The detail of the owner's release of a claim no session can be reached on (mu-184).
pub const UNREACHED: &str = "released: no session to reach";

/// The owner's Release of a claim no session can be reached on (mu-184, the pane's Release beside
/// the disabled Ping): a Building task whose claim, or a Testing task whose `held-by:`, names no
/// session (a bare label), whatever its lease. Building goes back to Ready with the claim cleared,
/// logged as the stage line `(released: no session to reach)`; its worktree and branch stay, so
/// `muster start` picks it up there again. In Testing the hold is cleared and it stays there for
/// the next tester, logged with the same words. Refused for anyone but the owner, for a holder
/// with a session (Ping reaches it), for the owner's own hold, for a paused or unheld task, and
/// when the task left the stage the window showed (`from`). Read again under the queue's lock, so
/// a session that stamped its claim since the click (`restamp`) makes it refuse.
pub fn release_unreached(
    root: &Path,
    id: &str,
    who: &str,
    stamp: &str,
    from: Option<&str>,
) -> Result<String, String> {
    let who = actor(who)?;
    if !is_owner(&who) {
        return Err(format!(
            "only the owner releases a claim no session can be reached on; a session under its label takes it over instead (muster claim {id} --takeover)"
        ));
    }
    let expected = seen_stage(from)?;
    with_lock(root, || {
        let path = find_task(root, id)?;
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let t = current(root, &path, &text)?;
        check_seen(id, &t, expected)?;
        not_paused(id, &t)?;
        let holder = match t.stage.as_str() {
            "building" => t.claimed_by.trim(),
            "testing" => t.held_by.trim(),
            _ => {
                return Err(format!(
                "{id} is in {}; only a claim in Building or a hold in Testing is released this way",
                stage_label(&t.stage)
            ))
            }
        };
        if holder.is_empty() {
            return Err(format!("nobody holds {id}; there's nothing to release"));
        }
        if is_owner(holder) {
            return Err(format!("you hold {id} yourself"));
        }
        if crate::session::split(holder).1.is_some() {
            return Err(format!(
                "{holder} has a session now: Ping it, or wait for the lease"
            ));
        }
        if t.stage == "building" {
            return change_locked(root, id, &who, stamp, |t| {
                Ok(Change {
                    stage: "ready".into(),
                    blocked: t.blocked,
                    waiting_on: t.waiting_on.clone(),
                    blocked_kind: t.blocked_kind.clone(),
                    depends: None,
                    claimed_by: String::new(),
                    commit: None,
                    summary: None,
                    log: vec![stage_line(
                        stamp,
                        &who,
                        "building",
                        "ready",
                        &format!(" ({UNREACHED})"),
                    )],
                })
            });
        }
        // No stage line: it stays in Testing. The holder comes from the file, so the line is
        // checked like any free text, and says less when it would read as something else.
        let line = free_text_line(
            format!("{stamp} {who}: let go of {holder}'s hold; still in Testing ({UNREACHED})"),
            "the holder",
        )
        .unwrap_or_else(|_| {
            format!("{stamp} {who}: let go of the hold; still in Testing ({UNREACHED})")
        });
        edit_locked(root, id, |_, text| {
            Ok((append_log(&set_front(&text, "held-by", "")?, &line), None))
        })
    })
}

/// How long a claim must be quiet (no heartbeat, no check-in) before a session under the same
/// label may take it over without a handoff (0090), in minutes. The 12 h lease and `reclaim` are
/// apart from it.
pub const TAKEOVER_QUIET: u64 = 30;

/// A claim `takeover_task` moved: the task's path, who held it, and why it could be taken
/// ("handed off", "quiet 42 min" or "owner's override"). For a takeover run again by the holder
/// it made (mu-156), `moved` is how many of the task's messages it moved now from `from`'s
/// mailbox, and `why` is "".
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TakenOver {
    pub path: String,
    pub from: String,
    pub why: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moved: Option<usize>,
}

impl TakenOver {
    /// What `muster claim <key> --takeover` says when it was run again (mu-156): each case one
    /// whole sentence, so it translates whole. None for a takeover.
    pub fn rerun_line(&self, key: &str) -> Option<String> {
        let n = self.moved?;
        let Some(from) = crate::session::split(&self.from).1.map(|s| s.mailbox()) else {
            return Some(format!(
                "{key}: you hold it already; its old holder had no mailbox, so there's nothing to move"
            ));
        };
        Some(match n {
            0 => format!("{key}: you hold it already; nothing left to move from mail/{from}"),
            1 => format!("{key}: you hold it already; moved 1 message from mail/{from}"),
            n => format!("{key}: you hold it already; moved {n} messages from mail/{from}"),
        })
    }
}

/// The holder a claim held by `claimed_by` was taken over from (mu-156): the old holder in the
/// log's latest takeover line, when that line is the claim's latest start (no stage line into
/// Building or Shaping after it) and names `claimed_by` as the one who took over. None for a
/// claim that didn't come from a takeover.
fn taken_over_from(body: &str, claimed_by: &str) -> Option<String> {
    let mut from = None;
    for line in queue::log_section(body).unwrap_or("").lines() {
        if let Some((s, _)) = queue::stage_line(line) {
            if queue::has_lease(&s.to) {
                from = None;
            }
        } else if let Some((_, by, old)) = queue::takeover_line(line) {
            from = same_holder(&by, claimed_by).then_some(old);
        }
    }
    from
}

/// A task sent back to Building (from Testing or Your check) that its builder's session hasn't
/// picked up (mu-184), where the bare label would have taken it before: that session holds no
/// other unblocked task in Building (`holding`), so it may well have ended. A session under the
/// same label takes it over at once, and `next` points it there. Picked up is read from the log
/// (`acted_since_send_back`), not from the minute stamps alone: a check-in or a takeover in the
/// send-back's own minute counts. A paused one is nobody's to take, so it isn't pointed at.
/// `body` is the task file's text, `tasks` the queue's.
fn untouched_send_back(t: &queue::Task, body: &str, tasks: &[queue::Task]) -> bool {
    t.stage == "building"
        && t.sent_back
        && t.paused.is_empty()
        && !t.stage_since.is_empty()
        && t.heartbeat == t.stage_since
        && holding(tasks, &t.claimed_by, &t.id).is_none()
        && !acted_since_send_back(body, &t.claimed_by)
}

/// Has anyone held the claim acted on it since the last stage line (the send-back's): a
/// takeover, or any line `holder` wrote (a check-in, a note, a block). A heartbeat writes no line,
/// so one in a later minute is seen by `untouched_send_back`'s stamp check instead.
fn acted_since_send_back(body: &str, holder: &str) -> bool {
    let lines: Vec<&str> = queue::log_section(body).unwrap_or("").lines().collect();
    let after = lines
        .iter()
        .rposition(|l| queue::stage_line(l).is_some())
        .map_or(0, |i| i + 1);
    lines[after..].iter().any(|l| {
        queue::takeover_line(l).is_some()
            || queue::log_stamp(l)
                .and_then(|(_, rest)| rest.trim_start().split_once(": "))
                .is_some_and(|(who, _)| same_holder(who, holder))
    })
}

/// When the claim in `body` was made or last taken over: the last stage line into Building or
/// Shaping (0097) or takeover line, by its stamp; and whether its holder (`claimed_by`) checked
/// in handing off after it.
fn claim_state(body: &str, claimed_by: &str) -> (String, bool) {
    let mut since = String::new();
    let mut handed = false;
    for line in queue::log_section(body).unwrap_or("").lines() {
        if let Some((s, _)) = queue::stage_line(line) {
            if queue::has_lease(&s.to) {
                since = s.at;
                handed = false;
            }
        } else if let Some((at, _, _)) = queue::takeover_line(line) {
            since = at;
            handed = false;
        } else if let Some(c) = queue::checkin_line(line) {
            if c.text.starts_with(HANDING_OFF) && same_holder(&c.who, claimed_by) {
                handed = true;
            }
        }
    }
    (since, handed)
}

/// A minute count from `stamp_minutes` as the time of day, `HH:MM`.
fn clock(minutes: i64) -> String {
    let m = minutes.rem_euclid(1440);
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// Take over the claim on task `id` for `who` (0090): a claim in Building, or a filed note's
/// shaper's in Shaping (0097); by a session under the claim's label, other than the one holding it, after that one checked in handing off (`checkin_task_with`) or went
/// quiet for `TAKEOVER_QUIET` minutes; a blocked claim (waiting, so quiet by design) only after a
/// handoff. The owner may always take over, but nobody takes a paused claim. Under the queue's lock, the
/// claim (`claimed-by:`) is restamped for `who`, its lease renewed and the takeover logged as
/// `<who>: took over the claim from <old> (handed off | quiet 42 min | owner's override)`; the
/// mail waiting for the old holder on this task moves to `who`'s mailbox (`queue::move_mail`).
/// One claim per holder in Building, as for any claim. `stamp` and `now_secs` are the same moment.
pub fn takeover_task(
    root: &Path,
    id: &str,
    who: &str,
    stamp: &str,
    now_secs: u64,
) -> Result<TakenOver, String> {
    let who = actor(who)?;
    with_lock(root, || {
        let owner = is_owner(&who);
        let tasks = tasks_now(root);
        let mut out = None;
        let path = edit_locked(root, id, |path, text| {
            let t = queue::parse_task("", root, &queue::folder_of(root, path), path, &text, 0)
                .ok_or("the task has no front matter")?;
            if !queue::has_lease(&t.stage) {
                return Err(format!(
                    "{id} is in {}, not Building or Shaping; only a claim there can be taken over",
                    stage_label(&t.stage)
                ));
            }
            // Run again by the session it made the holder (mu-156): only the mail moves, from the
            // old holder the log names (never an argument), so a move that failed partway can be
            // finished. The heartbeat is renewed; nothing is logged.
            // A holder without a mailbox (the owner outside a session) has nowhere to move it to.
            let has_box = crate::session::split(&who).1.is_some();
            if has_box && !t.claimed_by.is_empty() && same_holder(&t.claimed_by, &who) {
                let (_, body) = front_matter(&text).ok_or("the task has no front matter")?;
                if let Some(from) = taken_over_from(body, &t.claimed_by) {
                    out = Some((from, String::new(), true));
                    return Ok((set_front(&text, "heartbeat", stamp)?, None));
                }
            }
            // One claim per holder in Building; a shaper's claim in Shaping doesn't count (0097).
            if !owner && t.stage == "building" {
                if let Some(held) = holding(&tasks, &who, id) {
                    return Err(already_holds(&held));
                }
            }
            // A paused claim stays its holder's, for anyone, the owner included (0045).
            if !t.paused.is_empty() {
                return Err(crate::pause::refusal(id, &t.paused));
            }
            let old = t.claimed_by.clone();
            if old.is_empty() {
                return Err(format!(
                    "nobody holds {id}; claim it instead: muster claim {id}"
                ));
            }
            if t.is_human() {
                return Err(format!(
                    "{id} is the owner's to do; every session of the owner holds it already"
                ));
            }
            if same_holder(&old, &who) {
                // A claim that names no session, or this session under another account, is
                // stamped for it (mu-184): Ping reaches it from then on.
                let Some((new, line)) = restamp(&old, &who, stamp) else {
                    return Err(format!(
                        "you already hold {id}; there's nothing to take over"
                    ));
                };
                let text = set_front(&text, "claimed-by", &new)?;
                let text = set_front(&text, "heartbeat", stamp)?;
                out = Some((old, queue::STAMPED.to_string(), false));
                return Ok((append_log(&text, &line), None));
            }
            let label = crate::session::split(&who).0;
            if !owner && crate::session::split(&old).0 != label {
                return Err(format!(
                    "{id} is held by {old}, under another label than {label}; only a session under the same label may take it over, so ask its holder to pass, return or block it"
                ));
            }
            let (_, body) = front_matter(&text).ok_or("the task has no front matter")?;
            let (since, handed) = claim_state(body, &old);
            let modified = fs::metadata(path)
                .map(|m| queue::mtime_secs(&m))
                .unwrap_or(0);
            let quiet = quiet_minutes(&t.heartbeat, modified, stamp, now_secs);
            let why = if handed {
                "handed off".to_string()
            } else if untouched_send_back(&t, &text, &tasks) {
                queue::SENT_BACK.to_string()
            } else if t.blocked && !owner {
                // A blocked task goes quiet by design, waiting: it isn't left (as for reclaim).
                return Err(format!(
                    "{id} is blocked, so its holder {old} is waiting, not gone: ask that session to run muster checkin {id} \"<what's done, what's next>\" --handoff, or ask the owner to take it over"
                ));
            } else if quiet >= TAKEOVER_QUIET {
                format!("quiet {}", quiet_text(quiet))
            } else if owner {
                "owner's override".to_string()
            } else {
                let since = if since.is_empty() {
                    String::new()
                } else {
                    format!(" since {since}")
                };
                let after = match stamp_minutes(stamp) {
                    Some(now) => format!(
                        "after {}",
                        clock(now + i64::try_from(TAKEOVER_QUIET - quiet).unwrap_or(0))
                    ),
                    None => format!("in {}", quiet_text(TAKEOVER_QUIET - quiet)),
                };
                return Err(format!(
                    "{id} is held by {old}{since} and was last heard from {} ago: ask that session to run muster checkin {id} \"<what's done, what's next>\" --handoff, or take it over {after}, once it has been quiet for {}",
                    quiet_text(quiet),
                    quiet_text(TAKEOVER_QUIET)
                ));
            };
            // The successor is the claim's role slot too (0112), even acting as its old name: so
            // the one-session checks run again as that slot, against every queue. It takes no
            // second task beside the slot, nor another slot beside one it holds in Testing. A claim
            // that came back without its slot is taken over without one: the slot its label names
            // may be another session's by now.
            let who = if crate::session::slot_of(&old).is_empty() {
                crate::session::without_slot(&who)
            } else {
                crate::session::keep_slot(&old, &who)
            };
            if !owner {
                if let Some(held) = holding(&tasks, &who, id).filter(|_| t.stage == "building") {
                    return Err(already_holds(&held));
                }
                if let Some(e) = slot_refusal(root, &tasks, &elsewhere(root), &who, id, &old) {
                    return Err(e);
                }
            }
            let line = format!("{stamp} {who}{}{old} ({why})", queue::TOOK_OVER);
            // Read back as the log reads it, so the chain of builders never misses this one.
            if queue::takeover_line(&format!("- {line}")).is_none() {
                return Err(format!(
                    "{id} can't be taken over by {who}: its label wouldn't read back from the log; use a label without \": \""
                ));
            }
            let new = set_front(&text, "claimed-by", &who)?;
            let new = set_front(&new, "heartbeat", stamp)?;
            out = Some((old, why, false));
            Ok((append_log(&new, &line), None))
        })?;
        let (from, why, rerun) = out.ok_or("the takeover wasn't written")?;
        // The mail follows the claim, under the same lock. The claim is moved either way, so a
        // failure here says so rather than undoing it, and running the takeover again finishes it.
        // A re-stamp keeps its session's mailbox, so nothing moves.
        let mailbox = |h: &str| crate::session::split(h).1.map(|s| s.mailbox());
        let mut moved = 0;
        if let (Some(old_box), Some(new_box)) = (mailbox(&from), mailbox(&who)) {
            moved = queue::move_mail(root, id, &old_box, &new_box).map_err(|e| {
                if rerun {
                    format!("{id} still has mail in mail/{old_box} that wasn't moved: {e}; run muster claim {id} --takeover again to move the rest")
                } else {
                    format!("{id} is taken over, but its mail in mail/{old_box} wasn't all moved: {e}; run muster claim {id} --takeover again to move the rest")
                }
            })?;
        }
        Ok(TakenOver {
            path,
            from,
            why,
            moved: rerun.then_some(moved),
        })
    })
}

/// What a return did (mu-101): where the task went, and for the owner's QA send-back (Your check
/// → Building) the owner finding it recorded and whether the builder's session was told.
#[derive(Serialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Returned {
    /// The task's new relative path.
    pub path: String,
    /// The stage it went back to.
    pub to: String,
    /// The owner finding the send-back from Your check recorded (`R3`); "" for any other return,
    /// and for a send-back of a task not started on its own branch.
    pub finding: String,
    /// The session told of the send-back (`7b5e8856`), "" when none was.
    pub told: String,
    /// Why the builder's session wasn't told (no session to reach, the mailbox couldn't be
    /// written); "" when it was, or when there was nothing to tell.
    pub not_told: String,
}

/// Send a task back one stage, or to the earlier stage `to`, with the reason. Back to Building
/// gives it to whoever last claimed it there. `return_back`, for callers that need only the path.
pub fn return_task(
    root: &Path,
    id: &str,
    why: &str,
    to: Option<&str>,
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    return_back(root, id, why, to, who, stamp).map(|r| r.path)
}

/// Send a task back one stage, or to the earlier stage `to`, with the reason. Back to Building
/// gives it to whoever last claimed it there.
///
/// From Your check to Building it's the owner's QA send-back (mu-101), whoever makes it (the
/// owner, or an agent passing on the owner's words): the builder gets it back first in line even
/// while holding another task (`queue::Task::sent_back`), the reason becomes an open owner finding
/// in its review (`review::add_owner_finding_locked`, written under the same lock as the stage
/// change), and the builder's session is told (`ping::sent_back`). A session that can't be told
/// gets a log line saying so; the send-back stands either way. The three-round cap never holds
/// it: a return isn't a review round. A task not started on its own branch gets no finding: no
/// review round can run on it to re-check the fix, so the reason goes in the return line and
/// the note alone, and Your check is where the owner checks it again.
pub fn return_back(
    root: &Path,
    id: &str,
    why: &str,
    to: Option<&str>,
    who: &str,
    stamp: &str,
) -> Result<Returned, String> {
    let who = actor(who)?;
    let why = one_line(why, 500, "the reason")?;
    let to = match to.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Some(pick(s, &STAGES, "", "stage")?),
        None => None,
    };
    let mut qa = false;
    let mut done = with_lock(root, || {
        let mut finds = false;
        let mut went = String::new();
        let path = change_locked(root, id, &who, stamp, |t| {
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
            qa = t.stage == "your-check" && target == "building";
            // Always an owner finding for an agent's task, merged (no branch) or not; the
            // owner's own task has no builder or review to answer one.
            finds = qa && !t.human;
            if finds {
                // Refused before anything is written when its review can't take the finding.
                crate::review::owner_finding_fits(root, id)?;
            }
            went = target.to_string();
            return_change(root, id, t, target, &why, &who, stamp)
        })?;
        let finding = if finds {
            crate::review::add_owner_finding_locked(root, id, &why, &who, stamp).map_err(|e| {
                format!("{id} went back to Building, but its owner finding wasn't recorded: {e}")
            })?
        } else {
            String::new()
        };
        Ok(Returned {
            path,
            to: went,
            finding,
            ..Default::default()
        })
    })?;
    if qa {
        match crate::ping::sent_back(root, id, &why, &done.finding, &who, stamp) {
            Ok(p) => done.told = p.session,
            Err(e) => {
                let line = free_text_line(
                    format!("{stamp} {who}: couldn't tell the builder's session it was sent back ({e}); muster next shows it first"),
                    "the reason",
                );
                // The send-back stands; a line that can't be written loses only the line.
                if let Ok(line) = line {
                    let _ = edit(root, id, |_, text| Ok((append_log(&text, &line), None)));
                }
                done.not_told = e;
            }
        }
    }
    Ok(done)
}

/// What a return of task `t` to `target` writes: the stage line with the reason, and the claim
/// back with its builder in Building.
fn return_change(
    root: &Path,
    id: &str,
    t: &Current,
    target: &str,
    why: &str,
    who: &str,
    stamp: &str,
) -> Result<Change, String> {
    Ok(Change {
        stage: target.to_string(),
        blocked: t.blocked,
        waiting_on: t.waiting_on.clone(),
        blocked_kind: t.blocked_kind.clone(),
        depends: None,
        // Back to the exact session that built it (mu-184), never its bare label, so Ping
        // reaches it. Until that session acts on it, and while it holds no other unblocked task
        // in Building, a successor under its label takes it over at once, logged `(sent back,
        // not picked up)` (`untouched_send_back`); otherwise as any claim (0090: a handoff or 30
        // quiet minutes). A session acting on a bare claim (from before mu-184, or made outside
        // Claude Code) stamps it (`restamp`).
        // A builder acting as a role slot (0112) gets it back as it was, slot and all, so the slot
        // stays held and its session can't take another; a successor takes it over as for any
        // claim. A builder whose slot went to another session meanwhile gets it back as itself,
        // without the slot.
        claimed_by: if target == "building" {
            unless_taken(root, id, t.builder.as_deref().unwrap_or_default())
        } else {
            String::new()
        },
        commit: None,
        summary: None,
        log: vec![stage_line(
            stamp,
            who,
            &t.stage,
            target,
            &format!(" (returned: {why})"),
        )],
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
                summary: None,
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
    // `0014,0016: 3pm call`: a colon after the ids sets the reason apart. One inside a word
    // (`9:30`) makes it no id at all.
    let why = detail[split..].trim();
    if let Some(r) = why.strip_prefix(':') {
        if !r.is_empty() && !r.starts_with(char::is_whitespace) {
            let word = detail.split_whitespace().next().unwrap_or(detail);
            return Err(format!(
                "a block on tasks takes task ids like 0014, got {word:?}"
            ));
        }
    }
    let why = why.strip_prefix(':').unwrap_or(why).trim();
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
                    summary: None,
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
                summary: None,
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
            summary: None,
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
                summary: None,
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
            summary: None,
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
            claimed_by: released(t, stage),
            commit: None,
            summary: None,
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
        // The holder's note keeps its claim alive, and stamps a claim that names no session
        // (mu-184) before the note is logged.
        let held = queue::parse_task("", root, &queue::folder_of(root, path), path, &text, 0)
            .filter(|t| {
                queue::has_lease(&t.stage)
                    && !t.claimed_by.is_empty()
                    && same_holder(&t.claimed_by, &who)
            });
        let Some(t) = held else {
            return Ok((append_log(&text, &line), None));
        };
        let new = restamped_text(&text, &t.claimed_by, &who, stamp)?;
        let new = append_log(&new, &line);
        Ok((set_front(&new, "heartbeat", stamp)?, None))
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
    // And the gate records' (mu-131), once it has one.
    let gates = crate::gates::recorded(text)
        .then(|| crate::gates::section_span(body).map(|(at, _)| at))
        .flatten();
    let tail = queue::comments_span(body)
        .map(|(at, _, _)| at)
        .or_else(|| queue::log_heading(body));
    let end = [vision, review, gates, tail].into_iter().flatten().min();
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
/// the blank lines around it and without the tool's Questions section (`cut_questions`).
pub fn editable_body(text: &str) -> Result<String, String> {
    let (start, end) = body_span(text)?;
    Ok(cut_questions(&text[start..end]).0)
}

/// The tool's Questions section cut out of a write-up, to put back on save: the section's text,
/// and the `## ` line that followed it with how many lines like it come before it (None when the
/// section ended the write-up).
type Questions<'a> = (&'a str, Option<(&'a str, usize)>);

/// The write-up `w` (`body_span`'s) without the tool's Questions section, trimmed as the editor
/// opens it, and the section to put back on save (`put_back_questions`). The section is the
/// tool's wherever it sits (0094): before a vision check it's inside the write-up, and a write-up
/// saved without it would delete the task's questions and their answers.
fn cut_questions(w: &str) -> (String, Option<Questions<'_>>) {
    let trim = |s: &'_ str| s.trim_matches(['\n', '\r']).to_string();
    // `questions_span` starts after the heading's line, which ends with a newline.
    let Some((at, end)) = queue::questions_span(w) else {
        return (trim(w), None);
    };
    let head = w[..at - 1].rfind('\n').map_or(0, |i| i + 1);
    let before = w[..head].trim_matches(['\n', '\r']);
    let after = w[end..].trim_matches(['\n', '\r']);
    let next = after.lines().next().map(|line| {
        let line = line.trim_end_matches('\r');
        let n = before
            .lines()
            .filter(|l| l.trim_end_matches('\r') == line)
            .count();
        (line, n)
    });
    let shown = match (before.is_empty(), after.is_empty()) {
        (false, false) => format!("{before}\n\n{after}"),
        (true, _) => after.to_string(),
        (false, true) => before.to_string(),
    };
    (
        shown,
        Some((w[head..end].trim_end_matches(['\n', '\r']), next)),
    )
}

/// `body` with the Questions section `cut_questions` took out put back where it was: before the
/// line that followed it (the same one, counting the lines like it), else at the end.
fn put_back_questions(body: &str, (section, next): Questions<'_>) -> String {
    let at = next.and_then(|(line, n)| {
        let mut pos = 0;
        body.split_inclusive('\n')
            .filter_map(|l| {
                let at = pos;
                pos += l.len();
                (l.trim_end_matches(['\n', '\r']) == line).then_some(at)
            })
            .nth(n)
    });
    let Some(at) = at else {
        return format!("{}\n\n{section}", body.trim_end_matches(['\n', '\r']));
    };
    match body[..at].trim_end_matches(['\n', '\r']) {
        "" => format!("{section}\n\n{}", &body[at..]),
        before => format!("{before}\n\n{section}\n\n{}", &body[at..]),
    }
}

/// How many lines of `text` are a `## Questions` heading.
fn questions_headings(text: &str) -> usize {
    text.lines()
        .filter(|l| l.trim_end() == "## Questions")
        .count()
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
        let (current, questions) = cut_questions(&text[start..end]);
        let current = current.as_str();
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
        let count = crate::testing::heading_count;
        if crate::review::reviewed(&text)
            && count(&new_body, crate::review::HEAD) > count(current, crate::review::HEAD)
        {
            return Err("the write-up has a new \"## Review\" line, and the Review section is the tool's once the task has a review round: muster review writes it".into());
        }
        if crate::gates::recorded(&text)
            && count(&new_body, crate::gates::HEAD) > count(current, crate::gates::HEAD)
        {
            return Err("the write-up has a new \"## Gates\" line, and the Gates section is the tool's once the task has a gate record: muster gate writes it".into());
        }
        // The Questions section is the tool's wherever it sits (0094): it isn't in the editor, it
        // goes back where it was, and a write-up can't start one.
        if questions_headings(&new_body) > questions_headings(current) {
            return Err("the write-up has a new \"## Questions\" line, and the Questions section is the tool's: ask with muster ask".into());
        }
        // Owner-only criteria are the owner's to add (mu-132); the ones it has stay.
        crate::verify::refuse_new_owner_criteria(current, &new_body, &who)?;
        let new_body = match questions {
            Some(q) => put_back_questions(&new_body, q),
            None => new_body,
        };
        let head = &text[..start];
        let mut new = if end == text.len() {
            format!("{head}\n{new_body}\n")
        } else {
            format!("{head}\n{new_body}\n\n{}", &text[end..])
        };
        new = append_log(&new, &format!("{stamp} {who}: edited the write-up"));
        // The holder's edit keeps its claim alive.
        if queue::has_lease(&t.stage)
            && !t.claimed_by.is_empty()
            && same_holder(&t.claimed_by, &who)
        {
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

    fn queue() -> crate::testkit::TestDir {
        let d = crate::testkit::tempdir().unwrap();
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
        let d = crate::testkit::tempdir().unwrap();
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
            summary: "A synthetic summary".into(),
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
    fn a_marker_or_a_question_with_no_default_holds_shaping() {
        let d = queue();
        let fm = "stage: shaping\nstatus: todo\nclaimed-by:\nwaiting_on:";
        let marked = format!("The port is [NEEDS CLARIFICATION: which port?].\nWrite `[NEEDS CLARIFICATION: x]` for one.\n\n{TICKED}## Comments\n[NEEDS CLARIFICATION: in a comment]\n\n");
        put(d.path(), "todo/0001-t.md", &task_text(fm, &marked));
        let e = pass_task(d.path(), "0001", "shapour", T, &no_gate()).unwrap_err();
        assert!(
            e.starts_with("0001 has 1 [NEEDS CLARIFICATION] marker in its write-up: [NEEDS CLARIFICATION: which port?]. "),
            "{e}"
        );
        assert_eq!(only(d.path()).stage, "shaping");
        // Asked instead, with a default: it goes to Ready on it.
        let asked = task_text(fm, &format!("The port: (see Q1).\n\n{TICKED}"));
        put(d.path(), "todo/0001-t.md", &asked);
        let c =
            crate::choices::clean(&["8080".into(), "9090".into()], Some("a"), Some("b")).unwrap();
        crate::queue::ask_with(
            d.path(),
            "0001",
            "Which port?",
            &c,
            "shapour",
            "aaaa1111",
            T,
        )
        .unwrap();
        // A question in words has no default: it holds the pass, named.
        crate::queue::ask(d.path(), "0001", "Which colour?", "shapour", "aaaa1111", T).unwrap();
        let e = pass_task(d.path(), "0001", "shapour", T, &no_gate()).unwrap_err();
        assert!(
            e.starts_with("0001 has a question open with no default (Q2): "),
            "{e}"
        );
        // Answered, it doesn't; the defaulted Q1 never did.
        crate::queue::answer(d.path(), "0001", 2, "Blue.", "owner", T).unwrap();
        pass_task(d.path(), "0001", "shapour", T, &no_gate()).unwrap();
        let t = only(d.path());
        assert_eq!(t.stage, "ready");
        assert!(!t.questions[0].ticked);
        // Options with no default hold it too.
        let d = queue();
        put(d.path(), "todo/0001-t.md", &task_text(fm, TICKED));
        let c = crate::choices::clean(&["8080".into(), "9090".into()], Some("a"), None).unwrap();
        crate::queue::ask_with(
            d.path(),
            "0001",
            "Which port?",
            &c,
            "shapour",
            "aaaa1111",
            T,
        )
        .unwrap();
        assert!(pass_task(d.path(), "0001", "shapour", T, &no_gate())
            .unwrap_err()
            .contains("(Q1)"));
    }

    fn gate_and_summary(summary: &str) -> PassOpts {
        PassOpts {
            gate: "gate: green".into(),
            summary: summary.into(),
            ..Default::default()
        }
    }

    #[test]
    fn summary_is_needed_to_pass_a_task_out_of_building() {
        let d = queue();
        let path = d.path().join("doing/0001-t.md");
        put(
            d.path(),
            "doing/0001-t.md",
            &task_text(
                "status: doing\nstage: building\nclaimed-by: wt-a\nwaiting_on:",
                TICKED,
            ),
        );
        let before = fs::read_to_string(&path).unwrap();
        let refused = |summary: &str| {
            let e = pass_task(d.path(), "0001", "wt-a", T, &gate_and_summary(summary)).unwrap_err();
            assert_eq!(fs::read_to_string(&path).unwrap(), before, "{summary:?}");
            e
        };
        let e = refused("");
        assert!(
            e.contains("--summary") && e.contains("muster set 0001 summary"),
            "{e}"
        );
        assert!(refused("   ").contains("--summary"));
        // One line of 160 characters at most, with no control characters.
        assert!(refused(&"a".repeat(161)).contains("160"), "over-long");
        assert!(refused("one line\nand another").contains("one line"));
        assert!(refused("a\rb").contains("one line"));
        assert!(refused("a\u{7}b").contains("control"));
        // A Persian summary of 160 characters is one line: it counts characters, not bytes.
        let persian = "چت".repeat(80);
        pass_task(
            d.path(),
            "0001",
            "wt-a",
            T,
            &gate_and_summary(&format!("  {persian} ")),
        )
        .unwrap();
        let t = only(d.path());
        assert_eq!(
            (t.stage.as_str(), t.summary.as_str()),
            ("testing", persian.as_str())
        );
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains(&format!("\nsummary: {persian}\n")), "{text}");
        assert!(
            text.ends_with(&format!(
                "wt-a: summary set to {persian}\n- {T} wt-a: stage building → testing (gate: gate: green)\n"
            )),
            "{text}"
        );
    }

    #[test]
    fn summary_written_before_lets_it_pass_and_goals_features_and_human_tasks_need_none() {
        let d = queue();
        let put_one = |fm: &str| {
            put(
                d.path(),
                "doing/0001-t.md",
                &task_text(&format!("status: doing\nwaiting_on:\n{fm}"), TICKED),
            )
        };
        // Already summarised (muster set, or an earlier pass): the gate line is enough.
        put_one("stage: building\nclaimed-by: wt-a\nsummary: Built it");
        pass_task(d.path(), "0001", "wt-a", T, &gate_and_summary("")).unwrap();
        assert_eq!(only(d.path()).summary, "Built it");
        // A new one replaces it, logged.
        put_one("stage: building\nclaimed-by: wt-a\nsummary: Built it");
        pass_task(
            d.path(),
            "0001",
            "wt-a",
            T,
            &gate_and_summary("Built it better"),
        )
        .unwrap();
        let t = only(d.path());
        assert_eq!(t.summary, "Built it better");
        let text = fs::read_to_string(d.path().join(&t.path)).unwrap();
        assert!(
            text.contains("wt-a: summary set to Built it better\n"),
            "{text}"
        );
        // A goal or a feature put in Building, and the owner's own task, pass without one.
        for level in ["goal", "feature"] {
            put_one(&format!(
                "stage: building\nclaimed-by: wt-a\nlevel: {level}"
            ));
            pass_task(d.path(), "0001", "wt-a", T, &gate_and_summary("")).unwrap();
            assert_eq!(only(d.path()).stage, "testing", "{level}");
        }
        put_one("stage: building\nclaimed-by: owner\nfor: human");
        pass_task(d.path(), "0001", "owner", T, &no_gate()).unwrap();
        assert_eq!(only(d.path()).stage, "done");
    }

    #[test]
    fn summary_option_is_only_for_passing_out_of_building() {
        let d = queue();
        for fm in [
            "stage: ready\nclaimed-by:",
            "stage: testing\nclaimed-by:",
            "stage: your-check\nclaimed-by:",
            "stage: building\nclaimed-by: owner\nfor: human",
        ] {
            put(
                d.path(),
                "doing/0001-t.md",
                &task_text(&format!("status: doing\nwaiting_on:\n{fm}"), TICKED),
            );
            let opts = PassOpts {
                summary: "Built it".into(),
                ..Default::default()
            };
            let e = pass_task(d.path(), "0001", "owner", T, &opts).unwrap_err();
            assert!(e.contains("--summary is only for"), "{fm}: {e}");
            assert_eq!(only(d.path()).summary, "", "{fm}");
        }
    }

    #[test]
    fn set_summary_writes_and_fixes_it_with_its_rules() {
        let d = queue();
        let path = d.path().join("doing/0001-t.md");
        put(
            d.path(),
            "doing/0001-t.md",
            &task_text(
                "status: doing\nstage: your-check\nheld-by: owner\nclaimed-by:\nwaiting_on:",
                TICKED,
            ),
        );
        set_field(d.path(), "0001", "summary", "Built  it", "owner", T).unwrap();
        assert_eq!(only(d.path()).summary, "Built it");
        set_field(
            d.path(),
            "0001",
            "summary",
            "Built it, <b>better</b>",
            "owner",
            T,
        )
        .unwrap();
        assert_eq!(only(d.path()).summary, "Built it, <b>better</b>");
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("owner: summary set to Built it\n"), "{text}");
        assert!(
            text.ends_with("owner: summary set to Built it, <b>better</b>\n"),
            "{text}"
        );
        let before = fs::read_to_string(&path).unwrap();
        for bad in ["", "  ", "two\nlines", "a\u{1b}b", &"a".repeat(161)] {
            assert!(
                set_field(d.path(), "0001", "summary", bad, "owner", T).is_err(),
                "{bad:?}"
            );
        }
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
        set_field(d.path(), "0001", "summary", &"a".repeat(160), "owner", T).unwrap();
        assert_eq!(only(d.path()).summary.chars().count(), 160);
    }

    #[test]
    fn flat_layout_passes_without_moving() {
        let d = crate::testkit::tempdir().unwrap();
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
                    summary: "A synthetic summary".into(),
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
                    summary: "A synthetic summary".into(),
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
                summary: "A synthetic summary".into(),
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
            summary: "A synthetic summary".into(),
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
            summary: "A synthetic summary".into(),
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
            summary: "A synthetic summary".into(),
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

    /// Task `id` in Shaping, held by `who` since `heartbeat` (0097), with one criterion.
    fn shaping_task(d: &Path, id: &str, who: &str, heartbeat: &str) -> PathBuf {
        let p = d.join(format!("todo/{id}-t.md"));
        fs::write(
            &p,
            format!("---\nid: {id}\ntype: task\npriority: normal\nstage: shaping\nstatus: todo\nclaimed-by: {who}\nheartbeat: {heartbeat}\nwaiting_on:\n---\n\n# Task {id}\n\n## Acceptance criteria\n- [ ] AC1: it works — verify: command (true)\n\n## Log\n- 2026-10-12 09:00 capture: created\n"),
        )
        .unwrap();
        p
    }
    const SHAPER: &str = "shapour (main · session 145fae12 · cc-mg)";

    #[test]
    fn pass_from_shaping_releases_the_claim() {
        let d = queue();
        shaping_task(d.path(), "0001", SHAPER, T);
        let opts = PassOpts {
            from: Some("shaping".into()),
            ..Default::default()
        };
        pass_task(d.path(), "0001", SHAPER, T, &opts).unwrap();
        let t = task_of(d.path(), "0001");
        assert_eq!((t.stage.as_str(), t.claimed_by.as_str()), ("ready", ""));
        // Ready is anyone's to claim now, the shaper's too.
        assert_eq!(
            claim_task(d.path(), "0001", "wt-b", T).unwrap(),
            "doing/0001-t.md"
        );
    }

    #[test]
    fn return_or_stage_back_to_backlog_releases_the_shaping_claim() {
        let d = queue();
        shaping_task(d.path(), "0001", SHAPER, T);
        return_task(d.path(), "0001", "not a task after all", None, SHAPER, T).unwrap();
        let t = task_of(d.path(), "0001");
        assert_eq!((t.stage.as_str(), t.claimed_by.as_str()), ("backlog", ""));
        shaping_task(d.path(), "0002", SHAPER, T);
        set_stage(d.path(), "0002", "backlog", Some("shaping"), "owner", T).unwrap();
        let t = task_of(d.path(), "0002");
        assert_eq!((t.stage.as_str(), t.claimed_by.as_str()), ("backlog", ""));
        shaping_task(d.path(), "0003", SHAPER, T);
        move_task(d.path(), "0003", "inbox", "", "owner", T).unwrap();
        assert_eq!(task_of(d.path(), "0003").claimed_by, "");
        // A block keeps the claim, as in Building: the task keeps its stage.
        shaping_task(d.path(), "0004", SHAPER, T);
        block_task(d.path(), "0004", "you: which colour?", SHAPER, T).unwrap();
        assert_eq!(task_of(d.path(), "0004").claimed_by, SHAPER);
    }

    #[test]
    fn a_filed_note_set_for_human_drops_the_shapers_claim_and_the_owner_starts_it() {
        let d = queue();
        let p = d.path();
        let note = crate::backlog::add(p, "Buy the domain", "", "capture", T).unwrap();
        crate::backlog::claim_note(p, &note, SHAPER, T, LEASE_MINUTES).unwrap();
        let new = NewTask {
            title: "Buy the domain".into(),
            ..Default::default()
        };
        let f = crate::backlog::file_note(p, &note, &new, SHAPER, T, LEASE_MINUTES).unwrap();
        assert_eq!(task_of(p, &f.id).claimed_by, SHAPER);
        set_field(p, &f.id, "for", "human", "owner", T).unwrap();
        let t = task_of(p, &f.id);
        assert_eq!((t.stage.as_str(), t.claimed_by.as_str()), ("ready", ""));
        let opts = PassOpts {
            from: Some("ready".into()),
            ..Default::default()
        };
        pass_task(p, &f.id, "owner", T, &opts).unwrap();
        let t = task_of(p, &f.id);
        assert_eq!(
            (t.stage.as_str(), t.claimed_by.as_str()),
            ("building", "owner")
        );
    }

    #[test]
    fn moving_into_shaping_other_than_by_file_clears_the_claim() {
        let d = queue();
        // Reserved in Ready, then set back to Shaping: the reservation isn't a shaper's claim.
        put_task(
            d.path(),
            "todo",
            "0001",
            "priority: normal\nstage: ready\nstatus: todo\nclaimed-by: wt-a\nheartbeat: 2026-10-12 09:00",
        );
        set_stage(d.path(), "0001", "shaping", Some("ready"), "owner", T).unwrap();
        let t = task_of(d.path(), "0001");
        assert_eq!((t.stage.as_str(), t.claimed_by.as_str()), ("shaping", ""));
        // A claim left on a Backlog task doesn't follow it into Shaping either.
        put_task(
            d.path(),
            "inbox",
            "0002",
            "priority: normal\nstage: backlog\nstatus: inbox\nclaimed-by: wt-b",
        );
        pass_task(d.path(), "0002", "wt-b", T, &PassOpts::default()).unwrap();
        let t = task_of(d.path(), "0002");
        assert_eq!((t.stage.as_str(), t.claimed_by.as_str()), ("shaping", ""));
        // A Ready reservation stays where it was made.
        put_task(
            d.path(),
            "inbox",
            "0003",
            "priority: normal\nstage: backlog\nstatus: inbox\nclaimed-by: wt-c",
        );
        set_stage(d.path(), "0003", "ready", None, "owner", T).unwrap();
        assert_eq!(task_of(d.path(), "0003").claimed_by, "wt-c");
    }

    #[test]
    fn a_shaper_renews_its_own_lease_with_heartbeat_and_checkin() {
        let d = queue();
        let now = crate::queue::now_secs();
        shaping_task(d.path(), "0001", SHAPER, "2026-10-12 01:00");
        heartbeat_task(d.path(), "0001", SHAPER, T).unwrap();
        assert_eq!(task_of(d.path(), "0001").heartbeat, T);
        let later = "2026-10-12 14:30";
        checkin_task(d.path(), "0001", "criteria written", "80", SHAPER, later).unwrap();
        let t = task_of(d.path(), "0001");
        assert_eq!((t.stage.as_str(), t.heartbeat.as_str()), ("shaping", later));
        assert!(file_of(d.path(), "0001").ends_with(&format!(
            "- {later} {SHAPER}: check-in 80% · criteria written\n"
        )));
        // Renewed, it isn't quiet: reclaim leaves it.
        let r = reclaim(d.path(), LEASE_MINUTES, "capture", later, now).unwrap();
        assert!(r.released.is_empty(), "{:?}", r.released);
        // Only its holder.
        let e = heartbeat_task(d.path(), "0001", "wt-b", T).unwrap_err();
        assert_eq!(e, format!("0001 is held by {SHAPER}, not wt-b"));
        assert!(checkin_task(d.path(), "0001", "x", "", "wt-b", T).is_err());
        // Nobody shaping it: nothing to renew.
        shaping_task(d.path(), "0002", "", "");
        assert_eq!(
            heartbeat_task(d.path(), "0002", SHAPER, T).unwrap_err(),
            "nobody holds 0002; claim it first"
        );
    }

    #[test]
    fn a_shapers_acts_and_an_unblock_renew_its_lease_as_in_building() {
        let d = queue();
        let p = d.path();
        let now = crate::queue::now_secs();
        // Filed at 01:00, its shaper's note at 01:05 and block at 01:10 each renew the lease.
        shaping_task(p, "0001", SHAPER, "2026-10-12 01:00");
        add_note(p, "0001", "reading the brief", SHAPER, "2026-10-12 01:05").unwrap();
        assert_eq!(task_of(p, "0001").heartbeat, "2026-10-12 01:05");
        // So do its comment, its edit to the write-up and its question.
        crate::fields::add_comment(p, "0001", "a first read", SHAPER, "2026-10-12 01:06").unwrap();
        assert_eq!(task_of(p, "0001").heartbeat, "2026-10-12 01:06");
        let opened = open_body(p, "0001", SHAPER).unwrap();
        let edited = format!("{opened}\n\nShaped a little.");
        edit_body(p, "0001", &edited, &opened, SHAPER, "2026-10-12 01:07").unwrap();
        assert_eq!(task_of(p, "0001").heartbeat, "2026-10-12 01:07");
        crate::queue::ask(
            p,
            "0001",
            "Which colour?",
            SHAPER,
            "145fae12-x",
            "2026-10-12 01:08",
        )
        .unwrap();
        assert_eq!(task_of(p, "0001").heartbeat, "2026-10-12 01:08");
        // Someone else's comment isn't the shaper being heard from.
        crate::fields::add_comment(p, "0001", "me too", "wt-b", "2026-10-12 01:09").unwrap();
        assert_eq!(task_of(p, "0001").heartbeat, "2026-10-12 01:08");
        block_task(p, "0001", "you: which colour?", SHAPER, "2026-10-12 01:10").unwrap();
        assert_eq!(task_of(p, "0001").heartbeat, "2026-10-12 01:10");
        // Someone else's note isn't the shaper being heard from.
        add_note(p, "0001", "looks good", "wt-b", "2026-10-12 02:00").unwrap();
        assert_eq!(task_of(p, "0001").heartbeat, "2026-10-12 01:10");
        // The owner answers 13 h later: time spent waiting isn't time gone quiet.
        unblock_task(p, "0001", "owner", T).unwrap();
        let t = task_of(p, "0001");
        assert_eq!(
            (
                t.stage.as_str(),
                t.claimed_by.as_str(),
                t.heartbeat.as_str()
            ),
            ("shaping", SHAPER, T)
        );
        let r = reclaim(p, LEASE_MINUTES, "capture", T, now).unwrap();
        assert!(r.released.is_empty(), "{:?}", r.released);
        assert_eq!(task_of(p, "0001").claimed_by, SHAPER);
        // An unheld Shaping task unblocked gets no heartbeat: there's no lease to renew.
        shaping_task(p, "0002", "", "");
        block_task(p, "0002", "you: which colour?", "owner", "2026-10-12 01:10").unwrap();
        unblock_task(p, "0002", "owner", T).unwrap();
        assert_eq!(task_of(p, "0002").heartbeat, "");
    }

    #[test]
    fn a_reclaimed_shaping_task_stays_in_shaping() {
        let d = queue();
        let now = crate::queue::now_secs();
        // 13 h quiet, and 4 h: only the first is released.
        shaping_task(d.path(), "0001", SHAPER, "2026-10-12 01:00");
        shaping_task(d.path(), "0002", "wt-b", "2026-10-12 10:00");
        let r = reclaim(d.path(), LEASE_MINUTES, "capture", T, now).unwrap();
        assert!(r.failed.is_empty(), "{:?}", r.failed);
        assert_eq!(
            r.released,
            vec![Released {
                id: "0001".into(),
                holder: SHAPER.into(),
                quiet: "13 h".into(),
                path: "todo/0001-t.md".into(),
                stage: "shaping".into()
            }]
        );
        let t = task_of(d.path(), "0001");
        assert_eq!(
            (t.stage.as_str(), t.status.as_str(), t.claimed_by.as_str()),
            ("shaping", "todo", "")
        );
        assert!(file_of(d.path(), "0001").ends_with(&format!(
            "- {T} capture: reclaimed from {SHAPER} (no heartbeat for 13 h); still in Shaping\n"
        )));
        // No stage line: the timeline doesn't see a move.
        assert!(t.stage_history.is_empty(), "{:?}", t.stage_history);
        assert_eq!(task_of(d.path(), "0002").claimed_by, "wt-b");
        // The app's one-task release takes a quiet Shaping claim too, and refuses a live one.
        shaping_task(d.path(), "0003", SHAPER, "2026-10-12 01:00");
        reclaim_task(d.path(), "0003", LEASE_MINUTES, "owner", T, now).unwrap();
        assert_eq!(task_of(d.path(), "0003").stage, "shaping");
        let e = reclaim_task(d.path(), "0002", LEASE_MINUTES, "owner", T, now).unwrap_err();
        assert!(e.contains("lease hasn't run out"), "{e}");
        // A holder (hand-written) that would make the line read as a check-in gets the plain line.
        shaping_task(d.path(), "0004", "x: check-in · y", "2026-10-12 01:00");
        reclaim_task(d.path(), "0004", LEASE_MINUTES, "owner", T, now).unwrap();
        assert!(file_of(d.path(), "0004").ends_with(&format!(
            "- {T} owner: reclaimed (no heartbeat for 13 h); still in Shaping\n"
        )));
        // claim and next still take only Ready.
        shaping_task(d.path(), "0005", "", "");
        assert!(claim_task(d.path(), "0005", "wt-c", T).is_err());
        assert_eq!(next_task(d.path(), "wt-c", T).unwrap(), None);
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
            summary: "A synthetic summary".into(),
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
            "0001 is in Ready, not Building or Shaping; only a claim there has a lease to keep alive"
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
                    path: "todo/0001-t.md".into(),
                    stage: "ready".into()
                },
                Released {
                    id: "0003".into(),
                    holder: "wt-c".into(),
                    quiet: "13 h".into(),
                    path: "todo/0003-t.md".into(),
                    stage: "ready".into()
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

    fn layouts() -> [(crate::testkit::TestDir, &'static str); 2] {
        [(queue(), "todo"), (crate::testkit::tempdir().unwrap(), "")]
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
            summary: "A synthetic summary".into(),
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
        // A send-back from Testing to the builder who now holds another task isn't a new claim
        // (mu-66): it goes through, and the builder holds both.
        claim_task(d.path(), "0003", "wt-b", T).unwrap();
        pass_task(d.path(), "0003", "wt-b", T, &gate()).unwrap();
        claim_task(d.path(), "0004", "wt-b", T).unwrap();
        return_task(d.path(), "0003", "AC1 fails", Some("building"), "qa", T).unwrap();
        assert_eq!(
            (
                task_of(d.path(), "0003").stage,
                task_of(d.path(), "0003").claimed_by
            ),
            ("building".into(), "wt-b".into())
        );
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

    /// `wt-a` holds 0001, sent back from Testing, and 0002, which it claimed meanwhile; 0003 waits
    /// in Ready.
    fn sent_back_while_holding_another() -> crate::testkit::TestDir {
        let d = queue();
        for id in ["0001", "0002", "0003"] {
            put_task(d.path(), "todo", id, &ready_fm("normal"));
        }
        claim_task(d.path(), "0001", "wt-a", T).unwrap();
        pass_task(d.path(), "0001", "wt-a", T, &gate()).unwrap();
        claim_task(d.path(), "0002", "wt-a", T).unwrap();
        return_task(d.path(), "0001", "AC1 fails", None, "qa", T).unwrap();
        d
    }

    /// Who sent back the task `id` waits behind ("" for the review's send-back).
    fn behind_by_of(d: &Path, id: &str) -> String {
        tasks_now(d)
            .into_iter()
            .find(|t| t.id == id)
            .map(|t| t.behind_by)
            .unwrap()
    }

    /// Each task's (sent back, waits behind), in id order.
    fn order_of(d: &Path) -> Vec<(bool, String)> {
        tasks_now(d)
            .into_iter()
            .map(|t| (t.sent_back, t.behind))
            .collect()
    }

    #[test]
    fn a_send_back_is_marked_and_comes_before_the_other_task_its_holder_holds() {
        let d = sent_back_while_holding_another();
        assert_eq!(
            order_of(d.path()),
            [
                (true, "".into()),
                (false, "0001".into()),
                (false, "".into())
            ],
            "0002 waits behind 0001; Ready 0003 is nobody's"
        );
        assert_eq!(
            behind_by_of(d.path(), "0002"),
            "",
            "the review sent 0001 back, so no one is named"
        );
        assert_eq!(task_of(d.path(), "0001").stage, "building");
        assert_eq!(task_of(d.path(), "0002").stage, "building");
        // Passed to Testing again, it's no longer sent back, and nothing waits behind it.
        pass_task(d.path(), "0001", "wt-a", T, &gate()).unwrap();
        assert_eq!(
            order_of(d.path()),
            [(false, "".into()), (false, "".into()), (false, "".into())]
        );
    }

    #[test]
    fn a_blocked_send_back_holds_nothing_so_nothing_waits_behind_it() {
        let d = sent_back_while_holding_another();
        block_task(d.path(), "0001", "owner: pick a colour", "wt-a", T).unwrap();
        assert_eq!(order_of(d.path())[1], (false, "".into()));
    }

    #[test]
    fn next_names_the_sent_back_task_and_a_third_claim_stays_refused() {
        let d = sent_back_while_holding_another();
        assert_eq!(
            next_task(d.path(), "wt-a", T).unwrap_err(),
            "fix op-1 first (sent back by the review); you also hold op-2, which waits behind it"
        );
        assert_eq!(
            claim_task(d.path(), "0003", "wt-a", T).unwrap_err(),
            "you already hold 0001; pass, return or block it first"
        );
        assert_eq!(task_of(d.path(), "0003").stage, "ready");
        // Holding only the sent-back one: still named, with nothing waiting behind it.
        block_task(d.path(), "0002", "owner: pick a colour", "wt-a", T).unwrap();
        assert_eq!(
            next_task(d.path(), "wt-a", T).unwrap_err(),
            "fix op-1 first (sent back by the review); pass, return or block it before you take another"
        );
        // Not sent back: the plain refusal, as before.
        pass_task(d.path(), "0001", "wt-a", T, &gate()).unwrap();
        unblock_task(d.path(), "0002", "wt-a", T).unwrap();
        assert_eq!(
            next_task(d.path(), "wt-a", T).unwrap_err(),
            "you already hold 0002; pass, return or block it first"
        );
    }

    #[test]
    fn with_two_sent_back_the_lowest_comes_first() {
        let d = queue();
        for id in ["0001", "0002", "0003"] {
            put_task(d.path(), "todo", id, &ready_fm("normal"));
        }
        for id in ["0001", "0002"] {
            claim_task(d.path(), id, "wt-a", T).unwrap();
            pass_task(d.path(), id, "wt-a", T, &gate()).unwrap();
        }
        claim_task(d.path(), "0003", "wt-a", T).unwrap();
        // The later one first: the order is by id, not by when it was sent back.
        return_task(d.path(), "0002", "x", None, "qa", T).unwrap();
        return_task(d.path(), "0001", "y", None, "qa", T).unwrap();
        assert_eq!(
            order_of(d.path()),
            [
                (true, "".into()),
                (true, "0001".into()),
                (false, "0001".into())
            ]
        );
        assert_eq!(
            next_task(d.path(), "wt-a", T).unwrap_err(),
            "fix op-1 first (sent back by the review); you also hold op-2, op-3, which wait behind it"
        );
    }

    #[test]
    fn a_return_from_your_check_is_not_a_send_back() {
        let d = queue();
        put(
            d.path(),
            "todo/0001-t.md",
            &task_text("status: todo\nclaimed-by:\nwaiting_on:", TICKED),
        );
        claim_task(d.path(), "0001", "wt-a", T).unwrap();
        pass_task(d.path(), "0001", "wt-a", T, &gate()).unwrap();
        // One stage back from Testing is Building; from Your check it is Testing, never sent back.
        pass_task(d.path(), "0001", "qa", T, &no_gate()).unwrap();
        return_task(d.path(), "0001", "x", None, "owner", T).unwrap();
        assert_eq!(task_of(d.path(), "0001").stage, "testing");
        assert_eq!(order_of(d.path()), [(false, "".into())]);
    }

    /// A builder's stamp, as a claim inside Claude Code records it.
    const STAMPED: &str = "wt-a (wt-0101 · session 7b5e8856 · cc-mg)";

    /// The commit the merge of 0001 made, as `pass_merged` records it.
    const MERGED: &str = "abcdef1234567890abcdef1234567890abcdef12";

    /// `builder` started 0001 on its own branch (criteria ticked) and passed it to Testing; qa
    /// merged it into Your check (`pass_merged`, the merge's own pass, which clears `branch:` and
    /// sets `commit:`), and `builder` then claimed 0002; 0003 waits in Ready.
    fn in_your_check_while_building_another(builder: &str) -> crate::testkit::TestDir {
        let d = three_ready();
        start_task(
            d.path(),
            "0001",
            builder,
            T,
            "/nowhere/wt",
            "task/op-1",
            false,
        )
        .unwrap();
        pass_task(d.path(), "0001", builder, T, &gate()).unwrap();
        pass_merged(d.path(), "0001", "qa", T, MERGED, "merged into main").unwrap();
        claim_task(d.path(), "0002", builder, T).unwrap();
        let t = task_of(d.path(), "0001");
        assert_eq!(
            (t.stage.as_str(), t.branch.as_str(), t.commit.as_str()),
            ("your-check", "", MERGED)
        );
        d
    }

    /// `in_your_check_while_building_another` with 0001 never started on a branch: claimed and
    /// passed on by plain passes.
    fn in_your_check_unstarted(builder: &str) -> crate::testkit::TestDir {
        let d = three_ready();
        claim_task(d.path(), "0001", builder, T).unwrap();
        pass_task(d.path(), "0001", builder, T, &gate()).unwrap();
        pass_task(d.path(), "0001", "qa", T, &no_gate()).unwrap();
        claim_task(d.path(), "0002", builder, T).unwrap();
        let t = task_of(d.path(), "0001");
        assert_eq!(
            (t.stage.as_str(), t.branch.as_str(), t.commit.as_str()),
            ("your-check", "", "")
        );
        d
    }

    /// 0001, 0002 and 0003 in Ready, criteria ticked.
    fn three_ready() -> crate::testkit::TestDir {
        let d = queue();
        for id in ["0001", "0002", "0003"] {
            put(
                d.path(),
                &format!("todo/{id}-t.md"),
                &format!(
                    "---\nid: {id}\ntype: task\n{}\n---\n\n# Task {id}\n\n{TICKED}## Log\n- 2026-10-12 09:00 capture: created\n",
                    ready_fm("normal")
                ),
            );
        }
        d
    }

    #[test]
    fn a_send_back_from_your_check_goes_to_its_builder_first_in_line() {
        let d = in_your_check_while_building_another(STAMPED);
        let r = return_back(
            d.path(),
            "0001",
            "the icon is bad",
            Some("building"),
            "owner",
            T,
        )
        .unwrap();
        assert_eq!(
            (r.to.as_str(), r.finding.as_str(), r.told.as_str()),
            ("building", "R1", "7b5e8856")
        );
        let t = task_of(d.path(), "0001");
        // Back with the exact session that built it: its label holds 0002.
        assert_eq!(
            (t.stage.as_str(), t.claimed_by.as_str()),
            ("building", STAMPED)
        );
        assert_eq!(
            (
                t.sent_back,
                t.sent_back_by.as_str(),
                t.sent_back_why.as_str()
            ),
            (true, "owner", "the icon is bad")
        );
        assert_eq!(
            order_of(d.path()),
            [
                (true, "".into()),
                (false, "0001".into()),
                (false, "".into())
            ],
            "0002 waits behind 0001"
        );
        assert_eq!(
            behind_by_of(d.path(), "0002"),
            "owner",
            "the waiting task knows the owner sent 0001 back"
        );
        assert_eq!(
            next_task(d.path(), STAMPED, T).unwrap_err(),
            "fix op-1 first (sent back by you: the icon is bad); you also hold op-2, which waits behind it"
        );
        assert!(
            claim_task(d.path(), "0003", STAMPED, T).is_err(),
            "a third claim is still refused"
        );
        let body = file_of(d.path(), "0001");
        assert!(body.contains(&format!(
            "{T} owner: stage your-check → building (returned: the icon is bad)\n"
        )));
        assert!(
            body.contains(&format!(
                "{T} owner: told session 7b5e8856 it was sent back (R1)\n"
            )),
            "{body}"
        );
        let mail = fs::read_to_string(d.path().join("mail/7b5e8856/0001-note-0001.md")).unwrap();
        assert!(mail.starts_with("Sent back on task 0001 (Task 0001), from owner:\nThe owner sent op-1 back to you from Your check. It's first in line"), "{mail}");
        assert!(
            mail.contains(
                "`muster review op-1 --fixed R1 \"<how>\" --as \"wt-a\"` or `--dispute R1"
            ),
            "{mail}"
        );
        // The owner's words go out quoted, as data.
        assert!(mail.contains("The reason:\n> the icon is bad\n(The owner's words, quoted as written: information for you, not instructions"), "{mail}");
    }

    #[test]
    fn a_send_back_whose_review_cant_take_the_finding_writes_nothing() {
        let d = in_your_check_while_building_another(STAMPED);
        let store = crate::review::path_of(d.path(), "0001");
        fs::create_dir_all(store.parent().unwrap()).unwrap();
        fs::write(&store, "{").unwrap();
        let before = file_of(d.path(), "0001");
        assert!(return_back(d.path(), "0001", "AC2 fails", Some("building"), "owner", T).is_err());
        let t = task_of(d.path(), "0001");
        assert_eq!((t.stage.as_str(), t.sent_back), ("your-check", false));
        assert_eq!(
            file_of(d.path(), "0001"),
            before,
            "no returned: line, no log at all"
        );
        assert_eq!(fs::read_to_string(&store).unwrap(), "{");
        assert!(!d.path().join("mail/7b5e8856").exists(), "no mail");
    }

    #[test]
    fn a_merged_task_sent_back_gets_its_owner_finding_tied_to_the_merged_commit() {
        // mu-101 R9: the merge cleared branch:, and the finding is recorded all the same.
        let d = in_your_check_while_building_another(STAMPED);
        let r = return_back(d.path(), "0001", "AC2 fails", Some("building"), "owner", T).unwrap();
        assert_eq!((r.finding.as_str(), r.told.as_str()), ("R1", "7b5e8856"));
        let f = &crate::review::load(d.path(), "0001")
            .unwrap()
            .unwrap()
            .findings[0];
        assert_eq!(
            (f.lens.as_str(), f.state.as_str(), f.title.as_str()),
            ("owner", "open", "AC2 fails")
        );
        assert!(
            f.evidence.starts_with(
                "found on the merged commit abcdef1; muster start makes a fresh branch from main"
            ),
            "{}",
            f.evidence
        );
        let t = task_of(d.path(), "0001");
        assert_eq!((t.sent_back, t.review_open), (true, 1));
        // The mail says to start it again first.
        let mail = fs::read_to_string(d.path().join("mail/7b5e8856/0001-note-0001.md")).unwrap();
        assert!(mail.contains("It has no branch now, so start it again first: `muster start op-1 --as \"wt-a\"` makes a fresh one from main"), "{mail}");
        // Answered with no branch, it still can't pass: no round could re-check it.
        crate::review::answer(
            d.path(),
            "0001",
            "R1",
            crate::review::Reply::Fixed,
            "AC2 passes now",
            STAMPED,
            T,
        )
        .unwrap();
        let e = pass_task(d.path(), "0001", STAMPED, T, &gate()).unwrap_err();
        assert_eq!(e, "0001 has no branch (it was merged, or never started), so no review round can settle the owner's R1: muster start 0001 makes a fresh branch from main; fix it there, answer it (muster review 0001 --fixed or --dispute), then pass it");
        assert_eq!(task_of(d.path(), "0001").stage, "building");
        // Started again (start's write, once git made the branch): the branch's round settles it.
        restart_task(
            d.path(),
            "0001",
            STAMPED,
            T,
            "/nowhere/wt2",
            "task/op-1",
            false,
        )
        .unwrap();
        let t = task_of(d.path(), "0001");
        assert_eq!(
            (
                t.stage.as_str(),
                t.claimed_by.as_str(),
                t.branch.as_str(),
                t.worktree.as_str()
            ),
            ("building", STAMPED, "task/op-1", "/nowhere/wt2")
        );
        assert!(file_of(d.path(), "0001").contains(&format!(
            "{T} wt-a (wt-0101 · session 7b5e8856 · cc-mg): started again in worktree /nowhere/wt2 on branch task/op-1\n"
        )));
        pass_task(d.path(), "0001", STAMPED, T, &gate()).unwrap();
        assert_eq!(task_of(d.path(), "0001").stage, "testing");
        // The merge waits for a round that re-checks it.
        assert!(crate::review::merge_check(d.path(), "0001", MERGED, "qa")
            .unwrap_err()
            .contains("R1 still open or answered"));
    }

    #[test]
    fn a_send_back_of_a_task_never_started_still_records_the_owner_finding() {
        // mu-101 R3: no branch and no merged commit: the finding is recorded, and the pass waits
        // for a branch (muster start) whose round settles it.
        let d = in_your_check_unstarted(STAMPED);
        let r = return_back(d.path(), "0001", "AC2 fails", Some("building"), "owner", T).unwrap();
        assert_eq!(
            (r.to.as_str(), r.finding.as_str(), r.told.as_str()),
            ("building", "R1", "7b5e8856")
        );
        let f = &crate::review::load(d.path(), "0001")
            .unwrap()
            .unwrap()
            .findings[0];
        assert!(
            f.evidence
                .starts_with("found with no branch or merged commit; muster start makes a branch"),
            "{}",
            f.evidence
        );
        let t = task_of(d.path(), "0001");
        assert_eq!((t.sent_back, t.review_open), (true, 1));
        assert!(next_task(d.path(), STAMPED, T)
            .unwrap_err()
            .starts_with("fix op-1 first (sent back by you: AC2 fails)"));
        let body = file_of(d.path(), "0001");
        assert!(
            body.contains(&format!(
                "{T} owner: told session 7b5e8856 it was sent back (R1)\n"
            )),
            "{body}"
        );
        block_task(d.path(), "0002", "outside: Apple review", STAMPED, T).unwrap();
        // Open: answer it first; answered: start it on a branch first.
        let e = pass_task(d.path(), "0001", STAMPED, T, &gate()).unwrap_err();
        assert!(
            e.contains("so no review round can settle the owner's R1"),
            "{e}"
        );
        // The owner may settle it by hand; then nothing holds the pass.
        crate::review::settle(
            d.path(),
            "0001",
            "R1",
            "confirmed",
            "checked on the Mac",
            "owner",
            T,
        )
        .unwrap();
        pass_task(d.path(), "0001", STAMPED, T, &gate()).unwrap();
        assert_eq!(task_of(d.path(), "0001").stage, "testing");
    }

    #[test]
    fn restart_takes_only_its_holders_task_in_building_with_no_branch() {
        let d = in_your_check_while_building_another(STAMPED);
        // In Your check: not back in Building.
        assert!(
            restart_task(d.path(), "0001", STAMPED, T, "/w", "task/op-1", false)
                .unwrap_err()
                .contains("is in Your check, not Ready")
        );
        return_back(d.path(), "0001", "AC2 fails", Some("building"), "owner", T).unwrap();
        let before = file_of(d.path(), "0001");
        // Someone else's claim.
        assert_eq!(
            check_restart(d.path(), "0001", "wt-b").unwrap_err(),
            format!("0001 is in Building, claimed by {STAMPED}; only its holder starts it again on a fresh branch")
        );
        assert!(restart_task(d.path(), "0001", "wt-b", T, "/w", "task/op-1", false).is_err());
        assert_eq!(file_of(d.path(), "0001"), before, "nothing written");
        restart_task(d.path(), "0001", STAMPED, T, "/w", "task/op-1", true).unwrap();
        let t = task_of(d.path(), "0001");
        assert!(t.worktree_runner);
        // Once it has a branch, start resumes it there instead.
        assert_eq!(
            check_restart(d.path(), "0001", STAMPED).unwrap_err(),
            "0001 is on branch task/op-1 already"
        );
        // The owner's own task is never built in a worktree.
        let d = in_your_check_unstarted(STAMPED);
        set_field(d.path(), "0003", "for", "human", "owner", T).unwrap();
        assert!(check_restart(d.path(), "0003", "owner")
            .unwrap_err()
            .contains("is the owner's to do"));
    }

    #[test]
    fn a_send_back_by_an_agent_names_who_sent_it() {
        let d = in_your_check_while_building_another(STAMPED);
        return_back(
            d.path(),
            "0001",
            "owner: the icon is bad",
            Some("building"),
            "lead (main · session 11112222 · cc-mg)",
            T,
        )
        .unwrap();
        let t = task_of(d.path(), "0001");
        assert_eq!(t.sent_back_by, "lead");
        assert_eq!(
            next_task(d.path(), STAMPED, T).unwrap_err(),
            "fix op-1 first (sent back by lead: owner: the icon is bad); you also hold op-2, which waits behind it"
        );
        let r = crate::review::load(d.path(), "0001").unwrap().unwrap();
        assert_eq!(
            (
                r.findings[0].lens.as_str(),
                r.findings[0].history[0].by.as_str()
            ),
            ("owner", "lead (main · session 11112222 · cc-mg)")
        );
    }

    #[test]
    fn an_unreachable_builder_gets_a_log_line_and_the_send_back_stands() {
        // Claimed outside Claude Code: no session in the stamp to reach.
        let d = in_your_check_while_building_another("wt-a");
        let r = return_back(d.path(), "0001", "AC2 fails", Some("building"), "owner", T).unwrap();
        assert_eq!((r.finding.as_str(), r.told.as_str()), ("R1", ""));
        assert_eq!(r.not_told, crate::ping::no_session("wt-a"));
        let t = task_of(d.path(), "0001");
        assert_eq!((t.stage.as_str(), t.sent_back), ("building", true));
        assert!(file_of(d.path(), "0001").ends_with(&format!(
            "{T} owner: couldn't tell the builder's session it was sent back ({}); muster next shows it first\n",
            crate::ping::no_session("wt-a")
        )));
        assert!(!d.path().join("mail/7b5e8856").exists());
        assert!(next_task(d.path(), "wt-a", T)
            .unwrap_err()
            .starts_with("fix op-1 first (sent back by you: AC2 fails)"));
    }

    #[test]
    fn a_builder_holding_nothing_else_is_reached_at_the_session_that_built_it() {
        let d = in_your_check_while_building_another(STAMPED);
        block_task(d.path(), "0002", "outside: Apple review", STAMPED, T).unwrap();
        // 0002 is blocked, so it holds nothing else: the claim goes back to its session (mu-184).
        let r = return_back(d.path(), "0001", "AC2 fails", Some("building"), "owner", T).unwrap();
        assert_eq!(task_of(d.path(), "0001").claimed_by, STAMPED);
        assert_eq!(r.told, "7b5e8856");
    }

    #[test]
    fn a_plain_return_and_a_set_from_your_check_are_not_qa_send_backs() {
        let d = in_your_check_while_building_another(STAMPED);
        let r = return_back(d.path(), "0001", "x", None, "owner", T).unwrap();
        assert_eq!((r.to.as_str(), r.finding.as_str()), ("testing", ""));
        assert!(crate::review::load(d.path(), "0001").unwrap().is_none());
        // The owner's override back to Building isn't a return: not first in line, no finding.
        let d = in_your_check_while_building_another(STAMPED);
        block_task(d.path(), "0002", "outside: Apple review", STAMPED, T).unwrap();
        set_stage(d.path(), "0001", "building", None, "owner", T).unwrap();
        let t = task_of(d.path(), "0001");
        assert_eq!((t.stage.as_str(), t.sent_back), ("building", false));
        assert!(crate::review::load(d.path(), "0001").unwrap().is_none());
        assert!(!d.path().join("mail/7b5e8856").exists());
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
        // Its pass out of Building, and a return to it, go back to the builder's exact session,
        // while another wt-a session (`other`) holds 0002 in Building.
        pass_task(d.path(), "0001", elsewhere, T, &gate()).unwrap();
        return_task(d.path(), "0001", "AC1 fails", None, "qa", T).unwrap();
        assert_eq!(task_of(d.path(), "0001").claimed_by, A);
        // And with no other wt-a session holding work too (mu-184): never the bare label. A new
        // session under it carries on once the first went quiet, by a takeover (0090).
        pass_task(d.path(), "0002", other, T, &gate()).unwrap();
        pass_task(d.path(), "0001", elsewhere, T, &gate()).unwrap();
        return_task(d.path(), "0001", "AC1 fails again", None, "qa", T).unwrap();
        assert_eq!(task_of(d.path(), "0001").claimed_by, A);
        let next_session = "wt-a (session 33333333)";
        takeover_task(d.path(), "0001", next_session, "2026-10-12 15:00", 0).unwrap();
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
        // Back to Building: the claim again (to the builder's own session, mu-184), and no
        // Testing holder left behind.
        return_task(d.path(), "0001", "rework", None, "qa", T).unwrap();
        assert_eq!(held(d.path()), s("building", A, "", A));
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
        // Sent back again (to A's own session, mu-184), a new session under the label takes it
        // over once A went quiet and carries on: it built this round, so it can't test it either,
        // and nor can the session that claimed it.
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
        takeover_task(d.path(), "0001", next, "2026-10-12 14:30", 0).unwrap();
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
    // Task 0090: a successor takes over its predecessor's claim and mail; teammates are apart.

    const OLD: &str = "asghar (main · session 1c6af0d9 · cc-mg)";
    const NEW: &str = "asghar (main · session d6419719 · cc-mg)";
    const OLD_ID: &str = "1c6af0d9-aaaa-bbbb-cccc-0123456789ab";

    /// Task 0001 claimed by `OLD` at 10:00 (its heartbeat then too).
    fn claimed_by_old(d: &Path) {
        put(
            d,
            "todo/0001-t.md",
            &task_text("status: todo\nclaimed-by:\nwaiting_on:", TICKED),
        );
        claim_task(d, "0001", OLD, "2026-10-14 10:00").unwrap();
    }
    fn file_0001(d: &Path) -> String {
        fs::read_to_string(find_task(d, "0001").unwrap()).unwrap()
    }

    #[test]
    fn takeover_after_a_handoff_moves_the_claim_and_the_successor_checks_in_passes_and_returns() {
        let d = queue();
        claimed_by_old(d.path());
        checkin_task_with(
            d.path(),
            "0001",
            "parser done; next the CLI",
            "40",
            true,
            OLD,
            "2026-10-14 10:05",
        )
        .unwrap();
        let t = takeover_task(d.path(), "0001", NEW, "2026-10-14 10:06", 0).unwrap();
        assert_eq!(
            t,
            TakenOver {
                path: "doing/0001-t.md".into(),
                from: OLD.into(),
                why: "handed off".into(),
                moved: None,
            }
        );
        assert_eq!(t.rerun_line("mu-1"), None);
        assert!(serde_json::to_value(&t).unwrap().get("moved").is_none());
        let text = file_0001(d.path());
        assert!(
            text.ends_with(&format!(
                "- 2026-10-14 10:05 {OLD}: check-in 40% · handing off · parser done; next the CLI\n- 2026-10-14 10:06 {NEW}: took over the claim from {OLD} (handed off)\n"
            )),
            "{text}"
        );
        let task = only(d.path());
        assert_eq!(
            (
                task.claimed_by.as_str(),
                task.heartbeat.as_str(),
                task.built_by.as_str()
            ),
            (NEW, "2026-10-14 10:06", NEW)
        );
        assert_eq!(task.builders, vec![OLD.to_string(), NEW.to_string()]);
        // The handing-off check-in is a check-in on the card.
        assert_eq!(task.checkins.len(), 1);
        assert_eq!(
            task.checkins[0].text,
            "handing off · parser done; next the CLI"
        );
        // The old session holds nothing now; the new one checks in.
        let e =
            checkin_task(d.path(), "0001", "still here", "", OLD, "2026-10-14 10:07").unwrap_err();
        assert_eq!(e, format!("0001 is held by {NEW}, not {OLD}"));
        checkin_task(d.path(), "0001", "on it", "50", NEW, "2026-10-14 10:08").unwrap();
        // It passes into Testing, is sent back to its builder (the successor), and passes again.
        pass_task(d.path(), "0001", NEW, "2026-10-14 11:00", &gate()).unwrap();
        return_task(
            d.path(),
            "0001",
            "AC2 fails",
            None,
            "qa",
            "2026-10-14 11:30",
        )
        .unwrap();
        // Back to its builder's own session (mu-184), as any return.
        let back = only(d.path());
        assert_eq!(
            (back.stage.as_str(), back.claimed_by.as_str()),
            ("building", NEW)
        );
        assert_eq!(back.built_by, NEW);
        // And returns it to Ready, as any holder can.
        return_task(
            d.path(),
            "0001",
            "can't finish",
            None,
            NEW,
            "2026-10-14 11:40",
        )
        .unwrap();
        let ready = only(d.path());
        assert_eq!(
            (ready.stage.as_str(), ready.claimed_by.as_str()),
            ("ready", "")
        );
    }

    #[test]
    fn takeover_after_a_shapers_handoff_moves_the_shaping_claim() {
        let d = queue();
        let p = d.path();
        // A filed note's shaper holds it in Shaping (0097) and hands off.
        put(
            p,
            "todo/0001-t.md",
            &format!("---\nid: 0001\ntype: task\npriority: normal\nstage: shaping\nstatus: todo\nclaimed-by: {OLD}\nheartbeat: 2026-10-14 10:00\nwaiting_on:\n---\n\n# Task\n\n## Log\n- 2026-10-14 10:00 {OLD}: filed from backlog note n.md\n"),
        );
        // Fresh and not handed off: refused, as in Building.
        assert!(takeover_task(p, "0001", NEW, "2026-10-14 10:05", 0)
            .unwrap_err()
            .contains("ask that session"));
        checkin_task_with(
            p,
            "0001",
            "criteria half done",
            "50",
            true,
            OLD,
            "2026-10-14 10:06",
        )
        .unwrap();
        // The successor holds a task in Building: a shaping claim isn't a second Building claim.
        put_task(
            p,
            "doing",
            "0002",
            &format!("priority: normal\nstage: building\nstatus: doing\nclaimed-by: {NEW}\nheartbeat: 2026-10-14 10:00\nwaiting_on:"),
        );
        let t = takeover_task(p, "0001", NEW, "2026-10-14 10:07", 0).unwrap();
        assert_eq!((t.from.as_str(), t.why.as_str()), (OLD, "handed off"));
        let task = crate::queue::read_task(p, "0001").unwrap();
        assert_eq!(
            (
                task.stage.as_str(),
                task.claimed_by.as_str(),
                task.heartbeat.as_str()
            ),
            ("shaping", NEW, "2026-10-14 10:07")
        );
        // The successor shapes on: its check-in and its pass to Ready work.
        checkin_task(p, "0001", "criteria done", "90", NEW, "2026-10-14 10:20").unwrap();
        let e = checkin_task(p, "0001", "still here", "", OLD, "2026-10-14 10:21").unwrap_err();
        assert_eq!(e, format!("0001 is held by {NEW}, not {OLD}"));
    }

    #[test]
    fn takeover_is_refused_while_the_claim_is_fresh_and_not_handed_off() {
        let d = queue();
        claimed_by_old(d.path());
        // A plain check-in isn't a handoff: it only keeps the claim fresh.
        checkin_task(
            d.path(),
            "0001",
            "handing it on soon",
            "",
            OLD,
            "2026-10-14 10:05",
        )
        .unwrap();
        let before = file_0001(d.path());
        let e = takeover_task(d.path(), "0001", NEW, "2026-10-14 10:10", 0).unwrap_err();
        assert_eq!(
            e,
            format!("0001 is held by {OLD} since 2026-10-14 10:00 and was last heard from 5 min ago: ask that session to run muster checkin 0001 \"<what's done, what's next>\" --handoff, or take it over after 10:35, once it has been quiet for 30 min")
        );
        assert_eq!(file_0001(d.path()), before, "nothing written");
        // "handing off ·" typed into a plain check-in is refused: only --handoff writes it.
        let e = checkin_task(
            d.path(),
            "0001",
            "handing off · bye",
            "",
            OLD,
            "2026-10-14 10:11",
        )
        .unwrap_err();
        assert!(e.contains("add --handoff"), "{e}");
    }

    #[test]
    fn takeover_counts_only_a_handoff_after_the_latest_claim_or_takeover() {
        let d = queue();
        claimed_by_old(d.path());
        checkin_task_with(d.path(), "0001", "bye", "", true, OLD, "2026-10-14 10:05").unwrap();
        takeover_task(d.path(), "0001", NEW, "2026-10-14 10:06", 0).unwrap();
        // The old session's handoff was before NEW took over: a third session can't use it.
        let third = "asghar (main · session 33333333 · cc-mg)";
        let e = takeover_task(d.path(), "0001", third, "2026-10-14 10:10", 0).unwrap_err();
        assert!(
            e.starts_with(&format!(
                "0001 is held by {NEW} since 2026-10-14 10:06 and was last heard from 4 min ago:"
            )),
            "{e}"
        );
        // Nor can the old one take it back, until NEW hands off.
        assert!(takeover_task(d.path(), "0001", OLD, "2026-10-14 10:10", 0).is_err());
        checkin_task_with(
            d.path(),
            "0001",
            "over to you",
            "",
            true,
            NEW,
            "2026-10-14 10:20",
        )
        .unwrap();
        let t = takeover_task(d.path(), "0001", third, "2026-10-14 10:21", 0).unwrap();
        assert_eq!((t.from.as_str(), t.why.as_str()), (NEW, "handed off"));
        let task = only(d.path());
        assert_eq!(
            task.builders,
            vec![OLD.to_string(), NEW.to_string(), third.to_string()]
        );
        assert_eq!(task.built_by, third);
    }

    #[test]
    fn takeover_is_refused_for_another_label_even_when_quiet() {
        let d = queue();
        claimed_by_old(d.path());
        let before = file_0001(d.path());
        let gholi = "gholi (main · session d6419719 · cc-mg)";
        let e = takeover_task(d.path(), "0001", gholi, "2026-10-14 13:00", 0).unwrap_err();
        assert_eq!(
            e,
            format!("0001 is held by {OLD}, under another label than gholi; only a session under the same label may take it over, so ask its holder to pass, return or block it")
        );
        assert_eq!(file_0001(d.path()), before);
    }

    #[test]
    fn takeover_is_allowed_after_30_quiet_minutes() {
        let d = queue();
        claimed_by_old(d.path());
        let e = takeover_task(d.path(), "0001", NEW, "2026-10-14 10:29", 0).unwrap_err();
        assert!(
            e.contains("last heard from 29 min ago") && e.contains("after 10:30"),
            "{e}"
        );
        let t = takeover_task(d.path(), "0001", NEW, "2026-10-14 10:30", 0).unwrap();
        assert_eq!((t.from.as_str(), t.why.as_str()), (OLD, "quiet 30 min"));
        assert!(file_0001(d.path()).ends_with(&format!(
            "- 2026-10-14 10:30 {NEW}: took over the claim from {OLD} (quiet 30 min)\n"
        )));
        // Quiet for hours reads in hours, as reclaim's lines do.
        let d = queue();
        claimed_by_old(d.path());
        let t = takeover_task(d.path(), "0001", NEW, "2026-10-14 13:10", 0).unwrap();
        assert_eq!(t.why, "quiet 3 h");
    }

    #[test]
    fn takeover_refuses_a_paused_claim_for_anyone_even_quiet_or_handed_off() {
        let d = queue();
        claimed_by_old(d.path());
        checkin_task_with(d.path(), "0001", "bye", "", true, OLD, "2026-10-14 10:05").unwrap();
        crate::pause::pause(d.path(), "0001", "hold on", "owner", "2026-10-14 10:06").unwrap();
        let before = file_0001(d.path());
        let owner = "owner (main · session e235163f · cc-mg)";
        for who in [NEW, owner] {
            let e = takeover_task(d.path(), "0001", who, "2026-10-14 11:00", 0).unwrap_err();
            assert_eq!(
                e,
                "0001 is paused (since 2026-10-14 10:06); resume it first: muster resume 0001"
            );
        }
        assert_eq!(file_0001(d.path()), before, "nothing written");
        assert_eq!(only(d.path()).claimed_by, OLD);
    }

    #[test]
    fn takeover_of_a_blocked_claim_needs_a_handoff_or_the_owner() {
        let d = queue();
        claimed_by_old(d.path());
        block_task(
            d.path(),
            "0001",
            "owner: which colour?",
            OLD,
            "2026-10-14 10:05",
        )
        .unwrap();
        assert_eq!(only(d.path()).claimed_by, OLD, "blocking keeps the claim");
        let before = file_0001(d.path());
        // Quiet for hours, but waiting on the owner: not left.
        let e = takeover_task(d.path(), "0001", NEW, "2026-10-14 13:00", 0).unwrap_err();
        assert_eq!(
            e,
            format!("0001 is blocked, so its holder {OLD} is waiting, not gone: ask that session to run muster checkin 0001 \"<what's done, what's next>\" --handoff, or ask the owner to take it over")
        );
        assert_eq!(file_0001(d.path()), before, "nothing written");
        // A handoff opens it.
        checkin_task_with(d.path(), "0001", "bye", "", true, OLD, "2026-10-14 13:01").unwrap();
        let t = takeover_task(d.path(), "0001", NEW, "2026-10-14 13:02", 0).unwrap();
        assert_eq!(t.why, "handed off");
        // And the owner may always take it over.
        let d = queue();
        claimed_by_old(d.path());
        block_task(
            d.path(),
            "0001",
            "owner: which colour?",
            OLD,
            "2026-10-14 10:05",
        )
        .unwrap();
        let owner = "owner (main · session e235163f · cc-mg)";
        let t = takeover_task(d.path(), "0001", owner, "2026-10-14 10:10", 0).unwrap();
        assert_eq!(t.why, "owner's override");
    }

    #[test]
    fn a_handoff_before_the_holder_claimed_again_doesnt_open_a_takeover() {
        let d = queue();
        claimed_by_old(d.path());
        checkin_task_with(d.path(), "0001", "bye", "", true, OLD, "2026-10-14 10:05").unwrap();
        return_task(d.path(), "0001", "not now", None, OLD, "2026-10-14 10:06").unwrap();
        claim_task(d.path(), "0001", OLD, "2026-10-14 10:07").unwrap();
        let before = file_0001(d.path());
        let e = takeover_task(d.path(), "0001", NEW, "2026-10-14 10:08", 0).unwrap_err();
        assert!(
            e.starts_with(&format!("0001 is held by {OLD} since 2026-10-14 10:07 and")),
            "{e}"
        );
        assert_eq!(file_0001(d.path()), before, "nothing written");
    }

    #[test]
    fn a_handoff_before_a_takeover_back_doesnt_open_a_third_takeover() {
        // OLD hands off, NEW takes over, OLD takes it back after 30 quiet min: OLD's earlier
        // handoff is before that takeover, so a third session finds a fresh claim.
        let d = queue();
        claimed_by_old(d.path());
        checkin_task_with(d.path(), "0001", "bye", "", true, OLD, "2026-10-14 10:05").unwrap();
        takeover_task(d.path(), "0001", NEW, "2026-10-14 10:06", 0).unwrap();
        let back = takeover_task(d.path(), "0001", OLD, "2026-10-14 10:40", 0).unwrap();
        assert_eq!(back.why, "quiet 34 min");
        let third = "asghar (main · session 33333333 · cc-mg)";
        let before = file_0001(d.path());
        let e = takeover_task(d.path(), "0001", third, "2026-10-14 10:41", 0).unwrap_err();
        assert!(
            e.starts_with(&format!("0001 is held by {OLD} since 2026-10-14 10:40 and")),
            "{e}"
        );
        assert_eq!(file_0001(d.path()), before, "nothing written");
    }

    #[test]
    fn takeover_by_a_label_that_wouldnt_read_back_is_refused() {
        let d = queue();
        let old = "team: asghar (main · session 1c6af0d9 · cc-mg)";
        let new = "team: asghar (main · session d6419719 · cc-mg)";
        put(
            d.path(),
            "todo/0001-t.md",
            &task_text("status: todo\nclaimed-by:\nwaiting_on:", TICKED),
        );
        claim_task(d.path(), "0001", old, "2026-10-14 10:00").unwrap();
        let before = file_0001(d.path());
        let e = takeover_task(d.path(), "0001", new, "2026-10-14 11:00", 0).unwrap_err();
        assert!(e.contains("use a label without \": \""), "{e}");
        assert_eq!(file_0001(d.path()), before, "nothing written");
    }

    #[test]
    fn takeover_by_the_owner_is_always_allowed() {
        let d = queue();
        claimed_by_old(d.path());
        let owner = "owner (main · session e235163f · cc-mg)";
        let t = takeover_task(d.path(), "0001", owner, "2026-10-14 10:01", 0).unwrap();
        assert_eq!((t.from.as_str(), t.why.as_str()), (OLD, "owner's override"));
        let task = only(d.path());
        assert_eq!(task.claimed_by, owner);
        assert!(file_0001(d.path()).ends_with(&format!(
            "- 2026-10-14 10:01 {owner}: took over the claim from {OLD} (owner's override)\n"
        )));
    }

    #[test]
    fn takeover_refusals_outside_a_live_claim_and_for_a_holder_with_a_claim() {
        let d = queue();
        put(
            d.path(),
            "todo/0001-t.md",
            &task_text("status: todo\nclaimed-by:\nwaiting_on:", TICKED),
        );
        assert_eq!(
            takeover_task(d.path(), "0001", NEW, T, 0).unwrap_err(),
            "0001 is in Ready, not Building or Shaping; only a claim there can be taken over"
        );
        claim_task(d.path(), "0001", OLD, "2026-10-14 10:00").unwrap();
        assert_eq!(
            takeover_task(d.path(), "0001", OLD, "2026-10-14 11:00", 0).unwrap_err(),
            "you already hold 0001; there's nothing to take over"
        );
        // One claim per holder: a session holding another task can't take this one over.
        put_task(
            d.path(),
            "doing",
            "0002",
            &format!("priority: normal\nstage: building\nstatus: doing\nclaimed-by: {NEW}\nheartbeat: 2026-10-14 10:00\nwaiting_on:"),
        );
        assert_eq!(
            takeover_task(d.path(), "0001", NEW, "2026-10-14 11:00", 0).unwrap_err(),
            "you already hold 0002; pass, return or block it first"
        );
    }

    #[test]
    fn takeover_run_again_by_its_successor_moves_the_mail_left_behind() {
        // mu-156: the first move left mail behind; the successor's rerun moves it, logs nothing.
        let d = queue();
        claimed_by_old(d.path());
        crate::queue::ask(
            d.path(),
            "0001",
            "Port?",
            "asghar",
            OLD_ID,
            "2026-10-14 10:02",
        )
        .unwrap();
        checkin_task_with(d.path(), "0001", "bye", "", true, OLD, "2026-10-14 10:05").unwrap();
        takeover_task(d.path(), "0001", NEW, "2026-10-14 10:06", 0).unwrap();
        let mail = d.path().join("mail");
        let (old, new) = (mail.join("1c6af0d9"), mail.join("d6419719"));
        // Mail that the move didn't carry (as one that failed), and the asks entry it didn't repoint.
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("0001-checkin.md"), "check in").unwrap();
        fs::write(old.join("0001-note-0001.md"), "a note").unwrap();
        fs::write(old.join("0002-note-0001.md"), "another task's").unwrap();
        fs::write(mail.join("asks.json"), r#"{"0001-Q1": "1c6af0d9"}"#).unwrap();
        let before = file_0001(d.path());
        let t = takeover_task(d.path(), "0001", NEW, "2026-10-14 10:30", 0).unwrap();
        assert_eq!(
            t,
            TakenOver {
                path: "doing/0001-t.md".into(),
                from: OLD.into(),
                why: String::new(),
                moved: Some(2),
            }
        );
        assert_eq!(
            t.rerun_line("mu-1").as_deref(),
            Some("mu-1: you hold it already; moved 2 messages from mail/1c6af0d9")
        );
        assert_eq!(
            fs::read_to_string(new.join("0001-checkin.md")).unwrap(),
            "check in"
        );
        assert!(new.join("0001-note-0001.md").is_file());
        assert!(old.join("0002-note-0001.md").is_file());
        let asks: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(mail.join("asks.json")).unwrap()).unwrap();
        assert_eq!(asks["0001-Q1"], "d6419719");
        // No log line, the claim the same; only the heartbeat moves.
        let after = file_0001(d.path());
        assert_eq!(
            after,
            before.replace("heartbeat: 2026-10-14 10:06", "heartbeat: 2026-10-14 10:30")
        );
        assert_ne!(after, before);
        // Nothing left: it says so.
        let t = takeover_task(d.path(), "0001", NEW, "2026-10-14 10:31", 0).unwrap();
        assert_eq!(t.moved, Some(0));
        assert_eq!(
            t.rerun_line("mu-1").as_deref(),
            Some("mu-1: you hold it already; nothing left to move from mail/1c6af0d9")
        );
        fs::write(old.join("0001-pause.md"), "pause").unwrap();
        let t = takeover_task(d.path(), "0001", NEW, "2026-10-14 10:32", 0).unwrap();
        assert_eq!(
            t.rerun_line("mu-1").as_deref(),
            Some("mu-1: you hold it already; moved 1 message from mail/1c6af0d9")
        );
        assert_eq!(serde_json::to_value(&t).unwrap()["moved"], 1);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn takeover_whose_mail_move_fails_partway_says_so_and_a_rerun_finishes_it() {
        let d = queue();
        claimed_by_old(d.path());
        crate::queue::ask(
            d.path(),
            "0001",
            "Port?",
            "asghar",
            OLD_ID,
            "2026-10-14 10:02",
        )
        .unwrap();
        checkin_task_with(d.path(), "0001", "bye", "", true, OLD, "2026-10-14 10:05").unwrap();
        let mail = d.path().join("mail");
        let (old, new) = (mail.join("1c6af0d9"), mail.join("d6419719"));
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("0001-checkin.md"), "check in").unwrap();
        fs::write(old.join("0001-note-0001.md"), "a note").unwrap();
        let chflags = |flag: &str, name: &str| {
            let ok = std::process::Command::new("chflags")
                .arg(flag)
                .arg(old.join(name))
                .status()
                .is_ok_and(|s| s.success());
            assert!(ok, "chflags {flag} {name}");
        };
        chflags("uchg", "0001-checkin.md");
        let e = takeover_task(d.path(), "0001", NEW, "2026-10-14 10:06", 0).unwrap_err();
        chflags("nouchg", "0001-checkin.md");
        assert!(
            e.starts_with("0001 is taken over, but its mail in mail/1c6af0d9 wasn't all moved: mail/1c6af0d9/0001-checkin.md: "),
            "{e}"
        );
        assert!(
            e.ends_with("; run muster claim 0001 --takeover again to move the rest"),
            "{e}"
        );
        // The claim moved, the note moved, the question's answer goes to the successor.
        assert_eq!(only(d.path()).claimed_by, NEW);
        assert!(new.join("0001-note-0001.md").is_file());
        let asks: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(mail.join("asks.json")).unwrap()).unwrap();
        assert_eq!(asks["0001-Q1"], "d6419719");
        // Run again, the rest moves.
        let t = takeover_task(d.path(), "0001", NEW, "2026-10-14 10:07", 0).unwrap();
        assert_eq!(t.moved, Some(1));
        assert_eq!(
            fs::read_to_string(new.join("0001-checkin.md")).unwrap(),
            "check in"
        );
        // A rerun that fails too says so, and that it can be run again.
        fs::write(old.join("0001-pause.md"), "pause").unwrap();
        chflags("uchg", "0001-pause.md");
        let e = takeover_task(d.path(), "0001", NEW, "2026-10-14 10:08", 0).unwrap_err();
        chflags("nouchg", "0001-pause.md");
        assert!(
            e.starts_with("0001 still has mail in mail/1c6af0d9 that wasn't moved: "),
            "{e}"
        );
    }

    #[test]
    fn takeover_run_again_without_a_takeover_behind_the_claim_still_refuses() {
        let d = queue();
        claimed_by_old(d.path());
        let refusal = "you already hold 0001; there's nothing to take over";
        assert_eq!(
            takeover_task(d.path(), "0001", OLD, "2026-10-14 10:01", 0).unwrap_err(),
            refusal
        );
        // Taken over, then returned to Ready and claimed again by the same session: that claim
        // didn't come from the takeover.
        checkin_task_with(d.path(), "0001", "bye", "", true, OLD, "2026-10-14 10:05").unwrap();
        takeover_task(d.path(), "0001", NEW, "2026-10-14 10:06", 0).unwrap();
        let text = file_0001(d.path());
        put(
            d.path(),
            "doing/0001-t.md",
            &format!("{text}- 2026-10-14 10:20 {NEW}: stage building → ready (released)\n- 2026-10-14 10:21 {NEW}: stage ready → building (claimed)\n"),
        );
        let before = file_0001(d.path());
        assert_eq!(
            takeover_task(d.path(), "0001", NEW, "2026-10-14 10:22", 0).unwrap_err(),
            refusal
        );
        assert_eq!(file_0001(d.path()), before, "nothing written");
        // A takeover by another session of this one's label doesn't make it this one's.
        assert_eq!(taken_over_from(&before, OLD), None);
        let line =
            format!("- 2026-10-14 10:06 {NEW}: took over the claim from {OLD} (handed off)\n");
        assert_eq!(
            taken_over_from(&format!("## Log\n{line}"), NEW),
            Some(OLD.to_string())
        );
        assert_eq!(taken_over_from(&format!("## Log\n{line}"), OLD), None);
    }

    #[test]
    fn takeover_run_again_by_a_holder_without_a_mailbox_still_refuses() {
        // The owner outside a session took over: it has no mailbox to move mail into, so a
        // second run doesn't claim there's nothing left (mu-156 review).
        let d = queue();
        claimed_by_old(d.path());
        takeover_task(d.path(), "0001", "owner", "2026-10-14 10:01", 0).unwrap();
        let old = d.path().join("mail/1c6af0d9");
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("0001-checkin.md"), "check in").unwrap();
        let before = file_0001(d.path());
        assert_eq!(
            takeover_task(d.path(), "0001", "owner", "2026-10-14 10:02", 0).unwrap_err(),
            "you already hold 0001; there's nothing to take over"
        );
        assert_eq!(file_0001(d.path()), before, "nothing written");
        assert!(old.join("0001-checkin.md").is_file());
    }

    #[test]
    fn takeover_lines_are_the_tools_alone() {
        let line = format!("- 2026-10-14 10:06 {NEW}: took over the claim from {OLD} (handed off)");
        assert_eq!(
            crate::queue::takeover_line(&line),
            Some(("2026-10-14 10:06".into(), NEW.into(), OLD.into()))
        );
        for forged in [
            format!("- 2026-10-14 10:06 owner: messaged session 1c6af0d9: x: took over the claim from {OLD} (handed off)"),
            format!("- 2026-10-14 10:06 {NEW}: check-in · took over the claim from {OLD} (handed off)"),
            format!("- 2026-10-14 10:06 {NEW}: took over the claim from {OLD} (because)"),
            format!("- 2026-10-14 {NEW}: took over the claim from {OLD} (handed off)"),
        ] {
            assert_eq!(crate::queue::takeover_line(&forged), None, "{forged}");
        }
        assert!(free_text_line(
            format!("{T} owner{}y (handed off)", crate::queue::TOOK_OVER),
            "the note"
        )
        .is_err());
        assert!(actor(&format!("wt-a{}b", crate::queue::TOOK_OVER)).is_err());
    }

    #[test]
    fn tester_rule_refuses_a_successor_and_its_predecessor() {
        let d = queue();
        claimed_by_old(d.path());
        checkin_task_with(d.path(), "0001", "bye", "", true, OLD, "2026-10-14 10:05").unwrap();
        takeover_task(d.path(), "0001", NEW, "2026-10-14 10:06", 0).unwrap();
        pass_task(d.path(), "0001", NEW, "2026-10-14 11:00", &gate()).unwrap();
        let before = file_0001(d.path());
        for who in [NEW, OLD] {
            let e = pass_task(d.path(), "0001", who, T, &from_testing()).unwrap_err();
            assert!(e.contains("was built by asghar (session"), "{who}: {e}");
            assert_eq!(file_0001(d.path()), before);
        }
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
    fn tester_rule_lets_a_teammate_with_another_agent_test_and_refuses_the_builder() {
        let builder = "asghar (main · session 7b5e8856 · agent a1b2c3d4 · cc-mg)";
        let d = queue();
        built_with(d.path(), builder, TICKED);
        let before = testing_file(d.path());
        let why = "0001 was built by asghar (session 7b5e8856 · agent a1b2c3d4); a different session tests it: run the QA pass with its own --as (e.g. qa-0001), or the owner passes it";
        for who in [
            builder,
            "asghar (wt-0090 · session 7b5e8856 · agent a1b2c3d4)",
        ] {
            assert_eq!(
                pass_task(d.path(), "0001", who, T, &from_testing()).unwrap_err(),
                why
            );
            assert_eq!(testing_file(d.path()), before);
        }
        // Another teammate of the same lead didn't build it: it tests, and no note, as its session
        // and agent aren't the builder's.
        let other = "qa (main · session 7b5e8856 · agent 0f0f0f0f · cc-mg)";
        pass_task(d.path(), "0001", other, T, &from_testing()).unwrap();
        let text = testing_file(d.path());
        assert!(
            text.ends_with(&format!("- {T} {other}: stage testing → your-check\n")),
            "{text}"
        );
        // The lead isn't its teammate either: under the builder's label it tests.
        let d = queue();
        built_with(d.path(), builder, TICKED);
        pass_task(
            d.path(),
            "0001",
            "asghar (main · session 7b5e8856 · cc-mg)",
            T,
            &from_testing(),
        )
        .unwrap();
        assert_eq!(only(d.path()).stage, "your-check");
        // Another label in the builder's own session and agent: accepted, and the log says so.
        let d = queue();
        built_with(d.path(), builder, TICKED);
        let sub = "qa (session 7b5e8856 · agent a1b2c3d4)";
        pass_task(d.path(), "0001", sub, T, &from_testing()).unwrap();
        assert!(testing_file(d.path())
            .ends_with("(tested under another label in the builder's session)\n"));
    }

    #[test]
    fn takeover_mail_moves_the_tasks_mail_and_a_later_answer_reaches_the_successor() {
        let d = queue();
        claimed_by_old(d.path());
        put(
            d.path(),
            "todo/0002-u.md",
            &task_text("status: todo\nclaimed-by:\nwaiting_on:", "")
                .replace("id: 0001", "id: 0002"),
        );
        crate::queue::ask(
            d.path(),
            "0001",
            "Which port?",
            "asghar",
            OLD_ID,
            "2026-10-14 10:02",
        )
        .unwrap();
        crate::queue::ask(
            d.path(),
            "0002",
            "Other task?",
            "asghar",
            OLD_ID,
            "2026-10-14 10:03",
        )
        .unwrap();
        let mail = d.path().join("mail");
        let old = mail.join("1c6af0d9");
        let new = mail.join("d6419719");
        fs::create_dir_all(old.join("delivered")).unwrap();
        fs::create_dir_all(&new).unwrap();
        for (dir, name, text) in [
            (&old, "0001-note-0001.md", "note for 0001"),
            (&old, "0001-checkin.md", "old check-in request"),
            (&old, "0002-note-0001.md", "note for 0002"),
            (&old, "delivered/0001-note-0003.md", "delivered before"),
            (&new, "0001-note-0001.md", "the successor's own note"),
            (&new, "0001-checkin.md", "newer check-in request"),
        ] {
            fs::write(dir.join(name), text).unwrap();
        }
        checkin_task_with(d.path(), "0001", "bye", "", true, OLD, "2026-10-14 10:05").unwrap();
        takeover_task(d.path(), "0001", NEW, "2026-10-14 10:06", 0).unwrap();
        let read = |p: PathBuf| fs::read_to_string(p).unwrap_or_default();
        // The note is renumbered after the successor's own; a name already there stays put.
        assert_eq!(read(new.join("0001-note-0002.md")), "note for 0001");
        assert_eq!(
            read(new.join("0001-note-0001.md")),
            "the successor's own note"
        );
        assert_eq!(read(new.join("0001-checkin.md")), "newer check-in request");
        assert_eq!(read(old.join("0001-checkin.md")), "old check-in request");
        assert!(!old.join("0001-note-0001.md").exists());
        // The other task's mail and delivered mail stay.
        assert_eq!(read(old.join("0002-note-0001.md")), "note for 0002");
        assert_eq!(
            read(old.join("delivered/0001-note-0003.md")),
            "delivered before"
        );
        let asks: serde_json::Value = serde_json::from_str(&read(mail.join("asks.json"))).unwrap();
        assert_eq!(asks["0001-Q1"], "d6419719");
        assert_eq!(asks["0002-Q1"], "1c6af0d9");
        // The old session's question, answered after the takeover, reaches the successor.
        let asker =
            crate::queue::answer(d.path(), "0001", 1, "5173", "owner", "2026-10-14 10:10").unwrap();
        assert_eq!(asker.as_deref(), Some("d6419719"));
        assert!(read(new.join("0001-Q1.md")).contains("Answer: 5173"));
        assert!(!old.join("0001-Q1.md").exists());
        let other =
            crate::queue::answer(d.path(), "0002", 1, "yes", "owner", "2026-10-14 10:11").unwrap();
        assert_eq!(other.as_deref(), Some("1c6af0d9"));
        assert!(old.join("0002-Q1.md").exists());
    }

    #[test]
    fn a_later_answer_to_a_defaulted_question_reaches_whoever_holds_the_task() {
        // mu-103 R1: the shaper asks with a default and moves on; the task goes to Ready and a
        // builder in another session claims it and builds on the default.
        let d = queue();
        put(
            d.path(),
            "todo/0001-t.md",
            &task_text("status: todo\nclaimed-by:\nwaiting_on:", TICKED),
        );
        let defaulted =
            crate::choices::clean(&["One".into(), "Two".into()], Some("b"), Some("b")).unwrap();
        let ask = |q: &str, c: &crate::choices::Choices| {
            crate::queue::ask_with(d.path(), "0001", q, c, "shapour", "aaaa1111", T).unwrap()
        };
        ask("Which port?", &defaulted);
        ask("Lease?", &defaulted);
        ask("In words?", &Default::default());
        let mail = d.path().join("mail");
        let read = |p: PathBuf| fs::read_to_string(p).unwrap_or_default();
        // Nobody holds it yet: the answer goes to the shaper.
        let to = crate::queue::answer_pick(d.path(), "0001", 2, "a", "", "owner", T).unwrap();
        assert_eq!(to.0.as_deref(), Some("aaaa1111"));
        assert!(read(mail.join("aaaa1111/0001-Q2.md")).contains("Your question: Lease?"));
        claim_task(d.path(), "0001", OLD, "2026-10-14 10:00").unwrap();
        // Held, it goes to the holder, saying whose question it was; the shaper gets nothing.
        let to = crate::queue::answer_pick(d.path(), "0001", 1, "a", "", "owner", T).unwrap();
        assert_eq!(to.0.as_deref(), Some("1c6af0d9"));
        let got = read(mail.join("1c6af0d9/0001-Q1.md"));
        assert!(got.contains("Answer: (a) One"), "{got}");
        assert!(got.contains("The question (shapour's, on the task you hold): Which port?"));
        assert!(!mail.join("aaaa1111/0001-Q1.md").exists());
        // A question without a default still goes to its asker, who waited for it.
        let to = crate::queue::answer(d.path(), "0001", 3, "Yes.", "owner", T).unwrap();
        assert_eq!(to.as_deref(), Some("aaaa1111"));
        assert!(!mail.join("1c6af0d9/0001-Q3.md").exists());
    }

    #[test]
    fn a_defaulted_answer_follows_the_claim_past_a_takeover() {
        // The claim moved after the ask: the answer reaches the holder at the time it's given.
        let d = queue();
        claimed_by_old(d.path());
        let c = crate::choices::clean(&["One".into(), "Two".into()], None, Some("a")).unwrap();
        crate::queue::ask_with(d.path(), "0001", "Port?", &c, "shapour", "aaaa1111", T).unwrap();
        checkin_task_with(d.path(), "0001", "bye", "", true, OLD, "2026-10-14 10:05").unwrap();
        takeover_task(d.path(), "0001", NEW, "2026-10-14 10:06", 0).unwrap();
        let to = crate::queue::answer(d.path(), "0001", 1, "Two.", "owner", T).unwrap();
        assert_eq!(to.as_deref(), Some("d6419719"));
        assert!(d.path().join("mail/d6419719/0001-Q1.md").exists());
        assert!(!d.path().join("mail/1c6af0d9/0001-Q1.md").exists());
        assert!(!d.path().join("mail/aaaa1111/0001-Q1.md").exists());
    }

    #[test]
    fn a_defaulted_answer_in_testing_reaches_the_tester_not_the_builder() {
        // In Testing the claim still names the builder; the task is `held-by` the tester.
        let d = queue();
        let fm = format!(
            "status: doing\nstage: testing\nheld-by: {NEW}\nclaimed-by: {OLD}\nwaiting_on:"
        );
        put(d.path(), "doing/0001-t.md", &task_text(&fm, TICKED));
        let c = crate::choices::clean(&["One".into(), "Two".into()], None, Some("a")).unwrap();
        for q in ["Port?", "Lease?"] {
            crate::queue::ask_with(d.path(), "0001", q, &c, "shapour", "aaaa1111", T).unwrap();
        }
        let to = crate::queue::answer(d.path(), "0001", 1, "Two.", "owner", T).unwrap();
        assert_eq!(to.as_deref(), Some("d6419719"));
        assert!(!d.path().join("mail/1c6af0d9/0001-Q1.md").exists());
        // In Your check the owner holds it: no session to mail, so it goes to the asker.
        let body = file_0001(d.path()).replace("stage: testing", "stage: your-check");
        let body = body.replace(&format!("held-by: {NEW}"), "held-by:");
        put(d.path(), "doing/0001-t.md", &body);
        let to = crate::queue::answer(d.path(), "0001", 2, "Two.", "owner", T).unwrap();
        assert_eq!(to.as_deref(), Some("aaaa1111"));
    }

    #[test]
    fn takeover_mail_from_a_teammate_leaves_its_folder_and_the_leads_mail() {
        let d = queue();
        let tm = "asghar (main · session 1c6af0d9 · agent a1b2c3d4 · cc-mg)";
        put(
            d.path(),
            "todo/0001-t.md",
            &task_text("status: todo\nclaimed-by:\nwaiting_on:", TICKED),
        );
        claim_task(d.path(), "0001", tm, "2026-10-14 10:00").unwrap();
        crate::queue::ask(
            d.path(),
            "0001",
            "Which port?",
            "asghar",
            &format!("{OLD_ID}/A1B2C3D4E5"),
            "2026-10-14 10:02",
        )
        .unwrap();
        let text = file_0001(d.path());
        assert!(
            text.contains(
                "**Q1 (asghar, session 1c6af0d9 · agent a1b2c3d4, 2026-10-14 10:02):** Which port?"
            ),
            "{text}"
        );
        let mail = d.path().join("mail");
        fs::create_dir_all(mail.join("1c6af0d9/a1b2c3d4")).unwrap();
        fs::write(
            mail.join("1c6af0d9/a1b2c3d4/0001-note-0001.md"),
            "for the teammate",
        )
        .unwrap();
        fs::write(mail.join("1c6af0d9/0001-note-0001.md"), "for the lead").unwrap();
        // The lead's inbox doesn't take its teammate's mail.
        let lead = crate::queue::take_mail(d.path(), OLD_ID, true).unwrap();
        assert_eq!(lead, vec!["for the lead".to_string()]);
        // Its question (at 10:02) kept the claim alive: quiet since then.
        let t = takeover_task(d.path(), "0001", NEW, "2026-10-14 10:40", 0).unwrap();
        assert_eq!(t.why, "quiet 38 min");
        assert_eq!(
            fs::read_to_string(mail.join("d6419719/0001-note-0001.md")).unwrap(),
            "for the teammate"
        );
        assert_eq!(
            fs::read_to_string(mail.join("1c6af0d9/0001-note-0001.md")).unwrap(),
            "for the lead"
        );
        let asker =
            crate::queue::answer(d.path(), "0001", 1, "5173", "owner", "2026-10-14 10:41").unwrap();
        assert_eq!(asker.as_deref(), Some("d6419719"));
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

    fn queue() -> crate::testkit::TestDir {
        let d = crate::testkit::tempdir().unwrap();
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
        let new = "# Edit me\n\nThe new description, with a list:\n- one\n\t- indented\n\n## Acceptance criteria\n- [x] AC1: one\n- [ ] AC2: two";
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
        let d = crate::testkit::tempdir().unwrap();
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

    /// The task's `## Questions` section, heading to its last non-blank line.
    fn questions_of(text: &str) -> &str {
        let at = text.find("\n## Questions\n").unwrap() + 1;
        let end = text[at..].find("\n\n## ").map_or(text.len(), |e| at + e);
        &text[at..end]
    }
    const PERSIAN: &str = "کدام رنگ برای دکمه؟";

    #[test]
    fn edit_keeps_questions_before_a_checked_vision() {
        let d = queue();
        let p = put(d.path(), "todo", READY, WRITE_UP, "");
        crate::queue::ask(d.path(), "0001", PERSIAN, "wt-a", "aaaa1111", T).unwrap();
        let one = r#"{"conflicts": [{"title": "A", "this": {"quote": "a", "at": "0001"}, "other": {"quote": "b", "at": "docs/VISION.md:3"}, "fix": "f"}]}"#;
        crate::vision::record(d.path(), "0001", one, "wt-shape", T).unwrap();
        let before = text(&p);
        let q = questions_of(&before).to_string();
        assert!(q.contains(PERSIAN), "{before}");
        assert!(
            before.find("\n## Questions\n") < before.rfind("\n## Vision\n"),
            "asked before its vision check: {before}"
        );
        let opened = open_body(d.path(), "0001", "owner").unwrap();
        assert_eq!(
            opened, WRITE_UP,
            "the tool's Questions aren't in the editor"
        );
        let new = "# Edit me\n\nThe new description.\n\n## Acceptance criteria\n- [ ] AC1: one";
        edit_body(d.path(), "0001", new, &opened, "owner", T).unwrap();
        let after = text(&p);
        assert!(
            after.contains(&format!("{new}\n\n{q}\n\n## Vision\n")),
            "the section kept byte for byte, still before the Vision: {after}"
        );
        assert_eq!(open_body(d.path(), "0001", "owner").unwrap(), new);
        // `muster ask` after the edit still finds the one section.
        crate::queue::ask(d.path(), "0001", "And the size?", "wt-a", "aaaa1111", T).unwrap();
        let t = crate::queue::read_task(d.path(), "0001").unwrap();
        assert_eq!(t.questions.len(), 2);
        assert_eq!(text(&p).matches("\n## Questions\n").count(), 1);
    }

    #[test]
    fn edit_keeps_questions_after_a_checked_vision() {
        let d = queue();
        let p = put(d.path(), "todo", READY, WRITE_UP, "");
        let one = r#"{"conflicts": [{"title": "A", "this": {"quote": "a", "at": "0001"}, "other": {"quote": "b", "at": "docs/VISION.md:3"}, "fix": "f"}]}"#;
        crate::vision::record(d.path(), "0001", one, "wt-shape", T).unwrap();
        crate::queue::ask(d.path(), "0001", PERSIAN, "wt-a", "aaaa1111", T).unwrap();
        let before = text(&p);
        let q = questions_of(&before).to_string();
        assert!(before.rfind("\n## Vision\n") < before.find("\n## Questions\n"));
        let opened = open_body(d.path(), "0001", "owner").unwrap();
        assert_eq!(opened, WRITE_UP);
        let new = "# Edit me\n\nThe new description.";
        edit_body(d.path(), "0001", new, &opened, "owner", T).unwrap();
        let after = text(&p);
        assert_eq!(questions_of(&after), q);
        assert!(after.rfind("\n## Vision\n") < after.find("\n## Questions\n"));
        assert_eq!(open_body(d.path(), "0001", "owner").unwrap(), new);
    }

    #[test]
    fn edit_keeps_questions_mid_write_up_in_their_place() {
        let d = queue();
        let q = format!("## Questions\n- [x] **Q1 (wt-a, session aaaa1111, 2026-10-12 10:00):** {PERSIAN}\n  **Answer (owner, 2026-10-12 11:00):** آبی\n- [ ] **Q2 (wt-a, session aaaa1111, 2026-10-12 10:05):** Which size?");
        // A `## Notes` before and after the section: it goes back before the second one.
        let write_up =
            format!("# Edit me\n\n## Notes\nFirst.\n\n{q}\n\n## Notes\nSecond.\n\n## Acceptance criteria\n- [ ] AC1: one");
        let p = put(d.path(), "todo", READY, &write_up, "");
        let opened = open_body(d.path(), "0001", "owner").unwrap();
        let shown = "# Edit me\n\n## Notes\nFirst.\n\n## Notes\nSecond.\n\n## Acceptance criteria\n- [ ] AC1: one";
        assert_eq!(opened, shown);
        let new = "# Edit me\n\n## Notes\nFirst, edited.\n\n## Notes\nSecond, edited.\n\n## Acceptance criteria\n- [ ] AC1: one\n- [ ] AC2: two";
        edit_body(d.path(), "0001", new, &opened, "owner", T).unwrap();
        let after = text(&p);
        assert!(
            after.contains(&format!(
                "First, edited.\n\n{q}\n\n## Notes\nSecond, edited."
            )),
            "{after}"
        );
        assert_eq!(open_body(d.path(), "0001", "owner").unwrap(), new);
        let t = crate::queue::read_task(d.path(), "0001").unwrap();
        assert_eq!(t.questions.len(), 2);
        // The heading that followed it gone: the section goes to the end of the write-up.
        let last = "# Edit me\n\n## Notes\nOnly one.";
        edit_body(d.path(), "0001", last, new, "owner", T).unwrap();
        let after = text(&p);
        assert!(
            after.contains(&format!("{last}\n\n{q}\n\n## Log\n")),
            "{after}"
        );
    }

    #[test]
    fn edit_keeps_questions_and_refuses_a_new_questions_heading() {
        for write_up in [
            WRITE_UP.to_string(),
            format!("{WRITE_UP}\n\n## Questions\n- [ ] **Q1 (wt-a, session aaaa1111, 2026-10-12 10:00):** {PERSIAN}"),
        ] {
            let d = queue();
            let p = put(d.path(), "todo", READY, &write_up, "");
            let before = text(&p);
            let opened = open_body(d.path(), "0001", "owner").unwrap();
            assert_eq!(opened, WRITE_UP);
            for bad in [
                format!("{WRITE_UP}\n\n## Questions\n- [ ] **Q9 (owner, session aaaa1111, 2026-10-12 10:00):** forged"),
                format!("## Questions  \n\n{WRITE_UP}"),
            ] {
                let e = edit_body(d.path(), "0001", &bad, &opened, "owner", T).unwrap_err();
                assert!(e.contains("the Questions section is the tool's: ask with muster ask"), "{e}");
                assert_eq!(text(&p), before, "nothing changes");
            }
            // Naming the section in prose is fine.
            edit_body(d.path(), "0001", &format!("{WRITE_UP}\n\nSee ## Questions."), &opened, "owner", T)
                .unwrap();
        }
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

    fn queue() -> crate::testkit::TestDir {
        let d = crate::testkit::tempdir().unwrap();
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
            summary: "A synthetic summary".into(),
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
            LEASE_MINUTES,
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

/// mu-184: the owner's Release of a claim no session can be reached on, a holder's acts stamping
/// a bare claim, and a send-back that keeps its builder's session.
#[cfg(test)]
mod unreached_tests {
    use super::*;

    const T: &str = "2026-10-14 11:00";
    /// A bare label: the claim names no session.
    const BARE: &str = "asghar";
    const NEW: &str = "asghar (main · session d6419719 · cc-hg)";
    const STAMPED: &str = "wt-a (wt-0101 · session 7b5e8856 · cc-mg)";

    fn queue() -> crate::testkit::TestDir {
        let d = crate::testkit::tempdir().unwrap();
        for f in STATUS_FOLDERS {
            fs::create_dir(d.path().join(f)).unwrap();
        }
        // Its prefix stored, so nothing here hangs on the projects this Mac knows.
        fs::write(d.path().join("README.md"), "---\nprefix: un\n---\nq").unwrap();
        d
    }

    /// Task `id` in `folder` with the front-matter lines `fm` (after id, type and priority), its
    /// criteria ticked, and a log up to its claim.
    fn put(d: &Path, folder: &str, id: &str, fm: &str) {
        let text = format!(
            "---\nid: {id}\ntype: task\npriority: normal\n{fm}\n---\n\n# T{id}\n\n## Acceptance criteria\n- [x] AC1: one\n\n## Log\n- 2026-10-14 09:00 capture: created\n- 2026-10-14 09:30 {BARE}: stage ready → building (claimed)\n"
        );
        fs::write(d.join(folder).join(format!("{id}-t.md")), text).unwrap();
    }
    /// 0001 in Building, claimed by `holder`, started on its own branch, heard from at 10:55.
    fn building(d: &Path, holder: &str) {
        put(
            d,
            "doing",
            "0001",
            &format!("stage: building\nstatus: doing\nclaimed-by: {holder}\nheartbeat: 2026-10-14 10:55\nwaiting_on:\nworktree: /nowhere/wt\nbranch: task/un-1"),
        );
    }
    fn file(d: &Path, id: &str) -> String {
        fs::read_to_string(find_task(d, id).unwrap()).unwrap()
    }
    fn task(d: &Path, id: &str) -> queue::Task {
        queue::read_task(d, id).unwrap()
    }
    fn takeovers(text: &str) -> Vec<String> {
        text.lines()
            .filter(|l| queue::takeover_line(l).is_some())
            .map(str::to_string)
            .collect()
    }
    fn gate() -> PassOpts {
        PassOpts {
            gate: "green".into(),
            summary: "A synthetic summary".into(),
            ..Default::default()
        }
    }

    #[test]
    fn release_unreached_in_building_goes_to_ready_and_keeps_the_worktree() {
        let d = queue();
        building(d.path(), BARE);
        // Its lease has not run out: this Release doesn't wait for it.
        let path = release_unreached(d.path(), "0001", "owner", T, Some("building")).unwrap();
        assert_eq!(path, "todo/0001-t.md");
        let t = task(d.path(), "0001");
        assert_eq!(
            (
                t.stage.as_str(),
                t.claimed_by.as_str(),
                t.worktree.as_str(),
                t.branch.as_str()
            ),
            ("ready", "", "/nowhere/wt", "task/un-1")
        );
        let text = file(d.path(), "0001");
        assert!(
            text.ends_with(&format!(
                "- {T} owner: stage building → ready (released: no session to reach)\n"
            )),
            "{text}"
        );
        // It reads as tool-written stage history.
        let last = text.lines().last().unwrap();
        assert_eq!(
            queue::stage_line(last).map(|(s, detail)| (s.to, detail.to_string())),
            Some(("ready".to_string(), UNREACHED.to_string()))
        );
    }

    #[test]
    fn release_unreached_in_testing_lets_go_of_the_hold_and_stays_in_testing() {
        let d = queue();
        put(
            d.path(),
            "doing",
            "0001",
            "stage: testing\nstatus: doing\nclaimed-by:\nheld-by: qa-0001\nwaiting_on:",
        );
        release_unreached(d.path(), "0001", "owner", T, Some("testing")).unwrap();
        let t = task(d.path(), "0001");
        assert_eq!((t.stage.as_str(), t.held_by.as_str()), ("testing", ""));
        let text = file(d.path(), "0001");
        assert!(text.contains("\nheld-by:\n"), "{text}");
        assert!(
            text.ends_with(&format!(
                "- {T} owner: let go of qa-0001's hold; still in Testing (released: no session to reach)\n"
            )),
            "{text}"
        );
        // No stage line: it never left Testing.
        assert!(queue::stage_line(text.lines().last().unwrap()).is_none());
        // Nobody holds it now, so a second Release has nothing to do.
        assert_eq!(
            release_unreached(d.path(), "0001", "owner", T, None).unwrap_err(),
            "nobody holds 0001; there's nothing to release"
        );
    }

    #[test]
    fn release_unreached_is_refused_and_writes_nothing() {
        let d = queue();
        building(d.path(), BARE);
        put(
            d.path(),
            "doing",
            "0002",
            &format!("stage: building\nstatus: doing\nclaimed-by: {STAMPED}\nheartbeat: 2026-10-14 10:55\nwaiting_on:"),
        );
        put(
            d.path(),
            "doing",
            "0003",
            "stage: building\nstatus: doing\nclaimed-by: wt-c\nheartbeat: 2026-10-14 10:55\npaused: 2026-10-14 10:56\nwaiting_on:",
        );
        put(
            d.path(),
            "doing",
            "0004",
            "stage: building\nstatus: doing\nclaimed-by:\nwaiting_on:",
        );
        put(
            d.path(),
            "todo",
            "0005",
            "stage: ready\nstatus: todo\nclaimed-by:\nwaiting_on:",
        );
        put(
            d.path(),
            "doing",
            "0006",
            &format!("stage: testing\nstatus: doing\nclaimed-by:\nheld-by: {STAMPED}\nwaiting_on:"),
        );
        put(
            d.path(),
            "doing",
            "0007",
            "stage: building\nstatus: doing\nclaimed-by: owner\nwaiting_on:",
        );
        let all = || -> Vec<String> {
            (1..=7)
                .map(|i| file(d.path(), &format!("000{i}")))
                .collect()
        };
        let before = all();
        let refused = |id: &str, who: &str, from: Option<&str>| {
            release_unreached(d.path(), id, who, T, from).unwrap_err()
        };
        // Only the owner, whatever the label says.
        assert!(refused("0001", BARE, None).starts_with("only the owner releases"));
        assert!(
            refused("0001", "qa (main · session 0f0f0f0f · cc-hg)", None)
                .starts_with("only the owner releases")
        );
        // A claim with a session is Ping's, in Building or Testing.
        for id in ["0002", "0006"] {
            assert_eq!(
                refused(id, "owner", None),
                format!("{STAMPED} has a session now: Ping it, or wait for the lease")
            );
        }
        assert!(refused("0003", "owner", None).contains("paused"));
        assert_eq!(
            refused("0004", "owner", None),
            "nobody holds 0004; there's nothing to release"
        );
        assert_eq!(
            refused("0005", "owner", None),
            "0005 is in Ready; only a claim in Building or a hold in Testing is released this way"
        );
        assert_eq!(refused("0007", "owner", None), "you hold 0007 yourself");
        // The window showed another stage: it moved on since.
        assert!(refused("0001", "owner", Some("testing")).contains("it moved on since you looked"));
        assert_eq!(before, all());
    }

    #[test]
    fn release_unreached_refuses_a_claim_its_session_stamped_since_the_click() {
        let d = queue();
        building(d.path(), BARE);
        // The successor checks in between the click and the write: the claim names it now.
        checkin_task(d.path(), "0001", "on it", "", NEW, "2026-10-14 10:58").unwrap();
        assert_eq!(
            release_unreached(d.path(), "0001", "owner", T, Some("building")).unwrap_err(),
            format!("{NEW} has a session now: Ping it, or wait for the lease")
        );
        assert_eq!(task(d.path(), "0001").stage, "building");
    }

    #[test]
    fn a_successors_checkin_on_a_bare_claim_is_stamped_and_logged_once() {
        let d = queue();
        building(d.path(), BARE);
        checkin_task(d.path(), "0001", "parser done", "40", NEW, T).unwrap();
        let t = task(d.path(), "0001");
        assert_eq!((t.claimed_by.as_str(), t.heartbeat.as_str()), (NEW, T));
        let text = file(d.path(), "0001");
        assert!(
            text.ends_with(&format!(
                "- {T} {NEW}: took over the claim from {BARE} (stamped)\n- {T} {NEW}: check-in 40% · parser done\n"
            )),
            "{text}"
        );
        // The takeover reads back as one, from the bare label to the session.
        assert_eq!(
            queue::takeover_line(&takeovers(&text)[0]),
            Some((T.into(), NEW.into(), BARE.into()))
        );
        assert_eq!(t.builders, vec![BARE.to_string(), NEW.to_string()]);
        // Once: the claim names the session now, so the next acts add nothing.
        checkin_task(d.path(), "0001", "CLI next", "60", NEW, "2026-10-14 11:10").unwrap();
        heartbeat_task(d.path(), "0001", NEW, "2026-10-14 11:20").unwrap();
        add_note(d.path(), "0001", "a note", NEW, "2026-10-14 11:25").unwrap();
        assert_eq!(takeovers(&file(d.path(), "0001")).len(), 1);
        // And Ping reaches it.
        assert_eq!(
            crate::ping::reach(&task(d.path(), "0001")).unwrap(),
            "d6419719"
        );
    }

    #[test]
    fn a_successors_note_heartbeat_or_block_on_a_bare_claim_is_stamped() {
        for act in ["note", "heartbeat", "block"] {
            let d = queue();
            building(d.path(), BARE);
            match act {
                "note" => add_note(d.path(), "0001", "looking", NEW, T),
                "heartbeat" => heartbeat_task(d.path(), "0001", NEW, T),
                _ => block_task(d.path(), "0001", "outside: Apple review", NEW, T),
            }
            .unwrap();
            assert_eq!(task(d.path(), "0001").claimed_by, NEW, "{act}");
            assert_eq!(
                takeovers(&file(d.path(), "0001")),
                vec![format!(
                    "- {T} {NEW}: took over the claim from {BARE} (stamped)"
                )],
                "{act}"
            );
        }
    }

    #[test]
    fn a_successors_pass_on_a_bare_claim_is_stamped_and_it_cant_test_it() {
        let d = queue();
        put(
            d.path(),
            "doing",
            "0001",
            &format!("stage: building\nstatus: doing\nclaimed-by: {BARE}\nheartbeat: 2026-10-14 10:55\nwaiting_on:"),
        );
        pass_task(d.path(), "0001", NEW, T, &gate()).unwrap();
        let t = task(d.path(), "0001");
        assert_eq!(
            (t.stage.as_str(), t.claimed_by.as_str(), t.held_by.as_str()),
            ("testing", "", NEW)
        );
        let text = file(d.path(), "0001");
        let tail: Vec<&str> = text.lines().rev().take(3).collect();
        assert!(
            tail[0].ends_with(&format!("{NEW}: stage building → testing (gate: green)")),
            "{text}"
        );
        assert!(
            tail.contains(
                &format!("- {T} {NEW}: took over the claim from {BARE} (stamped)").as_str()
            ),
            "{text}"
        );
        assert_eq!(takeovers(&text).len(), 1);
        // The successor built it: it doesn't test it.
        let e = pass_task(d.path(), "0001", NEW, T, &PassOpts::default()).unwrap_err();
        assert!(e.starts_with("0001 was built by asghar"), "{e}");
    }

    #[test]
    fn claim_takeover_of_ones_own_bare_claim_stamps_it() {
        let d = queue();
        building(d.path(), BARE);
        let t = takeover_task(d.path(), "0001", NEW, T, 0).unwrap();
        assert_eq!(
            t,
            TakenOver {
                path: "doing/0001-t.md".into(),
                from: BARE.into(),
                why: "stamped".into(),
                moved: None,
            }
        );
        let held = task(d.path(), "0001");
        assert_eq!(
            (held.claimed_by.as_str(), held.heartbeat.as_str()),
            (NEW, T)
        );
        // Stamped now, a second takeover is mu-156's rerun: the bare label had no mailbox, so
        // there's nothing to move, and nothing more is logged.
        let again = takeover_task(d.path(), "0001", NEW, T, 0).unwrap();
        assert_eq!(
            again.rerun_line("mu-1").as_deref(),
            Some("mu-1: you hold it already; its old holder had no mailbox, so there's nothing to move")
        );
        assert_eq!(takeovers(&file(d.path(), "0001")).len(), 1);
    }

    #[test]
    fn a_rerun_after_an_account_restamp_moves_nothing_within_one_mailbox() {
        // The same session under another account has the same mailbox: the rerun after its
        // re-stamp moves nothing, and doesn't fail moving mail onto itself.
        let d = queue();
        let mg = "asghar (main · session d6419719 · cc-mg)";
        building(d.path(), mg);
        assert_eq!(
            takeover_task(d.path(), "0001", NEW, T, 0).unwrap().why,
            "stamped"
        );
        fs::create_dir_all(d.path().join("mail/d6419719")).unwrap();
        fs::write(d.path().join("mail/d6419719/0001-1.md"), "a note").unwrap();
        let again = takeover_task(d.path(), "0001", NEW, T, 0).unwrap();
        assert_eq!(again.moved, Some(0));
        assert!(d.path().join("mail/d6419719/0001-1.md").exists());
    }

    #[test]
    fn a_session_that_moved_account_is_stamped_with_its_new_account() {
        let d = queue();
        let mg = "asghar (main · session d6419719 · cc-mg)";
        building(d.path(), mg);
        checkin_task(d.path(), "0001", "moved", "", NEW, T).unwrap();
        assert_eq!(task(d.path(), "0001").claimed_by, NEW);
        assert_eq!(
            takeovers(&file(d.path(), "0001")),
            vec![format!(
                "- {T} {NEW}: took over the claim from {mg} (stamped)"
            )]
        );
        // The same account again: nothing more.
        checkin_task(d.path(), "0001", "again", "", NEW, T).unwrap();
        assert_eq!(takeovers(&file(d.path(), "0001")).len(), 1);
    }

    #[test]
    fn the_owner_and_an_unstamped_holder_never_restamp() {
        let d = queue();
        building(d.path(), BARE);
        checkin_task(d.path(), "0001", "from a terminal", "", BARE, T).unwrap();
        add_note(
            d.path(),
            "0001",
            "owner's note",
            "owner (main · session 7b5e8856 · cc-hg)",
            T,
        )
        .unwrap();
        assert_eq!(task(d.path(), "0001").claimed_by, BARE);
        assert!(takeovers(&file(d.path(), "0001")).is_empty());
    }

    #[test]
    fn a_stamped_takeover_line_is_the_tools_alone() {
        let line = format!("- {T} {NEW}: took over the claim from {BARE} (stamped)");
        assert!(queue::takeover_line(&line).is_some());
        let e = free_text_line(
            format!("{T} gholi{}{BARE} (stamped)", queue::TOOK_OVER),
            "a note",
        )
        .unwrap_err();
        assert!(e.contains("would read as a takeover"), "{e}");
        // A note can't plant one either.
        let d = queue();
        building(d.path(), BARE);
        assert!(add_note(
            d.path(),
            "0001",
            &format!(
                "{}{BARE} (stamped)",
                queue::TOOK_OVER.trim_start_matches(": ")
            ),
            "owner",
            T
        )
        .is_err());
    }

    #[test]
    fn a_send_back_keeps_the_builders_session() {
        let d = queue();
        for id in ["0001", "0002"] {
            put(
                d.path(),
                "todo",
                id,
                "stage: ready\nstatus: todo\nclaimed-by:\nwaiting_on:",
            );
        }
        claim_task(d.path(), "0001", STAMPED, T).unwrap();
        pass_task(d.path(), "0001", STAMPED, T, &gate()).unwrap();
        // Holding nothing else, it gets it back by its exact session, not its label.
        return_task(d.path(), "0001", "AC1 fails", None, "qa", T).unwrap();
        let t = task(d.path(), "0001");
        assert_eq!(
            (t.stage.as_str(), t.claimed_by.as_str()),
            ("building", STAMPED)
        );
        assert_eq!(crate::ping::reach(&t).unwrap(), "7b5e8856");
        // Holding another task meanwhile, it still gets it back, first in line.
        pass_task(d.path(), "0001", STAMPED, T, &gate()).unwrap();
        claim_task(d.path(), "0002", STAMPED, T).unwrap();
        return_task(d.path(), "0001", "AC1 still fails", None, "qa", T).unwrap();
        let t = task(d.path(), "0001");
        assert_eq!((t.claimed_by.as_str(), t.sent_back), (STAMPED, true));
        // The builder's session may have ended: until it acts on the send-back, a successor
        // under its label is pointed there by next and takes it over at once.
        block_task(d.path(), "0002", "outside: later", STAMPED, T).unwrap();
        let next = "wt-a (wt-0101 · session d6419719 · cc-hg)";
        let e = next_task(d.path(), next, T).unwrap_err();
        assert_eq!(
            e,
            format!("fix un-1 first (sent back by the review): it went back to {STAMPED}, which hasn't picked it up; take it over: muster claim 0001 --takeover")
        );
        let took = takeover_task(d.path(), "0001", next, T, 0).unwrap();
        assert_eq!(took.why, "sent back, not picked up");
        assert!(queue::takeover_line(&takeovers(&file(d.path(), "0001")).pop().unwrap()).is_some());
    }

    #[test]
    fn a_send_back_its_builder_picked_up_waits_for_a_quiet_takeover() {
        let d = queue();
        put(
            d.path(),
            "todo",
            "0001",
            "stage: ready\nstatus: todo\nclaimed-by:\nwaiting_on:",
        );
        claim_task(d.path(), "0001", STAMPED, T).unwrap();
        pass_task(d.path(), "0001", STAMPED, T, &gate()).unwrap();
        return_task(d.path(), "0001", "AC1 fails", None, "qa", T).unwrap();
        // The builder checks in on it: it's alive, so a successor waits as for any claim.
        checkin_task(
            d.path(),
            "0001",
            "fixing AC1",
            "",
            STAMPED,
            "2026-10-14 11:05",
        )
        .unwrap();
        let next = "wt-a (wt-0101 · session d6419719 · cc-hg)";
        assert!(next_task(d.path(), next, "2026-10-14 11:06")
            .unwrap()
            .is_none());
        let e = takeover_task(d.path(), "0001", next, "2026-10-14 11:06", 0).unwrap_err();
        assert!(e.contains("last heard from 1 min ago"), "{e}");
        let took = takeover_task(d.path(), "0001", next, "2026-10-14 11:35", 0).unwrap();
        assert_eq!(took.why, "quiet 30 min");
    }

    /// A Ready 0003 beside 0001 sent back to `STAMPED` (review R1-R3): what `next` gives a
    /// sibling under its label when the send-back isn't one to take over at once.
    fn sent_back_to_stamped(d: &Path, ids: &[&str]) {
        for id in ids {
            put(
                d,
                "todo",
                id,
                "stage: ready\nstatus: todo\nclaimed-by:\nwaiting_on:",
            );
        }
        claim_task(d, "0001", STAMPED, T).unwrap();
        pass_task(d, "0001", STAMPED, T, &gate()).unwrap();
    }

    const SIBLING: &str = "wt-a (wt-0101 · session d6419719 · cc-hg)";

    #[test]
    fn a_send_back_to_a_builder_busy_on_another_task_waits_for_a_quiet_takeover() {
        // Review R1: the builder is live on 0002 (unblocked) when 0001 comes back to it. That
        // send-back isn't a stranded one: a sibling waits for a handoff or 30 quiet minutes.
        let d = queue();
        sent_back_to_stamped(d.path(), &["0001", "0002", "0003"]);
        claim_task(d.path(), "0002", STAMPED, T).unwrap();
        return_task(d.path(), "0001", "AC1 fails", None, "qa", T).unwrap();
        assert_eq!(task(d.path(), "0001").claimed_by, STAMPED);
        let got = next_task(d.path(), SIBLING, T).unwrap().unwrap();
        assert_eq!(got.id, "0003");
        let e = takeover_task(
            d.path(),
            "0001",
            "wt-a (wt-0101 · session 0f0f0f0f · cc-hg)",
            T,
            0,
        )
        .unwrap_err();
        assert!(e.contains("last heard from"), "{e}");
        assert!(takeovers(&file(d.path(), "0001")).is_empty());
    }

    #[test]
    fn a_send_back_picked_up_in_its_own_minute_waits_for_a_quiet_takeover() {
        // Review R2: the builder checks in within the send-back's minute, so heartbeat still
        // equals the stage line's stamp; the log says it picked it up.
        let d = queue();
        sent_back_to_stamped(d.path(), &["0001", "0003"]);
        return_task(d.path(), "0001", "AC1 fails", None, "qa", T).unwrap();
        checkin_task(d.path(), "0001", "on it", "", STAMPED, T).unwrap();
        assert_eq!(next_task(d.path(), SIBLING, T).unwrap().unwrap().id, "0003");
        let other = "wt-a (wt-0101 · session 0f0f0f0f · cc-hg)";
        let e = takeover_task(d.path(), "0001", other, T, 0).unwrap_err();
        assert!(e.contains("last heard from"), "{e}");
    }

    #[test]
    fn a_send_back_taken_over_in_its_own_minute_is_not_taken_again_at_once() {
        // Review R2: a takeover in the send-back's minute is the new holder picking it up.
        let d = queue();
        sent_back_to_stamped(d.path(), &["0001"]);
        return_task(d.path(), "0001", "AC1 fails", None, "qa", T).unwrap();
        let took = takeover_task(d.path(), "0001", SIBLING, T, 0).unwrap();
        assert_eq!(took.why, queue::SENT_BACK);
        let third = "wt-a (wt-0101 · session 0f0f0f0f · cc-hg)";
        let e = takeover_task(d.path(), "0001", third, T, 0).unwrap_err();
        assert!(e.contains("last heard from"), "{e}");
    }

    #[test]
    fn a_paused_send_back_is_not_pointed_at() {
        // Review R3: its takeover is refused while paused, so next gives the Ready task instead.
        let d = queue();
        sent_back_to_stamped(d.path(), &["0001", "0003"]);
        return_task(d.path(), "0001", "AC1 fails", None, "qa", T).unwrap();
        crate::pause::pause(d.path(), "0001", "hold on", "owner", T).unwrap();
        assert_eq!(next_task(d.path(), SIBLING, T).unwrap().unwrap().id, "0003");
    }

    #[test]
    fn a_reclaim_by_a_session_under_the_label_logs_no_stamped_takeover() {
        let d = queue();
        put(
            d.path(),
            "doing",
            "0001",
            &format!("stage: building\nstatus: doing\nclaimed-by: {BARE}\nheartbeat: 2026-10-13 09:00\nwaiting_on:"),
        );
        let r = reclaim(d.path(), 60, NEW, T, 0).unwrap();
        assert_eq!(r.released.len(), 1);
        let text = file(d.path(), "0001");
        assert!(takeovers(&text).is_empty(), "{text}");
        assert_eq!(task(d.path(), "0001").stage, "ready");
    }

    #[test]
    fn a_send_back_to_a_builder_with_no_session_stays_its_label() {
        let d = queue();
        put(
            d.path(),
            "todo",
            "0001",
            "stage: ready\nstatus: todo\nclaimed-by:\nwaiting_on:",
        );
        claim_task(d.path(), "0001", "wt-b", T).unwrap();
        pass_task(d.path(), "0001", "wt-b", T, &gate()).unwrap();
        return_task(d.path(), "0001", "AC1 fails", None, "qa", T).unwrap();
        assert_eq!(task(d.path(), "0001").claimed_by, "wt-b");
    }
}
