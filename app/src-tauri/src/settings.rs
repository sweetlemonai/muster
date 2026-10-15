//! Per-machine settings that both the app and `muster` read (task 0036): the claim lease, the
//! LIVE window, whether Mac notifications are on (0052), whether the menu bar icon is (0085) and the
//! Claude accounts Muster reads (0099, `accounts`); and ones only the app reads: the language
//! (mu-146), the digits and the calendar dates show in (mu-151), and whether it has offered to turn the old pauses into pauses (0059), so it offers
//! once. Saved as
//! `settings.json` beside `projects.json` in the app's config folder
//! (`~/Library/Application Support/ai.sweetlemon.muster/`). A missing or broken file, or a value
//! out of bounds, reads as the default, field by field: a bad setting never stops the tools.

use crate::{accounts, i18n, ops, queue, transcripts};
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

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// A claim quiet for longer than this can be released to Ready (`muster reclaim`, Release).
    pub lease_minutes: u64,
    /// A session (the card's dot) or a task file (the pane's LIVE) that changed this recently is live.
    pub live_minutes: u64,
    /// A Mac notification when something new waits for the owner (0052): on unless turned off.
    pub notify: bool,
    /// The menu bar icon (0085, `tray`): on unless turned off.
    pub menu_bar: bool,
    /// The app has offered, on load, to turn the old paused blocks into pauses (0059,
    /// `old_pause`); it offers once.
    pub old_pauses_offered: bool,
    /// The Claude accounts Muster reads (0099): labels, `mg` for `~/.claude-mg`, each valid
    /// (`accounts::is_label`), once. Never `~/.claude`. Empty: no account is read.
    pub claude_accounts: Vec<String>,
    /// The language picked in Settings (mu-146, `i18n::LOCALES`); None follows macOS's language
    /// for Muster (`i18n::resolve`). Written only when picked, so a file from before it is the
    /// same file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<&'static str>,
    /// The digits numbers show in (mu-151, `DIGITS`): `persian` (۱۲۳) or `latin` (123). None
    /// follows the language (Persian digits in فارسی). Only what people read; what they copy and
    /// what's stored stay Latin. Written only when picked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digits: Option<&'static str>,
    /// The calendar dates show in (mu-151, `CALENDARS`): `jalali` or `gregorian`. None follows
    /// the language (Jalali in فارسی). Stored dates stay Gregorian ISO. Written only when picked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calendar: Option<&'static str>,
}

/// The digits setting's values (mu-151), as settingsData.ts `DIGITS` has them.
pub const DIGITS: [&str; 2] = ["persian", "latin"];
/// The calendar setting's values (mu-151), as settingsData.ts `CALENDARS` has them.
pub const CALENDARS: [&str; 2] = ["jalali", "gregorian"];

