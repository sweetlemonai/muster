//! Pausing a task (task 0045): "not now, keep it as it is". A paused task keeps its stage,
//! write-up, claim and worktree; nobody can claim, pass or `next` it, its lease doesn't run out
//! (reclaim skips it), and the tasks that depend on it still wait. It isn't blocked: nothing is
//! waited on. The front matter says it (`paused:` the stamp, `paused-why:` the reason), and the
//! log keeps the stretches (`<who>: paused (why)`, `<who>: resumed after 2 h`) for the timeline
//! and the actuals, which leave paused time out.
//!
//! In Building or Testing the session working on it is told, through its mailbox as 0039's pings
//! are (`mail/<session8>/<id>-pause.md`, `<id>-resume.md`, one waiting per task: the newer
//! replaces the older), and `bin/answer-hook.py` hands it over only while it still holds.

use crate::ops::{clear_front, one_line, set_front, stamp_minutes};
use crate::ping;
use crate::queue::{self, actor, append_log, with_lock, write_atomic};
use crate::session;
use serde::Serialize;
use std::fs;
use std::path::Path;

/// The stages a task can be paused in: from Backlog through Testing. Your check is the owner's
/// own look, and Done is done.
pub const PAUSABLE: [&str; 5] = ["backlog", "shaping", "ready", "building", "testing"];

/// The longest reason, in characters.
const MAX_WHY: usize = 200;

/// A stretch a task was paused, as `YYYY-MM-DD HH:MM` stamps; `to` is empty while it still is.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PauseSpan {
    pub from: String,
    pub to: String,
}

/// What a pause or resume did: the task's path, and the session it told ("" when none), or why
/// none was told ("" when one was, or when nobody works on it in its stage).
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Paused {
    pub path: String,
    pub session: String,
    pub unreached: String,
}

/// A tool-written pause or resume line from the log: its stamp, and true for a pause. `- <at>
/// <who>: paused`, `… paused (<why>)`, `… paused; asked session <id> to stop`, `- <at> <who>:
/// resumed after 2 h` (and `; asked session <id> to carry on`). A stage line or a check-in never
/// is one, whatever its text says.
pub fn pause_line(line: &str) -> Option<(String, bool)> {
    if queue::stage_line(line).is_some() || queue::checkin_line(line).is_some() {
        return None;
    }
    pause_shaped(line)
}

/// `pause_line`'s shape alone, for a line that would be read as something else first: a check-in
/// whose text has this shape is refused (`ops::checkin_task`), so it never reads as one.
pub fn pause_shaped(line: &str) -> Option<(String, bool)> {
    let rest = line.strip_prefix("- ")?;
    let at = rest.get(..16).filter(|a| stamp_minutes(a).is_some())?;
    let tail = rest.get(16..)?.strip_prefix(' ')?;
    if let Some((who, after)) = tail.split_once(": paused") {
        let ends = after.is_empty() || after.starts_with(" (") || after.starts_with("; ");
        if ends && !who.trim().is_empty() {
            return Some((at.to_string(), true));
        }
    }
    let (who, after) = tail.split_once(": resumed")?;
    let ends = after.is_empty() || after.starts_with(" after ") || after.starts_with("; ");
    (ends && !who.trim().is_empty()).then(|| (at.to_string(), false))
}

/// Would this log line (without its `- `) read as a pause or resume? Only `pause` and `resume`
/// write those, so a note that would is refused (`ops::free_text_line`).
pub fn reads_as_pause(line: &str) -> bool {
    pause_line(&format!("- {line}")).is_some()
}

/// The log's pause and resume lines as stretches, oldest first. A pause while one is open, or a
/// resume with none, adds nothing.
pub fn history(log_lines: &[&str]) -> Vec<PauseSpan> {
    let mut out: Vec<PauseSpan> = Vec::new();
    for l in log_lines {
        let Some((at, paused)) = pause_line(l) else {
            continue;
        };
        let open = out.last().is_some_and(|p| p.to.is_empty());
        if paused && !open {
            out.push(PauseSpan {
                from: at,
                to: String::new(),
            });
        } else if !paused && open {
            if let Some(p) = out.last_mut() {
                p.to = at;
            }
        }
    }
    out
}

