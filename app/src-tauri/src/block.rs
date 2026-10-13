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
//! `sentence` is mirrored, word for word, by the window's `blockWords.ts`.

use crate::queue::{stage_label, Task};
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

/// The words a block's sentence starts with, the kind's, bold in the window: "Blocked by tasks",
/// "Blocked on you", "Blocked by permission", "Blocked by a question", "Blocked by outside".
pub fn lead(kind: &str) -> &'static str {
    match kind {
        "tasks" => "Blocked by tasks",
        "you" => "Blocked on you",
        "permission" => "Blocked by permission",
        "question" => "Blocked by a question",
        "outside" => "Blocked by outside",
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
/// (Ready)"; ids only in the short form), a question's detail and "waiting on capture", or the
/// detail as written. The short form, the card's, cuts a detail at `SHORT` characters.
pub fn words(kind: &str, detail: &str, by: &[BlockedBy], short: bool) -> String {
    let detail = detail.trim();
    let shown = if short {
        cut(detail)
    } else {
        detail.to_string()
    };
    match kind {
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

/// The sentence for a block, kind first: "Blocked by tasks: 0014 (Building, codex), 0016
/// (Ready)", "Blocked on you: set the API key", "Blocked by a question: Q2, waiting on capture".
/// Just the lead when there's nothing after it. Pure; `blockWords.ts` gives the same words.
pub fn sentence(kind: &str, detail: &str, by: &[BlockedBy], short: bool) -> String {
    let lead = lead(kind);
    match words(kind, detail, by, short) {
        w if w.is_empty() => lead.to_string(),
        w => format!("{lead}: {w}"),
    }
}

/// `sentence` for a task: "" when nothing blocks it.
pub fn sentence_of(t: &Task, short: bool) -> String {
    if t.blocked_kind.is_empty() {
        return String::new();
    }
    sentence(&t.blocked_kind, &t.block_detail, &t.blocked_by, short)
}

/// Once every task of a queue is read: what each is blocked by. An old block on task ids lists
/// those ids; then the tasks in `depends:` that aren't done (any file with that id, as `claim`
/// reads them; a dependency that doesn't exist doesn't hold it, `check` names it). A task that
/// isn't flagged blocked but has such a dependency is blocked by tasks; a Done task never is.
pub fn derive(tasks: &mut [Task]) {
    let mut info: HashMap<String, (String, String)> = HashMap::new();
    let mut open: HashMap<String, bool> = HashMap::new();
    for t in tasks.iter() {
        let holder = crate::session::split(t.holder()).0.to_string();
        info.entry(t.id.clone())
            .or_insert_with(|| (t.stage.clone(), holder));
        *open.entry(t.id.clone()).or_default() |= t.stage != "done";
    }
    for t in tasks.iter_mut() {
        let mut ids: Vec<String> = Vec::new();
        if t.blocked && t.blocked_kind == "tasks" {
            ids = ids_in(&t.waiting_on);
        }
        if t.stage != "done" {
            for d in &t.depends {
                let waits = *d != t.id && open.get(d).copied().unwrap_or(false);
                if waits && !ids.contains(d) {
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

    /// The same cases as blockWords.test.ts: the window and the core say the same words.
    #[test]
    fn the_sentence_says_the_kind_first() {
        let tasks = [by("0014", "building", "codex"), by("0016", "ready", "")];
        assert_eq!(
            sentence("tasks", "", &tasks, false),
            "Blocked by tasks: 0014 (Building, codex), 0016 (Ready)"
        );
        assert_eq!(
            sentence("tasks", "", &tasks, true),
            "Blocked by tasks: 0014, 0016"
        );
        assert_eq!(
            sentence("tasks", "", &[by("0099", "", "")], false),
            "Blocked by tasks: 0099"
        );
        assert_eq!(
            sentence("you", "set the API key in the Keychain", &[], false),
            "Blocked on you: set the API key in the Keychain"
        );
        assert_eq!(
            sentence(
                "permission",
                "write to ~/playground/x/tasks (sandbox), grant in the layer's write list",
                &[],
                false
            ),
            "Blocked by permission: write to ~/playground/x/tasks (sandbox), grant in the layer's write list"
        );
        assert_eq!(
            sentence("question", "Q2", &[], false),
            "Blocked by a question: Q2, waiting on capture"
        );
        assert_eq!(
            sentence("question", "Q2", &[], true),
            "Blocked by a question: Q2"
        );
        assert_eq!(
            sentence("question", "", &[], false),
            "Blocked by a question: waiting on capture"
        );
        assert_eq!(
            sentence("outside", "Apple review", &[], false),
            "Blocked by outside: Apple review"
        );
        assert_eq!(sentence("you", "", &[], false), "Blocked on you");
        assert_eq!(sentence("", "x", &[], false), "Blocked: x");
        let long = "é".repeat(70);
        assert_eq!(
            sentence("outside", &long, &[], true),
            format!("Blocked by outside: {}…", "é".repeat(60))
        );
        assert_eq!(
            sentence("outside", &long, &[], false),
            format!("Blocked by outside: {long}")
        );
    }

    // ---- Blocking through muster's rules (ops), on a throwaway queue ----------------------------

    use crate::ops::{
        block_as, block_task, claim_task, pass_task, reclaim, reclaim_task, set_stage,
        unblock_task, PassOpts,
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
        fs::write(d.path().join("README.md"), "q").unwrap();
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
        assert_eq!(
            sentence_of(t4, false),
            "Blocked by tasks: 0001 (Ready), 0099"
        );
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
                .any(|w| w.contains("(muster block 0004 --on 0001,0099)")),
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
        for (kind, detail, words, needs_you) in [
            (
                "you",
                "set the API key in the Keychain",
                "Blocked on you: set the API key in the Keychain",
                true,
            ),
            (
                "permission",
                "write to ~/x/tasks (sandbox), grant in the layer's write list",
                "Blocked by permission: write to ~/x/tasks (sandbox), grant in the layer's write list",
                true,
            ),
            (
                "question",
                "q1",
                "Blocked by a question: Q1, waiting on capture",
                false,
            ),
            (
                "outside",
                "Apple review ✓",
                "Blocked by outside: Apple review ✓",
                false,
            ),
        ] {
            block_as(p, "0001", kind, detail, "wt-a", T).unwrap();
            let t = task(p, "0001");
            assert_eq!(t.blocked_kind, kind);
            assert_eq!(sentence_of(&t, false), words);
            assert_eq!(t.waiting_for_owner, needs_you, "{kind}");
            assert!(!t.kind_guessed);
            let file = fs::read_to_string(p.join("blocked/0001-t.md")).unwrap();
            let pair = format!("\nwaiting_on: {}\nblocked-kind: {kind}\n", t.waiting_on);
            assert!(file.contains(&pair), "{file}");
            let line = format!("wt-a: blocked on {kind}: {}", t.waiting_on);
            assert!(last_log(p, "0001").ends_with(&line), "{line}");
        }
        assert!(state(p).problems.is_empty(), "{:?}", state(p).problems);
        // The timeline's span carries the kind of the block that opened it.
        assert_eq!(task(p, "0001").block_history[0].kind, "you");
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
            "Blocked by tasks: 0002 (Ready), 0003 (Building, codex), 0001 (Ready)"
        );
        assert_eq!(sentence_of(&t, true), "Blocked by tasks: 0002, 0003, 0001");
        assert!(last_log(p, "0004")
            .ends_with("wt-a: blocked on tasks: 0003, 0001 (after the API lands)"));
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
                "capture: Q1 answered, still blocked: wt-a already holds 0002 in Building; one claim per holder, so they pass, return or block it first"
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
        assert_eq!(sentence_of(&t, false), "Blocked by tasks: 0002 (Ready)");
        assert!(last_log(p, "0003").ends_with("wt-a: blocked on tasks: 0001, 0002"));
        pass_task(p, "0002", "wt-b", T, &PassOpts::default()).unwrap();
        set_stage(p, "0002", "done", None, "owner", "2026-10-13 11:00").unwrap();
        let t = task(p, "0003");
        assert_eq!((t.blocked_kind.as_str(), t.blocked_by.len()), ("", 0));
        assert!(
            t.log_tail
                .last()
                .unwrap()
                .ends_with("owner: unblocked: 0001, 0002 done"),
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
}
