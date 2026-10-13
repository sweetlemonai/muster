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
/// Goal → Feature → Task (decision 0003), highest first. `level:` in the front matter; a file
/// without one is a task.
pub const LEVELS: [&str; 3] = ["goal", "feature", "task"];
/// Who does a task (0022): `for:` in the front matter. An agent (`ai`, what a file without one
/// is), or the owner (`human`): the owner's own to-do, To do (Ready) → Doing (Building) → Done,
/// which agents never take.
pub const FOR_WHO: [&str; 2] = ["ai", "human"];
pub(crate) const MAX_TASK_BYTES: u64 = 1024 * 1024;
/// The queue's master list, INDEX.md (task 0018).
pub use crate::index::index_md;
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
    /// Its options, when it was asked with them (0049, `choices::read`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choices: Option<crate::choices::Choices>,
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

/// A check-in from the log (0030): `- <at> <who>: check-in 60% · <text>`, or `check-in · <text>`
/// without a percentage. The percentage is the session's own estimate.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Checkin {
    pub at: String,
    pub who: String,
    pub pct: Option<u8>,
    pub text: String,
}

/// Is this the shape of a tool stamp, `YYYY-MM-DD HH:MM`?
fn stamp_shaped(at: &str) -> bool {
    at.len() == 16
        && at.bytes().enumerate().all(|(i, c)| match i {
            4 | 7 => c == b'-',
            10 => c == b' ',
            13 => c == b':',
            _ => c.is_ascii_digit(),
        })
}

/// A tool-written check-in line from the log. A stage line never is one, whatever its detail says.
pub fn checkin_line(line: &str) -> Option<Checkin> {
    if stage_line(line).is_some() {
        return None;
    }
    let rest = line.strip_prefix("- ")?;
    let at = rest.get(..16).filter(|at| stamp_shaped(at))?;
    let (who, tail) = rest
        .get(16..)?
        .strip_prefix(' ')?
        .split_once(": check-in ")?;
    let (pct, text) = match tail.strip_prefix("· ") {
        Some(text) => (None, text),
        None => {
            let (n, text) = tail.split_once("% · ")?;
            let ok = (1..=3).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_digit());
            let n: u8 = n.parse().ok().filter(|n| ok && *n <= 100)?;
            (Some(n), text)
        }
    };
    if who.trim().is_empty() || text.trim().is_empty() {
        return None;
    }
    Some(Checkin {
        at: at.to_string(),
        who: who.to_string(),
        pct,
        text: text.trim().to_string(),
    })
}

/// A tool-written stage line from the log, with its detail (the text in the closing brackets).
/// Anything else, a hand-typed "moved todo → doing" included, is not stage history.
pub fn stage_line(line: &str) -> Option<(StageStep, &str)> {
    let rest = line.strip_prefix("- ")?;
    let at = rest.get(..16)?;
    if !stamp_shaped(at) {
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

/// A stretch a task was blocked, as `YYYY-MM-DD HH:MM` stamps (a date-only line reads as 00:00);
/// `to` is empty while it still is. `kind` is the block's (0024, `block::line_kind`), "" for a
/// hand-moved `→ blocked`.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BlockSpan {
    pub from: String,
    pub to: String,
    pub kind: String,
}

/// The stamp a log line starts with (`- YYYY-MM-DD HH:MM …` or `- YYYY-MM-DD …`), to the minute,
/// and the rest of the line.
pub(crate) fn log_stamp(line: &str) -> Option<(String, &str)> {
    let rest = line.strip_prefix("- ")?;
    let date = rest.get(..10)?;
    let b = date.as_bytes();
    if !(b[4] == b'-'
        && b[7] == b'-'
        && date
            .bytes()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit()))
    {
        return None;
    }
    let after = rest.get(10..)?;
    if let Some(t) = after.get(1..6).filter(|t| {
        let tb = t.as_bytes();
        after.starts_with(' ')
            && tb[2] == b':'
            && t.bytes()
                .enumerate()
                .all(|(i, c)| i == 2 || c.is_ascii_digit())
    }) {
        return Some((format!("{date} {t}"), after.get(6..).unwrap_or("")));
    }
    Some((format!("{date} 00:00"), after))
}

/// When a task last moved into Done, to the minute, from its log: a stage line `… → done` or a
/// hand `moved … → done`; "" when the log only has a date or none (0031's timeline).
pub fn done_at(log_lines: &[&str]) -> String {
    log_lines
        .iter()
        .rev()
        .find_map(|l| {
            let (at, rest) = log_stamp(l)?;
            let timed = l.get(12..13) == Some(" ") && l.get(15..16) == Some(":");
            (timed && rest.contains("→ done")).then_some(at)
        })
        .unwrap_or_default()
}

/// Block and unblock lines of a log, as spans (see `Task::block_history`).
pub fn block_history(log_lines: &[&str]) -> Vec<BlockSpan> {
    let mut out: Vec<BlockSpan> = Vec::new();
    for l in log_lines {
        let Some((at, rest)) = log_stamp(l) else {
            continue;
        };
        let Some((_, what)) = rest.split_once(": ") else {
            continue;
        };
        let open = out.last().is_some_and(|b| b.to.is_empty());
        let kind = what.strip_prefix("blocked on").map(crate::block::line_kind);
        if kind.is_some() || (what.starts_with("moved ") && what.contains("→ blocked")) {
            if !open {
                out.push(BlockSpan {
                    from: at,
                    to: String::new(),
                    kind: kind.unwrap_or("").to_string(),
                });
            }
        } else if what.starts_with("unblocked") && open {
            if let Some(b) = out.last_mut() {
                b.to = at;
            }
        }
    }
    out
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
    /// Its project's prefix (0033), for its key (`mu-12`): set by whoever read the state for
    /// the window or a command (`key::apply`), "" until then.
    pub prefix: String,
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
    /// The tasks it waits on (`depends:`, ids in the same queue, 0019).
    pub depends: Vec<String>,
    /// Anything in `depends:` that isn't a task id, for `check`.
    #[serde(skip)]
    pub bad_depends: Vec<String>,
    /// The file says its stage (`stage:`), so its status must mirror it, for `check`.
    #[serde(skip)]
    pub stage_set: bool,
    pub folder: String,
    pub path: String,
    pub claimed_by: String,
    /// Who holds it in Testing or Your check (`held-by:`, set by `pass`), as Building has
    /// `claimed_by`.
    pub held_by: String,
    /// Who built it (0047): the last `ready → building (claimed)` stage line's who, or "".
    pub built_by: String,
    /// Who passed it Building → Testing with the gate's line most recently (0047): a builder too,
    /// unlike an override into Testing. "" when nobody has.
    pub gated_by: String,
    /// Who passed it into Your check from an earlier stage most recently (its tester, or the
    /// owner's override), or "".
    pub tested_by: String,
    /// When the current stage began: the stamp of the latest stage line into it, or "".
    pub stage_since: String,
    /// Every check-in in the log, oldest first (0030).
    pub checkins: Vec<Checkin>,
    /// When the session working on it was last asked for a check-in (0039): the stamp of the
    /// log's latest `asked for a check-in` line (`ping::asked_line`), or "".
    pub checkin_asked: String,
    pub heartbeat: String,
    pub waiting_on: String,
    /// Why it's blocked (0024): you, permission, question or outside from `blocked-kind:` (in a
    /// file from before kinds, read from `waiting_on`: `kind_guessed`); "tasks" when only its
    /// `depends:` hold it (`block::derive` fills that in); "" when nothing does.
    pub blocked_kind: String,
    /// What the block says (`waiting_on`; an old file's leading owner or capture taken off).
    pub block_detail: String,
    /// The tasks it's blocked by, with their stage and holder (`block::derive`).
    pub blocked_by: Vec<crate::block::BlockedBy>,
    /// Blocked with no `blocked-kind:`, so the kind was read from `waiting_on`; `check` warns
    /// until it's blocked again with one.
    #[serde(skip)]
    pub kind_guessed: bool,
    pub commit: String,
    pub captured: String,
    /// When it was captured to the minute (`YYYY-MM-DD HH:MM`), from the log's first line when
    /// that line is stamped on the capture day; empty when only the date is known (0031).
    pub captured_at: String,
    /// When it was blocked, oldest first: from the log's `blocked on …` (or a hand `moved … →
    /// blocked`) to the next `unblocked`; `to` is empty while it still is (0031's timeline).
    pub block_history: Vec<BlockSpan>,
    /// Paused by the owner (0045): when (`paused:`, a tool stamp), "" when it isn't; and why
    /// (`paused-why:`), "" when no reason was given.
    pub paused: String,
    pub paused_why: String,
    /// When it was paused, oldest first, from the log's `paused` and `resumed` lines
    /// (`pause::history`); `to` is empty while it still is.
    pub pause_history: Vec<crate::pause::PauseSpan>,
    /// When it moved into Done, to the minute, from the log (`done_at`); empty when unknown.
    pub done_at: String,
    pub questions: Vec<Question>,
    pub open_questions: usize,
    /// The criteria only the owner verifies (`— verify: owner`), ticked or not, for the task
    /// pane's checklist in Your check (0028).
    pub owner_criteria: Vec<Criterion>,
    /// The vision check (0029): its open conflicts, by the front matter's `vision: <n> open`; None
    /// before the first check. While it's above 0 the task can't pass from Shaping to Ready.
    pub vision_open: Option<usize>,
    /// The open conflicts' ids (V1, V2…) in its Vision section, once checked.
    pub vision_ids: Vec<String>,
    /// A `vision:` value that isn't `<n> open` (`check` reports it; such a task isn't counted as
    /// unchecked), else "".
    pub bad_vision: String,
    /// An agent's task waits on the owner (`needs_owner`): a needs-you card on the app's board.
    pub waiting_for_owner: bool,
    /// One of FOR_WHO: `for:` from the file, "ai" when it has none (or one that isn't either).
    pub for_who: String,
    /// A `for:` value that isn't one of FOR_WHO (read as ai), for `check`.
    #[serde(skip)]
    pub bad_for: String,
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
    /// One of LEVELS: `level:` from the file, "task" when it has none (or one that isn't a level).
    pub level: String,
    /// A `level:` value that isn't a level (read as a task), for `check`.
    #[serde(skip)]
    pub bad_level: String,
    /// The id of the feature or goal this sits under (`parent:`), or "".
    pub parent: String,
    /// A goal's date (`due:`, `YYYY-MM-DD`), or "".
    pub due: String,
    /// Roll-ups, filled by `rollup` once every task of the queue is read (goals and features
    /// only): the ids whose parent this is, and its tasks' progress.
    pub children: Vec<String>,
    pub progress: Option<Progress>,
    /// The stage to show: a task's own; for a goal or feature, the one its children give it
    /// ("ready", "in-progress" or "done"), unless it's being shaped or the owner set it.
    pub shown_stage: String,
    /// The last stage change was the owner's override (`(set)`), so a goal or feature shows it.
    #[serde(skip)]
    pub stage_set_by_owner: bool,
    /// What search looks in (task 0032): title, tags and body, folded, each word once, capped
    /// (`search::search_text`). Built here, with the file, so the window filters in memory and
    /// the cache keeps it until the file changes. Never serialized: the polled state and
    /// `muster list --json` leave it out, and the window fetches it on its own (`search_index`).
    #[serde(skip)]
    pub search: String,
    /// The sessions its holder (`holder()`) and log name, as short ids (0025), for the card's
    /// live check.
    #[serde(skip)]
    pub session_ids: Vec<String>,
    /// One of those sessions' transcripts changed in the last 5 minutes (0025). Set by the app's
    /// state read (`work::mark_live`); false wherever nothing set it.
    pub live: bool,
    /// What Claude Code says its holder's session is doing (0048, `agents::apply`): working,
    /// needs-input, ended, or unknown wherever nothing read it (the heartbeat lines decide then).
    pub session_state: crate::agents::SessionState,
    /// The folders and globs it works in (`touches:`, 0023), which `ready --parallel` keeps apart.
    pub touches: Vec<String>,
    /// Where `muster start` put it (0023): its git worktree and branch, until the merge or a
    /// release clears them; "" for a task that wasn't started that way.
    pub worktree: String,
    pub branch: String,
    /// The worktree is a runner's (`start --here`, `worktree-runner: yes`): muster never removes it.
    pub worktree_runner: bool,
    /// The latest merge report from its log (`merged task/… into main as …`), or "".
    pub merge_report: String,
}

