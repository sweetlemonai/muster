//! Why a task is blocked (task 0024): the kind of block, what it says, and the sentence the card,
//! the task pane, the table, the timeline, `muster list` and INDEX.md show for it.
//!
//! A block has one of five kinds. Four are typed (`blocked-kind:` in the front matter, next to
//! `waiting_on`, which holds the detail): you, permission, question and outside. The fifth, tasks,
//! is never typed: it's derived from `depends:` (0019), so a wait on other tasks has one record. A
//! file blocked before kinds existed has no `blocked-kind:`; its kind is read from `waiting_on`
//! (`guess`) and `muster check` warns until it's blocked again with one. Decision 0003 keeps the
//! kind in the live state with the flag; until task 0005 builds that state, it's in the file.
//!
//! `sentence` is mirrored, word for word, by the window's `blockWords.ts`; 0060 gave each kind its
//! own words ("Your answer", "Your action", "Permission", "After", "Outside") and look.

use crate::queue::{stage_label, Question, Task};
use serde::Serialize;
use std::collections::HashMap;

/// Every kind, in the order the app's Block form shows them.
pub const KINDS: [&str; 5] = ["tasks", "you", "permission", "question", "outside"];
/// The kinds `blocked-kind:` may hold: all but tasks, which `depends:` holds.
pub const TYPED: [&str; 4] = ["you", "permission", "question", "outside"];
/// The most of a detail the card's short form shows, in characters.
pub const SHORT: usize = 60;

/// One task a task is blocked by: its id, stage and who's on it (the label, without the session).
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BlockedBy {
    pub id: String,
    /// One of the stages, or "" when there's no task with that id.
    pub stage: String,
    pub holder: String,
}

/// `text` without its first word when that word is `word` (any case) and what follows isn't a
/// letter or digit ("owner: x" for owner, not "owners"); the rest, with the one separator after
/// the word and the spaces around it taken off: a `:` or `,`, or a dash with a space after it.
/// Text that itself starts with a dash keeps it: "owner --verbose" is "--verbose".
fn after_word<'a>(text: &'a str, word: &str) -> Option<&'a str> {
    let head = text.get(..word.len())?;
    let rest = &text[word.len()..];
    if !head.eq_ignore_ascii_case(word) || rest.starts_with(char::is_alphanumeric) {
        return None;
    }
    let rest = rest.trim_start();
    let dash = rest
        .strip_prefix(['-', '—'])
        .filter(|r| r.is_empty() || r.starts_with(char::is_whitespace));
    let rest = rest.strip_prefix([':', ',']).or(dash).unwrap_or(rest);
    Some(rest.trim_start())
}

/// Does `text` start with a task id (four digits not followed by a letter or digit)?
fn starts_with_id(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() >= 4
        && b[..4].iter().all(u8::is_ascii_digit)
        && !text[4..].starts_with(char::is_alphanumeric)
}

/// The kind a `waiting_on` written before kinds reads as: owner → you, capture → question, a task
/// id → tasks, anything else → outside.
pub fn guess(waiting_on: &str) -> &'static str {
    let w = waiting_on.trim();
    if after_word(w, "owner").is_some() {
        "you"
    } else if after_word(w, "capture").is_some() {
        "question"
    } else if starts_with_id(w) {
        "tasks"
    } else {
        "outside"
    }
}

/// What a guessed block says under its kind: an old "owner: pick a colour" is "pick a colour" on
/// you, "capture, needs a spec" is "needs a spec" on a question; the rest as written.
pub fn guessed_detail(waiting_on: &str, kind: &str) -> String {
    let w = waiting_on.trim();
    let word = match kind {
        "you" => "owner",
        "question" => "capture",
        _ => return w.to_string(),
    };
    after_word(w, word).unwrap_or(w).to_string()
}

/// What `waiting_on` holds for a typed block: the detail, after the old words readers from before
/// kinds look for (Drover's waiting list reads "owner …" as needing the owner): `owner: <detail>`
/// for you, `capture: <detail>` for a question, the detail alone for the rest. `guessed_detail`
/// reads it back.
pub fn stored_waiting_on(kind: &str, detail: &str) -> String {
    match kind {
        "you" => format!("owner: {detail}"),
        "question" => format!("capture: {detail}"),
        _ => detail.to_string(),
    }
}

/// The kind a log line's `blocked on …` names: the kind it starts with, as muster writes it since
/// 0024 (`blocked on you: …`), else what `guess` reads from it.
pub fn line_kind(text: &str) -> &'static str {
    let t = text.trim();
    KINDS
        .into_iter()
        .find(|k| after_word(t, k).is_some())
        .unwrap_or_else(|| guess(t))
}

/// The task ids in an old `waiting_on` ("0016 (levels), after 0014": 0016 and 0014), each once.
pub fn ids_in(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in text.split(|c: char| !c.is_ascii_alphanumeric()) {
        if w.len() == 4 && w.bytes().all(|b| b.is_ascii_digit()) && !out.iter().any(|o| o == w) {
            out.push(w.to_string());
        }
    }
    out
}

/// A question's number when the detail is just that (`Q2`, `q2`), else None.
pub fn question_number(detail: &str) -> Option<u32> {
    let d = detail.trim();
    let digits = d.strip_prefix(['Q', 'q'])?;
    if digits.is_empty() || digits.len() > 6 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// `--on`'s value as (kind, detail): text that starts with a digit is task ids (and maybe a
/// reason) for tasks; otherwise the first word is the kind (in any case, as typed: `block`
/// checks it) and the rest is the detail, separators after the word taken off. `"owner: which
/// colour?"` is ("owner", "which colour?"); `"0014,0016"` is ("tasks", "0014,0016").
pub fn split_on(on: &str) -> (String, String) {
    let on = on.trim();
    if on.starts_with(|c: char| c.is_ascii_digit()) {
        return ("tasks".into(), on.to_string());
    }
    let end = on.find(|c: char| !c.is_alphanumeric()).unwrap_or(on.len());
    let (word, _) = on.split_at(end);
    let rest = after_word(on, word).unwrap_or("");
    (word.to_lowercase(), rest.to_string())
}

/// How a block looks (0060), from its kind and, for a question, the question it waits on: "answer"
/// (a question escalated to the owner and still open), "action" (blocked on you), "permission",
/// "question" (one the capture session still has), "tasks", "outside", or "" with no kind.
pub fn look(kind: &str, asked: Option<&Question>) -> &'static str {
    match kind {
        "question" if asked.is_some_and(|q| q.escalated && !q.ticked) => "answer",
        "question" => "question",
        "you" => "action",
        "permission" => "permission",
        "tasks" => "tasks",
        "outside" => "outside",
        _ => "",
    }
}

/// Does a look need the owner (your answer, your action, a permission)? Those take the needs-you
/// orange in the window; the rest, which clear without them, are neutral.
pub fn for_owner(look: &str) -> bool {
    matches!(look, "answer" | "action" | "permission")
}

/// The words a block's sentence starts with, the look's, bold in the window: "Your answer", "Your
/// action", "Permission", "Question", "After", "Outside".
pub fn lead(look: &str) -> &'static str {
    match look {
        "answer" => "Your answer",
        "action" => "Your action",
        "permission" => "Permission",
        "question" => "Question",
        "tasks" => "After",
        "outside" => "Outside",
        _ => "Blocked",
    }
}

/// `text` cut to `SHORT` characters, with "…" when it was longer.
fn cut(text: &str) -> String {
    if text.chars().count() <= SHORT {
        return text.to_string();
    }
    let kept: String = text.chars().take(SHORT).collect();
    format!("{}…", kept.trim_end())
}

