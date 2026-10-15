//! The run ledger (mu-114): one local record per agent run, its tokens by kind, wall time, turns,
//! context peak and outcome, under OpenTelemetry's GenAI attribute names (the version pinned in
//! `SCHEMA`) plus Muster's own `muster.*` fields. One JSON line per run in `ledger/YYYY-MM.jsonl`
//! in the app's config folder, by the month the run ended (UTC).
//!
//! The numbers are the task pane's: a session's record is made from its row of `work::work`
//! (`from_row`), which reads through `transcripts.rs` and counts as the Work section does, paused
//! stretches out of the time. A runner (mu-117) fills a `Record` from its run's `result` message
//! the same way and calls `record`. `backfill` writes one for each session stamped on a Done task.
//!
//! Privacy and security: nothing leaves the Mac (no exporter, research §7). A record holds ids,
//! labels, counts and times only: no title, prompt, transcript line, path or token, and `check`
//! refuses a field that could be one. Files are the user's alone (0600, the folder 0700), and a
//! month older than `KEEP_MONTHS` is removed on the next write.

use crate::accounts::Account;
use crate::queue::{self, Task};
use crate::roles::{self, Role, Slot};
use crate::transcripts::{Found, Reader};
use crate::work::{self, Row};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::{DirBuilderExt, FileExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// The ledger's folder, in the app's config folder.
pub const DIR: &str = "ledger";

/// The GenAI semantic conventions the `gen_ai.*` names follow: open-telemetry's
/// semantic-conventions-genai as read on 2026-10-14 (research §7, sources 60 and 61), all at
/// Status Development, so pinned by date. Every record carries it in `schema`.
pub const SCHEMA: &str = "otel-genai-semconv@2026-10-14";

/// Months of records kept, the current one included.
pub const KEEP_MONTHS: i64 = 13;

/// The longest text a field holds: ids and labels are short.
const MAX_FIELD: usize = 64;

/// The longest ledger line read back: a record is under 2 KB, so a longer line is hand-made and
/// skipped without being held whole.
const MAX_LINE: u64 = 8 * 1024;

/// A record's fields, in the order a line has them: the pinned list.
pub const FIELDS: [&str; 28] = [
    "schema",
    "muster.run",
    "gen_ai.operation.name",
    "gen_ai.provider.name",
    "gen_ai.agent.name",
    "gen_ai.agent.id",
    "gen_ai.conversation.id",
    "gen_ai.request.model",
    "gen_ai.usage.input_tokens",
    "gen_ai.usage.output_tokens",
    "gen_ai.usage.cache_read.input_tokens",
    "gen_ai.usage.cache_write.input_tokens",
    "muster.task",
    "muster.project",
    "muster.account",
    "muster.stage",
    "muster.started_at",
    "muster.ended_at",
    "muster.wall_secs",
    "muster.turns",
    "muster.context_peak",
    "muster.window_before",
    "muster.window_after",
    "muster.estimate",
    "muster.outcome",
    "muster.source",
    "muster.shared_with",
    "muster.counted_tokens",
];

/// How a run ended. The first three and `input_required` are A2A's task states; `limit` is an
/// account's window running out, `handed_off` a session that handed its task to a successor.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Completed,
    Failed,
    Cancelled,
    InputRequired,
    Limit,
    HandedOff,
}

/// Where a record came from: a run as it ended, or a finished task's transcripts afterwards.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Run,
    Backfill,
}

/// An account's window use at a moment, whole percent (mu-113's readings).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Window {
    pub five_hour: Option<u32>,
    pub seven_day: Option<u32>,
}

/// The task's estimate when the record was made: agent minutes and tokens (`est-time`,
/// `est-tokens`), 0 for none.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Estimate {
    pub minutes: u64,
    pub tokens: u64,
}

/// One run.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Record {
    pub schema: String,
    /// The run's id, what makes a second record of it a no-op: `<task key>:<session>` for a
    /// session's run.
    #[serde(rename = "muster.run")]
    pub run: String,
    /// Always `invoke_agent`.
    #[serde(rename = "gen_ai.operation.name")]
    pub operation: String,
    /// `anthropic` for a Claude account.
    #[serde(rename = "gen_ai.provider.name")]
    pub provider: String,
    /// The slot by its role's name (`Kaveh-2`), else the label the session acted under.
    #[serde(rename = "gen_ai.agent.name")]
    pub agent_name: String,
    /// The role's id (`coder`); "" for a session that took no slot.
    #[serde(rename = "gen_ai.agent.id")]
    pub agent_id: String,
    /// The session's short id.
    #[serde(rename = "gen_ai.conversation.id")]
    pub conversation: String,
    #[serde(rename = "gen_ai.request.model")]
    pub model: String,
    #[serde(rename = "gen_ai.usage.input_tokens")]
    pub input_tokens: u64,
    #[serde(rename = "gen_ai.usage.output_tokens")]
    pub output_tokens: u64,
    #[serde(rename = "gen_ai.usage.cache_read.input_tokens")]
    pub cache_read_tokens: u64,
    #[serde(rename = "gen_ai.usage.cache_write.input_tokens")]
    pub cache_write_tokens: u64,
    /// The task's key (`mu-114`).
    #[serde(rename = "muster.task")]
    pub task: String,
    #[serde(rename = "muster.project")]
    pub project: String,
    /// The Claude account's label (`hg`).
    #[serde(rename = "muster.account")]
    pub account: String,
    /// The stages the session held, comma-separated, in order.
    #[serde(rename = "muster.stage")]
    pub stage: String,
    /// UTC, ISO 8601 (`2026-10-15T07:22:00Z`); "" when the transcript has no time stamp.
    #[serde(rename = "muster.started_at")]
    pub started_at: String,
    #[serde(rename = "muster.ended_at")]
    pub ended_at: String,
    #[serde(rename = "muster.wall_secs")]
    pub wall_secs: u64,
    #[serde(rename = "muster.turns")]
    pub turns: u64,
    #[serde(rename = "muster.context_peak")]
    pub context_peak: u64,
    #[serde(rename = "muster.window_before")]
    pub window_before: Option<Window>,
    #[serde(rename = "muster.window_after")]
    pub window_after: Option<Window>,
    #[serde(rename = "muster.estimate")]
    pub estimate: Estimate,
    #[serde(rename = "muster.outcome")]
    pub outcome: Outcome,
    #[serde(rename = "muster.source")]
    pub source: Source,
    /// Other tasks that name the same session. Above 0 the Work section keeps its tokens out of
    /// the task's totals (mu-172), and a sum over the ledger should too.
    #[serde(rename = "muster.shared_with")]
    pub shared_with: usize,
    /// Input, output and cache writes: what counts against `est-tokens` (mu-172,
    /// `Tokens::counted`), the one count the Work section uses.
    #[serde(rename = "muster.counted_tokens")]
    pub counted_tokens: u64,
}

