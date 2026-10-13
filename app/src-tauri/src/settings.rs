//! Per-machine settings that both the app and `muster` read (task 0036): the claim lease, the
//! LIVE window and each project's Your check limit (0050). Saved as `settings.json` beside
//! `projects.json` in the app's config folder
//! (`~/Library/Application Support/ai.sweetlemon.muster/`). A missing or broken file, or a value
//! out of bounds, reads as the default, field by field: a bad setting never stops the tools.

use crate::{ops, queue, transcripts};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// The file's name, in the app's config folder.
pub const FILE: &str = "settings.json";

/// The lease's bounds, in minutes: sessions check in every 30 minutes, so a lease under an hour
/// would take tasks from sessions that follow the rules; over a week, a crashed session holds a
/// task for too long.
pub const LEASE_MIN: u64 = 60;
pub const LEASE_MAX: u64 = 7 * 1440;
/// The LIVE window's bounds, in minutes.
pub const LIVE_MIN: u64 = 1;
pub const LIVE_MAX: u64 = 60;
/// The default LIVE window, in minutes.
pub const LIVE_MINUTES: u64 = transcripts::LIVE_SECS / 60;
/// The Your check limit's bounds, in tasks, and its default (0050).
pub const YOUR_CHECK_MIN: usize = 1;
pub const YOUR_CHECK_MAX: usize = 50;
pub const YOUR_CHECK_LIMIT: usize = 5;

/// A project's Your check limit (0050): how many tasks may wait for the owner's check before
/// Muster warns, or Off. Saved as the number, or `"off"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    At(usize),
    Off,
}

impl Serialize for Limit {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Limit::At(n) => s.serialize_u64(*n as u64),
            Limit::Off => s.serialize_str("off"),
        }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// A claim quiet for longer than this can be released to Ready (`muster reclaim`, Release).
    pub lease_minutes: u64,
    /// A session (the card's dot) or a task file (the pane's LIVE) that changed this recently is live.
    pub live_minutes: u64,
    /// The Your check limit by project name (0050); a project not in it has the default.
    pub your_check_limits: BTreeMap<String, Limit>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            lease_minutes: ops::LEASE_MINUTES,
            live_minutes: LIVE_MINUTES,
            your_check_limits: BTreeMap::new(),
        }
    }
}

impl Settings {
    /// The LIVE window in seconds, as the transcripts reader counts it.
    pub fn live_secs(&self) -> u64 {
        self.live_minutes * 60
    }

    /// The project's Your check limit: its own, else the default; None when it's Off.
    pub fn your_check_limit(&self, project: &str) -> Option<usize> {
        match self.your_check_limits.get(project) {
            Some(Limit::At(n)) => Some(*n),
            Some(Limit::Off) => None,
            None => Some(YOUR_CHECK_LIMIT),
        }
    }
}

/// How many tasks wait in Your check, as the limit counts them (0050): agents' tasks, not goals
/// or features (theirs roll up from their tasks) and not the owner's own (`for: human`), which
/// skip Your check.
pub fn waiting(tasks: &[queue::Task]) -> usize {
    tasks
        .iter()
        .filter(|t| t.stage == "your-check" && t.level == "task" && !t.is_human())
        .count()
}

/// What `next`, `start`, `ready --parallel` and `list` say on stderr when more than `limit` tasks
/// wait in Your check (0050); None within it or with the limit Off. It never refuses anything.
pub fn over_line(waiting: usize, limit: Option<usize>) -> Option<String> {
    let limit = limit?;
    (waiting > limit).then(|| {
        format!("muster: {waiting} tasks wait in Your check (limit {limit}): the owner's review is the bottleneck.")
    })
}

/// A whole number of minutes within `min..=max`, or None.
fn bounded(v: Option<&serde_json::Value>, min: u64, max: u64) -> Option<u64> {
    v?.as_u64().filter(|n| (min..=max).contains(n))
}

