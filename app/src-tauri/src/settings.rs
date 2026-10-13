//! Per-machine settings that both the app and `muster` read (task 0036): the claim lease and the
//! LIVE window. Saved as `settings.json` beside `projects.json` in the app's config folder
//! (`~/Library/Application Support/ai.sweetlemon.muster/`). A missing or broken file, or a value
//! out of bounds, reads as the default, field by field: a bad setting never stops the tools.

use crate::{ops, queue, transcripts};
use serde::Serialize;
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

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// A claim quiet for longer than this can be released to Ready (`muster reclaim`, Release).
    pub lease_minutes: u64,
    /// A session (the card's dot) or a task file (the pane's LIVE) that changed this recently is live.
    pub live_minutes: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            lease_minutes: ops::LEASE_MINUTES,
            live_minutes: LIVE_MINUTES,
        }
    }
}

impl Settings {
    /// The LIVE window in seconds, as the transcripts reader counts it.
    pub fn live_secs(&self) -> u64 {
        self.live_minutes * 60
    }
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
    }
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
                live_minutes: 10
            }
        );
        fs::write(&f, r#"{"leaseMinutes": 120, "liveMinutes": 61}"#).unwrap();
        assert_eq!(
            load(&f),
            Settings {
                lease_minutes: 120,
                live_minutes: 5
            }
        );
        // The bounds themselves are fine.
        fs::write(&f, r#"{"leaseMinutes": 60, "liveMinutes": 60}"#).unwrap();
        assert_eq!(
            load(&f),
            Settings {
                lease_minutes: 60,
                live_minutes: 60
            }
        );
        fs::write(&f, r#"{"leaseMinutes": 10080, "liveMinutes": 1}"#).unwrap();
        assert_eq!(
            load(&f),
            Settings {
                lease_minutes: 10080,
                live_minutes: 1
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
                live_minutes: 10
            }
        );
        assert_eq!(
            load(&f),
            Settings {
                lease_minutes: 120,
                live_minutes: 10
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
    fn spans_read_as_words() {
        assert_eq!(span_text(720), "12 h");
        assert_eq!(span_text(45), "45 min");
        assert_eq!(span_text(90), "1 h 30 min");
        assert_eq!(span_text(1800), "1 d 6 h");
        assert_eq!(span_text(10080), "7 d");
        assert_eq!(span_text(0), "0 min");
    }
}