/// Minutes of `from..to` (minutes since 1970, local, as `stamp_minutes` gives) that fall in a
/// paused stretch; one still open runs to `now`.
pub fn paused_within(spans: &[PauseSpan], from: i64, to: i64, now: i64) -> i64 {
    spans
        .iter()
        .filter_map(|p| {
            let a = stamp_minutes(&p.from)?;
            let z = if p.to.is_empty() {
                now
            } else {
                stamp_minutes(&p.to)?
            };
            Some((a.max(from), z.min(to)))
        })
        .map(|(a, z)| (z - a).max(0))
        .sum()
}

/// Minutes a task spent in `stage` by its stage history, up to `now` while it's still there,
/// without the paused stretches: the merge report's "Building 2h 10m" (0023).
pub fn working_minutes(
    history: &[queue::StageStep],
    pauses: &[PauseSpan],
    stage: &str,
    now: &str,
) -> i64 {
    let Some(end) = stamp_minutes(now) else {
        return 0;
    };
    let mut total = 0;
    let mut since: Option<i64> = None;
    let mut add = |a: i64, b: i64| total += (b - a).max(0) - paused_within(pauses, a, b, end);
    for s in history {
        let at = stamp_minutes(&s.at);
        if s.from == stage {
            if let (Some(a), Some(b)) = (since.take(), at) {
                add(a, b);
            }
        }
        if s.to == stage {
            since = at;
        }
    }
    if let Some(a) = since {
        add(a, end);
    }
    total
}

/// Why a paused task can't be claimed, passed or picked.
pub fn refusal(id: &str, since: &str) -> String {
    format!("{id} is paused (since {since}); resume it first: muster resume {id}")
}

/// The session working on `t` that a pause or resume tells: Some(id) in Building or Testing when
/// its holder's stamp names one (`ping::reach`), else None with why ("" when nobody works on it
/// in its stage, so there's nothing to say).
fn told(t: &queue::Task) -> (Option<String>, String) {
    if t.stage != "building" && t.stage != "testing" {
        return (None, String::new());
    }
    match ping::reach(t) {
        Ok(sid) => (Some(sid), String::new()),
        Err(why) => (None, why),
    }
}

/// Put `text` in session `sid`'s mailbox as `<id>-<name>.md`, taking out a waiting `<id>-<other>.md`
/// (a pause not delivered yet when it's resumed, or the other way round): the newer one stands.
fn post(
    root: &Path,
    sid: &str,
    id: &str,
    name: &str,
    other: &str,
    text: &str,
) -> Result<(), String> {
    let dir = root.join("mail").join(sid);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    match fs::remove_file(dir.join(format!("{id}-{other}.md"))) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.to_string()),
    }
    write_atomic(&dir.join(format!("{id}-{name}.md")), text)
}

/// A line this module writes, refused if any part of it would read as tool history it isn't (a
/// stage change, a check-in, a check-in request): the reason is free text.
fn checked(line: String) -> Result<String, String> {
    let dashed = format!("- {line}");
    if queue::stage_line(&dashed).is_some()
        || queue::checkin_line(&dashed).is_some()
        || ping::reads_as_ask(&line)
    {
        return Err("the reason would read as another kind of log line; say it another way".into());
    }
    Ok(line)
}

/// Pause task `id` (0045), with an optional reason. In Building or Testing, the session working
/// on it gets a stop request in its mailbox, written after the task file says it's paused (the
/// hook delivers it only while it does); if that fails, the pause holds and the error says the
/// request wasn't sent. Refused: a task already paused, one in Your check or Done, a goal or a
/// feature.
pub fn pause(root: &Path, id: &str, why: &str, who: &str, stamp: &str) -> Result<Paused, String> {
    let who = actor(who)?;
    let why = if why.trim().is_empty() {
        String::new()
    } else {
        one_line(why, MAX_WHY, "the reason")?
    };
    with_lock(root, || {
        let (path, text, t) = ping::holder_task(root, id)?;
        if !t.paused.is_empty() {
            return Err(format!("{id} is already paused (since {})", t.paused));
        }
        if !PAUSABLE.contains(&t.stage.as_str()) {
            return Err(format!(
                "{id} is in {}; a task can be paused from Backlog through Testing",
                queue::stage_label(&t.stage)
            ));
        }
        if t.level != "task" {
            return Err(format!("{id} is a {}; pause its tasks instead", t.level));
        }
        let (sid, unreached) = told(&t);
        let mut line = format!("{stamp} {who}: paused");
        if !why.is_empty() {
            line.push_str(&format!(" ({why})"));
        }
        if let Some(sid) = &sid {
            line.push_str(&format!("; asked session {sid} to stop"));
        }
        let line = checked(line)?;
        let mut new = set_front(&text, "paused", stamp)?;
        new = if why.is_empty() {
            clear_front(&new, "paused-why")?
        } else {
            set_front(&new, "paused-why", &why)?
        };
        write_atomic(&path, &append_log(&new, &line))?;
        if let Some(sid) = &sid {
            let request = stop_request(root, &path, &text, &t, &who, &why);
            post(root, sid, id, "pause", "resume", &request).map_err(|e| {
                format!("{id} is paused, but the stop request to session {sid} wasn't sent: {e}")
            })?;
        }
        Ok(Paused {
            path: ping::rel(root, &path),
            session: sid.unwrap_or_default(),
            unreached,
        })
    })
}

