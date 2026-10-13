//! Who worked on a task and what it cost (task 0025): the sessions its claim and log name, what
//! each one's transcript says (`transcripts`), totals against the estimates, and a progress guess.
//! Only counts, ids and times leave this module; transcript text goes only to the owner's pane.

use crate::queue::{self, Task};
use crate::transcripts::{self, Reader, Tokens, Usage};
use serde::Serialize;
use std::path::Path;

/// A session that worked on a task, from the task file.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionRef {
    /// The short id (8 hex characters).
    pub id: String,
    /// The label it acted under, without the session part ("muster/wt-0025").
    pub who: String,
    /// The stages it held, in order: the stage it claimed, or the one it passed on or sent back.
    pub stages: Vec<String>,
    /// It holds the task now: its claim, or `held-by` in Testing and Your check (`Task::holder`).
    pub holds: bool,
}

/// Session ids mentioned as "session <8 hex>" in `text`, in order, each once. Both claim stamps
/// read: today's `label (session 7b5e8856)` and 0030's `label (checkout · session 7b5e8856 ·
/// cc-hg)`. A full id (`7b5e8856-c9c8-…`) counts by its first 8 characters.
pub fn mentions(text: &str) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    let mut out: Vec<String> = Vec::new();
    for (i, _) in lower.match_indices("session ") {
        let starts_word = lower[..i]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let Some(id) = lower.get(i + 8..i + 16) else {
            continue;
        };
        let ends_word = lower
            .get(i + 16..)
            .and_then(|rest| rest.chars().next())
            .is_none_or(|c| !c.is_alphanumeric());
        if starts_word && ends_word && transcripts::is_short_id(id) && !out.iter().any(|o| o == id)
        {
            out.push(id.to_string());
        }
    }
    out
}

/// A log line's who: `- YYYY-MM-DD HH:MM <who>: <what>`.
fn log_who(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("- ")?;
    crate::ops::stamp_minutes(rest.get(..16)?)?;
    let (who, _) = rest.get(16..)?.strip_prefix(' ')?.split_once(": ")?;
    Some(who)
}

/// The label someone acted under, without its session part: the text before the first `(` or
/// `,` ("capture (drover/main, session 349bc9c3)" → "capture").
fn label(who: &str) -> String {
    who.split(['(', ','])
        .next()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .unwrap_or(who.trim())
        .to_string()
}

/// Every session that worked on a task: named in its claim or `held-by` (`named`), or in the who
/// of a log line.
pub fn session_ids(named: &[&str], body: &str) -> Vec<String> {
    sessions(named, "", body)
        .into_iter()
        .map(|s| s.id)
        .collect()
}

/// The task's sessions with what the file says about each, in the order they first show up in
/// the log; a session that only `named` (its claim and `held-by`) names comes last. `holder` is
/// who holds it in its stage (`Task::holder`: the claim, or `held-by` in Testing and Your check),
/// and its session is marked as holding it. Only the who of a log line counts (a note that
/// mentions another session doesn't make it this task's), and only the Log section the tool
/// writes (`queue::log_section`).
pub fn sessions(named: &[&str], holder: &str, body: &str) -> Vec<SessionRef> {
    let mut out: Vec<SessionRef> = Vec::new();
    for line in queue::log_section(body).unwrap_or("").lines() {
        let Some(who) = log_who(line) else { continue };
        // The stage it held: the one it claimed, else the one it passed on, sent back or set.
        let held = queue::stage_line(line).map(|(step, detail)| {
            if detail == "claimed" {
                step.to
            } else {
                step.from
            }
        });
        for id in mentions(who) {
            let i = match out.iter().position(|s| s.id == id) {
                Some(i) => i,
                None => {
                    out.push(SessionRef {
                        id,
                        who: label(who),
                        stages: Vec::new(),
                        holds: false,
                    });
                    out.len() - 1
                }
            };
            if let Some(stage) = &held {
                if !out[i].stages.contains(stage) {
                    out[i].stages.push(stage.clone());
                }
            }
        }
    }
    for who in named.iter().copied().chain([holder]) {
        for id in mentions(who) {
            if !out.iter().any(|s| s.id == id) {
                out.push(SessionRef {
                    id,
                    who: label(who),
                    stages: Vec::new(),
                    holds: false,
                });
            }
        }
    }
    let holding = mentions(holder);
    for s in out.iter_mut() {
        s.holds = holding.contains(&s.id);
    }
    out
}