/// `value` as one of `of`, or None.
fn one_of(of: &[&'static str], value: &str) -> Option<&'static str> {
    of.iter().copied().find(|v| *v == value)
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            lease_minutes: ops::LEASE_MINUTES,
            live_minutes: LIVE_MINUTES,
            notify: true,
            menu_bar: true,
            old_pauses_offered: false,
            claude_accounts: accounts::default_labels(),
            lang: None,
            digits: None,
            calendar: None,
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
        notify: v
            .get("notify")
            .and_then(|n| n.as_bool())
            .unwrap_or(d.notify),
        menu_bar: v
            .get("menuBar")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(d.menu_bar),
        old_pauses_offered: v
            .get("oldPausesOffered")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(d.old_pauses_offered),
        claude_accounts: account_labels(v.get("claudeAccounts")).unwrap_or(d.claude_accounts),
        lang: v
            .get("lang")
            .and_then(serde_json::Value::as_str)
            .and_then(i18n::stored),
        digits: v
            .get("digits")
            .and_then(serde_json::Value::as_str)
            .and_then(|d| one_of(&DIGITS, d)),
        calendar: v
            .get("calendar")
            .and_then(serde_json::Value::as_str)
            .and_then(|c| one_of(&CALENDARS, c)),
    }
}

/// The allowlist as stored: a list whose valid labels are kept, once and in order, and the rest
/// dropped (counted on stderr, never echoed: a hand-edited file could hold anything). None when
/// it isn't a list, so the default applies.
fn account_labels(v: Option<&serde_json::Value>) -> Option<Vec<String>> {
    let items = v?.as_array()?;
    let mut out: Vec<String> = Vec::new();
    let mut dropped = 0;
    for item in items {
        match item.as_str() {
            Some(l) if accounts::is_label(l) => {
                if !out.iter().any(|o| o == l) {
                    out.push(l.to_string());
                }
            }
            _ => dropped += 1,
        }
    }
    if dropped > 0 {
        eprintln!("muster: settings.json: {dropped} Claude account entries aren't valid labels and were dropped");
    }
    Some(out)
}

/// The allowlist as typed: labels separated by commas or spaces (`mg, hg`), each one valid, once.
/// Empty is an empty list: no account is read.
fn parse_accounts(value: &str) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for l in value.split(|c: char| c == ',' || c.is_whitespace()) {
        if l.is_empty() {
            continue;
        }
        if !accounts::is_label(l) {
            let l: String = l.chars().take(24).collect();
            return Err(format!(
                "{l:?} is not a valid account label: 1 to 12 lowercase letters and digits, like mg"
            ));
        }
        if !out.iter().any(|o| o == l) {
            out.push(l.to_string());
        }
    }
    Ok(out)
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
/// checked against its bounds, `notify` or `menu-bar` to `on` or `off` (empty is on),
/// `old-pauses-offered` to `yes` or `no` (empty is no), `claude-accounts` to a list of labels
/// (`mg, hg`; empty reads no account), `lang` to a language Muster has (empty follows
/// macOS), or `digits` (persian, latin) and `calendar` (jalali, gregorian), empty following the
/// language, and save. Returns the settings now in force.
pub fn set(file: &Path, name: &str, value: &str) -> Result<Settings, String> {
    if name == "claude-accounts" {
        let labels = parse_accounts(value)?;
        let mut s = load(file);
        s.claude_accounts = labels;
        save(file, &s)?;
        return Ok(s);
    }
    if name == "lang" {
        let code = value.trim();
        if !i18n::valid_setting(code) {
            let code: String = code.chars().take(24).collect();
            return Err(format!(
                "no language {code:?}: {}, or empty to follow macOS",
                i18n::LOCALES.join(" or ")
            ));
        }
        let mut s = load(file);
        s.lang = i18n::stored(code);
        save(file, &s)?;
        return Ok(s);
    }
    if name == "digits" || name == "calendar" {
        let of: &[&'static str] = if name == "digits" {
            &DIGITS
        } else {
            &CALENDARS
        };
        let v = value.trim();
        let picked = one_of(of, v);
        if picked.is_none() && !v.is_empty() {
            let v: String = v.chars().take(24).collect();
            return Err(format!(
                "{name} is {}, or empty to follow the language, not {v:?}",
                of.join(" or ")
            ));
        }
        let mut s = load(file);
        if name == "digits" {
            s.digits = picked;
        } else {
            s.calendar = picked;
        }
        save(file, &s)?;
        return Ok(s);
    }
    if name == "old-pauses-offered" {
        let offered = match value.trim() {
            "yes" => true,
            "no" | "" => false,
            other => {
                let other: String = other.chars().take(24).collect();
                return Err(format!("old-pauses-offered is yes or no, not {other:?}"));
            }
        };
        let mut s = load(file);
        s.old_pauses_offered = offered;
        save(file, &s)?;
        return Ok(s);
    }
    let d = Settings::default();
    if name == "notify" || name == "menu-bar" {
        let what = if name == "notify" {
            "notifications are"
        } else {
            "the menu bar icon is"
        };
        let on = match value.trim() {
            "" => true,
            "on" => true,
            "off" => false,
            other => {
                let other: String = other.chars().take(24).collect();
                return Err(format!("{what} on or off, not {other:?}"));
            }
        };
        let mut s = load(file);
        if name == "notify" {
            s.notify = on;
        } else {
            s.menu_bar = on;
        }
        save(file, &s)?;
        return Ok(s);
    }
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

    fn file(d: &crate::testkit::TestDir) -> std::path::PathBuf {
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
        let d = crate::testkit::tempdir().unwrap();
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
                notify: true,
                ..Default::default()
            }
        );
        fs::write(&f, r#"{"leaseMinutes": 120, "liveMinutes": 61}"#).unwrap();
        assert_eq!(
            load(&f),
            Settings {
                lease_minutes: 120,
                live_minutes: 5,
                notify: true,
                ..Default::default()
            }
        );
        // The bounds themselves are fine.
        fs::write(&f, r#"{"leaseMinutes": 60, "liveMinutes": 60}"#).unwrap();
        assert_eq!(
            load(&f),
            Settings {
                lease_minutes: 60,
                live_minutes: 60,
                notify: true,
                ..Default::default()
            }
        );
        fs::write(&f, r#"{"leaseMinutes": 10080, "liveMinutes": 1}"#).unwrap();
        assert_eq!(
            load(&f),
            Settings {
                lease_minutes: 10080,
                live_minutes: 1,
                notify: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn set_takes_a_span_checks_its_bounds_and_saves() {
        let d = crate::testkit::tempdir().unwrap();
        let f = file(&d);
        // The config folder is made on the first save.
        assert_eq!(set(&f, "lease", "2h").unwrap().lease_minutes, 120);
        assert_eq!(
            set(&f, "live", " 10 m ").unwrap(),
            Settings {
                lease_minutes: 120,
                live_minutes: 10,
                notify: true,
                ..Default::default()
            }
        );
        assert_eq!(
            load(&f),
            Settings {
                lease_minutes: 120,
                live_minutes: 10,
                notify: true,
                ..Default::default()
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
        let d = crate::testkit::tempdir().unwrap();
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
    fn notifications_are_on_unless_turned_off_and_a_bad_value_reads_as_on() {
        let d = crate::testkit::tempdir().unwrap();
        let f = file(&d);
        assert!(Settings::default().notify);
        assert!(!set(&f, "notify", "off").unwrap().notify);
        assert!(!load(&f).notify, "remembered");
        // Turning it off keeps the other settings, and changing them keeps it off.
        assert_eq!(set(&f, "lease", "2h").unwrap().lease_minutes, 120);
        assert!(!load(&f).notify);
        assert!(set(&f, "notify", " on ").unwrap().notify);
        set(&f, "notify", "off").unwrap();
        assert!(
            set(&f, "notify", "").unwrap().notify,
            "empty is the default"
        );
        let before = fs::read_to_string(&f).unwrap();
        assert_eq!(
            set(&f, "notify", "maybe").unwrap_err(),
            "notifications are on or off, not \"maybe\""
        );
        assert_eq!(fs::read_to_string(&f).unwrap(), before);
        for bad in [
            r#"{"notify": "off"}"#,
            r#"{"notify": 0}"#,
            r#"{"notify": null}"#,
        ] {
            fs::write(&f, bad).unwrap();
            assert!(load(&f).notify, "{bad}");
        }
        fs::write(&f, r#"{"notify": false, "liveMinutes": 61}"#).unwrap();
        assert_eq!(
            load(&f),
            Settings {
                notify: false,
                ..Settings::default()
            }
        );
    }

    #[test]
    fn the_old_pauses_offer_is_remembered_and_kept_by_the_other_settings() {
        let d = crate::testkit::tempdir().unwrap();
        let f = file(&d);
        assert!(!load(&f).old_pauses_offered, "not offered yet");
        assert!(
            set(&f, "old-pauses-offered", "yes")
                .unwrap()
                .old_pauses_offered
        );
        assert!(load(&f).old_pauses_offered, "remembered");
        // Changing the lease keeps it, and it keeps the lease.
        assert!(set(&f, "lease", "2h").unwrap().old_pauses_offered);
        assert_eq!(
            set(&f, "old-pauses-offered", "no").unwrap(),
            Settings {
                lease_minutes: 120,
                live_minutes: 5,
                notify: true,
                menu_bar: true,
                old_pauses_offered: false,
                claude_accounts: vec!["mg".into(), "hg".into()],
                lang: None,
                digits: None,
                calendar: None,
            }
        );
        set(&f, "old-pauses-offered", "yes").unwrap();
        assert!(
            !set(&f, "old-pauses-offered", " ")
                .unwrap()
                .old_pauses_offered
        );
        let before = fs::read_to_string(&f).unwrap();
        assert_eq!(
            set(&f, "old-pauses-offered", "maybe").unwrap_err(),
            "old-pauses-offered is yes or no, not \"maybe\""
        );
        assert_eq!(fs::read_to_string(&f).unwrap(), before);
        // As the window reads it; anything but true or false is the default.
        fs::write(&f, r#"{"oldPausesOffered": true}"#).unwrap();
        assert!(load(&f).old_pauses_offered);
        fs::write(&f, r#"{"oldPausesOffered": "yes"}"#).unwrap();
        assert!(!load(&f).old_pauses_offered);
        assert_eq!(
            serde_json::to_string(&Settings::default()).unwrap(),
            r#"{"leaseMinutes":720,"liveMinutes":5,"notify":true,"menuBar":true,"oldPausesOffered":false,"claudeAccounts":["mg","hg"]}"#
        );
    }

    #[test]
    fn the_menu_bar_icon_is_on_unless_turned_off_and_a_bad_value_reads_as_on() {
        let d = crate::testkit::tempdir().unwrap();
        let f = file(&d);
        assert!(Settings::default().menu_bar);
        assert!(load(&f).menu_bar, "no file: on");
        assert!(!set(&f, "menu-bar", "off").unwrap().menu_bar);
        assert!(!load(&f).menu_bar, "remembered");
        // It and the notifications are separate switches, and the others keep it off.
        assert!(load(&f).notify);
        assert!(!set(&f, "notify", "off").unwrap().menu_bar);
        assert!(!set(&f, "lease", "2h").unwrap().menu_bar);
        assert!(set(&f, "menu-bar", " on ").unwrap().menu_bar);
        assert!(!load(&f).notify, "turning it on keeps notifications off");
        set(&f, "menu-bar", "off").unwrap();
        assert!(
            set(&f, "menu-bar", "").unwrap().menu_bar,
            "empty is the default"
        );
        set(&f, "menu-bar", "off").unwrap();
        let before = fs::read_to_string(&f).unwrap();
        assert_eq!(
            set(&f, "menu-bar", "maybe").unwrap_err(),
            "the menu bar icon is on or off, not \"maybe\""
        );
        assert_eq!(fs::read_to_string(&f).unwrap(), before);
        assert!(before.contains("\"menuBar\": false"), "{before}");
        for bad in [
            r#"{"menuBar": "off"}"#,
            r#"{"menuBar": 0}"#,
            r#"{"menuBar": null}"#,
        ] {
            fs::write(&f, bad).unwrap();
            assert!(load(&f).menu_bar, "{bad}");
        }
        fs::write(&f, r#"{"menuBar": false, "leaseMinutes": 5}"#).unwrap();
        assert_eq!(
            load(&f),
            Settings {
                menu_bar: false,
                ..Settings::default()
            }
        );
    }

    #[test]
    fn the_claude_accounts_are_mg_and_hg_unless_set_and_bad_entries_are_dropped() {
        let d = crate::testkit::tempdir().unwrap();
        let f = file(&d);
        assert_eq!(
            load(&f).claude_accounts,
            ["mg", "hg"],
            "no file: the default"
        );
        assert_eq!(
            set(&f, "claude-accounts", "hg").unwrap().claude_accounts,
            ["hg"]
        );
        assert_eq!(load(&f).claude_accounts, ["hg"], "remembered");
        // Other settings keep it, and it keeps them.
        set(&f, "lease", "2h").unwrap();
        assert_eq!(load(&f).claude_accounts, ["hg"]);
        assert_eq!(
            set(&f, "claude-accounts", " mg, hg  mg ,")
                .unwrap()
                .claude_accounts,
            ["mg", "hg"],
            "commas or spaces, once each"
        );
        assert_eq!(load(&f).lease_minutes, 120);
        // Empty is an empty list, kept as one: no account is read.
        assert!(set(&f, "claude-accounts", "")
            .unwrap()
            .claude_accounts
            .is_empty());
        assert!(load(&f).claude_accounts.is_empty());
        set(&f, "claude-accounts", "mg").unwrap();
        // Refused, and nothing written.
        let before = fs::read_to_string(&f).unwrap();
        for bad in [".claude", "../x", "MG", "abcdefghijklm", "cc-mg", "mg hg/x"] {
            let e = set(&f, "claude-accounts", bad).unwrap_err();
            assert!(e.contains("is not a valid account label"), "{bad}: {e}");
        }
        assert_eq!(
            set(&f, "claude-accounts", ".claude").unwrap_err(),
            "\".claude\" is not a valid account label: 1 to 12 lowercase letters and digits, like mg"
        );
        assert_eq!(fs::read_to_string(&f).unwrap(), before);
        // A hand-edited file: the bad entries dropped, the good kept; not a list is the default.
        for (text, want) in [
            (
                r#"{"claudeAccounts": ["", ".claude", "mg", "ih", "MG", 7, null, "mg", "../x"]}"#,
                vec!["mg", "ih"],
            ),
            (r#"{"claudeAccounts": []}"#, vec![]),
            (r#"{"claudeAccounts": "mg"}"#, vec!["mg", "hg"]),
            (r#"{"claudeAccounts": null}"#, vec!["mg", "hg"]),
            (r#"{"claudeAccounts": {"a": "mg"}}"#, vec!["mg", "hg"]),
        ] {
            fs::write(&f, text).unwrap();
            assert_eq!(load(&f).claude_accounts, want, "{text}");
        }
    }

    #[test]
    fn the_language_follows_macos_until_picked_and_an_old_file_loads() {
        let d = crate::testkit::tempdir().unwrap();
        let f = file(&d);
        assert_eq!(Settings::default().lang, None, "unset: macOS's language");
        assert_eq!(load(&f).lang, None, "no file");
        // A file from before the language (mu-146): everything it has, the language unset.
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(
            &f,
            r#"{"leaseMinutes":120,"liveMinutes":10,"notify":false,"menuBar":true,"oldPausesOffered":true}"#,
        )
        .unwrap();
        assert_eq!(
            load(&f),
            Settings {
                lease_minutes: 120,
                live_minutes: 10,
                notify: false,
                menu_bar: true,
                old_pauses_offered: true,
                claude_accounts: vec!["mg".into(), "hg".into()],
                lang: None,
                digits: None,
                calendar: None,
            }
        );
        // Picked, saved and read back, with the others kept.
        let now = set(&f, "lang", "fa").unwrap();
        assert_eq!(
            (now.lang, now.lease_minutes, now.notify),
            (Some("fa"), 120, false)
        );
        assert_eq!(load(&f), now, "remembered");
        assert!(fs::read_to_string(&f).unwrap().contains("\"lang\": \"fa\""));
        // Changing another setting keeps it.
        assert_eq!(set(&f, "live", "5m").unwrap().lang, Some("fa"));
        assert_eq!(set(&f, "lang", " en ").unwrap().lang, Some("en"));
        // Empty goes back to following macOS, and the file says nothing of it.
        assert_eq!(set(&f, "lang", "").unwrap().lang, None);
        assert!(!fs::read_to_string(&f).unwrap().contains("lang"));
        // Only a language Muster has.
        set(&f, "lang", "fa").unwrap();
        let before = fs::read_to_string(&f).unwrap();
        assert_eq!(
            set(&f, "lang", "de").unwrap_err(),
            "no language \"de\": en or fa, or empty to follow macOS"
        );
        assert!(set(&f, "lang", "fa-IR").is_err());
        assert_eq!(
            fs::read_to_string(&f).unwrap(),
            before,
            "refused: unchanged"
        );
        // A value that isn't one reads as unset, field by field.
        for bad in [
            r#"{"lang": "de", "liveMinutes": 9}"#,
            r#"{"lang": 3, "liveMinutes": 9}"#,
            r#"{"lang": null, "liveMinutes": 9}"#,
            r#"{"lang": "FA", "liveMinutes": 9}"#,
        ] {
            fs::write(&f, bad).unwrap();
            assert_eq!((load(&f).lang, load(&f).live_minutes), (None, 9), "{bad}");
        }
    }

    #[test]
    fn the_language_is_sent_only_when_picked() {
        assert!(!serde_json::to_string(&Settings::default())
            .unwrap()
            .contains("lang"));
        let fa = Settings {
            lang: Some("fa"),
            ..Settings::default()
        };
        assert!(serde_json::to_string(&fa)
            .unwrap()
            .ends_with(r#""oldPausesOffered":false,"claudeAccounts":["mg","hg"],"lang":"fa"}"#));
    }

    #[test]
    fn digits_and_calendar_follow_the_language_until_picked_each_on_its_own() {
        let d = crate::testkit::tempdir().unwrap();
        let f = file(&d);
        let unset = Settings::default();
        assert_eq!((unset.digits, unset.calendar), (None, None));
        let text = serde_json::to_string(&unset).unwrap();
        assert!(
            !text.contains("digits") && !text.contains("calendar"),
            "{text}"
        );
        // Each saved alone, the other and the language kept.
        set(&f, "lang", "fa").unwrap();
        let now = set(&f, "digits", "latin").unwrap();
        assert_eq!(
            (now.lang, now.digits, now.calendar),
            (Some("fa"), Some("latin"), None)
        );
        let now = set(&f, "calendar", " gregorian ").unwrap();
        assert_eq!(
            (now.digits, now.calendar),
            (Some("latin"), Some("gregorian"))
        );
        assert_eq!(load(&f), now, "remembered");
        let saved = fs::read_to_string(&f).unwrap();
        assert!(
            saved.contains("\"digits\": \"latin\"")
                && saved.contains("\"calendar\": \"gregorian\""),
            "{saved}"
        );
        assert_eq!(set(&f, "lease", "2h").unwrap().digits, Some("latin"));
        assert_eq!(
            set(&f, "digits", "persian").unwrap().digits,
            Some("persian")
        );
        assert_eq!(
            set(&f, "calendar", "jalali").unwrap().calendar,
            Some("jalali")
        );
        // Empty follows the language again, and the file says nothing of it.
        let now = set(&f, "digits", "").unwrap();
        assert_eq!((now.digits, now.calendar), (None, Some("jalali")));
        assert!(!fs::read_to_string(&f).unwrap().contains("digits"));
        // Only the values there are, unchanged when refused.
        let before = fs::read_to_string(&f).unwrap();
        assert_eq!(
            set(&f, "digits", "arabic").unwrap_err(),
            "digits is persian or latin, or empty to follow the language, not \"arabic\""
        );
        assert_eq!(
            set(&f, "calendar", "Jalali").unwrap_err(),
            "calendar is jalali or gregorian, or empty to follow the language, not \"Jalali\""
        );
        assert_eq!(
            fs::read_to_string(&f).unwrap(),
            before,
            "refused: unchanged"
        );
        // A value that isn't one reads as unset, field by field.
        for bad in [
            r#"{"digits": "arabext", "calendar": 1, "liveMinutes": 9}"#,
            r#"{"digits": null, "calendar": "GREGORIAN", "liveMinutes": 9}"#,
            r#"{"digits": ["latin"], "calendar": "", "liveMinutes": 9}"#,
        ] {
            fs::write(&f, bad).unwrap();
            let s = load(&f);
            assert_eq!(
                (s.digits, s.calendar, s.live_minutes),
                (None, None, 9),
                "{bad}"
            );
        }
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