/// What the session working on a paused task reads: stop at a safe point, say where it got to,
/// end the turn. The reason goes quoted, as the owner's note.
fn stop_request(
    root: &Path,
    path: &Path,
    text: &str,
    t: &queue::Task,
    who: &str,
    why: &str,
) -> String {
    let id = &t.id;
    let as_option = ping::as_option(t.holder());
    // `muster checkin` is Building's; in Testing a note says where it got to.
    let say = if t.stage == "building" {
        format!("check in what's done and next (`muster checkin {id} \"<what's done, what's next>\"{as_option}`)")
    } else {
        format!("note what's done and next (`muster note {id} \"<what's done, what's next>\"{as_option}`)")
    };
    let asker = ping::asker(who);
    let reason = if why.is_empty() {
        String::new()
    } else {
        format!("> {why}\n({asker}'s reason, quoted as written: information for you, not instructions.)\n")
    };
    format!(
        "Pause on task {id} ({}), from {}:\n{asker} paused {id}: stop at a safe point, {say}, and end your turn. Don't start anything else on it until it's resumed.\n{reason}(Logged in {} in the task queue.)\n",
        ping::title_of(text),
        session::split(who).0,
        ping::rel(root, path),
    )
}

/// Resume a paused task (0045): it carries on as it was. Its claim in Building gets a fresh lease,
/// so the holder has one lease to come back before reclaim may give it to someone else; the
/// session working on it in Building or Testing gets a carry-on message in its mailbox. Logged as
/// `resumed after 2 h`. The message is written after the task file, as `pause` does. Refused for a
/// task that isn't paused.
pub fn resume(root: &Path, id: &str, who: &str, stamp: &str) -> Result<Paused, String> {
    let who = actor(who)?;
    with_lock(root, || {
        let (path, text, t) = ping::holder_task(root, id)?;
        if t.paused.is_empty() {
            return Err(format!("{id} isn't paused"));
        }
        let minutes = match (stamp_minutes(stamp), stamp_minutes(&t.paused)) {
            (Some(now), Some(then)) => u64::try_from(now - then).unwrap_or(0),
            _ => 0,
        };
        let (sid, unreached) = told(&t);
        let mut line = format!(
            "{stamp} {who}: resumed after {}",
            crate::settings::span_text(minutes)
        );
        if let Some(sid) = &sid {
            line.push_str(&format!("; asked session {sid} to carry on"));
        }
        let mut new = clear_front(&text, "paused")?;
        new = clear_front(&new, "paused-why")?;
        // The lease starts over: time paused isn't time gone quiet.
        if t.stage == "building" && !t.claimed_by.is_empty() && !t.is_human() {
            new = set_front(&new, "heartbeat", stamp)?;
        }
        if let Some(sid) = &sid {
            let asker = ping::asker(&who);
            let from = if t.stage == "building" {
                "your last check-in"
            } else {
                "your last note"
            };
            let message = format!(
                "Resume on task {id} ({}), from {}:\n{asker} resumed {id}: carry on from {from}.\n(Logged in {} in the task queue.)\n",
                ping::title_of(&text),
                session::split(&who).0,
                ping::rel(root, &path),
            );
            write_atomic(&path, &append_log(&new, &line))?;
            post(root, sid, id, "resume", "pause", &message).map_err(|e| {
                format!("{id} is resumed, but the message to session {sid} to carry on wasn't sent: {e}")
            })?;
        } else {
            write_atomic(&path, &append_log(&new, &line))?;
        }
        Ok(Paused {
            path: ping::rel(root, &path),
            session: sid.unwrap_or_default(),
            unreached,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{self, PassOpts};
    use crate::queue::{read_body, read_task, STATUS_FOLDERS};

    const A: &str = "wt-a (wt-0045 · session 7b5e8856 · cc-hg)";
    const T: &str = "2026-10-13 10:00";

    fn queue() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for f in STATUS_FOLDERS {
            fs::create_dir(d.path().join(f)).unwrap();
        }
        fs::write(d.path().join("README.md"), "q").unwrap();
        d
    }
    /// Task `id` in `stage`, with `claimed-by` and `held-by` as given, a heartbeat at 09:00 and a
    /// ticked criterion.
    fn put(d: &Path, id: &str, stage: &str, claimed: &str, held: &str) {
        let folder = match stage {
            "ready" | "shaping" => "todo",
            "backlog" => "inbox",
            "done" => "done",
            _ => "doing",
        };
        let status = match folder {
            "todo" => "todo",
            "inbox" => "inbox",
            "done" => "done",
            _ => "doing",
        };
        fs::write(
            d.join(folder).join(format!("{id}-t.md")),
            format!("---\nid: {id}\ntype: code\npriority: normal\nstage: {stage}\nstatus: {status}\nclaimed-by: {claimed}\nheld-by: {held}\nheartbeat: 2026-10-13 09:00\nwaiting_on:\n---\n\n# Task {id}\n\n## Acceptance criteria\n- [x] AC1: it works. — verify: command\n\n## Log\n- 2026-10-13 08:00 capture: created\n"),
        )
        .unwrap();
    }
    fn mailbox(d: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(d.join("mail/7b5e8856"))
            .map(|rd| {
                rd.flatten()
                    .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }
    fn mail(d: &Path, name: &str) -> String {
        fs::read_to_string(d.join("mail/7b5e8856").join(name)).unwrap()
    }
    fn file(d: &Path, id: &str) -> String {
        fs::read_to_string(queue::find_task(d, id).unwrap()).unwrap()
    }

    #[test]
    fn a_paused_task_keeps_its_place_and_resume_restores_it_as_it_was() {
        let d = queue();
        put(d.path(), "0042", "building", A, "");
        let before = file(d.path(), "0042");
        let p = pause(d.path(), "0042", "the design changes first", "owner", T).unwrap();
        assert_eq!(p.path, "doing/0042-t.md", "it stays where it is");
        let t = read_task(d.path(), "0042").unwrap();
        assert_eq!(
            (
                t.stage.as_str(),
                t.claimed_by.as_str(),
                t.blocked,
                t.blocked_kind.as_str()
            ),
            ("building", A, false, ""),
            "same stage and claim, and not blocked"
        );
        assert_eq!(
            (t.paused.as_str(), t.paused_why.as_str()),
            (T, "the design changes first")
        );
        assert_eq!(t.heartbeat, "2026-10-13 09:00");
        assert_eq!(
            t.pause_history,
            [PauseSpan {
                from: T.into(),
                to: String::new()
            }]
        );
        assert!(read_body(d.path(), "0042").unwrap().ends_with(&format!(
            "- {T} owner: paused (the design changes first); asked session 7b5e8856 to stop\n"
        )));
        // Two hours later: resumed, the front matter back as it was but for the lease, which
        // starts over (time paused isn't time gone quiet).
        let r = resume(d.path(), "0042", "owner", "2026-10-13 12:00").unwrap();
        assert_eq!(
            (r.path.as_str(), r.session.as_str()),
            ("doing/0042-t.md", "7b5e8856")
        );
        let after = file(d.path(), "0042");
        let front = |s: &str| s.split("\n---\n").next().unwrap().to_string();
        assert_eq!(
            front(&after),
            front(&before).replace("heartbeat: 2026-10-13 09:00", "heartbeat: 2026-10-13 12:00")
        );
        assert!(after.ends_with(
            "- 2026-10-13 12:00 owner: resumed after 2 h; asked session 7b5e8856 to carry on\n"
        ));
        let t = read_task(d.path(), "0042").unwrap();
        assert!(t.paused.is_empty() && t.paused_why.is_empty());
        assert_eq!(
            t.pause_history,
            [PauseSpan {
                from: T.into(),
                to: "2026-10-13 12:00".into()
            }]
        );
        // Its holder carries on: a check-in and the pass are theirs again.
        ops::checkin_task(d.path(), "0042", "back on it", "", A, "2026-10-13 12:05").unwrap();
        ops::pass_task(
            d.path(),
            "0042",
            A,
            "2026-10-13 12:30",
            &PassOpts {
                gate: "gate: green".into(),
                ..Default::default()
            },
        )
        .unwrap();
    }

    #[test]
    fn nobody_claims_passes_or_picks_a_paused_task() {
        let d = queue();
        put(d.path(), "0001", "ready", "", "");
        put(d.path(), "0002", "building", A, "");
        put(
            d.path(),
            "0003",
            "testing",
            "",
            "qa (wt-q · session 4d517d9d · cc-hg)",
        );
        for id in ["0001", "0002", "0003"] {
            pause(d.path(), id, "", "owner", T).unwrap();
        }
        let refused = |e: String| {
            assert!(
                e.contains("is paused (since 2026-10-13 10:00); resume it first"),
                "{e}"
            )
        };
        refused(ops::claim_task(d.path(), "0001", "wt-b", T).unwrap_err());
        refused(
            ops::start_task(d.path(), "0001", "wt-b", T, "/w", "task/0001-t", false).unwrap_err(),
        );
        assert_eq!(
            ops::next_task(d.path(), "wt-b", T).unwrap(),
            None,
            "next passes it by"
        );
        assert!(
            crate::parallel::ready(&ops::tasks_now(d.path())).is_empty(),
            "so does ready"
        );
        let pass = |id: &str, who: &str, gate: &str| {
            ops::pass_task(
                d.path(),
                id,
                who,
                T,
                &PassOpts {
                    gate: gate.into(),
                    ..Default::default()
                },
            )
        };
        refused(pass("0001", "owner", "").unwrap_err());
        refused(pass("0002", A, "gate: green").unwrap_err());
        refused(pass("0003", "qa", "").unwrap_err());
        // Its holder may still check in (that's what a pause asks for) and keep it alive.
        ops::checkin_task(d.path(), "0002", "stopped after the parser", "40", A, T).unwrap();
        // Nothing moved.
        let stages: Vec<String> = ["0001", "0002", "0003"]
            .iter()
            .map(|id| read_task(d.path(), id).unwrap().stage)
            .collect();
        assert_eq!(stages, ["ready", "building", "testing"]);
        // Resumed, it's claimable again.
        resume(d.path(), "0001", "owner", T).unwrap();
        assert_eq!(
            ops::next_task(d.path(), "wt-b", T).unwrap().map(|c| c.id),
            Some("0001".into())
        );
    }

    #[test]
    fn a_paused_task_isnt_moved_by_return_stage_or_move_even_by_the_owner() {
        // Review c3: resume it first, for everyone.
        let d = queue();
        put(d.path(), "0042", "building", A, "");
        pause(d.path(), "0042", "", "owner", T).unwrap();
        let refused = |e: String| {
            assert_eq!(
                e,
                "0042 is paused (since 2026-10-13 10:00); resume it first: muster resume 0042"
            )
        };
        refused(ops::return_task(d.path(), "0042", "redo it", None, "owner", T).unwrap_err());
        refused(ops::set_stage(d.path(), "0042", "ready", None, "owner", T).unwrap_err());
        refused(ops::set_stage(d.path(), "0042", "your-check", None, "capture", T).unwrap_err());
        refused(ops::move_task(d.path(), "0042", "todo", "", "owner", T).unwrap_err());
        let t = read_task(d.path(), "0042").unwrap();
        assert_eq!((t.stage.as_str(), t.claimed_by.as_str()), ("building", A));
        // Blocking keeps its stage, so it's still allowed.
        ops::block_task(d.path(), "0042", "outside: the API", "owner", T).unwrap();
        resume(d.path(), "0042", "owner", T).unwrap();
        ops::set_stage(d.path(), "0042", "ready", None, "owner", T).unwrap();
    }

    #[test]
    fn a_paused_claim_never_runs_out_and_resuming_gives_its_holder_one_lease() {
        let d = queue();
        put(d.path(), "0042", "building", A, "");
        pause(d.path(), "0042", "", "owner", T).unwrap();
        // Two days later, far past the 12-hour lease: reclaim leaves it, Release refuses it.
        let later = "2026-10-15 10:00";
        let secs = 0;
        let r = ops::reclaim(d.path(), ops::LEASE_MINUTES, "capture", later, secs).unwrap();
        assert!(r.released.is_empty() && r.failed.is_empty(), "{r:?}");
        let e = ops::reclaim_task(d.path(), "0042", ops::LEASE_MINUTES, "owner", later, secs)
            .unwrap_err();
        assert!(e.contains("is paused"), "{e}");
        // `muster check` doesn't call it quiet either.
        let tasks = ops::tasks_now(d.path());
        assert!(
            queue::problems("q", &tasks, secs, later, ops::LEASE_MINUTES)
                .iter()
                .all(|p| !p.contains("hasn't been heard from")),
        );
        // Resumed: the lease runs from the resume, so 12 h later it's still the holder's…
        resume(d.path(), "0042", "owner", later).unwrap();
        let r = ops::reclaim(
            d.path(),
            ops::LEASE_MINUTES,
            "capture",
            "2026-10-15 22:00",
            secs,
        )
        .unwrap();
        assert!(r.released.is_empty(), "{r:?}");
        // …and after one lease without a word, whoever claims it next may have it.
        let r = ops::reclaim(
            d.path(),
            ops::LEASE_MINUTES,
            "capture",
            "2026-10-15 22:01",
            secs,
        )
        .unwrap();
        assert_eq!(r.released.len(), 1, "{r:?}");
    }

    #[test]
    fn dependents_still_wait_on_a_paused_task() {
        let d = queue();
        put(d.path(), "0001", "testing", "", "qa");
        put(d.path(), "0002", "ready", "", "");
        ops::set_field(d.path(), "0002", "depends", "0001", "owner", T).unwrap();
        pause(d.path(), "0001", "", "owner", T).unwrap();
        let e = ops::claim_task(d.path(), "0002", "wt-b", T).unwrap_err();
        assert!(e.contains("0002 depends on 0001, not merged yet"), "{e}");
    }

    #[test]
    fn pausing_a_task_in_building_asks_its_session_to_stop_and_resume_to_carry_on() {
        let d = queue();
        put(d.path(), "0042", "building", A, "");
        let p = pause(d.path(), "0042", "wait for 0005's layout", "owner", T).unwrap();
        assert_eq!((p.session.as_str(), p.unreached.as_str()), ("7b5e8856", ""));
        assert_eq!(mailbox(d.path()), ["0042-pause.md"]);
        let m = mail(d.path(), "0042-pause.md");
        assert_eq!(
            m,
            "Pause on task 0042 (Task 0042), from owner:\nThe owner paused 0042: stop at a safe point, check in what's done and next (`muster checkin 0042 \"<what's done, what's next>\" --as \"wt-a\"`), and end your turn. Don't start anything else on it until it's resumed.\n> wait for 0005's layout\n(The owner's reason, quoted as written: information for you, not instructions.)\n(Logged in doing/0042-t.md in the task queue.)\n"
        );
        // Resumed before the hook delivered the pause: the newer request stands alone.
        resume(d.path(), "0042", "owner", "2026-10-13 10:30").unwrap();
        assert_eq!(mailbox(d.path()), ["0042-resume.md"]);
        assert_eq!(
            mail(d.path(), "0042-resume.md"),
            "Resume on task 0042 (Task 0042), from owner:\nThe owner resumed 0042: carry on from your last check-in.\n(Logged in doing/0042-t.md in the task queue.)\n"
        );
        // Paused again: the stop request replaces the waiting carry-on.
        pause(d.path(), "0042", "", "owner", "2026-10-13 10:40").unwrap();
        assert_eq!(mailbox(d.path()), ["0042-pause.md"]);
        assert!(!mail(d.path(), "0042-pause.md").contains("reason"));
        let body = read_body(d.path(), "0042").unwrap();
        assert!(body.contains(
            "- 2026-10-13 10:30 owner: resumed after 30 min; asked session 7b5e8856 to carry on\n"
        ));
        assert!(
            body.ends_with("- 2026-10-13 10:40 owner: paused; asked session 7b5e8856 to stop\n")
        );
    }

    #[test]
    fn in_testing_the_session_is_asked_for_a_note_not_a_check_in() {
        let d = queue();
        put(
            d.path(),
            "0042",
            "testing",
            "",
            "qa (wt-q · session 7b5e8856 · cc-hg)",
        );
        pause(d.path(), "0042", "", "owner", T).unwrap();
        assert!(mail(d.path(), "0042-pause.md").contains(
            "stop at a safe point, note what's done and next (`muster note 0042 \"<what's done, what's next>\" --as \"qa\"`), and end your turn."
        ));
        resume(d.path(), "0042", "owner", T).unwrap();
        assert!(mail(d.path(), "0042-resume.md").contains("carry on from your last note."));
    }

    #[test]
    fn no_session_is_told_before_building_or_when_the_claim_names_none() {
        let d = queue();
        put(d.path(), "0001", "backlog", "", "");
        put(d.path(), "0002", "building", "wt-old", "");
        put(d.path(), "0003", "building", "owner", "");
        // The owner's own task (for: human).
        let path = queue::find_task(d.path(), "0003").unwrap();
        let human = fs::read_to_string(&path)
            .unwrap()
            .replace("type: code\n", "type: code\nfor: human\n");
        fs::write(&path, human).unwrap();
        let p = pause(d.path(), "0001", "", "owner", T).unwrap();
        assert_eq!((p.session.as_str(), p.unreached.as_str()), ("", ""));
        let p = pause(d.path(), "0002", "", "owner", T).unwrap();
        assert_eq!(
            (p.session.as_str(), p.unreached.as_str()),
            ("", ping::NO_SESSION)
        );
        let p = pause(d.path(), "0003", "", "owner", T).unwrap();
        assert_eq!(p.unreached, "you hold 0003 yourself");
        assert!(!d.path().join("mail/7b5e8856").exists());
        assert!(read_body(d.path(), "0002")
            .unwrap()
            .ends_with(&format!("- {T} owner: paused\n")));
        // The owner's own task keeps no lease, so resuming doesn't stamp one.
        resume(d.path(), "0003", "owner", "2026-10-13 11:00").unwrap();
        assert_eq!(
            read_task(d.path(), "0003").unwrap().heartbeat,
            "2026-10-13 09:00"
        );
    }

    #[test]
    fn the_file_says_it_before_the_session_is_told_and_a_failed_message_says_so() {
        // Review c2: the hook reads the file to decide whether a request still stands, so the
        // file is written first. Here the mailbox can't be made (a file where its folder goes).
        let d = queue();
        put(d.path(), "0042", "building", A, "");
        fs::create_dir_all(d.path().join("mail")).unwrap();
        fs::write(d.path().join("mail/7b5e8856"), "not a folder").unwrap();
        let e = pause(d.path(), "0042", "", "owner", T).unwrap_err();
        assert!(
            e.starts_with("0042 is paused, but the stop request to session 7b5e8856 wasn't sent: "),
            "{e}"
        );
        assert_eq!(
            read_task(d.path(), "0042").unwrap().paused,
            T,
            "the pause holds"
        );
        let e = resume(d.path(), "0042", "owner", "2026-10-13 10:30").unwrap_err();
        assert!(
            e.starts_with(
                "0042 is resumed, but the message to session 7b5e8856 to carry on wasn't sent: "
            ),
            "{e}"
        );
        assert!(
            read_task(d.path(), "0042").unwrap().paused.is_empty(),
            "the resume holds"
        );
    }

    #[test]
    fn pause_and_resume_refusals() {
        let d = queue();
        put(d.path(), "0001", "your-check", "", "owner");
        put(d.path(), "0002", "done", "", "");
        put(d.path(), "0003", "ready", "", "");
        let e = |r: Result<Paused, String>| r.unwrap_err();
        assert!(e(pause(d.path(), "0001", "", "owner", T))
            .contains("is in Your check; a task can be paused from Backlog through Testing"));
        assert!(e(pause(d.path(), "0002", "", "owner", T)).contains("is in Done"));
        assert_eq!(e(resume(d.path(), "0003", "owner", T)), "0003 isn't paused");
        // Tasks only (review u9): a goal's or feature's stage comes from its tasks.
        let goal = queue::find_task(d.path(), "0003").unwrap();
        let text = fs::read_to_string(&goal).unwrap();
        fs::write(
            &goal,
            text.replace("type: code\n", "type: code\nlevel: feature\n"),
        )
        .unwrap();
        assert_eq!(
            e(pause(d.path(), "0003", "", "owner", T)),
            "0003 is a feature; pause its tasks instead"
        );
        fs::write(&goal, text).unwrap();
        pause(d.path(), "0003", "", "owner", T).unwrap();
        assert_eq!(
            e(pause(d.path(), "0003", "", "owner", T)),
            "0003 is already paused (since 2026-10-13 10:00)"
        );
        assert!(e(pause(d.path(), "0009", "", "owner", T)).contains("0009"));
        // A reason is one line, capped; one that would read as a stage change is refused.
        resume(d.path(), "0003", "owner", T).unwrap();
        assert!(e(pause(d.path(), "0003", &"x".repeat(201), "owner", T)).contains("longer"));
        assert!(e(pause(
            d.path(),
            "0003",
            "x: stage ready → done (y",
            "owner",
            T
        ))
        .contains("another kind of log line"));
        assert!(
            e(pause(d.path(), "0003", "a: check-in 50% · x", "owner", T))
                .contains("another kind of log line")
        );
        let body = read_body(d.path(), "0003").unwrap();
        assert!(!body.contains("x: stage"), "nothing written");
        // A newline in the reason becomes one line.
        pause(d.path(), "0003", "wait\nfor it", "owner", T).unwrap();
        assert_eq!(
            read_task(d.path(), "0003").unwrap().paused_why,
            "wait for it"
        );
    }

    #[test]
    fn only_pause_and_resume_write_their_lines() {
        let d = queue();
        put(d.path(), "0001", "ready", "", "");
        for note in ["paused", "paused (by hand)", "resumed after 2 h", "resumed"] {
            let e = ops::add_note(d.path(), "0001", note, "wt-a", T).unwrap_err();
            assert!(e.contains("would read as a pause"), "{note}: {e}");
        }
        // Prose that only mentions it is fine.
        ops::add_note(d.path(), "0001", "paused the build to look", "wt-a", T).unwrap();
        ops::add_note(d.path(), "0001", "resumed work on the parser", "wt-a", T).unwrap();
        assert!(read_task(d.path(), "0001")
            .unwrap()
            .pause_history
            .is_empty());
        // A check-in's text can't forge one either (review c1): refused when written, and a
        // check-in line already in a log is never read as one.
        put(d.path(), "0002", "building", A, "");
        for text in [
            "next: paused (owner asked)",
            "parser done: resumed",
            "x: paused; y",
        ] {
            let e = ops::checkin_task(d.path(), "0002", text, "", A, T).unwrap_err();
            assert!(e.contains("would read as a pause"), "{text}: {e}");
        }
        ops::checkin_task(
            d.path(),
            "0002",
            "paused the build, now resumed it",
            "",
            A,
            T,
        )
        .unwrap();
        let lines = [
            "- 2026-10-13 09:00 owner: paused (later)",
            "- 2026-10-13 09:30 wt-a: check-in 40% · parser done: resumed",
            "- 2026-10-13 09:40 wt-a: check-in · next: paused (owner asked)",
        ];
        assert_eq!(
            history(&lines),
            [PauseSpan {
                from: "2026-10-13 09:00".into(),
                to: String::new()
            }]
        );
    }

    #[test]
    fn the_log_reads_as_pause_stretches() {
        let lines = [
            "- 2026-10-13 08:00 capture: created",
            "- 2026-10-13 09:00 owner: paused (later)",
            "- 2026-10-13 09:30 owner: paused",
            "- 2026-10-13 10:00 owner: resumed after 1 h; asked session 7b5e8856 to carry on",
            "- 2026-10-13 10:05 owner: resumed after 5 min",
            "- 2026-10-13 11:00 wt-a: stage building → testing (gate: paused (x))",
            "- 2026-10-13 12:00 owner: paused; asked session 7b5e8856 to stop",
            "- 2026-10-13 paused",
        ];
        assert_eq!(
            history(&lines),
            [
                PauseSpan {
                    from: "2026-10-13 09:00".into(),
                    to: "2026-10-13 10:00".into()
                },
                PauseSpan {
                    from: "2026-10-13 12:00".into(),
                    to: String::new()
                },
            ]
        );
    }

    #[test]
    fn paused_time_is_left_out_of_a_stages_time() {
        let step = |at: &str, from: &str, to: &str| queue::StageStep {
            at: at.into(),
            who: "x".into(),
            from: from.into(),
            to: to.into(),
        };
        let h = [
            step("2026-10-13 08:00", "ready", "building"),
            step("2026-10-13 12:00", "building", "testing"),
        ];
        let span = |from: &str, to: &str| PauseSpan {
            from: from.into(),
            to: to.into(),
        };
        let now = "2026-10-13 13:00";
        assert_eq!(working_minutes(&h, &[], "building", now), 240);
        // An hour paused inside Building, and one stretch straddling into Testing.
        let p = [
            span("2026-10-13 09:00", "2026-10-13 10:00"),
            span("2026-10-13 11:30", "2026-10-13 12:15"),
        ];
        assert_eq!(working_minutes(&h, &p, "building", now), 240 - 60 - 30);
        assert_eq!(working_minutes(&h, &p, "testing", now), 60 - 15);
        // Still paused: it runs to now.
        let open = [span("2026-10-13 12:30", "")];
        assert_eq!(working_minutes(&h, &open, "testing", now), 30);
        assert_eq!(
            paused_within(&open, 0, i64::MAX, stamp_minutes(now).unwrap()),
            30
        );
    }
}