/// How far along a task looks, from its stage and ticked criteria: a guess, and labelled so.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Guess {
    pub pct: u32,
    /// What it's based on, in words.
    pub basis: String,
}

/// The guess: the share of criteria ticked, kept within what the stage says (Building 0–90%,
/// Testing 80–95%, Your check 90–99%, Done 100%); before Building it hasn't started (0%).
/// Without criteria, the stage alone: Building 50%, Testing 85%, Your check 95%. `criteria` is
/// (items, ticked).
pub fn progress_guess(stage: &str, criteria: (usize, usize)) -> Guess {
    let name = queue::stage_label(stage);
    let (lo, hi, mid) = match stage {
        "building" => (0, 90, 50),
        "testing" => (80, 95, 85),
        "your-check" => (90, 99, 95),
        "done" => (100, 100, 100),
        _ => {
            let name = if name.is_empty() { stage } else { name };
            return Guess {
                pct: 0,
                basis: format!("not started ({name})"),
            };
        }
    };
    let (items, ticked) = criteria;
    if items == 0 {
        return Guess {
            pct: mid,
            basis: format!("no criteria to count, so from the stage ({name})"),
        };
    }
    let share = pct(ticked.min(items) as u64, items as u64).unwrap_or(0);
    Guess {
        pct: share.clamp(lo, hi),
        basis: format!("{ticked} of {items} criteria ticked, in {name}"),
    }
}

/// One session's row in the Work section.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    #[serde(flatten)]
    pub session: SessionRef,
    #[serde(flatten)]
    pub usage: Usage,
    /// How many other tasks in the queue name this session. Its tokens and minutes count in full
    /// on each of them, so the window says so on the row and the totals.
    pub shared_with: usize,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub sessions: usize,
    /// Sessions whose transcript was read on this Mac.
    pub read: usize,
    pub live: usize,
    /// Rows whose session other tasks name too (see `Row::shared_with`).
    pub shared: usize,
    pub tokens: Tokens,
    pub total_tokens: u64,
    pub minutes: u64,
}

/// The estimates, and the totals as a share of each (None without an estimate).
#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct VsEstimate {
    pub minutes: u64,
    pub tokens: u64,
    pub minutes_pct: Option<u32>,
    pub tokens_pct: Option<u32>,
}

/// The Work section: who worked on the task, what it cost against the estimate, how far along.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Work {
    pub rows: Vec<Row>,
    pub totals: Totals,
    pub estimate: VsEstimate,
    pub guess: Guess,
}

/// `actual` as a whole percentage of `estimate`; None without an estimate.
pub fn pct(actual: u64, estimate: u64) -> Option<u32> {
    if estimate == 0 {
        return None;
    }
    let p = (u128::from(actual) * 100 + u128::from(estimate) / 2) / u128::from(estimate);
    Some(u32::try_from(p).unwrap_or(u32::MAX))
}

/// The Work section for `task` (whose text after the front matter is `body`), at `now`.
/// How many tasks other than `id` name each session, among `tasks`.
pub fn shared_counts(tasks: &[Task], id: &str) -> std::collections::HashMap<String, usize> {
    let mut out = std::collections::HashMap::new();
    for t in tasks.iter().filter(|t| t.id != id) {
        for s in &t.session_ids {
            *out.entry(s.clone()).or_insert(0) += 1;
        }
    }
    out
}

