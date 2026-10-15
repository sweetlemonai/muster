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
use crate::transcripts::{self, Found, Reader, Tokens, Usage};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
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
pub(crate) fn log_who(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("- ")?;
    crate::ops::stamp_minutes(rest.get(..16)?)?;
    let (who, _) = rest.get(16..)?.strip_prefix(' ')?.split_once(": ")?;
    Some(who)
}

/// The label someone acted under, without its session part: the text before the first `(` or
/// `,` ("capture (drover/main, session 349bc9c3)" → "capture").
pub(crate) fn label(who: &str) -> String {
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
    /// How many other tasks in the queue name this session. Above 0, its tokens and minutes are
    /// kept out of the totals (mu-172), and the window says so on the row and the totals.
    pub shared_with: usize,
}

/// The Work section's totals (mu-172): what counts is the task's own sessions' input, output and
/// cache writes, and its agent time. A session another task of the queue names too is kept out of
/// every count and shown apart, since its transcript can't tell which part was this task's.
#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub sessions: usize,
    /// Sessions whose transcript was read on this Mac, shared ones included.
    pub read: usize,
    pub live: usize,
    /// Rows whose session other tasks name too (see `Row::shared_with`).
    pub shared: usize,
    /// The own sessions' tokens by kind, all of them, and every kind together.
    pub tokens: Tokens,
    pub total_tokens: u64,
    /// The own sessions' tokens that count against `est-tokens` (`Tokens::counted`).
    pub counted_tokens: u64,
    /// The own sessions' transcript spans, summed, less paused stretches: the rows' breakdown.
    pub minutes: u64,
    /// The read sessions kept out as shared: their tokens by kind, and how many.
    pub shared_tokens: Tokens,
    pub shared_read: usize,
    /// Agent time (`agent_minutes`), what `est-time` is held against; None before a claim.
    pub agent_minutes: Option<u64>,
}

/// A task's sessions' usages split into its own and the shared ones (mu-172). The Work section's
/// totals and the card's token ribbon (`mark_tokens`) both come from it, so they agree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Split {
    /// The own sessions read: their tokens, how many, and their minutes.
    pub own: Tokens,
    pub read: usize,
    pub minutes: u64,
    /// The shared sessions read: their tokens and how many, counted nowhere.
    pub shared: Tokens,
    pub shared_read: usize,
}

/// Split `usages` by `shared`, one flag per usage (a usage with no flag is the task's own): a
/// session is shared when another task of the same queue names it (`shared_counts`). Only read
/// transcripts add up, as in `transcripts::actuals`.
pub fn split(usages: &[Usage], shared: &[bool]) -> Split {
    let is_shared = |i: usize| shared.get(i).copied().unwrap_or(false);
    let pick = |want: bool| {
        transcripts::actuals(
            usages
                .iter()
                .enumerate()
                .filter(move |(i, _)| is_shared(*i) == want)
                .map(|(_, u)| u),
        )
    };
    let (own, kept_out) = (pick(false), pick(true));
    Split {
        own: own.tokens,
        read: own.read,
        minutes: own.minutes,
        shared: kept_out.tokens,
        shared_read: kept_out.read,
    }
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
    /// The same sessions' tokens and the task's time by stage (mu-172): the Work section's table.
    pub stages: Vec<StageRow>,
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

/// The stages the Work section has a row for (mu-172), in order; tokens spent in any other stage
/// (Backlog, Ready, Done) or before the task was made go to an "other" row.
const STAGE_ROWS: [&str; 4] = ["shaping", "building", "testing", "your-check"];

/// One stage's row of the Work section (mu-172): the time the task spent in it, less blocked and
/// paused stretches (Your check's is the owner's wait), and the tokens its sessions used while it
/// was there. `tokens` and `counted_tokens` are the own sessions'; a shared session's tokens are
/// in `shared_tokens`, counted nowhere.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StageRow {
    /// One of `STAGE_ROWS`, or "other".
    pub stage: String,
    pub minutes: u64,
    pub tokens: Tokens,
    pub counted_tokens: u64,
    pub shared_tokens: Tokens,
    pub sessions: Vec<StageSession>,
}

/// A session's part of one stage: the tokens of its turns stamped while the task was in it, and
/// minutes from its first to its last such turn, paused stretches left out. `id` is a `rows`
/// entry's, which carries the rest (who, slot, holds, shared with).
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StageSession {
    pub id: String,
    pub tokens: Tokens,
    pub counted_tokens: u64,
    pub minutes: u64,
    /// Another task of the queue names it: shown, never counted.
    pub shared: bool,
}

/// Stretches as (from, to) seconds since 1970.
type Spans = Vec<(i64, i64)>;

/// A task's times, read once (each `to_epoch` starts `date`): where it was when, and when it was
/// stopped.
struct Timeline {
    /// Each stage it was in and when (`stage_windows`); None when a stage line's stamp can't be
    /// read, or `now` is before the last one (a clock set back): no stage can be told then.
    stages: Option<Vec<(String, Spans)>>,
    /// Blocked or paused, merged; one still open runs to now.
    stops: Spans,
    /// Paused only (0045), for the sessions' minutes.
    pauses: Spans,
}

/// `spans` in order with overlapping or touching ones joined, empty ones dropped.
fn merged(mut spans: Spans) -> Spans {
    spans.retain(|(a, z)| z > a);
    spans.sort_unstable();
    let mut out: Spans = Vec::with_capacity(spans.len());
    for (a, z) in spans {
        match out.last_mut() {
            Some((_, end)) if a <= *end => *end = (*end).max(z),
            _ => out.push((a, z)),
        }
    }
    out
}

/// Seconds of `spans` (merged) that fall in `stops` (merged).
fn overlap(spans: &[(i64, i64)], stops: &[(i64, i64)]) -> i64 {
    spans
        .iter()
        .flat_map(|(a, z)| {
            stops
                .iter()
                .map(move |(b, y)| ((*z).min(*y) - (*a).max(*b)).max(0))
        })
        .sum()
}

/// Seconds to whole minutes, to the nearest.
fn to_minutes(secs: i64) -> u64 {
    u64::try_from((secs + 30) / 60).unwrap_or(0)
}

fn timeline(task: &Task, now: u64, to_epoch: &dyn Fn(&str) -> Option<i64>) -> Timeline {
    let mut read: HashMap<String, Option<i64>> = HashMap::new();
    let mut epoch =
        |s: &str| -> Option<i64> { *read.entry(s.to_string()).or_insert_with(|| to_epoch(s)) };
    let now = i64::try_from(now).unwrap_or(i64::MAX);
    // A stretch's stamps, one still open running to now; one that can't be read is dropped.
    let mut spans = |from: &str, to: &str| -> Option<(i64, i64)> {
        let z = if to.is_empty() { now } else { epoch(to)? };
        Some((epoch(from)?, z))
    };
    let pauses: Spans = task
        .pause_history
        .iter()
        .filter_map(|p| spans(&p.from, &p.to))
        .collect();
    let blocks: Spans = task
        .block_history
        .iter()
        .filter_map(|b| spans(&b.from, &b.to))
        .collect();
    let stops = merged(blocks.into_iter().chain(pauses.iter().copied()).collect());
    let h = &task.stage_history;
    let ats: Option<Vec<i64>> = h.iter().map(|s| epoch(&s.at)).collect();
    let created = (!task.captured_at.is_empty())
        .then(|| epoch(&task.captured_at))
        .flatten();
    let stages = ats
        .filter(|ats| ats.last().is_none_or(|last| *last <= now))
        .map(|ats| stage_windows(task, &ats, created, now));
    Timeline {
        stages,
        stops,
        pauses,
    }
}