impl Record {
    /// The seconds since 1970 the run ended, else started; None with neither.
    pub fn ended_secs(&self) -> Option<u64> {
        crate::transcripts::iso_secs(&self.ended_at)
            .or_else(|| crate::transcripts::iso_secs(&self.started_at))
    }
}

/// Is `s` a value a record may hold: up to `MAX_FIELD` characters, none a control, a slash or a
/// backslash, and no run of spaces? Ids, labels, keys and project names pass; paths, prompts and
/// transcript lines don't.
fn plain(s: &str) -> bool {
    s.chars().count() <= MAX_FIELD
        && !s
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | '\u{2028}' | '\u{2029}'))
        && !s.contains("  ")
        && s.trim() == s
}

/// `s` when it's `plain`, else "".
fn clean(s: &str) -> String {
    if plain(s) {
        s.to_string()
    } else {
        String::new()
    }
}

/// Refuse a record with a field that could hold a path, a transcript line or free text: every
/// text field `plain` (at most 64 characters, no line break, slash or run of spaces), the model a
/// model id (`model_id`), the account a label, the agent id a role name, the session a short id
/// and the times ISO 8601, each but the run and the session or empty. A short single-spaced
/// phrase or a token-shaped word in a name or label still passes: the writer's callers fill
/// those from ids and names, never from a prompt.
pub fn check(r: &Record) -> Result<(), String> {
    let texts = [
        ("schema", &r.schema),
        ("muster.run", &r.run),
        ("gen_ai.operation.name", &r.operation),
        ("gen_ai.provider.name", &r.provider),
        ("gen_ai.agent.name", &r.agent_name),
        ("gen_ai.agent.id", &r.agent_id),
        ("gen_ai.request.model", &r.model),
        ("muster.task", &r.task),
        ("muster.project", &r.project),
        ("muster.account", &r.account),
        ("muster.stage", &r.stage),
    ];
    for (name, v) in texts {
        if !plain(v) {
            return Err(format!("{name} isn't a short plain label"));
        }
    }
    if !r.model.is_empty() && crate::transcripts::model_id(&r.model).is_none() {
        return Err("gen_ai.request.model isn't a model id".into());
    }
    if !r.account.is_empty() && !crate::accounts::is_label(&r.account) {
        return Err("muster.account isn't an account's label".into());
    }
    let role =
        (1..=20).contains(&r.agent_id.len()) && r.agent_id.bytes().all(|b| b.is_ascii_lowercase());
    if !r.agent_id.is_empty() && !role {
        return Err("gen_ai.agent.id isn't a role name".into());
    }
    if r.run.is_empty() {
        return Err("muster.run is empty: a record needs its run's id".into());
    }
    if !crate::transcripts::is_short_id(&r.conversation) {
        return Err("gen_ai.conversation.id isn't a session's short id".into());
    }
    for (name, v) in [
        ("muster.started_at", &r.started_at),
        ("muster.ended_at", &r.ended_at),
    ] {
        if !v.is_empty() && crate::transcripts::iso_secs(v).is_none() {
            return Err(format!("{name} isn't a UTC time"));
        }
    }
    Ok(())
}

/// Seconds since 1970 as UTC ISO 8601, `2026-10-15T07:22:00Z`.
pub fn iso(secs: u64) -> String {
    let days = i64::try_from(secs / 86400).unwrap_or(i64::MAX);
    let (y, m, d) = civil(days);
    let s = secs % 86400;
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        s / 3600,
        s % 3600 / 60,
        s % 60
    )
}

/// The date `days` after 1970-01-01 (Howard Hinnant's civil_from_days).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let m = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// The month a record goes in, `YYYY-MM`, from when it ended (else `now`).
fn month_of(r: &Record, now: u64) -> String {
    iso(r.ended_secs().unwrap_or(now))[..7].to_string()
}

/// A month as a count, for `KEEP_MONTHS`: `YYYY-MM` → year × 12 + month.
fn month_number(m: &str) -> Option<i64> {
    let (y, mm) = m.split_once('-')?;
    let ok =
        y.len() == 4 && mm.len() == 2 && (y.bytes().chain(mm.bytes())).all(|b| b.is_ascii_digit());
    let mm: i64 = mm.parse().ok().filter(|n| ok && (1..=12).contains(n))?;
    Some(y.parse::<i64>().ok()? * 12 + mm)
}

/// The ledger's folder in the app's config folder `dir`.
pub fn folder(dir: &Path) -> PathBuf {
    dir.join(DIR)
}

/// The month files in the ledger, oldest first, with their month.
fn months(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(rd) = fs::read_dir(folder(dir)) else {
        return Vec::new();
    };
    let mut out: Vec<(String, PathBuf)> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            let month = name.strip_suffix(".jsonl")?.to_string();
            let real = e.file_type().is_ok_and(|t| t.is_file());
            (real && month_number(&month).is_some()).then(|| (month, e.path()))
        })
        .collect();
    out.sort();
    out
}

/// Each well-formed record in the ledger, oldest month first and in the order written, streamed
/// line by line. The file is untrusted on read too (a hand edit): a line over `MAX_LINE`, one
/// that isn't a record, or a record `check` refuses (a control character, a path, free text) is
/// skipped, so nothing it holds reaches the terminal or a script.
fn each(dir: &Path, mut f: impl FnMut(Record)) {
    for (_, path) in months(dir) {
        let Ok(file) = fs::File::open(&path) else {
            continue;
        };
        let mut reader = BufReader::new(file);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match (&mut reader).take(MAX_LINE).read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            if buf.last() != Some(&b'\n') && buf.len() as u64 == MAX_LINE {
                // Over the cap: read past the rest of it, never holding it.
                let mut rest = Vec::new();
                while (&mut reader)
                    .take(MAX_LINE)
                    .read_until(b'\n', &mut rest)
                    .is_ok_and(|n| n > 0)
                    && rest.last() != Some(&b'\n')
                {
                    rest.clear();
                }
                continue;
            }
            if let Ok(r) = serde_json::from_slice::<Record>(&buf) {
                if check(&r).is_ok() {
                    f(r);
                }
            }
        }
    }
}

/// The run ids the ledger holds.
fn run_ids(dir: &Path) -> HashSet<String> {
    let mut out = HashSet::new();
    each(dir, |r| {
        out.insert(r.run);
    });
    out
}

/// What `read` keeps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filter {
    /// A task as typed (`key::parse`): `mu-114` keeps that project's, `114` any project's.
    pub task: Option<crate::key::Typed>,
    pub account: Option<String>,
    /// Runs that ended at or after this, in seconds since 1970.
    pub since: Option<u64>,
}

/// The records in the app's config folder `dir` that `filter` keeps, oldest first.
pub fn read(dir: &Path, filter: &Filter) -> Vec<Record> {
    let mut out = Vec::new();
    each(dir, |r| {
        let task = filter.task.as_ref().is_none_or(|t| {
            crate::key::parse(&r.task).is_some_and(|k| {
                k.id == t.id
                    && t.prefix
                        .as_ref()
                        .is_none_or(|p| k.prefix.as_ref() == Some(p))
            })
        });
        let account = filter.account.as_ref().is_none_or(|a| *a == r.account);
        let since = filter
            .since
            .is_none_or(|s| r.ended_secs().is_some_and(|e| e >= s));
        if task && account && since {
            out.push(r);
        }
    });
    out
}