/// `touches:` from the front matter: a comma list of folders and globs, each trimmed.
pub fn parse_touches(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(String::from)
        .collect()
}

/// The text of a tool-written merge report line (`- <at> <who>: merged task/…`), from `merged`.
pub fn merge_report_line(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("- ")?;
    stamp_shaped(rest.get(..16)?).then_some(())?;
    let at = rest.find(": merged task/")?;
    Some(&rest[at + 2..])
}

/// A goal's or feature's tasks (its descendants of level task): how many are done, of how many,
/// and the estimates of the ones still open, summed (`est_minutes` is agent time plus test time,
/// as the board's totals count it).
#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub est_minutes: u64,
    pub est_token_count: u64,
}

impl Task {
    /// The owner's own task (`for: human`, 0022), which agents never take.
    pub fn is_human(&self) -> bool {
        self.for_who == "human"
    }

    /// Who's on it in its stage: the claim in Ready and Building, `held-by` in Testing and Your
    /// check (the owner's when it names nobody), nobody elsewhere.
    pub fn holder(&self) -> &str {
        match self.stage.as_str() {
            "ready" | "building" => &self.claimed_by,
            "testing" => &self.held_by,
            "your-check" if self.held_by.is_empty() => "owner",
            "your-check" => &self.held_by,
            _ => "",
        }
    }

    /// The latest check-in of the current Building stage, if any.
    pub fn current_checkin(&self) -> Option<&Checkin> {
        if self.stage != "building" {
            return None;
        }
        self.checkins
            .iter()
            .rev()
            .find(|c| c.at.as_str() >= self.stage_since.as_str())
    }
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
    /// Its tasks' prefix (0033), as `Task::prefix`.
    pub prefix: String,
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
    /// The per-machine settings the state was judged by (0036): the lease for the quiet claims,
    /// and the LIVE window the app's live marks use.
    pub settings: crate::settings::Settings,
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// A queue found under the base.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    /// The queue folder, spelled under the base as given.
    pub path: PathBuf,
    /// For a queue one level deeper (`<base>/<group>/<project>/tasks`), the folder that holds its
    /// project; "" for `<base>/<project>/tasks`.
    pub group: String,
}

/// Queues at `<base>/*/tasks` and `<base>/*/*/tasks` that look like Muster queues
/// (`projects::looks_like_queue`), and the repos' `.muster/` at `<base>/*/.muster` and
/// `<base>/*/*/.muster` in a main checkout (task 0005, `layout::found_queue`, whose group
/// `layout::found_group` says), sorted by folder. Two levels and no further. Folders named `.*`,
/// `_*`, `node_modules`, `worktrees`, `*.worktrees`, `target` or `dist` are passed over, and a
/// symlink is followed only while it stays inside the base: one that leads out of it, or back to
/// the base itself, isn't. A queue reached by two paths is found once, by the first.
pub fn discover(base: &Path) -> Vec<Found> {
    let Ok(root) = fs::canonicalize(base) else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    for (group, dir) in subfolders(base, &root) {
        candidates.push((dir.join("tasks"), String::new()));
        candidates.push((dir.join(crate::layout::DIR), String::new()));
        for (_, inner) in subfolders(&dir, &root) {
            candidates.push((inner.join("tasks"), group.clone()));
            let muster = inner.join(crate::layout::DIR);
            let in_group = crate::layout::found_group(&muster, &group);
            candidates.push((muster, in_group));
        }
    }
    candidates.sort();
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|(path, _)| {
            crate::projects::looks_like_queue(path)
                && (!crate::layout::is_muster(path) || crate::layout::found_queue(path))
        })
        .filter_map(|(path, group)| {
            let real = inside(&path, &root)?;
            seen.insert(real).then_some(Found { path, group })
        })
        .collect()
}

/// The folders in `dir` (itself inside `root`) that discovery looks into, as (name, path): real
/// folders, and symlinks to folders inside `root`, without the names it skips.
fn subfolders(dir: &Path, root: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let skipped = name.starts_with('.')
                || name.starts_with('_')
                || name.ends_with(".worktrees")
                || ["node_modules", "worktrees", "target", "dist"].contains(&name.as_str());
            let kind = e.file_type().ok()?;
            let path = e.path();
            let looked_into = kind.is_dir() || (kind.is_symlink() && inside(&path, root).is_some());
            (!skipped && looked_into).then_some((name, path))
        })
        .collect()
}

/// `path`'s real folder, when it is a folder strictly inside `root` (itself canonical).
fn inside(path: &Path, root: &Path) -> Option<PathBuf> {
    let real = fs::canonicalize(path).ok()?;
    (real != root && real.starts_with(root) && real.is_dir()).then_some(real)
}

/// Every task file of a queue: (folder it's in, path). The folder is "" for the flat layout and
/// `.muster/tasks/` (`layout::task_dirs`).
pub fn task_files(root: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for (folder, dir) in crate::layout::task_dirs(root) {
        if let Ok(rd) = fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                let name = e.file_name().to_string_lossy().to_string();
                if p.is_file() && is_task_name(&name) {
                    out.push((folder.to_string(), p));
                }
            }
        }
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
pub(crate) fn log_section(body: &str) -> Option<&str> {
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
    let text = unindent_text(text);
    Question {
        n,
        ticked,
        asker,
        choices: crate::choices::read(&text),
        text,
        escalated: item.contains("Escalated to owner"),
        owner_answered: item.contains("**A (owner"),
    }
}

/// Does a task wait on the owner (the one definition, 0022; `waiting_for_owner`)? An open question
/// escalated to them, Your check, or blocked on you or by a permission (`blocked_kind`, 0024: a
/// person has to act on both; an older `waiting_on: owner …` reads as on you). The app's needs-you
/// count is these agents' tasks plus the owner's own tasks not yet done.
pub fn needs_owner(questions: &[Question], stage: &str, blocked_kind: &str) -> bool {
    questions.iter().any(|q| !q.ticked && q.escalated)
        || stage == "your-check"
        || matches!(blocked_kind, "you" | "permission")
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
    let block_history = block_history(&log_lines);
    let done_at = done_at(&log_lines);
    let captured_at = log_lines
        .first()
        .filter(|l| stage_line(l).is_none())
        .and_then(|l| l.get(2..18))
        .filter(|st| {
            let b = st.as_bytes();
            b[4] == b'-'
                && b[7] == b'-'
                && b[10] == b' '
                && b[13] == b':'
                && st
                    .bytes()
                    .enumerate()
                    .all(|(i, c)| matches!(i, 4 | 7 | 10 | 13) || c.is_ascii_digit())
        })
        .filter(|st| {
            let day = &st[..10];
            let captured = get("captured");
            captured.is_empty() || captured == day
        })
        .map(str::to_string)
        .unwrap_or_default();
    let claimed_on = log_lines
        .iter()
        .find(|l| l.to_lowercase().contains("claimed"))
        .and_then(|l| day_of(l))
        .unwrap_or_default();
    let id = get("id");
    // In `.muster/` the stage is the only truth: a `status:` left in the file isn't read, and
    // the status is worked out from the stage below, for the readers that still use it.
    let muster = crate::layout::is_muster(root);
    let mut status = if muster { String::new() } else { get("status") };
    if status.is_empty() {
        status = folder.to_string();
    }
    let waiting_on = get("waiting_on");
    let (depends, bad_depends) = parse_depends(&get("depends"));
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
    if muster {
        status = mirror_status(&stage, blocked).to_string();
    }
    let typed = get("blocked-kind").to_lowercase();
    let (blocked_kind, block_detail, kind_guessed) = if !blocked {
        (String::new(), String::new(), false)
    } else if crate::block::TYPED.contains(&typed.as_str()) {
        // A block on you or a question keeps the old words in `waiting_on` (owner …, capture …)
        // for readers from before kinds (Drover); the detail is what follows them.
        let detail = crate::block::guessed_detail(&waiting_on, &typed);
        (typed, detail, false)
    } else {
        let kind = crate::block::guess(&waiting_on);
        let detail = crate::block::guessed_detail(&waiting_on, kind);
        (kind.to_string(), detail, true)
    };
    let stage_history: Vec<StageStep> = stage_lines(body).into_iter().map(|(s, _)| s).collect();
    let stage_since = stage_history
        .iter()
        .rev()
        .find(|s| s.to == stage)
        .map(|s| s.at.clone())
        .unwrap_or_default();
    let checkins = log_lines.iter().filter_map(|l| checkin_line(l)).collect();
    let open_questions = questions.iter().filter(|q| !q.ticked).count();
    let owner_criteria: Vec<Criterion> = criteria_items(body)
        .into_iter()
        .filter(|c| c.owner)
        .collect();
    let (vision_open, bad_vision) = match crate::vision::parse_count(&get("vision")) {
        Some(n) => (n, String::new()),
        None => (None, get("vision").chars().take(40).collect()),
    };
    let vision_ids = if vision_open.is_some() {
        crate::vision::open_ids(body)
    } else {
        Vec::new()
    };
    let waiting_for_owner = needs_owner(&questions, &stage, &blocked_kind);
    let raw_for = get("for").to_lowercase();
    let (for_who, bad_for) = match FOR_WHO.iter().find(|f| **f == raw_for) {
        Some(f) => (f.to_string(), String::new()),
        None if raw_for.is_empty() => ("ai".to_string(), String::new()),
        None => ("ai".to_string(), raw_for.chars().take(40).collect()),
    };
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
    let raw_level = get("level").to_lowercase();
    let (level, bad_level) = match LEVELS.iter().find(|l| **l == raw_level) {
        Some(l) => (l.to_string(), String::new()),
        None if raw_level.is_empty() => ("task".to_string(), String::new()),
        None => ("task".to_string(), raw_level.chars().take(40).collect()),
    };
    let steps = stage_lines(body);
    // `(set)`, or `(set; owner's override: no separate tester)` past Testing (0047).
    let stage_set_by_owner = steps
        .last()
        .is_some_and(|(_, detail)| *detail == "set" || detail.starts_with("set; "));
    let built_by = steps
        .iter()
        .rev()
        .find(|(s, detail)| s.from == "ready" && s.to == "building" && *detail == "claimed")
        .map(|(s, _)| s.who.clone())
        .unwrap_or_default();
    let gated_by = steps
        .iter()
        .rev()
        .find(|(s, detail)| {
            s.from == "building" && s.to == "testing" && detail.starts_with("gate: ")
        })
        .map(|(s, _)| s.who.clone())
        .unwrap_or_default();
    let tested_by = steps
        .iter()
        .rev()
        .find(|(s, _)| s.to == "your-check" && s.from != "done")
        .map(|(s, _)| s.who.clone())
        .unwrap_or_default();
    let est_test_time = crate::ops::clean_time(&get("est-test-time")).unwrap_or_default();
    let tags: Vec<String> = get("tags")
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    let search = crate::search::search_text(&title, &tags, body);
    let mut task = Task {
        prefix: String::new(),
        level,
        bad_level,
        parent: get("parent").chars().take(40).collect(),
        due: get("due").chars().take(40).collect(),
        children: Vec::new(),
        progress: None,
        shown_stage: stage.clone(),
        stage_set_by_owner,
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
        depends,
        bad_depends,
        stage_set,
        folder: folder.to_string(),
        path: rel,
        claimed_by: get("claimed-by"),
        held_by: get("held-by"),
        built_by,
        gated_by,
        tested_by,
        stage_since,
        checkins,
        checkin_asked: log_lines
            .iter()
            .rev()
            .find_map(|l| crate::ping::asked_line(l))
            .unwrap_or_default(),
        heartbeat: get("heartbeat"),
        waiting_on,
        blocked_kind,
        block_detail,
        blocked_by: Vec::new(),
        kind_guessed,
        commit: get("commit"),
        captured: get("captured"),
        captured_at,
        block_history,
        paused: get("paused"),
        paused_why: get("paused-why"),
        pause_history: crate::pause::history(&log_lines),
        done_at,
        questions,
        open_questions,
        owner_criteria,
        vision_open,
        vision_ids,
        bad_vision,
        waiting_for_owner,
        for_who,
        bad_for,
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
        tags,
        links: crate::fields::parse_links(&get("links")),
        comments: crate::fields::comments(body),
        attachments,
        log_tail,
        modified,
        search,
        session_ids: Vec::new(),
        live: false,
        session_state: Default::default(),
        touches: parse_touches(&get("touches")),
        worktree: get("worktree"),
        branch: get("branch"),
        worktree_runner: get("worktree-runner") == "yes",
        merge_report: log_lines
            .iter()
            .rev()
            .find_map(|l| merge_report_line(l))
            .unwrap_or_default()
            .to_string(),
    };
    // The sessions its log names, and any its claim or `held-by` names.
    task.session_ids = crate::work::session_ids(&[&task.claimed_by, &task.held_by], body);
    Some(task)
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
    files: TaskFiles,
    /// Backlog notes, the same way: by path, while size and mtime match.
    pub(crate) notes: HashMap<PathBuf, (u64, u64, crate::backlog::Note)>,
    git: HashMap<PathBuf, (u64, Vec<Activity>)>,
    /// The local time stamp, and the minute (seconds since 1970 / 60) it was read in.
    stamp: Option<(u64, String)>,
}

