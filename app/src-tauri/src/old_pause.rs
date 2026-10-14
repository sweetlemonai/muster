//! Old pauses (task 0059). Before 0045 a pause was a block whose reason said so: `waiting_on:
//! owner (paused)`, `paused: design first`. `find` lists them in a queue and `convert` turns each
//! into a real pause with the same reason and time: `paused:` the time it was paused, `paused-why:`
//! what the reason says besides "paused", and the block cleared as `muster unblock` clears it (the
//! status mirror, and in an old queue the folder, follow the stage). No session is told: a task
//! blocked as paused had nobody working on it. `muster check --fix-pauses` and the app's offer on
//! load run them on every queue Muster knows; `muster check` warns about one until then.
//!
//! The log gets the pause line stamped with the time it was paused, so the timeline and the
//! actuals (`pause::history`) see the pause from then; the unblocked line has the same stamp, so
//! the block it ends takes no time (it was a pause, not a wait). Both are written after the lines
//! that came since, so a now-stamped line after them says what happened and when.

use crate::ops::{self, clear_front, set_front, stamp_minutes};
use crate::pause::{self, MAX_WHY, PAUSABLE};
use crate::queue::{self, actor, append_log, with_lock, write_atomic, Task};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// An old pause: a task blocked with a reason that says paused, as `find` reports it.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OldPause {
    /// The queue's name, as the app and `muster projects` call it.
    pub queue: String,
    pub id: String,
    /// Its key (0033): `dr-22`.
    pub key: String,
    pub title: String,
    pub stage: String,
    /// The reason it keeps (`paused-why:`): the block's words without "owner", "capture" and the
    /// word paused itself; "" when nothing else is left.
    pub reason: String,
    /// When it was paused (`paused:`), `YYYY-MM-DD HH:MM`.
    pub since: String,
    /// Its file, relative to the queue: where it is, or after `convert`, where it went.
    pub path: String,
}

/// What a conversion did: the old pauses it turned into pauses, and the ones it couldn't, each as
/// "<key> (<queue>): <why>" ("<queue>: <why>" for a whole queue).
#[derive(Serialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Fixed {
    pub converted: Vec<OldPause>,
    pub failed: Vec<String>,
}