/// E2 (mu-172): each stage line opens a window of the stage it moved to, until the next stage
/// line; the last runs to `now`. Before the first stage line the task was in that line's `from`
/// stage since it was made (`created`, the log's first stamp), or, with no stage line yet, in its
/// stage now: so a task made in Shaping has its Shaping row. `ats` are the stage lines' stamps.
fn stage_windows(task: &Task, ats: &[i64], created: Option<i64>, now: i64) -> Vec<(String, Spans)> {
    let mut out: Vec<(String, Spans)> = Vec::new();
    let mut add = |stage: &str, a: i64, z: i64| {
        let span = (a, z.max(a));
        match out.iter_mut().find(|(s, _)| s == stage) {
            Some((_, spans)) => spans.push(span),
            None => out.push((stage.to_string(), vec![span])),
        }
    };
    let h = &task.stage_history;
    let (first_stage, first_end) = match (h.first(), ats.first()) {
        (Some(s), Some(at)) => (s.from.as_str(), *at),
        _ => (task.stage.as_str(), now),
    };
    if let Some(c) = created.filter(|c| *c <= first_end) {
        add(first_stage, c, first_end);
    }
    for (i, (s, at)) in h.iter().zip(ats).enumerate() {
        add(&s.to, *at, ats.get(i + 1).copied().unwrap_or(now));
    }
    out
}

impl Timeline {
    /// The stretches the task was in `stage`, merged; None when it never was.
    fn windows(&self, stage: &str) -> Option<Spans> {
        let (_, spans) = self.stages.as_ref()?.iter().find(|(s, _)| s == stage)?;
        Some(merged(spans.clone()))
    }

    /// Minutes the task was in `stage`, blocked and paused stretches left out.
    fn minutes(&self, stage: &str) -> u64 {
        self.windows(stage).map_or(0, |w| {
            let all: i64 = w.iter().map(|(a, z)| z - a).sum();
            to_minutes(all - overlap(&w, &self.stops))
        })
    }

    /// E5: Building + Testing stage time; None before it ever got there.
    fn agent_minutes(&self) -> Option<u64> {
        let worked = ["building", "testing"];
        worked
            .iter()
            .any(|s| self.windows(s).is_some())
            .then(|| worked.iter().map(|s| self.minutes(s)).sum())
    }
}

/// A task's agent time in minutes, what its `est-time` is held against (mu-172, E5): its time in
/// Building plus its time in Testing, less the time it was blocked or paused, each minute once.
/// In Building or Testing it runs to `now`. For a task never released back to Ready nor sent back
/// from Your check this is mu-168's `measured::agent_minutes` (first claim to last leaving
/// Testing); a stretch back in Ready isn't work, and the owner's wait in Your check is neither
/// agent's (D3). Stamps are read with `to_epoch`, as `work`'s pauses are, and `now` is seconds
/// since 1970. None when it never got to Building or Testing, a stage stamp can't be read, or
/// `now` is before the last stage line.
pub fn agent_minutes(task: &Task, now: u64, to_epoch: &dyn Fn(&str) -> Option<i64>) -> Option<u64> {
    timeline(task, now, to_epoch).agent_minutes()
}

/// Seconds of `pauses` between `start` and `end`.
fn paused_secs(pauses: &[(i64, i64)], start: i64, end: i64) -> i64 {
    pauses
        .iter()
        .map(|(a, z)| ((*z).min(end) - (*a).max(start)).max(0))
        .sum()
}

/// `a` less `b`, kind by kind, never below 0.
fn less(a: Tokens, b: Tokens) -> Tokens {
    Tokens {
        input: a.input.saturating_sub(b.input),
        output: a.output.saturating_sub(b.output),
        cache_read: a.cache_read.saturating_sub(b.cache_read),
        cache_write: a.cache_write.saturating_sub(b.cache_write),
    }
}

/// E2–E4 (mu-172): the Work section's rows by stage. Each read session's minute buckets go to the
/// stage the task was in then; what fits no shown stage (Ready, Backlog, Done, before the task was
/// made, or a reply with no time stamp) goes to "other", so the rows always add up to the
/// session's tokens. `refs`, `usages` and `shared` are `work`'s, one per session.
fn stage_rows(
    tl: &Timeline,
    refs: &[SessionRef],
    usages: &[Usage],
    shared: &[bool],
) -> Vec<StageRow> {
    // Every shown stage's windows, flat and in order: they never overlap.
    let mut flat: Vec<(i64, i64, usize)> = Vec::new();
    for (i, stage) in STAGE_ROWS.iter().enumerate() {
        for (a, z) in tl.windows(stage).unwrap_or_default() {
            flat.push((a, z, i));
        }
    }
    flat.sort_unstable();
    // The window a minute falls in, as its place in `flat`.
    let window_of = |minute: u64| -> Option<usize> {
        let m = i64::try_from(minute).ok()?;
        let at = flat.partition_point(|(a, _, _)| *a <= m).checked_sub(1)?;
        (m < flat[at].1).then_some(at)
    };
    let other = STAGE_ROWS.len();
    let mut rows: Vec<StageRow> = STAGE_ROWS
        .iter()
        .map(|s| s.to_string())
        .chain(["other".to_string()])
        .enumerate()
        .map(|(i, stage)| StageRow {
            minutes: if i < other { tl.minutes(&stage) } else { 0 },
            stage,
            tokens: Tokens::default(),
            counted_tokens: 0,
            shared_tokens: Tokens::default(),
            sessions: Vec::new(),
        })
        .collect();
    for (n, (r, u)) in refs.iter().zip(usages).enumerate() {
        if u.found != Found::Read {
            continue;
        }
        // Per row its tokens; per window the first and last minute it used any there, so a
        // stage entered twice never counts the time spent elsewhere in between.
        let mut parts: Vec<Option<Tokens>> = vec![None; rows.len()];
        let mut spans: Vec<Option<(u64, u64)>> = vec![None; flat.len()];
        let mut in_stages = Tokens::default();
        for (minute, t) in u.buckets.iter() {
            let i = match window_of(*minute) {
                Some(w) => {
                    in_stages.add(t);
                    let s = spans[w].get_or_insert((*minute, *minute));
                    s.1 = *minute;
                    flat[w].2
                }
                None => other,
            };
            parts[i].get_or_insert_with(Tokens::default).add(t);
        }
        // Whatever the stages don't hold is "other": its buckets there and turns with no time.
        let rest = less(u.tokens, in_stages);
        if rest != Tokens::default() || parts[other].is_some() {
            parts[other] = Some(rest);
        }
        let is_shared = shared.get(n).copied().unwrap_or(false);
        for (i, (row, part)) in rows.iter_mut().zip(parts).enumerate() {
            let Some(tokens) = part else {
                continue;
            };
            // Each stretch first to last minute, paused left out; "other" gathers stretches far
            // apart (before the task, Ready, Done), so no time there, as the row has none.
            let secs: i64 = flat
                .iter()
                .zip(&spans)
                .filter(|((_, _, stage), _)| *stage == i)
                .filter_map(|(_, s)| *s)
                .map(|(first, last)| {
                    let (a, z) = (
                        i64::try_from(first).unwrap_or(i64::MAX),
                        i64::try_from(last).unwrap_or(i64::MAX),
                    );
                    (z - a - paused_secs(&tl.pauses, a, z)).max(0)
                })
                .sum();
            if is_shared {
                row.shared_tokens.add(&tokens);
            } else {
                row.tokens.add(&tokens);
            }
            row.sessions.push(StageSession {
                id: r.id.clone(),
                tokens,
                counted_tokens: tokens.counted(),
                minutes: to_minutes(secs),
                shared: is_shared,
            });
        }
    }
    rows.retain(|row| {
        let entered = row.stage != "other" && tl.windows(&row.stage).is_some();
        entered || !row.sessions.is_empty()
    });
    for row in rows.iter_mut() {
        row.counted_tokens = row.tokens.counted();
    }
    rows
}

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

