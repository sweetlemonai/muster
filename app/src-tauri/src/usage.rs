//! How much of each account's 5-hour and weekly window is used (task 0113), read from Claude
//! Code and never guessed (decision 0013): a run's `rate_limit_event` on the stream
//! (`parse_rate_limit_event`) and the status line's `rate_limits` (`parse_statusline`, through
//! `bin/statusline-usage`). Each reading is one line of `usage/<label>.jsonl` in the app's config
//! folder: the two windows, when it was read and where from. No session, folder, model, cost or
//! token is kept, and nothing older than 7 days.
//!
//! Readings are untrusted input: typed decoders, unknown fields ignored, and a window whose use
//! isn't a finite 0 to 1000 % or whose reset isn't within a year of now is dropped. Use past a
//! window's cap (104 %) is kept as it is; only the display clamps it to 100.

use crate::accounts;
use crate::settings::Settings;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// The readings' folder, in the app's config folder.
pub const DIR: &str = "usage";
/// The most use a reading may say, in percent: past the cap is real (mu-111), ten times it isn't.
pub const MAX_PERCENT: f64 = 1000.0;
/// A reset more than this far from now, either way, isn't a reading of this window.
const YEAR_SECS: i64 = 365 * 86400;
/// The file never holds a reading older than this when it's written to.
pub const KEEP_SECS: i64 = 7 * 86400;
/// When the oldest reading is past `KEEP_SECS`, the file is rewritten with the readings of this
/// last span only, so it's rewritten about once a day, not on every status line.
const KEPT_SECS: i64 = 6 * 86400;
/// A reading with the same windows as the latest, within this, isn't written again: the status
/// line runs after every message.
pub const SAME_SECS: i64 = 5 * 60;
/// How much of the end of a readings file is read for the latest reading.
const TAIL_BYTES: u64 = 64 * 1024;

/// One window's use: percent used (past 100 when use ran past the cap) and when it resets, in
/// seconds since 1970.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct Window {
    pub used_percentage: f64,
    pub resets_at: i64,
}

impl Window {
    /// The window, when its use is a finite 0 to `MAX_PERCENT` and it resets within a year of
    /// `now`.
    fn checked(self, now: i64) -> Option<Window> {
        let used = self.used_percentage;
        (used.is_finite()
            && (0.0..=MAX_PERCENT).contains(&used)
            && self.resets_at.abs_diff(now) <= YEAR_SECS as u64)
            .then_some(self)
    }

    /// A window that has reset since it was read says nothing about its use now.
    pub fn stale(&self, now: i64) -> bool {
        self.resets_at <= now
    }

    /// What's left of the window above the owner's reserve, in percent: below 0 is into the
    /// reserve. Use past the cap counts as 100.
    pub fn headroom(&self, reserve: u32) -> f64 {
        100.0 - self.used_percentage.min(100.0) - f64::from(reserve)
    }
}

/// The two windows a reading carries; either may be missing.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Windows {
    pub five_hour: Option<Window>,
    pub seven_day: Option<Window>,
}

impl Windows {
    /// Each window checked against `now`; None when neither is left.
    fn checked(self, now: i64) -> Option<Windows> {
        let w = Windows {
            five_hour: self.five_hour.and_then(|w| w.checked(now)),
            seven_day: self.seven_day.and_then(|w| w.checked(now)),
        };
        (w.five_hour.is_some() || w.seven_day.is_some()).then_some(w)
    }
}

/// Where a reading came from.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// A run's `rate_limit_event`.
    Stream,
    /// The status line's `rate_limits`, through `bin/statusline-usage`.
    Statusline,
}

/// One line of `usage/<label>.jsonl`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Reading {
    /// The account's label: `mg` for `~/.claude-mg`.
    pub account: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub five_hour: Option<Window>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seven_day: Option<Window>,
    /// When it was read, in seconds since 1970.
    pub at: i64,
    pub source: Source,
}

impl Reading {
    pub fn new(account: &str, w: Windows, at: i64, source: Source) -> Reading {
        Reading {
            account: account.to_string(),
            five_hour: w.five_hour,
            seven_day: w.seven_day,
            at,
            source,
        }
    }

    pub fn windows(&self) -> Windows {
        Windows {
            five_hour: self.five_hour,
            seven_day: self.seven_day,
        }
    }