impl Cache {
    /// Puts back a cache a command took out to read with: what's here stays, and what `other`
    /// read that isn't here is added. Two commands can each take the cache (one gets an empty
    /// one) and put theirs back in any order without losing the fuller one. Every entry is checked
    /// against the file's size and mtime when it's used, and `read_state` drops the files that
    /// are gone, so keeping either copy of an entry is safe.
    pub fn merge(&mut self, other: Cache) {
        for (path, entry) in other.files {
            self.files.entry(path).or_insert(entry);
        }
        for (path, entry) in other.notes {
            self.notes.entry(path).or_insert(entry);
        }
        for (root, entry) in other.git {
            self.git.entry(root).or_insert(entry);
        }
        if self.stamp.is_none() {
            self.stamp = other.stamp;
        }
    }

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

/// A tool stamp (`YYYY-MM-DD HH:MM`, local time) as seconds since 1970, by the zone's own rules
/// for that day, daylight saving included (`date`, as `local_stamp` makes them). `tz` names
/// another zone (tests); None is this Mac's. None for anything that isn't a stamp.
pub fn stamp_epoch(stamp: &str, tz: Option<&str>) -> Option<i64> {
    crate::ops::stamp_minutes(stamp)?;
    let mut date = std::process::Command::new("date");
    date.args([
        "-j",
        "-f",
        "%Y-%m-%d %H:%M:%S",
        &format!("{stamp}:00"),
        "+%s",
    ]);
    if let Some(tz) = tz {
        date.env("TZ", tz);
    }
    let out = date.output().ok().filter(|o| o.status.success())?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

pub(crate) fn mtime_secs(m: &fs::Metadata) -> u64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

type TaskFiles = HashMap<PathBuf, (u64, u64, Option<Task>)>;

/// A task file through the cache: parsed again only when its size or mtime changed. None when it
/// can't be read or has no front matter.
fn cached_task<'c>(
    files: &'c mut TaskFiles,
    name: &str,
    root: &Path,
    folder: &str,
    path: &Path,
    meta: &fs::Metadata,
) -> Option<&'c Task> {
    let key = (meta.len(), mtime_secs(meta));
    let fresh = matches!(files.get(path), Some((len, m, _)) if (*len, *m) == key);
    if !fresh {
        let t = fs::read_to_string(path)
            .ok()
            .and_then(|text| parse_task(name, root, folder, path, &text, key.1));
        files.insert(path.to_path_buf(), (key.0, key.1, t));
    }
    files.get(path).and_then(|(_, _, t)| t.as_ref())
}

/// Each task's search text by `queue/id`, for the queues given (task 0032). The window asks for it
/// only while it searches, and again only when a task changed, so the polled state doesn't carry
/// up to `search::MAX_SEARCH_BYTES` a task. Read through the same cache as `read_state`; a file
/// `read_state` would report as a problem is left out.
pub fn search_index(queues: &[(String, PathBuf)], cache: &mut Cache) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (name, root) in queues {
        for (folder, path) in task_files(root) {
            let Ok(meta) = fs::metadata(&path) else {
                continue;
            };
            if meta.len() > MAX_TASK_BYTES {
                continue;
            }
            if let Some(t) = cached_task(&mut cache.files, name, root, &folder, &path, &meta) {
                out.insert(format!("{name}/{}", t.id), t.search.clone());
            }
        }
    }
    out
}

pub fn read_state(queues: &[(String, PathBuf)], cache: &mut Cache) -> State {
    read_state_with(queues, cache, &crate::settings::Settings::default())
}

/// `read_state` with this machine's settings (0036): a claim is quiet past `settings`' lease.
pub fn read_state_with(
    queues: &[(String, PathBuf)],
    cache: &mut Cache,
    settings: &crate::settings::Settings,
) -> State {
    let stamp = cache.stamp_at(now_secs());
    read_state_at_with(queues, cache, &stamp, settings)
}

/// `read_state` with the local time given: `stamp` (`YYYY-MM-DD HH:MM`) is "now" for the claims'
/// heartbeats, so tests that stamp a claim at a fixed time judge its lease at that time, not at
/// whatever the wall clock says. File times are still judged by the real clock.
pub fn read_state_at(queues: &[(String, PathBuf)], cache: &mut Cache, stamp: &str) -> State {
    read_state_at_with(queues, cache, stamp, &crate::settings::Settings::default())
}

/// `read_state_at` with this machine's settings.
pub fn read_state_at_with(
    queues: &[(String, PathBuf)],
    cache: &mut Cache,
    stamp: &str,
    settings: &crate::settings::Settings,
) -> State {
    read_state_from(queues, cache, stamp, settings, &|_| String::new())
}

/// `read_state_with`, each queue and its tasks with its project's prefix (0033), `prefix_of(name)`,
/// before anything is worked out: what the problems, the block sentences and `--json` say then
/// name tasks by their keys. For the window and the command line; the rules' own reads leave it
/// out.
pub fn read_state_keyed(
    queues: &[(String, PathBuf)],
    cache: &mut Cache,
    settings: &crate::settings::Settings,
    prefix_of: &dyn Fn(&str) -> String,
) -> State {
    let stamp = cache.stamp_at(now_secs());
    read_state_from(queues, cache, &stamp, settings, prefix_of)
}