/// Does `text` say paused: the word, in any case, not inside another word ("unpaused")?
pub fn says_paused(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.match_indices("paused").any(|(i, w)| {
        let before = lower[..i].chars().next_back();
        let after = lower[i + w.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

/// Is `t` paused the old way? A task (not a goal or feature) in a stage it can be paused in,
/// flagged blocked (not only by the tasks it depends on, nor by an old block on task ids) with a
/// `waiting_on` that says paused, and not paused already.
pub fn is_old_pause(t: &Task) -> bool {
    t.blocked
        && t.paused.is_empty()
        && t.level == "task"
        && t.blocked_kind != "tasks"
        && PAUSABLE.contains(&t.stage.as_str())
        && says_paused(&t.waiting_on)
}

/// What may stand between the word paused and the rest of a reason: "paused: x", "x (paused)".
const SEPARATORS: &[char] = &[' ', ':', ',', ';', '.', '-', '—'];

/// `text` without the word paused (or `(paused)`) at its start or its end, again and again, and
/// the separators next to it.
fn without_paused(text: &str) -> &str {
    let mut s = text.trim();
    loop {
        let before = s;
        for word in ["(paused)", "paused"] {
            let n = word.len();
            if s.get(..n).is_some_and(|h| h.eq_ignore_ascii_case(word))
                && !s[n..].starts_with(char::is_alphanumeric)
            {
                s = s[n..].trim_start_matches(SEPARATORS);
            }
            let end = s.len().saturating_sub(n);
            if s.len() >= n
                && s.get(end..).is_some_and(|t| t.eq_ignore_ascii_case(word))
                && !s[..end].ends_with(char::is_alphanumeric)
            {
                s = s[..end].trim_end_matches(SEPARATORS);
            }
        }
        if s == before {
            return s;
        }
    }
}

/// The reason an old pause keeps: `waiting_on` without a leading "owner" or "capture" (as an old
/// block's detail is read, `block::guessed_detail`) and without the word paused at its start or
/// end, with the separators next to it; "" when nothing else is left. At most `MAX_WHY` characters.
pub fn reason(waiting_on: &str) -> String {
    let w = waiting_on.trim();
    let detail = match crate::block::guess(w) {
        kind @ ("you" | "question") => crate::block::guessed_detail(w, kind),
        _ => w.to_string(),
    };
    let kept: String = without_paused(&detail).chars().take(MAX_WHY).collect();
    // One line, as a pause's reason is (`ops::one_line`); nothing left is no reason.
    ops::one_line(&kept, MAX_WHY, "the reason").unwrap_or_default()
}

/// When `t` was paused, from its log (`log`, its `- ` lines): the line that blocked it (the
/// start of its open block, `queue::block_history`), else the latest line that mentions the pause,
/// else the last line; `stamp` when no line has a time. Never inside a pause that's over: from
/// its resume at the earliest, so the stretches don't overlap.
fn paused_at(t: &Task, log: &[&str], stamp: &str) -> String {
    let timed = |at: String| stamp_minutes(&at).is_some().then_some(at);
    let at_of = |l: &&str| queue::log_stamp(l).and_then(|(at, _)| timed(at));
    let blocked_at = t
        .block_history
        .last()
        .filter(|b| b.to.is_empty())
        .and_then(|b| timed(b.from.clone()));
    let at = blocked_at
        .or_else(|| {
            log.iter()
                .rev()
                .filter(|l| l.to_lowercase().contains("pause"))
                .find_map(at_of)
        })
        .or_else(|| log.iter().rev().find_map(at_of))
        .unwrap_or_else(|| stamp.to_string());
    match t.pause_history.last() {
        Some(p) if p.to > at => p.to.clone(),
        _ => at,
    }
}

/// `t`, an old pause of the queue at `root`, as `find` reports it.
fn found(queue: &str, root: &Path, prefix: &str, t: &Task, stamp: &str) -> OldPause {
    let text = fs::read_to_string(root.join(&t.path)).unwrap_or_default();
    let body = queue::front_matter(&text).map(|(_, b)| b).unwrap_or("");
    let log: Vec<&str> = queue::log_section(body)
        .map(|s| s.lines().filter(|l| l.starts_with("- ")).collect())
        .unwrap_or_default();
    OldPause {
        queue: queue.to_string(),
        id: t.id.clone(),
        key: crate::key::key(prefix, &t.id),
        title: t.title.clone(),
        stage: t.stage.clone(),
        reason: reason(&t.waiting_on),
        since: paused_at(t, &log, stamp),
        path: t.path.clone(),
    }
}

/// The old pauses in the queue at `root` (named `queue`), by id. Reads only: nothing is written,
/// not even the lock. `stamp` is the time for one whose log has no time at all.
pub fn find(queue: &str, root: &Path, stamp: &str) -> Vec<OldPause> {
    let prefix = crate::key::prefix_for(root);
    ops::tasks_now(root)
        .iter()
        .filter(|t| is_old_pause(t))
        .map(|t| found(queue, root, &prefix, t, stamp))
        .collect()
}

/// `find` on every queue, in order.
pub fn find_all(queues: &[(String, PathBuf)], stamp: &str) -> Vec<OldPause> {
    queues
        .iter()
        .flat_map(|(name, root)| find(name, root, stamp))
        .collect()
}

/// Turn the old pauses among `ids` in the queue at `root` into pauses, under the queue's lock,
/// each checked again there; an id that isn't one (any more) is left alone, so running it again
/// changes nothing. One that can't be converted (its holder holds another task in Building, say)
/// is put back as it was and listed in `failed`. Err only when the queue can't be locked.
pub fn convert(
    queue: &str,
    root: &Path,
    ids: &[String],
    who: &str,
    stamp: &str,
) -> Result<Fixed, String> {
    let who = actor(who)?;
    let wanted = |p: &OldPause| ids.contains(&p.id);
    // Nothing to do: not even the lock (which pins a prefix and refreshes INDEX.md).
    if !find(queue, root, stamp).iter().any(wanted) {
        return Ok(Fixed::default());
    }
    with_lock(root, || {
        let mut out = Fixed::default();
        for p in find(queue, root, stamp).into_iter().filter(wanted) {
            match convert_locked(root, &p, &who, stamp) {
                Ok(path) => out.converted.push(OldPause { path, ..p }),
                Err(e) => out.failed.push(format!("{} ({queue}): {e}", p.key)),
            }
        }
        Ok(out)
    })
}

/// Turn old pause `p` into a pause, holding the lock: first the pause (`paused:`, `paused-why:`
/// and its line, the file where it is), then the block cleared as `unblock` clears it, with the
/// unblocked line and the note saying what happened. If clearing it is refused, the file is put
/// back as it was. Returns its path now.
fn convert_locked(root: &Path, p: &OldPause, who: &str, stamp: &str) -> Result<String, String> {
    let path = root.join(&p.path);
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut line = format!("{} {who}: paused", p.since);
    if !p.reason.is_empty() {
        line.push_str(&format!(" ({})", p.reason));
    }
    let line = pause::checked(line)?;
    let note = ops::free_text_line(
        format!(
            "{stamp} {who}: turned the old paused block into a pause, paused since {}",
            p.since
        ),
        "the note",
    )?;
    let mut new = set_front(&text, "paused", &p.since)?;
    new = if p.reason.is_empty() {
        clear_front(&new, "paused-why")?
    } else {
        set_front(&new, "paused-why", &p.reason)?
    };
    write_atomic(&path, &append_log(&new, &line))?;
    ops::unblock_locked_at(root, &p.id, who, stamp, &p.since, &[note]).map_err(|e| {
        match write_atomic(&path, &text) {
            Ok(()) => e,
            Err(w) => format!("{e}; and it couldn't be put back as it was: {w}"),
        }
    })
}

/// An old pause as the app listed it and sends it back once the owner says yes: its queue's name
/// and its id.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Listed {
    pub queue: String,
    pub id: String,
}

/// Convert the old pauses the owner was shown (`items`), by `who`, in `queues` (the ones Muster
/// knows): nothing else, so one blocked as paused after the list was made stays as it is.
pub fn convert_listed(
    queues: &[(String, PathBuf)],
    items: &[Listed],
    who: &str,
    stamp: &str,
) -> Fixed {
    let mut out = Fixed::default();
    // Checked first: a queue Muster doesn't know, or an id that isn't one, goes no further.
    let mut wanted: Vec<(&str, &Path, Vec<String>)> = Vec::new();
    for item in items {
        let Some((name, root)) = queues.iter().find(|(n, _)| *n == item.queue) else {
            let name: String = item.queue.chars().take(60).collect();
            out.failed.push(format!("{name}: no queue named {name:?}"));
            continue;
        };
        if let Err(e) = crate::check_id(&item.id) {
            let id: String = item.id.chars().take(12).collect();
            out.failed.push(format!("{id} ({name}): {e}"));
            continue;
        }
        match wanted.iter_mut().find(|(n, _, _)| *n == name.as_str()) {
            Some((_, _, ids)) if !ids.contains(&item.id) => ids.push(item.id.clone()),
            Some(_) => {}
            None => wanted.push((name.as_str(), root.as_path(), vec![item.id.clone()])),
        }
    }
    // Then each queue in Muster's order, as `muster check --fix-pauses` does it.
    wanted.sort_by_key(|(n, _, _)| queues.iter().position(|(q, _)| q == n));
    for (name, root, ids) in wanted {
        match convert(name, root, &ids, who, stamp) {
            Ok(f) => {
                let prefix = crate::key::prefix_for(root);
                // Listed, but neither converted nor refused: not an old pause now (resumed,
                // unblocked or gone since).
                let gone: Vec<String> = ids
                    .iter()
                    .map(|i| crate::key::key(&prefix, i))
                    .filter(|k| {
                        !f.converted.iter().any(|p| p.key == *k)
                            && !f.failed.iter().any(|e| e.starts_with(&format!("{k} (")))
                    })
                    .map(|k| format!("{k} ({name}): isn't an old paused block (any more)"))
                    .collect();
                out.converted.extend(f.converted);
                out.failed.extend(f.failed);
                out.failed.extend(gone);
            }
            Err(e) => out.failed.push(format!("{name}: {e}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::{read_task, STATUS_FOLDERS};

    const NOW: &str = "2026-10-13 18:00";
    const A: &str = "wt-a (wt-0045 · session 7b5e8856 · cc-hg)";

    /// An old queue (`<project>/tasks`, status folders) with the prefix dr.
    fn old_queue() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for f in STATUS_FOLDERS {
            fs::create_dir(d.path().join(f)).unwrap();
        }
        fs::write(d.path().join("README.md"), "---\nprefix: dr\n---\n\nq").unwrap();
        d
    }
    /// A repo's `.muster/` with the prefix mu, and the folder that holds it.
    fn muster_queue() -> (tempfile::TempDir, PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let q = d.path().join(crate::layout::DIR);
        fs::create_dir_all(q.join("tasks")).unwrap();
        fs::write(
            q.join(crate::layout::CONFIG),
            crate::layout::config_text(true, Some("mu")),
        )
        .unwrap();
        (d, q)
    }
    /// Task `id` in `folder`, with `front` in its front matter and `log` after its created line.
    fn put(root: &Path, folder: &str, id: &str, front: &str, log: &str) {
        fs::write(
            root.join(folder).join(format!("{id}-t.md")),
            format!("---\nid: {id}\ntype: code\npriority: normal\n{front}\n---\n\n# Task {id}\n\n## Log\n- 2026-10-13 08:00 capture: created\n{log}"),
        )
        .unwrap();
    }
    /// An old file blocked on `waiting_on`, in blocked/ with no stage (read as Ready).
    fn blocked(root: &Path, id: &str, waiting_on: &str) {
        put(
            root,
            "blocked",
            id,
            &format!("status: blocked\nclaimed-by:\nheartbeat:\nwaiting_on: {waiting_on}"),
            "",
        );
    }
    /// Drover's dr-22 as it was paused before 0045: no stage, `owner (paused)`, and a free-text
    /// line saying so.
    const DROVER_22: &str = "---\nid: 0022\ntype: feature\npriority: medium\ntags: code, spike\nstatus: blocked\ncaptured: 2026-10-12\nclaimed-by:\nheartbeat:\nwaiting_on: owner (paused)\ncommit:\n---\n\n# Desk pets: a cartoon cat, dog and parrot\n\n## Log\n- 2026-10-12 capture: created from the owner's message\n- 2026-10-12 18:01 muster/main 4d517d9d: stage ready → building (claimed)\n- 2026-10-13 01:09 capture (349bc9c3): claimed at the owner's request; branch pets-0022\n- 2026-10-13 02:51 owner: \"pause the pet project\". Paused; claim released. State to resume from: branch pets-0022.\n";
    fn file(root: &Path, id: &str) -> String {
        fs::read_to_string(queue::find_task(root, id).unwrap()).unwrap()
    }
    /// No session's mailbox was made (the lock's own file aside).
    fn no_mail(root: &Path) {
        let boxes: Vec<_> = fs::read_dir(root.join("mail"))
            .map(|rd| rd.flatten().filter(|e| e.path().is_dir()).collect())
            .unwrap_or_default();
        assert!(boxes.is_empty(), "{boxes:?}");
    }

    #[test]
    fn paused_is_the_word_in_any_case_and_never_inside_another() {
        for yes in [
            "paused",
            "owner (paused)",
            "PAUSED: design first",
            "the API first, paused",
            "owner—paused",
        ] {
            assert!(says_paused(yes), "{yes}");
        }
        for no in [
            "",
            "waiting on the design",
            "unpaused build",
            "pausedness",
            "pause it",
            "owner: pick a colour",
        ] {
            assert!(!says_paused(no), "{no}");
        }
    }

    #[test]
    fn the_reason_kept_is_the_block_without_owner_capture_and_the_word_paused() {
        for (waiting_on, kept) in [
            ("owner (paused)", ""),
            ("paused", ""),
            ("(paused)", ""),
            ("Owner: paused", ""),
            ("Paused: design first", "design first"),
            (
                "paused — the design changes first",
                "the design changes first",
            ),
            (
                "owner: paused, the design changes first",
                "the design changes first",
            ),
            ("the API first (paused)", "the API first"),
            ("capture, paused; Q3 first", "Q3 first"),
            (
                "owner: on hold, paused by the owner",
                "on hold, paused by the owner",
            ),
            ("design first. Paused", "design first"),
        ] {
            assert_eq!(reason(waiting_on), kept, "{waiting_on}");
        }
        // Capped as a pause's reason is.
        let long = format!("paused: {}", "é".repeat(300));
        assert_eq!(reason(&long).chars().count(), MAX_WHY);
    }

    #[test]
    fn finds_the_blocks_that_say_paused_and_nothing_else() {
        let d = old_queue();
        let r = d.path();
        blocked(r, "0001", "owner (paused)");
        blocked(r, "0002", "paused");
        blocked(r, "0003", "Paused: design first");
        blocked(r, "0004", "owner: paused, the design changes first");
        blocked(r, "0005", "the API first (paused)");
        // Typed since 0024, as muster writes a block on you.
        put(
            r,
            "blocked",
            "0006",
            "status: blocked\nstage: building\nclaimed-by: wt-a\nwaiting_on: owner: paused\nblocked-kind: you",
            "",
        );
        put(
            r,
            "blocked",
            "0007",
            "status: blocked\nstage: backlog\nwaiting_on: paused\nblocked-kind: outside",
            "",
        );
        // Not old pauses: another reason, the word inside another, a wait on tasks alone or an
        // old block on a task id, one paused already, Your check, Done, a feature.
        blocked(r, "0010", "waiting on the design");
        blocked(r, "0011", "unpaused build");
        put(
            r,
            "todo",
            "0012",
            "status: todo\nstage: ready\ndepends: [0013]\nwaiting_on:",
            "",
        );
        put(
            r,
            "todo",
            "0013",
            "status: todo\nstage: ready\nwaiting_on:",
            "",
        );
        blocked(r, "0014", "0013 (paused)");
        put(
            r,
            "blocked",
            "0015",
            "status: blocked\nstage: ready\nwaiting_on: owner (paused)\npaused: 2026-10-13 09:00",
            "- 2026-10-13 09:00 owner: paused\n",
        );
        put(
            r,
            "blocked",
            "0016",
            "status: blocked\nstage: your-check\nwaiting_on: paused",
            "",
        );
        put(
            r,
            "blocked",
            "0017",
            "status: blocked\nstage: done\nwaiting_on: paused",
            "",
        );
        put(
            r,
            "blocked",
            "0018",
            "status: blocked\nstage: ready\nlevel: feature\nwaiting_on: paused",
            "",
        );
        let found = find("drover", r, NOW);
        let got: Vec<(&str, &str, &str, &str)> = found
            .iter()
            .map(|p| {
                (
                    p.key.as_str(),
                    p.stage.as_str(),
                    p.reason.as_str(),
                    p.path.as_str(),
                )
            })
            .collect();
        assert_eq!(
            got,
            [
                ("dr-1", "ready", "", "blocked/0001-t.md"),
                ("dr-2", "ready", "", "blocked/0002-t.md"),
                ("dr-3", "ready", "design first", "blocked/0003-t.md"),
                (
                    "dr-4",
                    "ready",
                    "the design changes first",
                    "blocked/0004-t.md"
                ),
                ("dr-5", "ready", "the API first", "blocked/0005-t.md"),
                ("dr-6", "building", "", "blocked/0006-t.md"),
                ("dr-7", "backlog", "", "blocked/0007-t.md"),
            ]
        );
        assert!(found
            .iter()
            .all(|p| p.queue == "drover" && p.title == format!("Task {}", p.id)));
    }

    #[test]
    fn the_time_is_the_block_line_else_a_line_about_the_pause_else_the_last_line() {
        let d = old_queue();
        let r = d.path();
        let front = "status: blocked\nstage: ready\nwaiting_on: owner (paused)";
        // The tool's block line, though a later line mentions the pause.
        put(
            r,
            "blocked",
            "0001",
            front,
            "- 2026-10-13 09:00 owner: blocked on you: paused\n- 2026-10-13 11:00 wt-a: note: paused work kept on the branch\n",
        );
        // No block line (written by hand): the latest line about the pause.
        put(
            r,
            "blocked",
            "0002",
            front,
            "- 2026-10-13 01:00 capture: the owner may pause this\n- 2026-10-13 02:51 owner: \"pause the pet project\". Paused; claim released.\n- 2026-10-13 03:10 capture: answered Q1\n",
        );
        // Nothing about it: the last line.
        put(
            r,
            "blocked",
            "0003",
            front,
            "- 2026-10-13 09:30 capture: waiting for the owner\n",
        );
        // No log at all: now.
        fs::write(
            r.join("blocked/0004-t.md"),
            format!("---\nid: 0004\n{front}\n---\n\n# Task 0004\n"),
        )
        .unwrap();
        // Blocked as paused, then paused and resumed since: from the resume, never inside a
        // pause that's over.
        put(
            r,
            "blocked",
            "0005",
            front,
            "- 2026-10-13 09:00 owner: blocked on you: paused\n- 2026-10-13 10:00 owner: paused\n- 2026-10-13 11:00 owner: resumed after 1 h\n",
        );
        // Its last block over (unblocked), then paused in words and waiting_on edited by hand: the
        // closed block isn't when it was paused, the line about the pause is.
        put(
            r,
            "blocked",
            "0006",
            front,
            "- 2026-10-13 09:00 owner: blocked on you: pick a colour\n- 2026-10-13 10:00 owner: unblocked (was waiting on you: pick a colour)\n- 2026-10-13 11:00 owner: Paused until the design is back\n",
        );
        let since: Vec<String> = find("q", r, NOW).into_iter().map(|p| p.since).collect();
        assert_eq!(
            since,
            [
                "2026-10-13 09:00",
                "2026-10-13 02:51",
                "2026-10-13 09:30",
                NOW,
                "2026-10-13 11:00",
                "2026-10-13 11:00"
            ]
        );
    }

    #[test]
    fn an_old_file_moves_back_as_unblock_moves_it_and_is_paused_from_when_it_was() {
        let d = old_queue();
        let r = d.path();
        let old = r.join("blocked/0022-desk-pets.md");
        fs::write(&old, DROVER_22).unwrap();
        let found = find("drover", r, NOW);
        assert_eq!(
            found,
            [OldPause {
                queue: "drover".into(),
                id: "0022".into(),
                key: "dr-22".into(),
                title: "Desk pets: a cartoon cat, dog and parrot".into(),
                stage: "ready".into(),
                reason: String::new(),
                since: "2026-10-13 02:51".into(),
                path: "blocked/0022-desk-pets.md".into(),
            }]
        );
        let f = convert("drover", r, &["0022".into()], "owner", NOW).unwrap();
        assert!(f.failed.is_empty(), "{f:?}");
        assert_eq!(f.converted.len(), 1);
        assert_eq!(f.converted[0].path, "todo/0022-desk-pets.md");
        assert!(!old.exists());
        let text = file(r, "0022");
        // The stage an old blocked file reads as, written now, and its status and folder with it.
        assert!(text.contains("\nstatus: todo\n"), "{text}");
        assert!(text.contains("\nstage: ready\n"), "{text}");
        assert!(text.contains("\nwaiting_on:\n"), "{text}");
        assert!(text.contains("\npaused: 2026-10-13 02:51\n"), "{text}");
        assert!(!text.contains("paused-why"), "no reason left: {text}");
        assert!(
            !text.contains("blocked-kind"),
            "none was ever written: {text}"
        );
        assert!(text.ends_with(&format!(
            "Paused; claim released. State to resume from: branch pets-0022.\n- 2026-10-13 02:51 owner: paused\n- 2026-10-13 02:51 owner: unblocked (was waiting on owner (paused))\n- {NOW} owner: turned the old paused block into a pause, paused since 2026-10-13 02:51\n"
        )));
        let t = read_task(r, "0022").unwrap();
        assert_eq!(
            (t.stage.as_str(), t.blocked, t.paused.as_str()),
            ("ready", false, "2026-10-13 02:51")
        );
        assert_eq!(
            t.pause_history,
            [pause::PauseSpan {
                from: "2026-10-13 02:51".into(),
                to: String::new()
            }]
        );
        assert!(t.block_history.is_empty(), "{:?}", t.block_history);
        assert_eq!(
            t.stage_since, "",
            "no stage line: it's where it was, not moved"
        );
        no_mail(r);
        // `muster check` has nothing to say about it any more.
        let tasks = ops::tasks_now(r);
        let p = queue::problems("q", &tasks, 0, NOW, ops::LEASE_MINUTES);
        assert!(p.is_empty(), "{p:?}");
        // Again: nothing to do, nothing written.
        assert!(find("drover", r, NOW).is_empty());
        let again = convert("drover", r, &["0022".into()], "owner", "2026-10-13 19:00").unwrap();
        assert_eq!(again, Fixed::default());
        assert_eq!(file(r, "0022"), text);
        // Resumed, it's back as it was: Ready, in todo/.
        let back = pause::resume(r, "0022", "owner", "2026-10-14 09:00").unwrap();
        assert_eq!(back.path, "todo/0022-desk-pets.md");
        let t = read_task(r, "0022").unwrap();
        assert_eq!(
            (t.stage.as_str(), t.blocked, t.paused.as_str()),
            ("ready", false, "")
        );
        assert_eq!(
            t.pause_history,
            [pause::PauseSpan {
                from: "2026-10-13 02:51".into(),
                to: "2026-10-14 09:00".into()
            }]
        );
        assert!(file(r, "0022").ends_with("owner: resumed after 1 d 6 h 9 min\n"));
    }

    #[test]
    fn in_muster_a_claimed_task_keeps_its_stage_and_claim_and_no_session_is_told() {
        let (_d, q) = muster_queue();
        put(
            &q,
            "tasks",
            "0042",
            &format!("stage: building\nclaimed-by: {A}\nheartbeat: 2026-10-13 09:00\nwaiting_on: owner: paused: the design first\nblocked-kind: you"),
            "- 2026-10-13 10:00 owner: blocked on you: paused: the design first\n",
        );
        let found = find("muster", &q, NOW);
        assert_eq!(found.len(), 1);
        assert_eq!(
            (
                found[0].key.as_str(),
                found[0].reason.as_str(),
                found[0].since.as_str()
            ),
            ("mu-42", "the design first", "2026-10-13 10:00")
        );
        let f = convert("muster", &q, &["0042".into()], "owner", NOW).unwrap();
        assert_eq!(
            f.converted[0].path, "tasks/0042-t.md",
            "files never move here"
        );
        let t = read_task(&q, "0042").unwrap();
        assert_eq!(
            (
                t.stage.as_str(),
                t.claimed_by.as_str(),
                t.blocked,
                t.blocked_kind.as_str()
            ),
            ("building", A, false, "")
        );
        assert_eq!(
            (
                t.paused.as_str(),
                t.paused_why.as_str(),
                t.heartbeat.as_str()
            ),
            ("2026-10-13 10:00", "the design first", NOW),
            "the lease starts over, as any unblock's does"
        );
        // Blocked for no time at all, paused since then.
        assert_eq!(
            t.block_history,
            [queue::BlockSpan {
                from: "2026-10-13 10:00".into(),
                to: "2026-10-13 10:00".into(),
                kind: "you".into()
            }]
        );
        assert_eq!(
            t.pause_history,
            [pause::PauseSpan {
                from: "2026-10-13 10:00".into(),
                to: String::new()
            }]
        );
        assert!(file(&q, "0042").contains(
            "- 2026-10-13 10:00 owner: paused (the design first)\n- 2026-10-13 10:00 owner: unblocked (was waiting on you: paused: the design first)\n"
        ));
        no_mail(&q);
        // Its holder can't pass it until it's resumed; resumed, it's Building with its claim.
        let pass = ops::PassOpts {
            gate: "gate: green".into(),
            summary: "A synthetic summary".into(),
            ..Default::default()
        };
        assert!(ops::pass_task(&q, "0042", A, NOW, &pass)
            .unwrap_err()
            .contains("is paused (since 2026-10-13 10:00)"));
        pause::resume(&q, "0042", "owner", "2026-10-13 19:00").unwrap();
        let t = read_task(&q, "0042").unwrap();
        assert_eq!(
            (t.stage.as_str(), t.claimed_by.as_str(), t.paused.as_str()),
            ("building", A, "")
        );
        // Building's time leaves the pause out: 08:00–19:00 is 11 h, 9 of them paused.
        let h = [queue::StageStep {
            at: "2026-10-13 08:00".into(),
            who: "x".into(),
            from: "ready".into(),
            to: "building".into(),
        }];
        assert_eq!(
            pause::working_minutes(&h, &t.pause_history, "building", "2026-10-13 19:00"),
            120
        );
    }

    #[test]
    fn one_that_cant_be_unblocked_is_put_back_as_it_was() {
        let (_d, q) = muster_queue();
        // wt-a holds 0001 in Building, so unblocking 0002, theirs too, would make two.
        put(
            &q,
            "tasks",
            "0001",
            &format!("stage: building\nclaimed-by: {A}\nheartbeat: 2026-10-13 09:00\nwaiting_on:"),
            "",
        );
        put(
            &q,
            "tasks",
            "0002",
            &format!("stage: building\nclaimed-by: {A}\nheartbeat: 2026-10-13 09:00\nwaiting_on: owner (paused)"),
            "",
        );
        let before = file(&q, "0002");
        let f = convert("muster", &q, &["0002".into()], "owner", NOW).unwrap();
        assert!(f.converted.is_empty(), "{f:?}");
        assert_eq!(f.failed.len(), 1);
        assert!(
            f.failed[0].starts_with("mu-2 (muster): ") && f.failed[0].contains("already holds"),
            "{:?}",
            f.failed
        );
        assert_eq!(file(&q, "0002"), before, "as it was");
        assert_eq!(find("muster", &q, NOW).len(), 1, "still an old pause");
    }

    #[test]
    fn only_the_ids_given_and_still_old_pauses_are_converted() {
        let d = old_queue();
        let r = d.path();
        blocked(r, "0001", "owner (paused)");
        blocked(r, "0002", "paused");
        blocked(r, "0003", "waiting on the design");
        let f = convert(
            "q",
            r,
            &["0001".into(), "0003".into(), "0009".into()],
            "owner",
            NOW,
        )
        .unwrap();
        let ids: Vec<&str> = f.converted.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["0001"]);
        assert!(f.failed.is_empty());
        assert!(read_task(r, "0002").unwrap().blocked, "not asked for");
        assert!(read_task(r, "0003").unwrap().blocked, "not a pause");
        assert!(read_task(r, "0003").unwrap().paused.is_empty());
        // Nothing to convert: not even the lock is taken.
        let e = tempfile::tempdir().unwrap();
        for f in STATUS_FOLDERS {
            fs::create_dir(e.path().join(f)).unwrap();
        }
        assert_eq!(
            convert("e", e.path(), &[], "owner", NOW).unwrap(),
            Fixed::default()
        );
        assert!(!e.path().join("mail").exists());
    }

    #[test]
    fn only_the_listed_ones_are_converted_in_every_queue() {
        let d = old_queue();
        fs::write(d.path().join("blocked/0022-desk-pets.md"), DROVER_22).unwrap();
        let (_m, q) = muster_queue();
        put(
            &q,
            "tasks",
            "0042",
            "stage: ready\nwaiting_on: paused: later",
            "- 2026-10-13 10:00 owner: blocked on outside: paused: later\n",
        );
        let queues = vec![
            ("drover".to_string(), d.path().to_path_buf()),
            ("muster".to_string(), q.clone()),
        ];
        let mut items: Vec<Listed> = find_all(&queues, NOW)
            .into_iter()
            .map(|p| Listed {
                queue: p.queue,
                id: p.id,
            })
            .collect();
        assert_eq!(items.len(), 2);
        // Blocked as paused after the owner saw the list: not theirs to convert yet.
        blocked(d.path(), "0023", "owner (paused)");
        let listed = |queue: &str, id: &str| Listed {
            queue: queue.into(),
            id: id.into(),
        };
        items.extend([
            listed("nowhere", "0001"),
            listed("drover", "22"),
            listed("drover", "0099"),
        ]);
        let f = convert_listed(&queues, &items, "owner", NOW);
        let keys: Vec<&str> = f.converted.iter().map(|p| p.key.as_str()).collect();
        assert_eq!(keys, ["dr-22", "mu-42"]);
        assert_eq!(
            f.failed,
            [
                "nowhere: no queue named \"nowhere\"",
                "22 (drover): bad task id",
                "dr-99 (drover): isn't an old paused block (any more)",
            ]
        );
        assert_eq!(read_task(&q, "0042").unwrap().paused_why, "later");
        let late = read_task(d.path(), "0023").unwrap();
        assert!(late.blocked && late.paused.is_empty(), "{late:?}");
        assert_eq!(
            find_all(&queues, NOW)
                .into_iter()
                .map(|p| p.key)
                .collect::<Vec<_>>(),
            ["dr-23"]
        );
    }

    #[test]
    fn check_warns_about_an_old_pause_and_points_at_fix_pauses() {
        let d = old_queue();
        let r = d.path();
        blocked(r, "0001", "owner (paused)");
        blocked(r, "0002", "owner: pick a colour");
        let mut tasks = ops::tasks_now(r);
        for t in &mut tasks {
            t.prefix = "dr".into();
        }
        let p = queue::problems("q", &tasks, 0, NOW, ops::LEASE_MINUTES);
        assert_eq!(
            p,
            [
                "q: warning: blocked/0001-t.md is paused the old way, as a block on \"owner (paused)\"; make it a pause: muster check --fix-pauses",
                "q: warning: blocked/0002-t.md is blocked on \"owner: pick a colour\" without a kind, read as you; block it again with one to confirm (muster block dr-2 --on you \"…\")",
            ]
        );
        assert!(p.iter().all(|l| queue::is_warning(l)));
    }
}