/// The settings in `file`; the default for anything missing, broken or out of bounds.
pub fn load(file: &Path) -> Settings {
    let d = Settings::default();
    let Some(v) = fs::read_to_string(file)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
    else {
        return d;
    };
    Settings {
        lease_minutes: bounded(v.get("leaseMinutes"), LEASE_MIN, LEASE_MAX)
            .unwrap_or(d.lease_minutes),
        live_minutes: bounded(v.get("liveMinutes"), LIVE_MIN, LIVE_MAX).unwrap_or(d.live_minutes),
        // Project by project: a bad entry is left out, so that project has the default.
        your_check_limits: v
            .get("yourCheckLimits")
            .and_then(serde_json::Value::as_object)
            .map(|m| {
                m.iter()
                    .filter_map(|(name, v)| Some((name.clone(), limit(v)?)))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// A saved Your check limit: a whole number within the bounds, or `"off"`.
fn limit(v: &serde_json::Value) -> Option<Limit> {
    if v.as_str() == Some("off") {
        return Some(Limit::Off);
    }
    bounded(Some(v), YOUR_CHECK_MIN as u64, YOUR_CHECK_MAX as u64)
        .and_then(|n| usize::try_from(n).ok())
        .map(Limit::At)
}

fn save(file: &Path, s: &Settings) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    queue::write_atomic(
        file,
        &(serde_json::to_string_pretty(s).map_err(|e| e.to_string())? + "\n"),
    )
}

/// "12 h", "1 d 6 h", "45 min", "1 h 30 min": a span of minutes in words.
pub fn span_text(minutes: u64) -> String {
    let (d, h, m) = (minutes / 1440, minutes % 1440 / 60, minutes % 60);
    let parts: Vec<String> = [(d, "d"), (h, "h"), (m, "min")]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, u)| format!("{n} {u}"))
        .collect();
    if parts.is_empty() {
        "0 min".into()
    } else {
        parts.join(" ")
    }
}

/// A span as typed: a number and m, h or d (`45m`, `1.25h`, `2 d`; a day is 24 h), in minutes,
/// rounded to the minute. Read from the raw text, so `1.25h` is 75 minutes (estimates' cleaned
/// form keeps one decimal, which would make it 72). None for anything else, for no number above
/// zero, or for more than 1000 of the unit. `--after` and Settings both read spans this way.
pub fn span_minutes(input: &str) -> Option<u64> {
    let t = input.trim().to_lowercase().replace(' ', "");
    let (num, unit) = t.split_at(t.find(|c: char| c.is_ascii_alphabetic())?);
    let per = match unit {
        "m" | "min" | "mins" => 1.0,
        "h" | "hr" | "hrs" | "hour" | "hours" => 60.0,
        "d" | "day" | "days" => 1440.0,
        _ => return None,
    };
    if num.is_empty() || !num.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return None;
    }
    let n: f64 = num.parse().ok()?;
    // n is at most 1000, so this is at most 1,440,000 minutes.
    (n > 0.0 && n <= 1000.0).then(|| (n * per).round() as u64)
}

/// `span_minutes`, or why not, for the setting named `what`.
fn minutes(input: &str, what: &str) -> Result<u64, String> {
    span_minutes(input)
        .ok_or_else(|| format!("{what} {input:?}: use a number and m, h or d, like 30m, 2h or 1d"))
}

/// Change one setting, `lease` or `live`, to a span like `12h` or `5m` (empty is the default),
/// checked against its bounds, and save. Returns the settings now in force.
pub fn set(file: &Path, name: &str, value: &str) -> Result<Settings, String> {
    let d = Settings::default();
    let (what, min, max, default) = match name {
        "lease" => ("the lease", LEASE_MIN, LEASE_MAX, d.lease_minutes),
        "live" => ("the LIVE window", LIVE_MIN, LIVE_MAX, d.live_minutes),
        _ => return Err(format!("no setting named {name:?}")),
    };
    let m = if value.trim().is_empty() {
        default
    } else {
        minutes(value, what)?
    };
    if !(min..=max).contains(&m) {
        return Err(format!(
            "{what} can be from {} to {}, not {}",
            span_text(min),
            span_text(max),
            span_text(m)
        ));
    }
    let mut s = load(file);
    match name {
        "lease" => s.lease_minutes = m,
        _ => s.live_minutes = m,
    }
    save(file, &s)?;
    Ok(s)
}