/// The Work section for `task` (whose text after the front matter is `body`), at `now`.
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
    // Its stages, blocks and pauses in seconds, each stamp by its own day's offset; a pause still
    // open runs to now.
    let tl = timeline(task, now, to_epoch);
    for u in usages.iter_mut() {
        skip_paused(u, &tl.pauses);
    }
    let flags: Vec<bool> = refs
        .iter()
        .map(|r| shared.get(&r.id).copied().unwrap_or(0) > 0)
        .collect();
    let sum = split(&usages, &flags);
    let stages = stage_rows(&tl, &refs, &usages, &flags);
    let totals = Totals {
        sessions: refs.len(),
        read: sum.read + sum.shared_read,
        live: usages.iter().filter(|u| u.live).count(),
        shared: flags.iter().filter(|f| **f).count(),
        tokens: sum.own,
        total_tokens: sum.own.total(),
        counted_tokens: sum.own.counted(),
        minutes: sum.minutes,
        shared_tokens: sum.shared,
        shared_read: sum.shared_read,
        agent_minutes: tl.agent_minutes(),
    };
    let estimate = VsEstimate {
        minutes: task.est_minutes,
        tokens: task.est_token_count,
        minutes_pct: totals.agent_minutes.and_then(|m| pct(m, task.est_minutes)),
        tokens_pct: pct(totals.counted_tokens, task.est_token_count),
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
        stages,
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

/// How long a card's token count is reused, in seconds: a live transcript changes on every
/// poll, and reading it again each time (a big one runs to 60 MB) would be the poll's cost.
pub const TOKEN_MEMO_SECS: u64 = 30;

/// The cards' token counts by (queue, id), between polls (mu-172). Holds only the tasks the
/// latest `mark_tokens` marked.
#[derive(Default)]
pub struct TokenMemo {
    entries: HashMap<(String, String), Counted>,
}

/// One card's count: the sessions it was taken over (readable ones, each with whether it's
/// shared), and what it gave, at `at` (seconds since 1970).
struct Counted {
    sessions: Vec<(String, bool)>,
    used: Option<u64>,
    shared: bool,
    at: u64,
}

impl TokenMemo {
    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }
}

/// Does a card in this stage get the token ribbon: an agent's task being shaped, built or tested.
fn counts_tokens(t: &Task) -> bool {
    matches!(t.stage.as_str(), "shaping" | "building" | "testing") && !t.is_human()
}