/// Would a record ending in `month` be kept at `now`: within `KEEP_MONTHS` of now's month, and
/// not after the next one (a clock a little ahead, never a year 9999 no prune removes)?
fn kept(month: &str, now: u64) -> bool {
    match (month_number(month), month_number(&iso(now)[..7])) {
        (Some(m), Some(this)) => this - m < KEEP_MONTHS && m <= this + 1,
        _ => false,
    }
}

/// Write `records` to the ledger in the app's config folder `dir`, each once: under the ledger's
/// lock, a record whose run is already there (or earlier in `records`) is left out, and so is
/// one whose month isn't `kept` (it would go at once, and be written again next time). Returns
/// how many were written. Every record is checked first (`check`); one that fails refuses them
/// all. Months past `KEEP_MONTHS` before `now` go.
pub fn record_all(dir: &Path, records: &[Record], now: u64) -> Result<usize, String> {
    for r in records {
        check(r)?;
    }
    let folder = folder(dir);
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&folder)
        .map_err(|e| format!("the ledger's folder: {e}"))?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(folder.join(".lock"))
        .map_err(|e| format!("the ledger's lock: {e}"))?;
    lock.lock().map_err(|e| format!("the ledger's lock: {e}"))?;
    let mut seen = run_ids(dir);
    let mut written = 0;
    for r in records {
        if !kept(&month_of(r, now), now) || !seen.insert(r.run.clone()) {
            continue;
        }
        let line = serde_json::to_string(r).map_err(|e| e.to_string())? + "\n";
        let path = folder.join(format!("{}.jsonl", month_of(r, now)));
        let mut f = fs::OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .mode(0o600)
            .open(&path)
            .map_err(|e| format!("the ledger: {e}"))?;
        end_line(&mut f).map_err(|e| format!("the ledger: {e}"))?;
        f.write_all(line.as_bytes())
            .map_err(|e| format!("the ledger: {e}"))?;
        written += 1;
    }
    prune(dir, now);
    Ok(written)
}

/// End the month file `f` (open to append) with a line end when a write cut short (a crash, a
/// full disk) left its last line without one, so the next record starts a line of its own: the
/// torn line costs only itself, and a whole record that lost only its line end is read again.
fn end_line(f: &mut fs::File) -> std::io::Result<()> {
    let len = f.metadata()?.len();
    if len == 0 {
        return Ok(());
    }
    let mut last = [0u8];
    f.read_exact_at(&mut last, len - 1)?;
    if last[0] != b'\n' {
        f.write_all(b"\n")?;
    }
    Ok(())
}

/// Write one run's record (`record_all`); false when the ledger has its run already.
pub fn record(dir: &Path, r: &Record, now: u64) -> Result<bool, String> {
    record_all(dir, std::slice::from_ref(r), now).map(|n| n == 1)
}

/// Remove the month files more than `KEEP_MONTHS` months before `now`'s.
fn prune(dir: &Path, now: u64) {
    let Some(this) = month_number(&iso(now)[..7]) else {
        return;
    };
    for (month, path) in months(dir) {
        if month_number(&month).is_some_and(|m| this - m >= KEEP_MONTHS) {
            let _ = fs::remove_file(path);
        }
    }
}

/// The accounts' window readings (mu-113) in the app's config folder: each account's file read
/// and parsed once, however many sessions a backfill asks about.
pub struct Readings {
    dir: PathBuf,
    by_label: RefCell<HashMap<String, Vec<crate::usage::Reading>>>,
}

impl Readings {
    pub fn new(dir: &Path) -> Readings {
        Readings {
            dir: dir.to_path_buf(),
            by_label: RefCell::default(),
        }
    }

    /// `label`'s window use at `at`: the latest reading at or before `at` and not before
    /// `not_before`, each of its windows only while it hadn't reset by `at` (a window that has
    /// reset says nothing about its use after, `usage::Window::stale`). None without one.
    fn window_at(&self, label: &str, at: u64, not_before: u64) -> Option<Window> {
        if !crate::accounts::is_label(label) {
            return None;
        }
        let (at, not_before) = (i64::try_from(at).ok()?, i64::try_from(not_before).ok()?);
        let pct = |w: Option<crate::usage::Window>| {
            w.filter(|w| !w.stale(at))
                .map(|w| w.used_percentage)
                .filter(|p| p.is_finite() && (0.0..=crate::usage::MAX_PERCENT).contains(p))
                .and_then(|p| u32::try_from(p.round() as i64).ok())
        };
        let mut by_label = self.by_label.borrow_mut();
        let readings = by_label.entry(label.to_string()).or_insert_with(|| {
            fs::read_to_string(crate::usage::file(&self.dir, label))
                .unwrap_or_default()
                .lines()
                .filter_map(|l| serde_json::from_str::<crate::usage::Reading>(l).ok())
                .filter(|r| r.account == label)
                .collect()
        });
        readings
            .iter()
            .filter(|r| (not_before..=at).contains(&r.at))
            .max_by_key(|r| r.at)
            .map(|r| Window {
                five_hour: pct(r.five_hour),
                seven_day: pct(r.seven_day),
            })
            .filter(|w| w.five_hour.is_some() || w.seven_day.is_some())
    }
}

/// Did session `id` hand its task off (`checkin --handoff`) in the log of `body`?
fn handed_off(body: &str, id: &str) -> bool {
    let marker = format!("· {}", crate::ops::HANDING_OFF);
    queue::log_section(body).unwrap_or("").lines().any(|line| {
        work::log_who(line).is_some_and(|who| work::mentions(who).iter().any(|m| m == id))
            && line.contains(&marker)
    })
}

/// Where a task's records come from: its queue's project name and key prefix, the roles for slot
/// names, and the accounts' window readings.
pub struct Context<'a> {
    pub project: &'a str,
    pub prefix: &'a str,
    pub roles: &'a [Role],
    pub readings: &'a Readings,
}