/// Change `project`'s Your check limit (0050) to a number from 1 to 50 or `off` (any case);
/// empty is the default, so its entry goes. Returns the settings now in force.
pub fn set_your_check_limit(file: &Path, project: &str, value: &str) -> Result<Settings, String> {
    let project = project.trim();
    if project.is_empty() {
        return Err("which project? the name is empty".into());
    }
    let limit = match value.trim().to_lowercase().as_str() {
        "" => None,
        "off" => Some(Limit::Off),
        n => Some(
            n.parse::<usize>()
                .ok()
                .filter(|n| (YOUR_CHECK_MIN..=YOUR_CHECK_MAX).contains(n))
                .map(Limit::At)
                .ok_or_else(|| {
                    format!(
                        "the Your check limit can be from {YOUR_CHECK_MIN} to {YOUR_CHECK_MAX}, or off, not {value:?}"
                    )
                })?,
        ),
    };
    let mut s = load(file);
    match limit {
        Some(l) => s.your_check_limits.insert(project.to_string(), l),
        None => s.your_check_limits.remove(project),
    };
    save(file, &s)?;
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(d: &tempfile::TempDir) -> std::path::PathBuf {
        d.path().join("ai.sweetlemon.muster").join(FILE)
    }

    #[test]
    fn the_defaults_are_the_lease_and_live_window_the_tools_always_had() {
        let s = Settings::default();
        assert_eq!((s.lease_minutes, s.live_minutes), (12 * 60, 5));
        assert_eq!(s.live_secs(), transcripts::LIVE_SECS);
    }

    #[test]
    fn missing_or_broken_reads_as_the_defaults_field_by_field() {
        let d = tempfile::tempdir().unwrap();
        let f = file(&d);
        assert_eq!(load(&f), Settings::default(), "no file");
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        for broken in [
            "",
            "not json",
            "[1,2]",
            "{\"leaseMinutes\": \"2h\"}",
            "{\"leaseMinutes\": -5}",
            "{\"leaseMinutes\": 1.5}",
        ] {
            fs::write(&f, broken).unwrap();
            assert_eq!(load(&f), Settings::default(), "{broken}");
        }
        // Out of bounds: that field's default, the other kept.
        fs::write(&f, r#"{"leaseMinutes": 5, "liveMinutes": 10}"#).unwrap();
        assert_eq!(
            load(&f),
            Settings {
                lease_minutes: 720,
                live_minutes: 10,
                ..Settings::default()
            }
        );
        fs::write(&f, r#"{"leaseMinutes": 120, "liveMinutes": 61}"#).unwrap();
        assert_eq!(
            load(&f),
            Settings {
                lease_minutes: 120,
                live_minutes: 5,
                ..Settings::default()
            }
        );
        // The bounds themselves are fine.
        fs::write(&f, r#"{"leaseMinutes": 60, "liveMinutes": 60}"#).unwrap();
        assert_eq!(
            load(&f),
            Settings {
                lease_minutes: 60,
                live_minutes: 60,
                ..Settings::default()
            }
        );
        fs::write(&f, r#"{"leaseMinutes": 10080, "liveMinutes": 1}"#).unwrap();
        assert_eq!(
            load(&f),
            Settings {
                lease_minutes: 10080,
                live_minutes: 1,
                ..Settings::default()
            }
        );
    }

    #[test]
    fn set_takes_a_span_checks_its_bounds_and_saves() {
        let d = tempfile::tempdir().unwrap();
        let f = file(&d);
        // The config folder is made on the first save.
        assert_eq!(set(&f, "lease", "2h").unwrap().lease_minutes, 120);
        assert_eq!(
            set(&f, "live", " 10 m ").unwrap(),
            Settings {
                lease_minutes: 120,
                live_minutes: 10,
                ..Settings::default()
            }
        );
        assert_eq!(
            load(&f),
            Settings {
                lease_minutes: 120,
                live_minutes: 10,
                ..Settings::default()
            },
            "remembered"
        );
        assert_eq!(set(&f, "lease", "1.5d").unwrap().lease_minutes, 2160);
        assert_eq!(set(&f, "lease", "90min").unwrap().lease_minutes, 90);
        // As typed, never rounded to one decimal on the way (review of 0036).
        assert_eq!(set(&f, "lease", "1.25h").unwrap().lease_minutes, 75);
        assert_eq!(set(&f, "lease", "0.75d").unwrap().lease_minutes, 1080);
        assert_eq!(
            set(&f, "live", "2.5m").unwrap().live_minutes,
            3,
            "rounded to the minute"
        );
        // Empty is the default.
        assert_eq!(set(&f, "lease", "").unwrap().lease_minutes, 720);
        assert_eq!(set(&f, "live", "  ").unwrap().live_minutes, 5);
    }

    #[test]
    fn set_refuses_what_isnt_a_span_or_is_out_of_bounds_and_keeps_the_file() {
        let d = tempfile::tempdir().unwrap();
        let f = file(&d);
        set(&f, "lease", "3h").unwrap();
        let before = fs::read_to_string(&f).unwrap();
        assert_eq!(
            set(&f, "lease", "30m").unwrap_err(),
            "the lease can be from 1 h to 7 d, not 30 min"
        );
        assert_eq!(
            set(&f, "lease", "8d").unwrap_err(),
            "the lease can be from 1 h to 7 d, not 8 d"
        );
        assert_eq!(
            set(&f, "live", "2h").unwrap_err(),
            "the LIVE window can be from 1 min to 1 h, not 2 h"
        );
        assert_eq!(
            set(&f, "live", "20s").unwrap_err(),
            "the LIVE window \"20s\": use a number and m, h or d, like 30m, 2h or 1d"
        );
        assert!(set(&f, "lease", "12")
            .unwrap_err()
            .contains("use a number and m, h or d"));
        assert!(set(&f, "lease", "-2h").is_err());
        assert!(set(&f, "lease", "zwölf h").is_err());
        assert_eq!(
            set(&f, "poll", "2m").unwrap_err(),
            "no setting named \"poll\""
        );
        assert_eq!(
            fs::read_to_string(&f).unwrap(),
            before,
            "a refused change writes nothing"
        );
    }

    #[test]
    fn the_your_check_limit_is_5_per_project_unless_set() {
        let s = Settings::default();
        assert!(s.your_check_limits.is_empty());
        assert_eq!(s.your_check_limit("muster"), Some(5));
        let d = tempfile::tempdir().unwrap();
        let f = file(&d);
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(
            &f,
            r#"{"leaseMinutes": 120, "yourCheckLimits": {"muster": 8, "drover": "off", "germ": 1, "hood": 50}}"#,
        )
        .unwrap();
        let s = load(&f);
        assert_eq!(s.lease_minutes, 120);
        assert_eq!(s.your_check_limit("muster"), Some(8));
        assert_eq!(s.your_check_limit("drover"), None, "off");
        assert_eq!(s.your_check_limit("germ"), Some(1));
        assert_eq!(s.your_check_limit("hood"), Some(50));
        assert_eq!(s.your_check_limit("chit"), Some(5), "not set: the default");
    }

    #[test]
    fn a_bad_your_check_limit_reads_as_the_default_and_keeps_the_rest() {
        let d = tempfile::tempdir().unwrap();
        let f = file(&d);
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(
            &f,
            r#"{"liveMinutes": 10, "yourCheckLimits": {"a": 0, "b": 51, "c": -1, "d": 2.5, "e": "5", "f": "OFF", "g": null, "h": true, "i": [3], "ok": 7}}"#,
        )
        .unwrap();
        let s = load(&f);
        assert_eq!(s.live_minutes, 10, "the other settings stay");
        for p in ["a", "b", "c", "d", "e", "f", "g", "h", "i"] {
            assert_eq!(s.your_check_limit(p), Some(5), "{p}");
        }
        assert_eq!(s.your_check_limit("ok"), Some(7));
        assert_eq!(s.your_check_limits.len(), 1, "only the good one is kept");
        for broken in [r#"{"yourCheckLimits": 5}"#, r#"{"yourCheckLimits": "off"}"#] {
            fs::write(&f, broken).unwrap();
            assert_eq!(load(&f), Settings::default(), "{broken}");
        }
    }

    #[test]
    fn set_your_check_limit_takes_1_to_50_or_off_and_empty_is_the_default() {
        let d = tempfile::tempdir().unwrap();
        let f = file(&d);
        set(&f, "lease", "2h").unwrap();
        let s = set_your_check_limit(&f, "muster", " 8 ").unwrap();
        assert_eq!(s.your_check_limit("muster"), Some(8));
        assert_eq!(s.lease_minutes, 120, "the rest kept");
        assert_eq!(
            set_your_check_limit(&f, "drover", "Off")
                .unwrap()
                .your_check_limit("drover"),
            None
        );
        // Saved as the number or "off", which is how the file reads them back.
        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(
            saved["yourCheckLimits"],
            serde_json::json!({"drover": "off", "muster": 8})
        );
        assert_eq!(load(&f).your_check_limit("drover"), None);
        assert_eq!(load(&f).your_check_limit("muster"), Some(8));
        // The bounds, and empty for the default (its entry goes).
        assert_eq!(
            set_your_check_limit(&f, "muster", "1")
                .unwrap()
                .your_check_limit("muster"),
            Some(1)
        );
        assert_eq!(
            set_your_check_limit(&f, "muster", "50")
                .unwrap()
                .your_check_limit("muster"),
            Some(50)
        );
        let s = set_your_check_limit(&f, "muster", "").unwrap();
        assert_eq!(s.your_check_limit("muster"), Some(5));
        assert!(!s.your_check_limits.contains_key("muster"));
    }

    #[test]
    fn set_your_check_limit_refuses_the_rest_and_keeps_the_file() {
        let d = tempfile::tempdir().unwrap();
        let f = file(&d);
        set_your_check_limit(&f, "muster", "8").unwrap();
        let before = fs::read_to_string(&f).unwrap();
        for bad in ["0", "51", "-1", "2.5", "five", "on", "1e1"] {
            assert_eq!(
                set_your_check_limit(&f, "muster", bad).unwrap_err(),
                format!("the Your check limit can be from 1 to 50, or off, not {bad:?}")
            );
        }
        assert_eq!(
            set_your_check_limit(&f, " ", "5").unwrap_err(),
            "which project? the name is empty"
        );
        assert_eq!(fs::read_to_string(&f).unwrap(), before);
    }

    /// A task of `level`, `for` and `stage`, as the queue reads it.
    fn task(id: &str, stage: &str, level: &str, for_who: &str) -> queue::Task {
        let text =
            format!("---\nid: {id}\nstage: {stage}\nlevel: {level}\nfor: {for_who}\n---\n\n# T\n");
        queue::parse_task("q", Path::new("/q"), "", Path::new("/q/x.md"), &text, 0).unwrap()
    }

    #[test]
    fn only_agents_tasks_waiting_in_your_check_count() {
        let tasks = [
            task("0001", "your-check", "task", "ai"),
            task("0002", "your-check", "task", "ai"),
            // The owner's own task, a feature and a goal don't count; nor do other stages.
            task("0003", "your-check", "task", "human"),
            task("0004", "your-check", "feature", "ai"),
            task("0005", "your-check", "goal", "ai"),
            task("0006", "testing", "task", "ai"),
            task("0007", "done", "task", "ai"),
        ];
        assert_eq!(waiting(&tasks), 2);
        assert_eq!(waiting(&[]), 0);
    }

    #[test]
    fn the_over_line_says_how_many_and_the_limit_only_when_over() {
        assert_eq!(
            over_line(7, Some(5)).as_deref(),
            Some("muster: 7 tasks wait in Your check (limit 5): the owner's review is the bottleneck.")
        );
        assert_eq!(
            over_line(6, Some(5)).unwrap(),
            "muster: 6 tasks wait in Your check (limit 5): the owner's review is the bottleneck."
        );
        assert_eq!(over_line(5, Some(5)), None, "at the limit isn't over");
        assert_eq!(over_line(0, Some(5)), None);
        assert_eq!(over_line(40, None), None, "off");
    }

    #[test]
    fn spans_read_as_words() {
        assert_eq!(span_text(720), "12 h");
        assert_eq!(span_text(45), "45 min");
        assert_eq!(span_text(90), "1 h 30 min");
        assert_eq!(span_text(1800), "1 d 6 h");
        assert_eq!(span_text(10080), "7 d");
        assert_eq!(span_text(0), "0 min");
    }
}
