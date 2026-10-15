//! Who worked on a task and what it cost (task 0025): the sessions its claim and log name, what
//! each one's transcript says (`transcripts`), totals against the estimates, and a progress guess.
//! Only counts, ids and times leave this module; transcript text goes only to the owner's pane.
//!
//! Only a session whose stamp names an account Muster reads (0099, `accounts::allowed`) is handed
//! to the reader: one stamped with no account (bare `claude`, the company account, or a stamp
//! from before 0030) or with an account not on the list gets no tokens, no live dot and no
//! transcript, wherever its transcript is filed.

use crate::accounts::{self, Account};
use crate::queue::{self, Task};
use crate::transcripts::{self, Reader, Tokens, Usage};
use serde::Serialize;
use std::collections::HashSet;
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
    /// The role slot its stamps act as (0112, `coder-2`), so the window names it by the role's
    /// name now; "" when none does.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub slot: String,
    /// The Claude account its stamps name (`hg`), "" when none does or two name different ones:
    /// then it's never read. Not sent to the window.
    #[serde(skip)]
    pub account: String,
}

/// A session a task names, with the account its stamps name (`SessionRef::account`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SessionId {
    pub id: String,
    pub account: String,
}

impl SessionId {
    /// May its transcript be read: does its stamp name one of `allowed`?
    pub fn readable(&self, allowed: &[Account]) -> bool {
        accounts::is_allowed(&self.account, allowed)
    }
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

/// The role slot `who`'s stamp acts as for session `id` (0112): "" when `who` has no stamp, its
/// stamp no slot, or it's another session's.
fn stamp_slot(who: &str, id: &str) -> String {
    match crate::session::split(who.trim()).1 {
        Some(s) if s.id == id => s.slot,
        _ => String::new(),
    }
}

/// The account `who`'s stamp (`session::split`) names for session `id`: "" when `who` has no
/// stamp, its stamp has no account, or it's another session's.
fn stamp_account(who: &str, id: &str) -> String {
    match crate::session::split(who.trim()).1 {
        Some(s) if s.id == id => s.account,
        _ => String::new(),
    }
}

/// Add what one stamp says of `s`'s account: the first account named sticks, and a different one
/// later makes it "" for good (`clashed`), so a session isn't read on a disputed account.
fn note_account(s: &mut SessionRef, account: String, clashed: &mut HashSet<String>) {
    if account.is_empty() || clashed.contains(&s.id) {
        return;
    }
    if s.account.is_empty() {
        s.account = account;
    } else if s.account != account {
        s.account.clear();
        clashed.insert(s.id.clone());
    }
}

/// The slot a session's stamps act as: the latest one named (a session is one slot at a time).
fn note_slot(s: &mut SessionRef, slot: String) {
    if !slot.is_empty() {
        s.slot = slot;
    }
}

/// Where session `id` is in `out`, added there with `who`'s label when it isn't yet.
fn entry(out: &mut Vec<SessionRef>, id: String, who: &str) -> usize {
    if let Some(i) = out.iter().position(|s| s.id == id) {
        return i;
    }
    out.push(SessionRef {
        id,
        who: label(who),
        stages: Vec::new(),
        holds: false,
        slot: String::new(),
        account: String::new(),
    });
    out.len() - 1
}

/// Every session that worked on a task: named in its claim or `held-by` (`named`), or in the who
/// of a log line, with the account its stamps name.
pub fn session_ids(named: &[&str], body: &str) -> Vec<SessionId> {
    sessions(named, "", body)
        .into_iter()
        .map(|s| SessionId {
            id: s.id,
            account: s.account,
        })
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
    let mut clashed: HashSet<String> = HashSet::new();
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
            let account = stamp_account(who, &id);
            let slot = stamp_slot(who, &id);
            let i = entry(&mut out, id, who);
            note_account(&mut out[i], account, &mut clashed);
            note_slot(&mut out[i], slot);
            if let Some(stage) = &held {
                if !out[i].stages.contains(stage) {
                    out[i].stages.push(stage.clone());
                }
            }
        }
    }
    for who in named.iter().copied().chain([holder]) {
        for id in mentions(who) {
            let account = stamp_account(who, &id);
            let slot = stamp_slot(who, &id);
            let i = entry(&mut out, id, who);
            note_account(&mut out[i], account, &mut clashed);
            note_slot(&mut out[i], slot);
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
            *out.entry(s.id.clone()).or_insert(0) += 1;
        }
    }
    out
}