/// `offset` is local time minus UTC, in seconds (`utc_offset`): the log's stamps are local, the
/// transcripts' UTC.
pub fn work(
    reader: &Reader,
    task: &Task,
    body: &str,
    now: u64,
    shared: &std::collections::HashMap<String, usize>,
    offset: i64,
) -> Work {
    let refs = sessions(&[&task.claimed_by, &task.held_by], task.holder(), body);
    let ids: Vec<String> = refs.iter().map(|s| s.id.clone()).collect();
    let mut usages = reader.usages(&ids, now);
    for u in usages.iter_mut() {
        skip_paused(u, &task.pause_history, offset, now);
    }
    let sum = transcripts::actuals(&usages);
    let totals = Totals {
        sessions: refs.len(),
        read: sum.read,
        live: usages.iter().filter(|u| u.live).count(),
        shared: refs
            .iter()
            .filter(|r| shared.get(&r.id).copied().unwrap_or(0) > 0)
            .count(),
        tokens: sum.tokens,
        total_tokens: sum.tokens.total(),
        minutes: sum.minutes,
    };
    let estimate = VsEstimate {
        minutes: task.est_minutes,
        tokens: task.est_token_count,
        minutes_pct: pct(totals.minutes, task.est_minutes),
        tokens_pct: pct(totals.total_tokens, task.est_token_count),
    };
    let rows = refs
        .into_iter()
        .zip(usages)
        .map(|(session, usage)| {
            let shared_with = shared.get(&session.id).copied().unwrap_or(0);
            Row {
                session,
                usage,
                shared_with,
            }
        })
        .collect();
    Work {
        rows,
        totals,
        estimate,
        guess: progress_guess(&task.stage, queue::criteria(body)),
    }
}

/// Task `id` in the queue at `root`, parsed, with its text after the front matter.
fn read(root: &Path, id: &str) -> Result<(Task, String), String> {
    let text = queue::read_body(root, id)?;
    let path = queue::find_task(root, id)?;
    let task = queue::parse_task("", root, &queue::folder_of(root, &path), &path, &text, 0)
        .ok_or_else(|| format!("task {id} has no front matter"))?;
    let body = queue::front_matter(&text)
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();
    Ok((task, body))
}

/// The Work section of task `id` in the queue at `root`, read from disk now.
pub fn for_task(reader: &Reader, root: &Path, id: &str, now: u64) -> Result<Work, String> {
    let (task, body) = read(root, id)?;
    // Every task of the queue, to say which sessions this task shares with others.
    let all = queue::read_state(
        &[("q".into(), root.to_path_buf())],
        &mut queue::Cache::default(),
    )
    .tasks;
    Ok(work(
        reader,
        &task,
        &body,
        now,
        &shared_counts(&all, id),
        utc_offset(),
    ))
}

/// A session's minutes without the task's paused stretches (0045): what of them falls between its
/// first and last stamp. Its tokens stay as they are: a paused session spends none. `offset` as
/// `work` takes it.
fn skip_paused(u: &mut Usage, pauses: &[crate::pause::PauseSpan], offset: i64, now: u64) {
    let (Some(start), Some(end)) = (u.start, u.end) else {
        return;
    };
    let local = |secs: u64| {
        (i64::try_from(secs)
            .unwrap_or(i64::MAX)
            .saturating_add(offset))
            / 60
    };
    let paused = crate::pause::paused_within(pauses, local(start), local(end), local(now));
    u.minutes = u.minutes.saturating_sub(u64::try_from(paused).unwrap_or(0));
}

/// This Mac's local time minus UTC, in seconds, from the clock (`date`, as the log's stamps are
/// made), to the nearest quarter hour, which every time zone is a multiple of.
fn utc_offset() -> i64 {
    let now = i64::try_from(queue::now_secs()).unwrap_or(0);
    let Some(local) = crate::ops::stamp_minutes(&queue::local_stamp()) else {
        return 0;
    };
    let quarter = 15 * 60;
    let diff = local * 60 - now;
    (diff + quarter / 2).div_euclid(quarter) * quarter
}

/// Session `session`'s transcript as Markdown, only for a session that worked on task `id`: the
/// window can't read any other transcript through this.
pub fn transcript(reader: &Reader, root: &Path, id: &str, session: &str) -> Result<String, String> {
    if !transcripts::is_short_id(session) {
        return Err("bad session id".into());
    }
    let (task, _) = read(root, id)?;
    if !task.session_ids.iter().any(|s| s == session) {
        return Err(format!("session {session} didn't work on task {id}"));
    }
    reader.transcript_markdown(session)
}