/// Fill each card's token ribbon (mu-172): `tokens_used`, its own sessions' counted tokens, the
/// same number the Work section's `counted_tokens` gives (`split`), and `tokens_shared`, a
/// session read was kept out as shared. Only for tasks in Shaping, Building or Testing that
/// aren't the owner's; the rest stay None and false. None too when no session could be read.
/// A count is reused for `TOKEN_MEMO_SECS` while the task names the same sessions, each shared
/// or not as before; the transcripts needed are read in one go.
pub fn mark_tokens(
    tasks: &mut [Task],
    reader: &Reader,
    allowed: &[Account],
    now: u64,
    memo: &mut TokenMemo,
) {
    // How many tasks of each queue name each session.
    let mut named: HashMap<(&str, &str), usize> = HashMap::new();
    for t in tasks.iter() {
        for s in &t.session_ids {
            *named.entry((t.queue.as_str(), s.id.as_str())).or_insert(0) += 1;
        }
    }
    let mut kept: HashMap<(String, String), Counted> = HashMap::new();
    let mut due: Vec<(usize, Vec<(String, bool)>)> = Vec::new();
    for (i, t) in tasks.iter().enumerate() {
        if !counts_tokens(t) {
            continue;
        }
        let sessions: Vec<(String, bool)> = t
            .session_ids
            .iter()
            .filter(|s| s.readable(allowed))
            .map(|s| {
                let others = named.get(&(t.queue.as_str(), s.id.as_str())).copied();
                (s.id.clone(), others.unwrap_or(0) > 1)
            })
            .collect();
        let key = (t.queue.clone(), t.id.clone());
        match memo.entries.remove(&key) {
            Some(c) if c.sessions == sessions && now.saturating_sub(c.at) < TOKEN_MEMO_SECS => {
                kept.insert(key, c);
            }
            _ => due.push((i, sessions)),
        }
    }
    let ids: Vec<String> = due
        .iter()
        .flat_map(|(_, s)| s.iter().map(|(id, _)| id.clone()))
        .collect::<HashSet<String>>()
        .into_iter()
        .collect();
    // Nothing due (every count reused): no index refresh either.
    let usages = if ids.is_empty() {
        Vec::new()
    } else {
        reader.usages(allowed, &ids, now)
    };
    let read: HashMap<String, Usage> = ids.iter().cloned().zip(usages).collect();
    for (i, sessions) in due {
        let usages: Vec<Usage> = sessions
            .iter()
            .map(|(id, _)| read.get(id).cloned().unwrap_or_else(Usage::missing))
            .collect();
        let flags: Vec<bool> = sessions.iter().map(|(_, shared)| *shared).collect();
        let s = split(&usages, &flags);
        let t = &tasks[i];
        kept.insert(
            (t.queue.clone(), t.id.clone()),
            Counted {
                sessions,
                used: (s.read + s.shared_read > 0).then(|| s.own.counted()),
                shared: s.shared_read > 0,
                at: now,
            },
        );
    }
    for t in tasks.iter_mut() {
        let c = counts_tokens(t)
            .then(|| kept.get(&(t.queue.clone(), t.id.clone())))
            .flatten();
        t.tokens_used = c.and_then(|c| c.used);
        t.tokens_shared = c.is_some_and(|c| c.shared);
    }
    memo.entries = kept;
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

    fn queue() -> crate::testkit::TestDir {
        let d = crate::testkit::tempdir().unwrap();
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
        // mu-172: kept out of the totals and shown apart; only B's tokens count now.
        assert_eq!(first.totals.counted_tokens, 8015 + 1501);
        assert_eq!((w.totals.read, w.totals.shared_read), (2, 1));
        assert_eq!(w.totals.shared_tokens, row.usage.tokens);
        assert_eq!(
            (w.totals.counted_tokens, w.totals.total_tokens),
            (1501, 11501)
        );
        assert_eq!(w.totals.minutes, 10);
        assert_eq!(w.estimate.tokens_pct, Some(2));
        // Agent time is the task's, whoever's sessions did it.
        assert_eq!(w.totals.agent_minutes, first.totals.agent_minutes);
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
                // mu-172: input, output and cache writes count; the 60k cache reads don't.
                counted_tokens: 9516,
                minutes: 40,
                shared_tokens: Tokens::default(),
                shared_read: 0,
                // mu-172: agent time, claimed 10:00 → left Testing 13:00, not the sessions' 40
                // minutes of transcript.
                agent_minutes: Some(180),
            }
        );
        assert_eq!(
            w.estimate,
            VsEstimate {
                minutes: 120,
                tokens: 100_000,
                // mu-172: was 33 (40 transcript minutes of 120); now agent time, 180 of 120.
                minutes_pct: Some(150),
                // mu-172: was 70 (every kind, 69,516 of 100k); now counted tokens, 9,516.
                tokens_pct: Some(10)
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

    /// A task in `stage` whose Log is `log`.
    fn staged(stage: &str, log: &str) -> Task {
        let text = format!(
            "---\nid: 0001\nstage: {stage}\nstatus: {}\n---\n\n# T\n\n## Log\n{log}",
            queue::mirror_status(stage, false)
        );
        queue::parse_task(
            "p",
            Path::new("/q"),
            "",
            Path::new("/q/0001-t.md"),
            &text,
            0,
        )
        .unwrap()
    }

    const BUILT: &str = "- 2026-10-12 09:00 capture: created\n- 2026-10-12 10:00 wt (session 7b5e8856): stage ready → building (claimed)\n- 2026-10-12 11:00 wt (session 7b5e8856): blocked on question: Q1\n- 2026-10-12 11:30 owner: unblocked: answered\n- 2026-10-12 12:00 wt (session 7b5e8856): stage building → testing (gate full: green)\n- 2026-10-12 13:00 qa (session 4d517d9d): stage testing → your-check\n";

    /// D3 (mu-172): agent time is the first move into Building to the last move out of Testing,
    /// less the time blocked, paused or waiting in Your check, each minute once; unfinished work
    /// runs to now. The same in any zone, as long as the stamps and now are read in it.
    #[test]
    fn agent_minutes_run_from_the_first_claim_to_leaving_testing_less_stops() {
        for zone in ["UTC", "Etc/GMT-2"] {
            let epoch = |s: &str| queue::stamp_epoch(s, Some(zone));
            let at = |s: &str| u64::try_from(epoch(s).unwrap()).unwrap();
            let done = at("2026-10-14 09:00");
            // 10:00 → 13:00 is 180 min, blocked 11:00–11:30.
            let finished = staged("your-check", BUILT);
            assert_eq!(agent_minutes(&finished, done, &epoch), Some(150), "{zone}");
            // A pause 11:15–11:45 overlaps the block: 11:00–11:45 is left out once.
            let paused = staged(
                "your-check",
                &BUILT.replace(
                    "- 2026-10-12 12:00",
                    "- 2026-10-12 11:15 owner: paused (lunch)\n- 2026-10-12 11:45 owner: resumed after 30 min\n- 2026-10-12 12:00",
                ),
            );
            assert_eq!(paused.pause_history.len(), 1);
            assert_eq!(agent_minutes(&paused, done, &epoch), Some(135), "{zone}");
            // Still in Building: it runs to now.
            let building = staged(
                "building",
                "- 2026-10-12 10:00 wt (session 7b5e8856): stage ready → building (claimed)\n",
            );
            assert_eq!(
                agent_minutes(&building, at("2026-10-12 10:45"), &epoch),
                Some(45),
                "{zone}"
            );
            // Paused now: the open pause runs to now too.
            let held = staged(
                "building",
                "- 2026-10-12 10:00 wt (session 7b5e8856): stage ready → building (claimed)\n- 2026-10-12 10:30 owner: paused\n",
            );
            assert_eq!(
                agent_minutes(&held, at("2026-10-12 11:00"), &epoch),
                Some(30),
                "{zone}"
            );
            // The owner sent it back: 10:00–13:00, then 15:00–16:00; the wait in Your check
            // 13:00–15:00 is the owner's, not agent time.
            let back = "- 2026-10-12 10:00 wt (session 7b5e8856): stage ready → building (claimed)\n- 2026-10-12 12:00 wt (session 7b5e8856): stage building → testing (gate full: green)\n- 2026-10-12 13:00 qa (session 4d517d9d): stage testing → your-check\n- 2026-10-12 15:00 owner: stage your-check → building (returned: the colours)\n- 2026-10-12 16:00 wt (session 7b5e8856): stage building → testing (gate full: green)\n- 2026-10-12 16:00 qa (session 4d517d9d): stage testing → your-check\n";
            assert_eq!(
                agent_minutes(&staged("your-check", back), done, &epoch),
                Some(240),
                "{zone}"
            );
            // Back in Building after the send-back, at 15:30: 180 + 30.
            let again = staged(
                "building",
                &back.replace("- 2026-10-12 16:00 wt (session 7b5e8856): stage building → testing (gate full: green)\n- 2026-10-12 16:00 qa (session 4d517d9d): stage testing → your-check\n", ""),
            );
            assert_eq!(
                agent_minutes(&again, at("2026-10-12 15:30"), &epoch),
                Some(210),
                "{zone}"
            );
            // Done after Your check: still ends where it left Testing.
            let accepted = staged(
                "done",
                &format!("{BUILT}- 2026-10-13 09:00 owner: stage your-check → done\n"),
            );
            assert_eq!(agent_minutes(&accepted, done, &epoch), Some(150), "{zone}");
            // Released back to Ready: it ends at its last move out of Building.
            let released = staged(
                "ready",
                "- 2026-10-12 10:00 wt (session 7b5e8856): stage ready → building (claimed)\n- 2026-10-12 10:20 wt (session 7b5e8856): stage building → ready (released)\n",
            );
            assert_eq!(agent_minutes(&released, done, &epoch), Some(20), "{zone}");
            // Never claimed.
            let ready = staged("ready", "- 2026-10-12 09:00 capture: created\n");
            assert_eq!(agent_minutes(&ready, done, &epoch), None, "{zone}");
        }
        // Stamps that can't be read as times.
        let finished = staged("your-check", BUILT);
        assert_eq!(agent_minutes(&finished, NOW, &|_| None), None);
        let bad_day = staged(
            "your-check",
            &BUILT.replace("2026-10-12 12:00", "2026-02-30 12:00"),
        );
        assert_eq!(
            agent_minutes(&bad_day, NOW, &|s| queue::stamp_epoch(s, Some("UTC"))),
            None
        );
        // Now before the claim (a clock set back): no time rather than a negative one.
        let building = staged(
            "building",
            "- 2026-10-12 10:00 wt (session 7b5e8856): stage ready → building (claimed)\n",
        );
        let utc = |s: &str| queue::stamp_epoch(s, Some("UTC"));
        let before = u64::try_from(utc("2026-10-12 09:00").unwrap()).unwrap();
        assert_eq!(agent_minutes(&building, before, &utc), None);
    }

    /// mu-172: the task the owner looked at, est-time 1d and est-tokens 1.5M. Its builder A
    /// (claimed it) and the lead's session L (it passed it on here, and holds another task of the
    /// queue, so most of its transcript is that work).
    const OWNERS: &str = "---
id: 0025
stage: your-check
status: doing
est-time: 1d
est-tokens: 1.5M
claimed-by:
held-by:
---

# The owner's task

## Log
- 2026-10-12 09:00 capture: created
- 2026-10-12 10:00 asghar (main · session a0a0a0a0 · cc-mg): stage ready → building (claimed)
- 2026-10-12 11:00 asghar (main · session a0a0a0a0 · cc-mg): blocked on question: Q1
- 2026-10-12 11:30 owner: unblocked: answered
- 2026-10-12 12:00 asghar (main · session a0a0a0a0 · cc-mg): stage building → testing (gate full: green)
- 2026-10-12 13:00 lead (main · session 1ead0001 · cc-hg): stage testing → your-check
";

    const LEADS: &str = "---\nid: 0026\nstage: building\nstatus: doing\nclaimed-by: lead (main · session 1ead0001 · cc-hg)\n---\n\n# The lead's own task\n\n## Log\n- 2026-10-12 05:00 lead (main · session 1ead0001 · cc-hg): stage ready → building (claimed)\n";

    /// The owner's queue on disk, and a home with A's and L's transcripts: A 150 minutes,
    /// L 290 minutes, their kinds of tokens adding up to 168,060,000 all told.
    fn owners_fixture() -> (crate::testkit::TestDir, crate::testkit::TestDir) {
        let q = queue();
        fs::write(q.path().join("doing/0025-owners.md"), OWNERS).unwrap();
        fs::write(q.path().join("doing/0026-leads.md"), LEADS).unwrap();
        let h = home();
        write(
            h.path(),
            ".claude-mg",
            "a0a0a0a0-0000",
            &[
                prompt("2026-10-12T08:00:00Z", "build 0025"),
                reply(
                    "a1",
                    "2026-10-12T10:30:00Z",
                    2_000,
                    600_000,
                    40_000_000,
                    1_200_000,
                ),
            ],
            NOW - 3600,
        );
        write(
            h.path(),
            ".claude",
            "1ead0001-0000",
            &[
                prompt("2026-10-12T05:00:00Z", "lead"),
                reply(
                    "l1",
                    "2026-10-12T09:50:00Z",
                    8_000,
                    250_000,
                    124_000_000,
                    2_000_000,
                ),
            ],
            NOW - 3600,
        );
        (q, h)
    }

    #[test]
    fn the_owners_fixture_168m_of_1_5m_and_7_3_h_of_1_d_now_reads_1_8m_and_2_5_h() {
        let (q, h) = owners_fixture();
        let (r, a) = reader(h.path());
        let w = for_task(&r, &a, q.path(), "0025", NOW).unwrap();
        let ids: Vec<(&str, usize)> = w
            .rows
            .iter()
            .map(|r| (r.session.id.as_str(), r.shared_with))
            .collect();
        assert_eq!(ids, [("a0a0a0a0", 0), ("1ead0001", 1)]);
        assert!(w.rows.iter().all(|r| r.usage.found == Found::Read));
        // What the section said before: every kind of every session's tokens, and every
        // session's transcript span, against the estimates.
        let old_tokens: u64 = w.rows.iter().map(|r| r.usage.tokens.total()).sum();
        let old_minutes: u64 = w.rows.iter().map(|r| r.usage.minutes).sum();
        assert_eq!(old_tokens, 168_060_000);
        assert_eq!(pct(old_tokens, w.estimate.tokens), Some(11204));
        assert_eq!(old_minutes, 440);
        assert_eq!(pct(old_minutes, w.estimate.minutes), Some(92));
        // What it says now: A's input, output and cache writes against 1.5M; its 40M cache
        // reads shown, not counted; L kept out as shared; agent time against 1d (480 min).
        assert_eq!((w.estimate.minutes, w.estimate.tokens), (480, 1_500_000));
        assert_eq!(w.totals.counted_tokens, 1_802_000);
        assert_eq!(w.estimate.tokens_pct, Some(120));
        assert_eq!(w.totals.tokens.cache_read, 40_000_000);
        assert_eq!(w.totals.total_tokens, 41_802_000);
        assert_eq!(
            w.totals.shared_tokens,
            Tokens {
                input: 8_000,
                output: 250_000,
                cache_read: 124_000_000,
                cache_write: 2_000_000
            }
        );
        assert_eq!(w.totals.shared_tokens.counted(), 2_258_000);
        assert_eq!((w.totals.shared, w.totals.shared_read), (1, 1));
        assert_eq!((w.totals.sessions, w.totals.read), (2, 2));
        // 10:00 → 13:00, blocked 11:00–11:30.
        assert_eq!(w.totals.agent_minutes, Some(150));
        assert_eq!(w.estimate.minutes_pct, Some(31));
        // The own sessions' transcript span, for the breakdown.
        assert_eq!(w.totals.minutes, 150);
        // The window's names for them.
        let json = serde_json::to_value(&w).unwrap();
        assert_eq!(json["totals"]["countedTokens"], 1_802_000);
        assert_eq!(json["totals"]["sharedTokens"]["cacheWrite"], 2_000_000);
        assert_eq!(json["totals"]["sharedRead"], 1);
        assert_eq!(json["totals"]["agentMinutes"], 150);
        // The card's ribbon says the same number, from the same split.
        let mut tasks = queue::read_state(
            &[("q".into(), q.path().to_path_buf())],
            &mut queue::Cache::default(),
        )
        .tasks;
        tasks.sort_by(|x, y| x.id.cmp(&y.id));
        mark_tokens(&mut tasks, &r, &a, NOW, &mut TokenMemo::default());
        // 0025 is in Your check: no ribbon there. Moved back to Testing it gets one.
        assert_eq!(
            (tasks[0].tokens_used, tasks[0].tokens_shared),
            (None, false)
        );
        tasks[0].stage = "testing".into();
        mark_tokens(&mut tasks, &r, &a, NOW, &mut TokenMemo::default());
        assert_eq!(tasks[0].tokens_used, Some(w.totals.counted_tokens));
        assert!(tasks[0].tokens_shared);
        // The lead's own task names only L, which 0025 names too: read, all of it kept out.
        assert_eq!(
            (tasks[1].tokens_used, tasks[1].tokens_shared),
            (Some(0), true)
        );
        let json = serde_json::to_value(&tasks[0]).unwrap();
        assert_eq!(json["tokensUsed"], 1_802_000);
        assert_eq!(json["tokensShared"], true);
    }

    #[test]
    fn one_split_keeps_shared_sessions_apart() {
        let read = |i, o, r, w, minutes| Usage {
            found: Found::Read,
            tokens: Tokens {
                input: i,
                output: o,
                cache_read: r,
                cache_write: w,
            },
            minutes,
            ..Usage::missing()
        };
        let usages = [
            read(1, 2, 300, 4, 10),
            read(10, 20, 3000, 40, 20),
            Usage::missing(),
            Usage::not_read(),
            read(100, 200, 30000, 400, 40),
        ];
        let s = split(&usages, &[false, true, true, false, false]);
        assert_eq!(
            s,
            Split {
                own: Tokens {
                    input: 101,
                    output: 202,
                    cache_read: 30300,
                    cache_write: 404
                },
                read: 2,
                minutes: 50,
                shared: Tokens {
                    input: 10,
                    output: 20,
                    cache_read: 3000,
                    cache_write: 40
                },
                shared_read: 1,
            }
        );
        // Nothing shared, nothing read, and flags shorter than the usages (own, then).
        assert_eq!(split(&usages, &[]).shared_read, 0);
        assert_eq!(split(&usages, &[]).read, 3);
        assert_eq!(split(&[], &[]), Split::default());
    }

    #[test]
    fn the_ribbon_counts_only_tasks_being_worked_on_by_agents() {
        let h = home();
        transcripts(h.path());
        let (r, a) = reader(h.path());
        let mut human = parsed("0006", "building", "wt (session 7b5e8856 · cc-hg)");
        human.for_who = "human".into();
        let mut tasks = vec![
            parsed("0001", "shaping", "wt (session 7b5e8856 · cc-hg)"),
            parsed("0002", "building", "qa (session 4d517d9d · cc-mg)"),
            parsed("0003", "testing", "wt (session 0000aaaa · cc-mg)"),
            parsed("0004", "your-check", "wt (session 7b5e8856 · cc-hg)"),
            parsed("0005", "ready", "wt (session 7b5e8856 · cc-hg)"),
            human,
            // Not stamped with an account Muster reads: nothing read.
            parsed("0007", "building", "wt (session 7b5e8856)"),
        ];
        // Each task in its own queue, so no session is shared.
        for (n, t) in tasks.iter_mut().enumerate() {
            t.queue = format!("q{n}");
        }
        mark_tokens(&mut tasks, &r, &a, NOW, &mut TokenMemo::default());
        let got: Vec<(&str, Option<u64>, bool)> = tasks
            .iter()
            .map(|t| (t.id.as_str(), t.tokens_used, t.tokens_shared))
            .collect();
        assert_eq!(
            got,
            [
                ("0001", Some(8015), false),
                ("0002", Some(1501), false),
                // Its session's transcript isn't on this Mac.
                ("0003", None, false),
                ("0004", None, false),
                ("0005", None, false),
                ("0006", None, false),
                ("0007", None, false),
            ]
        );
        // In one queue, 0001 and 0004 name the same session: it's shared, and kept out.
        for t in tasks.iter_mut() {
            t.queue = "q".into();
        }
        mark_tokens(&mut tasks, &r, &a, NOW, &mut TokenMemo::default());
        assert_eq!(
            (tasks[0].tokens_used, tasks[0].tokens_shared),
            (Some(0), true)
        );
        assert_eq!(
            (tasks[1].tokens_used, tasks[1].tokens_shared),
            (Some(1501), false)
        );
        // Off a card it's null, not 0.
        let json = serde_json::to_value(&tasks[3]).unwrap();
        assert!(json["tokensUsed"].is_null() && json["tokensShared"] == false);
    }

    #[test]
    fn the_ribbon_reuses_a_count_for_30_s_unless_its_sessions_change() {
        let h = home();
        let lines = |n: u64| -> Vec<String> {
            (0..n)
                .map(|i| reply(&format!("m{i}"), "2026-10-12T08:00:00Z", 1, 10, 100, 1000))
                .collect()
        };
        let file = write(h.path(), ".claude", A, &lines(1), NOW);
        let (r, a) = reader(h.path());
        let mut memo = TokenMemo::default();
        let mut tasks = vec![parsed("0001", "building", "wt (session 7b5e8856 · cc-hg)")];
        let at = |tasks: &mut Vec<Task>, memo: &mut TokenMemo, now| {
            mark_tokens(tasks, &r, &a, now, memo);
            tasks[0].tokens_used
        };
        assert_eq!(at(&mut tasks, &mut memo, NOW), Some(1011));
        // The live session goes on writing.
        fs::write(&file, lines(3).join("\n") + "\n").unwrap();
        transcripts::tests::touch(&file, NOW + 10);
        assert_eq!(at(&mut tasks, &mut memo, NOW + 10), Some(1011));
        assert_eq!(at(&mut tasks, &mut memo, NOW + 29), Some(1011));
        assert_eq!(at(&mut tasks, &mut memo, NOW + 30), Some(3033));
        // Another session named: counted at once.
        write(h.path(), ".claude-mg", B, &lines(2), NOW + 31);
        tasks[0] = staged(
            "building",
            "- 2026-10-12 10:00 wt (session 7b5e8856 · cc-hg): stage ready → building (claimed)\n- 2026-10-12 11:00 qa (session 4d517d9d · cc-mg): looked at it\n",
        );
        assert_eq!(tasks[0].session_ids.len(), 2);
        assert_eq!(at(&mut tasks, &mut memo, NOW + 31), Some(3033 + 2022));
        // Bounded by the cards marked: a task that left the board leaves the memo.
        assert_eq!(memo.len(), 1);
        tasks[0].stage = "your-check".into();
        at(&mut tasks, &mut memo, NOW + 32);
        assert_eq!(memo.len(), 0);
    }

    /// E5 (mu-172): agent time is Building + Testing stage time. With no release and no send-back
    /// from Your check that's mu-168's `measured::agent_minutes`, on mu-168's own cases; with
    /// one, the time back in Ready or waiting in Your check isn't agent time (D3).
    #[test]
    fn agent_minutes_match_mu_168_on_its_own_cases() {
        let utc = |s: &str| queue::stamp_epoch(s, Some("UTC"));
        let now = u64::try_from(utc("2026-10-03 09:00").unwrap()).unwrap();
        let task = |lines: &[String]| staged("done", &(lines.join("\n") + "\n"));
        let stage = |at: &str, from: &str, to: &str| format!("- {at} akbar: stage {from} → {to}");
        let steps = |log: &[(&str, &str, &str)]| -> Vec<String> {
            log.iter().map(|(at, f, t)| stage(at, f, t)).collect()
        };
        let built = |mins: u32| {
            let at = |m: u32| format!("2026-10-01 {:02}:{:02}", 9 + m / 60, m % 60);
            steps(&[
                (&at(0), "ready", "building"),
                (&at(mins / 2), "building", "testing"),
                (&at(mins), "testing", "your-check"),
            ])
        };
        // (log, mu-168's number) where both agree.
        let same: Vec<(Vec<String>, u64)> = vec![
            (built(30), 30),
            (built(60), 60),
            (built(15), 15),
            (built(90), 90),
            (built(120), 120),
            // Sent back from Testing.
            (
                steps(&[
                    ("2026-10-01 09:00", "ready", "building"),
                    ("2026-10-01 10:00", "building", "testing"),
                    ("2026-10-01 10:30", "testing", "building"),
                    ("2026-10-01 11:00", "building", "testing"),
                    ("2026-10-01 12:00", "testing", "done"),
                ]),
                180,
            ),
            // Blocked 09:30–10:30 and paused 10:00–11:00: 90 min stopped, each minute once.
            (
                vec![
                    stage("2026-10-01 09:00", "ready", "building"),
                    "- 2026-10-01 09:30 akbar: blocked on question: Q1".into(),
                    "- 2026-10-01 10:00 owner: paused (lunch)".into(),
                    "- 2026-10-01 10:30 owner: unblocked: Q1 answered".into(),
                    "- 2026-10-01 11:00 owner: resumed after 1 h".into(),
                    stage("2026-10-01 11:30", "building", "testing"),
                    stage("2026-10-01 12:00", "testing", "your-check"),
                ],
                90,
            ),
            // A block from before the claim counts from the claim; one still open runs on.
            (
                vec![
                    "- 2026-10-01 08:00 akbar: blocked on outside: Apple review".into(),
                    stage("2026-10-01 09:00", "ready", "building"),
                    "- 2026-10-01 09:15 owner: unblocked: approved".into(),
                    "- 2026-10-01 09:30 akbar: blocked on you: which colour".into(),
                    stage("2026-10-01 10:00", "testing", "your-check"),
                ],
                15,
            ),
            // A pause after leaving Testing isn't agent time.
            (
                vec![
                    stage("2026-10-01 09:00", "ready", "building"),
                    stage("2026-10-01 10:00", "testing", "your-check"),
                    "- 2026-10-01 10:30 owner: paused".into(),
                ],
                60,
            ),
        ];
        for (log, want) in &same {
            let t = task(log);
            assert_eq!(agent_minutes(&t, now, &utc), Some(*want), "{log:?}");
            // mu-168's own function, now on main, gives the same number.
            assert_eq!(
                crate::measured::agent_minutes(&t),
                i64::try_from(*want).ok(),
                "mu-168: {log:?}"
            );
        }
        // Sent back from Your check: mu-168 says 195 (09:00 → 12:15); the owner's wait in Your
        // check 10:00–11:00 isn't agent time.
        let again = steps(&[
            ("2026-10-01 09:00", "ready", "building"),
            ("2026-10-01 09:30", "building", "testing"),
            ("2026-10-01 10:00", "testing", "your-check"),
            ("2026-10-01 11:00", "your-check", "building"),
            ("2026-10-01 11:30", "building", "testing"),
            ("2026-10-01 12:15", "testing", "your-check"),
        ]);
        assert_eq!(agent_minutes(&task(&again), now, &utc), Some(135));
        assert_eq!(crate::measured::agent_minutes(&task(&again)), Some(195));
        // Back to Testing from Your check: mu-168 150, here 30 + 30 + 30.
        let w = steps(&[
            ("2026-10-01 09:00", "ready", "building"),
            ("2026-10-01 09:30", "building", "testing"),
            ("2026-10-01 10:00", "testing", "your-check"),
            ("2026-10-01 11:00", "your-check", "testing"),
            ("2026-10-01 11:30", "testing", "your-check"),
            ("2026-10-01 14:00", "your-check", "done"),
        ]);
        assert_eq!(agent_minutes(&task(&w), now, &utc), Some(90));
        assert_eq!(crate::measured::agent_minutes(&task(&w)), Some(150));
        // Released back to Ready and claimed again: the stretch in Ready isn't work.
        let released = steps(&[
            ("2026-10-01 09:00", "ready", "building"),
            ("2026-10-01 09:30", "building", "ready"),
            ("2026-10-01 11:00", "ready", "building"),
            ("2026-10-01 11:30", "building", "testing"),
            ("2026-10-01 12:00", "testing", "your-check"),
        ]);
        assert_eq!(agent_minutes(&task(&released), now, &utc), Some(90));
    }

    /// R1/R2 (mu-172): the owner's table by stage. Shaping 09:00 (created there) → Ready 09:30 →
    /// Building 10:00 → Testing 12:00 → Your check 13:00, blocked 11:00–11:30. S shaped and
    /// built it, B built and tested it, L (the lead's, claimed by another task) looked in
    /// during Testing.
    const STAGED: &str = "---
id: 0025
stage: your-check
status: doing
est-time: 1d
est-tokens: 1.5M
claimed-by:
held-by:
---

# A task by stage

## Log
- 2026-10-12 09:00 capture: created
- 2026-10-12 09:05 shapour (main · session 5e550001 · cc-mg): shaping it
- 2026-10-12 09:30 shapour (main · session 5e550001 · cc-mg): stage shaping → ready
- 2026-10-12 10:00 shapour (main · session 5e550001 · cc-mg): stage ready → building (claimed)
- 2026-10-12 11:00 shapour (main · session 5e550001 · cc-mg): blocked on question: Q1
- 2026-10-12 11:30 owner: unblocked: answered
- 2026-10-12 11:40 gholi (main · session b0b00002 · cc-mg): helping
- 2026-10-12 12:00 gholi (main · session b0b00002 · cc-mg): stage building → testing (gate full: green)
- 2026-10-12 12:30 lead (main · session 1ead0001 · cc-hg): looked at it
- 2026-10-12 13:00 gholi (main · session b0b00002 · cc-mg): stage testing → your-check
";

    #[test]
    fn the_work_section_splits_tokens_and_time_by_stage() {
        let q = queue();
        fs::write(q.path().join("doing/0025-staged.md"), STAGED).unwrap();
        fs::write(q.path().join("doing/0026-leads.md"), LEADS).unwrap();
        let h = home();
        // A turn's input, output, cache reads and cache writes.
        type Used = (u64, u64, u64, u64);
        let out = |n| (0, n, 0, 0);
        let session = |account: &str, name: &str, turns: &[(&str, Used)]| {
            let lines: Vec<String> = turns
                .iter()
                .enumerate()
                .map(|(i, (at, (i_, o, r, w)))| {
                    // "" is a reply with no time stamp at all.
                    let stamp = if at.is_empty() {
                        String::new()
                    } else {
                        format!("2026-10-12T{at}:00Z")
                    };
                    let line = reply(&format!("{name}{i}"), &stamp, *i_, *o, *r, *w);
                    line.replace(r#""timestamp":"","#, "")
                })
                .collect();
            write(
                h.path(),
                account,
                &format!("{name}-0000"),
                &lines,
                NOW - 3600,
            );
        };
        session(
            ".claude-mg",
            "5e550001",
            &[
                ("09:05", (0, 1_000, 50_000, 0)),
                ("09:20", out(2_000)),
                // In Ready: no stage row of its own, so "other" (R2), with no time there.
                ("09:35", out(0)),
                ("09:45", out(4_000)),
                ("10:10", (0, 8_000, 0, 10_000)),
                ("11:50", (0, 16_000, 100_000, 0)),
            ],
        );
        session(
            ".claude-mg",
            "b0b00002",
            &[
                ("11:40", out(32_000)),
                ("12:20", out(64_000)),
                // No time stamp: no stage can be told, so "other" keeps it (review R3), and
                // nothing B spent is dropped from the rows.
                ("", out(7_000)),
                ("12:50", out(128_000)),
            ],
        );
        session(
            ".claude",
            "1ead0001",
            &[
                ("08:00", out(500_000)),
                ("12:30", (0, 1_000_000, 5_000_000, 0)),
            ],
        );
        let utc = |s: &str| queue::stamp_epoch(s, Some("UTC"));
        let now = u64::try_from(utc("2026-10-12 15:00").unwrap()).unwrap();
        let tasks = queue::read_state(
            &[("q".into(), q.path().to_path_buf())],
            &mut queue::Cache::default(),
        )
        .tasks;
        let task = tasks.iter().find(|t| t.id == "0025").unwrap();
        let (r, a) = reader(h.path());
        let w = work(
            &r,
            &a,
            task,
            body(STAGED),
            now,
            &shared_counts(&tasks, "0025"),
            &utc,
        );
        let tok = |i, o, cr, cw| Tokens {
            input: i,
            output: o,
            cache_read: cr,
            cache_write: cw,
        };
        let sess = |id: &str, tokens: Tokens, minutes, shared| StageSession {
            id: id.into(),
            tokens,
            counted_tokens: tokens.counted(),
            minutes,
            shared,
        };
        let row = |stage: &str, minutes, tokens: Tokens, shared_tokens, sessions| StageRow {
            stage: stage.into(),
            minutes,
            tokens,
            counted_tokens: tokens.counted(),
            shared_tokens,
            sessions,
        };
        let none = Tokens::default();
        assert_eq!(
            w.stages,
            [
                row(
                    "shaping",
                    30,
                    tok(0, 3_000, 50_000, 0),
                    none,
                    vec![sess("5e550001", tok(0, 3_000, 50_000, 0), 15, false)]
                ),
                // 120 min less the half hour blocked; S's turns 10:10 and 11:50 split from
                // its Shaping ones by their time.
                row(
                    "building",
                    90,
                    tok(0, 56_000, 100_000, 10_000),
                    none,
                    vec![
                        sess("5e550001", tok(0, 24_000, 100_000, 10_000), 100, false),
                        sess("b0b00002", tok(0, 32_000, 0, 0), 0, false),
                    ]
                ),
                // L is listed, shared, and its tokens are kept out of the row's.
                row(
                    "testing",
                    60,
                    tok(0, 192_000, 0, 0),
                    tok(0, 1_000_000, 5_000_000, 0),
                    vec![
                        sess("b0b00002", tok(0, 192_000, 0, 0), 30, false),
                        sess("1ead0001", tok(0, 1_000_000, 5_000_000, 0), 0, true),
                    ]
                ),
                // The owner's wait, to now.
                row("your-check", 120, none, none, vec![]),
                // S's turn in Ready, B's with no time stamp and L's from before the task was made.
                row(
                    "other",
                    0,
                    tok(0, 11_000, 0, 0),
                    tok(0, 500_000, 0, 0),
                    vec![
                        sess("5e550001", tok(0, 4_000, 0, 0), 0, false),
                        sess("b0b00002", tok(0, 7_000, 0, 0), 0, false),
                        sess("1ead0001", tok(0, 500_000, 0, 0), 0, true),
                    ]
                ),
            ]
        );
        // The total: the own rows add up to the section's counted tokens and the card's.
        let rows: u64 = w.stages.iter().map(|s| s.counted_tokens).sum();
        assert_eq!(rows, 272_000);
        assert_eq!(w.totals.counted_tokens, rows);
        assert_eq!(w.totals.agent_minutes, Some(150));
        let mut cards = tasks.clone();
        for t in cards.iter_mut().filter(|t| t.id == "0025") {
            t.stage = "testing".into();
        }
        mark_tokens(&mut cards, &r, &a, now, &mut TokenMemo::default());
        let card = cards.iter().find(|t| t.id == "0025").unwrap();
        assert_eq!(card.tokens_used, Some(rows));
        // What the window gets.
        let json = serde_json::to_value(&w).unwrap();
        let testing = &json["stages"][2];
        assert_eq!(testing["stage"], "testing");
        assert_eq!(testing["countedTokens"], 192_000);
        assert_eq!(testing["sharedTokens"]["output"], 1_000_000);
        assert_eq!(testing["sessions"][1]["id"], "1ead0001");
        assert_eq!(testing["sessions"][1]["shared"], true);
        assert_eq!(testing["sessions"][1]["countedTokens"], 1_000_000);
        assert_eq!(json["stages"][0]["sessions"][0]["minutes"], 15);
    }

    #[test]
    fn a_task_never_moved_has_its_stage_from_when_it_was_made_and_bad_stamps_put_all_in_other() {
        let h = home();
        write(
            h.path(),
            ".claude-mg",
            "5e550001-0000",
            &[
                reply("a", "2026-10-12T08:30:00Z", 0, 7, 0, 0),
                reply("b", "2026-10-12T09:10:00Z", 0, 5, 0, 0),
            ],
            NOW - 3600,
        );
        let (r, a) = reader(h.path());
        let utc = |s: &str| queue::stamp_epoch(s, Some("UTC"));
        let now = u64::try_from(utc("2026-10-12 09:40").unwrap()).unwrap();
        let none = HashMap::new();
        // Made in Shaping at 09:00 and shaped since: one Shaping row, to now.
        let log = "- 2026-10-12 09:00 shapour (main · session 5e550001 · cc-mg): created\n";
        let t = staged("shaping", log);
        let w = work(
            &r,
            &a,
            &t,
            &format!("# T\n\n## Log\n{log}"),
            now,
            &none,
            &utc,
        );
        let got: Vec<(&str, u64, u64)> = w
            .stages
            .iter()
            .map(|s| (s.stage.as_str(), s.minutes, s.counted_tokens))
            .collect();
        assert_eq!(got, [("shaping", 40, 5), ("other", 0, 7)]);
        assert_eq!(w.totals.agent_minutes, None);
        // A stamp that can't be read as a time: no stage can be told, all of it is "other".
        let log = "- 2026-10-12 09:00 shapour (main · session 5e550001 · cc-mg): created\n- 2026-02-30 09:20 shapour (main · session 5e550001 · cc-mg): stage shaping → ready\n";
        let t = staged("ready", log);
        assert_eq!(t.stage_history.len(), 1);
        let w = work(
            &r,
            &a,
            &t,
            &format!("# T\n\n## Log\n{log}"),
            now,
            &none,
            &utc,
        );
        let got: Vec<(&str, u64, u64)> = w
            .stages
            .iter()
            .map(|s| (s.stage.as_str(), s.minutes, s.counted_tokens))
            .collect();
        assert_eq!(got, [("other", 0, 12)]);
        // Nothing read and never moved: no rows at all.
        let t = staged("ready", "- 2026-10-12 09:00 capture: created\n");
        let w = work(&r, &a, &t, "# T\n\n## Log\n", now, &none, &utc);
        assert!(w.stages.is_empty());
    }

    /// Review (mu-172): a session's minutes in a stage the task was in twice are counted per
    /// stretch, never across the time it spent in another stage in between.
    #[test]
    fn a_sessions_minutes_in_a_stage_entered_twice_skip_the_time_between() {
        let log = "- 2026-10-12 09:00 capture: created\n- 2026-10-12 10:00 wt (main · session 5e550001 · cc-mg): stage ready → building (claimed)\n- 2026-10-12 12:00 wt (main · session 5e550001 · cc-mg): stage building → testing (gate full: green)\n- 2026-10-12 13:00 qa: stage testing → building\n- 2026-10-12 14:00 wt (main · session 5e550001 · cc-mg): stage building → testing (gate full: green)\n- 2026-10-12 15:00 qa: stage testing → your-check\n- 2026-10-12 13:20 owner: paused\n- 2026-10-12 13:30 owner: resumed after 10 min\n";
        let h = home();
        write(
            h.path(),
            ".claude-mg",
            "5e550001-0000",
            &["10:10", "11:50", "13:10", "13:50"]
                .iter()
                .enumerate()
                .map(|(i, at)| {
                    reply(
                        &format!("s{i}"),
                        &format!("2026-10-12T{at}:00Z"),
                        0,
                        1,
                        0,
                        0,
                    )
                })
                .collect::<Vec<_>>(),
            NOW - 3600,
        );
        let (r, a) = reader(h.path());
        let utc = |s: &str| queue::stamp_epoch(s, Some("UTC"));
        let now = u64::try_from(utc("2026-10-12 16:00").unwrap()).unwrap();
        let t = staged("your-check", log);
        assert_eq!(t.pause_history.len(), 1);
        let w = work(
            &r,
            &a,
            &t,
            &format!("# T\n\n## Log\n{log}"),
            now,
            &HashMap::new(),
            &utc,
        );
        let building = w.stages.iter().find(|s| s.stage == "building").unwrap();
        // 10:00–12:00 and 13:00–14:00, less the 10 minutes paused.
        assert_eq!(building.minutes, 170);
        // 10:10–11:50, then 13:10–13:50 less the pause: 100 + 30, not 10:10 → 13:50.
        assert_eq!(building.sessions[0].minutes, 130);
        assert!(building.sessions[0].minutes <= building.minutes);
        assert_eq!(building.sessions[0].counted_tokens, 4);
    }

    /// AC5 (mu-172): what the token ribbon adds to the window's 2 s poll. Ten held Building
    /// tasks, each with its own session; one is live, about 30 MB, and grows before every poll.
    /// After a warm-up poll, 150 polls of `mark_live` and `mark_tokens`, now 2 s later each:
    /// steady polls reuse the memo, every 15th (30 s) counts again and sums the grown transcript.
    /// `cargo test --release the_ribbon_adds_little -- --nocapture` prints the numbers; the bound
    /// is generous so a loaded machine doesn't fail the gate.
    #[test]
    fn the_ribbon_adds_little_to_the_poll() {
        let h = home();
        let ids: Vec<String> = (0..10u64).map(|n| format!("a{n:07x}")).collect();
        let mut tasks: Vec<Task> = ids
            .iter()
            .enumerate()
            .map(|(n, id)| {
                parsed(
                    &format!("{:04}", n + 1),
                    "building",
                    &format!("wt (session {id} · cc-mg)"),
                )
            })
            .collect();
        let line = |i: u64| {
            let s = i % 86_400;
            let at = format!(
                "2026-10-12T{:02}:{:02}:{:02}.000Z",
                s / 3600,
                s / 60 % 60,
                s % 60
            );
            reply(&format!("msg_{i:0>48}"), &at, 1, 2, 3, 4)
        };
        // The live one: about 30 MB.
        let mut lines: Vec<String> = Vec::new();
        let mut bytes = 0;
        while bytes < 30 * 1024 * 1024 {
            let l = line(lines.len() as u64);
            bytes += l.len() + 1;
            lines.push(l);
        }
        let mut replies = lines.len() as u64;
        let live = write(
            h.path(),
            ".claude-mg",
            &format!("{}-0000", ids[0]),
            &lines,
            NOW,
        );
        drop(lines);
        for id in &ids[1..] {
            let small: Vec<String> = (0..1000).map(line).collect();
            write(
                h.path(),
                ".claude-mg",
                &format!("{id}-0000"),
                &small,
                NOW - 3600,
            );
        }
        let (r, a) = reader(h.path());
        let mut memo = TokenMemo::default();
        // The warm-up poll: every transcript read cold.
        mark_live(&mut tasks, &r, &a, NOW);
        mark_tokens(&mut tasks, &r, &a, NOW, &mut memo);
        assert_eq!(tasks[0].tokens_used, Some(replies * 7));
        assert!(tasks.iter().all(|t| t.live == (t.id == "0001")));
        let (mut steady, mut refresh, mut live_check) = (Vec::new(), Vec::new(), Vec::new());
        for k in 1..=150u64 {
            let now = NOW + 2 * k;
            // The live session writes a reply between polls.
            let mut f = fs::OpenOptions::new().append(true).open(&live).unwrap();
            std::io::Write::write_all(&mut f, (line(replies) + "\n").as_bytes()).unwrap();
            drop(f);
            replies += 1;
            transcripts::tests::touch(&live, now);
            let t = Instant::now();
            mark_live(&mut tasks, &r, &a, now);
            live_check.push(t.elapsed());
            let t = Instant::now();
            mark_tokens(&mut tasks, &r, &a, now, &mut memo);
            let took = t.elapsed();
            if memo.entries.values().any(|c| c.at == now) {
                refresh.push(took);
                // A refresh poll counts every reply so far.
                assert_eq!(tasks[0].tokens_used, Some(replies * 7));
            } else {
                steady.push(took);
            }
        }
        assert_eq!((steady.len(), refresh.len()), (140, 10));
        let stats = |v: &mut Vec<std::time::Duration>| {
            v.sort();
            let at = |q: usize| v[(v.len() * q / 100).min(v.len() - 1)];
            (at(50), at(95), v[v.len() - 1])
        };
        let (s50, s95, smax) = stats(&mut steady);
        let (r50, r95, rmax) = stats(&mut refresh);
        let (l50, l95, lmax) = stats(&mut live_check);
        eprintln!("poll cost, 10 held cards, one live 30 MB transcript: mark_tokens steady (140) p50 {s50:?} p95 {s95:?} max {smax:?}; refresh (10) p50 {r50:?} p95 {r95:?} max {rmax:?}; mark_live alone (150) p50 {l50:?} p95 {l95:?} max {lmax:?}");
        assert!(s95.as_millis() < 50, "steady polls' p95 took {s95:?}");
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