fn read_state_from(
    queues: &[(String, PathBuf)],
    cache: &mut Cache,
    stamp: &str,
    settings: &crate::settings::Settings,
    prefix_of: &dyn Fn(&str) -> String,
) -> State {
    let mut state = State {
        now: now_secs(),
        settings: *settings,
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
            match cached_task(&mut cache.files, name, root, &folder, &path, &meta).cloned() {
                Some(t) => tasks.push(t),
                None => state
                    .problems
                    .push(format!("{name}: {} has no front matter", path.display())),
            }
        }
        let prefix = prefix_of(name);
        for t in &mut tasks {
            t.prefix.clone_from(&prefix);
        }
        crate::block::derive(&mut tasks);
        state.problems.extend(problems(
            name,
            &tasks,
            state.now,
            stamp,
            settings.lease_minutes,
        ));
        rollup(&mut tasks);
        let today = stamp.get(..10).unwrap_or("");
        state.problems.extend(level_problems(name, &tasks, today));
        for t in &tasks {
            *counts.entry(t.stage.clone()).or_default() += 1;
            if t.blocked {
                *counts.entry("blocked".into()).or_default() += 1;
            }
        }
        state.problems.extend(crate::layout::problems(name, root));
        state.activity.extend(git_activity(name, root, cache));
        state.queues.push(QueueInfo {
            name: name.clone(),
            path: root.to_string_lossy().to_string(),
            prefix,
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

/// `depends:` from the front matter, `[0014, 0016]` or `0014, 0016`: the ids (each once, in
/// order) and anything that isn't one.
pub fn parse_depends(raw: &str) -> (Vec<String>, Vec<String>) {
    let inner = raw.trim().trim_start_matches('[').trim_end_matches(']');
    let mut ids: Vec<String> = Vec::new();
    let mut bad = Vec::new();
    for tok in inner
        .split([',', ' ', '\t'])
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        if tok.len() == 4 && tok.bytes().all(|b| b.is_ascii_digit()) {
            if !ids.iter().any(|i| i == tok) {
                ids.push(tok.to_string());
            }
        } else {
            bad.push(tok.chars().take(24).collect());
        }
    }
    (ids, bad)
}

/// Loops in "depends on": each set of tasks that wait on each other (two or more), ids sorted,
/// loops sorted. A task depending on itself isn't listed here (`check` names it separately).
pub fn dependency_loops<'a>(edges: &HashMap<&'a str, Vec<&'a str>>) -> Vec<Vec<String>> {
    // Tarjan's strongly connected components, iterative so a long chain can't overflow the stack.
    let mut nodes: Vec<&str> = edges.keys().copied().collect();
    nodes.sort_unstable();
    let mut index: HashMap<&str, usize> = HashMap::new();
    let mut low: HashMap<&str, usize> = HashMap::new();
    let mut on_stack: HashSet<&str> = HashSet::new();
    let mut stack: Vec<&str> = Vec::new();
    let mut loops = Vec::new();
    let mut next = 0;
    for &start in &nodes {
        if index.contains_key(start) {
            continue;
        }
        // (node, position in its edge list)
        let mut work: Vec<(&str, usize)> = vec![(start, 0)];
        while let Some(&mut (v, ref mut i)) = work.last_mut() {
            if *i == 0 && !index.contains_key(v) {
                index.insert(v, next);
                low.insert(v, next);
                next += 1;
                stack.push(v);
                on_stack.insert(v);
            }
            let out = edges.get(v).map(Vec::as_slice).unwrap_or(&[]);
            if let Some(&w) = out.get(*i) {
                *i += 1;
                if !edges.contains_key(w) {
                    continue; // a dependency on a task that doesn't exist: check names it
                }
                if !index.contains_key(w) {
                    work.push((w, 0));
                } else if on_stack.contains(w) {
                    let lw = index[w];
                    let lv = low.get_mut(v).expect("v was indexed when first visited");
                    *lv = (*lv).min(lw);
                }
                continue;
            }
            work.pop();
            if let Some(&(parent, _)) = work.last() {
                let lv = low[v];
                let lp = low
                    .get_mut(parent)
                    .expect("parent was indexed before its child");
                *lp = (*lp).min(lv);
            }
            if low[v] == index[v] {
                let mut group = Vec::new();
                while let Some(w) = stack.pop() {
                    on_stack.remove(w);
                    group.push(w.to_string());
                    if w == v {
                        break;
                    }
                }
                if group.len() > 1 {
                    group.sort();
                    loops.push(group);
                }
            }
        }
    }
    loops.sort();
    loops
}

/// What's wrong in a queue's tasks. `now` and `stamp` are the same moment, as seconds since 1970
/// and as local time (`YYYY-MM-DD HH:MM`), for the claims' lease of `lease` minutes.
pub fn problems(queue: &str, tasks: &[Task], now: u64, stamp: &str, lease: u64) -> Vec<String> {
    let mut out = Vec::new();
    // Tasks named by their keys (0033) when they have the queue's prefix; files by their path.
    let key = |id: &str| crate::key::key(tasks.first().map_or("", |t| t.prefix.as_str()), id);
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
        // Only Building is held by someone; a blocked task (by tasks too) isn't being worked on.
        let building = t.stage == "building" && t.blocked_kind.is_empty();
        if building && t.claimed_by.is_empty() {
            out.push(format!(
                "{queue}: {} is being worked on but nobody has claimed it",
                t.path
            ));
        }
        if !t.bad_for.is_empty() {
            out.push(format!(
                "{queue}: {} has for '{}', which isn't ai or human (read as ai)",
                t.path, t.bad_for
            ));
        }
        // The lease `muster reclaim` uses: since the heartbeat, else since the file changed. The
        // owner's own task (for: human) has none, and a paused one's doesn't run (0045).
        let quiet = crate::ops::quiet_minutes(&t.heartbeat, t.modified, stamp, now);
        if building && !t.is_human() && t.paused.is_empty() && quiet > lease {
            out.push(format!(
                "{queue}: {} is claimed but hasn't been heard from in {} h",
                t.path,
                quiet / 60
            ));
        }
        if !t.bad_vision.is_empty() {
            out.push(format!(
                "{queue}: {} has vision '{}', which isn't a count of open conflicts (`vision: <n> open`); record a new check",
                t.path, t.bad_vision
            ));
        }
        if let Some(n) = t.vision_open.filter(|n| *n != t.vision_ids.len()) {
            out.push(format!(
                "{queue}: {} says vision {n} open but its Vision section has {} open (edited by hand? record a new check)",
                t.path,
                t.vision_ids.len()
            ));
        }
        if t.status == "blocked" && t.waiting_on.is_empty() {
            out.push(format!(
                "{queue}: {} is blocked but doesn't say on what",
                t.path
            ));
        }
        if t.kind_guessed {
            // For an old block on task ids, only those still open: merged (0042) or missing ones
            // don't hold it, and with none left it only needs unblocking.
            let under_way = crate::block::started(t);
            let open: Vec<String> = crate::block::ids_in(&t.waiting_on)
                .into_iter()
                .filter(|d| {
                    tasks
                        .iter()
                        .any(|o| o.id == *d && crate::block::holds(o, under_way))
                })
                .collect();
            let what = t.waiting_on.chars().take(80).collect::<String>();
            let (kind, id) = (&t.blocked_kind, key(&t.id));
            let how = match (kind.as_str(), open.is_empty()) {
                ("tasks", true) => format!("none of them is open now: muster unblock {id}"),
                ("tasks", false) => format!(
                    "block it again with one to confirm (muster block {id} --on {})",
                    open.iter().map(|d| key(d)).collect::<Vec<_>>().join(",")
                ),
                _ => format!(
                    "block it again with one to confirm (muster block {id} --on {kind} \"…\")"
                ),
            };
            out.push(format!(
                "{queue}: {WARNING}{} is blocked on {what:?} without a kind, read as {kind}; {how}",
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
    // Dependencies (0019): ids only, never itself, every one a task here, and no loops.
    let mut edges: HashMap<&str, Vec<&str>> = HashMap::new();
    for t in tasks {
        for b in &t.bad_depends {
            out.push(format!(
                "{queue}: {}'s depends has {b:?}, which isn't a task id",
                t.path
            ));
        }
        for d in &t.depends {
            if *d == t.id {
                out.push(format!("{queue}: {} depends on itself", t.path));
            } else if !known.contains(d.as_str()) {
                out.push(format!(
                    "{queue}: {} depends on {}, which doesn't exist",
                    t.path,
                    key(d)
                ));
            }
        }
        edges
            .entry(t.id.as_str())
            .or_default()
            .extend(t.depends.iter().map(String::as_str).filter(|d| *d != t.id));
    }
    for l in dependency_loops(&edges) {
        let l: Vec<String> = l.iter().map(|d| key(d)).collect();
        out.push(format!(
            "{queue}: {} depend on each other in a loop",
            l.join(", ")
        ));
    }
    // An old block on a task id (a typed block's detail is free text).
    for t in tasks
        .iter()
        .filter(|t| t.kind_guessed && t.blocked_kind == "tasks")
    {
        let r: String = t.waiting_on.chars().take(4).collect();
        if r.len() == 4 && r.bytes().all(|b| b.is_ascii_digit()) && !known.contains(r.as_str()) {
            out.push(format!(
                "{queue}: {} waits on {}, which doesn't exist",
                t.path,
                key(&r)
            ));
        }
    }
    out.sort();
    out
}

/// The goals' and features' roll-ups, once every task of a queue is read: their children (by
/// `parent:` in this queue), their tasks' progress and open estimates, and the stage they show
/// (`Task::shown_stage`): Done when every direct child is Done (and it has one; a feature with no
/// tasks isn't Done), In progress once any child is past Ready, else Ready. Features are worked
/// out before goals, so a goal reads its features' shown stages. One being shaped (Backlog,
/// Shaping), or whose stage the owner set, shows its own. Loops and wrong levels are
/// `level_problems`' to report; here each task is counted once, so a loop can't make it go round.
pub fn rollup(tasks: &mut [Task]) {
    let mut index: HashMap<&str, usize> = HashMap::new();
    for (i, t) in tasks.iter().enumerate() {
        index.entry(t.id.as_str()).or_insert(i);
    }
    let mut kids: Vec<Vec<usize>> = vec![Vec::new(); tasks.len()];
    for (i, t) in tasks.iter().enumerate() {
        match index.get(t.parent.as_str()) {
            Some(&p) if p != i && !t.parent.is_empty() => kids[p].push(i),
            _ => {}
        }
    }
    let ready = STAGES.iter().position(|s| *s == "ready").unwrap_or(0);
    for g in 0..tasks.len() {
        if tasks[g].level == "task" {
            continue;
        }
        let mut progress = Progress::default();
        let mut seen = HashSet::from([g]);
        let mut stack = kids[g].clone();
        while let Some(i) = stack.pop() {
            if !seen.insert(i) {
                continue;
            }
            stack.extend(&kids[i]);
            let t = &tasks[i];
            if t.level != "task" {
                continue;
            }
            progress.total += 1;
            if t.stage == "done" {
                progress.done += 1;
            } else {
                progress.est_minutes = progress
                    .est_minutes
                    .saturating_add(t.est_minutes)
                    .saturating_add(t.est_test_minutes);
                progress.est_token_count =
                    progress.est_token_count.saturating_add(t.est_token_count);
            }
        }
        let mut children: Vec<String> = kids[g].iter().map(|&i| tasks[i].id.clone()).collect();
        children.sort();
        tasks[g].children = children;
        tasks[g].progress = Some(progress);
    }
    // The shown stage from the direct children's: features first, then goals that read them.
    let past_ready =
        |s: &str| s == "in-progress" || STAGES.iter().position(|x| *x == s).unwrap_or(0) > ready;
    for level in ["feature", "goal"] {
        for g in 0..tasks.len() {
            let t = &tasks[g];
            let own = matches!(t.stage.as_str(), "backlog" | "shaping") || t.stage_set_by_owner;
            if t.level != level || own {
                continue;
            }
            let shown: Vec<&str> = kids[g]
                .iter()
                .map(|&i| tasks[i].shown_stage.as_str())
                .collect();
            let stage = if !shown.is_empty() && shown.iter().all(|s| *s == "done") {
                "done"
            } else if shown.iter().any(|s| past_ready(s)) {
                "in-progress"
            } else {
                "ready"
            };
            tasks[g].shown_stage = stage.to_string();
        }
    }
}

/// How a warning line starts after its queue's name: it's shown with the problems, but `muster
/// check` doesn't fail on it.
const WARNING: &str = "warning: ";

/// Is this line of `State::problems` a warning (`level_problems`' late goal)?
pub fn is_warning(line: &str) -> bool {
    line.split_once(": ")
        .is_some_and(|(_, rest)| rest.starts_with(WARNING))
}

/// A `YYYY-MM-DD` date that exists.
pub fn is_date(d: &str) -> bool {
    d.len() == 10 && crate::ops::stamp_minutes(&format!("{d} 00:00")).is_some()
}

/// What's wrong with a queue's levels and parent links, after `rollup`: a level that isn't one, a
/// parent that doesn't exist or is at the wrong level (a task's is a feature or a goal, a
/// feature's a goal, a goal has none), parents that go round in a loop, a due that isn't a date,
/// and, as a warning, a goal past its due date (before `today`, `YYYY-MM-DD`) with tasks open.
pub fn level_problems(queue: &str, tasks: &[Task], today: &str) -> Vec<String> {
    let mut out = Vec::new();
    // Tasks named by their keys (0033) when they have the queue's prefix.
    let key = |id: &str| crate::key::key(tasks.first().map_or("", |t| t.prefix.as_str()), id);
    let mut index: HashMap<&str, &Task> = HashMap::new();
    for t in tasks {
        index.entry(t.id.as_str()).or_insert(t);
    }
    for t in tasks {
        if !t.bad_level.is_empty() {
            out.push(format!(
                "{queue}: {} has level '{}', which isn't a level (read as task)",
                t.path, t.bad_level
            ));
        }
        if !t.parent.is_empty() {
            let p = &key(&t.parent);
            match (
                t.level.as_str(),
                index.get(t.parent.as_str()).map(|p| p.level.as_str()),
            ) {
                ("goal", _) => out.push(format!(
                    "{queue}: {} is a goal with a parent ({p}); a goal has none",
                    t.path
                )),
                (_, None) => out.push(format!(
                    "{queue}: {} has parent {p}, which doesn't exist",
                    t.path
                )),
                ("feature", Some(pl)) if pl != "goal" => out.push(format!(
                    "{queue}: {} is a feature under {pl} {p}; a feature's parent is a goal",
                    t.path
                )),
                ("task", Some("task")) => out.push(format!(
                    "{queue}: {} is a task under task {p}; a task's parent is a feature or a goal",
                    t.path
                )),
                _ => {}
            }
        }
        if !t.due.is_empty() && !is_date(&t.due) {
            out.push(format!(
                "{queue}: {} has due '{}', which isn't a date (YYYY-MM-DD)",
                t.path, t.due
            ));
        } else if t.level == "goal" && is_date(&t.due) && t.due.as_str() < today {
            if let Some(p) = t.progress.as_ref().filter(|p| p.done < p.total) {
                out.push(format!(
                    "{queue}: {WARNING}{} is past its due date ({}) with {} of {} tasks open",
                    t.path,
                    t.due,
                    p.total - p.done,
                    p.total
                ));
            }
        }
    }
    // Each loop once, from its lowest id.
    let mut loops: HashSet<Vec<&str>> = HashSet::new();
    for start in tasks {
        let mut path: Vec<&str> = vec![start.id.as_str()];
        let mut cur = start;
        while let Some(p) = index
            .get(cur.parent.as_str())
            .filter(|_| !cur.parent.is_empty())
        {
            if let Some(at) = path.iter().position(|id| *id == p.id) {
                let mut ring = path[at..].to_vec();
                let low = (0..ring.len()).min_by_key(|&i| ring[i]).unwrap_or(0);
                ring.rotate_left(low);
                if loops.insert(ring.clone()) {
                    let ring: Vec<String> = ring.iter().map(|id| key(id)).collect();
                    out.push(format!(
                        "{queue}: parents go round in a loop: {} → {}",
                        ring.join(" → "),
                        ring[0]
                    ));
                }
                break;
            }
            path.push(p.id.as_str());
            cur = p;
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

/// The status folder a task file is in, or "" when it sits in the queue folder (flat layout) or
/// in `.muster/tasks/`.
pub fn folder_of(root: &Path, path: &Path) -> String {
    crate::layout::folder_of(root, path)
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

/// Who's acting, as it goes into the front matter and the log: one line, a label (`--as`) of at
/// most 100 characters plus the session stamp `muster` adds (0030, `session::stamped`), and
/// nothing that would let a log line pass for tool-written stage history or a check-in.
pub fn actor(who: &str) -> Result<String, String> {
    let t: String = who
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() {
        return Err("who (--as) is empty".into());
    }
    if crate::session::split(&t).0.chars().count() > 100 {
        return Err("who (--as) is longer than 100 characters".into());
    }
    if t.contains(": stage ") || t.contains(" → ") || t.contains(": check-in ") {
        return Err(format!(
            "who (--as) can't contain \": stage \", \" → \" or \": check-in \" (that's what the tool logs): {t:?}"
        ));
    }
    Ok(t)
}

/// `actor` for the Questions section (contract rule 2): at most 100 characters in all.
fn answerer(who: &str) -> Result<String, String> {
    let t = actor(who)?;
    if t.chars().count() > 100 {
        return Err("who (--as) is longer than 100 characters".into());
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
pub(crate) fn questions_span(body: &str) -> Option<(usize, usize)> {
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
    ask_with(root, id, text, &Default::default(), who, session, stamp)
}

/// `ask`, with the options `choices::clean` checked (none for a question in words) written under
/// the question as its last lines (`choices::lines`).
pub fn ask_with(
    root: &Path,
    id: &str,
    text: &str,
    choices: &crate::choices::Choices,
    who: &str,
    session: &str,
    stamp: &str,
) -> Result<Asked, String> {
    let who = answerer(who)?;
    let question = clean_text_as(text, "the question")?;
    let written = match crate::choices::lines(choices) {
        options if options.is_empty() => question.clone(),
        options => format!("{question}\n{options}"),
    };
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
        let n = next_question(&items)?;
        let item = format!(
            "- [ ] **Q{n} ({who}, session {sid}, {stamp}):** {}\n",
            indent_text(&written)
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
                t.stage == "building"
                    && !t.claimed_by.is_empty()
                    && crate::session::same_holder(&t.claimed_by, &who)
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

/// The number the next question in a task gets: one past the highest, and past the count, so old
/// unnumbered questions are counted too.
fn next_question(items: &[Question]) -> Result<u32, String> {
    let highest = items.iter().filter_map(|q| q.n).max().unwrap_or(0);
    let count = u32::try_from(items.len()).unwrap_or(u32::MAX);
    highest
        .max(count)
        .checked_add(1)
        .ok_or_else(|| "the task has too many questions".to_string())
}

/// A question the tool asks the owner, not a session (the vision check's change of direction,
/// 0029), added to the task `text`: numbered on from its questions, written in `## Questions`
/// (lines after the first indented, contract rule 3) and already "Escalated to owner", so it waits
/// for the owner like any escalated question. No session asked it, so its answer stays in the
/// task. `about` says where it comes from, beside who and when. Returns the new text and the
/// question's number; the caller holds the lock and writes the file.
pub(crate) fn add_owner_question(
    text: &str,
    who: &str,
    about: &str,
    question: &str,
    stamp: &str,
) -> Result<(String, u32), String> {
    let body = with_questions(text);
    let (start, end) = questions_span(&body).ok_or("the task's Questions section is gone")?;
    let sect = &body[start..end];
    let items: Vec<Question> = question_items(sect)
        .into_iter()
        .map(|(a, b)| parse_question(&sect[a..b]))
        .collect();
    let n = next_question(&items)?;
    let item = format!(
        "- [ ] **Q{n} ({who}, {about}, {stamp}):** {}\n  **Escalated to owner ({who}, {stamp}).**\n",
        indent_text(question.trim())
    );
    let kept = sect.trim_end_matches('\n');
    let new_sect = if kept.trim().is_empty() {
        format!("{item}\n")
    } else {
        format!("{kept}\n{item}\n")
    };
    Ok((format!("{}{new_sect}{}", &body[..start], &body[end..]), n))
}

/// Was a question with this asker part asked by the tool with `about` (`<who>, <about>, <stamp>`,
/// as `add_owner_question` writes it)? A session's question has its session there instead.
fn asked_by_tool(asker: &str, about: &str) -> bool {
    let parts: Vec<&str> = asker.rsplitn(3, ", ").collect();
    parts.len() == 3 && parts[1] == about && crate::ops::stamp_minutes(parts[0]).is_some()
}

/// Close the open questions the tool asked with `about` (`add_owner_question`): each is ticked
/// with `answer` under it as `who`'s answer, so it leaves the owner's list. Answered questions and
/// every session's question stay as they are. Returns the new text and the closed questions'
/// numbers; the caller holds the lock and writes the file.
pub(crate) fn close_owner_questions(
    text: &str,
    about: &str,
    who: &str,
    stamp: &str,
    answer: &str,
) -> (String, Vec<u32>) {
    let Some((start, end)) = questions_span(text) else {
        return (text.to_string(), Vec::new());
    };
    let sect = &text[start..end];
    let mut out = String::new();
    let mut last = 0;
    let mut closed = Vec::new();
    for (a, b) in question_items(sect) {
        let item = &sect[a..b];
        let q = parse_question(item);
        if q.ticked || !asked_by_tool(&q.asker, about) {
            continue;
        }
        out.push_str(&sect[last..a]);
        out.push_str(&format!(
            "{}\n  **A ({who}, {stamp}):** {}",
            item.replacen("- [ ] **Q", "- [x] **Q", 1),
            indent_text(answer)
        ));
        last = b;
        closed.extend(q.n);
    }
    out.push_str(&sect[last..]);
    (format!("{}{out}{}", &text[..start], &text[end..]), closed)
}

/// Mark question `n` of task `id` "Escalated to owner" (capture takes it to the owner), once.
pub fn escalate(root: &Path, id: &str, n: u32, who: &str, stamp: &str) -> Result<(), String> {
    let who = answerer(who)?;
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
    let text = clean_text(text)?;
    answer_with(root, id, n, who, stamp, |_| Ok(text)).map(|(asker, _)| asker)
}

/// Answer question `n` of task `id` by picking one of its options (0049): `pick` is a letter or a
/// number (`choices::which`), and the answer is `(b) <the option's text>`, then `extra` after
/// " — " when there is any, written, logged and mailed as `answer` does. Refused, with nothing
/// written, for a pick that isn't one of the options or a question without any. Returns the
/// asker's session, if one was found, and the answer as written.
pub fn answer_pick(
    root: &Path,
    id: &str,
    n: u32,
    pick: &str,
    extra: &str,
    who: &str,
    stamp: &str,
) -> Result<(Option<String>, String), String> {
    answer_with(root, id, n, who, stamp, |item| {
        let choices = parse_question(item)
            .choices
            .ok_or_else(|| format!("Q{n} has no options to pick from: answer it in words"))?;
        let text =
            crate::choices::answer_text(&choices, pick, extra).map_err(|e| format!("Q{n}: {e}"))?;
        clean_text(&text)
    })
}

/// Answer question `n` of task `id` as `who` with the text `answer_of` makes from the question's
/// item, under the queue's lock, then drop the asker's mailbox file. Nothing is written when it
/// refuses. Returns the asker's session, if one was found, and the answer as written.
fn answer_with(
    root: &Path,
    id: &str,
    n: u32,
    who: &str,
    stamp: &str,
    answer_of: impl FnOnce(&str) -> Result<String, String>,
) -> Result<(Option<String>, String), String> {
    let who = answerer(who)?;
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
        // One answer per question: a second (a double click on a chip) would add a second answer
        // and mail the asker again.
        if parse_question(item).ticked {
            return Err(format!("Q{n} is already answered"));
        }
        let text = answer_of(item)?;
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
        // A block on this question clears with its answer (0024), under the same lock. The answer
        // is written either way, so a failure here leaves the block for `muster unblock` rather
        // than reporting an answer that's in the file as failed.
        let _ = crate::ops::unblock_answered_locked(root, id, n, &who, stamp);
        // Unblocked, the file may have moved out of blocked/.
        let path = find_task(root, id).unwrap_or(path);
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
        Ok((asker, text))
    })
}

/// Run `f` holding the queue's lock (an exclusive flock on `mail/.lock`, file contract rule 1),
/// then, still holding it, bring INDEX.md up to date (task 0018). Every write muster makes to a
/// queue goes through here, so this is the one place the master list follows the files. A queue
/// with no stored prefix gets its default stored first (0033, `key::pin_prefix`), so its keys
/// don't move when another project comes along.
pub fn with_lock<T>(root: &Path, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    hold_lock(root, || {
        crate::key::pin_prefix(root);
        let result = f();
        // The change is made (or refused) either way, so a list that can't be written never
        // turns it into an error: `muster check` reports the stale list instead.
        let _ = crate::index::refresh_locked(root);
        result
    })
}

/// The queue's lock alone, without bringing INDEX.md up to date: for `index::rebuild`.
pub(crate) fn hold_lock<T>(
    root: &Path,
    f: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
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

/// Contract rule 4: `mail/asks.json`, else `session xxxxxxxx` in the question line's asker part
/// (`**Q3 (<who>, session <id>, <stamp>):**`). Only there: the question's own text is anyone's,
/// so a session it names never gets the answer, and a question with none there gets no mailbox.
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
    let asker = parse_question(item).asker;
    let i = asker.find("session ")? + 8;
    let s: String = asker[i..].chars().take(8).collect();
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
    fn needs_owner_is_a_question_for_them_your_check_or_a_block_on_you_or_by_a_permission() {
        let q = |ticked: bool, escalated: bool| Question {
            n: Some(1),
            ticked,
            asker: "wt-a".into(),
            text: "Which?".into(),
            escalated,
            owner_answered: false,
            choices: None,
        };
        // 0022's: an open question escalated to the owner, and Your check.
        assert!(needs_owner(&[q(false, true)], "building", ""));
        assert!(!needs_owner(
            &[q(true, true), q(false, false)],
            "building",
            ""
        ));
        assert!(needs_owner(&[], "your-check", ""));
        // 0024's: blocked on you or by a permission; no other kind.
        assert!(needs_owner(&[], "building", "you"));
        assert!(needs_owner(&[], "ready", "permission"));
        for kind in ["tasks", "question", "outside", ""] {
            assert!(!needs_owner(&[], "building", kind), "{kind}");
        }
        // A block from before kinds on the owner reads as one on you (block::guess), so it counts.
        let text = "---\nid: 0001\nstage: building\nstatus: blocked\nwaiting_on: owner: pick a colour\n---\n\n# T\n";
        let t = parse_task(
            "q",
            Path::new("/q"),
            "blocked",
            Path::new("/q/blocked/0001-t.md"),
            text,
            0,
        )
        .unwrap();
        assert_eq!((t.blocked_kind.as_str(), t.kind_guessed), ("you", true));
        assert!(t.waiting_for_owner);
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
    fn the_polled_state_leaves_the_search_text_out() {
        let d = queue();
        let s = read_state(
            &[("p".into(), d.path().to_path_buf())],
            &mut Cache::default(),
        );
        assert!(s.tasks[0].search.starts_with("something broke details"));
        let json = serde_json::to_value(&s).unwrap();
        assert!(json["tasks"][0].get("search").is_none(), "{json}");
        assert_eq!(json["tasks"][0]["title"], "Something broke");
    }

    #[test]
    fn search_index_gives_each_tasks_text_by_queue_and_id_through_the_cache() {
        let a = queue();
        let b = tempfile::tempdir().unwrap();
        let file = b.path().join("0007-x.md");
        let task = "---\nid: 0007\nstatus: todo\n---\n\n# Board flickers\n\nThe columns jump.\n";
        fs::write(&file, task).unwrap();
        fs::write(b.path().join("0008-y.md"), "no front matter").unwrap();
        let qa = ("a".to_string(), a.path().to_path_buf());
        let qb = ("b".to_string(), b.path().to_path_buf());
        let mut c = Cache::default();
        let all = search_index(&[qa.clone(), qb.clone()], &mut c);
        let mut keys: Vec<_> = all.keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(keys, ["a/0042", "b/0007"]);
        assert!(all["a/0042"].starts_with("something broke details"));
        assert_eq!(all["b/0007"], "board flickers the columns jump");
        // Read once, kept in the cache with the files `state` reads.
        assert_eq!(c.files.len(), 3);
        // One queue: only its tasks, the same shape.
        let one = search_index(std::slice::from_ref(&qb), &mut c);
        assert_eq!(one.keys().collect::<Vec<_>>(), ["b/0007"]);
        // A changed file is read again.
        fs::write(&file, task.replace("jump", "jump about")).unwrap();
        assert_eq!(
            search_index(&[qb], &mut c)["b/0007"],
            "board flickers the columns jump about"
        );
        assert_eq!(read_state(&[qa], &mut c).tasks.len(), 1);
    }

    #[test]
    fn a_cache_put_back_keeps_what_is_there_and_adds_what_it_lacks() {
        let a = queue();
        let b = queue();
        let qa = ("a".to_string(), a.path().to_path_buf());
        let qb = ("b".to_string(), b.path().to_path_buf());
        let mut there = Cache::default();
        read_state(std::slice::from_ref(&qa), &mut there);
        let mut mine = Cache::default();
        search_index(&[qa, qb], &mut mine);
        there.merge(mine);
        assert_eq!(there.files.len(), 2);
        assert!(there.stamp.is_some());
        let mut empty = Cache::default();
        let mut only_b = Cache::default();
        search_index(&[("b".to_string(), b.path().to_path_buf())], &mut only_b);
        empty.merge(only_b);
        assert_eq!(empty.files.len(), 1);
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
    fn check_ins_the_stage_start_and_the_holder_come_from_the_file() {
        let log = "## Log\n\
- 2026-10-12 10:00 wt-a (session aaaa1111): check-in 10% · before the claim\n\
- 2026-10-12 10:05 wt-a (session aaaa1111): stage ready → building (claimed)\n\
- 2026-10-12 10:30 wt-a (session aaaa1111): check-in 40% · gate green · writing tests\n\
- 2026-10-12 10:31 wt-a: check-in 101% · too much\n\
- 2026-10-12 10:32 wt-a: check-in 4o% · not a number\n\
- 2026-10-12 10:33 wt-a: check-in · \n\
- 2026-10-12 10:34 : check-in · nobody\n\
- 2026-10-12 10:35 wt-a: Check-in 50% · not the tool's\n\
- 2026-10-12 wt-a: check-in 50% · no time\n\
- 2026-10-12 10:36 x: stage testing → done (y: check-in 9% · z)\n\
- 2026-10-12 10:40 owner: fyi: check-in 50% · a note's who with a colon\n\
- 2026-10-12 10:50 wt-a (session aaaa1111): check-in · 60% · no pct given, so this is text\n";
        let text = format!(
            "{}{log}",
            TASK.split("## Log").next().unwrap().replace(
                "claimed-by: wt-a (session aaaa1111)",
                "claimed-by: wt-a (session aaaa1111)\nheld-by: qa"
            )
        );
        let t = parse_one(&text, "doing");
        let got: Vec<_> = t
            .checkins
            .iter()
            .map(|c| format!("{} {}|{:?}|{}", c.at, c.who, c.pct, c.text))
            .collect();
        assert_eq!(
            got,
            [
                "2026-10-12 10:00 wt-a (session aaaa1111)|Some(10)|before the claim",
                "2026-10-12 10:30 wt-a (session aaaa1111)|Some(40)|gate green · writing tests",
                "2026-10-12 10:40 owner: fyi|Some(50)|a note's who with a colon",
                "2026-10-12 10:50 wt-a (session aaaa1111)|None|60% · no pct given, so this is text",
            ]
        );
        assert_eq!(t.stage_since, "2026-10-12 10:05");
        assert_eq!(t.held_by, "qa");
        // Building's holder is the claim; the check-in it shows is the latest since the stage began.
        assert_eq!(t.holder(), "wt-a (session aaaa1111)");
        assert_eq!(
            t.current_checkin().map(|c| c.at.as_str()),
            Some("2026-10-12 10:50")
        );
        let v = serde_json::to_value(&t).unwrap();
        assert_eq!(v["heldBy"], "qa");
        assert_eq!(v["stageSince"], "2026-10-12 10:05");
        assert_eq!(v["checkins"][0]["pct"], 10);
        assert!(v["checkins"][3]["pct"].is_null());
        // Testing and Your check are held by `held-by`; with no stage line into the stage, no start.
        let testing = parse_one(
            &text.replace("status: doing", "status: doing\nstage: testing"),
            "doing",
        );
        assert_eq!((testing.holder(), testing.stage_since.as_str()), ("qa", ""));
        assert_eq!(
            testing.current_checkin(),
            None,
            "Building's check-ins aren't Testing's"
        );
        let ready = parse_one(
            &text.replace("status: doing", "status: todo\nstage: ready"),
            "todo",
        );
        assert_eq!(ready.holder(), "wt-a (session aaaa1111)");
        let done = parse_one(
            &text.replace("status: doing", "status: done\nstage: done"),
            "done",
        );
        assert_eq!(done.holder(), "");
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
                .replace(
                    "waiting_on:",
                    "waiting_on: pick a colour\nblocked-kind: you",
                ),
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
        let quiet: Vec<_> = problems("p", &tasks, now, stamp, crate::ops::LEASE_MINUTES)
            .into_iter()
            .filter(|p| p.contains("heard from"))
            .collect();
        // A shorter lease from the settings (0036): 0002, an hour quiet, is quiet past 30 min.
        let short: Vec<_> = problems("p", &tasks, now, stamp, 30)
            .into_iter()
            .filter(|p| p.contains("heard from"))
            .collect();
        assert_eq!(short.len(), 4, "{short:?}");
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
        // With this machine's settings (0036): a 2-hour lease, and the state says which it used.
        let short = crate::settings::Settings {
            lease_minutes: 120,
            live_minutes: 10,
        };
        let s = read_state_at_with(&q, &mut Cache::default(), "2026-10-12 03:01", &short);
        assert_eq!(
            s.problems
                .iter()
                .filter(|p| p.contains("heard from"))
                .count(),
            1
        );
        assert_eq!(s.settings, short);
        assert!(serde_json::to_string(&s)
            .unwrap()
            .contains(r#""settings":{"leaseMinutes":120,"liveMinutes":10}"#));
        assert_eq!(
            quiet("2026-10-12 03:01"),
            0,
            "the default lease is still 12 h"
        );
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

    fn lease_choices() -> crate::choices::Choices {
        crate::choices::clean(
            &[
                "Lease 1 h to 7 d".into(),
                "Any length".into(),
                "No lease".into(),
            ],
            Some("a"),
            Some("c"),
        )
        .unwrap()
    }

    #[test]
    fn ask_with_options_writes_them_as_indented_lines_and_reads_them_back() {
        let d = plain();
        let q = "How long may a lease be?\nIt's set in Settings.";
        let a = ask_with(d.path(), "0007", q, &lease_choices(), "wt-a", "aaaa1111", S).unwrap();
        assert_eq!(a.n, 1);
        let b = body_of(d.path(), "0007");
        assert!(
            b.contains("## Questions\n- [ ] **Q1 (wt-a, session aaaa1111, 2026-10-12 09:00):** How long may a lease be?\n  It's set in Settings.\n  - (a) Lease 1 h to 7 d (my pick)\n  - (b) Any length\n  - (c) No lease\n  With no answer: (c)\n\n## Log\n"),
            "{b}"
        );
        let t = read_task(d.path(), "0007").unwrap();
        let c = t.questions[0].choices.clone().expect("read back");
        assert_eq!(
            (c.options.len(), c.pick, c.no_answer, c.question.as_str()),
            (3, Some(0), Some(2), q)
        );
        // Escalated, it still has them, with the escalation under the question.
        escalate(d.path(), "0007", 1, "capture", S).unwrap();
        let t = read_task(d.path(), "0007").unwrap();
        let c = t.questions[0].choices.clone().expect("still there");
        assert_eq!(
            c.question,
            format!("{q}\n**Escalated to owner (capture, {S}).**")
        );
        assert_eq!(c.options[1], "Any length");
        // A question without them has none.
        ask(d.path(), "0007", "Which port?", "wt-a", "aaaa1111", S).unwrap();
        assert_eq!(
            read_task(d.path(), "0007").unwrap().questions[1].choices,
            None
        );
    }

    #[test]
    fn answer_pick_writes_the_options_text_logs_it_and_mails_the_asker() {
        let d = plain();
        let q = "How long may a lease be?";
        ask_with(d.path(), "0007", q, &lease_choices(), "wt-a", "aaaa1111", S).unwrap();
        let at = "2026-10-12 10:00";
        let (asker, text) =
            answer_pick(d.path(), "0007", 1, "B", "but warn\nafter 7 d", "owner", at).unwrap();
        assert_eq!(asker.as_deref(), Some("aaaa1111"));
        assert_eq!(text, "(b) Any length — but warn\nafter 7 d");
        let b = body_of(d.path(), "0007");
        assert!(
            b.contains("- [x] **Q1 (wt-a, session aaaa1111, 2026-10-12 09:00):** How long may a lease be?\n  - (a) Lease 1 h to 7 d (my pick)\n  - (b) Any length\n  - (c) No lease\n  With no answer: (c)\n  **A (owner, 2026-10-12 10:00):** (b) Any length — but warn\n  after 7 d\n"),
            "{b}"
        );
        assert!(
            b.ends_with("- 2026-10-12 10:00 owner: answered Q1\n"),
            "{b}"
        );
        let mail = fs::read_to_string(d.path().join("mail/aaaa1111/0007-Q1.md")).unwrap();
        assert!(
            mail.contains("\nAnswer: (b) Any length — but warn\nafter 7 d\n"),
            "{mail}"
        );
        let t = read_task(d.path(), "0007").unwrap();
        assert!(t.questions[0].ticked && t.questions[0].owner_answered);
        // A number picks too, and with no extra words it's the option alone.
        ask_with(
            d.path(),
            "0007",
            "Again?",
            &lease_choices(),
            "wt-a",
            "aaaa1111",
            S,
        )
        .unwrap();
        let (_, text) = answer_pick(d.path(), "0007", 2, "3", " ", "owner", at).unwrap();
        assert_eq!(text, "(c) No lease");
    }

    #[test]
    fn an_answered_question_refuses_a_second_answer_and_writes_nothing() {
        // A second click on a chip (review, 0049): one answer per question, one mail.
        let d = plain();
        ask_with(
            d.path(),
            "0007",
            "Lease?",
            &lease_choices(),
            "wt-a",
            "aaaa1111",
            S,
        )
        .unwrap();
        answer_pick(d.path(), "0007", 1, "a", "", "owner", S).unwrap();
        let mail = d.path().join("mail/aaaa1111/0007-Q1.md");
        fs::rename(&mail, d.path().join("taken.md")).unwrap();
        let before = body_of(d.path(), "0007");
        let later = "2026-10-12 09:01";
        assert!(answer_pick(d.path(), "0007", 1, "b", "", "owner", later)
            .unwrap_err()
            .contains("Q1 is already answered"));
        assert!(answer(d.path(), "0007", 1, "No, b.", "owner", later)
            .unwrap_err()
            .contains("Q1 is already answered"));
        assert_eq!(body_of(d.path(), "0007"), before);
        assert_eq!(before.matches("**A (owner").count(), 1);
        assert_eq!(before.matches("answered Q1").count(), 1);
        assert!(!mail.exists(), "no second mail");
    }

    #[test]
    fn a_bad_pick_or_bad_options_write_nothing() {
        let d = plain();
        ask_with(
            d.path(),
            "0007",
            "Lease?",
            &lease_choices(),
            "wt-a",
            "aaaa1111",
            S,
        )
        .unwrap();
        ask(d.path(), "0007", "Which port?", "wt-a", "aaaa1111", S).unwrap();
        let before = body_of(d.path(), "0007");
        let pick = |n: u32, p: &str| answer_pick(d.path(), "0007", n, p, "", "owner", S);
        assert!(pick(1, "d")
            .unwrap_err()
            .contains("\"d\" isn't one of the options"));
        assert!(pick(1, "0")
            .unwrap_err()
            .contains("isn't one of the options"));
        assert!(pick(2, "a")
            .unwrap_err()
            .contains("Q2 has no options to pick from: answer it in words"));
        assert!(pick(9, "a").unwrap_err().contains("no Q9 in 0007"));
        assert!(
            answer_pick(d.path(), "0007", 1, "a", &"x".repeat(4000), "owner", S)
                .unwrap_err()
                .contains("longer than 4000")
        );
        assert_eq!(body_of(d.path(), "0007"), before);
        assert!(!d.path().join("mail/aaaa1111").exists(), "no mail");
        // Free text still answers a question with options.
        answer(d.path(), "0007", 1, "Something else: 3 d.", "owner", S).unwrap();
        assert!(body_of(d.path(), "0007")
            .contains("**A (owner, 2026-10-12 09:00):** Something else: 3 d.\n"));
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

    /// A flat queue of items `(id, front-matter lines)`, each titled "Item <id>".
    fn levels_queue(items: &[(&str, &str)]) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for (id, fm) in items {
            fs::write(
                d.path().join(format!("{id}-x.md")),
                format!("---\nid: {id}\ntype: task\npriority: normal\n{fm}\n---\n\n# Item {id}\n\n## Log\n- 2026-10-12 09:00 capture: created\n"),
            )
            .unwrap();
        }
        d
    }
    fn levels_state(d: &tempfile::TempDir) -> State {
        read_state_at(
            &[("p".into(), d.path().to_path_buf())],
            &mut Cache::default(),
            "2026-10-12 14:05",
        )
    }
    const READY: &str = "stage: ready\nstatus: todo";
    const BUILDING: &str = "stage: building\nstatus: doing\nclaimed-by: wt-a";
    const DONE: &str = "stage: done\nstatus: done";

    #[test]
    fn reads_level_parent_and_due_and_serializes_them_camel_case() {
        let t = parse_one(
            &TASK.replace(
                "status: doing",
                "status: doing\nlevel: Feature\nparent: 0030\ndue: 2026-11-01",
            ),
            "",
        );
        assert_eq!(
            (t.level.as_str(), t.parent.as_str(), t.due.as_str()),
            ("feature", "0030", "2026-11-01")
        );
        // No level is a task; an unknown one is read as a task, and `check` says so.
        let t = parse_one(TASK, "");
        assert_eq!(
            (t.level.as_str(), t.parent.as_str(), t.due.as_str()),
            ("task", "", "")
        );
        let v = serde_json::to_value(&t).unwrap();
        assert_eq!(
            (v["level"].as_str(), v["parent"].as_str()),
            (Some("task"), Some(""))
        );
        assert_eq!(v["due"], "");
        assert_eq!(v["children"], serde_json::json!([]));
        assert!(v["progress"].is_null());
        assert_eq!(v["shownStage"], "building");
        let t = parse_one(
            &TASK.replace("status: doing", "status: doing\nlevel: epic"),
            "",
        );
        assert_eq!((t.level.as_str(), t.bad_level.as_str()), ("task", "epic"));
        let v = serde_json::to_value(&t).unwrap();
        assert!(v.get("badLevel").is_none() && v.get("bad_level").is_none());
    }

    #[test]
    fn goals_and_features_roll_up_their_tasks() {
        let d = levels_queue(&[
            ("0001", &format!("level: goal\ndue: 2026-12-01\n{READY}")),
            ("0002", &format!("level: feature\nparent: 0001\n{READY}")),
            ("0003", &format!("level: feature\nparent: 0001\n{READY}")),
            (
                "0004",
                &format!("parent: 0002\nest-time: 1h\nest-tokens: 100k\n{DONE}"),
            ),
            (
                "0005",
                &format!("parent: 0002\nest-time: 2h\nest-tokens: 200k\n{BUILDING}"),
            ),
            ("0006", &format!("parent: 0001\nest-time: 30m\n{READY}")),
            ("0007", &format!("est-time: 5h\n{READY}")),
        ]);
        let s = levels_state(&d);
        assert!(s.problems.is_empty(), "{:?}", s.problems);
        let t = |id: &str| s.tasks.iter().find(|t| t.id == id).unwrap().clone();
        let p = |id: &str| t(id).progress.unwrap();
        assert_eq!(t("0002").children, ["0004", "0005"]);
        assert_eq!(
            p("0002"),
            Progress {
                done: 1,
                total: 2,
                est_minutes: 120,
                est_token_count: 200_000
            }
        );
        // A goal counts the tasks under its features and its own tasks.
        assert_eq!(t("0001").children, ["0002", "0003", "0006"]);
        assert_eq!(
            p("0001"),
            Progress {
                done: 1,
                total: 3,
                est_minutes: 150,
                est_token_count: 200_000
            }
        );
        assert_eq!(
            p("0003"),
            Progress {
                done: 0,
                total: 0,
                est_minutes: 0,
                est_token_count: 0
            }
        );
        // Shown stage: In progress once a task is past Ready; Ready with none started.
        assert_eq!(t("0002").shown_stage, "in-progress");
        assert_eq!(t("0001").shown_stage, "in-progress");
        assert_eq!(t("0003").shown_stage, "ready", "no tasks yet");
        // Tasks have no roll-up and show their own stage.
        assert_eq!(
            (
                t("0005").progress,
                t("0005").children.len(),
                t("0005").shown_stage.as_str()
            ),
            (None, 0, "building")
        );
        let v = serde_json::to_value(t("0001")).unwrap();
        assert_eq!(
            v["progress"],
            serde_json::json!({"done": 1, "total": 3, "estMinutes": 150, "estTokenCount": 200_000})
        );
        assert_eq!(v["children"], serde_json::json!(["0002", "0003", "0006"]));
    }

    #[test]
    fn a_roll_up_counts_test_time_with_agent_time() {
        let d = levels_queue(&[
            ("0001", &format!("level: feature\n{READY}")),
            (
                "0002",
                &format!("parent: 0001\nest-time: 2h\nest-test-time: 30m\n{READY}"),
            ),
            (
                "0003",
                &format!("parent: 0001\nest-test-time: 15m\n{READY}"),
            ),
            (
                "0004",
                &format!("parent: 0001\nest-time: 1h\nest-test-time: 1h\n{DONE}"),
            ),
        ]);
        let s = levels_state(&d);
        assert!(s.problems.is_empty(), "{:?}", s.problems);
        let p = s.tasks.iter().find(|t| t.id == "0001").unwrap();
        // Open tasks only: 2h + 30m + 15m; the done task's time isn't left.
        assert_eq!(
            p.progress.clone().unwrap(),
            Progress {
                done: 1,
                total: 3,
                est_minutes: 165,
                est_token_count: 0
            }
        );
    }

    #[test]
    fn a_parent_is_done_when_all_its_tasks_are_and_the_owners_stage_wins() {
        let d = levels_queue(&[
            ("0001", &format!("level: goal\n{READY}")),
            ("0002", &format!("level: feature\nparent: 0001\n{READY}")),
            ("0003", &format!("parent: 0002\n{DONE}")),
            ("0004", &format!("parent: 0001\n{DONE}")),
            // Untouched by the tool in Ready, though its tasks wait: Ready.
            ("0005", &format!("level: feature\n{READY}")),
            ("0006", &format!("parent: 0005\n{READY}")),
            // Still being shaped: shown as it is.
            ("0007", "level: feature\nstage: shaping\nstatus: todo"),
            ("0008", &format!("parent: 0007\n{BUILDING}")),
        ]);
        // The owner put 0009 in Your check by hand (`muster stage`): shown as set.
        let set = "---\nid: 0009\nlevel: feature\nstage: your-check\nstatus: doing\n---\n\n# Item 0009\n\n## Log\n- 2026-10-12 10:00 owner: stage ready → your-check (set)\n";
        fs::write(d.path().join("0009-x.md"), set).unwrap();
        fs::write(
            d.path().join("0010-x.md"),
            format!("---\nid: 0010\nparent: 0009\n{READY}\n---\n# Item 0010\n"),
        )
        .unwrap();
        let s = levels_state(&d);
        assert!(s.problems.is_empty(), "{:?}", s.problems);
        let shown = |id: &str| {
            s.tasks
                .iter()
                .find(|t| t.id == id)
                .unwrap()
                .shown_stage
                .clone()
        };
        assert_eq!(shown("0002"), "done");
        assert_eq!(shown("0001"), "done");
        assert_eq!(shown("0005"), "ready");
        assert_eq!(shown("0007"), "shaping");
        assert_eq!(shown("0009"), "your-check");
        // The file's stage is left as it is: only the display is derived.
        let goal = s.tasks.iter().find(|t| t.id == "0001").unwrap();
        assert_eq!(
            (goal.stage.as_str(), goal.status.as_str()),
            ("ready", "todo")
        );
    }

    #[test]
    fn a_goal_is_done_only_when_every_direct_child_is_and_an_empty_feature_is_not() {
        let d = levels_queue(&[
            ("0001", &format!("level: goal\n{READY}")),
            ("0002", &format!("level: feature\nparent: 0001\n{READY}")),
            ("0003", &format!("parent: 0002\n{DONE}")),
            // A feature with no tasks yet: not Done, so its goal isn't either.
            ("0004", &format!("level: feature\nparent: 0001\n{READY}")),
            // A goal whose only child is a feature with no tasks: Ready, not In progress.
            ("0005", &format!("level: goal\n{READY}")),
            ("0006", &format!("level: feature\nparent: 0005\n{READY}")),
        ]);
        let s = levels_state(&d);
        assert!(s.problems.is_empty(), "{:?}", s.problems);
        let t = |id: &str| s.tasks.iter().find(|t| t.id == id).unwrap().clone();
        assert_eq!(t("0002").shown_stage, "done");
        assert_eq!(t("0004").shown_stage, "ready");
        assert_eq!(
            t("0001").shown_stage,
            "in-progress",
            "a done feature beside an empty one"
        );
        // Every task under it is done, yet it isn't: the progress says 1 of 1.
        assert_eq!(t("0001").progress.map(|p| (p.done, p.total)), Some((1, 1)));
        assert_eq!(t("0005").shown_stage, "ready");
    }

    #[test]
    fn check_finds_missing_parents_wrong_levels_loops_and_late_goals() {
        let d = levels_queue(&[
            ("0001", &format!("level: goal\ndue: 2026-10-01\n{READY}")),
            ("0002", &format!("level: feature\nparent: 0001\n{READY}")),
            ("0003", &format!("parent: 0002\n{READY}")),
            // A task under a task, a feature under a feature, a goal with a parent.
            ("0004", &format!("parent: 0003\n{READY}")),
            ("0005", &format!("level: feature\nparent: 0002\n{READY}")),
            ("0006", &format!("level: goal\nparent: 0001\n{READY}")),
            ("0007", &format!("parent: 0099\n{READY}")),
            // 0008 and 0009 are each other's parent.
            ("0008", &format!("level: feature\nparent: 0009\n{READY}")),
            ("0009", &format!("level: feature\nparent: 0008\n{READY}")),
            ("0010", &format!("level: epic\n{READY}")),
            ("0011", &format!("level: goal\ndue: next week\n{READY}")),
            // Past due, but all its work is done: fine. Due later with work open: fine.
            ("0012", &format!("level: goal\ndue: 2026-10-01\n{READY}")),
            ("0013", &format!("parent: 0012\n{DONE}")),
            ("0014", &format!("level: goal\ndue: 2026-10-12\n{READY}")),
            ("0015", &format!("parent: 0014\n{READY}")),
        ]);
        let s = levels_state(&d);
        assert_eq!(
            s.problems,
            [
                "p: 0004-x.md is a task under task 0003; a task's parent is a feature or a goal",
                "p: 0005-x.md is a feature under feature 0002; a feature's parent is a goal",
                "p: 0006-x.md is a goal with a parent (0001); a goal has none",
                "p: 0007-x.md has parent 0099, which doesn't exist",
                "p: 0008-x.md is a feature under feature 0009; a feature's parent is a goal",
                "p: 0009-x.md is a feature under feature 0008; a feature's parent is a goal",
                "p: 0010-x.md has level 'epic', which isn't a level (read as task)",
                "p: 0011-x.md has due 'next week', which isn't a date (YYYY-MM-DD)",
                "p: parents go round in a loop: 0008 → 0009 → 0008",
                "p: warning: 0001-x.md is past its due date (2026-10-01) with 2 of 2 tasks open",
            ]
        );
        assert!(is_warning(s.problems.last().unwrap()));
        assert!(!is_warning(&s.problems[0]));
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
    fn captured_at_comes_from_the_first_log_line_on_the_capture_day() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("todo/0001-t.md");
        let at = |captured: &str, log: &str| {
            let text = format!(
                "---\nid: 0001\nstatus: todo\ncaptured: {captured}\n---\n\n# T\n\n## Log\n{log}"
            );
            parse_task("q", d.path(), "todo", &path, &text, 0)
                .unwrap()
                .captured_at
        };
        assert_eq!(
            at("2026-10-12", "- 2026-10-12 15:04 capture: created\n"),
            "2026-10-12 15:04"
        );
        assert_eq!(
            at("", "- 2026-10-12 15:04 capture: created\n"),
            "2026-10-12 15:04"
        );
        // Date only, another day, or not a stamp: unknown.
        assert_eq!(at("2026-10-12", "- 2026-10-12 captured\n"), "");
        assert_eq!(
            at("2026-10-11", "- 2026-10-12 15:04 capture: created\n"),
            ""
        );
        assert_eq!(at("2026-10-12", "- 2026-10-12 1x:04 capture\n"), "");
        assert_eq!(at("2026-10-12", ""), "");
    }

    #[test]
    fn done_at_is_the_last_timed_move_into_done() {
        assert_eq!(
            done_at(&[
                "- 2026-10-12 04:47 capture: created",
                "- 2026-10-12 04:55 capture: moved todo → done",
            ]),
            "2026-10-12 04:55"
        );
        assert_eq!(
            done_at(&["- 2026-10-12 18:32 wt-a: stage your-check → done (commit abc1234)"]),
            "2026-10-12 18:32"
        );
        // Date only, or never moved to done: unknown.
        assert_eq!(done_at(&["- 2026-10-12 capture: moved todo → done"]), "");
        assert_eq!(
            done_at(&["- 2026-10-12 09:00 wt-a: stage ready → building (claimed)"]),
            ""
        );
    }

    #[test]
    fn block_history_pairs_blocks_with_unblocks() {
        let lines = [
            "- 2026-10-12 capture: created",
            "- 2026-10-12 15:05 capture: moved todo → blocked (waiting on 0014)",
            "- 2026-10-12 22:02 capture: unblocked (was waiting on 0014)",
            "- 2026-10-12 22:10 wt-a: note about being blocked on nothing",
            "- 2026-10-12 23:00 wt-a: blocked on 0002 — merge",
            "- 2026-10-12 23:01 wt-a: blocked on 0002 again (ignored: already open)",
        ];
        let span = |f: &str, t: &str, kind: &str| BlockSpan {
            from: f.into(),
            to: t.into(),
            kind: kind.into(),
        };
        assert_eq!(
            block_history(&lines),
            vec![
                span("2026-10-12 15:05", "2026-10-12 22:02", ""),
                span("2026-10-12 23:00", "", "tasks")
            ]
        );
        // A date-only line reads as midnight; an unblock without a block is ignored.
        assert_eq!(
            block_history(&[
                "- 2026-10-11 capture: unblocked",
                "- 2026-10-11 capture: blocked on owner"
            ]),
            vec![span("2026-10-11 00:00", "", "you")]
        );
        // Lines muster writes since kinds (0024), and the unblocks that close them by themselves.
        assert_eq!(
            block_history(&[
                "- 2026-10-13 09:00 wt-a: blocked on permission: write to ~/x/tasks",
                "- 2026-10-13 09:30 owner: unblocked (was waiting on permission: write to ~/x/tasks)",
                "- 2026-10-13 10:00 wt-a: blocked on question: Q2",
                "- 2026-10-13 10:05 capture: unblocked: Q2 answered",
                "- 2026-10-13 11:00 wt-a: blocked on tasks: 0014, 0016",
                "- 2026-10-13 12:00 wt-b: unblocked: 0014, 0016 done",
                "- 2026-10-13 13:00 wt-a: blocked on outside: Apple review",
            ]),
            vec![
                span("2026-10-13 09:00", "2026-10-13 09:30", "permission"),
                span("2026-10-13 10:00", "2026-10-13 10:05", "question"),
                span("2026-10-13 11:00", "2026-10-13 12:00", "tasks"),
                span("2026-10-13 13:00", "", "outside"),
            ]
        );
    }

    #[test]
    fn captured_at_skips_a_first_line_that_is_a_stage_change() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("doing/0001-t.md");
        let text = "---\nid: 0001\nstatus: doing\ncaptured: 2026-10-12\n---\n\n# T\n\n## Log\n- 2026-10-12 16:59 wt-a: stage ready → building (claimed)\n";
        assert_eq!(
            parse_task("q", d.path(), "doing", &path, text, 0)
                .unwrap()
                .captured_at,
            ""
        );
    }

    #[test]
    fn depends_reads_ids_and_keeps_the_rest_for_check() {
        let v = |a: &[&str]| a.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(
            parse_depends("[0014, 0016]"),
            (v(&["0014", "0016"]), v(&[]))
        );
        assert_eq!(
            parse_depends("0014,0016 0014"),
            (v(&["0014", "0016"]), v(&[]))
        );
        assert_eq!(parse_depends(""), (v(&[]), v(&[])));
        assert_eq!(parse_depends("[]"), (v(&[]), v(&[])));
        assert_eq!(
            parse_depends("[14, 0016, owner]"),
            (v(&["0016"]), v(&["14", "owner"]))
        );
    }

    #[test]
    fn dependency_loops_finds_each_loop_once() {
        let edges = |pairs: &[(&'static str, &[&'static str])]| {
            pairs
                .iter()
                .map(|(k, v)| (*k, v.to_vec()))
                .collect::<HashMap<&str, Vec<&str>>>()
        };
        // A chain, a two-loop, a three-loop with a tail, and an edge to a missing task.
        let e = edges(&[
            ("0001", &["0002"]),
            ("0002", &["0003"]),
            ("0003", &[]),
            ("0010", &["0011"]),
            ("0011", &["0010"]),
            ("0020", &["0021"]),
            ("0021", &["0022", "0099"]),
            ("0022", &["0020"]),
            ("0023", &["0020"]),
        ]);
        assert_eq!(
            dependency_loops(&e),
            vec![
                vec!["0010".to_string(), "0011".into()],
                vec!["0020".into(), "0021".into(), "0022".into()]
            ]
        );
        assert!(dependency_loops(&edges(&[("0001", &["0002"]), ("0002", &[])])).is_empty());
    }

    #[test]
    fn a_long_chain_doesnt_overflow() {
        let ids: Vec<String> = (0..20_000).map(|i| format!("{i:04}")).collect();
        let mut e: HashMap<&str, Vec<&str>> = HashMap::new();
        for w in ids.windows(2) {
            e.insert(w[0].as_str(), vec![w[1].as_str()]);
        }
        e.insert(ids.last().unwrap().as_str(), vec![ids[0].as_str()]);
        let loops = dependency_loops(&e);
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].len(), 20_000);
    }

    #[test]
    fn check_names_bad_self_missing_and_looping_dependencies() {
        let d = tempfile::tempdir().unwrap();
        for f in STATUS_FOLDERS {
            fs::create_dir(d.path().join(f)).unwrap();
        }
        fs::write(d.path().join("README.md"), "q").unwrap();
        let put = |id: &str, deps: &str| {
            fs::write(
                d.path().join(format!("todo/{id}-t.md")),
                format!("---\nid: {id}\nstatus: todo\ndepends: {deps}\n---\n\n# T\n\n## Log\n"),
            )
            .unwrap();
        };
        put("0001", "[0002]");
        put("0002", "[0001, 0009]");
        put("0003", "[0003, soon]");
        put("0004", "[0001]");
        let s = read_state(
            &[("q".into(), d.path().to_path_buf())],
            &mut Cache::default(),
        );
        let mut p = s.problems.clone();
        p.sort();
        assert_eq!(
            p,
            vec![
                "q: 0001, 0002 depend on each other in a loop".to_string(),
                "q: todo/0002-t.md depends on 0009, which doesn't exist".into(),
                "q: todo/0003-t.md depends on itself".into(),
                "q: todo/0003-t.md's depends has \"soon\", which isn't a task id".into(),
            ]
        );
        let t4 = s.tasks.iter().find(|t| t.id == "0004").unwrap();
        assert_eq!(t4.depends, vec!["0001".to_string()]);
        assert_eq!(serde_json::to_value(t4).unwrap()["depends"][0], "0001");
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
        assert_eq!(found[0].path, q);
        assert_eq!(found[0].group, "");
    }

    fn make_queue(q: &Path) {
        fs::create_dir_all(q.join("todo")).unwrap();
        fs::write(q.join("README.md"), "q").unwrap();
    }

    /// The relative paths and groups of what `discover` found, in order.
    fn found_in(base: &Path) -> Vec<(String, String)> {
        discover(base)
            .into_iter()
            .map(|f| {
                let rel = f.path.strip_prefix(base).unwrap();
                (rel.to_string_lossy().to_string(), f.group)
            })
            .collect()
    }

    #[test]
    fn discovers_queues_one_level_deeper_and_no_further() {
        let base = tempfile::tempdir().unwrap();
        let b = base.path();
        make_queue(&b.join("proj/tasks"));
        make_queue(&b.join("persianUtils/colors/tasks"));
        make_queue(&b.join("persianUtils/fonts/tasks"));
        // A manifest instead of todo/ counts too.
        fs::create_dir_all(b.join("manifest/tasks")).unwrap();
        fs::write(b.join("manifest/tasks/README.md"), "q").unwrap();
        fs::write(b.join("manifest/tasks/tasks.json"), "{}").unwrap();
        // Not queues: a tasks/ folder without the README and todo/ (or the manifest).
        fs::create_dir_all(b.join("harness/c3-docs/tasks")).unwrap();
        fs::write(b.join("harness/c3-docs/tasks/notes.md"), "n").unwrap();
        fs::create_dir_all(b.join("persianUtils/ui/tasks/todo")).unwrap();
        fs::create_dir_all(b.join("lonely/tasks")).unwrap();
        fs::write(b.join("lonely/tasks/README.md"), "q").unwrap();
        // Two levels down is as far as it looks.
        make_queue(&b.join("a/b/c/tasks"));
        // A file where a folder would be is passed over.
        fs::write(b.join("file.txt"), "x").unwrap();
        assert_eq!(
            found_in(b),
            vec![
                ("manifest/tasks".to_string(), String::new()),
                ("persianUtils/colors/tasks".into(), "persianUtils".into()),
                ("persianUtils/fonts/tasks".into(), "persianUtils".into()),
                ("proj/tasks".into(), String::new()),
            ]
        );
    }

    #[test]
    fn discovery_skips_hidden_archived_and_build_folders() {
        let base = tempfile::tempdir().unwrap();
        let b = base.path();
        for skipped in [
            ".hidden",
            "_arch",
            "__del",
            "node_modules",
            "worktrees",
            "chit.worktrees",
            "target",
            "dist",
        ] {
            make_queue(&b.join(skipped).join("tasks"));
            make_queue(&b.join(skipped).join("inner/tasks"));
            make_queue(&b.join("group").join(skipped).join("tasks"));
        }
        make_queue(&b.join("group/kept/tasks"));
        assert_eq!(
            found_in(b),
            vec![("group/kept/tasks".to_string(), "group".to_string())]
        );
    }

    #[cfg(unix)]
    #[test]
    fn discovery_stays_inside_the_base_through_symlinks() {
        use std::os::unix::fs::symlink;
        let outside = tempfile::tempdir().unwrap();
        let o = outside.path();
        make_queue(&o.join("away/tasks"));
        make_queue(&o.join("grp/deep/tasks"));
        make_queue(&o.join("q"));
        let base = tempfile::tempdir().unwrap();
        let b = base.path();
        make_queue(&b.join("real/tasks"));
        // Out of the base: a project folder, a group folder and a tasks folder.
        symlink(o.join("away"), b.join("away")).unwrap();
        symlink(o.join("grp"), b.join("grp")).unwrap();
        fs::create_dir(b.join("proj")).unwrap();
        symlink(o.join("q"), b.join("proj/tasks")).unwrap();
        // Back to the base itself, and to a folder inside it: no loop, and no queue twice.
        symlink(b, b.join("loop")).unwrap();
        symlink(b.join("real"), b.join("zz-alias")).unwrap();
        assert_eq!(found_in(b), vec![("real/tasks".to_string(), String::new())]);
    }

    #[test]
    fn discovery_of_a_missing_base_finds_nothing() {
        let base = tempfile::tempdir().unwrap();
        assert!(discover(&base.path().join("nope")).is_empty());
    }
}