/// The record of the session in `row`, one of `task`'s Work rows (`work::work`), made from a
/// finished task's transcripts (`Source::Backfill`); None when its transcript wasn't read.
pub fn from_row(cx: &Context, task: &Task, body: &str, row: &Row) -> Option<Record> {
    let u = &row.usage;
    if u.found != Found::Read {
        return None;
    }
    let s = &row.session;
    let slot = Slot::parse(&s.slot);
    let agent_name = match &slot {
        Some(slot) => roles::slot_name(cx.roles, slot),
        None => s.who.clone(),
    };
    let task_key = crate::key::key(cx.prefix, &task.id);
    let (window_before, window_after) = match (u.start, u.end) {
        (Some(a), Some(z)) => (
            cx.readings.window_at(&s.account, a, 0),
            cx.readings.window_at(&s.account, z, a),
        ),
        _ => (None, None),
    };
    Some(Record {
        schema: SCHEMA.into(),
        run: clean(&format!("{task_key}:{}", s.id)),
        operation: "invoke_agent".into(),
        provider: "anthropic".into(),
        agent_name: clean(&agent_name),
        agent_id: slot.map(|s| s.role).unwrap_or_default(),
        conversation: s.id.clone(),
        model: u.run.model.clone().unwrap_or_default(),
        input_tokens: u.tokens.input,
        output_tokens: u.tokens.output,
        cache_read_tokens: u.tokens.cache_read,
        cache_write_tokens: u.tokens.cache_write,
        task: clean(&task_key),
        project: clean(cx.project),
        account: clean(&s.account),
        stage: clean(&s.stages.join(",")),
        started_at: u.start.map(iso).unwrap_or_default(),
        ended_at: u.end.map(iso).unwrap_or_default(),
        wall_secs: u.run.secs,
        turns: u.run.turns,
        context_peak: u.run.context_peak,
        window_before,
        window_after,
        estimate: Estimate {
            minutes: task.est_minutes,
            tokens: task.est_token_count,
        },
        outcome: if handed_off(body, &s.id) {
            Outcome::HandedOff
        } else {
            Outcome::Completed
        },
        source: Source::Backfill,
        shared_with: row.shared_with,
        counted_tokens: u.tokens.counted(),
    })
}

/// What `backfill` did, or with a dry run would do.
#[derive(Serialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Backfill {
    /// Done tasks looked at, and the sessions their claims and logs name.
    pub tasks: usize,
    pub sessions: usize,
    /// Records written (with a dry run: that would be).
    pub written: usize,
    /// Sessions the ledger has already.
    pub existing: usize,
    /// Sessions with no transcript read: not stamped with an account Muster reads, not on this
    /// Mac, or too big.
    pub unread: usize,
    /// Sessions still running (their transcript changed in the LIVE window): a record is written
    /// once and never changed, so they wait for a later backfill.
    pub live: usize,
    /// Sessions that ended more than `KEEP_MONTHS` months ago: the ledger doesn't keep them.
    pub too_old: usize,
    pub dry_run: bool,
}

/// Records for the sessions of every Done task in `queues` (name, folder), one per session whose
/// transcript is read through `reader` in `allowed`, at `now`; with `dry_run`, counted and
/// nothing written. A session the ledger has already is left as it is, so a second run adds none.
pub fn backfill(
    dir: &Path,
    reader: &Reader,
    allowed: &[Account],
    roles: &[Role],
    queues: &[(String, PathBuf)],
    now: u64,
    dry_run: bool,
) -> Result<Backfill, String> {
    let mut out = Backfill {
        dry_run,
        ..Backfill::default()
    };
    let mut records: Vec<Record> = Vec::new();
    let readings = Readings::new(dir);
    for (name, root) in queues {
        let all = queue::read_state(
            &[(name.clone(), root.clone())],
            &mut queue::Cache::default(),
        )
        .tasks;
        let prefix = crate::key::prefix_for(root);
        let cx = Context {
            project: name,
            prefix: &prefix,
            roles,
            readings: &readings,
        };
        for t in all.iter().filter(|t| t.stage == "done") {
            let Ok((task, body)) = work::read(root, &t.id) else {
                continue;
            };
            let w = work::work(
                reader,
                allowed,
                &task,
                &body,
                now,
                &work::shared_counts(&all, &t.id),
                // In-process, as `stamp_epoch(s, None)` reads them: no `date` per log line.
                &queue::local_epoch,
            );
            out.tasks += 1;
            out.sessions += w.rows.len();
            for row in &w.rows {
                match from_row(&cx, &task, &body, row) {
                    None => out.unread += 1,
                    Some(_) if row.usage.live => out.live += 1,
                    Some(r) if !kept(&month_of(&r, now), now) => out.too_old += 1,
                    Some(r) => records.push(r),
                }
            }
        }
    }
    let known = run_ids(dir);
    let mut taken: HashSet<String> = HashSet::new();
    let mut new: Vec<Record> = Vec::new();
    for r in records {
        if known.contains(&r.run) {
            out.existing += 1;
        } else if taken.insert(r.run.clone()) {
            // Once, as `record_all` writes it, so a dry run counts what a run would write.
            new.push(r);
        }
    }
    out.written = if dry_run {
        new.len()
    } else {
        record_all(dir, &new, now)?
    };
    Ok(out)
}