/// What follows the lead: the tasks with their stage and holder ("0014 (Building, codex), 0016
/// (Ready)"; ids only in the short form), a question's number and its first line (and "waiting on
/// capture" while capture has it), or the detail as written. The short form, the card's, cuts a
/// detail at `SHORT` characters.
pub fn words(
    kind: &str,
    detail: &str,
    by: &[BlockedBy],
    asked: Option<&Question>,
    short: bool,
) -> String {
    let detail = detail.trim();
    let detail = match asked.and_then(|q| q.text.lines().next()).map(str::trim) {
        Some(q) if kind == "question" && !q.is_empty() => format!("{detail} {q}"),
        _ => detail.to_string(),
    };
    let shown = if short { cut(&detail) } else { detail };
    match look(kind, asked) {
        "tasks" if !by.is_empty() => by
            .iter()
            .map(|b| {
                let stage = stage_label(&b.stage);
                match (short, stage, b.holder.as_str()) {
                    (true, _, _) | (false, "", _) => b.id.clone(),
                    (false, s, "") => format!("{} ({s})", b.id),
                    (false, s, h) => format!("{} ({s}, {h})", b.id),
                }
            })
            .collect::<Vec<_>>()
            .join(", "),
        "question" if !short && shown.is_empty() => "waiting on capture".into(),
        "question" if !short => format!("{shown}, waiting on capture"),
        _ => shown,
    }
}

/// The sentence for a block, saying plainly what it waits for (0060): "After 0014 (Building,
/// codex), 0016 (Ready)", "Your action: set the API key", "Your answer: Q2 which colour?",
/// "Question: Q2 which colour?, waiting on capture". Just the lead when there's nothing after it.
/// `asked` is the question a question block waits on, when there is one. Pure; `blockWords.ts`
/// gives the same words.
pub fn sentence(
    kind: &str,
    detail: &str,
    by: &[BlockedBy],
    asked: Option<&Question>,
    short: bool,
) -> String {
    let look = look(kind, asked);
    let lead = lead(look);
    match words(kind, detail, by, asked, short) {
        w if look == "tasks" && !by.is_empty() => format!("{lead} {w}"),
        w if look == "tasks" && w.is_empty() => format!("{lead} other tasks"),
        w if look == "tasks" => format!("{lead} other tasks: {w}"),
        w if w.is_empty() => lead.to_string(),
        w => format!("{lead}: {w}"),
    }
}

/// The question a task's question block waits on, by the number in its detail.
pub fn asked(t: &Task) -> Option<&Question> {
    if t.blocked_kind != "question" {
        return None;
    }
    let n = question_number(&t.block_detail)?;
    t.questions.iter().find(|q| q.n == Some(n))
}

/// `sentence` for a task: "" when nothing blocks it. The tasks it's blocked by are named by their
/// keys (0033) when it has its project's prefix.
pub fn sentence_of(t: &Task, short: bool) -> String {
    if t.blocked_kind.is_empty() {
        return String::new();
    }
    let by: Vec<BlockedBy> = t
        .blocked_by
        .iter()
        .map(|b| BlockedBy {
            id: crate::key::key(&t.prefix, &b.id),
            ..b.clone()
        })
        .collect();
    sentence(&t.blocked_kind, &t.block_detail, &by, asked(t), short)
}

/// Is a task in `stage` met as a dependency (0042)? An agent's task once it's merged: work merges
/// into main after Testing passes and then waits in Your check (VISION.md), so Your check counts
/// as Done does. The owner's own task (`human`, `for: human`) only once it's Done, since nothing
/// merges for it.
pub fn met_at(stage: &str, human: bool) -> bool {
    stage == "done" || (stage == "your-check" && !human)
}

/// `met_at` for a task as it is now.
pub fn met(dep: &Task) -> bool {
    met_at(&dep.stage, dep.is_human())
}

/// Was it met once (its stage history reached a met stage), whatever it is now: merged, then sent
/// back.
fn was_met(dep: &Task) -> bool {
    dep.stage_history
        .iter()
        .any(|s| met_at(&s.to, dep.is_human()))
}

/// Is `t` under way: held in Building, or past it in Testing or Your check?
pub fn started(t: &Task) -> bool {
    (t.stage == "building" && !t.claimed_by.is_empty())
        || matches!(t.stage.as_str(), "testing" | "your-check")
}

/// Does `dep` hold back a task that depends on it, one under way (`started`) or not? Until it's
/// met, it does. Except that a dependency merged once and then sent back (`return`, an override)
/// holds back only the tasks not started yet: one already under way keeps going, with a note on
/// its log saying it went back, rather than a forced return.
pub fn holds(dep: &Task, started: bool) -> bool {
    !met(dep) && !(started && was_met(dep))
}

/// Why `dep` doesn't hold `id` back (`holds` is false): the refusal for a block on it.
pub fn nothing_to_wait_for(dep: &Task, id: &str) -> String {
    match dep.stage.as_str() {
        "done" => format!("{} is done already; there's nothing to wait for", dep.id),
        _ if met(dep) => format!(
            "{} is in Your check, merged already; there's nothing to wait for",
            dep.id
        ),
        // Nothing merges for the owner's own task: it was done.
        _ => format!(
            "{} went back after it was {}, and {id} is under way, so it doesn't wait for it again; block on outside or you to wait anyway",
            dep.id,
            if dep.is_human() { "done" } else { "merged" }
        ),
    }
}

/// Per task id, whether it holds back a task not started yet, and one under way (`holds`). Any
/// file with that id counts, as `claim` reads them.
pub type Open = HashMap<String, (bool, bool)>;

/// `Open` for a queue's tasks.
pub fn open_ids<'a>(tasks: impl IntoIterator<Item = &'a Task>) -> Open {
    let mut open = Open::new();
    for t in tasks {
        let o = open.entry(t.id.clone()).or_default();
        o.0 |= holds(t, false);
        o.1 |= holds(t, true);
    }
    open
}

/// Does the task with id `d` hold `t` back (by `open_ids`)? Never itself; a dependency that
/// doesn't exist doesn't.
pub fn holds_id(open: &Open, d: &str, t: &Task) -> bool {
    let under_way = started(t);
    d != t.id
        && open
            .get(d)
            .is_some_and(|o| if under_way { o.1 } else { o.0 })
}

/// Once every task of a queue is read: what each is blocked by. An old block on task ids lists
/// those ids; then the tasks in `depends:` that still hold it (`holds`: not merged yet; any file
/// with that id, as `claim` reads them; a dependency that doesn't exist doesn't hold it, `check`
/// names it). A task that isn't flagged blocked but has such a dependency is blocked by tasks; a
/// Done task never is.
pub fn derive(tasks: &mut [Task]) {
    let mut info: HashMap<String, (String, String)> = HashMap::new();
    let open = open_ids(tasks.iter());
    let mut latest: HashMap<String, String> = HashMap::new();
    for t in tasks.iter() {
        let holder = crate::session::split(t.holder()).0.to_string();
        info.entry(t.id.clone())
            .or_insert_with(|| (t.stage.clone(), holder));
        // The latest stamp its log shows, for a span that ends without a line (below).
        let last = t
            .log_tail
            .iter()
            .filter_map(|l| crate::queue::log_stamp(&format!("- {l}")).map(|(at, _)| at))
            .max();
        if let Some(at) = last {
            let e = latest.entry(t.id.clone()).or_insert_with(|| at.clone());
            if at > *e {
                *e = at;
            }
        }
    }
    for t in tasks.iter_mut() {
        let mut ids: Vec<String> = Vec::new();
        if t.blocked && t.blocked_kind == "tasks" {
            ids = ids_in(&t.waiting_on);
        }
        if t.stage != "done" {
            for d in &t.depends {
                if holds_id(&open, d, t) && !ids.contains(d) {
                    ids.push(d.clone());
                }
            }
        }
        t.blocked_by = ids
            .into_iter()
            .map(|id| {
                let (stage, holder) = info.get(&id).cloned().unwrap_or_default();
                BlockedBy { id, stage, holder }
            })
            .collect();
        if !t.blocked && !t.blocked_by.is_empty() {
            t.blocked_kind = "tasks".into();
        }
        // A block on tasks that cleared without a line in this log (its depends done outside
        // muster): its span ends at the latest change its depends show, not never.
        if t.blocked_kind.is_empty() {
            if let Some(span) = t
                .block_history
                .last_mut()
                .filter(|s| s.to.is_empty() && s.kind == "tasks")
            {
                let to = t
                    .depends
                    .iter()
                    .filter_map(|d| latest.get(d))
                    .max()
                    .filter(|at| **at >= span.from)
                    .cloned()
                    .unwrap_or_else(|| span.from.clone());
                span.to = to;
            }
        }
    }
    put_sent_back_first(tasks);
}