    /// This reading, but within one window (the same reset) use only rises: where `last` says
    /// more, it's `last`'s. A session re-rendering an older status line then can't lower it.
    fn rising_from(&self, last: &Reading) -> Reading {
        let higher = |w: Option<Window>, l: Option<Window>| match (w, l) {
            (Some(w), Some(l))
                if l.resets_at == w.resets_at && l.used_percentage > w.used_percentage =>
            {
                Some(l)
            }
            _ => w,
        };
        Reading {
            five_hour: higher(self.five_hour, last.five_hour),
            seven_day: higher(self.seven_day, last.seven_day),
            ..self.clone()
        }
    }
}

/// A window as the stream says it: `utilization` a fraction (1.04 past the cap), camelCase.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StreamWindow {
    utilization: f64,
    resets_at: i64,
}

/// The two windows under one key, each decoded on its own so a bad one doesn't drop the other.
#[derive(Deserialize)]
struct Pair {
    five_hour: Option<serde_json::Value>,
    seven_day: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RateLimitInfo {
    unified_windows: Option<Pair>,
}

#[derive(Deserialize)]
struct StreamLine {
    #[serde(rename = "type")]
    kind: String,
    rate_limit_info: Option<RateLimitInfo>,
}

#[derive(Deserialize)]
struct StatusLine {
    rate_limits: Option<Pair>,
}

/// The windows in one line of `claude -p --output-format stream-json`, when it's a
/// `rate_limit_event` (`rate_limit_info.unifiedWindows.{five_hour,seven_day}`, each
/// `{utilization, resetsAt}`), checked against `now`; None for every other line and for a
/// malformed one. Use is `utilization` in percent to one decimal, as the status line rounds it
/// (1.04 is 104.0). A run's runner calls this on each line it reads (mu-117); `muster accounts
/// record --from stream` does too.
pub fn parse_rate_limit_event(line: &str, now: i64) -> Option<Windows> {
    // Most lines are messages; skip them without decoding.
    if !line.contains("rate_limit_event") {
        return None;
    }
    let l: StreamLine = serde_json::from_str(line).ok()?;
    if l.kind != "rate_limit_event" {
        return None;
    }
    let pair = l.rate_limit_info?.unified_windows?;
    let window = |v: Option<serde_json::Value>| {
        let w: StreamWindow = serde_json::from_value(v?).ok()?;
        Some(Window {
            used_percentage: (w.utilization * 1000.0).round() / 10.0,
            resets_at: w.resets_at,
        })
    };
    Windows {
        five_hour: window(pair.five_hour),
        seven_day: window(pair.seven_day),
    }
    .checked(now)
}

/// The windows in the status line's stdin (`rate_limits.{five_hour,seven_day}`, each
/// `{used_percentage, resets_at}`), checked against `now`; None when it has no `rate_limits`
/// (before the session's first response) or isn't JSON. Nothing else of it is read.
pub fn parse_statusline(bytes: &[u8], now: i64) -> Option<Windows> {
    let s: StatusLine = serde_json::from_slice(bytes).ok()?;
    let pair = s.rate_limits?;
    let window = |v: Option<serde_json::Value>| serde_json::from_value::<Window>(v?).ok();
    Windows {
        five_hour: window(pair.five_hour),
        seven_day: window(pair.seven_day),
    }
    .checked(now)
}

/// `usage/<label>.jsonl` in the app's config folder `dir`.
pub fn file(dir: &Path, label: &str) -> PathBuf {
    dir.join(DIR).join(format!("{label}.jsonl"))
}

/// A line of a readings file as a reading of `label`; None when it's malformed, another
/// account's, or has no window in bounds of when it was read.
fn parse_line(line: &str, label: &str) -> Option<Reading> {
    let r: Reading = serde_json::from_str(line).ok()?;
    if r.account != label {
        return None;
    }
    let w = r.windows().checked(r.at)?;
    Some(Reading::new(&r.account, w, r.at, r.source))
}

/// The first line of `path` (up to 4 KB) and the last `TAIL_BYTES` of it, from the first whole
/// line in them. Empty for a missing file.
fn ends(path: &Path) -> (String, String) {
    let Ok(mut f) = fs::File::open(path) else {
        return (String::new(), String::new());
    };
    let mut head = Vec::new();
    let _ = (&mut f).take(4096).read_to_end(&mut head);
    let first = head.split(|&b| b == b'\n').next().unwrap_or_default();
    let first = String::from_utf8_lossy(first).into_owned();
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(TAIL_BYTES);
    let mut tail = Vec::new();
    if f.seek(SeekFrom::Start(start)).is_ok() {
        let _ = f.read_to_end(&mut tail);
    }
    if start > 0 {
        // Cut into a line: start after it.
        let cut = tail
            .iter()
            .position(|&b| b == b'\n')
            .map_or(tail.len(), |i| i + 1);
        tail.drain(..cut);
    }
    (first, String::from_utf8_lossy(&tail).into_owned())
}

/// The latest reading of `label` in the app's config folder `dir`: the last well-formed line.
pub fn latest(dir: &Path, label: &str) -> Option<Reading> {
    let (_, tail) = ends(&file(dir, label));
    tail.lines().rev().find_map(|l| parse_line(l, label))
}

/// Add `r` to its account's readings in the app's config folder `dir`, as one appended line;
/// true when written. Not written when the latest reading has the same windows and is under
/// `SAME_SECS` old at `now`. When the oldest line is past `KEEP_SECS` (or malformed), the file
/// is rewritten, atomically, with the readings of the last `KEPT_SECS` and this one. Two writers
/// at once each append whole lines; a rewrite can lose a line appended meanwhile, which the next
/// status line puts back.
pub fn record(dir: &Path, r: &Reading, now: i64) -> Result<bool, String> {
    if !accounts::is_label(&r.account) {
        return Err(format!("{:?} isn't an account label", r.account));
    }
    let path = file(dir, &r.account);
    let (first, tail) = ends(&path);
    let last = tail.lines().rev().find_map(|l| parse_line(l, &r.account));
    let rising;
    let r = match &last {
        Some(last) => {
            rising = r.rising_from(last);
            if last.windows() == rising.windows() && (0..SAME_SECS).contains(&(now - last.at)) {
                return Ok(false);
            }
            &rising
        }
        None => r,
    };
    let line = serde_json::to_string(r).map_err(|e| e.to_string())? + "\n";
    let prune =
        !first.is_empty() && parse_line(&first, &r.account).is_none_or(|f| now - f.at > KEEP_SECS);
    if prune {
        let text = fs::read_to_string(&path).unwrap_or_default();
        let mut kept: String = text
            .lines()
            .filter(|l| parse_line(l, &r.account).is_some_and(|x| now - x.at <= KEPT_SECS))
            .map(|l| format!("{l}\n"))
            .collect();
        kept.push_str(&line);
        return crate::queue::write_atomic(&path, &kept).map(|()| true);
    }
    fs::create_dir_all(dir.join(DIR)).map_err(|e| e.to_string())?;
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    f.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Hold the 7 days on read too, so an account nothing records for any more doesn't keep its
/// readings (`profiles::read` calls it): in the app's config folder `dir`, the readings of an
/// account not in `keep` (off the allowlist) go, and a file whose oldest reading is past
/// `KEEP_SECS` at `now` keeps only its last `KEPT_SECS`, or goes when that's none. Other files in
/// `usage/` are left alone. Like `record`'s rewrite, it can lose a line appended meanwhile.
pub fn prune(dir: &Path, keep: &[&str], now: i64) {
    let Ok(entries) = fs::read_dir(dir.join(DIR)) else {
        return;
    };
    for path in entries.flatten().map(|e| e.path()) {
        let Some(label) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".jsonl"))
            .filter(|l| accounts::is_label(l))
        else {
            continue;
        };
        if !keep.contains(&label) {
            let _ = fs::remove_file(&path);
            continue;
        }
        let (first, _) = ends(&path);
        if first.is_empty() || parse_line(&first, label).is_some_and(|f| now - f.at <= KEEP_SECS) {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap_or_default();
        let kept: String = text
            .lines()
            .filter(|l| parse_line(l, label).is_some_and(|x| now - x.at <= KEPT_SECS))
            .map(|l| format!("{l}\n"))
            .collect();
        if kept.is_empty() {
            let _ = fs::remove_file(&path);
        } else {
            let _ = crate::queue::write_atomic(&path, &kept);
        }
    }
}

/// What `muster accounts record --from statusline` records from the status line's stdin
/// `bytes`: its `rate_limits`, for the account `CLAUDE_CONFIG_DIR` (`config_dir`) names, when
/// that's an allowed account under `home` (`accounts::from_config_dir`); into the app's config
/// folder `dir`. Ok(false) when nothing was recorded: no account, the company one or one off the
/// list, no `rate_limits`, or the same as the latest.
pub fn record_statusline(
    bytes: &[u8],
    config_dir: Option<&Path>,
    settings: &Settings,
    home: &Path,
    dir: &Path,
    now: i64,
) -> Result<bool, String> {
    let Some(account) = config_dir.and_then(|d| accounts::from_config_dir(d, settings, home))
    else {
        return Ok(false);
    };
    let Some(w) = parse_statusline(bytes, now) else {
        return Ok(false);
    };
    record(
        dir,
        &Reading::new(account.label(), w, now, Source::Statusline),
        now,
    )
}

/// Local time at `secs` (seconds since 1970) as `YYYY-MM-DD HH:MM`, from `date`, as the tools'
/// stamps are. `tz` names another zone (tests); None is this Mac's.
pub fn local_time(secs: i64, tz: Option<&str>) -> Option<String> {
    let mut date = std::process::Command::new("date");
    date.args(["-r", &secs.to_string(), "+%Y-%m-%d %H:%M"]);
    if let Some(tz) = tz {
        date.env("TZ", tz);
    }
    let out = date.output().ok().filter(|o| o.status.success())?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (s.len() == 16).then_some(s)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A moment before the fixtures' resets (1792035600 and 1792346400), so they're in bounds
    /// and not stale whatever the clock says.
    pub(crate) const NOW: i64 = 1_792_030_000;

    fn fixture(name: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name);
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    fn w(used: f64, resets_at: i64) -> Option<Window> {
        Some(Window {
            used_percentage: used,
            resets_at,
        })
    }

    pub(crate) fn windows(five: f64, seven: f64) -> Windows {
        Windows {
            five_hour: w(five, 1_792_035_600),
            seven_day: w(seven, 1_792_346_400),
        }
    }

    /// Each line of a stream fixture through the parser.
    fn events(name: &str) -> Vec<Windows> {
        fixture(name)
            .lines()
            .filter_map(|l| parse_rate_limit_event(l, NOW))
            .collect()
    }

    #[test]
    fn the_stream_parser_reads_the_rate_limit_event_of_mu_111s_runs() {
        assert_eq!(
            events("stream/claude-rate-limit-allowed.jsonl"),
            [windows(35.0, 41.0)]
        );
        // Past the cap: kept as it is, 1.04 is 104 %, never dropped or clamped.
        assert_eq!(
            events("stream/claude-rate-limit-over-cap.jsonl"),
            [windows(104.0, 41.5)]
        );
        // A run on an account that isn't logged in has no rate_limit_event.
        assert!(events("stream/claude-not-logged-in.jsonl").is_empty());
    }

    #[test]
    fn the_status_line_parser_reads_only_rate_limits() {
        let f: serde_json::Value =
            serde_json::from_str(&fixture("statusline-rate-limits.json")).unwrap();
        // The status line's stdin as it comes, with the keys mu-111 saw beside rate_limits.
        let stdin = serde_json::json!({
            "cwd": "/Users/x/playground/a", "session_id": "s-1", "transcript_path": "/t.jsonl",
            "model": {"id": "claude-opus-5-5"}, "cost": {"total_cost_usd": 1.5},
            "rate_limits": f["after_first_response"]["rate_limits"],
        });
        assert_eq!(
            parse_statusline(stdin.to_string().as_bytes(), NOW),
            Some(windows(35.0, 41.0))
        );
        // Before the first response there's no rate_limits key.
        let before = serde_json::json!({"cwd": "/x", "session_id": "s-1", "version": "2.1.296"});
        assert_eq!(parse_statusline(before.to_string().as_bytes(), NOW), None);
        // Over the cap, one decimal: kept.
        assert_eq!(
            parse_statusline(
                fixture("statusline-rate-limits-over-cap.json").as_bytes(),
                NOW
            ),
            Some(windows(104.0, 41.5))
        );
        assert_eq!(parse_statusline(b"", NOW), None);
        assert_eq!(parse_statusline(b"not json", NOW), None);
        assert_eq!(parse_statusline(&[0xff, 0xfe, b'{'], NOW), None);
    }

    #[test]
    fn a_malformed_reading_or_window_is_ignored() {
        let five_years = NOW + 5 * YEAR_SECS;
        let stream = |five: &str, seven: &str| {
            format!(
                r#"{{"type":"rate_limit_event","rate_limit_info":{{"status":"allowed","unifiedWindows":{{"five_hour":{five},"seven_day":{seven}}}}}}}"#
            )
        };
        let good = r#"{"utilization":0.5,"resetsAt":1792035600}"#;
        for bad in [
            r#"{"utilization":"NaN","resetsAt":1792035600}"#.to_string(),
            r#"{"utilization":-0.1,"resetsAt":1792035600}"#.to_string(),
            r#"{"utilization":10.01,"resetsAt":1792035600}"#.to_string(),
            r#"{"utilization":0.5}"#.to_string(),
            r#"{"utilization":0.5,"resetsAt":"soon"}"#.to_string(),
            r#"{"utilization":0.5,"resetsAt":1792035600.5}"#.to_string(),
            format!(r#"{{"utilization":0.5,"resetsAt":{five_years}}}"#),
            r#"{"utilization":0.5,"resetsAt":-9223372036854775808}"#.to_string(),
            "null".to_string(),
            "[]".to_string(),
        ] {
            // A bad window beside a good one: the good one is kept.
            assert_eq!(
                parse_rate_limit_event(&stream(good, &bad), NOW),
                Some(Windows {
                    five_hour: w(50.0, 1_792_035_600),
                    seven_day: None
                }),
                "{bad}"
            );
            // No good window: no reading.
            assert_eq!(
                parse_rate_limit_event(&stream(&bad, &bad), NOW),
                None,
                "{bad}"
            );
        }
        for line in [
            "",
            "{",
            r#"{"type":"rate_limit_event"}"#,
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed"}}"#,
            r#"{"type":"rate_limit_event","rate_limit_info":{"unifiedWindows":{}}}"#,
            r#"{"type":"assistant","note":"rate_limit_event","rate_limit_info":{"unifiedWindows":{"five_hour":{"utilization":0.5,"resetsAt":1792035600}}}}"#,
            r#"{"type":7,"rate_limit_info":"rate_limit_event"}"#,
            // A number no f64 holds: the decoder refuses the whole line.
            r#"{"type":"rate_limit_event","rate_limit_info":{"unifiedWindows":{"five_hour":{"utilization":1e999,"resetsAt":1792035600}}}}"#,
        ] {
            assert_eq!(parse_rate_limit_event(line, NOW), None, "{line}");
        }
        // The status line: the same bounds.
        for bad in [
            r#"{"rate_limits":{"five_hour":{"used_percentage":-1,"resets_at":1792035600}}}"#,
            r#"{"rate_limits":{"five_hour":{"used_percentage":1000.1,"resets_at":1792035600}}}"#,
            r#"{"rate_limits":{"five_hour":{"used_percentage":"NaN","resets_at":1792035600}}}"#,
            r#"{"rate_limits":{"five_hour":{"used_percentage":40,"resets_at":1950000000}}}"#,
            r#"{"rate_limits":{"five_hour":{"used_percentage":40}}}"#,
            r#"{"rate_limits":null}"#,
            r#"{"rate_limits":{}}"#,
            r#"[1,2]"#,
        ] {
            assert_eq!(parse_statusline(bad.as_bytes(), NOW), None, "{bad}");
        }
        // The bounds themselves are in.
        let edge = format!(
            r#"{{"rate_limits":{{"five_hour":{{"used_percentage":0,"resets_at":{}}},"seven_day":{{"used_percentage":1000,"resets_at":{}}}}}}}"#,
            NOW + YEAR_SECS,
            NOW - YEAR_SECS
        );
        assert_eq!(
            parse_statusline(edge.as_bytes(), NOW),
            Some(Windows {
                five_hour: w(0.0, NOW + YEAR_SECS),
                seven_day: w(1000.0, NOW - YEAR_SECS),
            })
        );
    }

    #[test]
    fn a_window_is_stale_once_it_has_reset_and_headroom_is_what_is_left_above_the_reserve() {
        let win = Window {
            used_percentage: 35.0,
            resets_at: 1000,
        };
        assert!(!win.stale(999));
        assert!(win.stale(1000), "at its reset");
        assert!(win.stale(5000));
        assert_eq!(win.headroom(20), 45.0);
        assert_eq!(win.headroom(0), 65.0);
        let at = |used: f64| Window {
            used_percentage: used,
            resets_at: 0,
        };
        // Into the reserve: below 0.
        assert_eq!(at(90.0).headroom(20), -10.0);
        // Past the cap counts as 100.
        assert_eq!(at(104.0).headroom(20), -20.0);
        assert_eq!(at(0.0).headroom(90), 10.0);
        assert_eq!(at(41.5).headroom(20), 38.5);
    }

    fn lines(path: &Path) -> Vec<Reading> {
        fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn record_appends_a_line_and_skips_the_same_windows_within_five_minutes() {
        let d = crate::testkit::tempdir().unwrap();
        let r = |w: Windows, at: i64| Reading::new("mg", w, at, Source::Statusline);
        assert!(record(d.path(), &r(windows(35.0, 41.0), NOW), NOW).unwrap());
        let path = file(d.path(), "mg");
        assert_eq!(path, d.path().join("usage/mg.jsonl"));
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            format!(
                "{{\"account\":\"mg\",\"five_hour\":{{\"used_percentage\":35.0,\"resets_at\":1792035600}},\"seven_day\":{{\"used_percentage\":41.0,\"resets_at\":1792346400}},\"at\":{NOW},\"source\":\"statusline\"}}\n"
            )
        );
        // The same windows a minute later: not written.
        assert!(!record(d.path(), &r(windows(35.0, 41.0), NOW + 60), NOW + 60).unwrap());
        // New use: written.
        assert!(record(d.path(), &r(windows(36.0, 41.0), NOW + 90), NOW + 90).unwrap());
        // The same again after 5 minutes: written, so the reading's age stays true.
        let later = NOW + 90 + SAME_SECS;
        assert!(record(d.path(), &r(windows(36.0, 41.0), later), later).unwrap());
        let got = lines(&path);
        assert_eq!(got.len(), 3);
        assert_eq!(got[2].at, later);
        assert_eq!(latest(d.path(), "mg"), Some(got[2].clone()));
        // A run's reading, only one window.
        let one = Windows {
            five_hour: None,
            ..windows(0.0, 50.0)
        };
        let s = Reading::new("mg", one, later + 1, Source::Stream);
        assert!(record(d.path(), &s, later + 1).unwrap());
        assert!(!fs::read_to_string(&path)
            .unwrap()
            .lines()
            .last()
            .unwrap()
            .contains("five_hour"));
        assert_eq!(latest(d.path(), "mg"), Some(s));
        assert_eq!(latest(d.path(), "hg"), None, "no file, no reading");
    }

    #[test]
    fn record_keeps_seven_days_at_most_and_rewrites_rarely() {
        let d = crate::testkit::tempdir().unwrap();
        let path = file(d.path(), "mg");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let line = |at: i64, used: f64| {
            serde_json::to_string(&Reading::new(
                "mg",
                Windows {
                    five_hour: w(used, at + 3600),
                    seven_day: None,
                },
                at,
                Source::Stream,
            ))
            .unwrap()
                + "\n"
        };
        let day = 86400;
        // The oldest is 6.5 days old: appended to, nothing dropped.
        let young = line(NOW - 13 * day / 2, 1.0) + &line(NOW - day, 2.0);
        fs::write(&path, &young).unwrap();
        let r = Reading::new("mg", windows(3.0, 4.0), NOW, Source::Statusline);
        assert!(record(d.path(), &r, NOW).unwrap());
        assert_eq!(lines(&path).len(), 3);
        // The oldest is past 7 days: rewritten with the last 6 days only.
        let old = line(NOW - 8 * day, 1.0)
            + &line(NOW - 13 * day / 2, 2.0)
            + "not json\n"
            + &line(NOW - day, 3.0);
        fs::write(&path, &old).unwrap();
        assert!(record(d.path(), &r, NOW).unwrap());
        let ats: Vec<i64> = lines(&path).iter().map(|r| r.at).collect();
        assert_eq!(ats, [NOW - day, NOW]);
        // A malformed first line is dropped the same way.
        fs::write(
            &path,
            "{\"account\":\"mg\"\n".to_string() + &line(NOW - day, 3.0),
        )
        .unwrap();
        let r2 = Reading::new("mg", windows(9.0, 4.0), NOW, Source::Statusline);
        assert!(record(d.path(), &r2, NOW).unwrap());
        assert_eq!(lines(&path).len(), 2);
    }

    #[test]
    fn within_one_window_use_only_rises() {
        // Session A records 60 %; session B then re-renders its older 30 % of the same window.
        let d = crate::testkit::tempdir().unwrap();
        let r = |w: Windows, at: i64| Reading::new("mg", w, at, Source::Statusline);
        assert!(record(d.path(), &r(windows(60.0, 41.0), NOW), NOW).unwrap());
        assert!(!record(d.path(), &r(windows(30.0, 41.0), NOW + 10), NOW + 10).unwrap());
        assert_eq!(
            latest(d.path(), "mg").unwrap().windows(),
            windows(60.0, 41.0)
        );
        // Past SAME_SECS it's written again, with the higher use; the other window's rise is kept.
        let later = NOW + SAME_SECS;
        assert!(record(d.path(), &r(windows(30.0, 45.0), later), later).unwrap());
        assert_eq!(
            latest(d.path(), "mg").unwrap(),
            r(windows(60.0, 45.0), later)
        );
        // A new window (another reset) starts again from what it says.
        let next = Windows {
            five_hour: w(5.0, 1_792_035_600 + 5 * 3600),
            ..windows(0.0, 45.0)
        };
        assert!(record(d.path(), &r(next, later + 1), later + 1).unwrap());
        assert_eq!(latest(d.path(), "mg").unwrap().windows(), next);
    }

    #[test]
    fn prune_holds_the_readings_to_seven_days_and_drops_accounts_off_the_list() {
        let d = crate::testkit::tempdir().unwrap();
        let day = 86400;
        let line = |label: &str, at: i64| {
            serde_json::to_string(&Reading::new(label, windows(1.0, 2.0), at, Source::Stream))
                .unwrap()
                + "\n"
        };
        let usage = d.path().join(DIR);
        fs::create_dir_all(&usage).unwrap();
        // mg: one past 7 days, one inside them; hg: idle for 8 days; ih: off the list.
        fs::write(
            file(d.path(), "mg"),
            line("mg", NOW - 8 * day) + &line("mg", NOW - day),
        )
        .unwrap();
        fs::write(file(d.path(), "hg"), line("hg", NOW - 8 * day)).unwrap();
        fs::write(file(d.path(), "ih"), line("ih", NOW)).unwrap();
        // Young enough: left as it is, byte for byte.
        let young = line("xy", NOW - 6 * day) + "not json\n";
        fs::write(file(d.path(), "xy"), &young).unwrap();
        fs::write(usage.join("notes.txt"), "kept").unwrap();
        fs::write(usage.join("NotALabel.jsonl"), "kept").unwrap();
        prune(d.path(), &["mg", "hg", "xy"], NOW);
        assert_eq!(
            lines(&file(d.path(), "mg"))
                .iter()
                .map(|r| r.at)
                .collect::<Vec<_>>(),
            [NOW - day]
        );
        assert!(
            !file(d.path(), "hg").exists(),
            "nothing left in 7 days: the file goes"
        );
        assert!(
            !file(d.path(), "ih").exists(),
            "off the list: its readings go"
        );
        assert_eq!(fs::read_to_string(file(d.path(), "xy")).unwrap(), young);
        assert!(usage.join("notes.txt").exists() && usage.join("NotALabel.jsonl").exists());
        // No usage folder, or a file where it should be: nothing to do.
        prune(&d.path().join("none"), &[], NOW);
        let f = crate::testkit::tempdir().unwrap();
        fs::write(f.path().join(DIR), "a file").unwrap();
        prune(f.path(), &[], NOW);
        assert_eq!(fs::read_to_string(f.path().join(DIR)).unwrap(), "a file");
    }

    #[test]
    fn latest_skips_malformed_lines_and_other_accounts() {
        let d = crate::testkit::tempdir().unwrap();
        let path = file(d.path(), "mg");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let good = Reading::new("mg", windows(10.0, 20.0), NOW, Source::Stream);
        let text = serde_json::to_string(&good).unwrap()
            + "\n{\"account\":\"hg\",\"five_hour\":{\"used_percentage\":1.0,\"resets_at\":1792035600},\"at\":1792030000,\"source\":\"stream\"}\n"
            + "{\"account\":\"mg\",\"five_hour\":{\"used_percentage\":-5,\"resets_at\":1792035600},\"at\":1792030000,\"source\":\"stream\"}\n"
            + "{\"account\":\"mg\",\"at\":1792030000,\"source\":\"stream\"}\n"
            + "{\"account\":\"mg\",\"five_hour\":{\"used_percentage\":5,\"resets_at\":1792035600},\"at\":1792030000,\"source\":\"guess\"}\n"
            + "garbage\n\n";
        fs::write(&path, text).unwrap();
        assert_eq!(latest(d.path(), "mg"), Some(good));
        // A big file: only its end is read, from a whole line.
        let filler = "x".repeat(200) + "\n";
        let mut big = filler.repeat(1000);
        let last = Reading::new("mg", windows(77.0, 20.0), NOW + 5, Source::Statusline);
        big.push_str(&(serde_json::to_string(&last).unwrap() + "\n"));
        fs::write(&path, big).unwrap();
        assert_eq!(latest(d.path(), "mg"), Some(last));
    }

    #[test]
    fn record_refuses_a_label_that_isnt_one() {
        let d = crate::testkit::tempdir().unwrap();
        for bad in ["", "../x", ".claude", "MG"] {
            let r = Reading::new(bad, windows(1.0, 2.0), NOW, Source::Stream);
            assert!(record(d.path(), &r, NOW).is_err(), "{bad}");
        }
        assert!(!d.path().join(DIR).exists());
    }

    #[test]
    fn the_status_line_records_only_for_an_allowed_account_and_only_rate_limits() {
        let home = crate::accounts::tests::fake_home();
        let h = home.path();
        let dir = crate::testkit::tempdir().unwrap();
        let s = Settings::default();
        let stdin = serde_json::json!({
            "cwd": "/Users/x/playground/a", "session_id": "s-1", "transcript_path": "/t.jsonl",
            "model": {"id": "claude-opus-5-5", "display_name": "Opus"},
            "cost": {"total_cost_usd": 1.5}, "workspace": {"current_dir": "/Users/x"},
            "rate_limits": {
                "five_hour": {"used_percentage": 35, "resets_at": 1_792_035_600_i64},
                "seven_day": {"used_percentage": 41, "resets_at": 1_792_346_400_i64},
            },
        })
        .to_string();
        let rec = |config: Option<&Path>| {
            record_statusline(stdin.as_bytes(), config, &s, h, dir.path(), NOW).unwrap()
        };
        // The company account, one off the list, none: nothing.
        assert!(!rec(Some(&h.join(".claude"))));
        assert!(!rec(Some(&h.join(".claude-ih"))));
        assert!(!rec(None));
        assert!(!dir.path().join(DIR).exists(), "nothing written");
        assert!(rec(Some(&h.join(".claude-mg"))));
        let text = fs::read_to_string(file(dir.path(), "mg")).unwrap();
        let v: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        let mut keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(
            keys,
            ["account", "at", "five_hour", "seven_day", "source"],
            "{text}"
        );
        for private in [
            "cwd",
            "session",
            "transcript",
            "model",
            "cost",
            "Users",
            "opus",
        ] {
            assert!(!text.contains(private), "{private} in {text}");
        }
    }

    #[test]
    fn local_time_is_the_zones_own() {
        assert_eq!(
            local_time(1_792_035_600, Some("UTC")).as_deref(),
            Some("2026-10-15 03:40")
        );
        assert_eq!(
            local_time(1_792_035_600, Some("Pacific/Kiritimati")).as_deref(),
            Some("2026-10-15 17:40")
        );
    }
}