/// The records as plain text in columns with a header line, for `muster ledger`.
pub fn text(records: &[Record]) -> String {
    let head = [
        "ended (UTC)",
        "task",
        "project",
        "account",
        "agent",
        "stage",
        "input",
        "output",
        "cache read",
        "cache write",
        "counted",
        "secs",
        "turns",
        "peak",
        "outcome",
    ];
    let outcome = |o: Outcome| {
        serde_json::to_value(o)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default()
    };
    let rows: Vec<Vec<String>> = records
        .iter()
        .map(|r| {
            vec![
                r.ended_at.get(..16).unwrap_or("").replace('T', " "),
                r.task.clone(),
                r.project.clone(),
                r.account.clone(),
                r.agent_name.clone(),
                r.stage.clone(),
                r.input_tokens.to_string(),
                r.output_tokens.to_string(),
                r.cache_read_tokens.to_string(),
                r.cache_write_tokens.to_string(),
                r.counted_tokens.to_string(),
                r.wall_secs.to_string(),
                r.turns.to_string(),
                r.context_peak.to_string(),
                outcome(r.outcome),
            ]
        })
        .collect();
    let widths: Vec<usize> = (0..head.len())
        .map(|i| {
            rows.iter()
                .map(|r| r[i].chars().count())
                .chain([head[i].chars().count()])
                .max()
                .unwrap_or(0)
        })
        .collect();
    let line = |cells: Vec<&str>| {
        let mut s = String::new();
        for (i, c) in cells.iter().enumerate() {
            if i + 1 == cells.len() {
                s.push_str(c);
            } else {
                s.push_str(&format!("{c:<w$}  ", w = widths[i]));
            }
        }
        s.trim_end().to_string() + "\n"
    };
    let mut out = line(head.to_vec());
    for r in &rows {
        out.push_str(&line(r.iter().map(String::as_str).collect()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcripts::tests::{home, prompt, reader, reply, reply_by, write, NOW};
    use crate::transcripts::{iso_secs, minutes_of};
    use std::os::unix::fs::PermissionsExt;

    const A: &str = "7b5e8856-c9c8-45ec-aa03-c0ddbf1d4394";
    const B: &str = "4d517d9d-1173-481b-80cc-27e72f903f3c";

    /// A Done task: A (slot coder-2, hg) built it and handed off once, B (mg) tested it, and
    /// session c0ffee00, stamped with no account (bare `claude`), checked in: never read. Paused
    /// 08:10 to 08:20 UTC, inside A's span (`done` writes the stamps in this Mac's zone).
    const DONE: &str = "---
id: 0025
stage: done
status: done
est-time: 2h
est-tokens: 100k
claimed-by:
heartbeat:
waiting_on:
---

# Sessions and cost

## Log
- 2026-10-12 07:00 capture: created
- 2026-10-12 08:00 Kaveh-2 (main · session 7b5e8856 · slot coder-2 · cc-hg): stage ready → building (claimed)
- 2026-10-12 08:05 Kaveh-2 (main · session 7b5e8856 · slot coder-2 · cc-hg): check-in 50% · handing off · next: tests
- PAUSED owner: paused
- RESUMED owner: resumed
- 2026-10-12 08:30 Kaveh-2 (main · session 7b5e8856 · slot coder-2 · cc-hg): stage building → testing (gate: green)
- 2026-10-12 08:40 someone (main · session c0ffee00): check-in · looked
- 2026-10-12 11:00 qa (main · session 4d517d9d · cc-mg): stage testing → your-check
- 2026-10-12 12:00 owner: stage your-check → done
";

    /// `DONE` with its pause stamped in local time, as the tools write it.
    fn done() -> String {
        let local = |at: &str| {
            let secs = i64::try_from(iso_secs(at).unwrap()).unwrap();
            crate::usage::local_time(secs, None).unwrap()
        };
        DONE.replace("PAUSED", &local("2026-10-12T08:10:00Z"))
            .replace("RESUMED", &local("2026-10-12T08:20:00Z"))
    }

    fn queue() -> crate::testkit::TestDir {
        let d = crate::testkit::tempdir().unwrap();
        for f in queue::STATUS_FOLDERS {
            fs::create_dir(d.path().join(f)).unwrap();
        }
        fs::write(d.path().join("done/0025-sessions.md"), done()).unwrap();
        fs::write(d.path().join("README.md"), "---\nprefix: mu\n---\n").unwrap();
        d
    }

    /// A's and B's transcripts, with a prompt that holds a path and a key-shaped secret, which no
    /// record may carry.
    fn transcripts(home: &Path) {
        write(
            home,
            ".claude",
            A,
            &[
                prompt(
                    "2026-10-12T08:00:00Z",
                    "build /Users/someone/playground/secret.txt with sk-ant-api03-XXXXXXXX",
                ),
                reply_by(
                    "claude-opus-5-5",
                    "a1",
                    "2026-10-12T08:01:00Z",
                    10,
                    2000,
                    30000,
                    5000,
                ),
                reply_by(
                    "claude-opus-5-5",
                    "a1",
                    "2026-10-12T08:01:00Z",
                    10,
                    2000,
                    30000,
                    5000,
                ),
                reply_by(
                    "claude-opus-5-5",
                    "a2",
                    "2026-10-12T08:30:00Z",
                    5,
                    1000,
                    20000,
                    0,
                ),
            ],
            today() - 86400,
        );
        write(
            home,
            ".claude-mg",
            B,
            &[
                prompt("2026-10-12T10:50:00Z", "test 0025"),
                reply("b1", "2026-10-12T11:00:00Z", 1, 500, 10000, 1000),
            ],
            today() - 86400,
        );
    }

    fn backfill_into(app: &Path, q: &Path, h: &Path, dry_run: bool) -> Result<Backfill, String> {
        let (r, a) = reader(h);
        backfill(
            app,
            &r,
            &a,
            &roles::defaults(),
            &[("demo".into(), q.to_path_buf())],
            today(),
            dry_run,
        )
    }

    fn lines(app: &Path) -> Vec<String> {
        months(app)
            .iter()
            .flat_map(|(_, p)| {
                fs::read_to_string(p)
                    .unwrap()
                    .lines()
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// A day soon after the fixtures': a write prunes months by it (`NOW` is in 2033).
    fn today() -> u64 {
        iso_secs("2026-10-15T00:00:00Z").unwrap()
    }

    fn by_session<'a>(records: &'a [Record], id: &str) -> &'a Record {
        records.iter().find(|r| r.conversation == id).unwrap()
    }

    #[test]
    fn a_run_writes_one_record_with_the_pinned_names_and_a_second_write_is_a_no_op() {
        let (q, h, app) = (queue(), home(), crate::testkit::tempdir().unwrap());
        transcripts(h.path());
        let (r, a) = reader(h.path());
        let w = work::for_task(&r, &a, q.path(), "0025", NOW).unwrap();
        let (task, body) = work::read(q.path(), "0025").unwrap();
        let roles = roles::defaults();
        let cx = Context {
            project: "demo",
            prefix: "mu",
            roles: &roles,
            readings: &Readings::new(app.path()),
        };
        let row = w.rows.iter().find(|r| r.session.id == "7b5e8856").unwrap();
        let rec = from_row(&cx, &task, &body, row).unwrap();
        assert!(record(app.path(), &rec, today()).unwrap());
        assert!(!record(app.path(), &rec, today()).unwrap());
        // Twice in one call is once too.
        assert_eq!(
            record_all(app.path(), &[rec.clone(), rec.clone()], today()).unwrap(),
            0
        );
        let got = lines(app.path());
        assert_eq!(got.len(), 1);
        // In the month it ended, the user's alone.
        let file = folder(app.path()).join("2026-10.jsonl");
        assert!(file.is_file());
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&file), 0o600);
        assert_eq!(mode(&folder(app.path())), 0o700);
        // The pinned names, and nothing else.
        let v: serde_json::Value = serde_json::from_str(&got[0]).unwrap();
        let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        let mut pinned = FIELDS.to_vec();
        pinned.sort_unstable();
        assert_eq!(keys, pinned);
        assert!(got[0].starts_with(&format!(
            "{{\"schema\":\"{SCHEMA}\",\"muster.run\":\"mu-25:7b5e8856\""
        )));
        assert_eq!(v["gen_ai.operation.name"], "invoke_agent");
        assert_eq!(v["gen_ai.provider.name"], "anthropic");
        assert_eq!(v["gen_ai.agent.name"], "Kaveh-2");
        assert_eq!(v["gen_ai.agent.id"], "coder");
        assert_eq!(v["gen_ai.conversation.id"], "7b5e8856");
        assert_eq!(v["gen_ai.request.model"], "claude-opus-5-5");
        assert_eq!(v["muster.task"], "mu-25");
        assert_eq!(v["muster.project"], "demo");
        assert_eq!(v["muster.account"], "hg");
        assert_eq!(v["muster.stage"], "building");
        assert_eq!(v["muster.started_at"], "2026-10-12T08:00:00Z");
        assert_eq!(v["muster.ended_at"], "2026-10-12T08:30:00Z");
        assert_eq!(v["muster.turns"], 2);
        assert_eq!(v["muster.context_peak"], 10 + 30000 + 5000);
        assert_eq!(
            v["muster.estimate"],
            serde_json::json!({"minutes": 120, "tokens": 100000})
        );
        assert_eq!(v["muster.outcome"], "handed_off");
        assert_eq!(v["muster.source"], "backfill");
        assert_eq!(v["muster.window_before"], serde_json::Value::Null);
        // It reads back as it was written.
        assert_eq!(read(app.path(), &Filter::default()), vec![rec]);
    }

    #[test]
    fn a_records_tokens_and_time_equal_the_work_sections() {
        let (q, h, app) = (queue(), home(), crate::testkit::tempdir().unwrap());
        transcripts(h.path());
        backfill_into(app.path(), q.path(), h.path(), false).unwrap();
        let records = read(app.path(), &Filter::default());
        let (r, a) = reader(h.path());
        let w = work::for_task(&r, &a, q.path(), "0025", NOW).unwrap();
        let read_rows: Vec<&Row> = w
            .rows
            .iter()
            .filter(|r| r.usage.found == Found::Read)
            .collect();
        assert_eq!(records.len(), read_rows.len());
        assert_eq!(records.len(), 2);
        for row in read_rows {
            let rec = by_session(&records, &row.session.id);
            let t = row.usage.tokens;
            assert_eq!(
                (
                    rec.input_tokens,
                    rec.output_tokens,
                    rec.cache_read_tokens,
                    rec.cache_write_tokens
                ),
                (t.input, t.output, t.cache_read, t.cache_write)
            );
            assert_eq!(rec.counted_tokens, t.counted());
            assert_eq!(rec.wall_secs, row.usage.run.secs);
            assert_eq!(minutes_of(rec.wall_secs), row.usage.minutes);
        }
        // A's 30 minutes less the 10 paused; B's 10.
        assert_eq!(by_session(&records, "7b5e8856").wall_secs, 20 * 60);
        assert_eq!(by_session(&records, "4d517d9d").wall_secs, 10 * 60);
        // The ledger's sums are the Work section's totals.
        let counted: u64 = records.iter().map(|r| r.counted_tokens).sum();
        assert_eq!(counted, w.totals.counted_tokens);
        let minutes: u64 = records.iter().map(|r| minutes_of(r.wall_secs)).sum();
        assert_eq!(minutes, w.totals.minutes);
    }

    #[test]
    fn no_record_field_holds_a_path_a_prompt_a_transcript_line_or_a_token() {
        let (q, h, app) = (queue(), home(), crate::testkit::tempdir().unwrap());
        transcripts(h.path());
        // A label that's a path is left out, not written.
        let text = done().replace("qa (main", "/Users/someone/qa (main");
        fs::write(q.path().join("done/0025-sessions.md"), text).unwrap();
        backfill_into(app.path(), q.path(), h.path(), false).unwrap();
        let all = lines(app.path()).join("\n");
        assert!(!all.is_empty());
        let home_path = h.path().to_string_lossy().to_string();
        for leak in [
            "/Users",
            "secret.txt",
            "sk-ant",
            "build ",
            "test 0025",
            "\"text\"",
            home_path.as_str(),
            "\\",
        ] {
            assert!(!all.contains(leak), "{leak:?} in {all}");
        }
        let records = read(app.path(), &Filter::default());
        assert_eq!(by_session(&records, "4d517d9d").agent_name, "");
        // Every text value is a short plain label.
        for line in lines(app.path()) {
            let v: serde_json::Value = serde_json::from_str(&line).unwrap();
            for (k, x) in v.as_object().unwrap() {
                if let Some(s) = x.as_str() {
                    assert!(plain(s), "{k} = {s:?}");
                }
            }
        }
        // And the writer refuses one that isn't.
        let good = records[0].clone();
        let refused = |f: &dyn Fn(&mut Record)| {
            let mut r = good.clone();
            r.run = "mu-25:other".into();
            f(&mut r);
            check(&r).is_err() && record(app.path(), &r, today()).is_err()
        };
        assert!(refused(
            &|r| r.project = "/Users/someone/playground/demo".into()
        ));
        assert!(refused(&|r| r.model = "a prompt  with   spacing".into()));
        assert!(refused(&|r| r.model = "fix the login bug".into()));
        assert!(refused(&|r| r.account = "cc hg".into()));
        assert!(refused(&|r| r.account = "Hg".into()));
        assert!(refused(&|r| r.agent_id = "coder-2".into()));
        assert!(refused(&|r| r.agent_id = "a b".into()));
        assert!(refused(&|r| r.agent_name = "x".repeat(65)));
        assert!(refused(&|r| r.stage = "line\nbreak".into()));
        assert!(refused(&|r| r.task = "C:\\tasks".into()));
        assert!(refused(&|r| r.conversation = "sk-ant-api03".into()));
        assert!(refused(&|r| r.ended_at = "yesterday".into()));
        assert!(refused(&|r| r.run = String::new()));
        assert_eq!(lines(app.path()).len(), 2);
    }

    #[test]
    fn backfill_writes_one_record_per_read_session_of_each_done_task_and_nothing_twice() {
        let (q, h, app) = (queue(), home(), crate::testkit::tempdir().unwrap());
        transcripts(h.path());
        // A task not done yet: its sessions wait for it to finish.
        let open = "---\nid: 0026\nstage: building\nstatus: doing\nclaimed-by: x (main · session 4d517d9d · cc-mg)\n---\n\n# Open\n\n## Log\n";
        fs::write(q.path().join("doing/0026-open.md"), open).unwrap();
        let dry = backfill_into(app.path(), q.path(), h.path(), true).unwrap();
        assert_eq!(
            dry,
            Backfill {
                tasks: 1,
                sessions: 3,
                written: 2,
                existing: 0,
                unread: 1,
                live: 0,
                too_old: 0,
                dry_run: true
            }
        );
        // A dry run writes nothing, not even the folder.
        assert!(!folder(app.path()).exists());
        let first = backfill_into(app.path(), q.path(), h.path(), false).unwrap();
        assert_eq!((first.written, first.existing, first.unread), (2, 0, 1));
        assert!(!first.dry_run);
        let second = backfill_into(app.path(), q.path(), h.path(), false).unwrap();
        assert_eq!((second.written, second.existing), (0, 2));
        assert_eq!(lines(app.path()).len(), 2);
        let records = read(app.path(), &Filter::default());
        assert!(records.iter().all(|r| r.source == Source::Backfill));
        let b = by_session(&records, "4d517d9d");
        assert_eq!(
            (b.outcome, b.stage.as_str()),
            (Outcome::Completed, "testing")
        );
        assert_eq!(b.shared_with, 1);
        // Once the open task is done, the session gets a record of its own for it.
        fs::remove_file(q.path().join("doing/0026-open.md")).unwrap();
        let done = open.replace("stage: building", "stage: done");
        fs::write(q.path().join("done/0026-open.md"), done).unwrap();
        let third = backfill_into(app.path(), q.path(), h.path(), false).unwrap();
        assert_eq!((third.tasks, third.written, third.existing), (2, 1, 2));
        let runs: Vec<String> = read(app.path(), &Filter::default())
            .into_iter()
            .map(|r| r.run)
            .collect();
        assert_eq!(runs, ["mu-25:7b5e8856", "mu-25:4d517d9d", "mu-26:4d517d9d"]);
    }

    #[test]
    fn a_record_carries_the_accounts_window_before_and_after_from_its_readings() {
        let (q, h, app) = (queue(), home(), crate::testkit::tempdir().unwrap());
        transcripts(h.path());
        let start = iso_secs("2026-10-12T08:00:00Z").unwrap();
        let end = iso_secs("2026-10-12T08:30:00Z").unwrap();
        let reading = |at: u64, five: f64, seven: f64| {
            serde_json::json!({
                "account": "hg",
                "five_hour": {"used_percentage": five, "resets_at": at + 3600},
                "seven_day": {"used_percentage": seven, "resets_at": at + 86400},
                "at": at,
                "source": "statusline"
            })
            .to_string()
        };
        let file = crate::usage::file(app.path(), "hg");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        let readings = [
            reading(start - 600, 10.4, 50.0),
            reading(start - 60, 12.6, 51.0),
            "not json".to_string(),
            reading(end - 120, 30.0, 55.0),
            reading(end + 600, 90.0, 60.0),
        ];
        fs::write(&file, readings.join("\n")).unwrap();
        backfill_into(app.path(), q.path(), h.path(), false).unwrap();
        let records = read(app.path(), &Filter::default());
        let a = by_session(&records, "7b5e8856");
        let w = |five, seven| {
            Some(Window {
                five_hour: Some(five),
                seven_day: Some(seven),
            })
        };
        assert_eq!((a.window_before, a.window_after), (w(13, 51), w(30, 55)));
        // B's account (mg) has no readings.
        let b = by_session(&records, "4d517d9d");
        assert_eq!((b.window_before, b.window_after), (None, None));
    }

    #[test]
    fn a_window_that_had_reset_by_then_says_nothing_and_readings_are_read_once() {
        let app = crate::testkit::tempdir().unwrap();
        let start = iso_secs("2026-10-12T09:00:00Z").unwrap();
        let reading = |at: u64, five_resets: u64| {
            serde_json::json!({
                "account": "hg",
                "five_hour": {"used_percentage": 80.0, "resets_at": five_resets},
                "seven_day": {"used_percentage": 40.0, "resets_at": at + 86400},
                "at": at,
                "source": "statusline"
            })
            .to_string()
        };
        let file = crate::usage::file(app.path(), "hg");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        // Read at 01:00, its five-hour window reset at 03:00: by 09:00 it says nothing.
        let one = start - 8 * 3600;
        fs::write(&file, reading(one, one + 2 * 3600)).unwrap();
        let readings = Readings::new(app.path());
        let w = |five, seven| Window {
            five_hour: five,
            seven_day: seven,
        };
        assert_eq!(readings.window_at("hg", start, 0), Some(w(None, Some(40))));
        // At 02:59 it was still running; at 03:00 it had reset.
        assert_eq!(
            readings.window_at("hg", one + 2 * 3600 - 1, 0),
            Some(w(Some(80), Some(40)))
        );
        assert_eq!(
            readings.window_at("hg", one + 2 * 3600, 0),
            Some(w(None, Some(40)))
        );
        // Both windows reset: no reading.
        assert_eq!(readings.window_at("hg", one + 2 * 86400, 0), None);
        // The file was read once: a later change isn't seen by the same backfill.
        fs::write(&file, reading(start, start + 3600)).unwrap();
        assert_eq!(readings.window_at("hg", start, 0), Some(w(None, Some(40))));
        assert_eq!(
            Readings::new(app.path()).window_at("hg", start, 0),
            Some(w(Some(80), Some(40)))
        );
        assert_eq!(readings.window_at("../hg", start, 0), None);
    }

    #[test]
    fn reading_keeps_what_the_filter_asks_for() {
        let (q, h, app) = (queue(), home(), crate::testkit::tempdir().unwrap());
        transcripts(h.path());
        backfill_into(app.path(), q.path(), h.path(), false).unwrap();
        let n = |f: Filter| read(app.path(), &f).len();
        let task = |t: &str| Filter {
            task: crate::key::parse(t),
            ..Filter::default()
        };
        let since = |s: u64| Filter {
            since: Some(s),
            ..Filter::default()
        };
        assert_eq!(n(Filter::default()), 2);
        assert_eq!(n(task("MU-25")), 2);
        assert_eq!(n(task("mu-2")), 0);
        assert_eq!(n(task("mu-0025")), 2);
        assert_eq!(n(task("25")), 2);
        assert_eq!(n(task("br-25")), 0);
        let mg = Filter {
            account: Some("mg".into()),
            ..Filter::default()
        };
        assert_eq!(n(mg), 1);
        assert_eq!(n(since(iso_secs("2026-10-12T10:00:00Z").unwrap())), 1);
        assert_eq!(n(since(NOW)), 0);
        // A malformed line and a stray file are skipped.
        let month = folder(app.path()).join("2026-10.jsonl");
        let mut text = fs::read_to_string(&month).unwrap();
        text.push_str("{\"half\n");
        fs::write(&month, text).unwrap();
        fs::write(folder(app.path()).join("notes.jsonl"), "x\n").unwrap();
        assert_eq!(n(Filter::default()), 2);
    }

    #[test]
    fn a_write_after_one_cut_short_starts_a_line_of_its_own() {
        let (q, h, app) = (queue(), home(), crate::testkit::tempdir().unwrap());
        transcripts(h.path());
        backfill_into(app.path(), q.path(), h.path(), false).unwrap();
        let all = read(app.path(), &Filter::default());
        let month = folder(app.path()).join("2026-10.jsonl");
        let text = fs::read_to_string(&month).unwrap();
        let (first, second) = text.split_once('\n').unwrap();
        let next = |run: &str| {
            let mut r = all[0].clone();
            r.run = run.into();
            r
        };
        // A whole record that lost only its line end, then a write: both read.
        fs::write(&month, first).unwrap();
        assert!(record(app.path(), &next("mu-25:after1"), today()).unwrap());
        let runs = |f: &Filter| -> Vec<String> {
            read(app.path(), f).into_iter().map(|r| r.run).collect()
        };
        assert_eq!(runs(&Filter::default()), ["mu-25:7b5e8856", "mu-25:after1"]);
        // A record cut mid-line: the next write is read, the torn line alone is lost.
        let mut torn = fs::read_to_string(&month).unwrap();
        torn.push_str(&second[..second.len() / 2]);
        fs::write(&month, &torn).unwrap();
        assert!(record(app.path(), &next("mu-25:after2"), today()).unwrap());
        assert_eq!(
            runs(&Filter::default()),
            ["mu-25:7b5e8856", "mu-25:after1", "mu-25:after2"]
        );
        let text = fs::read_to_string(&month).unwrap();
        assert!(text.ends_with('\n') && !text.contains("\n\n"));
        // A file that ends well gets no blank line.
        assert!(record(app.path(), &next("mu-25:after3"), today()).unwrap());
        assert!(!fs::read_to_string(&month).unwrap().contains("\n\n"));
    }

    #[test]
    fn months_past_the_thirteenth_go_on_the_next_write() {
        let (q, h, app) = (queue(), home(), crate::testkit::tempdir().unwrap());
        transcripts(h.path());
        let dir = folder(app.path());
        fs::create_dir_all(&dir).unwrap();
        for m in ["2025-09", "2025-10", "2025-11"] {
            fs::write(dir.join(format!("{m}.jsonl")), "").unwrap();
        }
        fs::write(dir.join("keep-me.txt"), "").unwrap();
        let (r, a) = reader(h.path());
        let now = today();
        let queues = [("demo".to_string(), q.path().to_path_buf())];
        backfill(app.path(), &r, &a, &roles::defaults(), &queues, now, false).unwrap();
        let mut names: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| !n.starts_with('.'))
            .collect();
        names.sort();
        // 2025-10 is the 13th month, the current one counted; 2025-09 is past it.
        assert_eq!(
            names,
            [
                "2025-10.jsonl",
                "2025-11.jsonl",
                "2026-10.jsonl",
                "keep-me.txt"
            ]
        );
    }

    #[test]
    fn a_session_still_running_waits_for_a_later_backfill() {
        let (q, h, app) = (queue(), home(), crate::testkit::tempdir().unwrap());
        transcripts(h.path());
        let b = h
            .path()
            .join(".claude-mg/projects")
            .join(crate::transcripts::tests::PROJECT);
        let b = b.join(format!("{B}.jsonl"));
        crate::transcripts::tests::touch(&b, today() - 60);
        let first = backfill_into(app.path(), q.path(), h.path(), false).unwrap();
        assert_eq!((first.written, first.live), (1, 1));
        assert_eq!(
            read(app.path(), &Filter::default())[0].conversation,
            "7b5e8856"
        );
        // Once it has ended, the next backfill records it.
        crate::transcripts::tests::touch(&b, today() - 3600);
        let second = backfill_into(app.path(), q.path(), h.path(), false).unwrap();
        assert_eq!((second.written, second.live, second.existing), (1, 0, 1));
    }

    #[test]
    fn a_run_older_than_the_ledger_keeps_is_never_written() {
        let (q, h, app) = (queue(), home(), crate::testkit::tempdir().unwrap());
        write(
            h.path(),
            ".claude-mg",
            B,
            &[reply("b1", "2025-01-10T11:00:00Z", 1, 500, 10000, 1000)],
            today() - 86400,
        );
        let dry = backfill_into(app.path(), q.path(), h.path(), true).unwrap();
        assert_eq!((dry.written, dry.too_old), (0, 1));
        let run = backfill_into(app.path(), q.path(), h.path(), false).unwrap();
        assert_eq!((run.written, run.too_old), (0, 1));
        assert!(lines(app.path()).is_empty());
        // Nor by a runner, nor one from far ahead; last month and next are.
        let (r, a) = reader(h.path());
        let w = work::for_task(&r, &a, q.path(), "0025", NOW).unwrap();
        let (task, body) = work::read(q.path(), "0025").unwrap();
        let roles = roles::defaults();
        let cx = Context {
            project: "demo",
            prefix: "mu",
            roles: &roles,
            readings: &Readings::new(app.path()),
        };
        let row = w.rows.iter().find(|r| r.session.id == "4d517d9d").unwrap();
        let mut rec = from_row(&cx, &task, &body, row).unwrap();
        assert!(!record(app.path(), &rec, today()).unwrap());
        for (at, written) in [
            ("9999-12-01T00:00:00Z", false),
            ("2026-12-01T00:00:00Z", false),
            ("2026-11-30T00:00:00Z", true),
            ("2025-10-01T00:00:00Z", true),
        ] {
            rec.ended_at = at.into();
            rec.run = format!("mu-25:{at}").replace(':', ".");
            assert_eq!(record(app.path(), &rec, today()).unwrap(), written, "{at}");
        }
    }

    #[test]
    fn a_hand_edited_line_is_read_only_when_it_passes_the_writers_check() {
        let (q, h, app) = (queue(), home(), crate::testkit::tempdir().unwrap());
        transcripts(h.path());
        backfill_into(app.path(), q.path(), h.path(), false).unwrap();
        let month = folder(app.path()).join("2026-10.jsonl");
        let good = lines(app.path()).remove(0);
        let edit = |from: &str, to: &str| {
            assert!(good.contains(from), "{from}");
            good.replace(from, to)
                .replace("mu-25:7b5e8856", "mu-25:edited")
        };
        let mut text = fs::read_to_string(&month).unwrap();
        for line in [
            edit(
                "\"muster.task\":\"mu-25\"",
                "\"muster.task\":\"\\u001b]52;c;aGk=\\u0007\"",
            ),
            edit(
                "\"muster.project\":\"demo\"",
                "\"muster.project\":\"/Users/someone/demo\"",
            ),
            edit(
                "\"muster.stage\":\"building\"",
                &format!("\"muster.stage\":\"{}\"", "x".repeat(10_000)),
            ),
        ] {
            text.push_str(&line);
            text.push('\n');
        }
        // One past the line cap with no newline at the end, then a good record after it.
        text.push_str(&"y".repeat(20_000));
        text.push('\n');
        text.push_str(&edit(
            "\"muster.account\":\"hg\"",
            "\"muster.account\":\"mg\"",
        ));
        text.push('\n');
        fs::write(&month, text).unwrap();
        let runs: Vec<String> = read(app.path(), &Filter::default())
            .into_iter()
            .map(|r| r.run)
            .collect();
        assert_eq!(runs, ["mu-25:7b5e8856", "mu-25:4d517d9d", "mu-25:edited"]);
    }

    #[test]
    fn times_are_utc_iso_8601() {
        assert_eq!(iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(iso(NOW), "2033-05-18T03:33:20Z");
        for s in [0, 951_782_400, 1_760_000_000, NOW, 4_102_444_799] {
            assert_eq!(iso_secs(&iso(s)), Some(s), "{s}");
        }
        assert_eq!(month_number("2026-10"), Some(2026 * 12 + 10));
        for bad in ["2026-13", "2026-00", "26-10", "2026-1", "abcd-10", ""] {
            assert_eq!(month_number(bad), None, "{bad}");
        }
    }

    #[test]
    fn the_text_has_a_header_and_a_column_per_field() {
        let (q, h, app) = (queue(), home(), crate::testkit::tempdir().unwrap());
        transcripts(h.path());
        backfill_into(app.path(), q.path(), h.path(), false).unwrap();
        let t = text(&read(app.path(), &Filter::default()));
        let rows: Vec<&str> = t.lines().collect();
        assert_eq!(rows.len(), 3);
        assert!(
            rows[0].starts_with("ended (UTC)       task   project  account  agent"),
            "{t}"
        );
        assert!(rows[0].ends_with("outcome"));
        assert!(
            rows[1].starts_with("2026-10-12 08:30  mu-25  demo     hg       Kaveh-2  building"),
            "{t}"
        );
        assert!(rows[1].ends_with("handed_off"));
        assert!(rows[2].ends_with("completed"));
        assert_eq!(text(&[]).lines().count(), 1);
    }
}