/// A task is held when it's in Building with a holder and isn't blocked (`ops::holding`).
fn held(t: &Task) -> bool {
    t.stage == "building" && t.blocked_kind.is_empty() && !t.claimed_by.is_empty()
}

/// A send-back comes first (mu-66): of the tasks one holder holds in Building, the lowest-id one
/// sent back (from Testing, or from Your check: mu-101) is `behind` for each of the others they
/// hold, and its `sent_back_by` is their `behind_by`.
fn put_sent_back_first(tasks: &mut [Task]) {
    let first: Vec<(String, String, String)> = tasks
        .iter()
        .filter(|t| held(t) && t.sent_back)
        .map(|t| (t.id.clone(), t.claimed_by.clone(), t.sent_back_by.clone()))
        .collect();
    for t in tasks.iter_mut() {
        let ahead = if held(t) {
            first
                .iter()
                .filter(|(id, by, _)| *id != t.id && crate::session::same_holder(by, &t.claimed_by))
                // A sent-back task is behind another sent-back one with a lower id only.
                .filter(|(id, _, _)| !t.sent_back || *id < t.id)
                .min_by(|a, b| a.0.cmp(&b.0))
        } else {
            None
        };
        (t.behind, t.behind_by) = ahead
            .map(|(id, _, sender)| (id.clone(), sender.clone()))
            .unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn by(id: &str, stage: &str, holder: &str) -> BlockedBy {
        BlockedBy {
            id: id.into(),
            stage: stage.into(),
            holder: holder.into(),
        }
    }

    #[test]
    fn old_waiting_on_reads_as_a_kind() {
        for (w, kind, detail) in [
            ("owner", "you", ""),
            ("Owner: pick a colour", "you", "pick a colour"),
            ("owner - pick", "you", "pick"),
            ("capture", "question", ""),
            ("capture, needs a spec", "question", "needs a spec"),
            (
                "0016 (levels), after 0014",
                "tasks",
                "0016 (levels), after 0014",
            ),
            ("0012", "tasks", "0012"),
            ("owners", "outside", "owners"),
            ("00123 x", "outside", "00123 x"),
            ("Apple review", "outside", "Apple review"),
            ("", "outside", ""),
        ] {
            assert_eq!(guess(w), kind, "{w:?}");
            assert_eq!(guessed_detail(w, kind), detail, "{w:?}");
        }
        assert_eq!(
            ids_in("0016 (levels), after 0014, 0016"),
            vec!["0016", "0014"]
        );
        assert!(ids_in("owner").is_empty());
    }

    #[test]
    fn block_lines_name_their_kind() {
        for (text, kind) in [
            ("you: set the key", "you"),
            ("permission: write to ~/x", "permission"),
            ("question: Q2", "question"),
            ("outside: Apple review", "outside"),
            ("tasks: 0014, 0016", "tasks"),
            ("owner: which colour?", "you"),
            ("capture", "question"),
            ("0002 — merge", "tasks"),
            ("the release", "outside"),
        ] {
            assert_eq!(line_kind(text), kind, "{text:?}");
        }
    }

    #[test]
    fn on_splits_into_kind_and_detail() {
        let s = |on: &str| {
            let (k, d) = split_on(on);
            (k, d)
        };
        assert_eq!(s("you"), ("you".into(), "".into()));
        assert_eq!(
            s("Owner: which colour?"),
            ("owner".into(), "which colour?".into())
        );
        assert_eq!(
            s("permission write to ~/x"),
            ("permission".into(), "write to ~/x".into())
        );
        assert_eq!(s("0014,0016"), ("tasks".into(), "0014,0016".into()));
        assert_eq!(s(" 0012 the API "), ("tasks".into(), "0012 the API".into()));
        assert_eq!(s("someday"), ("someday".into(), "".into()));
        assert_eq!(s("ünïcode: x"), ("ünïcode".into(), "x".into()));
        // Only the one separator after the word goes: text that starts with a dash keeps it.
        assert_eq!(s("owner: --it's ✓"), ("owner".into(), "--it's ✓".into()));
        assert_eq!(s("you - -1 votes"), ("you".into(), "-1 votes".into()));
        assert_eq!(
            s("permission --it's"),
            ("permission".into(), "--it's".into())
        );
        assert_eq!(
            s("outside — the release"),
            ("outside".into(), "the release".into())
        );
        assert_eq!(s(""), ("".into(), "".into()));
        assert_eq!(question_number("Q2"), Some(2));
        assert_eq!(question_number(" q12 "), Some(12));
        for bad in ["Q", "Q2a", "2", "Q 2", "Q1234567", "which port"] {
            assert_eq!(question_number(bad), None, "{bad:?}");
        }
    }

    /// The question `n` (Q2), escalated to the owner or not, its text's first line `text`.
    fn asked(text: &str, escalated: bool) -> Question {
        Question {
            n: Some(2),
            ticked: false,
            asker: "wt-a".into(),
            text: format!("{text}\n**Escalated to owner** by capture."),
            escalated,
            owner_answered: false,
            choices: None,
        }
    }

    /// Each kind has its own look (0060): the three the owner acts on, and the three they can't.
    #[test]
    fn each_kind_has_its_look() {
        let mut answered = asked("which colour?", true);
        assert_eq!(look("question", Some(&answered)), "answer");
        answered.ticked = true;
        assert_eq!(look("question", Some(&answered)), "question");
        assert_eq!(
            look("question", Some(&asked("which colour?", false))),
            "question"
        );
        assert_eq!(look("question", None), "question");
        assert_eq!(look("you", None), "action");
        assert_eq!(look("permission", None), "permission");
        assert_eq!(look("tasks", None), "tasks");
        assert_eq!(look("outside", None), "outside");
        assert_eq!(look("", None), "");
        for (l, owner) in [
            ("answer", true),
            ("action", true),
            ("permission", true),
            ("question", false),
            ("tasks", false),
            ("outside", false),
            ("", false),
        ] {
            assert_eq!(for_owner(l), owner, "{l}");
        }
    }

    /// The same cases as blockWords.test.ts: the window and the core say the same words.
    #[test]
    fn the_sentence_says_plainly_what_it_waits_for() {
        let tasks = [by("0014", "building", "codex"), by("0016", "ready", "")];
        assert_eq!(
            sentence("tasks", "", &tasks, None, false),
            "After 0014 (Building, codex), 0016 (Ready)"
        );
        assert_eq!(
            sentence("tasks", "", &tasks, None, true),
            "After 0014, 0016"
        );
        assert_eq!(
            sentence("tasks", "", &[by("0099", "", "")], None, false),
            "After 0099"
        );
        assert_eq!(sentence("tasks", "", &[], None, false), "After other tasks");
        assert_eq!(
            sentence("you", "set the API key in the Keychain", &[], None, false),
            "Your action: set the API key in the Keychain"
        );
        assert_eq!(
            sentence(
                "permission",
                "write to ~/playground/x/tasks (sandbox), grant in the layer's write list",
                &[],
                None,
                false
            ),
            "Permission: write to ~/playground/x/tasks (sandbox), grant in the layer's write list"
        );
        // A question escalated to the owner waits on their answer, and says which.
        let theirs = asked("which colour for the button?", true);
        assert_eq!(
            sentence("question", "Q2", &[], Some(&theirs), false),
            "Your answer: Q2 which colour for the button?"
        );
        assert_eq!(
            sentence("question", "Q2", &[], Some(&theirs), true),
            "Your answer: Q2 which colour for the button?"
        );
        // One the capture session still has isn't theirs yet.
        let capture = asked("which colour for the button?", false);
        assert_eq!(
            sentence("question", "Q2", &[], Some(&capture), false),
            "Question: Q2 which colour for the button?, waiting on capture"
        );
        assert_eq!(
            sentence("question", "Q2", &[], Some(&capture), true),
            "Question: Q2 which colour for the button?"
        );
        assert_eq!(
            sentence("question", "Q2", &[], None, false),
            "Question: Q2, waiting on capture"
        );
        assert_eq!(sentence("question", "Q2", &[], None, true), "Question: Q2");
        assert_eq!(
            sentence("question", "", &[], None, false),
            "Question: waiting on capture"
        );
        assert_eq!(
            sentence("outside", "Apple review", &[], None, false),
            "Outside: Apple review"
        );
        assert_eq!(sentence("you", "", &[], None, false), "Your action");
        assert_eq!(sentence("", "x", &[], None, false), "Blocked: x");
        let long = "é".repeat(70);
        assert_eq!(
            sentence("outside", &long, &[], None, true),
            format!("Outside: {}…", "é".repeat(60))
        );
        assert_eq!(
            sentence("outside", &long, &[], None, false),
            format!("Outside: {long}")
        );
        let wordy = asked(&"é".repeat(70), true);
        assert_eq!(
            sentence("question", "Q2", &[], Some(&wordy), true),
            format!("Your answer: Q2 {}…", "é".repeat(57))
        );
    }

    // ---- Blocking through muster's rules (ops), on a throwaway queue ----------------------------

    use crate::ops::{
        block_as, block_task, claim_task, next_task, pass_task, reclaim, reclaim_task, return_task,
        set_field, set_stage, unblock_task, PassOpts,
    };
    use crate::queue::{is_warning, read_state_at, Cache, State, STATUS_FOLDERS};
    use std::fs;
    use std::path::Path;

    const T: &str = "2026-10-13 09:00";
    const READY: &str = "stage: ready\nstatus: todo\nclaimed-by:\nheartbeat:\nwaiting_on:";
    const QUESTIONS: &str = "## Questions\n- [ ] **Q1 (wt-a, session aaaa1111, 2026-10-13 08:00):** Which port?\n- [x] **Q2 (wt-a, session aaaa1111, 2026-10-13 08:01):** Which host?\n  **A (capture, 2026-10-13 08:30):** localhost\n\n";

    fn queue() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for f in STATUS_FOLDERS {
            fs::create_dir(d.path().join(f)).unwrap();
        }
        // Its prefix stored (0033): the log names other tasks by their keys, bk-1.
        fs::write(d.path().join("README.md"), "---\nprefix: bk\n---\nq").unwrap();
        d
    }
    /// Task `id` in `folder` with these front-matter lines and sections above the Log.
    fn put(d: &Path, folder: &str, id: &str, fm: &str, sections: &str) {
        fs::write(
            d.join(folder).join(format!("{id}-t.md")),
            format!("---\nid: {id}\ntype: code\npriority: normal\n{fm}\n---\n\n# Task {id}\n\n{sections}## Log\n- 2026-10-12 09:00 capture: created\n"),
        )
        .unwrap();
    }
    fn state(d: &Path) -> State {
        read_state_at(&[("p".into(), d.to_path_buf())], &mut Cache::default(), T)
    }
    fn task(d: &Path, id: &str) -> Task {
        state(d).tasks.into_iter().find(|t| t.id == id).unwrap()
    }
    fn last_log(d: &Path, id: &str) -> String {
        task(d, id).log_tail.last().cloned().unwrap_or_default()
    }

    #[test]
    fn old_blocks_read_as_a_kind_and_check_warns_until_blocked_again() {
        let d = queue();
        let p = d.path();
        put(p, "todo", "0001", READY, "");
        for (id, on) in [
            ("0002", "owner: pick a colour"),
            ("0003", "capture"),
            ("0004", "0001 (levels), after 0099"),
            ("0005", "Apple review"),
        ] {
            let fm = format!("stage: ready\nstatus: blocked\nclaimed-by:\nwaiting_on: {on}");
            put(p, "blocked", id, &fm, "");
        }
        let s = state(p);
        let kind = |id: &str| {
            let t = s.tasks.iter().find(|t| t.id == id).unwrap();
            (
                t.blocked_kind.as_str(),
                t.block_detail.as_str(),
                t.kind_guessed,
                t.waiting_for_owner,
            )
        };
        assert_eq!(kind("0002"), ("you", "pick a colour", true, true));
        assert_eq!(kind("0003"), ("question", "", true, false));
        assert_eq!(
            kind("0004"),
            ("tasks", "0001 (levels), after 0099", true, false)
        );
        assert_eq!(kind("0005"), ("outside", "Apple review", true, false));
        let t4 = s.tasks.iter().find(|t| t.id == "0004").unwrap();
        assert_eq!(sentence_of(t4, false), "After 0001 (Ready), 0099");
        assert_eq!(serde_json::to_value(t4).unwrap()["blockedKind"], "tasks");
        // Warnings: shown, but the check doesn't fail on them.
        assert!(s.problems.iter().all(|p| is_warning(p)), "{:?}", s.problems);
        assert_eq!(s.problems.len(), 4, "{:?}", s.problems);
        assert!(
            s.problems.iter().any(|w| w.contains(
                "blocked/0002-t.md is blocked on \"owner: pick a colour\" without a kind, read as you; block it again with one to confirm (muster block 0002 --on you \"…\")"
            )),
            "{:?}",
            s.problems
        );
        assert!(
            s.problems
                .iter()
                .any(|w| w.contains("(muster block 0004 --on 0001)")),
            "{:?}",
            s.problems
        );
        // Confirmed by a block with a kind: the warning goes.
        block_task(p, "0002", "you: pick a colour", "capture", T).unwrap();
        block_task(p, "0005", "outside: Apple review", "capture", T).unwrap();
        // An old block on task ids becomes depends: the flag goes, the block on tasks shows.
        assert_eq!(
            block_task(p, "0004", "0001", "capture", T).unwrap(),
            "todo/0004-t.md"
        );
        let t4 = task(p, "0004");
        assert_eq!(
            (t4.blocked, t4.status.as_str(), t4.blocked_kind.as_str()),
            (false, "todo", "tasks")
        );
        assert_eq!(t4.depends, ["0001"]);
        let s = state(p);
        assert_eq!(s.problems.len(), 1, "{:?}", s.problems);
        assert!(s.problems[0].contains("0003-t.md"), "{:?}", s.problems);
    }

    #[test]
    fn each_typed_kind_is_stored_next_to_what_it_waits_on() {
        let d = queue();
        let p = d.path();
        put(p, "todo", "0001", READY, QUESTIONS);
        // `waiting_on` keeps the old words Drover reads (owner …, capture …) for you and question;
        // `blocked-kind:` is the truth, and the detail is read back without them.
        for (kind, detail, stored, words, needs_you) in [
            (
                "you",
                "set the API key in the Keychain",
                "owner: set the API key in the Keychain",
                "Your action: set the API key in the Keychain",
                true,
            ),
            (
                "permission",
                "write to ~/x/tasks (sandbox), grant in the layer's write list",
                "write to ~/x/tasks (sandbox), grant in the layer's write list",
                "Permission: write to ~/x/tasks (sandbox), grant in the layer's write list",
                true,
            ),
            (
                "question",
                "q1",
                "capture: Q1",
                "Question: Q1 Which port?, waiting on capture",
                false,
            ),
            (
                "outside",
                "Apple review ✓",
                "Apple review ✓",
                "Outside: Apple review ✓",
                false,
            ),
        ] {
            block_as(p, "0001", kind, detail, "wt-a", T).unwrap();
            let t = task(p, "0001");
            assert_eq!(t.blocked_kind, kind);
            assert_eq!(t.waiting_on, stored);
            assert_eq!(sentence_of(&t, false), words);
            assert_eq!(t.waiting_for_owner, needs_you, "{kind}");
            assert!(!t.kind_guessed);
            let file = fs::read_to_string(p.join("blocked/0001-t.md")).unwrap();
            let pair = format!("\nwaiting_on: {stored}\nblocked-kind: {kind}\n");
            assert!(file.contains(&pair), "{file}");
            let line = format!("wt-a: blocked on {kind}: {}", t.block_detail);
            assert!(last_log(p, "0001").ends_with(&line), "{line}");
            // The same block again is refused, as stored.
            assert!(block_as(p, "0001", kind, detail, "wt-a", T)
                .unwrap_err()
                .contains("already blocked"));
        }
        unblock_task(p, "0001", "owner", T).unwrap();
        assert!(last_log(p, "0001")
            .ends_with("owner: unblocked (was waiting on outside: Apple review ✓)"));
        block_as(p, "0001", "you", "pick", "wt-a", T).unwrap();
        unblock_task(p, "0001", "owner", T).unwrap();
        assert!(last_log(p, "0001").ends_with("owner: unblocked (was waiting on you: pick)"));
        assert!(state(p).problems.is_empty(), "{:?}", state(p).problems);
        // The timeline's span carries the kind of the block that opened it.
        assert_eq!(task(p, "0001").block_history[0].kind, "you");
    }

    #[test]
    fn typed_blocks_read_with_or_without_the_old_words() {
        let d = queue();
        let p = d.path();
        for (id, waiting_on, kind, detail) in [
            ("0001", "owner: pick a colour", "you", "pick a colour"),
            ("0002", "pick a colour", "you", "pick a colour"),
            ("0003", "capture: Q2", "question", "Q2"),
            ("0004", "Q2", "question", "Q2"),
            (
                "0005",
                "owner: of the repo",
                "outside",
                "owner: of the repo",
            ),
        ] {
            let fm = format!(
                "stage: ready\nstatus: blocked\nclaimed-by:\nwaiting_on: {waiting_on}\nblocked-kind: {kind}"
            );
            put(p, "blocked", id, &fm, "");
            let t = task(p, id);
            assert_eq!(
                (
                    t.blocked_kind.as_str(),
                    t.block_detail.as_str(),
                    t.kind_guessed
                ),
                (kind, detail, false),
                "{id}"
            );
        }
        assert!(state(p).problems.is_empty(), "{:?}", state(p).problems);
    }

    #[test]
    fn refusals_say_what_is_wrong_and_write_nothing() {
        let d = queue();
        let p = d.path();
        put(p, "todo", "0001", READY, QUESTIONS);
        let done = "stage: done\nstatus: done\nclaimed-by:\nwaiting_on:";
        put(p, "done", "0002", done, "");
        put(p, "todo", "0003", &format!("{READY}\ndepends: [0001]"), "");
        let err =
            |kind: &str, detail: &str| block_as(p, "0001", kind, detail, "wt-a", T).unwrap_err();
        assert!(err("someday", "x").contains("\"someday\" isn't a kind of block"));
        assert!(err("", "x").contains("say what it's blocked by"));
        assert_eq!(
            err("you", " "),
            "a block on you needs what you have to do or decide"
        );
        assert_eq!(
            err("permission", ""),
            "a block on permission needs what access it lacks and where to grant it"
        );
        assert_eq!(
            err("outside", ""),
            "a block on outside needs what it waits on outside"
        );
        assert_eq!(
            err("capture", ""),
            "a block on question needs the question's number, like Q2"
        );
        assert!(err("question", "which port?").contains("the question's number, like Q2"));
        assert_eq!(
            err("question", "Q9"),
            "0001 has no Q9; ask it first (muster ask)"
        );
        assert_eq!(err("question", "Q2"), "0001's Q2 is answered already");
        assert!(err("you", &"x".repeat(201)).contains("longer than 200"));
        assert!(err("you", "x: stage ready → done").contains("would read as a stage change"));
        assert!(err("tasks", "").contains("needs their ids"));
        assert!(err("tasks", "soon").contains("needs their ids"));
        assert!(err("tasks", "14").contains("takes task ids like 0014, got \"14\""));
        assert_eq!(err("tasks", "0099"), "there's no task 0099 in this queue");
        assert_eq!(err("tasks", "0001"), "0001 can't depend on itself");
        assert_eq!(
            err("tasks", "0002"),
            "0002 is done already; there's nothing to wait for"
        );
        assert_eq!(
            err("tasks", "0003"),
            "that would make a loop: 0001, 0003 would wait on each other"
        );
        assert_eq!(
            block_as(p, "0003", "tasks", "0001", "wt-a", T).unwrap_err(),
            "0003 already waits on 0001"
        );
        let t = task(p, "0001");
        assert!(!t.blocked && t.depends.is_empty() && t.blocked_kind.is_empty());
        assert_eq!(t.log_tail.len(), 1);
    }

    #[test]
    fn a_block_on_tasks_adds_to_depends_and_sets_no_flag() {
        let d = queue();
        let p = d.path();
        for id in ["0001", "0002"] {
            put(p, "todo", id, READY, "");
        }
        let held = "stage: building\nstatus: doing\nclaimed-by: codex (session aaaa1111)\nheartbeat: 2026-10-13 08:00\nwaiting_on:";
        put(p, "doing", "0003", held, "");
        put(p, "todo", "0004", &format!("{READY}\ndepends: [0002]"), "");
        assert_eq!(
            block_task(p, "0004", "0003, 0001 after the API lands", "wt-a", T).unwrap(),
            "todo/0004-t.md"
        );
        let t = task(p, "0004");
        assert_eq!(t.depends, ["0002", "0003", "0001"]);
        assert_eq!(
            (t.blocked, t.status.as_str(), t.blocked_kind.as_str()),
            (false, "todo", "tasks")
        );
        assert_eq!(
            sentence_of(&t, false),
            "After 0002 (Ready), 0003 (Building, codex), 0001 (Ready)"
        );
        assert_eq!(sentence_of(&t, true), "After 0002, 0003, 0001");
        assert!(last_log(p, "0004")
            .ends_with("wt-a: blocked on tasks: bk-3, bk-1 (after the API lands)"));
        let file = fs::read_to_string(p.join("todo/0004-t.md")).unwrap();
        assert!(file.contains("\ndepends: [0002, 0003, 0001]\n"), "{file}");
        assert!(!file.contains("blocked-kind"), "{file}");
        // It isn't flagged, so there's nothing to unblock: it clears by itself.
        assert!(unblock_task(p, "0004", "owner", T)
            .unwrap_err()
            .contains("is blocked by tasks 0002, 0003, 0001: that clears by itself"));
        assert!(state(p).problems.is_empty(), "{:?}", state(p).problems);
    }

    #[test]
    fn a_holder_blocked_by_tasks_has_let_go_of_its_claim() {
        let d = queue();
        let p = d.path();
        for id in ["0001", "0002", "0003"] {
            put(p, "todo", id, READY, "");
        }
        claim_task(p, "0001", "wt-a", T).unwrap();
        assert!(
            claim_task(p, "0002", "wt-a", T).is_err(),
            "one claim per holder"
        );
        block_task(p, "0001", "0003", "wt-a", T).unwrap();
        // Blocked by tasks is waiting: wt-a may take another, and 0001 is never quiet.
        claim_task(p, "0002", "wt-a", T).unwrap();
        let (later, now) = ("2026-10-14 09:00", crate::queue::now_secs());
        let r = reclaim(p, 60, "capture", later, now).unwrap();
        let ids: Vec<&str> = r.released.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, ["0002"], "{r:?}");
        assert!(reclaim_task(p, "0001", 60, "owner", later, now)
            .unwrap_err()
            .contains("0001 is blocked by tasks 0003; it's waiting, not quiet"));
        let s = read_state_at(
            &[("p".into(), p.to_path_buf())],
            &mut Cache::default(),
            later,
        );
        assert!(s.problems.is_empty(), "{:?}", s.problems);
    }

    #[test]
    fn a_question_block_clears_when_it_is_answered() {
        let d = queue();
        let p = d.path();
        put(p, "todo", "0001", READY, QUESTIONS);
        claim_task(p, "0001", "wt-a", T).unwrap();
        block_task(p, "0001", "question Q1", "wt-a", T).unwrap();
        assert_eq!(task(p, "0001").folder, "blocked");
        // Another question's answer doesn't clear it.
        crate::queue::ask(p, "0001", "Which user?", "wt-a", "aaaa1111", T).unwrap();
        crate::queue::answer(p, "0001", 3, "root", "capture", T).unwrap();
        assert!(task(p, "0001").blocked);
        let at = "2026-10-13 09:30";
        crate::queue::answer(p, "0001", 1, "8080", "capture", at).unwrap();
        let t = task(p, "0001");
        assert_eq!(
            (
                t.blocked,
                t.blocked_kind.as_str(),
                t.waiting_on.as_str(),
                t.folder.as_str(),
                t.stage.as_str()
            ),
            (false, "", "", "doing", "building")
        );
        assert!(
            t.log_tail
                .last()
                .unwrap()
                .ends_with("capture: unblocked: Q1 answered"),
            "{:?}",
            t.log_tail
        );
        assert_eq!(t.block_history.last().unwrap().to, at);
        // The answer's mail names where the task is now.
        let mail = fs::read_to_string(p.join("mail/aaaa1111/0001-Q1.md")).unwrap();
        assert!(mail.contains("doing/0001-t.md"), "{mail}");
        assert!(state(p).problems.is_empty(), "{:?}", state(p).problems);
    }

    #[test]
    fn an_answer_that_cannot_unblock_says_why_and_it_stays_blocked() {
        let d = queue();
        let p = d.path();
        put(p, "todo", "0001", READY, QUESTIONS);
        put(p, "todo", "0002", READY, "");
        claim_task(p, "0001", "wt-a", T).unwrap();
        block_task(p, "0001", "question Q1", "wt-a", T).unwrap();
        claim_task(p, "0002", "wt-a", T).unwrap();
        crate::queue::answer(p, "0001", 1, "8080", "capture", T).unwrap();
        let t = task(p, "0001");
        assert!(t.blocked && t.questions[0].ticked);
        assert!(
            t.log_tail.last().unwrap().ends_with(
                "capture: Q1 answered, still blocked: wt-a already holds bk-2 in Building; one claim per holder, so they pass, return or block it first"
            ),
            "{:?}",
            t.log_tail
        );
    }

    #[test]
    fn you_permission_and_outside_stay_until_someone_unblocks() {
        let d = queue();
        let p = d.path();
        put(p, "todo", "0001", READY, QUESTIONS);
        for on in [
            "you: pick",
            "permission: write to ~/x",
            "outside: Apple review",
        ] {
            block_task(p, "0001", on, "wt-a", T).unwrap();
            crate::queue::ask(p, "0001", on, "wt-a", "aaaa1111", T).unwrap();
            let n = task(p, "0001").questions.len() as u32;
            crate::queue::answer(p, "0001", n, "ok", "capture", T).unwrap();
            assert!(task(p, "0001").blocked, "{on}");
        }
        unblock_task(p, "0001", "owner", T).unwrap();
        assert!(last_log(p, "0001")
            .ends_with("owner: unblocked (was waiting on outside: Apple review)"));
    }

    #[test]
    fn a_block_on_tasks_clears_when_the_last_one_is_done() {
        let d = queue();
        let p = d.path();
        for id in ["0001", "0002", "0003", "0004"] {
            put(p, "todo", id, READY, "");
        }
        block_task(p, "0003", "0001,0002", "wt-a", T).unwrap();
        // Waiting on 0001 and flagged on you too: the flag stays, and no line is written.
        block_task(p, "0004", "0001", "wt-a", T).unwrap();
        block_task(p, "0004", "you: sign off", "wt-a", T).unwrap();
        set_stage(p, "0001", "done", None, "owner", "2026-10-13 10:00").unwrap();
        let t = task(p, "0003");
        assert_eq!(sentence_of(&t, false), "After 0002 (Ready)");
        assert!(last_log(p, "0003").ends_with("wt-a: blocked on tasks: bk-1, bk-2"));
        pass_task(p, "0002", "wt-b", T, &PassOpts::default()).unwrap();
        set_stage(p, "0002", "done", None, "owner", "2026-10-13 11:00").unwrap();
        let t = task(p, "0003");
        assert_eq!((t.blocked_kind.as_str(), t.blocked_by.len()), ("", 0));
        assert!(
            t.log_tail
                .last()
                .unwrap()
                .ends_with("owner: unblocked: bk-1, bk-2 done"),
            "{:?}",
            t.log_tail
        );
        let span = t.block_history.last().unwrap();
        assert_eq!(
            (span.from.as_str(), span.to.as_str(), span.kind.as_str()),
            (T, "2026-10-13 11:00", "tasks")
        );
        let t4 = task(p, "0004");
        assert_eq!((t4.blocked, t4.blocked_kind.as_str()), (true, "you"));
        assert!(last_log(p, "0004").ends_with("wt-a: blocked on you: sign off"));
        // Back out of Done: blocked by it again, derived, with nothing written.
        set_stage(p, "0002", "testing", None, "owner", T).unwrap();
        assert_eq!(task(p, "0003").blocked_kind, "tasks");
    }

    #[test]
    fn a_freed_task_whose_holder_holds_another_goes_back_to_ready() {
        let d = queue();
        let p = d.path();
        for id in ["0001", "0002", "0003"] {
            put(p, "todo", id, READY, "");
        }
        claim_task(p, "0001", "wt-a", T).unwrap();
        block_task(p, "0001", "0003", "wt-a", T).unwrap();
        claim_task(p, "0002", "wt-a", T).unwrap();
        set_stage(p, "0003", "done", None, "owner", "2026-10-13 10:00").unwrap();
        let t = task(p, "0001");
        assert_eq!(
            (
                t.stage.as_str(),
                t.claimed_by.as_str(),
                t.blocked_kind.as_str()
            ),
            ("ready", "", "")
        );
        let n = t.log_tail.len();
        assert!(
            t.log_tail[n - 2].ends_with("owner: unblocked: bk-3 done"),
            "{:?}",
            t.log_tail
        );
        assert!(
            t.log_tail[n - 1]
                .ends_with("owner: stage building → ready (released: wt-a already holds bk-2)"),
            "{:?}",
            t.log_tail
        );
        assert_eq!(t.stage_history.last().unwrap().to, "ready");
        assert_eq!(task(p, "0002").claimed_by, "wt-a");
        assert!(state(p).problems.is_empty(), "{:?}", state(p).problems);
        // The owner may hold several: nothing is released.
        for id in ["0004", "0005", "0006"] {
            put(p, "todo", id, READY, "");
        }
        claim_task(p, "0004", "owner", T).unwrap();
        block_task(p, "0004", "0006", "owner", T).unwrap();
        claim_task(p, "0005", "owner", T).unwrap();
        set_stage(p, "0006", "done", None, "owner", T).unwrap();
        assert_eq!(task(p, "0004").stage, "building");
    }

    #[test]
    fn a_freed_claim_starts_its_lease_again() {
        let d = queue();
        let p = d.path();
        for id in ["0001", "0002"] {
            put(p, "todo", id, READY, "");
        }
        claim_task(p, "0001", "wt-a", T).unwrap();
        block_task(p, "0001", "0002", "wt-a", T).unwrap();
        // Done 25 h later: the time spent waiting isn't time gone quiet.
        let later = "2026-10-14 10:00";
        set_stage(p, "0002", "done", None, "owner", later).unwrap();
        let t = task(p, "0001");
        assert_eq!(
            (t.stage.as_str(), t.heartbeat.as_str()),
            ("building", later)
        );
        let r = reclaim(p, 720, "capture", later, crate::queue::now_secs()).unwrap();
        assert!(r.released.is_empty(), "{r:?}");
        let s = read_state_at(
            &[("p".into(), p.to_path_buf())],
            &mut Cache::default(),
            later,
        );
        assert!(s.problems.is_empty(), "{:?}", s.problems);
    }

    #[test]
    fn a_block_on_tasks_cleared_another_way_closes_its_span() {
        let d = queue();
        let p = d.path();
        for id in ["0001", "0002", "0003"] {
            put(p, "todo", id, READY, "");
        }
        block_task(p, "0002", "0001", "wt-a", T).unwrap();
        block_task(p, "0003", "0001", "wt-a", T).unwrap();
        // Its depends emptied: logged, so the span ends there.
        set_field(p, "0002", "depends", "", "owner", "2026-10-13 10:00").unwrap();
        let t = task(p, "0002");
        assert!(t
            .log_tail
            .last()
            .unwrap()
            .ends_with("owner: unblocked: no open depends left"));
        let span = t.block_history.last().unwrap();
        assert_eq!(
            (span.to.as_str(), span.kind.as_str()),
            ("2026-10-13 10:00", "tasks")
        );
        // A depends that still waits on something logs nothing.
        block_task(p, "0002", "0001", "wt-a", T).unwrap();
        set_field(p, "0002", "depends", "0001,0003", "owner", T).unwrap();
        assert!(task(p, "0002")
            .log_tail
            .last()
            .unwrap()
            .ends_with("depends set to [bk-1, bk-3]"));
        // 0001 done by hand, outside muster: 0003's span closes at the last change it shows.
        let f = p.join("todo/0001-t.md");
        let text = fs::read_to_string(&f)
            .unwrap()
            .replace("stage: ready\nstatus: todo", "stage: done\nstatus: done")
            + "- 2026-10-13 12:00 owner: done by hand\n";
        fs::remove_file(&f).unwrap();
        fs::write(p.join("done/0001-t.md"), text).unwrap();
        let t = task(p, "0003");
        assert_eq!(t.blocked_kind, "");
        let span = t.block_history.last().unwrap();
        assert_eq!(
            (span.from.as_str(), span.to.as_str()),
            (T, "2026-10-13 12:00")
        );
    }

    #[test]
    fn an_old_block_on_tasks_that_are_done_or_gone_clears_when_blocked_again() {
        let d = queue();
        let p = d.path();
        put(p, "todo", "0001", READY, "");
        put(
            p,
            "done",
            "0002",
            "stage: done\nstatus: done\nclaimed-by:\nwaiting_on:",
            "",
        );
        let old =
            |on: &str| format!("stage: ready\nstatus: blocked\nclaimed-by:\nwaiting_on: {on}");
        put(p, "blocked", "0005", &old("0002"), "");
        put(p, "blocked", "0006", &old("0001, 0002, 0099"), "");
        let s = state(p);
        let warned = |id: &str| {
            s.problems
                .iter()
                .find(|w| w.contains(&format!("{id}-t.md")))
                .cloned()
                .unwrap()
        };
        assert!(
            warned("0005").ends_with("none of them is open now: muster unblock 0005"),
            "{}",
            warned("0005")
        );
        assert!(
            warned("0006").ends_with("(muster block 0006 --on 0001)"),
            "{}",
            warned("0006")
        );
        // Blocked again on its ids: the done and missing ones are dropped, the flag clears.
        block_task(p, "0005", "0002", "capture", T).unwrap();
        let t = task(p, "0005");
        assert_eq!(
            (
                t.blocked,
                t.blocked_kind.as_str(),
                t.depends.len(),
                t.status.as_str()
            ),
            (false, "", 0, "todo")
        );
        assert!(
            t.log_tail
                .last()
                .unwrap()
                .ends_with("capture: unblocked (was waiting on 0002)"),
            "{:?}",
            t.log_tail
        );
        block_task(p, "0006", "0001, 0002, 0099", "capture", T).unwrap();
        let t = task(p, "0006");
        assert_eq!(
            (t.blocked, t.blocked_kind.as_str(), t.depends.clone()),
            (false, "tasks", vec!["0001".to_string()])
        );
        assert!(
            t.log_tail
                .last()
                .unwrap()
                .ends_with("capture: blocked on tasks: bk-1"),
            "{:?}",
            t.log_tail
        );
        assert!(state(p).problems.is_empty(), "{:?}", state(p).problems);
    }

    // ---- Merged is met (0042): a dependency in Your check counts, as Done does -----------------

    const TESTED: &str = "## Acceptance criteria\n- [x] AC1: works\n\n";
    const TESTING: &str = "stage: testing\nstatus: doing\nclaimed-by:\nheartbeat:\nwaiting_on:";

    #[test]
    fn a_dependency_in_your_check_is_met_and_one_in_testing_is_not() {
        let d = queue();
        let p = d.path();
        put(p, "doing", "0001", TESTING, TESTED);
        put(p, "todo", "0002", &format!("{READY}\ndepends: [0001]"), "");
        put(p, "todo", "0003", READY, "");
        put(p, "todo", "0004", &format!("{READY}\ndepends: [0003]"), "");
        // 0001 in Testing isn't merged yet: 0002 waits, and next passes it by.
        let e = claim_task(p, "0002", "wt-a", T).unwrap_err();
        assert_eq!(
            e,
            "0002 depends on 0001, not merged yet (an agent's task counts once it's in Your check, the owner's own once it's Done); pick another (muster next)"
        );
        assert_eq!(sentence_of(&task(p, "0002"), false), "After 0001 (Testing)");
        assert_eq!(next_task(p, "wt-a", T).unwrap().unwrap().id, "0003");
        // Merged after Testing, it waits in Your check: that counts.
        pass_task(p, "0001", "qa", T, &PassOpts::default()).unwrap();
        let t = task(p, "0002");
        assert_eq!((t.blocked_kind.as_str(), t.blocked_by.len()), ("", 0));
        assert_eq!(next_task(p, "wt-b", T).unwrap().unwrap().id, "0002");
        // A block on a task in Your check has nothing to wait for; check agrees nothing holds it.
        assert_eq!(
            block_task(p, "0004", "0001", "wt-c", T).unwrap_err(),
            "0001 is in Your check, merged already; there's nothing to wait for"
        );
        // Its depends set to one in Your check: nothing open is left, and the log says so.
        set_field(p, "0004", "depends", "0001", "owner", T).unwrap();
        assert!(last_log(p, "0004").ends_with("owner: unblocked: no open depends left"));
        let old = "stage: ready\nstatus: blocked\nclaimed-by:\nwaiting_on: 0001";
        put(p, "blocked", "0005", old, "");
        let s = state(p);
        assert!(
            s.problems
                .iter()
                .any(|w| w.ends_with("none of them is open now: muster unblock 0005")),
            "{:?}",
            s.problems
        );
    }

    #[test]
    fn reaching_your_check_logs_unblocked_on_the_tasks_it_frees() {
        let d = queue();
        let p = d.path();
        put(p, "doing", "0001", TESTING, TESTED);
        put(
            p,
            "done",
            "0002",
            "stage: done\nstatus: done\nclaimed-by:\nwaiting_on:",
            "",
        );
        put(p, "todo", "0003", READY, "");
        put(p, "todo", "0004", &format!("{READY}\ndepends: [0001]"), "");
        put(
            p,
            "todo",
            "0005",
            &format!("{READY}\ndepends: [0002, 0001]"),
            "",
        );
        // Still waiting on 0003 too: no line.
        put(
            p,
            "todo",
            "0006",
            &format!("{READY}\ndepends: [0001, 0003]"),
            "",
        );
        pass_task(p, "0001", "qa", "2026-10-13 10:00", &PassOpts::default()).unwrap();
        assert!(
            last_log(p, "0004").ends_with("qa: unblocked: bk-1 in Your check"),
            "{}",
            last_log(p, "0004")
        );
        assert!(
            last_log(p, "0005").ends_with("qa: unblocked: bk-2 done; bk-1 in Your check"),
            "{}",
            last_log(p, "0005")
        );
        assert!(last_log(p, "0006").ends_with("capture: created"));
        let t = task(p, "0006");
        assert_eq!(sentence_of(&t, false), "After 0003 (Ready)");
        // Accepted later: it was free already, so Done writes nothing more on it.
        pass_task(p, "0001", "owner", T, &PassOpts::default()).unwrap();
        assert!(last_log(p, "0004").ends_with("qa: unblocked: bk-1 in Your check"));
        assert_eq!(task(p, "0004").log_tail.len(), 2);
        assert!(state(p).problems.is_empty(), "{:?}", state(p).problems);
    }

    #[test]
    fn a_return_from_your_check_blocks_unclaimed_dependents_and_notes_claimed_ones() {
        let d = queue();
        let p = d.path();
        put(p, "doing", "0001", TESTING, TESTED);
        put(p, "todo", "0002", &format!("{READY}\ndepends: [0001]"), "");
        put(p, "todo", "0003", &format!("{READY}\ndepends: [0001]"), "");
        put(p, "todo", "0004", READY, "");
        put(p, "todo", "0005", &format!("{READY}\ndepends: [0001]"), "");
        pass_task(p, "0001", "qa", T, &PassOpts::default()).unwrap();
        claim_task(p, "0003", "wt-a", T).unwrap();
        // 0005 is in Testing already.
        claim_task(p, "0005", "wt-b", T).unwrap();
        pass_task(
            p,
            "0005",
            "wt-b",
            T,
            &PassOpts {
                gate: "gate full: green".into(),
                summary: "A synthetic summary".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let before = fs::read_to_string(p.join("todo/0002-t.md")).unwrap();
        let at = "2026-10-13 11:00";
        return_task(
            p,
            "0001",
            "the colours are off",
            Some("building"),
            "owner",
            at,
        )
        .unwrap();
        // Not claimed yet: blocked again, derived, with nothing written.
        let t = task(p, "0002");
        assert_eq!(sentence_of(&t, false), "After 0001 (Building)");
        assert_eq!(
            fs::read_to_string(p.join("todo/0002-t.md")).unwrap(),
            before
        );
        assert!(claim_task(p, "0002", "wt-c", T)
            .unwrap_err()
            .contains("0002 depends on 0001, not merged yet"));
        // Claimed ones keep going, with a note: no forced return, no block, the claim still held.
        for (id, stage, holder) in [("0003", "building", "wt-a"), ("0005", "testing", "")] {
            let t = task(p, id);
            assert_eq!(
                (
                    t.stage.as_str(),
                    t.blocked_kind.as_str(),
                    t.claimed_by.as_str()
                ),
                (stage, "", holder),
                "{id}"
            );
            assert!(
                t.log_tail
                    .last()
                    .unwrap()
                    .ends_with("owner: bk-1 went back to Building: the colours are off"),
                "{id}: {:?}",
                t.log_tail
            );
        }
        assert!(claim_task(p, "0004", "wt-a", T)
            .unwrap_err()
            .contains("you already hold 0003"));
        // A task under way doesn't wait for it again, even when asked to: that would hide nothing.
        assert_eq!(
            block_task(p, "0003", "0001", "wt-a", T).unwrap_err(),
            "0001 went back after it was merged, and 0003 is under way, so it doesn't wait for it again; block on outside or you to wait anyway"
        );
        // Merged again: the unclaimed one is free and says so; the claimed one has nothing new.
        // The send-back's owner finding (mu-101) is answered first; nobody's claim is on it, so
        // the owner answers.
        let n3 = task(p, "0003").log_tail.len();
        crate::review::answer(
            p,
            "0001",
            "R1",
            crate::review::Reply::Fixed,
            "colours fixed",
            "owner",
            T,
        )
        .unwrap();
        pass_task(
            p,
            "0001",
            "wt-x",
            T,
            &PassOpts {
                gate: "gate full: green".into(),
                summary: "A synthetic summary".into(),
                ..Default::default()
            },
        )
        .unwrap();
        pass_task(p, "0001", "qa", "2026-10-13 12:00", &PassOpts::default()).unwrap();
        assert!(last_log(p, "0002").ends_with("qa: unblocked: bk-1 in Your check"));
        assert_eq!(task(p, "0003").log_tail.len(), n3);
        // Sent back by the owner's override, without a reason: the note still says where.
        set_stage(p, "0001", "testing", None, "owner", T).unwrap();
        assert!(last_log(p, "0003").ends_with("owner: bk-1 went back to Testing"));
        assert!(state(p).problems.is_empty(), "{:?}", state(p).problems);
    }

    #[test]
    fn a_pass_into_building_waits_on_depends_as_a_claim_does() {
        let d = queue();
        let p = d.path();
        put(p, "doing", "0001", TESTING, TESTED);
        put(p, "todo", "0002", &format!("{READY}\ndepends: [0001]"), "");
        // Merged, then sent back: 0002 hasn't started, so it waits again.
        pass_task(p, "0001", "qa", T, &PassOpts::default()).unwrap();
        return_task(
            p,
            "0001",
            "the colours are off",
            Some("building"),
            "owner",
            T,
        )
        .unwrap();
        let before = fs::read_to_string(p.join("todo/0002-t.md")).unwrap();
        // Pass on the Ready card is a claim: refused the same way, for anyone, nothing written.
        for who in ["wt-z", "owner"] {
            let e = pass_task(p, "0002", who, T, &PassOpts::default()).unwrap_err();
            assert_eq!(
                e,
                "0002 depends on 0001, not merged yet (an agent's task counts once it's in Your check, the owner's own once it's Done); pick another (muster next)"
            );
        }
        assert_eq!(
            fs::read_to_string(p.join("todo/0002-t.md")).unwrap(),
            before
        );
        let t = task(p, "0002");
        assert_eq!(
            (t.stage.as_str(), sentence_of(&t, false).as_str()),
            ("ready", "After 0001 (Building)")
        );
        // The owner's override still moves it, logged as one.
        set_stage(p, "0002", "building", None, "owner", T).unwrap();
        assert!(last_log(p, "0002").ends_with("owner: stage ready → building (set)"));
    }

    #[test]
    fn the_owners_own_task_reopened_from_done_says_done_not_merged() {
        let d = queue();
        let p = d.path();
        let doing = "stage: building\nstatus: doing\nclaimed-by: owner\nwaiting_on:\nfor: human";
        put(p, "doing", "0001", doing, "");
        put(p, "todo", "0002", &format!("{READY}\ndepends: [0001]"), "");
        pass_task(p, "0001", "owner", T, &PassOpts::default()).unwrap();
        claim_task(p, "0002", "wt-a", T).unwrap();
        set_stage(p, "0001", "building", None, "owner", T).unwrap();
        // Overridden back out of Done: noted on the task under way, which keeps going.
        assert!(last_log(p, "0002").ends_with("owner: bk-1 went back to Building"));
        assert_eq!(
            block_task(p, "0002", "0001", "wt-a", T).unwrap_err(),
            "0001 went back after it was done, and 0002 is under way, so it doesn't wait for it again; block on outside or you to wait anyway"
        );
    }
}