/// Mark the tasks that have a live session, for the card's dot. Done tasks are skipped, so the
/// cost follows the open work, not the queue's history. One index refresh, then a few file
/// times per session; no transcript is read.
pub fn mark_live(tasks: &mut [Task], reader: &Reader, now: u64) {
    let open = || tasks.iter().filter(|t| t.stage != "done");
    let ids: std::collections::HashSet<&str> = open()
        .flat_map(|t| t.session_ids.iter().map(String::as_str))
        .collect();
    let live = reader.live_set(ids, now);
    for t in tasks.iter_mut() {
        t.live = t.stage != "done" && t.session_ids.iter().any(|s| live.contains(s));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcripts::tests::{home, prompt, reply, write, NOW};
    use crate::transcripts::{iso_secs, Found};
    use std::fs;
    use std::time::Instant;

    const A: &str = "7b5e8856-c9c8-45ec-aa03-c0ddbf1d4394";
    const B: &str = "4d517d9d-1173-481b-80cc-27e72f903f3c";

    fn queue() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for f in queue::STATUS_FOLDERS {
            fs::create_dir(d.path().join(f)).unwrap();
        }
        d
    }

    const TASK: &str = "---
id: 0025
stage: your-check
status: doing
est-time: 2h
est-tokens: 100k
claimed-by:
heartbeat:
waiting_on:
---

# Sessions and cost

## Acceptance criteria
- [x] AC1: one
- [x] AC2: two
- [ ] AC3: three — verify: owner
- [ ] AC4: four

## Log
- 2026-10-12 09:00 capture: created
- 2026-10-12 10:00 muster/wt-0025 (session 7b5e8856): stage ready → building (claimed)
- 2026-10-12 11:00 muster/wt-0025 (session 7b5e8856): found by session eeeeeeee, which isn't this task's
- 2026-10-12 12:00 main-29 · wt-0025 (checkout · session 7b5e8856 · cc-hg): stage building → testing (gate full: green)
- 2026-10-12 12:30 qa (checkout · session 4d517d9d · cc-mg): ticked AC2
- 2026-10-12 13:00 qa (checkout · session 4d517d9d · cc-mg): stage testing → your-check
";

    fn body(text: &str) -> &str {
        queue::front_matter(text).unwrap().1
    }

    /// Two sessions' transcripts: A built it (two replies, 30 minutes), B tested it (one reply,
    /// 10 minutes, still live at NOW).
    fn transcripts(home: &Path) {
        write(
            home,
            ".claude",
            A,
            &[
                prompt("2026-10-12T08:00:00Z", "build 0025"),
                reply("a1", "2026-10-12T08:01:00Z", 10, 2000, 30000, 5000),
                reply("a1", "2026-10-12T08:01:00Z", 10, 2000, 30000, 5000),
                reply("a2", "2026-10-12T08:30:00Z", 5, 1000, 20000, 0),
            ],
            NOW - 3600,
        );
        write(
            home,
            ".claude-mg",
            B,
            &[
                prompt("2026-10-12T10:50:00Z", "test 0025"),
                reply("b1", "2026-10-12T11:00:00Z", 1, 500, 10000, 1000),
            ],
            NOW - 60,
        );
    }

    #[test]
    fn finds_sessions_in_claims_and_in_who_of_log_lines() {
        // Today's claim and 0030's stamped one.
        assert_eq!(mentions("drover/main (session 3fba2222)"), ["3fba2222"]);
        assert_eq!(
            mentions("main-29 · wt-0019 (checkout · session 7b5e8856 · cc-hg)"),
            ["7b5e8856"]
        );
        assert_eq!(
            mentions("capture (drover/main, session 349bc9c3), Session 7B5E8856-c9c8 and session 349bc9c3"),
            ["349bc9c3", "7b5e8856"]
        );
        for none in [
            "",
            "session",
            "session 7b5e885",
            "session 7b5e8856a",
            "subsession 7b5e8856",
            "session 7b5e885g",
            "session  7b5e8856",
            "sessions 7b5e8856",
            "session ééééééé",
        ] {
            assert!(mentions(none).is_empty(), "{none:?}");
        }
        let refs = sessions(&[], "", body(TASK));
        assert_eq!(
            refs,
            [
                SessionRef {
                    id: "7b5e8856".into(),
                    who: "muster/wt-0025".into(),
                    stages: vec!["building".into()],
                    holds: false
                },
                SessionRef {
                    id: "4d517d9d".into(),
                    who: "qa".into(),
                    stages: vec!["testing".into()],
                    holds: false
                },
            ]
        );
        // A claim's session the log doesn't name comes last; one it does name is marked.
        let claim = "wt-b (checkout · session 0123abcd · cc-hg)";
        let held = sessions(&[claim], claim, body(TASK));
        assert_eq!(held.len(), 3);
        assert_eq!(
            held[2],
            SessionRef {
                id: "0123abcd".into(),
                who: "wt-b".into(),
                stages: vec![],
                holds: true
            }
        );
        let qa = "qa (session 4d517d9d)";
        assert!(sessions(&[qa], qa, body(TASK))[1].holds);
        // Named by the claim but not holding it (the holder is someone else): listed, not holding.
        let stale = sessions(&[claim, qa], qa, body(TASK));
        assert_eq!(
            stale
                .iter()
                .map(|s| (s.id.as_str(), s.holds))
                .collect::<Vec<_>>(),
            [("7b5e8856", false), ("4d517d9d", true), ("0123abcd", false)]
        );
        assert_eq!(session_ids(&[], body(TASK)), ["7b5e8856", "4d517d9d"]);
        assert_eq!(
            session_ids(&[claim, "owner"], body(TASK)),
            ["7b5e8856", "4d517d9d", "0123abcd"]
        );
        // A log line quoted in the description isn't the log.
        let quoted = "# T\n\n```\n- 2026-10-12 10:00 x (session 11111111): stage ready → building (claimed)\n```\n\n## Log\n- 2026-10-12 09:00 capture: created\n";
        assert!(session_ids(&[], quoted).is_empty());
    }

    #[test]
    fn the_guess_follows_ticked_criteria_and_the_stage() {
        let g = |stage, items, ticked| progress_guess(stage, (items, ticked)).pct;
        assert_eq!(g("building", 5, 3), 60);
        assert_eq!(g("building", 5, 5), 90);
        assert_eq!(g("building", 0, 0), 50);
        assert_eq!(g("testing", 5, 1), 80);
        assert_eq!(g("testing", 5, 5), 95);
        assert_eq!(g("testing", 0, 0), 85);
        assert_eq!(g("your-check", 4, 2), 90);
        assert_eq!(g("your-check", 4, 4), 99);
        assert_eq!(g("your-check", 0, 0), 95);
        assert_eq!(g("done", 4, 0), 100);
        for early in ["backlog", "shaping", "ready", "nonsense"] {
            assert_eq!(g(early, 4, 2), 0, "{early}");
        }
        assert_eq!(
            progress_guess("building", (5, 3)).basis,
            "3 of 5 criteria ticked, in Building"
        );
        assert_eq!(
            progress_guess("testing", (0, 0)).basis,
            "no criteria to count, so from the stage (Testing)"
        );
        assert_eq!(progress_guess("ready", (5, 3)).basis, "not started (Ready)");
    }

    #[test]
    fn percent_of_an_estimate() {
        assert_eq!(pct(60, 100), Some(60));
        assert_eq!(pct(1, 3), Some(33));
        assert_eq!(pct(2, 3), Some(67));
        assert_eq!(pct(250, 100), Some(250));
        assert_eq!(pct(5, 0), None);
        assert_eq!(pct(u64::MAX, 1), Some(u32::MAX));
    }

    #[test]
    fn a_session_other_tasks_also_name_is_marked_shared() {
        let q = queue();
        fs::write(q.path().join("doing/0025-sessions.md"), TASK).unwrap();
        let h = home();
        transcripts(h.path());
        let first = for_task(&Reader::new(h.path()), q.path(), "0025", NOW).unwrap();
        let id = first.rows[0].session.id.clone();
        assert!(first.rows.iter().all(|r| r.shared_with == 0));
        assert_eq!(first.totals.shared, 0);
        // Two other tasks claimed by the same session.
        for other in ["0026", "0027"] {
            let text = format!("---\nid: {other}\nstatus: done\nstage: done\nclaimed-by: capture (session {id})\n---\n\n# Other\n\n## Log\n");
            fs::write(q.path().join(format!("done/{other}-other.md")), text).unwrap();
        }
        let w = for_task(&Reader::new(h.path()), q.path(), "0025", NOW).unwrap();
        let row = w.rows.iter().find(|r| r.session.id == id).unwrap();
        assert_eq!(row.shared_with, 2);
        assert_eq!(w.totals.shared, 1);
        assert_eq!(serde_json::to_value(row).unwrap()["sharedWith"], 2);
    }

    #[test]
    fn two_sessions_each_with_stage_times_and_tokens_and_the_totals() {
        let q = queue();
        fs::write(q.path().join("doing/0025-sessions.md"), TASK).unwrap();
        let h = home();
        transcripts(h.path());
        let w = for_task(&Reader::new(h.path()), q.path(), "0025", NOW).unwrap();
        assert_eq!(w.rows.len(), 2);
        let (a, b) = (&w.rows[0], &w.rows[1]);
        assert_eq!(
            (a.session.id.as_str(), a.session.stages.as_slice()),
            ("7b5e8856", &["building".to_string()][..])
        );
        assert_eq!(a.usage.found, Found::Read);
        assert_eq!(a.usage.start, iso_secs("2026-10-12T08:00:00Z"));
        assert_eq!(a.usage.end, iso_secs("2026-10-12T08:30:00Z"));
        assert_eq!(a.usage.minutes, 30);
        assert!(!a.usage.live);
        assert_eq!(
            a.usage.tokens,
            Tokens {
                input: 15,
                output: 3000,
                cache_read: 50000,
                cache_write: 5000
            }
        );
        assert_eq!(
            (b.session.id.as_str(), b.session.who.as_str()),
            ("4d517d9d", "qa")
        );
        assert_eq!(b.usage.minutes, 10);
        assert!(b.usage.live);
        assert_eq!(b.usage.tokens.total(), 11501);
        // Totals: both sessions, against est-time 2h and est-tokens 100k.
        assert_eq!(
            w.totals,
            Totals {
                sessions: 2,
                read: 2,
                live: 1,
                shared: 0,
                tokens: Tokens {
                    input: 16,
                    output: 3500,
                    cache_read: 60000,
                    cache_write: 6000
                },
                total_tokens: 69516,
                minutes: 40,
            }
        );
        assert_eq!(
            w.estimate,
            VsEstimate {
                minutes: 120,
                tokens: 100_000,
                minutes_pct: Some(33),
                tokens_pct: Some(70)
            }
        );
        // Your check with 2 of 4 criteria ticked.
        assert_eq!(w.guess.pct, 90);
        // What the window gets: counts, ids and times; never a path.
        let json = serde_json::to_string(&w).unwrap();
        assert!(json.contains("\"id\":\"7b5e8856\"") && json.contains("\"cacheRead\":50000"));
        assert!(!json.contains(".claude") && !json.contains(h.path().to_str().unwrap()));
    }

    #[test]
    fn paused_time_is_left_out_of_a_sessions_minutes() {
        // 0045: A worked 08:00–08:30 UTC; the task was paused 08:10–08:20 local, at UTC+2 that
        // is 10:10–10:20 in the log. B (10:50–11:00 UTC) overlaps nothing.
        let text = TASK.replace(
            "- 2026-10-12 11:00 muster",
            "- 2026-10-12 10:10 owner: paused (lunch); asked session 7b5e8856 to stop\n- 2026-10-12 10:20 owner: resumed after 10 min\n- 2026-10-12 11:00 muster",
        );
        let task = queue::parse_task(
            "",
            Path::new("/q"),
            "doing",
            Path::new("/q/doing/0025-s.md"),
            &text,
            0,
        )
        .unwrap();
        let h = home();
        transcripts(h.path());
        let reader = Reader::new(h.path());
        let shared = std::collections::HashMap::new();
        let w = work(&reader, &task, body(&text), NOW, &shared, 2 * 3600);
        assert_eq!((w.rows[0].usage.minutes, w.rows[1].usage.minutes), (20, 10));
        assert_eq!(w.totals.minutes, 30);
        // Tokens aren't time: they stay what the transcript says.
        assert_eq!(w.totals.total_tokens, 69516);
        // In UTC the same stamps miss A's stretch entirely.
        let w = work(&reader, &task, body(&text), NOW, &shared, 0);
        assert_eq!(w.totals.minutes, 40);
        // Still paused: it runs to now, so A's whole tail after 08:10 is left out.
        let open = TASK.replace(
            "- 2026-10-12 11:00 muster",
            "- 2026-10-12 10:10 owner: paused\n- 2026-10-12 11:00 muster",
        );
        let task = queue::parse_task(
            "",
            Path::new("/q"),
            "doing",
            Path::new("/q/doing/0025-s.md"),
            &open,
            0,
        )
        .unwrap();
        let w = work(&reader, &task, body(&open), NOW, &shared, 2 * 3600);
        assert_eq!((w.rows[0].usage.minutes, w.rows[1].usage.minutes), (10, 0));
    }

    #[test]
    fn a_session_not_on_this_mac_still_has_its_row() {
        let q = queue();
        let text = TASK.replace("est-time: 2h\nest-tokens: 100k\n", "");
        fs::write(q.path().join("doing/0025-sessions.md"), text).unwrap();
        let h = home();
        write(
            h.path(),
            ".claude",
            A,
            &[reply("a1", "2026-10-12T08:01:00Z", 1, 1, 1, 1)],
            NOW - 3600,
        );
        let w = for_task(&Reader::new(h.path()), q.path(), "0025", NOW).unwrap();
        assert_eq!(w.rows[1].usage.found, Found::Missing);
        assert_eq!(
            (w.totals.sessions, w.totals.read, w.totals.total_tokens),
            (2, 1, 4)
        );
        assert_eq!(
            (w.estimate.minutes_pct, w.estimate.tokens_pct),
            (None, None)
        );
        // A task nobody has worked on yet.
        fs::write(
            q.path().join("todo/0026-new.md"),
            "---\nid: 0026\nstage: ready\nstatus: todo\n---\n\n# New\n\n## Log\n- 2026-10-12 09:00 capture: created\n",
        )
        .unwrap();
        let w = for_task(&Reader::new(h.path()), q.path(), "0026", NOW).unwrap();
        assert!(w.rows.is_empty());
        assert_eq!(w.totals, Totals::default());
        assert_eq!(w.guess.pct, 0);
    }

    #[test]
    fn the_transcript_is_only_for_a_session_of_that_task() {
        let q = queue();
        fs::write(q.path().join("doing/0025-sessions.md"), TASK).unwrap();
        let h = home();
        transcripts(h.path());
        write(
            h.path(),
            ".claude",
            "eeeeeeee-0000",
            &[prompt("2026-10-12T08:00:00Z", "other")],
            NOW,
        );
        let r = Reader::new(h.path());
        assert!(transcript(&r, q.path(), "0025", "7b5e8856")
            .unwrap()
            .contains("**You:** build 0025"));
        assert_eq!(
            transcript(&r, q.path(), "0025", "eeeeeeee").unwrap_err(),
            "session eeeeeeee didn't work on task 0025"
        );
        for bad in ["7B5E8856", "../../etc", "7b5e8856-c9c8", ""] {
            assert_eq!(
                transcript(&r, q.path(), "0025", bad).unwrap_err(),
                "bad session id",
                "{bad:?}"
            );
        }
        assert!(transcript(&r, q.path(), "25", "7b5e8856").is_err());
        assert!(for_task(&r, q.path(), "../0025", NOW).is_err());
    }

    fn parsed(id: &str, stage: &str, log_who: &str) -> Task {
        let text = format!(
            "---\nid: {id}\nstage: {stage}\nstatus: {}\n---\n\n# T\n\n## Log\n- 2026-10-12 10:00 {log_who}: stage ready → building (claimed)\n",
            queue::mirror_status(stage, false)
        );
        queue::parse_task(
            "p",
            Path::new("/q"),
            "",
            &Path::new("/q").join(format!("{id}-t.md")),
            &text,
            0,
        )
        .unwrap()
    }

    #[test]
    fn a_card_is_live_while_one_of_its_sessions_is() {
        let h = home();
        transcripts(h.path());
        let r = Reader::new(h.path());
        let mut tasks = vec![
            parsed("0001", "testing", "qa (session 4d517d9d)"),
            parsed("0002", "building", "wt (session 7b5e8856)"),
            parsed("0003", "done", "qa (session 4d517d9d)"),
            parsed("0004", "building", "wt (session 0000aaaa)"),
        ];
        assert_eq!(tasks[0].session_ids, ["4d517d9d"]);
        mark_live(&mut tasks, &r, NOW);
        let live: Vec<bool> = tasks.iter().map(|t| t.live).collect();
        assert_eq!(live, [true, false, false, false]);
        // Five minutes and a second later, nothing is.
        mark_live(&mut tasks, &r, NOW - 60 + transcripts::LIVE_SECS + 1);
        assert!(tasks.iter().all(|t| !t.live));
        // The window gets the flag.
        assert!(serde_json::to_string(&tasks[0])
            .unwrap()
            .contains("\"live\":false"));
    }

    #[test]
    fn the_session_holding_it_in_testing_is_its_holder_and_lights_the_card() {
        // 0030's `held-by`: the QA session holds it in Testing; the claim was released when it
        // left Building, and here no log line names that session.
        let text = "---\nid: 0001\nstage: testing\nstatus: doing\nclaimed-by:\nheld-by: qa (checkout · session 4d517d9d · cc-mg)\n---\n\n# T\n\n## Log\n- 2026-10-12 10:00 wt (session 7b5e8856): stage ready → building (claimed)\n";
        let q = queue();
        fs::write(q.path().join("doing/0001-t.md"), text).unwrap();
        let h = home();
        transcripts(h.path());
        let r = Reader::new(h.path());
        let w = for_task(&r, q.path(), "0001", NOW).unwrap();
        let held: Vec<(&str, &str, bool)> = w
            .rows
            .iter()
            .map(|row| {
                (
                    row.session.id.as_str(),
                    row.session.who.as_str(),
                    row.session.holds,
                )
            })
            .collect();
        assert_eq!(held, [("7b5e8856", "wt", false), ("4d517d9d", "qa", true)]);
        let mut tasks = queue::read_state(
            &[("q".into(), q.path().to_path_buf())],
            &mut queue::Cache::default(),
        )
        .tasks;
        assert_eq!(tasks[0].session_ids, ["7b5e8856", "4d517d9d"]);
        mark_live(&mut tasks, &r, NOW);
        assert!(tasks[0].live);
        assert!(transcript(&r, q.path(), "0001", "4d517d9d").is_ok());
    }

    /// 50 sessions' transcripts: summed off the main thread in the app, and the cards' live check,
    /// which runs on every state poll, stays cheap. Best of three runs, with generous bounds so
    /// a busy machine doesn't fail the gate.
    #[test]
    fn fifty_transcripts_are_read_quickly_and_the_live_check_is_cheap() {
        let h = home();
        let mut ids = Vec::new();
        let mut log = String::new();
        for n in 0..50u64 {
            let id = format!("{n:08x}");
            let lines: Vec<String> = (0..1000u64)
                .map(|i| reply(&format!("m{i}"), "2026-10-12T08:00:00Z", 1, 2, 3, 4))
                .collect();
            write(
                h.path(),
                if n % 2 == 0 { ".claude" } else { ".claude-mg" },
                &format!("{id}-0000"),
                &lines,
                NOW - 3600,
            );
            log.push_str(&format!(
                "- 2026-10-12 10:00 wt (session {id}): stage ready → building (claimed)\n"
            ));
            ids.push(id);
        }
        let best = |f: &dyn Fn()| {
            (0..3)
                .map(|_| {
                    let t = Instant::now();
                    f();
                    t.elapsed()
                })
                .min()
                .unwrap()
        };
        let cold = best(&|| {
            let r = Reader::new(h.path());
            let u = r.usages(&ids, NOW);
            assert!(u
                .iter()
                .all(|u| u.found == Found::Read && u.tokens.total() == 10_000));
        });
        let r = Reader::new(h.path());
        r.usages(&ids, NOW);
        let warm = best(&|| {
            r.usages(&ids, NOW);
        });
        let text =
            format!("---\nid: 0001\nstage: building\nstatus: doing\n---\n\n# T\n\n## Log\n{log}");
        let task = queue::parse_task(
            "p",
            Path::new("/q"),
            "",
            Path::new("/q/0001-t.md"),
            &text,
            0,
        )
        .unwrap();
        assert_eq!(task.session_ids.len(), 50);
        let mut tasks = vec![task; 20];
        let poll = best(&|| {
            let mut t = tasks.clone();
            mark_live(&mut t, &r, NOW);
        });
        mark_live(&mut tasks, &r, NOW);
        eprintln!("50 transcripts (1000 lines each): cold {cold:?}, warm {warm:?}; live check for 20 cards × 50 sessions {poll:?}");
        assert!(
            cold.as_secs_f64() < 5.0,
            "cold read of 50 transcripts took {cold:?}"
        );
        assert!(
            warm.as_millis() < 250,
            "warm read of 50 transcripts took {warm:?}"
        );
        assert!(
            poll.as_millis() < 250,
            "the cards' live check took {poll:?}"
        );
    }
}