/// `to_epoch` turns the log's local stamps into seconds since 1970, as the transcripts' times are
/// (`queue::stamp_epoch`). Only the sessions stamped with one of `allowed` are read, in `allowed`;
/// the others keep their rows, with nothing read.
pub fn work(
    reader: &Reader,
    allowed: &[Account],
    task: &Task,
    body: &str,
    now: u64,
    shared: &std::collections::HashMap<String, usize>,
    to_epoch: &dyn Fn(&str) -> Option<i64>,
) -> Work {
    let refs = sessions(&[&task.claimed_by, &task.held_by], task.holder(), body);
    let readable = |s: &SessionRef| accounts::is_allowed(&s.account, allowed);
    let ids: Vec<String> = refs
        .iter()
        .filter(|s| readable(s))
        .map(|s| s.id.clone())
        .collect();
    let mut read = reader.usages(allowed, &ids, now).into_iter();
    let mut usages: Vec<Usage> = refs
        .iter()
        .map(|s| {
            if readable(s) {
                read.next().unwrap_or_else(Usage::missing)
            } else {
                Usage::not_read()
            }
        })
        .collect();
    // Each paused stretch in seconds, each stamp by its own day's offset; one still open runs to now.
    let now_i = i64::try_from(now).unwrap_or(i64::MAX);
    let pauses: Vec<(i64, i64)> = task
        .pause_history
        .iter()
        .filter_map(|p| {
            let to = if p.to.is_empty() {
                now_i
            } else {
                to_epoch(&p.to)?
            };
            Some((to_epoch(&p.from)?, to))
        })
        .collect();
    for u in usages.iter_mut() {
        skip_paused(u, &pauses);
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

/// The Work section of task `id` in the queue at `root`, read from disk now, its sessions read in
/// `allowed` (`work`).
pub fn for_task(
    reader: &Reader,
    allowed: &[Account],
    root: &Path,
    id: &str,
    now: u64,
) -> Result<Work, String> {
    let (task, body) = read(root, id)?;
    // Every task of the queue, to say which sessions this task shares with others.
    let all = queue::read_state(
        &[("q".into(), root.to_path_buf())],
        &mut queue::Cache::default(),
    )
    .tasks;
    Ok(work(
        reader,
        allowed,
        &task,
        &body,
        now,
        &shared_counts(&all, id),
        &|s| queue::stamp_epoch(s, None),
    ))
}

/// A session's minutes without the task's paused stretches (0045, in seconds since 1970): what of
/// them falls between its first and last stamp, to the nearest minute. Its tokens stay as they
/// are: a paused session spends none.
fn skip_paused(u: &mut Usage, pauses: &[(i64, i64)]) {
    let (Some(start), Some(end)) = (u.start, u.end) else {
        return;
    };
    let (start, end) = (
        i64::try_from(start).unwrap_or(i64::MAX),
        i64::try_from(end).unwrap_or(i64::MAX),
    );
    let paused: i64 = pauses
        .iter()
        .map(|(a, z)| ((*z).min(end) - (*a).max(start)).max(0))
        .sum();
    u.minutes = u
        .minutes
        .saturating_sub(u64::try_from((paused + 30) / 60).unwrap_or(0));
}

/// Session `session`'s transcript as Markdown, only for a session that worked on task `id` and
/// whose stamps name one of `allowed`: the window can't read any other transcript through this.
pub fn transcript(
    reader: &Reader,
    allowed: &[Account],
    root: &Path,
    id: &str,
    session: &str,
) -> Result<String, String> {
    if !transcripts::is_short_id(session) {
        return Err("bad session id".into());
    }
    let (task, _) = read(root, id)?;
    let Some(named) = task.session_ids.iter().find(|s| s.id == session) else {
        return Err(format!("session {session} didn't work on task {id}"));
    };
    if !named.readable(allowed) {
        return Err(format!(
            "session {session} isn't stamped with a Claude account Muster reads"
        ));
    }
    reader.transcript_markdown(allowed, session)
}

/// Mark the tasks that have a live session stamped with one of `allowed`, for the card's dot.
/// Done tasks are skipped, so the cost follows the open work, not the queue's history. One index
/// refresh, then a few file times per session; no transcript is read.
pub fn mark_live(tasks: &mut [Task], reader: &Reader, allowed: &[Account], now: u64) {
    let open = || tasks.iter().filter(|t| t.stage != "done");
    let ids: HashSet<&str> = open()
        .flat_map(|t| t.session_ids.iter())
        .filter(|s| s.readable(allowed))
        .map(|s| s.id.as_str())
        .collect();
    let live = reader.live_set(allowed, ids, now);
    for t in tasks.iter_mut() {
        t.live = t.stage != "done"
            && t.session_ids
                .iter()
                .any(|s| s.readable(allowed) && live.contains(&s.id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcripts::tests::{home, prompt, reader, reply, write, NOW};
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
                    holds: false,
                    slot: String::new(),
                    // Its first stamps have no account; a later one names it.
                    account: "hg".into(),
                },
                SessionRef {
                    id: "4d517d9d".into(),
                    who: "qa".into(),
                    stages: vec!["testing".into()],
                    holds: false,
                    slot: String::new(),
                    account: "mg".into(),
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
                holds: true,
                slot: String::new(),
                account: "hg".into(),
            }
        );
        // The account comes only from a stamp of that session: a hand-typed group, a stamp
        // without one, or another session's stamp names none; two different ones clash for good.
        let named_accounts = |named: &[&str], log: &str| -> Vec<(String, String)> {
            let body = format!("# T\n\n## Log\n{log}");
            session_ids(named, &body)
                .into_iter()
                .map(|s| (s.id, s.account))
                .collect()
        };
        let pair = |id: &str, a: &str| (id.to_string(), a.to_string());
        assert_eq!(
            named_accounts(&["capture (drover/main, session 349bc9c3)"], ""),
            [pair("349bc9c3", "")]
        );
        assert_eq!(
            named_accounts(&["wt (main · session 11111111)"], ""),
            [pair("11111111", "")]
        );
        assert_eq!(
            named_accounts(
                &["x (session 22222222), session 33333333 (main · session 33333333 · cc-mg)"],
                ""
            ),
            [pair("22222222", ""), pair("33333333", "mg")]
        );
        let clash = "- 2026-10-12 10:00 a (session 44444444 · cc-mg): noted\n- 2026-10-12 11:00 a (session 44444444 · cc-ih): noted\n- 2026-10-12 12:00 a (session 44444444 · cc-mg): noted\n";
        assert_eq!(named_accounts(&[], clash), [pair("44444444", "")]);
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
        let ids = |named: &[&str]| -> Vec<String> {
            session_ids(named, body(TASK))
                .into_iter()
                .map(|s| s.id)
                .collect()
        };
        assert_eq!(ids(&[]), ["7b5e8856", "4d517d9d"]);
        assert_eq!(ids(&[claim, "owner"]), ["7b5e8856", "4d517d9d", "0123abcd"]);
        // A log line quoted in the description isn't the log.
        let quoted = "# T\n\n```\n- 2026-10-12 10:00 x (session 11111111): stage ready → building (claimed)\n```\n\n## Log\n- 2026-10-12 09:00 capture: created\n";
        assert!(session_ids(&[], quoted).is_empty());
    }

    #[test]
    fn a_sessions_row_carries_the_slot_its_stamps_act_as() {
        let body = "# T\n\n## Log\n- 2026-10-14 19:41 Kaveh-2 (main · session 7b5e8856 · slot coder-2 · cc-hg): stage ready → building (claimed)\n- 2026-10-14 19:45 akbar (main · session 1c6af0d9): check-in 10% · on it\n";
        let refs = sessions(
            &[],
            "Kaveh-2 (main · session 7b5e8856 · slot coder-2 · cc-hg)",
            body,
        );
        let slots: Vec<(&str, &str, &str)> = refs
            .iter()
            .map(|r| (r.id.as_str(), r.who.as_str(), r.slot.as_str()))
            .collect();
        assert_eq!(
            slots,
            [
                ("7b5e8856", "Kaveh-2", "coder-2"),
                ("1c6af0d9", "akbar", "")
            ]
        );
        // Sent to the window only when there is one.
        let json = serde_json::to_value(&refs).unwrap();
        assert_eq!(json[0]["slot"], "coder-2");
        assert!(json[1].get("slot").is_none());
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
        let (r, a) = reader(h.path());
        let first = for_task(&r, &a, q.path(), "0025", NOW).unwrap();
        let id = first.rows[0].session.id.clone();
        assert!(first.rows.iter().all(|r| r.shared_with == 0));
        assert_eq!(first.totals.shared, 0);
        // Two other tasks claimed by the same session.
        for other in ["0026", "0027"] {
            let text = format!("---\nid: {other}\nstatus: done\nstage: done\nclaimed-by: capture (session {id})\n---\n\n# Other\n\n## Log\n");
            fs::write(q.path().join(format!("done/{other}-other.md")), text).unwrap();
        }
        let w = for_task(&r, &a, q.path(), "0025", NOW).unwrap();
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
        let (r, a) = reader(h.path());
        let w = for_task(&r, &a, q.path(), "0025", NOW).unwrap();
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
        let (reader, a) = reader(h.path());
        let shared = std::collections::HashMap::new();
        // Etc/GMT-2 is UTC+2 all year (POSIX names the sign the other way round).
        let plus2 = |s: &str| queue::stamp_epoch(s, Some("Etc/GMT-2"));
        let w = work(&reader, &a, &task, body(&text), NOW, &shared, &plus2);
        assert_eq!((w.rows[0].usage.minutes, w.rows[1].usage.minutes), (20, 10));
        assert_eq!(w.totals.minutes, 30);
        // Tokens aren't time: they stay what the transcript says.
        assert_eq!(w.totals.total_tokens, 69516);
        // In UTC the same stamps miss A's stretch entirely.
        let utc = |s: &str| queue::stamp_epoch(s, Some("UTC"));
        let w = work(&reader, &a, &task, body(&text), NOW, &shared, &utc);
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
        let w = work(&reader, &a, &task, body(&open), NOW, &shared, &plus2);
        assert_eq!((w.rows[0].usage.minutes, w.rows[1].usage.minutes), (10, 0));
    }

    #[test]
    fn each_paused_stamp_is_read_by_its_own_days_offset() {
        // Review c4: in Los Angeles, a pause in July is UTC-7 and one in December UTC-8. A built
        // it in July (17:00–17:30 UTC), B tested it in December (18:00–18:30 UTC); each was
        // paused 10:10–10:20 local, inside its session, so each loses 10 minutes. One offset for
        // both would miss one of them.
        let text = TASK.replace(
            "- 2026-10-12 11:00 muster",
            "- 2026-07-01 10:10 owner: paused\n- 2026-07-01 10:20 owner: resumed after 10 min\n- 2026-12-01 10:10 owner: paused\n- 2026-12-01 10:20 owner: resumed after 10 min\n- 2026-10-12 11:00 muster",
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
        assert_eq!(task.pause_history.len(), 2);
        let h = home();
        write(
            h.path(),
            ".claude",
            A,
            &[
                prompt("2026-07-01T17:00:00Z", "build 0025"),
                reply("a1", "2026-07-01T17:30:00Z", 1, 1, 1, 1),
            ],
            NOW - 3600,
        );
        write(
            h.path(),
            ".claude",
            B,
            &[
                prompt("2026-12-01T18:00:00Z", "test 0025"),
                reply("b1", "2026-12-01T18:30:00Z", 1, 1, 1, 1),
            ],
            NOW - 3600,
        );
        let la = |s: &str| queue::stamp_epoch(s, Some("America/Los_Angeles"));
        let (r, a) = reader(h.path());
        let w = work(
            &r,
            &a,
            &task,
            body(&text),
            NOW,
            &std::collections::HashMap::new(),
            &la,
        );
        assert_eq!((w.rows[0].usage.minutes, w.rows[1].usage.minutes), (20, 20));
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
        let (r, a) = reader(h.path());
        let w = for_task(&r, &a, q.path(), "0025", NOW).unwrap();
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
        let w = for_task(&r, &a, q.path(), "0026", NOW).unwrap();
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
        let (r, a) = reader(h.path());
        assert!(transcript(&r, &a, q.path(), "0025", "7b5e8856")
            .unwrap()
            .contains("**You:** build 0025"));
        assert_eq!(
            transcript(&r, &a, q.path(), "0025", "eeeeeeee").unwrap_err(),
            "session eeeeeeee didn't work on task 0025"
        );
        for bad in ["7B5E8856", "../../etc", "7b5e8856-c9c8", ""] {
            assert_eq!(
                transcript(&r, &a, q.path(), "0025", bad).unwrap_err(),
                "bad session id",
                "{bad:?}"
            );
        }
        assert!(transcript(&r, &a, q.path(), "25", "7b5e8856").is_err());
        assert!(for_task(&r, &a, q.path(), "../0025", NOW).is_err());
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
        let (r, a) = reader(h.path());
        let mut tasks = vec![
            parsed("0001", "testing", "qa (session 4d517d9d · cc-mg)"),
            parsed("0002", "building", "wt (session 7b5e8856 · cc-hg)"),
            parsed("0003", "done", "qa (session 4d517d9d · cc-mg)"),
            parsed("0004", "building", "wt (session 0000aaaa · cc-mg)"),
            // The live session, stamped before 0030 (no account), with none, or with ih: the
            // company account or one not on the list, so not read (0099).
            parsed("0005", "testing", "qa (session 4d517d9d)"),
            parsed("0006", "testing", "qa (main · session 4d517d9d)"),
            parsed("0007", "testing", "qa (main · session 4d517d9d · cc-ih)"),
        ];
        assert_eq!(
            tasks[0].session_ids,
            [SessionId {
                id: "4d517d9d".into(),
                account: "mg".into()
            }]
        );
        mark_live(&mut tasks, &r, &a, NOW);
        let live: Vec<bool> = tasks.iter().map(|t| t.live).collect();
        assert_eq!(live, [true, false, false, false, false, false, false]);
        // Five minutes and a second later, nothing is.
        mark_live(&mut tasks, &r, &a, NOW - 60 + transcripts::LIVE_SECS + 1);
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
        let (r, a) = reader(h.path());
        let w = for_task(&r, &a, q.path(), "0001", NOW).unwrap();
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
        // The builder's stamp is from before 0030, with no account: its row stays, unread (0099).
        assert_eq!(w.rows[0].usage, Usage::not_read());
        assert_eq!(w.rows[1].usage.found, Found::Read);
        let mut tasks = queue::read_state(
            &[("q".into(), q.path().to_path_buf())],
            &mut queue::Cache::default(),
        )
        .tasks;
        let ids: Vec<(&str, &str)> = tasks[0]
            .session_ids
            .iter()
            .map(|s| (s.id.as_str(), s.account.as_str()))
            .collect();
        assert_eq!(ids, [("7b5e8856", ""), ("4d517d9d", "mg")]);
        mark_live(&mut tasks, &r, &a, NOW);
        assert!(tasks[0].live);
        assert!(transcript(&r, &a, q.path(), "0001", "4d517d9d").is_ok());
        assert!(transcript(&r, &a, q.path(), "0001", "7b5e8856").is_err());
    }

    /// R1 (0099): a session stamped with no account (bare `claude`: the company account) or with
    /// one not on the list is never read, even when its transcript is filed under a playground
    /// project an allowed account reaches: no tokens, no live dot, no transcript. Its row stays,
    /// since the task file names it. A session stamped with mg still reads.
    #[test]
    fn only_sessions_stamped_with_an_allowed_account_are_read() {
        let text = "---\nid: 0001\nstage: building\nstatus: doing\nclaimed-by: wt-m (main · session 33333333 · cc-mg)\n---\n\n# T\n\n## Log\n- 2026-10-12 09:00 wt-c (main · session 11111111): stage ready → building (claimed)\n- 2026-10-12 09:30 wt-i (main · session 22222222 · cc-ih): stage building → ready\n- 2026-10-12 10:00 wt-m (main · session 33333333 · cc-mg): stage ready → building (claimed)\n";
        let other = "---\nid: 0002\nstage: building\nstatus: doing\nclaimed-by: wt-c (main · session 11111111)\n---\n\n# U\n\n## Log\n- 2026-10-12 09:00 wt-i (main · session 22222222 · cc-ih): stage ready → building (claimed)\n";
        let q = queue();
        fs::write(q.path().join("doing/0001-t.md"), text).unwrap();
        fs::write(q.path().join("doing/0002-u.md"), other).unwrap();
        let h = home();
        // All three in mg's own playground project, each changed a minute ago.
        for id in ["11111111", "22222222", "33333333"] {
            write(
                h.path(),
                ".claude-mg",
                &format!("{id}-0000"),
                &[
                    prompt("2026-10-12T08:00:00Z", "go"),
                    reply(id, "2026-10-12T08:10:00Z", 1, 1, 1, 1),
                ],
                NOW - 60,
            );
        }
        let (r, a) = reader(h.path());
        let w = for_task(&r, &a, q.path(), "0001", NOW).unwrap();
        let rows: Vec<(&str, Found, bool, u64)> = w
            .rows
            .iter()
            .map(|row| {
                (
                    row.session.id.as_str(),
                    row.usage.found,
                    row.usage.live,
                    row.usage.tokens.total(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                ("11111111", Found::NotRead, false, 0),
                ("22222222", Found::NotRead, false, 0),
                ("33333333", Found::Read, true, 4),
            ]
        );
        // The window is told why, not "not on this Mac".
        let json = serde_json::to_string(&w.rows[0].usage).unwrap();
        assert!(json.contains("\"found\":\"not-read\""), "{json}");
        assert_eq!((w.totals.sessions, w.totals.read, w.totals.live), (3, 1, 1));
        assert_eq!((w.totals.total_tokens, w.totals.minutes), (4, 10));
        // The transcript view refuses them, by the stamp, not by where the file is.
        for id in ["11111111", "22222222"] {
            assert_eq!(
                transcript(&r, &a, q.path(), "0001", id).unwrap_err(),
                format!("session {id} isn't stamped with a Claude account Muster reads")
            );
        }
        assert!(transcript(&r, &a, q.path(), "0001", "33333333")
            .unwrap()
            .contains("**You:** go"));
        // The card's dot: 0002 has only the bare and the ih session, both live on disk.
        let mut tasks = queue::read_state(
            &[("q".into(), q.path().to_path_buf())],
            &mut queue::Cache::default(),
        )
        .tasks;
        tasks.sort_by(|x, y| x.id.cmp(&y.id));
        mark_live(&mut tasks, &r, &a, NOW);
        assert_eq!((tasks[0].live, tasks[1].live), (true, false));
        // With mg off the list too, nothing of this task is read at all.
        let hg_only = crate::accounts::resolve(&["hg".into()], h.path());
        let w = for_task(&r, &hg_only, q.path(), "0001", NOW).unwrap();
        assert_eq!((w.totals.read, w.totals.live), (0, 0));
        assert!(transcript(&r, &hg_only, q.path(), "0001", "33333333").is_err());
        mark_live(&mut tasks, &r, &hg_only, NOW);
        assert!(!tasks[0].live);
    }

    /// 0182: a builder stamped `cc-mg` and resumed on cc-hg with the same id and claim (cc-mg hit
    /// its limit) writes its transcript under `~/.claude-hg` only. Its Work row, the card's live
    /// dot and its transcript are found by session id in every allowed account, not only the
    /// stamp's.
    #[test]
    fn a_session_stamped_with_one_account_and_resumed_on_another_is_still_read() {
        let text = "---\nid: 0001\nstage: building\nstatus: doing\nclaimed-by: asghar_20 (main · session 3c09f6c7 · cc-mg)\n---\n\n# T\n\n## Log\n- 2026-10-12 10:00 asghar_20 (main · session 3c09f6c7 · cc-mg): stage ready → building (claimed)\n";
        let q = queue();
        fs::write(q.path().join("doing/0001-t.md"), text).unwrap();
        let h = home();
        let file = write(
            h.path(),
            ".claude-hg",
            "3c09f6c7-0000",
            &[
                prompt("2026-10-12T08:00:00Z", "go on"),
                reply("r1", "2026-10-12T08:10:00Z", 1, 1, 1, 1),
            ],
            NOW - 60,
        );
        assert!(file.starts_with(h.path().join(".claude-hg")));
        let (r, a) = reader(h.path());
        assert_eq!(
            a.iter().map(Account::label).collect::<Vec<_>>(),
            ["mg", "hg"]
        );
        let w = for_task(&r, &a, q.path(), "0001", NOW).unwrap();
        assert_eq!(w.rows.len(), 1);
        assert_eq!(w.rows[0].session.account, "mg");
        assert_eq!(
            (w.rows[0].usage.found, w.rows[0].usage.live),
            (Found::Read, true)
        );
        assert_eq!((w.totals.read, w.totals.live), (1, 1));
        assert!(transcript(&r, &a, q.path(), "0001", "3c09f6c7")
            .unwrap()
            .contains("**You:** go on"));
        let mut tasks = queue::read_state(
            &[("q".into(), q.path().to_path_buf())],
            &mut queue::Cache::default(),
        )
        .tasks;
        mark_live(&mut tasks, &r, &a, NOW);
        assert!(tasks[0].live, "the card's dot");
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
                "- 2026-10-12 10:00 wt (session {id} · cc-mg): stage ready → building (claimed)\n"
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
            let (r, a) = reader(h.path());
            let u = r.usages(&a, &ids, NOW);
            assert!(u
                .iter()
                .all(|u| u.found == Found::Read && u.tokens.total() == 10_000));
        });
        let (r, a) = reader(h.path());
        r.usages(&a, &ids, NOW);
        let warm = best(&|| {
            r.usages(&a, &ids, NOW);
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
            mark_live(&mut t, &r, &a, NOW);
        });
        mark_live(&mut tasks, &r, &a, NOW);
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
